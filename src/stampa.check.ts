/**
 * stampa.check.ts — Verifica di `stampa.ts`. Esce con errore al primo controllo fallito.
 */
import { attendiCondizione, regoleStampa } from "./stampa.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

// regole di stampa: un solo formato
const A4 = { larghezza: 595.2756, altezza: 841.8898 };
uguali(
  regoleStampa([A4, A4, A4]),
  [
    "@page { size: 595.28pt 841.89pt; margin: 0; }",
    '@media print { .pagina-svg[data-pagina="0"], .pagina-svg[data-pagina="1"], .pagina-svg[data-pagina="2"] { height: 841.69pt; } }',
  ],
  "tre pagine A4: una @page e una regola con l'altezza appena sotto il foglio",
);
// formati misti: la pagina 1 è orizzontale (pagine con nome)
const ORIZZ = { larghezza: 841.8898, altezza: 595.2756 };
uguali(
  regoleStampa([A4, ORIZZ, A4, ORIZZ]),
  [
    "@page { size: 595.28pt 841.89pt; margin: 0; }",
    '@media print { .pagina-svg[data-pagina="0"], .pagina-svg[data-pagina="2"] { height: 841.69pt; } }',
    "@page f1 { size: 841.89pt 595.28pt; margin: 0; }",
    '@media print { .pagina-svg[data-pagina="1"], .pagina-svg[data-pagina="3"] { page: f1; height: 595.08pt; } }',
  ],
  "formati misti: pagina con nome per il secondo formato",
);
uguali(regoleStampa([{ larghezza: 612, altezza: 792 }])?.[0], "@page { size: 612pt 792pt; margin: 0; }", "Letter senza decimali inutili");
uguali(regoleStampa([]), null, "nessuna pagina");
uguali(regoleStampa([A4, { larghezza: 0, altezza: 100 }]), null, "una pagina con larghezza zero");
uguali(regoleStampa([{ larghezza: -1, altezza: 100 }]), null, "larghezza negativa");
uguali(regoleStampa([{ larghezza: Number.NaN, altezza: 100 }]), null, "NaN");
uguali(regoleStampa([{ larghezza: 100, altezza: Number.POSITIVE_INFINITY }]), null, "infinito");

// attesa con tempo simulato: nessuna attesa reale
{
  let t = 0;
  const finto = (ms: number): Promise<void> => {
    t += ms;
    return Promise.resolve();
  };
  const r1 = await attendiCondizione(() => t >= 200, 1000, 50, finto);
  vero(r1 === true && t === 200, `la condizione diventa vera a 200 ms (t=${t})`);
  t = 0;
  const r2 = await attendiCondizione(() => false, 300, 50, finto);
  vero(r2 === false && t === 300, `scadenza a 300 ms (t=${t})`);
  t = 0;
  const r3 = await attendiCondizione(() => true, 300, 50, finto);
  vero(r3 === true && t === 0, "già vera: nessuna attesa");
}

console.log("OK stampa");
