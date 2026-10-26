// Misura del dedup delle definizioni SVG (F5-009) su una cartella di pagine SVG.
//
// Uso (le pagine si generano con MISURA_DUMP, vedi tools/misura.ps1):
//   $env:MISURA_DUMP = "C:\percorso\svg"; tools\misura.ps1
//   node --experimental-strip-types tools/misura_dedup.mjs C:\percorso\svg
//
// Simula l'inserimento delle pagine nell'ordine, come `anteprima.ts`: byte totali
// prima/dopo (pagina senza <defs> + definizioni nuove) e tempo di `separaDefs`.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { separaDefs } from "../src/svgdedup.ts";

const cartella = process.argv[2];
if (!cartella) {
  console.error("uso: node --experimental-strip-types tools/misura_dedup.mjs <cartella-svg>");
  process.exit(2);
}
const file = readdirSync(cartella).filter((f) => f.endsWith(".svg")).sort();
const visti = new Set();
let prima = 0;
let dopo = 0;
let nonRiconosciute = 0;
const t0 = performance.now();
for (const f of file) {
  const svg = readFileSync(join(cartella, f), "utf8");
  prima += Buffer.byteLength(svg);
  const d = separaDefs(svg, visti);
  if (d === null) {
    nonRiconosciute++;
    dopo += Buffer.byteLength(svg);
    continue;
  }
  d.idNuovi.forEach((id) => visti.add(id));
  dopo += Buffer.byteLength(d.svg) + Buffer.byteLength(d.nuove);
}
const ms = performance.now() - t0;
console.log(`pagine = ${file.length}`);
console.log(`byte_prima = ${prima}`);
console.log(`byte_dopo = ${dopo} (${((100 * dopo) / prima).toFixed(1)}%)`);
console.log(`definizioni_uniche = ${visti.size}`);
console.log(`pagine_non_riconosciute = ${nonRiconosciute}`);
console.log(`separaDefs_totale_ms = ${ms.toFixed(1)} (${(ms / file.length).toFixed(2)} ms/pagina)`);
