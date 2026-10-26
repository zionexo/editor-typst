//! Logica pura del controllo dei file modificati da fuori (F5-022, D-014/D-016).
//!
//! Nessuna dipendenza da `notify`, da Tauri o dal tempo di sistema: il modulo
//! [`crate::sorveglianza`] traduce gli eventi del watcher in [`EventoFs`], passa
//! gli istanti (in ms) e legge i file; qui si decide **cosa fare**, così tutto è
//! testabile headless.
//!
//! # Pezzi
//! - [`RegistroScritture`]: impronte SHA-256 dei contenuti scritti dall'app
//!   (salvataggio, autosave, ripristino, export): un evento il cui contenuto
//!   coincide con l'impronta registrata è un'eco di una scrittura propria e si
//!   ignora (anti-loop, prima difesa insieme a [`Osservatore`]).
//! - [`Osservatore`]: macchina a stati per file, adattata da Tinymist
//!   (`tinymist-project/src/watch.rs`, ADATTA, fase8.md §7.4): a ogni evento
//!   si confronta il contenuto riletto con l'ultimo noto; file vuoto o sparito
//!   entra in uno stato transitorio con ricontrollo, perché gli editor che
//!   salvano "tronca/rinomina/riscrivi" producono un secondo evento che
//!   riporta il contenuto.
//! - [`Batcher`] + [`riduci_eventi`]: coalescenza degli eventi a raffica e
//!   abbinamento delle rinomine `From`/`To` dentro lo stesso batch.
//! - [`decidi`]: decisione `ricarica | chiedi | sparito | segui | invalida`
//!   da stato del documento + cambiamento.
//! - [`verifica_integrita`] / [`ripristina_documento`]: il testo dell'editor
//!   di un file sparito si salva di nuovo solo se integro.
//!
//! Derivato da typst-kit 0.15.0 `src/watcher.rs` (commit
//! 3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright
//! Typst contributors: i tempi di batching ([`BATCH_MS`], [`STARVE_MS`]).
//! La classificazione degli eventi rilevanti (`e_rilevante`, dallo stesso file)
//! vive in [`crate::sorveglianza`], che conosce `notify`. Modello della
//! macchina a stati: Tinymist `tinymist-project/src/watch.rs` (commit
//! f3b00eb553d6b7a35b5d59cce529c8d0d774cfce), Apache-2.0 — Copyright
//! Myriad-Dreamin e contributori Tinymist (idea riscritta, nessun codice copiato).

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};

use crate::albero::{CARTELLE_RUMORE, CARTELLE_SOLO_ALBERO};
use crate::config::scrivi_atomico;
use crate::cronologia::{percorso_normalizzato, Cronologia};

// Derivato da typst-kit src/watcher.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
/// Silenzio (ms) dopo l'ultimo evento oltre il quale il batch si chiude.
pub const BATCH_MS: u64 = 100;
// Derivato da typst-kit src/watcher.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
/// Durata massima (ms) di un batch dal primo evento, anche se gli eventi continuano.
pub const STARVE_MS: u64 = 500;
/// Attesa (ms) prima di ricontrollare un file svuotato o sparito: deve superare
/// il salvataggio "tronca/rinomina" degli altri editor (Tinymist: ≥ 50 ms).
pub const RICONTROLLO_MS: u64 = 150;

/// Tentativi massimi di rilettura di un file [`Lettura::Illeggibile`] per
/// evento (ognuno dopo [`RICONTROLLO_MS`]): copre un blocco breve (antivirus,
/// salvataggio in corso con `ERROR_SHARING_VIOLATION`) senza ciclare all'infinito
/// su un errore permanente (permessi).
pub const MAX_TENTATIVI_ILLEGGIBILE: u8 = 6;

/// Impronta di un contenuto: SHA-256 e lunghezza in byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impronta {
    hash: [u8; 32],
    /// Lunghezza in byte del contenuto.
    pub lunghezza: u64,
}

impl Impronta {
    /// Impronta di `contenuto`.
    pub fn di(contenuto: &[u8]) -> Self {
        Self {
            hash: Sha256::digest(contenuto).into(),
            lunghezza: contenuto.len() as u64,
        }
    }

    /// `true` se il contenuto era vuoto.
    pub fn vuota(&self) -> bool {
        self.lunghezza == 0
    }
}

/// Stato di un file letto dal disco al momento dell'evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lettura {
    /// Il file non esiste (`NotFound`).
    Assente,
    /// Il file esiste ma non si riesce a leggerlo (permessi, bloccato da chi lo
    /// sta scrivendo, non è un file…): **non** vale "sparito" e non cambia
    /// nessuno stato, si aspetta il prossimo evento.
    Illeggibile,
    /// Il file esiste con questo contenuto (anche vuoto).
    Contenuto(Impronta),
}

/// `true` se `percorso` ha il nome di un file di appoggio/backup di un altro
/// editor (`~`, `.bak`, `.orig`, `.swp`, `.swo`, `.tmp`, `.temp`): una rinomina
/// del documento verso un nome così **non** si segue mai (è il backup che
/// l'editor crea prima di riscrivere l'originale).
pub fn e_nome_di_backup(percorso: &Path) -> bool {
    let Some(nome) = percorso
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
    else {
        return false;
    };
    if nome.ends_with('~')
        || nome.starts_with(".#")
        || (nome.starts_with('#') && nome.ends_with('#'))
    {
        return true;
    }
    matches!(
        Path::new(&nome).extension().and_then(|e| e.to_str()),
        Some("bak" | "orig" | "swp" | "swo" | "tmp" | "temp")
    )
}

/// `true` se `a` e `b` indicano lo stesso file (forma normalizzata: prefisso
/// `\\?\`, separatori e, su Windows, maiuscole non contano).
pub fn stesso_file(a: &Path, b: &Path) -> bool {
    percorso_normalizzato(a) == percorso_normalizzato(b)
}

/// `true` se l'evento su `percorso` va ignorato perché sta in una cartella
/// nascosta (nome che inizia con `.`, come `.git` o i temporanei di
/// [`scrivi_atomico`]) o in una cartella esclusa, relativamente a `radice`.
/// Un percorso fuori da `radice` non è ignorato.
pub fn percorso_ignorato(radice: &Path, percorso: &Path) -> bool {
    percorso_escluso(radice, percorso, false)
}

/// Come [`percorso_ignorato`] ma per decidere se l'**albero** dei file va
/// riletto: esclude anche le cartelle nascoste solo nell'albero
/// ([`crate::albero::CARTELLE_SOLO_ALBERO`], es. `target/`), i cui eventi
/// restano però validi per l'invalidazione delle risorse.
pub fn percorso_fuori_dall_albero(radice: &Path, percorso: &Path) -> bool {
    percorso_escluso(radice, percorso, true)
}

fn percorso_escluso(radice: &Path, percorso: &Path, anche_solo_albero: bool) -> bool {
    // Confronto lessicale (senza canonicalizzare): la radice e i percorsi degli
    // eventi arrivano dalla stessa fonte (il watcher sulla radice canonica).
    let radice_n = crate::cronologia::normalizza_percorso(
        &radice.to_string_lossy(),
        crate::cronologia::fs_senza_maiuscole(),
    );
    let p_n = crate::cronologia::normalizza_percorso(
        &percorso.to_string_lossy(),
        crate::cronologia::fs_senza_maiuscole(),
    );
    let Some(resto) = p_n.strip_prefix(&radice_n) else {
        return false;
    };
    // Il prefisso deve finire su un confine di componente (`/prog` ≠ `/prog2`).
    if !resto.is_empty() && !resto.starts_with('/') && !radice_n.ends_with('/') {
        return false;
    }
    let relativo = resto.trim_start_matches('/');
    relativo.split('/').filter(|c| !c.is_empty()).any(|c| {
        c.starts_with('.')
            || CARTELLE_RUMORE.contains(&c)
            || crate::albero::e_file_rumore(c)
            || (anche_solo_albero && CARTELLE_SOLO_ALBERO.contains(&c))
    })
}

