// gantt-newgen — src/render/drawers.typ
// Drawers (RF11, Fase 6b): personalizzazione per elemento. Equivalente
// migliorato dell'API "drawers" di gantty 0.5.x (D-035): invece di
// sostituire interi strati, si sostituisce il content di un SINGOLO
// elemento. Ogni drawer riceve dati PURI in `pt` (già calcolati da
// `src/layout.typ`, RNF5: nessun nuovo `measure`/calcolo qui) e
// restituisce `content`, `str` o `none` — la guardia `disponi()` sotto
// (E-DRW-02, D-040) lo verifica prima che il chiamante lo disponga.
//
// I SEI drawer di default qui riproducono ESATTAMENTE lo stesso disegno
// di Fase 5/6a quando l'utente non ridefinisce nulla: il disegno in sé
// non si è spostato, solo la sua costruzione è passata da un punto
// unico e ridefinibile. `default-drawers` (sotto) è esportato da
// `lib.typ`; `gantt()` lo unisce con `drawers:` dell'utente
// (`default-drawers + drawers-utente`, l'ultimo vince — D-040: il merge
// con `(bar: none)` disattiva l'elemento).

#import "../errors.typ": fail, nome-tipo
#import "../layout.typ": voce-stile

/// Guardia E-DRW-02 (D-040): il valore restituito da un drawer deve
/// essere `content`, `str` o `none` — altrimenti `panic` col catalogo
/// RF7 (`percorso` = `"drawers.<nome>"`), invece dell'errore grezzo di
/// Typst ("expected content, found integer"). Una stringa diventa
/// `content` (`[#valore]`) PRIMA di tornare al chiamante: il resto del
/// render (sempre `place()`/`box()` su `content`) non deve più
/// distinguere i due casi. `none` passa invariato (l'elemento si
/// disattiva, nessun `place()` a valle).
#let disponi(valore, percorso, locale) = {
  if valore == none { return none }
  if type(valore) == str { return [#valore] }
  if type(valore) == content { return valore }
  fail("E-DRW-02", percorso, locale, received: nome-tipo(valore))
}

/// Chiama il drawer `nome` di `drawers` con `..args`, guardato (C1,
/// review di Fase 6b): un valore `none` dentro `drawers:` (D-040,
/// "none disattiva l'elemento", es. `drawers: (bar: none)`) è un
/// dizionario che contiene `none` al posto della funzione — chiamarlo
/// DIRETTAMENTE (`(drawers.at(nome))(..args)`, come prima di questa
/// correzione) fa `panic` NUDO di Typst ("expected function, found
/// none"), mai raggiunto da `disponi()`/E-DRW-02 (quel codice gira
/// SOLO se la chiamata riesce). Qui si controlla PRIMA: `none` ->
/// l'elemento è disattivato, nessuna chiamata, nessun `disponi()`
/// (lo stesso esito di un drawer che RESTITUISCE `none`, coerente con
/// D-040). Un valore diverso da funzione/`none` non arriva mai qui
/// (E-VAL-03, `validate.typ`, verificato all'origine — commento
/// corretto il 2026-10-17, era rimasto "E-DRW-01": quel codice e' per
/// la CHIAVE sconosciuta, S6/`validate.typ`; il VALORE di tipo errato
/// e' E-VAL-03): questa funzione non lo ricontrolla.
/// - drawers (dictionary): drawer EFFETTIVI (`default-drawers +`
///   l'utente, già uniti da `gantt()`).
/// - nome (string): uno dei sei nomi di `schema.chiavi-drawer`.
/// - locale (dictionary): locale risolto, per l'eventuale E-DRW-02.
/// - ..args (arguments): argomenti POSIZIONALI del drawer (`node` per
///   `label`, `seg` per `bar`, `m` per `milestone`, `(seg, label)` per
///   `header-cell`, `ctx` per `overlay`/`dependency`).
/// -> content|none
#let chiama(drawers, nome, locale, ..args) = {
  let fn = drawers.at(nome)
  if fn == none { return none }
  disponi(fn(..args), "drawers." + nome, locale)
}

