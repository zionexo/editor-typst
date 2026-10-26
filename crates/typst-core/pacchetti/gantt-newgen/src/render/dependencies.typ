// gantt-newgen — src/render/dependencies.typ
// Frecce di dipendenza (RF10/RF11, Fase 7): disegno PURO, zero calcolo
// (RNF5) — tutta la geometria (agganci, routing, spezzamento per riga,
// traslazione relativa, punta) è già pronta in `riga.frecce`/
// `ctx.dipendenze` (`src/layout.typ`). Due percorsi, come `label`/`bar`
// (D-115):
// - DEFAULT (nessuna ridefinizione dell'utente, confronto per IDENTITÀ
//   di funzione con `drawers-mod.dependency`): `disegna-riga-default`
//   disegna direttamente `riga.frecce` — tratti spezzati per riga +
//   punta, NESSUN `box(clip: true)` (D-037, costo misurato: 0,21s PNG
//   contro 1,44s col clip per riga);
// - RIDEFINITO (`drawers: (dependency: ..)`): il content di OGNI
//   dipendenza si costruisce UNA VOLTA (`render/chart.typ`, come
//   `overlay`, RF11), poi si dispone qui per ogni riga ATTRAVERSATA
//   dentro `box(clip: true)` — tecnica identica a `overlay`
//   (`render/bars.typ: riga-corpo`), stesso costo documentato (RF11).
// `drawers: (dependency: none)` disattiva TUTTE le frecce, ma passa dal
// percorso CUSTOM (`none` non è mai `== drawers-mod.dependency`, D-115):
// **commento corretto dopo S6** (D-121, la stesura precedente era
// SBAGLIATA) — `contenuti-dipendenze` chiama comunque `chiama()` per
// OGNI dipendenza (che restituisce `none` subito, `fn == none` sopra,
// D-040), quindi l'elenco passato a `disegna-riga-custom` ha **UNA VOCE
// PER DIPENDENZA con `content: none`** (NON un elenco vuoto): la riga
// resta senza frecce perché `disegna-riga-custom` SALTA (`continue`) le
// voci `content: none`, non perché l'elenco in ingresso sia vuoto —
// verificato in `unit/render-dependencies.typ` (`casi-s6-estrazione`).
// Risultato finale identico (nessun content aggiunto al `box()` della
// riga) al caso "nessuna dipendenza nello spec", dove l'elenco è invece
// VERAMENTE vuoto perché `ctx.dipendenze` lo è (`.map()` su un array
// vuoto resta vuoto).
// Primitive CONDIVISE con `render/drawers.typ` (DS-3, fonte unica):
// `dependency-tratto`/`dependency-punta` costruiscono `curve()`/
// `polygon()` dagli stessi punti già pronti, sia qui (un SOTTO-insieme
// di punti già spezzato per riga) sia nel drawer di default
// (`drawers-mod.dependency`, la polilinea INTERA) — stesso codice, non
// una copia mantenuta a mano.
#import "drawers.typ" as drawers-mod

/// Content del drawer `dependency` RIDEFINITO, UNA VOLTA per dipendenza
/// (non per riga, stessa tecnica dell'`overlay` globale, RF11) — array
/// vuoto quando il drawer è ancora quello di default (percorso rapido
/// per identità, D-115: `disegna-riga-default` disegna direttamente
/// `riga.frecce`, questa funzione non viene nemmeno chiamata) o quando
/// è `none` (D-040: nessuna freccia — `none` non è mai `==`
/// `drawers-mod.dependency`, una funzione, quindi senza questo
/// controllo esplicito finirebbe nel ramo "chiamalo", sbagliato).
/// `disponi()` (E-DRW-02) è già applicata da `chiama()`. **S6
/// (revisione del 2026-10-18): estratta da `render/chart.typ`** perché
/// sia la pipeline di produzione sia `unit/render-dependencies.typ`
/// (il confronto strutturale `dependency: none` ≡ "nessuna dipendenza")
/// chiamino la STESSA funzione — prima il test ricostruiva la logica a
/// mano, senza esercitare il ramo REALE di `chart.typ`.
/// -> array di `(content: content|none, "righe-attraversate": array)`.
#let contenuti-dipendenze(ctx, drawers) = if drawers.at("dependency") == drawers-mod.dependency {
  ()
} else {
  ctx.dipendenze.map(dep => (
    content: drawers-mod.chiama(drawers, "dependency", ctx.locale, dep),
    "righe-attraversate": dep.at("righe-attraversate"),
  ))
}

