/**
 * equazioni-logica.ts — Logica pura dello strumento «Equazioni»: strutture
 * matematiche con i loro campi, formula finale, import dei pacchetti e blocco
 * delle impostazioni di numerazione. Nessun DOM, nessun Tauri: verificata da
 * `equazioni.check.ts`. Contratto in `DECISIONI.md` («Strumento «Equazioni»»).
 */

import { etichettaValida } from "./inserisci-logica.ts";
import type { Cambio } from "./strumenti-logica";

// ---- Pacchetti ----

export type IdPacchetto = "physica" | "mitex" | "equate" | "lilaq" | "gantt-newgen";

/** Versioni dall'indice di Typst Universe del 2026-10-16 (da riverificare). */
export const PACCHETTI: Readonly<Record<IdPacchetto, { versione: string; descrizione: string }>> = {
  physica: { versione: "0.9.8", descrizione: "derivate, differenziali, bra-ket, operatori vettoriali" },
  mitex: { versione: "0.2.7", descrizione: "formule scritte in LaTeX" },
  equate: { versione: "0.3.3", descrizione: "numerazione per riga e formule su più pagine" },
  lilaq: { versione: "0.6.0", descrizione: "grafici scientifici (strumento Grafici)" },
  // Pacchetto dell'utente INCORPORATO nell'eseguibile (`@local`, nessun download): vedi pacchetti_incorporati.rs.
  "gantt-newgen": { versione: "0.1.0", descrizione: "diagrammi di Gantt (strumento Gantt; incorporato, LGPL-3.0)" },
};

/** Riga di import di un pacchetto (`#import "@preview/physica:0.9.8": *`). */
export function rigaImport(id: IdPacchetto): string {
  if (id === "equate") {
    return `#import "@preview/equate:${PACCHETTI.equate.versione}": equate`;
  }
  if (id === "lilaq") {
    return `#import "@preview/lilaq:${PACCHETTI.lilaq.versione}" as lq`;
  }
  if (id === "gantt-newgen") {
    return `#import "@local/gantt-newgen:${PACCHETTI["gantt-newgen"].versione}": gantt`;
  }
  return `#import "@preview/${id}:${PACCHETTI[id].versione}": *`;
}

/** `true` se il documento importa già il pacchetto (qualsiasi versione). */
export function haImport(testo: string, id: IdPacchetto): boolean {
  // `gantt-newgen` è un pacchetto locale (incorporato), gli altri vengono da Typst Universe.
  const ns = id === "gantt-newgen" ? "local" : "preview";
  return new RegExp(`@${ns}/${id}[:"]`).test(testo);
}

// ---- Strutture ----

/** Un campo del modulo di una struttura. */
export interface CampoStruttura {
  id: string;
  etichetta: string;
  predefinito: string;
  /** Se presente, il campo è un menu con queste scelte. */
  scelte?: readonly string[];
  /** Se presente, il campo è un numero intero in questo intervallo. */
  intero?: readonly [number, number];
  /** Il campo può restare vuoto. */
  facoltativo?: boolean;
}

/** Una struttura matematica inseribile. */
export interface Struttura {
  id: string;
  etichetta: string;
  /** Pacchetto richiesto, o `null` se è Typst nativo. */
  pacchetto: IdPacchetto | null;
  campi: readonly CampoStruttura[];
  genera(v: Readonly<Record<string, string>>): string;
}

/** Elenco `prefisso_1, prefisso_2, …` per `n` elementi. */
function indicizzati(prefisso: string, n: number): string {
  return Array.from({ length: n }, (_, i) => `${prefisso}_${i + 1}`).join(", ");
}