// --- label (RF11) ---------------------------------------------------------

/// Content dell'etichetta nella sidebar, a partire dai soli campi
/// scalari di una riga (`riga.stile-testo-sidebar`/`riga.nome-sidebar`):
/// DS-3 (revisione del 2026-10-17 su 6b/6c), estratta qui perché sia il
/// drawer `label` di default (sotto) sia il percorso rapido di
/// `render/bars.typ` (`sidebar-cella`, Fase 6c) devono produrre
/// ESATTAMENTE lo stesso content — fonte UNICA per costruzione, non due
/// copie da tenere sincronizzate a mano.
#let nome-content(riga) = text(..riga.stile-testo-sidebar)[#riga.nome-sidebar]

/// Drawer di default dell'etichetta nella sidebar: `node.name` è GIÀ il
/// content pronto per il render (stile/rientro per livello, eventuale
/// a-capo R5/D-108 — `render/bars.typ` lo costruisce con `nome-content`
/// prima di chiamare il drawer, RNF5: nessun nuovo `measure` qui). Il
/// default è l'identità pura: passa il content attraverso senza
/// modificarlo.
#let label(node) = node.name

// --- bar (RF3/RF11, DC-9) --------------------------------------------------

/// Segmento di STATO (`"uncompleted"`/`"body"`/`"timeframe"`), a partire
/// dai soli valori scalari (`width`/`height` in pt, `stile-barra`
/// risolto, `stato`/`parte` del segmento): DS-3, estratta dal corpo del
/// ramo "non progress" di `bar(seg)` (sotto) perché `render/bars.typ`
/// (`_bar-stato-fast`, Fase 6c) riproduce lo stesso disegno senza
/// costruire il dizionario `seg` — stessa funzione, non una copia.
#let bar-stato(width, height, stile-barra, stato, parte) = {
  let voce = voce-stile(stile-barra, stato, parte)
  box(width: width, height: height, fill: voce.fill, radius: stile-barra.radius)
}

/// Tratto di avanzamento (DC-9), a partire dai soli valori scalari
/// (`width` in pt, `stile-barra` risolto, per `stile-barra.progress.
/// stroke`): DS-3, estratta dal ramo `"progress"` di `bar(seg)` (sotto)
/// per lo stesso motivo di `bar-stato` — riusata dal percorso rapido di
/// `render/bars.typ`.
#let bar-progress(width, stile-barra) = line(start: (0pt, 0pt), end: (width, 0pt), stroke: stile-barra.progress.stroke)

/// Drawer di default di UN segmento di barra: per i segmenti di STATO
/// (`seg.kind` = `"uncompleted"`/`"body"`/`"timeframe"`) un `box` pieno
/// (`fill`/`radius` dalla voce di stile risolta per `(seg.state,
/// seg.kind)`, RF5, `bar-stato` sopra); per il tratto di avanzamento
/// (`seg.kind` = `"progress"`, DC-9) una `line` orizzontale con lo
/// stroke di `seg.style.progress` (`bar-progress` sopra) — IDENTICO al
/// disegno di Fase 5/6a (D-039), solo spostato qui da `render/bars.typ`
/// (che si limita ora a posizionare il content con `place(dx:, dy:)`).
///
/// **Limite noto per `seg.kind == "progress"` (da registrare in RF11,
/// nota per `project-init`):** `seg.state`/`seg.interval` NON sono
/// per-intervallo per il tratto di avanzamento — `tratto-progresso`
/// (`src/layout.typ`, DC-9) produce i suoi segmenti indipendentemente
/// da `segmenti-stato` (due funzioni pure separate, P4): un tratto di
/// progress che attraversa un confine fra due intervalli diversi porta
/// comunque `state: riga.stato` (l'aggregato del NODO, mai quello del
/// singolo intervallo attraversato) e `interval: none` (nessuna
/// associazione a un intervallo specifico). Il default qui sotto non
/// legge né l'uno né l'altro per `"progress"` (solo `seg.style.
/// progress.stroke`), quindi non è osservabile nel disegno di default —
/// un drawer `bar` custom che li legge per un tratto di progress
/// riceve dati non significativi per quel `kind`.
#let bar(seg) = if seg.kind == "progress" {
  bar-progress(seg.width, seg.style)
} else {
  bar-stato(seg.width, seg.height, seg.style, seg.state, seg.kind)
}

