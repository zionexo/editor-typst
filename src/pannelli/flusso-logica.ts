/**
 * flusso-logica.ts — Logica pura della scheda «Diagramma di flusso», basata sul pacchetto
 * `diagraph` (Typst Universe), che disegna con Graphviz: la disposizione, le intersezioni e i rami
 * paralleli li decide Graphviz. Nodi e archi → blocco `dot` dentro `#diagraph.raw-render(...)`.
 * Nessun DOM, nessun Tauri: verificata da `flusso.check.ts`.
 *
 * Il codice generato resta modificabile a mano (è DOT in chiaro). Limite noto di Graphviz: un nodo
 * non può stare insieme in un riquadro (`cluster`) e in un livello comune (`rank=same`): il riquadro
 * non si disegna. La validazione lo impedisce.
 */

export const NODI_MAX = 80;
export const ARCHI_MAX = 200;
/** Lunghezza massima di testi, titoli ed etichette. */
export const TESTO_MAX = 80;

/** Forma di un nodo, secondo la simbologia dei diagrammi di flusso. */
export type Forma = "processo" | "inizio-fine" | "decisione" | "io" | "sottoprocesso";

export const FORME: readonly { id: Forma; etichetta: string; attributi: string }[] = [
  { id: "processo", etichetta: "Processo", attributi: "shape=box" },
  { id: "inizio-fine", etichetta: "Inizio / fine", attributi: "shape=ellipse" },
  { id: "decisione", etichetta: "Decisione", attributi: "shape=diamond" },
  { id: "io", etichetta: "Input / output", attributi: "shape=parallelogram" },
  { id: "sottoprocesso", etichetta: "Sottoprocesso", attributi: "shape=box, peripheries=2" },
];

/** Colori di riempimento (tinte chiare, leggibili con testo nero). `id` vuoto = nessun riempimento. */
export const COLORI: readonly { id: string; etichetta: string; esadecimale: string }[] = [
  { id: "azzurro", etichetta: "Azzurro", esadecimale: "#dbeafe" },
  { id: "verde", etichetta: "Verde", esadecimale: "#dcfce7" },
  { id: "giallo", etichetta: "Giallo", esadecimale: "#fef9c3" },
  { id: "arancio", etichetta: "Arancio", esadecimale: "#fed7aa" },
  { id: "rosso", etichetta: "Rosso", esadecimale: "#fecaca" },
  { id: "viola", etichetta: "Viola", esadecimale: "#e9d5ff" },
  { id: "grigio", etichetta: "Grigio", esadecimale: "#e5e7eb" },
];

/** Direzione del flusso (`rankdir` di Graphviz). */
export type Direzione = "TB" | "LR";

export const DIREZIONI: readonly { id: Direzione; etichetta: string }[] = [
  { id: "TB", etichetta: "Dall'alto in basso" },
  { id: "LR", etichetta: "Da sinistra a destra" },
];

/** Un nodo (riga della tabella dei nodi). */
export interface Nodo {
  testo: string;
  forma: Forma;
  /** Id di una voce di `COLORI`, oppure vuoto. */
  colore: string;
  /** Titolo del riquadro che contiene il nodo (nodi con lo stesso titolo = stesso riquadro), o vuoto. */
  riquadro: string;
  /** Nodi con lo stesso valore stanno sulla stessa riga (o colonna, con direzione orizzontale), o vuoto. */
  livello: string;
}

/** Un arco: freccia dal nodo `da` al nodo `a` (indici 0-based nell'elenco dei nodi). */
export interface Arco {
  da: number;
  a: number;
  etichetta: string;
  tratteggiato: boolean;
}

export interface OpzioniFlusso {
  direzione: Direzione;
  /** Larghezza in % della pagina (10-100); `null` = dimensione naturale del diagramma. */
  larghezza: number | null;
}

export const OPZIONI_PREDEFINITE: Readonly<OpzioniFlusso> = { direzione: "TB", larghezza: null };

/** Qualsiasi fine riga di Typst (anche `\v`, `\f`, NEL e i separatori Unicode): spezzerebbe la riga DOT. */
const haACapo = (s: string): boolean => /[\r\n\v\f\u0085\u2028\u2029]/.test(s);

