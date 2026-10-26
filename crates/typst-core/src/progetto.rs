//! Rilevamento dell'entrypoint di una cartella-progetto (Fase 4).
//!
//! Euristica in ordine di priorità: `main.typ` nella radice → campo
//! `[package].entrypoint` di un `typst.toml` nella radice → unico file
//! `.typ` nella radice. Nessuna scansione ricorsiva (evita `.git`,
//! `node_modules` e cartelle simili).

use std::path::{Path, PathBuf};

use typst::syntax::package::PackageManifest;
use typst::syntax::VirtualPath;

/// Esito del rilevamento: radice canonica del progetto ed entrypoint assoluto
/// al suo interno.
#[derive(Debug, Clone)]
pub struct EntrypointRilevato {
    /// Radice del progetto, canonicalizzata.
    pub radice: PathBuf,
    /// Percorso assoluto dell'entrypoint rilevato, sotto `radice`.
    pub entrypoint: PathBuf,
}

/// Rileva l'entrypoint di una cartella-progetto.
///
/// La radice è canonicalizzata **una sola volta**: l'entrypoint restituito
/// è costruito a partire da quella stessa radice canonica, senza
/// ricanonicalizzare, per non riaprire una finestra TOCTOU fra le due
/// operazioni (vedi DECISIONI.md F4-001, revisione #1).
pub fn rileva_entrypoint(cartella: &Path) -> Result<EntrypointRilevato, String> {
    let radice = std::fs::canonicalize(cartella).map_err(|e| {
        format!(
            "Cartella di progetto non valida '{}': {e}",
            cartella.display()
        )
    })?;

    if let Some(entrypoint) = prova_main_typ(&radice) {
        return Ok(EntrypointRilevato { radice, entrypoint });
    }
    if let Some(entrypoint) = prova_manifesto(&radice)? {
        return Ok(EntrypointRilevato { radice, entrypoint });
    }
    let entrypoint = prova_unico_typ(&radice)?;
    Ok(EntrypointRilevato { radice, entrypoint })
}

/// Euristica 1: un file `main.typ` direttamente nella radice.
fn prova_main_typ(radice: &Path) -> Option<PathBuf> {
    let candidato = radice.join("main.typ");
    candidato.is_file().then_some(candidato)
}

/// Euristica 2: `[package].entrypoint` di un `typst.toml` nella radice.
///
/// Un `typst.toml` illeggibile o malformato (TOML non valido / campi
/// mancanti) non è un errore fatale: si degrada silenziosamente
/// all'euristica successiva (revisione F4-001, #6). Un entrypoint
/// sintatticamente non valido (es. contiene `..`) o assente sul disco è
/// invece un errore esplicito: il manifesto è valido e dichiara un'intenzione
/// precisa, che merita un messaggio chiaro piuttosto che un fallback silenzioso.
fn prova_manifesto(radice: &Path) -> Result<Option<PathBuf>, String> {
    let percorso_manifesto = radice.join("typst.toml");
    let Ok(testo) = std::fs::read_to_string(&percorso_manifesto) else {
        return Ok(None);
    };
    let Ok(manifesto) = toml::from_str::<PackageManifest>(&testo) else {
        return Ok(None);
    };

    let entrypoint_dichiarato = manifesto.package.entrypoint.as_str();
    let vpath = VirtualPath::new(entrypoint_dichiarato).map_err(|e| {
        format!("'typst.toml' contiene un entrypoint non valido '{entrypoint_dichiarato}': {e}")
    })?;
    let entrypoint = vpath.realize(radice).map_err(|e| {
        format!("'typst.toml' contiene un entrypoint non valido '{entrypoint_dichiarato}': {e}")
    })?;
    if !entrypoint.is_file() {
        return Err(format!(
            "'typst.toml' indica l'entrypoint '{entrypoint_dichiarato}', ma il file non esiste"
        ));
    }
    Ok(Some(entrypoint))
}

