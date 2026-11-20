//! Indice del documento (titoli) per la barra laterale (F5-020, D-015/D-016).
//!
//! Qui vive la parte **pura** (senza `World` né compilazione): il tipo
//! serializzato verso il frontend, [`VoceIndice`], e [`appiattisci`], che
//! trasforma la lista piatta dei titoli in un albero con
//! `typst::model::OutlineNode::build_tree` (gestisce livelli saltati e antenati
//! esclusi) e lo restituisce in ordine di documento con la profondità di
//! annidamento. La raccolta dei titoli dal documento compilato sta in
//! `world.rs` (`EditorWorld::costruisci_indice`), perché serve il `World` per
//! mappare gli span sui file.
//!
//! L'uso di `OutlineNode::build_tree` con la lista `(elemento, livello,
//! incluso)` segue `typst-pdf` 0.15.0 (`src/outline.rs`, righe 11-39), con
//! `outlined` al posto di `bookmarked`, come fa `#outline` (D-015).

// Derivato da Typst typst-pdf/src/outline.rs (3ae52774b48987fc78a72ff483068cacc28e46c2, tag v0.15.0), licenza Apache-2.0 — Copyright Typst contributors

use std::num::NonZeroUsize;

use serde::Serialize;
use typst::model::OutlineNode;

/// Una voce dell'indice: un titolo del documento compilato.
///
/// Le posizioni (`riga`, `colonna`, `offset`) sono nel **file** che contiene il
/// titolo, non necessariamente nel documento aperto nell'editor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoceIndice {
    /// Livello del titolo (`=` → 1, `==` → 2, …), com'è scritto nel documento.
    pub livello: usize,
    /// Profondità di annidamento nell'albero (0 = radice): differisce dal
    /// livello quando un livello è saltato (`=` seguito da `===` → profondità 1).
    pub profondita: usize,
    /// Testo del titolo, in chiaro e senza spazi ai bordi.
    pub testo: String,
    /// `true` se il titolo è nel file principale del documento compilato (in
    /// documento singolo o Markdown è l'unico file navigabile).
    pub principale: bool,
    /// File che contiene il titolo, relativo alla radice del progetto e con
    /// separatore `/`. `None` per il file principale non agganciato a un
    /// progetto e per i titoli che vengono da pacchetti (non navigabili).
    pub file: Option<String>,
    /// Riga (0-based) del titolo nel suo file, se nota.
    pub riga: Option<usize>,
    /// Colonna (0-based, in caratteri) del titolo nel suo file, se nota.
    pub colonna: Option<usize>,
    /// Offset UTF-16 del titolo nel suo file, se noto (come `doc.length` di
    /// CodeMirror).
    pub offset: Option<usize>,
    /// Pagina (1-based) in cui compare il titolo nel documento compilato.
    pub pagina: usize,
}

/// Costruisce l'albero dei titoli e lo restituisce in ordine di documento.
///
/// Ogni elemento di `piatti` è `(elemento, livello, incluso)`: `incluso` è
/// `false` per un titolo con `outlined: false`, che non compare ma non perde i
/// figli (l'albero li aggancia al più vicino antenato incluso, come `#outline`).
/// Il risultato è `(elemento, livello, profondità)` in ordine di documento.
///
/// La visita è **iterativa** (pila esplicita): un documento con livelli
/// strettamente crescenti molto alti non deve poter esaurire lo stack.
pub(crate) fn appiattisci<T>(piatti: Vec<(T, NonZeroUsize, bool)>) -> Vec<(T, usize, usize)> {
    let albero = OutlineNode::build_tree(piatti);
    let mut uscita = Vec::new();
    let mut pila = vec![albero.into_iter()];
    while let Some(iteratore) = pila.last_mut() {
        match iteratore.next() {
            Some(nodo) => {
                let OutlineNode {
                    entry,
                    level,
                    children,
                } = nodo;
                // La profondità è il numero di iteratori "padre" ancora aperti.
                uscita.push((entry, level.get(), pila.len() - 1));
                pila.push(children.into_iter());
            }
            None => {
                pila.pop();
            }
        }
    }
    uscita
}

#[cfg(test)]
#[path = "../tests/unit/indice.rs"]
mod test;
