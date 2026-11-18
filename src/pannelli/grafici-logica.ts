/**
 * grafici-logica.ts — Logica pura dello strumento «Grafici» (v1): analisi di una
 * tabella incollata e generazione del codice `lilaq` o `cetz` (linee, punti, barre).
 * Nessun DOM, nessun Tauri: verificata da `grafici.check.ts`. Contratto e piano in
 * `DECISIONI.md` («Strumento «Grafici»»).
 */

import type { IdPacchetto } from "./equazioni-logica.ts";
import { etichettaValida } from "./inserisci-logica.ts";
import { escapaContenuto } from "./pagina-logica.ts";

export const RIGHE_MAX = 2000;
export const SERIE_MAX = 6;

export type TipoGrafico = "linee" | "punti" | "barre";

export const TIPI_GRAFICO: readonly { id: TipoGrafico; etichetta: string; funzione: string }[] = [
  { id: "linee", etichetta: "Linee", funzione: "plot" },
  { id: "punti", etichetta: "Punti", funzione: "scatter" },
  { id: "barre", etichetta: "Barre", funzione: "bar" },
];

/** Libreria che disegna il grafico: `lilaq` (dati, assi e legenda automatici) o `cetz` (più libera). */
export type MotoreGrafico = "lilaq" | "cetz";

export const MOTORI_GRAFICO: readonly { id: MotoreGrafico; etichetta: string; descrizione: string; pacchetti: readonly IdPacchetto[] }[] = [
  {
    id: "lilaq",
    etichetta: "Lilaq — grafici di dati",
    descrizione: "Assi, legenda e stile curati in automatico: la scelta per grafici scientifici standard.",
    pacchetti: ["lilaq"],
  },
  {
    id: "cetz",
    etichetta: "CeTZ — grafici complessi",
    descrizione:
      "Il codice generato è un disegno CeTZ: si presta a essere ritoccato con annotazioni, frecce, più assi o elementi grafici liberi.",
    pacchetti: ["cetz", "cetz-plot"],
  },
];

/** Dati di un grafico: la colonna X e una o più serie Y, con i nomi se la tabella aveva un'intestazione. */
export interface DatiGrafico {
  x: number[];
  serie: number[][];
  /** Nome della colonna X e delle serie, dall'intestazione (o generici). */
  nomiColonne: string[];
}

/** Risultato dell'analisi della tabella. */
export type EsitoTabella = { dati: DatiGrafico } | { errore: string };

/** Separatore della tabella: tab, punto e virgola, spazi o, in mancanza, virgola. */
function separatore(righe: readonly string[]): RegExp {
  if (righe.some((r) => r.includes("\t"))) return /\t/;
  if (righe.some((r) => r.includes(";"))) return /;/;
  if (righe.some((r) => /\S\s+\S/.test(r))) return /\s+/;
  return /,/;
}

/**
 * Analizza una tabella incollata (almeno due colonne e due righe di dati). La prima
 * riga è un'intestazione se contiene celle non numeriche. La virgola è decimale
 * (`1,5`) salvo quando è l'unico separatore (CSV).
 */
export function analizzaTabella(testo: string): EsitoTabella {
  const righe = testo.split(/\r?\n/).map((r) => r.trim()).filter((r) => r !== "");
  if (righe.length === 0) {
    return { errore: "Incolla una tabella: la prima colonna è X, le altre sono le serie" };
  }
  const sep = separatore(righe);
  const virgolaDecimale = sep.source !== ",";
  const celle = righe.map((r) =>
    r.split(sep).map((c) => {
      const t = c.trim();
      return virgolaDecimale ? t.replace(",", ".") : t;
    }),
  );
  const colonne = celle[0].length;
  if (colonne < 2) {
    return { errore: "Servono almeno due colonne (X e una serie)" };
  }
  if (celle.some((r) => r.length !== colonne)) {
    return { errore: "Le righe non hanno tutte lo stesso numero di colonne" };
  }
  const eNumero = (c: string): boolean => c !== "" && Number.isFinite(Number(c));
  const conIntestazione = celle[0].some((c) => !eNumero(c));
  const corpo = conIntestazione ? celle.slice(1) : celle;
  if (corpo.length < 2) {
    return { errore: "Servono almeno due righe di dati" };
  }
  if (corpo.length > RIGHE_MAX) {
    return { errore: `Troppe righe: al massimo ${RIGHE_MAX}` };
  }
  if (colonne - 1 > SERIE_MAX) {
    return { errore: `Troppe serie: al massimo ${SERIE_MAX}` };
  }
  for (let i = 0; i < corpo.length; i++) {
    const cattiva = corpo[i].findIndex((c) => !eNumero(c));
    if (cattiva !== -1) {
      return { errore: `Riga ${i + 1 + (conIntestazione ? 1 : 0)}, colonna ${cattiva + 1}: «${corpo[i][cattiva]}» non è un numero` };
    }
  }
  const nomiColonne = conIntestazione
    ? celle[0].map((c, i) => (c === "" ? (i === 0 ? "X" : `Serie ${i}`) : c))
    : celle[0].map((_, i) => (i === 0 ? "X" : `Serie ${i}`));
  return {
    dati: {
      x: corpo.map((r) => Number(r[0])),
      serie: Array.from({ length: colonne - 1 }, (_, k) => corpo.map((r) => Number(r[k + 1]))),
      nomiColonne,
    },
  };
}

/** Opzioni del grafico. */
export interface OpzioniGrafico {
  motore: MotoreGrafico;
  tipo: TipoGrafico;
  titolo: string;
  etichettaX: string;
  etichettaY: string;
  /** Mostra i nomi delle serie in legenda (con una sola serie resta spenta se vuoto). */
  legenda: boolean;
  didascalia: string;
  /** Nome dell'etichetta per citare la figura (senza `< >`). */
  etichetta: string;
}

