// gantt-newgen — src/dates.typ
// Guardie e aritmetica sulle date (RF3, T1-T4, D-030). Mai ore nel
// modello: solo `duration(days: <int>)`. Nessun costruttore `datetime()`
// e' chiamato senza aver prima validato i componenti (T1): un panic di
// Typst non ha percorso né codice (contrario a RF7).

#import "errors.typ": fail, nome-tipo

/// Forma attesa di una data in stringa: "YYYY-MM-DD" (T8: `yaml()` da'
/// sempre le date come stringhe). `[0-9]` e non `\d`: in Typst `\d`
/// combacia anche con le cifre Unicode non ASCII (es. arabo-indiche
/// ٠-٩), che `int()` poi non sa convertire e farebbe panic nudo — qui
/// si vuole SOLO la forma ASCII, qualunque altra cifra deve cadere su
/// E-VAL-04, non su un panic di `int()` (D-030, I-8).
#let _re-data = regex("^[0-9]{4}-[0-9]{2}-[0-9]{2}$")

/// Anno bisestile (per la lunghezza di febbraio).
#let anno-bisestile(anno) = (
  calc.rem(anno, 4) == 0 and (calc.rem(anno, 100) != 0 or calc.rem(anno, 400) == 0)
)

/// Lunghezza del mese `mese` (1-12) dell'anno `anno` in giorni.
#let lunghezza-mese(anno, mese) = {
  let giorni = (31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31)
  if mese == 2 and anno-bisestile(anno) { 29 } else { giorni.at(mese - 1) }
}

/// Formatta una data (sola data, niente ora) come "YYYY-MM-DD" (T4:
/// `str(datetime)` e' un errore di Typst; `repr` darebbe
/// `datetime(year: .., month: .., day: ..)`, poco leggibile nei messaggi
/// d'errore di RF7 o nello spec normalizzato di RF1).
#let formatta(d) = {
  let due-cifre(n) = if n < 10 { "0" + str(n) } else { str(n) }
  // MINORI: l'anno ammesso va da 0 a 9999 (T1) — un anno sotto 1000
  // (es. 5) senza padding darebbe "5-03-01", non piu' "YYYY-MM-DD".
  let quattro-cifre(n) = {
    let s = str(n)
    while s.len() < 4 { s = "0" + s }
    s
  }
  quattro-cifre(d.year()) + "-" + due-cifre(d.month()) + "-" + due-cifre(d.day())
}

/// Valida i componenti di una data e li costruisce (T1: guardia PRIMA
/// del costruttore `datetime`, che altrimenti puo' fare un panic non
/// intercettabile di Typst, es. "date is invalid"). Restituisce
/// `(ok: bool, valore: datetime|none)`: non emette errori, e' compito di
/// `valida-data` decidere percorso e `repr` da mostrare nel messaggio.
#let _costruisci(anno, mese, giorno) = {
  if anno < 0 or anno > 9999 { return (ok: false, valore: none) }
  if mese < 1 or mese > 12 { return (ok: false, valore: none) }
  if giorno < 1 or giorno > lunghezza-mese(anno, mese) { return (ok: false, valore: none) }
  (ok: true, valore: datetime(year: anno, month: mese, day: giorno))
}

/// Valida e trasforma `valore` (stringa "YYYY-MM-DD", oppure un
/// `datetime` di sola data) in un `datetime` di sola data, nel punto
/// `percorso` dello schema d'origine (RF7). Guardia completa prima di
/// ogni costruzione (D-030): regex di forma, poi mese 1-12, poi giorno
/// <= lunghezza del mese (bisestili inclusi), poi anno 0-9999 (T1). Un
/// `datetime` con componenti di ora (T3: `date + duration(hours: ..)`
/// produce un datetime "con ora") e' rifiutato: il modello lavora solo
/// con date. `locale` e' il locale gia' risolto (per il testo del
/// messaggio, RF7/RF6).
#let valida-data(valore, percorso, locale) = {
  if type(valore) == datetime {
    if valore.hour() != none or valore.minute() != none or valore.second() != none {
      fail("E-VAL-04", percorso, locale, received: repr(valore))
    }
    if valore.year() == none or valore.month() == none or valore.day() == none {
      fail("E-VAL-04", percorso, locale, received: repr(valore))
    }
    return valore
  }
  if type(valore) != str {
    fail(
      "E-VAL-03", percorso, locale,
      argomenti: (expected: "string|datetime"),
      received: nome-tipo(valore),
    )
  }
  if valore.match(_re-data) == none {
    fail("E-VAL-04", percorso, locale, received: repr(valore))
  }
  let anno = int(valore.slice(0, 4))
  let mese = int(valore.slice(5, 7))
  let giorno = int(valore.slice(8, 10))
  let esito = _costruisci(anno, mese, giorno)
  if not esito.ok {
    fail("E-VAL-04", percorso, locale, received: repr(valore))
  }
  esito.valore
}

/// Differenza in giorni interi `b - a` (T2: `.days()` di una `duration`
/// e' sempre un `float`, anche per differenze esatte in giorni;
/// `calc.round` da solo resta `float`, va avvolto in `int(..)` prima di
/// usare il risultato come indice o durata).
#let giorni-fra(a, b) = int(calc.round((b - a).days()))

/// `d` piu' `n` giorni interi (mai ore nel modello, T3).
#let aggiungi-giorni(d, n) = d + duration(days: n)

/// Fine effettiva di una data "di fine" secondo `date-mode` (RF3):
/// `"inclusive"` (default) copre il giorno indicato (fine effettiva =
/// `d + 1` giorno); `"instant"` e' la semantica originale di gantty
/// (fine effettiva = `d`, un istante a mezzanotte).
#let fine-effettiva(d, modo) = if modo == "inclusive" { aggiungi-giorni(d, 1) } else { d }
