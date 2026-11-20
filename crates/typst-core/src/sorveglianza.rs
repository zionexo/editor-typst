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
#[path = "../tests/unit/sorveglianza.rs"]
mod test;
