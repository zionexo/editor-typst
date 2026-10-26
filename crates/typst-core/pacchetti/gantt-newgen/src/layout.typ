// gantt-newgen — src/layout.typ
// Geometria pura in `pt` (RF3/RF5/RF10/RNF3/RNF6), a partire SOLO
// dall'output di `timeline.calcola()` (viewport, headers, clamp, nodi,
// "dependency-anchors"/"dependency-constraints") e dallo stile già
// risolto (`style.risolvi`): righe misurate, header, soppressione
// `min-cell-width`, tratto di avanzamento per intervallo (DC-9),
// anti-collisione e clamp delle etichette milestone (skill
// `etichette-grafico-geometria-pura`), routing delle dipendenze come
// polilinee e spezzamento per fascia di riga (D-037). Core puro: non
// ricalcola mai date né filtri già fatti dalla timeline (RNF3); l'unico
// content che produce è per `measure()` (misure di larghezza/altezza,
// mai per il disegno finale: "Struttura dei moduli").
//
// Stato (Fase 7, revisione del 2026-10-18): contesto per barre/
// milestone/oggi/header/sidebar/frecce pronto per `src/render/*`. Il
// ROUTING delle dipendenze (`instrada-*`/`clip-segmento`/`spezza-per-
// righe`, qui, invariate dalla Fase 5) è ora anche DISEGNATO
// (`src/render/dependencies.typ`): `calcola()` assembla agganci,
// polilinee, stroke/punta risolti e tratti-per-riga in `ctx.dipendenze`/
// `riga.frecce`, consumati dal percorso rapido di default e da un
// drawer `dependency` ridefinito (RF11).

#import "dates.typ"
#import "errors.typ": fail
// Alias (non "locale"): il nome "locale" è usato ovunque in questo file
// per il parametro con il locale GIA' risolto (un dizionario) — un
// `#import "locale.typ"` senza alias lo ombreggerebbe col MODULO,
// rendendo `locale.at(..)`/`locale.months` irraggiungibili dentro
// `calcola()` (stessa insidia di `lib.typ` con `layout`/`layout-mod`).
#import "locale.typ" as locale-mod
#import "schema.typ": figli
#import "style.typ"

// --- Utilità di geometria in pt (calc.clamp non accetta `length`) ------

/// Clampa una lunghezza in `[minimo, massimo]` (`calc.clamp` richiede
/// `int`/`float`/`decimal`, non `length`: T-insidia di Typst, nessuna
/// voce dedicata nel catalogo T perché non fa panic, dà solo il tipo
/// sbagliato in uscita se usata male — qui la guardia è un confronto
/// diretto, non `calc.clamp`).
#let clampa(v, minimo, massimo) = if v < minimo { minimo } else if v > massimo { massimo } else { v }

/// Contenuto minimo per misurare la larghezza/altezza di un'etichetta
/// (nome di nodo, testo di milestone/oggi, etichetta d'intestazione):
/// `nome` può essere `content` già pronto (RF2/RF4 ammettono `content`)
/// o una stringa.
#let _contenuto(nome) = if type(nome) == str { [#nome] } else { nome }

/// Spezza una PAROLA singola (nessuno spazio) che non entra nemmeno da
/// sola in `larghezza` (pt) in più "pezzi" che ci stanno ciascuno, per
/// CLUSTER (caratteri visibili, non byte — coerente con l'ex
/// `tronca-una-riga`, D-102/T26). Bisezione (O(log n) misure per
/// pezzo, come l'ellissi di D-102): la larghezza di `slice(inizio, m)`
/// è monotona non-decrescente in `m`, quindi esiste un `m` massimo che
/// ci sta. Garantisce SEMPRE un progresso di almeno un carattere per
/// pezzo (`trovato` inizializzato a `inizio + 1`): nel caso degenere in
/// cui UN SOLO carattere sia già più largo di `larghezza` (font/cella
/// estremi), quel pezzo eccede — "best effort" documentato, stesso
/// limite noto di prima (l'ellissi che non ci stava da sola).
#let _spezza-parola(parola, stile-testo, larghezza) = {
  let misura(s) = measure(text(..stile-testo)[#s]).width
  let caratteri = parola.clusters()
  let pezzi = ()
  let inizio = 0
  while inizio < caratteri.len() {
    let lo = inizio + 1
    let hi = caratteri.len()
    let trovato = inizio + 1
    while lo <= hi {
      let mid = calc.quo(lo + hi, 2)
      if misura(caratteri.slice(inizio, mid).join()) <= larghezza {
        trovato = mid
        lo = mid + 1
      } else {
        hi = mid - 1
      }
    }
    pezzi.push(caratteri.slice(inizio, trovato).join())
    inizio = trovato
  }
  pezzi
}

/// Spezza un NOME (stringa) su più RIGHE per PAROLE quando non entra
/// su una sola, dentro `larghezza` (pt) — **DECISIONE UTENTE
/// 2026-10-16 (R5, D-108): sostituisce l'ellissi di D-102/D-100 S3.**
/// Quando `sidebar-width`/`style.sidebar` è esplicito e il nome non
/// entra, va A CAPO (non più troncato): la riga CRESCE in altezza per
/// contenerlo (il chiamante, `calcola()`, misura l'altezza del blocco
/// risultante e la propaga a `posiziona()`). Algoritmo greedy
/// standard: accumula parole finché `corrente + " " + parola` entra
/// ancora in `larghezza`; altrimenti chiude la riga corrente e ne apre
/// una nuova con la parola. Una parola che non entra NEMMENO da sola
/// (es. un URL, un identificatore senza spazi) si spezza per cluster
/// (`_spezza-parola`, sopra): **nessun trabocco MAI**, a differenza del
/// testo "nudo" affidato al motore di Typst (che non spezza una parola
/// troppo lunga all'interno di un `box` a larghezza fissa — resterebbe
/// più larga della cella). Restituisce un array di stringhe non-vuoto
/// (anche per un nome vuoto: una riga sola, vuota) — il chiamante lo
/// unisce con `linebreak()` in un unico `content`.
#let spezza-a-capo(nome, stile-testo, larghezza) = {
  let misura(s) = measure(text(..stile-testo)[#s]).width
  // Suggerimento (review finale di Fase 6a): split su QUALUNQUE
  // sequenza di spazi bianchi (`regex("\s+")`, non il solo spazio " ")
  // — un nome con un tab o un "a capo" letterale nella sorgente YAML
  // (raro ma possibile) va a capo per parole comunque, invece di
  // trattare l'intero spazio bianco "strano" come parte di una parola
  // (misurata assieme, cambierebbe dove la riga si spezza). `.filter`
  // toglie le stringhe vuote (spazi bianchi doppi/iniziali/finali).
  let parole = nome.split(regex("\s+")).filter(p => p != "")
  if parole.len() == 0 { return ("",) }
  let righe = ()
  let corrente = ""
  for parola in parole {
    let prova = if corrente == "" { parola } else { corrente + " " + parola }
    if misura(prova) <= larghezza {
      corrente = prova
      continue
    }
    // `prova` non entra: la riga corrente (se non vuota) si chiude qui.
    if corrente != "" { righe.push(corrente) }
    if misura(parola) <= larghezza {
      corrente = parola
    } else {
      // La parola non entra NEMMENO da sola: la spezza per cluster.
      let pezzi = _spezza-parola(parola, stile-testo, larghezza)
      righe += pezzi.slice(0, pezzi.len() - 1)
      corrente = pezzi.last()
    }
  }
  righe.push(corrente)
  righe
}

/// Nome della sidebar pronto per il render (R5, D-108): `none` se il
/// nome entra su UNA riga nella `larghezza` disponibile (percorso
/// comune, UNA sola misura di verifica — stesso costo della vecchia
/// `tronca-una-riga`, nessuna regressione di RNF3/S5) o se non è una
/// stringa (RF2 ammette `content`: nessuna scomposizione in "parole"
/// di un `content` arbitrario, limite noto e documentato — invariato
/// rispetto a prima, si limitava già a lasciarlo passare intatto).
/// Altrimenti va a capo per parole (`spezza-a-capo`) e restituisce
/// `(nome: content, altezza: length)`: `content` con un `linebreak()`
/// fra le righe, `altezza` misurata UNA volta sul blocco risultante
/// (il chiamante la usa per far CRESCERE la riga, mai per
/// rimpicciolire il testo, skill `etichette-grafico-geometria-pura`).
#let nome-sidebar-pronto(nome, stile-testo, larghezza) = {
  if type(nome) != str { return none }
  if measure(text(..stile-testo)[#nome]).width <= larghezza { return none }
  let righe = spezza-a-capo(nome, stile-testo, larghezza)
  let contenuto = righe.map(r => [#r]).join(linebreak())
  (nome: contenuto, altezza: measure(text(..stile-testo)[#contenuto]).height)
}

/// Mappatura data -> ascissa in pt (RF3: "posizione x per data =
/// frazione × larghezza"), già clampata in `[0, larghezza]` (RNF6).
/// `giorni-totali` è la larghezza del viewport in giorni (sempre > 0:
/// garantito dalla guardia C2 di `timeline._applica-snap`).
#let crea-x(viewport, larghezza) = {
  let giorni-totali = dates.giorni-fra(viewport.start, viewport.end)
  data => {
    let g = dates.giorni-fra(viewport.start, data)
    clampa(g / giorni-totali * larghezza, 0pt, larghezza)
  }
}

/// Come `crea-x`, ma SENZA clamp (RNF6-bis, rilievo C2 del
/// coordinatore): serve SOLO per interpolare un punto FRA due date,
/// quando una o entrambe potrebbero cadere fuori dal viewport (DC-9,
/// `tratto-progresso`). La mappatura data -> pt è lineare su TUTTO
/// l'asse del tempo, dentro o fuori dal viewport: interpolare fra due
/// posizioni GIÀ clampate (come faceva la versione precedente di
/// `tratto-progresso`) distorce la proporzione — l'interpolazione va
/// fatta sulle posizioni VERE, il clamp solo sul punto RISULTANTE
/// (a carico del chiamante).
#let crea-x-grezza(viewport, larghezza) = {
  let giorni-totali = dates.giorni-fra(viewport.start, viewport.end)
  data => dates.giorni-fra(viewport.start, data) / giorni-totali * larghezza
}

// --- Stile per segmento di barra e tacca "instant" (R1, D-102) ---------
// Spostato qui da `render/bars.typ` (D-100 punto 9, residuo R1): e'
// geometria/risoluzione di stile, non disegno (RNF5) -- `bars.typ`
// legge solo il risultato, non lo calcola piu'.

/// Voce di stile per un segmento di barra (`"timeframe"`/`"body"`, o
/// l'unica voce `uncompleted`): isola il caso "uncompleted" (niente
/// sottochiave `parte`) dagli altri due stati. Condivisa fra
/// `segmenti-stato` (qui, per la tacca D5 sotto) e `render/bars.typ`
/// (per `fill`/`radius`/`progress.stroke`, presentazione pura).
#let voce-stile(stile-barra, stato, parte) = if stato == "uncompleted" {
  stile-barra.uncompleted
} else {
  stile-barra.at(stato).at(parte)
}

/// Tacca "instant" (D5, D-100): un segmento di larghezza vera sotto
/// `altezza-minima` (es. `start == end`, un solo istante) sarebbe
/// invisibile -- nessuna tacca per un evento puntuale reale. Allarga
/// SIMMETRICAMENTE a `altezza-minima` (un quadratino centrato sul
/// punto); sopra quella soglia la larghezza vera resta invariata.
#let _tacca-larghezza(start, end, altezza-minima) = {
  let larghezza-vera = end - start
  if larghezza-vera < altezza-minima {
    (start: start - (altezza-minima - larghezza-vera) / 2, end: start - (altezza-minima - larghezza-vera) / 2 + altezza-minima)
  } else {
    (start: start, end: end)
  }
}

// --- Tratto di avanzamento per intervallo (RF3, DC-9) -------------------

/// Segmenti (in pt, array di `(start, end)`) del tratto di avanzamento
/// di UN nodo datato: `progress` ignorato se il nodo ha un `done`
/// effettivo (RF3 generale) o se la durata è 0 (guardia x/0, T18/D-039,
/// nessun tratto). Senza intervalli propri (container/gruppo
/// aggregato), si usa lo span sintetico `[inizio, fine]` del nodo
/// (`timeline.typ`, commento su `_raccogli`: "vuoto per un container,
/// Fase 5 usa inizio/fine direttamente"); questo è un'approssimazione
/// nota per i nodi con FIGLI DISTANZIATI (gap non rappresentati, a
/// differenza del caso a intervalli propri, dove ogni pausa reale resta
/// senza tratto): documentata, non testata oltre il caso foglia (gli
/// unici criteri RF3 di Fase 5 — 50%/75% — sono su un nodo a 2
/// intervalli propri).
/// **C2 (review di Fase 6b, BUG CHIUSO):** il denominatore del target
/// (`progress / 100 × denominatore`) è la somma delle `giorni-fra`
/// degli intervalli EFFETTIVAMENTE usati nel loop sotto (per un nodo a
/// intervalli propri: le sue durate reali, identico a `ninfo.durata`
/// per costruzione — `timeline._durata` somma esattamente questo;
/// per un container: la lunghezza dello SPAN SINTETICO disegnato, `[
/// inizio, fine]`) — **mai più `ninfo.durata` letto direttamente**: per
/// un container quel campo è la somma delle durate dei FIGLI
/// (`timeline._raccogli`), che con figli SOVRAPPOSTI supera la
/// lunghezza dello span disegnato (due figli di 10gg sulle stesse
/// 10gg: somma 20, span 10) — usarlo come denominatore disegnava il
/// tratto per l'INTERA barra anche con `progress: 50` (il target,
/// 50% di 20 = 10gg, coincideva o superava la lunghezza VERA dello
/// span, 10gg). Corretto: `durata` si ricalcola QUI sommando
/// `giorni-fra` sugli `intervalli` (sotto), MAI letto da `ninfo`.
/// **C2 (rilievo del coordinatore, revisione Fase 5):** il punto
/// d'arrivo PARZIALE (dentro un intervallo, non alla sua fine intera)
/// si calcola interpolando le posizioni NON clampate (`x-grezza`) e si
/// clampa SOLO il risultato — interpolare fra due posizioni GIÀ
/// clampate (versione precedente, bug) distorce la proporzione quando
/// un estremo dell'intervallo cade fuori dal viewport: un intervallo
/// che inizia PRIMA del viewport avrebbe dato un punto a metà della
/// porzione VISIBILE, non a metà della durata VERA. Per il tratto
/// INTERO (`rimanente >= lunghezza`, nessuna interpolazione) `x`
/// clampata resta corretta: i due estremi sono date reali dell'
/// intervallo, non un punto interpolato.
/// `x`: chiusura `data => pt` di `crea-x` (clampata). `x-grezza`:
/// chiusura `data => pt` di `crea-x-grezza` (NON clampata, stesso
/// viewport/larghezza). `larghezza-area`: per il clamp del punto finale.
#let tratto-progresso(ninfo, x, x-grezza, larghezza-area) = {
  if ninfo == none or ninfo.at("done-eff") != none or ninfo.progress == none {
    return ()
  }
  let intervalli = if ninfo.intervalli.len() > 0 {
    ninfo.intervalli
  } else {
    ((inizio: ninfo.inizio, "fine-eff": ninfo.fine),)
  }
  // C2 (review di Fase 6b): MAI `ninfo.durata` qui (per un container è
  // la somma delle durate dei FIGLI, non la lunghezza dello span
  // disegnato) — ricalcolata sugli `intervalli` DAVVERO usati nel loop
  // sotto, identica a `ninfo.durata` per un nodo a intervalli propri
  // (stessa somma, per costruzione) e alla lunghezza dello span per un
  // container (un solo intervallo sintetico).
  let durata = intervalli.fold(0, (acc, iv) => acc + dates.giorni-fra(iv.inizio, iv.at("fine-eff")))
  if durata == 0 { return () }
  let target = ninfo.progress / 100 * durata
  let rimanente = target
  let segmenti = ()
  for iv in intervalli {
    if rimanente <= 0 { break }
    let lunghezza = dates.giorni-fra(iv.inizio, iv.at("fine-eff"))
    if lunghezza <= 0 { continue }
    let x0 = x(iv.inizio)
    let x1 = x(iv.at("fine-eff"))
    if rimanente >= lunghezza {
      segmenti.push((start: x0, end: x1))
      rimanente -= lunghezza
    } else {
      let frazione = rimanente / lunghezza
      let x0-grezza = x-grezza(iv.inizio)
      let x1-grezza = x-grezza(iv.at("fine-eff"))
      let punto-grezzo = x0-grezza + frazione * (x1-grezza - x0-grezza)
      // Suggerimento (rereview di Fase 6a, D-106): `end` non è
      // GARANTITO `>= start` da nient'altro che l'aritmetica (il punto
      // interpolato e GREZZO è clampato al risultato, non vincolato a
      // stare dopo `x0`, già clampato A PARTE): la guardia `calc.max`
      // (prima in `render/bars.typ`, ricalcolata lì ogni volta) si
      // applica QUI, una volta sola, alla fonte della geometria — il
      // render legge `p.end - p.start` e la usa direttamente (RNF5,
      // nessun ricalcolo/nessuna guardia duplicata a valle).
      segmenti.push((start: x0, end: calc.max(clampa(punto-grezzo, 0pt, larghezza-area), x0)))
      rimanente = 0
    }
  }
  segmenti
}

// --- Stato e segmenti della barra (RF3: completed-early/-late) ---------

/// Stato (early/late/uncompleted) e segmenti GREZZI (senza tacca, in pt,
/// `(start, end, parte)`) di UN SOLO span con l'eventuale `done-eff` di
/// QUEL span — fattorizzato da `segmenti-stato` (sotto) perché Fase 6b
/// (RF3, P4) applica la STESSA regola sia al nodo nel suo complesso (0 o
/// 1 intervallo, invariato da Fase 5/6a) sia a CIASCUN intervallo
/// separatamente, quando ce ne sono più di uno (`casi-intervalli-stato`,
/// `unit/layout.typ`). `fine-propria` è la fine effettiva PROPRIA dello
/// span (usata per `completed-early`/`completed-late`, confrontata con
/// `done-eff`); `fine` è la fine VISIVA (usata SOLO per `uncompleted`,
/// sotto). Per un intervallo singolo le due coincidono sempre (nessuna
/// estensione possibile da "sotto" di lui); per il NODO AGGREGATO
/// (container) possono DIFFERIRE anche con `done-eff` del nodo `none`:
/// **S2 (review di Fase 6b, BUG CHIUSO — D-111 corretta):** `ninfo.fine`
/// di un container è `_max-data(figli.map(f => f.fine))` — il `fine` di
/// OGNI figlio include GIÀ la sua eventuale estensione per `done` tardivo
/// (`f.fine = max(f.fine-propria, f.done-eff)`) — mentre `ninfo.
/// fine-propria` è `_max-data(figli.map(f => f.fine-propria))`, SENZA
/// quell'estensione. Un solo figlio completato IN RITARDO (il suo `done`
/// supera la sua fine) basta a rendere `ninfo.fine > ninfo.fine-propria`
/// anche se il CONTAINER stesso non ha un `done-eff` aggregato (richiede
/// che TUTTI i figli abbiano `done`, non solo uno) — D-111 aveva
/// erroneamente assunto `fine == fine-propria` sempre quando `done-eff`
/// è `none` (vero SOLO per un nodo con fonte di date PROPRIA, mai per un
/// container: la dimostrazione di D-111 usava la formula del nodo
/// foglia, non quella — diversa — del ramo container di
/// `timeline._raccogli`). La barra `uncompleted` del container deve
/// arrivare fino alla fine VISIVA (comportamento di Fase 5/6a, nei
/// golden approvati), non fino a `fine-propria` (che "nasconderebbe"
/// l'estensione del figlio in ritardo).
/// Ogni segmento grezzo porta anche le DUE date che lo delimitano
/// (`"data-inizio"`/`"data-fine"`, mai esposte fuori da `con-tacca`,
/// sotto): residuo DS-1 (revisione del 2026-10-17 su 6b/6c, BUG CHIUSO)
/// — servono a distinguere, DOPO il clamp di `x()`, un segmento la cui
/// larghezza vera e' zero PERCHE' interamente fuori dal viewport (le
/// due date NON coincidono, es. "body" `[1gen,5gen]` con viewport che
/// parte l'8gen: `x()` le clampa entrambe a 0pt) da un istante VERO
/// (le due date COINCIDONO, D5): solo il secondo merita la tacca.
#let _stato-span(inizio, fine-propria, fine, done-eff, x) = {
  let x0 = x(inizio)
  if done-eff == none {
    return ("uncompleted", ((start: x0, end: x(fine), parte: "uncompleted", "data-inizio": inizio, "data-fine": fine),))
  }
  if done-eff <= fine-propria {
    let xd = x(done-eff)
    ("completed-early", (
      (start: x0, end: xd, parte: "timeframe", "data-inizio": inizio, "data-fine": done-eff),
      (start: xd, end: x(fine-propria), parte: "body", "data-inizio": done-eff, "data-fine": fine-propria),
    ))
  } else {
    let xf = x(fine-propria)
    ("completed-late", (
      (start: x0, end: xf, parte: "body", "data-inizio": inizio, "data-fine": fine-propria),
      (start: xf, end: x(done-eff), parte: "timeframe", "data-inizio": fine-propria, "data-fine": done-eff),
    ))
  }
}

/// Segmenti di stato della barra di un nodo datato: `(stato, segmenti)`
/// con `segmenti` = array di `(start, end, parte, stato)` in pt, GIA' con
/// la tacca "instant" (D5) applicata (R1, D-102: spostata da
/// `render/bars.typ` — geometria, non disegno, RNF5) e con il PROPRIO
/// `stato` per segmento (Fase 6b, P4: serve quando gli intervalli sono
/// più di uno e ciascuno può avere uno stato DIVERSO — `render/
/// drawers.typ` sceglie la voce di stile con `voce-stile(stile-barra,
/// s.stato, s.parte)`, mai più col `stato` complessivo del nodo). Il
/// campo `stato` di primo livello resta quello AGGREGATO del nodo
/// (`"none"` se il nodo è clampato interamente fuori dal viewport,
/// D-039: niente da disegnare; altrimenti `"uncompleted"`/
/// `"completed-early"`/`"completed-late"`, stessa regola di
/// `_stato-span` sul SOLO span `[inizio, fine-propria]`+`done-eff` del
/// nodo) — usato SOLO per il caso "none"/da chi vuole un riassunto
/// (es. test), mai per scegliere la voce di stile di un segmento
/// quando ci sono più intervalli (userebbe lo stato SBAGLIATO per gli
/// intervalli che non condividono lo stato del nodo). `x()` clampa già
/// da sé (il `clamp.outside` serve solo a saltare il calcolo, non a
/// correggere le ascisse: coerente con `dependency-anchors`/D-039).
/// `stile-barra` (RF3/RF5, già risolto dal chiamante) serve SOLO per
/// l'altezza di ogni `parte` (soglia della tacca D5): un segmento che la
/// tacca allarga non cambia `parte`, solo `start`/`end`.
///
/// **P4 (RF3, Fase 6b): con PIÙ di un intervallo proprio**, ogni
/// intervallo disegna il PROPRIO stato (confronto fra il suo `done-eff`
/// e la sua `fine-eff`, RF3 "identico nelle due modalità") — le PAUSE
/// fra un intervallo e il successivo restano vuote per costruzione
/// (nessun segmento generato per il tratto fra `intervalli.at(i).
/// fine-eff` e `intervalli.at(i+1).inizio`: la riga NON disegna un
/// segmento "ponte"). Con 0 o 1 intervallo proprio (caso comune,
/// container o task semplice) il comportamento è IDENTICO a Fase 5/6a:
/// un solo span `[ninfo.inizio, ninfo.fine-propria]`.
/// `larghezza-area` (Fase 6b, review, C3): serve a clampare la tacca
/// "instant" (D5) in `[0, larghezza-area]` — senza il clamp, un
/// istante ESATTAMENTE sul bordo del viewport (es. `start == end ==
/// viewport.start`) allarga la tacca SIMMETRICAMENTE (`_tacca-
/// larghezza`) anche verso il lato NEGATIVO (fuori dall'area barre, che
/// non ha più `clip: true` per prestazioni, S1 leva 2): un pezzo della
/// tacca uscirebbe visivamente dall'area. Col clamp resta solo la metà
/// visibile (la metà che cade davvero dentro l'area), mai invertita
/// (`calc.max`/`calc.min` non possono scambiare `start`/`end` perché il
/// punto vero è sempre dentro `[0, larghezza-area]` per costruzione di
/// `x()`, solo l'ALLARGAMENTO può uscirne).
#let segmenti-stato(ninfo, clamp, x, stile-barra, larghezza-area) = {
  if ninfo == none or clamp == none or clamp.outside {
    return (stato: "none", segmenti: ())
  }
  // `intervallo` (RF11, Fase 6b): l'intervallo PROPRIO (English:
  // `start`/`end`/`done`, interfaccia drawer) da cui il segmento viene,
  // passato a valle come `seg.interval` del drawer `bar` — dati puri,
  // mai usati dal default (che legge solo `width`/`height`/`style`),
  // utili a un drawer custom che vuole le date vere del tratto.
  // Suggerimento (review di Fase 6b): `altezza` (= `voce.height`) già
  // calcolata QUI per la soglia della tacca — esposta nel segmento
  // cosi' `render/bars.typ` non la ricalcola una seconda volta
  // (prima: `voce-stile(..)` chiamata di nuovo lì, stesso risultato,
  // RNF5 "calcola una volta").
  // Residuo DS-1 (revisione del 2026-10-17 su 6b/6c, BUG CHIUSO): un
  // segmento grezzo con larghezza vera zero (`s.end == s.start`, DOPO
  // il clamp di `x()`) merita la tacca "instant" (D5) SOLO se e' un
  // istante VERO (le due date che lo delimitano, "data-inizio"/
  // "data-fine" di `_stato-span`, COINCIDONO: per costruzione la loro
  // `x()` da' sempre lo stesso punto, a prescindere dal clamp). Se le
  // due date sono DIVERSE, la larghezza zero e' un artefatto del clamp
  // — l'intero segmento cade da UN SOLO lato del viewport (es. "body"
  // `[1gen,5gen]` con viewport che parte l'8gen: `x()` clampa ENTRAMBI
  // gli estremi a 0pt) — e va SCARTATO, non allargato: altrimenti
  // `_tacca-larghezza` disegnerebbe un quadratino FANTASMA sul bordo,
  // per un evento che non e' davvero lì. **Nota:** questo NON usa
  // `clamp.start`/`.end` (quelli sono lo span clampato del NODO, non il
  // viewport — per un nodo interamente contenuto nel viewport
  // coincidono con le date proprie del nodo, come dimostrato da un
  // falso positivo su un istante vero durante lo sviluppo: controprova
  // registrata nel verbale) — solo le due date del segmento, confronto
  // locale e indipendente dal clamp del nodo.
  let con-tacca(stato, grezzi, intervallo) = grezzi
    .filter(s => s.end - s.start > 0pt or s.at("data-inizio") == s.at("data-fine"))
    .map(s => {
      let voce = voce-stile(stile-barra, stato, s.parte)
      let t = _tacca-larghezza(s.start, s.end, voce.height)
      (
        start: clampa(t.start, 0pt, larghezza-area), end: clampa(t.end, 0pt, larghezza-area),
        parte: s.parte, stato: stato, intervallo: intervallo, altezza: voce.height,
      )
    })
  if ninfo.intervalli.len() > 1 {
    // C3 (review di Fase 6b, BUG CHIUSO): un intervallo INTERAMENTE
    // fuori dal viewport va ESCLUSO prima del `map` — `clamp.outside`
    // (sopra) guarda solo lo SPAN AGGREGATO del nodo (`n.inizio`/
    // `n.fine`, il min/max su TUTTI gli intervalli): con due intervalli
    // ai due lati OPPOSTI del viewport (es. 1-5 gennaio e 25-28
    // gennaio, viewport 8-20), lo span aggregato (1-28 gennaio)
    // ATTRAVERSA comunque il viewport (`clamp.outside = false`, il
    // nodo NON è saltato), anche se NESSUNO dei due intervalli tocca
    // davvero il viewport — senza questo filtro, ciascuno viene
    // comunque mappato da `x()` (che clampa al bordo più vicino) e,
    // se sotto la soglia della tacca, disegnerebbe un quadratino
    // FANTASMA esattamente sul bordo, segnalando un evento che non è
    // lì. Confronto sulle DATE (coerente con `clamp.start`/`.end`,
    // popolate da `timeline.typ` con `viewport.start`/`.end`, mai pt):
    // semiaperto `[clamp.start, clamp.end)` sul lato sinistro,
    // `(clamp.start, clamp.end]` sul destro — `fine-visiva > clamp.start`
    // esclude un intervallo che finisce ESATTAMENTE su `clamp.start`
    // (nessuna sovrapposizione, un punto di confine non è un giorno
    // visibile), `inizio < clamp.end` esclude uno che inizia
    // ESATTAMENTE su `clamp.end` (fuori per lo stesso motivo).
    // DS-1 (revisione del 2026-10-17 su 6b/6c, BUG CHIUSO): il
    // confronto usava SOLO `iv.fine-eff`, scartando un intervallo
    // completato IN RITARDO (`done-eff` > `fine-eff`) la cui coda di
    // ritardo cade dentro il viewport anche se la sua `fine-eff` è
    // fuori (es. 1-5 gennaio con `done` 10 gennaio, viewport 8-20:
    // `fine-eff`=5gen è fuori, ma la coda arriva fino al 10gen, dentro
    // — `casi-c3-done-tardivo`, `unit/layout.typ`). `fine-visiva` è il
    // maggiore fra `fine-eff` e `done-eff` dell'intervallo (stessa
    // formula di `timeline._raccogli`, `f.fine = max(f.fine-propria,
    // f.done-eff)`, D-111/S2, qui applicata al singolo INTERVALLO
    // invece che al nodo).
    let intervalli-visibili = ninfo.intervalli.filter(iv => {
      let done-eff = iv.at("done-eff")
      let fine-eff = iv.at("fine-eff")
      let fine-visiva = if done-eff != none and done-eff > fine-eff { done-eff } else { fine-eff }
      fine-visiva > clamp.start and iv.inizio < clamp.end
    })
    let segmenti = intervalli-visibili
      .map(iv => {
        // S2: per un INTERVALLO (non un container) `fine` e
        // `fine-propria` coincidono sempre (nessuna estensione
        // possibile "da sotto": l'eventuale `done` tardivo è
        // quello STESSO intervallo, già gestito dal ramo
        // `completed-late` sotto, non da una terza fonte).
        let (stato-iv, grezzi) = _stato-span(iv.inizio, iv.at("fine-eff"), iv.at("fine-eff"), iv.at("done-eff"), x)
        con-tacca(stato-iv, grezzi, (start: iv.inizio, end: iv.at("fine-eff"), done: iv.at("done-eff")))
      })
      .flatten()
    // Stato aggregato del NODO (stesso calcolo di sempre, sul suo span
    // complessivo): solo per il campo di primo livello (sopra), mai per
    // i segmenti (già taggati per-intervallo sopra). "none" se il
    // filtro sopra ha escluso TUTTI gli intervalli (nessun segmento da
    // disegnare, anche se lo span aggregato attraversa il viewport).
    if segmenti.len() == 0 {
      return (stato: "none", segmenti: ())
    }
    let (stato-agg, ..) = _stato-span(ninfo.inizio, ninfo.at("fine-propria"), ninfo.fine, ninfo.at("done-eff"), x)
    return (stato: stato-agg, segmenti: segmenti)
  }
  let (stato, grezzi) = _stato-span(ninfo.inizio, ninfo.at("fine-propria"), ninfo.fine, ninfo.at("done-eff"), x)
  (
    stato: stato,
    segmenti: con-tacca(stato, grezzi, (start: ninfo.inizio, end: ninfo.at("fine-propria"), done: ninfo.at("done-eff"))),
  )
}

// --- Albero appiattito in righe (document order, D-036) -----------------

/// Appiattisce l'albero validato (`spec.taskgroups`, SEMPRE il bosco
/// radice per entrambe le facce, DC-1) in un array di righe in ordine
/// di documento: una per nodo (gruppo o foglia, con o senza date).
/// Ricorsione bottom-up semplice (un `map`+`flatten` per livello,
/// profondità limitata da E-VAL-13: T19, come `timeline._raccogli`).
/// Ogni riga porta: `path, kind, level, name, color, style, id`, più
/// `ninfo` (`modello.nodi.at(path)`, `none` se il nodo non ha date),
/// `clamp` (`modello.clamp.at(path)`, `none` se non ha date) e
/// `"task-style-ereditato"` (RF2/RF5, D-102, Fase 6a): il `task-style`
/// del più vicino ANTENATO che lo dichiara non vuoto -- MAI quello del
/// nodo stesso, che si applica solo ai discendenti ("ereditato da tutti
/// i discendenti"). `task-style-ricevuto` è l'eredità del genitore;
/// `(:)` alla radice (nessun antenato).
#let _appiattisci(nodo, modello, task-style-ricevuto) = {
  let figli-nodo = figli(nodo)
  let qui = (
    path: nodo.path, kind: nodo.at("kind", default: "node"), level: nodo.level,
    name: nodo.name, color: nodo.at("color", default: none),
    style: nodo.at("style", default: (:)), id: nodo.at("id", default: none),
    // RF2/RF11 (P10): dizionario libero dell'utente, passato intatto
    // ai drawer (`node.x`, `ok/drawers-x.typ`) — mai letto dal core.
    x: nodo.at("x", default: none),
    ninfo: modello.nodi.at(nodo.path, default: none),
    clamp: modello.clamp.at(nodo.path, default: none),
    "task-style-ereditato": task-style-ricevuto,
    // RF11/Fase 6b: esposto al drawer `label` come `node.has-children`
    // (D-089/D-104: un nodo con date proprie PUÒ avere figli datati
    // visitati separatamente — questo flag dice solo "ha figli
    // nell'albero", non "aggrega i figli").
    "has-children": figli-nodo.len() > 0,
  )
  let task-style-proprio = nodo.at("task-style", default: (:))
  let task-style-per-figli = if task-style-proprio.len() > 0 { task-style-proprio } else { task-style-ricevuto }
  (qui,) + figli-nodo.map(f => _appiattisci(f, modello, task-style-per-figli)).flatten()
}

