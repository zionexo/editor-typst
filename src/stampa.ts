/**
 * stampa.ts — Parti pure della stampa del documento (nessun DOM, nessun Tauri).
 *
 * La stampa usa la finestra di stampa di WebView2 (`window.print()`) sulle pagine
 * SVG dell'anteprima, mostrate dal foglio di stile `stili/stampa.css`. Qui stanno le
 * regole CSS che danno a ogni pagina il suo formato carta (anche se il documento mescola
 * formati, con le «pagine con nome» di CSS) e l'attesa con scadenza; `stampa.check.ts`
 * le verifica.
 */

/** Dimensioni di una pagina del documento, in pt. */
export interface DimensioniPagina {
  larghezza: number;
  altezza: number;
}

/** Arrotonda a 2 decimali (0,01 pt ≈ 0,004 mm) e toglie gli zeri inutili. */
function pt(n: number): string {
  return String(Math.round(n * 100) / 100);
}

/** Sottrazione dall'altezza: tiene la pagina appena sotto il foglio, così gli arrotondamenti non generano una pagina bianca. */
const MARGINE_ALTEZZA_PT = 0.2;

/**
 * Regole CSS per stampare `pagine`: una `@page` per ogni formato distinto (la prima, senza
 * nome, è il formato della pagina 1; le altre sono pagine con nome `f1`, `f2`, …) e una regola
 * `@media print` per formato che assegna a ogni pagina del documento la sua pagina con nome e
 * la sua altezza in pt. `null` se manca una pagina o ci sono dimensioni non valide.
 */
export function regoleStampa(pagine: readonly DimensioniPagina[]): string[] | null {
  if (pagine.length === 0) {
    return null;
  }
  if (pagine.some((p) => !Number.isFinite(p.larghezza) || !Number.isFinite(p.altezza) || p.larghezza <= 0 || p.altezza <= 0)) {
    return null;
  }
  // Formati distinti, nell'ordine di comparsa, con gli indici delle pagine che li usano.
  const gruppi = new Map<string, { p: DimensioniPagina; indici: number[] }>();
  pagine.forEach((p, i) => {
    const chiave = `${pt(p.larghezza)}x${pt(p.altezza)}`;
    const g = gruppi.get(chiave);
    if (g === undefined) {
      gruppi.set(chiave, { p, indici: [i] });
    } else {
      g.indici.push(i);
    }
  });
  const regole: string[] = [];
  let k = 0;
  for (const g of gruppi.values()) {
    const nome = k === 0 ? "" : `f${k} `;
    regole.push(`@page ${nome}{ size: ${pt(g.p.larghezza)}pt ${pt(g.p.altezza)}pt; margin: 0; }`);
    const selettori = g.indici.map((i) => `.pagina-svg[data-pagina="${i}"]`).join(", ");
    const pagina = k === 0 ? "" : ` page: f${k};`;
    regole.push(`@media print { ${selettori} {${pagina} height: ${pt(g.p.altezza - MARGINE_ALTEZZA_PT)}pt; } }`);
    k++;
  }
  return regole;
}

/**
 * Attende che `condizione` diventi vera, controllandola ogni `passoMs`; `false` allo
 * scadere di `scadenzaMs`. `attendi` è iniettata per i test (default: `setTimeout`).
 */
export async function attendiCondizione(
  condizione: () => boolean,
  scadenzaMs: number,
  passoMs = 50,
  attendi: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms)),
): Promise<boolean> {
  let trascorso = 0;
  while (!condizione()) {
    if (trascorso >= scadenzaMs) {
      return false;
    }
    await attendi(passoMs);
    trascorso += passoMs;
  }
  return true;
}
