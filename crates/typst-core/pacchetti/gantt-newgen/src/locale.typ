// gantt-newgen — src/locale.typ
// Caricamento, validazione e risoluzione dei locali (RF6). Dati esterni
// versionati in data/locales/<id>.toml (gate di revisione manuale);
// questo modulo e' SOLO logica — nessun testo in linguaggio naturale qui
// dentro (skill testo-utente-fonte-unica-nel-core).

#import "errors.typ": catalogo, fail, interpola, nome-tipo
#import "schema.typ": suggerisci

/// Chiavi obbligatorie di un locale e la lunghezza richiesta se sono un
/// array (RF6); `none` = non e' un array, si controlla solo la presenza.
#let chiavi-richieste = (
  "months": 12,
  "months-short": 12,
  "weekdays": 7,
  "weekdays-short": 7,
  "weekdays-letter": 7,
  "today": none,
  "week-prefix": none,
  "quarter-prefix": none,
  "first-weekday": none,
  "milestone-date-format": none,
)

/// Locali inclusi, caricati una volta da data/locales/*.toml (percorso
/// assoluto, T17: dentro al pacchetto `/` e' la radice del pacchetto).
/// `toml()` darebbe le date come `datetime` (T9), ma qui non ci sono
/// campi data.
#let locales = (
  en: toml("/data/locales/en.toml"),
  it: toml("/data/locales/it.toml"),
)

/// Descrizione di "quanto" atteso per una chiave di `chiavi-richieste`
/// (S-4: spostata nelle voci composite `E-LOC-02.scalar`/
/// `E-LOC-02.count` di `data/locales/en.toml`, non piu' una stringa
/// scritta qui nel codice — anche se E-LOC-02 e' "sempre in en" per
/// RF7, quella prosa e' comunque un DATO, non logica).
#let _descrizione-attesa(lunghezza-attesa) = if lunghezza-attesa == none {
  locales.en.errors.at("E-LOC-02.scalar")
} else {
  interpola(locales.en.errors.at("E-LOC-02.count"), (n: str(lunghezza-attesa)))
}

/// Verifica che `dizionario` abbia tutte le chiavi di `chiavi-richieste`
/// con la lunghezza giusta per gli array; altrimenti E-LOC-02 (sempre in
/// inglese, RF7: "gli errori sul locale stesso... sono sempre in en").
/// `percorso-base` e' tipicamente `"locale"`.
#let _verifica-completezza(dizionario, percorso-base) = {
  for (chiave, lunghezza-attesa) in chiavi-richieste {
    if chiave not in dizionario {
      // Rifinitura (chiusura Fase 2): "missing" era un letterale cablato
      // qui (non un dato), anche se E-LOC-02 e' "sempre en" (RF7): ora
      // viene dal catalogo (E-LOC-02.missing), come le altre due voci
      // composite del "problema" di questo codice.
      fail(
        "E-LOC-02", percorso-base + "." + chiave, locales.en,
        argomenti: (expected: _descrizione-attesa(lunghezza-attesa)),
        received: locales.en.errors.at("E-LOC-02.missing"),
      )
    }
    let valore = dizionario.at(chiave)
    if lunghezza-attesa != none {
      let lunghezza-ottenuta = if type(valore) == array { valore.len() } else { none }
      if lunghezza-ottenuta != lunghezza-attesa {
        fail(
          "E-LOC-02", percorso-base + "." + chiave, locales.en,
          argomenti: (expected: _descrizione-attesa(lunghezza-attesa)),
          received: if lunghezza-ottenuta == none { nome-tipo(valore) } else { str(lunghezza-ottenuta) },
        )
      }
    }
  }
}