/// Righe di TUTTO il bosco, in ordine di documento (D-036). Ogni radice
/// parte senza alcun `task-style` ereditato (`(:)`, nessun antenato).
#let righe(spec, modello) = spec.taskgroups.map(r => _appiattisci(r, modello, (:))).flatten()

/// Altezza di contenuto (SENZA `row-gap`) di una riga, secondo
/// `style.levels[level].bar` (se non vuoto, RF5) con fallback su
/// `style.groups`/`style.tasks` per `kind` (D-102, Fase 6a:
/// `base-barra-livello`, che prima di Fase 6a era `base-barra` senza
/// distinzione di livello). `altezza-nome-sidebar` (R5, D-108): `none`
/// nel caso comune (nessuna crescita, risultato IDENTICO a prima — il
/// chiamante lo passa `none` tranne quando il nome della sidebar è
/// andato a capo, `nome-sidebar-pronto` sopra) oppure l'altezza
/// misurata del nome a capo, che FA CRESCERE la riga se eccede
/// l'altezza della barra (`calc.max`, mai il contrario: la barra non
/// si restringe mai per un nome corto).
#let altezza-contenuto(kind, level, stile-risolto, altezza-nome-sidebar: none) = {
  let base = style.base-barra-livello(stile-risolto, kind, level).uncompleted.height
  if altezza-nome-sidebar == none { base } else { calc.max(base, altezza-nome-sidebar) }
}

/// Altezza TOTALE di una riga (contenuto + `row-gap`, "la riga include
/// il proprio `row-gap`", RF10): metà sopra, metà sotto — qui basta il
/// totale, il render lo userà come padding simmetrico. `altezza-nome-
/// sidebar`: vedi `altezza-contenuto` sopra (R5, D-108).
#let altezza-riga(kind, level, stile-risolto, altezza-nome-sidebar: none) = (
  altezza-contenuto(kind, level, stile-risolto, altezza-nome-sidebar: altezza-nome-sidebar) + stile-risolto.layout.row-gap
)

/// Posiziona TUTTE le righe lungo l'asse y (pt, cumulativo, origine 0 =
/// cima della PRIMA riga del corpo, non la pagina: gli elementi
/// continui — griglie, milestone, frecce — attraversano solo le righe
/// del corpo, non l'intestazione che si ripete, RF12). Aggiunge `y0`,
/// `y1` (fascia semiaperta `[y0,y1)`, RF10) e `altezza` a ogni riga.
/// `righe-piatte` può portare `altezza-nome-capo` (R5, D-108: l'altezza
/// del nome andato a capo, se presente — `.at(default: none)` per i
/// chiamanti/test che non lo impostano, stesso risultato di prima).
#let posiziona(righe-piatte, stile-risolto) = {
  // `for` semplice (T25: una CHIUSURA — `.map(..)` — non può mutare una
  // variabile catturata dall'esterno; un `for` nel corpo della funzione
  // sì, come altrove nel progetto, es. `timeline.calcola`).
  let y = 0pt
  let risultato = ()
  for r in righe-piatte {
    let h = altezza-riga(r.kind, r.level, stile-risolto, altezza-nome-sidebar: r.at("altezza-nome-capo", default: none))
    let y0 = y
    y += h
    risultato.push(r + (y0: y0, y1: y, altezza: h))
  }
  risultato
}

/// Stroke della griglia verticale di un'unità (RF5: `style.gridlines.
/// <nome>s` è un dizionario `(stroke: ...)`, "come originale" — non uno
/// stroke nudo come `task-dividers`): se la chiave non è presente,
/// ricade su `gridline-definizione` (R6/RF4.3, D-102: il `gridline` di
/// DEFAULT della definizione d'unità, trasportato da
/// `timeline.calcola()` fino a qui — prima del fallback, era un residuo
/// di Fase 5 e il risultato era sempre `none` in quel caso). Catena
/// completa di RF4: `style.gridlines.<unit>s` > `gridline` della
/// definizione > `none` (il `none` arriva quando ANCHE la definizione
/// non ne dichiara uno, come tutte le unità incluse).
#let gridline-unita(stile-risolto, nome-unita, gridline-definizione) = {
  let voce = stile-risolto.gridlines.at(nome-unita + "s", default: none)
  // `.at("stroke", default: ..)` (suggerimento S1, 4a rereview, code-
  // reviewer): non un `.at("stroke")` nudo — per un'unità CUSTOM
  // (RF4.3, nessun default in `default-style.gridlines` con cui
  // fondersi) un dizionario `{}` scritto dall'utente (es. `style.
  // gridlines.semesters: {}`, o un'altra chiave senza `stroke`)
  // arriverebbe qui senza la chiave "stroke": panic "nudo" ("dizionario
  // non contiene la chiave"), invece di ricadere sul `gridline` della
  // definizione come fa già il caso "nessuna voce" (`voce == none`,
  // riga sopra) — stessa catena di RF4, stesso fallback.
  if voce != none { voce.at("stroke", default: gridline-definizione) } else { gridline-definizione }
}

// --- Intestazioni: segmenti in pt e soppressione min-cell-width (D-028) -

/// Larghezza (pt) di un segmento, già clampata dalla stessa `x()`.
#let _larghezza-segmento(seg, x) = x(seg.end) - x(seg.start)

