/**
 * recenti.ts — Elenco dei documenti aperti di recente (parte pura, senza DOM né Tauri).
 *
 * L'elenco vive in `Impostazioni.recenti` (percorsi assoluti, il più recente per primo).
 * Qui ci sono solo le regole: aggiunta, confronto dei percorsi e nomi da mostrare.
 */

/** Numero massimo di voci ricordate. */
export const MAX_RECENTI = 10;

/** Vero per un sorgente `.typ`/`.md` (un file); altrimenti il percorso è una cartella-progetto. */
export function eSorgente(percorso: string): boolean {
  return /\.(typ|md)$/i.test(percorso);
}

/** Forma con cui si confrontano due percorsi: Windows ignora maiuscole, barre e barra finale. */
function chiave(percorso: string): string {
  return percorso.replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
}

/** Nuovo elenco con `percorso` in testa, senza doppioni, al massimo `MAX_RECENTI` voci. */
export function aggiungiRecente(elenco: readonly string[], percorso: string): string[] {
  const k = chiave(percorso);
  return [percorso, ...elenco.filter((p) => chiave(p) !== k)].slice(0, MAX_RECENTI);
}

/** Ultimo segmento del percorso (nome del file o della cartella). */
export function nomeVoce(percorso: string): string {
  const parti = percorso.split(/[\\/]/).filter((s) => s !== "");
  return parti.length > 0 ? parti[parti.length - 1] : percorso;
}

/** Cartella che contiene la voce (percorso senza l'ultimo segmento); vuota se non c'è. */
export function cartellaVoce(percorso: string): string {
  const senzaFinale = percorso.replace(/[\\/]+$/, "");
  const i = Math.max(senzaFinale.lastIndexOf("\\"), senzaFinale.lastIndexOf("/"));
  return i > 0 ? senzaFinale.slice(0, i) : "";
}
