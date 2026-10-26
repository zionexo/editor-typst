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
mod test {
    use super::*;

    fn spec(testo: &str) -> PackageSpec {
        testo.parse().expect("spec valida")
    }

    fn spec_incorporato() -> PackageSpec {
        spec(&format!(
            "@{NAMESPACE}/{NOME}:{}",
            versione().expect("versione incorporata")
        ))
    }

    fn vp(p: &str) -> VirtualPath {
        VirtualPath::new(p).expect("percorso valido")
    }

    fn cartella_su_disco() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pacchetti/gantt-newgen")
    }

    #[test]
    fn versione_letta_dal_manifest_incorporato() {
        assert_eq!(versione().map(|v| v.to_string()).as_deref(), Some("0.1.0"));
    }

    #[test]
    fn trova_i_file_attesi() {
        let s = spec_incorporato();
        for percorso in [
            "lib.typ",
            "typst.toml",
            "src/render/chart.typ",
            "data/locales/it.toml",
            "LICENSE",
            "COPYING",
        ] {
            let byte = file(&s, &vp(percorso))
                .unwrap_or_else(|| panic!("`{percorso}`: pacchetto non riconosciuto"))
                .unwrap_or_else(|e| panic!("`{percorso}`: {e:?}"));
            assert!(!byte.is_empty(), "`{percorso}` vuoto");
        }
        // Il percorso con barra iniziale (forma rooted) equivale a quello relativo.
        assert!(matches!(file(&s, &vp("/lib.typ")), Some(Ok(_))));
    }

    #[test]
    fn percorso_inesistente_e_cartelle() {
        let s = spec_incorporato();
        assert!(matches!(
            file(&s, &vp("non/esiste.typ")),
            Some(Err(FileError::NotFound(_)))
        ));
        // Un prefisso parziale di nome file non è una cartella né un file.
        assert!(matches!(
            file(&s, &vp("li")),
            Some(Err(FileError::NotFound(_)))
        ));
        assert!(matches!(
            file(&s, &vp("src/render")),
            Some(Err(FileError::IsDirectory))
        ));
        assert!(matches!(
            file(&s, &vp("/")),
            Some(Err(FileError::IsDirectory))
        ));
    }

    #[test]
    fn pacchetto_diverso_non_e_riconosciuto() {
        let v = versione().expect("versione").to_string();
        let vp_lib = vp("lib.typ");
        assert!(file(&spec(&format!("@preview/{NOME}:{v}")), &vp_lib).is_none());
        assert!(file(&spec(&format!("@local/altro-nome:{v}")), &vp_lib).is_none());
        assert!(file(&spec(&format!("@local/{NOME}:9.9.9")), &vp_lib).is_none());
        assert!(file(&spec_incorporato(), &vp_lib).is_some());
    }

    #[test]
    fn origine_txt_non_e_incorporato() {
        assert!(elenca().iter().all(|(n, _)| !n.ends_with("ORIGINE.txt")));
        assert!(matches!(
            file(&spec_incorporato(), &vp("ORIGINE.txt")),
            Some(Err(FileError::NotFound(_)))
        ));
        // Il file esiste su disco: l'esclusione è deliberata, non un'assenza.
        assert!(cartella_su_disco().join("ORIGINE.txt").is_file());
    }

    /// Raccoglie `(relativo con '/', byte)` dei file su disco, senza ORIGINE.txt.
    fn su_disco(base: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        for voce in std::fs::read_dir(dir).expect("read_dir") {
            let p = voce.expect("voce").path();
            if p.is_dir() {
                su_disco(base, &p, out);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .expect("prefisso")
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel != "ORIGINE.txt" {
                    out.push((rel, std::fs::read(&p).expect("lettura")));
                }
            }
        }
    }

    #[test]
    fn elenco_coerente_con_la_cartella_su_disco() {
        let base = cartella_su_disco();
        let mut disco = Vec::new();
        su_disco(&base, &base, &mut disco);
        disco.sort();
        assert_eq!(elenca().len(), disco.len(), "numero di file diverso");
        for ((nome, byte), (nome_d, byte_d)) in elenca().iter().zip(&disco) {
            assert_eq!(nome, nome_d);
            assert_eq!(*byte, byte_d.as_slice(), "byte diversi per `{nome}`");
        }
        // Tabella ordinata e senza duplicati (necessario alla ricerca binaria).
        assert!(elenca().windows(2).all(|c| c[0].0 < c[1].0));
    }
}