// --- milestone (RF11) -------------------------------------------------------

/// Drawer di default dell'etichetta di una milestone/"oggi": `m.name`
/// (data sotto se `m.date-label` non è `none`, S2(c), come l'originale),
/// sfondo OPACO (nasconde un connettore che la attraversa, rereview di
/// Fase 5) e testo centrato (E3). Il grassetto del nome è GIÀ applicato
/// a monte (`layout.calcola`, per una milestone vera; "oggi" non è in
/// grassetto, asimmetria preesistente di Fase 5 — il default la
/// riproduce, non la introduce): questo drawer compone solo
/// `linebreak()`, non aggiunge `strong[..]`. `m.style` porta già
/// `text`/`fill`/`inset` risolti (colore del testo con la precedenza
/// utente sullo stroke, D-099+1): nessuna nuova risoluzione di stile
/// qui, solo disegno.
#let milestone(m) = {
  let corpo = if m.date-label != none {
    [#m.name#linebreak()#m.date-label]
  } else {
    m.name
  }
  box(inset: m.style.inset, fill: m.style.fill, align(center, text(..m.style.text)[#corpo]))
}

// --- dependency (RF10/RF11, Fase 7) -----------------------------------------

/// Un tratto di freccia (RF10): `curve()` da un array di SEGMENTI
/// `(p0, p1)` (non necessariamente consecutivi: ogni coppia riparte
/// con un proprio `curve.move`, quindi una lista con "buchi" — un
/// tratto spezzato per riga che la stessa dipendenza attraversa più
/// volte, caso raro ma possibile con `"bezier"` — resta corretta,
/// nessun segmento fantasma a chiudere il buco). DS-3 (come
/// `bar-stato`/`bar-progress` sopra): fonte UNICA per il drawer di
/// default (sotto, con i segmenti dell'INTERA polilinea `dep.points`)
/// e per il percorso rapido per riga (`render/dependencies.typ`,
/// D-115) — **non** byte-identici fra loro (il percorso rapido disegna
/// un SOTTO-INSIEME di segmenti già spezzato per riga, D-037: stessa
/// primitiva, ambito diverso, mai la stessa `content`), a differenza di
/// `bar-stato`/`nome-content` dove i due percorsi producono esattamente
/// lo stesso `content` (un solo segmento, mai spezzato).
#let dependency-tratto(tratti, stroke-risolto) = {
  let componenti = ()
  for (p0, p1) in tratti {
    componenti.push(curve.move((p0.x, p0.y)))
    componenti.push(curve.line((p1.x, p1.y)))
  }
  curve(stroke: stroke-risolto, ..componenti)
}

/// `dep.points` (polilinea INTERA, N+1 punti) come array di N segmenti
/// `(p0, p1)` CONSECUTIVI — forma richiesta da `dependency-tratto`
/// sopra, usata dal drawer di default (sotto).
#let _polilinea-a-segmenti(punti) = range(0, punti.len() - 1).map(i => (punti.at(i), punti.at(i + 1)))

/// Punta della freccia (RF10): `polygon` dai 3 vertici (apice + base)
/// già calcolati da `layout._punta-triangolo` (geometria pura, mai nel
/// render, RNF5) — `dep.punta`/`riga.frecce.*.punta.punti`, secondo il
/// percorso.
#let dependency-punta(vertici, fill-risolto) = polygon(fill: fill-risolto, ..vertici.map(p => (p.x, p.y)))

/// Drawer di default delle frecce di dipendenza (RF10/D-037): la
/// polilinea INTERA (`dep.points`, relativa all'origine dell'area
/// barre, RF11) più la punta (`dep.punta`), con lo stroke/colore già
/// risolti (`dep.style`, normale o `violated`, D-034 punto 6). Il
/// PERCORSO RAPIDO per identità di funzione (`render/dependencies.typ`,
/// D-115) non chiama mai questa funzione — disegna direttamente
/// `riga.frecce` (già spezzato per riga, zero `box(clip:)`, D-037),
/// con le STESSE primitive sopra: questa funzione serve a un drawer
/// `dependency` RIDEFINITO che vuole richiamare il default (es. per
/// aggiungere una condizione), che altrimenti otterrebbe `none` invece
/// delle frecce — chiamata (se mai chiamata) una volta per dipendenza,
/// disposta per riga attraversata dentro `box(clip: true)`, come
/// `overlay` (RF11, `render/chart.typ`).
/// **T44/GOTCHA (diagnosi CORRETTA dalla review del 2026-10-18, S3 —
/// stesso di `render/dependencies.typ`, NON specifico di `table`,
/// decisioni.md D-121): `A + B` (due content block-level sommati PRIMA
/// di essere piazzati) si impagina in flusso verticale, dentro O fuori
/// da una cella di `table`.** Ogni pezzo va nel proprio `place()`
/// ANCHE qui, non solo nel percorso rapido — altrimenti la punta (il
/// secondo pezzo) si impilerebbe sotto il tratto invece di sovrapporsi.
/// C1 (CRITICO, revisione del 2026-10-18): `dep.style.stroke == none`
/// (decisione del coordinatore, `src/layout.typ`) -> `none`, nessun
/// tratto né punta — stessa regola del percorso rapido (`render/
/// dependencies.typ`), applicata anche qui perché questa funzione può
/// essere richiamata direttamente da un drawer ridefinito.
#let dependency(dep) = if dep.style.stroke == none {
  none
} else {
  (
    place(top + left, dx: 0pt, dy: 0pt, dependency-tratto(_polilinea-a-segmenti(dep.points), dep.style.stroke))
      + place(top + left, dx: 0pt, dy: 0pt, dependency-punta(dep.punta, dep.style.at("fill-punta")))
  )
}

// --- header-cell (RF4/RF11) -------------------------------------------------

/// Drawer di default di una cella d'intestazione: etichetta allineata
/// col proprio inset, nello stile risolto per il livello — IDENTICO al
/// disegno di Fase 4/6a (`render/headers.typ`), solo spostato qui. Il
/// ritaglio per `overflow: "clip"` resta nel render (dipende dalla
/// larghezza della CELLA, non dal content del drawer: un drawer
/// ridefinito che restituisce un content più largo della cella viene
/// ritagliato comunque se `overflow` lo richiede).
#let header-cell(seg, label) = align(seg.align, pad(..seg.inset, text(..seg.text)[#label]))

// --- overlay (RF11, P23) ----------------------------------------------------

/// Drawer di default dell'overlay globale: `none` (nessun content
/// aggiunto). L'unico drawer la cui assenza di default è VISIBILE come
/// "niente" — tutti gli altri, di default, riproducono il disegno
/// consueto.
#let overlay(ctx) = none

/// Drawer di DEFAULT (RF1/RF11, Fase 6b), esportato da `lib.typ`. Fonte
/// unica dei SEI nomi ammessi, assieme a `schema.chiavi-drawer` (usato
/// da `validate.typ` per E-DRW-01): le chiavi qui devono restare
/// esattamente le stesse (verificato in `unit/style.typ`).
#let default-drawers = (
  label: label, bar: bar, milestone: milestone,
  dependency: dependency, "header-cell": header-cell, overlay: overlay,
)