// ---- Registro delle scritture proprie ------------------------------------

/// Impronte dei contenuti scritti dall'app, per percorso (anti-loop, D-016).
#[derive(Debug, Default)]
pub struct RegistroScritture {
    voci: HashMap<String, Impronta>,
}

impl RegistroScritture {
    /// Registro vuoto.
    pub fn nuovo() -> Self {
        Self::default()
    }

    /// Registra che l'app sta per scrivere `contenuto` su `percorso`.
    pub fn registra(&mut self, percorso: &Path, contenuto: &[u8]) {
        self.voci
            .insert(percorso_normalizzato(percorso), Impronta::di(contenuto));
    }

    /// Dimentica la voce di `percorso` (scrittura non riuscita).
    pub fn dimentica(&mut self, percorso: &Path) {
        self.voci.remove(&percorso_normalizzato(percorso));
    }

    /// `true` se c'è una voce per `percorso` (un file mai scritto dall'app non
    /// serve leggerlo per riconoscere un'eco).
    pub fn ha_voce(&self, percorso: &Path) -> bool {
        self.voci.contains_key(&percorso_normalizzato(percorso))
    }

    /// Toglie le voci dei percorsi che non stanno sotto `radice`: al cambio di
    /// cartella osservata le impronte vecchie non servono più e non devono
    /// crescere senza limite.
    pub fn pota(&mut self, radice: &Path) {
        let radice = percorso_normalizzato(radice);
        self.voci.retain(|p, _| {
            p.strip_prefix(&radice)
                .is_some_and(|r| r.is_empty() || r.starts_with('/') || radice.ends_with('/'))
        });
    }

    /// `true` se `letto` è il contenuto registrato per `percorso`: eco di una
    /// scrittura propria. La voce **resta** (gli eventi possono arrivare più
    /// volte, es. cloud drive che riemettono in ritardo). Se il contenuto è
    /// diverso, una scrittura esterna ha preso il posto della nostra: la voce
    /// si elimina e ritorna `false`.
    pub fn e_propria(&mut self, percorso: &Path, letto: &Impronta) -> bool {
        let chiave = percorso_normalizzato(percorso);
        match self.voci.get(&chiave) {
            Some(registrata) if registrata == letto => true,
            Some(_) => {
                self.voci.remove(&chiave);
                false
            }
            None => false,
        }
    }
}

/// Scrive `contenuto` in modo atomico registrando **prima** l'impronta: se
/// l'evento del watcher arrivasse prima del ritorno, il contenuto sarebbe già
/// riconosciuto come proprio. Su errore la voce viene tolta. Il chiamante tiene
/// il lock del registro per tutta la durata (il thread del watcher legge e
/// confronta sotto lo stesso lock: nessuna finestra fra scrittura e registro).
pub fn scrivi_registrando(
    registro: &mut RegistroScritture,
    percorso: &Path,
    contenuto: &[u8],
) -> io::Result<()> {
    registro.registra(percorso, contenuto);
    let esito = scrivi_atomico(percorso, contenuto);
    if esito.is_err() {
        registro.dimentica(percorso);
    }
    esito
}

// ---- Macchina a stati per file --------------------------------------------

/// Cosa fare dopo un evento (o un ricontrollo) su un file osservato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esito {
    /// Niente da notificare.
    Nessuno,
    /// Il contenuto su disco è cambiato per mano di altri.
    Modificato,
    /// Il file non esiste più (dopo il ricontrollo).
    Sparito,
    /// Stato transitorio (vuoto/sparito): riesaminare fra [`RICONTROLLO_MS`]
    /// passando questa `generazione` a [`Osservatore::ricontrolla`].
    Ricontrolla {
        /// Numero dell'evento che ha originato il ricontrollo.
        generazione: u64,
    },
}

#[derive(Debug, Clone, Copy, Default)]
struct Voce {
    /// Ultimo contenuto noto (`None` = file assente).
    prev: Option<Impronta>,
    /// Ultimo contenuto noto **con il file esistente**: sopravvive alla
    /// sparizione, serve alla verifica di integrità del ripristino.
    ultimo_esistente: Option<Impronta>,
    /// Contenuto su disco che l'editor ha **visto** l'ultima volta (aperto,
    /// salvato, ricaricato o riconosciuto dall'utente): `None` = file assente.
    /// A differenza di `prev` non avanza da solo con gli eventi: è la base per
    /// decidere se un salvataggio sovrascriverebbe una modifica esterna mai vista.
    base: Option<Impronta>,
    /// Contatore degli eventi: un ricontrollo con generazione vecchia è superato.
    generazione: u64,
    /// `true` fra un evento vuoto/sparito e il suo ricontrollo.
    transitorio: bool,
    /// Riletture già fatte per l'evento corrente dopo una lettura illeggibile.
    tentativi: u8,
    /// Il file era illeggibile quando è stato agganciato (lock breve di antivirus o
    /// sync): `base` è `None` per ignoranza, non perché il file sia assente. Il
    /// primo contenuto leggibile viene adottato come base ([`Osservatore::adotta_base_incerta`]);
    /// un evento che segnala una modifica esterna (`Modificato`/`Sparito`) toglie
    /// l'incertezza, così il conflitto vero resta un conflitto.
    base_incerta: bool,
}

/// Macchina a stati dei file osservati (di norma il solo documento aperto).
#[derive(Debug, Default)]
pub struct Osservatore {
    voci: HashMap<String, Voce>,
}

impl Osservatore {
    /// Osservatore senza file.
    pub fn nuovo() -> Self {
        Self::default()
    }

    /// Imposta il contenuto noto di `percorso` (apertura, salvataggio, ripristino)
    /// azzerando lo stato transitorio; annulla i ricontrolli pendenti.
    pub fn imposta_noto(&mut self, percorso: &Path, noto: Option<Impronta>) {
        let voce = self
            .voci
            .entry(percorso_normalizzato(percorso))
            .or_default();
        voce.prev = noto;
        voce.base = noto;
        if noto.is_some() {
            voce.ultimo_esistente = noto;
        }
        voce.generazione += 1;
        voce.transitorio = false;
        voce.tentativi = 0;
        voce.base_incerta = false;
    }

    /// Come [`Osservatore::imposta_noto`] con `None`, ma per un file che esiste
    /// e **non si è riusciti a leggere** all'aggancio: la base resta da stabilire
    /// (vedi [`Osservatore::adotta_base_incerta`]).
    pub fn imposta_noto_illeggibile(&mut self, percorso: &Path) {
        self.imposta_noto(percorso, None);
        if let Some(voce) = self.voci.get_mut(&percorso_normalizzato(percorso)) {
            voce.base_incerta = true;
        }
    }

    /// Se la base di `percorso` era incerta (file illeggibile all'aggancio e
    /// nessuna modifica esterna segnalata da allora), adotta `letto` come noto e
    /// base e ritorna `true`: è il primo contenuto leggibile, non c'è nulla con cui
    /// confrontarlo e un salvataggio non è un conflitto.
    pub fn adotta_base_incerta(&mut self, percorso: &Path, letto: Impronta) -> bool {
        let Some(voce) = self.voci.get_mut(&percorso_normalizzato(percorso)) else {
            return false;
        };
        if !voce.base_incerta || voce.base.is_some() {
            return false;
        }
        voce.prev = Some(letto);
        voce.base = Some(letto);
        voce.ultimo_esistente = Some(letto);
        voce.base_incerta = false;
        true
    }

    /// Imposta solo la base vista dall'editor (l'utente ha riconosciuto lo stato
    /// del disco, o ne ha letto il contenuto), senza toccare la macchina a stati.
    pub fn imposta_base(&mut self, percorso: &Path, base: Option<Impronta>) {
        self.voci
            .entry(percorso_normalizzato(percorso))
            .or_default()
            .base = base;
    }

    /// Contenuto su disco che l'editor ha visto l'ultima volta (`None` = assente
    /// o file non seguito).
    pub fn base(&self, percorso: &Path) -> Option<Impronta> {
        self.voci
            .get(&percorso_normalizzato(percorso))
            .and_then(|v| v.base)
    }

