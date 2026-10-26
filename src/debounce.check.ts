/**
 * debounce.check.ts — Verifica eseguibile di `debounce.ts` (F5-010).
 * Stesso schema di `comandi.check.ts`; esce con errore al primo controllo fallito.
 */
import { RITARDO_MIN_ASSOLUTO, ritardoDebounce } from "./debounce.ts";

let n = 0;
function uguale(atteso: number, reale: number, msg: string): void {
  n++;
  if (atteso !== reale) {
    throw new Error(`FALLITO: ${msg} (atteso ${atteso}, ottenuto ${reale})`);
  }
}

// Senza misura vale la base.
uguale(300, ritardoDebounce(300, null), "senza misura = base");
// Compilazione leggera: mezza base + durata (limite basso 150 con base 300).
uguale(150, ritardoDebounce(300, 0), "durata 0 = pavimento 150");
uguale(153, ritardoDebounce(300, 3), "documento leggero");
// Documento pesante: cresce con la durata, senza superare il tetto (600).
uguale(270, ritardoDebounce(300, 120), "documento da 120 ms");
uguale(600, ritardoDebounce(300, 450), "tetto raggiunto");
uguale(600, ritardoDebounce(300, 5000), "tetto: mai oltre 2*base");
// Monotonia: più lenta la compilazione, mai un ritardo minore.
let prec = 0;
for (let d = 0; d <= 1000; d += 10) {
  const r = ritardoDebounce(300, d);
  if (r < prec) {
    throw new Error(`FALLITO: non monotono a durata ${d}`);
  }
  prec = r;
  n++;
}
// La base configurata scala i limiti.
uguale(500, ritardoDebounce(1000, 0), "base 1000: pavimento 500");
uguale(2000, ritardoDebounce(1000, 9999), "base 1000: tetto 2000");
// Base minima del core (50): pavimento assoluto 50, tetto 100.
uguale(RITARDO_MIN_ASSOLUTO, ritardoDebounce(50, 0), "base 50: pavimento 50");
uguale(100, ritardoDebounce(50, 500), "base 50: tetto 100");
// Input anomali: nessun NaN verso setTimeout.
uguale(300, ritardoDebounce(Number.NaN, null), "base NaN, senza misura");
uguale(300, ritardoDebounce(300, Number.NaN), "durata NaN = senza misura");
uguale(150, ritardoDebounce(300, -20), "durata negativa = 0");

console.log(`debounce.check.ts: ${n} controlli OK`);
