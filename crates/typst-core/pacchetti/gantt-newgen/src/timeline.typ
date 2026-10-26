// gantt-newgen — src/timeline.typ
// Modello temporale (RF3): fini effettive secondo `date-mode` (D-050),
// viewport (automatico e parziale, con la guardia D-064), snap
// (`day`/`week`/`month`/`year`, settimana con `first-weekday`), segmenti
// per livello d'intestazione (RF4, via `units.typ`), aggregazioni
// ricorsive di intervallo/`done`/`progress` (D-039) e agganci delle
// dipendenze con i vincoli violati (RF10). Core puro: consuma l'albero
// GIA' validato da `validate.typ` (date ancora grezze) e produce un
// modello in istanti `datetime`; nessun disegno, nessun `measure` (RNF5).
//
// Ricorsione (T19/D-036): l'UNICA funzione ricorsiva di questo modulo e'
// `_raccogli` (sotto), che scende l'albero gia' validato una volta sola
// (bottom-up, come `graph._analizza`): la sua profondita' di chiamata e'
// quindi limitata a priori da E-VAL-13 (max 10 livelli, `validate.typ`),
// con lo stesso margine di `graph.typ` (~3 frame per livello, ben sotto
// il limite osservato di Typst, ~80 frame). Tutto il resto (viewport,
// snap, segmenti via `units.segmenti`, agganci delle dipendenze) e'
// iterativo (`for`/`map` su array, mai su ricorsione annidata).

#import "dates.typ"
#import "errors.typ": fail
#import "schema.typ": figli
#import "units.typ"

// --- Intervalli effettivi di un nodo (D-050) ---------------------------

/// Intervalli EFFETTIVI (istanti, non piu' date grezze) della fonte di
/// date PROPRIA di `nodo` (RF2: `start`/`end`/`done`, oppure
/// `intervals`; mai entrambe, gia' garantito da `validate.typ`/E-VAL-12):
/// array di `(inizio: datetime, "fine-eff": datetime, "done-eff":
/// datetime|none)`. Nodo senza fonte propria (container, D-039) -> `()`.
#let _intervalli-propri(nodo, modo, locale, percorso) = {
  // Un gruppo (faccia 0.1, livello 0) non ha MAI date proprie (RF2: "il
  // gruppo non ha chiavi 'start'/'end'/'intervals'", solo `tasks` e
  // l'override `done`): niente da leggere qui, accedere a `nodo.start`
  // su un dizionario senza quella chiave farebbe un panic "nudo" di
  // Typst (nessun default su un accesso diretto, a differenza di `.at(.,
  // default: ..)`). L'override `done` del gruppo si applica DOPO
  // l'aggregazione dai figli, in `_raccogli` (RF3/D-039), non qui.
  if nodo.at("kind", default: "node") == "group" {
    return ()
  }
  if nodo.intervals.len() > 0 {
    nodo.intervals.map(iv => (
      inizio: dates.valida-data(iv.start, percorso + ".start", locale),
      "fine-eff": dates.fine-effettiva(dates.valida-data(iv.end, percorso + ".end", locale), modo),
      "done-eff": if iv.done == none {
        none
      } else {
        dates.fine-effettiva(dates.valida-data(iv.done, percorso + ".done", locale), modo)
      },
    ))
  } else if nodo.start != none {
    (
      (
        inizio: dates.valida-data(nodo.start, percorso + ".start", locale),
        "fine-eff": dates.fine-effettiva(dates.valida-data(nodo.end, percorso + ".end", locale), modo),
        "done-eff": if nodo.done == none {
          none
        } else {
          dates.fine-effettiva(dates.valida-data(nodo.done, percorso + ".done", locale), modo)
        },
      ),
    )
  } else {
    ()
  }
}

/// `progress` PROPRIO di un nodo foglia (D-039): ignorato (-> `none`) se
/// ha un `done` di nodo, o se e' a intervalli e TUTTI gli intervalli
/// hanno `done` (altrimenti usato anche con `done` parziale, D-039 punto
/// 6). `nodo.progress` e' gia' 0-100 o `none` (validato da `validate.typ`).
#let _progress-proprio(nodo, intervalli) = {
  let tutti-gli-intervalli-fatti = (
    intervalli.len() > 0 and intervalli.all(iv => iv.at("done-eff") != none)
  )
  let ha-done-di-nodo = nodo.intervals.len() == 0 and nodo.done != none
  if ha-done-di-nodo or tutti-gli-intervalli-fatti { none } else { nodo.progress }
}

/// `done-eff` PROPRIO di un nodo foglia (D-039): per un nodo a
/// `start`/`end`, il suo `done` (gia' nell'unico intervallo di
/// `_intervalli-propri`); per un nodo a `intervals`, il nodo stesso non
/// ha un campo `done` (esclusivita' E-VAL-12) — qui si deriva: se TUTTI
/// gli intervalli hanno `done-eff`, il massimo; altrimenti `none` (stessa
/// regola D4 applicata a un solo nodo, non all'albero).
#let _done-proprio(nodo, intervalli) = {
  if intervalli.len() == 0 { return none }
  if nodo.intervals.len() == 0 {
    // start/end: l'unico intervallo sintetico porta il "done" vero.
    return intervalli.at(0).at("done-eff")
  }
  if intervalli.all(iv => iv.at("done-eff") != none) {
    let fatti = intervalli.map(iv => iv.at("done-eff"))
    let massimo = fatti.at(0)
    for f in fatti { if f > massimo { massimo = f } }
    return massimo
  }
  none
}

