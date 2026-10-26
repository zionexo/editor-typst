// gantt-newgen — src/render/milestones.typ
// Riga delle etichette di milestone/oggi (`table.footer`, D-038): le
// posizioni (già impilate su livelli e clampate da `layout.calcola`,
// skill `etichette-grafico-geometria-pura`), lo stroke per etichetta
// (D4, `style.stile-milestone`, già normalizzato con `stroke(..)`), il
// colore del testo (`e.fill-testo`, precedenza utente già risolta,
// D-099+1) e la geometria del connettore (`e.connettore`, array di
// punti o `none`, già calcolata da `layout.connettore-punti`) sono
// content puro qui, nessun nuovo calcolo (RNF5: il render non ricalcola
// mai). `style.milestones.label.repeat` decide se la riga si ripete su
// ogni pagina (RF12) o compare solo dopo l'ultima riga.
//
// Rilievo del coordinatore (seconda revisione di Fase 5): a differenza
// della prima versione (etichette tutte su UNA riga, con `x` spostata
// dall'anti-collisione), ora `e.x` non si sposta mai oltre il
// pre-clamp (C1) — una collisione orizzontale si risolve con un
// LIVELLO diverso (`e.livello`, 0-based, assegnato da
// `layout.impila-e-clampa`), non spostando l'ascissa. Il footer è alto
// `livelli-usati() × altezza-livello-etichetta` (più l'`overhang`).
//
// Rilievo del coordinatore (rereview di Fase 5): DUE cicli separati in
// `_cella-area` — TUTTI i connettori PRIMA, TUTTE le etichette DOPO —
// cosi' un'etichetta disegnata sopra un connettore che la attraversa lo
// NASCONDE (il suo `box` ha uno sfondo OPACO, `style.milestones.
// label.fill`, bianco di default: prima lo sfondo era assente,
// `fill: none` implicito di `box()`, quindi un connettore che passava
// dietro un'etichetta restava visibile attraverso di lei).
// **Verificato con una sonda dedicata (scenari densi, es. `ok/
// milestones-collisione.typ`):** questo vale per QUALUNQUE etichetta
// che attraversa, non solo la PROPRIA — in uno scenario affollato
// un'etichetta di livello 0 larga (il livello più in alto, disegnato
// per ultimo fra le etichette con lei nello stesso loop, ma SOPRA ogni
// connettore) può coprire quasi del tutto il connettore di
// un'etichetta vicina su un livello più profondo, se le loro colonne
// si sovrappongono — conseguenza diretta e VOLUTA del requisito
// ("l'etichetta nasconde il tratto che la attraversa", non solo il
// proprio): il segno VERO resta comunque visibile nel corpo del
// grafico (`gruppi-segni`, posizione esatta, RNF6); il connettore nel
// footer è un aiuto "best effort", non l'unica fonte di verità.

/// Un'etichetta su due righe (S2c, come l'originale): nome in
/// grassetto, data sotto (solo se `e.testo` la porta — qui `e.testo` è
/// già il content completo costruito da `layout.calcola`, che decide
/// se includere la data secondo `show-date`; non c'è bisogno di
/// separare le due righe qui, lo spezzamento nome/data avviene nel
/// content stesso tramite `linebreak()`). Sfondo OPACO
/// (`style.milestones.label.fill`) per nascondere un connettore che la
/// attraversa (sopra); testo CENTRATO (E3, decisione estetica: nome e
/// data — larghezze diverse quando `show-date` è attivo — restano
/// allineati al centro sotto il segno, non "a scaletta" a sinistra).
/// **D-099+1 (rilievo del coordinatore, diagnosi RI-VERIFICATA con una
/// sonda dedicata):** `e.fill-testo` è già il colore FINALE (precedenza
/// utente risolta in `layout.calcola`) — un solo dizionario unito (`+`)
/// spreadato in `text(..)`. **Non** un "duplicate argument" (sonda
/// dedicata: `text(..d, fill: x)` con `d` che contiene già `fill` NON fa
/// panic in Typst 0.15 — l'argomento nominale scritto per ULTIMO, anche
/// se arriva dopo uno spread, sovrascrive silenziosamente quello dello
/// spread). Il difetto vero, pre-fix, era quindi un'INVERSIONE di
/// precedenza SILENZIOSA: `text(..label.text, fill: e.stroke.paint)`
/// aveva `fill: e.stroke.paint` scritto DOPO lo spread, quindi vinceva
/// SEMPRE — un `style.milestones.label.text.fill` dell'utente veniva
/// ignorato in silenzio, mai applicato, mai un errore visibile
/// (controprova: mutazione a `fill: e.stroke.paint` diretto su
/// `ok/milestone-label-fill-utente.typ`, verde atteso → rosso: l'
/// etichetta si colora di rosso, lo stroke della milestone, non più di
/// verde, il colore scelto dall'utente).
#import "drawers.typ" as drawers-mod

