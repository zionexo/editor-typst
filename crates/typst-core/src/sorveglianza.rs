//! Sorveglianza dei file: watcher `notify`, elaborazione dei batch, scritture
//! registrate (F5-022, D-014/D-016).
//!
//! Il core traduce gli eventi di `notify` in [`EventoFs`], li raggruppa
//! ([`Batcher`]), legge i file, applica la macchina a stati e produce i
//! [`Cambio`] consolidati; **non** conosce Tauri né il frontend: emettere gli
//! eventi e agire sul World spetta al guscio (`src-tauri/src/osservatore.rs`).
//! Le decisioni pure sono in [`crate::esterni`].
//!
//! # Architettura dei watcher (deviazione da D-016, vedi rapporto)
//! Il piano prevedeva due watcher: `typst_kit::watcher::Watcher` sulle
//! dipendenze del World e questo sulla cartella. Il primo blocca in `wait()`
//! senza timeout né annullamento e non dice *quale* file è cambiato: cambiando
//! progetto resterebbe un thread appeso ai file del progetto precedente. Il
//! watcher ricorsivo sulla radice copre già lo stesso perimetro (file
//! inclusi/importati e risorse *del progetto*) più rinomine e albero, e si
//! sostituisce in modo pulito (drop del watcher → il thread termina). Restano
//! fuori le dipendenze esterne alla radice (pacchetti `@local`): coperte dal
//! comando manuale "Ricarica risorse".
//!
//! # Lock
//! [`Stato`] (registro scritture, macchina a stati, documento) sta sotto un
//! solo `Mutex`. Chi scrive un file lo tiene per tutta la scrittura e chi
//! legge per decidere lo tiene durante la lettura: non esiste finestra in cui
//! un evento veda il file scritto ma non ancora registrato. Il lock del World
//! si prende sempre **dopo** aver rilasciato [`Stato`] (mai insieme).
//!
//! # Salvataggio sicuro
//! [`scrivi_proprio`] non sovrascrive **mai** un documento il cui contenuto su
//! disco non è quello che l'editor ha visto l'ultima volta (`base` di
//! [`Osservatore`]): il confronto avviene sotto lo stesso lock della scrittura,
//! quindi vale anche per una modifica esterna non ancora elaborata dal watcher
//! e per un salvataggio che parte mentre è aperta la domanda "quale versione
//! tenere".
//!
//! Derivato da typst-kit 0.15.0 `src/watcher.rs` (commit
//! 3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright
//! Typst contributors: il filtro `e_rilevante` (`is_relevant_event_kind`).

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::cronologia::{ora_utc, Cronologia};
use crate::esterni::{
    da_rileggere, e_nome_di_backup, nuovo_nome_nella_forma_del_documento,
    percorso_fuori_dall_albero, percorso_ignorato, percorso_nella_forma_del_documento,
    riduci_eventi, ripristina_documento, scrivi_registrando, stesso_file, Batch, Batcher, Cambio,
    Esito, EsitoRipristino, EventoFs, Impronta, Integrita, Lettura, Osservatore, RegistroScritture,
    TipoEvento,
};

/// Stato condiviso dell'osservazione.
pub type Condiviso = Arc<Mutex<Stato>>;

/// Quante destinazioni "recenti" (file comparsi da poco) si ricordano per
/// abbinare una rinomina i cui eventi arrivano in batch diversi.
const MAX_RECENTI: usize = 16;

/// Età massima di un file "recente" per essere candidato a nuova casa del
/// documento rinominato. Il ricontrollo parte dopo
/// [`crate::esterni::RICONTROLLO_MS`] (150 ms) dall'evento: una rinomina vera
/// compare entro pochi secondi, mentre un file creato minuti prima con lo stesso
/// contenuto (copia, duplicato) non deve essere scambiato per il documento.
pub const ETA_MAX_RECENTE: Duration = Duration::from_secs(5);

/// Insieme condiviso dei file del progetto **effettivamente richiesti dalla
/// compilazione** (letti, o tentati e falliti, dal caricatore del `World`).
///
/// Il watcher osserva l'intera radice, ma a Typst interessano solo i file che
/// la compilazione ha toccato: un `.pdf` rigenerato da un visualizzatore o un
/// file di lock non devono invalidare la cache dei fratelli né provocare una
/// ricompilazione. Le chiavi sono percorsi normalizzati (maiuscole, separatori
/// e prefisso verbatim di Windows non contano). Clonare l'handle condivide l'insieme.
#[derive(Debug, Clone, Default)]
pub struct RisorseLette(Arc<Mutex<HashSet<String>>>);

impl RisorseLette {
    /// Insieme vuoto.
    pub fn nuovo() -> Self {
        Self::default()
    }

    fn blocca(&self) -> MutexGuard<'_, HashSet<String>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Registra che la compilazione ha richiesto `percorso` (anche se la
    /// lettura è fallita: la sua successiva comparsa deve invalidare).
    pub fn registra(&self, percorso: &Path) {
        let chiave = crate::cronologia::percorso_normalizzato(percorso);
        self.blocca().insert(chiave);
    }

    /// `true` se `percorso` è fra i file richiesti dalla compilazione.
    pub fn contiene(&self, percorso: &Path) -> bool {
        let chiave = crate::cronologia::percorso_normalizzato(percorso);
        self.blocca().contains(&chiave)
    }

    /// Svuota l'insieme (cambio di progetto).
    pub fn svuota(&self) {
        self.blocca().clear();
    }
}

/// Stato mutabile dell'osservazione.
#[derive(Default)]
pub struct Stato {
    /// Impronte delle scritture proprie (anti-loop).
    pub registro: RegistroScritture,
    /// Macchina a stati del documento aperto.
    pub osservatore: Osservatore,
    /// Documento aperto nell'editor (percorso su disco), se c'è.
    pub doc: Option<PathBuf>,
    /// Il documento ha modifiche non salvate (comunicato dal frontend).
    pub non_salvato: bool,
    /// Cartella osservata dal watcher.
    pub radice: Option<PathBuf>,
    /// Ultima radice su cui l'avvio del watcher è fallito: non si riprova a ogni
    /// chiamata (e a ogni battuta) finché la radice non cambia.
    pub radice_fallita: Option<PathBuf>,
    /// Watcher attivo; il drop lo ferma e fa terminare il thread degli eventi.
    pub watcher: Option<RecommendedWatcher>,
    /// Rinomine `(da, a)` del documento viste in un batch, la cui decisione è
    /// rimandata al ricontrollo (`da` ancora assente e `a` presente).
    rinomine_attese: Vec<(PathBuf, PathBuf)>,
    /// File comparsi da poco (creati o destinazione di una rinomina): candidati
    /// per una rinomina del documento i cui eventi Da/A stanno in batch diversi.
    /// Ogni voce porta l'istante in cui il file è comparso: oltre
    /// [`ETA_MAX_RECENTE`] non è più un candidato.
    recenti: Vec<(PathBuf, Instant)>,
    /// Ultima rinomina del documento seguita `(vecchio, nuovo)`. Finché il
    /// frontend non conferma il nuovo percorso, un `imposta_documento` col
    /// vecchio (inviato prima di sapere della rinomina) non deve far tornare
    /// indietro documento e macchina a stati.
    ultima_rinomina: Option<(PathBuf, PathBuf)>,
    /// File richiesti dalla compilazione (impostato dal guscio, condiviso col
    /// `World`). Se `None` non si filtra nulla: ogni file della radice che cambia
    /// invalida le risorse (comportamento prudente, usato dai test).
    pub letti: Option<RisorseLette>,
    /// Ultimo contenuto consegnato all'editor da `apri_file` `(percorso, impronta)`.
    /// Se all'aggancio il file è illeggibile è la base **vera** (ciò che l'editor
    /// ha davvero in mano): una versione diversa letta dopo è un Conflitto, non
    /// un contenuto da adottare in silenzio. Consumato dal primo aggancio del file.
    consegnato: Option<(PathBuf, Impronta)>,
}

/// Registra il contenuto che `apri_file` ha appena consegnato all'editor per
/// `percorso` (vedi [`Stato::consegnato`]). Sostituisce l'eventuale precedente.
pub fn segna_consegnato(condiviso: &Condiviso, percorso: &Path, contenuto: &[u8]) {
    blocca(condiviso).consegnato = Some((percorso.to_path_buf(), Impronta::di(contenuto)));
}

impl Stato {
    fn e_doc(&self, percorso: &Path) -> bool {
        self.doc
            .as_deref()
            .is_some_and(|d| stesso_file(d, percorso))
    }

    /// Scarta il contenuto consegnato se è dello STESSO file: una dichiarazione
    /// più recente (lettura, riconoscimento, scrittura, ripristino) è più
    /// autorevole di ciò che `apri_file` aveva consegnato, che altrimenti
    /// riporterebbe noto/base a una versione stantia al successivo aggancio.
    /// Il consegnato di un altro file resta intatto.
    fn azzera_consegnato(&mut self, percorso: &Path) {
        if self
            .consegnato
            .as_ref()
            .is_some_and(|(p, _)| stesso_file(p, percorso))
        {
            self.consegnato = None;
        }
    }

    fn ricorda_recente(&mut self, percorso: &Path, adesso: Instant) {
        if e_nome_di_backup(percorso) {
            return;
        }
        self.recenti.retain(|(r, _)| !stesso_file(r, percorso));
        self.recenti.push((percorso.to_path_buf(), adesso));
        if self.recenti.len() > MAX_RECENTI {
            self.recenti.remove(0);
        }
    }
}

/// Prende il lock di [`Stato`]. Nessun invariante si rompe a metà di una
/// sezione critica: si recupera dal lock avvelenato (stessa scelta di W-001
/// per il World).
pub fn blocca(c: &Condiviso) -> MutexGuard<'_, Stato> {
    c.lock().unwrap_or_else(|e| e.into_inner())
}

// ---- Traduzione degli eventi di notify ------------------------------------

// Derivato da typst-kit src/watcher.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
/// Se un tipo di evento è rilevante per il contenuto dei file.
fn e_rilevante(kind: &EventKind) -> bool {
    match kind {
        EventKind::Any => true,
        EventKind::Access(_) => false,
        EventKind::Create(_) => true,
        EventKind::Modify(kind) => match kind {
            notify::event::ModifyKind::Any => true,
            notify::event::ModifyKind::Data(_) => true,
            notify::event::ModifyKind::Metadata(_) => false,
            notify::event::ModifyKind::Name(_) => true,
            notify::event::ModifyKind::Other => false,
        },
        EventKind::Remove(_) => true,
        EventKind::Other => false,
    }
}

