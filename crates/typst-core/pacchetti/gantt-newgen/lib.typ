// gantt-newgen — entrypoint del pacchetto (RF1).
//
// Implementazione indipendente ispirata a "gantty" di John Toohey: nessun
// codice copiato (specs/decisioni.md, D-032). Licenza LGPL-3.0-or-later:
// vedi LICENSE, COPYING e D-031.
//
// Stato (Fase 8, "guardie residue e prestazioni", specs/plan.md; le
// Fasi 1-7 sono CHIUSE): `validate()` è COMPLETO su RF1-RF11 (anche
// RF10, `graph.valida-*`/E-DEP-*, e RF11, i drawer dell'utente via
// E-DRW-*/E-VAL-03). `gantt()`
// DISEGNA DAVVERO l'intero grafico: dopo `_validate-interno` e
// `timeline.calcola()` risolve lo stile (`src/style.typ`), calcola il
// contesto in `pt` (`src/layout.typ`: righe misurate, header,
// `min-cell-width`, anti-collisione/clamp delle etichette milestone,
// tratto di avanzamento per intervallo — DC-9, per-intervallo con
// pause vuote, P4 — e, da Fase 7, agganci/routing/spezzamento per
// riga delle frecce di dipendenza, RF10/D-037) e lo unisce ai drawer
// EFFETTIVI (`default-drawers + drawers:` dell'utente, RF11/D-035/
// D-040) prima di passare tutto a `src/render/chart.typ`, che produce
// una `table` con intestazione ripetuta (`table.header(repeat:
// true)`, RF12), le frecce disegnate per riga (il drawer `dependency`,
// validato in Fase 6b, è chiamato da Fase 7 in poi) e il piede delle
// etichette (`table.footer(repeat: ..)`, D-038 — con la regola del
// footer più alto di metà dell'altezza UTILE (non della pagina:
// `altezza-utile` è la regione intera, T46), `repeat: auto` di
// default, D-127, Fase 8.5).
// `layout()` (nativo di Typst) dà la larghezza disponibile; `context`
// (richiesto da `measure()`, usato solo dentro `layout.typ`) avvolge
// tutto il calcolo del contesto e il disegno, una volta sola (RNF3).
//
// Il lavoro di validazione resta in `src/validate.typ:
// _validate-interno(spec, ..opzioni) -> (spec, registro, locale)`:
// `validate()` pubblico ne espone solo `.spec` (D-029: niente funzioni
// in uscita — `header-units` vi è ridotto a `(name, rank)`); `gantt()`
// usa `_validate-interno` direttamente, quindi NESSUNA doppia
// costruzione del registry, nessuna doppia lettura degli argomenti
// grezzi di `header-units:`, nessuna doppia risoluzione del locale
// (tutto fatto una volta sola dentro `_validate-interno`). Le funzioni
// pure di timeline/units/layout/style si testano importando
// direttamente i rispettivi moduli (stessa convenzione di
// `unit/dates.typ`, `unit/graph.typ`: RNF5), non attraverso `gantt()`.
//
// `default-drawers` (RF11/drawers.typ) e `default-style` (RF5/style.typ)
// sono entrambe esportate (Fase 5 D-091 punto a; Fase 6b).
//
// Nessuna dipendenza (D-023): il core è in src/ (funzioni pure), il disegno
// nativo in src/render/ (chart.typ: table spezzabile su più pagine).
// Vedi "Struttura dei moduli" nella specifica.

#import "src/dates.typ"
// Alias (non "layout"): il nome "layout" è il posizionatore nativo di
// Typst, usato in `gantt()` sotto per conoscere la larghezza
// disponibile — un `#import "src/layout.typ"` senza alias lo
// ombreggerebbe con il MODULO, rendendo `layout(...)` irraggiungibile.
#import "src/layout.typ" as layout-mod
#import "src/locale.typ": locales
#import "src/render/chart.typ"
#import "src/render/drawers.typ"
#import "src/style.typ"
#import "src/timeline.typ"
#import "src/units.typ"
#import "src/validate.typ": validate

/// Registry delle unità di intestazione INCLUSE (RF1/RF4.3): `year`,
/// `quarter`, `month`, `week`, `day`, con `first-weekday` 1 (lunedì) —
/// valore di riferimento per introspezione/README (RF8); il registry
/// VERO usato da `gantt()` ricalcola `week` con il `first-weekday` del
/// locale risolto (RF3/D-028), non necessariamente 1.
#let header-units = units.incluse(1)