export const STRUTTURE: readonly Struttura[] = [
  {
    id: "frazione", etichetta: "Frazione", pacchetto: null,
    campi: [
      { id: "num", etichetta: "Numeratore", predefinito: "a" },
      { id: "den", etichetta: "Denominatore", predefinito: "b" },
    ],
    genera: (v) => `frac(${v.num}, ${v.den})`,
  },
  {
    id: "radice", etichetta: "Radice", pacchetto: null,
    campi: [
      { id: "radicando", etichetta: "Radicando", predefinito: "x" },
      { id: "indice", etichetta: "Indice (vuoto = quadrata)", predefinito: "", facoltativo: true },
    ],
    genera: (v) => (v.indice.trim() === "" ? `sqrt(${v.radicando})` : `root(${v.indice}, ${v.radicando})`),
  },
  {
    id: "sommatoria", etichetta: "Sommatoria o produttoria", pacchetto: null,
    campi: [
      { id: "op", etichetta: "Operatore", predefinito: "sum", scelte: ["sum", "product"] },
      { id: "da", etichetta: "Da", predefinito: "i = 1" },
      { id: "a", etichetta: "A", predefinito: "n" },
      { id: "termine", etichetta: "Termine", predefinito: "a_i" },
    ],
    genera: (v) => `${v.op}_(${v.da})^(${v.a}) ${v.termine}`,
  },
  {
    id: "integrale", etichetta: "Integrale", pacchetto: null,
    campi: [
      { id: "da", etichetta: "Da", predefinito: "0" },
      { id: "a", etichetta: "A", predefinito: "1" },
      { id: "f", etichetta: "Integrando", predefinito: "f(x)" },
      { id: "var", etichetta: "Variabile", predefinito: "x" },
    ],
    genera: (v) => `integral_(${v.da})^(${v.a}) ${v.f} dif ${v.var}`,
  },
  {
    id: "limite", etichetta: "Limite", pacchetto: null,
    campi: [
      { id: "var", etichetta: "Variabile", predefinito: "x" },
      { id: "verso", etichetta: "Tende a", predefinito: "0" },
      { id: "f", etichetta: "Espressione", predefinito: "f(x)" },
    ],
    genera: (v) => `lim_(${v.var} -> ${v.verso}) ${v.f}`,
  },
  {
    id: "matrice", etichetta: "Matrice", pacchetto: null,
    campi: [
      { id: "righe", etichetta: "Righe", predefinito: "2", intero: [1, 8] },
      { id: "colonne", etichetta: "Colonne", predefinito: "2", intero: [1, 8] },
    ],
    genera: (v) => {
      const r = Number(v.righe);
      const c = Number(v.colonne);
      const righe = Array.from({ length: r }, (_, i) =>
        Array.from({ length: c }, (_, j) => `a_(${i + 1}${j + 1})`).join(", "),
      );
      return `mat(${righe.join("; ")})`;
    },
  },
  {
    id: "sistema", etichetta: "Sistema di equazioni", pacchetto: null,
    campi: [{ id: "n", etichetta: "Equazioni", predefinito: "2", intero: [1, 8] }],
    genera: (v) =>
      `cases(${Array.from({ length: Number(v.n) }, (_, i) => `a_${i + 1} x + b_${i + 1} y = c_${i + 1}`).join(", ")})`,
  },
  {
    id: "vettore", etichetta: "Vettore", pacchetto: null,
    campi: [{ id: "n", etichetta: "Componenti", predefinito: "3", intero: [1, 8] }],
    genera: (v) => `vec(${indicizzati("x", Number(v.n))})`,
  },
  {
    id: "binomiale", etichetta: "Coefficiente binomiale", pacchetto: null,
    campi: [
      { id: "n", etichetta: "n", predefinito: "n" },
      { id: "k", etichetta: "k", predefinito: "k" },
    ],
    genera: (v) => `binom(${v.n}, ${v.k})`,
  },
  {
    id: "accento", etichetta: "Accento sopra un simbolo", pacchetto: null,
    campi: [
      { id: "tipo", etichetta: "Accento", predefinito: "hat", scelte: ["hat", "tilde", "bar", "dot", "arrow", "overline"] },
      { id: "base", etichetta: "Simbolo", predefinito: "x" },
    ],
    genera: (v) => `${v.tipo}(${v.base})`,
  },
  // ---- physica ----
  {
    id: "derivata", etichetta: "Derivata", pacchetto: "physica",
    campi: [
      { id: "f", etichetta: "Funzione", predefinito: "f" },
      { id: "x", etichetta: "Variabile", predefinito: "x" },
    ],
    genera: (v) => `dv(${v.f}, ${v.x})`,
  },
  {
    id: "parziale", etichetta: "Derivata parziale", pacchetto: "physica",
    campi: [
      { id: "f", etichetta: "Funzione", predefinito: "f" },
      { id: "x", etichetta: "Variabile", predefinito: "x" },
    ],
    genera: (v) => `pdv(${v.f}, ${v.x})`,
  },
  {
    id: "differenziale", etichetta: "Differenziale", pacchetto: "physica",
    campi: [{ id: "x", etichetta: "Variabile", predefinito: "x" }],
    genera: (v) => `dd(${v.x})`,
  },
  {
    id: "gradiente", etichetta: "Gradiente", pacchetto: "physica",
    campi: [{ id: "f", etichetta: "Funzione", predefinito: "f" }],
    genera: (v) => `grad ${v.f}`,
  },
  {
    id: "ket", etichetta: "Ket |ψ⟩", pacchetto: "physica",
    campi: [{ id: "s", etichetta: "Stato", predefinito: "psi" }],
    genera: (v) => `ket(${v.s})`,
  },
  {
    id: "bra", etichetta: "Bra ⟨φ|", pacchetto: "physica",
    campi: [{ id: "s", etichetta: "Stato", predefinito: "phi" }],
    genera: (v) => `bra(${v.s})`,
  },
  {
    id: "braket", etichetta: "Prodotto scalare ⟨φ|ψ⟩", pacchetto: "physica",
    campi: [
      { id: "a", etichetta: "Bra", predefinito: "phi" },
      { id: "b", etichetta: "Ket", predefinito: "psi" },
    ],
    genera: (v) => `braket(${v.a}, ${v.b})`,
  },
];

