//! `typst-core` — crate di dominio puro di Typster.
//!
//! Nessuna dipendenza da `tauri`: la compilazione, l'assistenza IDE, la
//! gestione di pacchetti/progetti e la configurazione vivono qui, testabili
//! headless (vedi PROGETTO.md §5, DECISIONI.md D-003). Il guscio Tauri
//! (`src-tauri`) e il frontend dipendono da questo crate, mai il contrario.
//!
//! # Moduli
//! - [`world`]: `EditorWorld`, l'unico tipo che implementa `typst::World` e
//!   `typst_ide::IdeWorld` — compilazione, render SVG/PDF, IDE (completamento,
//!   hover, jump sorgente↔anteprima).
//! - [`esporta`]: opzioni e logica pura dell'export (intervalli di pagine,
//!   standard PDF/A, limiti PNG, nomi dei file per pagina).
//! - [`indice`]: indice dei titoli del documento compilato (F5-020).
//! - [`albero`]: albero dei file della cartella-progetto, sola consultazione (F5-021).
//! - [`cronologia`]: cronologia locale delle versioni di un documento (F5-013).
//! - [`esterni`]: logica pura del controllo dei file modificati da fuori
//!   (F5-022): anti-loop, macchina a stati, batching, decisione, integrità.
//! - [`sorveglianza`]: watcher `notify`, elaborazione dei batch di eventi e
//!   scritture registrate (F5-022).
//! - [`config`]: snippet riutilizzabili e impostazioni utente, letti da file
//!   esterni versionabili (scrittura atomica inclusa).
//! - [`packages`]: indice di Typst Universe, installazione `@preview`,
//!   creazione di progetti da template, pubblicazione `@local`.
//! - [`pacchetti_incorporati`]: pacchetto `gantt-newgen` incorporato
//!   nell'eseguibile, servito come ripiego di `@local` (l'installato prevale).
//! - [`progetto`]: rilevamento dell'entrypoint di una cartella-progetto.
//! - [`markdown`]: conversione bidirezionale Markdown ↔ Typst (Fase 6).
//! - [`modelli`]: modelli di documento con campi e segnaposto (intestazione
//!   `// @…`, modelli integrati e dell'utente).
//! - [`conversione`]: esportazioni pure da sorgente Typst in testo semplice,
//!   HTML5 e LaTeX, con elenco degli avvisi sulle perdite.

pub mod albero;
pub mod config;
pub mod conversione;
pub mod cronologia;
pub mod esporta;
pub mod esterni;
pub mod indice;
mod markdown;
pub mod modelli;
pub mod pacchetti_incorporati;
pub mod packages;
mod progetto;
pub mod sorveglianza;
mod world;

pub use config::{scrivi_atomico, Impostazioni, Snippet};
pub use conversione::file::{
    ha_errori_sintassi, incorpora_immagine_da, md_dentro_il_progetto, opzioni_html_autonomo,
    opzioni_html_autonomo_con_tetti, opzioni_html_progetto, opzioni_md_per_documento,
    verifica_destinazione_conversione,
};
pub use conversione::{
    data_uri, typst_a_html, typst_a_latex, typst_a_testo, Conversione, OpzioniHtml, OpzioniLatex,
    OpzioniTesto, RisolviImmagine,
};
pub use esporta::{OpzioniPdf, StandardPdf};
pub use indice::VoceIndice;
pub use markdown::{
    md_a_typst, md_a_typst_con_opzioni, typst_a_md, typst_a_md_con_avvisi, OpzioniMd,
};
pub use packages::EsitoElenco;
pub use progetto::{rileva_entrypoint, EntrypointRilevato};
pub use world::{
    Completamento, Diagnostico, EditorWorld, FontPrecaricabili, MetadatiPagina,
    PaginaDaRenderizzare, PaginaSvg, PagineDaEsportare, Posizione, RisultatoCompilazione,
    RisultatoCompletamento,
};

/// Nome della cartella dati del prodotto (sotto `config_dir()` e `cache_dir()`).
///
/// Centralizzato qui, come `AGENTE_UTENTE`, per non avere valori incoerenti
/// fra configurazione (`config.rs`) e cache (`packages.rs`).
pub(crate) const NOME_CARTELLA_DATI: &str = "typster";

