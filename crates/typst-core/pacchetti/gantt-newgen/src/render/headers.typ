// gantt-newgen — src/render/headers.typ
// Righe d'intestazione (RF4) e griglie verticali di livello: content
// puro a partire dal contesto GIA' calcolato (`layout.calcola`, RNF5 —
// nessun `measure`, nessun ricalcolo di date/segmenti qui). Una riga
// per livello non soppresso (`min-cell-width`, D-028); `table.header
// (repeat: true)` la ripete su ogni pagina (RF12).

/// Cella AREA di un livello d'intestazione: un `box` con un `place()`
/// per etichetta e uno per ogni confine di segmento con griglia. **D-106
/// punto (a) — "headers a dizionario per livello":** `livello.align`/
/// `.text`/`.fill`/`.stroke`/`.gridline`/`.altezza`/`.inset`/`.overflow`
/// sono GIA' risolti da `layout.intestazioni` (`style.livello-header-
/// risolto`, precedenza `headers[i].<campo>` > `style.headers.
/// <unità>.<campo>` > definizione d'unità > default globale) — questo
/// modulo legge solo il risultato, nessun ricalcolo (RNF5).
/// **`overflow` (RF4):** `"hide"` (default) nasconde — non disegna —
/// un'etichetta che non ci sta (`seg.nascondi`, già deciso da `layout.
/// intestazioni`, mai vero per `"clip"`/`"show"`); `"clip"` la disegna
/// comunque ma RITAGLIATA alla cella (un `box` per segmento con `clip:
/// true`, non solo il box esterno dell'intero livello); `"show"` la
/// disegna comunque SENZA ritaglio PER SEGMENTO (può traboccare
/// visivamente nelle celle vicine dello STESSO livello, scelta
/// esplicita dell'utente). **Limite noto, non un difetto** (suggerimento,
/// review finale di Fase 6a): questo `box` ESTERNO (sotto, l'intero
/// livello) ha comunque `clip: true` — un'etichetta `"show"` che
/// trabocca OLTRE il bordo del LIVELLO (non solo della propria cella,
/// es. l'ultima etichetta a destra che sborda fuori dall'area barre)
/// viene comunque tagliata lì, non prosegue visivamente oltre: "show"
/// significa "non ritagliata alla CELLA del segmento", non "mai
/// ritagliata in assoluto".
#import "../layout.typ": griglia-visibile
#import "drawers.typ" as drawers-mod

#let _cella-area(livello, ctx, drawers, primo: false, ultimo: false) = box(
  width: ctx.larghezza-area, height: livello.altezza, clip: true,
  fill: livello.fill,
  // R4 (D-100 punto E4, D-102): bordo a rettangolo UNICO, stessa
  // logica di D2 (`render/bars.typ`): destra SEMPRE (chiude il lato
  // destro su ogni livello, come il campo barre), sopra SOLO sul primo
  // livello (il più grossolano, in alto), sotto SOLO sull'ultimo —
  // cosi' l'angolo in alto a destra del BLOCCO intestazione (sidebar +
  // aree) risulta chiuso: sul primo livello sopra+destra sono entrambi
  // presenti. Sinistra SEMPRE: e' il confine condiviso con la cella
  // sidebar (sotto, che non disegna piu' il proprio lato destro — un
  // solo bordo per quel confine, non due sovrapposti, come in D2).
  stroke: (
    left: livello.stroke, right: livello.stroke,
    top: if primo { livello.stroke } else { none },
    bottom: if ultimo { livello.stroke } else { none },
  ),
  {
    for seg in livello.segments {
      if seg.nascondi { continue }
      let larghezza-cella = seg.x1 - seg.x0
      // RF4/RF11 (Fase 6b, D-035/D-040): `seg` del drawer `header-cell`
      // porta i campi di stile risolti per QUESTO livello (align/
      // text/fill/stroke/gridline/overflow/inset) più la geometria
      // della CELLA (width/height) — il `label` (secondo argomento) è
      // il content già formattato (`format` di RF4, applicato a monte
      // in `layout.intestazioni`). Default = IDENTICO al disegno di
      // Fase 4/6a (`drawers.header-cell`).
      // Review di Fase 6b (specification.md RF11, D-113, "scarto dal
      // contratto, aperto" chiuso qui): «`seg` come RF4» aggiunge
      // `start`/`end` (date vere), `days` (durata intera, T2),
      // `index` (posizione nel livello, 0-based), `partial` (già
      // `seg.partial`, D-037: il segmento è tagliato da un bordo del
      // viewport) e `locale` (il locale risolto, per un drawer che
      // vuole formattare date/numeri senza plumbing separato) — tutti
      // GIÀ calcolati da `layout._intestazioni-grezze` (RNF5, nessun
      // nuovo calcolo qui). **Invariante (suggerimento, review di Fase
      // 8.4, asserito in `unit/layout.typ`, `casi-s3-invariante-rf11`):**
      // ogni livello che arriva FIN QUI (non soppresso, per via del
      // filtro di `render/headers.typ: intestazione()`) porta SEMPRE
      // queste 4 chiavi — un livello soppresso per larghezza esce PRESTO
      // da `_intestazioni-grezze` SOLO se non è il livello 0 (`if
      // indice-livello > 0 and soppresso-per-larghezza { .. }`: il
      // livello 0 prende sempre il ramo pieno, perché la guardia E6
      // potrebbe doverlo forzare a visibile), quindi un `seg.inizio` qui
      // non può mai incontrare un segmento "ridotto" senza quella
      // chiave.
      let seg-drawer = (
        unit: livello.at("unit", default: none), align: livello.align, text: livello.text,
        fill: livello.fill, stroke: livello.stroke, gridline: livello.gridline,
        overflow: livello.overflow, inset: livello.inset,
        width: larghezza-cella, height: livello.altezza,
        start: seg.inizio, end: seg.fine, days: seg.giorni, index: seg.indice,
        partial: seg.partial, locale: ctx.locale,
      )
      let etichetta = drawers-mod.chiama(drawers, "header-cell", ctx.locale, seg-drawer, seg.label)
      if etichetta == none { continue }
      let corpo = if livello.overflow == "clip" {
        box(width: larghezza-cella, height: livello.altezza, clip: true, etichetta)
      } else {
        // "hide" (gia' filtrato sopra, `seg.nascondi` mai vero qui) e
        // "show" (nessun ritaglio, puo' traboccare: scelta dell'utente).
        box(width: larghezza-cella, height: livello.altezza, etichetta)
      }
      place(top + left, dx: seg.x0, dy: 0pt, corpo)
    }
    // L1 (D-105, leva di prestazioni): un solo `curve()` per livello con
    // TUTTI i confini di segmento (sotto-componenti `curve.move`/
    // `curve.line` disgiunti), non un `place(line())` per confine —
    // stessa tecnica, stesso risultato geometrico (segmenti verticali
    // disgiunti, nessuna differenza visiva), di `bars.sfondo-riga`
    // (D-097): costruisce UN frame invece di N frame indipendenti.
    // Fase 6b Parte B (S5, review: corretta): `griglia-visibile`
    // esclude `x = 0` SOLO se il bordo sinistro della cella
    // (`livello.stroke`, PER LIVELLO: può differire dal corpo) è
    // davvero disegnato lì — altrimenti (`livello.stroke: none`)
    // resterebbe senza alcuna linea su quel lato (RNF5: funzione pura
    // in `layout.typ`, nessun nuovo calcolo qui, solo un filtro).
    if livello.gridline != none {
      let componenti = ()
      for gx in griglia-visibile(livello.segments, livello.stroke != none) {
        componenti.push(curve.move((gx, 0pt)))
        componenti.push(curve.line((gx, livello.altezza)))
      }
      place(top + left, dx: 0pt, dy: 0pt, curve(stroke: livello.gridline, ..componenti))
    }
  },
)