/// Percorso DEFAULT: disegna i tratti e le punte già pronti in
/// `riga.frecce` (array di `(stroke, tratti, punte)`, pt RELATIVI alla
/// riga — RNF3: un elemento per STROKE distinto nella riga, di norma
/// 1-2 (normale/`violated`), non uno per dipendenza: più dipendenze con
/// lo STESSO stroke che attraversano la stessa riga condividono un solo
/// `curve()`, stessa tecnica di `gruppi-griglia`/`gruppi-segni`, D-097;
/// misurato su S5/RNF3-dip: 100 dipendenze a lungo raggio). `none` se
/// non c'è nulla da disegnare (nessuna dipendenza attraversa questa
/// riga) — stesso valore esatto restituito da `disegna-riga-custom`
/// quando l'elenco è vuoto (RF11 "dependency: none" ≡ "nessuna
/// dipendenza", verificato in `unit/render-dependencies.typ`).
/// **T44/GOTCHA (diagnosi CORRETTA dalla review del 2026-10-18, S3 —
/// la prima diagnosi, D-118, era SBAGLIATA: non è specifico di
/// `table`): `place(.., A + B)` (UN SOLO `place()` su un content GIA'
/// sommato con `+`, dove A e B sono `curve()`/`polygon()`, content
/// block-level) impagina A e B in FLUSSO VERTICALE NORMALE (uno sotto
/// l'altro, come due paragrafi), ESATTAMENTE come se non fossero dentro
/// un `place()` — `place()` sposta solo l'ORIGINE del blocco intero,
/// non impedisce al suo CONTENUTO di impilarsi. Succede SIA dentro
/// `table` SIA fuori (verificato con una sonda dedicata, `box()` su
/// pagina senza nessuna `table`: stesso scostamento). La prima sonda
/// (D-118) sembrava "funzionare" fuori tabella solo perché usava un
/// tratto ORIZZONTALE (altezza quasi nulla): lo scostamento da
/// impilamento era trascurabile a quella scala, non assente — una
/// seconda sonda con un tratto VERTICALE alto ha mostrato lo stesso
/// scostamento anche fuori tabella. Dettagli e cifre: decisioni.md,
/// voce D-121.** La correzione resta quella già in uso in `sfondo-riga`
/// (`render/bars.typ`): un `place()` INDIPENDENTE per OGNI forma (mai
/// `curve(..) + polygon(..)` sommati PRIMA di un `place()` solo) — qui
/// un `place()` per il tratto (gruppo) e uno per OGNI punta.
#let disegna-riga-default(riga) = {
  if riga.frecce.len() == 0 { return none }
  let pezzi = ()
  for gruppo in riga.frecce {
    pezzi.push(place(top + left, dx: 0pt, dy: 0pt, drawers-mod.dependency-tratto(gruppo.tratti, gruppo.stroke)))
    for p in gruppo.punte {
      pezzi.push(place(top + left, dx: 0pt, dy: 0pt, drawers-mod.dependency-punta(p.punti, p.fill)))
    }
  }
  pezzi.join()
}

/// Percorso RIDEFINITO (RF11, D-037): `dipendenze-contenuto` è l'elenco
/// (uno per dipendenza, costruito UNA VOLTA da `render/chart.typ`, mai
/// qui) di `(content, righe-attraversate)` — `content` è già il
/// risultato di `drawers.chiama(.., "dependency", .., dep)`, guardato
/// (E-DRW-02); `none` se il drawer l'ha disattivata per QUESTA
/// dipendenza (D-040). Per ogni dipendenza che attraversa `riga.path`,
/// il content si dispone con `place`, ritagliato alla sola altezza
/// della riga e traslato di `-riga.y0` (`dy:`) — stessa tecnica
/// dell'overlay globale (`render/bars.typ: riga-corpo`): le coordinate
/// del content sono quelle ASSOLUTE dell'area barre (RF11, "`points`
/// relativi all'origine dell'area barre"), non quelle della riga.
#let disegna-riga-custom(riga, ctx, dipendenze-contenuto) = {
  let pezzi = ()
  for d in dipendenze-contenuto {
    if d.content == none { continue }
    if riga.path not in d.at("righe-attraversate") { continue }
    // Ogni `box(clip:)` nel proprio `place()` (stesso GOTCHA di sopra:
    // più elementi "sommati" nella stessa cella vanno SEMPRE placed
    // indipendentemente, mai concatenati prima): con più di una
    // dipendenza che attraversa questa riga, ogni box deve restare
    // sovrapposto all'origine, non impilato in sequenza.
    pezzi.push(place(top + left, dx: 0pt, dy: 0pt, box(
      width: ctx.larghezza-area, height: riga.altezza, clip: true,
      place(top + left, dx: 0pt, dy: -riga.y0, d.content),
    )))
  }
  if pezzi.len() == 0 { none } else { pezzi.join() }
}
