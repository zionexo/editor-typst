// gantt-newgen — src/units.typ
// Registry delle unita' di intestazione (RF4.3, D-006/D-017): unita'
// incluse (year/quarter/month/week/day), `sprint-unit(..)` e merge/
// validazione delle unita' custom passate con `header-units:`. Core
// puro: nessun disegno, nessun `measure` (RNF5) — il calcolo dei
// segmenti e' O(segmenti) per costruzione (D-007), con il tetto di
// sicurezza di E-HDR-06 contato con un `for … in range(..)` (mai un
// `while`, T26: il tetto di Typst coinciderebbe col nostro, D-076 #9).
//
// Implementazione indipendente (D-032): gantty 0.1.0/0.5.1 non hanno un
// registry di unita' (header giorno-per-giorno, D10) — letti solo come
// riferimento funzionale, mai trascritti.

#import "dates.typ"
#import "errors.typ": fail, interpola, nome-tipo
#import "schema.typ": suggerisci

/// Rank delle 5 unita' incluse (RF4.3): costanti, indipendenti da
/// `first-weekday` — usate da `validate.typ` (tramite `registro()`) per
/// il controllo di unicita' del rank nel registry effettivo senza dover
/// costruire le chiusure `floor`/`next` (che serve solo a `timeline.typ`
/// per il calcolo vero dei segmenti).
#let rank-inclusi = (year: 40, quarter: 30, month: 20, week: 10, day: 0)

/// Nomi riservati (unita' incluse): non possono essere ridefiniti da
/// `header-units:` (RF4.3, E-HDR-05).
#let nomi-inclusi = rank-inclusi.keys()

/// Numero di trimestre (1-4) del mese di `d`.
#let _numero-quarto(d) = calc.quo(d.month() - 1, 3) + 1

/// Giorno (datetime) con l'offset dei mesi applicato con riporto
/// d'anno (D1/RF3: dicembre + 1 mese -> gennaio dell'anno dopo). `d` e'
/// gia' il primo del mese (uso interno, `floor`/`next` di month/quarter).
#let _primo-mese-piu(d, n) = {
  let totale = (d.month() - 1) + n
  let anno = d.year() + calc.div-euclid(totale, 12)
  let mese = calc.rem-euclid(totale, 12) + 1
  datetime(year: anno, month: mese, day: 1)
}

// --- Unita' incluse (RF4.3) -------------------------------------------

#let _def-year = (
  rank: rank-inclusi.year,
  floor: d => datetime(year: d.year(), month: 1, day: 1),
  next: d => datetime(year: d.year() + 1, month: 1, day: 1),
  gridline: none,
  align: center + horizon,
  text: (:),
  "min-cell-width": 0pt,
  // Parole chiave di `format` (RF4): "number" (default) = "2025";
  // "short" = "'25".
  formati: (
    "number": seg => str(seg.start.year()),
    "short": seg => "'" + dates.formatta(seg.start).slice(2, 4),
  ),
  default-formato: "number",
)

#let _def-quarter = (
  rank: rank-inclusi.quarter,
  floor: d => datetime(year: d.year(), month: 3 * calc.quo(d.month() - 1, 3) + 1, day: 1),
  next: d => _primo-mese-piu(
    datetime(year: d.year(), month: 3 * calc.quo(d.month() - 1, 3) + 1, day: 1), 3,
  ),
  gridline: none,
  align: center + horizon,
  text: (:),
  "min-cell-width": 0pt,
  // "short" (default) = "{quarter-prefix}{n}" (es. "Q1"/"T1"); "year" =
  // "short" + " " + anno; "number" = solo "{n}" (RF4).
  formati: (
    "short": seg => seg.locale.at("quarter-prefix") + str(_numero-quarto(seg.start)),
    "year": seg => seg.locale.at("quarter-prefix") + str(_numero-quarto(seg.start)) + " " + str(seg.start.year()),
    "number": seg => str(_numero-quarto(seg.start)),
  ),
  default-formato: "short",
)

