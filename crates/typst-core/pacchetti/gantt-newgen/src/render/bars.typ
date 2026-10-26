// gantt-newgen — src/render/bars.typ
// Una riga del corpo (per nodo, RF12): sidebar (etichetta) e area
// barre (griglie, barra, avanzamento, segni di milestone/oggi, overlay
// RF11 P23). Ogni riga disegna SOLO dal proprio contesto
// (`layout.calcola`), senza stato condiviso con le altre righe (D-037):
// autosufficiente, quindi la tabella si spezza su più pagine senza
// ricalcoli.
//
// Prestazioni (S1, rilievo del coordinatore sulla revisione di Fase 5,
// RNF3): lo "sfondo" di una riga (griglie verticali + segni di
// milestone/oggi, `ctx.gruppi-griglia`/`ctx.gruppi-segni`) è IDENTICO
// per ogni riga della STESSA altezza — costruito una volta per altezza
// (da `render/chart.typ`, raggruppate per valore, Fase 6a/D-102: non
// più solo due `kind`, perché `style.levels[l].bar.height` può variare
// per livello) e riusato come lo STESSO `content` per tutte le righe di
// quell'altezza, invece di ricostruire gli stessi `curve()` riga per
// riga.
//
// R1 (D-100 punto 9, D-102): la geometria della tacca "instant" (D5) e
// dell'`inset-sopra` della sidebar, prima calcolate QUI a ogni riga,
// sono ora precalcolate in `layout.calcola` (`riga.segmenti-stato` già
// con la tacca applicata, `riga.inset-sopra`): questo modulo legge solo
// il risultato, nessun ricalcolo (RNF5). La guardia `if riga.stato !=
// "none" { ... }` che avvolgeva i due cicli di disegno (barra +
// avanzamento) è stata TOLTA: `layout.calcola` garantisce ora che
// `segmenti-stato`/`segmenti-progresso` siano ENTRAMBI vuoti quando
// `stato == "none"` (un nodo clampato fuori viewport), quindi i due
// `for` sotto non disegnano comunque nulla in quel caso — la guardia
// era ridondante (controprova nel verbale D-102).
//
// RF11/Fase 6b (D-035/D-040): il CONTENUTO della cella (nome in
// sidebar, segmento di barra) non è più cablato qui — viene da
// `drawers.label(node)`/`drawers.bar(seg)` (default identici al
// disegno di Fase 5/6a, `src/render/drawers.typ`), con la guardia
// `drawers.disponi()` (E-DRW-02) applicata al risultato prima di
// disporlo. Questo modulo resta responsabile SOLO del posizionamento
// (`place(dx:, dy:)`) e della geometria che non dipende dal drawer
// (box della cella, stroke/fill della riga, sfondo).

// Suggerimento (review di Fase 6b): `voce-stile` non serve più qui per
// il caso GENERALE — `s.altezza` (sotto, da `layout.segmenti-stato`)
// porta già il risultato, nessun secondo calcolo nel render (RNF5).
// Fase 6c (rilievo del benchmark S5, ipotesi (a) del coordinatore): il
// percorso rapido sotto (`_bar-stato-fast`/`sidebar-cella`/il tratto di
// avanzamento in `riga-corpo`) riproduce il drawer `bar`/`label` di
// DEFAULT (`drawers.typ`) senza l'indirection generica (`chiama`/
// `disponi`) quando l'utente non li ha ridefiniti — DS-3 (revisione del
// 2026-10-17 su 6b/6c): chiamando le STESSE primitive scalari del
// drawer (`drawers-mod.nome-content`/`.bar-stato`/`.bar-progress`),
// mai più una copia del loro corpo mantenuta a mano qui — nessun
// bisogno di `voce-stile` diretto in questo modulo.
#import "drawers.typ" as drawers-mod
#import "dependencies.typ" as dependencies-mod

/// Sfondo di una riga (griglie verticali + segni di milestone/oggi) per
/// una data ALTEZZA: un `curve()` per gruppo di stroke (D-097), niente
/// altro — calcolato una volta per altezza (sopra), non per riga.
#let sfondo-riga(ctx, altezza) = {
  let gruppi = ctx.gruppi-griglia + ctx.gruppi-segni
  if gruppi.len() == 0 { return [] }
  gruppi.map(g => {
    let componenti = ()
    for gx in g.xs {
      componenti.push(curve.move((gx, 0pt)))
      componenti.push(curve.line((gx, altezza)))
    }
    place(top + left, dx: 0pt, dy: 0pt, curve(stroke: g.stroke, ..componenti))
  }).join()
}

