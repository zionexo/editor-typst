//! Salvataggio di un'immagine incollata dagli appunti (funzione "Incolla immagine").
//!
//! Funzione di sola I/O, senza Tauri: il frontend manda nome e byte, il core
//! controlla tutto e scrive `<cartella>/img/<nome>`. Regole:
//! - **nome**: un semplice nome file (`[A-Za-z0-9._-]`, niente `..`, niente
//!   separatori), con estensione minuscola `png`, `jpg`, `gif` o `webp`;
//! - **byte**: non vuoti, al massimo [`IMMAGINE_MAX_BYTE`], con la firma del
//!   formato indicato dall'estensione;
//! - **mai sovrascrivere**: un file già esistente resta intatto.

use std::path::Path;

use crate::config::scrivi_atomico;

/// Dimensione massima di un'immagine incollata (10 MiB).
pub const IMMAGINE_MAX_BYTE: usize = 10 * 1024 * 1024;

/// Cartella (relativa al documento) in cui si salvano le immagini incollate.
const CARTELLA_IMMAGINI: &str = "img";

/// Estensioni ammesse (minuscole).
const ESTENSIONI: &[&str] = &["png", "jpg", "gif", "webp"];

/// Controlla che `nome` sia un semplice nome file con estensione ammessa.
/// Ritorna l'estensione.
fn verifica_nome(nome: &str) -> Result<&str, String> {
    let caratteri_validi = nome
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if nome.is_empty() || !caratteri_validi || nome.contains("..") {
        return Err(format!(
            "Nome file non valido: «{nome}». Usa solo lettere, cifre, «.», «_» e «-», \
             senza «..» e senza separatori di percorso."
        ));
    }
    // Serve un nome prima dell'estensione: «.png» da solo non è ammesso.
    match nome.rsplit_once('.') {
        Some((base, est)) if !base.is_empty() && ESTENSIONI.contains(&est) => Ok(est),
        _ => Err(format!(
            "Estensione non ammessa nel nome «{nome}». Estensioni ammesse (minuscole): {}.",
            ESTENSIONI.join(", ")
        )),
    }
}

/// `true` se `byte` inizia con la firma del formato indicato da `estensione`.
fn firma_coerente(estensione: &str, byte: &[u8]) -> bool {
    match estensione {
        "png" => byte.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "jpg" => byte.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => byte.starts_with(b"GIF87a") || byte.starts_with(b"GIF89a"),
        "webp" => byte.len() >= 12 && &byte[..4] == b"RIFF" && &byte[8..12] == b"WEBP",
        _ => false,
    }
}

