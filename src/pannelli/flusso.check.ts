/**
 * flusso.check.ts — Verifica di `flusso-logica.ts` (scheda «Diagramma di flusso», pacchetto diagraph).
 * Esce con errore al primo controllo fallito.
 */
import { PACCHETTI, haImport, pianoInserimento, rigaImport } from "./equazioni-logica.ts";
import {
  ARCHI_MAX,
  NODI_MAX,
  OPZIONI_PREDEFINITE,
  TESTO_MAX,
  esempioFlusso,
  generaFlusso,
  rimuoviNodo,
  validaFlusso,
  type Arco,
  type Nodo,
  type OpzioniFlusso,
} from "./flusso-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

const n = (testo: string, extra: Partial<Nodo> = {}): Nodo => ({ testo, forma: "processo", colore: "", riquadro: "", livello: "", ...extra });
const a = (da: number, a: number, extra: Partial<Arco> = {}): Arco => ({ da, a, etichetta: "", tratteggiato: false, ...extra });
const opz: OpzioniFlusso = { ...OPZIONI_PREDEFINITE };
const codice = (nodi: Nodo[], archi: Arco[] = [], o: OpzioniFlusso = opz): string => {
  const r = generaFlusso(nodi, archi, o);
  vero("codice" in r, `generazione: ${JSON.stringify(r)}`);
  return (r as { codice: string }).codice;
};
const errore = (nodi: Nodo[], archi: Arco[] = [], o: OpzioniFlusso = opz): string => {
  const r = generaFlusso(nodi, archi, o);
  vero("errore" in r, `atteso un errore per ${JSON.stringify([nodi, archi, o])}`);
  return (r as { errore: string }).errore;
};

// Un nodo solo: forma predefinita, nessuna larghezza.
uguali(
  codice([n("Fai")]),
  '#diagraph.raw-render(\n  ```dot\n  digraph {\n    rankdir=TB;\n    n1 [label="Fai", shape=box];\n  }\n  ```,\n  math-mode: "text",\n)\n',
  "un nodo",
);

// Forme, colore, arco con etichetta e tratteggio, direzione e larghezza.
uguali(
  codice(
    [n("Valido?", { forma: "decisione", colore: "arancio" }), n("Esci", { forma: "inizio-fine" })],
    [a(0, 1, { etichetta: "No", tratteggiato: true }), a(1, 0)],
    { direzione: "LR", larghezza: 80 },
  ),
  [
    "#diagraph.raw-render(",
    "  ```dot",
    "  digraph {",
    "    rankdir=LR;",
    '    n1 [label="Valido?", shape=diamond, style=filled, fillcolor="#fed7aa"];',
    '    n2 [label="Esci", shape=ellipse];',
    '    n1 -> n2 [label="No", style=dashed];',
    "    n2 -> n1;",
    "  }",
    "  ```,",
    '  math-mode: "text",',
    "  width: 80%,",
    ")",
    "",
  ].join("\n"),
  "forme, colore, archi, direzione, larghezza",
);

// Livello comune: rank=same solo con almeno due nodi.
const livelli = codice([n("A", { livello: "x" }), n("B", { livello: "x" }), n("C", { livello: "y" })]);
vero(livelli.includes("{ rank=same; n1; n2; }"), "livello con due nodi");
vero(!livelli.includes("n3; }"), "livello con un nodo solo: nessun rank=same");

// Riquadro: i nodi sono dichiarati dentro il riquadro, non alla radice; un riquadro per titolo.
const riq = codice([n("A", { riquadro: "Fase 1" }), n("B", { riquadro: "Fase 1" }), n("C", { riquadro: "Fase 2" }), n("D")], [a(0, 3)]);
vero(riq.includes('subgraph cluster_1 {\n      label="Fase 1"; style=dashed;\n      n1 [label="A", shape=box];\n      n2 [label="B", shape=box];\n    }'), "primo riquadro");
vero(riq.includes('subgraph cluster_2 {\n      label="Fase 2"; style=dashed;\n      n3 [label="C", shape=box];'), "secondo riquadro");
vero(riq.includes('n4 [label="D"'), "nodo senza riquadro dichiarato");
vero(riq.indexOf('n4 [label="D"') < riq.indexOf("subgraph cluster_1"), "nodo senza riquadro alla radice, prima dei riquadri");
vero(riq.split('n1 [label="A"').length === 2, "nodo dichiarato una volta sola");
uguali(
  codice([n("A", { riquadro: "Fase 1 " }), n("B", { riquadro: "Fase 1" })]).split("subgraph").length,
  2,
  "i titoli uguali a meno degli spazi ai bordi fanno un riquadro solo",
);
vero(codice([n("A", { riquadro: "Fase 1" }), n("B", { riquadro: "fase 1" })]).includes("cluster_2"), "i titoli distinguono maiuscole e minuscole");

