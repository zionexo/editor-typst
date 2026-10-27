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

/** Esito di un generatore con messaggio d'errore specifico. */
export type EsitoFigura = { codice: string } | { errore: string };

/**
 * Percorso da scrivere in `image(...)`: tolte le virgolette di «Copia come percorso» di Esplora
 * risorse, barre normali; un percorso assoluto di Windows è accettato solo se sta dentro la cartella
 * del documento (diventa relativo), perché Typst legge soltanto i file del progetto.
 */
export function percorsoImmagine(percorso: string, cartellaDocumento: string | null): EsitoFigura {
  let p = percorso.trim();
  while (p.length >= 2 && /^(["'])[\s\S]*\1$/.test(p)) {
    p = p.slice(1, -1).trim();
  }
  if (p === "") {
    return { errore: "Indica il percorso dell'immagine" };
  }
  if (/[\r\n]/.test(p)) {
    return { errore: "Il percorso non può andare a capo" };
  }
  p = p.replace(/\\/g, "/");
  if (/^([A-Za-z]:\/|\/\/)/.test(p)) {
    const base = (cartellaDocumento ?? "").replace(/\\/g, "/").replace(/\/+$/, "");
    if (base !== "" && p.toLowerCase().startsWith(`${base.toLowerCase()}/`)) {
      return { codice: p.slice(base.length + 1) };
    }
    return {
      errore:
        "Typst legge solo i file della cartella del documento: copia l'immagine lì (o in una sua sottocartella) e scrivi il percorso relativo, es. img/foto.png",
    };
  }
  return { codice: p };
}

/** `#figure(image(...), caption: [...]) <etichetta>`, oppure il motivo per cui non si può generare. */
export function esitoFigura(
  percorso: string,
  didascalia: string,
  etichetta: string,
  cartellaDocumento: string | null = null,
): EsitoFigura {
  const p = percorsoImmagine(percorso, cartellaDocumento);
  if ("errore" in p) {
    return p;
  }
  const e = etichetta.trim();
  if (e !== "" && !etichettaValida(e)) {
    return { errore: "L'etichetta deve iniziare con una lettera e non può contenere spazi (es. fig:grafico)" };
  }
  const d = didascalia.trim();
  const righe = [`  image(${stringaTypst(p.codice)}),`];
  if (d !== "") {
    righe.push(`  caption: [${escapaContenuto(d)}],`);
  }
  return { codice: `#figure(\n${righe.join("\n")}\n)${e !== "" ? ` <${e}>` : ""}\n` };
}

/** `#figure(image(...), caption: [...]) <etichetta>`; didascalia ed etichetta facoltative. */
export function generaFigura(
  percorso: string,
  didascalia: string,
  etichetta: string,
  cartellaDocumento: string | null = null,
): string | null {
  const r = esitoFigura(percorso, didascalia, etichetta, cartellaDocumento);
  return "codice" in r ? r.codice : null;
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