/** Errore su un testo che finirà nel DOT, o `null`. `nome` è il campo, per il messaggio. */
function erroreTesto(s: string, nome: string): string | null {
  if (haACapo(s)) return `${nome}: non può andare a capo`;
  if (s.includes("`")) return `${nome}: non può contenere il carattere \``;
  if (s.trim().length > TESTO_MAX) return `${nome}: al massimo ${TESTO_MAX} caratteri`;
  return null;
}

/** Primo errore di validazione in italiano, o `null` se i dati sono corretti. */
export function validaFlusso(nodi: readonly Nodo[], archi: readonly Arco[], opz: OpzioniFlusso): string | null {
  if (nodi.length === 0) {
    return "Aggiungi almeno un nodo";
  }
  if (nodi.length > NODI_MAX || archi.length > ARCHI_MAX) {
    return `Al massimo ${NODI_MAX} nodi e ${ARCHI_MAX} archi`;
  }
  for (let i = 0; i < nodi.length; i++) {
    const n = nodi[i];
    const nome = `Nodo ${i + 1}`;
    if (n.testo.trim() === "") return `${nome}: manca il testo`;
    const e = erroreTesto(n.testo, nome) ?? erroreTesto(n.riquadro, `${nome}, riquadro`) ?? erroreTesto(n.livello, `${nome}, livello`);
    if (e !== null) return e;
    if (!FORME.some((f) => f.id === n.forma)) return `${nome}: forma non valida`;
    if (n.colore !== "" && !COLORI.some((c) => c.id === n.colore)) return `${nome}: colore non valido`;
    if (n.riquadro.trim() !== "" && n.livello.trim() !== "") {
      return `${nome}: un nodo non può stare insieme in un riquadro e in un livello comune (Graphviz non disegna il riquadro)`;
    }
  }
  for (let i = 0; i < archi.length; i++) {
    const a = archi[i];
    const nome = `Arco ${i + 1}`;
    const valido = (k: number): boolean => Number.isInteger(k) && k >= 0 && k < nodi.length;
    if (!valido(a.da) || !valido(a.a)) return `${nome}: nodo di partenza o di arrivo non valido`;
    const e = erroreTesto(a.etichetta, `${nome}, etichetta`);
    if (e !== null) return e;
  }
  if (!DIREZIONI.some((d) => d.id === opz.direzione)) return "Direzione non valida";
  if (opz.larghezza !== null && (!Number.isInteger(opz.larghezza) || opz.larghezza < 10 || opz.larghezza > 100)) {
    return "La larghezza è un numero intero da 10 a 100 (% della pagina)";
  }
  return null;
}

/** Esito della generazione. */
export type EsitoFlusso = { codice: string } | { errore: string };

