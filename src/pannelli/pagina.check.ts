/**
 * pagina.check.ts — Verifica di `pagina-logica.ts`. Esce con errore al primo controllo fallito.
 */
import {
  escapaContenuto,
  geometriaFoglio,
  leggiPagina,
  leggiPaginaDaTesto,
  lunghezzaInMm,
  normalizzaLunghezza,
  scriviPagina,
  trovaRegolaPagina,
  type StatoPagina,
} from "./pagina-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}
/** Applica lo stato al testo e restituisce il testo risultante (come farebbe l'editor). */
function applica(testo: string, stato: StatoPagina): string {
  const e = scriviPagina(testo, stato);
  vero(e !== null, "scriviPagina non null");
  return testo.slice(0, e!.cambio.da) + e!.cambio.testo + testo.slice(e!.cambio.a);
}

// lunghezze
uguali(normalizzaLunghezza("2,5 cm"), "2.5cm", "virgola e spazio");
uguali(normalizzaLunghezza("20mm"), "20mm", "mm");
uguali(normalizzaLunghezza(""), "", "vuota");
uguali(normalizzaLunghezza("2 metri"), null, "unità ignota");
uguali(normalizzaLunghezza("-2cm"), null, "negativa");
uguali(normalizzaLunghezza("2cm; drop"), null, "iniezione");
uguali(lunghezzaInMm("2.5cm"), 25, "cm in mm");
uguali(Math.round((lunghezzaInMm("1in") ?? 0) * 10) / 10, 25.4, "pollici");
uguali(lunghezzaInMm("abc"), null, "non lunghezza");

// ricerca della regola
vero(trovaRegolaPagina("= Titolo\n\nTesto") === null, "nessuna regola");
vero(trovaRegolaPagina('// #set page(paper: "a3")\n= T') === null, "regola in un commento");
vero(trovaRegolaPagina("```\n#set page(x)\n```") === null, "regola in un raw");
vero(trovaRegolaPagina("testo #set page(paper: \"a4\")") === null, "non a inizio riga");
vero(trovaRegolaPagina("#set page(paper: ") === null, "sbilanciata");
const t1 = '#set text(lang: "it")\n#set page(paper: "a5", margin: (x: 2cm, y: 3cm), header: [Ciao, mondo])\n= T';
const r1 = trovaRegolaPagina(t1)!;
uguali(t1.slice(r1.da, r1.a).startsWith("#set page("), true, "inizio regola");
uguali(t1.slice(r1.da, r1.a).endsWith("[Ciao, mondo])"), true, "fine regola");
uguali(r1.args.map((a) => a.nome), ["paper", "margin", "header"], "nomi degli argomenti (virgola nel contenuto ignorata)");
const rr = trovaRegolaPagina('  #set page(\n  paper: "a4", // commento, con virgola\n  numbering: "1",\n)')!;
uguali(rr.args.map((a) => a.nome), ["paper", "numbering"], "multi-riga con commento");
uguali(rr.rientro, "  ", "rientro");

// lettura
const s1 = leggiPagina(r1);
uguali([s1.formato, s1.margineX, s1.margineY, s1.intestazione], ["a5", "2cm", "3cm", "Ciao, mondo"], "lettura completa");
uguali(leggiPagina(null).formato, "a4", "senza regola: a4");
const adv = leggiPaginaDaTesto('#set page(margin: (top: 1cm, left: 2cm), header: [#h(1fr) x], numbering: "I", fill: red)');
vero(adv.margineAvanzato && adv.intestazioneAvanzata && adv.numerazioneAvanzata, "argomenti avanzati riconosciuti");
uguali(leggiPaginaDaTesto("#set page(margin: 2cm)").margineY, "2cm", "margine uniforme");
uguali(leggiPaginaDaTesto('#set page(paper: "marziano")').formato, "a4", "formato ignoto → a4");
uguali(leggiPaginaDaTesto("#set page(columns: 3, flipped: true)").colonne, 3, "colonne");
vero(leggiPaginaDaTesto("#set page(columns: 3, flipped: true)").orizzontale, "orizzontale");
uguali(leggiPaginaDaTesto("#set page(columns: 99)").colonne, 1, "colonne fuori range");