#let _def-month = (
  rank: rank-inclusi.month,
  floor: d => datetime(year: d.year(), month: d.month(), day: 1),
  next: d => _primo-mese-piu(datetime(year: d.year(), month: d.month(), day: 1), 1),
  gridline: none,
  align: center + horizon,
  text: (:),
  "min-cell-width": 0pt,
  // "short" (default, nome breve localizzato); "long" (nome completo);
  // "number" ("3").
  formati: (
    "short": seg => seg.locale.at("months-short").at(seg.start.month() - 1),
    "long": seg => seg.locale.at("months").at(seg.start.month() - 1),
    "number": seg => str(seg.start.month()),
  ),
  default-formato: "short",
)

/// Inizio della settimana (secondo `first-weekday`, D-028) che contiene
/// `d`: `d - rem-euclid(weekday(d) - first-weekday, 7)` giorni.
#let _week-floor(d, first-weekday) = (
  dates.aggiungi-giorni(d, -calc.rem-euclid(d.weekday() - first-weekday, 7))
)

/// Giovedì del segmento di 7 giorni che INIZIA in `inizio-settimana`
/// (D-028: "il giovedì del segmento completo", definito con QUALUNQUE
/// `first-weekday" perche' 7 giorni consecutivi contengono esattamente
/// un giovedì — `weekday() == 4`). Ricalcolato dal `floor` del VERO
/// inizio di settimana (non da `seg.start`, che puo' essere tagliato dal
/// viewport su un segmento parziale): per questo il chiamante passa
/// sempre `_week-floor(seg.start, first-weekday)`, mai `seg.start` da solo.
#let _giovedi-settimana(inizio-settimana) = {
  let offset = range(0, 7).filter(i => dates.aggiungi-giorni(inizio-settimana, i).weekday() == 4).at(0)
  dates.aggiungi-giorni(inizio-settimana, offset)
}

/// Anno e numero ISO della settimana che contiene `giovedi` (D-028):
/// l'anno ISO e' l'anno del giovedì stesso (mai quello del lunedì/inizio
/// settimana, che potrebbe cadere nell'anno precedente); il numero e'
/// `quo(ordinal(giovedì) - 1, 7) + 1` (nessun `display`, RF4).
#let _iso(giovedi) = (anno: giovedi.year(), numero: calc.quo(giovedi.ordinal() - 1, 7) + 1)

#let _def-week(first-weekday) = (
  rank: rank-inclusi.week,
  floor: d => _week-floor(d, first-weekday),
  next: d => dates.aggiungi-giorni(_week-floor(d, first-weekday), 7),
  gridline: none,
  align: left + horizon,
  text: (:),
  "min-cell-width": 0pt,
  // "start-day" (default, compatibile con gantty: giorno del mese del
  // primo giorno della settimana); "iso" (numero+anno ISO dal giovedì
  // del segmento COMPLETO, D-028); "range" ("3–9"). **R7 (D-102,
  // decisione di specifica da registrare in RF4):** le tre varianti
  // usano SEMPRE il vero inizio di settimana (`_week-floor(seg.start,
  // first-weekday)`), MAI `seg.start` direttamente — `seg.start` e'
  // il primo giorno VISIBILE del segmento, che su una settimana
  // tagliata dal viewport (a sinistra) e' dopo il vero lunedì (o
  // `first-weekday`): un'etichetta "27" al bordo del viewport, quando
  // la settimana vera inizia il 24, confonderebbe la settimana con
  // quella successiva. Stessa regola già applicata a "iso" (il
  // giovedì si cerca a partire dal vero inizio, non da `seg.start`);
  // "range" aveva la STESSA incoerenza di "start-day" per l'estremo
  // sinistro (usava `seg.start.day()` per l'inizio ma il vero `floor`
  // per la fine, D-102): corretta allo stesso modo, per coerenza.
  formati: (
    "start-day": seg => str(_week-floor(seg.start, first-weekday).day()),
    "iso": seg => {
      let giovedi = _giovedi-settimana(_week-floor(seg.start, first-weekday))
      let info = _iso(giovedi)
      seg.locale.at("week-prefix") + str(info.numero)
    },
    "range": seg => {
      let vero-inizio = _week-floor(seg.start, first-weekday)
      let vera-fine = dates.aggiungi-giorni(vero-inizio, 6)
      str(vero-inizio.day()) + "–" + str(vera-fine.day())
    },
  ),
  default-formato: "start-day",
)