/// Verifica il TIPO/dominio delle chiavi scalari usate ALTROVE come
/// numeri o stringhe "vive" (S1, terza revisione Fase 4): `first-weekday`
/// deve essere un intero 1-7 (RF6: "1 = lunedi'"; usato in
/// `timeline.typ`/`units.typ` per l'aritmetica dello snap `week`, D-028
/// — un valore fuori range o di tipo sbagliato farebbe panic "nudo" li');
/// `week-prefix`/`quarter-prefix` devono essere stringhe (concatenate
/// direttamente nelle etichette, `src/units.typ`); `milestone-date-format`
/// deve essere una stringa (DS-2, revisione 2026-10-17: usata in
/// `formatta-milestone` con `.replace(..)` — senza questa guardia un
/// valore non stringa (es. un intero) arriva indenne fino a quella
/// chiamata, dove farebbe panic "nudo" di Typst, stesso schema di
/// T27/T33/D-110). Chiamata DOPO `_verifica-completezza` (le chiavi
/// devono gia' esistere). Sempre in "en" come il resto degli errori sul
/// locale stesso (RF7).
#let _verifica-tipi(dizionario, percorso-base) = {
  let fw = dizionario.first-weekday
  if type(fw) != int or fw < 1 or fw > 7 {
    fail(
      "E-LOC-02", percorso-base + ".first-weekday", locales.en,
      argomenti: (expected: locales.en.errors.at("E-LOC-02.first-weekday")),
      received: if type(fw) == int { repr(fw) } else { nome-tipo(fw) },
    )
  }
  for chiave in ("week-prefix", "quarter-prefix", "milestone-date-format") {
    let v = dizionario.at(chiave)
    if type(v) != str {
      fail(
        "E-LOC-02", percorso-base + "." + chiave, locales.en,
        argomenti: (expected: locales.en.errors.at("E-LOC-02.string")),
        received: nome-tipo(v),
      )
    }
  }
}

