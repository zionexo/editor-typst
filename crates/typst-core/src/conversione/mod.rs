//! Esportazioni da sorgente Typst in testo semplice, HTML5 e LaTeX.
//!
//! Tre funzioni **pure** (nessun I/O, nessuna dipendenza da GUI o trasporto):
//! [`typst_a_testo`], [`typst_a_html`], [`typst_a_latex`]. Ciascuna legge il
//! sorgente con il parser di `typst-syntax` (senza eseguirlo), costruisce una
//! rappresentazione intermedia comune (`ir`) e la rende nel formato scelto.
//! Restituiscono sempre una [`Conversione`]: il contenuto più gli **avvisi**
//! sulle perdite, mai un errore né uno scarto silenzioso.
//!
//! # Cosa viene interpretato
//! Markup (titoli, enfasi, liste, termini, raw, link, riferimenti, etichette),
//! math, e le funzioni `figure`, `table` (con `table.header`/`table.cell`),
//! `image`, `quote`, `footnote`, `link`, `text`/`emph`/`strong`/`strike`/…,
//! `bibliography`, `outline`, `pagebreak`, `line`, `datetime(..).display(..)`;
//! le regole `#set` per lingua, titolo, autore e numerazioni
//! (`heading`, `math.equation`). Funzioni definite con `#let` nel documento
//! sono rese con i soli argomenti.
//!
//! # Limiti (segnalati negli avvisi)
//! Il codice non è valutato: `#if`/`#for`/`#context`, variabili non
//! letterali, figure disegnate con codice, regole di stile (`#set`/`#show`
//! diverse da quelle sopra) e la bibliografia (il `.bib` non è letto) sono
//! omessi o approssimati.

pub mod file;
mod html;
mod ir;
mod latex;
mod matematica;
mod testo;

/// Elenco di avvisi senza duplicati, condiviso con `markdown.rs`.
pub(crate) use ir::Avvisi;

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

/// Risultato di una conversione: contenuto e perdite segnalate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Conversione {
    /// Documento convertito (testo, HTML o LaTeX).
    pub contenuto: String,
    /// Avvisi in italiano su ciò che è stato omesso o approssimato; vuoto se
    /// la conversione è senza perdite note.
    pub avvisi: Vec<String>,
}

/// Opzioni del testo semplice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpzioniTesto {
    /// Larghezza di a capo in colonne (default 80; limitata a 20..=500).
    pub larghezza: usize,
}

impl Default for OpzioniTesto {
    fn default() -> Self {
        Self { larghezza: 80 }
    }
}

/// Funzione che, dato il percorso di un'immagine come scritto nel sorgente,
/// restituisce un URL alternativo (tipicamente un `data:image/…;base64,…`)
/// da usare al posto del percorso. È il chiamante, che può leggere i file, a
/// fornirla: il core resta privo di I/O. Vedi [`data_uri`].
pub type RisolviImmagine = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Opzioni dell'HTML.
#[derive(Clone, Default)]
pub struct OpzioniHtml {
    /// Titolo della pagina (`<title>`); se `None` si usa quello del
    /// documento (`#set document(title: …)`) o il primo titolo.
    pub titolo: Option<String>,
    /// Lingua (`lang`); se `None` quella dichiarata nel documento (`en` se assente).
    pub lingua: Option<String>,
    /// Sostituzione opzionale dei percorsi delle immagini (es. data-URI).
    /// Se `None` o se restituisce `None`, `src` resta il percorso relativo.
    pub risolvi_immagine: Option<RisolviImmagine>,
}

impl fmt::Debug for OpzioniHtml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpzioniHtml")
            .field("titolo", &self.titolo)
            .field("lingua", &self.lingua)
            .field("risolvi_immagine", &self.risolvi_immagine.is_some())
            .finish()
    }
}

/// Opzioni del LaTeX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpzioniLatex {
    /// Classe del documento (default `article`; `report`/`book` usano `\chapter`).
    pub classe: String,
    /// Corpo del testo: 10, 11 (default) o 12 pt.
    pub dimensione_pt: u8,
    /// Formato carta per la classe (default `a4paper`).
    pub carta: String,
    /// Lingua per `babel`; se `None` quella dichiarata nel documento.
    pub lingua: Option<String>,
    /// Emette `\title`/`\author`/`\maketitle` dai metadati `#set document`
    /// (default `false`: i documenti Typst hanno di norma il proprio frontespizio).
    pub titolo_automatico: bool,
}

impl Default for OpzioniLatex {
    fn default() -> Self {
        Self {
            classe: "article".into(),
            dimensione_pt: 11,
            carta: "a4paper".into(),
            lingua: None,
            titolo_automatico: false,
        }
    }
}