/// Filtra/arricchisce i livelli d'intestazione di `timeline.calcola()`:
/// per ciascuno calcola i segmenti in pt (`x0,x1,label,partial,nascondi`)
/// e decide la soppressione DEL LIVELLO (D-028): confronta la **minima**
/// larghezza tra i segmenti NON tagliati con `"min-cell-width"` del
/// livello (se TUTTI sono tagliati, confronta la MASSIMA). Un livello
/// soppresso non produce riga (il chiamante lo riconosce da `soppresso:
/// true`, altezza 0 — nessuna riga né griglia, gli altri livelli non
/// cambiano). `nascondi` (RF4: `overflow` di default `"hide"`, "come
/// gantty") è per SINGOLO segmento: `true` se l'etichetta, misurata UNA
/// VOLTA (RNF3), è più larga della cella — a differenza di un `clip`
/// parziale (che mostrerebbe un'etichetta tagliata, illeggibile), il
/// default nasconde l'intera etichetta. `testo-stile` è lo stile del
/// testo da misurare (`style.headers.text`, Fase 5: niente stile per
/// livello, Fase 6a).
// RNF3 (rilievo del benchmark S5, D-097): misurare `measure()` UNA
// VOLTA PER SEGMENTO è troppo caro su un livello "day" di anni interi
// (migliaia di segmenti) — S5 (200 task/3 anni) superava la soglia di
// 0,5s. Le etichette dello STESSO livello ripetono pochissime stringhe
// distinte (es. "day" "number": al più 31; "month" "short": 12): una
// cache per TESTO (non per segmento) dentro ogni livello riporta le
// chiamate a `measure()` da "una per segmento" a "una per etichetta
// distinta", lo stesso principio "calcola una volta" applicato alla
// granularità giusta. `repr(label)` funziona sia per `str` sia per
// `content` (le funzioni di formato custom, RF4.3, possono restituire
// content).
/// Intestazioni grezze (un livello può risultare soppresso, PRIMA della
/// guardia E6 sotto che garantisce almeno un asse visibile). `stile-
/// risolto`/`locale` servono a risolvere, UNA VOLTA per livello (RNF3,
/// `style.livello-header-risolto`, D-106 punto (a) — "headers a
/// dizionario per livello"): allineamento, testo, `fill`, `stroke`,
/// `gridline`, altezza, `overflow` e inset — con la precedenza
/// `headers[i].<campo>` > `style.headers.<unità>.<campo>` > definizione
/// d'unità (dove esiste) > default globale.
#let _intestazioni-grezze(headers-timeline, x, stile-risolto, locale) = headers-timeline.enumerate().map(((indice-livello, livello)) => {
  let stile-liv = style.livello-header-risolto(stile-risolto, livello.unit, livello, locale)
  // Margine ORIZZONTALE totale richiesto dall'etichetta oltre al testo
  // vero: `left + right` dell'inset risolto — con `center` è `0pt`
  // (D-103 "con center nessun effetto"); con `left`/`right` è il solo
  // lato toccato (mai la somma dei due, perché solo UN lato riceve il
  // `pad`).
  let margine-inset = stile-liv.inset.left + stile-liv.inset.right
  // D-106 punto (a): `overflow` per livello — "hide" (default, come
  // prima) nasconde un'etichetta che non ci sta; "clip"/"show" non
  // nascondono MAI (la differenza fra i due è nel RENDER: "clip"
  // ritaglia il testo alla cella, "show" lo lascia traboccare, vedi
  // `render/headers.typ`).
  //
  // Fase 6c (rilievo del benchmark S5, review): il blocco sopra (L1,
  // D-105/D-097) aveva già riportato `measure()` da "una per segmento" a
  // "una per etichetta distinta" — ma su un livello "day" di anni interi
  // (~1000 segmenti in S5) restava comunque un costo per OGNI segmento
  // (due chiamate a `x()`, `repr(label)`+lookup di cache, costruzione di
  // un dizionario a 9 chiavi con `dates.giorni-fra` — RF11/D-113) anche
  // quando quel livello risulta POI soppresso (nessun consumatore legge
  // mai le etichette/i campi RF11 di un livello soppresso, confermato:
  // `render/headers.typ`/`gruppi-griglia` filtrano entrambi su `not
  // soppresso`). Qui si calcola PRIMA, per OGNI segmento, solo il minimo
  // che serve alla decisione di soppressione per LARGHEZZA (`x0`/`x1`/
  // `partial`, nessun `measure()`/`dates.giorni-fra`): se il livello
  // risulta già soppresso per `min-cell-width` su questa sola base, E
  // non è il livello 0 (la guardia **E6**, sotto in `intestazioni()`,
  // forza a visibile SOLO il PRIMO livello quando tutti risultano
  // soppressi — mai un livello successivo, verificato da `casi-e6`/
  // `casi-a4-3anni-soppresso` in `unit/layout.typ`), si esce PRESTO:
  // nessuna etichetta misurata, nessun campo RF11 calcolato, perché
  // nessuno li leggerà mai. Il livello 0 (e qualunque livello NON già
  // soppresso per larghezza) prosegue invariato nel ramo pieno sotto —
  // stesso identico risultato di prima per OGNI segmento che un
  // consumatore può ancora leggere (RNF3: "calcola una volta", qui
  // "calcola solo se qualcuno lo userà").
  let segs-base = livello.segments.map(s => (x0: x(s.start), x1: x(s.end), partial: s.partial))
  let confronto-larghezza(lista) = {
    let non-tagliati = lista.filter(s => not s.partial)
    if non-tagliati.len() > 0 {
      calc.min(..non-tagliati.map(s => s.x1 - s.x0))
    } else if lista.len() > 0 {
      calc.max(..lista.map(s => s.x1 - s.x0))
    } else {
      0pt
    }
  }
  let soppresso-per-larghezza = confronto-larghezza(segs-base) < stile-liv.at("min-cell-width")
  // `gridline-finale`/`altezza-propria` (sotto) sono per-LIVELLO, non
  // per-segmento (un costo fisso, mai moltiplicato per il numero di
  // segmenti): calcolati SEMPRE, anche nel ramo rapido, cosi' un livello
  // soppresso che l'E6 di un ALTRO livello potrebbe comunque esporre
  // (per introspezione/debug) resta con un dizionario della stessa
  // forma di un livello pieno, mai un sotto-insieme di chiavi.
  let gridline-finale = if stile-liv.at("gridline-livello") != none {
    stile-liv.at("gridline-livello")
  } else {
    gridline-unita(stile-risolto, livello.unit, livello.at("gridline", default: none))
  }
  let altezza-propria = if stile-liv.height != auto {
    stile-liv.height
  } else {
    measure(text(..stile-liv.text)[0]).height + 2 * stile-risolto.layout.at("header-inset")
  }
  if indice-livello > 0 and soppresso-per-larghezza {
    return (
      unit: livello.unit,
      // `label`/`nascondi` restano con un valore NEUTRO (mai letti: un
      // livello soppresso non raggiunge mai `render/headers.typ`/
      // `gruppi-griglia`, entrambi filtrati su `not soppresso`) — la
      // FORMA del dizionario (stesse chiavi: `x0`, `x1`, `partial`,
      // `label`, `nascondi`; MAI `inizio`/`fine`/`giorni`/`indice`, gli
      // unici campi "solo per un livello visibile", RF11) resta
      // comunque riconoscibile da qualunque introspezione esterna.
      segments: segs-base.map(s => s + (label: none, nascondi: false)),
      soppresso: true,
      align: stile-liv.align, inset: stile-liv.inset, text: stile-liv.text,
      fill: stile-liv.fill, stroke: stile-liv.stroke, overflow: stile-liv.overflow,
      gridline: gridline-finale, altezza: altezza-propria,
    )
  }
  let cache-larghezze = (:)
  let segs-pt = ()
  for (indice, s) in livello.segments.enumerate() {
    // `x0`/`x1` RIUSATI da `segs-base` (sopra, calcolati UNA volta per
    // la decisione di soppressione per larghezza): nessuna seconda
    // chiamata a `x(s.start)`/`x(s.end)` per lo stesso segmento qui nel
    // ramo pieno (RNF5, Fase 6c).
    let x0 = segs-base.at(indice).x0
    let x1 = segs-base.at(indice).x1
    let chiave = repr(s.label)
    let larghezza-testo = if chiave in cache-larghezze {
      cache-larghezze.at(chiave)
    } else {
      let w = measure(text(..stile-liv.text)[#_contenuto(s.label)]).width
      cache-larghezze.insert(chiave, w)
      w
    }
    segs-pt.push((
      x0: x0, x1: x1, label: s.label, partial: s.partial,
      // Review di Fase 6b (specification.md RF11, D-113): `start`/`end`
      // (date VERE, non pt) e `"giorni"` (durata intera, T2) per il
      // drawer `header-cell` ("seg come RF4"), più l'indice del
      // segmento nel livello — tutti e tre ADDITIVI (RF11, interfaccia
      // versionata "solo aggiunte"): nessun consumatore esistente di
      // `segs-pt` legge l'insieme completo delle chiavi.
      inizio: s.start, fine: s.end, giorni: dates.giorni-fra(s.start, s.end), indice: indice,
      // D-103: la larghezza "richiesta" include l'inset orizzontale
      // (misurato una volta sopra, non per segmento) — un'etichetta che
      // ci sta appena per il testo nudo ma non per testo+inset deve
      // nascondersi (altrimenti toccherebbe il bordo della cella/la
      // griglia, il difetto visivo segnalato). Mai nascosta se
      // `overflow` non è "hide" (D-106 punto (a)).
      nascondi: stile-liv.overflow == "hide" and larghezza-testo + margine-inset > x1 - x0,
    ))
  }
  let confronto = confronto-larghezza(segs-pt)
  // D8 (rilievo del coordinatore, decisione di specifica da registrare
  // nel verbale): un livello con TUTTE le etichette nascoste per
  // overflow (`nascondi`, sopra) non mostra nessun testo — una riga
  // d'intestazione vuota (solo griglia) non porta informazione, stessa
  // conclusione pratica di `min-cell-width` ma per una causa diversa
  // (il TESTO non ci sta, non la CELLA): soppresso come se fosse sotto
  // soglia, niente riga né griglia. Un livello SENZA segmenti (caso
  // degenere, viewport vuoto) non è "tutto nascosto": resta soppresso
  // solo per `min-cell-width` come prima.
  let tutte-nascoste = segs-pt.len() > 0 and segs-pt.all(s => s.nascondi)
  let soppresso = confronto < stile-liv.at("min-cell-width") or tutte-nascoste
  // `gridline-finale`/`altezza-propria` (D-106 punto (a)): calcolate
  // SOPRA, prima del ramo rapido — stesso valore, nessun ricalcolo qui.
  (
    unit: livello.unit, segments: segs-pt, soppresso: soppresso,
    align: stile-liv.align, inset: stile-liv.inset, text: stile-liv.text,
    fill: stile-liv.fill, stroke: stile-liv.stroke, overflow: stile-liv.overflow,
    gridline: gridline-finale, altezza: altezza-propria,
  )
})

/// Ascisse di griglia VISIBILI per un livello (Fase 6b Parte B, ritocco
/// estetico, decisione utente; S5, review: CORRETTA per rispettare un
/// bordo sinistro disattivato dall'utente): i confini `x0` dei segmenti,
/// ESCLUSO `x = 0` SOLO se un bordo sinistro è comunque disegnato lì
/// (`bordo-sinistro-visibile`, lo `stroke` — `style.field.stroke` per
/// il corpo, `livello.stroke` per l'intestazione — che il CHIAMANTE già
/// conosce: questa funzione non lo ricalcola, RNF5) — a `x0 = 0` la
/// griglia coinciderebbe altrimenti col bordo (D2/R4, doppia linea più
/// scura). **Se il bordo è `none`** (l'utente ha disattivato
/// `field.stroke`/`livello.stroke`), il confine a `x = 0` resta l'UNICO
/// segno visibile del limite sinistro e va mantenuto: toglierlo
/// lascerebbe quel lato senza nessuna linea. Un solo segmento può avere
/// `x0 == 0pt` per costruzione (il primo, "pieno" o "partial" — un
/// segmento tagliato all'inizio del viewport ha comunque `x0` clampato
/// a `0pt`): il filtro per VALORE (non per indice) resta corretto anche
/// su un livello senza segmenti. Usata sia da `gruppi-griglia` sotto
/// (griglia del corpo, con `style.field.stroke`) sia da
/// `render/headers.typ` (griglia del livello, con `livello.stroke` —
/// PUÒ differire dal corpo, per livello) — stessa funzione pura,
/// nessuna duplicazione della regola.
#let griglia-visibile(segments, bordo-sinistro-visibile) = {
  let xs = segments.map(s => s.x0)
  if bordo-sinistro-visibile { xs.filter(v => v != 0pt) } else { xs }
}

/// Intestazioni (D-028, RNF3): wrapper di `_intestazioni-grezze` con la
/// guardia **E6** (rilievo del coordinatore, rereview di Fase 5,
/// decisione di specifica da registrare nel verbale): se TUTTI i
/// livelli risultano soppressi (min-cell-width o "tutte nascoste",
/// sopra) il grafico resterebbe SENZA alcun asse del tempo —
/// illeggibile, peggio che un livello troppo fitto. Si conserva almeno
/// il livello PIÙ GROSSOLANO (il PRIMO dell'array: `headers` è sempre
/// ordinato dal più grossolano al più fine, RF4 "year, month, day" —
/// convenzione del CHIAMANTE, non verificata qui, perché `layout.typ`
/// non conosce la semantica delle unità). Un array di un solo livello
/// (il caso minimo, sempre presente) non viene mai "forzato" se
/// quell'unico livello non era soppresso: il ramo sotto si attiva SOLO
/// quando `.all(soppresso)` è vero.
#let intestazioni(headers-timeline, x, stile-risolto, locale) = {
  let livelli = _intestazioni-grezze(headers-timeline, x, stile-risolto, locale)
  if livelli.len() > 0 and livelli.all(l => l.soppresso) {
    livelli = (livelli.at(0) + (soppresso: false),) + livelli.slice(1)
  }
  livelli
}

// --- Anti-collisione ed etichette di milestone/oggi (RF3/RNF6/D-038) ---
// Skill `etichette-grafico-geometria-pura`: il passo/la collocazione si
// derivano dalla geometria (larghezza misurata UNA VOLTA per etichetta,
// RNF3), mai da un diradamento automatico; l'ordine (per data) si
// conserva sempre; nessuna etichetta scompare (RNF6: "nessun elemento
// oltre i bordi" si ottiene con un CLAMP della posizione, non
// togliendo etichette).

/// Un'etichetta "candidata" (milestone o "oggi"): `x` (centro,
/// pt), `larghezza` (pt, già misurata dal chiamante). Il chiamante
/// porta anche un campo proprio invariato (es. `"x-segno"`, la
/// posizione ESATTA del segno verticale: RF2/RNF6 "il segno... sempre
/// alla posizione esatta") che questa funzione non tocca.
// Margine minimo fra due etichette ADIACENTI sullo STESSO livello
// (oltre al semplice "non sovrapposte"): senza un distacco visibile due
// etichette appena non sovrapposte si toccano bordo a bordo, difficili
// da distinguere a occhio (rilievo della revisione visiva di Fase 5).
// Piccolo e fisso, non proporzionale (RNF6: resta trascurabile anche su
// aree strette). **Rilievo del coordinatore (rereview di Fase 5):** non
// più una costante cablata nel modulo — parametro `margine:` di
// `impila-e-clampa` sotto (default `3pt` per chi chiama la funzione
// direttamente, come i test unitari); `calcola()` passa invece
// `style.milestones.label.gap` (chiave di stile, personalizzabile).

/// Pre-clamp di UNA candidata nei limiti `[larghezza/2, W - larghezza/2]`
/// (C1, rilievo del coordinatore): fatto PRIMA dell'assegnazione dei
/// livelli (sotto), non dopo. Caso degenere (etichetta più larga
/// dell'intera area, `larghezza >= larghezza-totale`, min > max nel
/// clamp normale): centrata su `larghezza-totale / 2`, l'unico punto
/// sensato quando non può comunque stare tutta dentro.
/// **Nota aggiornata (rereview di Fase 5, D-099+1): questo ramo NON è
/// più irraggiungibile.** Il commento precedente (D-098) lo diceva
/// intercettato PRIMA da un ramo di "affollamento estremo" con
/// distribuzione uniforme — quel ramo è stato RIMOSSO in D-099
/// (sostituito dall'impilamento su più livelli): oggi una sola
/// candidata più larga dell'intera area arriva qui direttamente e viene
/// centrata (`casi-troppo-larga`, `unit/layout.typ`, verificato A MANO).
/// **Conseguenza accettata (RNF6 "best effort", da registrare nel
/// verbale):** in questo caso degenere il rettangolo dell'etichetta
/// ECCEDE comunque il footer su entrambi i lati (nessuna larghezza può
/// contenerla tutta); l'anti-collisione non ridimensiona mai il testo
/// (skill `etichette-grafico-geometria-pura`, "nessuna etichetta si
/// restringe"), quindi lo sbordo orizzontale in questo caso estremo è
/// una conseguenza voluta, non un difetto da correggere qui.
#let _preclampa-candidata(c, larghezza-totale) = {
  let semi = c.larghezza / 2
  if 2 * semi >= larghezza-totale {
    c + (x: larghezza-totale / 2)
  } else {
    c + (x: clampa(c.x, semi, larghezza-totale - semi))
  }
}

/// Impila su PIÙ LIVELLI (rilievo del coordinatore, seconda revisione
/// di Fase 5: "impilale su più righe, come l'originale" — P20,
/// `gantty` 0.1 spingeva verso il basso le etichette che collidevano).
/// A differenza della versione precedente (che spostava l'ascissa `x`
/// per fare spazio, fino a un ripiego a slot uniformi quando non c'era
/// abbastanza spazio su una riga sola — ripiego che, con molte
/// etichette ravvicinate, le faceva TOCCARE comunque, perché lo slot
/// poteva essere più stretto della larghezza vera di un'etichetta): qui
/// `x` NON si sposta mai oltre il pre-clamp (C1, sotto) — una
/// collisione orizzontale si risolve assegnando l'etichetta al PRIMO
/// LIVELLO (riga) libero per geometria pura (nessun ciclo con EPSILON,
/// skill `etichette-grafico-geometria-pura`): un algoritmo greedy di
/// "interval scheduling" su un array ordinato per data/`x`, O(livelli)
/// per etichetta. Nessuna etichetta sparisce MAI, qualunque sia la
/// densità — l'altezza del footer cresce con `livelli-usati()`
/// (sotto), non le etichette si restringono o si sovrappongono.
/// Restituisce lo stesso array con `x` (pre-clampata) e un nuovo campo
/// `livello` (intero, 0-based). `margine`: distacco minimo richiesto fra
/// due etichette adiacenti sullo stesso livello (default `3pt`, usato
/// dai test che chiamano questa funzione direttamente; `calcola()` sotto
/// passa invece `style.milestones.label.gap`, rilievo del coordinatore,
/// rereview di Fase 5 — niente più costante cablata nel modulo).
#let impila-e-clampa(candidate, larghezza-totale, margine: 3pt) = {
  if candidate.len() == 0 { return () }
  // Pre-clamp (C1): OGNI candidata dentro i bordi, prima candidata
  // inclusa (la versione precedente a D-097 clampava solo l'ultima,
  // lasciando la prima fuori bordo se la sua `x` originale lo era).
  let preclampate = candidate.map(c => _preclampa-candidata(c, larghezza-totale))
  // Assegnazione dei livelli: `bordi.at(i)` = bordo destro (pt) occupato
  // finora sul livello `i`. Per ogni candidata, NELL'ORDINE DATO dal
  // chiamante: il PRIMO livello il cui bordo destro, più il margine,
  // sta a sinistra del bordo sinistro della candidata; altrimenti un
  // livello NUOVO. `for` semplice (non `.map()`/chiusura): `bordi`/`out`
  // sono locali a QUESTA chiamata, la mutazione è ammessa (T25 riguarda
  // solo le variabili catturate da una chiusura esterna).
  // **Nota corretta (rereview di Fase 5, D-099+1): l'ordine per `x` NON
  // è garantito strettamente monotono** col solo ordinamento per data
  // del chiamante, nemmeno dopo il pre-clamp — vicino a un bordo, due
  // candidate con date crescenti ma LARGHEZZE molto diverse possono
  // finire con `x` pre-clampate in ordine INVERTITO (es.: una candidata
  // larga vicina al bordo sinistro si pre-clampa a `larghezza/2`, più a
  // destra di una candidata STRETTA di data successiva che non tocca
  // affatto il bordo e resta alla sua `x` originale, più piccola).
  // **Questo non rompe l'invariante "nessuna sovrapposizione per
  // livello"** (verificato: il controllo `sinistra >= bordi.at(i) +
  // margine` confronta sempre la geometria VERA della candidata corrente
  // contro quella già occupata su ogni livello, indipendentemente
  // dall'ordine di arrivo — non assume input ordinato) — può solo
  // rendere l'assegnazione meno OTTIMALE (qualche livello in più del
  // minimo teorico) in questo caso limite, mai sovrapposta (test
  // dedicato `casi-ordine-non-monotono`, `unit/layout.typ`).
  let bordi = ()
  let out = ()
  for c in preclampate {
    let sinistra = c.x - c.larghezza / 2
    let destra = c.x + c.larghezza / 2
    let livello = none
    for i in range(0, bordi.len()) {
      if sinistra >= bordi.at(i) + margine {
        livello = i
        break
      }
    }
    if livello == none {
      livello = bordi.len()
      bordi.push(destra)
    } else {
      bordi.at(livello) = destra
    }
    out.push(c + (livello: livello))
  }
  out
}

