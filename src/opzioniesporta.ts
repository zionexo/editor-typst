/**
 * opzioniesporta.ts — Logica pura delle opzioni di esportazione (F5-011).
 *
 * Nessuna dipendenza da DOM o da Tauri, così è verificabile da
 * `opzioniesporta.check.ts` (stesso schema di `debounce.ts`). L'intervallo di
 * pagine NON si analizza qui: lo valida il core (`analizza_pagine`), unica
 * fonte di verità; questo modulo lo passa così com'è.
 */

/**
 * Formati di esportazione offerti dall'app: PDF/PNG/SVG passano dalla
 * compilazione; txt/html/tex/md sono **conversioni** dal sorgente Typst
 * (`esporta_testo`, `esporta_html`, `esporta_latex`, `esporta_markdown`).
 */
export type FormatoEsportazione = "pdf" | "png" | "svg" | "txt" | "html" | "tex" | "md";

/** Formati ottenuti per conversione del sorgente (non dalla compilazione). */
export type FormatoConversione = "txt" | "html" | "tex" | "md";

/** `true` per i formati ottenuti per conversione del sorgente. */
export function eConversione(formato: FormatoEsportazione): formato is FormatoConversione {
  return formato === "txt" || formato === "html" || formato === "tex" || formato === "md";
}

/** Nome esteso del formato, per titoli e messaggi. */
export const NOME_FORMATO: Record<FormatoEsportazione, string> = {
  pdf: "PDF",
  png: "PNG",
  svg: "SVG",
  txt: "testo semplice",
  html: "HTML",
  tex: "LaTeX",
  md: "Markdown",
};

/** Larghezza di a capo del testo semplice: predefinita e limiti (come il core). */
export const LARGHEZZA_TESTO_PREDEFINITA = 80;
export const LARGHEZZA_TESTO_MIN = 20;
export const LARGHEZZA_TESTO_MAX = 500;

/** Classi e formati carta LaTeX accettati (allineati al comando `esporta_latex`). */
export const CLASSI_LATEX = ["article", "report", "book"] as const;
export const CARTE_LATEX = ["a4paper", "a5paper", "letterpaper"] as const;
export type ClasseLatex = (typeof CLASSI_LATEX)[number];
export type CartaLatex = (typeof CARTE_LATEX)[number];

/** Standard PDF offerti dalla UI (D-011); il core ne supporta altri. */
export type StandardPdfUi = "nessuno" | "a-2b" | "a-3b" | "a-4";

/** Risoluzione predefinita del PNG (come typst-cli e `PPI_PREDEFINITO` del core). */
export const PPI_PREDEFINITO = 144;
/** Risoluzione massima (allineata a `PPI_MASSIMO` del core, che comunque rivalida). */
export const PPI_MASSIMO = 1200;

/** Scelta di standard mostrata nella modale, con il testo d'aiuto (fase8.md §3.4). */
export const STANDARD_UI: readonly { valore: StandardPdfUi; etichetta: string; aiuto: string }[] = [
  { valore: "nessuno", etichetta: "Nessuno (predefinito)", aiuto: "PDF standard, con struttura per l'accessibilità (tag)." },
  { valore: "a-2b", etichetta: "PDF/A-2b", aiuto: "Archiviazione a lungo termine (basato su PDF 1.7)." },
  {
    valore: "a-3b",
    etichetta: "PDF/A-3b",
    aiuto: "Come PDF/A-2b (PDF 1.7), ma permette di allegare file, ad esempio i sorgenti.",
  },
  { valore: "a-4", etichetta: "PDF/A-4", aiuto: "Edizione più recente, basata su PDF 2.0." },
];

/** Opzioni raccolte dalla modale. */
export interface OpzioniEsportazione {
  /** Intervallo di pagine (es. `1-3, 5`); `null` = tutte. */
  pagine: string | null;
  /** Standard PDF (solo per il formato PDF). */
  standard: StandardPdfUi;
  /** Risoluzione del PNG in PPI (solo per il formato PNG). */
  ppi: number;
  /** Colonne di a capo del testo semplice (solo `txt`). */
  larghezza: number;
  /** HTML autonomo: immagini incorporate come data-URI (solo `html`). */
  htmlAutonomo: boolean;
  /** Classe del documento LaTeX (solo `tex`). */
  latexClasse: ClasseLatex;
  /** Formato carta LaTeX (solo `tex`). */
  latexCarta: CartaLatex;
}

