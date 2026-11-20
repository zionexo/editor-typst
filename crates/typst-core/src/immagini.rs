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
#[path = "../tests/unit/immagini.rs"]
mod test;