// scrittura: inserimento in cima
const base = leggiPagina(null);
vero(scriviPagina("= T", base) === null, "nulla da scrivere senza regola");
const nuovo = applica("= T", { ...base, formato: "a5", colonne: 2, numerazione: true });
uguali(nuovo, '#set page(\n  paper: "a5",\n  columns: 2,\n  numbering: "1",\n)\n\n= T', "inserimento in cima");
uguali(applica("= T", { ...base, orizzontale: true }), "#set page(flipped: true)\n\n= T", "una riga se ≤ 2 argomenti");

// scrittura: modifica che conserva il resto
const orig = '#set text(lang: "it")\n#set page(paper: "a4", fill: rgb("#eee"), margin: 2cm, footer: [Piè])\n= T';
const st = leggiPaginaDaTesto(orig);
const mod = applica(orig, { ...st, margineX: "3cm", margineY: "2cm", colonne: 2 });
vero(mod.includes('fill: rgb("#eee")') && mod.includes("footer: [Piè]"), "argomenti non gestiti conservati");
vero(mod.includes("margin: (x: 3cm, y: 2cm)"), `margine x/y: ${mod}`);
vero(mod.includes("columns: 2"), "colonne aggiunte");
vero(mod.startsWith('#set text(lang: "it")\n#set page('), "il resto del documento non cambia");
uguali(mod.endsWith("= T"), true, "coda intatta");
vero(mod.indexOf("paper") < mod.indexOf("fill") && mod.indexOf("fill") < mod.indexOf("margin"), "ordine originale mantenuto");

// rimozione di argomenti gestiti
const senza = applica(orig, { ...st, margineX: "", margineY: "" });
vero(!senza.includes("margin"), "margine rimosso");
vero(senza.includes("fill:") && senza.includes("footer:"), "altri argomenti restano");
const senzaCol = applica("#set page(columns: 2)", { ...leggiPaginaDaTesto("#set page(columns: 2)"), colonne: 1 });
uguali(senzaCol, "#set page()", "ultimo argomento rimosso: regola vuota ma valida");

// intestazione: escaping e roundtrip
uguali(escapaContenuto("a]b #x"), "a\\]b \\#x", "escaping contenuto");
const conInt = applica("= T", { ...base, intestazione: "Preventivo n. 12 [bozza] #1" });
uguali(leggiPaginaDaTesto(conInt).intestazione, "Preventivo n. 12 [bozza] #1", "intestazione round-trip con caratteri speciali");
// intestazione/margini avanzati restano com'erano anche se lo stato li ha
const avanz = '#set page(margin: (top: 1cm), header: [#h(1fr) X])';
const dopo = applica(avanz, { ...leggiPaginaDaTesto(avanz), colonne: 2 });
vero(dopo.includes("margin: (top: 1cm)") && dopo.includes("header: [#h(1fr) X]"), "avanzati intatti");

// selezione risultante
const e = scriviPagina(orig, { ...st, colonne: 2 })!;
const risultato = orig.slice(0, e.cambio.da) + e.cambio.testo + orig.slice(e.cambio.a);
vero(risultato.slice(e.selezione.da, e.selezione.a).startsWith("#set page(") && risultato.slice(e.selezione.da, e.selezione.a).endsWith(")"), "selezione = regola intera");

// idempotenza: riapplicare lo stato letto non cambia il significato
const due = applica(mod, leggiPaginaDaTesto(mod));
uguali(leggiPaginaDaTesto(due), leggiPaginaDaTesto(mod), "idempotente");

// geometria
const g = geometriaFoglio({ ...base, margineX: "2.5cm", margineY: "2.5cm" });
vero(Math.abs(g.sinistra - (25 / 210) * 100) < 1e-9 && Math.abs(g.alto - (25 / 297) * 100) < 1e-9, "margini in percentuale");
uguali(geometriaFoglio({ ...base, orizzontale: true }).rapporto > 1, true, "orizzontale: più larga che alta");
uguali(geometriaFoglio({ ...base, margineX: "500cm" }).sinistra, 45, "margine enorme limitato");

console.log("OK pagina");