/// Salva un'immagine incollata in `<cartella>/img/<nome>`; crea `img/` se manca.
///
/// # Parametri
/// - `cartella`: cartella del documento (deve esistere ed essere una cartella);
/// - `nome`: nome del file, vedi le regole nella documentazione del modulo;
/// - `byte`: contenuto dell'immagine.
///
/// La scrittura è atomica (vedi [`scrivi_atomico`]). Non sovrascrive: il
/// controllo dell'esistenza e la scrittura non sono un'unica operazione, quindi
/// un file creato da un altro processo proprio fra i due passi potrebbe essere
/// sostituito (finestra minima, accettata: il nome contiene data e ora).
///
/// # Ritorno
/// Percorso relativo alla cartella, con barra normale (es. `"img/img-1.png"`).
///
/// # Errori
/// `Err` con un messaggio in italiano se: il nome non è valido, il formato non
/// è riconosciuto, i byte sono vuoti o troppi, la cartella non esiste, il file
/// esiste già o la scrittura fallisce. Prima di ogni errore di validazione non
/// si crea nulla su disco.
pub fn salva_immagine_incollata(
    cartella: &Path,
    nome: &str,
    byte: &[u8],
) -> Result<String, String> {
    let estensione = verifica_nome(nome)?;
    if byte.is_empty() {
        return Err("L'immagine incollata è vuota.".to_string());
    }
    if byte.len() > IMMAGINE_MAX_BYTE {
        return Err(format!(
            "L'immagine è troppo grande ({} byte): il limite è {} MiB.",
            byte.len(),
            IMMAGINE_MAX_BYTE / (1024 * 1024)
        ));
    }
    if !firma_coerente(estensione, byte) {
        return Err(format!(
            "Formato dell'immagine non riconosciuto: il contenuto non è un file .{estensione}."
        ));
    }
    // Un percorso relativo (su Windows anche «C:», relativo all'unità) farebbe
    // scrivere nella cartella corrente del processo, non in quella del documento.
    if !cartella.is_absolute() {
        return Err(format!(
            "La cartella del documento deve essere un percorso assoluto: «{}».",
            cartella.display()
        ));
    }
    if !cartella.is_dir() {
        return Err(format!(
            "La cartella del documento non esiste: «{}».",
            cartella.display()
        ));
    }

    let destinazione = cartella.join(CARTELLA_IMMAGINI).join(nome);
    // `symlink_metadata`: vale anche per un collegamento simbolico penzolante.
    if std::fs::symlink_metadata(&destinazione).is_ok() {
        return Err(format!(
            "Il file «{CARTELLA_IMMAGINI}/{nome}» esiste già: non lo sovrascrivo."
        ));
    }
    scrivi_atomico(&destinazione, byte)
        .map_err(|e| format!("Impossibile salvare l'immagine «{CARTELLA_IMMAGINI}/{nome}»: {e}"))?;
    Ok(format!("{CARTELLA_IMMAGINI}/{nome}"))
}