#let _def-day = (
  rank: rank-inclusi.day,
  floor: d => d,
  next: d => dates.aggiungi-giorni(d, 1),
  gridline: none,
  align: center,
  text: (:),
  "min-cell-width": 2pt,
  // "number" (default, "7"); "weekday-short" (nome breve del giorno
  // della settimana); "weekday-letter" (iniziale).
  formati: (
    "number": seg => str(seg.start.day()),
    "weekday-short": seg => seg.locale.at("weekdays-short").at(seg.start.weekday() - 1),
    "weekday-letter": seg => seg.locale.at("weekdays-letter").at(seg.start.weekday() - 1),
  ),
  default-formato: "number",
)

/// Registry delle unita' incluse, con le chiusure vere (serve
/// `first-weekday` per `week`): usato sia da `registro()` (per
/// costruire il registry effettivo) sia direttamente dai test/README
/// (RF1: `header-units` esportato da `lib.typ`).
#let incluse(first-weekday) = (
  year: _def-year, quarter: _def-quarter, month: _def-month,
  week: _def-week(first-weekday), day: _def-day,
)

// --- sprint-unit(..) (RF4.3) -------------------------------------------

/// Numero di sprint (k + 1, D-017) del segmento che inizia in `d`
/// (gia' sul confine di un periodo, calcolato da chi chiama): `k =
/// div-euclid(giorni(d - anchor), length)`, anche negativo (date prima
/// di `anchor` ammesse).
#let _numero-sprint(d, anchor, length) = calc.div-euclid(dates.giorni-fra(anchor, d), length) + 1

/// Costruisce la definizione di un'unita' "sprint" a lunghezza fissa
/// (RF4.3): confini `anchor + k * length` giorni. Parametri non validi
/// -> E-HDR-05 con percorso `sprint-unit.<parametro>` (voce composita
/// `E-HDR-05.sprint-param`, D-061). Errori di `sprint-unit()` sono
/// SEMPRE in inglese (come E-API-*/E-LOC-*, RF7): la chiamata avviene
/// PRIMA della risoluzione di `locale` nella pipeline di `gantt()`/
/// `validate()`, spesso fuori da quella pipeline del tutto (l'utente la
/// invoca per costruire `header-units:`).
#let sprint-unit(anchor, length: 14, label: auto, rank: 15) = {
  // Import locale per gli errori "sempre en" (D-030: guardia prima del
  // costruttore) — import locale alla funzione per evitare un ciclo
  // (locale.typ non dipende da units.typ, quindi nessun ciclo reale, ma
  // l'import resta qui per tenere la dipendenza visibile solo dove serve).
  import "locale.typ": locales
  let loc = locales.en

  let ancora = dates.valida-data(anchor, "sprint-unit.anchor", loc)

  if type(length) != int or length < 1 {
    fail(
      "E-HDR-05", "sprint-unit.length", loc,
      argomenti: (detail: loc.errors.at("E-HDR-05.sprint-param")),
      received: repr(length),
    )
  }
  if type(rank) != int {
    fail(
      "E-HDR-05", "sprint-unit.rank", loc,
      argomenti: (detail: loc.errors.at("E-HDR-05.sprint-param")),
      received: nome-tipo(rank),
    )
  }
  if label != auto and type(label) != str and type(label) != function {
    fail(
      "E-HDR-05", "sprint-unit.label", loc,
      argomenti: (detail: loc.errors.at("E-HDR-05.sprint-param")),
      received: nome-tipo(label),
    )
  }

  let etichetta(seg) = {
    let n = _numero-sprint(seg.start, ancora, length)
    if label == auto {
      "S" + str(n)
    } else if type(label) == function {
      label(seg, n)
    } else {
      label.replace("{n}", str(n))
    }
  }

  (
    rank: rank,
    floor: d => dates.aggiungi-giorni(ancora, _numero-sprint(d, ancora, length) * length - length),
    next: d => dates.aggiungi-giorni(ancora, _numero-sprint(d, ancora, length) * length),
    gridline: none,
    align: center + horizon,
    text: (:),
    "min-cell-width": 0pt,
    formati: (:),
    "default-formato": none,
    // Funzione di formattazione gia' risolta (nessuna parola chiave per
    // le unita' custom, RF4): usata direttamente da `_risolvi-formato`.
    format: etichetta,
  )
}

