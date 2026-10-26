// gantt-newgen — src/validate.typ
// Validazione e normalizzazione dello schema di input (RF2): entrambe le
// facce (0.1 `taskgroups[].tasks[]`, 0.5 `tasks[].subtasks[]`, D-025/D-036)
// come lo STESSO albero di nodi, con annidamento illimitato fino al limite
// di profondita' (E-VAL-13), intervalli validati in profondita' (sovrapposizione
// E-VAL-12) e dipendenze (src/graph.typ: indice id, cicli, ordine topologico,
// E-DEP-01..05). Core puro: niente disegno, niente `measure` (RNF5).
//
// Date ancora GREZZE in uscita (D-050): la conversione in istanti effettivi
// di `date-mode` (RF3) e' compito della timeline (Fase 4); qui `modo` serve
// solo per i confronti di validita' (E-VAL-09, sovrapposizione di intervalli).

#import "errors.typ": fail, nome-tipo
#import "compat.typ"
#import "dates.typ"
#import "graph.typ"
#import "locale.typ"
#import "schema.typ"
#import "style.typ"
#import "units.typ"

/// Unisce due dizionari in profondita' (merge ricorsivo, RF1: "`style`
/// e' unito in profondita'"); per ogni coppia che non e' due dizionari
/// vale la sostituzione semplice (l'ultimo valore vince).
#let _unisci-profondo(a, b) = {
  if type(a) == dictionary and type(b) == dictionary {
    let r = a
    for (chiave, valore) in b {
      r.insert(chiave, if chiave in r { _unisci-profondo(r.at(chiave), valore) } else { valore })
    }
    r
  } else {
    b
  }
}

/// Una chiave opzionale di array (`milestones`, `taskgroups`, `tasks`,
/// `dependencies`, ...) scritta in YAML come `chiave:` senza valore (o
/// `~`) arriva come `none` (verificato: `yaml()` da' `none`, non `()`,
/// per uno scalare nullo), mentre `chiave: []` da' gia' `()`. Qui
/// `none` e assenza contano come "nessun elemento", non come errore di
/// tipo (MINORI: "liste vuote da YAML (none) come ()").
#let _array-o-vuoto(valore) = if valore == none { () } else { valore }

/// Verifica le chiavi di `dizionario` contro `ammesse` (`strict`,
/// D-018): ogni chiave non ammessa fa fallire con E-VAL-02, suggerimento
/// secondo D-029/SD-4. Non controlla nulla se `strict` e' falso.
#let _verifica-chiavi(dizionario, ammesse, percorso, strict, locale) = {
  if not strict { return }
  for chiave in dizionario.keys() {
    if chiave not in ammesse {
      let percorso-chiave = if percorso == "" { chiave } else { percorso + "." + chiave }
      fail("E-VAL-02", percorso-chiave, locale, suggest: schema.suggerisci(chiave, ammesse))
    }
  }
}

/// Risolve una STRINGA come colore (nome della tavola, D-053/D-106
/// punto (c): `style.tavola-colori`, fonte unica, o `"#rrggbb"`/`"#rgb"`
/// tra virgolette, T8/T10): E-VAL-06 se non riconosciuta. **Spostata in
/// `style.typ` (review finale di Fase 9, D-140): `style.colore-da-
/// stringa`** — `style.typ` è già la fonte unica di `tavola-colori` e,
/// da questa fase, anche dell'intera conversione di stringhe YAML nello
/// stile (l'"oracolo di tipo"), che deve poter usare questa stessa
/// funzione senza un'importazione circolare (`style.typ` non può
/// importare `validate.typ`, che già importa `style.typ`: RNF5,
/// "Struttura dei moduli"). Stessa funzione, stesso codice d'errore,
/// solo la sede: nessun cambiamento di comportamento. Riusata qui da
/// `_valida-colore` (sotto) e da `_valida-headers` (review finale di
/// Fase 6a: `headers[i].fill`/`.stroke`/`.gridline` accettano, OLTRE ai
/// tipi Typst già ammessi, una stringa della tavola colori).

/// Colore da una stringa YAML (nome della tavola o `"#rrggbb"`/`"#rgb"`
/// tra virgolette, T8/T10) o da un `color` Typst gia' costruito; `none`
/// esplicito o qualunque altro valore non valido -> E-VAL-06 (D-030/SD-5,
/// I-12: un solo modello per i due casi, "vedi README/colore assente o
/// non valido", senza affermare che il valore sia sempre `none`).
#let _valida-colore(valore, percorso, locale) = {
  if valore == none {
    fail("E-VAL-06", percorso, locale, received: "none")
  }
  if type(valore) == color {
    return valore
  }
  if type(valore) == str {
    return style.colore-da-stringa(valore, percorso, locale)
  }
  fail("E-VAL-06", percorso, locale, received: repr(valore))
}

/// `progress` normalizzato a un numero 0-100 (percentuale) o `none`
/// (D-012/D-029): ratio Typst, stringa `"40%"`, int 0-100, float `0.0`
/// o 1-100. Un float (o una stringa numerica) in (0, 1) -> E-VAL-07 col
/// suggerimento, messaggio reale "— forse '{valore × 100}'?" (it)/"did
/// you mean '{valore × 100}'?" (en), non "intendevi...?" (corretto nella
/// review finale di Fase 9, D-140 punto (e): la specifica aveva lo stesso
/// refuso, riallineata) (D-029 punto 4, estesa alle
/// stringhe come MINORI); fuori da 0-100 (qualunque forma) -> E-VAL-07
/// senza quel suggerimento. Mai `float()`/`int()` su una stringa non
/// gia' verificata con una regex (D-030: guardia prima del costruttore,
/// come per le date). Ordine tipo -> finitezza -> dominio (B-2, skill
/// guardia-validita-nel-lettore): un `nan` NON passa "silenziosamente"
/// un confronto d'ordine come in altri linguaggi — in Typst
/// `nan > 0`/`nan < 1` SOLLEVANO un errore ("cannot compare float.nan
/// with 0.0"), verificato mutando la guardia (D-057): e' un panic
/// "nudo" di Typst (nessun prefisso/codice/percorso, contrario a RF7),
/// non un valore che scivola oltre il controllo. La guardia di
/// finitezza va comunque intercettata PRIMA di qualunque "<"/">" su
/// `valore`, altrimenti e' quel confronto stesso a far panicare.
/// `inf`/`-inf` invece confrontano regolarmente (li cattura gia' il
/// controllo "< 0 or > 100" da solo), ma il controllo esplicito resta
/// per simmetria e per non dipendere da quella coincidenza.
#let _valida-progress(valore, percorso, locale) = {
  if valore == none { return none }
  if type(valore) == ratio {
    // S-1 (bug trovato): `float(29%) * 100` da' `28.999999999999996`,
    // non `29.0` (imprecisione binaria sulla conversione percentuale/
    // ratio di Typst) — `calc.round(.., digits: 9)` la cancella senza
    // perdere cifre decimali plausibili per una percentuale (verificato:
    // 33.33% resta `33.33`, non viene schiacciato a un intero).
    let numero = calc.round(float(valore) * 100, digits: 9)
    if numero.is-nan() or numero.is-infinite() {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    if numero < 0 or numero > 100 {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    return numero
  }
  if type(valore) == str {
    let testo = if valore.ends-with("%") { valore.slice(0, valore.len() - 1) } else { valore }
    if testo.match(regex("^-?[0-9]+(\.[0-9]+)?$")) == none {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    let numero = float(testo)
    if numero.is-nan() or numero.is-infinite() {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    if numero > 0 and numero < 1 {
      fail("E-VAL-07", percorso, locale, received: repr(valore), suggest: str(calc.round(numero * 100)))
    }
    if numero < 0 or numero > 100 {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    return numero
  }
  if type(valore) == int {
    if valore < 0 or valore > 100 {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    return float(valore)
  }
  if type(valore) == float {
    if valore.is-nan() or valore.is-infinite() {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    if valore > 0 and valore < 1 {
      fail("E-VAL-07", percorso, locale, received: repr(valore), suggest: str(calc.round(valore * 100)))
    }
    if valore < 0 or valore > 100 {
      fail("E-VAL-07", percorso, locale, received: repr(valore))
    }
    return valore
  }
  fail(
    "E-VAL-03", percorso, locale,
    argomenti: (expected: "ratio|string|integer|float"),
    received: nome-tipo(valore),
  )
}

/// `id` normalizzato a stringa (un int e' convertito, RF2); `none` se
/// assente; qualunque altro tipo -> E-VAL-03.
#let _valida-id(dato, percorso, locale) = {
  let grezzo = dato.at("id", default: none)
  if grezzo == none { return none }
  if type(grezzo) == str { return grezzo }
  if type(grezzo) == int { return str(grezzo) }
  fail("E-VAL-03", percorso + ".id", locale, argomenti: (expected: "string|integer"), received: nome-tipo(grezzo))
}

/// Come `_valida-id`, ma per un id OBBLIGATORIO (dipendenza, RF10): non
/// esiste il ramo "assente -> none", un valore che non e' stringa né
/// intero e' sempre E-VAL-03. Tenuta separata da `_valida-id` (che serve
/// davvero il ramo opzionale, sul nodo) per non riattivare quel ramo per
/// errore in un punto dove "id" e' gia' garantito presente.
#let _normalizza-id-richiesto(valore, percorso, locale) = {
  if type(valore) == str { return valore }
  if type(valore) == int { return str(valore) }
  fail("E-VAL-03", percorso, locale, argomenti: (expected: "string|integer"), received: nome-tipo(valore))
}

/// Tipo di dipendenza (RF10/SD-6) normalizzato alla forma BREVE canonica
/// (`fs`/`ss`/`ff`/`sf`): accetta sia le forme lunghe upstream
/// (`finish-to-start`, ...) sia le brevi, e l'alias `normal` (= `fs`,
/// risolto da `compat.alias-tipo-dipendenza` PRIMA di arrivare qui).
/// `none` (chiave assente o YAML nullo esplicito) -> default `"fs"`
/// (S-7, stessa convenzione di `headers`/`style`/`sidebar-width`: un
/// valore nullo e' "non specificato", non un errore). Un valore che non
/// e' nemmeno una stringa -> E-VAL-03 (bug trovato in revisione: PRIMA
/// un `type: 42` arrivava fino a `in canonici`, che su un intero non
/// fallisce ma restituisce semplicemente `false` — quindi cadeva
/// comunque su E-VAL-08, ma con `received: repr(42)` invece di
/// segnalare l'errore di TIPO, piu' specifico e piu' chiaro). Qualunque
/// stringa non riconosciuta -> E-VAL-08 con l'elenco ammesso.
#let _normalizza-tipo-dipendenza(valore, percorso, locale) = {
  if valore == none { return "fs" }
  if type(valore) != str {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "string"), received: nome-tipo(valore))
  }
  let alias = compat.alias-tipo-dipendenza(valore)
  let canonici = (
    "finish-to-start": "fs", "fs": "fs",
    "start-to-start": "ss", "ss": "ss",
    "finish-to-finish": "ff", "ff": "ff",
    "start-to-finish": "sf", "sf": "sf",
  )
  if alias in canonici { return canonici.at(alias) }
  fail(
    "E-VAL-08", percorso, locale, received: repr(valore),
    allowed: (
      "finish-to-start", "start-to-start", "finish-to-finish",
      "start-to-finish", "fs", "ss", "ff", "sf", "normal",
    ),
  )
}

/// Verifica di tipo leggera su chiavi validate in PROFONDITA' piu' sotto
/// (`intervals`, `subtasks`, `dependencies`: array o assenti/`none`;
/// `style`/`task-style`/`x`: dizionario) solo per dare un errore chiaro
/// PRIMA della ricorsione, invece di un panic "nudo" di Typst dentro un
/// `.len()`/`for`/`.map()` sul valore del tipo sbagliato (D-030).
#let _verifica-tipo-passthrough(dato, percorso, locale) = {
  for chiave in ("intervals", "subtasks", "dependencies") {
    if chiave in dato and dato.at(chiave) != none and type(dato.at(chiave)) != array {
      fail("E-VAL-03", percorso + "." + chiave, locale, argomenti: (expected: "array"), received: nome-tipo(dato.at(chiave)))
    }
  }
  for chiave in ("style", "task-style", "x") {
    if chiave in dato and type(dato.at(chiave)) != dictionary {
      fail("E-VAL-03", percorso + "." + chiave, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato.at(chiave)))
    }
  }
}