/// Dizionario `node` passato ai drawer `label`/`bar` (RF11): chiavi
/// ENGLISH (interfaccia utente, D-035), costruito dai campi italiani
/// della riga (`layout._appiattisci`). Versionato "solo aggiunte"
/// (RF11): nuove chiavi non rompono un drawer esistente.
#let _nodo-pubblico(riga) = (
  name: drawers-mod.nome-content(riga),
  level: riga.level, path: riga.path, id: riga.id,
  "has-children": riga.has-children, x: riga.x,
)

/// Cella SIDEBAR di una riga del corpo: nome del nodo, prodotto dal
/// drawer `label` (RF11, default = passthrough del content già pronto
/// per il render, `riga.nome-sidebar`, R5/D-108 — DECISIONE UTENTE
/// 2026-10-16, sostituisce l'ellissi di D-102/D-100 S3), rientro per
/// livello (`riga.indent-sidebar`, P12/RF5), stile
/// `riga.stile-testo-sidebar` (per livello, D-102; prima solo per
/// `kind`). `larghezza-sidebar` (`layout.calcola`) è già scelta per
/// contenere il nome su una riga (misurata con lo STESSO stile usato
/// qui, RNF3) quando `sidebar-width` è `auto`; quando è esplicito e il
/// nome NON entra, `riga.nome-sidebar` è già andato a capo (con
/// `linebreak()`, `layout.nome-sidebar-pronto`) e `riga.altezza` è GIA'
/// cresciuta a sufficienza per contenerlo (`layout.posiziona`): questo
/// `box` resta comunque una rete di sicurezza (`clip: true`) per il
/// caso degenere di un `content` non-stringa troppo largo (RF2, non
/// spezzabile in "parole" — limite noto, documentato in `layout.typ`).
// **S1 leva 3 / S3 (rilievo del coordinatore):** niente `align()` né
// `pad()` — la causa precisa del trabocco di D-097 bug 2 è `align()`
// dentro un box/cella a dimensioni fisse (verificato con una sonda
// dedicata: `box(width:, clip: true)` SENZA `align()` ritaglia
// correttamente, anche se non va a capo — resta su una riga e ritaglia
// in orizzontale, mai verticale, quindi mai sulla riga sopra), non
// `box` contro `block` in generale. Il rientro per livello e il
// centraggio verticale diventano `inset` (sinistro = inset base +
// rientro; sopra = metà dello spazio residuo, misurato una volta in
// `layout.calcola`, `riga.inset-sopra`): niente chiamata a
// `align()`/`pad()` per riga.
// **S2(a) (decisione estetica del coordinatore):** niente riquadro
// PER CELLA — solo il bordo sinistro sempre, sopra sulla PRIMA riga,
// sotto sull'ULTIMA (`primo`/`ultimo`, coerente col bordo unico
// dell'area, D2): il risultato è un unico rettangolo che avvolge
// l'intera sidebar, con separatori orizzontali interni dati dal
// divisore RISOLTO PER LIVELLO (`riga.divisore`, P14/RF5, D-102: prima
// sempre `style.gridlines.task-dividers` tal quale).
#let sidebar-cella(riga, ctx, stile-risolto, drawers, primo: false, ultimo: false) = {
  let s = stile-risolto.sidebar
  // Fase 6c (rilievo del benchmark S5, ipotesi (a) del coordinatore):
  // il drawer `label` di DEFAULT (`drawers.typ: label(node) = node.
  // name`) e' l'IDENTITA' pura su `node.name`, che e' sempre
  // `text(..riga.stile-testo-sidebar)[#riga.nome-sidebar]` (costruito
  // da `_nodo-pubblico`, sotto) -- quando l'utente non l'ha ridefinito
  // (confronto per RIFERIMENTO di funzione, `==`: Typst lo confronta
  // per identita', non per struttura, verificato con una sonda
  // dedicata), si ottiene lo STESSO content SENZA costruire l'intero
  // dizionario `node` (gli altri 5 campi — level/path/id/has-children/x
  // — sarebbero comunque ignorati da questo drawer) e senza
  // l'indirection di `chiama`/`disponi` (dict-lookup + chiamata di
  // funzione + le sue guardie di tipo): risultato IDENTICO per
  // costruzione, non un secondo calcolo (RNF5).
  let contenuto = if drawers.at("label") == drawers-mod.label {
    drawers-mod.nome-content(riga)
  } else {
    drawers-mod.chiama(drawers, "label", ctx.locale, _nodo-pubblico(riga))
  }
  // S2(f): `align` di default `left` resta sul percorso veloce (solo
  // `inset`, niente `align()`, S1 leva 3); un `align` esplicito diverso
  // torna a usare `align()` (ramo più lento ma sicuro: la causa di
  // D-097 bug 2 era il testo non misurato che andava a capo DENTRO UNA
  // RIGA TROPPO BASSA per contenerlo, non `align()` in sé — risolta
  // alla radice da R5/D-108: quando il nome va a capo, `riga.altezza`
  // cresce apposta per contenerlo, PRIMA che `align()`/il `box` la
  // vedano, quindi non c'è più nulla da tagliare, T34 o non T34).
  // Rilievo del coordinatore (rereview di Fase 5): `riga.sidebar-align`
  // è già la componente ORIZZONTALE normalizzata e tipo-controllata
  // (`layout.calcola`/`style.allineamento-orizzontale`/`sidebar-align-
  // livello`, E-VAL-03) — mai più `s.align + horizon` qui (panic con un
  // `align` già 2D, es. `center + horizon` esplicito — verificato con
  // una controprova: "cannot add a vertical and a 2D alignment" — o con
  // un valore non-`alignment`, D-099+1). **D-106 punto (d):** per
  // RIGA, non più uniforme per tutto il documento — `style.levels[l].
  // sidebar.align` (se presente) vince su `style.sidebar.align`.
  // Suggerimento (rereview di Fase 6a, D-106): con `align` diverso da
  // `left`, `align(.. + horizon, ..)` centra GIA' verticalmente da solo
  // dentro lo spazio disponibile — sommargli ANCHE `inset: (top:
  // riga.inset-sopra, ..)` (pensato per il percorso SENZA `align()`)
  // ridurrebbe lo spazio utile e sposterebbe il centro vero più in
  // basso (doppio centraggio). `inset-sopra` si applica SOLO nel ramo
  // `left` (dove non c'è `align()` a farlo).
  let corpo = if riga.sidebar-align == left { contenuto } else { align(riga.sidebar-align + horizon, contenuto) }
  let inset-sopra-riga = if riga.sidebar-align == left { riga.inset-sopra } else { 0pt }
  box(
    width: 100%, height: riga.altezza, clip: true,
    stroke: (
      left: s.stroke, top: if primo { s.stroke } else { none },
      bottom: if ultimo { s.stroke } else { riga.divisore },
    ),
    fill: s.at("fill"),
    inset: (left: s.inset + riga.indent-sidebar, top: inset-sopra-riga, right: s.inset),
    corpo,
  )
}