/// Durata (giorni interi, RF3) di una lista di intervalli effettivi:
/// somma di `(fine-eff - inizio)` per ciascuno (le pause fra intervalli
/// NON contano, D-039).
#let _durata(intervalli) = {
  let totale = 0
  for iv in intervalli { totale += dates.giorni-fra(iv.inizio, iv.at("fine-eff")) }
  totale
}

/// Minimo/massimo di un array non vuoto di `datetime` (guardia T5 a
/// carico del chiamante: mai su una lista vuota).
#let _min-data(valori) = {
  let m = valori.at(0)
  for v in valori { if v < m { m = v } }
  m
}
#let _max-data(valori) = {
  let m = valori.at(0)
  for v in valori { if v > m { m = v } }
  m
}

/// Visita bottom-up di UN nodo (gruppo o nodo, D-036; `figli()`
/// discrimina): calcola in un'UNICA passata (RNF3) `(ha-date, inizio,
/// fine, "fine-propria", durata, "done-eff", progress, "mostra-progress")`.
/// - `fine` = aggancio "fine visiva" (RF10: max fra `fine-eff` e
///   `done-eff`), usata per il viewport (D-010 punto 7: "done oltre end
///   estende il viewport") e per gli agganci delle dipendenze;
/// - `fine-propria` = fine EFFETTIVA (senza `done`), usata solo per
///   calcolare `durata` (RF3: "durata... somma delle durate dei suoi
///   intervalli", non di `done`);
/// - nodo CON fonte propria (D-039 punto 5): valori propri, i figli NON
///   contribuiscono all'aggregazione — ma vengono comunque VISITATI
///   (D-104, bug corretto: prima un nodo con date proprie e `subtasks`
///   datati non ricorreva su di loro affatto, quindi i sottotask non
///   finivano in `modello.nodi` e non venivano disegnati) per esporre
///   le loro voci piatte sotto;
/// - nodo SENZA fonte propria (container): aggregati dai figli con
///   `ha-date` vero (D3/D4): `done-eff` = `max` se e solo se TUTTI quei
///   figli hanno `done-eff` (altrimenti `none`, anche ricorsivamente: un
///   container con `done-eff: none` propaga "non tutti fatti" al proprio
///   genitore, D4); `progress` = media pesata su `durata` dei figli
///   (durata totale 0 -> media semplice, D-039 punto 2; foglia/figlio
///   senza `progress` ne' `done-eff` conta 0 nella media, senza
///   contribuire a "mostra-progress"); nessun figlio con date -> nodo
///   senza date (D3, gruppo vuoto).
/// `piatti` (RNF3, come `graph._analizza`): elenco `(id, path, inizio,
/// fine, dependencies)` di SE STESSO (solo se `ha-date`) piu' quelli di
/// TUTTI i discendenti, in un'UNICA passata — nessuna seconda visita
/// dell'albero per gli agganci delle dipendenze (`calcola()` sotto
/// costruisce `per-id`/i vincoli con `for` semplici su questo array, non
/// con una chiusura che muterebbe una variabile catturata: T25).
#let _raccogli(nodo, modo, locale, percorso) = {
  let intervalli = _intervalli-propri(nodo, modo, locale, percorso)
  let base = if intervalli.len() > 0 {
    let fine-propria = _max-data(intervalli.map(iv => iv.at("fine-eff")))
    let done-eff = _done-proprio(nodo, intervalli)
    let progress = _progress-proprio(nodo, intervalli)
    let fine = if done-eff != none and done-eff > fine-propria { done-eff } else { fine-propria }
    // BUG trovato dal coordinatore (D-102, corretto qui, D-104): un
    // nodo con date PROPRIE che ha ANCHE dei `subtasks` datati non
    // ricorreva MAI su di loro (prima "piatti-figli": () fisso) — i
    // sottotask non ricevevano un `ninfo` in `modello.nodi` e la loro
    // barra non si disegnava, SILENZIOSAMENTE (nessun errore). RF3
    // resta invariato ("nodo con figli e date proprie -> intervallo/
    // done/progress PROPRI", sopra: `inizio`/`fine`/`durata`/`done-eff`/
    // `progress` di QUESTO nodo vengono SOLO dai suoi `intervalli`, MAI
    // da `figli-raccolti-propri` sotto) — i figli vanno pero' comunque
    // VISITATI, per esporre le loro VOCI PIATTE (disegno/agganci delle
    // dipendenze, RNF3: nessuna seconda visita dell'albero), esattamente
    // come fa il ramo "container" sotto. Nessun rischio di profondita'
    // aggiuntiva (T19): e' la STESSA ricorsione di un livello, non una
    // visita in piu'.
    let figli-raccolti-propri = figli(nodo).map(f => _raccogli(f, modo, locale, f.path))
    (
      ha-date: true,
      inizio: _min-data(intervalli.map(iv => iv.inizio)),
      fine: fine,
      "fine-propria": fine-propria,
      durata: _durata(intervalli),
      "done-eff": done-eff,
      progress: progress,
      "mostra-progress": progress != none or done-eff != none,
      // S2 (rilievo del coordinatore): intervalli PROPRI, gia' in
      // istanti effettivi — Fase 5 (layout.typ, DC-9) ne ha bisogno per
      // collocare il tratto di `progress` DENTRO gli intervalli (RF3),
      // non sul singolo span `[inizio, fine]`. Vuoto per un container
      // (sotto): il suo "span" e' sintetico, non una lista di pause
      // reali — Fase 5 user `inizio`/`fine` direttamente in quel caso.
      intervalli: intervalli,
      "piatti-figli": figli-raccolti-propri.map(f => f.piatti).flatten(),
    )
  } else {
    let figli-raccolti = figli(nodo).map(f => _raccogli(f, modo, locale, f.path))
    let con-date = figli-raccolti.filter(f => f.ha-date)
    let piatti-figli = figli-raccolti.map(f => f.piatti).flatten()
    if con-date.len() == 0 {
      (
        ha-date: false, inizio: none, fine: none, "fine-propria": none,
        durata: 0, "done-eff": none, progress: none, "mostra-progress": false,
        intervalli: (), "piatti-figli": piatti-figli,
      )
    } else {
      let done-eff = if con-date.all(f => f.at("done-eff") != none) {
        _max-data(con-date.map(f => f.at("done-eff")))
      } else {
        none
      }
      let durata-totale = con-date.fold(0, (acc, f) => acc + f.durata)
      let mostra = con-date.any(f => f.at("mostra-progress"))
      let progress = if not mostra {
        none
      } else if durata-totale == 0 {
        // D-039 punto 2: guardia contro x/0 (T18) — media SEMPLICE.
        let valori = con-date.map(f => if f.progress != none { f.progress } else if f.at("done-eff") != none { 100.0 } else { 0.0 })
        valori.fold(0.0, (a, b) => a + b) / valori.len()
      } else {
        let somma = con-date.fold(0.0, (acc, f) => {
          let v = if f.progress != none { f.progress } else if f.at("done-eff") != none { 100.0 } else { 0.0 }
          acc + v * f.durata
        })
        somma / durata-totale
      }
      (
        ha-date: true,
        inizio: _min-data(con-date.map(f => f.inizio)),
        fine: _max-data(con-date.map(f => f.fine)),
        "fine-propria": _max-data(con-date.map(f => f.at("fine-propria"))),
        durata: durata-totale,
        "done-eff": done-eff,
        progress: progress,
        "mostra-progress": mostra,
        intervalli: (),
        "piatti-figli": piatti-figli,
      )
    }
  }
  // Override `done` di GRUPPO (RF2/RF3): un gruppo 0.1 non ha date
  // proprie (sopra, `_intervalli-propri` ritorna sempre `()` per
  // "kind: group"), quindi arriva sempre dal ramo container; il suo
  // `done` esplicito (se presente) SOVRASCRIVE il `done-eff` aggregato,
  // qualunque esso sia (anche se non tutti i discendenti sono fatti,
  // D-039: e' un override, non una condizione). Ignorato se il gruppo
  // non ha nessuna data aggregata (nessuna barra su cui applicarlo, D3).
  // Rilievo del coordinatore (D4): un `done` presente spegne `progress`
  // come per QUALUNQUE altro nodo con `done` (RF3: "con 'done' presente
  // 'progress' e' ignorato" — regola generale, non solo delle foglie);
  // `mostra-progress` resta vero (il `done` IMPLICA "mostrato" al 100%,
  // stessa convenzione di `_progress-proprio`/D-039 punto 6, DC-10).
  let base = if (
    base.ha-date and nodo.at("kind", default: "node") == "group" and nodo.at("done", default: none) != none
  ) {
    let done-override = dates.fine-effettiva(dates.valida-data(nodo.done, percorso + ".done", locale), modo)
    let fine-aggiornata = if done-override > base.fine { done-override } else { base.fine }
    (
      ha-date: true, inizio: base.inizio, fine: fine-aggiornata,
      "fine-propria": base.at("fine-propria"), durata: base.durata,
      "done-eff": done-override, progress: none, "mostra-progress": true,
      intervalli: base.at("intervalli"), "piatti-figli": base.at("piatti-figli"),
    )
  } else {
    base
  }
  // Fase 5 (layout.typ, DC-9): il render ha bisogno, per OGNI nodo
  // datato (non solo le radici), degli stessi valori aggregati che
  // `_raccogli` calcola gia' (durata/`done-eff`/progress/intervalli) per
  // disegnare barra e tratto di avanzamento — non solo per l'aggancio
  // delle dipendenze. Estensione additiva di "piatti" (RNF3: nessuna
  // seconda visita dell'albero, i valori sono gia' in `base`).
  let piatti-proprio = if base.ha-date {
    (
      (
        id: nodo.at("id", default: none), path: nodo.at("path", default: percorso),
        kind: nodo.at("kind", default: "node"),
        inizio: base.inizio, fine: base.fine, "fine-propria": base.at("fine-propria"),
        durata: base.durata, "done-eff": base.at("done-eff"),
        progress: base.progress, "mostra-progress": base.at("mostra-progress"),
        intervalli: base.at("intervalli"),
        dependencies: nodo.at("dependencies", default: ()),
      ),
    )
  } else {
    ()
  }
  (
    ha-date: base.ha-date, inizio: base.inizio, fine: base.fine,
    "fine-propria": base.at("fine-propria"), durata: base.durata,
    "done-eff": base.at("done-eff"), progress: base.progress,
    "mostra-progress": base.at("mostra-progress"),
    intervalli: base.at("intervalli"),
    piatti: piatti-proprio + base.at("piatti-figli"),
  )
}