/// Traduce un evento di `notify` nel tipo astratto del core; `None` se irrilevante.
fn evento_da_notify(ev: &Event) -> Option<EventoFs> {
    use notify::event::{ModifyKind, RenameMode};
    // Overflow del buffer del watcher: eventi persi, non si sa cosa è cambiato.
    // Va controllato PRIMA degli altri filtri (il tipo può essere irrilevante e
    // i percorsi vuoti): `elabora` ricontrolla documento, risorse e albero.
    if ev.need_rescan() {
        return Some(EventoFs {
            tipo: TipoEvento::Rescan,
            percorsi: ev.paths.clone(),
        });
    }
    if !e_rilevante(&ev.kind) || ev.paths.is_empty() {
        return None;
    }
    let tipo = match &ev.kind {
        EventKind::Create(_) => TipoEvento::Creato,
        EventKind::Remove(_) => TipoEvento::Rimosso,
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => TipoEvento::RinominatoDa,
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => TipoEvento::RinominatoA,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if ev.paths.len() == 2 => {
            TipoEvento::RinominatoEntrambi
        }
        // Dati, `Any` e rinomine di cui non si sa il verso: il file si rilegge
        // dal disco e lo stato reale decide (esiste → cambiato, no → sparito).
        _ => TipoEvento::Modificato,
    };
    Some(EventoFs {
        tipo,
        percorsi: ev.paths.clone(),
    })
}

/// Ciclo del thread degli eventi: raggruppa con [`Batcher`] e consegna i batch a
/// `elabora`. Termina quando il watcher (l'unico mittente) viene eliminato.
fn ciclo(rx: Receiver<notify::Result<Event>>, mut elabora: impl FnMut(Vec<EventoFs>)) {
    let inizio = Instant::now();
    let ms = || inizio.elapsed().as_millis() as u64;
    let mut batcher = Batcher::nuovo();
    loop {
        let attesa = match batcher.scadenza() {
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(scadenza) => {
                let ora = ms();
                if ora >= scadenza {
                    elabora(batcher.svuota());
                    continue;
                }
                rx.recv_timeout(Duration::from_millis(scadenza - ora))
            }
        };
        match attesa {
            Ok(Ok(ev)) => {
                if let Some(e) = evento_da_notify(&ev) {
                    batcher.aggiungi(ms(), e);
                }
            }
            Ok(Err(e)) => eprintln!("Errore del watcher dei file: {e}"),
            Err(RecvTimeoutError::Timeout) => elabora(batcher.svuota()),
            Err(RecvTimeoutError::Disconnected) => {
                let resto = batcher.svuota();
                if !resto.is_empty() {
                    elabora(resto);
                }
                return;
            }
        }
    }
}

/// Avvia un watcher ricorsivo su `radice` e il thread che consegna i batch a
/// `elabora`. Il valore restituito tiene in vita il watcher: eliminarlo ferma tutto.
pub fn avvia_watcher(
    radice: &Path,
    elabora: impl FnMut(Vec<EventoFs>) + Send + 'static,
) -> notify::Result<RecommendedWatcher> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
    watcher.watch(radice, RecursiveMode::Recursive)?;
    std::thread::Builder::new()
        .name("watcher-file".into())
        .spawn(move || ciclo(rx, elabora))
        .map_err(notify::Error::io)?;
    Ok(watcher)
}

/// Aggiorna documento, cartella osservata e watcher (comando `osserva_documento`).
///
/// Comunica il documento aperto e se ha modifiche non salvate. Al cambio di
/// documento la macchina a stati riparte dal contenuto su disco; al cambio di
/// radice il watcher si ferma, il registro delle scritture si pota alle sole
/// voci sotto la nuova radice e `avvia` ne crea uno nuovo. Se l'avvio fallisce
/// la radice viene ricordata e **non** si riprova finché non cambia.
/// Ritorna `true` se un watcher è attivo.
pub fn imposta_documento(
    stato: &mut Stato,
    doc: Option<PathBuf>,
    non_salvato: bool,
    radice: Option<PathBuf>,
    avvia: impl FnOnce(&Path) -> notify::Result<RecommendedWatcher>,
) -> bool {
    // Rinomina appena seguita: il frontend può ancora comunicare il vecchio
    // percorso (richiesta partita prima dell'evento). Finché non arriva il nuovo
    // lo si tratta come il nuovo; qualunque altro percorso chiude la questione.
    let mut doc = doc;
    if let Some((vecchio, nuovo)) = stato.ultima_rinomina.clone() {
        match &doc {
            Some(d) if stesso_file(d, &vecchio) && stato.e_doc(&nuovo) => {
                doc = stato.doc.clone();
            }
            _ => stato.ultima_rinomina = None,
        }
    }
    let cambiato = match (&stato.doc, &doc) {
        (Some(a), Some(b)) => !stesso_file(a, b),
        (None, None) => false,
        _ => true,
    };
    if cambiato {
        if let Some(vecchio) = stato.doc.take() {
            stato.osservatore.dimentica(&vecchio);
        }
        // Contenuto consegnato all'editor per QUESTO file (consumato comunque).
        let consegnato = match stato.consegnato.take() {
            Some((p, i)) if doc.as_deref().is_some_and(|n| stesso_file(&p, n)) => Some(i),
            _ => None,
        };
        if let Some(nuovo) = &doc {
            match leggi(nuovo) {
                Lettura::Contenuto(i) => {
                    // `prev` = disco (coerente col watcher: nessun evento spurio);
                    // `base` = ciò che l'editor ha davvero in mano. Se il disco è
                    // già oltre il consegnato (scrittura esterna prima
                    // dell'aggancio) il salvataggio è un Conflitto, non una
                    // sovrascrittura silenziosa. Per i file normali coincidono.
                    stato.osservatore.imposta_noto(nuovo, Some(i));
                    if let Some(c) = consegnato.filter(|c| *c != i) {
                        stato.osservatore.imposta_base(nuovo, Some(c));
                    }
                }
                Lettura::Assente => stato.osservatore.imposta_noto(nuovo, None),
                // Lock breve (antivirus, sync) subito dopo l'apertura. Con il
                // contenuto consegnato la base è quella vera: una versione diversa
                // letta dopo è un Conflitto reale (nessuna sovrascrittura silenziosa
                // di una scrittura esterna non ancora elaborata dal watcher).
                Lettura::Illeggibile if consegnato.is_some() => {
                    stato.osservatore.imposta_noto(nuovo, consegnato);
                }
                // Senza: la base è "da stabilire". Il primo contenuto leggibile la
                // adotta (`scrivi_proprio`), senza un Conflitto spurio.
                Lettura::Illeggibile => stato.osservatore.imposta_noto_illeggibile(nuovo),
            }
        }
        stato.doc = doc;
        stato.rinomine_attese.clear();
    } else if doc.is_some() {
        // Stesso file ma magari scritto in altra forma (drive mappato, `subst`):
        // i percorsi emessi verso il frontend seguono la sua forma.
        // Riapertura dello stesso file: se `apri_file` ha consegnato il suo
        // contenuto, quello è ciò che l'editor ha (il disco può essere già a una
        // versione più nuova che il watcher non ha ancora elaborato) e diventa
        // il noto, evitando un "ricaricato" spurio all'evento tardivo.
        if let (Some(d), Some((p, i))) = (&doc, stato.consegnato.clone()) {
            if stesso_file(&p, d) {
                stato.consegnato = None;
                stato.osservatore.imposta_noto(d, Some(i));
            }
        }
        stato.doc = doc;
    }
    stato.non_salvato = non_salvato;

    if stato.radice != radice {
        stato.watcher = None; // ferma il vecchio (il suo thread termina)
        stato.radice = radice.clone();
        stato.radice_fallita = None;
        stato.recenti.clear();
        if let Some(r) = &radice {
            stato.registro.pota(r);
        }
    }
    if let Some(r) = &radice {
        if stato.watcher.is_none() && stato.radice_fallita.as_ref() != Some(r) {
            match avvia(r) {
                Ok(w) => stato.watcher = Some(w),
                Err(e) => {
                    eprintln!("Watcher non avviato su '{}': {e}", r.display());
                    stato.radice_fallita = Some(r.clone());
                }
            }
        }
    }
    stato.watcher.is_some()
}

// ---- Elaborazione dei batch ------------------------------------------------