/// Cella AREA BARRE di una riga del corpo: sfondo (griglie verticali +
/// segni di milestone/oggi, precalcolato una volta per altezza dal
/// chiamante — `sfondo`, S1 leva 1), barra (stato RF3: uncompleted /
/// completed-early / completed-late, drawer `bar`), tratto di
/// avanzamento (DC-9, drawer `bar` con `kind: "progress"`) e overlay
/// (RF11 P23, ritagliato per riga come una finestra sul canvas intero
/// — D-037 — SOLO se `overlay` non è `none`, nessun costo extra per
/// riga nel caso comune).
// Niente `clip: true`: il divisore/bordo inferiore (sotto) è ora parte
// dello `stroke` del box stesso (S1 leva 2), non un tratto separato
// che un `clip` stretto rischierebbe di tagliare a metà; tutte le
// altre coordinate sono già dentro `[0, larghezza-area] × [0, altezza]`
// per costruzione (`layout.typ`).
// **D2/S2 (rilievo del coordinatore):** bordo dell'area barre
// (`style.field`, RF5 "bordo DELL'AREA", singolare) come un rettangolo
// UNICO: sinistra/destra su OGNI riga, sopra solo sulla prima, sotto
// solo sull'ultima (altrimenti sotto resta il divisore risolto per
// livello, `riga.divisore`, P14/D-102).
// Fase 6c (rilievo del benchmark S5, ipotesi (a) del coordinatore): il
// drawer `bar` di DEFAULT (`drawers.typ`), per un segmento di STATO
// (non `"progress"`), chiama `drawers-mod.bar-stato` -- STESSA funzione
// usata dal corpo di `drawers.bar` per quel ramo (DS-3, revisione del
// 2026-10-17 su 6b/6c: prima era una copia letterale del suo corpo, ora
// e' la stessa funzione, fonte unica per costruzione), qui chiamata sul
// percorso rapido (confronto per RIFERIMENTO di funzione, come
// `sidebar-cella` sopra) senza costruire il dizionario `seg` (6 chiavi,
// comprese `node`/`interval` che questo drawer non legge mai) né
// passare da `chiama`/`disponi`.
#let _bar-stato-fast(s, stile-barra) = drawers-mod.bar-stato(s.end - s.start, s.altezza, stile-barra, s.stato, s.parte)

