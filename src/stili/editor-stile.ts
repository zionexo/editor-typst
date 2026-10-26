/**
 * editor-stile.ts — Tema CodeMirror per gli stili «composizione» e «strumento».
 *
 * Il «classico» NON passa di qui: continua a usare il tema chiaro predefinito di
 * CodeMirror e `oneDark` (editor.ts), così il suo aspetto resta identico. Per gli
 * altri due stili il tema legge SOLO variabili CSS (`--sfondo`, `--testo`,
 * `--sint-*`, …) definite in `src/stili/*.css` come `light-dark(chiaro, scuro)`:
 * il cambio di tema chiaro/scuro e di stile avviene quindi via CSS, e questo
 * modulo serve soltanto a (a) sostituire i colori di `oneDark` e (b) mappare i
 * tag di evidenziazione sui token `--sint-*`, con i contrasti WCAG AA dichiarati
 * nelle proposte. Nessuno stile inline: CodeMirror inietta fogli di stile, già
 * ammessi dalla CSP di produzione (`style-src 'self' 'unsafe-inline'`, B-005).
 */
import { HighlightStyle, syntaxHighlighting, type TagStyle } from "@codemirror/language";
import { Prec, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags as t, type Tag } from "@lezer/highlight";
import { COLORI_CLASSICO_SCURO, COLORI_DA_VARIABILI, type ColoriSintassi } from "./tavolozza-typst";

/** Tag personalizzati di `codemirror-lang-typst` (`typstTags`): istanze diverse per ciascun motore. */
export interface TagTypst {
  mathDelimiter: Tag;
  listMarker: Tag;
  interpolated: Tag;
}

/**
 * Mappa dei tag (Typst e Markdown) sui colori di sintassi. Con `tagTypst` aggiunge i tag
 * propri di Typst (delimitatori di formula, marcatori di lista, interpolazioni `#`), che
 * `@lezer/highlight` non conosce: le istanze vengono dal motore caricato (wasm o lezer).
 */
function creaEvidenziazione(c: ColoriSintassi, tagTypst?: TagTypst): HighlightStyle {
  const regole: TagStyle[] = [
    { tag: [t.heading, t.heading1, t.heading2, t.heading3, t.heading4], color: c.titolo, fontWeight: "700" },
    { tag: t.strong, fontWeight: "700" },
    { tag: t.emphasis, fontStyle: "italic" },
    { tag: [t.comment, t.lineComment, t.blockComment], color: c.commento, fontStyle: "italic" },
    {
      tag: [t.controlKeyword, t.definitionKeyword, t.moduleKeyword, t.operatorKeyword, t.keyword, t.null, t.atom],
      color: c.funzione,
    },
    { tag: [t.variableName, t.function(t.variableName), t.special(t.variableName)], color: c.funzione },
    { tag: [t.string, t.special(t.string), t.quote], color: c.stringa },
    // Typst: escape, etichette, riferimenti ed operatori di formula condividono `t.escape`.
    { tag: [t.escape, t.labelName, t.annotation], color: c.rif },
    { tag: [t.number, t.integer, t.float, t.bool, t.literal], color: c.numero },
    { tag: [t.link, t.url], color: c.titolo, textDecoration: "underline" },
    { tag: [t.monospace, t.processingInstruction, t.propertyName], color: c.math },
    { tag: [t.list, t.definitionOperator], color: c.numero },
    {
      tag: [
        t.operator,
        t.arithmeticOperator,
        t.compareOperator,
        t.updateOperator,
        t.controlOperator,
        t.typeOperator,
        t.punctuation,
        t.separator,
        t.brace,
        t.bracket,
        t.paren,
        t.contentSeparator,
        t.meta,
        t.documentMeta,
      ],
      color: c.tenue,
    },
    { tag: t.invalid, color: c.errore, textDecoration: "underline wavy" },
  ];
  if (tagTypst !== undefined) {
    regole.push(
      { tag: tagTypst.mathDelimiter, color: c.math },
      { tag: tagTypst.listMarker, color: c.numero },
      { tag: tagTypst.interpolated, color: c.funzione },
    );
  }
  return HighlightStyle.define(regole);
}