/** Valori predefiniti delle opzioni specifiche delle conversioni. */
export const OPZIONI_CONVERSIONE_PREDEFINITE = {
  larghezza: LARGHEZZA_TESTO_PREDEFINITA,
  htmlAutonomo: false,
  latexClasse: "article" as ClasseLatex,
  latexCarta: "a4paper" as CartaLatex,
};

/** Riporta un valore qualunque a una classe LaTeX offerta (altrimenti `article`). */
export function classeLatexValida(v: unknown): ClasseLatex {
  return CLASSI_LATEX.find((c) => c === v) ?? "article";
}

/** Riporta un valore qualunque a un formato carta offerto (altrimenti `a4paper`). */
export function cartaLatexValida(v: unknown): CartaLatex {
  return CARTE_LATEX.find((c) => c === v) ?? "a4paper";
}

/**
 * Legge la larghezza del testo semplice: intero da 20 a 500, vuoto = predefinita,
 * altrimenti `null`.
 */
export function leggiLarghezza(testo: string): number | null {
  const t = testo.trim();
  if (t === "") {
    return LARGHEZZA_TESTO_PREDEFINITA;
  }
  if (!/^\d+$/.test(t)) {
    return null;
  }
  const n = Number(t);
  return n >= LARGHEZZA_TESTO_MIN && n <= LARGHEZZA_TESTO_MAX ? n : null;
}

/** Riporta un valore qualunque (es. da `impostazioni.json`) a uno standard offerto dalla UI. */
export function standardValido(valore: unknown): StandardPdfUi {
  return STANDARD_UI.some((s) => s.valore === valore) ? (valore as StandardPdfUi) : "nessuno";
}

/**
 * Legge la risoluzione dal campo di testo: intero da 1 a `PPI_MASSIMO`,
 * altrimenti `null`. Testo vuoto = valore predefinito.
 */
export function leggiPpi(testo: string): number | null {
  const t = testo.trim();
  if (t === "") {
    return PPI_PREDEFINITO;
  }
  if (!/^\d+$/.test(t)) {
    return null;
  }
  const n = Number(t);
  return n >= 1 && n <= PPI_MASSIMO ? n : null;
}

/** Intervallo di pagine da inviare al backend: `null` se vuoto o solo spazi. */
export function pagineOppureNull(testo: string): string | null {
  const t = testo.trim();
  return t === "" ? null : t;
}

/** Estensione del file per il formato. */
export function estensione(formato: FormatoEsportazione): string {
  return formato;
}

/**
 * Nome file suggerito nel dialog di salvataggio: deriva dal file corrente
 * (estensione sostituita), altrimenti un nome generico.
 */
export function nomePredefinito(percorso: string | null, formato: FormatoEsportazione): string {
  const ext = estensione(formato);
  if (!percorso) {
    return `documento.${ext}`;
  }
  // Un `.md` esportato da un sorgente `.md` coinciderebbe col documento: il
  // backend lo rifiuterebbe, quindi si suggerisce un nome diverso.
  const base = percorso.replace(/\.(typ|md)$/i, "");
  if (base === percorso) {
    // Estensione diversa da .typ/.md (es. `nota.txt`, `bozza`): si AGGIUNGE
    // l'estensione, mai si sostituisce, altrimenti si proporrebbe il sorgente
    // stesso (`nota.txt` esportato come txt).
    return `${percorso}.${ext}`;
  }
  return formato === "md" && /\.md$/i.test(percorso) ? `${base}-esportato.md` : `${base}.${ext}`;
}

/** Filtri del dialogo «Salva» per formato (estensione corretta). */
export function filtriSalvataggio(formato: FormatoEsportazione): { name: string; extensions: string[] }[] {
  const nomi: Record<FormatoEsportazione, string> = {
    pdf: "PDF",
    png: "PNG",
    svg: "SVG",
    txt: "Testo semplice",
    html: "HTML",
    tex: "LaTeX",
    md: "Markdown",
  };
  return [{ name: nomi[formato], extensions: [estensione(formato)] }];
}

/** Esito del comando di conversione (struttura `EsitoConversione` del guscio). */
export interface EsitoConversione {
  percorso: string;
  avvisi: string[];
  errori_sintassi: boolean;
}

/** Nome del comando Tauri che esporta il formato di conversione. */
export function comandoConversione(formato: FormatoConversione): string {
  const comandi: Record<FormatoConversione, string> = {
    txt: "esporta_testo",
    html: "esporta_html",
    tex: "esporta_latex",
    md: "esporta_markdown",
  };
  return comandi[formato];
}