/// Valida una voce di `…intervals[k]` (RF2, validazione in PROFONDITA':
/// Fase 3, D-036/D-052 punto 3). Restituisce
/// `(pubblico: (start, end, done, x), inizio: datetime, "fine-eff":
/// datetime, indice: int, percorso: string)`: `pubblico` e' la forma
/// normalizzata che finisce nell'output (date GREZZE, D-050); gli altri
/// campi servono SOLO al controllo di sovrapposizione del chiamante (non
/// entrano nell'output pubblico, che resta "stessa forma dell'input").
#let _valida-intervallo(dato, percorso, locale, modo, strict, indice) = {
  if type(dato) != dictionary {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato))
  }
  _verifica-chiavi(dato, schema.chiavi-intervallo, percorso, strict, locale)
  if "start" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "start"))
  }
  if "end" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "end"))
  }
  let inizio = dates.valida-data(dato.start, percorso + ".start", locale)
  let fine = dates.valida-data(dato.end, percorso + ".end", locale)
  if fine < inizio {
    fail(
      "E-VAL-05", percorso, locale,
      argomenti: (field: "end", value: dates.formatta(fine), other-field: "start", other-value: dates.formatta(inizio)),
    )
  }
  let fatto = none
  if "done" in dato and dato.done != none {
    fatto = dates.valida-data(dato.done, percorso + ".done", locale)
    if fatto < inizio {
      fail(
        "E-VAL-05", percorso, locale,
        argomenti: (field: "done", value: dates.formatta(fatto), other-field: "start", other-value: dates.formatta(inizio)),
      )
    }
  }
  let x = dato.at("x", default: (:))
  if type(x) != dictionary {
    fail("E-VAL-03", percorso + ".x", locale, argomenti: (expected: "dictionary"), received: nome-tipo(x))
  }
  (
    pubblico: (
      start: dates.formatta(inizio), end: dates.formatta(fine),
      done: if fatto == none { none } else { dates.formatta(fatto) },
      x: x,
    ),
    inizio: inizio, "fine-eff": dates.fine-effettiva(fine, modo),
    indice: indice, percorso: percorso,
  )
}

/// Valida una voce di `…dependencies[k]` (RF10): forma breve stringa O
/// INTERO (= id; un int si comporta come l'`id` del nodo stesso, che
/// pure accetta int->stringa: scelta di coerenza, non un'asimmetria,
/// seconda revisione Fase 3) o dizionario `(id, type?, x?)`. La
/// RISOLUZIONE (id esiste? antenato/discendente? nodo senza date?) e i
/// CICLI sono compito GLOBALE di `src/graph.typ` (richiede l'albero
/// completo): qui si valida solo la FORMA di questa singola voce.
#let _valida-dipendenza(voce, percorso, locale, strict) = {
  let dato = if type(voce) == str or type(voce) == int { (id: voce) } else { voce }
  if type(dato) != dictionary {
    // Terza revisione (rilievo 3): il messaggio diceva "string|dictionary",
    // ma la forma breve INTERA (`dependencies: (7,)`) e' ammessa da
    // questa stessa fase (suggerimento 4 della seconda revisione) — un
    // valore che non e' ne' stringa ne' intero ne' dizionario (es. un
    // float) deve vedere l'elenco VERO delle forme ammesse.
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "string|integer|dictionary"), received: nome-tipo(voce))
  }
  _verifica-chiavi(dato, schema.chiavi-dipendenza, percorso, strict, locale)
  if "id" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "id"))
  }
  let id = _normalizza-id-richiesto(dato.id, percorso + ".id", locale)
  let tipo = _normalizza-tipo-dipendenza(dato.at("type", default: "finish-to-start"), percorso + ".type", locale)
  let x = dato.at("x", default: (:))
  if type(x) != dictionary {
    fail("E-VAL-03", percorso + ".x", locale, argomenti: (expected: "dictionary"), received: nome-tipo(x))
  }
  (id: id, type: tipo, x: x)
}