// Escape: virgolette e barre nel DOT (testo del nodo, titolo del riquadro, etichetta dell'arco).
vero(codice([n('Dì "ciao" \\ a')]).includes('label="Dì \\"ciao\\" \\\\ a"'), "escape di virgolette e barra");
vero(codice([n("A", { riquadro: 'Fase "A" \\ 1' })]).includes('label="Fase \\"A\\" \\\\ 1"; style=dashed;'), "escape nel titolo del riquadro");
vero(codice([n("A"), n("B")], [a(0, 1, { etichetta: 'a"b\\' })]).includes('n1 -> n2 [label="a\\"b\\\\"];'), "escape nell'etichetta dell'arco");
vero(codice([n("}; n2 -> n1 {")]).includes('label="}; n2 -> n1 {"'), "parentesi e punto e virgola restano nelle virgolette");

// Validazione.
vero(errore([]).includes("almeno un nodo"), "nessun nodo");
vero(errore([n("  ")]).includes("Nodo 1: manca il testo"), "testo vuoto");
vero(errore([n("a\nb")]).includes("andare a capo"), "testo con a capo");
vero(errore([n("a`b")]).includes("carattere `"), "backtick nel testo");
vero(errore([n("x".repeat(TESTO_MAX + 1))]).includes(`${TESTO_MAX} caratteri`), "testo troppo lungo");
vero(codice([n("x".repeat(TESTO_MAX))]).includes("x".repeat(TESTO_MAX)), "testo al tetto");
vero(errore([n("A", { riquadro: "R", livello: "L" })]).includes("riquadro e in un livello"), "riquadro e livello insieme");
for (const fineRiga of ["\v", "\f", "\u0085", "\u2028", "\u2029"]) {
  vero(errore([n(`a${fineRiga}b`)]).includes("andare a capo"), `fine riga Unicode U+${fineRiga.charCodeAt(0).toString(16)} nel testo`);
}
vero(errore([n("A", { riquadro: "R`" })]).includes("riquadro"), "backtick nel titolo del riquadro");
vero(errore([n("A"), n("B")], [a(0, 1, { etichetta: "a`b" })]).includes("Arco 1, etichetta"), "backtick nell'etichetta dell'arco");
vero(errore([n("A", { colore: "blu" })]).includes("colore non valido"), "colore sconosciuto");
vero(errore([{ ...n("A"), forma: "stella" as never }]).includes("forma non valida"), "forma sconosciuta");
vero(errore([n("A")], [a(0, 1)]).includes("Arco 1"), "arco verso un nodo che non c'è");
vero(errore([n("A")], [a(-1, 0)]).includes("Arco 1"), "arco da un indice negativo");
vero(errore([n("A")], [a(0, 0.5)]).includes("Arco 1"), "arco con indice non intero");
vero(errore([n("A")], [a(0, 0, { etichetta: "x\ny" })]).includes("Arco 1, etichetta"), "etichetta con a capo");
vero(codice([n("A")], [a(0, 0)]).includes("n1 -> n1;"), "un arco da un nodo a se stesso è ammesso");
vero(errore([n("A")], [], { direzione: "XX" as never, larghezza: null }).includes("Direzione"), "direzione sconosciuta");
vero(errore([n("A")], [], { direzione: "TB", larghezza: 9 }).includes("larghezza"), "larghezza sotto il minimo");
vero(errore([n("A")], [], { direzione: "TB", larghezza: 101 }).includes("larghezza"), "larghezza sopra il massimo");
vero(errore([n("A")], [], { direzione: "TB", larghezza: 50.5 }).includes("larghezza"), "larghezza non intera");
vero(errore([n("A")], [], { direzione: "TB", larghezza: Number.NaN }).includes("larghezza"), "larghezza non numerica (NaN)");
vero(codice([n("A")], [], { direzione: "TB", larghezza: 100 }).includes("width: 100%,"), "larghezza 100");
vero(errore(Array.from({ length: NODI_MAX + 1 }, () => n("A"))).includes(`${NODI_MAX} nodi`), "troppi nodi");
vero(errore([n("A")], Array.from({ length: ARCHI_MAX + 1 }, () => a(0, 0))).includes(`${ARCHI_MAX} archi`), "troppi archi");
vero(codice(Array.from({ length: NODI_MAX }, () => n("A"))).includes(`n${NODI_MAX} [label="A"`), "ai tetti dei nodi");
vero(codice([n("A")], Array.from({ length: ARCHI_MAX }, () => a(0, 0))).split("n1 -> n1;").length === ARCHI_MAX + 1, "ai tetti degli archi");
vero(validaFlusso([n("A")], [], opz) === null, "dati validi");

