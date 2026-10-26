// gantt-newgen — src/style.typ
// Stile di default (RF5) e merge con lo stile dell'utente. Core puro:
// solo dati e funzioni di risoluzione, nessun disegno (RNF5) — il
// disegno vero legge `risolvi(..)` e basta, senza valori cablati.
//
// Stato (Fase 6a, D-102/D-106): `default-style` copre tutte le chiavi
// di RF5. Precedenza COMPLETA della barra (`node.style` > `node.color`
// > `task-style` dell'antenato più vicino > `style.levels[l]` >
// `style.groups`/`style.tasks` > `default-style`, RF5), `style.levels`
// per livello di annidamento (`bar`, `sidebar.text`/`.indent`/`.align`,
// `divider`) e la risoluzione COMPLETA di un livello d'intestazione
// (`livello-header-risolto`, D-106 punto (a): `headers[i].<campo>` >
// `style.headers.<unità>.<campo>` > definizione d'unità > default) sono
// realizzati qui. `tavola-colori` (D-053/D-106 punto (c), spostata qui
// da `schema.typ`: FONTE UNICA, nessun cambiamento di comportamento,
// stesso dizionario) serve sia a `validate.typ` (RF2, colore di un
// nodo/milestone da stringa) sia potenzialmente al render (stesso
// elenco di nomi ammessi). Fase 6b Parte B (D-106 punto (b), D-112): la
// data LOCALIZZATA delle etichette è ORA agganciata al render
// (`locale.formatta-milestone`, chiamata da `src/layout.typ`), con
// l'override per documento `style.milestones.label.date-format`
// risolto qui sotto (`default-style.milestones.label`).
//
#import "errors.typ": fail
// Rereview finale di Fase 9 (code-reviewer, suggerimento): SOLO per il
// ripiego di `locale: none` sotto (`risolvi`/`stile-barra`/`stile-
// milestone`) — nessun ciclo (`locale.typ` non importa `style.typ`).
#import "locale.typ": locales

// Riferimento (D-032, SOLO lettura funzionale, nessun codice copiato):
// gantty 0.1.0 (`repo/lib.typ`) e 0.5.1 usano `olive`/`orange`/`black`/
// `luma(33%)` come colori di stato e thickness 3pt (gruppi) / 2pt
// (task) — valori convenzionali di un diagramma di Gantt, non
// espressione creativa: qui sono dati nostri, scritti da capo, con una
// struttura diversa (RF5: "thickness" diventa "height" del box,
// "paint" diventa "fill", più gli stati nuovi `progress`/`radius`).
// Stessa provenienza, stesso motivo — valori di default imposti da RF5
// "come originale" per compatibilità VISIVA con gantty, non copia di
// codice (`specs/specification.md:924-956`, review di provenienza,
// code-reviewer, suggerimento S4): gli spessori delle griglie
// verticali (`gridlines`, sotto: 0.5pt/1pt/0.85pt/0.7pt/0.2pt/0.1pt per
// task-dividers/years/quarters/months/weeks/days), lo stroke delle
// milestone (`milestones.normal`/`.today`: 1pt nero/1pt rosso) e i
// valori di `dependencies` (stroke 0.75pt nero, gap 5pt, curviness 25%,
// bulge 10pt, arrow 4pt, margin 0.875pt, violated 0.75pt rosso).

/// Tavola dei nomi di colore ammessi in una stringa YAML (RF2, T10:
/// `rgb("red")` fallisce, `eval("red")` funzionerebbe ma esegue input
/// utente — niente `eval`, tabella propria). Sottoinsieme dei colori
/// predefiniti di Typst; ampliabile senza toccare la logica di
/// validazione (principio registro). D-053/D-106 punto (c): spostata
/// qui da `schema.typ` (era lì "in attesa" di Fase 6a, D-053) — FONTE
/// UNICA per `validate.typ` (colore di un nodo/milestone/gruppo da
/// stringa, RF2) e per un eventuale uso futuro nel render (stesso
/// elenco). Stesso identico dizionario di prima (nessun cambiamento
/// di comportamento).
#let tavola-colori = (
  "black": black, "gray": gray, "silver": silver, "white": white,
  "navy": navy, "blue": blue, "aqua": aqua, "teal": teal,
  "eastern": eastern, "purple": purple, "fuchsia": fuchsia,
  "maroon": maroon, "red": red, "orange": orange, "yellow": yellow,
  "olive": olive, "green": green, "lime": lime,
)

// --- Conversione di stringhe YAML nello stile (review finale di Fase 9, ---
// --- decisione dell'utente del 2026-10-20, contratto D-140) ---------------
//
// YAML non ha lunghezze, colori, ratio o stroke: arrivano come stringhe
// (`"8pt"`, `"navy"`, `"25%"`, `"2pt + blue"`). Il metodo e' l'"oracolo di
// tipo" di D-140: si confronta ogni foglia dello stile DELL'UTENTE con la
// foglia corrispondente di `default-style` (lo schema dei tipi attesi) —
// una stringa dove il default e' un `length`/`color`/`ratio`/`stroke` si
// CONVERTE verso quel tipo; una stringa dove il default e' GIA' una
// stringa resta invariata (`dependencies.route`, `overflow`, `weight`,
// `font`); un valore GIA' del tipo giusto (letterale Typst, es. `8pt`)
// passa INVARIATO (nessuna regressione, D-140: "i letterali Typst passano
// invariati"). Dove il default non da' il tipo (`none` per `fill`, `(:)`/
// dizionario quasi-vuoto per i dizionari di argomenti `text()`) serve una
// tabella esplicita, per NOME di chiave: vedi `converti-stringhe` sotto.
//
// Sintassi accettata (niente `eval`, solo regex e tabelle, D-140): stessa
// forma dei letterali Typst, nessuno spazio fra numero e unita'.
#let unita-lunghezza = (pt: 1pt, mm: 1mm, cm: 1cm, "in": 1in, em: 1em)

/// Prova a interpretare `valore` (stringa) come lunghezza nella sintassi
/// di D-140 (`"8pt"`, `"0.8em"`, `"-2.5mm"`; NON `"8 pt"`/`"8"`/`"8px"`):
/// `length` se riconosciuta, altrimenti `none` (nessun errore: usata
/// anche dentro `_stroke-da-stringa`, dove un tentativo fallito deve
/// poter proseguire su un'altra interpretazione senza un `panic`/`fail`
/// a meta' strada).
#let prova-lunghezza(valore) = {
  let m = valore.trim().match(regex("^(-?\d+(?:\.\d+)?)(pt|mm|cm|in|em)$"))
  if m == none { return none }
  float(m.captures.at(0)) * unita-lunghezza.at(m.captures.at(1))
}

/// Come `prova-lunghezza`, ma fallisce con `E-VAL-03` (`expected:
/// "length"`, token tecnico letterale, nessun testo `it` nuovo, D-140) se
/// la stringa non si interpreta. Punto d'ingresso pubblico (usata anche
/// da `validate.typ` per `headers[i].height`/`.min-cell-width`/`.inset`).
#let lunghezza-da-stringa(valore, percorso, locale) = {
  let l = prova-lunghezza(valore)
  if l == none {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "length"), received: repr(valore))
  }
  l
}

/// Prova a interpretare `valore` (stringa) come ratio (`"25%"`, `"-10%"`):
/// `ratio` se riconosciuta, altrimenti `none` (stessa logica "tentativo"
/// di `prova-lunghezza`).
#let prova-ratio(valore) = {
  let m = valore.trim().match(regex("^(-?\d+(?:\.\d+)?)%$"))
  if m == none { return none }
  float(m.captures.at(0)) * 1%
}

/// Come `prova-ratio`, ma fallisce con `E-VAL-03` (`expected: "ratio"`,
/// D-140) se la stringa non si interpreta.
#let ratio-da-stringa(valore, percorso, locale) = {
  let r = prova-ratio(valore)
  if r == none {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "ratio"), received: repr(valore))
  }
  r
}

/// Prova a interpretare `valore` (stringa) come colore: nome della
/// `tavola-colori` o `"#rgb"`/`"#rrggbb"` fra virgolette (T8/T10, stessa
/// sintassi di `color:`/`headers[i].fill`, D-110): `color` se riconosciuta,
/// altrimenti `none` (nessun errore: usata anche dentro
/// `_stroke-da-stringa`).
#let prova-colore(valore-grezzo) = {
  // Rereview finale di Fase 9 (code-reviewer): `prova-lunghezza`/`prova-
  // ratio` fanno `.trim()` (spazi tollerati a inizio/fine stringa, D-140),
  // questa no — disallineamento corretto: stesso `.trim()` qui.
  let valore = valore-grezzo.trim()
  if valore in tavola-colori { return tavola-colori.at(valore) }
  if valore.match(regex("^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$")) != none { return rgb(valore) }
  none
}

/// Come `prova-colore`, ma fallisce con `E-VAL-06` (stesso errore di
/// `color:`/`headers[i].fill`, D-110/D-140: due codici diversi per lo
/// stesso errore sarebbero incoerenti) se la stringa non si interpreta.
/// Punto d'ingresso pubblico, SOSTITUISCE `validate._colore-da-stringa`
/// (spostata qui, D-140: `style.typ` e' gia' la fonte unica di
/// `tavola-colori`, nessun cambiamento di comportamento, stessa funzione,
/// stesso codice d'errore — solo la sede).
#let colore-da-stringa(valore, percorso, locale) = {
  let c = prova-colore(valore)
  if c == none {
    fail("E-VAL-06", percorso, locale, received: repr(valore))
  }
  c
}

/// Stroke da una stringa YAML (D-140): `"<lunghezza>"`, `"<colore>"`
/// oppure `"<lunghezza> + <colore>"` (spazi ammessi intorno a `+`, stesso
/// ordine dei letterali Typst: `"2pt + blue"`). `E-VAL-03` (`expected:
/// "stroke"`) se nessuna delle tre forme si riconosce — UN SOLO errore
/// alla fine (non i tre errori dei tentativi interni: `prova-lunghezza`/
/// `prova-colore` non sollevano mai, per questo).
#let _stroke-da-stringa(valore, percorso, locale) = {
  let testo = valore.trim()
  let parti = testo.split(regex("\s*\+\s*"))
  if parti.len() == 2 {
    let l = prova-lunghezza(parti.at(0))
    let c = prova-colore(parti.at(1))
    if l != none and c != none { return stroke(paint: c, thickness: l) }
  } else if parti.len() == 1 {
    let l = prova-lunghezza(testo)
    if l != none { return stroke(l) }
    let c = prova-colore(testo)
    if c != none { return stroke(c) }
  }
  fail("E-VAL-03", percorso, locale, argomenti: (expected: "stroke"), received: repr(valore))
}