// --- Viewport (automatico e parziale, RF3; guardia D-064) --------------

/// Tutti i valori "di estremo" dell'albero PIU' milestone e "oggi" (se
/// mostrato) per il lato richiesto (`lato`: `"inizio"` o `"fine"`). Non
/// applica ancora lo snap.
/// **C2 (rilievo del coordinatore, rereview di Fase 6a, D-106):** da
/// `piatti` (OGNI nodo datato esposto, radici E discendenti — D-039/
/// D-104), non più solo dalle radici. Bug corretto: un nodo con date
/// PROPRIE che non aggrega dai figli (RF3) poteva avere un discendente
/// datato che si estende OLTRE il proprio intervallo proprio (es. padre
/// 1-5 gennaio, figlio 1-30 giugno) — prima, usando solo le radici, il
/// viewport automatico ignorava quell'estensione: il figlio sarebbe
/// stato clampato "fuori" (`clamp.outside`), nonostante D-104 lo esponga
/// ora correttamente in `modello.nodi`/si disegni. Ogni voce di `piatti`
/// ha GIA' `ha-date` implicito per costruzione (`_raccogli`: un nodo
/// entra in `piatti-proprio` solo se `base.ha-date`), quindi nessun
/// controllo aggiuntivo qui. Per un albero SENZA questo caso (nessun
/// nodo con date proprie e discendenti datati), il risultato è
/// MATEMATICAMENTE identico a prima: l'`inizio`/`fine` di una radice
/// aggregata è già il min/max dei suoi discendenti (per costruzione
/// dell'aggregazione, ramo "container" di `_raccogli`), quindi il min/
/// max su TUTTI i `piatti` coincide col min/max sulle sole radici.
#let _candidati(lato, piatti, milestones, oggi-eff, mostra-oggi) = {
  let valori = ()
  for n in piatti { valori.push(if lato == "inizio" { n.inizio } else { n.fine }) }
  for m in milestones { valori.push(m.data) }
  if mostra-oggi and oggi-eff != none { valori.push(oggi-eff) }
  valori
}

