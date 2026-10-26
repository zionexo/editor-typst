//! I frammenti che lo strumento «Inserisci» del frontend genera
//! (`src/pannelli/inserisci-logica.ts`) devono essere Typst valido: qui si
//! compilano davvero. Le stringhe sono copiate dal check TypeScript
//! (`inserisci.check.ts`); se cambiano là vanno aggiornate anche qui.

use std::path::PathBuf;

use typst_core::EditorWorld;

fn cartella() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prova-export")
}

/// Compila `testo` come documento principale della cartella di prova.
fn compila(testo: &str) -> (usize, Vec<String>) {
    let mut m = EditorWorld::nuovo("");
    m.apri_progetto(cartella(), "scientifico.typ", testo)
        .expect("apri_progetto");
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
fn tabella_con_intestazione_compila() {
    let (pagine, errori) =
        compila("#table(\n  columns: 2,\n  table.header[Colonna 1][Colonna 2],\n  [], [],\n)\n");
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

#[test]
fn tabella_solo_intestazione_compila() {
    let (_, errori) =
        compila("#table(\n  columns: 3,\n  table.header[Colonna 1][Colonna 2][Colonna 3],\n)\n");
    assert!(errori.is_empty(), "{errori:?}");
}

#[test]
fn equazione_nota_e_riferimento_compilano() {
    let (_, errori) = compila(
        r#"#set heading(numbering: "1.")
= Titolo <intro>

Vedi @intro#footnote[Fonte: \#x].

In linea $x^2$ e
$ x^2 + y^2 = r^2 $
"#,
    );
    assert!(errori.is_empty(), "{errori:?}");
}

#[test]
fn figura_con_didascalia_ed_etichetta_compila() {
    // Il contenuto generato per `image(...)` è sostituito da un corpo di prova: qui si
    // verifica la struttura `#figure(...) <etichetta>` con didascalia escapata.
    let (_, errori) = compila(
        r#"#figure(
  [corpo],
  caption: [Una \[prova\] \#1],
) <fig:a>

Vedi @fig:a.
"#,
    );
    assert!(errori.is_empty(), "{errori:?}");
}

#[test]
fn figura_con_percorso_inesistente_ha_solo_errore_di_file() {
    // La sintassi è valida: l'unico errore deve essere il file mancante.
    let (_, errori) = compila("#figure(\n  image(\"img/non-esiste.png\"),\n)\n");
    assert_eq!(errori.len(), 1, "{errori:?}");
}

/// Le strutture native dello strumento «Equazioni» (`src/pannelli/equazioni-logica.ts`,
/// valori predefiniti) devono compilare. Le strutture dei pacchetti (physica, mitex, equate)
/// non sono coperte: richiedono la rete.
#[test]
fn strutture_native_delle_equazioni_compilano() {
    let formule = [
        "frac(a, b)",
        "sqrt(x)",
        "root(3, x)",
        "sum_(i = 1)^(n) a_i",
        "product_(i = 1)^(n) a_i",
        "integral_(0)^(1) f(x) dif x",
        "lim_(x -> 0) f(x)",
        "mat(a_(11), a_(12); a_(21), a_(22))",
        "cases(a_1 x + b_1 y = c_1, a_2 x + b_2 y = c_2)",
        "vec(x_1, x_2, x_3)",
        "binom(n, k)",
        "hat(x) + tilde(x) + bar(x) + dot(x) + arrow(x) + overline(x)",
    ];
    for f in formule {
        let doc = format!(
            "#set math.equation(numbering: \"(1)\")\n$ {f} $ <eq:prova>\n\nVedi @eq:prova.\n"
        );
        let (_, errori) = compila(&doc);
        assert!(errori.is_empty(), "{f}: {errori:?}");
    }
}

/// Il blocco delle impostazioni senza `equate` (solo numerazione) compila.
#[test]
fn blocco_numerazione_equazioni_compila() {
    let (pagine, errori) = compila(
        "// typster:equazioni\n#set math.equation(numbering: \"(1)\")\n// fine typster:equazioni\n\n$ a + b $ <eq:somma>\n\nVedi @eq:somma.\n",
    );
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

/// Grafici `lilaq` come li genera lo strumento «Grafici»
/// (`src/pannelli/grafici-logica.ts`). Richiede la rete (scarica il pacchetto):
/// `cargo test -p typst-core --test inserisci_snippet -- --ignored`.
#[test]
#[ignore = "richiede la rete per scaricare lilaq"]
fn grafici_lilaq_compilano() {
    let doc = r#"#import "@preview/lilaq:0.6.0" as lq

#figure(
  lq.diagram(
    title: [Prova],
    xlabel: [Tempo],
    ylabel: [Valore],
    lq.plot((1, 2, 3), (2, 4, 3), label: [Serie A]),
    lq.scatter((1, 2, 3), (3, 1, 2), label: [Serie B]),
  ),
  caption: [Didascalia],
) <fig:prova>

Vedi @fig:prova.

#lq.diagram(
  lq.bar((1, 2, 3), (2, 4, 3)),
)
"#;
    let (pagine, errori) = compila(doc);
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

/// Strutture dello strumento «Equazioni» che usano pacchetti (physica, mitex, equate),
/// con le versioni di `src/pannelli/equazioni-logica.ts`. Richiede la rete.
#[test]
#[ignore = "richiede la rete per scaricare physica, mitex ed equate"]
fn equazioni_con_pacchetti_compilano() {
    let doc = r#"// typster:equazioni
#import "@preview/equate:0.3.3": equate
#show: equate.with(breakable: true, sub-numbering: true)
#set math.equation(numbering: "(1.1)")
// fine typster:equazioni

#import "@preview/physica:0.9.8": *
#import "@preview/mitex:0.2.7": *

$ dv(f, x) + pdv(f, x) + dd(x) + grad f $ <eq:fisica>

$ ket(psi) + bra(phi) + braket(phi, psi) $

#mitex(`\frac{a}{b} + \sqrt{x}`)

In linea #mi(`x^2`) e vedi @eq:fisica.
"#;
    let (pagine, errori) = compila(doc);
    assert!(errori.is_empty(), "{errori:?}");
    assert_eq!(pagine, 1);
}

/// Gantt generati dalla scheda «Gantt» (`src/pannelli/gantt-logica.ts`) con il pacchetto
/// dell'utente **gantt-newgen**, incorporato nell'eseguibile come `@local/gantt-newgen`
/// (`crates/typst-core/pacchetti/`): gira OFFLINE e senza installazione. Le stringhe sono copie
/// dell'output di `generaGantt` (esempio con date dal 2026-06-01, tutte le combinazioni di
/// intestazioni, i tipi di dipendenza e alcuni casi limite): vanno rigenerate se cambia il generatore.
#[test]
fn gantt_newgen_incorporato_compila() {
    let intestazione = "#import \"@local/gantt-newgen:0.1.0\": gantt\n\n";
    let casi: [(&str, &str); 9] = [
        (
            "semplice",
            r##"#gantt((
  locale: "it",
  headers: ("month", "week"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "figura",
            r##"#figure(
  gantt((
    locale: "it",
    headers: ("month", "week"),
    show-today: false,
    tasks: (
      (name: "Preparazione", subtasks: (
        (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
        (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
      )),
      (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
      (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
    ),
    milestones: (
      (name: "Consegna", date: "2026-08-10", show-date: true),
    ),
  )),
  caption: [Piano di progetto],
) <fig:piano>
"##,
        ),
        (
            "intestazioni-mese-settimana",
            r##"#gantt((
  locale: "it",
  headers: ("month", "week"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "intestazioni-mese-settimana-iso",
            r##"#gantt((
  locale: "it",
  headers: ("month", "week-iso"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "intestazioni-anno-trimestre",
            r##"#gantt((
  locale: "it",
  headers: ("year", "quarter"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "intestazioni-anno-mese",
            r##"#gantt((
  locale: "it",
  headers: ("year", "month"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "intestazioni-mese-giorno",
            r##"#gantt((
  locale: "it",
  headers: ("month", "day"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "intestazioni-settimana-giorno",
            r##"#gantt((
  locale: "it",
  headers: ("week", "day"),
  show-today: false,
  tasks: (
    (name: "Preparazione", subtasks: (
      (name: "Analisi dei requisiti", start: "2026-06-01", end: "2026-06-07", done: "2026-06-06", progress: 100, id: "a1"),
      (name: "Progettazione", start: "2026-06-08", end: "2026-06-21", progress: 40, id: "a2", dependencies: ((id: "a1"),)),
    )),
    (name: "Sviluppo", start: "2026-06-22", end: "2026-07-26", id: "a3", dependencies: ((id: "a2"),)),
    (name: "Collaudo", start: "2026-07-27", end: "2026-08-10", dependencies: ((id: "a3"),)),
  ),
  milestones: (
    (name: "Consegna", date: "2026-08-10", show-date: true),
  ),
))
"##,
        ),
        (
            "tipi-e-casi-limite",
            r##"#gantt((
  locale: "it",
  headers: ("month", "week"),
  show-today: false,
  start: "2026-05-25",
  end: "2026-07-31",
  tasks: (
    (name: "G", subtasks: (
      (name: "Uno", start: "2026-06-01", end: "2026-06-10", progress: 0, id: "a1"),
      (name: "Tre", start: "2026-06-11", end: "2026-06-20", done: "2026-06-19", progress: 100, id: "a3", dependencies: ((id: "a1", type: "finish-to-finish"),)),
    )),
    (name: "Due", start: "2026-06-05", end: "2026-06-05", dependencies: ((id: "a1", type: "start-to-start"),)),
    (name: "Quattro \"citato\"", start: "2026-06-21", end: "2026-07-05", dependencies: ((id: "a3", type: "start-to-finish"),)),
  ),
  milestones: (
    (name: "Fine", date: "2026-07-20", show-date: false),
  ),
))
"##,
        ),
    ];
    for (nome, corpo) in casi {
        let riferimento = if corpo.contains("<fig:piano>") {
            "\nVedi @fig:piano.\n"
        } else {
            ""
        };
        let (pagine, errori) = compila(&format!("{intestazione}{corpo}{riferimento}"));
        assert!(errori.is_empty(), "{nome}: {errori:?}");
        assert!(pagine >= 1, "{nome}: nessuna pagina");
    }
}