/// Costruisce la definizione di un'unità "sprint" da registrare con
/// `header-units:` (RF4.3). Vedi `src/units.typ` per i parametri.
#let sprint-unit = units.sprint-unit

/// Stile di default (RF1/RF5): fonte unica, usata anche per il README
/// (RF8, Fase 9). Vedi `src/style.typ`.
#let default-style = style.default-style

/// Drawer di default (RF1/RF11, Fase 6b): fonte unica, usata anche per
/// il README (RF8, Fase 9). Unito con `drawers:` dell'utente in
/// `gantt()` sotto (`default-drawers + drawers-utente`, D-040: l'ultimo
/// vince, `none` disattiva l'elemento). Vedi `src/render/drawers.typ`.
#let default-drawers = drawers.default-drawers

/// Disegna il diagramma di Gantt (RF1). Valida l'input con la stessa
/// validazione COMPLETA di `validate()` (stesso catalogo di errori di
/// RF7, registry delle unità di intestazione incluso), calcola la
/// timeline (D-064), risolve lo stile e il contesto in `pt`
/// (`src/style.typ`, `src/layout.typ`) e disegna una `table` spezzabile
/// su più pagine con intestazione ripetuta (`src/render/chart.typ`,
/// RF12, D-023/D-037/D-038/D-051).
///
/// - spec (dictionary): input secondo lo schema di RF2.
/// - ..opzioni (arguments): vedi `validate()`.
/// -> content
#let gantt(spec, ..opzioni) = {
  // Import LOCALE a questo blocco (suggerimento del coordinatore,
  // terza revisione Fase 4): un `#import` dentro il corpo di una
  // funzione resta scoped alla funzione, non diventa un binding al
  // livello del MODULO `lib.typ` — a differenza di un `#import` in
  // testa al file, che sarebbe RI-ESPORTATO (qualunque nome legato al
  // livello superiore di un file e' parte del suo modulo quando quel
  // file viene importato altrove, Typst non distingue "import" da
  // "export": verificato con una sonda dedicata). Cosi'
  // `_validate-interno` resta un dettaglio interno — non raggiungibile
  // con `#import "@preview/gantt-newgen:0.1.0": _validate-interno` da
  // un documento utente — pur restando chiamabile da `gantt()` stessa.
  import "src/validate.typ": _validate-interno
  // DC-15 (opzione A, terza revisione Fase 4): SOLO qui, non nel core,
  // "oggi" diventa automatico quando `today: none` + `show-today: true`
  // (RF2) — `datetime.today()` dipende dalla data di sistema, quindi
  // resta fuori da `validate()`/`timeline.typ` (RNF4, determinismo dei
  // test `unit/`, T20).
  let interno = _validate-interno(spec, risolvi-oggi-automatico: true, ..opzioni)
  let modello = timeline.calcola(interno.spec, interno.registro, interno.locale)
  let stile-risolto = style.risolvi(interno.spec.style, locale: interno.locale)
  // RF11/Fase 6b (D-035/D-040): drawer EFFETTIVI = default unito con
  // `drawers:` dell'utente (`interno.drawers-utente`, già VALIDATO —
  // E-DRW-01 — da `_validate-interno`, funzioni vere, fuori da
  // `.spec`). L'ultimo vince: `(bar: none)` disattiva l'elemento.
  let drawers-effettivi = default-drawers + interno.at("drawers-utente")

  // `layout()` (nativo di Typst) dà la larghezza del contenitore in
  // `pt` (sidebar + area barre, RF12: "larghezza... quella del
  // contenitore sulla prima pagina"); `context` è richiesto da
  // `measure()` (usato SOLO dentro `layout.calcola`, mai nel render:
  // "Struttura dei moduli"). Tutto il calcolo del contesto e il disegno
  // avvengono UNA VOLTA qui dentro (RNF3), non per riga.
  // `dimensioni.height` (RF12, Fase 8.5, D-127, T46): altezza della
  // REGIONE intera (non lo spazio rimasto), passata come `altezza-
  // utile:` per la regola del footer alto — stesso `layout()`, nessuna
  // misura in più.
  layout(dimensioni => context {
    let ctx = layout-mod.calcola(
      interno.spec, modello, stile-risolto, dimensioni.width, interno.locale,
      altezza-utile: dimensioni.height,
    )
    chart.disegna(ctx, stile-risolto, drawers-effettivi)
  })
}
