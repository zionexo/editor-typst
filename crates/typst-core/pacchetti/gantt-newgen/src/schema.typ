// gantt-newgen — src/schema.typ
// Descrizione dichiarativa delle chiavi ammesse per ENTRAMBE LE FACCE
// dello schema (RF2, D-008): un registro di nomi di chiave, non una
// catena di `if` sparsa nel validatore (skill registro-regole-validazione).
// `validate.typ` usa questi elenchi per il controllo `strict` (E-VAL-02)
// e per guidare la ricorsione sull'albero (D-036); `graph.typ` usa
// `figli()` per attraversare l'albero senza ripetere il discriminante
// gruppo/nodo in piu' punti (S8, seconda revisione Fase 3).
//
// Dalla Fase 3 (specs/plan.md): la faccia 0.5 (`tasks` di primo livello
// con `subtasks` annidati) e' riconosciuta (`src/compat.typ`, D-055) e
// il grafo delle dipendenze (`src/graph.typ`, E-DEP-01..05) e' attivo.

/// Profondita' massima dell'albero (E-VAL-13, D-036/SD-8): 10 livelli,
/// con margine sul limite di ricorsione di Typst (T19, "maximum function
/// call depth exceeded" verso 80 frame): ogni passata ricorsiva qui
/// costa pochi frame per livello (D-036).
#let max-livelli = 10

/// Chiavi di primo livello ammesse (RF2) piu' le opzioni di API
/// (`strict`, `header-units`, `drawers`: RF1). `"tasks"` (faccia 0.5,
/// D-025) e' riconosciuta da Fase 3 (`compat.typ`, D-055): mescolata con
/// `taskgroups` da' E-VAL-11, non piu' un generico E-VAL-02.
#let chiavi-primo-livello = (
  "taskgroups", "tasks", "milestones", "show-today", "today", "viewport-snap",
  "viewport", "start", "end", "headers", "locale", "style", "date-mode",
  "sidebar-width", "strict", "header-units", "drawers",
)

/// Default di `headers` (RF2) quando la chiave e' assente o `none`
/// (D-010 punto 1): dato di schema, non piu' un letterale cablato in
/// `validate.typ` (principio "registro"/dati esterni dove ha senso
/// averne uno; qui e' comunque un valore di schema, non testo per
/// l'utente, quindi resta in Typst e non in TOML).
#let headers-default = ("month", "week")

/// Chiavi di un gruppo (faccia 0.1, livello 0, RF2): niente date proprie
/// (l'intervallo si calcola dai discendenti, RF3).
#let chiavi-gruppo = ("name", "tasks", "done", "color", "style", "task-style", "id", "x")

/// Chiavi di un nodo (task/subtask, livelli >= 1, RF2).
#let chiavi-nodo = (
  "name", "start", "end", "done", "intervals", "subtasks", "id",
  "dependencies", "progress", "color", "style", "task-style", "x",
)

/// Chiavi di un intervallo (RF2).
#let chiavi-intervallo = ("start", "end", "done", "x")

/// Chiavi del dizionario `viewport` esplicito (RF2, DS-1: prima
/// sfuggiva a `strict`, nessun controllo lo guardava).
#let chiavi-viewport = ("start", "end")

/// Chiavi di una dipendenza nella forma dizionario (RF10, `…dependencies[k]`);
/// la forma breve stringa (= id) non passa da questo controllo (e' un
/// id, non un dizionario con chiavi). Fase 3 (`src/graph.typ`).
#let chiavi-dipendenza = ("id", "type", "x")

/// Chiavi di una milestone (RF2).
#let chiavi-milestone = ("name", "date", "show-date", "color", "style", "x")

/// Chiavi di un LIVELLO d'intestazione nella forma dizionario (RF4,
/// D-106 punto (a) — "headers a dizionario per livello"): oltre a
/// `unit`/`format` (gia' validati a parte, D-093 C1), le chiavi di
/// stile/geometria PER LIVELLO con la stessa precedenza di RF4/RF5
/// (dizionario del livello > `style.headers.<unita>` > definizione
/// d'unita' > default globale, risolta da `style.livello-header-
/// risolto`). `inset` e' lo stesso margine orizzontale di D-103
/// (`style.headers.inset`), qui per-livello.
#let chiavi-livello-header = (
  "unit", "format", "align", "text", "fill", "stroke", "gridline",
  "height", "overflow", "min-cell-width", "inset",
)

/// Nomi di drawer ammessi (RF11, Fase 6b): registro usato da
/// `validate.typ` per E-DRW-01 (chiave sconosciuta in `drawers:`) e da
/// `src/render/drawers.typ` per costruire `default-drawers` — fonte
/// UNICA dei sei nomi, niente elenco duplicato altrove (skill
/// registro-regole-validazione). A differenza delle chiavi dello
/// schema (`chiavi-*`, controllate solo con `strict`), un nome di
/// drawer sconosciuto e' SEMPRE un errore (RF11: "si verifica solo il
/// tipo `function`, mai la firma" — ma il NOME si verifica sempre,
/// indipendentemente da `strict`, perche' un drawer ignorato in
/// silenzio e' un difetto peggiore di una chiave ignorata: l'utente
/// crede di aver personalizzato un elemento che resta quello di
/// default, senza nessun avviso).
#let chiavi-drawer = ("label", "bar", "milestone", "dependency", "header-cell", "overlay")

/// Figli di un gruppo o nodo dell'albero (D-036): un gruppo (faccia 0.1,
/// livello 0) li ha in `.tasks`, un nodo (qualunque altro livello, in
/// entrambe le facce) in `.subtasks`. UNICO punto che conosce questo
/// discriminante (S8, seconda revisione Fase 3): `graph.typ` e i
/// consumatori futuri (`timeline.typ`, Fase 4) lo richiamano invece di
/// ripetere `if "tasks" in nodo {...} else {...}` in piu' punti. Mai
/// entrambe le chiavi insieme (vietato da E-VAL-11, `validate.typ`),
/// quindi il discriminante e' sempre sicuro.
#let figli(nodo) = if "tasks" in nodo { nodo.tasks } else { nodo.subtasks }

/// Suggerimento per una chiave sconosciuta (D-029 punto 2, SD-4): (a)
/// `_` -> `-` da' una chiave ammessa; (b) altrimenti, se esattamente una
/// chiave ammessa ha la ricevuta come prefisso (o viceversa), si
/// suggerisce quella. Altrimenti `none` (nessuna distanza di edit, D-029:
/// "niente distanza di edit in Typst").
#let suggerisci(chiave, ammesse) = {
  let convertita = chiave.replace("_", "-")
  if convertita in ammesse {
    return convertita
  }
  let candidate = ammesse.filter(a => a.starts-with(chiave) or chiave.starts-with(a))
  if candidate.len() == 1 {
    return candidate.at(0)
  }
  none
}