/// Snap (RF3) della coppia `(vs, ve)' secondo `modo-snap`
/// (`day`/`week`/`month`/`year`) e `first-weekday`: `day` = invariata
/// (si allarga di un giorno solo se `vs == ve`, possibile solo con
/// `instant`); `week`/`month`/`year` = `floor` di `vs` al confine
/// dell'unita' corrispondente, `next` di `ve` SE `ve` non e' gia' sul
/// confine (altrimenti invariata, D1/D-028).
/// `registro` (suggerimento del coordinatore, terza revisione Fase 4):
/// il registry GIA' costruito da `_validate-interno`/`calcola()` (con
/// `week` gia' calcolata sul `first-weekday` giusto) — non si rifa'
/// `units.incluse(first-weekday)` qui (ricostruzione ridondante delle 5
/// unita' incluse, quando la stessa copia esiste gia').
#let _applica-snap(vs, ve, modo-snap, registro) = {
  if modo-snap == "day" {
    if vs == ve { return (vs, dates.aggiungi-giorni(ve, 1)) }
    return (vs, ve)
  }
  let def = registro.at(modo-snap)
  let floor = def.at("floor")
  let next = def.at("next")
  let nuovo-vs = floor(vs)
  let confine-ve = floor(ve)
  let nuovo-ve = if confine-ve == ve { ve } else { next(confine-ve) }
  // C2 (rilievo del coordinatore): se `vs == ve` ed ENTRAMBI cadono gia'
  // sul confine dell'unita' (es. un'unica milestone il 1° del mese con
  // snap "month", o un singolo istante "instant" su un lunedi' con snap
  // "week"), `floor(vs) == floor(ve) == vs == ve`: senza questa guardia
  // il viewport risulterebbe DEGENERE (larghezza zero), `timeline.calcola`
  // non crasherebbe ma i segmenti sarebbero zero e `data-end <=
  // data-start` — un grafico vuoto ma senza errore (contro RNF6: "nessun
  // elemento oltre i bordi", implicitamente richiede bordi distinti).
  // Guardia DOPO lo snap, non prima: vale per QUALUNQUE causa di
  // `vs == ve` (viewport esplicito degenere incluso: gia' intercettato
  // da E-VAL-09 in `validate.typ` per`entrambi` espliciti; qui copre
  // anche il caso "un solo estremo esplicito, l'altro automatico, e i
  // due coincidono per caso").
  if nuovo-ve == nuovo-vs { nuovo-ve = next(nuovo-vs) }
  (nuovo-vs, nuovo-ve)
}