#[cfg(test)]
mod test {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    #[test]
    fn caso_felice_scrive_il_file_e_crea_img() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!dir.path().join("img").exists());
        let rel = salva_immagine_incollata(dir.path(), "img-20261119-153012.png", PNG).unwrap();
        assert_eq!(rel, "img/img-20261119-153012.png");
        assert!(dir.path().join("img").is_dir());
        let scritto = std::fs::read(dir.path().join(&rel)).unwrap();
        assert_eq!(scritto, PNG);
    }

    #[test]
    fn firme_valide_per_ogni_formato() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ".to_vec();
        webp.push(0);
        let casi: [(&str, &[u8]); 5] = [
            ("a.jpg", &[0xFF, 0xD8, 0xFF, 0xE0, 0]),
            ("b.gif", b"GIF87a\x01\x00"),
            ("c.gif", b"GIF89a\x01\x00"),
            ("d.webp", &webp),
            ("e.png", PNG),
        ];
        for (nome, byte) in casi {
            let rel = salva_immagine_incollata(dir.path(), nome, byte)
                .unwrap_or_else(|e| panic!("{nome}: {e}"));
            assert_eq!(rel, format!("img/{nome}"));
        }
    }

    #[test]
    fn nome_non_valido_e_rifiutato_senza_scrivere() {
        let dir = tempfile::tempdir().expect("tempdir");
        for nome in [
            "",
            "..",
            "../x.png",
            "a..b.png",
            "sub/x.png",
            "sub\\x.png",
            "x y.png",
            "è.png",
            ".png",
        ] {
            let e = salva_immagine_incollata(dir.path(), nome, PNG).unwrap_err();
            assert!(
                e.contains("non valido") || e.contains("Estensione"),
                "{nome:?}: {e}"
            );
        }
        assert!(!dir.path().join("img").exists(), "nulla scritto su disco");
    }

    #[test]
    fn estensione_non_ammessa_e_rifiutata() {
        let dir = tempfile::tempdir().expect("tempdir");
        for nome in ["x.bmp", "x.svg", "x.jpeg", "x.PNG", "x"] {
            let e = salva_immagine_incollata(dir.path(), nome, PNG).unwrap_err();
            assert!(e.contains("Estensione non ammessa"), "{nome}: {e}");
        }
        assert!(!dir.path().join("img").exists());
    }

    #[test]
    fn firma_incoerente_e_rifiutata() {
        let dir = tempfile::tempdir().expect("tempdir");
        // PNG vero con estensione .jpg, testo con estensione .png, RIFF non WEBP.
        let casi: [(&str, &[u8]); 3] = [
            ("x.jpg", PNG),
            ("y.png", b"non e un'immagine"),
            ("z.webp", b"RIFF\x00\x00\x00\x00WAVEfmt "),
        ];
        for (nome, byte) in casi {
            let e = salva_immagine_incollata(dir.path(), nome, byte).unwrap_err();
            assert!(e.contains("non riconosciuto"), "{nome}: {e}");
        }
        assert!(!dir.path().join("img").exists());
    }

    #[test]
    fn byte_vuoti_sono_rifiutati() {
        let dir = tempfile::tempdir().expect("tempdir");
        let e = salva_immagine_incollata(dir.path(), "x.png", &[]).unwrap_err();
        assert!(e.contains("vuota"), "{e}");
        assert!(!dir.path().join("img").exists());
    }

    #[test]
    fn oltre_il_tetto_e_rifiutato_prima_di_scrivere() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut grande = PNG.to_vec();
        grande.resize(IMMAGINE_MAX_BYTE + 1, 0);
        let e = salva_immagine_incollata(dir.path(), "x.png", &grande).unwrap_err();
        assert!(e.contains("troppo grande"), "{e}");
        assert!(!dir.path().join("img").exists());
        // Esattamente al tetto: ammesso.
        grande.truncate(IMMAGINE_MAX_BYTE);
        assert!(salva_immagine_incollata(dir.path(), "x.png", &grande).is_ok());
    }

    #[test]
    fn file_esistente_non_e_sovrascritto() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("img")).unwrap();
        std::fs::write(dir.path().join("img/x.png"), b"originale").unwrap();
        let e = salva_immagine_incollata(dir.path(), "x.png", PNG).unwrap_err();
        assert!(e.contains("esiste già"), "{e}");
        assert_eq!(
            std::fs::read(dir.path().join("img/x.png")).unwrap(),
            b"originale"
        );
        // Nessun temporaneo orfano in img/.
        assert_eq!(
            std::fs::read_dir(dir.path().join("img")).unwrap().count(),
            1
        );
    }

    #[test]
    fn cartella_relativa_e_rifiutata_senza_scrivere() {
        #[cfg(windows)]
        let relative = ["C:", "relativa"];
        #[cfg(not(windows))]
        let relative = ["relativa"];
        for cartella in relative {
            let e = salva_immagine_incollata(Path::new(cartella), "x.png", PNG).unwrap_err();
            assert!(e.contains("percorso assoluto"), "{cartella}: {e}");
        }
        // Nulla scritto nella cartella corrente del processo.
        assert!(!Path::new("img").join("x.png").exists());
        #[cfg(windows)]
        assert!(!Path::new("C:img").join("x.png").exists());
    }

    #[test]
    fn cartella_inesistente_e_rifiutata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let assente = dir.path().join("non-esiste");
        let e = salva_immagine_incollata(&assente, "x.png", PNG).unwrap_err();
        assert!(e.contains("non esiste"), "{e}");
        assert!(!assente.exists(), "la cartella non va creata");
    }

    #[test]
    fn cartella_che_e_un_file_e_rifiutata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("doc.typ");
        std::fs::write(&file, b"x").unwrap();
        let e = salva_immagine_incollata(&file, "x.png", PNG).unwrap_err();
        assert!(e.contains("non esiste"), "{e}");
    }

    #[test]
    fn img_che_e_un_file_da_errore_di_io() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("img"), b"x").unwrap();
        let e = salva_immagine_incollata(dir.path(), "x.png", PNG).unwrap_err();
        assert!(e.contains("Impossibile salvare"), "{e}");
    }
}
