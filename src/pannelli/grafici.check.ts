/**
 * grafici.check.ts — Verifica di `grafici-logica.ts`. Esce con errore al primo controllo fallito.
 */
import { analizzaTabella, generaGrafico, RIGHE_MAX, SERIE_MAX, type OpzioniGrafico } from "./grafici-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}
const dati = (testo: string) => {
  const r = analizzaTabella(testo);
  vero("dati" in r, `analisi di ${JSON.stringify(testo)}: ${JSON.stringify(r)}`);
  return (r as { dati: NonNullable<ReturnType<typeof ok>> }).dati;
};
function ok(r: ReturnType<typeof analizzaTabella>) {
  return "dati" in r ? r.dati : null;
}
const errore = (testo: string): string => {
  const r = analizzaTabella(testo);
  vero("errore" in r, `atteso errore per ${JSON.stringify(testo)}`);
  return (r as { errore: string }).errore;
};

// separatori
uguali(dati("1\t2\n2\t4\n3\t3").serie, [[2, 4, 3]], "tab");
uguali(dati("1;2\n2;4").x, [1, 2], "punto e virgola");
uguali(dati("1 2 5\n2 4 6").serie, [[2, 4], [5, 6]], "spazi, due serie");
uguali(dati("1,2\n2,4\n3,3").serie, [[2, 4, 3]], "CSV con la virgola come separatore");
uguali(dati("1,5;2,5\n2,5;3,5").serie, [[2.5, 3.5]], "virgola decimale con ;");
uguali(dati("1,5 2,5\n2,5 3,5").x, [1.5, 2.5], "virgola decimale con spazi");
uguali(dati("1\t2\r\n2\t4\r\n").x, [1, 2], "a capo di Windows e riga finale vuota");

// intestazione
const conInt = dati("tempo;temperatura;umidità\n0;20;50\n1;22;48");
uguali(conInt.nomiColonne, ["tempo", "temperatura", "umidità"], "nomi dall'intestazione");
uguali(conInt.serie.length, 2, "due serie");
uguali(dati("1;2\n2;4").nomiColonne, ["X", "Serie 1"], "nomi generici senza intestazione");
uguali(dati(";y\n1;2\n2;4").nomiColonne, ["X", "y"], "X senza nome");

// errori
vero(errore("").includes("Incolla"), "vuota");
vero(errore("1\n2\n3").includes("due colonne"), "una colonna");
vero(errore("1;2\n3;4;5").includes("stesso numero"), "righe irregolari");
vero(errore("x;y\n1;2").includes("due righe"), "una sola riga di dati");
vero(/Riga 3, colonna 2/.test(errore("x;y\n1;2\n2;abc")), "cella non numerica con posizione (conta l'intestazione)");
vero(/Riga 2, colonna 1/.test(errore("1;2\nx;4")), "testo dopo la prima riga numerica è un errore con posizione");
vero(errore("1;2\n3;4\n5;").includes("numero"), "cella vuota");
vero(errore(Array.from({ length: SERIE_MAX + 2 }, (_, i) => i).join(";") + "\n" + Array.from({ length: SERIE_MAX + 2 }, (_, i) => i).join(";")).includes("serie"), "troppe serie");
vero(errore(Array.from({ length: RIGHE_MAX + 1 }, (_, i) => `${i};${i}`).join("\n")).includes("righe"), "troppe righe");
vero(errore("1;2\n2;inf").includes("numero"), "infinito non è un numero valido");

// generazione
const base: OpzioniGrafico = { tipo: "linee", titolo: "", etichettaX: "", etichettaY: "", legenda: false, didascalia: "", etichetta: "" };
const d = dati("tempo;a;b\n1;2;3\n2;4;1");
uguali(
  generaGrafico(d, base),
  "#lq.diagram(\n  lq.plot((1, 2), (2, 4)),\n  lq.plot((1, 2), (3, 1)),\n)\n",
  "diagramma minimo, due serie",
);
uguali(
  generaGrafico(d, { ...base, tipo: "punti", titolo: "Titolo [x]", etichettaX: "Tempo", etichettaY: "Valore", legenda: true }),
  "#lq.diagram(\n  title: [Titolo \\[x\\]],\n  xlabel: [Tempo],\n  ylabel: [Valore],\n  lq.scatter((1, 2), (2, 4), label: [a]),\n  lq.scatter((1, 2), (3, 1), label: [b]),\n)\n",
  "punti con titolo, assi e legenda",
);
uguali(generaGrafico(dati("1;2\n2;4"), { ...base, tipo: "barre" }), "#lq.diagram(\n  lq.bar((1, 2), (2, 4)),\n)\n", "barre");
uguali(
  generaGrafico(dati("1;2\n2;4"), { ...base, didascalia: "Una prova", etichetta: "fig:prova" }),
  "#figure(\n  lq.diagram(\n    lq.plot((1, 2), (2, 4)),\n  ),\n  caption: [Una prova],\n) <fig:prova>\n",
  "figura con didascalia ed etichetta",
);
uguali(
  generaGrafico(dati("1;2\n2;4"), { ...base, etichetta: "fig:solo" }),
  "#figure(\n  lq.diagram(\n    lq.plot((1, 2), (2, 4)),\n  ),\n) <fig:solo>\n",
  "figura solo con etichetta",
);
vero(generaGrafico(d, { ...base, etichetta: "1x" }) === null, "etichetta non valida");
vero(generaGrafico(dati("0,5;1,5\n1,5;2,5"), base)!.includes("(0.5, 1.5)"), "decimali con il punto nel codice");

console.log("OK grafici");