/// Risolve `valore` (stringa id incluso, dizionario di override parziale
/// `(base: "it", ...)`, o dizionario completo di un locale utente) nel
/// locale effettivo: dizionario con tutte le chiavi di
/// `chiavi-richieste` piu' `errors`, tabella COMPLETA su tutti i codici
/// del catalogo (fallback a "en" voce per voce per ogni codice mancante,
/// RF6/RF7 — mai E-LOC-02 per voci di `errors` mancanti, sono le uniche
/// chiavi facoltative di un locale utente). `percorso` e' il percorso
/// nello schema d'origine (tipicamente `"locale"`), usato nei messaggi
/// E-LOC-01/02. `strict` (rifinitura, decisione 2026-10-15: proposta
/// approvata dal coordinatore) governa SOLO il controllo sulle chiavi di
/// `errors`: una chiave che non e' un codice del catalogo (es. il refuso
/// "E-VAL-1O" con la lettera O al posto dello zero) da' E-VAL-02 quando
/// `strict` e' vero; con `strict: false` resta ignorata in silenzio,
/// come ogni altra chiave sconosciuta dello schema (D-018).
#let resolve(valore, percorso: "locale", strict: true) = {
  let base-dict = none
  let override-dict = (:)

  if type(valore) == str {
    if valore not in locales {
      // I-6/S-3: "received" (il valore scritto) e "allowed" (forma
      // standard di RF7, non un segnaposto ad-hoc `{included}`).
      fail(
        "E-LOC-01", percorso, locales.en,
        received: repr(valore), allowed: locales.keys().sorted(),
      )
    }
    base-dict = locales.at(valore)
  } else if type(valore) == dictionary {
    if "base" in valore {
      let id-base = valore.at("base")
      if type(id-base) != str or id-base not in locales {
        // I-6/S-3: anche nel ramo ".base" il "received" e' il valore
        // scritto (es. "fr"), non l'intero dizionario di override.
        fail(
          "E-LOC-01", percorso + ".base", locales.en,
          received: repr(id-base), allowed: locales.keys().sorted(),
        )
      }
      base-dict = locales.at(id-base)
      override-dict = valore
    } else {
      base-dict = valore
    }
  } else {
    fail(
      "E-VAL-03", percorso, locales.en,
      argomenti: (expected: "string|dictionary"),
      received: nome-tipo(valore),
    )
  }

  let risolto = base-dict
  for (chiave, valore-override) in override-dict {
    if chiave == "base" { continue }
    risolto.insert(chiave, valore-override)
  }

  // I-7: la completezza si verifica DOPO il merge, sia col ramo
  // `base:` sia senza — prima si verificava SOLO il dizionario
  // dell'utente nel ramo senza `base`, quindi un override come
  // `(base: "it", months: ())` scriveva sopra una base completa senza
  // che nessun controllo lo rilevasse (bug trovato in revisione).
  _verifica-completezza(risolto, percorso)
  _verifica-tipi(risolto, percorso)

  // "errors": fallback a "en" voce per voce per OGNI codice del
  // catalogo attivo (RF6: le voci mancanti ricadono su "en" senza
  // generare E-LOC-02) piu' le tre voci comuni. Un `errors` fornito
  // dall'utente che non e' un dizionario e' un errore di tipo (I-7),
  // non un valore silenziosamente ignorato.
  let errori-utente = risolto.at("errors", default: (:))
  if type(errori-utente) != dictionary {
    fail(
      "E-VAL-03", percorso + ".errors", locales.en,
      argomenti: (expected: "dictionary"), received: nome-tipo(errori-utente),
    )
  }
  // S-6: ogni VALORE di `errors` deve essere una stringa (e' un
  // modello di messaggio): un valore di un altro tipo non viene
  // ignorato in silenzio (sarebbe un `panic` "nudo" di Typst dentro
  // `interpola`/`str.replace` la prima volta che il codice venisse
  // davvero emesso), ma segnalato subito con percorso preciso.
  for (codice-utente, valore-utente) in errori-utente {
    if type(valore-utente) != str {
      fail(
        "E-VAL-03", percorso + ".errors." + codice-utente, locales.en,
        argomenti: (expected: "string"), received: nome-tipo(valore-utente),
      )
    }
  }
  // Rifinitura: una chiave di "errors" che non e' un codice del
  // catalogo (ne' una delle tre voci comuni) e' quasi certamente un
  // refuso (es. "E-VAL-1O" al posto di "E-VAL-10"): senza questo
  // controllo la voce passava indenne (mai letta da nessuno, perche'
  // "errori-finali" sotto pesca SOLO le chiavi del catalogo) — lo stesso
  // difetto D7 che l'intero RF7 vuole evitare, qui dentro i dati del
  // locale stesso. Solo con "strict" (sempre in "en": e' un errore sul
  // locale stesso, RF7).
  if strict {
    let ammesse = catalogo.keys() + ("received", "allowed", "suggest")
    for chiave-utente in errori-utente.keys() {
      if chiave-utente not in ammesse {
        fail(
          "E-VAL-02", percorso + ".errors." + chiave-utente, locales.en,
          suggest: suggerisci(chiave-utente, ammesse),
        )
      }
    }
  }
  let errori-finali = (:)
  for codice in catalogo.keys() {
    errori-finali.insert(codice, errori-utente.at(codice, default: locales.en.errors.at(codice)))
  }
  for chiave-comune in ("received", "allowed", "suggest") {
    errori-finali.insert(chiave-comune, errori-utente.at(chiave-comune, default: locales.en.errors.at(chiave-comune)))
  }
  risolto.insert("errors", errori-finali)
  risolto
}

/// Segnaposto ammessi nel modello di `formatta-milestone` (RF6):
/// registro usato sia per la sostituzione sotto sia per l'elenco
/// `allowed` di E-VAL-08 (segnaposto sconosciuto residuo) — fonte
/// unica (skill registro-regole-validazione). `{weekday}` (review di
/// Fase 6b): nome COMPLETO del giorno (`locale.weekdays`), accanto al
/// già esistente `{weekday-short}`.
#let placeholder-milestone = ("day", "month", "month-short", "weekday", "weekday-short", "year")