/// Numero di livelli usati da un array già impilato con
/// `impila-e-clampa` (massimo `livello` più uno; `1` se l'array è
/// vuoto, così l'altezza del footer non crolla a zero quando non c'è
/// nessuna etichetta — il chiamante gestisce comunque il caso "nessuna
/// etichetta" a parte, non disegnando il footer affatto).
#let livelli-usati(etichette) = if etichette.len() == 0 {
  1
} else {
  calc.max(..etichette.map(e => e.livello)) + 1
}

/// Geometria del connettore a gomito fra il segno verticale ESATTO di
/// un'etichetta (`"x-segno"`, RNF6) e la sua posizione effettiva dopo
/// `impila-e-clampa` (`x`, `livello`) — rilievo del coordinatore,
/// rereview di Fase 5: spostata in `layout.typ` (prima viveva, non
/// testata a mano, in `render/milestones.typ`) cosi' è "testabile a
/// mano" come il resto della geometria pura (skill
/// `etichette-grafico-geometria-pura`). Restituisce `none` (nessun
/// connettore da disegnare) o un array di punti `(x, y)` che il render
/// unisce con `curve.move`/`curve.line`, SENZA ricalcolare nulla.
///
/// **Criterio "spostata" (geometrico puro, non un numero a caso):** il
/// gomito orizzontale serve SOLO se il segno (`x-segno`) cade FUORI dal
/// rettangolo del TESTO dell'etichetta (non del box con l'`inset`): la
/// metà larghezza del testo è `larghezza/2 - inset` (`larghezza`
/// include già `2 × inset`, vedi `calcola()`); `soglia-extra`
/// (`style.milestones.label.connector-threshold`, default `0pt`) è un
/// margine AGGIUNTIVO opzionale, oltre il quale si preferisce ancora la
/// sola verticale anche se il segno è appena fuori dal testo puro —
/// `0pt` lascia il criterio puramente geometrico. Se non "spostata": una
/// riga VERTICALE da `x-segno` fino alla cima del livello assegnato (il
/// segno entra visivamente nell'etichetta, nessun uncino — risolve
/// l'"uncino di M1" della revisione visiva: con `x-segno = 0pt` e
/// un'etichetta pre-clampata al centro del suo semi-larghezza, il segno
/// resta comunque dentro il rettangolo del testo per qualunque
/// etichetta non eccezionalmente stretta). Se il livello è `0` E non è
/// "spostata": nessun connettore (il segno tocca già direttamente la
/// cima dell'etichetta, `overhang` basta). Se
/// "spostata": un tratto comune verticale fino a metà `overhang` (i
/// connettori restano paralleli nella parte alta), poi un gomito
/// orizzontale fino al centro dell'etichetta, poi verticale fino alla
/// cima del suo livello.
#let connettore-punti(e, overhang, altezza-livello, inset, soglia-extra) = {
  let dx = e.x - e.at("x-segno")
  let meta-testo = e.larghezza / 2 - inset
  let soglia = meta-testo + soglia-extra
  let spostata = calc.abs(dx) > soglia
  let cima-livello = overhang + e.livello * altezza-livello
  if e.livello == 0 and not spostata { return none }
  if spostata {
    let meta-overhang = overhang / 2
    (
      (x: e.at("x-segno"), y: 0pt), (x: e.at("x-segno"), y: meta-overhang),
      (x: e.x, y: meta-overhang), (x: e.x, y: cima-livello),
    )
  } else {
    ((x: e.at("x-segno"), y: 0pt), (x: e.at("x-segno"), y: cima-livello))
  }
}

// --- Routing delle dipendenze (RF10, D-037, D-039) ----------------------

/// Segno dell'offset secondo il tipo di aggancio: `+1` per "fine"
/// (uscita/ingresso a destra), `-1` per "inizio" (a sinistra) — stessa
/// convenzione per uscita, rientro (ortogonale) e tangente (bezier).
#let _segno(tipo) = if tipo == "fine" { 1 } else { -1 }

/// Punto con ascissa clampata in `[0, larghezza]` (RNF6). Usato per gli
/// AGGANCI (primo/ultimo punto di una polilinea, D-039): toccano
/// davvero il bordo della barra, quindi restano liberi di arrivare
/// fino al bordo dell'area (`0pt`/`larghezza`).
#let _clampa-punto(p, larghezza) = (x: clampa(p.x, 0pt, larghezza), y: p.y)

/// Punto "di SVOLTA" (non un aggancio: un tratto di passaggio interno
/// alla polilinea, es. `x-uscita`/`x-rientro` di `instrada-ortogonale`
/// o un punto campionato di `instrada-bezier`) con ascissa clampata in
/// `[margine, larghezza - margine]` (G2, revisione del 2026-10-18,
/// D-121/D-123): evita che una verticale "di passaggio" coincida
/// ESATTAMENTE col bordo dell'area barre (visivamente indistinguibile
/// da `style.field.stroke`) senza toccare gli agganci veri, che restano
/// `_clampa-punto` sopra. **Area troppo stretta** (`larghezza <= 2 *
/// margine`): il margine collasserebbe l'intervallo a vuoto o
/// invertito, quindi si ricade sullo stesso `[0, larghezza]` degli
/// agganci (comportamento di oggi) invece di un intervallo
/// matematicamente scorretto — nessun `panic`, nessun punto fuori
/// dall'area. **Margine NEGATIVO** (SUGGERIMENTO 5, rereview di Fase 7
/// del 2026-10-18): `style.dependencies.margin` non ha una validazione
/// dedicata (stessa convenzione di `gap`/`curviness`/`bulge`/`arrow`,
/// `style.typ`) — un valore negativo scritto a mano darebbe un `bordo`
/// negativo, quindi un clamp in `[negativo, larghezza + positivo]`:
/// punti FUORI dall'area, RNF6 violato IN SILENZIO (nessun `panic` che
/// lo segnali). Un margine negativo non ha un significato geometrico
/// ("quanto staccarsi dal bordo" non può essere "quanto entrarci
/// dentro"): trattato come `0pt`.
#let _clampa-punto-interno(p, larghezza, margine) = {
  let margine-valido = calc.max(margine, 0pt)
  let bordo = if larghezza > 2 * margine-valido { margine-valido } else { 0pt }
  (x: clampa(p.x, bordo, larghezza - bordo), y: p.y)
}

/// Clampa una POLILINEA intera (array di punti, D-037): i due ESTREMI
/// (indice `0` e l'ultimo — gli agganci veri) con `_clampa-punto`
/// `[0, larghezza]`; i punti INTERNI (di svolta) con
/// `_clampa-punto-interno` `[margine, larghezza - margine]` (G2,
/// D-123). `margine: 0pt` (il default di ogni `instrada-*` sotto, per
/// restare retrocompatibile con le chiamate esistenti che non lo
/// specificano) fa collassare `_clampa-punto-interno` sullo stesso
/// `[0, larghezza]` degli estremi: stesso risultato di prima di questa
/// modifica, verificato a mano (i test unitari NON aggiornati per G2
/// continuano a passare perché non specificano `margine-bordo`).
#let _clampa-polilinea(punti, larghezza, margine) = {
  let ultimo = punti.len() - 1
  punti.enumerate().map(((i, p)) => {
    if i == 0 or i == ultimo { _clampa-punto(p, larghezza) } else { _clampa-punto-interno(p, larghezza, margine) }
  })
}

/// **RILIEVO 2 (rereview di Fase 7, 2026-10-18): BUG CHIUSO.** Dopo il
/// clamp con margine (G2, sopra), una colonna di svolta può cadere dal
/// lato SBAGLIATO del proprio aggancio quando l'aggancio stesso è entro
/// `margine` dal bordo — RF10 impone "entrata in un'«inizio» da
/// sinistra, in una «fine» da destra" (e simmetricamente per l'uscita):
/// con un aggancio «fine» ESATTAMENTE sul bordo destro (`a.x == w`), il
/// punto di rientro grezzo (`x-rientro = a.x + g`, anch'esso oltre `w`)
/// si clampava a `w - margine` — che finisce a SINISTRA di `a.x` se
/// `a.x > w - margine`, cioè l'ultimo tratto entra da SINISTRA invece
/// che da destra: `_direzione-finale` orienta la punta su un verso
/// invertito rispetto alla regola. Corretto COSÌ, senza toccare
/// l'algoritmo di routing: se il punto adiacente all'aggancio (indice
/// `i1`) è dal lato sbagliato (`(punti.at(i1).x - riferimento) *
/// _segno(tipo) < 0`), la colonna intera (`i1` **e** `i2`, la stessa
/// ascissa) si porta ESATTAMENTE sull'aggancio — **S1 (rereview del
/// 2026-10-18, precisazione):** la colonna COLLASSA sull'aggancio, ma
/// questo coincide col comportamento di PRIMA di G2 **solo quando
/// l'aggancio è ESATTAMENTE sul bordo** (`a.x == 0` o `a.x == w`): per
/// `w − margine < a.x < w` (aggancio vicino ma non AL bordo) il clamp
/// di PRIMA di G2 avrebbe dato la colonna esattamente a `w` più un
/// moncone ORIZZONTALE residuo fino ad `a.x` — qui invece la colonna va
/// dritta su `a.x`, un risultato DIVERSO (migliore: nessun moncone),
/// non lo stesso di prima. `_direzione-finale` risale al tratto
/// precedente quando il tratto collassato è l'ultimo (sempre non
/// degenere qui perché l'aggancio OPPOSTO non è anch'esso sul bordo in
/// questi casi). **S5 (precondizione):** `i1`/`i2` devono essere
/// indici VALIDI di `punti` (i chiamanti sotto passano sempre array di
/// 4, 6 o 17 punti — 3/5 tratti ortogonali o 16 tratti bezier — con
/// `i1`/`i2` calcolati dalla loro stessa lunghezza, mai da una costante
/// indipendente); `i1 == i2` (bezier, un solo punto campionato) è
/// ammesso: la seconda scrittura è ridondante ma innocua.
///
/// **D-148 (decisione dell'utente del 2026-10-23, Opzione B — backlog
/// 10.x/D-126): la verticale di rientro collassata esattamente sul
/// bordo si confondeva con `style.field.stroke` (stesso nero di
/// default) — invisibile, si vedeva solo la punta.** `i-aggancio:`/
/// `inset:` (entrambi opzionali, default `none`/`0pt` = comportamento
/// INVARIATO, usato da tutte le chiamate che non li specificano:
/// l'uscita in `instrada-ortogonale` sotto, e `instrada-bezier`) — se
/// presenti E la correzione scatta (stesso ramo "lato sbagliato" di
/// sopra), la colonna **e l'aggancio stesso** (indice `i-aggancio`) si
/// spostano INSIEME di `inset` verso l'interno (`riferimento -
/// _segno(tipo) * inset`), non solo fino a `riferimento`: l'ultimo
/// tratto resta VERTICALE (nessun moncone orizzontale — **la vera
/// causa del bug di RILIEVO 2 era spostare la colonna lasciando
/// l'aggancio fermo: PROVATO con una sonda, Opzione B1 del verbale
/// D-148, scartata**: la punta torna a orientarsi sul moncone e punta
/// DI LATO), ma la punta non tocca più esattamente il bordo vero della
/// barra (compromesso geometrico scelto dall'utente, visibile a
/// `inset`). Con `inset: 0pt` (o `i-aggancio: none`) il nuovo punto
/// coincide con `riferimento` = comportamento di PRIMA di D-148
/// (verificato: `nuovi.at(i-aggancio)` diventerebbe `riferimento`,
/// cioè lo stesso valore che l'aggancio ha già per costruzione in
/// questo ramo — un `insert` innocuo, mai raggiunto comunque quando
/// `i-aggancio` è `none`).
#let _correggi-lato-sbagliato(punti, i1, i2, riferimento, tipo, i-aggancio: none, inset: 0pt) = {
  // Guardia (S5-bis, D-148): `inset` senza `i-aggancio` sposterebbe
  // SOLO la colonna lasciando l'aggancio fermo -- esattamente
  // l'Opzione B1 scartata (moncone orizzontale, punta di lato). Questa
  // funzione non si fida del chiamante (S5, sopra): fallisce chiaro
  // invece di produrre in silenzio il bug già chiuso una volta.
  assert(
    inset == 0pt or i-aggancio != none,
    message: "_correggi-lato-sbagliato: inset non nullo richiede i-aggancio (altrimenti si riproduce il moncone di RILIEVO 2/Opzione B1, D-148)",
  )
  let x-attuale = punti.at(i1).x
  if (x-attuale - riferimento) * _segno(tipo) < 0pt {
    let x-nuovo = riferimento - _segno(tipo) * inset
    let nuovi = punti
    nuovi.at(i1) = (x: x-nuovo, y: punti.at(i1).y)
    nuovi.at(i2) = (x: x-nuovo, y: punti.at(i2).y)
    if i-aggancio != none {
      nuovi.at(i-aggancio) = (x: x-nuovo, y: punti.at(i-aggancio).y)
    }
    nuovi
  } else {
    punti
  }
}

/// Routing ortogonale (RF10, **default**): `da`/`a` = `(x, y, tipo)`
/// (`tipo`: `"inizio"` | `"fine"`); `riga-da` = fascia `(y0, y1)` del
/// PREDECESSORE (per il canale a 5 tratti); `w` = larghezza area barre;
/// `g` = `style.dependencies.gap`; `row-gap` = `style.layout.row-gap`
/// (il canale sta a `row-gap/4` dal bordo della riga). **3 tratti** se
/// il punto di uscita (`x-uscita = da.x ± g`) permette al tratto
/// orizzontale finale di entrare nell'aggancio di ARRIVO dal verso
/// giusto — **D1 (rilievo del coordinatore, revisione Fase 5):** la
/// condizione va sul tipo del BERSAGLIO (`a.tipo`: un "inizio" si entra
/// da sinistra, serve `x-uscita <= x-rientro`; una "fine" si entra da
/// destra, serve `x-uscita >= x-rientro`), MAI sul tipo della sorgente
/// (`da.tipo`, bug precedente): il tratto diretto a 3 segmenti usa
/// `x-uscita` come UNICO punto di svolta anche per l'ultimo tratto
/// (niente `x-rientro` separato in quel ramo, sotto), quindi è la
/// direzione di INGRESSO nel bersaglio a decidere se il verso è quello
/// giusto, non la direzione di uscita dalla sorgente — per una
/// dipendenza `ss`/`ff` (stesso tipo da entrambi i lati) la vecchia
/// condizione sceglieva il verso opposto a quello corretto (verificato
/// a mano: `ss`/`ff` con sorgente e bersaglio sugli stessi due punti di
/// `fs` danno sempre **5 tratti**, mai 3, perché l'uscita e l'ingresso
/// vanno nello stesso verso "all'indietro" l'uno rispetto all'altro).
/// Altrimenti **5 tratti**, con il tratto centrale nel canale dentro la
/// riga del predecessore (fondo − `row-gap/4` se il successore è sotto,
/// cima + `row-gap/4` se è sopra). Gli AGGANCI (primo/ultimo punto)
/// restano clampati in `[0, w]`; i punti DI SVOLTA interni (`x-uscita`/
/// `x-rientro`) in `[margine-bordo, w - margine-bordo]` (G2, revisione
/// del 2026-10-18, D-121/D-123 — `margine-bordo: 0pt` di default,
/// retrocompatibile: collassa sullo stesso `[0, w]` di prima). **Dopo**
/// il clamp, la colonna di rientro (adiacente ad `a`) e quella di
/// uscita (adiacente a `da`) sono corrette se il margine le ha spinte
/// dal lato SBAGLIATO del rispettivo aggancio
/// (`_correggi-lato-sbagliato`, RILIEVO 2 della rereview del
/// 2026-10-18): la colonna torna esattamente sull'aggancio invece di
/// restare dal lato sbagliato (invariante RF10 "entrata da sinistra/
/// destra" sempre rispettata, anche quando l'aggancio è entro
/// `margine-bordo` dal bordo). **D-148 (2026-10-23): SOLO sul rientro,
/// colonna e aggancio si spostano insieme di `2 × margine-bordo` verso
/// l'interno quando la correzione scatta** (visibilità: un aggancio
/// esattamente sul bordo altrimenti si confonde con `style.field.
/// stroke`, stesso nero di default — vedi `_correggi-lato-sbagliato`
/// sopra per il dettaglio). **S2 (rereview, robustezza):** il
/// riferimento passato a `_correggi-lato-sbagliato` è l'aggancio GIÀ
/// CLAMPATO (`clampati.first()/.last()`, sempre dentro `[0, w]` per
/// costruzione di `_clampa-punto`), non `da.x`/`a.x` grezzi: se un
/// chiamante passasse un aggancio fuori da `[0, w]` (mai il caso in
/// produzione, `x()` clampa già, D-039 — ma questa funzione non deve
/// FIDARSI del chiamante), usare il valore grezzo come bersaglio dello
/// snap produrrebbe un punto FUORI area (RNF6 violato) — con quello
/// clampato, mai. **S3 (ordine delle due chiamate):** nei **5 tratti**
/// gli indici toccati sono DISGIUNTI (`n-2`/`n-3` per il rientro,
/// `1`/`2` per l'uscita: colonne diverse, l'ordine non conta). Nei **3
/// tratti** (colonna CONDIVISA, `avanti`) le due chiamate toccano GLI
/// STESSI indici `1`/`2`: l'ordine conta SOLO nel caso degenere
/// "entrambi gli agganci entro margine dallo stesso bordo" (fuori
/// scala d'uso) — qui la seconda chiamata (lato `da`, uscita) VINCE
/// senza reintrodurre un lato sbagliato per `a`, perché in quel caso
/// `da`/`a` sono per costruzione a distanza minore di `2 × margine`
/// l'uno dall'altro (entrambi vicinissimi allo stesso bordo): lo
/// scarto residuo rispetto all'entrata "ideale" per `a` resta quindi
/// limitato dallo stesso ordine di grandezza del margine, non un
/// errore grande.
/// -> dictionary `(punti: array di (x,y), tratti: int)`.
#let instrada-ortogonale(da, a, riga-da, w, g, row-gap, margine-bordo: 0pt) = {
  let x-uscita = da.x + _segno(da.tipo) * g
  let x-rientro = a.x + _segno(a.tipo) * g
  let avanti = if a.tipo == "inizio" { x-uscita <= x-rientro } else { x-uscita >= x-rientro }
  let punti = if avanti {
    (
      (x: da.x, y: da.y), (x: x-uscita, y: da.y),
      (x: x-uscita, y: a.y), (x: a.x, y: a.y),
    )
  } else {
    let sotto = a.y > da.y
    let y-canale = if sotto { riga-da.y1 - row-gap / 4 } else { riga-da.y0 + row-gap / 4 }
    (
      (x: da.x, y: da.y), (x: x-uscita, y: da.y),
      (x: x-uscita, y: y-canale), (x: x-rientro, y: y-canale),
      (x: x-rientro, y: a.y), (x: a.x, y: a.y),
    )
  }
  let clampati = _clampa-polilinea(punti, w, margine-bordo)
  let n = clampati.len()
  // D-148 (Opzione B, decisione dell'utente del 2026-10-23): SOLO sul
  // lato di RIENTRO (arrivo, dove sta la punta — l'unico lato con un
  // problema di VISIBILITÀ, l'uscita non ha una punta) la correzione
  // sposta la colonna E l'aggancio insieme di `inset = 2 × margine-
  // bordo`, invece di farli collassare ESATTAMENTE sul bordo — "forma
  // più semplice, nessuna chiave di stile pubblica nuova" (scelta
  // dell'utente): deriva da `margine-bordo`, già un parametro
  // esistente, non introduce un nuovo parametro pubblico su questa
  // funzione né un nuovo campo di `style.dependencies`. Con `margine-
  // bordo: 0pt` (default) `inset` è `0pt`: nessun effetto (il ramo
  // "lato sbagliato" non scatta nemmeno con margine 0, dimostrato a
  // mano in D-124 — `unit/layout.typ`, `casi-s6-bit-identico`).
  let corretti = _correggi-lato-sbagliato(
    clampati, n - 2, n - 3, clampati.last().x, a.tipo,
    i-aggancio: n - 1, inset: 2 * margine-bordo,
  )
  let corretti = _correggi-lato-sbagliato(corretti, 1, 2, clampati.first().x, da.tipo)
  (punti: corretti, tratti: n - 1)
}

