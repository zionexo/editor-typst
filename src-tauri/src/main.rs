// Disabilita la finestra console su Windows in modalità release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Punto di ingresso dell'applicazione desktop.
///
/// Delega tutta la logica alla funzione `run()` definita in `lib.rs`,
/// che si occupa di costruire e avviare l'applicazione Tauri.
fn main() {
    typster_lib::run();
}
