// gantt-newgen — src/errors.typ
// Catalogo dei codici d'errore (RF7) e funzione unica di emissione.
//
// Principio "testo-utente-fonte-unica-nel-core" (skill omonima): qui non
// c'e' NESSUN testo in linguaggio naturale. Il catalogo elenca solo i
// CODICI attivi e i nomi dei segnaposto ammessi per ciascuno (D-008,
// RF6/RF7): il testo del modello vive in data/locales/<id>.toml, tabella
// [errors] (dati esterni versionati, gate di revisione manuale).
//
// Forma del messaggio finale (RF7):
//   gantt-newgen [<codice>]: <percorso>: <problema> (<received>: <valore>)[ — <extra...>]
// <percorso>, il codice e il prefisso "gantt-newgen" non si traducono;
// <problema> viene dal modello del locale gia' risolto (con fallback a
// "en" voce per voce, applicato da locale.typ prima di arrivare qui:
// RF6/RF7, D-019). Nessun `panic` con argomenti separati (D6): qui si
// costruisce sempre UNA stringa interpolata.

/// Catalogo dei codici attivi di RF7: per ciascuno, l'elenco dei nomi dei
/// segnaposto ammessi nel suo modello (oltre a `{path}`, che la funzione
/// d'errore aggiunge da se' e NON e' un segnaposto del modello — e'
/// gia' il `percorso` passato a `fail()`). Fonte unica usata da:
/// - `fail()` qui sotto, per interpolare il modello del locale risolto;
/// - `unit/locales.typ`, per verificare che "en" e "it" abbiano la stessa
///   voce con GLI STESSI segnaposto (RF6, skill
///   totalita-tabella-presentazione).
/// `E-HDR-03` e' RITIRATO (D-029/SD-4): nessuna voce qui, il codice non
/// si riassegna. `E-DEP-01…05` sono emessi da `graph.typ` da questa fase
/// (Fase 3, D-074): raggiungibili davvero, non solo "attivi per RF6".
/// `E-HDR-01/02/04/05/06` sono emessi da `validate.typ`/`units.typ` da
/// Fase 4 (E-HDR-01/02/04 su `headers` contro il registry, E-HDR-05
/// sulle definizioni custom, E-HDR-06 a runtime su `floor`/`next`):
/// raggiungibili davvero, non solo "attivi per RF6" (commento corretto,
/// terza revisione Fase 4, era stantio). `E-DRW-*` restano solo
/// "attivi" per RF6 (il locale li deve avere) ma nessun modulo li emette
/// ancora (Fase 6b).
#let catalogo = (
  "E-API-01": (),
  "E-API-02": (),
  "E-VAL-01": ("key",),
  "E-VAL-02": (),
  "E-VAL-03": ("expected",),
  "E-VAL-04": (),
  "E-VAL-05": ("field", "value", "other-field", "other-value"),
  "E-VAL-06": (),
  "E-VAL-07": (),
  "E-VAL-08": (),
  "E-VAL-09": ("end", "start", "mode", "end-written"),
  // Fase 5 (rilievo del coordinatore, wording): valore di `{end-written}`
  // quando "end" del viewport e' AUTOMATICO (non scritto dall'utente) —
  // senza questa voce, il messaggio mostrava la fine automatica come se
  // fosse "scritta" (fuorviante).
  "E-VAL-09.auto": (),
  "E-VAL-10": (),
  "E-VAL-11": ("key-a", "schema-a", "key-b", "schema-b"),
  // Voci "composite" (I-5/DS-7-bis, S-4): etichette o modelli alternativi
  // TRADOTTI usati come VALORE di un segnaposto (es. `{schema-a}` di
  // E-VAL-11) o come MODELLO alternativo selezionato con l'argomento
  // `modello:` di `fail()` (es. `E-VAL-01.no-dates`, variante di E-VAL-01
  // senza `{key}`). Stesso meccanismo del catalogo principale (totalita'
  // verificata da `unit/locales.typ`), solo con una chiave composta
  // "<codice>.<nome>" invece di un codice RF7 vero e proprio — il
  // messaggio finale mostra comunque "[<codice>]" (l'argomento `codice`
  // di `fail()`), mai la chiave composta.
  "E-VAL-11.viewport-explicit": (),
  "E-VAL-11.viewport-alias": (),
  // Fase 3 (compat.typ): etichette composite delle DUE FACCE dello
  // schema, riusate dal modello generico di E-VAL-11 (stesse 4
  // segnaposto di sopra) per "taskgroups"+"tasks" di primo livello,
  // "subtasks" su un gruppo 0.1 e "tasks" dentro un nodo 0.5 — nessuna
  // voce composita dedicata per ciascun caso, solo queste due etichette
  // (D-061/registro-regole-validazione: un modello, piu' valori).
  "E-VAL-11.schema-01": (),
  "E-VAL-11.schema-05": (),
  "E-VAL-01.no-dates": (),
  "E-VAL-12": ("other-field", "other-detail"),
  // Seconda revisione Fase 3 (S9): modello DEDICATO per la
  // sovrapposizione fra intervalli (distinto dal conflitto
  // "intervals"+"start/end/done", che resta sul modello generico
  // sopra): il trattino "–" fra le due date vive nel TESTO del
  // modello (dati), non costruito con una concatenazione di stringhe
  // nel core (`validate.typ`, principio testo-utente-fonte-unica-nel-core).
  "E-VAL-12.overlap": ("other-field", "other-start", "other-end"),
  "E-VAL-13": ("depth", "max"),
  "E-HDR-01": (),
  "E-HDR-02": ("order", "expected"),
  "E-HDR-04": ("unit",),
  "E-HDR-05": ("detail",),
  // Fase 4 (units.typ, D-061): voci composite usate come VALORE del
  // segnaposto `{detail}` di E-HDR-05 sopra, selezionate dal core senza
  // prosa cablata. `E-HDR-05.duplicate-rank` ha un segnaposto proprio
  // (`{other}`), risolto con `interpola()` PRIMA di passarlo come
  // `{detail}` (stesso schema di `E-LOC-02.count`, che ha `{n}`).
  "E-HDR-05.missing-key": (),
  "E-HDR-05.reserved-name": (),
  "E-HDR-05.duplicate-rank": ("other",),
  "E-HDR-05.sprint-param": (),
  // S5 (terza revisione Fase 4, rilievo del coordinatore): chiave
  // sconosciuta in una definizione d'unita' custom (es. "gridlne:" per
  // refuso), controllata solo con `strict` (come il resto dello schema,
  // D-018) — suggerimento gia' gestito dall'argomento `suggest:` di
  // `fail()`, non da un segnaposto qui.
  "E-HDR-05.unknown-key": ("key",),
  // TODO 4 (D-091 punto 3d, terza revisione Fase 4): nome custom non
  // kebab-case (RF4.3).
  "E-HDR-05.kebab-case": (),
  "E-HDR-06": ("unit", "call"),
  // Fase 4: varianti del MODELLO di E-HDR-06 (selezionate con
  // `modello:`, D-061), per due condizioni diverse dal "non avanza"
  // della voce base: `floor` che non rispetta `floor(d) <= d`, e il
  // tetto di sicurezza di `max-segmenti` superato (nessuna delle due
  // frasi si sovrappone al testo "non avanza" della voce base, che
  // resta per `next(d) <= d`).
  "E-HDR-06.floor-invalid": ("unit", "call"),
  "E-HDR-06.too-many": ("unit", "max"),
  "E-DEP-01": ("id", "other-path"),
  "E-DEP-02": (),
  "E-DEP-03": ("id",),
  "E-DEP-04": ("cycle",),
  "E-DEP-05": ("name",),
  "E-DRW-01": (),
  "E-DRW-02": (),
  // Fase 5 (D-097, rilievo del coordinatore S "D6"): primo codice della
  // famiglia "layout" — un problema di geometria scoperto solo a
  // `layout.calcola()` (serve la larghezza VERA del contenitore,
  // risolta da `layout()`/`context`, non prima): `sidebar-width`
  // esplicito (RF2) che non lascia spazio per l'area barre.
  "E-LAY-01": ("width", "available"),
  // Review finale di Fase 6a (rilievo del coordinatore): variante di
  // E-LAY-01 per quando `sidebar-width` lascia spazio all'area barre
  // (il caso base resta valido) ma NON abbastanza per il NOME della
  // riga dopo l'inset orizzontale e il rientro per livello (R5, D-108:
  // la larghezza disponibile per `layout.spezza-a-capo` scenderebbe a
  // zero o sotto) — testo DIVERSO dal caso base (quello parla di "area
  // barre", qui il problema e' lo spazio per il testo della sidebar),
  // stessi segnaposto `{width}`/`{available}`.
  "E-LAY-01.sidebar-name": ("width", "available"),
  // D5 (rilievo del coordinatore, rereview di Fase 6a, D-106): il
  // messaggio di `style.allineamento-orizzontale` (`style.sidebar.
  // align`/futuri usi) aveva la frase "alignment con componente
  // orizzontale" cablata in ITALIANO dentro `style.typ`, mostrata anche
  // con `locale: "en"` — violava "testo-utente-fonte-unica-nel-core"
  // (RF7: nessuna prosa fuori dai locali). Voce composita usata come
  // VALORE di `{expected}` di E-VAL-03 (stesso schema di `E-HDR-05.*`).
  "E-VAL-03.horizontal": (),
  // Rereview di Fase 8.4 (D-018, decisione dell'utente del 2026-10-19):
  // stessa tecnica di `E-VAL-03.horizontal` sopra — `headers: ()`
  // esplicito (array VUOTO) rifiutato in `validate.typ`, messaggio
  // "array non vuoto"/"a non-empty array" letto dal locale come VALORE
  // di `{expected}`, non cablato nel core (RF7 "testo-utente-fonte-
  // unica-nel-core").
  "E-VAL-03.non-empty": (),
  // S-3: E-LOC-01 nella forma standard di RF7 ("received"/"allowed"
  // strutturali, niente segnaposto proprio: era l'unico codice con un
  // segnaposto ad-hoc, `{included}`, per un elenco che "allowed" gia'
  // fa in generale).
  "E-LOC-01": (),
  "E-LOC-02": ("expected",),
  // S-4: le due varianti del "problema" di E-LOC-02 (chiave scalare
  // mancante vs. array della lunghezza sbagliata) come voci composite,
  // cosi' anche questa prosa (per quanto sempre in "en", RF7) vive nei
  // dati e non nel codice di locale.typ.
  "E-LOC-02.scalar": (),
  "E-LOC-02.count": ("n",),
  // S1 (terza revisione Fase 4, rilievo del coordinatore): `locale.typ`
  // non validava il TIPO/dominio di `first-weekday`
  // (`week-prefix`/`quarter-prefix` erano gia' implicitamente dei
  // valori scalari qualunque) — un `first-weekday: "7"` (stringa) o
  // `first-weekday: 8` passava indenne e avrebbe fatto panic "nudo"
  // dentro `src/timeline.typ`/`src/units.typ` (confronto/aritmetica con
  // un intero fuori range o un tipo sbagliato).
  "E-LOC-02.first-weekday": (),
  "E-LOC-02.string": (),
  // Rifinitura di chiusura Fase 2: `locale.typ` aveva `received:
  // "missing"` cablato in logica (il letterale non era nel core PRIMA
  // di questa correzione, violando "testo-utente-fonte-unica-nel-core"
  // anche se e' sempre in inglese per RF7, E-LOC-02 e' "sempre en"):
  // ora e' questa voce del catalogo, "morta" come E-LOC-02.scalar/.count
  // (mai mostrata diversamente dall'inglese, ma presente in entrambe le
  // lingue per la totalita' di unit/locales.typ, S-8).
  "E-LOC-02.missing": (),
)