/// Routing diretto (`"straight"`, RF10): un solo segmento, nessun
/// offset di uscita/rientro (è letteralmente "un segmento" fra i due
/// agganci). Ascisse clampate come sopra (per coerenza, anche se `da`/
/// `a` sono già dentro `[0, w]` per costruzione di `crea-x`). **Nessun
/// punto di svolta interno** (G2, D-123: la polilinea ha solo i due
/// agganci, nulla da clampare in `[margine, w - margine]`): a
/// differenza di `instrada-ortogonale`/`instrada-bezier`, questa
/// funzione non ha un parametro `margine-bordo`.
#let instrada-diretto(da, a, w) = (
  punti: ((x: da.x, y: da.y), (x: a.x, y: a.y)).map(p => _clampa-punto(p, w)),
  tratti: 1,
)

/// Routing a curva di Bézier cubica (`"bezier"`, RF10): tangenti
/// orizzontali agli estremi, punto di controllo a distanza
/// `max(curviness × |Δx|, bulge)` nel verso dell'aggancio (stessa
/// convenzione di `_segno`), campionata in 16 tratti (17 punti,
/// `t = 0, 1/16, …, 1`). `curviness` è una frazione (`float`, già
/// convertita dal chiamante: RF3 "un ratio va convertito prima di
/// moltiplicarlo"). Gli AGGANCI (`p0`/`p3`, punti `0` e `16`) restano
/// clampati in `[0, w]`; i punti CAMPIONATI interni (`1`…`15`, inclusi
/// quelli vicini ai controlli `p1`/`p2`) in `[margine-bordo,
/// w - margine-bordo]` (G2, D-121/D-123 — `margine-bordo: 0pt` di
/// default, retrocompatibile). **RILIEVO 2 (rereview del 2026-10-18),
/// SOLO il punto `15`** (quello adiacente all'aggancio di arrivo, da
/// cui `_direzione-finale` legge il verso della punta): se il margine
/// lo spinge dal lato sbagliato di `a` (stesso problema
/// dell'ortogonale, sopra), lo riporta esattamente su `a.x`
/// (`_correggi-lato-sbagliato` con `i1 == i2 == 15`, nessuna "colonna"
/// qui — a differenza dell'ortogonale non si corregge il punto `1`
/// (lato di `da`): non orienta nessuna punta, un'eventuale tangente
/// iniziale invertita cambia solo la forma della curva vicino a `da`,
/// non una direzione "promessa" da RF10). **S2 (rereview, robustezza,
/// come l'ortogonale sopra):** il riferimento è l'aggancio GIÀ
/// CLAMPATO (`clampati.last().x`), non `a.x` grezzo. **D-148
/// (2026-10-23): NON estesa con lo scostamento dell'Opzione B**
/// (`instrada-ortogonale`, sopra) — verificato con un PNG: le tangenti
/// orizzontali agli estremi (RF10) fanno arrivare la curva all'aggancio
/// in DIAGONALE, mai con un tratto verticale lungo il bordo, quindi
/// nessun problema di visibilità da correggere (`unit/layout.typ`,
/// `casi-r2-bezier-bordo`, nota D-148).
#let instrada-bezier(da, a, g, curviness, bulge, w, margine-bordo: 0pt) = {
  let dx = a.x - da.x
  let d = calc.max(curviness * calc.abs(dx), bulge)
  let p0 = (x: da.x, y: da.y)
  let p3 = (x: a.x, y: a.y)
  let p1 = (x: da.x + _segno(da.tipo) * d, y: da.y)
  let p2 = (x: a.x + _segno(a.tipo) * d, y: a.y)
  let punti-grezzi = range(0, 17).map(i => {
    let t = i / 16
    let mt = 1 - t
    let bx = mt * mt * mt * p0.x + 3 * mt * mt * t * p1.x + 3 * mt * t * t * p2.x + t * t * t * p3.x
    let by = mt * mt * mt * p0.y + 3 * mt * mt * t * p1.y + 3 * mt * t * t * p2.y + t * t * t * p3.y
    (x: bx, y: by)
  })
  let clampati = _clampa-polilinea(punti-grezzi, w, margine-bordo)
  (punti: _correggi-lato-sbagliato(clampati, 15, 15, clampati.last().x, a.tipo), tratti: 16)
}

/// Punto di clip di un segmento `(p0, p1)` all'ordinata `y-limite`
/// (interpolazione lineare in `t`): usato solo quando il segmento
/// ATTRAVERSA davvero quell'ordinata (`p0.y != p1.y`, guardia del
/// chiamante).
#let _interpola-a-y(p0, p1, y-limite) = {
  let t = (y-limite - p0.y) / (p1.y - p0.y)
  (x: p0.x + t * (p1.x - p0.x), y: y-limite)
}

/// Spezza UN segmento `(p0, p1)` sulla fascia semiaperta `[y0, y1)` di
/// una riga (RF10/D-037): `none` se il segmento non la interseca;
/// altrimenti il sotto-segmento `(q0, q1)` clippato. Un tratto
/// ORIZZONTALE (`p0.y == p1.y`) appartiene a UNA SOLA riga (quella con
/// `y0 <= p0.y < y1`, il confine superiore semiaperto risolve
/// l'ambiguità "tratto esattamente sul confine": appartiene alla riga
/// che INIZIA lì, non a quella che finisce lì). Un tratto verticale/
/// obliquo si clippa per interpolazione lineare in `y`.
#let clip-segmento(p0, p1, y0, y1) = {
  if p0.y == p1.y {
    if p0.y >= y0 and p0.y < y1 { (p0, p1) } else { none }
  } else {
    let y-min = calc.min(p0.y, p1.y)
    let y-max = calc.max(p0.y, p1.y)
    if y-max <= y0 or y-min >= y1 { return none }
    let y-basso = calc.max(y-min, y0)
    let y-alto = calc.min(y-max, y1)
    // I DUE estremi veri del tratto clippato sono sempre alle ordinate
    // y-basso/y-alto: se coincidono con un estremo originale del
    // segmento si usa quello (esatto, niente errore di interpolazione),
    // altrimenti si interpola.
    let estremo-a-y(y-target) = {
      if p0.y == y-target { p0 } else if p1.y == y-target { p1 } else { _interpola-a-y(p0, p1, y-target) }
    }
    let estremo-iniziale = estremo-a-y(if p0.y < p1.y { y-basso } else { y-alto })
    let estremo-finale = estremo-a-y(if p0.y < p1.y { y-alto } else { y-basso })
    (estremo-iniziale, estremo-finale)
  }
}

/// Spezza una POLILINEA (array di punti, D-037) per ciascuna riga di
/// `righe-posizionate` (deve avere `path`, `y0`, `y1`): dizionario
/// `path -> array di (p0, p1)`, con solo le righe che intersecano
/// almeno un tratto. Percorre ogni SEGMENTO della polilinea (coppie
/// consecutive) e ogni riga (O(tratti × righe): accettabile, i tratti
/// per dipendenza sono ≤ 16, RNF3).
#let spezza-per-righe(punti, righe-posizionate) = {
  let risultato = (:)
  for i in range(0, punti.len() - 1) {
    let p0 = punti.at(i)
    let p1 = punti.at(i + 1)
    for r in righe-posizionate {
      let clip = clip-segmento(p0, p1, r.y0, r.y1)
      if clip != none {
        let lista = risultato.at(r.path, default: ())
        lista.push(clip)
        risultato.insert(r.path, lista)
      }
    }
  }
  risultato
}

/// Indice della PRIMA riga (array ORDINATO per `y0`, CONTIGUO: `r.y1`
/// della riga `i` coincide sempre con `r.y0` della riga `i+1`,
/// `posiziona()` sopra) il cui `y1` supera `y` — ricerca binaria (G1,
/// RNF3): sostituisce un filtro O(righe) con O(log righe) quando serve
/// solo il PRIMO indice utile di un intervallo-y, non l'intero
/// sottoinsieme filtrato. `righe.len()` se NESSUNA riga lo supera (`y`
/// oltre l'ultima riga).
#let _prima-riga-oltre-y1(righe, y) = {
  let lo = 0
  let hi = righe.len()
  while lo < hi {
    let mid = calc.quo(lo + hi, 2)
    if righe.at(mid).y1 <= y { lo = mid + 1 } else { hi = mid }
  }
  lo
}

/// Come sopra, ma per il confine SUCCESSIVO all'ultima riga utile: la
/// PRIMA riga il cui `y0` raggiunge o supera `y` (ricerca binaria sullo
/// stesso array). Usata assieme a `_prima-riga-oltre-y1` per ricavare
/// con UNA `slice()` (invece di un `.filter()` su tutto l'array) il
/// sottoinsieme di righe la cui fascia `[y0,y1)` interseca un
/// intervallo-y `[y-min, y-max)` — valido SOLO perché le righe sono
/// ordinate e contigue (nessun buco, nessuna sovrapposizione): per un
/// array così fatto, "interseca l'intervallo" è una proprietà MONOTONA
/// nell'indice, quindi il sottoinsieme è sempre un segmento contiguo
/// `[start, end)`.
#let _prima-riga-con-y0-almeno(righe, y) = {
  let lo = 0
  let hi = righe.len()
  while lo < hi {
    let mid = calc.quo(lo + hi, 2)
    if righe.at(mid).y0 < y { lo = mid + 1 } else { hi = mid }
  }
  lo
}

/// Sottoinsieme CONTIGUO di `righe-posizionate` (ordinate/contigue per
/// `y0`, sopra) la cui fascia `[y0,y1)` interseca `[y-min, y-max)` —
/// stesso risultato di `righe.filter(r => r.y1 > y-min and r.y0 <
/// y-max)`, ma O(log righe) invece di O(righe) (G1, RNF3-dip: una
/// dipendenza a lungo raggio su un documento con molte righe non deve
/// scandire l'intero array solo per sapere DOVE comincia/finisce il
/// proprio intervallo).
#let righe-nell-intervallo-y(righe, y-min, y-max) = {
  let inizio = _prima-riga-oltre-y1(righe, y-min)
  let fine = _prima-riga-con-y0-almeno(righe, y-max)
  if fine <= inizio { () } else { righe.slice(inizio, fine) }
}

/// Estremo del nodo (`"inizio"` | `"fine"`) da usare come aggancio di
/// un CAPO di dipendenza (RF10, tabella "Agganci per type"): `capo` è
/// `"da"` (predecessore) o `"a"` (successore). `fs` = fine del
/// predecessore -> inizio del successore; `ss` = inizio -> inizio; `ff`
/// = fine -> fine; `sf` = inizio -> fine (`type` già normalizzato alla
/// forma breve da `validate._normalizza-tipo-dipendenza`).
#let _capo-dipendenza(tipo, capo) = if capo == "da" {
  if tipo == "fs" or tipo == "ff" { "fine" } else { "inizio" }
} else {
  if tipo == "fs" or tipo == "ss" { "inizio" } else { "fine" }
}

/// Direzione dell'ULTIMO tratto NON degenere di una polilinea (RF10,
/// per orientare la punta): scorre all'indietro dal punto finale finché
/// trova un punto DIVERSO da esso — un tratto orizzontale di lunghezza
/// zero è possibile quando `x-uscita` coincide esattamente con
/// `x-rientro` (instrada-ortogonale, caso "avanti" al limite). Se TUTTI
/// i punti coincidono (polilinea degenere — non raggiungibile con due
/// nodi distinti, E-DEP-03, ma gestito per sicurezza) restituisce una
/// direzione arbitraria orizzontale, "best effort" come la skill
/// `etichette-grafico-geometria-pura`. -> `(p-prev, p-punta)`.
#let _direzione-finale(punti) = {
  let p-punta = punti.last()
  let i = punti.len() - 2
  while i >= 0 and punti.at(i) == p-punta {
    i -= 1
  }
  let p-prev = if i >= 0 { punti.at(i) } else { (x: p-punta.x - 1pt, y: p-punta.y) }
  (p-prev, p-punta)
}

/// Vertici (array di 3 punti `(x,y)`, apice compreso) del triangolo
/// della punta di una freccia (RF10: "`polygon` di lato
/// `style.dependencies.arrow`"), apice ESATTO in `p-punta`, orientato
/// lungo la direzione `p-prev -> p-punta`. **Scelta d'implementazione**
/// (decisioni.md): isoscele, non equilatero — lunghezza = `lato` lungo
/// la direzione di marcia, base = `lato` perpendicolare (mezza base =
/// `lato / 2` per ciascun lato): evita `calc.sqrt(3)` (irrazionale) per
/// mantenere i valori attesi dei test esatti (razionali) nei casi
/// assiali, stesso effetto visivo di un triangolo "a freccia". I
/// vettori si normalizzano in FLOAT (`length / 1pt`): `calc.sqrt`
/// accetta solo `int`/`float`, mai `length` (verificato,
/// `scripts/search-api.py`). Caso degenere (`p-prev == p-punta`,
/// direzione nulla — non raggiungibile da `_direzione-finale`, che
/// garantisce sempre un punto diverso quando la polilinea non è
/// TOTALMENTE degenere): triangolo collassato sull'apice (area zero),
/// nessun panic.
#let _punta-triangolo(p-prev, p-punta, lato) = {
  let dxf = (p-punta.x - p-prev.x) / 1pt
  let dyf = (p-punta.y - p-prev.y) / 1pt
  let norma = calc.sqrt(dxf * dxf + dyf * dyf)
  if norma == 0 { return (p-punta, p-punta, p-punta) }
  let ux = dxf / norma
  let uy = dyf / norma
  // Perpendicolare a (ux,uy), ruotata di 90°.
  let px = -uy
  let py = ux
  let latof = lato / 1pt
  let base-x = p-punta.x - ux * latof * 1pt
  let base-y = p-punta.y - uy * latof * 1pt
  (
    p-punta,
    (x: base-x + px * (latof / 2) * 1pt, y: base-y + py * (latof / 2) * 1pt),
    (x: base-x - px * (latof / 2) * 1pt, y: base-y - py * (latof / 2) * 1pt),
  )
}

/// Colore a partire da uno `stroke` GIA' passato a `stroke(..)` (mai
/// `none`, vedi sotto): usato sia per la punta di una freccia
/// (`style.dependencies.stroke`/`.violated.stroke`, RF10) sia per il
/// colore del testo di una milestone (`stroke-m.paint`,
/// `candidati-etichette` sotto — STESSA latenza, stesso fix, trovata
/// nella stessa revisione). **C1 (CRITICO, revisione del 2026-10-18):
/// BUG CHIUSO, DUE punti.** Uno `stroke` scritto SOLO con lo spessore
/// (es. `style: (dependencies: (stroke: 1pt))`, o `style.milestones.
/// normal.stroke: 1pt`, nessun colore) ha `.paint == auto` (verificato,
/// `scripts/search-api.py` + sonda dedicata: `stroke(1pt).paint` dà
/// `type(auto)`) — `polygon(fill: auto)` (punta) e `text(fill: auto)`
/// (milestone) NON ammettono `auto` nel loro parametro `fill`
/// (`color|gradient|tiling[|none]`, MAI `auto`): panic "nudo" di Typst
/// in entrambi i casi ("expected color, gradient, tiling[, or none],
/// found auto"), mai raggiunto prima di questa revisione (i golden/casi
/// precedenti usano sempre uno stroke con colore esplicito, `0.75pt +
/// black`/`.. + red` di default). Qui: `auto` -> nero (stesso criterio
/// pratico di un bordo "senza colore proprio" nel resto del documento).
/// **`none` (decisione del coordinatore, SOLO per le dipendenze — le
/// milestone non hanno un `stroke: none` previsto da RF5) è un caso A
/// PARTE, gestito dal CHIAMANTE prima di arrivare qui** (sotto,
/// `calcola()`): `stroke(none)` non è nemmeno chiamabile (verificato:
/// `stroke()` NON accetta `none` fra i tipi ammessi — `length|color|
/// gradient|tiling|dictionary|stroke` — altro panic "nudo" diverso,
/// "expected length, color, ..., found none"); con uno stroke `none`
/// una dipendenza non viene disegnata affatto dal percorso di default
/// (nessun tratto, nessuna punta, nessun errore): questa funzione
/// quindi non riceve mai `none`.
#let _colore-da-stroke(stroke-valore) = {
  let paint = stroke(stroke-valore).paint
  if paint == auto { black } else { paint }
}

// --- Footer delle etichette (RF12, Fase 8.5, D-127) ----------------------

// Costante CON NOME (non una chiave di stile: decisione dell'utente,
// D-127) usata dalla docstring/corpo di `ripeti-etichette` sotto —
// spostata QUI, PRIMA della docstring (review del 2026-10-18, D-128:
// un `#let` incollato SUBITO dopo un blocco `///` lo lega a SÉ, non
// alla dichiarazione successiva), cosi' la docstring sotto documenta
// `ripeti-etichette`, non questa costante.
#let _frazione-footer-alto = 0.5

/// Regola ESATTA del footer alto (RF12, Fase 8.5, D-127, residuo R3,
/// T38): `table.footer(repeat: true)` non si spezza — un footer
/// ripetuto troppo alto moltiplica le pagine fino a una riga per
/// pagina, senza alcun errore/avviso di Typst (misurato, D-127 sonda
/// 2). Funzione PURA, testabile a mano (skill `etichette-grafico-
/// geometria-pura`), separata da `calcola()` apposta. `repeat`
/// (`auto`/`true`/`false`, GIÀ validato dal chiamante,
/// `style.valida-repeat-etichette`): `true`/`false` ESPLICITI si
/// rispettano SEMPRE, nessuna regola — solo `auto` la attiva. Con
/// `auto`: NON si ripete se `altezza-etichette` supera
/// `_frazione-footer-alto` (sopra) dell'`altezza-utile`
/// (altezza della REGIONE intera, T46 — `float.inf * 1pt` con
/// `page(height: auto)`: la regola non scatta mai, `0.5 * inf` resta
/// infinito e `x > inf` è sempre `false`). Confronto STRETTO (`>`, non
/// `>=`): a ESATTAMENTE metà il footer si ripete ancora.
/// -> bool.
#let ripeti-etichette(repeat, altezza-etichette, altezza-utile) = {
  if repeat == auto {
    not (altezza-etichette > _frazione-footer-alto * altezza-utile)
  } else {
    repeat
  }
}

