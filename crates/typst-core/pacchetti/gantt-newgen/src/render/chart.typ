// gantt-newgen — src/render/chart.typ
// Orchestrazione del disegno (RF12): una `table` a due colonne
// (sidebar | area barre) con `table.header(repeat: true)` per le
// intestazioni, una riga per nodo (D-037: cella non spezzabile — un
// `box` è "inline-level" e non si spezza mai fra pagine di suo, quindi
// non serve più avvolgerlo in `table.cell(breakable: false)`: S1 leva
// 4, verificato che la tabella continua a spezzarsi SOLO fra righe,
// mai dentro — ma la TABELLA sì: l'intestazione ripete, le righe si
// spezzano fra pagine) e `table.footer` per le etichette di
// milestone/oggi (D-038). Nessun `measure`/ricalcolo qui (RNF5): tutto
// arriva già pronto da `layout.calcola`.

#import "headers.typ"
#import "bars.typ"
#import "milestones.typ"
#import "drawers.typ" as drawers-mod
#import "dependencies.typ"

/// Campi di `ctx` (il contesto COMPLETO prodotto da `layout.calcola`,
/// con TUTTE le `n` righe/dipendenze/griglie del documento) che
/// `bars.sidebar-cella`/`bars.riga-corpo` — e `dependencies.
/// disegna-riga-custom`, che `riga-corpo` chiama passandole il proprio
/// `ctx` quando il drawer `dependency` è ridefinito, RF11 — usano
/// DAVVERO: SOLO due scalari (RNF3, D-146 addendum "Passo 1"). Passare
/// l'INTERO `ctx` per OGNI riga (invece di questi due campi) costa O(n)
/// PER CHIAMATA anche quando il corpo della funzione non legge gli
/// altri campi — dimostrato con un microbenchmark isolato (una funzione
/// che non fa nulla con l'argomento, chiamata N volte con un array
/// O(n): tempo totale O(n²); con un argomento piccolo costante: tempo
/// costante) e confermato sul codice reale (verbale D-146 addendum
/// "Passo 1", 2026-10-20): n righe × O(n) per chiamata = O(n²), la
/// causa misurata della catena quadratica e di gran parte del costo di
/// S5/S6. Nome PUBBLICO (non `_...`) apposta: letto da
/// `tests/unit/render-chart.typ` per la controprova statica che
/// nessun uso NUOVO di `ctx.<campo>` nelle tre funzioni sopra resti
/// fuori da questa lista (quel test legge il SORGENTE con `read()` e
/// cerca `ctx.<campo>`/`ctx.at("<campo>")` con una regex — **limite
/// noto**: un accesso INDIRETTO, es. `let c = ctx; c.qualcosa`, non
/// verrebbe rilevato, si confida sulla convenzione del modulo di
/// accedere sempre a `ctx.` direttamente).
#let campi-ctx-per-riga = ("locale", "larghezza-area")

/// Costruisce il `ctx` RIDOTTO (solo `campi-ctx-per-riga`, sopra) da
/// passare a `bars.sidebar-cella`/`bars.riga-corpo` per ciascuna riga —
/// fonte UNICA (usata sia da `disegna()` sotto sia dal test di
/// `tests/unit/render-chart.typ` che verifica che non contenga array
/// O(n), cioè che non cresca con la dimensione del documento).
#let riduci-ctx-per-riga(ctx) = {
  let ridotto = (:)
  for campo in campi-ctx-per-riga {
    ridotto.insert(campo, ctx.at(campo))
  }
  ridotto
}