// --- Validazione e merge delle unita' custom (RF4.3, E-HDR-05) --------

/// Chiavi obbligatorie di una definizione custom e il tipo atteso
/// (`function` per le tre chiusure, `int` per `rank`).
#let _chiavi-obbligatorie = ("rank": int, "floor": function, "next": function, "format": function)

/// Chiavi ammesse di una definizione custom (RF4.3): le 4 obbligatorie
/// piu' le 4 opzionali. Usata SOLO per il controllo di chiavi
/// sconosciute con `strict` (S5, terza revisione Fase 4): un refuso come
/// "gridlne:" (invece di "gridline:") altrimenti passerebbe indenne,
/// ignorato in silenzio — lo stesso difetto D7 che RF7 vuole evitare,
/// qui dentro le definizioni d'unita'.
// "formati"/"default-formato" sono campi INTERNI che `sprint-unit()`
// mette gia' nel proprio risultato (RF4.3): quando l'utente registra
// `header-units: (sprint: sprint-unit(..))`, quel dizionario passa di
// nuovo da qui (stessa funzione per tutte le definizioni, scritte a
// mano o costruite) — vanno ammessi, altrimenti il controllo chiavi
// romperebbe `sprint-unit` stessa (bug trovato con una sonda diretta:
// `ok/headers-sprint.typ` falliva con "unknown key 'formati'").
#let chiavi-definizione = (
  _chiavi-obbligatorie.keys() + ("gridline", "align", "text", "min-cell-width", "formati", "default-formato")
)

/// Valida UNA definizione custom (dizionario) e la normalizza con i
/// default mancanti (`gridline`, `align`, `text`, `min-cell-width`: come
/// `month`, RF4.3). `rank-usati` e' il dizionario `rank -> nome` delle
/// unita' GIA' registrate (incluse + custom precedenti), per il
/// controllo di duplicato. Non esegue `floor`/`next` (nessun controllo a
/// runtime qui: quello e' compito di `segmenti()`, E-HDR-06). `strict`
/// (S5): con `strict: true` (default, D-018) una chiave sconosciuta
/// nella definizione fa fallire con E-HDR-05.
/// Kebab-case (RF4.3: "Nomi: chiave di header-units... kebab-case"):
/// lettere minuscole e cifre, trattini singoli come separatore, inizia
/// per lettera, nessun trattino iniziale/finale/doppio.
#let _re-kebab-case = regex("^[a-z][a-z0-9]*(-[a-z0-9]+)*$")