/// Viewport effettivo (RF3, con la guardia D-064): combina l'esplicito
/// (gia' normalizzato da `validate.typ`, date grezze) con l'automatico
/// (dai candidati) PER OGNI ESTREMO separatamente, poi applica lo snap.
/// `spec` e' lo spec normalizzato di `validate()`; `piatti` (C2, D-106:
/// non più solo le radici, vedi `_candidati`) e `milestones-raccolte`
/// sono gia' calcolati dal chiamante (RNF3: una sola passata
/// sull'albero). Fa `panic` E-VAL-10 se un estremo non dato
/// esplicitamente non ha nessun candidato (T5, D-064).
#let _viewport(spec, piatti, milestones-raccolte, locale, registro) = {
  let modo = spec.at("date-mode")
  let vp = spec.at("viewport")
  let ha-vs-esplicito = vp != none and vp.start != none
  let ha-ve-esplicito = vp != none and vp.end != none

  let oggi-eff = if spec.today != none { dates.valida-data(spec.today, "today", locale) } else { none }
  let mostra-oggi = spec.at("show-today")

  // Percorso di E-VAL-10 (D-064, coerente con D-054): "tasks" per la
  // faccia 0.5, "taskgroups" per la 0.1 — l'output di `validate()` non
  // porta un flag di faccia esplicito (il bosco sta sempre sotto
  // `taskgroups`, D-082 punto 1), ma il `kind` della PRIMA radice lo
  // rivela quando esiste almeno una radice; bosco del tutto vuoto (solo
  // milestone, o nessun elemento) -> "taskgroups" (default storico,
  // D-054, come la guardia strutturale di `validate()`).
  let percorso-vuoto = if spec.taskgroups.len() > 0 and spec.taskgroups.at(0).kind == "node" {
    "tasks"
  } else {
    "taskgroups"
  }

  // Estensione REALE (D5): sempre dai candidati, indipendentemente da un
  // viewport esplicito — "il contesto espone `data-start`/`data-end`
  // reali ACCANTO al viewport" (RF3): serve a chi disegna per sapere se
  // un elemento e' stato clampato (D-039), non solo quali bordi usare.
  // Senza nessun candidato su un lato (bosco del tutto privo di date su
  // quel lato, possibile solo se l'ALTRO lato o entrambi sono espliciti,
  // altrimenti E-VAL-10 avrebbe gia' fallito sotto): ricade sul
  // viewport stesso (nessun dato "reale" da riportare).
  let candidati-reali-vs = _candidati("inizio", piatti, milestones-raccolte, oggi-eff, mostra-oggi)
  let candidati-reali-ve = _candidati("fine", piatti, milestones-raccolte, oggi-eff, mostra-oggi)

  let vs = if ha-vs-esplicito {
    dates.valida-data(vp.start, "viewport.start", locale)
  } else {
    if candidati-reali-vs.len() == 0 {
      fail("E-VAL-10", percorso-vuoto, locale)
    }
    _min-data(candidati-reali-vs)
  }
  // "end-written" (E-VAL-09, D-063): la data SCRITTA dall'utente per
  // `viewport.end`, prima della trasformazione di `date-mode` — solo se
  // `end` e' esplicito; se e' automatico non esiste una forma "scritta"
  // diversa dal valore effettivo gia' calcolato (nessuna trasformazione
  // di `date-mode` si applica a un valore gia' derivato dagli
  // intervalli, che sono gia' istanti effettivi).
  // Rifinitura (wording E-VAL-09, rilievo del coordinatore): quando
  // "end" NON e' scritto dall'utente, `ve-scritta` resta `none` e, SENZA
  // questo caso, `{end-written}` mostrerebbe la fine AUTOMATICA (dedotta
  // dai dati) come se fosse "scritta" — testo fuorviante (il messaggio
  // dice letteralmente "end... as written: <stessa data>", suggerendo
  // all'utente di aver scritto un valore che in realta' non ha mai
  // digitato). La voce composita `E-VAL-09.auto` (dati, non prosa nel
  // core) sostituisce la data con un marcatore esplicito.
  let ve-scritta = if ha-ve-esplicito { dates.valida-data(vp.end, "viewport.end", locale) } else { none }
  let ve = if ha-ve-esplicito {
    dates.fine-effettiva(ve-scritta, modo)
  } else {
    if candidati-reali-ve.len() == 0 {
      fail("E-VAL-10", percorso-vuoto, locale)
    }
    _max-data(candidati-reali-ve)
  }

  // K1 (rilievo del coordinatore, CORRETTO in una revisione successiva:
  // il controllo va PRIMA dello snap, non dopo). Un estremo ESPLICITO
  // puo' cadere DOPO (o uguale a) l'altro, quando quest'altro e'
  // AUTOMATICO: `viewport: (start: "2030-01-01")` con tutti i task nel
  // 2025 (`vs` esplicito, `ve` automatico ben prima), oppure `start:
  // "2025-01-11"` con un task che finisce il 10 in `inclusive` (fine-eff
  // 11: `vs == ve`, "viewport vuoto di un giorno") — senza un controllo
  // PRIMA dello snap, nessun errore li intercetta, e in piu' lo snap puo'
  // "raddrizzare" una coppia invertita per un motivo SBAGLIATO: con
  // `start: "2025-01-20"` e snap `month` su dati che finiscono prima,
  // `floor(vs)` e `floor(ve)` cadono nello STESSO mese -> il viewport
  // risultante (tutto gennaio) e' "non invertito" dopo lo snap, ma ha
  // SILENZIOSAMENTE ignorato lo `start` esplicito dell'utente (20
  // gennaio), che non compare piu' da nessuna parte nel risultato — un
  // comportamento scorretto, non una correzione legittima. **Lo snap
  // NON "non puo' mai restringere l'intervallo"** (correzione di
  // un'affermazione sbagliata di una revisione precedente di questo
  // commento): puo' spostare `vs`/`ve` in modo che un rapporto
  // vs<=ve PRIMA dello snap diventi vs<ve DOPO, mascherando l'errore
  // invece di segnalarlo. Il caso "ENTRAMBI espliciti" resta escluso da
  // `validate.typ` (`_valida-viewport`, E-VAL-09) PRIMA di arrivare qui,
  // quindi qui basta la condizione "ALMENO UN estremo esplicito" — il
  // caso "ENTRAMBI automatici" non puo' MAI invertirsi (per ogni singolo
  // elemento dell'albero `start <= fine-eff`, quindi
  // `min(start) <= max(fine-eff)` sempre, al piu' UGUALI con un solo
  // elemento a durata zero: quello e' il dominio legittimo della
  // guardia C2 sotto, non un errore).
  if (ha-vs-esplicito or ha-ve-esplicito) and ve <= vs {
    fail(
      "E-VAL-09", "viewport", locale,
      argomenti: (
        end: dates.formatta(ve), start: dates.formatta(vs), mode: modo,
        "end-written": if ve-scritta != none { dates.formatta(ve-scritta) } else { locale.errors.at("E-VAL-09.auto") },
      ),
    )
  }

  let (vs-snap, ve-snap) = _applica-snap(vs, ve, spec.at("viewport-snap"), registro)
  let data-start = if candidati-reali-vs.len() > 0 { _min-data(candidati-reali-vs) } else { vs-snap }
  let data-end = if candidati-reali-ve.len() > 0 { _max-data(candidati-reali-ve) } else { ve-snap }
  // S7 (rilievo del coordinatore): "oggi" effettivo gia' calcolato qui
  // sopra (serve ai candidati) — restituito cosi' `calcola()` non lo
  // ricalcola una seconda volta per il filtro "oggi fuori viewport".
  (start: vs-snap, end: ve-snap, "data-start": data-start, "data-end": data-end, "oggi-eff": oggi-eff)
}

