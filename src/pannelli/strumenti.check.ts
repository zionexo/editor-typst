/**
 * strumenti.check.ts — Verifica di `strumenti-logica.ts`. Esce con errore al primo controllo fallito.
 */
import {
  abbrevia,
  applicaCambi,
  campiNelSorgente,
  contaOccorrenze,
  convertiInCampo,
  dataEstesa,
  eDataIso,
  eGiaCampo,
  sostituzioneMultiplaSensata,
  tipoInputData,
  filtraModelli,
  isoOggi,
  nomeCampoSuggerito,
  nomeCampoValido,
  riepilogoModello,
  valoriIniziali,
  valoriPerCore,
  type CampoModello,
  type ModelloInfo,
} from "./strumenti-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

// date
uguali(dataEstesa("2026-10-16"), "16 ottobre 2026", "data estesa");
uguali(dataEstesa("2026-01-05"), "5 gennaio 2026", "giorno senza zero");
uguali(dataEstesa("2026-02-30"), "", "data inesistente");
uguali(dataEstesa("16/10/2026"), "", "formato errato");
uguali(dataEstesa(""), "", "vuota");
uguali(isoOggi(new Date(2026, 0, 5)), "2026-01-05", "iso con zeri");

// valori
const campi: CampoModello[] = [
  { nome: "destinatario", tipo: "testo", predefinito: "Studio" },
  { nome: "data", tipo: "data", predefinito: "oggi" },
  { nome: "voci", tipo: "elenco", predefinito: null },
  { nome: "importo", tipo: "numero" },
];
const iniz = valoriIniziali(campi, "2026-10-16");
uguali(iniz, { destinatario: "Studio", data: "2026-10-16", voci: "", importo: "" }, "valori iniziali");
const core = valoriPerCore(campi, { destinatario: "  Rossi ", data: "2026-10-16", voci: "uno\ndue\n\n", importo: " 12,5 " });
uguali(core, { destinatario: "Rossi", data: "16 ottobre 2026", voci: "uno\ndue", importo: "12,5" }, "valori per il core");
uguali(valoriPerCore(campi, {}).data, "", "data mancante → vuota");

// riepilogo e ricerca
const m = (nome: string, descr: string, pagine: number, n: number): ModelloInfo => ({
  id: nome.toLowerCase(), nome, descrizione: descr, pagine,
  campi: Array.from({ length: n }, (_, i) => ({ nome: `c${i}`, tipo: "testo" as const })), origine: "integrato",
});
uguali(riepilogoModello(m("A", "", 1, 4)), "1 pagina · 4 campi", "riepilogo singolare/plurale");
uguali(riepilogoModello(m("A", "", 2, 1)), "2 pagine · 1 campo", "riepilogo plurale/singolare");
const elenco = [m("Lettera", "Lettera formale", 1, 4), m("Scheda prodotto", "Due pagine", 2, 7), m("Relazione", "Più è breve", 2, 5)];
uguali(filtraModelli(elenco, "").length, 3, "ricerca vuota");
uguali(filtraModelli(elenco, "SCHEDA").map((x) => x.nome), ["Scheda prodotto"], "ricerca per nome");
uguali(filtraModelli(elenco, "piu").map((x) => x.nome), ["Relazione"], "ricerca senza accenti");
uguali(filtraModelli(elenco, "zzz").length, 0, "nessun risultato");

// nomi
vero(nomeCampoValido("data") && nomeCampoValido("a_1") && !nomeCampoValido("1a") && !nomeCampoValido("A") && !nomeCampoValido("a-b") && !nomeCampoValido(""), "nome campo valido");
vero(!nomeCampoValido("x".repeat(41)) && nomeCampoValido("x".repeat(40)), "limite 40");
uguali(nomeCampoSuggerito("Studio Bianchi & Associati"), "studio_bianchi_associati", "nome suggerito");
uguali(nomeCampoSuggerito("16 ottobre"), "campo_16_ottobre", "nome che inizia con cifra");
uguali(nomeCampoSuggerito("€ $"), "campo", "nome senza lettere");
vero(nomeCampoValido(nomeCampoSuggerito("Àèì ôù ".repeat(10))), "suggerito sempre valido");

