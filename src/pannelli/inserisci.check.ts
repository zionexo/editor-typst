/**
 * inserisci.check.ts — Verifica di `inserisci-logica.ts`. Esce con errore al primo controllo fallito.
 */
import {
  etichettaValida,
  etichetteNelTesto,
  generaEquazione,
  generaFigura,
  generaNota,
  generaRiferimento,
  generaTabella,
  stringaTypst,
} from "./inserisci-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

// tabella
uguali(generaTabella(2, 2, false), "#table(\n  columns: 2,\n  [], [],\n  [], [],\n)\n", "tabella 2x2 senza intestazione");
uguali(
  generaTabella(2, 2, true),
  "#table(\n  columns: 2,\n  table.header[Colonna 1][Colonna 2],\n  [], [],\n)\n",
  "intestazione conta come una riga",
);
uguali(generaTabella(1, 3, true), "#table(\n  columns: 3,\n  table.header[Colonna 1][Colonna 2][Colonna 3],\n)\n", "solo intestazione");
vero(generaTabella(0, 2, false) === null && generaTabella(2, 0, false) === null, "zero righe/colonne");
vero(generaTabella(51, 2, false) === null && generaTabella(2, 13, false) === null, "oltre i tetti");
vero(generaTabella(1.5, 2, false) === null && generaTabella(Number.NaN, 2, false) === null, "non intero");
vero(generaTabella(50, 12, true) !== null, "ai tetti");

// figura
uguali(generaFigura("img/a.png", "", ""), '#figure(\n  image("img/a.png"),\n)\n', "figura minima");
uguali(
  generaFigura("img\\a.png", "Una [prova] #1", "fig:a"),
  '#figure(\n  image("img/a.png"),\n  caption: [Una \\[prova\\] \\#1],\n) <fig:a>\n',
  "figura completa, barre normali e didascalia escapata",
);
vero(generaFigura("", "x", "") === null, "percorso obbligatorio");
vero(generaFigura("a.png", "", "1x") === null, "etichetta non valida");
vero(generaFigura("a\nb.png", "", "") === null, "percorso con a capo");
uguali(stringaTypst('a"b'), '"a\\"b"', "virgolette nel percorso");

// equazione
uguali(generaEquazione("x^2", false), "$x^2$", "in linea");
uguali(generaEquazione("x^2", true), "$ x^2 $\n", "visualizzata");
vero(generaEquazione("a $ b", false) === null, "dollaro nel testo");
vero(generaEquazione("a\nb", false) === null, "a capo nel testo");

// riferimento e nota
uguali(generaRiferimento("fig:a"), "@fig:a", "riferimento");
vero(generaRiferimento("a b") === null, "riferimento non valido");
uguali(generaNota(" Fonte: #x "), "#footnote[Fonte: \\#x]", "nota escapata");
vero(generaNota("  ") === null, "nota vuota");
vero(etichettaValida("a-b.c:d_1") && !etichettaValida("1a") && !etichettaValida(""), "etichette valide");
uguali(etichetteNelTesto("= T <intro>\n<a> <a> <b-1> x<2> <>"), ["intro", "a", "b-1"], "etichette nel testo");

console.log("OK inserisci");
