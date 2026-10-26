/**
 * quandoversione.check.ts — Verifica eseguibile di `quandoversione.ts`.
 * Stesso schema di `modalita.check.ts`; esce con errore al primo controllo fallito.
 */
import { formattaQuando } from "./quandoversione.ts";

function vero(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

const locale = "2026-09-29 23:30:05";
vero(formattaQuando({ quando: locale }, "it") === locale, "campo assente -> ripiego");
vero(formattaQuando({ quando: locale, quando_utc: null }, "it") === locale, "null -> ripiego");
vero(formattaQuando({ quando: locale, quando_utc: "" }, "it") === locale, "vuoto -> ripiego");
vero(formattaQuando({ quando: locale, quando_utc: "non una data" }, "it") === locale, "invalido -> ripiego");

const v = { quando: locale, quando_utc: "2026-09-29T21:30:05Z" };
const it = formattaQuando(v, "it");
const en = formattaQuando(v, "en-US");
vero(it !== locale && it.includes("2026"), `usa il campo UTC (${it})`);
// Oracolo indipendente da Intl: i getter di Date (fuso del sistema) devono
// ritrovarsi nel testo, e l'istante UTC (non il campo locale) deve determinare l'ora.
const d = new Date(v.quando_utc);
const due = (n: number): string => String(n).padStart(2, "0");
vero(
  it.includes(`${due(d.getHours())}:${due(d.getMinutes())}:${due(d.getSeconds())}`),
  `ora locale derivata dall'istante UTC (${it})`,
);
vero(it.includes(String(d.getFullYear())), `anno (${it})`);
vero(new RegExp(`(^|[^0-9])${d.getDate()}([^0-9]|$)`).test(it), `giorno ${d.getDate()} (${it})`);
// Un altro istante UTC deve cambiare l'output (il campo locale `quando` non conta).
vero(
  formattaQuando({ quando: locale, quando_utc: "2027-01-02T03:04:05Z" }, "it") !== it,
  "dipende dall'istante UTC",
);
vero(it !== en, `la lingua cambia il formato (${it} / ${en})`);
vero(formattaQuando(v, "!!non valida").length > 0, "lingua non valida: nessuna eccezione");
vero(formattaQuando(v, "") .includes("2026"), "lingua vuota: predefinita");

console.log("quandoversione.check.ts: OK");