// --- Entry point pubblico ------------------------------------------------

/// Contesto completo in `pt` per `src/render/*` (RNF3: calcolato UNA
/// VOLTA da `gantt()`, dentro un `context` — qui vivono i soli
/// `measure()` del modulo, RNF5/"Struttura dei moduli"). Combina
/// `spec` (validato) e `modello` (`timeline.calcola()`) con lo stile
/// già risolto (`style.risolvi`) e la larghezza TOTALE disponibile
/// (pt, dal `layout()` del documento).
///
/// - spec (dictionary): uscita di `validate()`/`_validate-interno`.
/// - modello (dictionary): uscita di `timeline.calcola()`.
/// - stile-risolto (dictionary): uscita di `style.risolvi()`.
/// - larghezza-totale (length): larghezza disponibile (sidebar + area).
/// - locale (dictionary): locale già risolto — qui serve SOLO per la
///   parola "oggi" (`locale.at("today")`, RF6) e per i messaggi
///   d'errore (RF7, es. `style.headers.<unit>.align` invalido).
/// - altezza-utile (length, nominale, RF12/D-127, Fase 8.5): altezza
///   della REGIONE intera (`layout()` di `gantt()` in `lib.typ`, T46 —
///   non lo spazio rimasto dopo il contenuto precedente), per la regola
///   del footer alto (`repeat: auto`). Default `float.inf * 1pt`: le
///   chiamate esistenti nei test `unit/`, che non lo passano, restano
///   invariate (la regola non scatta mai con un'altezza infinita).
/// -> dictionary: `(larghezza-sidebar, larghezza-area, x, righe` — ogni
///    riga porta anche `"stile-barra"`, `"stile-testo-sidebar"`,
///    `"indent-sidebar"`, `"inset-sopra"`, `"divisore"`, `"nome-
///    sidebar"` (R1/R5/P12, D-102: geometria/stile per livello già
///    risolti, niente ricalcolo nel render) `,
///    intestazioni` — ogni livello porta anche `align`/`gridline`
///    risolti (R6/RF4.3, D-102) `, "gruppi-griglia" (righe di griglia
///    raggruppate per stroke, D-097), "altezza-header", etichette, "gruppi-segni"
///    (segni di milestone/oggi raggruppati per stroke, D-097),
///    "altezza-etichette", "ripeti-etichette" (bool, RF12/D-127),
///    "altezza-corpo")`.
#let calcola(spec, modello, stile-risolto, larghezza-totale, locale, altezza-utile: float.inf * 1pt) = {
  // Fase 8.9, residuo 1 (D-138): `style.milestones.label` deve essere un
  // `dictionary` — validato QUI, prima di ogni lettura di un suo campo
  // più sotto in questa stessa funzione (`.inset`, `.overhang`, ...),
  // stesso schema di `style.route-dipendenze` subito sotto per
  // `style.dependencies.route`: altrimenti un `style: (milestones:
  // (label: none))` dell'utente arriverebbe al primo accesso di campo
  // come panic "nudo" di Typst (T27/T33). Risultato scartato: serve solo
  // a far fallire presto con un errore chiaro, non un valore da usare
  // (ogni lettura successiva continua a leggere `stile-risolto` stessa).
  let _ = style.valida-milestone-label(stile-risolto, locale)

  let righe-grezze = righe(spec, modello)

  // Larghezza sidebar (RNF3/D-091 punto b): `measure()` chiamato QUI,
  // UNA VOLTA per nodo (non per riga disegnata: il render di Fase 5
  // legge solo la larghezza già decisa, non rimisura nulla, RNF5), solo
  // se l'utente non ha dato un valore esplicito (RF2: `sidebar-width:
  // auto | lunghezza`).
  let larghezza-sidebar = if spec.at("sidebar-width") != auto {
    // D6 (rilievo del coordinatore): `sidebar-width` esplicito (RF2) è
    // un `length` (`validate.typ`: `type(..) != length` -> E-VAL-03),
    // che può essere RELATIVO a un'altra lunghezza — **correzione
    // (rilievo del coordinatore, rereview di Fase 5): NON una
    // PERCENTUALE** (`30%` è di tipo `ratio` in Typst, non `length`:
    // `validate.typ` lo rifiuterebbe con E-VAL-03, commento stantio
    // prima, mai verificato col tipo vero — sonda dedicata), ma un
    // valore in unità FONT-RELATIVE come `10em` (type `length`,
    // verificato): `.to-absolute()` lo risolve in `pt` usando il
    // contesto di testo CORRENTE (richiede `context`, già garantito:
    // `layout.calcola` gira sempre dentro il `context` di `gantt()`/
    // `layout()`, vedi `lib.typ`). Un valore già assoluto (es. `100pt`)
    // passa invariato. Caso `10em` coperto da `ok/sidebar-width-em.typ`
    // (prima il ramo `.to-absolute()` non era mai esercitato da un
    // valore DAVVERO relativo, solo da lunghezze già assolute).
    spec.at("sidebar-width").to-absolute()
  } else {
    // Bug trovato dalla revisione visiva di Fase 5: misurare il nome
    // col testo "nudo" (nessuno stile) sottostima la larghezza vera di
    // una riga di GRUPPO, che si disegna in GRASSETTO
    // (`style.sidebar.group-text`, più largo a parità di testo) — senza
    // lo stesso stile qui, un nome che "ci sta" nella misura va a capo
    // nel render vero, eccede l'altezza fissa della riga e trabocca
    // (visibilmente) nella riga sopra. **S3 (rilievo del coordinatore,
    // precisazione della diagnosi di D-097 bug 2):** la causa precisa
    // NON è `box` in sé (un `box` con `width`/`clip: true` e SOLO testo,
    // senza `align()`, ritaglia correttamente in orizzontale, verificato
    // con una sonda dedicata) — è `align()` dentro un `box` a dimensioni
    // fisse dentro una `table.cell` che fa saltare il ritaglio verticale
    // quando il testo va a capo. La correzione (niente `align()` nella
    // sidebar, solo `inset`, S1 leva 3, sotto) rimane comunque la
    // soluzione giusta; qui la larghezza resta comunque la prima difesa
    // (con la misura giusta, il testo non va quasi mai a capo).
    // **S1 leva 5 (rilievo del coordinatore):** UNA misura sola
    // (`measure(stack(..)).width`, non una per riga): uno `stack()`
    // mette ogni nome già stilizzato (col proprio rientro per livello,
    // `style.sidebar.indent × level`, RF5) su una riga propria —
    // misurare il blocco intero dà direttamente la larghezza MASSIMA
    // (la riga più larga), stesso risultato di `n` misure singole più
    // un `calc.max`, con una sola chiamata a `measure()`.
    // R5/P12 (D-102, Fase 6a): stile del testo e rientro per LIVELLO
    // (`style.levels[l]`, con fallback su `kind`/`style.sidebar.indent ×
    // level`, RF5) invece del solo `kind` — vedi `style.sidebar-testo-
    // livello`/`sidebar-indent-livello`.
    let nomi-stilizzati = righe-grezze.map(r => {
      let stile-testo = style.sidebar-testo-livello(stile-risolto, r.kind, r.level)
      pad(left: style.sidebar-indent-livello(stile-risolto, r.level), text(..stile-testo)[#_contenuto(r.name)])
    })
    let massima = if nomi-stilizzati.len() > 0 { measure(stack(..nomi-stilizzati)).width } else { 0pt }
    massima + 2 * stile-risolto.sidebar.inset
  }
  let larghezza-area = larghezza-totale - larghezza-sidebar
  // D6 (rilievo del coordinatore): guardia esplicita invece di
  // lasciare che un'area negativa/nulla produca un disegno silenzioso
  // e sbagliato (bar/griglie con larghezze negative, niente panic di
  // Typst che lo segnali — D-030, mai un errore "nudo" ma qui nemmeno
  // uno "muto"). `sidebar-width` è quasi sempre la causa raggiungibile
  // dall'utente (un valore esplicito troppo largo); un documento troppo
  // stretto per il default `auto` è un caso degenere non previsto da
  // RF2, ma il messaggio resta corretto (cita comunque `sidebar-width`,
  // il parametro su cui l'utente ha il controllo più diretto: con
  // `auto` può ridurre i nomi o allargare il contenitore).
  if larghezza-area <= 0pt {
    fail(
      "E-LAY-01", "sidebar-width", locale,
      argomenti: (width: repr(larghezza-sidebar), available: repr(larghezza-totale)),
    )
  }

  // Rilievo del coordinatore (rereview di Fase 5): `style.sidebar.align`
  // normalizzato QUI (una sola volta, RNF3) alla sua componente
  // orizzontale — `bars.typ` legge solo `ctx.sidebar-align`, mai più
  // `stile-risolto.sidebar.align` direttamente (guardia di tipo,
  // `style.allineamento-orizzontale`, E-VAL-03 se invalido).
  let sidebar-align = style.allineamento-orizzontale(stile-risolto.sidebar.align, "style.sidebar.align", locale)

  let x = crea-x(modello.viewport, larghezza-area)
  // C2: serve SOLO a `tratto-progresso`, per interpolare un punto
  // parziale senza la distorsione di interpolare fra estremi già
  // clampati (vedi il commento lì).
  let x-grezza = crea-x-grezza(modello.viewport, larghezza-area)

  // R1 (D-102, residuo di D-100 punto 9): altezza del TESTO (una riga)
  // della sidebar per (kind, level) distinti -- UNA misura per
  // combinazione davvero presente (RNF3), non per riga. `for` semplice
  // (T25: una chiusura di `.map()` non puo' mutare
  // `cache-sidebar-altezza-testo`, dichiarata fuori da lei; questo
  // `for` e' nello stesso scope di `calcola()`, come
  // `posiziona`/`gruppi-griglia`). Serve a `inset-sopra` sotto (R1:
  // spostato qui da `render/bars.typ`, era ricalcolato lì a ogni riga)
  // — **MOSSO PRIMA di `posiziona()`** (R5, D-108): la riga deve poter
  // CRESCERE per contenere il nome andato a capo, quindi la larghezza/
  // l'altezza della sidebar vanno decise prima di posizionare le righe
  // lungo l'asse y, non dopo.
  let livelli-attivi = stile-risolto.levels.len() > 0
  let cache-sidebar-altezza-testo = (:)
  for r in righe-grezze {
    let chiave = repr((r.kind, r.level))
    if chiave not in cache-sidebar-altezza-testo {
      let stile-testo = if livelli-attivi {
        style.sidebar-testo-livello(stile-risolto, r.kind, r.level)
      } else if r.kind == "group" {
        stile-risolto.sidebar.at("group-text")
      } else {
        stile-risolto.sidebar.at("task-text")
      }
      cache-sidebar-altezza-testo.insert(chiave, measure(text(..stile-testo)[0]).height)
    }
  }

  let larghezza-sidebar-esplicita = spec.at("sidebar-width") != auto
  // RNF3 (rilievo del benchmark S5, D-102): `style.levels` è VUOTO nello
  // scenario benchmark (come nella maggior parte dei documenti, RF5
  // "se 'levels' è vuoto..."). Le funzioni per-livello
  // (`sidebar-testo-livello`/`sidebar-indent-livello`/`divisore-livello`)
  // fanno già internamente questo stesso controllo (`_livello`, fallback
  // immediato) — ma CHIAMARLE resta un costo fisso per riga (centinaia di
  // righe). Un flag calcolato UNA VOLTA permette alla riga di saltare la
  // chiamata del tutto nel caso comune (stesso risultato, per
  // costruzione: `_livello` restituirebbe `none` comunque), invece di
  // pagare l'overhead della chiamata a vuoto per ogni riga. Il percorso
  // veloce del DIVISORE (sotto) vale solo se `task-dividers` non e' un
  // array (altrimenti serve comunque indicizzarlo per livello, RF5
  // "l'ultimo si ripete") — controllato una volta sola, non per riga.
  let divisore-array = type(stile-risolto.gridlines.at("task-dividers", default: none)) == array

  // R5 (D-108, DECISIONE UTENTE 2026-10-16, sostituisce l'ellissi di
  // D-102/D-100 S3): nome della sidebar (eventualmente a capo) e sua
  // altezza, risolti PRIMA di `posiziona()` -- la riga deve poter
  // CRESCERE per contenerlo. `nome-sidebar-pronto` ritorna `none` nel
  // caso comune (il nome entra su una riga, o non è una stringa): zero
  // misure aggiuntive oltre alla singola verifica di larghezza, nessuna
  // crescita di riga, stesso risultato di sempre (percorso veloce di
  // RNF3/S5, che usa `sidebar-width: auto` e quindi salta pure quella
  // verifica). `altezza-nome-capo` resta `none` in quel caso (letto da
  // `posiziona()`/`altezza-riga()` sopra come "nessuna crescita");
  // altrimenti porta l'altezza MISURATA del blocco multi-riga.
  let righe-con-sidebar = righe-grezze.map(r => {
    let stile-testo-sidebar = if livelli-attivi {
      style.sidebar-testo-livello(stile-risolto, r.kind, r.level)
    } else if r.kind == "group" {
      stile-risolto.sidebar.at("group-text")
    } else {
      stile-risolto.sidebar.at("task-text")
    }
    let indent-sidebar = if livelli-attivi {
      style.sidebar-indent-livello(stile-risolto, r.level)
    } else {
      stile-risolto.sidebar.indent * r.level
    }
    // D-106 punto (d): `style.levels[l].sidebar.align` per livello —
    // stesso percorso veloce di sopra (nessuna chiamata quando `style.
    // levels` è vuoto, lo stesso risultato del default globale già
    // calcolato una volta sopra, `sidebar-align`).
    let sidebar-align-riga = if livelli-attivi {
      style.sidebar-align-livello(stile-risolto, r.level, locale)
    } else {
      sidebar-align
    }
    let altezza-testo-sidebar = cache-sidebar-altezza-testo.at(repr((r.kind, r.level)))
    let pronto = if larghezza-sidebar-esplicita {
      let larghezza-disponibile-nome = larghezza-sidebar - 2 * stile-risolto.sidebar.inset - indent-sidebar
      // Guardia esplicita (review finale di Fase 6a, rilievo del
      // coordinatore): senza questo controllo, una `sidebar-width`
      // troppo stretta per l'inset+il rientro (es. `5pt`, o un rientro
      // ampio a un livello profondo) farebbe comunque "funzionare"
      // `spezza-a-capo` (ogni pezzo garantisce almeno un carattere di
      // progresso, mai un ciclo infinito) — ma il risultato sarebbe un
      // nome spezzato in DECINE di righe di un solo carattere,
      // illeggibile e silenzioso (nessun errore, un difetto "muto").
      // `<= 0pt` (non "< 0pt"): una larghezza ESATTAMENTE zero non ha
      // comunque spazio per nessun carattere.
      if larghezza-disponibile-nome <= 0pt {
        fail(
          "E-LAY-01", "sidebar-width", locale,
          argomenti: (width: repr(larghezza-sidebar), available: repr(larghezza-disponibile-nome)),
          modello: "E-LAY-01.sidebar-name",
        )
      }
      nome-sidebar-pronto(r.name, stile-testo-sidebar, larghezza-disponibile-nome)
    } else {
      none
    }
    r + (
      "stile-testo-sidebar": stile-testo-sidebar,
      "indent-sidebar": indent-sidebar,
      "sidebar-align": sidebar-align-riga,
      "altezza-testo-sidebar": altezza-testo-sidebar,
      "nome-sidebar": if pronto != none { pronto.nome } else { r.name },
      // Punto 3 (review finale di Fase 6a, rilievo del coordinatore):
      // quando il nome e' andato a capo, l'altezza che fa CRESCERE la
      // riga (`altezza-nome-capo`, letta da `posiziona()`/`altezza-
      // riga()`) include anche l'inset VERTICALE della sidebar (2 ×
      // `style.sidebar.inset`, la STESSA costante gia' usata in
      // orizzontale per il rientro/l'indentatura — nessuna nuova chiave
      // di stile) -- senza questo margine, con `row-gap: 0pt` il testo
      // a capo toccherebbe ESATTAMENTE i divisori/il bordo della riga
      // (nessun respiro residuo). `altezza-nome-testo` (sotto, SENZA il
      // margine) resta per il CENTRAGGIO: il testo si centra sulla sua
      // altezza VERA, il margine aggiunto qui si traduce comunque in
      // `inset-sopra`/`inset-sotto` pari a `sidebar.inset` ciascuno
      // (dimostrato a mano in `unit/layout.typ`), non in un padding
      // fisso separato dal centraggio. Entrambe restano `none` quando
      // il nome NON e' andato a capo (percorso comune, invariato).
      "altezza-nome-capo": if pronto != none { pronto.altezza + 2 * stile-risolto.sidebar.inset } else { none },
      "altezza-nome-testo": if pronto != none { pronto.altezza } else { none },
    )
  })

  let righe-posizionate = posiziona(righe-con-sidebar, stile-risolto)

  // --- Dipendenze (RF10/RF11, Fase 7): agganci, routing, polilinee, ---
  // --- tratti per riga, punta -----------------------------------------
  // Calcolato QUI, UNA VOLTA (RNF3), subito dopo `righe-posizionate`
  // (serve `y0`/`y1`/`altezza`/`path`/`id` di OGNI riga, nient'altro di
  // `righe-complete` — nessuna dipendenza circolare). `modello.
  // dependency-anchors`/`.dependency-constraints` sono già pronti da
  // `timeline.calcola()` (RF10, Fase 3/5): qui si TRADUCONO in pt con
  // `x()` (già clampata in `[0, larghezza-area]`, RNF6/D-039) e si
  // instradano con le funzioni di routing esistenti (`instrada-*`,
  // D-037/D-097, invariate: questo blocco le CHIAMA, non le riscrive).
  let route-dipendenze = style.route-dipendenze(stile-risolto, locale)
  let gap-dipendenze = stile-risolto.dependencies.gap
  // Margine dei punti DI SVOLTA interni (G2, revisione del 2026-10-18,
  // D-121/D-123): NON va confuso con `gap-dipendenze` sopra (distanza
  // del bridge di uscita/rientro dall'aggancio, D-037) — `margine`
  // serve solo a tenere un tratto "di passaggio" staccato dal bordo
  // dell'area barre, gli AGGANCI veri (`_clampa-punto`) restano
  // clampati in `[0, larghezza-area]` come prima di questa modifica.
  let margine-dipendenze = stile-risolto.dependencies.margin
  let curviness-dipendenze = float(stile-risolto.dependencies.curviness)
  let bulge-dipendenze = stile-risolto.dependencies.bulge
  let arrow-dipendenze = stile-risolto.dependencies.arrow
  let stroke-normale-dipendenze = stile-risolto.dependencies.stroke
  let stroke-violato-dipendenze = stile-risolto.dependencies.violated.stroke

  // Indici O(1) per id/percorso su `righe-posizionate` (RNF3: costruiti
  // una volta, non ricercati linearmente per ogni dipendenza/tratto).
  // Un nodo senza `id` non può essere capo di nessuna dipendenza
  // (E-DEP-05/E-DEP-02 l'hanno già escluso a monte, Fase 3), quindi
  // `riga-per-id` copre sempre sia `da` sia `a` di ogni voce di
  // `dependency-constraints`.
  let riga-per-id = (:)
  let riga-per-percorso = (:)
  for r in righe-posizionate {
    riga-per-percorso.insert(r.path, r)
    if r.id != none { riga-per-id.insert(r.id, r) }
  }

  // Un record per dipendenza: punto di uscita/rientro (RF10 "Agganci
  // per type"), polilinea instradata (`instrada-*`, invariate) e
  // stroke EFFETTIVO (normale o `violated`, D-034 punto 6) — fonte
  // UNICA per sia il percorso rapido di default (sotto) sia il
  // dizionario `dep` passato a un drawer `dependency` ridefinito
  // (RF11: `points` relativi all'origine dell'area barre, NON spezzati
  // per riga — la spezzatura/il clip restano del RENDER, D-037).
  let dipendenze-calcolate = modello.at("dependency-constraints").map(dep => {
    // BUG CHIUSO (sonda dedicata, `output/revisione-fase7/`): `dep.a`
    // (`n.id`, `timeline.typ`) è l'id del SUCCESSORE SOLO se quel nodo
    // NE HA uno — un nodo che DICHIARA una dipendenza non ha bisogno di
    // un proprio `id` (solo il BERSAGLIO referenziato, `dep.da`, deve
    // averne uno: lo garantisce la risoluzione E-DEP-02 a monte,
    // `graph.typ`). Con `dep.a == none`, `riga-per-id.at(none)` /
    // `dependency-anchors.at(none)` fallivano con un panic "nudo" di
    // Typst ("expected string, found none", `.at()` su un dizionario
    // richiede una chiave stringa) — MAI raggiunto prima di Fase 7
    // (nessun consumatore precedente usava `dep.a` per un lookup). La
    // riga del successore si trova invece per `dep.path` (`n.path`,
    // SEMPRE presente, RF10/D-036), e i suoi agganci dalle date GIA'
    // esposte nella riga stessa (`riga-a.ninfo.inizio`/`.fine`, uguali
    // per costruzione a quelle di `dependency-anchors` quando un id
    // c'è — stessi campi, stessa fonte, `timeline._raccogli`) — mai più
    // un secondo lookup per id sul lato successore.
    let riga-da = riga-per-id.at(dep.da)
    let riga-a = riga-per-percorso.at(dep.path)
    let anchor-da = modello.at("dependency-anchors").at(dep.da)
    let anchor-a = (inizio: riga-a.ninfo.inizio, fine: riga-a.ninfo.fine)
    let tipo-da = _capo-dipendenza(dep.type, "da")
    let tipo-a = _capo-dipendenza(dep.type, "a")
    let data-da = if tipo-da == "fine" { anchor-da.fine } else { anchor-da.inizio }
    let data-a = if tipo-a == "fine" { anchor-a.fine } else { anchor-a.inizio }
    // Ordinata = metà della BARRA nella riga del nodo (RF10): la barra
    // si centra sempre a metà dell'altezza TOTALE della riga
    // (`render/bars.typ: riga-corpo`, `dy: riga.altezza / 2 - ..`), mai
    // sulla sola altezza del contenuto — stesso punto qui, nessuna
    // seconda convenzione per le frecce.
    let punto-da = (x: x(data-da), y: riga-da.y0 + riga-da.altezza / 2, tipo: tipo-da)
    let punto-a = (x: x(data-a), y: riga-a.y0 + riga-a.altezza / 2, tipo: tipo-a)
    let instradato = if route-dipendenze == "bezier" {
      instrada-bezier(
        punto-da, punto-a, gap-dipendenze, curviness-dipendenze, bulge-dipendenze, larghezza-area,
        margine-bordo: margine-dipendenze,
      )
    } else if route-dipendenze == "straight" {
      instrada-diretto(punto-da, punto-a, larghezza-area)
    } else {
      instrada-ortogonale(
        punto-da, punto-a, (y0: riga-da.y0, y1: riga-da.y1),
        larghezza-area, gap-dipendenze, stile-risolto.layout.row-gap,
        margine-bordo: margine-dipendenze,
      )
    }
    let stroke-effettivo = if dep.violato { stroke-violato-dipendenze } else { stroke-normale-dipendenze }
    // C1 (CRITICO, revisione del 2026-10-18): `fill-punta` calcolato
    // SOLO se `stroke-effettivo` non è `none` (vedi `_fill-punta-da-
    // stroke`, sopra: `stroke(none)` non è nemmeno chiamabile). Con
    // `none` resta `none` qui (mai letto dal percorso di default, che
    // SALTA del tutto una dipendenza con stroke `none` — sotto,
    // `accumulo`/`default-drawers.dependency`), ma un drawer `dependency`
    // ridefinito che legge `dep.style.at("fill-punta")` vede comunque un
    // valore coerente (nessun colore) invece di un campo mancante.
    let fill-punta-effettivo = if stroke-effettivo == none { none } else { _colore-da-stroke(stroke-effettivo) }
    // Chiave di raggruppamento per stroke (G1, RNF3): calcolata UNA
    // VOLTA per dipendenza, non per OGNI riga attraversata (prima:
    // `repr(dep.style.stroke)` dentro il ciclo `accumulo`, sotto — `repr`
    // su uno `stroke` non è gratis, e col filtro per intervallo-y una
    // dipendenza a lungo raggio attraversa comunque molte righe).
    let chiave-stroke = repr(stroke-effettivo)
    // Punta (apice + base, RF10) calcolata QUI una volta, in coordinate
    // ASSOLUTE (origine dell'area barre): serve sia al percorso rapido
    // sotto (tradotta per riga) sia al dizionario pubblico del drawer
    // `dependency` ridefinito (`default-drawers.dependency`, RF11: un
    // drawer ridefinito che richiama quello di default deve poter
    // disegnare la STESSA punta, non restare senza).
    let (p-prev, p-punta) = _direzione-finale(instradato.punti)
    // RNF3 (misurato: S5 con 100 dipendenze, `spezza-per-righe` chiamata
    // su TUTTE le righe per OGNI dipendenza costava ~0,9s in più solo
    // nel calcolo — O(dipendenze × tratti × righe), 200 righe × 100
    // dipendenze anche per un tratto orizzontale confinato a 1 riga):
    // `spezza-per-righe` resta INVARIATA (già testata, O(tratti × righe)
    // "accettabile" per una SINGOLA chiamata, D-037) — qui si passa non
    // più `righe-posizionate` INTERA ma solo il SOTTOINSIEME la cui
    // fascia `[y0,y1)` interseca l'intervallo-y della polilinea di
    // QUESTA dipendenza (`y-min`/`y-max` sui suoi punti, UN solo giro,
    // nessun `clip-segmento`): un filtro O(righe) economico (confronto
    // di `length`, niente costruzione di dizionari) prima del lavoro
    // caro. Per una dipendenza fra righe vicine il sottoinsieme è
    // piccolo; per una "all'indietro" su tutto il documento resta grande
    // quanto il numero di righe VERAMENTE attraversate (mai di più).
    // G1 (RNF3): `righe-nell-intervallo-y` (sopra, bisezione) invece di
    // `.filter()` — STESSO risultato (verificato: `unit/layout.typ`,
    // `casi-righe-nell-intervallo-y`, più l'invariante G3 sotto), O(log
    // righe) invece di O(righe) per trovare l'intervallo.
    let ys = instradato.punti.map(p => p.y)
    let y-min = calc.min(..ys)
    let y-max = calc.max(..ys)
    let righe-nel-raggio = righe-nell-intervallo-y(righe-posizionate, y-min, y-max)
    (
      from: dep.da, to: dep.a, type: dep.type, violated: dep.violato,
      route: route-dipendenze, points: instradato.punti,
      style: (stroke: stroke-effettivo, arrow: arrow-dipendenze, "fill-punta": fill-punta-effettivo),
      x: dep.x,
      "riga-partenza": riga-da.path,
      "riga-arrivo": riga-a.path,
      "chiave-stroke": chiave-stroke,
      "punta-assoluta": _punta-triangolo(p-prev, p-punta, arrow-dipendenze),
      "tratti-per-riga": spezza-per-righe(instradato.punti, righe-nel-raggio),
    )
  })

  // Percorso rapido di default (D-037/D-115): per OGNI riga attraversata
  // da almeno un tratto, l'elenco dei tratti GIA' spezzati e TRADOTTI
  // in coordinate RELATIVE alla riga (`y - riga.y0`, stessa convenzione
  // di `sfondo-riga`/`riga-corpo` in `render/bars.typ`: ogni `place()`
  // dentro una riga è relativo alla sua origine, mai assoluto rispetto
  // al corpo — un `place` assoluto si romperebbe alla spezzatura di
  // pagina, D-037) — zero calcolo nel render (RNF5). La punta
  // (`_punta-triangolo`, sopra) si assegna SOLO alla riga che contiene
  // il punto d'arrivo (`dep.riga-arrivo`), orientata sull'ultimo tratto
  // NON degenere della polilinea COMPLETA (`_direzione-finale`, non dei
  // soli tratti clippati di questa riga: l'arrivo è sempre dentro la
  // fascia della riga d'arrivo per costruzione, mai sul confine, quindi
  // il tratto non viene mai tagliato).
  //
  // RNF3 (misurato: S5 con 100 dipendenze a lungo raggio — RNF3-dip,
  // coppie casuali su 200 righe, mediana fino a ~250 righe di distanza):
  // anche dopo il filtro per intervallo-y (sopra), il NUMERO di
  // `place()`/`curve()` costruiti (uno per dipendenza PER riga
  // attraversata) resta l'ordine di grandezza dominante nel render — lo
  // stesso principio di `gruppi-griglia`/`gruppi-segni` (D-097, "~3×
  // più veloce a parità di tratti" con UN `curve()` per GRUPPO per
  // riga invece di uno per elemento): le dipendenze con lo STESSO
  // stroke risolto (`dep.style.stroke`, normale/`violated`, di norma 2
  // soli valori distinti) nella STESSA riga condividono un UNICO
  // `curve()` con TUTTI i loro tratti (ogni coppia `(p0,p1)` riparte
  // comunque con un proprio `curve.move`, D-037: nessun segmento
  // fantasma a unire dipendenze diverse). Le punte restano separate
  // (un `polygon()` per punta, di norma 0-1 per riga: non raggruppabili
  // in un solo `curve()`, forme diverse).
  // Accumulatore intermedio PER RIGA: `(per-stroke: dizionario chiave ->
  // voce, ordine: array di chiavi in ordine di prima comparsa)` — solo
  // di servizio al raggruppamento, convertito in array subito dopo
  // (`frecce-per-percorso`, sotto): mai esposto oltre questo blocco.
  // C1 (CRITICO, revisione del 2026-10-18, decisione del coordinatore):
  // una dipendenza con stroke risolto `none` (`style.dependencies.
  // stroke: none`, o `.violated.stroke: none` se violata) NON entra nel
  // percorso rapido di default — nessun tratto, nessuna punta, nessun
  // errore (un `curve()`/`polygon()` con `none` non avrebbe senso da
  // disegnare comunque). Resta comunque in `ctx.dipendenze` per un
  // drawer `dependency` ridefinito (`dep.style.stroke == none` è
  // un'informazione legittima per un drawer che vuole un comportamento
  // diverso, RF11 "interfaccia versionata").
  let accumulo = (:)
  for dep in dipendenze-calcolate {
    if dep.style.stroke == none { continue }
    for (percorso, tratti) in dep.at("tratti-per-riga") {
      let y0-riga = riga-per-percorso.at(percorso).y0
      let trasla(p) = (x: p.x, y: p.y - y0-riga)
      let tratti-relativi = tratti.map(((p0, p1)) => (trasla(p0), trasla(p1)))
      let punta = if percorso == dep.riga-arrivo {
        (punti: dep.at("punta-assoluta").map(trasla), fill: dep.style.at("fill-punta"))
      } else {
        none
      }
      let voce-riga = accumulo.at(percorso, default: (per-stroke: (:), ordine: ()))
      // G1 (RNF3): chiave GIA' calcolata una volta per dipendenza
      // (`dep.at("chiave-stroke")`, sopra) — nessun `repr()` ripetuto
      // per ogni riga attraversata.
      let chiave = dep.at("chiave-stroke")
      if chiave not in voce-riga.per-stroke {
        voce-riga.per-stroke.insert(chiave, (stroke: dep.style.stroke, tratti: (), punte: ()))
        voce-riga.ordine.push(chiave)
      }
      let voce-stroke = voce-riga.per-stroke.at(chiave)
      voce-stroke.tratti += tratti-relativi
      if punta != none { voce-stroke.punte.push(punta) }
      voce-riga.per-stroke.insert(chiave, voce-stroke)
      accumulo.insert(percorso, voce-riga)
    }
  }
  // Array finale per riga (ordine di prima comparsa dello stroke,
  // deterministico — RNF4): `riga.frecce` = `((stroke:, tratti:,
  // punte:), ..)`, un elemento per STROKE distinto nella riga (di
  // norma 1-2: normale/`violated`), non più uno per dipendenza.
  let frecce-per-percorso = (:)
  for (percorso, voce-riga) in accumulo {
    frecce-per-percorso.insert(percorso, voce-riga.ordine.map(c => voce-riga.per-stroke.at(c)))
  }

  // Dizionario `dep` pubblico per un drawer `dependency` ridefinito
  // (RF11): le SOLE 8 chiavi documentate, più alcune ADDITIVE
  // (S4, revisione del 2026-10-18 — "interfaccia versionata: solo
  // aggiunte", mai tolte né rinominate le 8 originali):
  // - `"righe-attraversate"`: serve a `render/chart.typ` per sapere in
  //   quali righe disporre il content (una volta per riga attraversata,
  //   D-037) senza rifare `spezza-per-righe` una seconda volta;
  // - `"punta"`: apice + base del triangolo (ASSOLUTA), per
  //   `default-drawers.dependency` (sotto) — un drawer ridefinito che
  //   richiama il default deve poter disegnare la STESSA punta;
  // - `"from-path"`/`"to-path"`: il `path` (SEMPRE presente, D-036) di
  //   predecessore/successore — a differenza di `from`/`to` (gli ID,
  //   `dep.da`/`dep.a`), `to` può essere `none` quando il successore
  //   NON ha un proprio id (bug chiuso, D-121/D-118: un nodo che
  //   DICHIARA una dipendenza non ne ha bisogno) — `to-path` resta
  //   sempre una stringa valida, utile a un drawer che vuole comunque
  //   identificare la riga d'arrivo senza un id. `dep.style` resta
  //   `(stroke: <length|color|...>, arrow: <length>, "fill-punta":
  //   <color|none>)`: `stroke` è il valore RISOLTO (normale/`violated`,
  //   può essere `none` — C1, sopra); `fill-punta` è il colore già
  //   estratto per la punta (`auto` nello `stroke` -> nero, C1), `none`
  //   SOLO se `stroke` è `none`.
  let dipendenze-pubbliche = dipendenze-calcolate.map(dep => (
    from: dep.from, to: dep.to, type: dep.type, violated: dep.violated,
    route: dep.route, points: dep.points, style: dep.style, x: dep.x,
    "punta": dep.at("punta-assoluta"),
    "righe-attraversate": dep.at("tratti-per-riga").keys(),
    "from-path": dep.at("riga-partenza"),
    "to-path": dep.at("riga-arrivo"),
  ))

  let righe-complete = righe-posizionate.map(r => {
    // L6 (D-105, leva di prestazioni; commento corretto in rereview
    // D-106 — NON c'e' memoizzazione automatica delle chiamate in
    // Typst, l'affermazione precedente era sbagliata): `stile-barra`
    // riceve SOLO le chiavi che legge davvero (`kind`, `level`, `color`,
    // `style`), non l'intera riga `r` (che porta anche
    // `ninfo`/`clamp`/`name`/`path`, diversi per OGNI riga anche quando
    // lo STILE è identico). Il guadagno misurato (S5) è nel costruire e
    // copiare un dizionario piccolo invece di uno grande ad ogni
    // chiamata, non nel riuso di un risultato già calcolato.
    // `path` (review finale di Fase 9, D-140): solo per il percorso degli
    // eventuali errori di conversione YAML di `style`/`task-style`
    // (`style.stile-barra`) — nessun altro uso qui.
    let nodo-minimo = (kind: r.kind, level: r.level, color: r.color, style: r.style, path: r.path)
    let stile-barra = style.stile-barra(
      stile-risolto, nodo-minimo, task-style-ereditato: r.at("task-style-ereditato", default: (:)), locale: locale,
    )
    let stato = segmenti-stato(r.ninfo, r.clamp, x, stile-barra, larghezza-area)
    // R5 (D-108): il nome puo' essere andato a capo (`altezza-nome-
    // testo` non `none`) -- il centraggio verticale usa la SUA altezza
    // VERA (multi-riga, SENZA il margine di `sidebar.inset` aggiunto a
    // `altezza-nome-capo` per far crescere la riga, punto 3 sopra: il
    // margine non e' un padding fisso a parte, e' quello che resta del
    // centraggio una volta tolta l'altezza vera del testo), altrimenti
    // quella del testo su una riga sola (`altezza-testo-sidebar`,
    // invariato rispetto a prima: stesso valore/stessa formula, nessuna
    // differenza osservabile nel caso comune). La riga (`r.altezza`, da
    // `posiziona()`) e' GIA' cresciuta a sufficienza per contenere il
    // blocco, mai il contrario.
    let altezza-nome-per-centraggio = if r.at("altezza-nome-testo") != none { r.altezza-nome-testo } else { r.altezza-testo-sidebar }
    r + (
      "stile-barra": stile-barra,
      stato: stato.stato,
      "segmenti-stato": stato.segmenti,
      // RF10/Fase 7: tratti di dipendenza GIA' spezzati per questa riga
      // e tradotti in coordinate relative (sopra) — `()` nel caso
      // comune (nessuna dipendenza attraversa la riga), stesso costo
      // nullo di `segmenti-progresso`/`()` per una riga senza avanzamento.
      "frecce": frecce-per-percorso.at(r.path, default: ()),
      // R1 (D-102): `tratto-progresso` non controlla da sola `clamp`
      // (lavora su date, non su "dentro/fuori viewport") -- la guardia
      // qui garantisce che un nodo clampato interamente fuori viewport
      // (`stato == "none"`) abbia ANCHE `segmenti-progresso` vuoto,
      // cosi' `render/bars.typ` non ha piu' bisogno di un `if riga.stato
      // != "none"` esplicito attorno ai due cicli di disegno (erano
      // comunque entrambi vuoti in quel caso, ma solo grazie a quella
      // guardia ridondante nel render: ora lo sono per costruzione).
      "segmenti-progresso": if r.clamp != none and not r.clamp.outside {
        tratto-progresso(r.ninfo, x, x-grezza, larghezza-area)
      } else {
        ()
      },
      // R1: rientro per il centraggio verticale del nome nella sidebar
      // (prima ricalcolato a ogni riga in `render/bars.typ`). **Barra E
      // testo si centrano con la STESSA regola** (RF3, R5/D-108): dead-
      // center nell'altezza TOTALE della riga (`render/bars.typ` centra
      // la barra con `riga.altezza / 2 - voce.height / 2`, qui lo
      // stesso principio per il testo) -- quando la riga cresce per il
      // nome a capo, la barra resta comunque centrata nella riga più
      // alta (nessun cambiamento in `bars.typ`, la formula è già quella
      // giusta: legge `riga.altezza`, già cresciuto).
      "inset-sopra": calc.max((r.altezza - altezza-nome-per-centraggio) / 2, 0pt),
      "divisore": if livelli-attivi or divisore-array {
        style.divisore-livello(stile-risolto, r.level)
      } else {
        stile-risolto.gridlines.at("task-dividers", default: none)
      },
    )
  })
  let altezza-corpo = if righe-complete.len() > 0 { righe-complete.at(righe-complete.len() - 1).y1 } else { 0pt }

  let intestazioni-calcolate = intestazioni(modello.headers, x, stile-risolto, locale)
  // Righe verticali della griglia (RF4/D-037), APPIATTITE e risolte
  // UNA VOLTA sola qui (RNF3, D-097: rilievo del benchmark S5): prima
  // ogni riga del corpo rifaceva da sé, per OGNI livello, il filtro
  // "non soppresso" e la ricerca `style.gridlines.<unit>s` (concatenando
  // la stringa a ogni passo) — O(righe × livelli), con centinaia di
  // righe e qualche decina di segmenti per livello diventa un costo
  // reale (misurato: su 250 righe/3 livelli, abbattere questa
  // ridondanza ha riportato S5 sotto soglia). Qui si risolve lo stroke
  // una volta per livello (non per riga) e si appiattiscono i soli
  // segmenti dei livelli NON soppressi con uno stroke non-`none`: ogni
  // riga del corpo/header si limita a un `for` su questa lista già
  // pronta `(x, stroke)`.
  // Raggruppate per STROKE (non elenco piatto): `render/bars.typ`
  // disegna un solo `curve()` per gruppo per riga invece di un
  // `place(line())` per ogni singola ascissa — misurato ~3× più
  // veloce a parità di tratti (sonda dedicata), perché costruisce UN
  // frame invece di `n` frame indipendenti. Ordine di `chiave-ordine`
  // preservato per avere un output deterministico (RNF4): i dizionari
  // Typst conservano l'ordine d'inserimento, ma qui si itera
  // `intestazioni-calcolate` (già in ordine di documento) una sola
  // volta, quindi è già garantito.
  let gruppi-griglia = {
    let per-stroke = (:)
    let ordine = ()
    for livello in intestazioni-calcolate {
      if livello.soppresso { continue }
      // `livello.gridline` e' GIA' risolto (stile > definizione > none)
      // da `intestazioni()` sopra (D-102): niente seconda chiamata a
      // `gridline-unita` qui.
      let stroke = livello.gridline
      if stroke == none { continue }
      let chiave = repr(stroke)
      if chiave not in per-stroke {
        per-stroke.insert(chiave, (stroke: stroke, xs: ()))
        ordine.push(chiave)
      }
      let voce = per-stroke.at(chiave)
      for gx in griglia-visibile(livello.segments, stile-risolto.field.stroke != none) { voce.xs.push(gx) }
      per-stroke.insert(chiave, voce)
    }
    ordine.map(c => per-stroke.at(c))
  }
  // Altezza di riga d'intestazione (RNF3, D3/D-097: misurata con lo
  // STESSO stile usato dal render, mai col testo "nudo"): da D-106
  // punto (a) è PER LIVELLO (`livello.altezza`, già risolta da
  // `intestazioni()` — esplicita con `headers[i].height`/`style.
  // headers.<unità>.height`, altrimenti misurata col TESTO di quel
  // livello + `2 × header-inset`, come prima quando non c'è nessun
  // override: stesso risultato per i casi senza `style.levels`/
  // `headers[i]` espliciti). L'altezza TOTALE del blocco intestazione
  // (per la cella sidebar col `rowspan`) è la SOMMA delle altezze dei
  // soli livelli VISIBILI (non soppressi) — prima era `n ×
  // altezza-header`, valida solo quando tutti i livelli hanno la
  // STESSA altezza (il caso comune, invariato).
  let livelli-visibili = intestazioni-calcolate.filter(l => not l.soppresso)
  let altezza-header-totale = livelli-visibili.fold(0pt, (acc, l) => acc + l.altezza)

  // S1 leva 3 (rilievo del coordinatore): l'altezza del testo per
  // centrare verticalmente il nome nella sidebar con un `inset` invece
  // di `align()` (S3: `align()` dentro un box/table.cell a dimensioni
  // fisse è la causa precisa del trabocco di D-097 bug 2, oltre a
  // costare di più per riga) è ora precalcolata PER RIGA
  // (`righe-complete.*.inset-sopra`, R1/D-102, sopra, via la cache per
  // (kind,level)): niente più un dizionario `(group:, node:)` qui — il
  // render legge solo `riga.inset-sopra`.

  // Etichette di milestone/oggi (RF3/D-038): ciascuna misurata UNA
  // VOLTA (nome + data, già LOCALIZZATA — Fase 6b Parte B,
  // `locale-mod.formatta-milestone` — qui "oggi" usa solo
  // `locale.at("today")`, una chiave semplice, non una data da
  // formattare). Poi impilate/
  // clampate con `impila-e-clampa` (skill
  // `etichette-grafico-geometria-pura`).
  // Larghezza da usare per l'anti-collisione: il TESTO misurato più
  // `2 × label.inset` — esattamente quanto il render (`render/
  // milestones.typ`) aggiunge intorno al testo col proprio `box(inset:
  // ..)`. Senza questo margine l'anti-collisione sottostima la
  // larghezza vera: due etichette "non sovrapposte" per il calcolo
  // finiscono comunque a toccarsi (o a sovrapporsi) nel disegno reale,
  // perché l'inset del render non è nel preventivo (bug trovato dalla
  // revisione visiva di Fase 5, non raggiungibile prima che qualcosa
  // disegnasse davvero le etichette: D-051).
  let inset-etichetta = 2 * stile-risolto.milestones.label.inset
  // Rilievo del coordinatore (rereview di Fase 5): precedenza del
  // COLORE del testo (`fill`) fra lo stroke risolto della milestone
  // (`stile-m.stroke.paint`, default) e un override esplicito
  // dell'utente in `style.milestones.label.text.fill` — VINCE l'utente.
  // Risolto QUI (una volta sola, RNF3: lo stile del testo è condiviso da
  // TUTTE le etichette, non per etichetta). **Diagnosi corretta dopo una
  // sonda dedicata (non un "duplicate argument" panic, come ipotizzato
  // inizialmente):** il vecchio `text(..label.text, fill: e.stroke.
  // paint)` del render non faceva panic — in Typst 0.15 l'argomento
  // nominale scritto per ultimo (qui `fill: e.stroke.paint`, DOPO lo
  // spread) sovrascrive silenziosamente quello dello spread — quindi lo
  // stroke vinceva SEMPRE, un `fill` dell'utente in `label.text` restava
  // sempre ignorato, senza nessun errore. Qui si decide il valore FINALE
  // una volta (vince l'utente se presente), il render lo spreaddisce in
  // un SOLO dizionario già unito (`+`): oltre a correggere la
  // precedenza, evita anche di affidarsi all'ordine implicito
  // spread/esplicito di Typst (comportamento non ovvio per chi legge).
  let fill-utente-etichetta = stile-risolto.milestones.label.text.at("fill", default: none)
  // D4 (rilievo del coordinatore): lo stile EFFETTIVO di ciascuna
  // milestone (`m.style` > `style.milestones.normal`, via
  // `style.stile-milestone`) è risolto QUI, una volta, non più un unico
  // stroke condiviso da TUTTE le milestone normali — serve a
  // raggruppare poi per stroke RISOLTO (non per il solo flag `today`).
  // `stroke(..)` (costruttore nativo di Typst, non la nostra funzione)
  // normalizza qualunque valore ammesso da `stroke:` (colore nudo,
  // lunghezza, dizionario, stroke già pronto) in uno stroke vero: senza
  // questo passaggio un override come `style: (stroke: red)` (colore
  // nudo, non `1pt + red`) farebbe fallire `.paint` più sotto (un
  // `color` non ha quel campo).
  let candidati-etichette = modello.milestones.map(m => {
    // Campo "nome" (non "name"): cosi' lo espone `timeline.calcola`
    // (`milestones-raccolte`, in italiano come il resto del core).
    // S2(c): nome in grassetto su una riga, data (se mostrata) sulla
    // riga sotto (`linebreak()`, non uno spazio) — come l'originale.
    // `measure()` su un content con `linebreak()` misura il riquadro
    // (larghezza = la riga più larga, altezza = somma delle righe):
    // la larghezza usata dall'anti-collisione è già quella giusta per
    // due righe, non per una riga sola concatenata.
    // RF11 (Fase 6b): `nome`/`"data-label"` esposti SEPARATAMENTE (non
    // solo il `testo` già composto) al drawer `milestone`
    // (`render/drawers.typ`), che li compone SOLO con `linebreak()`
    // (`[#m.name#linebreak()#m.date-label]`) — il grassetto è GIA'
    // applicato qui a `nome` (`strong[..]`), non nel drawer: per una
    // milestone VERA `testo`/`nome` restano la STESSA identica
    // costruzione di prima (nessuna divergenza fra la larghezza
    // preventivata qui per l'anti-collisione e quella vera disegnata
    // dal drawer di default). **"oggi" non è in grassetto** (asimmetria
    // preesistente, Fase 5: `testo-oggi` sotto non è mai stato
    // `strong[..]`, a differenza del nome di una milestone vera) — il
    // drawer di default replica anche QUESTA differenza, perché il
    // vincolo di questo turno è "nessun pixel diverso", non
    // "simmetria fra milestone e oggi" (osservazione per il verbale,
    // non una correzione: resta fuori da questa fase). D-106 punto
    // (b)/D-107 D4(b)/Fase 6b Parte B (decisione utente, re-approvazione
    // dei golden): `"data-label"` ora LOCALIZZATO
    // (`locale-mod.formatta-milestone`, RF6 `milestone-date-format`,
    // default "14 Jan 2025"/"14 gen 2025" — prima ISO "2025-01-14"),
    // con l'override per documento `style.milestones.label.date-format`
    // (RF5, "lo stile vince sul locale", stessa precedenza di ogni
    // altro campo di `style`) se non `none`.
    let nome-contenuto = strong[#_contenuto(m.nome)]
    let data-label = if m.at("show-date") {
      locale-mod.formatta-milestone(m.data, locale, formato: stile-risolto.milestones.label.at("date-format"))
    } else {
      none
    }
    let testo = if data-label != none {
      [#nome-contenuto#linebreak()#data-label]
    } else {
      nome-contenuto
    }
    let stile-m = style.stile-milestone(stile-risolto, m, today: false, locale: locale)
    let stroke-m = stroke(stile-m.stroke)
    // C1 (CRITICO, revisione del 2026-10-18, BUG CHIUSO, stessa latenza
    // delle dipendenze): `stroke-m.paint` può essere `auto` (stroke
    // scritto senza colore, es. `style.milestones.normal.stroke: 1pt`)
    // — `text(fill: auto)` non è ammesso (panic "nudo", verificato).
    // `_colore-da-stroke` (sopra) applica lo stesso fallback nero.
    let fill-testo-m = if fill-utente-etichetta != none { fill-utente-etichetta } else { _colore-da-stroke(stroke-m) }
    (
      // S3 (review di Fase 6b, BUG CHIUSO): misurato CON lo stile
      // reale (`stile-risolto.milestones.label.text`) — prima
      // `measure(testo)` misurava col testo "nudo" (stile AMBIENTE del
      // documento, non quello configurato): un `label.text` con `size`
      // diverso dal default (es. 14pt) rendeva la larghezza
      // preventivata per l'anti-collisione DIVERSA da quella vera
      // disegnata dal drawer (che usa SEMPRE `m.style.text`, già
      // corretto) — due etichette "non sovrapposte" per il calcolo
      // potevano toccarsi o sovrapporsi nel disegno reale (stesso
      // sintomo del bug di D-051 su `label.inset`, ora corretto anche
      // per `label.text`). Default `text: (:)` (vuoto): nessuna
      // differenza di misura per un documento che non personalizza lo
      // stile (verificato: i 6 golden non toccano `milestones.label.
      // text`).
      x: x(m.data), "x-segno": x(m.data),
      larghezza: measure(text(..stile-risolto.milestones.label.text)[#testo]).width + inset-etichetta,
      testo: testo, today: false, data: m.data, stroke: stroke-m,
      "fill-testo": fill-testo-m,
      nome: nome-contenuto, "data-label": data-label,
      "stile-etichetta": (
        text: stile-risolto.milestones.label.text + (fill: fill-testo-m),
        fill: stile-risolto.milestones.label.fill, inset: stile-risolto.milestones.label.inset,
      ),
    )
  })
  let testo-oggi = _contenuto(locale.at("today"))
  let candidati-etichette = if modello.today != none {
    let stile-oggi = style.stile-milestone(stile-risolto, (style: (:)), today: true, locale: locale)
    let stroke-oggi = stroke(stile-oggi.stroke)
    // C1 (CRITICO, revisione del 2026-10-18, BUG CHIUSO): stessa latenza
    // di `fill-testo-m` sopra, per lo stroke di "oggi".
    let fill-testo-oggi = if fill-utente-etichetta != none { fill-utente-etichetta } else { _colore-da-stroke(stroke-oggi) }
    candidati-etichette + ((
      // S3: stesso fix di sopra, misurato CON lo stile reale.
      x: x(modello.today), "x-segno": x(modello.today),
      larghezza: measure(text(..stile-risolto.milestones.label.text)[#testo-oggi]).width + inset-etichetta,
      testo: testo-oggi, today: true, data: modello.today, stroke: stroke-oggi,
      "fill-testo": fill-testo-oggi,
      nome: testo-oggi, "data-label": none,
      "stile-etichetta": (
        text: stile-risolto.milestones.label.text + (fill: fill-testo-oggi),
        fill: stile-risolto.milestones.label.fill, inset: stile-risolto.milestones.label.inset,
      ),
    ),)
  } else {
    candidati-etichette
  }
  let candidati-ordinati = candidati-etichette.sorted(key: e => e.data)
  let etichette-impilate = impila-e-clampa(candidati-ordinati, larghezza-area, margine: stile-risolto.milestones.label.gap)
  // Altezza di UN livello di etichette (RNF3: una misura condivisa, non
  // per etichetta — la LARGHEZZA sì è per etichetta, sopra, perché il
  // testo cambia; l'altezza no, il corpo del testo è lo stesso stile).
  // **D3/S2(c):** misurata con lo stile reale (`milestones.label.text`).
  // **Rilievo del coordinatore (rereview di Fase 5): campione REALE a
  // DUE righe (con `linebreak()`, leading incluso) SOLO se almeno una
  // milestone ha `show-date` — altrimenti campione a UNA riga.** Prima
  // si raddoppiava sempre `measure(..)[0].height` (una riga sola) anche
  // quando NESSUNA etichetta porta la data (`show-date: false`
  // ovunque, come `ok/multipagina.typ`): uno spreco di spazio (~13pt
  // per livello) che grava sull'altezza del footer × il numero di
  // livelli, mai recuperabile dal testo reale (sempre una riga sola in
  // quel caso). Un campione REALE (non `2 × altezza-una-riga`) perché il
  // leading fra due righe non è necessariamente il doppio di una riga
  // sola (dipende dal font): misurare il content vero evita di
  // sottostimarlo o sovrastimarlo.
  let ha-due-righe = modello.milestones.any(m => m.at("show-date"))
  let campione-etichetta = if ha-due-righe {
    text(..stile-risolto.milestones.label.text)[#strong[0]#linebreak()0]
  } else {
    text(..stile-risolto.milestones.label.text)[#strong[0]]
  }
  let altezza-livello-etichetta = measure(campione-etichetta).height + 2 * stile-risolto.milestones.label.inset
  // **Rilievo del coordinatore, seconda revisione di Fase 5:** quando
  // l'anti-collisione impila su più livelli (sopra), l'altezza TOTALE
  // del footer cresce di conseguenza — `livelli-usati()` resta una
  // sola chiamata (RNF3, non una misura per livello: l'altezza di UN
  // livello è sempre la stessa, cambia solo quanti ce ne sono).
  let altezza-etichette = (
    stile-risolto.milestones.label.overhang + livelli-usati(etichette-impilate) * altezza-livello-etichetta
  )
  // RF12, Fase 8.5 (D-127, residuo R3, T38): `repeat` GIÀ validato qui
  // (`style.valida-repeat-etichette`, chiamata ad OGNI compilazione —
  // anche senza milestone, D-018 "mai un errore silenzioso") e passato
  // alla regola PURA `ripeti-etichette()` (sopra, testabile a mano
  // isolatamente). Nessuna misura nuova (D10): `altezza-etichette` è
  // già calcolata sopra, `altezza-utile` viene da `layout()` in
  // `lib.typ`, non da `measure` qui.
  let repeat-validato = style.valida-repeat-etichette(stile-risolto, locale)
  let ripeti-etichette-risolto = ripeti-etichette(repeat-validato, altezza-etichette, altezza-utile)
  // Geometria del connettore (rilievo del coordinatore, rereview di
  // Fase 5): calcolata QUI, una volta per etichetta (`connettore-punti`,
  // sopra — geometria pura, testabile a mano), non più nel render
  // (`render/milestones.typ` diventa content puro: disegna SOLO i punti
  // già pronti, RNF5).
  let etichette = etichette-impilate.map(e => e + (
    connettore: connettore-punti(
      e, stile-risolto.milestones.label.overhang, altezza-livello-etichetta,
      stile-risolto.milestones.label.inset, stile-risolto.milestones.label.at("connector-threshold"),
    ),
  ))
  // Segni verticali di milestone/oggi, raggruppati per lo stroke
  // EFFETTIVO già risolto per etichetta sopra (D4/D-097, stessa tecnica
  // di `gruppi-griglia`): di norma al più 2 gruppi (`normal`/`today`,
  // quando nessuna milestone ha un `style` proprio), ma una milestone
  // con `style: (stroke: ..)` diverso forma il proprio gruppo — un
  // `curve()` per gruppo per riga invece di un `place(line())` per
  // milestone.
  let gruppi-segni = {
    let per-stroke = (:)
    let ordine = ()
    for e in etichette {
      let chiave = repr(e.stroke)
      if chiave not in per-stroke {
        per-stroke.insert(chiave, (stroke: e.stroke, xs: ()))
        ordine.push(chiave)
      }
      let voce = per-stroke.at(chiave)
      voce.xs.push(e.at("x-segno"))
      per-stroke.insert(chiave, voce)
    }
    ordine.map(c => per-stroke.at(c))
  }

  (
    "larghezza-sidebar": larghezza-sidebar,
    "larghezza-area": larghezza-area,
    "sidebar-align": sidebar-align,
    x: x,
    righe: righe-complete,
    intestazioni: intestazioni-calcolate,
    "gruppi-griglia": gruppi-griglia,
    "gruppi-segni": gruppi-segni,
    "altezza-header-totale": altezza-header-totale,
    etichette: etichette,
    "altezza-etichette": altezza-etichette,
    // RF12, Fase 8.5 (D-127): bool GIÀ risolto (regola del footer alto
    // applicata solo con `repeat: auto`) — `render/milestones.typ` lo
    // passa diretto a `table.footer(repeat:)`, mai il valore di stile
    // grezzo (che può essere `auto`, non ammesso lì).
    "ripeti-etichette": ripeti-etichette-risolto,
    "altezza-livello-etichetta": altezza-livello-etichetta,
    "altezza-corpo": altezza-corpo,
    // RF10/RF11, Fase 7: dizionario `dep` pubblico per OGNI dipendenza
    // (8 chiavi documentate + `"righe-attraversate"`, sopra) — letto da
    // `render/chart.typ` per il content del drawer `dependency`
    // ridefinito (chiamato una volta per dipendenza, mai per riga,
    // D-037: stessa tecnica di `overlay`). Il percorso rapido di
    // default (`render/bars.typ`/`render/dependencies.typ`) legge
    // invece `riga.frecce` (sopra), già spezzato/tradotto per riga.
    dipendenze: dipendenze-pubbliche,
    // RF11/Fase 6b: esposto per `render/drawers.typ` (guardia E-DRW-02
    // sul risultato di un drawer, percorso + locale già risolto, come
    // gli altri usi di `locale` dentro questa funzione sopra) — prima
    // non usciva dal contesto, usato solo internamente.
    locale: locale,
    // RF11/P23: date vere del viewport (NON pixel), esposte al drawer
    // `overlay` (`ctx.viewport`, `render/chart.typ`) — `modello.
    // viewport` è già `(start, end)` istanti effettivi (RF3,
    // `timeline.calcola`).
    viewport: modello.viewport,
  )
}