/// Legge lo stato attuale di un file: `Assente` solo se non esiste (`NotFound`),
/// `Illeggibile` per qualunque altro errore (permessi, file bloccato da chi lo
/// sta scrivendo, è una cartella…): in quel caso non si conclude nulla.
pub fn leggi(percorso: &Path) -> Lettura {
    match std::fs::read(percorso) {
        Ok(byte) => Lettura::Contenuto(Impronta::di(&byte)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Lettura::Assente,
        Err(_) => Lettura::Illeggibile,
    }
}

/// Risultato di [`elabora`]/[`ricontrolla`]: cambi consolidati e ricontrolli
/// da programmare, con lo stato del documento **al momento della decisione**.
#[derive(Debug, Default)]
pub struct Elaborazione {
    /// Cambi già consolidati, da passare a [`crate::esterni::decidi`].
    pub cambi: Vec<Cambio>,
    /// `(percorso, generazione)` da ricontrollare fra
    /// [`crate::esterni::RICONTROLLO_MS`] con [`ricontrolla`].
    pub ricontrolli: Vec<(PathBuf, u64)>,
    /// Il batch contiene creazioni/rimozioni/rinomine: l'albero va aggiornato.
    pub albero: bool,
    /// Documento aperto prima che questa elaborazione lo cambiasse (una
    /// rinomina seguita aggiorna `Stato::doc` nella stessa sezione critica).
    pub doc: Option<PathBuf>,
    /// Il documento aveva modifiche non salvate.
    pub non_salvato: bool,
}

impl Elaborazione {
    fn da(stato: &Stato) -> Self {
        Self {
            doc: stato.doc.clone(),
            non_salvato: stato.non_salvato,
            ..Self::default()
        }
    }
}

/// Trasforma un batch di eventi in cambi consolidati, aggiornando registro e
/// macchina a stati (sotto lock, con letture dal disco).
pub fn elabora(stato: &mut Stato, eventi: &[EventoFs]) -> Elaborazione {
    elabora_a(stato, eventi, Instant::now())
}

/// Come [`elabora`] con l'istante `adesso` iniettato (test deterministici
/// dell'età dei file recenti).
pub fn elabora_a(stato: &mut Stato, eventi: &[EventoFs], adesso: Instant) -> Elaborazione {
    let mut out = Elaborazione::da(stato);
    let Some(radice) = stato.radice.clone() else {
        return out;
    };
    let doc = stato.doc.clone();
    let e_doc = |p: &Path| doc.as_deref().is_some_and(|d| stesso_file(d, p));
    // Nascosti/esclusi si ignorano, tranne il documento stesso (un file che
    // inizia per "." si osserva comunque).
    let ignora = |p: &Path| percorso_ignorato(&radice, p) && !e_doc(p);
    // Senza insieme dei file letti non si filtra nulla (comportamento prudente).
    let letti = stato.letti.clone();
    let letta = |p: &Path| letti.as_ref().is_none_or(|l| l.contiene(p));
    let mut batch = riduci_eventi(eventi);
    // Eventi persi (overflow): documento riletto come se fosse stato modificato
    // (la macchina a stati decide dal contenuto reale), cache dei file fratelli
    // invalidata (cambio su un percorso non-documento: `decidi` → InvalidaRisorse)
    // e albero da rileggere.
    let rescan = eventi.iter().any(|e| e.tipo == TipoEvento::Rescan);
    if rescan {
        if let Some(d) = doc.as_ref() {
            if !batch.toccati.iter().any(|p| stesso_file(p, d)) {
                batch.toccati.push(d.clone());
            }
        }
        out.cambi.push(Cambio::Contenuto(radice.clone()));
    }

    // File comparsi da poco (candidati di una rinomina spezzata in più batch).
    for e in eventi {
        let nuovi: &[PathBuf] = match e.tipo {
            TipoEvento::Creato | TipoEvento::RinominatoA => &e.percorsi,
            TipoEvento::RinominatoEntrambi => e.percorsi.get(1..).unwrap_or(&[]),
            _ => &[],
        };
        for p in nuovi {
            if !ignora(p) && !e_doc(p) {
                stato.ricorda_recente(p, adesso);
            }
        }
    }

    // Una rinomina con un solo capo nascosto/temporaneo è, per il capo visibile,
    // una creazione/modifica (salvataggio atomico di un altro editor) o una rimozione.
    let mut rinomine = Vec::new();
    for (da, a) in std::mem::take(&mut batch.rinomine) {
        match (ignora(&da), ignora(&a)) {
            (false, false) => rinomine.push((da, a)),
            (true, false) => batch.toccati.push(a),
            (false, true) => batch.rimossi.push(da),
            (true, true) => {}
        }
    }
    batch.toccati.retain(|p| !ignora(p));
    batch.rimossi.retain(|p| !ignora(p));

    // L'albero si rilegge solo per percorsi che vi compaiono: gli eventi in
    // cartelle nascoste solo nell'albero (`target/`) non lo invalidano.
    let nell_albero = |p: &PathBuf| !percorso_fuori_dall_albero(&radice, p);
    out.albero = rescan
        || (eventi.iter().any(|e| e.tipo != TipoEvento::Modificato)
            && (batch.toccati.iter().any(nell_albero)
                || batch.rimossi.iter().any(nell_albero)
                || rinomine
                    .iter()
                    .any(|(da, a)| nell_albero(da) || nell_albero(a))));

    // Rinomina del documento aperto: NON si decide qui. Il vecchio nome si
    // rilegge (se è sparito parte il ricontrollo) e la decisione "seguire il
    // nuovo nome" spetta a `ricontrolla`, a vecchio ancora assente e nuovo
    // presente: un editor che rinomina in backup e riscrive riporta il file al
    // suo posto e quella è una modifica di contenuto.
    let mut altre = Vec::new();
    for (da, a) in rinomine {
        if e_doc(&da) {
            if !e_nome_di_backup(&a) {
                stato.rinomine_attese.retain(|(d, _)| !stesso_file(d, &da));
                stato.rinomine_attese.push((da.clone(), a));
            }
            batch.toccati.push(da);
        } else {
            altre.push((da, a));
        }
    }
    let residuo = Batch {
        rinomine: altre,
        toccati: batch.toccati,
        rimossi: batch.rimossi,
    };

    for percorso in da_rileggere(&residuo) {
        if e_doc(&percorso) {
            let lettura = leggi(&percorso);
            match stato
                .osservatore
                .su_evento(&percorso, lettura, &mut stato.registro)
            {
                Esito::Modificato => out.cambi.push(Cambio::Contenuto(percorso)),
                Esito::Sparito => out.cambi.push(Cambio::Sparito(percorso)),
                Esito::Ricontrolla { generazione } => {
                    out.ricontrolli.push((percorso, generazione));
                }
                Esito::Nessuno => {}
            }
        } else if stato.registro.ha_voce(&percorso) {
            // Solo i file che l'app ha scritto si leggono, per riconoscerne l'eco.
            // Il filtro `letta` vale anche qui: un PDF esportato dall'app e poi
            // riscritto da un visualizzatore non è una risorsa della compilazione
            // e non deve farla ripartire.
            match leggi(&percorso) {
                Lettura::Contenuto(i) => {
                    if !stato.registro.e_propria(&percorso, &i) && letta(&percorso) {
                        out.cambi.push(Cambio::Contenuto(percorso));
                    }
                }
                Lettura::Assente => {
                    if letta(&percorso) {
                        out.cambi.push(Cambio::Sparito(percorso));
                    }
                }
                // Bloccato in questo momento (nessuna voce dell'osservatore su
                // cui ricontrollare): invalidazione prudente, solo se letto.
                Lettura::Illeggibile => {
                    if letta(&percorso) {
                        out.cambi.push(Cambio::Contenuto(percorso));
                    }
                }
            }
        } else if !letta(&percorso) {
            // Mai richiesto dalla compilazione (es. un `.pdf` rigenerato da un
            // visualizzatore, un file di lock): nulla da invalidare. L'albero
            // è già coperto da `out.albero`.
        } else if percorso.exists() {
            out.cambi.push(Cambio::Contenuto(percorso));
        } else {
            out.cambi.push(Cambio::Sparito(percorso));
        }
    }
    // Rinomine di altri file: solo invalidazione + albero (ma solo se almeno un
    // capo è fra i file letti dalla compilazione).
    for (da, a) in residuo.rinomine {
        if !e_doc(&a) && (letta(&da) || letta(&a)) {
            out.cambi.push(Cambio::Rinominato { da, a });
        }
    }
    out
}

/// Nuovo nome del documento `da` (sparito), se si può stabilire: la
/// destinazione di una rinomina vista in un batch (se esiste e non è un
/// backup) oppure l'unico file comparso entro [`ETA_MAX_RECENTE`] da `adesso`, **in entrambi i casi con lo
/// stesso contenuto** dell'ultima versione nota del documento: senza un
/// contenuto noto, o se differisce, non si segue (meglio "sparito" con verifica
/// d'integrità che seguire il file sbagliato).
fn destinazione_rinomina(stato: &Stato, da: &Path, adesso: Instant) -> Option<PathBuf> {
    let ultimo = stato.osservatore.ultimo_esistente(da)?;
    let valida = |a: &Path| {
        !e_nome_di_backup(a)
            && !stesso_file(a, da)
            && lunghezza_compatibile(a, &ultimo)
            && matches!(leggi(a), Lettura::Contenuto(i) if i == ultimo)
    };
    if let Some((_, a)) = stato
        .rinomine_attese
        .iter()
        .find(|(d, _)| stesso_file(d, da))
    {
        if valida(a) {
            return Some(a.clone());
        }
    }
    // Solo i file comparsi da poco: uno più vecchio di `ETA_MAX_RECENTE` non può
    // essere l'esito di questa rinomina.
    let mut candidati = stato
        .recenti
        .iter()
        .filter(|(_, quando)| adesso.saturating_duration_since(*quando) <= ETA_MAX_RECENTE)
        .map(|(r, _)| r)
        .filter(|r| valida(r));
    let primo = candidati.next()?;
    candidati.next().is_none().then(|| primo.clone())
}

/// `true` se `a` è un file regolare della stessa lunghezza di `ultimo`: filtro
/// economico (solo `metadata`) prima di leggere e calcolare l'hash, per non
/// leggere fino a 16 candidati sotto il lock di [`Stato`].
fn lunghezza_compatibile(a: &Path, ultimo: &Impronta) -> bool {
    std::fs::metadata(a).is_ok_and(|m| m.is_file() && m.len() == ultimo.lunghezza)
}

/// Ricontrollo programmato da un [`Esito::Ricontrolla`]: rilegge il disco e,
/// se l'ultimo evento conferma il cambiamento, ritorna il [`Cambio`].
///
/// Se il documento è ancora assente e si trova la sua nuova destinazione
/// ([`destinazione_rinomina`]) la rinomina viene **seguita**: `Stato::doc`
/// passa al nuovo percorso in questa stessa sezione critica, la macchina a
/// stati segue il nuovo file e la base "vista dall'editor" resta quella del
/// vecchio (se il contenuto è cambiato nel mezzo, il salvataggio lo rileverà).
pub fn ricontrolla(stato: &mut Stato, percorso: &Path, generazione: u64) -> Elaborazione {
    ricontrolla_a(stato, percorso, generazione, Instant::now())
}

/// Come [`ricontrolla`] con l'istante `adesso` iniettato (test deterministici).
pub fn ricontrolla_a(
    stato: &mut Stato,
    percorso: &Path,
    generazione: u64,
    adesso: Instant,
) -> Elaborazione {
    let mut out = Elaborazione::da(stato);
    let lettura = leggi(percorso);
    if lettura == Lettura::Assente
        && stato.osservatore.ricontrollo_valido(percorso, generazione)
        && stato.e_doc(percorso)
    {
        if let Some(nuovo) = destinazione_rinomina(stato, percorso, adesso) {
            let base = stato.osservatore.base(percorso);
            stato.osservatore.dimentica(percorso);
            let noto = match leggi(&nuovo) {
                Lettura::Contenuto(i) => Some(i),
                Lettura::Assente | Lettura::Illeggibile => None,
            };
            stato.osservatore.imposta_noto(&nuovo, noto);
            stato.osservatore.imposta_base(&nuovo, base);
            // Il documento resta nella forma di percorso del frontend (drive
            // mappato, `subst`, giunzione): il nuovo nome del watcher è canonico.
            // Via veloce lessicale per la rinomina nella stessa cartella (caso
            // comune, nessuna syscall sotto il lock); la via completa canonicalizza
            // gli antenati e resta solo per lo spostamento in un'altra cartella.
            let nuovo_fe = match stato.doc.as_deref() {
                Some(d) => nuovo_nome_nella_forma_del_documento(d, percorso, &nuovo)
                    .unwrap_or_else(|| percorso_nella_forma_del_documento(d, &nuovo, stesso_file)),
                None => nuovo.clone(),
            };
            stato.doc = Some(nuovo_fe);
            stato.ultima_rinomina = Some((percorso.to_path_buf(), nuovo.clone()));
            stato.rinomine_attese.clear();
            stato.recenti.retain(|(r, _)| !stesso_file(r, &nuovo));
            out.cambi.push(Cambio::Rinominato {
                da: percorso.to_path_buf(),
                a: nuovo,
            });
            return out;
        }
    }
    let Stato {
        osservatore,
        registro,
        ..
    } = stato;
    match osservatore.ricontrolla(percorso, generazione, lettura, registro) {
        Esito::Modificato => out.cambi.push(Cambio::Contenuto(percorso.to_path_buf())),
        Esito::Sparito => out.cambi.push(Cambio::Sparito(percorso.to_path_buf())),
        // Lettura transitoriamente illeggibile: si riprogramma (con tetto di
        // tentativi nell'osservatore) anche dal ricontrollo stesso.
        Esito::Ricontrolla { generazione } => {
            out.ricontrolli.push((percorso.to_path_buf(), generazione));
        }
        Esito::Nessuno => {}
    }
    out
}

// ---- Scritture proprie -----------------------------------------------------

/// Esito di [`scrivi_proprio`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsitoScrittura {
    /// Scritto (e registrato come scrittura propria).
    Scritto,
    /// **Non** scritto: il file su disco non è quello che l'editor ha visto
    /// (modifica esterna), oppure un salvataggio automatico su un file che non
    /// è il documento aperto.
    Conflitto,
    /// **Non** scritto: il documento è sparito dal disco (va verificata
    /// l'integrità del testo prima di ricrearlo, vedi [`ripristina_sparito`]).
    Sparito,
}

