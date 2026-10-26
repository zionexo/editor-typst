/// Script di build per il guscio Tauri.
///
/// Invoca `tauri_build::build()` per generare le risorse Tauri necessarie
/// (icone, side car manifest, ecc.) durante la compilazione.
fn main() {
    tauri_build::build()
}