// --- Costruzione dei livelli d'intestazione (RF4) -----------------------
//
// C1 (rilievo del coordinatore sulla Fase 4): E-HDR-01 (unita'
// sconosciuta), E-HDR-02 (ordine) ed E-HDR-04 (parola chiave di
// `format` non ammessa) sono ORA validati da `validate.typ`
// (`_valida-headers`, chiamata da `_validate-interno`) — `spec.headers`
// che arriva qui e' sempre GIA' valido contro `registro` (lo stesso
// registry, costruito una volta sola da `_validate-interno` e passato
// intatto da `gantt()`). Qui si ricalcola solo la FUNZIONE di formato
// risolta (serve la chiusura vera, che `validate()` pubblico non porta,
// D-029): ricalcolo puro ed economico (poche unita' per livello), non
// un doppio controllo di errore — se `registro`/`spec.headers` non
// fossero coerenti (uso scorretto di questa funzione fuori da
// `gantt()`) l'accesso diretto `registro.at(nome)` darebbe comunque un
// errore (dizionario senza quella chiave), mai un comportamento silente
// sbagliato. Resta qui SOLO **E-HDR-06** (RF4.3): e' un controllo A
// RUNTIME su `floor`/`next` che non si puo' anticipare senza calcolare
// davvero i segmenti.

// --- Entry point pubblico -----------------------------------------------

