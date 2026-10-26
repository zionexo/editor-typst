// gantt-newgen — src/compat.typ
// Riconoscimento delle DUE FACCE dello schema (RF2, D-025/D-036/D-055) e
// alias "normal" delle dipendenze (SD-6). Implementazione indipendente
// (D-032): gantty 0.5.1 (`%LOCALAPPDATA%/typst/packages/preview/gantty/0.5.1`,
// letto solo come riferimento funzionale) non ha una nozione di "faccia":
// legge sempre `tasks[].subtasks[]`. Qui entrambe le facce (0.1
// `taskgroups[].tasks[]` e 0.5 `tasks[].subtasks[]`) normalizzano nello
// STESSO albero di nodi (`src/validate.typ`, `_valida-nodo-ricorsivo`).
//
// Questo modulo NON duplica l'alias `start`/`end` di primo livello ->
// `viewport` ne' l'E-VAL-11 `viewport`+`start`/`end`: vivono gia' in
// `validate.typ` (`_valida-viewport`, Fase 2, D-049). Qui vivono solo i
// conflitti di faccia (`taskgroups`+`tasks`, `subtasks` su un gruppo 0.1,
// `tasks` dentro un nodo 0.5) e l'alias "normal".

#import "errors.typ": fail

/// Chiave composita del locale (D-061) per l'etichetta della faccia 0.1
/// ("taskgroups"/task con "tasks" come figli) nei messaggi E-VAL-11.
#let schema-01 = "E-VAL-11.schema-01"

/// Come `schema-01`, per la faccia 0.5 ("tasks" di primo livello con
/// "subtasks" annidati).
#let schema-05 = "E-VAL-11.schema-05"

/// Alias "normal" del tipo di dipendenza upstream (SD-6, D-034): gantty
/// 0.5.x usa "normal" dove qui il canonico e' "finish-to-start"/"fs".
/// Applicato PRIMA della validazione dell'enumerazione in
/// `validate.typ` (`_normalizza-tipo-dipendenza`): qualunque altro
/// valore passa invariato (la validazione dell'insieme ammesso resta
/// compito del chiamante).
#let alias-tipo-dipendenza(valore) = if valore == "normal" { "fs" } else { valore }

/// E-VAL-11 fra una chiave della faccia 0.1 e una della faccia 0.5
/// (D-025/D-036/D-055), con le etichette di faccia composite dal locale
/// (D-061, nessuna prosa cablata nel core): usata per
/// "taskgroups"+"tasks" di primo livello, "subtasks" su un gruppo 0.1 e
/// "tasks" dentro un nodo 0.5. `faccia-a`/`faccia-b` sono
/// `schema-01`/`schema-05` (in un ordine qualunque: il chiamante decide
/// quale chiave mostrare come "a" e quale come "b").
#let errore-facce-miste(percorso, chiave-a, faccia-a, chiave-b, faccia-b, locale) = {
  fail(
    "E-VAL-11", percorso, locale,
    argomenti: (
      key-a: chiave-a, schema-a: locale.errors.at(faccia-a),
      key-b: chiave-b, schema-b: locale.errors.at(faccia-b),
    ),
  )
}