#let _valida-definizione(nome, def, rank-usati, locale, strict) = {
  if nome in nomi-inclusi {
    fail(
      "E-HDR-05", "header-units." + nome, locale,
      argomenti: (detail: locale.errors.at("E-HDR-05.reserved-name")),
    )
  }
  // TODO 4 (D-091 punto 3d, terza revisione Fase 4): il nome registrato
  // deve essere kebab-case (RF4.3) — prima non controllato, un nome come
  // "Semester"/"semester_2" passava indenne (nessun danno diretto, ma
  // incoerente con lo STILE dello schema, RF2 "chiavi in kebab-case").
  if nome.match(_re-kebab-case) == none {
    fail(
      "E-HDR-05", "header-units." + nome, locale,
      argomenti: (detail: locale.errors.at("E-HDR-05.kebab-case")),
    )
  }
  if type(def) != dictionary {
    fail("E-VAL-03", "header-units." + nome, locale, argomenti: (expected: "dictionary"), received: nome-tipo(def))
  }
  // S5: chiavi sconosciute, solo con `strict` (D-018, come il resto
  // dello schema) — un refuso tipo "gridlne:" altrimenti non darebbe
  // mai errore (ne' qui ne' altrove: la definizione e' un dizionario
  // libero, non passa da `_verifica-chiavi` di `validate.typ`).
  if strict {
    for chiave in def.keys() {
      if chiave not in chiavi-definizione {
        fail(
          "E-HDR-05", "header-units." + nome + "." + chiave, locale,
          argomenti: (detail: interpola(locale.errors.at("E-HDR-05.unknown-key"), (key: chiave))),
          suggest: suggerisci(chiave, chiavi-definizione),
        )
      }
    }
  }
  for (chiave, tipo-atteso) in _chiavi-obbligatorie {
    if chiave not in def or type(def.at(chiave)) != tipo-atteso {
      fail(
        "E-HDR-05", "header-units." + nome + "." + chiave, locale,
        argomenti: (detail: locale.errors.at("E-HDR-05.missing-key")),
        received: if chiave in def { nome-tipo(def.at(chiave)) } else { locale.errors.at("E-LOC-02.missing") },
      )
    }
  }
  let rank = def.rank
  // NOTA (T27, chiavi di dizionario SOLO stringhe): "rank-usati" ha
  // chiavi str(rank), non l'intero nudo — `d.insert(40, ..)` darebbe
  // "expected string, found integer" (bug trovato con una sonda diretta,
  // stesso difetto generale di T27, qui su un dizionario costruito da
  // noi, non su un valore dello schema).
  if str(rank) in rank-usati {
    let dettaglio = interpola(locale.errors.at("E-HDR-05.duplicate-rank"), (other: rank-usati.at(str(rank))))
    fail(
      "E-HDR-05", "header-units." + nome + ".rank", locale,
      argomenti: (detail: dettaglio),
      received: str(rank),
    )
  }
  (
    rank: rank,
    floor: def.floor,
    next: def.next,
    gridline: def.at("gridline", default: none),
    align: def.at("align", default: center + horizon),
    text: def.at("text", default: (:)),
    "min-cell-width": def.at("min-cell-width", default: 0pt),
    formati: (:),
    "default-formato": none,
    format: def.format,
  )
}

/// Registry effettivo (RF4.3): unita' incluse (secondo `first-weekday`)
/// piu' `header-units-utente` (dizionario `nome -> definizione`, gia'
/// risolto da `gantt()`/`validate()` secondo la precedenza di RF1),
/// validate e unite una volta. Restituisce `(registro: dizionario con le
/// chiusure vere, "nomi-rank": array di (name, rank), ordinato per rank
/// decrescente — dal piu' grosso al piu' fine, coerente con RF4 — a
/// parita' di rank per nome)`. Usato sia da `validate.typ` (solo per
/// `"nomi-rank"`, D-029: l'output pubblico non porta funzioni) sia da
/// `gantt()`/`timeline.typ` (per il registry vero, con le chiusure).
/// `strict` (S5, default `true`): chiave sconosciuta in una definizione
/// custom -> E-HDR-05.
#let registro(header-units-utente, first-weekday, locale, strict: true) = {
  let base = incluse(first-weekday)
  let rank-usati = (:)
  for (nome, def) in base { rank-usati.insert(str(def.rank), nome) }

  let effettivo = base
  for (nome, def) in header-units-utente {
    let validata = _valida-definizione(nome, def, rank-usati, locale, strict)
    rank-usati.insert(str(validata.rank), nome)
    effettivo.insert(nome, validata)
  }

  let coppie = ()
  for (nome, def) in effettivo { coppie.push((name: nome, rank: def.rank)) }
  let ordinate = coppie.sorted(key: c => (-c.rank, c.name))
  (registro: effettivo, "nomi-rank": ordinate)
}

