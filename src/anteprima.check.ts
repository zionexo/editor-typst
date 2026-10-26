/**
 * anteprima.check.ts — Verifica eseguibile della logica pura di `anteprima.ts`
 * (F5-008/F5-009). Stesso schema di `comandi.check.ts`.
 */
import { haBisognoDiSvg, type StatoPagina } from "./anteprima.ts";

let n = 0;
function ok(cond: boolean, msg: string): void {
  n++;
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

const base: StatoPagina = { atteso: "aa", caricato: null, visibile: true, inVolo: false };

// F5-008: una pagina mai caricata si richiede solo se in zona di precaricamento.
ok(haBisognoDiSvg(base), "visibile e mai caricata: serve");
ok(!haBisognoDiSvg({ ...base, visibile: false }), "fuori vista: non si carica");
// Hash invariato: nessuna nuova richiesta.
ok(!haBisognoDiSvg({ ...base, caricato: "aa" }), "hash uguale: niente richiesta");
// Hash cambiato: si richiede di nuovo (se visibile).
ok(haBisognoDiSvg({ ...base, caricato: "bb" }), "hash cambiato e visibile: serve");
ok(!haBisognoDiSvg({ ...base, caricato: "bb", visibile: false }), "hash cambiato ma fuori vista");
// Mai due richieste sulla stessa pagina.
ok(!haBisognoDiSvg({ ...base, inVolo: true }), "richiesta già in volo");

console.log(`anteprima.check.ts: ${n} controlli OK`);
