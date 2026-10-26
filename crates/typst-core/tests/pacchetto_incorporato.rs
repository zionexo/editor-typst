//! Il pacchetto `gantt-newgen` incorporato nell'eseguibile deve compilare con
//! `#import "@local/gantt-newgen:0.1.0"` senza installazione e senza rete.
//!
//! I test usano `EditorWorld` senza progetto (documento singolo) e la risoluzione
//! standard dei pacchetti. Se sulla macchina esiste un `gantt-newgen` 0.1.0
//! installato in `@local`, quello prevale per costruzione (priorità all'utente):
//! i documenti restano comunque validi. La prova di priorità, isolata dalle
//! cartelle reali dell'utente, è nei test unitari di `world.rs`.

use typst_core::EditorWorld;

/// Compila `testo` come documento singolo: (pagine, messaggi di errore).
fn compila(testo: &str) -> (usize, Vec<String>) {
    let mut m = EditorWorld::nuovo(testo);
    let r = m.compila();
    let errori = r
        .diagnostici
        .iter()
        .filter(|d| d.gravita == "errore")
        .map(|d| d.messaggio.clone())
        .collect();
    (r.pagine.len(), errori)
}

#[test]
fn gantt_con_date_e_milestone_compila_offline() {
    let (pagine, errori) = compila(
        r#"#import "@local/gantt-newgen:0.1.0": gantt
#gantt((
  tasks: ((name: "A", start: "2026-01-05", end: "2026-01-12"),),
  milestones: ((name: "M", date: "2026-01-20", show-date: true),),
), locale: "it")
"#,
    );
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

#[test]
fn gantt_formato_gantty_con_sottoattivita_e_dipendenze_compila() {
    // Faccia «0.5» (gantty): `tasks[].subtasks[]`, dipendenza per `id`.
    let (pagine, errori) = compila(
        r#"#import "@local/gantt-newgen:0.1.0": gantt
#gantt((
  tasks: (
    (name: "Fase 1", subtasks: (
      (id: "a1", name: "Analisi", start: "2026-01-05", end: "2026-01-12"),
      (id: "a2", name: "Sviluppo", start: "2026-01-13", end: "2026-01-30",
       dependencies: ((id: "a1"),)),
    )),
  ),
), locale: "it")
"#,
    );
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

#[test]
fn gantt_formato_taskgroups_con_dipendenze_compila() {
    // Faccia «0.1»: `taskgroups[].tasks[]`.
    let (pagine, errori) = compila(
        r#"#import "@local/gantt-newgen:0.1.0": gantt
#gantt((
  taskgroups: (
    (name: "Fase 1", tasks: (
      (id: "a1", name: "Analisi", start: "2026-01-05", end: "2026-01-12"),
      (id: "a2", name: "Sviluppo", start: "2026-01-13", end: "2026-01-30",
       dependencies: ((id: "a1"),)),
    )),
  ),
))
"#,
    );
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

#[test]
fn versione_non_incorporata_da_errore_di_pacchetto_non_trovato() {
    let (pagine, errori) = compila(
        r#"#import "@local/gantt-newgen:9.9.9": gantt
#gantt((tasks: ()))
"#,
    );
    assert_eq!(pagine, 0, "la compilazione deve fallire");
    assert!(
        errori.iter().any(|e| e.contains("package not found")),
        "errore atteso di pacchetto non trovato: {errori:?}"
    );
}