/// Accoda a `a` gli avvisi di `b` non già presenti, mantenendo l'ordine.
/// Usa un insieme di appoggio: costo lineare, non quadratico.
fn unisci(mut a: Vec<String>, b: ir::Avvisi) -> Vec<String> {
    let mut visti: HashSet<String> = a.iter().cloned().collect();
    for x in b.into_vec() {
        if visti.insert(x.clone()) {
            a.push(x);
        }
    }
    a
}

/// Converte un sorgente Typst in **testo semplice** (.txt).
///
/// Titoli sottolineati (con numerazione se il documento la imposta),
/// paragrafi a capo a `opz.larghezza` colonne, liste, citazioni rientrate,
/// note a piè di pagina numerate `[n]` in coda, tabelle ASCII, formule in
/// forma lineare Unicode e figure come `[Figura N: didascalia]`. L'enfasi
/// (corsivo, grassetto) non è rappresentabile e non viene segnalata.
pub fn typst_a_testo(sorgente: &str, opz: &OpzioniTesto) -> Conversione {
    let doc = ir::analizza(sorgente);
    let mut r = testo::Rend::nuovo(&doc, opz.larghezza.clamp(20, 500));
    let contenuto = r.documento();
    Conversione {
        contenuto,
        avvisi: doc.avvisi.clone(),
    }
}

/// Converte un sorgente Typst in una pagina **HTML5** autonoma (.html).
///
/// CSS inline sobrio (responsive e di stampa), struttura semantica
/// (`h1`–`h6`, `ul`/`ol`, `blockquote`, `figure`/`figcaption`, `table`,
/// note `sup` con elenco finale) e formule in **MathML** (ripiego `<code>`
/// con il sorgente Typst per quelle non convertibili). Ogni testo è scappato;
/// gli URL con schemi pericolosi sono rifiutati con un avviso.
pub fn typst_a_html(sorgente: &str, opz: &OpzioniHtml) -> Conversione {
    let doc = ir::analizza(sorgente);
    let mut r = html::Rend::nuovo(&doc, opz);
    let contenuto = r.documento();
    Conversione {
        contenuto,
        avvisi: unisci(doc.avvisi.clone(), r.avvisi),
    }
}

/// Converte un sorgente Typst in un documento **LaTeX** (.tex) per `pdflatex`.
///
/// `\documentclass`, pacchetti caricati solo se servono (`babel`, `amsmath`,
/// `graphicx`, `hyperref`, `ulem`), `\section…`, `itemize`/`enumerate`,
/// `quote`/`quotation`, `figure` con `\includegraphics` e `\caption`,
/// `\footnote`, `tabular`, math con `equation`/`aligned`, riferimenti con
/// `\ref`/`\eqref`/`\cite`. I caratteri speciali sono scappati.
pub fn typst_a_latex(sorgente: &str, opz: &OpzioniLatex) -> Conversione {
    let doc = ir::analizza(sorgente);
    let mut r = latex::Rend::nuovo(&doc, opz);
    let contenuto = r.documento();
    Conversione {
        contenuto,
        avvisi: unisci(doc.avvisi.clone(), r.avvisi),
    }
}

/// Codifica `dati` come URI `data:` (base64), ad esempio per
/// [`OpzioniHtml::risolvi_immagine`]: `data_uri("image/png", &byte)`.
pub fn data_uri(mime: &str, dati: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(dati.len() * 4 / 3 + mime.len() + 16);
    s.push_str("data:");
    s.push_str(mime);
    s.push_str(";base64,");
    for blocco in dati.chunks(3) {
        let b = [
            blocco[0],
            blocco.get(1).copied().unwrap_or(0),
            blocco.get(2).copied().unwrap_or(0),
        ];
        s.push(T[(b[0] >> 2) as usize] as char);
        s.push(T[(((b[0] & 3) << 4) | (b[1] >> 4)) as usize] as char);
        s.push(if blocco.len() > 1 {
            T[(((b[1] & 15) << 2) | (b[2] >> 6)) as usize] as char
        } else {
            '='
        });
        s.push(if blocco.len() > 2 {
            T[(b[2] & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

#[cfg(test)]
mod test_comuni {
    use super::*;

    #[test]
    fn data_uri_base64() {
        assert_eq!(
            data_uri("text/plain", b"Man"),
            "data:text/plain;base64,TWFu"
        );
        assert_eq!(data_uri("x/y", b"Ma"), "data:x/y;base64,TWE=");
        assert_eq!(data_uri("x/y", b"M"), "data:x/y;base64,TQ==");
        assert_eq!(data_uri("x/y", b""), "data:x/y;base64,");
    }
}

#[cfg(test)]
mod test_correzioni;
#[cfg(test)]
mod test_costrutti;