/// Scrittura registrata di un file (salvataggio, autosave, export): registra
/// l'impronta nel registro delle scritture proprie **prima** di scrivere, e la
/// scrittura è atomica.
///
/// Per il **documento aperto** confronta, sotto lo stesso lock, il disco con la
/// base vista dall'editor: se è diverso e non è un'eco registrata di una
/// nostra scrittura non scrive ([`EsitoScrittura::Conflitto`]); se il file è
/// sparito non scrive ([`EsitoScrittura::Sparito`]). Con `automatico` rifiuta
/// anche ogni percorso diverso dal documento aperto. Dopo una scrittura del
/// documento il contenuto noto/base diventano quelli scritti.
pub fn scrivi_proprio(
    condiviso: &Condiviso,
    percorso: &Path,
    contenuto: &[u8],
    automatico: bool,
) -> io::Result<EsitoScrittura> {
    let mut stato = blocca(condiviso);
    let e_doc = stato.e_doc(percorso);
    if automatico && !e_doc {
        return Ok(EsitoScrittura::Conflitto);
    }
    if e_doc {
        let base = stato.osservatore.base(percorso);
        match leggi(percorso) {
            Lettura::Assente if base.is_some() => return Ok(EsitoScrittura::Sparito),
            Lettura::Assente => {}
            Lettura::Contenuto(i) => {
                if Some(i) != base
                    && !stato.registro.e_propria(percorso, &i)
                    && !stato.osservatore.adotta_base_incerta(percorso, i)
                {
                    return Ok(EsitoScrittura::Conflitto);
                }
            }
            Lettura::Illeggibile => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "il file su disco non è leggibile in questo momento (in uso da un altro programma?): riprova",
                ));
            }
        }
    }
    scrivi_registrando(&mut stato.registro, percorso, contenuto)?;
    if e_doc {
        stato.azzera_consegnato(percorso);
        stato
            .osservatore
            .imposta_noto(percorso, Some(Impronta::di(contenuto)));
    }
    Ok(EsitoScrittura::Scritto)
}

/// Legge il testo del file dal disco (ricarica di un file modificato da fuori).
/// Se è il documento aperto, ciò che si è letto diventa il contenuto noto e la
/// base vista dall'editor: da quel momento un salvataggio non è un conflitto.
pub fn leggi_e_segna(condiviso: &Condiviso, percorso: &Path) -> io::Result<String> {
    let mut stato = blocca(condiviso);
    let byte = std::fs::read(percorso)?;
    // Validazione PRIMA di aggiornare noto/base: se il file non è UTF-8 (es.
    // riscritto da fuori in UTF-16) l'editor non riceve il testo, quindi il
    // riferimento visto non deve avanzare, altrimenti il prossimo salvataggio
    // sovrascriverebbe la versione esterna senza Conflitto.
    let testo =
        String::from_utf8(byte).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if stato.e_doc(percorso) {
        stato.azzera_consegnato(percorso);
        stato
            .osservatore
            .imposta_noto(percorso, Some(Impronta::di(testo.as_bytes())));
    }
    Ok(testo)
}

/// L'utente ha scelto di **tenere la versione dell'editor** (o ha scelto nel
/// dialog di sistema lo stesso file): lo stato attuale del disco diventa il
/// riferimento visto, così il prossimo salvataggio lo sovrascrive senza altri
/// conflitti. Se il disco cambia di nuovo dopo questa chiamata, il conflitto si
/// ripresenta.
///
/// La versione su disco che sta per essere scartata viene **prima** registrata
/// in cronologia (se è testo UTF-8): se non si riesce a registrarla l'operazione
/// fallisce e il riferimento non cambia, quindi il salvataggio resta bloccato.
pub fn riconosci_disco(
    condiviso: &Condiviso,
    percorso: &Path,
    cronologia: Option<&Cronologia>,
    max_versioni: usize,
) -> io::Result<()> {
    let mut stato = blocca(condiviso);
    if !stato.e_doc(percorso) {
        return Ok(());
    }
    let byte = match std::fs::read(percorso) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "il file su disco non è leggibile in questo momento: riprova",
            ));
        }
    };
    if let (Some(c), Some(b)) = (cronologia, &byte) {
        if let Ok(testo) = std::str::from_utf8(b) {
            c.registra(percorso, testo, ora_utc(), max_versioni, None)?;
        }
    }
    let noto = byte.as_deref().map(Impronta::di);
    stato.azzera_consegnato(percorso);
    stato.osservatore.imposta_noto(percorso, noto);
    Ok(())
}

/// Esito del ripristino di un documento sparito, per il frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsitoSparito {
    /// File ricreato al percorso originale.
    Ripristinato,
    /// Il file esiste di nuovo (o non è leggibile): nulla da fare, mai sovrascriverlo.
    Esiste,
    /// La cartella che conteneva il file non esiste più: non si ricrea
    /// un'intera gerarchia, si chiede dove salvare.
    CartellaAssente,
    /// Testo non integro: nessun salvataggio, chiedere dove salvare.
    NonIntegro(Integrita),
}

