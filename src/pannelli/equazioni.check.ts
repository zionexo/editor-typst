/**
 * equazioni.check.ts — Verifica di `equazioni-logica.ts`. Esce con errore al primo controllo fallito.
 */
import {
  PACCHETTI,
  STRUTTURE,
  generaFormula,
  generaMitex,
  generaStruttura,
  haImport,
  leggiImpostazioni,
  pianoInserimento,
  rigaImport,
  scriviBlocco,
  strutturaPerId,
  testoBlocco,
  valoriIniziali,
  type ImpostazioniEquazioni,
} from "./equazioni-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}
/** Applica un piano al testo, come farebbe l'editor (cambi sul testo originale, in ordine). */
function applica(testo: string, cambi: readonly { da: number; a: number; testo: string }[]): string {
  let fuori = "";
  let da = 0;
  for (const c of cambi) {
    fuori += testo.slice(da, c.da) + c.testo;
    da = c.a;
  }
  return fuori + testo.slice(da);
}
const codice = (id: string, v: Record<string, string> = {}): string => {
  const s = strutturaPerId(id)!;
  const r = generaStruttura(s, { ...valoriIniziali(s), ...v });
  vero("codice" in r, `${id}: ${JSON.stringify(r)}`);
  return (r as { codice: string }).codice;
};

// strutture native, con i valori predefiniti
uguali(codice("frazione"), "frac(a, b)", "frazione");
uguali(codice("radice"), "sqrt(x)", "radice quadrata");
uguali(codice("radice", { indice: "3" }), "root(3, x)", "radice con indice");
uguali(codice("sommatoria"), "sum_(i = 1)^(n) a_i", "sommatoria");
uguali(codice("sommatoria", { op: "product" }), "product_(i = 1)^(n) a_i", "produttoria");
uguali(codice("integrale"), "integral_(0)^(1) f(x) dif x", "integrale");
uguali(codice("limite"), "lim_(x -> 0) f(x)", "limite");
uguali(codice("matrice"), "mat(a_(11), a_(12); a_(21), a_(22))", "matrice 2x2");
uguali(codice("matrice", { righe: "1", colonne: "3" }), "mat(a_(11), a_(12), a_(13))", "matrice 1x3");
uguali(codice("sistema"), "cases(a_1 x + b_1 y = c_1, a_2 x + b_2 y = c_2)", "sistema");
uguali(codice("vettore"), "vec(x_1, x_2, x_3)", "vettore");
uguali(codice("binomiale"), "binom(n, k)", "binomiale");
uguali(codice("accento", { tipo: "bar", base: "y" }), "bar(y)", "accento");
// physica
uguali(codice("derivata"), "dv(f, x)", "derivata");
uguali(codice("parziale"), "pdv(f, x)", "derivata parziale");
uguali(codice("differenziale"), "dd(x)", "differenziale");
uguali(codice("gradiente"), "grad f", "gradiente");
uguali(codice("braket"), "braket(phi, psi)", "braket");
vero(strutturaPerId("ket")!.pacchetto === "physica" && strutturaPerId("frazione")!.pacchetto === null, "pacchetto per struttura");

// validazione
const s = strutturaPerId("matrice")!;
const casiNonValidi: Record<string, string>[] = [{ righe: "0" }, { righe: "9" }, { righe: "1.5" }, { righe: "x" }, { colonne: "" }];
for (const v of casiNonValidi) {
  vero("errore" in generaStruttura(s, { ...valoriIniziali(s), ...v }), `matrice rifiutata: ${JSON.stringify(v)}`);
}
vero("errore" in generaStruttura(strutturaPerId("frazione")!, { num: "a $ b", den: "c" }), "dollaro rifiutato");
vero("errore" in generaStruttura(strutturaPerId("frazione")!, { num: "a\nb", den: "c" }), "a capo rifiutato");
vero("errore" in generaStruttura(strutturaPerId("frazione")!, { num: " ", den: "c" }), "campo vuoto obbligatorio");
vero("errore" in generaStruttura(strutturaPerId("accento")!, { tipo: "rm -rf", base: "x" }), "scelta non ammessa");
vero("codice" in generaStruttura(strutturaPerId("radice")!, { radicando: "x", indice: "" }), "campo facoltativo vuoto");
vero(STRUTTURE.every((st) => new Set(st.campi.map((c) => c.id)).size === st.campi.length), "id dei campi unici");
vero(new Set(STRUTTURE.map((st) => st.id)).size === STRUTTURE.length, "id delle strutture unici");
// ogni struttura con i predefiniti produce codice senza $ né a capo
vero(
  STRUTTURE.every((st) => {
    const r = generaStruttura(st, valoriIniziali(st));
    return "codice" in r && !/[$\n]/.test(r.codice);
  }),
  "predefiniti validi per tutte le strutture",
);