// --- Risoluzione di `format` (parola chiave | funzione | pattern) ------

/// Risolve il campo `format` di un livello di `headers` (RF4): funzione
/// -> usata direttamente; stringa con `[` -> pattern di
/// `datetime.display` (non localizzato, RF4); stringa parola chiave ->
/// cercata nella tavola `formati` della definizione (solo unita'
/// incluse: le custom non ne hanno, RF4.3) -> altrimenti E-HDR-04;
/// `none`/assente -> `def.format` se presente (sprint-unit/custom) o la
/// parola chiave di default della definizione.
///
/// - nome-unita (string): nome registrato (per il messaggio E-HDR-04).
#let risolvi-formato(nome-unita, def, valore, percorso, locale) = {
  if valore == none {
    if "format" in def and def.format != none { return def.format }
    return def.formati.at(def.default-formato)
  }
  if type(valore) == function { return valore }
  if type(valore) != str {
    fail("E-VAL-03", percorso, locale, argomenti: (expected: "string|function"), received: nome-tipo(valore))
  }
  if valore.contains("[") {
    return seg => seg.start.display(valore)
  }
  if valore in def.formati {
    return def.formati.at(valore)
  }
  fail(
    "E-HDR-04", percorso, locale,
    argomenti: (unit: nome-unita),
    received: repr(valore), allowed: def.formati.keys().sorted(),
  )
}

// --- Segmenti (RF4, D-007): per QUALUNQUE unita' del registry ----------

/// Tetto di sicurezza (E-HDR-06, RF4.3): oltre questo numero di segmenti
/// per livello, un `floor`/`next` utente che non avanza (o avanza troppo
/// poco) farebbe un ciclo pressoche' infinito — qui e' un errore del
/// catalogo, non il "loop seems to be infinite" nudo di Typst (T26: un
/// `while` per questo conteggio coinciderebbe col tetto DI TYPST stesso,
/// 10 000 iterazioni, D-076 #9), perche' il conteggio usa un
/// `for … in range(..)`, che non ha quel limite (T26: 50 000 ok).
#let max-segmenti = 10000