/** Stringa DOT tra virgolette (`\` e `"` escapati). */
function dot(s: string): string {
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

/** Id DOT del nodo `i`. */
const idNodo = (i: number): string => `n${i + 1}`;

/** Indici dei nodi raggruppati per valore (ordine di prima comparsa); i valori vuoti sono esclusi. */
function raggruppa(nodi: readonly Nodo[], chiave: (n: Nodo) => string): Map<string, number[]> {
  const gruppi = new Map<string, number[]>();
  nodi.forEach((n, i) => {
    const k = chiave(n).trim();
    if (k !== "") {
      gruppi.set(k, [...(gruppi.get(k) ?? []), i]);
    }
  });
  return gruppi;
}

/**
 * Codice Typst: `#diagraph.raw-render(` con un blocco `dot` (nodi, archi, riquadri, livelli comuni).
 * I nodi dentro un riquadro sono dichiarati nel riquadro stesso.
 */
export function generaFlusso(nodi: readonly Nodo[], archi: readonly Arco[], opz: OpzioniFlusso): EsitoFlusso {
  const errore = validaFlusso(nodi, archi, opz);
  if (errore !== null) {
    return { errore };
  }
  const istruzioneNodo = (n: Nodo, i: number): string => {
    const attributi = [`label=${dot(n.testo.trim())}`, FORME.find((f) => f.id === n.forma)!.attributi];
    const colore = COLORI.find((c) => c.id === n.colore);
    if (colore !== undefined) attributi.push("style=filled", `fillcolor="${colore.esadecimale}"`);
    return `${idNodo(i)} [${attributi.join(", ")}];`;
  };

  const riquadri = raggruppa(nodi, (n) => n.riquadro);
  const livelli = raggruppa(nodi, (n) => n.livello);
  const righe: string[] = [`rankdir=${opz.direzione};`];
  nodi.forEach((n, i) => {
    if (n.riquadro.trim() === "") righe.push(istruzioneNodo(n, i));
  });
  let k = 0;
  for (const [titolo, indici] of riquadri) {
    k += 1;
    righe.push(
      `subgraph cluster_${k} {`,
      `  label=${dot(titolo)}; style=dashed;`,
      ...indici.map((i) => `  ${istruzioneNodo(nodi[i], i)}`),
      "}",
    );
  }
  for (const a of archi) {
    const attributi: string[] = [];
    if (a.etichetta.trim() !== "") attributi.push(`label=${dot(a.etichetta.trim())}`);
    if (a.tratteggiato) attributi.push("style=dashed");
    righe.push(`${idNodo(a.da)} -> ${idNodo(a.a)}${attributi.length > 0 ? ` [${attributi.join(", ")}]` : ""};`);
  }
  for (const indici of livelli.values()) {
    if (indici.length > 1) righe.push(`{ rank=same; ${indici.map(idNodo).join("; ")}; }`);
  }

  const corpo = ["digraph {", ...righe.map((r) => `  ${r}`), "}"].map((r) => `  ${r}`).join("\n");
  const larghezza = opz.larghezza === null ? "" : `  width: ${opz.larghezza}%,\n`;
  // `math-mode: "text"`: con `auto` il plugin può trattare come formula un'etichetta come `x_1` o `Costo #1`.
  return { codice: `#diagraph.raw-render(\n  \`\`\`dot\n${corpo}\n  \`\`\`,\n  math-mode: "text",\n${larghezza})\n` };
}

/** Dati d'esempio: un ciclo («Correggi»), un fork con tre rami sullo stesso livello e un join. */
export function esempioFlusso(): { nodi: Nodo[]; archi: Arco[] } {
  const n = (testo: string, forma: Forma, extra: Partial<Nodo> = {}): Nodo => ({
    testo, forma, colore: "", riquadro: "", livello: "", ...extra,
  });
  const a = (da: number, a: number, etichetta = "", tratteggiato = false): Arco => ({ da, a, etichetta, tratteggiato });
  return {
    nodi: [
      n("Inizio", "inizio-fine", { colore: "rosso" }),
      n("Ricevi l'ordine", "io", { colore: "viola" }),
      n("Ordine valido?", "decisione", { colore: "arancio" }),
      n("Segnala l'errore", "io", { colore: "viola" }),
      n("Prepara la merce", "processo", { livello: "evasione" }),
      n("Emetti la fattura", "processo", { livello: "evasione" }),
      n("Controlla il pagamento", "processo", { livello: "evasione" }),
      n("Spedisci", "sottoprocesso", { colore: "azzurro" }),
      n("Fine", "inizio-fine", { colore: "rosso" }),
    ],
    archi: [
      a(0, 1), a(1, 2), a(2, 3, "No"), a(3, 1, "correggi", true),
      a(2, 4, "Sì"), a(2, 5, "Sì"), a(2, 6, "Sì"),
      a(4, 7), a(5, 7), a(6, 7), a(7, 8),
    ],
  };
}

/** Toglie il nodo `i` e gli archi che lo toccano; gli indici degli archi restanti scalano di un posto. */
export function rimuoviNodo(nodi: readonly Nodo[], archi: readonly Arco[], i: number): { nodi: Nodo[]; archi: Arco[] } {
  const scala = (k: number): number => (k > i ? k - 1 : k);
  return {
    nodi: nodi.filter((_, j) => j !== i),
    archi: archi.filter((a) => a.da !== i && a.a !== i).map((a) => ({ ...a, da: scala(a.da), a: scala(a.a) })),
  };
}
