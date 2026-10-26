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
mod test {
    use super::*;
    use crate::{typst_a_html, OpzioniHtml};

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 1, 2, 3, 4];

    fn cartella_con_immagini() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("img")).unwrap();
        std::fs::write(d.path().join("img/a.png"), PNG).unwrap();
        std::fs::write(d.path().join("img/b.svg"), b"<svg/>").unwrap();
        std::fs::write(d.path().join("nota.txt"), b"x").unwrap();
        d
    }

    #[test]
    fn incorpora_immagine_valida_e_radice_relativa() {
        let d = cartella_con_immagini();
        let uri = incorpora_immagine(d.path(), "img/a.png", 1024).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
        assert_eq!(
            uri,
            incorpora_immagine(d.path(), "/img/a.png", 1024).unwrap()
        );
        assert!(incorpora_immagine(d.path(), "img/b.svg", 1024)
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
    }

    #[test]
    fn rifiuta_traversal_assoluti_estensioni_e_dimensioni() {
        let d = cartella_con_immagini();
        // un file fuori dalla radice, ma esistente
        let fuori = d.path().parent().unwrap().join("fuori_typster_test.png");
        std::fs::write(&fuori, PNG).unwrap();
        let radice = d.path().join("img");
        for p in [
            "../fuori_typster_test.png",
            "img/../../fuori_typster_test.png",
            "..\\fuori_typster_test.png",
            "C:/Windows/x.png",
            "c:x.png",
            "http://a.it/x.png",
            "",
            "/",
            "a\0.png",
        ] {
            assert!(
                incorpora_immagine(&radice, p, 1024).is_err(),
                "{p:?} doveva essere rifiutato"
            );
        }
        let _ = std::fs::remove_file(&fuori);
        assert!(incorpora_immagine(d.path(), "nota.txt", 1024)
            .unwrap_err()
            .contains("formato"));
        assert!(incorpora_immagine(d.path(), "img/mancante.png", 1024).is_err());
        assert!(incorpora_immagine(d.path(), "img/a.png", 4)
            .unwrap_err()
            .contains("troppo grande"));
        // una cartella non è un file
        std::fs::create_dir_all(d.path().join("finta.png")).unwrap();
        assert!(incorpora_immagine(d.path(), "finta.png", 1024).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rifiuta_symlink_che_esce_dalla_radice() {
        let d = cartella_con_immagini();
        let fuori = tempfile::tempdir().unwrap();
        std::fs::write(fuori.path().join("s.png"), PNG).unwrap();
        std::os::unix::fs::symlink(fuori.path().join("s.png"), d.path().join("img/l.png")).unwrap();
        assert!(incorpora_immagine(d.path(), "img/l.png", 1024).is_err());
    }

    #[test]
    fn html_autonomo_incorpora_e_segnala_le_mancate() {
        let d = cartella_con_immagini();
        let (opz, mancate) = opzioni_html_autonomo(d.path());
        let c = typst_a_html(
            "#image(\"img/a.png\")\n\n#image(\"img/assente.png\")\n\n#image(\"../x.png\")",
            &opz,
        );
        assert!(
            c.contenuto.contains("src=\"data:image/png;base64,"),
            "{}",
            c.contenuto
        );
        assert!(c.contenuto.contains("src=\"img/assente.png\""));
        let m = mancate.lock().unwrap();
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m.iter().any(|x| x.contains("assente")));
        // senza opzione autonoma: percorsi relativi
        let c = typst_a_html("#image(\"img/a.png\")", &OpzioniHtml::default());
        assert!(c.contenuto.contains("src=\"img/a.png\""));
    }

    #[test]
    fn immagine_sopra_il_tetto_singolo_e_rifiutata() {
        // Il tetto sul TOTALE è in `tetto_totale_rifiuta_...`.
        let d = cartella_con_immagini();
        let (opz, mancate) = opzioni_html_autonomo(d.path());
        let f = opz.risolvi_immagine.as_ref().unwrap();
        assert!(f("img/a.png").is_some());
        std::fs::write(
            d.path().join("img/grande.png"),
            vec![0u8; (MAX_BYTE_IMMAGINE + 1) as usize],
        )
        .unwrap();
        assert!(f("img/grande.png").is_none());
        assert!(mancate
            .lock()
            .unwrap()
            .iter()
            .any(|m| m.contains("troppo grande")));
    }

    #[test]
    fn tetto_totale_rifiuta_la_seconda_immagine_che_lo_supera() {
        let d = tempfile::tempdir().unwrap();
        // Due immagini da 700 byte: ciascuna sotto il tetto singolo (1000), insieme
        // oltre il totale (1200).
        let mut png = PNG.to_vec();
        png.resize(700, 7);
        std::fs::write(d.path().join("a.png"), &png).unwrap();
        std::fs::write(d.path().join("b.png"), &png).unwrap();
        let (opz, mancate) = opzioni_html_autonomo_con_tetti(d.path(), d.path(), 1000, 1200);
        let f = opz.risolvi_immagine.as_ref().unwrap();
        assert!(f("a.png").is_some(), "la prima rientra nel totale");
        assert!(f("b.png").is_none(), "la seconda supera il totale");
        let m = mancate.lock().unwrap();
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(
            m[0].contains("b.png") && m[0].contains("troppo grande"),
            "{m:?}"
        );
        // Una terza, piccola, rientra comunque nel residuo? No: il residuo è 498 byte
        // e il file da 700 no, ma uno da 100 sì.
        std::fs::write(d.path().join("c.png"), &png[..100]).unwrap();
        assert!(f("c.png").is_some());
    }

    #[test]
    fn immagini_di_un_progetto_con_radice_diversa_dalla_cartella_del_documento() {
        let fuori = tempfile::tempdir().unwrap();
        let progetto = fuori.path().join("progetto");
        std::fs::create_dir_all(progetto.join("img")).unwrap();
        std::fs::create_dir_all(progetto.join("src")).unwrap();
        std::fs::write(progetto.join("img/x.png"), PNG).unwrap();
        std::fs::write(progetto.join("src/locale.png"), PNG).unwrap();
        std::fs::write(fuori.path().join("segreto.png"), PNG).unwrap();
        let doc = progetto.join("src");

        let ok = |p: &str| incorpora_immagine_da(&progetto, &doc, p, 1024);
        // Relativo alla radice del progetto, alla cartella del documento, con `..`.
        assert!(ok("/img/x.png").is_ok());
        assert!(ok("../img/x.png").is_ok());
        assert!(ok("locale.png").is_ok());
        // Un percorso relativo al documento non si risolve sotto la radice.
        assert!(ok("img/x.png").is_err());
        // Mai fuori dal progetto: il file esiste, ma è oltre la radice.
        assert!(fuori.path().join("segreto.png").is_file());
        for p in [
            "../../segreto.png",
            "/../segreto.png",
            "..\\..\\segreto.png",
            "../src/../../segreto.png",
        ] {
            let e = ok(p).unwrap_err();
            assert!(e.contains("fuori dalla cartella"), "{p:?}: {e}");
        }

        // Dal guscio: radice del progetto + cartella del documento.
        let (opz, mancate) = opzioni_html_progetto(Some(&progetto), &doc);
        let f = opz.risolvi_immagine.as_ref().unwrap();
        assert!(f("/img/x.png").is_some() && f("../img/x.png").is_some());
        assert!(f("../../segreto.png").is_none());
        assert_eq!(mancate.lock().unwrap().len(), 1);
    }

    #[test]
    fn senza_radice_o_con_documento_fuori_dalla_radice_vale_la_cartella_del_documento() {
        let d = cartella_con_immagini();
        let altro = tempfile::tempdir().unwrap();
        // Nessun progetto.
        let (opz, _) = opzioni_html_progetto(None, d.path());
        assert!(opz.risolvi_immagine.as_ref().unwrap()("img/a.png").is_some());
        // Radice che non contiene il documento: non si allarga a quella.
        std::fs::write(altro.path().join("fuori.png"), PNG).unwrap();
        let (opz, _) = opzioni_html_progetto(Some(altro.path()), d.path());
        let f = opz.risolvi_immagine.as_ref().unwrap();
        assert!(f("img/a.png").is_some());
        assert!(f("/img/a.png").is_some(), "«/» = cartella del documento");
    }

    #[test]
    fn destinazione_conversione() {
        let doc = Path::new("C:/lavoro/tesi.typ");
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/tesi.html"),
            Some(doc),
            false
        )
        .is_ok());
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/tesi.txt"),
            Some(doc),
            false
        )
        .is_ok());
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/tesi.typ"),
            Some(doc),
            false
        )
        .is_err());
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/altro.typ"),
            Some(doc),
            true
        )
        .is_err());
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/altro.md"),
            Some(doc),
            false
        )
        .is_err());
        assert!(verifica_destinazione_conversione(
            Path::new("C:/lavoro/altro.md"),
            Some(doc),
            true
        )
        .is_ok());
        // documento aperto Markdown: mai sovrascrivibile
        let md = Path::new("C:/lavoro/nota.md");
        assert!(
            verifica_destinazione_conversione(Path::new("C:/lavoro/nota.md"), Some(md), true)
                .is_err()
        );
    }

    #[test]
    fn typst_grezzo_ammesso_solo_per_i_md_del_progetto() {
        let fuori = tempfile::tempdir().unwrap();
        let progetto = fuori.path().join("progetto");
        std::fs::create_dir_all(progetto.join("cap")).unwrap();
        let dentro = progetto.join("cap").join("a.md");
        let esterno = fuori.path().join("scaricato.md");
        let accanto = fuori.path().join("progetto-bis").join("b.md");
        for f in [&dentro, &esterno] {
            std::fs::write(f, "x").unwrap();
        }
        std::fs::create_dir_all(accanto.parent().unwrap()).unwrap();
        std::fs::write(&accanto, "x").unwrap();
        let r = Some(progetto.as_path());

        assert!(md_dentro_il_progetto(Some(&dentro), r));
        // Non ancora salvato su disco, ma nella cartella del progetto.
        assert!(md_dentro_il_progetto(Some(&progetto.join("nuovo.md")), r));
        // Fuori, vicino di casa con prefisso comune, `..` che esce, nessun dato.
        assert!(!md_dentro_il_progetto(Some(&esterno), r));
        assert!(!md_dentro_il_progetto(Some(&accanto), r));
        let evasione = progetto
            .join("cap")
            .join("..")
            .join("..")
            .join("scaricato.md");
        assert!(!md_dentro_il_progetto(Some(&evasione), r));
        assert!(!md_dentro_il_progetto(Some(&dentro), None));
        assert!(!md_dentro_il_progetto(None, r));
        assert!(!md_dentro_il_progetto(
            Some(&dentro),
            Some(&progetto.join("manca"))
        ));

        // Effetto sulla conversione: `#read` letterale per il file esterno.
        let md = "```{=typst}\n#read(\"/segreto.txt\")\n```\n";
        let ok = crate::markdown::md_a_typst_con_opzioni(
            md,
            &opzioni_md_per_documento(Some(&dentro), r),
        );
        let no = crate::markdown::md_a_typst_con_opzioni(
            md,
            &opzioni_md_per_documento(Some(&esterno), r),
        );
        fn ha_chiamata(n: &typst_syntax::LinkedNode) -> bool {
            n.kind() == typst_syntax::SyntaxKind::FuncCall || n.children().any(|c| ha_chiamata(&c))
        }
        let eseguibile = |t: &str| {
            let radice = typst_syntax::parse(t);
            ha_chiamata(&typst_syntax::LinkedNode::new(&radice))
        };
        assert!(eseguibile(&ok), "progetto: Typst grezzo ammesso: {ok}");
        assert!(
            !eseguibile(&no),
            "esterno: nessuna chiamata di funzione: {no}"
        );
    }

    #[cfg(windows)]
    #[test]
    fn destinazione_markdown_con_nome_insidioso_e_rifiutata() {
        let d = tempfile::tempdir().unwrap();
        let doc = d.path().join("tesi.typ");
        std::fs::write(&doc, "= T").unwrap();
        for nome in ["tesi.typ.", "tesi.typ::$DATA", "nota.md::$DATA", "nota.md "] {
            let dest = d.path().join(nome);
            assert!(
                verifica_destinazione_conversione(&dest, Some(&doc), true).is_err(),
                "{nome:?}"
            );
        }
        assert!(
            verifica_destinazione_conversione(&d.path().join("nota.md"), Some(&doc), true).is_ok()
        );
    }

    #[test]
    fn errori_di_sintassi_rilevati() {
        assert!(ha_errori_sintassi("#let = ("));
        assert!(ha_errori_sintassi("*aperto"));
        assert!(!ha_errori_sintassi("= Titolo\n\ntesto *ok*"));
    }
}