/// Stroke da un DIZIONARIO YAML (`(paint: .., thickness: ..)`, forma
/// "come originale", D-140): `paint` si converte come colore, `thickness`
/// come lunghezza (solo se sono stringhe: un valore gia' tipizzato passa
/// invariato), le altre chiavi (`cap`/`join`/`dash`/`miter-limit`) restano
/// invariate. Costruito col costruttore nativo `stroke(..)` (normalizza
/// qualunque combinazione di chiavi ammesse in un vero `stroke`).
#let _stroke-da-dizionario(dizionario, percorso, locale) = {
  let d = dizionario
  if "paint" in d and type(d.paint) == str {
    d.insert("paint", colore-da-stringa(d.paint, percorso + ".paint", locale))
  }
  if "thickness" in d and type(d.thickness) == str {
    d.insert("thickness", lunghezza-da-stringa(d.thickness, percorso + ".thickness", locale))
  }
  stroke(..d)
}

/// Chiavi di un dizionario di argomenti `text()` (D-140, tabella
/// esplicita: il default e' spesso `(:)`/quasi vuoto, quindi non da' il
/// tipo da solo) che si convertono come LUNGHEZZA; `fill` (quando
/// presente) si converte SEMPRE come colore, gestito a parte sotto (stessa
/// chiave, stesso significato, in OGNI dizionario di argomenti `text()`
/// dello stile: `headers.text`, `headers[i].text`, `sidebar.group-text`/
/// `.task-text`, `milestones.label.text`, `levels[l].sidebar.text`).
#let campi-lunghezza-testo = ("size", "tracking", "spacing", "baseline")

/// Converte le stringhe di un dizionario di argomenti `text()` (D-140):
/// `fill` -> colore, `size`/`tracking`/`spacing`/`baseline` -> lunghezza,
/// le altre chiavi (`weight`, `style`, `font`, ...) passano invariate
/// (sono GIA' stringhe nello schema vero di `text()`, o non sono scalari
/// semplici). Pubblica: usata sia da `converti-stringhe` sotto sia
/// direttamente da `validate.typ` per `headers[i].text`.
#let converti-testo(dizionario, percorso, locale) = {
  let r = (:)
  for (chiave, valore) in dizionario {
    let percorso-chiave = percorso + "." + chiave
    r.insert(chiave, if type(valore) != str {
      valore
    } else if chiave == "fill" {
      colore-da-stringa(valore, percorso-chiave, locale)
    } else if chiave in campi-lunghezza-testo {
      lunghezza-da-stringa(valore, percorso-chiave, locale)
    } else {
      valore
    })
  }
  r
}

/// Converte un singolo elemento di STROKE (stringa o dizionario YAML),
/// lasciando invariato un valore già tipizzato: estratta dal ramo
/// `stroke` di `converti-stringhe` per riusarla anche sugli ARRAY di
/// stroke (`style.gridlines.task-dividers`, un elemento per livello,
/// Fase 6a — rereview finale di Fase 9: un array di stringhe dava
/// altrimenti un panic "nudo" al primo uso) e dai punti che leggono un
/// dizionario YAML di stroke SENZA passare da `converti-stringhe`
/// (`converti-livello`/`converti-gridline-personalizzata` sotto — 2a
/// rereview, code-reviewer, punto 1: un dizionario `{paint: ..}` ci
/// passava INVARIATO, panic "nudo" al primo uso nel render). La stringa
/// YAML `"none"` (punto 2 della stessa rereview: "none" non è una
/// parola riservata in YAML, arriva come stringa) diventa la parola
/// chiave `none` — uno stroke "nullo" è un valore legittimo ovunque
/// questa funzione è usata (divisori, griglie: D-140 non lo vieta).
#let _converti-elemento-stroke(valore, percorso, locale) = {
  if type(valore) == str and valore.trim() == "none" { none }
  else if type(valore) == str { _stroke-da-stringa(valore, percorso, locale) }
  else if type(valore) == dictionary { _stroke-da-dizionario(valore, percorso, locale) }
  else { valore }
}

/// Chiavi la cui stringa YAML `"auto"` si converte nella parola chiave
/// `auto` (D-140: "accettati SOLO dove il campo li ammette" — tabella
/// esplicita, non un'euristica sul nome): qui SOLO `style.milestones.
/// label.repeat` (il cui default è letteralmente `auto`, quindi lo
/// riconosce da solo via `schema-valore == auto` sotto, nessuna voce da
/// elencare) — `style.headers.<unità>.height` (l'ALTRO caso "auto" di
/// D-140) non passa da QUI: non ha un default in `default-style` (chiave
/// libera per unità, RF4.3), lo gestisce `converti-override-unita-header`
/// più sotto, chiamata a parte da `risolvi`.
///
/// Chiavi la cui stringa YAML `"none"` si converte nella parola chiave
/// `none` (rereview finale di Fase 9, code-reviewer: `field.stroke:
/// none`, `field.fill`/`sidebar.fill: none` da YAML — "none" non è una
/// parola riservata in YAML, arriva come la STRINGA `"none"` — davano
/// E-VAL-03/E-VAL-06 invece di disattivare il tratto/il riempimento,
/// come farebbe il letterale Typst `none`): ammesso per `fill` (sempre,
/// già un ramo a parte) e per qualunque campo di tipo `stroke` (uno
/// stroke "nullo" è un valore legittimo di `box(stroke: ..)`, D-140 non
/// lo vieta esplicitamente per questi campi). NON esteso a `length`/
/// `ratio` (nessun campo di quel tipo ammette `none` in questo schema,
/// D-140 non lo richiede: se emergesse un caso concreto va deciso
/// dall'utente, non assunto qui).
///
/// L'"oracolo di tipo" (D-140): converte ricorsivamente le stringhe di
/// `utente` (un sotto-dizionario dello stile scritto dall'utente, es.
/// `spec.style`, `node.style`, `m.style`) confrontandole con `schema` (il
/// sotto-dizionario corrispondente di `default-style`, O una tabella
/// esplicita per i casi che il default non tipizza — vedi `risolvi`/
/// `stile-barra`/`stile-milestone` sotto per come viene chiamata).
/// Regole, nell'ordine (D-140, estese nella rereview finale di Fase 9):
/// 1. un valore GIA' non stringa (letterale Typst, `auto`, `none`, un
///    array GIA' tipizzato, ...) passa INVARIATO: nessuna regressione
///    (`levels` stesso, che qui ha schema `array`, non viene toccato: la
///    conversione per livello la fa `converti-livello` sotto, chiamata a
///    parte da `risolvi`) — ECCETTO un array di STRINGHE dove lo schema
///    è uno `stroke` (`task-dividers`): ogni elemento stringa si
///    converte come stroke, un array già tipizzato passa invariato;
/// 2. un sotto-DIZIONARIO ricorre: se lo schema a quella chiave e' uno
///    `stroke` e il valore e' un dizionario, e' la forma "come originale"
///    (`_stroke-da-dizionario`); se la chiave e' `"text"`/`"group-text"`/
///    `"task-text"`, il dizionario intero si converte con le regole FLAT
///    di `converti-testo` (nessuna ricorsione ulteriore: non ci sono
///    dizionari di argomenti `text()` annidati nello schema vero);
/// 3. la stringa `"auto"` diventa la parola chiave `auto` SOLO se il
///    default a quella chiave è letteralmente `auto` (`milestones.
///    label.repeat`); la stringa `"none"` diventa la parola chiave
///    `none` per `fill` o per un campo di tipo `stroke`;
/// 4. la chiave `"fill"` e' SEMPRE un colore (anche quando il default e'
///    `none`, come `field.fill`/`sidebar.fill`: tabella esplicita di
///    D-140 — tutte le altre occorrenze di `fill` nello schema hanno
///    comunque un default `color`, stessa regola, un solo ramo);
/// 5. altrimenti il TIPO del default decide: `length`/`ratio`/`stroke` ->
///    conversione; `str` -> invariata (resta stringa, es.
///    `dependencies.route`); qualunque altro tipo (compreso uno schema
///    assente, es. `milestones.label.date-format: none`) -> invariata.

