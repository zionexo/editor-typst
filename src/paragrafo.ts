/**
 * paragrafo.ts — Individuazione del paragrafo del cursore (F5-023, modalità focus).
 *
 * Logica pura, senza CodeMirror né DOM (verificata da `modalita.check.ts`):
 * paragrafo = blocco di righe consecutive non vuote, delimitato da righe vuote
 * (o fatte di soli spazi), sia per `.typ` sia per `.md`.
 */

/** Righe oltre le quali la ricerca dei confini si ferma, per lato: protegge da un "paragrafo" di migliaia di righe. */
export const RIGHE_MAX_SCANSIONE = 5000;

/** Intervallo di righe (1-based, estremi inclusi). */
export interface IntervalloRighe {
  da: number;
  a: number;
}

/**
 * Intervallo del paragrafo che contiene `riga` (1-based).
 *
 * @param testoRiga testo della riga `n` (1..`righe`).
 * @param righe     numero di righe del documento (≥ 1).
 * @param riga      riga del cursore; fuori intervallo viene riportata nei limiti.
 *
 * Se la riga del cursore è vuota, il "paragrafo" è quella sola riga: appena si
 * inizia a scrivere su una riga nuova, tutto il resto è già schiarito.
 */
export function intervalloParagrafo(
  testoRiga: (n: number) => string,
  righe: number,
  riga: number,
): IntervalloRighe {
  if (righe < 1) {
    return { da: 1, a: 1 };
  }
  const r = Math.min(Math.max(Math.trunc(riga), 1), righe);
  const vuota = (n: number): boolean => testoRiga(n).trim() === "";
  if (vuota(r)) {
    return { da: r, a: r };
  }
  let da = r;
  while (da > 1 && r - da < RIGHE_MAX_SCANSIONE && !vuota(da - 1)) {
    da--;
  }
  let a = r;
  while (a < righe && a - r < RIGHE_MAX_SCANSIONE && !vuota(a + 1)) {
    a++;
  }
  return { da, a };
}
