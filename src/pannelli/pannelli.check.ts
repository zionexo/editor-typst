/**
 * pannelli.check.ts — Verifica eseguibile della logica pura della barra laterale
 * (F5-019/020/021): tastiera, indice, albero dei file, larghezza della barra.
 * Stesso schema di `comandi.check.ts`: nessuna dipendenza, esce con errore al
 * primo controllo fallito.
 *
 *   node --experimental-strip-types src/pannelli/pannelli.check.ts
 */
import { analizzaScorciatoia, canonica, corrisponde, type TastoEvento } from "../comandi.ts";
import { LARGHEZZA_MAX, LARGHEZZA_MIN, limitaLarghezza } from "../barra-laterale.ts";
import {
  cartelleAntenate,
  cartelleDaAprire,
  eIlDocumento,
  percorsoRelativo,
  righeVisibili,
  type NodoFile,
} from "./file-logica.ts";
import {
  destinazione,
  normalizzaTitolo,
  offsetNelDocumento,
  offsetPerMarkdown,
  posizioniFratelli,
  titoliMarkdown,
  voceCorrente,
  type VoceIndice,
} from "./indice-logica.ts";
import { naviga, navigaAlbero, type ElementoAlbero } from "./tastiera.ts";

let n = 0;
function ok(cond: boolean, msg: string): void {
  n++;
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguale<T>(atteso: T, reale: T, msg: string): void {
  ok(
    JSON.stringify(atteso) === JSON.stringify(reale),
    `${msg} (atteso ${JSON.stringify(atteso)}, ottenuto ${JSON.stringify(reale)})`,
  );
}

// ---- Scorciatoie dei nuovi comandi: corrispondono e non collidono con quelle esistenti ----
const tasto = (p: Partial<TastoEvento>): TastoEvento => ({
  key: "",
  code: "",
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  ...p,
});
// Combinazioni di CodeMirror (basicSetup: commands, search, lint, fold, history, completion)
// e riservate (Mod+Shift+F = ricerca nel progetto, F7-003; Mod+B = grassetto): le nostre non
// devono coincidere. Solo Mod+Shift+lettera, senza Alt (Ctrl+Alt = AltGr su tastiera italiana).
const RISERVATE = [
  "Mod+F", "Mod+G", "Mod+Shift+G", "Mod+D", "Mod+I", "Mod+U", "Mod+Y", "Mod+Z", "Mod+Shift+Z",
  "Mod+A", "Mod+/", "Mod+[", "Mod+]", "Mod+Shift+L", "Mod+Shift+M", "Mod+Shift+K", "Mod+Shift+U",
  "Mod+Shift+F", "Mod+B", "F8", "F3",
].map((r) => canonica(analizzaScorciatoia(r)));
const NOSTRE: [string, string, string, string][] = [
  // [scorciatoia, key, code, descrizione]
  ["Mod+Shift+B", "B", "KeyB", "barra laterale"],
  ["Mod+Shift+D", "D", "KeyD", "modalità focus"],
  ["Mod+Shift+T", "T", "KeyT", "macchina da scrivere"],
  ["Mod+Shift+E", "E", "KeyE", "pannello file"],
];
for (const [sc, key, code, desc] of NOSTRE) {
  const a = analizzaScorciatoia(sc);
  ok(!a.alt, `${desc}: senza Alt`);
  ok(a.mod, `${desc}: usa Mod (Ctrl su Windows/Linux, Cmd su macOS)`);
  ok(!RISERVATE.includes(canonica(a)), `${desc}: non coincide con CodeMirror né con le riservate`);
  ok(corrisponde(a, tasto({ key, code, ctrlKey: true, shiftKey: true }), false), `Ctrl+Shift+${key} ${desc}`);
  ok(corrisponde(a, tasto({ key, code, metaKey: true, shiftKey: true }), true), `Cmd+Shift+${key} ${desc} (macOS)`);
  ok(!corrisponde(a, tasto({ key, code, ctrlKey: true, shiftKey: true }), true), `Ctrl+Shift+${key} non agisce su macOS`);
}
ok(corrisponde(analizzaScorciatoia("F9"), tasto({ key: "F9", code: "F9" }), false), "F9 macchina (alternativa)");
ok(
  !corrisponde(analizzaScorciatoia("Mod+Shift+B"), tasto({ key: "b", code: "KeyB", ctrlKey: true }), false),
  "Ctrl+B (grassetto) non è la barra laterale",
);
ok(
  !corrisponde(analizzaScorciatoia("Mod+Shift+D"), tasto({ key: "F", code: "KeyF", ctrlKey: true, shiftKey: true }), false),
  "Ctrl+Shift+F non è la modalità focus (riservata alla ricerca nel progetto)",
);
ok(
  corrisponde(analizzaScorciatoia("Mod+Shift+E"), tasto({ key: "E", code: "KeyE", ctrlKey: true, shiftKey: true }), false),
  "Ctrl+Shift+E file",
);
ok(
  !corrisponde(analizzaScorciatoia("Mod+E"), tasto({ key: "E", code: "KeyE", ctrlKey: true, shiftKey: true }), false),
  "Ctrl+Shift+E non è l'export (Ctrl+E)",
);

// ---- Tastiera: liste ----
uguale(1, naviga(3, 0, "ArrowDown"), "giù");
uguale(0, naviga(3, 0, "ArrowUp"), "su al bordo resta");
uguale(2, naviga(3, 2, "ArrowDown"), "giù al bordo resta");
uguale(2, naviga(3, 0, "End"), "End");
uguale(0, naviga(3, 2, "Home"), "Home");
uguale(null, naviga(0, 0, "ArrowDown"), "lista vuota");
uguale(null, naviga(3, 0, "a"), "tasto estraneo");
uguale(1, naviga(3, 0, "ArrowRight", false), "orizzontale: destra");
uguale(null, naviga(3, 0, "ArrowDown", false), "orizzontale ignora giù");
uguale(0, naviga(3, -5, "ArrowUp"), "indice negativo riportato nei limiti");

// ---- Tastiera: albero ----
const albero: ElementoAlbero[] = [
  { livello: 0, eCartella: true, aperta: true }, // 0 cartella aperta
  { livello: 1, eCartella: false, aperta: false }, // 1 file
  { livello: 1, eCartella: true, aperta: false }, // 2 cartella chiusa
  { livello: 0, eCartella: false, aperta: false }, // 3 file radice
];
uguale({ tipo: "vai", indice: 1 }, navigaAlbero(albero, 0, "ArrowDown"), "albero: giù");
uguale({ tipo: "vai", indice: 3 }, navigaAlbero(albero, 0, "End"), "albero: End");
uguale({ tipo: "vai", indice: 1 }, navigaAlbero(albero, 0, "ArrowRight"), "destra su cartella aperta: primo figlio");
uguale({ tipo: "espandi", indice: 2 }, navigaAlbero(albero, 2, "ArrowRight"), "destra su cartella chiusa: espandi");
uguale(null, navigaAlbero(albero, 1, "ArrowRight"), "destra su file: nulla");
uguale({ tipo: "comprimi", indice: 0 }, navigaAlbero(albero, 0, "ArrowLeft"), "sinistra su cartella aperta: comprimi");
uguale({ tipo: "vai", indice: 0 }, navigaAlbero(albero, 1, "ArrowLeft"), "sinistra su file: padre");
uguale({ tipo: "vai", indice: 0 }, navigaAlbero(albero, 2, "ArrowLeft"), "sinistra su cartella chiusa: padre");
uguale(null, navigaAlbero(albero, 3, "ArrowLeft"), "sinistra al livello 0: nulla");
uguale({ tipo: "attiva", indice: 1 }, navigaAlbero(albero, 1, "Enter"), "Invio");
uguale({ tipo: "attiva", indice: 2 }, navigaAlbero(albero, 2, " "), "Spazio");
uguale(null, navigaAlbero(albero, 9, "Enter"), "indice fuori range");
uguale(null, navigaAlbero([], 0, "Enter"), "albero vuoto");
uguale(
  null,
  navigaAlbero([{ livello: 0, eCartella: true, aperta: true }], 0, "ArrowRight"),
  "cartella aperta senza figli visibili: nulla",
);

// ---- Larghezza della barra ----
uguale(LARGHEZZA_MIN, limitaLarghezza(10, 1600), "minimo");
uguale(LARGHEZZA_MAX, limitaLarghezza(5000, 4000), "massimo assoluto");
uguale(400, limitaLarghezza(400, 1600), "valore valido");
uguale(300, limitaLarghezza(500, 600), "al massimo metà finestra");
uguale(LARGHEZZA_MIN, limitaLarghezza(500, 200), "finestra piccola: mai sotto il minimo");
uguale(LARGHEZZA_MIN, limitaLarghezza(Number.NaN, 1600), "NaN");
uguale(LARGHEZZA_MAX, limitaLarghezza(9999, 0), "finestra ignota: solo il massimo assoluto");
uguale(301, limitaLarghezza(300.6, 1600), "intero");

// ---- Indice: voce corrente ----
const voce = (
  testo: string,
  profondita: number,
  offset: number | null,
  extra: Partial<VoceIndice> = {},
): VoceIndice => ({
  livello: profondita + 1,
  profondita,
  testo,
  principale: true,
  file: "main.typ",
  riga: 0,
  colonna: 0,
  offset,
  pagina: 1,
  ...extra,
});
uguale(-1, voceCorrente([10, 50, 90], 5), "cursore prima del primo titolo");
uguale(0, voceCorrente([10, 50, 90], 10), "sul primo titolo");
uguale(1, voceCorrente([10, 50, 90], 89), "fra secondo e terzo");
uguale(2, voceCorrente([10, 50, 90], 5000), "dopo l'ultimo");
uguale(0, voceCorrente([10, null, 50, null], 20), "le voci senza offset (altri file) sono saltate");
uguale(2, voceCorrente([10, null, 50, null], 60), "voce di un altro file in mezzo: si salta");
uguale(-1, voceCorrente([null, null], 100), "nessuna voce nel documento");
uguale(0, voceCorrente([50, 10], 60), "offset non ordinati (Markdown abbinato per testo): vince il più vicino al cursore");
uguale(-1, voceCorrente([], 0), "indice vuoto");

// ---- Indice: destinazioni ----
const inAltroFile = voce("Cap 2", 0, 40, { principale: false, file: "cap2.typ" });
uguale({ tipo: "cursore", offset: 33 }, destinazione(voce("A", 0, 33), 33), "titolo nel documento aperto");
uguale(
  { tipo: "apri-file", file: "cap2.typ", offset: 40 },
  destinazione(inAltroFile, null),
  "titolo in un altro file: apri il file",
);
uguale(
  { tipo: "pagina", pagina: 3 },
  destinazione(voce("Da pacchetto", 0, null, { principale: false, file: null, pagina: 3 }), null),
  "titolo non navigabile: solo la pagina",
);
uguale(
  { tipo: "pagina", pagina: 2 },
  destinazione(voce("Md non trovato", 0, 7, { pagina: 2 }), null),
  "principale senza offset nel documento (Markdown non abbinato): solo la pagina",
);

// ---- Indice: offset nel documento aperto ----
const vociTypst = [voce("A", 0, 0), inAltroFile, voce("B", 0, 99)];
uguale([0, null, 99], offsetNelDocumento(vociTypst, false, ""), "Typst: solo il file principale");

// ---- Indice: Markdown ----
uguale("ciao mondo", normalizzaTitolo("  Ciao   **mondo** "), "normalizza enfasi e spazi");
const md = [
  "# Uno",
  "",
  "```",
  "# non è un titolo",
  "```",
  "",
  "## Due *forte*",
  "Testo",
  "~~~python",
  "# neppure questo",
  "~~~",
  "## Due forte",
  "#hashtag non è un titolo",
  "### Tre ###",
  "",
].join("\n");
uguale(
  ["uno", "due forte", "due forte", "tre"],
  titoliMarkdown(md).map((t) => t.testo),
  "titoli ATX fuori dai blocchi di codice",
);
uguale(0, titoliMarkdown(md)[0].offset, "offset del primo titolo");
ok(md.slice(titoliMarkdown(md)[1].offset).startsWith("## Due *forte*"), "offset punta all'inizio della riga");
const vociMd = [voce("Uno", 0, 0), voce("Due forte", 1, 0), voce("Due forte", 1, 0), voce("Generato", 1, 0), voce("Tre", 2, 0)];
const offMd = offsetPerMarkdown(md, vociMd);
ok(md.slice(offMd[0] ?? -1).startsWith("# Uno"), "md: primo");
ok(md.slice(offMd[1] ?? -1).startsWith("## Due *forte*"), "md: primo dei due omonimi");
ok(md.slice(offMd[2] ?? -1).startsWith("## Due forte"), "md: secondo dei due omonimi");
uguale(null, offMd[3], "md: titolo generato dalla conversione, non abbinato");
ok(md.slice(offMd[4] ?? -1).startsWith("### Tre"), "md: titolo con # finali");
uguale(
  [0, null],
  offsetNelDocumento([voce("Uno", 0, 999), voce("Altro", 0, 5)], true, "# Uno\n"),
  "Markdown: gli offset del core (Typst generato) si ignorano",
);
uguale([], titoliMarkdown(""), "md vuoto");
uguale(["a"], titoliMarkdown("# a").map((t) => t.testo), "ultima riga senza a capo");
uguale(["a"], titoliMarkdown("# a\r\n\r\ntesto").map((t) => t.testo), "fine riga CRLF");

// ---- Indice: fratelli (aria-setsize/posinset) ----
uguale(
  [
    { dimensione: 2, posizione: 1 }, // A
    { dimensione: 2, posizione: 1 }, // A1
    { dimensione: 2, posizione: 2 }, // A2
    { dimensione: 2, posizione: 2 }, // B
    { dimensione: 1, posizione: 1 }, // B1
  ],
  posizioniFratelli([0, 1, 1, 0, 1].map((p) => ({ profondita: p }))),
  "fratelli: gruppi distinti sotto padri diversi",
);
uguale(
  [
    { dimensione: 2, posizione: 1 },
    { dimensione: 1, posizione: 1 },
    { dimensione: 2, posizione: 2 },
  ],
  posizioniFratelli([0, 1, 0].map((p) => ({ profondita: p }))),
  "fratelli: un livello più profondo in mezzo non spezza il gruppo",
);
uguale([], posizioniFratelli([]), "fratelli: vuoto");

// ---- File: righe visibili ----
const nodi: NodoFile[] = [
  {
    nome: "Cap",
    percorso: "Cap",
    tipo: "cartella",
    figli: [
      { nome: "a.typ", percorso: "Cap/a.typ", tipo: "documento", figli: [] },
      {
        nome: "Sub",
        percorso: "Cap/Sub",
        tipo: "cartella",
        figli: [{ nome: "b.md", percorso: "Cap/Sub/b.md", tipo: "documento", figli: [] }],
      },
    ],
  },
  { nome: "main.typ", percorso: "main.typ", tipo: "documento", figli: [] },
  { nome: "logo.png", percorso: "logo.png", tipo: "risorsa", figli: [] },
];
uguale(
  ["Cap", "main.typ", "logo.png"],
  righeVisibili(nodi, new Set()).map((r) => r.nodo.nome),
  "tutte chiuse: solo la radice",
);
uguale(
  [
    ["Cap", 0, true],
    ["a.typ", 1, false],
    ["Sub", 1, false],
    ["main.typ", 0, false],
    ["logo.png", 0, false],
  ],
  righeVisibili(nodi, new Set(["Cap"])).map((r) => [r.nodo.nome, r.livello, r.aperta]),
  "Cap aperta: figli visibili, Sub ancora chiusa",
);
uguale(
  ["Cap", "a.typ", "Sub", "b.md", "main.typ", "logo.png"],
  righeVisibili(nodi, new Set(["Cap", "Cap/Sub"])).map((r) => r.nodo.nome),
  "tutte aperte",
);
uguale(
  ["Cap", "main.typ", "logo.png"],
  righeVisibili(nodi, new Set(["Cap/Sub"])).map((r) => r.nodo.nome),
  "una cartella aperta dentro una chiusa non mostra nulla",
);
uguale(["a", "a/b"], cartelleAntenate("a/b/c.typ"), "antenati");
uguale([], cartelleAntenate("c.typ"), "nessun antenato");

// ---- File: percorsi (Windows con prefisso \\?\ e maiuscole) ----
const bs = String.fromCharCode(92);
const radiceWin = `${bs}${bs}?${bs}C:${bs}Progetti${bs}Tesi`;
uguale("cap/a.typ", percorsoRelativo(radiceWin, `C:${bs}progetti${bs}TESI${bs}Cap${bs}a.typ`), "relativo, ignora maiuscole e prefisso");
uguale(null, percorsoRelativo(radiceWin, `C:${bs}Progetti${bs}Tesi2${bs}a.typ`), "cartella con prefisso uguale non è sotto la radice");
uguale(null, percorsoRelativo(radiceWin, radiceWin), "la radice stessa non è un file");
uguale(null, percorsoRelativo("/a/b", "/x/y.typ"), "fuori dalla radice");
ok(eIlDocumento(radiceWin, "Cap/a.typ", `C:${bs}Progetti${bs}Tesi${bs}Cap${bs}a.typ`), "documento aperto riconosciuto");
ok(!eIlDocumento(radiceWin, "Cap/b.typ", `C:${bs}Progetti${bs}Tesi${bs}Cap${bs}a.typ`), "altro file non è il documento");
ok(!eIlDocumento(radiceWin, "Cap/a.typ", null), "nessun documento aperto");
uguale(
  ["Cap", "Cap/Sub"],
  cartelleDaAprire(nodi, "/p", "/p/cap/sub/b.md"),
  "cartelle da aprire per mostrare il documento (nomi reali, non normalizzati)",
);
uguale([], cartelleDaAprire(nodi, "/p", "/altro/x.typ"), "documento fuori dalla radice: nessuna");
uguale([], cartelleDaAprire(nodi, "/p", null), "nessun documento: nessuna");

console.log(`pannelli.check.ts: OK (${n} controlli)`);
