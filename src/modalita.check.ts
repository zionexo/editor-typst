/**
 * modalita.check.ts — Verifica eseguibile di `paragrafo.ts` (F5-023, modalità focus).
 * Stesso schema di `comandi.check.ts`; esce con errore al primo controllo fallito.
 */
import { intervalloParagrafo, RIGHE_MAX_SCANSIONE } from "./paragrafo.ts";

let n = 0;
function uguale<T>(atteso: T, reale: T, msg: string): void {
  n++;
  if (JSON.stringify(atteso) !== JSON.stringify(reale)) {
    throw new Error(`FALLITO: ${msg} (atteso ${JSON.stringify(atteso)}, ottenuto ${JSON.stringify(reale)})`);
  }
}

/** Interfaccia come quella di CodeMirror: righe 1-based da un array. */
function su(righe: string[]): (r: number) => IntervalloRighe {
  return (r) => intervalloParagrafo((k) => righe[k - 1], righe.length, r);
}
type IntervalloRighe = ReturnType<typeof intervalloParagrafo>;

const doc = [
  "= Titolo", //        1
  "", //                2
  "Primo paragrafo,", // 3
  "su due righe.", //   4
  "", //                5
  "   ", //              6  (solo spazi = vuota)
  "Secondo.", //        7
  "Ancora.", //         8
  "Fine", //            9
];
const p = su(doc);
uguale({ da: 1, a: 1 }, p(1), "titolo isolato");
uguale({ da: 3, a: 4 }, p(3), "prima riga del paragrafo");
uguale({ da: 3, a: 4 }, p(4), "ultima riga del paragrafo");
uguale({ da: 2, a: 2 }, p(2), "riga vuota: solo se stessa");
uguale({ da: 6, a: 6 }, p(6), "riga di soli spazi: vuota");
uguale({ da: 7, a: 9 }, p(8), "paragrafo fino a fine documento");
uguale({ da: 7, a: 9 }, p(9), "ultima riga del documento");

// Limiti: riga fuori intervallo riportata dentro.
uguale({ da: 1, a: 1 }, p(0), "riga 0 → prima riga");
uguale({ da: 1, a: 1 }, p(-7), "riga negativa → prima riga");
uguale({ da: 7, a: 9 }, p(500), "riga oltre la fine → ultima riga");
uguale({ da: 7, a: 9 }, p(8.9), "riga non intera");

// Documenti degeneri.
uguale({ da: 1, a: 1 }, su([""])(1), "documento vuoto");
uguale({ da: 1, a: 1 }, su(["solo"])(1), "una riga");
uguale({ da: 1, a: 1 }, intervalloParagrafo(() => "x", 0, 1), "zero righe");

// Markdown: elenco senza righe vuote è un solo blocco; le righe vuote separano.
const md = ["# Titolo", "", "- uno", "- due", "", "testo"];
uguale({ da: 3, a: 4 }, su(md)(4), "elenco Markdown");
uguale({ da: 6, a: 6 }, su(md)(6), "ultimo paragrafo Markdown");

// Un "paragrafo" enorme non fa scansioni illimitate: si ferma a RIGHE_MAX_SCANSIONE per lato.
const enorme = Array.from({ length: RIGHE_MAX_SCANSIONE * 3 }, (_, i) => `riga ${i}`);
const centro = RIGHE_MAX_SCANSIONE + Math.floor(RIGHE_MAX_SCANSIONE / 2);
const e = intervalloParagrafo((k) => enorme[k - 1], enorme.length, centro);
uguale(RIGHE_MAX_SCANSIONE, centro - e.da, "scansione all'indietro limitata");
uguale(RIGHE_MAX_SCANSIONE, e.a - centro, "scansione in avanti limitata");

console.log(`modalita.check.ts: OK (${n} controlli)`);