/** Struttura per id. */
export function strutturaPerId(id: string): Struttura | undefined {
  return STRUTTURE.find((s) => s.id === id);
}

/** Valori iniziali dei campi di una struttura. */
export function valoriIniziali(s: Struttura): Record<string, string> {
  return Object.fromEntries(s.campi.map((c) => [c.id, c.predefinito]));
}

/**
 * Codice matematico della struttura, o un messaggio di errore. Un campo vuoto non
 * facoltativo, un `$` o un a capo in qualsiasi campo, un intero fuori intervallo o una
 * scelta non ammessa invalidano il risultato.
 */
export function generaStruttura(
  s: Struttura,
  valori: Readonly<Record<string, string>>,
): { codice: string } | { errore: string } {
  const v: Record<string, string> = {};
  for (const c of s.campi) {
    const t = (valori[c.id] ?? "").trim();
    if (/[$\r\n]/.test(t)) {
      return { errore: `«${c.etichetta}»: non sono ammessi $ né a capo` };
    }
    if (t === "" && c.facoltativo !== true) {
      return { errore: `«${c.etichetta}» è obbligatorio` };
    }
    if (c.intero !== undefined) {
      const n = Number(t);
      if (!Number.isInteger(n) || n < c.intero[0] || n > c.intero[1]) {
        return { errore: `«${c.etichetta}»: un intero da ${c.intero[0]} a ${c.intero[1]}` };
      }
    }
    if (c.scelte !== undefined && !c.scelte.includes(t)) {
      return { errore: `«${c.etichetta}»: scelta non valida` };
    }
    v[c.id] = t;
  }
  return { codice: s.genera(v) };
}

// ---- Formula e inserimento ----

/** Opzioni della formula finale. */
export interface OpzioniFormula {
  visualizzata: boolean;
  /** Nome dell'etichetta (senza `< >`), facoltativo. */
  etichetta: string;
}

/** `$ formula $ <etichetta>` (visualizzata) o `$formula$` (in linea); `null` se non valida. */
export function generaFormula(formula: string, opz: OpzioniFormula): string | null {
  const f = formula.trim();
  const e = opz.etichetta.trim();
  if (f === "" || f.includes("$") || (e !== "" && !etichettaValida(e))) {
    return null;
  }
  if (!opz.visualizzata) {
    return `$${f}$`;
  }
  return `$ ${f.replace(/\r?\n/g, "\n  ")} $${e !== "" ? ` <${e}>` : ""}\n`;
}