/// Dizionario `m` passato al drawer `milestone` (RF11): chiavi ENGLISH
/// (interfaccia utente). `date-label` è GIÀ localizzato/formattato (o
/// `none` se `show-date: false` o "oggi", RF11) — `name`/`date-label`
/// bastano al drawer di default per ricomporre lo STESSO content già
/// misurato da `layout.calcola` per l'anti-collisione (`e.testo`,
/// invariato: usato solo per la larghezza, mai più disegnato qui).
/// Suggerimento (review di Fase 6b): `width`/`level` aggiunti (la
/// larghezza riservata dall'anti-collisione e il livello d'impilamento
/// assegnato da `layout.impila-e-clampa`) — utili a un drawer custom
/// che vuole adattarsi allo spazio GIÀ riservato, interfaccia
/// versionata "solo aggiunte" (RF11).
#let _milestone-pubblica(e) = (
  name: e.nome, date: e.data, "date-label": e.at("data-label"), today: e.today,
  style: e.at("stile-etichetta"), x: e.x, width: e.larghezza, level: e.livello,
)

#let _etichetta(e, ctx, drawers) = drawers-mod.chiama(drawers, "milestone", ctx.locale, _milestone-pubblica(e))

/// Connettore: SOLO content, dai punti già calcolati da
/// `layout.connettore-punti` (`e.connettore`, `none` se non serve —
/// geometria/soglia spostate in `layout.typ`, rereview di Fase 5).
#let _connettore(e) = {
  if e.connettore == none { return none }
  let punti = e.connettore
  curve(
    stroke: e.stroke,
    curve.move((punti.at(0).x, punti.at(0).y)),
    ..punti.slice(1).map(p => curve.line((p.x, p.y))),
  )
}

/// Cella AREA della riga di etichette: DUE cicli (sopra) — connettori,
/// poi etichette, ciascuna posizionata al proprio LIVELLO (`e.livello`,
/// S1/S2, geometria gia' in `e.connettore`/`layout.calcola`).
#let _cella-area(ctx, stile-risolto, drawers) = box(
  width: ctx.larghezza-area, height: ctx.at("altezza-etichette"),
  {
    let overhang = stile-risolto.milestones.label.overhang
    let altezza-livello = ctx.at("altezza-livello-etichetta")
    for e in ctx.etichette {
      let connettore = _connettore(e)
      if connettore != none {
        place(top + left, dx: 0pt, dy: 0pt, connettore)
      }
    }
    for e in ctx.etichette {
      let contenuto = _etichetta(e, ctx, drawers)
      if contenuto != none {
        place(
          top + left, dx: e.x - e.larghezza / 2, dy: overhang + e.livello * altezza-livello,
          contenuto,
        )
      }
    }
  },
)

/// `table.footer(repeat:)` con la riga di etichette, oppure `none` se
/// non c'è nessuna etichetta da mostrare (nessuna milestone nel
/// viewport e "oggi" assente/fuori range: niente footer vuoto).
/// **RF12, Fase 8.5 (D-127):** `repeat:` legge `ctx.ripeti-etichette`
/// (bool GIÀ risolto da `layout.calcola`, regola del footer alto
/// applicata solo con `style.milestones.label.repeat: auto`), MAI il
/// valore di stile grezzo (che può essere `auto`, non ammesso da
/// `table.footer(repeat:)`, un `bool` nativo di Typst).
#let etichette(ctx, stile-risolto, drawers) = {
  if ctx.etichette.len() == 0 { return none }
  table.footer(
    repeat: ctx.ripeti-etichette,
    table.cell(breakable: false)[],
    table.cell(breakable: false, _cella-area(ctx, stile-risolto, drawers)),
  )
}
