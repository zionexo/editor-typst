/**
 * caricatorepagine.check.ts — Verifica della macchina a stati di
 * `caricatorepagine.ts` (richieste, `stato[i] !== s`, `riprovaSeCambiato`,
 * reimpostazione delle definizioni). Esce con errore al primo controllo fallito.
 */
import type { PaginaSvg } from "./anteprima.ts";
import { creaCaricatorePagine, tuttePagineCaricate } from "./caricatorepagine.ts";

let n = 0;
function vero(c: boolean, msg: string): void {
  n++;
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
const microtask = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

interface Richiesta {
  indice: number;
  ok: (p: PaginaSvg | null) => void;
  ko: (e: unknown) => void;
}

function banco(limiteDefs = 5000) {
  const richieste: Richiesta[] = [];
  const dom: (string | null)[] = [];
  const errori: unknown[] = [];
  const caricate: number[] = [];
  let defs = 0;
  let svuotamentiDefs = 0;
  const c = creaCaricatorePagine({
    chiediSvg: (indice) =>
      new Promise<PaginaSvg | null>((ok, ko) => {
        richieste.push({ indice, ok, ko });
      }),
    inserisci: (i, p) => {
      dom[i] = p.hash;
      defs += 3;
    },
    svuotaPagina: (i) => {
      dom[i] = null;
    },
    svuotaDefs: () => {
      defs = 0;
      svuotamentiDefs++;
    },
    numeroDefs: () => defs,
    limiteDefs,
    onPaginaCaricata: (i) => caricate.push(i),
    onErrore: (e) => errori.push(e),
  });
  const aggiungi = (atteso: string, visibile = true): void => {
    c.stato.push({ atteso, caricato: null, visibile, inVolo: false });
  };
  return { c, richieste, dom, errori, caricate, aggiungi, defs: () => defs, svuotamenti: () => svuotamentiDefs };
}

// 1. Una sola richiesta per pagina; hash uguale: niente nuova richiesta.
{
  const b = banco();
  b.aggiungi("aa");
  b.c.carica(0);
  b.c.carica(0);
  vero(b.richieste.length === 1, "mai due richieste sulla stessa pagina");
  b.richieste[0].ok({ hash: "aa", svg: "<svg/>" });
  await microtask();
  vero(b.dom[0] === "aa" && b.caricate.length === 1, "SVG inserito e notificato");
  b.c.carica(0);
  vero(b.richieste.length === 1, "hash uguale: nessuna richiesta");
  b.c.stato[0].atteso = "bb";
  b.c.carica(0);
  vero(b.richieste.length === 2, "hash cambiato: nuova richiesta");
}

// 2. Atteso cambiato in volo: alla risposta si richiede di nuovo (anche dopo null ed errore).
for (const esito of ["svg", "null", "errore"] as const) {
  const b = banco();
  b.aggiungi("aa");
  b.c.carica(0);
  b.c.stato[0].atteso = "bb"; // `aggiorna` durante il volo
  const r = b.richieste[0];
  if (esito === "svg") {
    r.ok({ hash: "aa", svg: "" });
  } else if (esito === "null") {
    r.ok(null);
  } else {
    r.ko(new Error("IPC"));
  }
  await microtask();
  vero(b.richieste.length === 2, `atteso cambiato in volo (${esito}): si richiede di nuovo`);
  vero(esito !== "errore" || b.errori.length === 1, "errore segnalato");
}

// 3. Errore con atteso invariato: nessun ciclo di ritentativi.
{
  const b = banco();
  b.aggiungi("aa");
  b.c.carica(0);
  b.richieste[0].ko(new Error("IPC"));
  await microtask();
  vero(b.richieste.length === 1, "errore persistente: non si ritenta in ciclo");
  vero(!b.c.stato[0].inVolo, "inVolo azzerato");
}

// 4. Pagina rimossa/sostituita in volo: risposta ignorata.
{
  const b = banco();
  b.aggiungi("aa");
  b.c.carica(0);
  b.c.stato.pop(); // il documento si è accorciato
  b.richieste[0].ok({ hash: "aa", svg: "" });
  await microtask();
  vero(b.dom[0] === undefined && b.caricate.length === 0, "risposta di pagina rimossa ignorata");
  b.aggiungi("cc"); // nuova pagina allo stesso indice
  b.c.carica(0);
  vero(b.richieste.length === 2, "la nuova pagina allo stesso indice si carica");
}

// 5. Fuori vista: non si carica.
{
  const b = banco();
  b.aggiungi("aa", false);
  b.c.carica(0);
  vero(b.richieste.length === 0, "fuori vista: nessuna richiesta");
}

// 6. Oltre il limite delle definizioni: reimpostazione, le altre pagine si ricaricano.
{
  const b = banco(5);
  b.aggiungi("p0");
  b.aggiungi("p1");
  b.c.carica(0);
  b.c.carica(1);
  b.richieste[0].ok({ hash: "p0", svg: "" });
  await microtask();
  b.richieste[1].ok({ hash: "p1", svg: "" });
  await microtask();
  vero(b.defs() === 6 && b.svuotamenti() === 0, "al momento dell'inserimento le definizioni erano sotto il limite");
  b.c.stato[0].atteso = "p0b";
  b.c.carica(0);
  b.richieste[b.richieste.length - 1].ok({ hash: "p0b", svg: "" });
  await microtask();
  vero(b.svuotamenti() === 1, "oltre il limite: contenitore svuotato");
  vero(b.dom[0] === "p0b", "la pagina corrente è reinserita");
  vero(b.richieste[b.richieste.length - 1].indice === 1, "l'altra pagina si ricarica");
}

// tuttePagineCaricate: serve alla stampa
{
  const p = (atteso: string, caricato: string | null) => ({ atteso, caricato, visibile: false, inVolo: false });
  vero(!tuttePagineCaricate([]), "nessuna pagina: niente da stampare");
  vero(tuttePagineCaricate([p("a", "a"), p("b", "b")]), "tutte caricate");
  vero(!tuttePagineCaricate([p("a", "a"), p("b", null)]), "una pagina mai caricata");
  vero(!tuttePagineCaricate([p("a", "a"), p("b2", "b")]), "una pagina con hash stantio");
}

console.log(`caricatorepagine.check.ts: ${n} controlli OK`);