/// Formatta una DATA (sola data, D-106 punto (b) — "data localizzata
/// delle etichette milestone") per l'etichetta di una milestone/"oggi"
/// (RF6: `locale.milestone-date-format`, modello con segnaposto propri
/// — vedi `placeholder-milestone` sopra — localizzati, perché
/// `datetime.display` non lo è, T7). Sostituzione letterale (come
/// `errors.interpola`, non regex): ogni segnaposto NOTO non scritto nel
/// modello resta semplicemente non sostituito; un segnaposto fra `{ }`
/// SCONOSCIUTO (refuso, es. `{yeer}`) resta anch'esso non sostituito ma
/// è un ERRORE (sotto, E-VAL-08: quasi certamente un refuso dell'utente,
/// non un segnaposto ammesso da ignorare in silenzio). Prima
/// `layout.typ` usava SEMPRE `dates.formatta` (ISO fisso, "YYYY-MM-DD",
/// pensato per i MESSAGGI D'ERRORE, mai localizzato per RF7) anche per
/// le etichette mostrate all'utente — nessuna differenza fra `locale:
/// "it"` e `"en"` nella data della milestone.
/// - d (datetime): data (sola data, mai con ora, T3).
/// - locale (dictionary): locale GIA' risolto (`resolve(..)`, con
///   `months`/`months-short`/`weekdays`/`weekdays-short`/
///   `milestone-date-format` completi — garantito da
///   `_verifica-completezza`).
/// - formato (string|none): modello da usare al posto di
///   `locale.milestone-date-format` (Fase 6b Parte B:
///   `style.milestones.label.date-format`, override PER DOCUMENTO —
///   RF5, stessa precedenza "stile vince sul locale" di ogni altro
///   campo di `style`). `none` (default) = usa quello del locale.
///   **Review di Fase 6b (S1): tipo validato QUI** (E-VAL-03, percorso
///   `"style.milestones.label.date-format"`) — `style:` in generale non
///   è validato in profondità (RF5), ma questo campo arriva fino a una
///   chiamata `.replace()` su un valore non-stringa, che farebbe
///   `panic` nudo di Typst senza la guardia (stesso schema di T27/T33,
///   D-110: un valore letto da `style.*` mai controllato altrove).
/// -> str (NON `content`: una stringa pura, come `dates.formatta` —
///    il chiamante la incorpora in `content` con `[#..]` se serve,
///    RF2 "content|str" per i campi testo).
#let formatta-milestone(d, locale, formato: none) = {
  if formato != none and type(formato) != str {
    fail(
      "E-VAL-03", "style.milestones.label.date-format", locale,
      argomenti: (expected: "string"), received: nome-tipo(formato),
    )
  }
  let da-stile = formato != none
  let modello = if da-stile { formato } else { locale.at("milestone-date-format") }
  let risultato = modello
    .replace("{day}", str(d.day()))
    .replace("{month}", locale.months.at(d.month() - 1))
    .replace("{month-short}", locale.at("months-short").at(d.month() - 1))
    .replace("{weekday}", locale.weekdays.at(d.weekday() - 1))
    .replace("{weekday-short}", locale.at("weekdays-short").at(d.weekday() - 1))
    .replace("{year}", str(d.year()))
  // Review di Fase 6b (S1): un segnaposto fra `{ }` SCONOSCIUTO (non
  // sostituito da nessuna `.replace()` sopra) resta letteralmente nel
  // risultato — quasi certamente un refuso del modello (es. `{yeer}`
  // invece di `{year}`), non un segnaposto da ignorare in silenzio:
  // E-VAL-08 con l'elenco ORDINATO di quelli ammessi. Percorso
  // diverso a seconda della fonte del modello (stile o locale): il
  // messaggio deve indicare DOVE correggere il refuso.
  let residui = risultato.matches(regex("\{[a-zA-Z0-9_-]+\}"))
  if residui.len() > 0 {
    fail(
      "E-VAL-08",
      if da-stile { "style.milestones.label.date-format" } else { "locale.milestone-date-format" },
      locale,
      received: residui.at(0).text, allowed: placeholder-milestone.sorted().map(p => "{" + p + "}"),
    )
  }
  risultato
}
