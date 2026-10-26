/**
 * inserisci-logica.ts — Generatori puri del codice Typst dello strumento «Inserisci».
 *
 * Ogni generatore restituisce il testo da inserire oppure `null` se i dati non
 * sono validi (il chiamante mostra il messaggio). Nessun DOM, nessun Tauri:
 * verificata da `inserisci.check.ts`.
 */

import { escapaContenuto } from "./pagina-logica.ts";

export const RIGHE_MAX = 50;
export const COLONNE_MAX = 12;

/** Etichetta Typst valida (senza i `< >`). */
export function etichettaValida(nome: string): boolean {
  return /^[A-Za-z_][\w:.-]*$/.test(nome);
}

/** Etichette `<nome>` presenti nel testo, uniche e nell'ordine di comparsa. */
export function etichetteNelTesto(testo: string): string[] {
  const viste = new Set<string>();
  for (const m of testo.matchAll(/<([A-Za-z_][\w:.-]*)>/g)) {
    viste.add(m[1]);
  }
  return [...viste];
}

/** Stringa Typst letterale: barre normali, `\` e `"` escapati. */
export function stringaTypst(valore: string): string {
  return `"${valore.replace(/\\/g, "/").replace(/"/g, '\\"')}"`;
}

/** `#table(...)` con `righe` × `colonne` celle vuote e, se richiesto, la riga di intestazione. */
export function generaTabella(righe: number, colonne: number, intestazione: boolean): string | null {
  if (!Number.isInteger(righe) || !Number.isInteger(colonne)) {
    return null;
  }
  if (righe < 1 || righe > RIGHE_MAX || colonne < 1 || colonne > COLONNE_MAX) {
    return null;
  }
  const corpo = intestazione ? righe - 1 : righe;
  const parti = [`  columns: ${colonne},`];
  if (intestazione) {
    parti.push(`  table.header${Array.from({ length: colonne }, (_, i) => `[Colonna ${i + 1}]`).join("")},`);
  }
  for (let r = 0; r < corpo; r++) {
    parti.push(`  ${Array.from({ length: colonne }, () => "[]").join(", ")},`);
  }
  return `#table(\n${parti.join("\n")}\n)\n`;
}

/** `#figure(image(...), caption: [...]) <etichetta>`; didascalia ed etichetta facoltative. */
export function generaFigura(percorso: string, didascalia: string, etichetta: string): string | null {
  const p = percorso.trim();
  const e = etichetta.trim();
  if (p === "" || /[\r\n]/.test(p) || (e !== "" && !etichettaValida(e))) {
    return null;
  }
  const d = didascalia.trim();
  const righe = [`  image(${stringaTypst(p)}),`];
  if (d !== "") {
    righe.push(`  caption: [${escapaContenuto(d)}],`);
  }
  return `#figure(\n${righe.join("\n")}\n)${e !== "" ? ` <${e}>` : ""}\n`;
}

/** Equazione in linea (`$x$`) o visualizzata (`$ x $`); rifiuta un `$` nel testo. */
export function generaEquazione(testo: string, visualizzata: boolean): string | null {
  const t = testo.trim();
  if (t.includes("$") || /[\r\n]/.test(t)) {
    return null;
  }
  return visualizzata ? `$ ${t} $\n` : `$${t}$`;
}

/** Riferimento a un'etichetta: `@nome`. */
export function generaRiferimento(etichetta: string): string | null {
  return etichettaValida(etichetta) ? `@${etichetta}` : null;
}

/** `#footnote[testo]`; testo obbligatorio. */
export function generaNota(testo: string): string | null {
  const t = testo.trim();
  return t === "" ? null : `#footnote[${escapaContenuto(t)}]`;
}