/// DS-A (4a rereview, code-reviewer, REGRESSIONE): un `stroke` con
/// `none` NON è ammesso OVUNQUE, contro quanto il punto 3 sopra
/// lasciava intendere — per le milestone (`style.milestones.normal.
/// stroke`/`.today.stroke`, lo `style.stroke` di UNA milestone, stesso
/// schema in `stile-milestone`) e per `*.progress.stroke`
/// (`groups`/`tasks`/`levels[l].bar`), un `none` finisce in `stroke
/// (none)` (`layout.typ`, `stile-milestone`) o `line(stroke: none)`
/// (`render/drawers.typ: bar-progress`) — ENTRAMBI panic "nudo" di
/// Typst (verificato con 4 sonde dedicate PRIMA di questa guardia:
/// `normal.stroke`/`today.stroke`/`m.style.stroke`/`groups.progress.
/// stroke`, tutti e 4 "expected length, color, gradient, tiling,
/// dictionary, or stroke, found none"). Prima di D-140 la stessa
/// stringa YAML dava un E-VAL-03 pulito (non riconosciuta da
/// `_stroke-da-stringa`): la conversione generica di "none" l'ha
/// rotto. Esclusione per PERCORSO (non per nome di campo, "stroke" da
/// solo è troppo generico: `headers.stroke`/`dependencies.stroke`/
/// `gridlines.*.stroke` DEVONO continuare ad accettare "none"):
/// "milestones" in qualunque punto del percorso (copre sia lo schema
/// GLOBALE, `style.milestones.normal.stroke`, sia quello PER
/// ISTANZA, `milestones[i].style.stroke`: `stile-milestone` passa
/// SEMPRE un percorso che contiene "milestones", mai "today" da solo
/// per una milestone vera — vedi sotto) o il percorso che finisce per
/// ".progress.stroke". Una stringa "none" esclusa qui NON resta
/// invariata: ricade nel ramo generico `type(schema-valore) ==
/// stroke` due righe sotto, che prova `_stroke-da-stringa("none", ..)`
/// e fallisce con lo STESSO E-VAL-03 pulito di prima di D-140 (nessun
/// codice nuovo per quel caso). Il letterale TYPST `none` (mai
/// passato da qui: non è mai una stringa) resta guardato A PARTE in
/// `stile-milestone` sotto — questa esclusione, da sola, NON lo
/// coprirebbe.
#let _stroke-none-escluso(percorso) = "milestones" in percorso or percorso.ends-with(".progress.stroke")
#let converti-stringhe(schema, utente, percorso, locale) = {
  let r = (:)
  for (chiave, valore) in utente {
    let percorso-chiave = percorso + "." + chiave
    let schema-valore = schema.at(chiave, default: none)
    r.insert(chiave, if type(valore) == array and type(schema-valore) == stroke {
      valore.enumerate().map(((i, v)) => _converti-elemento-stroke(v, percorso-chiave + "[" + str(i) + "]", locale))
    } else if type(valore) == dictionary {
      if type(schema-valore) == stroke {
        _stroke-da-dizionario(valore, percorso-chiave, locale)
      } else if chiave in ("text", "group-text", "task-text") {
        converti-testo(valore, percorso-chiave, locale)
      } else {
        let schema-sotto = if type(schema-valore) == dictionary { schema-valore } else { (:) }
        converti-stringhe(schema-sotto, valore, percorso-chiave, locale)
      }
    } else if type(valore) != str {
      valore
    // `.trim()` (2a rereview, code-reviewer): spazi iniziali/finali
    // tollerati anche per "auto"/"none", come per lunghezza/ratio/
    // colore (D-140, stessa regola). `valore` è una STRINGA qui
    // (garantito dal ramo sopra): `.trim()` non solleva mai.
    // Suggerimento S12 (4a rereview, code-reviewer): anche "none"
    // diventa `auto` dove il default è `auto` (qui SOLO `milestones.
    // label.repeat`, l'unico campo con quel default in tutto lo
    // schema) — coerenza con D-127 ("`none` vale come chiave assente,
    // cioè `auto`"), già vera per il letterale Typst `none`/YAML `~`
    // (`valida-repeat-etichette`: `valore-grezzo == none { auto }`),
    // non ancora per la STRINGA YAML "none" (dava E-VAL-03 pulito, non
    // un panic: un'incoerenza, non una regressione).
    } else if (valore.trim() == "auto" or valore.trim() == "none") and schema-valore == auto {
      auto
    } else if valore.trim() == "none" and chiave == "fill" {
      none
    } else if valore.trim() == "none" and type(schema-valore) == stroke and not _stroke-none-escluso(percorso-chiave) {
      none
    } else if chiave == "date-format" and valore.trim() == "none" {
      // Suggerimento (2a rereview, code-reviewer): `milestones.label.
      // date-format` "resta stringa" (D-140), MA il default è
      // letteralmente `none` (usa il formato del locale) — una stringa
      // YAML "none" (non una parola riservata in YAML) andava
      // altrimenti a `locale.formatta-milestone` come se fosse un
      // VERO formato con segnaposto (`.replace(..)` su "none" non
      // tocca nulla, nessun segnaposto): il risultato nel disegno era
      // la parola "none" al posto della data, silenziosamente — mai un
      // errore, un difetto "muto" (skill `testo-utente-fonte-unica-
      // nel-core`). Solo QUESTA chiave: le altre stringhe col default
      // `none` (nessuna, nello schema attuale, a parte `fill`/`stroke`
      // già sopra) non sono toccate da questo ramo.
      none
    } else if chiave == "fill" {
      colore-da-stringa(valore, percorso-chiave, locale)
    } else if type(schema-valore) == length {
      lunghezza-da-stringa(valore, percorso-chiave, locale)
    } else if type(schema-valore) == ratio {
      ratio-da-stringa(valore, percorso-chiave, locale)
    } else if type(schema-valore) == stroke {
      _stroke-da-stringa(valore, percorso-chiave, locale)
    } else if type(schema-valore) == dictionary {
      // Suggerimento (2a rereview, code-reviewer): una stringa dove il
      // default è un DIZIONARIO (es. `style.gridlines.years: "1pt +
      // red"` invece di `{stroke: "1pt + red"}`) non ha nessuna regola
      // sopra a cui appoggiarsi — restava invariata (stringa), panic
      // "nudo" nel render al primo accesso a un suo campo (T27/T33).
      // E-VAL-03 chiaro invece del panic: nessuna conversione
      // automatica della forma breve (non richiesta da D-140).
      fail("E-VAL-03", percorso-chiave, locale, argomenti: (expected: "dictionary"), received: repr(valore))
    } else {
      valore
    })
  }
  r
}

/// Converte UN elemento di `style.levels` (D-140: "si usa lo schema di
/// `groups`/`tasks` per `bar`, quello di `sidebar` per `sidebar` e quello
/// di stroke per `divider`" — `levels` e' vuoto di default, quindi non da'
/// il tipo da solo: tabella esplicita, un elemento alla volta).
/// `schema-barra` e' `default-style.groups` o `.tasks` (stessa FORMA,
/// stessi tipi, valori diversi: indifferente quale delle due si passi).
#let converti-livello(schema-barra, schema-sidebar, livello, percorso, locale) = {
  let r = livello
  if "bar" in livello {
    r.insert("bar", converti-stringhe(schema-barra, livello.bar, percorso + ".bar", locale))
  }
  if "sidebar" in livello {
    r.insert("sidebar", converti-stringhe(schema-sidebar, livello.sidebar, percorso + ".sidebar", locale))
  }
  // `_converti-elemento-stroke` (2a rereview, code-reviewer, punti 1/2):
  // non solo `_stroke-da-stringa` — un `divider` scritto come dizionario
  // YAML (`{paint: .., thickness: ..}`) passava invariato (panic "nudo"
  // nel render), e `"none"` (ammesso: `divisore-livello` lo riconosce
  // già come "nessun divisore per questo livello") falliva con
  // E-VAL-03 invece di diventare la parola chiave `none`.
  if "divider" in livello and (type(livello.divider) == str or type(livello.divider) == dictionary) {
    r.insert("divider", _converti-elemento-stroke(livello.divider, percorso + ".divider", locale))
  }
  r
}

/// Chiavi FISSE di `style.headers` (quelle che `default-style.headers`
/// definisce: l'oracolo di tipo generico le converte già, via lo schema).
/// Le ALTRE chiavi di `style.headers` sono override PER UNITÀ (RF4.3,
/// `week`/`month`/... o un'unità custom registrata): dizionari liberi,
/// SENZA un default in `default-style` — l'oracolo generico, che confronta
/// con lo schema del default, non li vede (schema assente, tabella
/// esplicita sotto). Stessa idea per `style.gridlines` (vedi sotto).
#let chiavi-fisse-headers = ("stroke", "text", "inset")

/// Converte le stringhe di un override PER UNITÀ di `style.headers.
/// <unità>` (RF4.3 — rereview finale di Fase 9, code-reviewer, punto 3:
/// "il contratto «ovunque compaia» non è coperto"): `height` (lunghezza O
/// la stringa letterale `"auto"`, l'esempio stesso di D-140), `min-cell-
/// width`/`inset` (lunghezza), `fill` (colore, via `_override-unita-
/// headers`/`livello-header-risolto` sotto — gia' letto come stringa
/// grezza altrove per `headers[i].fill`, qui e' lo stesso per lo stile
/// GLOBALE), `stroke`/`gridline` (stroke), `text` (dizionario di
/// argomenti `text()`, `converti-testo`). `align` NON passa da qui:
/// resta fuori da questa voce (non richiesto da D-140, gestito altrove
/// con una propria normalizzazione — nessuna doppia conversione).
#let converti-override-unita-header(dizionario, percorso, locale) = {
  let r = (:)
  for (chiave, valore) in dizionario {
    let percorso-chiave = percorso + "." + chiave
    r.insert(chiave, if chiave == "text" and type(valore) == dictionary {
      converti-testo(valore, percorso-chiave, locale)
    // `stroke`/`gridline` PRIMA del filtro "non e' una stringa" (2a
    // rereview, code-reviewer, punti 1/2): un dizionario YAML
    // (`{paint:, thickness:}`) ci passava invariato (panic "nudo" nel
    // render), e `_converti-elemento-stroke` gestisce anche la stringa
    // "none" (il campo la ammette: uno stroke "nullo" e' legittimo).
    } else if chiave in ("stroke", "gridline") {
      _converti-elemento-stroke(valore, percorso-chiave, locale)
    } else if type(valore) != str {
      valore
    } else if chiave == "height" and valore.trim() == "auto" {
      auto
    } else if chiave == "height" {
      let l = prova-lunghezza(valore)
      if l == none {
        fail("E-VAL-03", percorso-chiave, locale, argomenti: (expected: "auto|length"), received: repr(valore))
      }
      l
    } else if chiave in ("min-cell-width", "inset") {
      lunghezza-da-stringa(valore, percorso-chiave, locale)
    } else if chiave == "fill" and valore.trim() == "none" {
      none
    } else if chiave == "fill" {
      colore-da-stringa(valore, percorso-chiave, locale)
    } else {
      valore
    })
  }
  r
}

/// Chiavi FISSE di `style.gridlines` (`default-style.gridlines` le
/// definisce tutte: l'oracolo di tipo generico le converte già). Le
/// ALTRE chiavi sono griglie PER UNITÀ CUSTOM (RF4.3, es. `semesters`,
/// vedi il commento di `gridlines:` dentro `default-style` sopra):
/// stessa forma `(stroke: ..)` delle fisse, ma senza un default da cui
/// l'oracolo generico possa leggere il tipo. (2a rereview, code-
/// reviewer: qui citava un numero di riga, "style.typ:363-364" —
/// diventato stantio dopo altre modifiche: nessun riferimento a riga
/// fissa da questa voce in poi.)
#let chiavi-fisse-gridlines = ("task-dividers", "years", "quarters", "months", "weeks", "days")

/// Converte `style.gridlines.<unità-custom>s.stroke` (rereview finale di
/// Fase 9, code-reviewer, punto 3): stessa forma/stesso errore di
/// `gridlines.years`/`.months`/... (già convertite dall'oracolo
/// generico), qui per una chiave che il default non elenca.
#let converti-gridline-personalizzata(dizionario, percorso, locale) = {
  let r = dizionario
  if "stroke" in r {
    r.insert("stroke", _converti-elemento-stroke(r.stroke, percorso + ".stroke", locale))
  }
  r
}