/// Valida un NODO (faccia 0.1 `taskgroups[i].tasks[j]`, livello 1; o
/// faccia 0.5 `tasks[i]`/`…subtasks[j]`, qualunque livello): stessa
/// funzione per ENTRAMBE le facce e per QUALUNQUE profondita' (D-036),
/// chiamata ricorsivamente su `subtasks`. `profondita` parte da 1 per un
/// nodo radice (faccia 0.5) o da 2 per un task diretto di un gruppo
/// (faccia 0.1, il gruppo stesso e' profondita 1): E-VAL-13 se supera
/// `schema.max-livelli` (10, D-036/SD-8). `strict` e' quello RISOLTO da
/// `validate()` (B-1).
#let _valida-nodo-ricorsivo(dato, percorso, locale, modo, strict, profondita) = {
  if profondita > schema.max-livelli {
    fail("E-VAL-13", percorso, locale, argomenti: (depth: str(profondita), max: str(schema.max-livelli)))
  }
  if type(dato) != dictionary {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato))
  }
  if "tasks" in dato {
    // compat.typ (Fase 3): "tasks" DENTRO un nodo e' la chiave della
    // faccia 0.5 "fraintesa" al posto di "subtasks" — E-VAL-11, non un
    // generico E-VAL-02 (D-055).
    compat.errore-facce-miste(percorso, "subtasks", compat.schema-05, "tasks", compat.schema-01, locale)
  }
  _verifica-chiavi(dato, schema.chiavi-nodo, percorso, strict, locale)
  if "name" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "name"))
  }
  if type(dato.name) not in (str, content) {
    fail("E-VAL-03", percorso + ".name", locale, argomenti: (expected: "string|content"), received: nome-tipo(dato.name))
  }
  _verifica-tipo-passthrough(dato, percorso, locale)

  // S4 (seconda revisione Fase 3): DUE domande diverse, due variabili
  // diverse. "ha-intervalli" (chiave presente, anche vuota) serve
  // all'ESCLUSIVITA' con start/end/done: "intervals: []" insieme a
  // "start" resta un conflitto di schema (l'utente ha scelto lo schema
  // "a intervalli", anche se non ne ha ancora scritto nessuno) — E-VAL-12.
  // "ha-intervalli-con-dati" (chiave presente E non vuota) serve invece
  // alla domanda "serve almeno UNA fonte di date EFFETTIVA": un
  // "intervals: []" da solo (senza start/end/subtasks) NON fornisce
  // nessuna data, quindi NON basta a evitare E-VAL-01 — diversamente da
  // "subtasks: []" (D-036: quello resta "sola intestazione" per
  // costruzione, un gruppo/nodo senza figli e' comunque un nodo valido;
  // un task "a intervalli" senza nessun intervallo scritto non lo e').
  let ha-intervalli = "intervals" in dato and dato.intervals != none
  let ha-start = "start" in dato
  let ha-end = "end" in dato
  // Terza revisione (rilievo 1): come "intervals", un "subtasks: ~"
  // (YAML nullo ESPLICITO) conta come ASSENTE, non come "container
  // presente" — bug reale: un nodo con SOLO "subtasks: ~" (nessuna
  // altra fonte di date) passava indenne (la chiave presente bastava),
  // mentre semanticamente non ha ne' figli ne' date proprie. Diverso da
  // "subtasks: []" (array VUOTO esplicito, D-036: quello resta "sola
  // intestazione" per costruzione — un contenitore dichiarato vuoto e'
  // un nodo valido, un contenitore ASSENTE no).
  let ha-subtasks = "subtasks" in dato and dato.subtasks != none

  if ha-intervalli and (ha-start or ha-end or ("done" in dato and dato.done != none)) {
    // I-5: nessuna prosa inglese cablata — entrambi i segnaposto sono
    // NOMI DI CHIAVE dello schema (non si traducono, RF7), non una
    // frase composta nel core.
    fail(
      "E-VAL-12", percorso, locale,
      argomenti: (other-field: "intervals", other-detail: "start/end/done"),
    )
  }
  let ha-intervalli-con-dati = ha-intervalli and type(dato.intervals) == array and dato.intervals.len() > 0
  if not ha-intervalli-con-dati and not ha-start and not ha-end and not ha-subtasks {
    // DS-7-bis (ex I-5): "nessuna fonte di date" usa la voce composita
    // `E-VAL-01.no-dates` (frase fissa, nessun segnaposto): "subtasks:
    // []" esplicito conta comunque come "ha figli" (sola intestazione,
    // D-036), coerente col gruppo vuoto 0.1 (D3); "intervals: []" NON
    // conta come fonte di date (S4, bug trovato in revisione: prima
    // bastava la sola PRESENZA della chiave, anche vuota).
    fail("E-VAL-01", percorso, locale, modello: "E-VAL-01.no-dates")
  }

  let inizio = none
  let fine = none
  if ha-start or ha-end {
    if not ha-start {
      fail("E-VAL-01", percorso, locale, argomenti: (key: "start"))
    }
    if not ha-end {
      fail("E-VAL-01", percorso, locale, argomenti: (key: "end"))
    }
    inizio = dates.valida-data(dato.start, percorso + ".start", locale)
    fine = dates.valida-data(dato.end, percorso + ".end", locale)
    if fine < inizio {
      fail(
        "E-VAL-05", percorso, locale,
        argomenti: (field: "end", value: dates.formatta(fine), other-field: "start", other-value: dates.formatta(inizio)),
      )
    }
  }

  // Fase 3 (nota del revisore, bug reale): "done" si valida SEMPRE se
  // presente, anche su un nodo con SOLI `subtasks` — prima restava non
  // validato (un valore invalido passava indenne in silenzio), perche'
  // il calcolo avveniva solo dentro il ramo "ha-start or ha-end". Il
  // confronto con "inizio" si fa solo se "inizio" e' noto.
  let fatto = none
  if "done" in dato and dato.done != none {
    fatto = dates.valida-data(dato.done, percorso + ".done", locale)
    if inizio != none and fatto < inizio {
      fail(
        "E-VAL-05", percorso, locale,
        argomenti: (field: "done", value: dates.formatta(fatto), other-field: "start", other-value: dates.formatta(inizio)),
      )
    }
  }

  // Intervalli: validazione in profondita' + sovrapposizione (E-VAL-12,
  // con le fini EFFETTIVE secondo `modo`, D-036/RF3 "Gli intervalli sono
  // ordinati per start; due intervalli sovrapposti... -> E-VAL-12").
  let grezzi-intervalli = if ha-intervalli { dato.intervals } else { () }
  let intervalli-validati = grezzi-intervalli
    .enumerate()
    .map(((k, iv)) => _valida-intervallo(iv, percorso + ".intervals[" + str(k) + "]", locale, modo, strict, k))
  let ordinati = intervalli-validati.sorted(key: iv => iv.inizio)
  for i in range(0, calc.max(0, ordinati.len() - 1)) {
    let a = ordinati.at(i)
    let b = ordinati.at(i + 1)
    if a.at("fine-eff") > b.inizio {
      // S9 (seconda revisione Fase 3): modello DEDICATO
      // `E-VAL-12.overlap` (selezionato con `modello:`), non il
      // generico di sopra — il trattino fra le due date vive nel TESTO
      // del modello (data/locales/*.toml), non costruito qui con una
      // concatenazione di stringhe (principio
      // testo-utente-fonte-unica-nel-core).
      fail(
        "E-VAL-12", b.percorso, locale, modello: "E-VAL-12.overlap",
        argomenti: (
          other-field: "intervals[" + str(a.indice) + "]",
          other-start: a.pubblico.start, other-end: a.pubblico.end,
        ),
      )
    }
  }
  let intervalli = intervalli-validati.map(iv => iv.pubblico)

  // Sottoalberi (D-036, livelli successivi): E-VAL-13 si controlla alla
  // PROSSIMA chiamata (profondita + 1), non qui.
  let grezzi-subtasks = _array-o-vuoto(dato.at("subtasks", default: none))
  let sottoalberi = grezzi-subtasks
    .enumerate()
    .map(((j, s)) => _valida-nodo-ricorsivo(s, percorso + ".subtasks[" + str(j) + "]", locale, modo, strict, profondita + 1))

  let grezze-dipendenze = _array-o-vuoto(dato.at("dependencies", default: ()))
  let dipendenze = grezze-dipendenze
    .enumerate()
    .map(((k, d)) => _valida-dipendenza(d, percorso + ".dependencies[" + str(k) + "]", locale, strict))

  let progresso = _valida-progress(dato.at("progress", default: none), percorso + ".progress", locale)
  let colore = if "color" in dato { _valida-colore(dato.color, percorso + ".color", locale) } else { none }

  (
    kind: "node",
    path: percorso,
    // S8 (seconda revisione Fase 3): profondita' 0-based, UNIFORME fra
    // le due facce (il conteggio e' sempre "nodi dalla VERA radice",
    // gruppo incluso per la faccia 0.1 — non un'asimmetria, D-079):
    // un nodo radice di faccia 0.5 e' livello 0; un task diretto di un
    // gruppo 0.1 (il gruppo stesso e' livello 0) e' livello 1.
    level: profondita - 1,
    name: dato.name,
    start: if inizio == none { none } else { dates.formatta(inizio) },
    end: if fine == none { none } else { dates.formatta(fine) },
    done: if fatto == none { none } else { dates.formatta(fatto) },
    intervals: intervalli,
    subtasks: sottoalberi,
    id: _valida-id(dato, percorso, locale),
    dependencies: dipendenze,
    progress: progresso,
    color: colore,
    style: dato.at("style", default: (:)),
    "task-style": dato.at("task-style", default: (:)),
    x: dato.at("x", default: (:)),
  )
}