/// Interpola un modello con segnaposto nominati `{nome}` (sostituzione
/// letterale: `str.replace` su un pattern di tipo `str`, non `regex`, non
/// interpreta caratteri speciali). Il chiamante passa esattamente i nomi
/// dichiarati nel catalogo per quel codice (verificato da
/// `unit/locales.typ`): un segnaposto non previsto nel modello resta
/// semplicemente non sostituito, non e' un errore qui (la totalita' la
/// garantisce il test sul catalogo, non questa funzione a runtime).
#let interpola(modello, argomenti) = {
  let s = modello
  for (nome, valore) in argomenti {
    s = s.replace("{" + nome + "}", valore)
  }
  s
}

/// Nome del tipo Typst di un valore, per i segnaposto/`received` dei
/// messaggi di tipo (E-VAL-03, E-API-01, ...). Non tradotto (RF7: "nomi
/// di chiave e repr dei valori non si traducono" — lo stesso vale per i
/// nomi di tipo, che sono token tecnici come i nomi di chiave, non prosa).
#let nome-tipo(valore) = str(type(valore))

/// Emette l'errore `codice` nel punto `percorso` (es.
/// `taskgroups[1].tasks[0].end`, indici base 0, RF7) con il modello del
/// `locale` gia' risolto (RF6: fallback a "en" voce per voce, applicato
/// da `locale.resolve` prima di arrivare qui). Termina sempre con un
/// `panic` su una stringa unica, gia' interpolata (D6).
///
/// - codice (string): codice del catalogo RF7 (es. `"E-VAL-05"`).
/// - percorso (string): percorso nello schema d'origine.
/// - locale (dictionary): locale risolto, con `locale.errors.<codice>` e
///   le voci comuni `received`/`allowed`/`suggest` (gia' col fallback a
///   "en" applicato da `locale.typ`).
/// - argomenti (dictionary): valori per i segnaposto dichiarati nel
///   catalogo per `codice` (default `(:)`, nessun segnaposto).
/// - received (string|none): valore "ricevuto" da mostrare fra parentesi
///   dopo il problema, gia' formattato (repr o nome di tipo); `none` =
///   non mostrato.
/// - allowed (array|none): elenco di valori ammessi, gia' in forma
///   stringa; `none` = non mostrato.
/// - suggest (string|none): valore suggerito (es. una chiave), passato al
///   modello comune `suggest` ("forse '{value}'?"/"did you mean
///   '{value}'?"); `none` = nessun suggerimento.
/// - modello (string|none): chiave del catalogo da usare per il MODELLO
///   del messaggio, se diversa da `codice` (voce composita, es.
///   `"E-VAL-01.no-dates"` per una variante di `E-VAL-01` senza
///   segnaposto: DS-7-bis/S-4). Il tag `[<codice>]` mostrato nel
///   messaggio resta sempre quello di `codice`, non quello di `modello`
///   — e' solo il TESTO del problema a venire dalla voce composita.
///   `none` (default) = usa `codice` anche per il modello (caso comune).
/// -> none (non ritorna mai: la funzione termina sempre con `panic`)
#let fail(
  codice,
  percorso,
  locale,
  argomenti: (:),
  received: none,
  allowed: none,
  suggest: none,
  modello: none,
) = {
  let chiave-modello = if modello != none { modello } else { codice }
  // Fallback NEUTRO (non in italiano, non in nessun'altra lingua: e'
  // un segnale di bug del programmatore, non testo per l'utente finale
  // — se compare davvero, qualunque lingua lo riconosce come anomalo).
  let testo-modello = locale.errors.at(chiave-modello, default: "[MISSING-TEMPLATE:" + chiave-modello + "]")
  let problema = interpola(testo-modello, argomenti)
  let msg = "gantt-newgen [" + codice + "]: " + percorso + ": " + problema
  if received != none {
    msg += " (" + locale.errors.received + ": " + received + ")"
  }
  let extra = ()
  if allowed != none {
    extra.push(locale.errors.allowed + ": " + allowed.join(", "))
  }
  if suggest != none {
    extra.push(interpola(locale.errors.suggest, (value: suggest)))
  }
  if extra.len() > 0 {
    msg += " — " + extra.join(" — ")
  }
  panic(msg)
}