const evidenziazione = creaEvidenziazione(COLORI_DA_VARIABILI);

/**
 * Evidenziazione dei tag di Typst, da mettere in un compartment separato perché dipende dal
 * motore caricato (import dinamico) e dal tema:
 * - stili non classici: la mappa completa su `--sint-*` più i tag di Typst;
 * - Classico scuro: la tavolozza `COLORI_CLASSICO_SCURO` (i colori di `TypstHighlightSytle`
 *   sono illeggibili sul fondo di `oneDark`);
 * - Classico chiaro: nessuna estensione (`TypstHighlightSytle` è già leggibile su bianco).
 * `Prec.highest`: deve prevalere sullo stile del pacchetto e su `oneDark`.
 */
export function estensioniEvidenziazioneTypst(tagTypst: TagTypst, classico: boolean, scuro: boolean): Extension {
  if (classico && !scuro) {
    return [];
  }
  const colori = classico ? COLORI_CLASSICO_SCURO : COLORI_DA_VARIABILI;
  return Prec.highest(syntaxHighlighting(creaEvidenziazione(colori, tagTypst)));
}

/**
 * Estensioni dell'editor per uno stile non classico. `scuro` imposta solo il flag
 * `dark` del tema base di CodeMirror (cursore, selezione di default): i colori
 * veri vengono dalle variabili CSS.
 */
export function estensioniStile(scuro: boolean): Extension {
  const tema = EditorView.theme(
    {
      "&": { color: "var(--testo)", backgroundColor: "var(--sfondo)" },
      ".cm-content": { caretColor: "var(--accento)" },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accento)" },
      "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection":
        { backgroundColor: "var(--rilievo)" },
      ".cm-activeLine": { backgroundColor: "color-mix(in oklab, var(--pannello) 70%, transparent)" },
      ".cm-gutters": {
        backgroundColor: "var(--pannello)",
        color: "var(--testo-tenue)",
        border: "none",
        borderRight: "1px solid var(--bordo)",
      },
      ".cm-activeLineGutter": { backgroundColor: "var(--bordo)", color: "var(--accento)" },
      ".cm-lineNumbers .cm-gutterElement": { color: "inherit" },
      ".cm-foldPlaceholder": { backgroundColor: "var(--bordo)", color: "var(--testo-tenue)", border: "none" },
      ".cm-tooltip": {
        backgroundColor: "var(--pannello)",
        color: "var(--testo)",
        border: "1px solid var(--bordo)",
      },
      ".cm-tooltip .cm-tooltip-arrow:before, .cm-tooltip .cm-tooltip-arrow:after": { borderTopColor: "var(--bordo)", borderBottomColor: "var(--bordo)" },
      ".cm-tooltip-autocomplete > ul > li[aria-selected]": {
        backgroundColor: "var(--accento)",
        color: "var(--su-accento)",
      },
      ".cm-panels": { backgroundColor: "var(--pannello)", color: "var(--testo)" },
      ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--bordo)" },
      ".cm-panels.cm-panels-bottom": { borderTop: "1px solid var(--bordo)" },
      ".cm-searchMatch": { backgroundColor: "var(--giallo)", color: "var(--su-giallo)" },
      ".cm-searchMatch.cm-searchMatch-selected": { outline: "2px solid var(--fuoco)" },
      ".cm-selectionMatch": { backgroundColor: "var(--bordo)" },
      "&.cm-focused .cm-matchingBracket": { backgroundColor: "var(--bordo)", outline: "1px solid var(--controllo)" },
    },
    { dark: scuro },
  );
  return [tema, syntaxHighlighting(evidenziazione)];
}