/// Valida un gruppo (faccia 0.1, livello 0; `chiavi-gruppo`): niente
/// date proprie (RF2, l'intervallo si calcola dai discendenti in
/// Fase 4), gruppo senza task ammesso (D3). I task diretti (`tasks[j]`)
/// sono nodi a tutti gli effetti (profondita 2, D-036): validati da
/// `_valida-nodo-ricorsivo`, che ricorre da solo sui loro `subtasks`.
/// `strict` e' quello RISOLTO da `validate()` (B-1).
#let _valida-gruppo(dato, indice, locale, modo, strict) = {
  let percorso = "taskgroups[" + str(indice) + "]"
  if type(dato) != dictionary {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato))
  }
  if "subtasks" in dato {
    // compat.typ (Fase 3): "subtasks" su un GRUPPO 0.1 e' la chiave
    // della faccia 0.5 "fraintesa" al posto di "tasks" — E-VAL-11
    // (D-036, non un generico E-VAL-02).
    compat.errore-facce-miste(percorso, "tasks", compat.schema-01, "subtasks", compat.schema-05, locale)
  }
  _verifica-chiavi(dato, schema.chiavi-gruppo, percorso, strict, locale)
  if "name" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "name"))
  }
  if type(dato.name) not in (str, content) {
    fail("E-VAL-03", percorso + ".name", locale, argomenti: (expected: "string|content"), received: nome-tipo(dato.name))
  }
  let task-grezzi = _array-o-vuoto(dato.at("tasks", default: ()))
  if type(task-grezzi) != array {
    fail("E-VAL-03", percorso + ".tasks", locale, argomenti: (expected: "array"), received: nome-tipo(task-grezzi))
  }
  let compiti = task-grezzi
    .enumerate()
    .map(((j, t)) => _valida-nodo-ricorsivo(t, percorso + ".tasks[" + str(j) + "]", locale, modo, strict, 2))

  let colore = if "color" in dato { _valida-colore(dato.color, percorso + ".color", locale) } else { none }
  // Validato SEMPRE se presente (come nei nodi, nessuna "start" con cui
  // confrontare: e' solo un override, RF3).
  let fatto = none
  if "done" in dato and dato.done != none {
    fatto = dates.formatta(dates.valida-data(dato.done, percorso + ".done", locale))
  }
  // S-7: il gruppo ha anche "task-style"/"x" (come il task), non solo
  // "style" — stessa guardia leggera, coerenza fra livelli.
  for chiave in ("style", "task-style", "x") {
    if chiave in dato and type(dato.at(chiave)) != dictionary {
      fail("E-VAL-03", percorso + "." + chiave, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato.at(chiave)))
    }
  }

  (
    kind: "group",
    path: percorso,
    level: 0,
    name: dato.name,
    tasks: compiti,
    done: fatto,
    color: colore,
    style: dato.at("style", default: (:)),
    "task-style": dato.at("task-style", default: (:)),
    id: _valida-id(dato, percorso, locale),
    x: dato.at("x", default: (:)),
  )
}

/// Valida una milestone (`chiavi-milestone`, RF2). `strict` e' quello
/// RISOLTO da `validate()` (B-1).
#let _valida-milestone(dato, indice, locale, modo, strict) = {
  let percorso = "milestones[" + str(indice) + "]"
  if type(dato) != dictionary {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato))
  }
  _verifica-chiavi(dato, schema.chiavi-milestone, percorso, strict, locale)
  if "name" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "name"))
  }
  if type(dato.name) not in (str, content) {
    fail("E-VAL-03", percorso + ".name", locale, argomenti: (expected: "string|content"), received: nome-tipo(dato.name))
  }
  if "date" not in dato {
    fail("E-VAL-01", percorso, locale, argomenti: (key: "date"))
  }
  // S-7: la milestone non aveva NESSUN controllo di tipo su
  // "style"/"x" (a differenza di gruppo/task): stessa guardia leggera.
  for chiave in ("style", "x") {
    if chiave in dato and type(dato.at(chiave)) != dictionary {
      fail("E-VAL-03", percorso + "." + chiave, locale, argomenti: (expected: "dictionary"), received: nome-tipo(dato.at(chiave)))
    }
  }
  let data = dates.valida-data(dato.date, percorso + ".date", locale)
  let colore = if "color" in dato { _valida-colore(dato.color, percorso + ".color", locale) } else { none }
  // Rifinitura (chiusura Fase 2): "show-date" non aveva NESSUN controllo
  // di tipo (a differenza di "show-today" di primo livello) — una
  // chiave YAML nulla ("show-date:" senza valore) ricade sul default
  // `true` come "show-today"; qualunque altro tipo -> E-VAL-03.
  let mostra-data = dato.at("show-date", default: true)
  if mostra-data == none { mostra-data = true }
  if type(mostra-data) != bool {
    fail("E-VAL-03", percorso + ".show-date", locale, argomenti: (expected: "boolean"), received: nome-tipo(mostra-data))
  }
  (
    name: dato.name,
    date: dates.formatta(data),
    "show-date": mostra-data,
    color: colore,
    style: dato.at("style", default: (:)),
    x: dato.at("x", default: (:)),
  )
}

/// Viewport esplicito (RF3): `viewport` o l'alias di primo livello
/// `start`/`end` (0.5.x); insieme -> E-VAL-11. Un solo estremo e'
/// ammesso (l'altro resta automatico, calcolato in Fase 4). Entrambi gli
/// estremi dati -> E-VAL-09 se la fine effettiva non segue l'inizio.
/// `strict` e' quello RISOLTO da `validate()` (DS-1, bug trovato in
/// revisione: il dizionario `viewport` non passava da NESSUN controllo
/// di chiave, a differenza di gruppo/task/milestone).
#let _valida-viewport(unito, locale, modo, strict) = {
  let ha-viewport = "viewport" in unito and unito.viewport != none
  // Rifinitura (chiusura Fase 2): un alias `start`/`end` di primo
  // livello scritto come YAML nullo (`start:` senza valore) NON deve
  // contare come "presente" — altrimenti un `start: ~` da solo, senza
  // `end`, costruirebbe un `viewport` fittizio `(start: none, end:
  // none)` (nessun danno diretto, ma apre la porta a un E-VAL-11 spurio
  // se insieme a un `viewport:` esplicito scritto apposta, quando in
  // realta' l'utente non ha mai davvero fissato `start` in cima).
  let ha-start-top = "start" in unito and unito.start != none
  let ha-end-top = "end" in unito and unito.end != none
  if ha-viewport and (ha-start-top or ha-end-top) {
    // I-5: "explicit viewport" / "gantty 0.5.x top-level alias" NON sono
    // prosa cablata nel core: sono voci del catalogo locale
    // (`E-VAL-11.viewport-explicit`/`E-VAL-11.viewport-alias`),
    // tradotte in data/locales/*.toml come ogni altro testo di RF7.
    fail(
      "E-VAL-11", "viewport", locale,
      argomenti: (
        key-a: "viewport", schema-a: locale.errors.at("E-VAL-11.viewport-explicit"),
        key-b: if ha-start-top { "start" } else { "end" }, schema-b: locale.errors.at("E-VAL-11.viewport-alias"),
      ),
    )
  }
  let vp = if ha-viewport {
    unito.viewport
  } else if ha-start-top or ha-end-top {
    (start: unito.at("start", default: none), end: unito.at("end", default: none))
  } else {
    none
  }
  if vp == none { return none }
  if type(vp) != dictionary {
    fail("E-VAL-03", "viewport", locale, argomenti: (expected: "dictionary"), received: nome-tipo(vp))
  }
  // DS-1: il dizionario "viewport" (scritto dall'utente o costruito
  // dall'alias start/end di primo livello, che ha comunque solo le
  // chiavi ammesse) passa da "strict" come ogni altro dizionario dello
  // schema.
  _verifica-chiavi(vp, schema.chiavi-viewport, "viewport", strict, locale)
  let inizio = if "start" in vp and vp.start != none { dates.valida-data(vp.start, "viewport.start", locale) } else { none }
  let fine = if "end" in vp and vp.end != none { dates.valida-data(vp.end, "viewport.end", locale) } else { none }
  if inizio != none and fine != none {
    let fine-eff = dates.fine-effettiva(fine, modo)
    if fine-eff <= inizio {
      // MINORI (D-063, "E-VAL-09 mostra anche la data scritta"): oltre
      // alla fine EFFETTIVA (quella confrontata), il quarto segnaposto
      // mostra la data scritta dall'utente, utile quando i due
      // differiscono (date-mode: "inclusive").
      fail(
        "E-VAL-09", "viewport", locale,
        argomenti: (
          end: dates.formatta(fine-eff), start: dates.formatta(inizio), mode: modo,
          "end-written": dates.formatta(fine),
        ),
      )
    }
  }
  (
    start: if inizio == none { none } else { dates.formatta(inizio) },
    end: if fine == none { none } else { dates.formatta(fine) },
  )
}

/// Stringa "ordine atteso" per il messaggio di E-HDR-02 (D-029): i nomi
/// nello stesso ordine scritto, ordinati per rank decrescente (dal piu'
/// grosso al piu' fine). Spostata qui da `timeline.typ` (C1, rilievo del
/// coordinatore sulla Fase 4): E-HDR-01/02/04 sono validazioni di
/// `headers` contro il REGISTRY effettivo (nomi, rank, parole chiave di
/// `format`), note non appena il registry e' costruito — non serve
/// aspettare il calcolo dei segmenti (quello resta a `timeline.typ`,
/// E-HDR-06 soltanto, un controllo A RUNTIME su `floor`/`next`).
#let _livelli-attesi-string(livelli) = (
  livelli.sorted(key: l => -l.rank).map(l => l.nome).join(", ")
)