/// Calcola il modello temporale (RF3/RF4) a partire dallo spec
/// normalizzato di `validate()` (date grezze, D-050), dal registry delle
/// unita' di intestazione (`_validate-interno(..).registro`, con le
/// chiusure vere e GIA' validato anche contro `spec.headers`, C1) e dal
/// locale GIA' risolto (per `first-weekday` e per i messaggi, RF7). Fa
/// `panic` con un errore del catalogo su input strutturalmente valido ma
/// privo di date per un estremo del viewport (E-VAL-10, D-064) o su
/// `floor`/`next` incoerenti a runtime (E-HDR-06).
///
/// - spec (dictionary): uscita di `validate()`.
/// - registro (dictionary): `_validate-interno(..).registro` (nome ->
///   definizione con le chiusure vere).
/// - locale (dictionary): locale gia' risolto (`locale.resolve(..)`).
/// -> dictionary: `(viewport: (start, end), radici: array, milestones:
///    array, headers: array di (unit, "min-cell-width", align, gridline,
///    segments: array di segmenti con label) — "align"/"gridline" sono
///    i default della DEFINIZIONE d'unita' (RF4/RF4.3, R6/fallback
///    gridline, D-102), non ancora un override per livello,
///    "dependency-anchors": dizionario id ->
///    (inizio, fine), "dependency-constraints": array, clamp: dizionario
///    path -> (start, end, outside), nodi: dizionario path -> (inizio,
///    fine, "fine-propria", durata, "done-eff", progress,
///    "mostra-progress", intervalli) — Fase 5, DC-9: valori aggregati
///    di OGNI nodo datato, non solo le radici)`.
#let calcola(spec, registro, locale) = {
  let modo = spec.at("date-mode")

  let radici-raccolte = spec.taskgroups.map(r => _raccogli(r, modo, locale, r.path))
  // D4 (rilievo del coordinatore, revisione Fase 5, D-097+1): `style`/
  // `color` passano QUI invariati (nessuna interpretazione temporale,
  // solo trasporto) — bug trovato con una controprova a mutazione:
  // senza questi due campi, `layout.calcola` (che legge SOLO
  // `modello.milestones`, mai `spec.milestones` direttamente, per non
  // ricalcolare il filtro di viewport già fatto qui, RNF3/DC-8) non
  // può MAI vedere lo stile proprio di una milestone — `m.style` vi
  // arriverebbe sempre vuoto, `style.stile-milestone` userebbe sempre
  // il solo stroke di base, silenziosamente (nessun errore, nessun
  // avviso: un difetto "muto", lo stesso tipo di bug che il principio
  // "testo-utente-fonte-unica-nel-core" vuole evitare per i dati).
  // `path` (rereview finale di Fase 9, code-reviewer, punto 4): SOLO per
  // il percorso di un eventuale errore di conversione YAML di `m.style`
  // (`style.stile-milestone`) — prima mancava del tutto: un errore
  // mostrava `milestones["?"].style...` (il valore di ripiego di
  // `m.at("name", default: "?")`, MAI raggiunto: questo record ha
  // `nome`, non `name`), senza nessun indice utile a trovare QUALE
  // milestone.
  let milestones-raccolte = spec.milestones.enumerate().map(((i, m)) => (
    nome: m.name, data: dates.valida-data(m.date, "milestones", locale), "show-date": m.at("show-date"),
    style: m.at("style", default: (:)), color: m.at("color", default: none),
    path: "milestones[" + str(i) + "]",
  ))

  // C2 (rilievo del coordinatore, rereview di Fase 6a, D-106): `piatti`
  // calcolato QUI, PRIMA di `_viewport` (non più dopo) — serve ai
  // candidati del viewport automatico (vedi `_candidati`), non solo agli
  // agganci delle dipendenze sotto. Nessuna ricostruzione dell'albero
  // (RNF3): e' la STESSA chiamata, solo spostata prima nel tempo.
  let piatti = radici-raccolte.map(r => r.piatti).flatten()

  let viewport = _viewport(spec, piatti, milestones-raccolte, locale, registro)

  // `spec.headers` e' gia' valido contro `registro` (E-HDR-01/02/04 in
  // `validate.typ`, C1): qui si estrae solo `(nome, "format-utente")`
  // per livello e si costruiscono i segmenti.
  let headers = spec.at("headers").enumerate().map(((i, h)) => {
    let percorso = "headers[" + str(i) + "]"
    let nome = if type(h) == str { h } else { h.unit }
    let formato-utente = if type(h) == dictionary { h.at("format", default: none) } else { none }
    let def = registro.at(nome)
    let formattatore = units.risolvi-formato(nome, def, formato-utente, percorso + ".format", locale)
    let segs = units.segmenti(nome, def, viewport.start, viewport.end, locale)
    (
      unit: nome,
      // Fase 5 (layout.typ, RF4/D-028): soglia di default DELL'UNITA'
      // per la soppressione "min-cell-width" (il livello non disegna
      // riga ne' griglia se i segmenti sono troppo stretti). Qui solo il
      // default della DEFINIZIONE: l'override per livello da
      // `headers[i].min-cell-width` e' di Fase 6a (non ancora validato
      // da `_valida-headers`).
      "min-cell-width": def.at("min-cell-width"),
      // R6 (D-102): `align` DICHIARATO dalla DEFINIZIONE dell'unita'
      // (`units.typ`: "center + horizon" per la maggior parte, "left +
      // horizon" per "week", "center" per "day").
      align: def.at("align"),
      // RF4.3 (D-102, fallback `gridline`): il `gridline` di DEFAULT
      // della definizione d'unita' (`none` per le unita' incluse,
      // eventualmente non-`none` per una unita' custom registrata con
      // `header-units:`).
      gridline: def.at("gridline"),
      // D-106 punto (a) — "headers a dizionario per livello": `def.text`
      // (gia' validato da `units.typ`, mai letto da `layout.typ` prima
      // d'ora) e l'eventuale dizionario DEL LIVELLO (`headers[i]`,
      // gia' validato — tipi e chiavi ammesse — da `validate.typ`), con
      // ciascuno dei campi "opzionali" presente solo se l'utente lo ha
      // scritto (`.at(.., default: none)`: `none` qui significa "non
      // scritto", non un valore esplicito — la precedenza vera, con
      // `style.headers.<unita>` nel mezzo, si risolve in `layout.typ`
      // per evitare di ricalcolarla per ogni livello a ogni chiamata).
      "def-text": def.at("text"),
      "livello-align": if type(h) == dictionary { h.at("align", default: none) } else { none },
      "livello-text": if type(h) == dictionary { h.at("text", default: none) } else { none },
      "livello-fill": if type(h) == dictionary { h.at("fill", default: none) } else { none },
      "livello-stroke": if type(h) == dictionary { h.at("stroke", default: none) } else { none },
      "livello-gridline": if type(h) == dictionary { h.at("gridline", default: none) } else { none },
      "livello-height": if type(h) == dictionary { h.at("height", default: none) } else { none },
      "livello-overflow": if type(h) == dictionary { h.at("overflow", default: none) } else { none },
      "livello-min-cell-width": if type(h) == dictionary { h.at("min-cell-width", default: none) } else { none },
      "livello-inset": if type(h) == dictionary { h.at("inset", default: none) } else { none },
      segments: segs.map(s => (
        unit: s.unit, start: s.start, end: s.end, days: s.days,
        index: s.index, partial: s.partial, label: formattatore(s),
      )),
    )
  })

  // Agganci delle dipendenze (RF10): da "piatti" (gia' calcolato sopra,
  // prima di `_viewport`, C2/D-106 — RNF3, nessuna ricostruzione
  // dell'albero: una chiusura che mutasse una variabile catturata
  // fallirebbe comunque con "variables from outside... are read-only",
  // T25, verificato con una sonda dedicata). Un `for` semplice nel corpo
  // di `calcola()` (non una chiusura annidata) PUO' mutare un dizionario
  // locale (come in `graph._prepara`).
  let per-id-agganci = (:)
  for n in piatti {
    if n.id != none { per-id-agganci.insert(n.id, (inizio: n.inizio, fine: n.fine)) }
  }
  // Fase 5 (layout.typ, DC-9): dizionario "per-path" con i valori
  // aggregati di OGNI nodo datato (gruppo o foglia), non solo le radici
  // -- serve al render per disegnare la barra/il tratto di avanzamento
  // di ciascuna riga senza ripetere la ricorsione di `_raccogli` (RNF3).
  let per-path-nodi = (:)
  for n in piatti {
    per-path-nodi.insert(n.path, (
      inizio: n.inizio, fine: n.fine, "fine-propria": n.at("fine-propria"), durata: n.durata,
      "done-eff": n.at("done-eff"), progress: n.progress,
      "mostra-progress": n.at("mostra-progress"), intervalli: n.at("intervalli"),
    ))
  }

  // Vincoli violati (D-034 punto 6, D-039): per ogni dipendenza di ogni
  // nodo PIATTO con un aggancio noto. I nodi SENZA date non arrivano qui
  // (E-DEP-05 li ha gia' esclusi in `validate()`), quindi sia la
  // sorgente (`n`) sia il bersaglio (`dep.id`) hanno sempre un aggancio.
  let vincoli = ()
  for n in piatti {
    let aggancio-n = (inizio: n.inizio, fine: n.fine)
    for dep in n.dependencies {
      if dep.id in per-id-agganci {
        let aggancio-pred = per-id-agganci.at(dep.id)
        let violato = (
          if dep.type == "fs" { aggancio-n.inizio < aggancio-pred.fine }
          else if dep.type == "ss" { aggancio-n.inizio < aggancio-pred.inizio }
          else if dep.type == "ff" { aggancio-n.fine < aggancio-pred.fine }
          else { aggancio-n.fine < aggancio-pred.inizio }
        )
        // "x" (Fase 7, RF11 P10): dizionario libero dell'utente su
        // QUESTA voce di `dependencies[k]` (`validate._valida-dipendenza`
        // lo normalizza sempre a un dizionario, `(:)` se assente) --
        // trasportato PASSIVAMENTE fino al drawer `dependency` ridefinito
        // (`dep.x`, RF11), mai letto dal core (stessa convenzione di
        // `node.x` per il drawer `label`). Chiave ADDITIVA: nessun
        // consumatore esistente di `vincoli`/`dependency-constraints`
        // legge l'insieme completo delle chiavi (RF11 "interfaccia
        // versionata, solo aggiunte").
        vincoli.push((da: dep.id, a: n.id, path: n.path, type: dep.type, violato: violato, x: dep.x))
      }
    }
  }

  // Clamp al viewport (D-039/D-064 punto f): per OGNI nodo datato
  // (`piatti`, stessa lista di sopra — nessuna seconda visita
  // dell'albero), l'intervallo clampato a `[viewport.start,
  // viewport.end]` e il flag "outside" (nodo interamente fuori:
  // nessuna barra da disegnare, riga mantenuta vuota, RF3). Dizionario
  // per `path`, cosi' il render (Fase 5+) non ricalcola nulla.
  let clamp = (:)
  for n in piatti {
    let fuori = n.fine <= viewport.start or n.inizio >= viewport.end
    // Rilievo del coordinatore (D5f, bug trovato dalla controprova nel
    // test): per un nodo INTERAMENTE fuori range, clampare OGNI estremo
    // per conto proprio (il vecchio codice) dava un intervallo
    // "all'indietro" (es. nodo tutto PRIMA del viewport: inizio
    // clampato a `viewport.start`, ma fine lasciata invariata, ancora
    // PRIMA di `viewport.start` — inizio > fine). Qui invece, se
    // `fuori`, i due estremi collassano sullo stesso bordo piu' vicino
    // (un intervallo di larghezza zero, sempre valido: inizio <= fine):
    // `outside` resta il segnale primario per il render, ma
    // `start`/`end` non sono piu' spazzatura se qualcosa li legge senza
    // controllare `outside` per primo.
    let (inizio-clampato, fine-clampata) = if fuori {
      if n.fine <= viewport.start { (viewport.start, viewport.start) } else { (viewport.end, viewport.end) }
    } else {
      (
        if n.inizio < viewport.start { viewport.start } else { n.inizio },
        if n.fine > viewport.end { viewport.end } else { n.fine },
      )
    }
    clamp.insert(n.path, (start: inizio-clampato, end: fine-clampata, outside: fuori))
  }

  // Milestone/oggi fuori range (D-039/D-064 punto f): "non si
  // disegnano" -> assenti dal CONTESTO (non solo ignorati dal render),
  // calcolato una volta qui (RNF3). Rilievo Fase 5 (bug trovato dalla
  // revisione visiva del render vero, non raggiungibile col segnaposto
  // di D-051): a differenza dei SEGMENTI (RF4: `[inizio, fine)`
  // semiaperto, cosi' due segmenti adiacenti non si sovrappongono), una
  // milestone e' un ISTANTE puntuale, non uno span — se cade esattamente
  // su `viewport.end` e' comunque al bordo DESTRO dell'area (x = W, una
  // posizione valida e visibile, non "oltre", RNF6), non appena fuori:
  // il confine superiore va incluso (`<=`), altrimenti una milestone
  // posta esattamente sull'ultimo giorno/istante del grafico (frequente:
  // spesso è proprio quel giorno a determinare `viewport.end` via
  // `_candidati`) sparisce in silenzio dal contesto. `viewport.start` è
  // già incluso (`>=`) perché è sempre il PRIMO istante disegnato.
  let milestones-nel-viewport = milestones-raccolte.filter(
    m => m.data >= viewport.start and m.data <= viewport.end,
  )
  // S7: "oggi" effettivo riusato da `viewport.at("oggi-eff")` (gia'
  // calcolato da `_viewport`), non ricalcolato una seconda volta.
  let oggi-eff-finale = viewport.at("oggi-eff")
  let oggi-nel-viewport = (
    spec.at("show-today") and oggi-eff-finale != none
      and oggi-eff-finale >= viewport.start and oggi-eff-finale <= viewport.end
  )

  (
    viewport: viewport,
    radici: radici-raccolte,
    milestones: milestones-nel-viewport,
    today: if oggi-nel-viewport { oggi-eff-finale } else { none },
    headers: headers,
    "dependency-anchors": per-id-agganci,
    "dependency-constraints": vincoli,
    clamp: clamp,
    nodi: per-path-nodi,
  )
}
