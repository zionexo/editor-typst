//! Ausili per l'**esportazione su file** delle conversioni.
//!
//! Le funzioni di conversione (`typst_a_testo`, …) sono pure; qui vive l'unico
//! codice con I/O del modulo, usato dal guscio: incorporamento sicuro delle
//! immagini in HTML (data-URI), controllo della destinazione e rilevamento
//! degli errori di sintassi.

use std::collections::HashSet;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::{data_uri, OpzioniHtml};

/// Tetto di dimensione per ogni immagine incorporata (8 MiB).
pub const MAX_BYTE_IMMAGINE: u64 = 8 * 1024 * 1024;
/// Tetto sul totale delle immagini incorporate in un documento (32 MiB).
pub const MAX_BYTE_TOTALE: u64 = 32 * 1024 * 1024;

/// `true` se il sorgente Typst contiene errori di sintassi.
pub fn ha_errori_sintassi(sorgente: &str) -> bool {
    typst_syntax::parse(sorgente).diagnosis().errors
}

/// Legge e codifica come data-URI l'immagine `relativo`, risolta **sotto**
/// `radice` (cartella del documento). Rifiuta con un motivo leggibile:
/// percorsi assoluti o con `..`/`:`, estensioni non immagine, file fuori dalla
/// radice (anche via symlink), non regolari o oltre `max_byte`.
pub fn incorpora_immagine(radice: &Path, relativo: &str, max_byte: u64) -> Result<String, String> {
    incorpora_immagine_da(radice, radice, relativo, max_byte)
}

/// Come [`incorpora_immagine`] con radice e cartella di partenza distinte, come
/// in Typst: un percorso che inizia con «/» è relativo alla `radice` (del
/// progetto), gli altri alla `base` (cartella del documento) e possono salire con
/// `..` **purché il file canonico resti sotto `radice`**. Percorsi assoluti, `:`
/// (unità, flussi alternativi) e componenti diversi da nomi e `..` sono rifiutati;
/// il confronto col prefisso avviene dopo `canonicalize` (symlink e giunzioni
/// compresi).
pub fn incorpora_immagine_da(
    radice: &Path,
    base: &Path,
    relativo: &str,
    max_byte: u64,
) -> Result<String, String> {
    let relativo = relativo.trim();
    // In Typst un percorso che inizia con «/» è relativo alla radice del progetto.
    let dalla_radice = relativo.starts_with(['/', '\\']);
    let rel = relativo.trim_start_matches(['/', '\\']);
    if rel.is_empty() || rel.contains(['\0', ':']) {
        return Err("percorso non valido".into());
    }
    let p = Path::new(rel);
    if !p
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::ParentDir))
    {
        return Err("percorso fuori dalla cartella del progetto".into());
    }
    let mime = match p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => return Err("formato non incorporabile".into()),
    };
    let radice_c =
        std::fs::canonicalize(radice).map_err(|e| format!("cartella illeggibile: {e}"))?;
    let partenza = if dalla_radice {
        radice_c.clone()
    } else {
        std::fs::canonicalize(base).map_err(|e| format!("cartella illeggibile: {e}"))?
    };
    let reale =
        std::fs::canonicalize(partenza.join(p)).map_err(|_| "file non trovato".to_string())?;
    if !reale.starts_with(&radice_c) {
        return Err("percorso fuori dalla cartella del progetto".into());
    }
    let meta = std::fs::metadata(&reale).map_err(|e| format!("illeggibile: {e}"))?;
    if !meta.is_file() {
        return Err("non è un file".into());
    }
    if meta.len() > max_byte {
        return Err(format!(
            "troppo grande ({} KiB, massimo {} KiB)",
            meta.len() / 1024,
            max_byte / 1024
        ));
    }
    let byte = std::fs::read(&reale).map_err(|e| format!("illeggibile: {e}"))?;
    Ok(data_uri(mime, &byte))
}

/// Opzioni HTML «autonomo»: le immagini sono incorporate come data-URI,
/// risolte sotto `radice`. Ritorna anche l'elenco (condiviso) dei motivi per cui
/// qualche immagine è rimasta un percorso relativo, da accodare agli avvisi.
pub fn opzioni_html_autonomo(radice: &Path) -> (OpzioniHtml, Arc<Mutex<Vec<String>>>) {
    opzioni_html_autonomo_con_tetti(radice, radice, MAX_BYTE_IMMAGINE, MAX_BYTE_TOTALE)
}

/// Opzioni HTML «autonomo» per un documento dentro un **progetto**: `/img/x.png`
/// si risolve sotto la radice del progetto, `img/x.png` e `../img/x.png` sotto la
/// cartella del documento, senza mai uscire dalla radice. Se la radice manca o
/// la cartella del documento non le appartiene, la radice è la cartella del
/// documento (comportamento di [`opzioni_html_autonomo`]).
pub fn opzioni_html_progetto(
    radice_progetto: Option<&Path>,
    cartella_documento: &Path,
) -> (OpzioniHtml, Arc<Mutex<Vec<String>>>) {
    let radice = radice_progetto
        .filter(|r| {
            match (
                std::fs::canonicalize(r),
                std::fs::canonicalize(cartella_documento),
            ) {
                (Ok(r), Ok(d)) => d.starts_with(r),
                _ => false,
            }
        })
        .unwrap_or(cartella_documento);
    opzioni_html_autonomo_con_tetti(
        radice,
        cartella_documento,
        MAX_BYTE_IMMAGINE,
        MAX_BYTE_TOTALE,
    )
}