/// Valida l'array `headers` (RF4) contro il `registro` effettivo (gia'
/// costruito da `units.registro(..)`, con le chiusure vere): per ogni
/// livello, forma (stringa o dizionario con `unit`/`format`), unita'
/// nota (E-HDR-01), parola chiave/funzione di `format` ammessa per
/// quell'unita' (E-HDR-04, via `units.risolvi-formato`, risultato
/// scartato: qui serve solo l'EFFETTO di validazione, non la funzione
/// risolta — `timeline.typ` la ricalcola quando costruisce i segmenti,
/// ricalcolo puro ed economico, non un doppio controllo di errore);
/// infine l'ordine per `rank` strettamente decrescente (E-HDR-02). Non
/// ritorna nulla di utile al chiamante: e' chiamata solo per l'effetto
/// di `fail()` su un `headers` non valido (`validate()` COMPLETO, C1).
///
/// - D1 (rilievo del coordinatore): `headers[i].unit` non stringa ->
///   E-VAL-03 con percorso ".unit", controllato PRIMA di `nome not in
///   registro` (T27: "cannot apply 'in' to <tipo non stringa> and
///   dictionary" sarebbe un panic "nudo"). Una chiave "unit" assente O
///   esplicitamente `none` (YAML "unit: ~") conta come MANCANTE
///   (E-VAL-01), stessa convenzione di `k in d and d.k != none` usata
///   altrove in questo modulo (T29).
/// Mappa delle tre stringhe ammesse per `headers[i].align` (review
/// finale di Fase 6a): stessi NOMI di `style.sidebar.align`/`left`/
/// `center`/`right` nativi di Typst, qui come scorciatoia da YAML
/// (dove un `alignment` vero non si scrive). Nessuna componente
/// verticale: un livello d'intestazione la fissa sempre a `horizon` a
/// parte (vedi `style.livello-header-risolto`), coerente con RF4 (solo
/// `day` usa un 1D, `center`).
#let _align-da-stringa = (left: left, center: center, right: right)

/// Valida l'array `headers` (RF4) contro il `registro` effettivo (gia'
/// costruito da `units.registro(..)`, con le chiusure vere): per ogni
/// livello, forma (stringa o dizionario con `unit`/`format`), unita'
/// nota (E-HDR-01), parola chiave/funzione di `format` ammessa per
/// quell'unita' (E-HDR-04, via `units.risolvi-formato`, risultato
/// scartato: qui serve solo l'EFFETTO di validazione, non la funzione
/// risolta — `timeline.typ` la ricalcola quando costruisce i segmenti,
/// ricalcolo puro ed economico, non un doppio controllo di errore);
/// infine l'ordine per `rank` strettamente decrescente (E-HDR-02).
/// **Restituisce l'array NORMALIZZATO** (review finale di Fase 6a,
/// rilievo del coordinatore — prima non restituiva nulla di utile,
/// "solo per l'effetto di `fail()`"): `headers[i].align` ammette ora
/// anche una stringa `"left"|"center"|"right"` (oltre a un `alignment`
/// Typst) e `headers[i].fill`/`.stroke`/`.gridline` ammettono anche una
/// stringa della tavola colori (oltre ai tipi Typst già ammessi, che
/// restano OPACHI — nessun altro controllo): la stringa viene risolta
/// QUI nel valore Typst vero (`alignment`/`color`) cosi' che `spec.
/// headers` porti sempre un valore GIA' pronto per `timeline.calcola`/
/// `style.livello-header-risolto`, mai piu' una stringa grezza. Il
/// chiamante (`_validate-interno`) deve usare QUESTO valore di ritorno
/// al posto dell'array grezzo in `spec.headers`.
///
/// - D1 (rilievo del coordinatore): `headers[i].unit` non stringa ->
///   E-VAL-03 con percorso ".unit", controllato PRIMA di `nome not in
///   registro` (T27: "cannot apply 'in' to <tipo non stringa> and
///   dictionary" sarebbe un panic "nudo"). Una chiave "unit" assente O
///   esplicitamente `none` (YAML "unit: ~") conta come MANCANTE
///   (E-VAL-01), stessa convenzione di `k in d and d.k != none` usata
///   altrove in questo modulo (T29).
#let _valida-headers(headers-grezzi, registro, locale, strict) = {
  // T25 (insidia nota di questo progetto): una CHIUSURA (`.map()`) non
  // puo' mutare una variabile catturata dall'esterno — niente `push` su
  // un array esterno dentro il `.map()` sotto. Ogni elemento porta GIA'
  // sia cio' che serve al controllo d'ordine (`nome`/`rank`) sia il
  // dizionario NORMALIZZATO (`normalizzato`): i due array derivati
  // (`livelli` per l'ordine, il valore di ritorno per `spec.headers`)
  // si ottengono con un `.map()` separato a valle, non mutando nulla.
  let elementi = headers-grezzi.enumerate().map(((i, h)) => {
    let percorso = "headers[" + str(i) + "]"
    let (nome, formato-utente, normalizzato) = if type(h) == str {
      (h, none, h)
    } else if type(h) == dictionary {
      if "unit" not in h or h.at("unit") == none {
        fail("E-VAL-01", percorso, locale, argomenti: (key: "unit"))
      }
      if type(h.unit) != str {
        fail(
          "E-VAL-03", percorso + ".unit", locale,
          argomenti: (expected: "string"), received: nome-tipo(h.unit),
        )
      }
      let fmt = h.at("format", default: none)
      if fmt != none and type(fmt) != str and type(fmt) != function {
        fail(
          "E-VAL-03", percorso + ".format", locale,
          argomenti: (expected: "string|function"), received: nome-tipo(fmt),
        )
      }
      // D-106 punto (a): intestazioni a dizionario per livello — chiavi
      // ammesse (E-VAL-02, `strict`) e tipo delle chiavi "strutturali"
      // (quelle che il core deve poter leggere senza ambiguità:
      // `align`/`text`/`overflow`/`height`/`min-cell-width`/`inset`).
      // `fill`/`stroke`/`gridline` restano OPACHI per qualunque tipo
      // Typst (come il resto di `style:`, mai validato in profondità
      // altrove in questo modulo): SOLO una stringa viene controllata
      // (e risolta), review finale di Fase 6a sotto.
      _verifica-chiavi(h, schema.chiavi-livello-header, percorso, strict, locale)
      let align-h = h.at("align", default: none)
      let align-normalizzato = if align-h == none {
        none
      } else if type(align-h) == alignment {
        align-h
      } else if type(align-h) == str {
        if align-h in _align-da-stringa {
          _align-da-stringa.at(align-h)
        } else {
          fail(
            "E-VAL-08", percorso + ".align", locale, received: repr(align-h),
            allowed: _align-da-stringa.keys().sorted(),
          )
        }
      } else {
        fail(
          "E-VAL-03", percorso + ".align", locale,
          argomenti: (expected: "alignment|string"), received: nome-tipo(align-h),
        )
      }
      let text-h = h.at("text", default: none)
      if text-h != none and type(text-h) != dictionary {
        fail("E-VAL-03", percorso + ".text", locale, argomenti: (expected: "dictionary"), received: nome-tipo(text-h))
      }
      let overflow-h = h.at("overflow", default: none)
      if overflow-h != none and overflow-h not in ("hide", "clip", "show") {
        fail("E-VAL-08", percorso + ".overflow", locale, received: repr(overflow-h), allowed: ("clip", "hide", "show"))
      }
      // Conversione di stringhe YAML (review finale di Fase 9, D-140):
      // `height`/`min-cell-width`/`inset` accettano ORA anche una
      // stringa nella sintassi dei letterali Typst (`"8pt"`, via
      // `style.prova-lunghezza`, "tentativo" che NON solleva — un
      // tentativo fallito lascia il valore COM'ERA, cosi' la guardia di
      // tipo sotto (invariata) resta l'UNICA fonte dell'errore finale,
      // con lo stesso messaggio di prima per chi scrive un tipo
      // sbagliato che NON è nemmeno una stringa valida). `height`
      // accetta anche la stringa letterale `"auto"` (D-140: "auto e
      // none accettati solo dove il campo li ammette", esempio proprio
      // `headers[i].height`), convertita nella parola chiave `auto`.
      let height-h = h.at("height", default: none)
      if type(height-h) == str and height-h.trim() == "auto" {
        height-h = auto
      } else if type(height-h) == str {
        let l = style.prova-lunghezza(height-h)
        if l != none { height-h = l }
      }
      if height-h != none and height-h != auto and type(height-h) != length {
        // `repr` (rereview finale di Fase 9, code-reviewer, suggerimento):
        // non `nome-tipo` — mostra il VALORE ricevuto (es. `"8 pt"`), non
        // solo il nome del tipo (`"string"`, poco utile da solo: lo stesso
        // tipo per chiavi diverse), coerente con `style.typ` (sempre
        // `repr(valore)` nei suoi `fail()`).
        fail("E-VAL-03", percorso + ".height", locale, argomenti: (expected: "auto|length"), received: repr(height-h))
      }
      let mcw-h = h.at("min-cell-width", default: none)
      if type(mcw-h) == str {
        let l = style.prova-lunghezza(mcw-h)
        if l != none { mcw-h = l }
      }
      if mcw-h != none and type(mcw-h) != length {
        fail("E-VAL-03", percorso + ".min-cell-width", locale, argomenti: (expected: "length"), received: repr(mcw-h))
      }
      let inset-h = h.at("inset", default: none)
      if type(inset-h) == str {
        let l = style.prova-lunghezza(inset-h)
        if l != none { inset-h = l }
      }
      if inset-h != none and type(inset-h) != length {
        fail("E-VAL-03", percorso + ".inset", locale, argomenti: (expected: "length"), received: repr(inset-h))
      }
      // Normalizzazione (review finale di Fase 6a): SOLO le chiavi
      // presenti E di tipo stringa vengono sostituite col valore Typst
      // risolto — `h + (chiave: ..)` lascia INVARIATO tutto il resto
      // (compresa l'assenza della chiave, se non era presente: `+` su
      // un dizionario con una chiave gia' assente la inserisce, ma qui
      // si entra nell'`if` solo se la chiave C'ERA GIA').
      let h-norm = h
      if align-h != none { h-norm = h-norm + (align: align-normalizzato) }
      if "text" in h-norm and h-norm.text != none {
        h-norm = h-norm + (text: style.converti-testo(h-norm.text, percorso + ".text", locale))
      }
      if height-h != h.at("height", default: none) { h-norm = h-norm + (height: height-h) }
      if mcw-h != h.at("min-cell-width", default: none) { h-norm = h-norm + ("min-cell-width": mcw-h) }
      if inset-h != h.at("inset", default: none) { h-norm = h-norm + (inset: inset-h) }
      // `"none"` (rereview finale di Fase 9, code-reviewer, punto 2):
      // questi tre campi ammettono `none` (nessun colore/nessuna
      // griglia) — la stringa YAML "none" (non una parola riservata in
      // YAML) deve diventare la parola chiave `none`, non fallire con
      // E-VAL-06 come farebbe `colore-da-stringa` (che non la
      // riconosce, giustamente: "none" non è nella tavola colori).
      if "fill" in h-norm and type(h-norm.fill) == str and h-norm.fill.trim() == "none" {
        h-norm = h-norm + (fill: none)
      } else if "fill" in h-norm and type(h-norm.fill) == str {
        h-norm = h-norm + (fill: style.colore-da-stringa(h-norm.fill, percorso + ".fill", locale))
      }
      if "stroke" in h-norm and type(h-norm.stroke) == str and h-norm.stroke.trim() == "none" {
        h-norm = h-norm + (stroke: none)
      } else if "stroke" in h-norm and type(h-norm.stroke) == str {
        h-norm = h-norm + (stroke: style.colore-da-stringa(h-norm.stroke, percorso + ".stroke", locale))
      }
      if "gridline" in h-norm and type(h-norm.gridline) == str and h-norm.gridline.trim() == "none" {
        h-norm = h-norm + (gridline: none)
      } else if "gridline" in h-norm and type(h-norm.gridline) == str {
        h-norm = h-norm + (gridline: style.colore-da-stringa(h-norm.gridline, percorso + ".gridline", locale))
      }
      (h.unit, fmt, h-norm)
    } else {
      fail("E-VAL-03", percorso, locale, argomenti: (expected: "string|dictionary"), received: nome-tipo(h))
    }
    // D-150 (decisione dell'utente, D-151, 2026-10-26): scorciatoia
    // "week-iso" -> `(unit: "week", format: "iso")`, risolta QUI,
    // PRIMA di E-HDR-01/02 (come da specifica, RF4) -- cosi' tutto cio'
    // che segue (E-HDR-01/02/04 sotto, `units.risolvi-formato`,
    // `timeline.typ`, il render) vede SOLO "week" col formato "iso",
    // MAI la stringa "week-iso": non e' un'unita' del registry (RF1,
    // resta di 5 unita'), e' un alias risolto solo qui. Accettata sia
    // come stringa NUDA (`normalizzato` era gia' la stringa "week-iso",
    // va PROMOSSA a dizionario: "week-iso" da sola non e' un'unita'
    // valida, "week" da sola perderebbe il formato) sia come `unit:` di
    // un dizionario (`normalizzato` e' gia' `h-norm`, con le altre
    // chiavi — `align`/`fill`/... — gia' normalizzate: qui si
    // sovrascrivono SOLO `unit`/`format`).
    //
    // Un `format` esplicito insieme a `unit: "week-iso"` (regola di
    // dettaglio lasciata a `typst-dev` dalla specifica, D-150): se
    // diverso da "iso" CONTRADDICE l'alias (l'utente ha scritto due
    // cose incompatibili: l'alias impone "iso", un altro `format`
    // chiederebbe un'altra resa) -> **E-HDR-04** ("format not allowed
    // for 'week-iso'", RIUSATO as-is: stesso identico testo/token di
    // quando un `format` non e' nella tavola di un'unita', coerente con
    // cosa succederebbe scrivendo un formato sconosciuto per "week" —
    // zero testo `it`/`en` nuovo, scelta deliberata). `format: "iso"`
    // esplicito è ridondante ma ammesso (nessun errore): l'utente ha
    // solo ripetuto cio' che l'alias intende gia'. Ipotesi scartata:
    // "l'ultimo scritto vince" (il `format` dell'utente silenziosamente
    // sovrascrive l'alias) — meno prevedibile, nasconderebbe un refuso
    // (es. `format: "start-day"` per abitudine da gantty) dietro un
    // comportamento muto.
    if nome == "week-iso" {
      if formato-utente != none and formato-utente != "iso" {
        fail(
          "E-HDR-04", percorso + ".format", locale,
          argomenti: (unit: "week-iso"),
          received: repr(formato-utente), allowed: ("iso",),
        )
      }
      normalizzato = if type(normalizzato) == dictionary {
        normalizzato + (unit: "week", format: "iso")
      } else {
        (unit: "week", format: "iso")
      }
      nome = "week"
      formato-utente = "iso"
    }
    if nome not in registro {
      // "week-iso" elencata fra gli ammessi anche se NON e' una chiave
      // del registry (D-150/D-151): a questo punto del codice `nome` e'
      // GIA' "week" se l'input era valido (ramo sopra), quindi non
      // finisce mai qui per un "week-iso" corretto — ma un utente che
      // scrive "weeks-iso" o simile deve comunque VEDERE "week-iso" fra
      // i nomi ammessi, non solo le 5 unita' del registry.
      fail(
        "E-HDR-01", percorso + ".unit", locale, received: repr(nome),
        allowed: (registro.keys() + ("week-iso",)).dedup().sorted(),
      )
    }
    let def = registro.at(nome)
    // E-HDR-04 (parola chiave/funzione di `format` non ammessa): la
    // funzione risolta non serve qui, solo l'eventuale `fail()`.
    // T24 (istruzione "nuda" con valore non `none`, qui una `function`):
    // senza `let _ = ..` il valore si unirebbe implicitamente al
    // dizionario `(nome:, rank:, normalizzato:)` sotto ("cannot join
    // function with dictionary", bug trovato con una sonda diretta).
    let _ = units.risolvi-formato(nome, def, formato-utente, percorso + ".format", locale)
    (nome: nome, rank: def.rank, normalizzato: normalizzato)
  })
  for i in range(0, calc.max(0, elementi.len() - 1)) {
    let a = elementi.at(i)
    let b = elementi.at(i + 1)
    if a.rank <= b.rank {
      fail(
        "E-HDR-02", "headers", locale,
        argomenti: (order: a.nome + ", " + b.nome, expected: _livelli-attesi-string(elementi)),
      )
    }
  }
  elementi.map(e => e.normalizzato)
}