/// Ripristina al percorso originale il documento sparito dal disco, se il testo
/// dell'editor è integro (D-014): voce in cronologia, poi scrittura atomica
/// registrata come propria. Non scrive mai se il file esiste di nuovo, né se la
/// cartella genitrice non c'è più.
pub fn ripristina_sparito(
    condiviso: &Condiviso,
    percorso: &Path,
    contenuto: &str,
    cronologia: Option<&Cronologia>,
    max_versioni: usize,
) -> io::Result<EsitoSparito> {
    let mut stato = blocca(condiviso);
    // Controllo dentro il lock: chi ricrea il file da fuori non viene sovrascritto.
    if leggi(percorso) != Lettura::Assente {
        return Ok(EsitoSparito::Esiste);
    }
    match percorso.parent() {
        Some(p) if !p.as_os_str().is_empty() && !p.is_dir() => {
            return Ok(EsitoSparito::CartellaAssente);
        }
        None => return Ok(EsitoSparito::CartellaAssente),
        Some(_) => {}
    }
    let non_vuoto = stato
        .osservatore
        .ultimo_esistente(percorso)
        .is_some_and(|i| !i.vuota());
    let esito = ripristina_documento(
        percorso,
        contenuto,
        non_vuoto,
        cronologia,
        ora_utc(),
        max_versioni,
        &mut stato.registro,
    )?;
    Ok(match esito {
        EsitoRipristino::Ripristinato => {
            stato.azzera_consegnato(percorso);
            stato
                .osservatore
                .imposta_noto(percorso, Some(Impronta::di(contenuto.as_bytes())));
            EsitoSparito::Ripristinato
        }
        EsitoRipristino::NonIntegro(motivo) => EsitoSparito::NonIntegro(motivo),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::mpsc;

    fn attendi_batch(
        rx: &mpsc::Receiver<Vec<EventoFs>>,
        finche: impl Fn(&[EventoFs]) -> bool,
    ) -> Vec<EventoFs> {
        let mut visti: Vec<EventoFs> = Vec::new();
        let limite = Instant::now() + Duration::from_secs(10);
        while Instant::now() < limite {
            if let Ok(b) = rx.recv_timeout(Duration::from_millis(200)) {
                visti.extend(b);
                if finche(&visti) {
                    return visti;
                }
            }
        }
        panic!("evento atteso non arrivato entro 10 s; visti: {visti:?}");
    }

    fn tocca(eventi: &[EventoFs], nome: &str) -> bool {
        eventi.iter().any(|e| {
            e.percorsi
                .iter()
                .any(|p| p.file_name().is_some_and(|n| n == nome))
        })
    }

    fn stato_con_doc(radice: &Path, doc: &Path, contenuto: &[u8]) -> Condiviso {
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        {
            let mut s = blocca(&condiviso);
            s.radice = Some(radice.to_path_buf());
            s.doc = Some(doc.to_path_buf());
            s.osservatore
                .imposta_noto(doc, Some(Impronta::di(contenuto)));
        }
        condiviso
    }

    fn ev(tipo: TipoEvento, p: &Path) -> EventoFs {
        EventoFs {
            tipo,
            percorsi: vec![p.to_path_buf()],
        }
    }

    #[test]
    fn filtro_eventi_rilevanti() {
        use notify::event::{
            AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind, RenameMode,
        };
        assert!(!e_rilevante(&EventKind::Access(AccessKind::Any)));
        assert!(!e_rilevante(&EventKind::Modify(ModifyKind::Metadata(
            MetadataKind::Any
        ))));
        assert!(!e_rilevante(&EventKind::Other));
        assert!(e_rilevante(&EventKind::Create(CreateKind::File)));
        assert!(e_rilevante(&EventKind::Remove(RemoveKind::File)));
        assert!(e_rilevante(&EventKind::Modify(ModifyKind::Data(
            DataChange::Any
        ))));
        assert!(e_rilevante(&EventKind::Modify(ModifyKind::Name(
            RenameMode::From
        ))));
    }

    #[test]
    fn traduzione_delle_rinomine() {
        use notify::event::{ModifyKind, RenameMode};
        let mk = |modo, paths: &[&str]| {
            let mut e = Event::new(EventKind::Modify(ModifyKind::Name(modo)));
            for p in paths {
                e = e.add_path(PathBuf::from(p));
            }
            evento_da_notify(&e)
        };
        assert_eq!(
            mk(RenameMode::From, &["/a"]).map(|e| e.tipo),
            Some(TipoEvento::RinominatoDa)
        );
        assert_eq!(
            mk(RenameMode::To, &["/b"]).map(|e| e.tipo),
            Some(TipoEvento::RinominatoA)
        );
        assert_eq!(
            mk(RenameMode::Both, &["/a", "/b"]).map(|e| e.tipo),
            Some(TipoEvento::RinominatoEntrambi)
        );
        assert_eq!(
            mk(RenameMode::Any, &["/a"]).map(|e| e.tipo),
            Some(TipoEvento::Modificato)
        );
        assert!(
            mk(RenameMode::From, &[]).is_none(),
            "senza percorsi non c'è nulla da fare"
        );
    }

    /// Prova con il watcher reale della piattaforma: creazione, rinomina
    /// (abbinata nel batch) e cancellazione arrivano fino a `riduci_eventi`.
    // Il backend nativo (ReadDirectoryChangesW / inotify) è affidabile in un test
    // solo su Windows e Linux; su macOS (FSEvents) i tempi variano troppo.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn watcher_reale_crea_rinomina_cancella() {
        let dir = tempfile::tempdir().expect("tempdir");
        let radice = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let (tx, rx) = mpsc::channel();
        let _watcher = avvia_watcher(&radice, move |b| {
            let _ = tx.send(b);
        })
        .expect("watcher");
        std::thread::sleep(Duration::from_millis(300)); // il backend si arma

        let f = radice.join("capitolo.typ");
        std::fs::write(&f, "uno").expect("crea");
        attendi_batch(&rx, |v| tocca(v, "capitolo.typ"));

        std::thread::sleep(Duration::from_millis(300));
        let nuovo = radice.join("rinominato.typ");
        std::fs::rename(&f, &nuovo).expect("rinomina");
        let visti = attendi_batch(&rx, |v| tocca(v, "rinominato.typ"));
        let ridotto = riduci_eventi(&visti);
        assert!(
            ridotto
                .rinomine
                .iter()
                .any(|(da, a)| da.ends_with("capitolo.typ") && a.ends_with("rinominato.typ")),
            "rinomina non abbinata: {ridotto:?} da {visti:?}"
        );

        std::thread::sleep(Duration::from_millis(300));
        std::fs::remove_file(&nuovo).expect("rimuovi");
        attendi_batch(&rx, |v| {
            riduci_eventi(v)
                .rimossi
                .iter()
                .any(|p| p.ends_with("rinominato.typ"))
        });
    }

    #[test]
    fn drop_del_watcher_termina_il_thread() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (tx, rx) = mpsc::channel::<()>();
        let watcher = avvia_watcher(dir.path(), move |_| {
            let _ = tx.send(());
        })
        .expect("watcher");
        drop(watcher);
        // Il thread possiede la closure (e il mittente `tx`): quando esce la
        // closure viene rilasciata e il canale si chiude.
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    fn radice_temp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let radice = std::fs::canonicalize(dir.path()).expect("canonicalize");
        (dir, radice)
    }

    #[test]
    fn elabora_ignora_eco_propria_e_rileva_modifica_esterna() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");

        // Scrittura propria (autosave): nessun cambio, anche con eventi ripetuti.
        let esito = scrivi_proprio(&condiviso, &doc, b"salvato dall'app", true).expect("scrittura");
        assert_eq!(esito, EsitoScrittura::Scritto);
        let el = elabora(
            &mut blocca(&condiviso),
            &[
                ev(TipoEvento::Modificato, &doc),
                ev(TipoEvento::Modificato, &doc),
            ],
        );
        assert!(
            el.cambi.is_empty() && el.ricontrolli.is_empty(),
            "eco ignorata"
        );

        // Scrittura esterna: stesso percorso, contenuto diverso.
        std::fs::write(&doc, "da un altro programma").expect("esterna");
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        assert_eq!(el.cambi, vec![Cambio::Contenuto(doc.clone())]);

        // Temporanei nascosti (salvataggio atomico): ignorati.
        let tmp = radice.join(".main.typ.tmp-1-2");
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Creato, &tmp)]);
        assert!(el.cambi.is_empty());
    }

    #[test]
    fn documento_con_nome_che_inizia_per_punto_si_osserva() {
        let (_d, radice) = radice_temp();
        let doc = radice.join(".note.typ");
        std::fs::write(&doc, "uno").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"uno");
        std::fs::write(&doc, "due").expect("esterna");
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        assert_eq!(el.cambi, vec![Cambio::Contenuto(doc)]);
    }

    #[test]
    fn file_illeggibile_non_e_sparito_e_non_cambia_stato() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "testo").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        // Al posto del file c'è una cartella: esiste ma non si legge.
        std::fs::remove_file(&doc).expect("rimuovi");
        std::fs::create_dir(&doc).expect("cartella");
        assert_eq!(leggi(&doc), Lettura::Illeggibile);
        assert_eq!(leggi(&radice.join("non-esiste.typ")), Lettura::Assente);

        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        // Non è "sparito" né modificato; è transitorio: si ricontrolla...
        assert!(el.cambi.is_empty(), "{el:?}");
        assert_eq!(el.ricontrolli.len(), 1, "{el:?}");
        // ...ma con un tetto di tentativi (qui l'errore è permanente).
        let (percorso, generazione) = el.ricontrolli[0].clone();
        let mut tentativi = 1;
        loop {
            let el = ricontrolla(&mut blocca(&condiviso), &percorso, generazione);
            assert!(el.cambi.is_empty(), "{el:?}");
            if el.ricontrolli.is_empty() {
                break;
            }
            tentativi += 1;
            assert!(tentativi <= 100, "ciclo infinito");
        }
        assert_eq!(
            tentativi,
            usize::from(crate::esterni::MAX_TENTATIVI_ILLEGGIBILE)
        );
        let s = blocca(&condiviso);
        assert_eq!(s.osservatore.noto(&doc), Some(Impronta::di(b"testo")));
    }

    // ---- rinomina del documento (C2) ----

    /// Sposta il documento e ritorna lo stato pronto per gli eventi.
    fn doc_rinominato(
        contenuto: &str,
        nuovo_contenuto: &str,
    ) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, Condiviso) {
        let (d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, contenuto).expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, contenuto.as_bytes());
        let nuovo = radice.join("nuovo.typ");
        std::fs::rename(&doc, &nuovo).expect("rinomina");
        if nuovo_contenuto != contenuto {
            std::fs::write(&nuovo, nuovo_contenuto).expect("contenuto diverso");
        }
        (d, radice, doc, nuovo, condiviso)
    }

    #[test]
    fn rinomina_abbinata_nello_stesso_batch_si_segue_al_ricontrollo() {
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "testo");
        let eventi = [
            ev(TipoEvento::RinominatoDa, &doc),
            ev(TipoEvento::RinominatoA, &nuovo),
        ];
        let el = elabora(&mut blocca(&condiviso), &eventi);
        assert!(
            !el.cambi
                .iter()
                .any(|c| matches!(c, Cambio::Rinominato { .. })),
            "la decisione è rimandata al ricontrollo: {:?}",
            el.cambi
        );
        assert_eq!(el.ricontrolli.len(), 1);
        let (p, g) = el.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(
            el.cambi,
            vec![Cambio::Rinominato {
                da: doc.clone(),
                a: nuovo.clone()
            }]
        );
        // Il documento è già il nuovo nel core; lo snapshot dice com'era prima.
        assert_eq!(el.doc.as_deref(), Some(doc.as_path()));
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(nuovo.as_path()));
    }

    #[test]
    fn rinomina_con_da_e_a_in_batch_diversi_si_segue_se_il_contenuto_coincide() {
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "testo");
        // Batch 1: solo il vecchio nome (Windows: Name(From) e Name(To) separati).
        let el1 = elabora(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::RinominatoDa, &doc)],
        );
        assert_eq!(el1.ricontrolli.len(), 1);
        // Batch 2: il nuovo nome compare come creazione.
        elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Creato, &nuovo)]);
        let (p, g) = el1.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(el.cambi, vec![Cambio::Rinominato { da: doc, a: nuovo }]);
    }

    #[test]
    fn rinomina_spezzata_ignora_i_candidati_troppo_vecchi() {
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "testo");
        let t0 = Instant::now();
        let el1 = elabora_a(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::RinominatoDa, &doc)],
            t0,
        );
        elabora_a(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::Creato, &nuovo)],
            t0,
        );
        let (p, g) = el1.ricontrolli[0].clone();
        // Ricontrollo oltre la finestra: il file "recente" è ormai troppo
        // vecchio per essere la destinazione, il documento risulta sparito.
        let tardi = t0 + ETA_MAX_RECENTE + Duration::from_secs(1);
        let el = ricontrolla_a(&mut blocca(&condiviso), &p, g, tardi);
        assert_eq!(el.cambi, vec![Cambio::Sparito(doc.clone())]);
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(doc.as_path()));
    }

    #[test]
    fn rinomina_spezzata_entro_la_finestra_si_segue() {
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "testo");
        let t0 = Instant::now();
        let el1 = elabora_a(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::RinominatoDa, &doc)],
            t0,
        );
        elabora_a(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::Creato, &nuovo)],
            t0,
        );
        let (p, g) = el1.ricontrolli[0].clone();
        let el = ricontrolla_a(
            &mut blocca(&condiviso),
            &p,
            g,
            t0 + ETA_MAX_RECENTE - Duration::from_secs(1),
        );
        assert_eq!(el.cambi, vec![Cambio::Rinominato { da: doc, a: nuovo }]);
    }

    #[test]
    fn file_non_letti_dalla_compilazione_non_invalidano_le_risorse() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let pdf = radice.join("riferimento.pdf");
        let lock = radice.join("~$lucido.pptx");
        let immagine = radice.join("logo.png");
        let dati = radice.join("dati.csv");
        let mancante = radice.join("bib.bib");
        for p in [&doc, &pdf, &lock, &immagine, &dati] {
            std::fs::write(p, "x").expect("fixture");
        }
        let condiviso = stato_con_doc(&radice, &doc, b"x");
        let letti = RisorseLette::nuovo();
        letti.registra(&immagine);
        letti.registra(&dati);
        letti.registra(&mancante); // letto ma fallito: la sua comparsa deve contare
        blocca(&condiviso).letti = Some(letti);

        // Estensioni non Typst mai lette: nessun cambio.
        let el = elabora(
            &mut blocca(&condiviso),
            &[
                ev(TipoEvento::Modificato, &pdf),
                ev(TipoEvento::Modificato, &lock),
            ],
        );
        assert!(el.cambi.is_empty(), "{:?}", el.cambi);

        // Risorse legittime (immagine, csv, bib creato dopo un errore): invalidano.
        std::fs::write(&mancante, "@book{}").expect("bib");
        for p in [&immagine, &dati, &mancante] {
            let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, p)]);
            assert_eq!(el.cambi, vec![Cambio::Contenuto(p.clone())], "{p:?}");
        }
        // Cancellazione di una risorsa letta.
        std::fs::remove_file(&dati).expect("rimuovi");
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Rimosso, &dati)]);
        assert_eq!(el.cambi, vec![Cambio::Sparito(dati.clone())]);
    }

    #[test]
    fn file_non_letto_aggiorna_comunque_l_albero() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let pdf = radice.join("nuovo.pdf");
        std::fs::write(&doc, "x").expect("doc");
        std::fs::write(&pdf, "x").expect("pdf");
        let condiviso = stato_con_doc(&radice, &doc, b"x");
        blocca(&condiviso).letti = Some(RisorseLette::nuovo());
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Creato, &pdf)]);
        assert!(el.cambi.is_empty());
        assert!(el.albero, "la comparsa del file va comunque nell'albero");
    }

    #[test]
    fn risorse_lette_normalizza_il_percorso() {
        let (_d, radice) = radice_temp();
        let f = radice.join("Dati.CSV");
        std::fs::write(&f, "x").expect("fixture");
        let letti = RisorseLette::nuovo();
        letti.registra(&f);
        assert!(letti.contiene(&f));
        letti.svuota();
        assert!(!letti.contiene(&f));
    }

    #[test]
    fn rinomina_spezzata_con_contenuto_diverso_e_una_sparizione() {
        // Nessun abbinamento certo (contenuto diverso): meglio "sparito" con
        // verifica d'integrità che seguire il file sbagliato.
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "tutt'altro");
        let el1 = elabora(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::RinominatoDa, &doc)],
        );
        elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Creato, &nuovo)]);
        let (p, g) = el1.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(el.cambi, vec![Cambio::Sparito(doc)]);
    }

    #[test]
    fn rinomina_abbinata_con_contenuto_diverso_non_si_segue() {
        // Da/A nello stesso batch ma il file di destinazione ha un contenuto
        // diverso dall'ultimo noto del documento: è un file diverso, non si segue.
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "tutt'altro");
        let eventi = [
            ev(TipoEvento::RinominatoDa, &doc),
            ev(TipoEvento::RinominatoA, &nuovo),
        ];
        let el = elabora(&mut blocca(&condiviso), &eventi);
        let (p, g) = el.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(el.cambi, vec![Cambio::Sparito(doc.clone())]);
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(doc.as_path()));
    }

    #[test]
    fn overflow_del_watcher_diventa_rescan() {
        use notify::event::Flag;
        // Tipo irrilevante e nessun percorso: senza il controllo del flag verrebbe scartato.
        let ev_rescan = Event::new(EventKind::Other).set_flag(Flag::Rescan);
        let e = evento_da_notify(&ev_rescan).expect("il rescan non va scartato");
        assert_eq!(e.tipo, TipoEvento::Rescan);
        assert!(evento_da_notify(&Event::new(EventKind::Other)).is_none());
    }

    #[test]
    fn rescan_rilegge_il_documento_invalida_le_risorse_e_aggiorna_l_albero() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        // Il documento è cambiato mentre gli eventi andavano persi.
        std::fs::write(&doc, "cambiato durante l'overflow").expect("esterna");
        let rescan = EventoFs {
            tipo: TipoEvento::Rescan,
            percorsi: Vec::new(),
        };
        let el = elabora(&mut blocca(&condiviso), &[rescan]);
        assert!(el.albero, "albero da aggiornare");
        assert!(
            el.cambi.contains(&Cambio::Contenuto(doc.clone())),
            "documento modificato: {:?}",
            el.cambi
        );
        // Un cambio su un percorso non-documento => InvalidaRisorse.
        assert!(
            el.cambi
                .iter()
                .any(|c| matches!(c, Cambio::Contenuto(p) if p != &doc)
                    && crate::esterni::decidi(Some(&doc), false, c)
                        == vec![crate::esterni::Azione::InvalidaRisorse]),
            "invalidazione risorse: {:?}",
            el.cambi
        );
    }

    #[test]
    fn rinomina_verso_un_backup_non_si_segue_mai() {
        for nome in ["main.typ~", "main.typ.bak", "main.orig", "main.typ.tmp"] {
            let (_d, radice) = radice_temp();
            let doc = radice.join("main.typ");
            std::fs::write(&doc, "testo").expect("fixture");
            let condiviso = stato_con_doc(&radice, &doc, b"testo");
            let backup = radice.join(nome);
            std::fs::rename(&doc, &backup).expect("backup");
            let eventi = [
                ev(TipoEvento::RinominatoDa, &doc),
                ev(TipoEvento::RinominatoA, &backup),
            ];
            let el = elabora(&mut blocca(&condiviso), &eventi);
            let (p, g) = el.ricontrolli[0].clone();
            // L'editor non ha riscritto l'originale: il documento è sparito, e
            // il backup (stesso contenuto!) non è la sua nuova casa.
            let el = ricontrolla(&mut blocca(&condiviso), &p, g);
            assert_eq!(el.cambi, vec![Cambio::Sparito(doc.clone())], "{nome}");
            assert_eq!(blocca(&condiviso).doc.as_deref(), Some(doc.as_path()));
        }
    }

    #[test]
    fn rinomina_in_backup_e_riscrittura_e_un_cambio_di_contenuto() {
        // Editor che salva rinominando l'originale in backup e riscrivendo.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");

        let backup = radice.join("main.typ~");
        std::fs::rename(&doc, &backup).expect("backup");
        std::fs::write(&doc, "nuovo").expect("riscrittura");
        let eventi = [
            ev(TipoEvento::RinominatoDa, &doc),
            ev(TipoEvento::RinominatoA, &backup),
            ev(TipoEvento::Creato, &doc),
        ];
        let el = elabora(&mut blocca(&condiviso), &eventi);
        assert_eq!(el.cambi, vec![Cambio::Contenuto(doc.clone())], "{el:?}");
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(doc.as_path()));
    }

    #[test]
    fn rinomina_con_il_vecchio_nome_tornato_al_ricontrollo_non_si_segue() {
        // Da assente nel batch, ma al ricontrollo l'originale è di nuovo lì.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "testo").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        let altro = radice.join("altro.typ");
        std::fs::rename(&doc, &altro).expect("via");
        let eventi = [
            ev(TipoEvento::RinominatoDa, &doc),
            ev(TipoEvento::RinominatoA, &altro),
        ];
        let el = elabora(&mut blocca(&condiviso), &eventi);
        std::fs::rename(&altro, &doc).expect("indietro");
        let (p, g) = el.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert!(el.cambi.is_empty(), "{:?}", el.cambi);
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(doc.as_path()));
    }

    #[test]
    fn elabora_ricontrolla_la_cancellazione() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "testo").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        std::fs::remove_file(&doc).expect("cancella");
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Rimosso, &doc)]);
        assert!(
            el.cambi.is_empty(),
            "prima si ricontrolla, non si notifica subito"
        );
        assert_eq!(el.ricontrolli.len(), 1);
        let (p, g) = el.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(el.cambi, vec![Cambio::Sparito(doc)]);
    }

    #[test]
    fn elabora_fratello_modificato_e_eco_di_export() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let fratello = radice.join("capitolo.typ");
        std::fs::write(&doc, "x").expect("doc");
        std::fs::write(&fratello, "y").expect("fratello");
        let condiviso = stato_con_doc(&radice, &doc, b"x");

        let el = elabora(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::Modificato, &fratello)],
        );
        assert_eq!(el.cambi, vec![Cambio::Contenuto(fratello.clone())]);
        let el = elabora(
            &mut blocca(&condiviso),
            &[ev(TipoEvento::Rimosso, &radice.join("assente.typ"))],
        );
        assert_eq!(el.cambi, vec![Cambio::Sparito(radice.join("assente.typ"))]);

        let pdf = radice.join("out.pdf");
        let esito = scrivi_proprio(&condiviso, &pdf, b"%PDF", false).expect("export");
        assert_eq!(esito, EsitoScrittura::Scritto);
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &pdf)]);
        assert!(
            el.cambi.is_empty(),
            "l'export dell'app non è una modifica esterna"
        );
    }

    // ---- salvataggio sicuro (B1) ----

    #[test]
    fn salvataggio_rifiuta_una_modifica_esterna_non_ancora_elaborata() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        // Un altro programma scrive; il watcher non ha ancora consegnato nulla.
        std::fs::write(&doc, "esterno").expect("esterna");
        for automatico in [false, true] {
            let esito = scrivi_proprio(&condiviso, &doc, b"dell'editor", automatico).expect("ok");
            assert_eq!(esito, EsitoScrittura::Conflitto, "automatico={automatico}");
            assert_eq!(std::fs::read_to_string(&doc).expect("file"), "esterno");
        }
    }

    #[test]
    fn salvataggio_durante_la_domanda_non_sovrascrive() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        std::fs::write(&doc, "esterno").expect("esterna");
        // Il watcher elabora: il cambio è noto, la domanda "quale versione tenere" è aperta.
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        assert_eq!(el.cambi, vec![Cambio::Contenuto(doc.clone())]);
        // Un salvataggio (automatico o manuale) parte mentre la domanda è aperta.
        for automatico in [true, false] {
            let esito = scrivi_proprio(&condiviso, &doc, b"dell'editor", automatico).expect("ok");
            assert_eq!(esito, EsitoScrittura::Conflitto);
            assert_eq!(std::fs::read_to_string(&doc).expect("file"), "esterno");
        }
        // L'utente sceglie "tieni quella dell'editor": ora sovrascrivere è consentito.
        riconosci_disco(&condiviso, &doc, None, 10).expect("riconosci");
        let esito = scrivi_proprio(&condiviso, &doc, b"dell'editor", false).expect("ok");
        assert_eq!(esito, EsitoScrittura::Scritto);
        assert_eq!(std::fs::read_to_string(&doc).expect("file"), "dell'editor");
    }

    #[test]
    fn tenere_la_versione_dell_editor_salva_prima_quella_su_disco_in_cronologia() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        let cron = Cronologia::in_radice(radice.join("storia"));
        std::fs::write(&doc, "versione esterna").expect("esterna");
        riconosci_disco(&condiviso, &doc, Some(&cron), 10).expect("riconosci");
        assert_eq!(cron.ultima(&doc).as_deref(), Some("versione esterna"));
        // Ora sovrascrivere è consentito, ma la versione scartata è recuperabile.
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mia", false).expect("ok"),
            EsitoScrittura::Scritto
        );
        assert_eq!(std::fs::read_to_string(&doc).expect("file"), "mia");
        assert_eq!(cron.elenca(&doc).len(), 1);
    }

    #[test]
    fn riconoscere_un_file_sparito_permette_di_ricrearlo() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "testo").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        std::fs::remove_file(&doc).expect("cancella");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok"),
            EsitoScrittura::Sparito
        );
        // Scelta esplicita nel dialog di sistema dello stesso percorso.
        riconosci_disco(&condiviso, &doc, None, 10).expect("riconosci");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn dopo_una_rinomina_seguita_il_vecchio_percorso_non_fa_tornare_indietro() {
        let (_d, _r, doc, nuovo, condiviso) = doc_rinominato("testo", "testo");
        let el = elabora(
            &mut blocca(&condiviso),
            &[
                ev(TipoEvento::RinominatoDa, &doc),
                ev(TipoEvento::RinominatoA, &nuovo),
            ],
        );
        let (p, g) = el.ricontrolli[0].clone();
        ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(nuovo.as_path()));

        let radice = nuovo.parent().expect("radice").to_path_buf();
        let fallisci = |_: &Path| -> notify::Result<RecommendedWatcher> {
            Err(notify::Error::generic("prova"))
        };
        // Richiesta partita prima dell'evento: comunica ancora il vecchio percorso.
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            true,
            Some(radice.clone()),
            fallisci,
        );
        {
            let s = blocca(&condiviso);
            assert_eq!(
                s.doc.as_deref(),
                Some(nuovo.as_path()),
                "nessun ritorno indietro"
            );
            assert_eq!(s.osservatore.noto(&nuovo), Some(Impronta::di(b"testo")));
            assert!(s.non_salvato, "lo stato non salvato si aggiorna comunque");
        }
        // Il frontend conferma il nuovo percorso: la questione è chiusa.
        imposta_documento(
            &mut blocca(&condiviso),
            Some(nuovo.clone()),
            false,
            Some(radice.clone()),
            fallisci,
        );
        assert!(blocca(&condiviso).ultima_rinomina.is_none());
        // Da qui un percorso diverso è un vero cambio di documento.
        let altro = radice.join("altro.typ");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(altro.clone()),
            false,
            Some(radice),
            fallisci,
        );
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(altro.as_path()));
    }

    #[test]
    fn se_il_disco_cambia_ancora_dopo_il_riconoscimento_il_conflitto_torna() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "a").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"a");
        std::fs::write(&doc, "b").expect("esterna 1");
        riconosci_disco(&condiviso, &doc, None, 10).expect("riconosci");
        std::fs::write(&doc, "c").expect("esterna 2");
        let esito = scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok");
        assert_eq!(esito, EsitoScrittura::Conflitto);
    }

    #[test]
    fn dopo_la_ricarica_da_disco_il_salvataggio_e_consentito() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        std::fs::write(&doc, "esterno").expect("esterna");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
        assert_eq!(leggi_e_segna(&condiviso, &doc).expect("leggi"), "esterno");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"esterno modificato", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn lunghezza_compatibile_scarta_file_di_lunghezza_diversa() {
        let (_d, radice) = radice_temp();
        let f = radice.join("a.typ");
        std::fs::write(&f, "abc").expect("fixture");
        assert!(lunghezza_compatibile(&f, &Impronta::di(b"xyz")));
        assert!(!lunghezza_compatibile(&f, &Impronta::di(b"xy")));
        assert!(!lunghezza_compatibile(&radice, &Impronta::di(b"")));
        assert!(!lunghezza_compatibile(
            &radice.join("manca"),
            &Impronta::di(b"abc")
        ));
    }

    #[test]
    fn export_registrato_e_riscritto_da_un_visualizzatore_non_ricompila() {
        use crate::esterni::TipoEvento;
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "a").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"a");
        let pdf = radice.join("out.pdf");
        scrivi_proprio(&condiviso, &pdf, b"%PDF-1", false).expect("export");
        // Il visualizzatore riscrive il PDF con altro contenuto.
        std::fs::write(&pdf, b"%PDF-2").expect("riscrittura");
        let mut stato = blocca(&condiviso);
        stato.letti = Some(RisorseLette::nuovo()); // la compilazione non lo ha letto
        let ev = EventoFs {
            tipo: TipoEvento::Modificato,
            percorsi: vec![pdf.clone()],
        };
        let el = elabora(&mut stato, &[ev]);
        assert!(el.cambi.is_empty(), "{:?}", el.cambi);
    }

    #[test]
    fn ricarica_di_file_non_utf8_non_fa_avanzare_la_base() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "vecchio").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"vecchio");
        // riscrittura esterna in UTF-16 LE (non UTF-8 valido)
        let utf16: Vec<u8> = "\u{feff}esterno"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        std::fs::write(&doc, &utf16).expect("esterna");
        let err = leggi_e_segna(&condiviso, &doc).expect_err("UTF-16 non valido");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        // Il salvataggio deve restare un Conflitto: la base non è avanzata.
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
        assert_eq!(std::fs::read(&doc).expect("disco"), utf16);
    }

    #[test]
    fn salvataggio_non_ricrea_un_file_cancellato_prima_del_ricontrollo() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "testo").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        // Cancellato, nessun evento ancora elaborato.
        std::fs::remove_file(&doc).expect("cancella");
        for automatico in [false, true] {
            let esito = scrivi_proprio(&condiviso, &doc, b"x", automatico).expect("ok");
            assert_eq!(esito, EsitoScrittura::Sparito);
            assert!(
                !doc.exists(),
                "nessun file ricreato (automatico={automatico})"
            );
        }
        // Anche con l'evento elaborato e il ricontrollo ancora pendente.
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Rimosso, &doc)]);
        assert_eq!(el.ricontrolli.len(), 1);
        let esito = scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok");
        assert_eq!(esito, EsitoScrittura::Sparito);
        assert!(!doc.exists());
    }

    #[test]
    fn salvataggio_automatico_solo_sul_documento_aperto() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let altro = radice.join("altro.typ");
        std::fs::write(&doc, "x").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"x");
        let esito = scrivi_proprio(&condiviso, &altro, b"y", true).expect("ok");
        assert_eq!(esito, EsitoScrittura::Conflitto);
        assert!(!altro.exists());
        // Il salvataggio manuale altrove (salva con nome) resta possibile.
        let esito = scrivi_proprio(&condiviso, &altro, b"y", false).expect("ok");
        assert_eq!(esito, EsitoScrittura::Scritto);
        // E salvare due volte di fila il documento non è mai un conflitto (eco propria).
        for _ in 0..2 {
            assert_eq!(
                scrivi_proprio(&condiviso, &doc, b"z", true).expect("ok"),
                EsitoScrittura::Scritto
            );
        }
    }

    // ---- ripristino, osservazione ----

    #[test]
    fn ripristina_sparito_non_sovrascrive_un_file_ricomparso() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "altrui").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        let esito = ripristina_sparito(&condiviso, &doc, "testo dell'editor", None, 10)
            .expect("ripristino");
        assert_eq!(esito, EsitoSparito::Esiste);
        assert_eq!(std::fs::read_to_string(&doc).expect("file"), "altrui");
    }

    #[test]
    fn ripristina_sparito_non_ricrea_una_cartella_scomparsa() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("sotto").join("main.typ");
        let condiviso = stato_con_doc(&radice, &doc, b"testo");
        let esito = ripristina_sparito(&condiviso, &doc, "testo integro", None, 10).expect("ok");
        assert_eq!(esito, EsitoSparito::CartellaAssente);
        assert!(!radice.join("sotto").exists(), "nessuna gerarchia ricreata");
    }

    #[test]
    fn ripristina_sparito_ricrea_se_integro_e_rifiuta_se_vuoto() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let cron = Cronologia::in_radice(radice.join("storia"));
        let condiviso = stato_con_doc(&radice, &doc, b"testo originale");
        // Il file non esiste (mai scritto): è "sparito".

        let vuoto = ripristina_sparito(&condiviso, &doc, "", Some(&cron), 10).expect("vuoto");
        assert_eq!(vuoto, EsitoSparito::NonIntegro(Integrita::Vuoto));
        assert!(!doc.exists());

        let ok = ripristina_sparito(&condiviso, &doc, "= Titolo\n#let x = (", Some(&cron), 10)
            .expect("integro");
        assert_eq!(ok, EsitoSparito::Ripristinato);
        assert_eq!(
            std::fs::read_to_string(&doc).expect("file"),
            "= Titolo\n#let x = ("
        );
        assert_eq!(cron.elenca(&doc).len(), 1);
        // L'eco del file appena ricreato non produce cambi.
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Creato, &doc)]);
        assert!(el.cambi.is_empty() && el.ricontrolli.is_empty());
    }

    #[test]
    fn radice_fallita_non_si_riprova_e_il_registro_si_pota_al_cambio_radice() {
        let (_d, radice) = radice_temp();
        let altra = radice.join("altra");
        let mut s = Stato::default();
        s.registro.registra(&radice.join("fuori.pdf"), b"x");
        s.registro.registra(&altra.join("dentro.pdf"), b"y");
        let tentativi = std::cell::Cell::new(0);
        let fallisci = |_: &Path| -> notify::Result<RecommendedWatcher> {
            tentativi.set(tentativi.get() + 1);
            Err(notify::Error::generic("prova"))
        };
        assert!(!imposta_documento(
            &mut s,
            None,
            false,
            Some(radice.clone()),
            fallisci
        ));
        assert!(!imposta_documento(
            &mut s,
            None,
            true,
            Some(radice.clone()),
            fallisci
        ));
        assert_eq!(
            tentativi.get(),
            1,
            "la radice fallita non si riprova a ogni chiamata"
        );
        // Cambiando radice si riprova, e il registro perde le voci fuori dalla nuova.
        assert!(!imposta_documento(
            &mut s,
            None,
            false,
            Some(altra.clone()),
            fallisci
        ));
        assert_eq!(tentativi.get(), 2);
        assert!(!s.registro.ha_voce(&radice.join("fuori.pdf")));
        assert!(s.registro.ha_voce(&altra.join("dentro.pdf")));
    }

    fn non_avviare(_: &Path) -> notify::Result<RecommendedWatcher> {
        Err(notify::Error::generic("prova"))
    }

    /// Crea un ostacolo che rende `doc` "illeggibile" (cartella al posto del file),
    /// poi lo toglie e scrive `contenuto`: simula un lock che si libera.
    fn illeggibile_poi_leggibile(doc: &Path) -> impl FnOnce(&str) + '_ {
        std::fs::create_dir(doc).expect("ostacolo");
        move |contenuto| {
            std::fs::remove_dir(doc).expect("rimuovi ostacolo");
            std::fs::write(doc, contenuto).expect("contenuto");
        }
    }

    #[test]
    fn aggancio_illeggibile_non_da_un_conflitto_spurio_al_primo_salvataggio() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let libera = illeggibile_poi_leggibile(&doc);
        let mut s = Stato::default();
        imposta_documento(&mut s, Some(doc.clone()), false, Some(radice), non_avviare);
        assert_eq!(leggi(&doc), Lettura::Illeggibile);
        libera("contenuto aperto dall'editor");
        let condiviso: Condiviso = Arc::new(Mutex::new(s));
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"modificato", false).expect("ok"),
            EsitoScrittura::Scritto
        );
        assert_eq!(std::fs::read(&doc).expect("file"), b"modificato");
        // Dopo l'adozione la base è normale: una modifica esterna vera è un conflitto.
        std::fs::write(&doc, "esterno").expect("esterna");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
    }

    #[test]
    fn aggancio_illeggibile_e_poi_modifica_esterna_segnalata_resta_un_conflitto() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let libera = illeggibile_poi_leggibile(&doc);
        let mut s = Stato::default();
        imposta_documento(&mut s, Some(doc.clone()), true, Some(radice), non_avviare);
        libera("scritto da fuori");
        let condiviso: Condiviso = Arc::new(Mutex::new(s));
        // Il watcher vede il file: contenuto sconosciuto → Modificato (si chiede
        // all'utente) e l'incertezza cade.
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        assert_eq!(el.cambi, vec![Cambio::Contenuto(doc.clone())]);
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
        assert_eq!(std::fs::read(&doc).expect("file"), b"scritto da fuori");
    }

    #[test]
    fn aggancio_illeggibile_con_contenuto_consegnato_vede_la_versione_esterna_come_conflitto() {
        // `apri_file` ha consegnato all'editor "consegnato"; prima dell'aggancio
        // il file era illeggibile e nel frattempo qualcuno ha scritto altro.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let libera = illeggibile_poi_leggibile(&doc);
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        segna_consegnato(&condiviso, &doc, b"consegnato");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        assert_eq!(leggi(&doc), Lettura::Illeggibile);
        libera("versione esterna");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
        assert_eq!(std::fs::read(&doc).expect("file"), b"versione esterna");
    }

    #[test]
    fn aggancio_illeggibile_con_contenuto_consegnato_uguale_al_disco_salva_senza_conflitto() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let libera = illeggibile_poi_leggibile(&doc);
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        segna_consegnato(&condiviso, &doc, b"consegnato");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        libera("consegnato");
        // Significativo: con il consegnato ignorato la base sarebbe `None`
        // (incerta, adottata solo al salvataggio); con il consegnato è già il disco.
        assert_eq!(
            blocca(&condiviso).osservatore.base(&doc),
            Some(Impronta::di(b"consegnato"))
        );
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn aggancio_leggibile_con_consegnato_diverso_dal_disco_e_un_conflitto_con_cronologia() {
        // `apri_file` ha consegnato "v1"; prima di `osserva_documento` un altro
        // programma ha scritto "v2" e il file è leggibile: la base è ciò che
        // l'editor ha (v1), non il disco, quindi v2 non si perde in silenzio.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v2").expect("fixture");
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        segna_consegnato(&condiviso, &doc, b"v1");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice.clone()),
            non_avviare,
        );
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
        assert_eq!(std::fs::read(&doc).expect("file"), b"v2");
        // Il watcher vedrà v2 come già noto (prev = disco): nessun ricarica ora,
        // ma al riconoscimento la versione esterna finisce in cronologia.
        let cron = Cronologia::in_radice(radice.join("storia"));
        riconosci_disco(&condiviso, &doc, Some(&cron), 10).expect("riconosci");
        assert_eq!(cron.ultima(&doc).as_deref(), Some("v2"));
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn aggancio_leggibile_con_consegnato_uguale_al_disco_non_cambia_nulla() {
        // Guardia: per i file normali (consegnato = disco) base e noto sono il disco.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        segna_consegnato(&condiviso, &doc, b"v1");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        let s = blocca(&condiviso);
        assert_eq!(s.osservatore.noto(&doc), Some(Impronta::di(b"v1")));
        assert_eq!(s.osservatore.base(&doc), Some(Impronta::di(b"v1")));
        drop(s);
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn riapertura_dello_stesso_file_usa_il_consegnato_come_noto() {
        // Il disco è passato a v2 e il watcher non l'ha ancora elaborato: la
        // riapertura consegna v2 all'editor, quindi v2 è il contenuto noto e
        // l'evento tardivo non genera un "ricaricato" spurio.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        std::fs::write(&doc, "v2").expect("esterna");
        segna_consegnato(&condiviso, &doc, b"v2");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        assert_eq!(
            blocca(&condiviso).osservatore.noto(&doc),
            Some(Impronta::di(b"v2"))
        );
        let el = elabora(&mut blocca(&condiviso), &[ev(TipoEvento::Modificato, &doc)]);
        assert!(el.cambi.is_empty(), "{el:?}");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn riapertura_dello_stesso_file_ignora_il_consegnato_di_un_altro_file() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        segna_consegnato(&condiviso, &radice.join("altro.typ"), b"altro");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        assert_eq!(
            blocca(&condiviso).osservatore.noto(&doc),
            Some(Impronta::di(b"v1"))
        );
    }

    /// Ri-aggancia lo stesso documento (come fa `osserva_documento` dopo una
    /// rinomina seguita) e restituisce il noto che ne risulta.
    fn noto_dopo_riaggancio(condiviso: &Condiviso, doc: &Path, radice: &Path) -> Option<Impronta> {
        imposta_documento(
            &mut blocca(condiviso),
            Some(doc.to_path_buf()),
            false,
            Some(radice.to_path_buf()),
            non_avviare,
        );
        blocca(condiviso).osservatore.noto(doc)
    }

    #[test]
    fn leggi_e_segna_azzera_il_consegnato_stantio_dello_stesso_file() {
        // Rinomina seguita: `apri_file` consegna v1 (testo poi scartato dal
        // frontend), poi un altro programma scrive v2 e l'editor lo ricarica.
        // Il consegnato v1 è più vecchio di ciò che l'editor ha visto: non deve
        // riportare noto/base a v1 al successivo `osserva_documento`.
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        segna_consegnato(&condiviso, &doc, b"v1");
        std::fs::write(&doc, "v2").expect("esterna");
        assert_eq!(leggi_e_segna(&condiviso, &doc).expect("leggi"), "v2");
        assert_eq!(
            noto_dopo_riaggancio(&condiviso, &doc, &radice),
            Some(Impronta::di(b"v2"))
        );
        assert_eq!(
            blocca(&condiviso).osservatore.base(&doc),
            Some(Impronta::di(b"v2"))
        );
    }

    #[test]
    fn riconosci_disco_azzera_il_consegnato_stantio_dello_stesso_file() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        segna_consegnato(&condiviso, &doc, b"v1");
        std::fs::write(&doc, "v2").expect("esterna");
        riconosci_disco(&condiviso, &doc, None, 10).expect("riconosci");
        assert_eq!(
            noto_dopo_riaggancio(&condiviso, &doc, &radice),
            Some(Impronta::di(b"v2"))
        );
    }

    #[test]
    fn ripristina_sparito_azzera_il_consegnato_stantio_dello_stesso_file() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        segna_consegnato(&condiviso, &doc, b"v1");
        // Il file non esiste: il ripristino lo ricrea col testo dell'editor.
        let esito = ripristina_sparito(&condiviso, &doc, "= Titolo", None, 10).expect("ripristino");
        assert_eq!(esito, EsitoSparito::Ripristinato);
        assert_eq!(
            noto_dopo_riaggancio(&condiviso, &doc, &radice),
            Some(Impronta::di(b"= Titolo"))
        );
    }

    #[test]
    fn scrivi_proprio_azzera_il_consegnato_stantio_dello_stesso_file() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        segna_consegnato(&condiviso, &doc, b"v1");
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
        assert_eq!(
            noto_dopo_riaggancio(&condiviso, &doc, &radice),
            Some(Impronta::di(b"mio"))
        );
    }

    #[test]
    fn le_dichiarazioni_sul_documento_non_toccano_il_consegnato_di_un_altro_file() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let altro = radice.join("altro.typ");
        std::fs::write(&doc, "v1").expect("fixture");
        let condiviso = stato_con_doc(&radice, &doc, b"v1");
        let atteso = Some((altro.clone(), Impronta::di(b"altro")));
        let ancora_li = |c: &Condiviso| blocca(c).consegnato == atteso;

        segna_consegnato(&condiviso, &altro, b"altro");
        leggi_e_segna(&condiviso, &doc).expect("leggi");
        assert!(ancora_li(&condiviso), "leggi_e_segna");

        riconosci_disco(&condiviso, &doc, None, 10).expect("riconosci");
        assert!(ancora_li(&condiviso), "riconosci_disco");

        scrivi_proprio(&condiviso, &doc, b"mio", false).expect("scrivi");
        assert!(ancora_li(&condiviso), "scrivi_proprio");

        std::fs::remove_file(&doc).expect("rimuovi");
        let esito = ripristina_sparito(&condiviso, &doc, "= T", None, 10).expect("ripristino");
        assert_eq!(esito, EsitoSparito::Ripristinato);
        assert!(ancora_li(&condiviso), "ripristina_sparito");
    }

    #[test]
    fn contenuto_consegnato_di_un_altro_file_non_conta() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let libera = illeggibile_poi_leggibile(&doc);
        let condiviso: Condiviso = Arc::new(Mutex::new(Stato::default()));
        segna_consegnato(&condiviso, &radice.join("altro.typ"), b"altro");
        imposta_documento(
            &mut blocca(&condiviso),
            Some(doc.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        libera("qualunque");
        // Base incerta come prima: il primo contenuto leggibile si adotta.
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"mio", false).expect("ok"),
            EsitoScrittura::Scritto
        );
    }

    #[test]
    fn file_assente_all_aggancio_non_e_un_aggancio_incerto() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        let mut s = Stato::default();
        imposta_documento(&mut s, Some(doc.clone()), false, Some(radice), non_avviare);
        // Un file comparso dopo da altri non si adotta: Conflitto.
        std::fs::write(&doc, "altrui").expect("esterna");
        let condiviso: Condiviso = Arc::new(Mutex::new(s));
        assert_eq!(
            scrivi_proprio(&condiviso, &doc, b"x", false).expect("ok"),
            EsitoScrittura::Conflitto
        );
    }

    #[test]
    fn rinomina_seguita_mantiene_la_forma_di_percorso_del_frontend() {
        let (_d, radice) = radice_temp();
        let canonico = radice.join("main.typ");
        std::fs::write(&canonico, "testo").expect("fixture");
        // Il frontend conosce il file con un'altra grafia dello stesso percorso
        // (su Windows senza il prefisso verbatim `\\?\` del watcher).
        let radice_fe = grafia_del_frontend(&radice);
        let doc_fe = radice_fe.join("main.typ");
        let condiviso = stato_con_doc(&radice, &doc_fe, b"testo");
        let nuovo = radice.join("nuovo.typ");
        std::fs::rename(&canonico, &nuovo).expect("rinomina");
        let el = elabora(
            &mut blocca(&condiviso),
            &[
                ev(TipoEvento::RinominatoDa, &canonico),
                ev(TipoEvento::RinominatoA, &nuovo),
            ],
        );
        assert_eq!(el.doc.as_deref(), Some(doc_fe.as_path()));
        let (p, g) = el.ricontrolli[0].clone();
        let el = ricontrolla(&mut blocca(&condiviso), &p, g);
        assert_eq!(el.cambi.len(), 1, "{el:?}");
        let atteso = radice_fe.join("nuovo.typ");
        assert_eq!(blocca(&condiviso).doc.as_deref(), Some(atteso.as_path()));
        if cfg!(windows) {
            assert_ne!(atteso, nuovo, "la grafia deve differire da quella canonica");
        }
        // Gli eventi successivi riportano ancora la grafia del frontend.
        let el = elabora(&mut blocca(&condiviso), &[]);
        assert_eq!(el.doc.as_deref(), Some(atteso.as_path()));
    }

    /// Altra grafia dello stesso percorso: `\\?\C:\x` → `C:\x` (come lo dà un dialogo).
    fn grafia_del_frontend(canonico: &Path) -> PathBuf {
        PathBuf::from(crate::albero::percorso_per_frontend(canonico))
    }

    #[test]
    fn imposta_documento_adotta_la_grafia_del_frontend_per_lo_stesso_file() {
        let (_d, radice) = radice_temp();
        let canonico = radice.join("main.typ");
        std::fs::write(&canonico, "x").expect("fixture");
        let mut s = Stato::default();
        imposta_documento(
            &mut s,
            Some(canonico),
            false,
            Some(radice.clone()),
            non_avviare,
        );
        let altra = grafia_del_frontend(&radice).join("main.typ");
        if cfg!(windows) {
            assert_ne!(altra, radice.join("main.typ"));
        }
        imposta_documento(
            &mut s,
            Some(altra.clone()),
            false,
            Some(radice),
            non_avviare,
        );
        assert_eq!(s.doc.as_deref(), Some(altra.as_path()));
    }

    #[test]
    fn imposta_documento_parte_dal_contenuto_su_disco() {
        let (_d, radice) = radice_temp();
        let doc = radice.join("main.typ");
        std::fs::write(&doc, "su disco").expect("fixture");
        let mut s = Stato::default();
        let fallisci = |_: &Path| -> notify::Result<RecommendedWatcher> {
            Err(notify::Error::generic("prova"))
        };
        imposta_documento(&mut s, Some(doc.clone()), false, Some(radice), fallisci);
        assert_eq!(s.osservatore.noto(&doc), Some(Impronta::di(b"su disco")));
        assert_eq!(s.osservatore.base(&doc), Some(Impronta::di(b"su disco")));
        assert_eq!(s.doc.as_deref(), Some(doc.as_path()));
    }
}