/** Argomenti specifici del formato per il comando di conversione (oltre a percorso e sorgente). */
export function argomentiConversione(
  formato: FormatoConversione,
  o: Pick<OpzioniEsportazione, "larghezza" | "htmlAutonomo" | "latexClasse" | "latexCarta">,
): Record<string, unknown> {
  switch (formato) {
    case "txt":
      return { larghezza: o.larghezza };
    case "html":
      return { autonomo: o.htmlAutonomo };
    case "tex":
      return { classe: o.latexClasse, carta: o.latexCarta };
    case "md":
      return {};
  }
}

/** Numero massimo di avvisi mostrati per esteso. */
export const MAX_AVVISI_MOSTRATI = 6;

/**
 * Testo del riquadro di avviso dopo una conversione: elenco compatto (al più
 * `MAX_AVVISI_MOSTRATI`, poi «e altri N») preceduto, se il sorgente ha errori di
 * sintassi, da un avviso esplicito. `null` se non c'è nulla da dire.
 */
export function testoAvvisiConversione(formato: FormatoEsportazione, esito: EsitoConversione): string | null {
  const righe: string[] = [];
  if (esito.errori_sintassi) {
    righe.push("Il sorgente contiene errori di sintassi: la conversione è approssimata.");
  }
  // L'avviso del core sugli errori di sintassi è già reso sopra.
  const altri = esito.avvisi.filter((a) => !(esito.errori_sintassi && a.includes("errori di sintassi")));
  for (const a of altri.slice(0, MAX_AVVISI_MOSTRATI)) {
    righe.push(`• ${a}`);
  }
  if (altri.length > MAX_AVVISI_MOSTRATI) {
    righe.push(`… e altri ${altri.length - MAX_AVVISI_MOSTRATI}`);
  }
  if (righe.length === 0) {
    return null;
  }
  return `Esportazione ${NOME_FORMATO[formato]}: ${altri.length === 0 ? "attenzione" : `${altri.length} avvisi sulla conversione`}\n${righe.join("\n")}`;
}

/** Struttura `OpzioniPdf` attesa dal comando `esporta_pdf` (vedi `esporta.rs` nel core). */
export interface OpzioniPdfBackend {
  standard: string[];
  pagine: string | null;
}

/** Converte le scelte della modale nelle opzioni PDF del backend. */
export function opzioniPdfPerBackend(o: Pick<OpzioniEsportazione, "pagine" | "standard">): OpzioniPdfBackend {
  return {
    standard: o.standard === "nessuno" ? [] : [o.standard],
    pagine: pagineOppureNull(o.pagine ?? ""),
  };
}

/** Testo di stato a export riuscito. */
export function messaggioEsito(formato: FormatoEsportazione, file: number): string {
  if (formato === "pdf") {
    return "PDF esportato";
  }
  if (eConversione(formato)) {
    return `Esportato come ${NOME_FORMATO[formato]}`;
  }
  const nome = formato.toUpperCase();
  return file === 1 ? `${nome} esportato` : `${file} file ${nome} esportati`;
}

/** Testo della richiesta di conferma sovrascrittura: al più 5 nomi, poi "e altri N". */
export function testoConflitto(esistenti: string[]): string {
  const MAX = 5;
  const elenco = esistenti.slice(0, MAX).join("\n");
  const resto = esistenti.length > MAX ? `\ne altri ${esistenti.length - MAX}` : "";
  return `Questi file esistono già e verranno sovrascritti:\n${elenco}${resto}`;
}

/**
 * Testo di un errore di esportazione (stringa, array di stringhe, `Error`) con
 * il prefisso del formato: il backend può rifiutare la destinazione (documento
 * aperto, estensione `.typ`/`.md`) e il motivo deve arrivare all'utente così com'è.
 */
export function testoErroreEsportazione(formato: FormatoEsportazione, errore: unknown): string {
  let motivo: string;
  if (Array.isArray(errore)) {
    motivo = errore.map(String).join("\n");
  } else if (typeof errore === "string") {
    motivo = errore;
  } else if (errore instanceof Error) {
    motivo = errore.message;
  } else {
    motivo = "errore sconosciuto";
  }
  motivo = motivo.trim() === "" ? "errore sconosciuto" : motivo.trim();
  const nome = eConversione(formato) ? NOME_FORMATO[formato] : formato.toUpperCase();
  return `Esportazione ${nome} non riuscita: ${motivo}`;
}
