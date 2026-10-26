/**
 * formattazione.ts — Comandi di formattazione per l'editor (Fase 2).
 *
 * Trasformazioni CodeMirror 6 pure che inseriscono il markup Typst corretto,
 * così chi non conosce la sintassi può formattare dalla toolbar. Ogni comando
 * opera su una `EditorView` e ne preserva/aggiorna la selezione in modo idiomatico
 * (`changeByRange`), supportando anche selezioni multiple.
 *
 * Il modulo non conosce il DOM: è invocato da `editor.ts`, che incapsula la view.
 */

import { EditorView } from "@codemirror/view";
import { EditorSelection } from "@codemirror/state";

/** Comandi di formattazione esposti alla toolbar. */
export type ComandoFormattazione =
  | "grassetto"
  | "corsivo"
  | "codice"
  | "titolo"
  | "lista"
  | "lista-numerata"
  | "link"
  | "immagine"
  | "tabella";

/** Template di tabella inserito dal comando "tabella" (cursore nella prima cella). */
const TEMPLATE_TABELLA = `#table(
  columns: 2,
  [], [],
  [], [],
)`;

/**
 * Avvolge ogni selezione con i marcatori `prima`/`dopo`.
 * Con selezione vuota inserisce i marcatori e mette il cursore in mezzo;
 * con testo selezionato lo ri-seleziona tra i marcatori.
 */
function avvolgi(view: EditorView, prima: string, dopo: string): void {
  const spec = view.state.changeByRange((range) => {
    const testo = view.state.sliceDoc(range.from, range.to);
    const inizio = range.from + prima.length;
    return {
      changes: { from: range.from, to: range.to, insert: prima + testo + dopo },
      range: range.empty
        ? EditorSelection.cursor(inizio)
        : EditorSelection.range(inizio, inizio + testo.length),
    };
  });
  view.dispatch({ ...spec, scrollIntoView: true });
  view.focus();
}

/**
 * Aggiunge `prefisso` all'inizio di ogni riga toccata dalla selezione
 * (o della riga corrente se la selezione è vuota). La selezione si rimappa
 * automaticamente attraverso le modifiche.
 */
function prefiggiRighe(view: EditorView, prefisso: string): void {
  const { state } = view;
  const righe = new Set<number>();
  for (const range of state.selection.ranges) {
    const da = state.doc.lineAt(range.from).number;
    let a = state.doc.lineAt(range.to).number;
    // Se la selezione (non vuota) termina esattamente all'inizio di una riga,
    // quella riga non è realmente toccata: va esclusa per non prefissarla.
    if (a > da && range.to === state.doc.line(a).from) {
      a -= 1;
    }
    for (let n = da; n <= a; n++) {
      righe.add(n);
    }
  }
  const changes = [...righe]
    .sort((x, y) => x - y)
    .map((n) => ({ from: state.doc.line(n).from, insert: prefisso }));
  view.dispatch({ changes, scrollIntoView: true });
  view.focus();
}

/**
 * Inserisce un blocco di `testo` sostituendo la selezione PRIMARIA, mettendo il
 * cursore a `offsetCursore` caratteri dall'inizio dell'inserimento (default: alla
 * fine). Opera sulla sola selezione primaria: per blocchi come tabelle/immagini
 * inserire una copia per ciascuna selezione multipla non avrebbe senso.
 */
function inserisci(view: EditorView, testo: string, offsetCursore?: number): void {
  const range = view.state.selection.main;
  const offset = offsetCursore ?? testo.length;
  view.dispatch({
    changes: { from: range.from, to: range.to, insert: testo },
    selection: EditorSelection.cursor(range.from + offset),
    scrollIntoView: true,
  });
  view.focus();
}

/**
 * Inserisce un collegamento `#link("url")[etichetta]` e posiziona il cursore
 * dentro le virgolette dell'URL. Se c'è testo selezionato diventa l'etichetta;
 * con selezione vuota l'etichetta resta vuota (`[]`).
 */
function inserisciLink(view: EditorView): void {
  const spec = view.state.changeByRange((range) => {
    const testo = view.state.sliceDoc(range.from, range.to);
    return {
      changes: { from: range.from, to: range.to, insert: `#link("")[${testo}]` },
      // Cursore dentro le virgolette: '#link("'.length === 7.
      range: EditorSelection.cursor(range.from + 7),
    };
  });
  view.dispatch({ ...spec, scrollIntoView: true });
  view.focus();
}

/**
 * Esegue un comando di formattazione sulla view indicata.
 *
 * @param view    editor su cui applicare la formattazione.
 * @param comando comando da eseguire.
 */
export function eseguiComando(view: EditorView, comando: ComandoFormattazione): void {
  switch (comando) {
    case "grassetto":
      avvolgi(view, "*", "*");
      break;
    case "corsivo":
      avvolgi(view, "_", "_");
      break;
    case "codice":
      avvolgi(view, "`", "`");
      break;
    case "titolo":
      prefiggiRighe(view, "= ");
      break;
    case "lista":
      prefiggiRighe(view, "- ");
      break;
    case "lista-numerata":
      prefiggiRighe(view, "+ ");
      break;
    case "link":
      inserisciLink(view);
      break;
    case "immagine":
      // Cursore dentro le virgolette del percorso: '#image("'.length === 8.
      inserisci(view, `#image("")`, 8);
      break;
    case "tabella":
      // Cursore nella prima cella vuota del template.
      inserisci(view, TEMPLATE_TABELLA, TEMPLATE_TABELLA.indexOf("[]") + 1);
      break;
  }
}