/// Stile di default (RF1/RF5): fonte unica, esportata da `lib.typ` e
/// usata anche per il README (RF8, Fase 9). Chiavi in kebab-case (RF2).
#let default-style = (
  groups: (
    uncompleted: (fill: black, height: 3pt),
    completed-early: (
      timeframe: (fill: olive, height: 3pt),
      body: (fill: black, height: 3pt),
    ),
    completed-late: (
      timeframe: (fill: orange, height: 3pt),
      body: (fill: black, height: 3pt),
    ),
    // Nuovo (RF5): tratto di avanzamento (D-039), disegnato come un
    // segno a parte (non un riempimento) — `stroke`, non `fill`.
    progress: (stroke: 2pt + blue),
    // Nuovo (RF5): angoli della barra.
    radius: 0pt,
  ),
  tasks: (
    uncompleted: (fill: luma(33%), height: 2pt),
    completed-early: (
      timeframe: (fill: olive, height: 2pt),
      body: (fill: luma(33%), height: 2pt),
    ),
    completed-late: (
      timeframe: (fill: orange, height: 2pt),
      body: (fill: luma(33%), height: 2pt),
    ),
    progress: (stroke: 2pt + blue),
    radius: 0pt,
  ),
  // Stile per livello di annidamento (gantty 0.5.x, RF5): array di
  // `(bar:, sidebar: (text:, indent:, align:), divider:)`. Vuoto di
  // default: "se `levels` è vuoto valgono `style.groups`/`style.tasks`"
  // (RF5). Se non vuoto, l'ULTIMO elemento si ripete per i livelli oltre
  // la lunghezza dell'array (RF5 "l'ultimo si ripete"): `_livello()`
  // sotto. `bar` ha la stessa forma di `groups`/`tasks` (merge profondo
  // con la voce di `kind` come fallback, D-102: il `kind` resta comunque
  // la base per le chiavi che l'utente non sovrascrive a livello, es.
  // l'altezza predefinita). `sidebar.align` per-livello (D-106 punto
  // (d)): `sidebar-align-livello`, vince su `style.sidebar.align`.
  levels: (),
  milestones: (
    normal: (stroke: 1pt + black),
    today: (stroke: 1pt + red),
    // `fill` (rilievo del coordinatore, rereview di Fase 5): sfondo
    // OPACO dell'etichetta — bianco di default, cosi' nasconde il
    // tratto del connettore che la attraversa (il connettore si disegna
    // PRIMA, l'etichetta DOPO: `render/milestones.typ`). `gap`: ex
    // `_margine-etichette` cablata in `layout.typ` — distacco minimo fra
    // due etichette adiacenti sullo STESSO livello (D-099). `connector-
    // threshold`: margine AGGIUNTIVO (oltre al criterio geometrico puro,
    // "il segno cade fuori dal rettangolo del testo") prima di disegnare
    // il gomito orizzontale del connettore — `0pt` di default lascia il
    // criterio puramente geometrico.
    label: (
      // `repeat` (RF12, Fase 8.5, D-127): default `auto` (prima `true`)
      // — attiva la regola del footer alto (`style.
      // valida-repeat-etichette`/`layout.ripeti-etichette`/`ctx.
      // ripeti-etichette`); `true`/`false` sono scelte ESPLICITE
      // dell'utente, sempre rispettate, nessuna regola.
      inset: 5pt, overhang: 5pt, text: (:), repeat: auto,
      fill: white, gap: 3pt, "connector-threshold": 0pt,
      // Fase 6b Parte B: override PER DOCUMENTO del modello di
      // `locale.formatta-milestone` (RF6 `milestone-date-format`) —
      // `none` (default) = usa quello del locale risolto.
      "date-format": none,
    ),
  ),
  // RF5: `task-dividers` è uno stroke "nudo" (o un array di stroke per
  // livello, Fase 6a); le griglie PER UNITA' (`years`/`months`/.../
  // `quarters`/`<nome>s` custom) sono invece un dizionario `(stroke:
  // ...)`, "come originale" (gantty 0.1.0: `(stroke: (paint:,
  // thickness:))`) — la stessa forma che un utente scrive da YAML per
  // `style.gridlines.semesters`, RF4.3.
  gridlines: (
    "task-dividers": 0.5pt + luma(66%),
    years: (stroke: 1pt + luma(66%)),
    quarters: (stroke: 0.85pt + luma(66%)),
    months: (stroke: 0.7pt + luma(66%)),
    weeks: (stroke: 0.2pt + luma(66%)),
    days: (stroke: 0.1pt + luma(66%)),
  ),
  // RF10: letto solo da Fase 7 (il disegno delle frecce); il dato
  // esiste già perché `default-style` è la fonte unica di TUTTE le
  // chiavi di RF5, non solo quelle che Fase 5 disegna. `margin` (G2,
  // revisione del 2026-10-18, D-121/D-123): margine dei punti DI
  // SVOLTA interni di una polilinea (`layout._clampa-punto-interno`),
  // MAI degli agganci — tiene un tratto "di passaggio" staccato dal
  // bordo dell'area barre (`style.field.stroke`), dove altrimenti
  // coinciderebbe esattamente (`g` cioè `gap` sotto è un'altra cosa:
  // distanza del bridge di uscita/rientro dall'aggancio). Default
  // `0.875pt` = metà dello spessore di default di `stroke` sotto
  // (`0.75pt / 2`) più `0.5pt` di margine fisso — un valore pensato per
  // lo STROKE di default: se l'utente usa uno stroke molto più spesso,
  // può alzare `margin` di conseguenza (nessun calcolo automatico dal
  // proprio `stroke`, stessa scelta di `arrow`/`gap`, scalari fissi).
  dependencies: (
    stroke: 0.75pt + black, route: "orthogonal",
    gap: 5pt, curviness: 25%, bulge: 10pt, arrow: 4pt, margin: 0.875pt,
    violated: (stroke: 0.75pt + red),
  ),
  field: (stroke: 1pt + black, fill: none),
  // `inset` (D-103, rilievo visivo del coordinatore su R6): margine
  // ORIZZONTALE dell'etichetta quando l'allineamento del livello (RF4,
  // `def.align`/`style.headers.<unita>.align`) ha una componente
  // orizzontale diversa da `center` (cioe' tocca un bordo della cella,
  // sinistro con `left`, destro con `right`) — con `center` l'etichetta
  // e' gia' centrata, lontana da entrambi i bordi "in media", quindi
  // l'inset non si applica (nessun effetto, come per l'originale
  // `pad(2pt, ..)` solo sul lato rilevante). Default `2pt`, come
  // l'originale (gantty 0.1.0/0.5.1: `pad(2pt, text(0.8em, align(left +
  // horizon, ..)))`, D-032 solo riferimento funzionale).
  headers: (stroke: 1pt + black, text: (:), inset: 2pt),
  sidebar: (
    stroke: 1pt + black, inset: 5pt, fill: none,
    "group-text": (weight: "bold"), "task-text": (:),
    // S2(f) (decisione estetica del coordinatore, da registrare nel
    // verbale): `indent` di default > 0pt (prima 0pt, nessun rientro
    // visibile per i livelli annidati) — 8pt è un passo leggibile senza
    // sprecare la larghezza, già misurata per livello (DC-9/RF5).
    // `align` di default `left` (prima `center`): coerente col render
    // SENZA `align()` (S1 leva 3/S3 — niente `align()` nel percorso
    // comune, solo `inset`, per prestazioni E perché `align()` dentro
    // un box/cella a dimensioni fisse è stata la causa del trabocco di
    // D-097 bug 2): un `align` esplicito diverso da `left` nello stile
    // dell'utente torna a usare `align()` (ramo più lento, ma ora
    // sicuro: la causa del trabocco era il testo non misurato, non
    // `align()` in sé, già risolta dalla larghezza misurata col vero
    // stile).
    align: left, indent: 8pt,
  ),
  layout: (row-gap: 10pt, "header-inset": 5pt),
)

/// Unisce due dizionari in profondità (merge ricorsivo, RF1/RF5): per
/// ogni coppia che non è due dizionari vale la sostituzione semplice
/// (l'ultimo valore vince). Copia indipendente di
/// `validate._unisci-profondo` (stessa funzione, duplicata apposta: RF5
/// è un modulo a se' — `style.typ` non deve dipendere da `validate.typ`,
/// RNF5/"Struttura dei moduli").
#let unisci-profondo(a, b) = {
  if type(a) == dictionary and type(b) == dictionary {
    let r = a
    for (chiave, valore) in b {
      r.insert(chiave, if chiave in r { unisci-profondo(r.at(chiave), valore) } else { valore })
    }
    r
  } else {
    b
  }
}

/// Stile effettivo (RF5): `default-style` unito in profondità con lo
/// stile dell'utente già normalizzato da `validate()` (`spec.style`,
/// merge argomento/`spec` già fatto a monte). Risolto **una volta**
/// (RNF3) da `gantt()`/`layout.calcola`, mai nel render. **Review finale
/// di Fase 9 (D-140):** le stringhe YAML dello stile si convertono QUI,
/// PRIMA del merge (`converti-stringhe`, l'"oracolo di tipo" sopra) — il
/// merge vede solo valori già tipizzati, come prima di questa modifica.
/// `style.levels` è convertito A PARTE (`converti-livello`): il suo
/// default è un array VUOTO, quindi non dà il tipo da solo (schema
/// esplicito: `groups`/`tasks` per `bar`, `sidebar` per `sidebar`). Idem
/// per gli override PER UNITÀ di `style.headers`/`style.gridlines`
/// (rereview finale di Fase 9, code-reviewer, punto 3: `converti-
/// override-unita-header`/`converti-gridline-personalizzata`, chiavi
/// senza default in `default-style`, RF4.3).
/// `locale: none` di default (NON `locale` posizionale obbligatorio):
/// decine di fixture di `tests/unit/` chiamano `risolvi()` con un solo
/// argomento, sempre con `style:` già di soli letterali Typst (mai una
/// stringa da convertire) — `locale` lì non viene mai letto per davvero
/// (nessuna chiamata a `fail()` si attiva), quindi `none` è sicuro; la
/// pipeline vera (`gantt()`, `lib.typ`) passa sempre il locale risolto.
/// **Ripiego su `locales.en` se `locale` è `none`** (rereview finale di
/// Fase 9, code-reviewer, suggerimento): un chiamante futuro che
/// DIMENTICA `locale:` ma scrive DAVVERO una stringa da convertire
/// avrebbe altrimenti un panic "nudo" dentro `fail()` stessa
/// (`none.errors`, T27 di nuovo) invece di un messaggio RF7 — più
/// difficile da diagnosticare di un messaggio in inglese "sbagliato".
#let risolvi(stile-utente, locale: none) = {
  let locale = if locale == none { locales.en } else { locale }
  let convertito = converti-stringhe(default-style, stile-utente, "style", locale)
  let livelli-utente = stile-utente.at("levels", default: ())
  if livelli-utente.len() > 0 {
    convertito.insert("levels", livelli-utente.enumerate().map(((i, liv)) => {
      converti-livello(default-style.tasks, default-style.sidebar, liv, "style.levels[" + str(i) + "]", locale)
    }))
  }
  for (chiave, valore) in stile-utente.at("headers", default: (:)) {
    if chiave not in chiavi-fisse-headers and type(valore) == dictionary {
      convertito.headers.insert(chiave, converti-override-unita-header(valore, "style.headers." + chiave, locale))
    }
  }
  for (chiave, valore) in stile-utente.at("gridlines", default: (:)) {
    if chiave not in chiavi-fisse-gridlines and type(valore) == dictionary {
      convertito.gridlines.insert(chiave, converti-gridline-personalizzata(valore, "style.gridlines." + chiave, locale))
    } else if chiave not in chiavi-fisse-gridlines and type(valore) == str {
      // Suggerimento S1 (4a rereview, code-reviewer): `style.gridlines.
      // <unità-custom>: "1pt + red"` (stringa al posto del dizionario
      // `{stroke: ..}`) non passava da QUESTO ciclo (solo `type(valore)
      // == dictionary` era gestito: una stringa restava quella scritta
      // dall'oracolo generico sopra, che non la tocca — nessuno
      // schema per una chiave custom) e arrivava a `layout.gridline-
      // unita`, che chiama `.at("stroke")` su di essa: panic "nudo"
      // ("expected integer, found string", verificato con una sonda).
      // E-VAL-03 chiaro, stesso schema delle chiavi FISSE (`style.
      // gridlines.years: "stringa"`, sopra): nessuna conversione
      // automatica della forma breve, non richiesta da D-140.
      fail(
        "E-VAL-03", "style.gridlines." + chiave, locale,
        argomenti: (expected: "dictionary"), received: repr(valore),
      )
    }
  }
  unisci-profondo(default-style, convertito)
}