// formula
uguali(generaFormula("x^2", { visualizzata: false, etichetta: "" }), "$x^2$", "in linea");
uguali(generaFormula("x^2", { visualizzata: true, etichetta: "" }), "$ x^2 $\n", "visualizzata");
uguali(generaFormula("a + b", { visualizzata: true, etichetta: "eq:somma" }), "$ a + b $ <eq:somma>\n", "con etichetta");
uguali(generaFormula("a\n+ b", { visualizzata: true, etichetta: "" }), "$ a\n  + b $\n", "formula su più righe");
vero(generaFormula("", { visualizzata: true, etichetta: "" }) === null, "formula vuota");
vero(generaFormula("a $ b", { visualizzata: true, etichetta: "" }) === null, "dollaro nella formula");
vero(generaFormula("a", { visualizzata: true, etichetta: "1x" }) === null, "etichetta non valida");

// mitex
uguali(generaMitex("\\frac{1}{2}", false), "#mitex(`\\frac{1}{2}`)\n", "mitex blocco");
uguali(generaMitex("x^2", true), "#mi(`x^2`)", "mitex in linea");
vero(generaMitex("a`b", false) === null && generaMitex("  ", false) === null, "mitex rifiutato");

// import dei pacchetti
uguali(rigaImport("physica"), `#import "@preview/physica:${PACCHETTI.physica.versione}": *`, "import physica");
uguali(rigaImport("equate"), `#import "@preview/equate:${PACCHETTI.equate.versione}": equate`, "import equate");
vero(haImport('#import "@preview/physica:0.9.0": *', "physica") && !haImport("= T", "physica"), "haImport");
const piano = pianoInserimento("= T\n", { da: 4, a: 4 }, "$x$", ["physica", "physica"]);
uguali(applica("= T\n", piano.cambi), `${rigaImport("physica")}\n\n= T\n$x$`, "import una sola volta, in cima");
uguali(piano.selezione, { da: rigaImport("physica").length + 2 + 4, a: rigaImport("physica").length + 2 + 4 + 3 }, "selezione dopo l'import");
const gia = pianoInserimento('#import "@preview/physica:0.9.0": *\n= T', { da: 5, a: 7 }, "$x$", ["physica"]);
uguali(gia.cambi.length, 1, "import già presente: nessuna aggiunta");
const zero = pianoInserimento("ab", { da: 0, a: 0 }, "$x$", ["mitex"]);
uguali(applica("ab", zero.cambi), `${rigaImport("mitex")}\n\n$x$ab`.replace("$x$ab", "$x$ab"), "inserimento all'offset 0 insieme all'import");
vero(pianoInserimento("ab", { da: 0, a: 1 }, "Z", []).cambi.length === 1, "senza pacchetti un solo cambio");

// blocco delle impostazioni
const tutti: ImpostazioniEquazioni = { numerazione: true, equate: true };
vero(testoBlocco({ numerazione: false, equate: false }) === "", "blocco vuoto se tutto spento");
vero(testoBlocco({ numerazione: true, equate: false }).includes('numbering: "(1)"'), "numerazione semplice");
vero(testoBlocco(tutti).includes("equate.with(") && testoBlocco(tutti).includes('numbering: "(1.1)"'), "equate con sotto-numerazione");
uguali(leggiImpostazioni("= T"), { numerazione: false, equate: false }, "senza blocco");
uguali(leggiImpostazioni(testoBlocco(tutti) + "= T"), tutti, "round-trip equate");
uguali(leggiImpostazioni(testoBlocco({ numerazione: true, equate: false })), { numerazione: true, equate: false }, "round-trip numerazione");
vero(scriviBlocco("= T", { numerazione: false, equate: false }) === null, "niente da fare senza blocco");
const inserito = applica("= T", scriviBlocco("= T", { numerazione: true, equate: false })!.cambi);
vero(inserito.startsWith("// typster:equazioni\n") && inserito.endsWith("= T"), "blocco inserito in cima");
uguali(scriviBlocco(inserito, { numerazione: true, equate: false }), null, "idempotente");
const cambiato = applica(inserito, scriviBlocco(inserito, tutti)!.cambi);
uguali(leggiImpostazioni(cambiato), tutti, "blocco aggiornato");
vero(cambiato.endsWith("= T") && cambiato.split("typster:equazioni").length === 3, "un solo blocco, resto intatto");
const tolto = applica(cambiato, scriviBlocco(cambiato, { numerazione: false, equate: false })!.cambi);
uguali(tolto, "= T", "blocco rimosso del tutto");
// un testo che cita il marcatore a metà riga non è un blocco
uguali(leggiImpostazioni("x // typster:equazioni\n// fine typster:equazioni"), { numerazione: false, equate: false }, "marcatore non a inizio riga");

console.log("OK equazioni");