    /// `true` se un ricontrollo con questa `generazione` è ancora quello valido
    /// (nessun evento successivo e stato ancora transitorio).
    pub fn ricontrollo_valido(&self, percorso: &Path, generazione: u64) -> bool {
        self.voci
            .get(&percorso_normalizzato(percorso))
            .is_some_and(|v| v.transitorio && v.generazione == generazione)
    }

    /// Smette di seguire `percorso`.
    pub fn dimentica(&mut self, percorso: &Path) {
        self.voci.remove(&percorso_normalizzato(percorso));
    }

    /// Ultimo contenuto noto del file, se lo si segue e non è assente.
    pub fn noto(&self, percorso: &Path) -> Option<Impronta> {
        self.voci
            .get(&percorso_normalizzato(percorso))
            .and_then(|v| v.prev)
    }

    /// Ultimo contenuto noto a file esistente, anche se ora è sparito.
    pub fn ultimo_esistente(&self, percorso: &Path) -> Option<Impronta> {
        self.voci
            .get(&percorso_normalizzato(percorso))
            .and_then(|v| v.ultimo_esistente)
    }

    /// Un evento su `percorso`: `lettura` è ciò che si legge ora dal disco.
    /// Un file non seguito è ignorato ([`Esito::Nessuno`]).
    pub fn su_evento(
        &mut self,
        percorso: &Path,
        lettura: Lettura,
        registro: &mut RegistroScritture,
    ) -> Esito {
        self.applica(percorso, lettura, registro, None)
    }

    /// Ricontrollo programmato da un [`Esito::Ricontrolla`]. Se nel frattempo
    /// è arrivato un altro evento (`generazione` diversa) il ricontrollo è
    /// superato e non fa nulla: solo l'ultimo evento decide.
    pub fn ricontrolla(
        &mut self,
        percorso: &Path,
        generazione: u64,
        lettura: Lettura,
        registro: &mut RegistroScritture,
    ) -> Esito {
        self.applica(percorso, lettura, registro, Some(generazione))
    }

    fn applica(
        &mut self,
        percorso: &Path,
        lettura: Lettura,
        registro: &mut RegistroScritture,
        ricontrollo: Option<u64>,
    ) -> Esito {
        let Some(voce) = self.voci.get_mut(&percorso_normalizzato(percorso)) else {
            return Esito::Nessuno;
        };
        match ricontrollo {
            Some(g) if g != voce.generazione || !voce.transitorio => return Esito::Nessuno,
            Some(_) => {}
            None => {
                voce.generazione += 1;
                voce.tentativi = 0;
            }
        }

        match lettura {
            // Non leggibile (tipicamente bloccato da chi lo sta scrivendo, es.
            // sharing violation su Windows): nessuna conclusione sul contenuto,
            // ma è una condizione transitoria come "vuoto/sparito" e si
            // ricontrolla, con un tetto di tentativi.
            Lettura::Illeggibile => {
                if voce.tentativi >= MAX_TENTATIVI_ILLEGGIBILE {
                    voce.transitorio = false;
                    voce.tentativi = 0;
                    return Esito::Nessuno;
                }
                voce.tentativi += 1;
                voce.transitorio = true;
                Esito::Ricontrolla {
                    generazione: voce.generazione,
                }
            }
            Lettura::Contenuto(imp) => {
                let definitiva = !imp.vuota() || ricontrollo.is_some();
                if registro.e_propria(percorso, &imp) {
                    voce.prev = Some(imp);
                    voce.ultimo_esistente = Some(imp);
                    voce.transitorio = false;
                    return Esito::Nessuno;
                }
                if !definitiva {
                    return Self::transitorio(voce, Some(imp));
                }
                voce.transitorio = false;
                if voce.prev == Some(imp) {
                    Esito::Nessuno
                } else {
                    voce.prev = Some(imp);
                    voce.ultimo_esistente = Some(imp);
                    // Modifica esterna segnalata: non è più un file "mai visto",
                    // un salvataggio prima della scelta dell'utente è un conflitto.
                    voce.base_incerta = false;
                    Esito::Modificato
                }
            }
            Lettura::Assente => {
                if ricontrollo.is_some() {
                    voce.transitorio = false;
                    return if voce.prev.is_none() {
                        Esito::Nessuno
                    } else {
                        voce.prev = None;
                        Esito::Sparito
                    };
                }
                Self::transitorio(voce, None)
            }
        }
    }

    /// Entra (o resta) nello stato transitorio, salvo che il file sia già
    /// nello stato osservato (`nuovo`): allora non c'è nulla da ricontrollare.
    fn transitorio(voce: &mut Voce, nuovo: Option<Impronta>) -> Esito {
        if voce.prev == nuovo && !voce.transitorio {
            return Esito::Nessuno;
        }
        voce.transitorio = true;
        Esito::Ricontrolla {
            generazione: voce.generazione,
        }
    }
}

// ---- Eventi, batching, abbinamento delle rinomine --------------------------

/// Tipo di evento del filesystem, già filtrato per rilevanza dal guscio
/// (accessi e metadati non arrivano fin qui).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEvento {
    /// File o cartella creati.
    Creato,
    /// Contenuto modificato.
    Modificato,
    /// Rimosso.
    Rimosso,
    /// Vecchio nome di una rinomina (`Name(From)`): un solo percorso.
    RinominatoDa,
    /// Nuovo nome di una rinomina (`Name(To)`): un solo percorso.
    RinominatoA,
    /// Rinomina con entrambi i nomi (`Name(Both)`): percorsi `[da, a]`.
    RinominatoEntrambi,
    /// Il watcher ha perso eventi (overflow del buffer di sistema, flag
    /// `Rescan` di `notify`): non si sa cosa sia cambiato. I percorsi possono
    /// essere vuoti. Chi elabora ricontrolla documento, risorse e albero.
    Rescan,
}

/// Evento del filesystem astratto da `notify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoFs {
    /// Tipo.
    pub tipo: TipoEvento,
    /// Percorsi coinvolti.
    pub percorsi: Vec<PathBuf>,
}

/// Coalescenza degli eventi a raffica (batching, niente crate debouncer):
/// il batch si chiude dopo [`BATCH_MS`] di silenzio o, al più tardi,
/// [`STARVE_MS`] dopo il primo evento. Il tempo è passato dal chiamante (ms).
#[derive(Debug, Default)]
pub struct Batcher {
    inizio: Option<u64>,
    ultimo: u64,
    eventi: Vec<EventoFs>,
}

impl Batcher {
    /// Batch vuoto.
    pub fn nuovo() -> Self {
        Self::default()
    }

    /// Aggiunge un evento arrivato all'istante `ora_ms`.
    pub fn aggiungi(&mut self, ora_ms: u64, evento: EventoFs) {
        self.inizio.get_or_insert(ora_ms);
        self.ultimo = ora_ms;
        self.eventi.push(evento);
    }

    /// Istante (ms) in cui il batch va chiuso; `None` se è vuoto.
    pub fn scadenza(&self) -> Option<u64> {
        self.inizio
            .map(|i| (self.ultimo + BATCH_MS).min(i + STARVE_MS))
    }

    /// `true` se all'istante `ora_ms` il batch è da chiudere.
    pub fn pronto(&self, ora_ms: u64) -> bool {
        self.scadenza().is_some_and(|s| ora_ms >= s)
    }

    /// Consegna gli eventi accumulati e riparte da vuoto.
    pub fn svuota(&mut self) -> Vec<EventoFs> {
        self.inizio = None;
        std::mem::take(&mut self.eventi)
    }
}

/// Sintesi di un batch di eventi.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Batch {
    /// Rinomine abbinate `(da, a)`.
    pub rinomine: Vec<(PathBuf, PathBuf)>,
    /// Percorsi creati o modificati (senza duplicati, in ordine di arrivo).
    pub toccati: Vec<PathBuf>,
    /// Percorsi rimossi, comprese le rinomine `From` non abbinabili.
    pub rimossi: Vec<PathBuf>,
}