/// Stile di base per `kind` (gruppo/nodo, D-036), SENZA livello:
/// `style.groups` se `kind == "group"`, altrimenti `style.tasks`. Usata
/// da sola quando `style.levels` è vuoto (RF5: "se `levels` è vuoto
/// valgono `style.groups`/`style.tasks`"); altrimenti è il FALLBACK di
/// `base-barra-livello` sotto, per le chiavi che l'utente non
/// sovrascrive in `style.levels[l].bar`.
#let base-barra(stile-risolto, kind) = if kind == "group" { stile-risolto.groups } else { stile-risolto.tasks }

/// Elemento di `style.levels` per `level` (RF5 "l'ultimo si ripete",
/// D-102): `none` se l'array è vuoto (nessun override per livello,
/// il chiamante ricade su `base-barra`/`style.sidebar`/
/// `style.gridlines.task-dividers` tal quali).
#let _livello(stile-risolto, level) = {
  let liv = stile-risolto.levels
  if liv.len() == 0 { none } else { liv.at(calc.min(level, liv.len() - 1)) }
}

/// Stile di base per la barra di un nodo di `kind`/`level` (RF5,
/// D-102): se `style.levels` è vuoto, identico a `base-barra` (nessuna
/// distinzione per livello). Altrimenti `style.levels[l].bar` (merge
/// PROFONDO, "con fallback") sopra `base-barra(stile-risolto, kind)`:
/// le chiavi non scritte dall'utente in `bar` (es. l'altezza) restano
/// quelle del `kind` — lo stile per livello non deve obbligare a
/// ridichiarare l'intero blocco RF3 (uncompleted/completed-*/progress/
/// radius) per sovrascrivere solo, ad esempio, il colore.
#let base-barra-livello(stile-risolto, kind, level) = {
  let base = base-barra(stile-risolto, kind)
  let liv = _livello(stile-risolto, level)
  if liv == none { return base }
  let bar-utente = liv.at("bar", default: (:))
  if bar-utente.len() > 0 { unisci-profondo(base, bar-utente) } else { base }
}

/// Stile EFFETTIVO della barra di un nodo (RF3/RF5 "Precedenza dello
/// stile barra", COMPLETA da Fase 6a, D-102): `node.style` >
/// `node.color` > `task-style` dell'antenato più vicino >
/// `style.levels[l]` > `style.groups`/`style.tasks` > `default-style`.
/// `nodo` è la voce validata dell'albero (con `kind`, `level`, `style`,
/// `color`); `task-style-ereditato` è il `task-style` del più vicino
/// ANTENATO che lo dichiara non vuoto (D-036/RF2 "ereditato da tutti i
/// discendenti", calcolato da `layout._appiattisci`: NON è mai il
/// `task-style` del nodo stesso, che si applica solo ai suoi
/// discendenti, non a lui). `color` (se non `none`) sostituisce il
/// `fill` di `uncompleted` e di `body` (early/late): i colori
/// `timeframe` restano quelli di stato, come da RF3, salvo override
/// esplicito in `node.style`/`task-style` (precedenza più alta, possono
/// comunque cambiarli).
/// `locale: none` di default (review finale di Fase 9, D-140): serve
/// SOLO per convertire le stringhe YAML di `task-style-ereditato`/
/// `nodo.style` (`converti-stringhe`, stesso schema di `groups`/`tasks`
/// — le due hanno la STESSA forma, indifferente quale delle due si
/// passi: qui `tasks`, scelta arbitraria); `none` e' sicuro per le tante
/// fixture di `tests/unit/` che chiamano questa funzione senza locale,
/// sempre con stili di soli letterali Typst (stessa nota di `risolvi`).
/// Chiamata una volta per nodo (RNF3, come il resto di questa funzione),
/// non per riga disegnata. Ripiego su `locales.en` se `locale` è `none`
/// (rereview finale di Fase 9, code-reviewer: stessa nota di `risolvi`).
#let stile-barra(stile-risolto, nodo, task-style-ereditato: (:), locale: none) = {
  let locale = if locale == none { locales.en } else { locale }
  let base = base-barra-livello(stile-risolto, nodo.at("kind", default: "node"), nodo.at("level", default: 0))
  let percorso-nodo = nodo.at("path", default: "<nodo>")
  let ereditato-convertito = if task-style-ereditato.len() > 0 {
    converti-stringhe(default-style.tasks, task-style-ereditato, percorso-nodo + ".task-style", locale)
  } else {
    (:)
  }
  let con-ereditato = if ereditato-convertito.len() > 0 {
    unisci-profondo(base, ereditato-convertito)
  } else {
    base
  }
  let con-colore = if nodo.at("color", default: none) != none {
    let c = nodo.color
    let b = con-ereditato
    b.insert("uncompleted", unisci-profondo(b.uncompleted, (fill: c)))
    b.insert("completed-early", unisci-profondo(b.at("completed-early"), (body: (fill: c))))
    b.insert("completed-late", unisci-profondo(b.at("completed-late"), (body: (fill: c))))
    b
  } else {
    con-ereditato
  }
  let proprio = nodo.at("style", default: (:))
  let proprio-convertito = if proprio.len() > 0 {
    converti-stringhe(default-style.tasks, proprio, percorso-nodo + ".style", locale)
  } else {
    (:)
  }
  if proprio-convertito.len() > 0 { unisci-profondo(con-colore, proprio-convertito) } else { con-colore }
}

/// Stile del TESTO della sidebar per `kind`/`level` (RF5
/// `style.levels[l].sidebar.text`, D-102): fallback su
/// `style.sidebar.group-text`/`.task-text` (per `kind`, come in Fase
/// 5), con l'eventuale override per livello unito in profondità sopra
/// (stessa logica di `base-barra-livello`: solo le chiavi scritte
/// dall'utente cambiano).
#let sidebar-testo-livello(stile-risolto, kind, level) = {
  let base = if kind == "group" { stile-risolto.sidebar.at("group-text") } else { stile-risolto.sidebar.at("task-text") }
  let liv = _livello(stile-risolto, level)
  if liv == none { return base }
  let testo-utente = liv.at("sidebar", default: (:)).at("text", default: (:))
  if testo-utente.len() > 0 { unisci-profondo(base, testo-utente) } else { base }
}

/// Rientro della sidebar per `level` (RF5 `style.levels[l].sidebar.
/// indent`, D-102): se il livello (o quello ripetuto, "l'ultimo si
/// ripete") dichiara un `indent` proprio, VINCE sul calcolo globale
/// `style.sidebar.indent × level` (è un valore assoluto per quel
/// livello, non un fattore aggiuntivo: coerente con l'array "uno per
/// livello" di RF5, a differenza del fattore unico `indent` di
/// `style.sidebar`, pensato per una progressione lineare). Senza
/// override, resta il comportamento di Fase 5.
#let sidebar-indent-livello(stile-risolto, level) = {
  let liv = _livello(stile-risolto, level)
  let override = if liv == none { none } else { liv.at("sidebar", default: (:)).at("indent", default: none) }
  if override != none { override } else { stile-risolto.sidebar.indent * level }
}

/// Allineamento orizzontale della sidebar per `level` (RF5 `style.
/// levels[l].sidebar.align`, D-106 punto (d)): se il livello (o quello
/// ripetuto) dichiara un `align` proprio, VINCE su `style.sidebar.align`
/// globale — stessa normalizzazione di `allineamento-orizzontale`
/// (componente orizzontale, `E-VAL-03` se non è un `alignment` con
/// componente orizzontale), cosi' un `style.levels[l].sidebar.align`
/// scritto male fallisce con lo STESSO errore del globale, non un
/// `panic` diverso a seconda di quale dei due l'utente ha scritto.
/// `percorso` è quello del valore EFFETTIVAMENTE letto (per livello o
/// globale), per un messaggio d'errore preciso.
/// Divisore orizzontale per `level` (RF5: `style.levels[l].divider` >
/// `style.gridlines.task-dividers` — quest'ultimo puo' essere uno
/// stroke "nudo" (invariato per ogni livello, Fase 5) o un ARRAY di
/// stroke per livello ("l'ultimo si ripete", D-102, stessa regola di
/// `_livello` sopra, applicata però a un array di STROKE, non di
/// dizionari `style.levels`). `none` ammesso a ogni passo (nessun
/// divisore, come un `style.field`/`style.sidebar` senza bordo fra le
/// righe).
#let divisore-livello(stile-risolto, level) = {
  let liv = _livello(stile-risolto, level)
  if liv != none and "divider" in liv { return liv.divider }
  let td = stile-risolto.gridlines.at("task-dividers", default: none)
  if type(td) == array {
    if td.len() == 0 { none } else { td.at(calc.min(level, td.len() - 1)) }
  } else {
    td
  }
}