/// Validatore COMPLETO (C1, rilievo del coordinatore sulla Fase 4): fa
/// TUTTO cio' che faceva `validate()` PIU' la costruzione e validazione
/// del registry delle unita' di intestazione (`units.registro`,
/// E-HDR-05) e la validazione di `headers` contro quel registry
/// (E-HDR-01/02/04, sopra). Restituisce `(spec: dictionary, registro:
/// dictionary, locale: dictionary)`: `spec` e' lo stesso spec
/// normalizzato che `validate()` espone pubblicamente (header-units GIA'
/// ridotte a `(name, rank)`, D-029); `registro` e' il registry EFFETTIVO
/// con le chiusure vere (`floor`/`next`/`format`), per chi deve davvero
/// disegnare (oggi solo `gantt()`/`timeline.calcola`, mai nei test
/// unitari attraverso `validate()`); `locale` e' il locale GIA' risolto
/// (RF6, con `errors` completo e `first-weekday`).
///
/// Nome con underscore ma IMPORTABILE da altri moduli (Typst non
/// impone privacy di modulo, D-088 lo conferma: e' solo una convenzione
/// di naming): il SOLO chiamante sanzionato fuori da questo file e'
/// `lib.typ`/`gantt()` (C1: "nessun registry/resolve doppio, nessuna
/// doppia lettura degli argomenti grezzi con precedenza duplicata" — un
/// documento Typst normale continua a usare `validate()`, pubblica).
///
/// - spec (dictionary): vedi `validate()`.
/// - risolvi-oggi-automatico (bool): se vero, `today: none` con
///   `show-today: true` chiama `datetime.today()` (DC-15 opzione A);
///   default falso (core deterministico, RNF4/T20) — SOLO `gantt()`
///   passa `true`.
/// - ..opzioni (arguments): vedi `validate()`.
/// -> dictionary (spec, registro, locale)
#let _validate-interno(spec, risolvi-oggi-automatico: false, ..opzioni) = {
  let nominati = opzioni.named()

  if "drawer" in nominati {
    fail("E-API-02", "drawer", locale.locales.en)
  }
  // I-9: due condizioni distinte di E-API-01, con percorso e "received"
  // propri (prima erano un unico ramo con un "argomenti" non dichiarato
  // nel catalogo, che e' vuoto per questo codice).
  if opzioni.pos().len() > 0 {
    fail("E-API-01", "args[1]", locale.locales.en, received: nome-tipo(opzioni.pos().at(0)))
  }
  if type(spec) != dictionary {
    fail("E-API-01", "spec", locale.locales.en, received: nome-tipo(spec))
  }

  // Rifinitura (chiusura Fase 2): "strict" si risolve PRIMA del locale
  // (come le altre opzioni di API, E-API-*): serve a `locale.resolve`
  // per il controllo sulle chiavi di `errors` (decisione 2026-10-15). Il
  // suo stesso errore di tipo resta quindi sempre in "en" (nessun test
  // dipende dalla lingua di questo messaggio).
  let strict = nominati.at("strict", default: spec.at("strict", default: true))
  if type(strict) != bool {
    fail("E-VAL-03", "strict", locale.locales.en, argomenti: (expected: "boolean"), received: nome-tipo(strict))
  }

  let locale-valore = nominati.at("locale", default: spec.at("locale", default: "en"))
  let loc = locale.resolve(locale-valore, strict: strict)

  // Chiavi sconosciute (D-018/D-029): sia sullo "spec" sia sugli
  // argomenti nominati (un nome sbagliato in entrambi i casi e' lo
  // stesso errore per l'utente).
  _verifica-chiavi(spec, schema.chiavi-primo-livello, "", strict, loc)
  _verifica-chiavi(nominati, schema.chiavi-primo-livello, "", strict, loc)

  let unito = spec
  for (chiave, valore) in nominati {
    if chiave == "style" {
      unito.insert("style", _unisci-profondo(unito.at("style", default: (:)), valore))
    } else {
      unito.insert(chiave, valore)
    }
  }

  let modo = unito.at("date-mode", default: "inclusive")
  if modo not in ("inclusive", "instant") {
    fail("E-VAL-08", "date-mode", loc, received: repr(modo), allowed: ("inclusive", "instant"))
  }

  let snap = unito.at("viewport-snap", default: "day")
  if snap not in ("day", "week", "month", "year") {
    fail("E-VAL-08", "viewport-snap", loc, received: repr(snap), allowed: ("day", "week", "month", "year"))
  }

  let mostra-oggi = unito.at("show-today", default: false)
  if type(mostra-oggi) != bool {
    fail("E-VAL-03", "show-today", loc, argomenti: (expected: "boolean"), received: nome-tipo(mostra-oggi))
  }

  let oggi = none
  if "today" in unito and unito.today != none {
    oggi = dates.formatta(dates.valida-data(unito.today, "today", loc))
  } else if risolvi-oggi-automatico and mostra-oggi {
    // DC-15 (opzione A, decisione dell'utente non contraddetta, terza
    // revisione Fase 4): "oggi" NON e' mai automatico nel CORE per
    // default (`risolvi-oggi-automatico: false`, usato da `validate()`
    // pubblica e da ogni test `unit/`: resta deterministico, RNF4/T20 —
    // nessuna chiamata a `datetime.today()` senza che il chiamante lo
    // chieda esplicitamente). Solo `gantt()` (`lib.typ`, il livello che
    // produce davvero il disegno, mai interrogato con `typst.query`)
    // passa `risolvi-oggi-automatico: true`: con `today: none` e
    // `show-today: true` (RF2: "'none' = `datetime.today()`"),
    // sostituisce la data vera PRIMA di procedere — non e' piu' un
    // comportamento "mai automatico", solo spostato fuori dal core.
    oggi = dates.formatta(datetime.today())
  }

  if "headers" in unito and unito.headers != none and type(unito.headers) != array {
    fail("E-VAL-03", "headers", loc, argomenti: (expected: "array"), received: nome-tipo(unito.headers))
  }

  // Rifinitura (chiusura Fase 2): "style" di primo livello non aveva
  // NESSUN controllo di tipo (a differenza di "headers" qui sopra); un
  // valore YAML nullo esplicito ("style:" senza valore) ricade sul
  // default `(:)`, come "headers:" (S-7).
  let stile-grezzo = unito.at("style", default: (:))
  let stile = if stile-grezzo == none { (:) } else { stile-grezzo }
  if type(stile) != dictionary {
    fail("E-VAL-03", "style", loc, argomenti: (expected: "dictionary"), received: nome-tipo(stile))
  }

  // Rifinitura: "sidebar-width" non aveva NESSUN controllo di tipo. Un
  // valore YAML nullo esplicito ricade sul default `auto` (stessa
  // convenzione di "headers"/"style" sopra, S-7).
  let larghezza-sidebar-grezza = unito.at("sidebar-width", default: auto)
  let larghezza-sidebar = if larghezza-sidebar-grezza == none { auto } else { larghezza-sidebar-grezza }
  // Conversione di stringhe YAML (rereview finale di Fase 9, code-
  // reviewer, suggerimento: "e' una chiave top-level da YAML evidente:
  // l'utente scrive sidebar-width: 120pt"): "120pt" -> lunghezza ("auto"
  // letterale gia' gestito sopra come CHIAVE ASSENTE/`none`, ma da YAML
  // "auto" senza virgolette arriva come la STRINGA "auto", non come
  // l'assenza della chiave: va riconosciuta anche qui). `.trim()` (2a
  // rereview): spazi iniziali/finali tollerati, come per lunghezza/
  // ratio/colore altrove (D-140).
  if type(larghezza-sidebar) == str and larghezza-sidebar.trim() == "auto" {
    larghezza-sidebar = auto
  } else if type(larghezza-sidebar) == str {
    let l = style.prova-lunghezza(larghezza-sidebar)
    if l != none { larghezza-sidebar = l }
  }
  if larghezza-sidebar != auto and type(larghezza-sidebar) != length {
    fail("E-VAL-03", "sidebar-width", loc, argomenti: (expected: "auto|length"), received: repr(larghezza-sidebar))
  }

  let viewport = _valida-viewport(unito, loc, modo, strict)

  // compat.typ (Fase 3, D-055): riconoscimento della faccia dal top
  // level. Una chiave YAML nulla ("tasks:"/"taskgroups:" senza valore)
  // non conta come "presente" (stesso criterio dell'alias viewport,
  // sopra): evita un E-VAL-11 spurio su un file che ha scritto la chiave
  // sbagliata per abitudine ma vuota.
  let ha-taskgroups = "taskgroups" in unito and unito.taskgroups != none
  let ha-tasks = "tasks" in unito and unito.tasks != none
  if ha-taskgroups and ha-tasks {
    compat.errore-facce-miste("tasks", "taskgroups", compat.schema-01, "tasks", compat.schema-05, loc)
  }
  let faccia-05 = ha-tasks

  let radici = if faccia-05 {
    let nodi-grezzi = unito.tasks
    if type(nodi-grezzi) != array {
      fail("E-VAL-03", "tasks", loc, argomenti: (expected: "array"), received: nome-tipo(nodi-grezzi))
    }
    nodi-grezzi.enumerate().map(((i, t)) => _valida-nodo-ricorsivo(t, "tasks[" + str(i) + "]", loc, modo, strict, 1))
  } else {
    let gruppi-grezzi = _array-o-vuoto(unito.at("taskgroups", default: ()))
    if type(gruppi-grezzi) != array {
      fail("E-VAL-03", "taskgroups", loc, argomenti: (expected: "array"), received: nome-tipo(gruppi-grezzi))
    }
    gruppi-grezzi.enumerate().map(((i, g)) => _valida-gruppo(g, i, loc, modo, strict))
  }

  let pietre-grezze = _array-o-vuoto(unito.at("milestones", default: ()))
  if type(pietre-grezze) != array {
    fail("E-VAL-03", "milestones", loc, argomenti: (expected: "array"), received: nome-tipo(pietre-grezze))
  }
  let pietre = pietre-grezze.enumerate().map(((i, m)) => _valida-milestone(m, i, loc, modo, strict))

  if radici.len() == 0 and pietre.len() == 0 {
    // D-054: percorso "taskgroups" per la faccia 0.1 (anche quando
    // nessuna delle due chiavi e' scritta: default storico), "tasks"
    // per la faccia 0.5 (Fase 3, RF7).
    fail("E-VAL-10", if faccia-05 { "tasks" } else { "taskgroups" }, loc)
  }

  // graph.typ (Fase 3, RF10): indice id (E-DEP-01), risoluzione delle
  // dipendenze (E-DEP-02/03/05) e cicli (E-DEP-04) sull'INTERO bosco
  // (gruppi+task, o nodi radice): un errore qui e' sempre PIU'
  // specifico di uno strutturale gia' superato sopra.
  let ordine-dipendenze = graph.valida(radici, loc)

  // S-7 (suggerimento del coordinatore, terza revisione Fase 4): una
  // chiave YAML nulla esplicita ("header-units: ~"/"drawers: ~") conta
  // come "non specificato", non come `none` letterale da rifiutare —
  // stessa convenzione gia' usata per `headers`/`style`/`sidebar-width`
  // sopra (".at(default:)" si applica solo se la chiave e' ASSENTE, non
  // se e' presente con valore `none`, T29). Prima: `header-units: ~`
  // dava E-VAL-03 invece di ricadere sul default `(:)`.
  let unita-header-grezzo = unito.at("header-units", default: (:))
  let unita-header = if unita-header-grezzo == none { (:) } else { unita-header-grezzo }
  if type(unita-header) != dictionary {
    fail("E-VAL-03", "header-units", loc, argomenti: (expected: "dictionary"), received: nome-tipo(unita-header))
  }
  let drawer-utente-grezzo = unito.at("drawers", default: (:))
  let drawer-utente = if drawer-utente-grezzo == none { (:) } else { drawer-utente-grezzo }
  if type(drawer-utente) != dictionary {
    fail("E-VAL-03", "drawers", loc, argomenti: (expected: "dictionary"), received: nome-tipo(drawer-utente))
  }
  // RF11/Fase 6b (D-040): chiave sconosciuta o valore ne' `function` ne'
  // `none` -> E-DRW-01, SEMPRE (non solo con `strict`, a differenza di
  // `_verifica-chiavi`/E-VAL-02: un drawer ignorato in silenzio e' un
  // difetto peggiore di una chiave ignorata, l'utente crede di aver
  // personalizzato un elemento che resta quello di default). "si
  // verifica solo il tipo `function`, mai la firma" (RF11): nessun
  // controllo sul numero di argomenti o sul valore di ritorno qui, solo
  // su E-DRW-02 a runtime (`src/render/drawers.typ`, Fase 6b).
  for (nome-drawer, valore-drawer) in drawer-utente {
    if nome-drawer not in schema.chiavi-drawer {
      fail(
        "E-DRW-01", "drawers." + nome-drawer, loc,
        received: repr(nome-drawer), allowed: schema.chiavi-drawer,
        suggest: schema.suggerisci(nome-drawer, schema.chiavi-drawer),
      )
    }
    // S6 (review di Fase 6b): chiave VALIDA ma valore di tipo errato ->
    // E-VAL-03 (generico "tipo sbagliato", stesso codice già usato per
    // ogni altro mismatch di tipo in questo modulo — es. `headers`/
    // `style.milestones.label.date-format`, S1), NON E-DRW-01 (che
    // dice "drawer sconosciuto": fuorviante quando la chiave è
    // ammessa, solo il VALORE è sbagliato — due difetti diversi,
    // messaggi diversi). Scelta più semplice del coordinatore: nessun
    // testo nuovo per il gate (niente voce composita `E-DRW-01.type`).
    if type(valore-drawer) != function and valore-drawer != none {
      fail(
        "E-VAL-03", "drawers." + nome-drawer, loc,
        argomenti: (expected: "function|none"), received: nome-tipo(valore-drawer),
      )
    }
  }

  // C1 (rilievo del coordinatore): registry EFFETTIVO costruito QUI,
  // un'unica volta (`gantt()` non lo ricostruisce piu': legge
  // `interno.registro`), con validazione delle definizioni custom
  // (E-HDR-05, `units.registro`/`_valida-definizione`).
  let reg = units.registro(unita-header, loc.at("first-weekday"), loc, strict: strict)

  // S-7: ".at(default:)" si applica solo se la chiave e' ASSENTE, non se
  // e' presente con valore `none` (YAML "headers:" senza valore): qui va
  // trattato come "non specificato". Default da `schema.typ`.
  let headers-valore = if "headers" in unito and unito.headers != none { unito.headers } else { schema.headers-default }
  // Review di Fase 8.4 (D-018 "mai un errore silenzioso"): `headers: ()`
  // ESPLICITO (array VUOTO, diverso dalla chiave ASSENTE/`none` SOPRA,
  // che cade sul default `schema.headers-default`, mai vuoto) passava
  // indenne fino al render — `_valida-headers(())` sotto non controlla
  // un array vuoto, `layout.intestazioni()` resterebbe `()`, e
  // `render/headers.typ` costruirebbe un `table.header(repeat: true)`
  // SENZA celle (altezza totale 0pt): un grafico senza asse del tempo
  // né griglia, nessun errore né avviso di Typst. Rifiutato QUI, prima
  // di `_valida-headers`, con lo stesso `E-VAL-03` generico usato per
  // altri valori "di tipo giusto ma di valore sbagliato" nel modulo.
  // **Rereview (D-018, decisione dell'utente del 2026-10-19):** il
  // rilievo iniziale del coordinatore — RF4 ammetterebbe `headers: ()`
  // — era un errore del coordinatore stesso, non del codice: l'utente
  // ha deciso di MANTENERE il rifiuto. Il testo di `{expected}` NON è
  // cablato qui (prosa fuori dai locali, RF7 "testo-utente-fonte-unica-
  // nel-core"): `E-VAL-03.non-empty` (`data/locales/{en,it}.toml`,
  // stesso schema di `E-VAL-03.horizontal`, `style.typ`).
  if headers-valore.len() == 0 {
    fail("E-VAL-03", "headers", loc, argomenti: (expected: loc.errors.at("E-VAL-03.non-empty")), received: repr(headers-valore))
  }
  // C1: E-HDR-01/02/04 (headers contro il registry) — `validate()` e'
  // ora COMPLETO su questo fronte, non solo `gantt()`. Review finale di
  // Fase 6a: il valore di RITORNO (non piu' scartato) porta `align`/
  // `fill`/`stroke`/`gridline` GIA' risolti quando l'utente li ha
  // scritti come stringa (YAML) — `spec.headers` deve usare QUESTO, non
  // l'array grezzo, altrimenti la stringa arriverebbe intatta fino al
  // render (dove `fill`/`stroke`/`align` richiedono un `color`/
  // `alignment` vero, non una stringa).
  let headers-normalizzati = _valida-headers(headers-valore, reg.registro, loc, strict)

  (
    spec: (
      taskgroups: radici,
      milestones: pietre,
      "show-today": mostra-oggi,
      today: oggi,
      "viewport-snap": snap,
      viewport: viewport,
      headers: headers-normalizzati,
      locale: locale-valore,
      style: stile,
      "date-mode": modo,
      "sidebar-width": larghezza-sidebar,
      strict: strict,
      // RF1 (C1, rilievo del coordinatore): ridotte a `(name, rank)`,
      // non piu' ai soli nomi (D-029: niente funzioni in uscita). Solo
      // le unita' CUSTOM dell'utente (quelle incluse si leggono gia' da
      // `lib.typ: header-units`, D-089/D-091 punto 1): filtrate dal
      // registry effettivo scartando i nomi riservati.
      "header-units": reg.at("nomi-rank").filter(c => c.name not in units.nomi-inclusi),
      drawers: drawer-utente.keys(),
      // RF10 (Fase 3): ordine topologico dei nodi con id, base del
      // percorso critico (v0.2); array vuoto SOLO se NESSUN nodo ha un
      // id (terza revisione, rilievo 8: la clausola precedente era FALSA
      // — con id presenti ma senza dipendenze che li collegano l'array
      // non e' vuoto, contiene comunque tutti gli id, in ordine di
      // visita/preordine del documento, D-082).
      "dependency-order": ordine-dipendenze,
    ),
    registro: reg.registro,
    locale: loc,
    // RF11/Fase 6b: dizionario CON LE FUNZIONI vere (nome -> funzione |
    // `none`), fuori da `.spec` apposta (D-029: `spec` non porta mai
    // funzioni, e' quello che restituisce anche `validate()` pubblico).
    // Sibling solo per `gantt()` (`lib.typ`), che unisce questi con
    // `default-drawers` (`src/render/drawers.typ`) PRIMA di disegnare —
    // `validate()` pubblico non espone questa chiave (ne restituisce
    // solo `.spec`, vedi sotto).
    "drawers-utente": drawer-utente,
  )
}