const lista = (v: readonly number[]): string => `(${v.map(String).join(", ")})`;

const coppie = (x: readonly number[], y: readonly number[]): string =>
  `(${x.map((v, i) => `(${v}, ${y[i]})`).join(", ")})`;

/** Corpo `lilaq` del diagramma (senza `#`). */
function diagrammaLilaq(dati: DatiGrafico, o: OpzioniGrafico): string {
  const funzione = TIPI_GRAFICO.find((t) => t.id === o.tipo)?.funzione ?? "plot";
  const righe: string[] = [];
  if (o.titolo.trim() !== "") righe.push(`title: [${escapaContenuto(o.titolo.trim())}],`);
  if (o.etichettaX.trim() !== "") righe.push(`xlabel: [${escapaContenuto(o.etichettaX.trim())}],`);
  if (o.etichettaY.trim() !== "") righe.push(`ylabel: [${escapaContenuto(o.etichettaY.trim())}],`);
  dati.serie.forEach((y, i) => {
    const nome = dati.nomiColonne[i + 1] ?? `Serie ${i + 1}`;
    const etichettaSerie = o.legenda ? `, label: [${escapaContenuto(nome)}]` : "";
    righe.push(`lq.${funzione}(${lista(dati.x)}, ${lista(y)}${etichettaSerie}),`);
  });
  return `lq.diagram(\n${righe.map((r) => `  ${r}`).join("\n")}\n)`;
}

/** Larghezza delle barre: 80% della distanza minima fra due X (1 se coincidono tutte). */
function larghezzaBarre(x: readonly number[]): number {
  const ordinati = [...new Set(x)].sort((a, b) => a - b);
  let minimo = Infinity;
  for (let i = 1; i < ordinati.length; i++) minimo = Math.min(minimo, ordinati[i] - ordinati[i - 1]);
  return Number.isFinite(minimo) ? Number(((minimo * 4) / 5).toPrecision(6)) : 1;
}

/**
 * Corpo `cetz` del diagramma (senza `#`): un `cetz.canvas` con `plot.plot` di cetz-plot.
 * cetz-plot non ha il titolo: con un titolo il canvas va in uno `stack` sotto il testo.
 * Le barre di più serie sono affiancate (`clustered`), non sovrapposte.
 */
function diagrammaCetz(dati: DatiGrafico, o: OpzioniGrafico): string {
  const nome = (i: number): string => escapaContenuto(dati.nomiColonne[i + 1] ?? `Serie ${i + 1}`);
  const aggiunte: string[] = [];
  if (o.tipo === "barre") {
    const righe = `(${dati.x.map((v, i) => `(${[v, ...dati.serie.map((s) => s[i])].join(", ")})`).join(", ")})`;
    const chiavi = dati.serie.map((_, i) => i + 1).join(", ");
    const etichette = o.legenda ? `, labels: (${dati.serie.map((_, i) => `[${nome(i)}]`).join(", ")},)` : "";
    aggiunte.push(
      `plot.add-bar(${righe}, x-key: 0, y-key: (${chiavi},), mode: "clustered", bar-width: ${larghezzaBarre(dati.x)}${etichette})`,
    );
  } else {
    dati.serie.forEach((y, i) => {
      const stile = o.tipo === "punti" ? ', mark: "o", style: (stroke: none)' : "";
      const etichettaSerie = o.legenda ? `, label: [${nome(i)}]` : "";
      aggiunte.push(`plot.add(${coppie(dati.x, y)}${stile}${etichettaSerie})`);
    });
  }
  const opzioni = ["size: (10, 6),"];
  if (o.etichettaX.trim() !== "") opzioni.push(`x-label: [${escapaContenuto(o.etichettaX.trim())}],`);
  if (o.etichettaY.trim() !== "") opzioni.push(`y-label: [${escapaContenuto(o.etichettaY.trim())}],`);
  const canvas = [
    "cetz.canvas({",
    "  plot.plot(",
    ...opzioni.map((r) => `    ${r}`),
    "    {",
    ...aggiunte.map((r) => `      ${r}`),
    "    },",
    "  )",
    "})",
  ].join("\n");
  const t = o.titolo.trim();
  if (t === "") return canvas;
  const interno = canvas.replace(/^/gm, "  ");
  return `stack(\n  dir: ttb,\n  spacing: 0.8em,\n  align(center, strong[${escapaContenuto(t)}]),\n${interno},\n)`;
}

/**
 * Codice del grafico con la libreria scelta (`o.motore`). Con didascalia o etichetta il
 * diagramma va in una `#figure`. `null` se l'etichetta non è valida. L'import dei
 * pacchetti (`MOTORI_GRAFICO[..].pacchetti`) lo aggiunge il chiamante.
 */
export function generaGrafico(dati: DatiGrafico, o: OpzioniGrafico): string | null {
  const e = o.etichetta.trim();
  if (e !== "" && !etichettaValida(e)) {
    return null;
  }
  const diagramma = o.motore === "cetz" ? diagrammaCetz(dati, o) : diagrammaLilaq(dati, o);
  const d = o.didascalia.trim();
  if (d === "" && e === "") {
    return `#${diagramma}\n`;
  }
  const interno = diagramma.replace(/^/gm, "  ");
  return `#figure(\n${interno},\n${d !== "" ? `  caption: [${escapaContenuto(d)}],\n` : ""})${e !== "" ? ` <${e}>` : ""}\n`;
}
