/**
 * pagina-logica.ts — Logica pura dello strumento «Pagina»: legge e riscrive la
 * regola `#set page(...)` di un documento Typst.
 *
 * Gestisce `paper`, `flipped`, `margin` (un valore o `(x:, y:)`), `columns`,
 * `header` (contenuto semplice `[testo]`) e `numbering` (`"1"`). Ogni altro
 * argomento si conserva alla lettera, nello stesso ordine. Nessun DOM, nessun
 * Tauri: verificata da `pagina.check.ts`.
 */

import type { Cambio } from "./strumenti-logica";

/** Formati carta ammessi dal pannello (nome Typst → dimensioni in mm, verticale). */
export const FORMATI: Readonly<Record<string, readonly [number, number]>> = {
  a3: [297, 420],
  a4: [210, 297],
  a5: [148, 210],
  "us-letter": [215.9, 279.4],
  "us-legal": [215.9, 355.6],
};

/** Etichette dei formati per l'interfaccia. */
export const ETICHETTE_FORMATI: Readonly<Record<string, string>> = {
  a3: "A3",
  a4: "A4",
  a5: "A5",
  "us-letter": "Letter",
  "us-legal": "Legal",
};

/** Margine predefinito di Typst, in mm, usato solo per l'anteprima quando il documento non ne specifica uno. */
export const MARGINE_PREDEFINITO_MM = 25;

/** Stato dei controlli del pannello. */
export interface StatoPagina {
  /** Nome Typst del formato; sempre uno di `FORMATI`. */
  formato: string;
  orizzontale: boolean;
  /** Margine alto/basso e sinistro/destro, già normalizzati (`2.5cm`); stringa vuota = non specificato. */
  margineY: string;
  margineX: string;
  /** `margin` presente ma non rappresentabile (es. `top`/`left`): campi disabilitati, argomento conservato. */
  margineAvanzato: boolean;
  colonne: number;
  /** Testo dell'intestazione (non escapato); vuoto = nessuna. */
  intestazione: string;
  intestazioneAvanzata: boolean;
  numerazione: boolean;
  numerazioneAvanzata: boolean;
}

/** Un argomento della chiamata, come scritto. */
interface Argomento {
  nome: string | null;
  /** Testo dell'argomento (valore, senza `nome:`). */
  valore: string;
  /** Testo originale intero (con `nome:`). */
  grezzo: string;
}

/** Regola `#set page(...)` trovata nel documento. */
export interface RegolaPagina {
  /** Offset di `#` e offset subito dopo la `)`. */
  da: number;
  a: number;
  /** Rientro (spazi/tab) davanti a `#set`. */
  rientro: string;
  args: Argomento[];
}

const INIZIO_REGOLA = /[ \t]*#set\s+page\s*\(/y;

/** Salta un raw (`` ` ``…`` ` ``) che inizia in `i`; restituisce l'offset dopo la chiusura (o la fine). */
function saltaRaw(t: string, i: number): number {
  let n = 0;
  while (t[i + n] === "`") {
    n++;
  }
  const f = t.indexOf("`".repeat(n), i + n);
  return f === -1 ? t.length : f + n;
}

/** Salta un commento (di riga o a blocco) che inizia in `i`; restituisce l'offset dopo, o -1 se in `i` non c'è un commento. */
function saltaCommento(t: string, i: number): number {
  if (t[i] === "/" && t[i + 1] === "/") {
    const f = t.indexOf("\n", i);
    return f === -1 ? t.length : f;
  }
  if (t[i] === "/" && t[i + 1] === "*") {
    const f = t.indexOf("*/", i + 2);
    return f === -1 ? t.length : f + 2;
  }
  return -1;
}

/** Offset subito dopo la `)` che chiude la `(` in `apertura`, o -1 se sbilanciata. */
function chiusuraParentesi(t: string, apertura: number): number {
  let profondita = 0;
  let i = apertura;
  while (i < t.length) {
    const c = t[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === '"') {
      i++;
      while (i < t.length && t[i] !== '"') {
        i += t[i] === "\\" ? 2 : 1;
      }
      i++;
      continue;
    }
    if (c === "`") {
      i = saltaRaw(t, i);
      continue;
    }
    const salto = saltaCommento(t, i);
    if (salto !== -1) {
      i = salto;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") {
      profondita++;
    } else if (c === ")" || c === "]" || c === "}") {
      profondita--;
      if (profondita === 0) {
        return c === ")" ? i + 1 : -1;
      }
    }
    i++;
  }
  return -1;
}