/// Componente ORIZZONTALE di `style.sidebar.align` (rilievo del
/// coordinatore, rereview di Fase 5): `bars.typ` faceva `align(s.align +
/// horizon, ..)`, un `panic` se `s.align` era già 2D (es. `center +
/// horizon`, "duplicate alignment component") o non un `alignment`
/// affatto (es. una stringa scritta da YAML, "center"). Normalizzato QUI
/// (RNF3: una volta sola, non per riga) con un tipo controllato: non un
/// `alignment` -> `E-VAL-03`; un `alignment` puramente VERTICALE (es.
/// `top`, `.x` dà `none`, verificato con una sonda dedicata) -> stesso
/// `E-VAL-03` (nessuna componente orizzontale da estrarre). Un
/// `alignment` 1D orizzontale o 2D passa invariato nella sua componente
/// orizzontale (`.x`, campo nativo di Typst).
#let allineamento-orizzontale(valore, percorso, locale) = {
  if type(valore) != alignment {
    fail(
      "E-VAL-03", percorso, locale,
      argomenti: (expected: "alignment"), received: repr(valore),
    )
  }
  let orizzontale = valore.x
  if orizzontale == none {
    // D5 (rilievo del coordinatore, rereview di Fase 6a, D-106): prima
    // la frase "alignment con componente orizzontale" era cablata in
    // ITALIANO qui, mostrata anche con `locale: "en"` — ora è la voce
    // composita `E-VAL-03.horizontal` (en/it, `data/locales/*.toml`),
    // letta direttamente dal locale come VALORE di `{expected}` (stesso
    // schema di `E-HDR-05.*`/`E-VAL-09.auto`): nessuna prosa nel core.
    fail(
      "E-VAL-03", percorso, locale,
      argomenti: (expected: locale.errors.at("E-VAL-03.horizontal")), received: repr(valore),
    )
  }
  orizzontale
}

/// Allineamento orizzontale della sidebar per `level` (RF5 `style.
/// levels[l].sidebar.align`, D-106 punto (d)): se il livello (o quello
/// ripetuto) dichiara un `align` proprio, VINCE su `style.sidebar.align`
/// globale — stessa normalizzazione di `allineamento-orizzontale`
/// (componente orizzontale, `E-VAL-03` se non è un `alignment` con
/// componente orizzontale), cosi' un `style.levels[l].sidebar.align`
/// scritto male fallisce con lo STESSO errore del globale, non un
/// `panic` diverso a seconda di quale dei due l'utente ha scritto.
#let sidebar-align-livello(stile-risolto, level, locale) = {
  let liv = _livello(stile-risolto, level)
  let override = if liv == none { none } else { liv.at("sidebar", default: (:)).at("align", default: none) }
  if override != none {
    allineamento-orizzontale(override, "style.levels." + str(level) + ".sidebar.align", locale)
  } else {
    allineamento-orizzontale(stile-risolto.sidebar.align, "style.sidebar.align", locale)
  }
}

/// Dizionario di override per-unita' di `style.headers.<nome-unita>`
/// (RF4.3), con guardia per una collisione di namespace DOCUMENTATA
/// (suggerimento, rereview di Fase 6a/D-106): `style.headers` mescola
/// chiavi FISSE globali (`stroke`/`text`/`inset`, non dizionari di
/// override — es. `headers.stroke` è UNO stroke, non un dizionario per-
/// unità) con chiavi PER-UNITA' (RF4.3 permette unità CUSTOM con
/// qualunque nome, `units.typ`/drawer): un'unità chiamata letteralmente
/// `"stroke"` o `"inset"` farebbe leggere qui il valore FISSO (uno
/// stroke/una lunghezza, non un dizionario) invece di un override per
/// quell'unità, e un `.at("align", ..)` su un valore non-dizionario
/// andrebbe in panic. La guardia sotto rende il caso INOFFENSIVO (nessun
/// override silenzioso, nessun panic): un valore fisso non-dizionario
/// non è un override valido, quindi si comporta come "nessun override".
/// **`"text"` inclusa nella stessa guardia** (suggerimento, review
/// finale di Fase 6a): `style.headers.text` è ANCH'ESSA una chiave
/// FISSA globale (un dizionario di argomenti `text()`, non un override
/// per unità) — senza il controllo di TIPO qui sarebbe passata
/// indenne (è un `dictionary`, come un override vero), e un'unità
/// chiamata letteralmente `"text"` avrebbe silenziosamente letto lo
/// stile GLOBALE del testo come se fosse il SUO override: non fa
/// panic da sola (`"text" in override-unita` cercherebbe una chiave
/// `text` dentro quel dizionario, quasi sempre assente), ma è comunque
/// un comportamento sbagliato e silenzioso per un'unità chiamata così.
/// Non c'e' un test end-to-end per un'unità chiamata `"stroke"`/
/// `"text"`/`"inset"` (nessuna unità inclusa si chiama così, D-091/
/// RF4.3) — limite noto, qui solo la guardia che evita l'ambiguità.
#let _override-unita-headers(stile-risolto, nome-unita) = {
  if nome-unita in ("stroke", "text", "inset") { return (:) }
  let v = stile-risolto.headers.at(nome-unita, default: (:))
  if type(v) == dictionary { v } else { (:) }
}

/// Allineamento di un livello d'intestazione (RF4/RF4.3, R6 — D-102):
/// `style.headers.<nome-unita>.align` (se presente) sostituisce
/// INTERAMENTE l'`align` DICHIARATO dalla definizione d'unità
/// (`align-definizione`, `units.typ`: `center + horizon` per la
/// maggior parte, `left + horizon` per `week`, `center` per `day`) —
/// nessun merge (un `alignment` non è un `dictionary`, `unisci-
/// profondo` farebbe comunque una sostituzione semplice): l'override è
/// o tutto o niente, come un allineamento qualunque. Il TIPO si
/// controlla SOLO se l'utente lo ha scritto (`align-definizione` è un
/// valore interno, sempre valido per costruzione) — a differenza di
/// `allineamento-orizzontale`, qui è ammesso anche un `alignment`
/// puramente verticale o 1D orizzontale (RF4 lo richiede per `day`:
/// `center`, 1D).
#let allineamento-header(stile-risolto, nome-unita, align-definizione, locale) = {
  let override = _override-unita-headers(stile-risolto, nome-unita).at("align", default: none)
  if override == none { return align-definizione }
  if type(override) != alignment {
    fail(
      "E-VAL-03", "style.headers." + nome-unita + ".align", locale,
      argomenti: (expected: "alignment"), received: repr(override),
    )
  }
  override
}

/// Inset orizzontale minimo per un'etichetta CENTRATA (Fase 6b Parte B,
/// ritocco estetico, decisione utente): un piccolo respiro simmetrico
/// anche quando l'etichetta non tocca stabilmente un bordo (prima
/// `0pt`, D-103 — "con center nessun effetto") — costante fissa, non
/// derivata da `style.headers.inset` (quel valore resta SOLO per il
/// caso `left`/`right`, un margine dal bordo toccato: un significato
/// diverso).
#let _inset-center = 1pt

/// Inset orizzontale `(left:, right:)` secondo la componente
/// orizzontale di `allineamento` (`left`/`right`/`center` o nessuna
/// componente): `left` -> `valore` a sinistra, `right` -> `valore` a
/// destra, altrimenti (Fase 6b Parte B) `_inset-center` SIMMETRICO su
/// entrambi i lati — vedi `inset-header` sotto per il perché. Estratta
/// (suggerimento, review finale di Fase 6a) dai TRE punti che
/// ripetevano lo stesso `if allineamento.x == left {...} else if ==
/// right {...} else {...}`: qui, nell'override `headers[i].inset` e
/// nell'override `style.headers.<unità>.inset` di
/// `livello-header-risolto` sotto — stessa regola, fonte unica (RNF5).
#let _inset-lato(allineamento, valore) = {
  let oriz = allineamento.x
  if oriz == left {
    (left: valore, right: 0pt)
  } else if oriz == right {
    (left: 0pt, right: valore)
  } else {
    (left: _inset-center, right: _inset-center)
  }
}

/// Inset ORIZZONTALE di un'etichetta d'intestazione secondo il suo
/// allineamento RISOLTO (RF4/R6, D-103, rilievo visivo del
/// coordinatore): `left` -> inset a sinistra (allontana l'etichetta dal
/// bordo sinistro/dalla griglia a cui e' "attaccata"); `right` -> inset
/// a destra; `center` (o nessuna componente orizzontale, es. un
/// allineamento puramente verticale) -> inset SIMMETRICO minimo di 1pt
/// (Fase 6b Parte B, decisione utente — prima `0pt` su entrambi i lati,
/// D-103: un'etichetta centrata non tocca stabilmente un bordo, ma un
/// filo di respiro evita che tocchi la griglia a parità di larghezza
/// cella). L'originale (D-032, solo riferimento funzionale) applicava
/// lo stesso `pad` fisso solo nel caso `left + horizon`. Restituisce un
/// dizionario `(left:, right:)` pronto per `pad(..)`: STESSA funzione
/// usata sia per il render (`render/headers.typ`) sia per la larghezza
/// "richiesta" nel calcolo di overflow (`layout._intestazioni-grezze`),
/// cosi' le due letture non possono divergere (RNF5, fonte unica).
#let inset-header(stile-risolto, allineamento) = _inset-lato(allineamento, stile-risolto.headers.inset)

/// Legge e valida un campo di TIPO `length` (eventualmente anche
/// `auto`, con `ammetti-auto: true`) di `style.headers.<nome-unita>.
/// <nome-campo>` (T27/T33, rilievo del coordinatore in review finale
/// di Fase 6a): a differenza di `headers[i].<campo>` (gia' validato in
/// profondita' da `_valida-headers` in `validate.typ`), `style:` non è
/// MAI validato in profondita' altrove in questo modulo — un valore
/// col TIPO sbagliato scritto qui (es. una stringa per refuso) andrebbe
/// altrimenti in un panic "nudo" di Typst alla prima operazione
/// aritmetica/di confronto in `layout.typ` (T27/T33: "expected
/// integer/length, found string"), non nel formato RF7 del catalogo.
/// Stessa tecnica di `allineamento-header` sopra (controllo di tipo
/// SOLO se l'utente lo ha scritto: `valore` qui arriva già da `.at(..,
/// default: none)`/`"<campo>" in override-unita` del chiamante, mai
/// chiamata "a vuoto").
#let _valida-lunghezza-header(valore, percorso, locale, ammetti-auto: false) = {
  if ammetti-auto and valore == auto { return valore }
  if type(valore) != length {
    fail(
      "E-VAL-03", percorso, locale,
      argomenti: (expected: if ammetti-auto { "auto|length" } else { "length" }),
      received: repr(valore),
    )
  }
  valore
}

/// Come sopra, per `style.headers.<nome-unita>.overflow` (E-VAL-08,
/// stesso elenco ammesso, in ordine alfabetico, di `headers[i].
/// overflow` in `validate.typ`: "clip"/"hide"/"show").
#let _valida-overflow-header(valore, percorso, locale) = {
  if valore not in ("clip", "hide", "show") {
    fail("E-VAL-08", percorso, locale, received: repr(valore), allowed: ("clip", "hide", "show"))
  }
  valore
}

