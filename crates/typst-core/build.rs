//! Script di build di `typst-core`: incorpora nell'eseguibile i file del
//! pacchetto Typst `gantt-newgen` (cartella `pacchetti/gantt-newgen/`).
//!
//! Genera in `OUT_DIR/pacchetti_incorporati.rs` una tabella ordinata
//! `&[(&str, &[u8])]` (percorso relativo con `/`, contenuto) costruita con
//! `include_bytes!` su percorsi **assoluti**: il progetto vive in un percorso
//! con spazi e `include_bytes!` è relativo al file generato (in `OUT_DIR`), non
//! ai sorgenti. Solo libreria standard, nessuna dipendenza di build.
//!
//! Il file `ORIGINE.txt` (appunti di provenienza scritti dallo script di
//! sincronizzazione) non fa parte del pacchetto e non viene incorporato.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Cartella (relativa al manifest) con i file del pacchetto da incorporare.
const CARTELLA_PACCHETTO: &str = "pacchetti/gantt-newgen";

/// File della radice del pacchetto che NON va incorporato.
const FILE_ESCLUSI: &[&str] = &["ORIGINE.txt"];

/// Raccoglie ricorsivamente i file sotto `base`, restituendo
/// (percorso relativo con `/`, percorso assoluto con `/`).
fn raccogli(base: &Path, corrente: &Path, uscita: &mut Vec<(String, String)>) {
    let voci = fs::read_dir(corrente)
        .unwrap_or_else(|e| panic!("impossibile leggere '{}': {e}", corrente.display()));
    for voce in voci {
        let voce =
            voce.unwrap_or_else(|e| panic!("voce non leggibile in '{}': {e}", corrente.display()));
        let percorso = voce.path();
        let tipo = voce
            .file_type()
            .unwrap_or_else(|e| panic!("tipo non leggibile per '{}': {e}", percorso.display()));
        if tipo.is_dir() {
            raccogli(base, &percorso, uscita);
        } else if tipo.is_file() {
            let relativo = percorso
                .strip_prefix(base)
                .unwrap_or_else(|e| panic!("prefisso inatteso per '{}': {e}", percorso.display()));
            let relativo = relativo
                .to_str()
                .unwrap_or_else(|| panic!("nome non UTF-8: '{}'", percorso.display()))
                .replace('\\', "/");
            if FILE_ESCLUSI.contains(&relativo.as_str()) {
                continue;
            }
            let assoluto = percorso
                .to_str()
                .unwrap_or_else(|| panic!("percorso non UTF-8: '{}'", percorso.display()))
                .replace('\\', "/");
            uscita.push((relativo, assoluto));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR non impostata da cargo"),
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR non impostata da cargo"));
    let base = manifest.join(CARTELLA_PACCHETTO);

    // Ricompila se cambia un file (o ne compare/sparisce uno) nella cartella.
    println!("cargo:rerun-if-changed={CARTELLA_PACCHETTO}");
    println!("cargo:rerun-if-changed=build.rs");

    let mut file = Vec::new();
    if base.is_dir() {
        raccogli(&base, &base, &mut file);
    } else {
        // La cartella è versionata: senza, l'exe uscirebbe senza il pacchetto.
        panic!(
            "cartella '{}' assente: pacchetto da incorporare mancante",
            base.display()
        );
    }
    file.sort();

    let mut codice = String::from(
        "// Generato da build.rs: NON modificare.\n\
         /// Tabella ordinata (percorso relativo con `/`, contenuto) del pacchetto incorporato.\n\
         pub(crate) static FILE_INCORPORATI: &[(&str, &[u8])] = &[\n",
    );
    for (relativo, assoluto) in &file {
        // `{:?}` produce un letterale di stringa Rust correttamente escapato
        // (apici, backslash, caratteri non ASCII): sicuro anche con gli spazi.
        let _ = writeln!(codice, "    ({relativo:?}, include_bytes!({assoluto:?})),");
    }
    codice.push_str("];\n");

    fs::write(out_dir.join("pacchetti_incorporati.rs"), codice)
        .expect("impossibile scrivere pacchetti_incorporati.rs in OUT_DIR");
}
