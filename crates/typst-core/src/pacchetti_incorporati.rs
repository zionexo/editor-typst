//! Pacchetti Typst incorporati nell'eseguibile.
//!
//! Oggi contiene un solo pacchetto, `gantt-newgen` (LGPL-3.0-or-later), servito
//! come **ripiego** dello spazio dei nomi `@local`: `#import
//! "@local/gantt-newgen:0.1.0": gantt` compila senza installazione e senza rete.
//! La priorità spetta sempre al pacchetto installato dall'utente (vedi
//! `CaricatoreFile` in `world.rs`): questo modulo viene interpellato solo quando
//! la risoluzione standard di `@local` fallisce.
//!
//! I file sono generati da `build.rs` (tabella `FILE_INCORPORATI`, ordinata per
//! percorso) a partire da `pacchetti/gantt-newgen/`, rigenerata a sua volta dallo
//! script `tools/sincronizza-gantt-newgen.ps1`. Il modulo è puro: nessun I/O,
//! nessuna rete, nessuna dipendenza da UI o trasporto.

use std::path::PathBuf;
use std::sync::OnceLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::Bytes;
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::VirtualPath;

include!(concat!(env!("OUT_DIR"), "/pacchetti_incorporati.rs"));

/// Spazio dei nomi in cui il pacchetto incorporato è visibile.
pub const NAMESPACE: &str = "local";

/// Nome del pacchetto incorporato.
pub const NOME: &str = "gantt-newgen";

/// Percorso (relativo alla radice del pacchetto) del manifest incorporato.
pub const FILE_MANIFEST: &str = "typst.toml";

/// Elenco ordinato `(percorso relativo con '/', contenuto)` dei file
/// incorporati. `ORIGINE.txt` non vi compare (vedi `build.rs`).
pub fn elenca() -> &'static [(&'static str, &'static [u8])] {
    FILE_INCORPORATI
}

/// Versione del pacchetto incorporato, letta dal `typst.toml` incorporato
/// (campi `package.name` e `package.version`). `None` se il manifest manca o è
/// malformato, oppure se il nome non coincide con [`NOME`]: in quel caso
/// nessuna richiesta viene servita.
pub fn versione() -> Option<PackageVersion> {
    static VERSIONE: OnceLock<Option<PackageVersion>> = OnceLock::new();
    *VERSIONE.get_or_init(|| {
        let contenuto = cerca(FILE_MANIFEST)?;
        let testo = std::str::from_utf8(contenuto).ok()?;
        let tabella: toml::Table = testo.parse().ok()?;
        let pacchetto = tabella.get("package")?.as_table()?;
        if pacchetto.get("name")?.as_str()? != NOME {
            return None;
        }
        pacchetto.get("version")?.as_str()?.parse().ok()
    })
}

/// Contenuto del file incorporato con questo percorso relativo (con `/`).
fn cerca(percorso: &str) -> Option<&'static [u8]> {
    // La tabella è ordinata per percorso (stesso ordine dei byte di `str`).
    FILE_INCORPORATI
        .binary_search_by(|(nome, _)| (*nome).cmp(percorso))
        .ok()
        .map(|i| FILE_INCORPORATI[i].1)
}

/// Vero se `percorso` (senza `/` finale; vuoto = radice) è una cartella del
/// pacchetto incorporato, cioè prefisso di almeno un file.
fn e_cartella(percorso: &str) -> bool {
    if percorso.is_empty() {
        return !FILE_INCORPORATI.is_empty();
    }
    FILE_INCORPORATI.iter().any(|(nome, _)| {
        nome.strip_prefix(percorso)
            .is_some_and(|resto| resto.starts_with('/'))
    })
}

/// Serve un file del pacchetto incorporato.
///
/// - `None`: `spec` non è il pacchetto incorporato (namespace diverso da
///   `local`, nome diverso o versione diversa da quella del manifest
///   incorporato). Il chiamante deve propagare l'errore originale.
/// - `Some(Ok(byte))`: file trovato.
/// - `Some(Err(FileError::IsDirectory))`: il percorso è una cartella.
/// - `Some(Err(FileError::NotFound(..)))`: nessun file con quel percorso.
pub fn file(spec: &PackageSpec, vpath: &VirtualPath) -> Option<FileResult<Bytes>> {
    if spec.namespace != NAMESPACE || spec.name != NOME || Some(spec.version) != versione() {
        return None;
    }
    let percorso = vpath.get_without_slash();
    Some(match cerca(percorso) {
        Some(contenuto) => Ok(Bytes::new(contenuto)),
        None if e_cartella(percorso) => Err(FileError::IsDirectory),
        None => Err(FileError::NotFound(PathBuf::from(vpath.get_with_slash()))),
    })
}

#[cfg(test)]
#[path = "../tests/unit/pacchetti_incorporati.rs"]
mod test;