/** `#mitex(`…`)` (blocco) o `#mi(`…`)` (in linea); `null` se vuoto o con un backtick. */
export function generaMitex(latex: string, inLinea: boolean): string | null {
  const t = latex.trim();
  if (t === "" || t.includes("`")) {
    return null;
  }
  return inLinea ? `#mi(\`${t}\`)` : `#mitex(\`${t}\`)\n`;
}

/** Esito di un inserimento: i cambi sul documento attuale e la selezione nel testo risultante. */
export interface PianoInserimento {
  cambi: Cambio[];
  selezione: { da: number; a: number };
}

/**
 * Piano di inserimento di `codice` al posto della selezione `sel`: se servono import di
 * pacchetti non ancora presenti li aggiunge in cima, nella stessa transazione.
 */
export function pianoInserimento(
  testo: string,
  sel: { da: number; a: number },
  codice: string,
  pacchetti: readonly IdPacchetto[],
): PianoInserimento {
  const mancanti = [...new Set(pacchetti)].filter((p) => !haImport(testo, p));
  const cambi: Cambio[] = [];
  let spostamento = 0;
  if (mancanti.length > 0) {
    const righe = `${mancanti.map(rigaImport).join("\n")}\n\n`;
    cambi.push({ da: 0, a: 0, testo: righe });
    spostamento = righe.length;
  }
  cambi.push({ da: sel.da, a: sel.a, testo: codice });
  return { cambi, selezione: { da: sel.da + spostamento, a: sel.da + spostamento + codice.length } };
}

// ---- Blocco delle impostazioni delle equazioni ----

const INIZIO_BLOCCO = "// typster:equazioni";
const FINE_BLOCCO = "// fine typster:equazioni";

/** Impostazioni lette dal blocco gestito. */
export interface ImpostazioniEquazioni {
  /** Numera le equazioni `(1)`. */
  numerazione: boolean;
  /** Usa `equate`: numerazione per riga e formule su più pagine. */
  equate: boolean;
}

/** Intervallo del blocco `[da, a)` (con a capo finale e riga vuota), o `null`. */
function trovaBlocco(testo: string): { da: number; a: number; corpo: string } | null {
  const i = testo.indexOf(INIZIO_BLOCCO);
  if (i === -1 || (i > 0 && testo[i - 1] !== "\n")) {
    return null;
  }
  const f = testo.indexOf(FINE_BLOCCO, i);
  if (f === -1) {
    return null;
  }
  let a = f + FINE_BLOCCO.length;
  for (let k = 0; k < 2 && testo[a] === "\n"; k++) {
    a++;
  }
  return { da: i, a, corpo: testo.slice(i, f) };
}

/** Legge le impostazioni dal blocco (tutto falso se manca). */
export function leggiImpostazioni(testo: string): ImpostazioniEquazioni {
  const b = trovaBlocco(testo);
  return {
    numerazione: b !== null && b.corpo.includes("numbering"),
    equate: b !== null && b.corpo.includes("equate.with"),
  };
}

/** Testo del blocco per le impostazioni (vuoto se tutto spento). */
export function testoBlocco(imp: ImpostazioniEquazioni): string {
  if (!imp.numerazione && !imp.equate) {
    return "";
  }
  const righe = [INIZIO_BLOCCO];
  if (imp.equate) {
    righe.push(rigaImport("equate"), "#show: equate.with(breakable: true, sub-numbering: true)");
  }
  righe.push(`#set math.equation(numbering: "${imp.equate ? "(1.1)" : "(1)"}")`, FINE_BLOCCO);
  return `${righe.join("\n")}\n\n`;
}

/** Riscrive, inserisce o rimuove il blocco; `null` se non c'è nulla da cambiare. */
export function scriviBlocco(testo: string, imp: ImpostazioniEquazioni): PianoInserimento | null {
  const nuovo = testoBlocco(imp);
  const b = trovaBlocco(testo);
  if (b === null) {
    return nuovo === "" ? null : { cambi: [{ da: 0, a: 0, testo: nuovo }], selezione: { da: 0, a: nuovo.length } };
  }
  if (testo.slice(b.da, b.a) === nuovo) {
    return null;
  }
  return { cambi: [{ da: b.da, a: b.a, testo: nuovo }], selezione: { da: b.da, a: b.da + nuovo.length } };
}