/// Nome del file del registro dei crash, sotto `<config_dir>/typster/`.
const NOME_FILE_CRASH: &str = "crash.log";

/// Riga del registro dei crash: `<ora UTC RFC 3339> <posizione> <messaggio>`.
///
/// Funzione pura (testabile): una sola riga, con i ritorni a capo del messaggio
/// resi visibili come `\n` così ogni panic occupa esattamente una riga.
pub fn riga_crash(ora: chrono::DateTime<chrono::Utc>, posizione: &str, messaggio: &str) -> String {
    let messaggio = messaggio.replace('\r', "").replace('\n', "\\n");
    format!(
        "{} [{}] {}\n",
        ora.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        posizione,
        messaggio
    )
}

/// Appende una riga di crash (ora UTC corrente) a `<config_dir>/typster/crash.log`.
///
/// Pensata per il panic hook del guscio (con `panic = "abort"` e sottosistema
/// `windows` un panic altrimenti non lascia traccia): **best effort**, ogni
/// errore di I/O è ignorato e non va mai in panic.
pub fn registra_crash(posizione: &str, messaggio: &str) {
    use std::io::Write;
    let Some(base) = dirs::config_dir() else {
        return;
    };
    let cartella = base.join(NOME_CARTELLA_DATI);
    if std::fs::create_dir_all(&cartella).is_err() {
        return;
    }
    let riga = riga_crash(chrono::Utc::now(), posizione, messaggio);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(cartella.join(NOME_FILE_CRASH))
    {
        let _ = file.write_all(riga.as_bytes());
    }
}

/// User-Agent condiviso per tutte le richieste HTTP verso Typst Universe
/// (download di pacchetti `@preview` e font di sistema via `typst-kit`).
///
/// Centralizzato qui — anziché duplicato in `world.rs` e `packages.rs`, che
/// creano ciascuno un proprio `SystemPackages` stateless — così le due parti
/// del crate non possano mai finire con User-Agent incoerenti.
pub(crate) const AGENTE_UTENTE: &str = concat!("typster/", env!("CARGO_PKG_VERSION"));

/// Suffisso univoco (PID + nanosecondi correnti) per nomi di file/cartelle
/// temporanei: il solo PID potrebbe ripresentarsi (riuso da parte dell'OS) e
/// collidere con un residuo lasciato da un crash precedente.
pub(crate) fn suffisso_univoco() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    )
}

/// Crea un handle ai pacchetti standard (`@preview` via download di sistema,
/// `@local`/dati via le directory standard dell'OS). Stateless: ogni chiamata
/// crea un'istanza indipendente, così le operazioni di rete non condividono
/// stato con il `World` persistente né fra loro.
pub(crate) fn crea_system_packages() -> typst_kit::packages::SystemPackages {
    typst_kit::packages::SystemPackages::new(typst_kit::downloader::SystemDownloader::new(
        AGENTE_UTENTE,
    ))
}

/// Svuota completamente la cache di memoizzazione di `comemo`.
///
/// Usata dopo una pubblicazione riuscita di un pacchetto `@local`: una
/// precedente risoluzione fallita (`NotFound`) di quel `FileId` potrebbe
/// essere rimasta memoizzata, nascondendo il pacchetto appena scritto su
/// disco fino al prossimo sfratto naturale della cache.
///
/// # Nota di implementazione
/// Delega a `typst::comemo::evict` — il `comemo` **ri-esportato dal crate
/// `typst`** — l'unica istanza che traccia il `World`. Non esiste una
/// dipendenza diretta da `comemo`: due versioni separate darebbero due cache
/// distinte, e sfrattare quella sbagliata sarebbe un no-op silenzioso.
pub fn invalida_cache() {
    typst::comemo::evict(0);
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn riga_crash_e_una_sola_riga_con_ora_utc_e_posizione() {
        let ora = chrono::Utc.with_ymd_and_hms(2026, 9, 30, 8, 5, 3).unwrap();
        let riga = riga_crash(ora, "src/lib.rs:10:5", "prima\r\nseconda");
        assert_eq!(
            riga,
            "2026-09-30T08:05:03Z [src/lib.rs:10:5] prima\\nseconda\n"
        );
        assert_eq!(riga.matches('\n').count(), 1, "una sola riga: {riga:?}");
    }
}