/// Come [`opzioni_html_autonomo`] con radice, cartella di partenza e tetti
/// (per immagine e sul totale, in byte) espliciti: i tetti sono iniettabili per
/// poterli esercitare nei test senza file da decine di MiB.
pub fn opzioni_html_autonomo_con_tetti(
    radice: &Path,
    base: &Path,
    max_immagine: u64,
    max_totale: u64,
) -> (OpzioniHtml, Arc<Mutex<Vec<String>>>) {
    let mancate = Arc::new(Mutex::new(Vec::new()));
    let totale = Arc::new(AtomicU64::new(0));
    let radice = radice.to_path_buf();
    let base = base.to_path_buf();
    let mancate_cb = Arc::clone(&mancate);
    // insieme di appoggio per scartare i duplicati in O(1) (invece di
    // `Vec::contains`, quadratico con molte immagini mancanti)
    let gia_viste: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let opz = OpzioniHtml {
        risolvi_immagine: Some(Arc::new(move |p: &str| {
            let residuo = max_totale.saturating_sub(totale.load(Ordering::Relaxed));
            match incorpora_immagine_da(&radice, &base, p, max_immagine.min(residuo)) {
                Ok(uri) => {
                    // Byte effettivi: il base64 dopo la virgola ha 4 caratteri ogni 3 byte.
                    let base64 = uri.split_once(',').map_or(uri.len(), |(_, b)| b.len());
                    totale.fetch_add(base64 as u64 * 3 / 4, Ordering::Relaxed);
                    Some(uri)
                }
                Err(motivo) => {
                    let mut v = mancate_cb.lock().unwrap_or_else(|e| e.into_inner());
                    let msg = format!(
                        "immagine «{p}» non incorporata ({motivo}): resta il percorso relativo"
                    );
                    let mut viste = gia_viste.lock().unwrap_or_else(|e| e.into_inner());
                    if viste.insert(msg.clone()) {
                        v.push(msg);
                    }
                    None
                }
            }
        })),
        ..Default::default()
    };
    (opz, mancate)
}

/// `true` se il Markdown `documento` sta **dentro** la `radice` del progetto
/// aperto (confronto sui percorsi canonici: `..`, symlink e giunzioni che portano
/// fuori non contano). Senza documento (testo non salvato) o senza progetto la
/// risposta è `false`: il file è esterno o sconosciuto, quindi non fidato.
pub fn md_dentro_il_progetto(documento: Option<&Path>, radice: Option<&Path>) -> bool {
    let (Some(documento), Some(radice)) = (documento, radice) else {
        return false;
    };
    let Ok(radice) = std::fs::canonicalize(radice) else {
        return false;
    };
    crate::cronologia::percorso_stabile(documento).starts_with(radice)
}

/// Opzioni di conversione md→Typst per il Markdown `documento`: il Typst grezzo
/// `{=typst}` (che può contenere `#read`/`#include`) è ammesso **solo** per i
/// `.md` del progetto aperto ([`md_dentro_il_progetto`]); per un file esterno o
/// scaricato blocchi e span diventano codice letterale.
pub fn opzioni_md_per_documento(
    documento: Option<&Path>,
    radice: Option<&Path>,
) -> crate::markdown::OpzioniMd {
    crate::markdown::OpzioniMd {
        consenti_typst_grezzo: md_dentro_il_progetto(documento, radice),
    }
}

/// Controlla la destinazione di una conversione. Rifiuta il documento aperto e
/// i file sorgente (`.typ`/`.typc`/`.md`) — salvo `.md` quando `formato_md` è
/// `true` (esportazione in Markdown), che resta comunque vietato sul documento
/// aperto.
pub fn verifica_destinazione_conversione(
    destinazione: &Path,
    documento: Option<&Path>,
    formato_md: bool,
) -> Result<(), String> {
    let ext = destinazione
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if formato_md && ext.as_deref() == Some("md") {
        if documento.is_some_and(|d| crate::esterni::stesso_file(d, destinazione)) {
            return Err(format!(
                "La destinazione '{}' è il documento aperto: scegli un altro nome, \
                 l'export lo sovrascriverebbe.",
                destinazione.display()
            ));
        }
        // Nome non insidioso e nessun collegamento verso un sorgente Typst.
        return crate::esporta::verifica_nome_destinazione(destinazione, &["typ", "typc"]);
    }
    crate::esporta::verifica_destinazione_export(destinazione, documento)
}

#[cfg(test)]
#[path = "../../tests/unit/conversione_file.rs"]
mod test;