/** Divide il corpo di una chiamata in argomenti (virgole di primo livello). */
function dividiArgomenti(corpo: string): Argomento[] {
  const pezzi: string[] = [];
  let profondita = 0;
  let inizio = 0;
  let i = 0;
  while (i < corpo.length) {
    const c = corpo[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === '"') {
      i++;
      while (i < corpo.length && corpo[i] !== '"') {
        i += corpo[i] === "\\" ? 2 : 1;
      }
      i++;
      continue;
    }
    if (c === "`") {
      i = saltaRaw(corpo, i);
      continue;
    }
    const salto = saltaCommento(corpo, i);
    if (salto !== -1) {
      i = salto;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") {
      profondita++;
    } else if (c === ")" || c === "]" || c === "}") {
      profondita--;
    } else if (c === "," && profondita === 0) {
      pezzi.push(corpo.slice(inizio, i));
      inizio = i + 1;
    }
    i++;
  }
  pezzi.push(corpo.slice(inizio));
  return pezzi
    .map((p) => p.trim())
    .filter((p) => p !== "")
    .map((grezzo) => {
      // I commenti che precedono l'argomento restano nel testo originale ma non nel riconoscimento del nome.
      const senza = grezzo.replace(/^(?:\s*(?:\/\/[^\n]*\n|\/\*[\s\S]*?\*\/))+\s*/, "");
      const m = /^([A-Za-z_][\w-]*)\s*:\s*([\s\S]*)$/.exec(senza);
      return m === null ? { nome: null, valore: senza, grezzo } : { nome: m[1], valore: m[2].trim(), grezzo };
    });
}

/** Prima `#set page(...)` che inizia una riga, fuori da commenti e raw; `null` se manca o è sbilanciata. */
export function trovaRegolaPagina(testo: string): RegolaPagina | null {
  let i = 0;
  while (i < testo.length) {
    if (i === 0 || testo[i - 1] === "\n") {
      INIZIO_REGOLA.lastIndex = i;
      const m = INIZIO_REGOLA.exec(testo);
      if (m !== null) {
        const apertura = i + m[0].length - 1;
        const fine = chiusuraParentesi(testo, apertura);
        if (fine === -1) {
          return null;
        }
        const rientro = /^[ \t]*/.exec(m[0])![0];
        return {
          da: i + rientro.length,
          a: fine,
          rientro,
          args: dividiArgomenti(testo.slice(apertura + 1, fine - 1)),
        };
      }
    }
    const c = testo[i];
    if (c === "`") {
      i = saltaRaw(testo, i);
      continue;
    }
    const salto = saltaCommento(testo, i);
    i = salto !== -1 ? salto : i + 1;
  }
  return null;
}

// ---- Lunghezze ----

const RE_LUNGHEZZA = /^(\d+(?:\.\d+)?)(cm|mm|pt|in|em)$/;

/** `2,5 cm` → `2.5cm`; `null` se non è una lunghezza ammessa. Vuoto → stringa vuota. */
export function normalizzaLunghezza(valore: string): string | null {
  const s = valore.trim().replace(",", ".").replace(/\s+/g, "");
  if (s === "") {
    return "";
  }
  return RE_LUNGHEZZA.test(s) ? s : null;
}

/** Lunghezza in millimetri (per l'anteprima); `em` vale 4,2 mm (≈ 12 pt). */
export function lunghezzaInMm(valore: string): number | null {
  const m = RE_LUNGHEZZA.exec(valore);
  if (m === null) {
    return null;
  }
  const n = Number(m[1]);
  const fattore: Record<string, number> = { cm: 10, mm: 1, pt: 25.4 / 72, in: 25.4, em: 4.2 };
  return n * fattore[m[2]];
}