/// Euristica 3: un unico file `.typ` nella radice (nessuna ricorsione).
///
/// L'estensione è confrontata **case-insensitive** (`eq_ignore_ascii_case`):
/// il filesystem di Windows è già case-insensitive, quindi un file come
/// `Documento.TYP` è comunque il file `.typ` dell'utente e va rilevato come
/// tale, non ignorato per un confronto sensibile al maiuscolo/minuscolo.
fn prova_unico_typ(radice: &Path) -> Result<PathBuf, String> {
    let voci = std::fs::read_dir(radice).map_err(|e| {
        format!(
            "Impossibile leggere la cartella '{}': {e}",
            radice.display()
        )
    })?;

    let mut trovati: Vec<PathBuf> = Vec::new();
    // A differenza di un `voci.flatten()`, le voci non leggibili (`Err`) non
    // vengono scartate silenziosamente: propagano un errore esplicito. Una
    // voce non leggibile ignorata in silenzio potrebbe far perdere di vista
    // l'unico `.typ` presente e restituire un fuorviante "nessun .typ
    // trovato" invece di segnalare il vero problema (basso impatto in
    // pratica, ma la propagazione è comunque più corretta e conservativa).
    for voce in voci {
        let voce = voce.map_err(|e| {
            format!(
                "Errore nella lettura di una voce della cartella '{}': {e}",
                radice.display()
            )
        })?;
        let percorso = voce.path();
        let e_typ = percorso
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("typ"));
        if e_typ && percorso.is_file() {
            trovati.push(percorso);
        }
    }

    match trovati.len() {
        0 => Err(format!(
            "Nessun file .typ trovato in '{}': nessun entrypoint individuabile",
            radice.display()
        )),
        1 => Ok(trovati.remove(0)),
        _ => Err(format!(
            "Più file .typ trovati in '{}': entrypoint ambiguo, specifica il file da aprire",
            radice.display()
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rileva_main_typ() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("main.typ"), "= Ciao").expect("scrittura fixture");
        std::fs::write(dir.path().join("altro.typ"), "= Altro").expect("scrittura fixture");

        let esito = rileva_entrypoint(dir.path()).expect("rilevamento riuscito");
        assert_eq!(esito.entrypoint.file_name().unwrap(), "main.typ");
    }

    #[test]
    fn rileva_da_manifesto() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("src")).expect("crea src");
        std::fs::write(dir.path().join("src").join("lib.typ"), "= Ciao")
            .expect("scrittura fixture");
        std::fs::write(
            dir.path().join("typst.toml"),
            "[package]\nname = \"prova\"\nversion = \"0.1.0\"\nentrypoint = \"src/lib.typ\"\n",
        )
        .expect("scrittura fixture");

        let esito = rileva_entrypoint(dir.path()).expect("rilevamento riuscito");
        assert_eq!(esito.entrypoint.file_name().unwrap(), "lib.typ");
        assert!(esito.entrypoint.starts_with(&esito.radice));
    }

    #[test]
    fn rifiuta_entrypoint_manifesto_con_path_traversal() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("typst.toml"),
            "[package]\nname = \"prova\"\nversion = \"0.1.0\"\nentrypoint = \"../fuori.typ\"\n",
        )
        .expect("scrittura fixture");

        assert!(rileva_entrypoint(dir.path()).is_err());
    }

    #[test]
    fn manifesto_malformato_ripiega_su_unico_typ() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("typst.toml"),
            "questo non è toml valido {{{",
        )
        .expect("scrittura fixture");
        std::fs::write(dir.path().join("documento.typ"), "= Ciao").expect("scrittura fixture");

        let esito = rileva_entrypoint(dir.path()).expect("dovrebbe ripiegare sull'unico .typ");
        assert_eq!(esito.entrypoint.file_name().unwrap(), "documento.typ");
    }

    #[test]
    fn rileva_unico_typ() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("documento.typ"), "= Ciao").expect("scrittura fixture");

        let esito = rileva_entrypoint(dir.path()).expect("rilevamento riuscito");
        assert_eq!(esito.entrypoint.file_name().unwrap(), "documento.typ");
    }

    #[test]
    fn rileva_unico_typ_estensione_maiuscola() {
        // Filesystem Windows: `.TYP` è comunque un file `.typ` per l'utente,
        // non va ignorato per un confronto sensibile al maiuscolo/minuscolo.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Documento.TYP"), "= Ciao").expect("scrittura fixture");

        let esito = rileva_entrypoint(dir.path()).expect("rilevamento riuscito");
        assert_eq!(esito.entrypoint.file_name().unwrap(), "Documento.TYP");
    }

    #[test]
    fn errore_se_piu_typ_ambigui() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.typ"), "= A").expect("scrittura fixture");
        std::fs::write(dir.path().join("b.typ"), "= B").expect("scrittura fixture");

        assert!(rileva_entrypoint(dir.path()).is_err());
    }

    #[test]
    fn errore_se_nessun_typ() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(rileva_entrypoint(dir.path()).is_err());
    }

    #[test]
    fn errore_su_cartella_inesistente() {
        let percorso = Path::new("questa/cartella/non/esiste/di/certo");
        assert!(rileva_entrypoint(percorso).is_err());
    }
}