// RF10/RF11, Fase 7 (D-115: fast path per identità di funzione, come
// `bar-e-default`/`label` sopra): il percorso rapido di default
// (`dependencies-mod.disegna-riga-default`, D-037: tratti spezzati per
// riga, NESSUN `box(clip:)`) si usa quando l'utente non ha ridefinito
// `dependency` (confronto per RIFERIMENTO con `drawers-mod.dependency`,
// Typst confronta le funzioni per identità, D-115/`render/bars.typ`).
// Un `dependency` ridefinito (compresa l'assenza, `drawers: (dependency:
// none)`: `none` non è MAI `== drawers-mod.dependency`, una funzione)
// passa dal percorso "custom" (`dipendenze-contenuto`, costruito UNA
// VOLTA da `render/chart.typ`, mai qui — RNF3, come l'`overlay`).
// **Commento corretto dopo S6** (D-121, la stesura precedente era
// SBAGLIATA sul caso `dependency: none`): con `dependency: none` NON
// è l'elenco in ingresso a `disegna-riga-custom` a essere vuoto — ha
// una voce per dipendenza con `content: none`, che la funzione salta
// (`continue`, `render/dependencies.typ`) — mentre con "nessuna
// dipendenza nello spec" l'elenco è invece VERAMENTE vuoto
// (`ctx.dipendenze` lo è). Nei DUE casi il risultato finale è
// comunque `none`, STESSO valore esatto del percorso di default
// quando `riga.frecce` è vuoto (`unit/render-dependencies.typ`).
#let riga-corpo(
  riga, ctx, stile-risolto, drawers, sfondo: none, overlay: none,
  dipendenze-contenuto: (), primo: false, ultimo: false,
) = {
  let bar-e-default = drawers.at("bar") == drawers-mod.bar
  // `node` (RF11, letto SOLO da un drawer `bar` ridefinito: il default
  // non lo legge mai, vedi sopra) -- costruito solo se serve davvero
  // (RNF3/RNF5): zero `text()`/dizionari sprecati nel percorso comune.
  let node = if bar-e-default { none } else { _nodo-pubblico(riga) }
  box(
    width: ctx.larghezza-area, height: riga.altezza,
    stroke: (
      left: stile-risolto.field.stroke, right: stile-risolto.field.stroke,
      top: if primo { stile-risolto.field.stroke } else { none },
      bottom: if ultimo { stile-risolto.field.stroke } else { riga.divisore },
    ),
    fill: stile-risolto.field.at("fill"),
    {
      sfondo

      // R1 (D-102): niente più `if riga.stato != "none" { ... }` -- i
      // due array sotto sono GIA' entrambi vuoti in quel caso
      // (`layout.calcola`), quindi i `for` non disegnano nulla: la
      // vecchia guardia era ridondante (verbale D-102).
      for s in riga.segmenti-stato {
        // D5 (rilievo del coordinatore, geometria spostata in
        // `layout.typ` da R1/D-102): la tacca "instant" (larghezza
        // minima = altezza della barra, centrata sul punto) è già
        // applicata a `s.start`/`s.end` da `layout.segmenti-stato`.
        // Suggerimento (review di Fase 6b): `s.altezza` già calcolata
        // da `layout.segmenti-stato` (la stessa `voce.height` usata
        // per la soglia della tacca) -- nessun secondo `voce-stile(..)`
        // qui (RNF5).
        let contenuto = if bar-e-default {
          _bar-stato-fast(s, riga.stile-barra)
        } else {
          let seg = (
            width: s.end - s.start, height: s.altezza,
            kind: s.parte, state: s.stato, style: riga.stile-barra, node: node, interval: s.intervallo,
          )
          drawers-mod.chiama(drawers, "bar", ctx.locale, seg)
        }
        if contenuto != none {
          place(top + left, dx: s.start, dy: riga.altezza / 2 - s.altezza / 2, contenuto)
        }
      }
      for p in riga.segmenti-progresso {
        // Suggerimento (rereview di Fase 6a, D-106): `p.end >= p.start`
        // e' ora un invariante GARANTITO alla fonte (`layout.tratto-
        // progresso`, unico produttore di questi segmenti) — niente
        // piu' `calc.max` ricalcolato qui ad ogni riga (RNF5): la
        // guardia vive una volta sola a monte.
        let contenuto = if bar-e-default {
          // Default `bar(seg)` per `kind: "progress"` (`drawers.typ`):
          // STESSA primitiva `drawers-mod.bar-progress` del ramo
          // `"progress"` del drawer (DS-3): una `line` orizzontale con
          // lo stroke di `style.progress`.
          drawers-mod.bar-progress(p.end - p.start, riga.stile-barra)
        } else {
          let seg = (
            width: p.end - p.start, height: 0pt,
            kind: "progress", state: riga.stato, style: riga.stile-barra, node: node, interval: none,
          )
          drawers-mod.chiama(drawers, "bar", ctx.locale, seg)
        }
        if contenuto != none {
          place(top + left, dx: p.start, dy: riga.altezza / 2, contenuto)
        }
      }

      // RF10 (D-037): strato delle frecce di dipendenza, fra le barre e
      // i segni di milestone/oggi (già disegnati PRIMA, dentro `sfondo`,
      // RF10 "ordine degli strati"). Percorso rapido per identità di
      // funzione come `bar-e-default`/`label` sopra (D-115/DS-3).
      // `contenuto-frecce` è GIA' auto-posizionato (ogni forma nel
      // proprio `place()`, `render/dependencies.typ`): nessun `place()`
      // ulteriore qui attorno (GOTCHA di tabella documentato lì — un
      // `place()` esterno su un content che CONTIENE già più elementi
      // piazzati indipendentemente non li sposterebbe, ma non serve e
      // rischierebbe di reintrodurre lo stesso bug se la forma cambia).
      let contenuto-frecce = if drawers.at("dependency") == drawers-mod.dependency {
        dependencies-mod.disegna-riga-default(riga)
      } else {
        dependencies-mod.disegna-riga-custom(riga, ctx, dipendenze-contenuto)
      }
      if contenuto-frecce != none {
        contenuto-frecce
      }

      // RF11/P23: overlay globale, già costruito UNA VOLTA da
      // `render/chart.typ` (il drawer riceve tutta la geometria, non
      // per riga, RNF3) — qui si dispone solo la "finestra" di questa
      // riga sul canvas intero (D-037, stessa tecnica prevista per un
      // drawer `dependency` ridefinito, Fase 7): `box(clip: true)`
      // della sola altezza della riga, col canvas spostato in su di
      // `riga.y0` perché le coordinate del drawer sono relative
      // all'origine del corpo, non della riga. Suggerimento (review di
      // Fase 8.4, T44): il `box` è avvolto nel proprio `place()` (come
      // `render/dependencies.typ`), invece di comparire NUDO nella
      // sequenza — qui è l'ULTIMO elemento quindi non impilava nulla
      // (nessun difetto osservabile oggi), ma un elemento aggiunto DOPO
      // in futuro impilerebbe sotto di lui senza questo `place()`:
      // stessa regola "un `place()` per forma" del resto del modulo,
      // applicata per difesa in profondità, non per un bug reale.
      if overlay != none {
        place(top + left, dx: 0pt, dy: 0pt, box(
          width: ctx.larghezza-area, height: riga.altezza, clip: true,
          place(top + left, dx: 0pt, dy: -riga.y0, overlay),
        ))
      }
    },
  )
}