/// `style.dependencies.route` (RF10, Fase 7): unico campo di
/// `style.dependencies` a selezionare un RAMO di codice in
/// `layout.calcola` (quale funzione di routing chiamare) — un valore
/// sbagliato qui, lasciato senza controllo, cadrebbe silenziosamente sul
/// ramo "orthogonal" di default (contrario a D-018, "mai un errore
/// silenzioso"), invece che segnalare l'errore di scrittura. Stessa
/// tecnica di `_valida-overflow-header` sopra (E-VAL-08, elenco ammesso
/// in ordine alfabetico). Gli altri campi numerici di `style.
/// dependencies` (`gap`/`curviness`/`bulge`/`arrow`/`margin`/`stroke`/
/// `violated.stroke`) restano, come altri campi scalari di stile in
/// questo modulo (es. `milestones.label.gap`), senza una validazione
/// dedicata: un tipo sbagliato lì produce il panic "nudo" di Typst alla
/// prima operazione aritmetica in `layout.typ`, limite noto e coerente
/// con il resto del modulo (nessuna nuova asimmetria introdotta qui).
#let route-dipendenze(stile-risolto, locale) = {
  let valore = stile-risolto.dependencies.route
  if valore not in ("bezier", "orthogonal", "straight") {
    fail("E-VAL-08", "style.dependencies.route", locale, received: repr(valore), allowed: ("bezier", "orthogonal", "straight"))
  }
  valore
}

/// `style.milestones.label.repeat` (RF12, Fase 8.5, D-127): `auto`
/// (default) attiva la regola del footer alto in `layout.calcola`
/// (`ctx.ripeti-etichette`), `true`/`false` sono scelte ESPLICITE
/// dell'utente e si rispettano sempre (nessuna regola). Validato QUI
/// (come `route-dipendenze` sopra, D-018 "mai un errore silenzioso"),
/// ad OGNI compilazione, anche senza milestone (la regola gira comunque
/// su un `altezza-etichette` pari al solo `overhang`): un valore fuori
/// da `(auto, true, false)` darebbe altrimenti un confronto silenzioso
/// sbagliato nella regola (`repeat == auto` falso per qualunque valore
/// "strano" → cadrebbe nel ramo `else { repeat }`, che userebbe quel
/// valore non booleano direttamente in `table.footer(repeat: ..)`,
/// panic "nudo" lontano dal vero errore di scrittura). `E-VAL-03` con
/// `expected: "auto|boolean"` (stringa LETTERALE, stessa convenzione di
/// `"auto|length"` per `sidebar-width`, `validate.typ`: nessun testo
/// `it` nuovo nei locali, D-127 punto 1). **CRITICO (review del
/// 2026-10-18, D-128): `none` vale come chiave ASSENTE, cioè `auto`**
/// (D-127 punto 1, RF5: "`none` vale come chiave assente (`auto`,
/// T29)") — SENZA questa normalizzazione un `style: (milestones:
/// (label: (repeat: none)))` dell'utente (YAML `repeat: ~` o una
/// chiave vuota) darebbe un falso `E-VAL-03`: `unisci-profondo`
/// (sopra) per un valore `b` che non è un `dictionary` restituisce
/// `b` COSÌ COM'È (anche se `none`), quindi `none` SOVRASCRIVE il
/// default `auto` invece di esserne assorbito — a differenza di una
/// chiave del tutto ASSENTE da `style.milestones.label` (che
/// `unisci-profondo` lascia al valore del default, `auto`, perché
/// quella chiave non compare affatto in `b`). La normalizzazione va
/// PRIMA del controllo sull'insieme ammesso, non dopo.
#let valida-repeat-etichette(stile-risolto, locale) = {
  let valore-grezzo = stile-risolto.milestones.label.repeat
  let valore = if valore-grezzo == none { auto } else { valore-grezzo }
  if valore not in (auto, true, false) {
    fail("E-VAL-03", "style.milestones.label.repeat", locale, argomenti: (expected: "auto|boolean"), received: repr(valore))
  }
  valore
}

/// Come `_valida-lunghezza-header`, per `style.headers.<nome-unita>.
/// text` (deve essere un `dictionary` di argomenti `text()`: un valore
/// di altro tipo farebbe fallire `unisci-profondo`/lo spread `..` in
/// `text(..)` sotto con un panic "nudo", T27/T33).
#let _valida-dizionario-header(valore, percorso, locale) = {
  if type(valore) != dictionary {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "dictionary"), received: repr(valore))
  }
  valore
}

/// `style.milestones.label` (Fase 8.9, residuo 1, D-138; esteso nella
/// review finale di Fase 9, D-141): deve restare un `dictionary` — un
/// `style: (milestones: (label: none))` dell'utente sovrascriverebbe il
/// default (`unisci-profondo` restituisce `b` così com'è se non è un
/// `dictionary`, STESSO caso di `_valida-dizionario-header` sopra per
/// `style.headers.<unità>.text`): il primo accesso a un campo di
/// `style.milestones.label` in `layout.calcola` darebbe altrimenti un
/// panic "nudo" di Typst ("cannot access fields on type none"), lontano
/// dal vero errore di scrittura (T27/T33). **Esteso (non solo il
/// dizionario esterno):** la PRIMA versione di questa guardia (D-138)
/// lasciava passare un `label` ancora valido come dizionario ma con un
/// CAMPO interno sbagliato, es. `label: (text: none)` o `label: (inset:
/// none)` — `unisci-profondo` sovrascrive solo la chiave toccata
/// dall'utente, le altre restano dal default, quindi il dizionario
/// esterno resta un `dictionary` (la guardia di D-138 lo accetta) ma
/// `layout.calcola` andava comunque in panic nudo al primo uso di quel
/// campo (`label.text.at("fill", ..)` su `none` -> "type none has no
/// method `at`"; `2 * label.inset` su `none` -> "cannot multiply
/// integer with none", verificato con una sonda dedicata). Validati ORA
/// anche `label.text` (`dictionary`, stesso controllo di
/// `_valida-dizionario-header`) e `label.inset`/`label.overhang`
/// (`length`, stesso controllo di `_valida-lunghezza-header`, usato
/// altrove per `style.headers.<unità>.inset`/`.height`): sono gli unici
/// campi di `label` letti con un'operazione (`.at(..)`, `*`, `+`) che
/// fa panic nudo su un tipo sbagliato invece di un valore neutro
/// (`label.fill`/`.gap`/`."connector-threshold"` restano FUORI da
/// questa guardia manuale: `box(fill: none)` non va in panic, e una
/// validazione sistematica di OGNI campo di `style` arriva con la
/// conversione YAML stringa->tipo della review finale di Fase 9,
/// contratto in D-140 — qui si chiude solo il residuo dimostrato).
/// Chiamata UNA VOLTA, all'inizio di `layout.calcola` (come
/// `route-dipendenze`), PRIMA di qualunque lettura di un campo — anche
/// senza nessuna milestone in `spec`: la regola del footer alto (D-127)
/// legge comunque `.overhang`. Nessun testo `it` nuovo (token tecnico
/// `"dictionary"`/`"length"`, stesso `E-VAL-03` di sempre).
#let valida-milestone-label(stile-risolto, locale) = {
  let label = _valida-dizionario-header(stile-risolto.milestones.label, "style.milestones.label", locale)
  // `let _ = ..` su OGNI chiamata (bug trovato nella review finale di
  // Fase 9, D-140, scrivendo i test per label.text/.inset): due o più
  // chiamate "nude" di seguito, con valori di ritorno di TIPO DIVERSO
  // (qui un `dictionary` da `_valida-dizionario-header` seguito da un
  // `length` da `_valida-lunghezza-header`), vengono "unite" dal blocco
  // di codice come farebbe il markup — "cannot join dictionary with
  // length", un panic "nudo" di Typst dentro QUESTA stessa guardia
  // anti-panic (ironico, T27/T33 di nuovo). `let _ =` scarta il
  // risultato invece di lasciarlo "in giro" per il blocco.
  let _ = _valida-dizionario-header(label.text, "style.milestones.label.text", locale)
  let _ = _valida-lunghezza-header(label.inset, "style.milestones.label.inset", locale)
  let _ = _valida-lunghezza-header(label.overhang, "style.milestones.label.overhang", locale)
  label
}