/// Cella SIDEBAR dell'intestazione: vuota (nessuna etichetta per
/// livello nella sidebar, Fase 6b/RF11 drawer "label"; nessun contenuto
/// cablato, solo lo stile della cornice). **S2(b) (decisione estetica
/// del coordinatore):** UNA sola cella con `rowspan` = numero di
/// livelli NON soppressi, non una per livello — senza bordo interno fra
/// i livelli (il blocco in alto a sinistra è un unico riquadro, come
/// l'originale). **R4 (D-102):** niente lato destro qui (il confine con
/// l'area lo disegna `_cella-area`, sinistra sempre, come in D2) —
/// sinistra/sopra/sotto SEMPRE (questa cella è un blocco unico alto
/// quanto TUTTA l'intestazione, quindi "sopra solo sul primo livello" e
/// "sotto solo sull'ultimo" coincidono qui con "sempre"). `altezza-
/// totale` (D-106 punto (a)): la SOMMA delle altezze dei livelli
/// visibili (non più `n × un'unica altezza` — un livello può avere
/// un'altezza propria, `headers[i].height`/`style.headers.<unità>.
/// height`). **Stroke GLOBALE, non per livello** (suggerimento, review
/// finale di Fase 6a): questa cella usa SEMPRE `stile-risolto.headers.
/// stroke` (il default globale), MAI `livello.stroke` di un livello
/// specifico — anche quando un `headers[i].stroke`/`style.headers.
/// <unità>.stroke` fa variare il bordo di `_cella-area` per quel
/// livello (sopra), il blocco sidebar dell'intestazione (un UNICO
/// riquadro per TUTTI i livelli, S2(b)) resta con un bordo uniforme:
/// non avrebbe senso un bordo "a strisce" per livello su una cella che
/// non ha contenuto per livello.
// Fase 6b Parte B (ritocco estetico, decisione utente): `fill` riusa
// `style.sidebar.fill` (nessuna nuova chiave) — cosi' un `sidebar.fill`
// dell'utente colora l'INTERA colonna sinistra (angolo dell'intestazione
// compreso), non solo le celle del corpo. Default `none` (invariato,
// come ogni altro `fill` del pacchetto): nessun effetto finché l'utente
// non lo imposta.
#let _cella-sidebar(ctx, stile-risolto, altezza-totale) = box(
  width: ctx.larghezza-sidebar, height: altezza-totale,
  stroke: (
    left: stile-risolto.headers.stroke, top: stile-risolto.headers.stroke, bottom: stile-risolto.headers.stroke,
  ),
  fill: stile-risolto.sidebar.at("fill"),
)

/// `table.header(repeat: true)` con una riga per livello NON soppresso.
#let intestazione(ctx, stile-risolto, drawers) = {
  let livelli = ctx.intestazioni.filter(l => not l.soppresso)
  let n = livelli.len()
  let ultimo-indice = n - 1
  let celle = livelli
    .enumerate()
    .map(((i, l)) => {
      let area = table.cell(breakable: false, _cella-area(l, ctx, drawers, primo: i == 0, ultimo: i == ultimo-indice))
      if i == 0 {
        (
          table.cell(
            rowspan: n, breakable: false,
            _cella-sidebar(ctx, stile-risolto, ctx.at("altezza-header-totale")),
          ),
          area,
        )
      } else {
        (area,)
      }
    })
    .flatten()
  table.header(repeat: true, ..celle)
}