fn aggiungi_unico(v: &mut Vec<PathBuf>, p: &Path) {
    if !v.iter().any(|x| x == p) {
        v.push(p.to_path_buf());
    }
}

/// Riduce un batch di eventi: abbina le rinomine `From`/`To` (su Windows
/// arrivano come eventi **separati**) e deduplica i percorsi.
///
/// L'abbinamento `From`/`To` è fatto solo se i due conteggi coincidono (in
/// ordine di arrivo): con conteggi diversi non si può sapere chi corrisponde
/// a chi, e abbinare a caso farebbe "seguire" un file sbagliato. I non
/// abbinati diventano rimozione (`From`) e creazione (`To`), cioè il caso
/// "sparito dal disco" di D-014.
pub fn riduci_eventi(eventi: &[EventoFs]) -> Batch {
    let mut batch = Batch::default();
    let da: Vec<&Path> = eventi
        .iter()
        .filter(|e| e.tipo == TipoEvento::RinominatoDa)
        .filter_map(|e| e.percorsi.first().map(PathBuf::as_path))
        .collect();
    let a: Vec<&Path> = eventi
        .iter()
        .filter(|e| e.tipo == TipoEvento::RinominatoA)
        .filter_map(|e| e.percorsi.first().map(PathBuf::as_path))
        .collect();
    let abbinabili = da.len() == a.len();
    if abbinabili {
        for (d, n) in da.iter().zip(a.iter()) {
            batch.rinomine.push((d.to_path_buf(), n.to_path_buf()));
        }
    } else {
        for d in &da {
            aggiungi_unico(&mut batch.rimossi, d);
        }
        for n in &a {
            aggiungi_unico(&mut batch.toccati, n);
        }
    }

    for e in eventi {
        match e.tipo {
            TipoEvento::Creato | TipoEvento::Modificato => {
                for p in &e.percorsi {
                    aggiungi_unico(&mut batch.toccati, p);
                }
            }
            TipoEvento::Rimosso => {
                for p in &e.percorsi {
                    aggiungi_unico(&mut batch.rimossi, p);
                }
            }
            TipoEvento::RinominatoEntrambi => {
                if let [d, n] = e.percorsi.as_slice() {
                    let coppia = (d.clone(), n.clone());
                    if !batch.rinomine.contains(&coppia) {
                        batch.rinomine.push(coppia);
                    }
                }
            }
            TipoEvento::RinominatoDa | TipoEvento::RinominatoA | TipoEvento::Rescan => {}
        }
    }
    batch
}

/// Percorsi da rileggere per aggiornare lo stato dei file: creati, modificati,
/// rimossi e destinazioni delle rinomine (una rinomina *verso* un percorso è
/// il modo in cui molti editor sostituiscono il contenuto di un file).
pub fn da_rileggere(batch: &Batch) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    for p in batch
        .toccati
        .iter()
        .chain(&batch.rimossi)
        .chain(batch.rinomine.iter().map(|(_, a)| a))
    {
        aggiungi_unico(&mut v, p);
    }
    v
}

// ---- Decisione -------------------------------------------------------------

/// Cambiamento (già consolidato) di un file dentro la cartella osservata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cambio {
    /// Contenuto cambiato o file comparso.
    Contenuto(PathBuf),
    /// File sparito (dopo il ricontrollo, se è il documento aperto).
    Sparito(PathBuf),
    /// Rinomina abbinata.
    Rinominato {
        /// Vecchio percorso.
        da: PathBuf,
        /// Nuovo percorso.
        a: PathBuf,
    },
}

/// Azione da compiere nell'app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Azione {
    /// Documento aperto senza modifiche non salvate: ricarica dal disco e ricompila.
    Ricarica,
    /// Documento aperto con modifiche non salvate: chiedere all'utente quale
    /// versione tenere; nulla si sovrascrive prima della risposta.
    Chiedi,
    /// Documento aperto sparito: verificare l'integrità del testo e ripristinare.
    DocumentoSparito,
    /// Documento rinominato da fuori: seguire il nuovo nome.
    SeguiRinomina {
        /// Nuovo percorso del documento.
        nuovo: PathBuf,
    },
    /// File fratello cambiato: invalidare la cache dei file e ricompilare.
    InvalidaRisorse,
    /// Alberi dei file da aggiornare (creazioni/cancellazioni/rinomine).
    AggiornaAlbero,
}

/// Decide le azioni per un `cambio`, dato il documento aperto (`doc`) e se ha
/// modifiche non salvate (`non_salvato`).
///
/// - documento con contenuto cambiato: `Ricarica` se pulito, `Chiedi` altrimenti;
/// - documento sparito o rinominato: `DocumentoSparito` / `SeguiRinomina` (+ albero);
/// - qualunque altro file: `InvalidaRisorse` (+ `AggiornaAlbero` se creato,
///   sparito o rinominato).
///
/// Una rinomina **verso** il documento (`a == doc`, tipico del salvataggio
/// atomico di altri editor) equivale a un cambio di contenuto.
pub fn decidi(doc: Option<&Path>, non_salvato: bool, cambio: &Cambio) -> Vec<Azione> {
    let e_doc = |p: &Path| doc.is_some_and(|d| stesso_file(d, p));
    let contenuto_doc = if non_salvato {
        Azione::Chiedi
    } else {
        Azione::Ricarica
    };
    match cambio {
        Cambio::Contenuto(p) if e_doc(p) => vec![contenuto_doc],
        Cambio::Contenuto(_) => vec![Azione::InvalidaRisorse],
        Cambio::Sparito(p) if e_doc(p) => vec![Azione::DocumentoSparito, Azione::AggiornaAlbero],
        Cambio::Sparito(_) => vec![Azione::InvalidaRisorse, Azione::AggiornaAlbero],
        Cambio::Rinominato { da, a } if e_doc(da) => vec![
            Azione::SeguiRinomina { nuovo: a.clone() },
            Azione::AggiornaAlbero,
        ],
        Cambio::Rinominato { a, .. } if e_doc(a) => vec![contenuto_doc, Azione::AggiornaAlbero],
        Cambio::Rinominato { .. } => vec![Azione::InvalidaRisorse, Azione::AggiornaAlbero],
    }
}

/// Percorso da comunicare al frontend per `p`, **nella forma di percorso con cui
/// il frontend ha aperto il documento** (`doc`).
///
/// Il watcher lavora sulla radice canonica del World (`\\?\UNC\srv\share\…`,
/// `C:\reale\…`) mentre il frontend confronta i percorsi in modo lessicale con
/// quello che gli ha dato il dialogo (drive di rete mappato, `subst`, giunzione):
/// un evento con la forma canonica verrebbe scartato in silenzio. Regole:
/// - `p` è il documento (secondo `stesso`) → `doc` così com'è;
/// - altrimenti si cerca il più vicino antenato di `p` che coincide con un
///   antenato di `doc` e `p` si riscrive sotto quell'antenato nella forma di
///   `doc` (rinomina nella stessa cartella o in un'altra cartella del volume);
/// - nessuna corrispondenza → `p` invariato.
///
/// `stesso` decide se due percorsi indicano lo stesso file/cartella
/// (di norma [`stesso_file`]): iniettato per testare la logica senza disco.
pub fn percorso_nella_forma_del_documento(
    doc: &Path,
    p: &Path,
    stesso: impl Fn(&Path, &Path) -> bool,
) -> PathBuf {
    if stesso(doc, p) {
        return doc.to_path_buf();
    }
    for ap in p
        .ancestors()
        .skip(1)
        .take_while(|a| !a.as_os_str().is_empty())
    {
        for ad in doc
            .ancestors()
            .skip(1)
            .take_while(|a| !a.as_os_str().is_empty())
        {
            if stesso(ap, ad) {
                if let Ok(resto) = p.strip_prefix(ap) {
                    return ad.join(resto);
                }
            }
        }
    }
    p.to_path_buf()
}

