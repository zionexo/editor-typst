/**
 * tavolozza-typst.ts — Colori di sintassi del Classico scuro per i documenti Typst.
 *
 * Perché esiste: `TypstHighlightSytle` di `codemirror-lang-typst` usa i colori dell'app web di
 * Typst, pensati per il fondo chiaro (sul fondo di `oneDark`, #282c34, scendono a 2,1–3,1:1).
 * Per il Classico scuro l'editor applica quindi questa tavolozza (pastelli Catppuccin Mocha,
 * già usati dall'interfaccia); il Classico chiaro conserva `TypstHighlightSytle` (4,6–6,7:1 su
 * bianco). Dati puri, senza CodeMirror: verificati da `stili.check.ts` (WCAG AA, 4,5:1).
 */

/** Fondo dell'editor nel Classico scuro (`oneDark`). */
export const SFONDO_ONEDARK = "#282c34";

/** Colori usati dalla mappa di evidenziazione (valori CSS: `#rrggbb` o `var(--...)`). */
export interface ColoriSintassi {
  titolo: string;
  funzione: string;
  stringa: string;
  numero: string;
  commento: string;
  math: string;
  rif: string;
  tenue: string;
  errore: string;
}

/** Tavolozza del Classico scuro (tutti `#rrggbb`: il check ne misura il contrasto su `SFONDO_ONEDARK`). */
export const COLORI_CLASSICO_SCURO: Readonly<ColoriSintassi> = {
  titolo: "#89b4fa",
  funzione: "#cba6f7",
  stringa: "#a6e3a1",
  numero: "#fab387",
  commento: "#9399b2",
  math: "#94e2d5",
  rif: "#f38ba8",
  tenue: "#a6adc8",
  errore: "#f38ba8",
};

/** Tavolozza degli stili «composizione»/«strumento»: variabili CSS `--sint-*` dello stile attivo. */
export const COLORI_DA_VARIABILI: Readonly<ColoriSintassi> = {
  titolo: "var(--sint-titolo)",
  funzione: "var(--sint-funzione)",
  stringa: "var(--sint-stringa)",
  numero: "var(--sint-numero)",
  commento: "var(--sint-commento)",
  math: "var(--sint-math)",
  rif: "var(--sint-rif)",
  tenue: "var(--testo-tenue)",
  errore: "var(--errore)",
};