// Rimozione di un nodo: via gli archi che lo toccano, gli altri indici scalano.
const rim = rimuoviNodo([n("A"), n("B"), n("C")], [a(0, 1), a(1, 2), a(0, 2)], 1);
uguali(rim.nodi.map((x) => x.testo), ["A", "C"], "nodi dopo la rimozione");
uguali(rim.archi.map((x) => [x.da, x.a]), [[0, 1]], "archi dopo la rimozione");
uguali(rimuoviNodo([n("A")], [a(0, 0)], 0), { nodi: [], archi: [] }, "rimuovere l'unico nodo");
// Archi con `da` maggiore dell'indice rimosso: scalano anche loro, e conservano etichetta e tratteggio.
const rim2 = rimuoviNodo([n("A"), n("B"), n("C")], [a(2, 0, { etichetta: "x", tratteggiato: true }), a(0, 2)], 1);
uguali(rim2.archi, [{ da: 1, a: 0, etichetta: "x", tratteggiato: true }, { da: 0, a: 1, etichetta: "", tratteggiato: false }], "indici e attributi degli archi dopo la rimozione");

// Esempio: valido, con ciclo tratteggiato, tre rami sullo stesso livello e un join.
const es = esempioFlusso();
const codiceEsempio = codice(es.nodi, es.archi);
vero(codiceEsempio.includes("{ rank=same; n5; n6; n7; }"), "esempio: rami paralleli");
vero(codiceEsempio.includes('n4 -> n2 [label="correggi", style=dashed];'), "esempio: ciclo di ritorno");
vero(codiceEsempio.includes("n5 -> n8;") && codiceEsempio.includes("n6 -> n8;") && codiceEsempio.includes("n7 -> n8;"), "esempio: join");

// Pacchetto: import come modulo (nessun nome da importare) e aggiunto una sola volta.
uguali(rigaImport("diagraph"), `#import "@preview/diagraph:${PACCHETTI.diagraph.versione}"`, "import diagraph");
vero(haImport(`${rigaImport("diagraph")}\n`, "diagraph") && !haImport("= T", "diagraph"), "rilevamento dell'import");
vero(haImport(rigaImport("diagraph"), "diagraph"), "import come modulo a fine testo (senza a capo)");
for (const altro of ['#import "@preview/diagraph:0.3.8": *', '#import "@preview/diagraph:0.3.8": raw-render', '#import "@preview/diagraph:0.3.8" as dg']) {
  vero(!haImport(`${altro}\n`, "diagraph"), `import in un'altra forma (${altro}): serve comunque il modulo`);
}
uguali(pianoInserimento('#import "@preview/diagraph:0.3.8": *\n', { da: 0, a: 0 }, "x", ["diagraph"]).cambi.length, 2, "import `: *` presente: si aggiunge anche quello come modulo");
const piano = pianoInserimento("= T\n", { da: 4, a: 4 }, "#diagraph.raw-render()\n", ["diagraph"]);
uguali(piano.cambi.length, 2, "import e inserimento");
uguali(pianoInserimento(`${rigaImport("diagraph")}\n\n= T\n`, { da: 0, a: 0 }, "x", ["diagraph"]).cambi.length, 1, "import già presente: niente doppione");

console.log("OK flusso");