/// Via veloce di [`percorso_nella_forma_del_documento`] per la rinomina del
/// documento `vecchio` → `nuovo` (entrambi nella grafia canonica del watcher)
/// nella **stessa cartella**: il nuovo nome è `doc.parent()/nome`, senza alcuna
/// syscall (`stesso_file` canonicalizza, O(d²) chiamate: su un'unità di rete
/// lenta non va fatto sotto il lock di `Stato`). `None` se le cartelle
/// differiscono (confronto lessicale): serve allora la via completa.
pub fn nuovo_nome_nella_forma_del_documento(
    doc: &Path,
    vecchio: &Path,
    nuovo: &Path,
) -> Option<PathBuf> {
    let lessicale = |p: &Path| {
        crate::cronologia::normalizza_percorso(
            &p.to_string_lossy(),
            crate::cronologia::fs_senza_maiuscole(),
        )
    };
    let (cartella_v, cartella_n) = (vecchio.parent()?, nuovo.parent()?);
    if lessicale(cartella_v) != lessicale(cartella_n) {
        return None;
    }
    Some(doc.parent()?.join(nuovo.file_name()?))
}

// ---- Integrità e ripristino del documento sparito --------------------------

/// Esito della verifica di integrità del testo dell'editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrita {
    /// Il testo si può salvare.
    Integro,
    /// Testo vuoto (o di soli spazi) mentre il file salvato non lo era.
    Vuoto,
    /// Testo non UTF-8 valido.
    NonUtf8,
    /// Testo che è un troncamento (prefisso proprio) dell'ultima versione nota.
    Troncato,
}

/// Verifica che `buffer` (il testo dell'editor) sia salvabile (D-014).
///
/// Criteri: UTF-8 valido; non vuoto se il file salvato non lo era
/// (`file_non_vuoto`); non un troncamento — prefisso proprio — dell'ultima
/// versione nota (`ultimo_noto`, di norma l'ultima voce di cronologia). **Non**
/// si richiede che il documento compili: gli errori di sintassi non rendono
/// un documento "compromesso". Nel dubbio si sbaglia per prudenza (chiedere
/// dove salvare), mai per perdita: chi ha davvero accorciato il testo a mano
/// dovrà solo scegliere il percorso.
pub fn verifica_integrita(
    buffer: &[u8],
    ultimo_noto: Option<&[u8]>,
    file_non_vuoto: bool,
) -> Integrita {
    if std::str::from_utf8(buffer).is_err() {
        return Integrita::NonUtf8;
    }
    if file_non_vuoto && buffer.iter().all(u8::is_ascii_whitespace) {
        return Integrita::Vuoto;
    }
    if let Some(u) = ultimo_noto {
        if buffer.len() < u.len() && u.starts_with(buffer) {
            return Integrita::Troncato;
        }
    }
    Integrita::Integro
}

/// Esito di [`ripristina_documento`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EsitoRipristino {
    /// File ricreato al percorso originale (dopo aver registrato la versione).
    Ripristinato,
    /// Testo non integro: **nessun** salvataggio, chiedere dove salvare.
    NonIntegro(Integrita),
}