/// Stile EFFETTIVO di un livello d'intestazione (RF4/RF5, D-106 punto
/// (a) — "headers a dizionario per livello", richiesta originaria
/// dell'utente): per OGNI campo, la precedenza è
/// `headers[i].<campo>` (dizionario del livello) > `style.headers.
/// <nome-unita>.<campo>` > (per i campi che la definizione d'unità
/// dichiara: `align`, `text`, `gridline`, `min-cell-width`) la
/// DEFINIZIONE > il default GLOBALE (`style.headers.<campo>`, o un
/// valore cablato per i campi senza default globale, es. `fill: none`).
/// `text` è un CASO A PARTE: le tre fonti NON si sostituiscono a
/// vicenda ma si FONDONO in profondità (ogni livello di precedenza può
/// cambiare SOLO le chiavi di `text()` che scrive, es. solo `size`),
/// nell'ordine default globale -> `def.text` (gia' letta da `units.
/// typ`, mai usata prima d'ora) -> `style.headers.<unita>.text` ->
/// `headers[i].text`. `livello` è l'elemento di `modello.headers`
/// (uscita di `timeline.calcola()`, con i campi `livello-*`/`def-text`
/// già estratti). Risolto UNA VOLTA per livello (RNF3), mai per
/// segmento.
#let livello-header-risolto(stile-risolto, nome-unita, livello, locale) = {
  let override-unita = _override-unita-headers(stile-risolto, nome-unita)
  // Letture DIFENSIVE (`.at(default: none)`): alcuni fixture di test
  // (`unit/layout.typ`) costruiscono un `livello` a mano senza i campi
  // `livello-*`/`def-text` — producono tutti `timeline.calcola()` (D-106
  // punto (a)) ma in produzione, non per forza nei fixture che testano
  // SOLO la geometria dei segmenti.
  let livello-align = livello.at("livello-align", default: none)
  let livello-text = livello.at("livello-text", default: none)
  let livello-fill = livello.at("livello-fill", default: none)
  let livello-stroke = livello.at("livello-stroke", default: none)
  let livello-height = livello.at("livello-height", default: none)
  let livello-overflow = livello.at("livello-overflow", default: none)
  let livello-min-cell-width = livello.at("livello-min-cell-width", default: none)
  let livello-inset = livello.at("livello-inset", default: none)
  // Campo a sostituzione semplice (non fuso): valore-livello se
  // presente (anche `none` esplicito, RF2 "la chiave c'e'" conta più
  // del suo valore), altrimenti lo style per-unità (con guardia di tipo
  // OPZIONALE `valida:`, review finale di Fase 6a: `style.headers.
  // <unità>.<campo>` non è MAI validato altrove, a differenza di
  // `headers[i].<campo>`, già controllato in profondità da
  // `_valida-headers` — senza questa guardia un valore scritto male
  // qui arriverebbe fino a `layout.typ` come panic "nudo", T27/T33),
  // altrimenti il fallback passato dal chiamante (già la definizione o
  // il default globale, a seconda del campo — MAI validato: è un
  // valore interno, sempre valido per costruzione).
  let campo(valore-livello, nome-campo-stile, fallback, valida: none) = {
    if valore-livello != none { return valore-livello }
    if nome-campo-stile in override-unita {
      let v = override-unita.at(nome-campo-stile)
      if valida != none { return valida(v) }
      return v
    }
    fallback
  }
  // `align` NON passa da `campo()` generico: `headers[i].align` (se
  // presente) è GIA' di tipo `alignment` (validato da `validate.typ`,
  // `_valida-headers`) — usarlo direttamente evita di farlo passare
  // ANCHE per `allineamento-header` sotto, che farebbe un SECONDO
  // controllo "style.headers.<unità>.align" e lo sovrascriverebbe
  // (bug trovato scrivendo il test "headers[i].align VINCE..."
  // sotto: la precedenza più alta va PRIMA di tutto, non dopo).
  // `allineamento-header` resta la via per gli altri DUE livelli
  // (style.headers.<unità> > definizione), con la sua validazione di
  // tipo per il caso NON ancora validato altrove (`style:` non è
  // validato in profondità).
  let allineamento = if livello-align != none {
    livello-align
  } else {
    allineamento-header(stile-risolto, nome-unita, livello.at("align", default: center + horizon), locale)
  }
  let testo = {
    let t = stile-risolto.headers.text
    t = unisci-profondo(t, livello.at("def-text", default: (:)))
    if "text" in override-unita {
      let t-unita = _valida-dizionario-header(override-unita.text, "style.headers." + nome-unita + ".text", locale)
      t = unisci-profondo(t, t-unita)
    }
    if livello-text != none { t = unisci-profondo(t, livello-text) }
    t
  }
  // `gridline` NON passa da `campo()`/`style.headers.<unita>`: la sua
  // catena di RF4.3 e' una namespace SEPARATA, gia' stabilita (D-102,
  // `style.gridlines.<unit>s`, non `style.headers.<unit>.gridline`) —
  // qui si applica SOLO il nuovo livello piu' alto, `headers[i].
  // gridline` (D-106 punto (a)); il resto della catena (`style.
  // gridlines.<unit>s` > definizione > `none`) resta in `layout.
  // gridline-unita`, chiamata dal chiamante di questa funzione (RNF5:
  // `style.typ` non dipende da `layout.typ`).
  let riempimento = campo(livello-fill, "fill", none)
  let tratto = campo(livello-stroke, "stroke", stile-risolto.headers.stroke)
  // Guardie di tipo (review finale di Fase 6a, rilievo del
  // coordinatore, T27/T33): STESSO schema di `allineamento-header`
  // sopra, applicato a `height`/`min-cell-width`/`overflow` di
  // `style.headers.<unità>` — `headers[i].<campo>` (il valore-livello
  // passato a `campo()`) resta NON validato qui perché `validate.typ`
  // lo ha già fatto in profondità.
  let altezza = campo(
    livello-height, "height", auto,
    valida: v => _valida-lunghezza-header(v, "style.headers." + nome-unita + ".height", locale, ammetti-auto: true),
  )
  let overflow = campo(
    livello-overflow, "overflow", "hide",
    valida: v => _valida-overflow-header(v, "style.headers." + nome-unita + ".overflow", locale),
  )
  let larghezza-minima = campo(
    livello-min-cell-width, "min-cell-width", livello.at("min-cell-width"),
    valida: v => _valida-lunghezza-header(v, "style.headers." + nome-unita + ".min-cell-width", locale),
  )
  // `_inset-lato` (estratta, suggerimento): stessa regola di
  // `inset-header` sopra, qui con un valore ESPLICITO (del livello o
  // dello style per-unità) invece del default globale.
  let inset-oriz = inset-header(stile-risolto, allineamento)
  if livello-inset != none {
    inset-oriz = _inset-lato(allineamento, livello-inset)
  } else if "inset" in override-unita {
    let i = _valida-lunghezza-header(override-unita.inset, "style.headers." + nome-unita + ".inset", locale)
    inset-oriz = _inset-lato(allineamento, i)
  }
  (
    align: allineamento, text: testo, fill: riempimento, stroke: tratto,
    // `headers[i].gridline`, se scritto: il SOLO livello aggiunto qui
    // (vedi sopra); il chiamante lo combina con `layout.gridline-unita`.
    "gridline-livello": livello.at("livello-gridline", default: none),
    height: altezza, overflow: overflow, "min-cell-width": larghezza-minima,
    inset: inset-oriz,
  )
}

/// Stile effettivo di una milestone (RF5, COMPLETO da Fase 6a, D-102):
/// `m.style` > `m.color` > (`style.milestones.today` se `today`,
/// altrimenti `style.milestones.normal`). `color` sostituisce SOLO il
/// `paint` dello stroke (coerente con `stile-barra`: il `color` di un
/// nodo lascia invariati i colori di stato/`timeframe`, qui lascia
/// invariato lo spessore e ogni altro campo dello stroke): lo stroke di
/// base si normalizza PRIMA con il costruttore nativo `stroke(..)`
/// (T40: un valore come `1pt + black` è già un `stroke`, ma un futuro
/// default scritto come dizionario non lo sarebbe) per poterne leggere
/// `.thickness` senza perderlo nella sostituzione. `m.style` (se non
/// vuoto) ha sempre la precedenza più alta, come per i nodi. `locale:
/// none` di default (review finale di Fase 9, D-140): converte le
/// stringhe YAML di `m.style` (`converti-stringhe`, schema `(stroke: ..)`
/// — la stessa forma di `stile-risolto.milestones.normal`/`.today`);
/// `none` e' sicuro per le fixture di `tests/unit/style.typ` che
/// chiamano questa funzione senza locale (stessa nota di `risolvi`).
/// Ripiego su `locales.en` se `locale` è `none` (rereview finale di
/// Fase 9, code-reviewer).
#let stile-milestone(stile-risolto, m, today: false, locale: none) = {
  let locale = if locale == none { locales.en } else { locale }
  let base = if today { stile-risolto.milestones.today } else { stile-risolto.milestones.normal }
  // DS-A (4a rereview, code-reviewer): una milestone disegna SEMPRE un
  // segno (RF5) — `stroke: none` non è ammesso, a differenza di altri
  // campi di tipo stroke (`headers.stroke`/`dependencies.stroke`/
  // `gridlines.*.stroke`, dove "nessun tratto" ha senso). Guardia QUI,
  // PRIMA di `stroke(base.stroke)` due righe sotto (che altrimenti
  // panica "nudo" su un letterale Typst `none` — MAI una stringa,
  // quindi MAI intercettato da `_stroke-none-escluso`/`converti-
  // stringhe` sopra, che lavorano solo sulle stringhe YAML): copre
  // anche questo caso, non solo la stringa "none" da YAML.
  if base.stroke == none {
    fail(
      "E-VAL-03", "style.milestones." + (if today { "today" } else { "normal" }) + ".stroke", locale,
      argomenti: (expected: "stroke"), received: repr(base.stroke),
    )
  }
  let con-colore = if m.at("color", default: none) != none {
    // Suggerimento (rereview di Fase 6a, D-106): ricostruire lo stroke
    // con TUTTI i campi di base (non solo `paint`/`thickness`) — prima
    // un `dash`/`cap`/`join`/`miter-limit` esplicito nello stile di
    // base andava perso quando la milestone aveva solo `color`, non
    // `style` (che invece passa da `unisci-profondo` e li conserva).
    let base-stroke = stroke(base.stroke)
    unisci-profondo(base, (stroke: (
      paint: m.color, thickness: base-stroke.thickness,
      cap: base-stroke.cap, join: base-stroke.join,
      dash: base-stroke.dash, "miter-limit": base-stroke.miter-limit,
    )))
  } else {
    base
  }
  let proprio = m.at("style", default: (:))
  // `path` (rereview finale di Fase 9, code-reviewer, punto 4): questo
  // record (uscita di `timeline.calcola`) ha `nome`, non `name` — `m.at(
  // "name", default: "?")` non trovava MAI la chiave vera, dava sempre
  // il percorso fuorviante `milestones["?"]`. `timeline.typ` espone ora
  // `path: "milestones[i]"` per ogni milestone vera; la chiamata
  // sintetica per "oggi" (`layout.typ`, `(style: (:))`, nessuna
  // milestone reale dietro) non ha `path`: ripiego su `"today"`, più
  // chiaro di un indice inventato.
  let percorso-m = m.at("path", default: "today")
  // `default-style.milestones.normal` (2a rereview, code-reviewer,
  // punto 3), NON `stile-risolto.milestones.normal`: lo schema deve
  // essere lo stesso FISSO di `stile-barra` (`default-style.tasks`,
  // sopra) — se l'utente avesse già cambiato `style.milestones.normal.
  // stroke` (non più ammesso in `none`, DS-A: vedi sopra), usare il
  // valore RISOLTO come schema avrebbe perso il tipo `stroke` proprio
  // quando serve di più: una stringa in QUELLA stessa milestone
  // sarebbe rimasta invariata (nessun tipo da confrontare) e andata in
  // panic "nudo" più sotto nel render.
  let proprio-convertito = if proprio.len() > 0 {
    converti-stringhe(default-style.milestones.normal, proprio, percorso-m + ".style", locale)
  } else {
    (:)
  }
  let risultato = if proprio-convertito.len() > 0 { unisci-profondo(con-colore, proprio-convertito) } else { con-colore }
  // DS-A (4a rereview, code-reviewer): stessa guardia di sopra, qui per
  // `m.style.stroke: none` (letterale Typst — la stringa YAML "none" è
  // già esclusa da `_stroke-none-escluso` e fallisce prima, dentro
  // `converti-stringhe`, con lo stesso E-VAL-03).
  if risultato.stroke == none {
    fail("E-VAL-03", percorso-m + ".style.stroke", locale, argomenti: (expected: "stroke"), received: repr(risultato.stroke))
  }
  risultato
}