// conversione in campo
const t = "A: Studio\nOggetto: x\nCordiali, Studio";
const uno = convertiInCampo(t, 3, 9, "destinatario", false)!;
uguali(applicaCambi(t, uno.cambi), "A: {{destinatario}}\nOggetto: x\nCordiali, Studio", "una sola occorrenza");
uguali(uno.selezione, { da: 3, a: 3 + "{{destinatario}}".length }, "selezione singola");
const tutte = convertiInCampo(t, 3, 9, "destinatario", true)!;
uguali(applicaCambi(t, tutte.cambi), "A: {{destinatario}}\nOggetto: x\nCordiali, {{destinatario}}", "tutte le occorrenze");
uguali(tutte.cambi.length, 2, "numero di cambi");
uguali(tutte.cambi.map((c) => [c.da, c.a]), [[3, 9], [31, 37]], "intervalli sul testo originale");
// la selezione NON è la prima occorrenza: il primo cambio sta prima, la selezione segue il primo segnaposto
const dopo = convertiInCampo(t, 31, 37, "d", true)!;
uguali(applicaCambi(t, dopo.cambi), "A: {{d}}\nOggetto: x\nCordiali, {{d}}", "selezione sull'ultima occorrenza");
uguali(dopo.selezione, { da: 3, a: 3 + "{{d}}".length }, "selezione sul primo segnaposto");
// occorrenze sovrapposte: «aaa» con selezione «aa» in 1..3 → solo la selezione
const ov = convertiInCampo("aaa", 1, 3, "x", true)!;
uguali(applicaCambi("aaa", ov.cambi), "a{{x}}", "sovrapposte: solo la selezione");
vero(convertiInCampo(t, 4, 4, "x", false) === null, "selezione vuota");
vero(convertiInCampo("a   b", 1, 4, "x", false) === null, "frammento di soli spazi");
vero(convertiInCampo(t, 3, 9, "Nome Errato", false) === null, "nome non valido");
vero(convertiInCampo(t, 3, 999, "x", false) === null, "fuori dal testo");
vero(convertiInCampo("a {{x}} b", 2, 7, "y", false) === null, "frammento con segnaposto");
uguali(contaOccorrenze("aaaa", "aa"), 2, "occorrenze senza sovrapposizioni");
uguali(contaOccorrenze("abc", ""), 0, "frammento vuoto");
vero(eGiaCampo("{{data}}") && eGiaCampo(" {{!n_1}} ") && !eGiaCampo("{{Data}}") && !eGiaCampo("x {{a}}"), "già campo");
vero(sostituzioneMultiplaSensata("Studio") && !sostituzioneMultiplaSensata("it") && !sostituzioneMultiplaSensata("   "), "multipla sensata");
uguali(abbrevia("😀".repeat(50), 40), "😀".repeat(40) + "…", "abbreviazione senza spezzare surrogati");
uguali(abbrevia("breve", 40), "breve", "abbreviazione non necessaria");

// date non ISO
vero(eDataIso("2026-10-16") && !eDataIso("12 marzo 2026") && !eDataIso(""), "data ISO");
uguali(tipoInputData("2026-10-16"), "date", "input date per ISO");
uguali(tipoInputData(""), "date", "input date per vuoto");
uguali(tipoInputData("12 marzo 2026"), "text", "input text per predefinito libero");
uguali(valoriPerCore([{ nome: "q", tipo: "data" }], { q: "12 marzo 2026" }).q, "12 marzo 2026", "data libera passa com'è");

// campi nel sorgente
uguali(campiNelSorgente("{{a}} {{!b}} {{a}} {{ c }} {{D}} {{e_1}}"), ["a", "b", "e_1"], "campi nel sorgente");

console.log("OK strumenti");