/// Valida e normalizza `spec` (RF1/RF2): restituisce lo spec
/// normalizzato, nella stessa forma generale dell'input (chiavi
/// kebab-case), senza funzioni (D-029: `header-units` ridotte a
/// `(name, rank)`, `drawers` ridotti ai soli nomi). Fa `panic` con un
/// errore del catalogo RF7 su qualunque input non valido (D6/D-030: mai
/// un panic "nudo" di Typst). Wrapper pubblico di `_validate-interno`
/// (C1): stesso identico comportamento osservabile, solo il risultato
/// e' ridotto al solo `spec` (senza il registry con le funzioni, che
/// D-029 vieta in uscita).
///
/// - spec (dictionary): input secondo lo schema di RF2, faccia 0.1
///   (`taskgroups[].tasks[]`) o faccia 0.5 (`tasks[].subtasks[]`,
///   D-025/D-055): riconosciute da `taskgroups`/`tasks` di primo
///   livello (`src/compat.typ`); mescolarle -> E-VAL-11.
/// - ..opzioni (arguments): argomenti nominati con lo stesso nome delle
///   chiavi di primo livello, piu' le opzioni di API `strict`,
///   `header-units`, `drawers` (RF1). Nessun argomento posizionale oltre
///   a `spec`. `drawer:` (API cetz di gantty 0.5.x) -> E-API-02.
/// -> dictionary
#let validate(spec, ..opzioni) = _validate-interno(spec, ..opzioni).spec