/// Segmenti `[inizio, fine)` dell'unita' `def` (nome `nome-unita`, per i
/// messaggi) sul viewport `[vs, ve)`, gia' in istanti effettivi (D-050):
/// salta da un confine al successivo con `floor`/`next` (D-007, O(segmenti)
/// — mai giorno per giorno, D10), taglia il primo e l'ultimo segmento al
/// viewport (`partial: true`, D8: l'ultimo termina esattamente a `ve`).
/// Guardie a runtime (E-HDR-06, RF4.3): `floor(vs) <= vs`; `next(cur) >
/// cur` a ogni passo; oltre `max-segmenti` segmenti, errore invece di un
/// ciclo pressoche' infinito. Restituisce un array di
/// `(start: datetime, end: datetime, days: int, index: int, partial: bool)`.
#let segmenti(nome-unita, def, vs, ve, locale) = {
  // NOTA (nuova insidia di Typst, da riportare): `def.floor(vs)` con
  // `def` dizionario fallisce con "cannot directly call dictionary keys
  // as functions" — la sintassi punto-chiamata su un valore funzione
  // DENTRO un dizionario non e' permessa, qualunque sia il nome della
  // chiave (verificato con una sonda dedicata: anche `d.next(1)` con
  // `next` chiave qualsiasi fallisce identico). Va presa con `.at(..)`
  // e chiamata separatamente: `def.at("floor")(vs)`.
  let floor = def.at("floor")
  let next = def.at("next")
  let iniziale = floor(vs)
  // D3 (quarta revisione, rilievo del coordinatore): il risultato di
  // `floor` potrebbe non essere un `datetime` (funzione utente errata,
  // es. `floor: d => 42`): senza questo controllo, `dates.formatta`
  // sotto o il confronto `iniziale > vs` farebbero un panic "nudo" di
  // Typst (contro D-030), non un errore del catalogo. `repr()` mostra
  // il valore anche se non e' una data.
  if type(iniziale) != datetime {
    fail(
      "E-HDR-06", "header-units." + nome-unita + ".floor", locale,
      modello: "E-HDR-06.floor-invalid",
      argomenti: (
        unit: nome-unita,
        call: "floor(" + dates.formatta(vs) + ") = " + repr(iniziale),
      ),
    )
  }
  if iniziale > vs {
    fail(
      "E-HDR-06", "header-units." + nome-unita + ".floor", locale,
      modello: "E-HDR-06.floor-invalid",
      argomenti: (
        unit: nome-unita,
        call: "floor(" + dates.formatta(vs) + ") = " + dates.formatta(iniziale),
      ),
    )
  }
  let risultato = ()
  let cur = iniziale
  let indice = 0
  let finito = false
  for _ in range(0, max-segmenti) {
    if cur >= ve { finito = true; break }
    let nxt = next(cur)
    // Stesso controllo di tipo per `next` (D3): modello di BASE ("non
    // avanza"), non `.floor-invalid` (quel testo cita "floor"
    // esplicitamente) — imprecisione accettata (due varianti bastano,
    // non si introduce un terzo modello solo per questo caso).
    if type(nxt) != datetime {
      fail(
        "E-HDR-06", "header-units." + nome-unita + ".next", locale,
        argomenti: (unit: nome-unita, call: "next(" + dates.formatta(cur) + ") = " + repr(nxt)),
      )
    }
    if nxt <= cur {
      fail(
        "E-HDR-06", "header-units." + nome-unita + ".next", locale,
        argomenti: (
          unit: nome-unita,
          call: "next(" + dates.formatta(cur) + ") = " + dates.formatta(nxt),
        ),
      )
    }
    let inizio-seg = if cur > vs { cur } else { vs }
    let fine-seg = if nxt < ve { nxt } else { ve }
    risultato.push((
      unit: nome-unita, start: inizio-seg, end: fine-seg,
      days: dates.giorni-fra(inizio-seg, fine-seg),
      index: indice,
      partial: inizio-seg > cur or fine-seg < nxt,
      locale: locale,
    ))
    indice += 1
    cur = nxt
  }
  // D3 (off-by-one, rilievo del coordinatore): il `for` sopra gira
  // ESATTAMENTE `max-segmenti` volte (`range(0, max-segmenti)`); se
  // servono ESATTAMENTE `max-segmenti` segmenti, l'ultima iterazione li
  // produce tutti ma non resta nessuna iterazione successiva per
  // rilevare `cur >= ve` e porre `finito = true` — senza questo
  // controllo DOPO il ciclo, il caso limite "esattamente 10 000"
  // darebbe un E-HDR-06.too-many SPURIO (falso negativo sul limite).
  if not finito and cur >= ve { finito = true }
  if not finito {
    fail(
      "E-HDR-06", "header-units." + nome-unita, locale,
      modello: "E-HDR-06.too-many",
      argomenti: (unit: nome-unita, max: str(max-segmenti)),
    )
  }
  risultato
}