// ---- Lettura ----

const STATO_VUOTO: StatoPagina = {
  formato: "a4",
  orizzontale: false,
  margineY: "",
  margineX: "",
  margineAvanzato: false,
  colonne: 1,
  intestazione: "",
  intestazioneAvanzata: false,
  numerazione: false,
  numerazioneAvanzata: false,
};

/** Toglie i backslash di escape di un contenuto `[..]` semplice. */
function disescapa(t: string): string {
  return t.replace(/\\(.)/gs, "$1");
}

/** Escapa i caratteri speciali del contenuto Typst (markup) con la barra rovesciata. */
export function escapaContenuto(t: string): string {
  // Anche «/»: una didascalia con `//` o `/*` diventerebbe un commento Typst e romperebbe la riga.
  return t.replace(/[\\#$*_@<>`~[\]"'=/]/g, "\\$&").replace(/\r?\n/g, " ");
}

/** Legge lo stato dei controlli da una regola trovata (o dai valori vuoti se manca). */
export function leggiPagina(regola: RegolaPagina | null): StatoPagina {
  const s: StatoPagina = { ...STATO_VUOTO };
  if (regola === null) {
    return s;
  }
  for (const a of regola.args) {
    const v = a.valore;
    switch (a.nome) {
      case "paper": {
        const m = /^"([^"]*)"$/.exec(v);
        if (m !== null && m[1] in FORMATI) {
          s.formato = m[1];
        }
        break;
      }
      case "flipped":
        s.orizzontale = v === "true";
        break;
      case "margin": {
        const uniforme = normalizzaLunghezza(v);
        if (uniforme !== null && uniforme !== "") {
          s.margineX = uniforme;
          s.margineY = uniforme;
          break;
        }
        const d = /^\(\s*([\s\S]*?)\s*,?\s*\)$/.exec(v);
        const coppie = d === null ? [] : dividiArgomenti(d[1]);
        const ok =
          coppie.length > 0 &&
          coppie.every((c) => (c.nome === "x" || c.nome === "y") && normalizzaLunghezza(c.valore)) &&
          new Set(coppie.map((c) => c.nome)).size === coppie.length;
        if (ok) {
          for (const c of coppie) {
            const l = normalizzaLunghezza(c.valore) as string;
            if (c.nome === "x") s.margineX = l;
            else s.margineY = l;
          }
        } else {
          s.margineAvanzato = true;
        }
        break;
      }
      case "columns": {
        const n = Number(v);
        s.colonne = Number.isInteger(n) && n >= 1 && n <= 12 ? n : 1;
        break;
      }
      case "header": {
        const m = /^\[([\s\S]*)\]$/.exec(v);
        // Semplice = niente altre parentesi quadre, `#` o `$` non escapati.
        if (m !== null && !/(^|[^\\])[[\]#$`]/.test(m[1])) {
          s.intestazione = disescapa(m[1]);
        } else {
          s.intestazioneAvanzata = true;
        }
        break;
      }
      case "numbering":
        if (v === '"1"') {
          s.numerazione = true;
        } else {
          s.numerazioneAvanzata = true;
        }
        break;
      default:
        break;
    }
  }
  return s;
}

/** Legge lo stato direttamente dal testo del documento. */
export function leggiPaginaDaTesto(testo: string): StatoPagina {
  return leggiPagina(trovaRegolaPagina(testo));
}

// ---- Scrittura ----

const GESTITI = ["paper", "flipped", "margin", "columns", "header", "numbering"] as const;

/** Argomenti gestiti da scrivere (in ordine fisso); `null` = rimuovere l'argomento se c'è. */
function argomentiGestiti(s: StatoPagina, esistenti: ReadonlySet<string>): Record<string, string | null> {
  const fuori: Record<string, string | null> = {};
  fuori.paper = esistenti.has("paper") || s.formato !== "a4" ? `"${s.formato}"` : null;
  fuori.flipped = s.orizzontale ? "true" : null;
  if (!s.margineAvanzato) {
    const { margineX: x, margineY: y } = s;
    fuori.margin =
      x === "" && y === ""
        ? null
        : x === y
          ? x
          : `(${[x !== "" ? `x: ${x}` : "", y !== "" ? `y: ${y}` : ""].filter(Boolean).join(", ")})`;
  }
  fuori.columns = s.colonne > 1 ? String(s.colonne) : null;
  if (!s.intestazioneAvanzata) {
    fuori.header = s.intestazione.trim() === "" ? null : `[${escapaContenuto(s.intestazione.trim())}]`;
  }
  if (!s.numerazioneAvanzata) {
    fuori.numbering = s.numerazione ? '"1"' : null;
  }
  return fuori;
}

/** Esito di `scriviPagina`. */
export interface EsitoPagina {
  cambio: Cambio;
  /** Selezione da proporre nel testo risultante: la regola intera. */
  selezione: { da: number; a: number };
}

/**
 * Riscrive la regola nel documento: gli argomenti gestiti prendono i valori dello
 * stato, gli altri restano com'erano. Senza regola esistente e senza nulla da
 * scrivere restituisce `null`; con regola assente ma qualcosa da scrivere la
 * inserisce in cima al documento.
 */
export function scriviPagina(testo: string, stato: StatoPagina): EsitoPagina | null {
  const regola = trovaRegolaPagina(testo);
  const esistenti = new Set((regola?.args ?? []).map((a) => a.nome).filter((n): n is string => n !== null));
  const nuovi = argomentiGestiti(stato, esistenti);
  const righe: string[] = [];
  const scritti = new Set<string>();
  for (const a of regola?.args ?? []) {
    if (a.nome !== null && (GESTITI as readonly string[]).includes(a.nome) && a.nome in nuovi) {
      scritti.add(a.nome);
      if (nuovi[a.nome] !== null) {
        righe.push(`${a.nome}: ${nuovi[a.nome]}`);
      }
    } else {
      righe.push(a.grezzo);
    }
  }
  for (const nome of GESTITI) {
    if (!scritti.has(nome) && nuovi[nome] !== null && nuovi[nome] !== undefined) {
      righe.push(`${nome}: ${nuovi[nome]}`);
    }
  }
  if (regola === null && righe.length === 0) {
    return null;
  }
  const rientro = regola?.rientro ?? "";
  const unaRiga = righe.length <= 2 && righe.every((r) => !r.includes("\n"));
  const regolaTesto = unaRiga
    ? `#set page(${righe.join(", ")})`
    : `#set page(\n${righe.map((r) => `${rientro}  ${r},`).join("\n")}\n${rientro})`;
  if (regola !== null) {
    return {
      cambio: { da: regola.da, a: regola.a, testo: regolaTesto },
      selezione: { da: regola.da, a: regola.da + regolaTesto.length },
    };
  }
  return {
    cambio: { da: 0, a: 0, testo: `${regolaTesto}\n\n` },
    selezione: { da: 0, a: regolaTesto.length },
  };
}

// ---- Anteprima ----

/** Geometria del foglio in scala per l'anteprima (percentuali di larghezza/altezza). */
export interface GeometriaFoglio {
  /** Rapporto larghezza/altezza della pagina. */
  rapporto: number;
  alto: number;
  basso: number;
  sinistra: number;
  destra: number;
}

/** Calcola la geometria del foglio; i margini vuoti valgono il predefinito di Typst. */
export function geometriaFoglio(s: StatoPagina): GeometriaFoglio {
  const [l0, h0] = FORMATI[s.formato] ?? FORMATI.a4;
  const [l, h] = s.orizzontale ? [h0, l0] : [l0, h0];
  const mmY = lunghezzaInMm(s.margineY) ?? MARGINE_PREDEFINITO_MM;
  const mmX = lunghezzaInMm(s.margineX) ?? MARGINE_PREDEFINITO_MM;
  const limita = (v: number): number => Math.max(0, Math.min(45, v));
  return {
    rapporto: l / h,
    alto: limita((mmY / h) * 100),
    basso: limita((mmY / h) * 100),
    sinistra: limita((mmX / l) * 100),
    destra: limita((mmX / l) * 100),
  };
}
