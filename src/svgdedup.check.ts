/**
 * svgdedup.check.ts — Verifica eseguibile di `svgdedup.ts` (F5-009).
 * Stesso schema di `comandi.check.ts`; esce con errore al primo controllo fallito.
 */
import { separaDefs } from "./svgdedup.ts";

// Il progetto non ha `@types/node`: si tipizza a mano il solo pezzo che serve
// (`process.getBuiltinModule`, Node >= 22.3) per leggere la fixture.
const { readFileSync } = (
  globalThis as unknown as {
    process: { getBuiltinModule(nome: "fs"): { readFileSync(url: URL, codifica: "utf8"): string } };
  }
).process.getBuiltinModule("fs");

let n = 0;
function ok(cond: boolean, msg: string): void {
  n++;
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

/** Pagina sintetica nella forma di `typst-svg`: contenuto + `<defs>` in coda. */
function pagina(defs: string): string {
  return (
    '<svg viewBox="0 0 10 10" width="10pt" height="10pt" xmlns="http://www.w3.org/2000/svg">' +
    '<g><use xlink:href="#gAA" x="0" y="0"/></g>' +
    `<defs>${defs}</defs></svg>`
  );
}
const simboloA = '<symbol id="gAA" overflow="visible"><path d="M 0 0 L 1 1"/></symbol>';
const simboloB = '<symbol id="gBB" overflow="visible"><path d="M 2 2 L 3 3"/></symbol>';
const clip = '<clipPath id="cCC"><path d="M 0 0 h 5 v 5 Z"/></clipPath>';
const grad = '<linearGradient id="gDD"><stop offset="0" stop-color="#fff"/></linearGradient>';

// Prima pagina: nessun id visto, tutte le definizioni sono nuove, il blocco <defs> sparisce.
const visti = new Set<string>();
const p1 = separaDefs(pagina(simboloA + simboloB + clip + grad), visti);
if (p1 === null) {
  throw new Error("FALLITO: pagina non riconosciuta");
}
ok(p1.idNuovi.join(",") === "gAA,gBB,cCC,gDD", "id nuovi in ordine");
ok(p1.nuove === simboloA + simboloB + clip + grad, "markup nuovo identico all'originale");
ok(!p1.svg.includes("<defs"), "defs tolto dalla pagina");
ok(p1.svg.includes('xlink:href="#gAA"'), "i riferimenti restano");
ok(p1.svg.endsWith("</g></svg>"), "SVG ancora ben chiuso");

// Seconda pagina: gAA e cCC già visti, resta solo il nuovo.
p1.idNuovi.forEach((id) => visti.add(id));
const gEE = '<symbol id="gEE" overflow="visible"><path d="M 9 9"/></symbol>';
const p2 = separaDefs(pagina(simboloA + gEE + clip), visti);
ok(p2 !== null && p2.idNuovi.join(",") === "gEE", "solo la definizione non ancora vista");
ok(p2 !== null && p2.nuove === gEE, "markup nuovo = solo gEE");

// Tutte già viste: nulla da aggiungere.
const p3 = separaDefs(pagina(simboloA + clip), visti);
ok(p3 !== null && p3.nuove === "" && p3.idNuovi.length === 0, "nulla di nuovo");

// Id duplicato dentro la stessa pagina: aggiunto una sola volta.
const gFF = simboloB.replace("gBB", "gFF");
const p4 = separaDefs(pagina(gFF + gFF), visti);
ok(p4 !== null && p4.idNuovi.length === 1, "duplicato interno scartato");

// Forme inattese: nessun dedup (pagina intatta), mai un SVG corrotto.
ok(separaDefs("<svg><g/></svg>", visti) === null, "senza defs finale");
ok(separaDefs(pagina('<filter id="x"/>'), visti) === null, "elemento defs non riconosciuto");
ok(separaDefs(pagina(simboloA + "<junk/>"), visti) === null, "resto non riconosciuto");
ok(separaDefs(pagina(""), visti) !== null, "defs vuoto è valido");

// Round trip: la pagina senza defs + i defs nuovi ricostruiscono l'originale.
const orig = pagina(simboloA + clip);
const r = separaDefs(orig, new Set());
ok(
  r !== null && r.svg.replace("</svg>", `<defs>${r.nuove}</defs></svg>`) === orig,
  "round trip senza visti",
);

// Più blocchi <defs> consecutivi (typst-svg ne scrive fino a sette, uno per categoria).
const multi =
  '<svg viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><g><use xlink:href="#gAA"/></g>' +
  `<defs>${simboloA}</defs><defs>${clip}</defs><defs>${grad}</defs></svg>`;
const m1 = separaDefs(multi, new Set());
ok(m1 !== null && m1.idNuovi.join(",") === "gAA,cCC,gDD", "tutti i blocchi defs finali processati");
ok(m1 !== null && !m1.svg.includes("<defs"), "nessun defs residuo nella pagina");
ok(m1 !== null && m1.svg.endsWith("</g></svg>"), "SVG ben chiuso dopo blocchi multipli");
const m2 = separaDefs(multi, new Set(["cCC"]));
ok(m2 !== null && m2.idNuovi.join(",") === "gAA,gDD", "id già visto in un blocco intermedio scartato");

// Elemento self-closing seguito da uno pieno: il primo non deve inghiottire il secondo.
const rif = '<linearGradient id="rA" href="#fB"/>';
const pieno = '<linearGradient id="fB"><stop offset="0"/></linearGradient>';
const sc = separaDefs(pagina(rif + pieno), new Set());
ok(sc !== null && sc.idNuovi.join(",") === "rA,fB", "self-closing + elemento pieno: due id distinti");
ok(sc !== null && sc.nuove === rif + pieno, "markup self-closing e pieno conservato");
const sc2 = separaDefs(pagina(rif + pieno), new Set(["rA"]));
ok(sc2 !== null && sc2.nuove === pieno, "self-closing già visto scartato, il pieno resta");

// Pagina REALE di typst-svg 0.15 (testo, gradienti lineare/radiale, clip, tiling, immagine SVG),
// generata da `tools/misura.ps1` con MISURA_DUMP (cartella `fixture/`): tutti i blocchi
// devono essere riconosciuti e nessuna definizione deve andare persa.
const reale = readFileSync(new URL("./fixtures/pagina-ricca.svg", import.meta.url), "utf8");
const dr = separaDefs(reale, new Set());
ok(dr !== null, "pagina reale riconosciuta (zero pagine non riconosciute)");
if (dr !== null) {
  const blocchi = reale.split("<defs>").length - 1;
  ok(blocchi >= 4, `la pagina reale ha più blocchi defs (${blocchi})`);
  ok(!dr.svg.includes("<defs"), "pagina reale senza defs residui");
  ok(dr.idNuovi.length === new Set(dr.idNuovi).size, "id nuovi senza duplicati");
  // Nessun id perso: gli id nel testo originale = id estratti.
  const idOriginali = [...reale.matchAll(/\sid="([^"]+)"/g)].map((x) => x[1]);
  ok(
    idOriginali.length === dr.idNuovi.length && idOriginali.every((id) => dr.idNuovi.includes(id)),
    "tutte le definizioni della pagina reale estratte",
  );
  // Seconda inserzione della stessa pagina: nulla di nuovo.
  const di = separaDefs(reale, new Set(dr.idNuovi));
  ok(di !== null && di.nuove === "", "stessa pagina reale già vista: nessuna definizione nuova");
}

console.log(`svgdedup.check.ts: ${n} controlli OK`);
