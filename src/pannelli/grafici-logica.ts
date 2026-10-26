/**
 * grafici-logica.ts — Logica pura dello strumento «Grafici» (v1): analisi di una
 * tabella incollata e generazione del codice `lilaq` (linee, punti, barre).
 * Nessun DOM, nessun Tauri: verificata da `grafici.check.ts`. Contratto e piano in
 * `DECISIONI.md` («Strumento «Grafici»»).
 */

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

/**
 * Codice `lilaq` del grafico. Con didascalia o etichetta il diagramma va in una
 * `#figure`. `null` se l'etichetta non è valida.
 */
export function generaGrafico(dati: DatiGrafico, o: OpzioniGrafico): string | null {
  const e = o.etichetta.trim();
  if (e !== "" && !etichettaValida(e)) {
    return null;
  }
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
  const diagramma = `lq.diagram(\n${righe.map((r) => `  ${r}`).join("\n")}\n)`;
  const d = o.didascalia.trim();
  if (d === "" && e === "") {
    return `#${diagramma}\n`;
  }
  const interno = diagramma.replace(/^/gm, "  ");
  return `#figure(\n${interno},\n${d !== "" ? `  caption: [${escapaContenuto(d)}],\n` : ""})${e !== "" ? ` <${e}>` : ""}\n`;
}