/// Ripristina un documento sparito dal disco (D-014).
///
/// Se `contenuto` è integro ([`verifica_integrita`], con l'ultima versione
/// della `cronologia` come riferimento): prima registra una voce in cronologia
/// (così nulla si perde; un errore della cronologia non impedisce il
/// ripristino), poi riscrive il file al percorso originale passando dal
/// `registro` delle scritture proprie (nessun nuovo evento "modificato da
/// fuori"). Altrimenti non scrive nulla.
pub fn ripristina_documento(
    percorso: &Path,
    contenuto: &str,
    file_non_vuoto: bool,
    cronologia: Option<&Cronologia>,
    ora: NaiveDateTime,
    max_versioni: usize,
    registro: &mut RegistroScritture,
) -> io::Result<EsitoRipristino> {
    let ultimo = cronologia.and_then(|c| c.ultima(percorso));
    let esito = verifica_integrita(
        contenuto.as_bytes(),
        ultimo.as_deref().map(str::as_bytes),
        file_non_vuoto,
    );
    if esito != Integrita::Integro {
        return Ok(EsitoRipristino::NonIntegro(esito));
    }
    if let Some(c) = cronologia {
        if let Err(e) = c.registra(percorso, contenuto, ora, max_versioni, None) {
            eprintln!("Cronologia non aggiornata prima del ripristino: {e}");
        }
    }
    scrivi_registrando(registro, percorso, contenuto.as_bytes())?;
    Ok(EsitoRipristino::Ripristinato)
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::NaiveDate;

    /// `stesso` simulato: `Z:` è un `subst` di `C:\reale` e il watcher emette la
    /// forma canonica.
    fn stesso_subst(a: &Path, b: &Path) -> bool {
        let canon = |p: &Path| {
            let t = p.to_string_lossy().replace('\\', "/");
            match t.strip_prefix("Z:") {
                Some(r) => format!("C:/reale{r}"),
                None => t,
            }
            .trim_end_matches('/')
            .to_string()
        };
        canon(a) == canon(b)
    }

    #[test]
    fn nuovo_nome_nella_stessa_cartella_si_riscrive_senza_toccare_il_disco() {
        // Percorsi inesistenti: la funzione è puramente lessicale.
        let doc = Path::new("/fe/prog/main.typ");
        let vecchio = Path::new("/reale/prog/main.typ");
        assert_eq!(
            nuovo_nome_nella_forma_del_documento(doc, vecchio, Path::new("/reale/prog/nuovo.typ")),
            Some(PathBuf::from("/fe/prog/nuovo.typ"))
        );
        // Cartella diversa: serve il confronto sul disco, la via veloce rinuncia.
        assert_eq!(
            nuovo_nome_nella_forma_del_documento(doc, vecchio, Path::new("/reale/altra/nuovo.typ")),
            None
        );
    }

    #[test]
    fn percorso_emesso_per_il_documento_usa_la_forma_del_frontend() {
        let doc = Path::new("Z:\\prog\\main.typ");
        let evento = Path::new("C:\\reale\\prog\\main.typ");
        assert_eq!(
            percorso_nella_forma_del_documento(doc, evento, stesso_subst),
            PathBuf::from(doc)
        );
    }

    #[test]
    fn percorso_emesso_per_la_rinomina_resta_nella_forma_del_frontend() {
        let doc = Path::new("Z:\\prog\\main.typ");
        // Stessa cartella.
        assert_eq!(
            percorso_nella_forma_del_documento(
                doc,
                Path::new("C:\\reale\\prog\\nuovo.typ"),
                stesso_subst
            ),
            PathBuf::from("Z:\\prog\\nuovo.typ")
        );
        // Altra cartella dello stesso volume (antenato comune più in alto).
        assert_eq!(
            percorso_nella_forma_del_documento(
                doc,
                Path::new("C:\\reale\\altra\\sotto\\n.typ"),
                stesso_subst
            ),
            PathBuf::from("Z:\\altra\\sotto\\n.typ")
        );
    }

    #[test]
    fn percorso_emesso_fuori_dal_volume_resta_invariato() {
        let doc = Path::new("Z:\\prog\\main.typ");
        let altrove = Path::new("D:\\x\\y.typ");
        assert_eq!(
            percorso_nella_forma_del_documento(doc, altrove, stesso_subst),
            PathBuf::from(altrove)
        );
    }

    #[test]
    fn percorso_emesso_con_forma_gia_uguale_non_cambia() {
        let doc = Path::new("C:\\prog\\main.typ");
        let nuovo = Path::new("C:\\prog\\nuovo.typ");
        assert_eq!(
            percorso_nella_forma_del_documento(doc, nuovo, stesso_file),
            PathBuf::from(nuovo)
        );
    }

    fn imp(s: &str) -> Impronta {
        Impronta::di(s.as_bytes())
    }
    fn c(s: &str) -> Lettura {
        Lettura::Contenuto(imp(s))
    }
    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }
    fn ora() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 29)
            .and_then(|d| d.and_hms_opt(12, 0, 0))
            .expect("data")
    }

    // ---- registro / anti-loop ----

    #[test]
    fn scrittura_propria_ignorata_e_scrittura_esterna_rilevata() {
        let f = p("/prog/doc.typ");
        let mut reg = RegistroScritture::nuovo();
        let mut osv = Osservatore::nuovo();
        osv.imposta_noto(&f, Some(imp("vecchio")));

        // L'app scrive "nuovo": l'evento col contenuto "nuovo" è un'eco.
        reg.registra(&f, b"nuovo");
        assert_eq!(osv.su_evento(&f, c("nuovo"), &mut reg), Esito::Nessuno);
        // Eco ripetuta (cloud drive): ancora ignorata.
        assert_eq!(osv.su_evento(&f, c("nuovo"), &mut reg), Esito::Nessuno);
        // Stesso percorso, contenuto DIVERSO: scrittura esterna, va rilevata.
        assert_eq!(osv.su_evento(&f, c("altro"), &mut reg), Esito::Modificato);
        // La voce del registro è stata superata: tornare a "nuovo" ora è esterno.
        assert_eq!(osv.su_evento(&f, c("nuovo"), &mut reg), Esito::Modificato);
    }

    #[test]
    fn registro_confronta_percorsi_normalizzati() {
        let mut reg = RegistroScritture::nuovo();
        reg.registra(Path::new(r"\\?\C:\prog\doc.typ"), b"x");
        assert!(reg.e_propria(Path::new("C:/prog/doc.typ"), &imp("x")));
    }

    #[test]
    fn scrivi_registrando_registra_prima_e_dimentica_su_errore() {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("doc.typ");
        let mut reg = RegistroScritture::nuovo();
        scrivi_registrando(&mut reg, &f, b"ciao").expect("scrittura");
        assert_eq!(std::fs::read(&f).expect("lettura"), b"ciao");
        assert!(reg.e_propria(&f, &imp("ciao")));

        // Scrittura impossibile (il bersaglio è una cartella): nessuna voce residua.
        let cartella = dir.path().join("cartella");
        std::fs::create_dir(&cartella).expect("dir");
        assert!(scrivi_registrando(&mut reg, &cartella, b"x").is_err());
        assert!(!reg.e_propria(&cartella, &imp("x")));
    }

    #[test]
    fn nomi_di_backup_riconosciuti() {
        for si in [
            "main.typ~",
            "main.typ.bak",
            "MAIN.ORIG",
            "x.swp",
            "x.tmp",
            "x.temp",
            ".#main.typ",
            "#main.typ#",
        ] {
            assert!(e_nome_di_backup(Path::new(si)), "{si}");
        }
        for no in ["main.typ", "capitolo.md", "backup.typ", "tmp.typ", "note"] {
            assert!(!e_nome_di_backup(Path::new(no)), "{no}");
        }
    }

    #[test]
    fn registro_pota_le_voci_fuori_dalla_radice() {
        // Cartelle vere: la chiave dei percorsi passa dalla canonicalizzazione.
        let dir = tempfile::tempdir().expect("tempdir");
        let base = std::fs::canonicalize(dir.path()).expect("canonicalize");
        for n in ["prog/sotto", "prog2", "altro"] {
            std::fs::create_dir_all(base.join(n)).expect("dir");
        }
        let mut reg = RegistroScritture::nuovo();
        let voci = [
            "prog/a.pdf",
            "prog/sotto/b.pdf",
            "prog2/c.pdf",
            "altro/d.pdf",
        ];
        for v in voci {
            reg.registra(&base.join(v), b"x");
        }
        reg.pota(&base.join("prog"));
        assert!(reg.ha_voce(&base.join("prog/a.pdf")));
        assert!(reg.ha_voce(&base.join("prog/sotto/b.pdf")));
        assert!(
            !reg.ha_voce(&base.join("prog2/c.pdf")),
            "`prog2` non sta sotto `prog`"
        );
        assert!(!reg.ha_voce(&base.join("altro/d.pdf")));
    }

    #[test]
    fn lettura_illeggibile_non_cambia_il_contenuto_noto_e_non_e_sparito() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        assert!(matches!(
            osv.su_evento(&f, Lettura::Illeggibile, &mut reg),
            Esito::Ricontrolla { .. }
        ));
        assert_eq!(osv.noto(&f), Some(imp("testo")));
        // Un ricontrollo illeggibile dopo "assente" non è "sparito".
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        assert!(matches!(
            osv.ricontrolla(&f, generazione, Lettura::Illeggibile, &mut reg),
            Esito::Ricontrolla { .. }
        ));
        assert_eq!(osv.noto(&f), Some(imp("testo")));
    }

    #[test]
    fn lettura_illeggibile_transitoria_poi_leggibile_rileva_la_modifica() {
        // Sharing violation durante la scrittura esterna: al ricontrollo il
        // file e' leggibile e la modifica non va persa.
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("vecchio")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Illeggibile, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        assert_eq!(
            osv.ricontrolla(&f, generazione, c("nuovo"), &mut reg),
            Esito::Modificato
        );
    }

    #[test]
    fn lettura_illeggibile_permanente_ha_un_tetto_di_tentativi() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Illeggibile, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        let mut richieste = 1;
        while let Esito::Ricontrolla { .. } =
            osv.ricontrolla(&f, generazione, Lettura::Illeggibile, &mut reg)
        {
            richieste += 1;
            assert!(richieste <= 100, "ciclo infinito");
        }
        assert_eq!(richieste, usize::from(MAX_TENTATIVI_ILLEGGIBILE));
        // Un nuovo evento riparte con un nuovo budget.
        assert!(matches!(
            osv.su_evento(&f, Lettura::Illeggibile, &mut reg),
            Esito::Ricontrolla { .. }
        ));
    }

    #[test]
    fn la_base_vista_non_avanza_con_gli_eventi() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("aperto")));
        assert_eq!(osv.su_evento(&f, c("esterno"), &mut reg), Esito::Modificato);
        assert_eq!(osv.noto(&f), Some(imp("esterno")));
        assert_eq!(
            osv.base(&f),
            Some(imp("aperto")),
            "l'editor non l'ha ancora visto"
        );
        osv.imposta_base(&f, Some(imp("esterno")));
        assert_eq!(osv.base(&f), Some(imp("esterno")));
    }

    // ---- macchina a stati ----

    #[test]
    fn stesso_contenuto_non_notifica() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("a")));
        assert_eq!(osv.su_evento(&f, c("a"), &mut reg), Esito::Nessuno);
        assert_eq!(osv.su_evento(&f, c("b"), &mut reg), Esito::Modificato);
        assert_eq!(osv.su_evento(&f, c("b"), &mut reg), Esito::Nessuno);
    }

    #[test]
    fn file_non_seguito_e_ignorato() {
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        assert_eq!(osv.su_evento(&p("/x"), c("a"), &mut reg), Esito::Nessuno);
    }

    #[test]
    fn troncamento_seguito_da_riscrittura_non_notifica() {
        // Salvataggio "tronca + riscrivi" di un altro editor con lo stesso testo.
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, c(""), &mut reg) else {
            panic!("il vuoto deve avviare il ricontrollo");
        };
        // Secondo evento: il contenuto è tornato uguale.
        assert_eq!(osv.su_evento(&f, c("testo"), &mut reg), Esito::Nessuno);
        // Il ricontrollo del primo evento è superato.
        assert_eq!(
            osv.ricontrolla(&f, generazione, c("testo"), &mut reg),
            Esito::Nessuno
        );
    }

    #[test]
    fn troncamento_seguito_da_contenuto_diverso_notifica_modifica() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        assert!(matches!(
            osv.su_evento(&f, c(""), &mut reg),
            Esito::Ricontrolla { .. }
        ));
        assert_eq!(
            osv.su_evento(&f, c("nuovo testo"), &mut reg),
            Esito::Modificato
        );
    }

    #[test]
    fn file_rimasto_vuoto_dopo_il_ricontrollo_e_una_modifica() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, c(""), &mut reg) else {
            panic!("ricontrollo atteso");
        };
        assert_eq!(
            osv.ricontrolla(&f, generazione, c(""), &mut reg),
            Esito::Modificato
        );
        // E ora, stabile a vuoto, un nuovo evento vuoto non notifica di nuovo.
        assert_eq!(osv.su_evento(&f, c(""), &mut reg), Esito::Nessuno);
    }

    #[test]
    fn file_sparito_oltre_il_ricontrollo_e_sparito() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        assert_eq!(
            osv.ricontrolla(&f, generazione, Lettura::Assente, &mut reg),
            Esito::Sparito
        );
        // Già sparito: altri eventi di rimozione non ripetono la notifica.
        assert_eq!(
            osv.su_evento(&f, Lettura::Assente, &mut reg),
            Esito::Nessuno
        );
    }

    #[test]
    fn sparito_e_ricomparso_entro_il_ricontrollo_col_contenuto_originale_non_notifica() {
        // Rinomina-via + rinomina-indietro (salvataggio con backup).
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        // Al ricontrollo il file è di nuovo lì, identico.
        assert_eq!(
            osv.ricontrolla(&f, generazione, c("testo"), &mut reg),
            Esito::Nessuno
        );
    }

    #[test]
    fn ricontrollo_di_una_generazione_vecchia_non_decide() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione: g1 } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        let Esito::Ricontrolla { generazione: g2 } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        assert_ne!(g1, g2);
        assert_eq!(
            osv.ricontrolla(&f, g1, Lettura::Assente, &mut reg),
            Esito::Nessuno
        );
        assert_eq!(
            osv.ricontrolla(&f, g2, Lettura::Assente, &mut reg),
            Esito::Sparito
        );
    }

    #[test]
    fn ultimo_esistente_sopravvive_alla_sparizione() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        osv.ricontrolla(&f, generazione, Lettura::Assente, &mut reg);
        assert_eq!(osv.noto(&f), None);
        assert_eq!(osv.ultimo_esistente(&f), Some(imp("testo")));
    }

    #[test]
    fn ripristino_proprio_dopo_sparizione_non_genera_notifica() {
        // Sparito → l'app ricrea il file registrando la scrittura: l'eco è ignorata.
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        let Esito::Ricontrolla { generazione } = osv.su_evento(&f, Lettura::Assente, &mut reg)
        else {
            panic!("ricontrollo atteso");
        };
        assert_eq!(
            osv.ricontrolla(&f, generazione, Lettura::Assente, &mut reg),
            Esito::Sparito
        );
        reg.registra(&f, b"testo dell'editor");
        assert_eq!(
            osv.su_evento(&f, c("testo dell'editor"), &mut reg),
            Esito::Nessuno
        );
    }

    #[test]
    fn scrittura_propria_di_file_vuoto_non_e_transitoria() {
        let f = p("/prog/doc.typ");
        let (mut reg, mut osv) = (RegistroScritture::nuovo(), Osservatore::nuovo());
        osv.imposta_noto(&f, Some(imp("testo")));
        reg.registra(&f, b"");
        assert_eq!(osv.su_evento(&f, c(""), &mut reg), Esito::Nessuno);
    }

    // ---- batching ----

    fn ev(tipo: TipoEvento, percorsi: &[&str]) -> EventoFs {
        EventoFs {
            tipo,
            percorsi: percorsi.iter().map(|s| p(s)).collect(),
        }
    }

    #[test]
    fn batch_si_chiude_dopo_il_silenzio() {
        let mut b = Batcher::nuovo();
        assert_eq!(b.scadenza(), None);
        assert!(!b.pronto(1_000_000));
        b.aggiungi(1000, ev(TipoEvento::Modificato, &["/a"]));
        assert!(!b.pronto(1099));
        b.aggiungi(1080, ev(TipoEvento::Modificato, &["/a"]));
        // Il secondo evento sposta la chiusura a 1180.
        assert!(!b.pronto(1179));
        assert!(b.pronto(1180));
        assert_eq!(b.svuota().len(), 2);
        assert_eq!(b.scadenza(), None);
    }

    #[test]
    fn batch_ha_un_tetto_anche_con_eventi_continui() {
        let mut b = Batcher::nuovo();
        let mut t = 0;
        while t <= 800 {
            b.aggiungi(t, ev(TipoEvento::Modificato, &["/a"]));
            t += 50; // sempre entro 100 ms dal precedente
        }
        // Tetto: 500 ms dal primo evento nonostante il flusso continuo.
        assert_eq!(b.scadenza(), Some(STARVE_MS));
        assert!(b.pronto(STARVE_MS));
    }

    #[test]
    fn rinomina_abbinata_nel_batch() {
        let b = riduci_eventi(&[
            ev(TipoEvento::RinominatoDa, &["/p/vecchio.typ"]),
            ev(TipoEvento::RinominatoA, &["/p/nuovo.typ"]),
        ]);
        assert_eq!(b.rinomine, vec![(p("/p/vecchio.typ"), p("/p/nuovo.typ"))]);
        assert!(b.rimossi.is_empty() && b.toccati.is_empty());
    }

    #[test]
    fn rinomina_both_e_deduplicata() {
        let b = riduci_eventi(&[
            ev(TipoEvento::RinominatoEntrambi, &["/p/a", "/p/b"]),
            ev(TipoEvento::RinominatoEntrambi, &["/p/a", "/p/b"]),
        ]);
        assert_eq!(b.rinomine.len(), 1);
    }

    #[test]
    fn rinomina_non_abbinabile_diventa_rimozione_e_creazione() {
        // Spostato fuori dalla cartella osservata: solo il From.
        let b = riduci_eventi(&[ev(TipoEvento::RinominatoDa, &["/p/doc.typ"])]);
        assert!(b.rinomine.is_empty());
        assert_eq!(b.rimossi, vec![p("/p/doc.typ")]);
        // Conteggi diversi (2 From, 1 To): ambiguo, niente abbinamenti a caso.
        let b = riduci_eventi(&[
            ev(TipoEvento::RinominatoDa, &["/p/a"]),
            ev(TipoEvento::RinominatoDa, &["/p/b"]),
            ev(TipoEvento::RinominatoA, &["/p/c"]),
        ]);
        assert!(b.rinomine.is_empty());
        assert_eq!(b.rimossi, vec![p("/p/a"), p("/p/b")]);
        assert_eq!(b.toccati, vec![p("/p/c")]);
    }

    #[test]
    fn eventi_duplicati_si_fondono_e_da_rileggere_include_le_destinazioni() {
        let b = riduci_eventi(&[
            ev(TipoEvento::Modificato, &["/p/a"]),
            ev(TipoEvento::Modificato, &["/p/a"]),
            ev(TipoEvento::Creato, &["/p/b"]),
            ev(TipoEvento::Rimosso, &["/p/c"]),
            ev(TipoEvento::RinominatoDa, &["/p/x"]),
            ev(TipoEvento::RinominatoA, &["/p/a"]),
        ]);
        assert_eq!(b.toccati, vec![p("/p/a"), p("/p/b")]);
        assert_eq!(b.rimossi, vec![p("/p/c")]);
        let r = da_rileggere(&b);
        assert_eq!(r, vec![p("/p/a"), p("/p/b"), p("/p/c")]);
    }

    #[test]
    fn percorsi_ignorati_nascosti_e_esclusi() {
        let radice = Path::new("/prog");
        assert!(percorso_ignorato(radice, Path::new("/prog/.git/HEAD")));
        assert!(percorso_ignorato(
            radice,
            Path::new("/prog/.doc.typ.tmp-1-2")
        ));
        assert!(percorso_ignorato(
            radice,
            Path::new("/prog/node_modules/x/y.js")
        ));
        assert!(percorso_ignorato(
            radice,
            Path::new("/prog/sez/__pycache__/m.pyc")
        ));
        assert!(percorso_ignorato(radice, Path::new("/prog/Thumbs.db")));
        // `target/` puo' essere una cartella di risorse dell'utente: il watcher
        // non la ignora, solo l'aggiornamento dell'albero si'.
        assert!(!percorso_ignorato(
            radice,
            Path::new("/prog/target/fig.png")
        ));
        assert!(percorso_fuori_dall_albero(
            radice,
            Path::new("/prog/target/fig.png")
        ));
        assert!(percorso_fuori_dall_albero(
            radice,
            Path::new("/prog/node_modules/x")
        ));
        assert!(!percorso_fuori_dall_albero(
            radice,
            Path::new("/prog/capitoli/uno.typ")
        ));
        assert!(!percorso_ignorato(
            radice,
            Path::new("/prog/capitoli/uno.typ")
        ));
        // Solo i componenti RELATIVI alla radice contano: una radice dentro una
        // cartella nascosta non azzera tutti gli eventi.
        assert!(!percorso_ignorato(
            Path::new("/home/.dati/prog"),
            Path::new("/home/.dati/prog/main.typ")
        ));
        assert!(!percorso_ignorato(radice, Path::new("/altrove/.x")));
    }

    // ---- decisione: tutte le combinazioni stato x evento ----

    #[test]
    fn decisione_per_il_documento_aperto() {
        let doc = p("/prog/main.typ");
        let cambio = Cambio::Contenuto(doc.clone());
        assert_eq!(decidi(Some(&doc), false, &cambio), vec![Azione::Ricarica]);
        assert_eq!(decidi(Some(&doc), true, &cambio), vec![Azione::Chiedi]);

        let sparito = Cambio::Sparito(doc.clone());
        for non_salvato in [false, true] {
            assert_eq!(
                decidi(Some(&doc), non_salvato, &sparito),
                vec![Azione::DocumentoSparito, Azione::AggiornaAlbero]
            );
        }

        let nuovo = p("/prog/rinominato.typ");
        let rin = Cambio::Rinominato {
            da: doc.clone(),
            a: nuovo.clone(),
        };
        for non_salvato in [false, true] {
            assert_eq!(
                decidi(Some(&doc), non_salvato, &rin),
                vec![
                    Azione::SeguiRinomina {
                        nuovo: nuovo.clone()
                    },
                    Azione::AggiornaAlbero
                ]
            );
        }
    }

    #[test]
    fn decisione_rinomina_verso_il_documento_e_un_cambio_di_contenuto() {
        let doc = p("/prog/main.typ");
        let rin = Cambio::Rinominato {
            da: p("/prog/.main.typ.swp"),
            a: doc.clone(),
        };
        assert_eq!(
            decidi(Some(&doc), false, &rin),
            vec![Azione::Ricarica, Azione::AggiornaAlbero]
        );
        assert_eq!(
            decidi(Some(&doc), true, &rin),
            vec![Azione::Chiedi, Azione::AggiornaAlbero]
        );
    }

    #[test]
    fn decisione_per_i_file_fratelli_non_chiede_mai() {
        let doc = p("/prog/main.typ");
        let altro = p("/prog/capitolo.typ");
        for non_salvato in [false, true] {
            assert_eq!(
                decidi(Some(&doc), non_salvato, &Cambio::Contenuto(altro.clone())),
                vec![Azione::InvalidaRisorse]
            );
            for cambio in [
                Cambio::Sparito(altro.clone()),
                Cambio::Rinominato {
                    da: altro.clone(),
                    a: p("/prog/altro.typ"),
                },
            ] {
                assert_eq!(
                    decidi(Some(&doc), non_salvato, &cambio),
                    vec![Azione::InvalidaRisorse, Azione::AggiornaAlbero]
                );
            }
        }
    }

    #[test]
    fn decisione_senza_documento_aperto_invalida_soltanto() {
        let cambio = Cambio::Contenuto(p("/prog/a.typ"));
        assert_eq!(decidi(None, false, &cambio), vec![Azione::InvalidaRisorse]);
    }

    #[test]
    fn decisione_riconosce_il_documento_a_prescindere_dalla_forma_del_percorso() {
        let cambio = Cambio::Contenuto(p(r"\\?\C:\Prog\Main.typ"));
        assert_eq!(
            decidi(Some(Path::new("C:/Prog/Main.typ")), false, &cambio),
            vec![Azione::Ricarica]
        );
    }

    // ---- integrità ----

    #[test]
    fn integrita_criteri() {
        assert_eq!(
            verifica_integrita(b"= Titolo", None, true),
            Integrita::Integro
        );
        // Vuoto: solo se il file salvato non lo era.
        assert_eq!(verifica_integrita(b"", None, true), Integrita::Vuoto);
        assert_eq!(verifica_integrita(b" \n\t", None, true), Integrita::Vuoto);
        assert_eq!(verifica_integrita(b"", None, false), Integrita::Integro);
        // UTF-8 non valido.
        assert_eq!(
            verifica_integrita(&[0x66, 0xff, 0xfe], None, false),
            Integrita::NonUtf8
        );
        // Troncamento rispetto all'ultima versione nota.
        assert_eq!(
            verifica_integrita(b"= Tito", Some(b"= Titolo\ntesto"), true),
            Integrita::Troncato
        );
        // Testo più corto ma NON prefisso (l'utente ha riscritto): integro.
        assert_eq!(
            verifica_integrita(b"= Altro", Some(b"= Titolo\ntesto"), true),
            Integrita::Integro
        );
        // Più lungo o uguale: integro.
        assert_eq!(
            verifica_integrita(b"= Titolo\ntesto\nfine", Some(b"= Titolo\ntesto"), true),
            Integrita::Integro
        );
        // Errori di sintassi Typst non contano.
        assert_eq!(
            verifica_integrita(b"#let x = (\n= Titolo [", None, true),
            Integrita::Integro
        );
    }

    fn cron() -> (tempfile::TempDir, Cronologia) {
        let dir = tempfile::tempdir().expect("tempdir");
        let c = Cronologia::in_radice(dir.path().join("cronologia"));
        (dir, c)
    }

    #[test]
    fn ripristino_integro_registra_in_cronologia_poi_ricrea_il_file() {
        let (dir, cr) = cron();
        let f = dir.path().join("doc.typ");
        let mut reg = RegistroScritture::nuovo();
        // Testo con errori di sintassi ma integro.
        let testo = "= Titolo\n#let x = (";
        let esito = ripristina_documento(&f, testo, true, Some(&cr), ora(), 10, &mut reg)
            .expect("ripristino");
        assert_eq!(esito, EsitoRipristino::Ripristinato);
        assert_eq!(std::fs::read_to_string(&f).expect("file"), testo);
        assert_eq!(cr.ultima(&f).as_deref(), Some(testo), "voce in cronologia");
        // Nessun evento di ritorno: la scrittura è registrata come propria.
        assert!(reg.e_propria(&f, &imp(testo)));
    }

    #[test]
    fn ripristino_rifiuta_buffer_vuoto_o_troncato_senza_scrivere() {
        let (dir, cr) = cron();
        let f = dir.path().join("doc.typ");
        let mut reg = RegistroScritture::nuovo();
        cr.registra(&f, "= Titolo\nun testo lungo", ora(), 10, None)
            .expect("storia");

        let vuoto =
            ripristina_documento(&f, "", true, Some(&cr), ora(), 10, &mut reg).expect("vuoto");
        assert_eq!(vuoto, EsitoRipristino::NonIntegro(Integrita::Vuoto));
        let troncato =
            ripristina_documento(&f, "= Titolo\nun te", true, Some(&cr), ora(), 10, &mut reg)
                .expect("troncato");
        assert_eq!(troncato, EsitoRipristino::NonIntegro(Integrita::Troncato));

        assert!(!f.exists(), "nessun file deve essere stato ricreato");
        assert_eq!(
            cr.elenca(&f).len(),
            1,
            "nessuna voce aggiunta alla cronologia"
        );
    }
}