/// Disegna il grafico completo a partire dal contesto di
/// `layout.calcola`, dallo stile risolto e dai drawer EFFETTIVI (RF11,
/// Fase 6b: `default-drawers` già unito con `drawers:` dell'utente da
/// `gantt()`, `lib.typ`).
/// -> content
#let disegna(ctx, stile-risolto, drawers) = {
  // S1 leva 1 (rilievo del coordinatore): lo sfondo di riga (griglie +
  // segni di milestone/oggi) dipende SOLO dall'altezza — costruito una
  // volta per ciascuna altezza DISTINTA, riusato dalla stessa `content`
  // per tutte le righe di quell'altezza (non un `curve()` nuovo per
  // riga). **C1 (rilievo del coordinatore, rereview di Fase 6a, D-102):
  // indicizzato per `r.altezza` (`repr(..)`, non per `r.kind`** — con
  // `style.levels[l].bar.height` (P12, D-102) l'altezza può variare PER
  // LIVELLO, non solo per `kind`: due righe dello stesso `kind` ma di
  // livello/altezza diversi finivano a condividere la STESSA voce della
  // cache (l'ultima scritta per quel `kind`), quindi lo sfondo (griglie/
  // segni di milestone) di una riga più bassa/alta veniva disegnato
  // all'altezza SBAGLIATA — bug silenzioso (nessun errore, nessun
  // avviso), mai esercitato prima perché Fase 5/6a non avevano ancora
  // un caso con più di due altezze reali nello stesso `ok/`. Indicizzare
  // per altezza invece che per `kind` rende il caso degenere (nessun
  // `style.levels`, tutte le righe di uno stesso `kind` hanno la stessa
  // altezza) IDENTICO a prima: stesso numero di voci in cache (al più
  // le altezze distinte, non più dei `kind` distinti in quel caso).
  let sfondo-per-altezza = (:)
  for r in ctx.righe {
    let chiave = repr(r.altezza)
    if chiave not in sfondo-per-altezza {
      sfondo-per-altezza.insert(chiave, bars.sfondo-riga(ctx, r.altezza))
    }
  }

  // RF11/P23 (Fase 6b, D-035/D-037): overlay GLOBALE, chiamato UNA
  // VOLTA (RNF3: nessun ricalcolo per riga), guardato con `disponi()`
  // (E-DRW-02) qui — `none` nel caso comune (default), zero costo per
  // riga (nessun `box(clip:)` extra, `bars.riga-corpo` lo salta). `ctx-
  // overlay` è l'interfaccia PUBBLICA del drawer (RF11): `rows` con una
  // voce per riga disegnabile (`path, id, y, h`), `viewport` le date
  // vere (non il pixel), `x` la stessa chiusura data->pt del contesto.
  let ctx-overlay = (
    width: ctx.larghezza-area, height: ctx.at("altezza-corpo"), x: ctx.x,
    rows: ctx.righe.map(r => (path: r.path, id: r.id, y: r.y0, h: r.altezza)),
    viewport: ctx.viewport,
  )
  let overlay-contenuto = drawers-mod.chiama(drawers, "overlay", ctx.locale, ctx-overlay)

  // RF10/RF11, Fase 7 (S6, revisione del 2026-10-18: estratta in
  // `render/dependencies.typ: contenuti-dipendenze`, fonte UNICA anche
  // per `unit/render-dependencies.typ` — prima il test ricostruiva
  // questa stessa logica a mano, senza esercitare il ramo reale).
  let dipendenze-contenuto = dependencies.contenuti-dipendenze(ctx, drawers)

  // RNF3 (D-146 addendum "Passo 1"/"Passo 2"): `ctx` ridotto, costruito
  // UNA VOLTA (non per riga, vedi `riduci-ctx-per-riga`/`campi-ctx-per-
  // riga` sopra) — evita di passare array O(n) (`ctx.righe`,
  // `ctx.dipendenze`, `ctx.gruppi-griglia`, `ctx.gruppi-segni`, ...) a
  // `sidebar-cella`/`riga-corpo` per CIASCUNA delle `n` righe, che
  // costerebbe O(n) per chiamata (quindi O(n²) in totale) anche se il
  // corpo di quelle funzioni non legge quei campi. Verificato
  // byte-identico su 17 golden + l'esempio con drawer personalizzato +
  // tutti i casi `tests/ok/` (verbale).
  let ctx-ridotto-per-riga = riduci-ctx-per-riga(ctx)

  let ultimo-indice = ctx.righe.len() - 1
  // D2 (rilievo del coordinatore): il bordo dell'area barre (RF5/P26,
  // "bordo DELL'AREA", singolare) e quello della sidebar (S2a) sono un
  // rettangolo UNICO ciascuno, non uno per riga — ogni riga disegna
  // solo il PROPRIO pezzo (sinistra/destra sempre; sopra SOLO la prima
  // riga; sotto SOLO l'ultima, altrimenti `task-dividers`).
  let righe-corpo = ctx.righe
    .enumerate()
    .map(((i, r)) => {
      let primo = i == 0
      let ultimo = i == ultimo-indice
      (
        bars.sidebar-cella(r, ctx-ridotto-per-riga, stile-risolto, drawers, primo: primo, ultimo: ultimo),
        bars.riga-corpo(
          r, ctx-ridotto-per-riga, stile-risolto, drawers,
          sfondo: sfondo-per-altezza.at(repr(r.altezza)), overlay: overlay-contenuto,
          dipendenze-contenuto: dipendenze-contenuto, primo: primo, ultimo: ultimo,
        ),
      )
    })
    .flatten()
  let piede = milestones.etichette(ctx, stile-risolto, drawers)
  table(
    columns: (ctx.larghezza-sidebar, ctx.larghezza-area),
    column-gutter: 0pt, row-gutter: 0pt, stroke: none, inset: 0pt,
    headers.intestazione(ctx, stile-risolto, drawers),
    ..righe-corpo,
    ..if piede != none { (piede,) } else { () },
  )
}
