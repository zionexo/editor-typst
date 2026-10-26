/**
 * modalita.ts — Modalità di scrittura dell'editor: focus e macchina da scrivere (F5-023).
 *
 * Due estensioni CodeMirror 6 indipendenti, ciascuna in un `Compartment` di
 * `editor.ts` (si accendono e spengono a caldo senza toccare testo, cursore o
 * selezione):
 * - **focus** ([`estensioneFocus`]): tutte le righe visibili fuori dal paragrafo
 *   del cursore sono schiarite (`Decoration.line` con classe `cm-sfocato`),
 *   ricalcolate su `docChanged | selectionSet | viewportChanged`. Scritta in
 *   casa; il paragrafo lo individua `paragrafo.ts`.
 * - **macchina da scrivere** ([`estensioneMacchina`]): la riga del cursore resta
 *   a metà altezza dell'editor. Il padding sopra e sotto il contenuto (via
 *   `element.style`, compatibile con la CSP di produzione) permette anche alla
 *   prima e all'ultima riga di centrarsi; lo scorrimento avviene solo per gli
 *   eventi utente che spostano il cursore scrivendo o con la tastiera (non col
 *   mouse), con una guardia contro il rientro.
 *
 * Nessun effetto su anteprima e sincronizzazione: le estensioni non modificano
 * il documento né la selezione.
 */

// Derivato da deathau/cm-typewriter-scroll-obsidian extension.ts (625b42f), licenza MIT — Copyright (c) deathau

import { Facet, RangeSetBuilder, Transaction, type Extension } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { intervalloParagrafo } from "./paragrafo";

// ---------------------------------------------------------------- focus

const rigaSfocata = Decoration.line({ class: "cm-sfocato" });

/** Decorazioni delle righe visibili fuori dal paragrafo del cursore principale. */
function decorazioniFocus(view: EditorView): DecorationSet {
  const doc = view.state.doc;
  const rigaCursore = doc.lineAt(view.state.selection.main.head).number;
  const { da, a } = intervalloParagrafo((n) => doc.line(n).text, doc.lines, rigaCursore);
  const costruttore = new RangeSetBuilder<Decoration>();
  for (const { from, to } of view.visibleRanges) {
    let pos = from;
    while (pos <= to) {
      const riga = doc.lineAt(pos);
      if (riga.number < da || riga.number > a) {
        costruttore.add(riga.from, riga.from, rigaSfocata);
      }
      pos = riga.to + 1;
    }
  }
  return costruttore.finish();
}

/** Modalità focus: schiarisce tutto tranne il paragrafo del cursore. */
export function estensioneFocus(): Extension {
  return [
    ViewPlugin.fromClass(
      class {
        decorations: DecorationSet;
        constructor(view: EditorView) {
          this.decorations = decorazioniFocus(view);
        }
        update(update: ViewUpdate): void {
          if (update.docChanged || update.selectionSet || update.viewportChanged) {
            this.decorations = decorazioniFocus(update.view);
          }
        }
      },
      { decorations: (v) => v.decorations },
    ),
    EditorView.baseTheme({
      ".cm-line.cm-sfocato": { opacity: "0.35" },
    }),
  ];
}

// ------------------------------------------------------ macchina da scrivere

/** Eventi utente che fanno scorrere: scrittura, cancellazione, annulla/ripeti e selezione da tastiera. */
const eventiConsentiti = /^(select|input|delete|undo|redo)(\..+)?$/;
/** La selezione col mouse non fa scorrere (sposterebbe il testo sotto il puntatore). */
const eventiVietati = /^(select.pointer)$/;

/** Posizione verticale della riga del cursore, in frazione dell'altezza dell'editor (0.5 = metà). */
const offsetMacchina = Facet.define<number, number>({
  combine: (valori) => (valori.length > 0 ? Math.min(...valori) : 0.5),
});

/** Distanza (px) dal bordo alto a cui portare la riga del cursore per centrarla. */
function margineCentro(view: EditorView): number {
  return view.dom.clientHeight * view.state.facet(offsetMacchina) - view.defaultLineHeight / 2;
}

/**
 * Porta la testa della selezione principale a metà altezza. `false` (senza
 * inviare nulla) se la vista non è più agganciata al documento.
 */
export function centraSuTesta(view: EditorView): boolean {
  if (!view.dom.isConnected) {
    return false;
  }
  const testa = view.state.selection.main.head;
  view.dispatch({ effects: EditorView.scrollIntoView(testa, { y: "start", yMargin: margineCentro(view) }) });
  return true;
}

/** Padding sopra e sotto il contenuto, così anche prima e ultima riga si centrano. */
const paddingMacchina = ViewPlugin.fromClass(
  class {
    constructor(private view: EditorView) {
      this.applica();
    }
    update(_update: ViewUpdate): void {
      this.applica();
    }
    applica(): void {
      const sopra = margineCentro(this.view);
      const valore = `${sopra}px`;
      if (this.view.contentDOM.style.paddingTop !== valore) {
        this.view.contentDOM.style.paddingTop = valore;
        this.view.contentDOM.style.paddingBottom = `${this.view.dom.clientHeight - sopra}px`;
      }
    }
    // Spegnendo la modalità il padding va tolto (l'editor torna com'era).
    destroy(): void {
      this.view.contentDOM.style.paddingTop = "";
      this.view.contentDOM.style.paddingBottom = "";
    }
  },
);

/** Scorrimento: dopo un evento consentito, la riga del cursore torna a metà altezza. */
const scorrimentoMacchina = ViewPlugin.fromClass(
  class {
    /** `true` fra il nostro dispatch e il suo `update`: evita di rispondere a noi stessi. */
    private mioAggiornamento = false;
    private distrutto = false;
    constructor(private view: EditorView) {}
    update(update: ViewUpdate): void {
      if (this.mioAggiornamento) {
        this.mioAggiornamento = false;
        return;
      }
      const eventi = update.transactions.map((tr) => tr.annotation(Transaction.userEvent));
      const consentito =
        eventi.length > 0 &&
        eventi.every((e) => e !== undefined && eventiConsentiti.test(e) && !eventiVietati.test(e));
      if (consentito) {
        this.centra(update.startState.selection.main.head);
      }
    }
    centra(testaPrima: number): void {
      // Non si può inviare una transazione dentro un `update`: si aspetta il frame successivo.
      window.requestAnimationFrame(() => {
        if (this.distrutto) {
          return;
        }
        const stato = this.view.state;
        if (stato.selection.ranges.length === 1 && stato.selection.main.head !== testaPrima) {
          // La guardia si arma solo se la transazione parte davvero: altrimenti
          // ingoierebbe il prossimo evento dell'utente.
          this.mioAggiornamento = true;
          if (!centraSuTesta(this.view)) {
            this.mioAggiornamento = false;
          }
        }
      });
    }
    destroy(): void {
      this.distrutto = true;
    }
  },
);

/**
 * Modalità macchina da scrivere. Va messa in un `Compartment`: riconfigurandolo
 * a `[]` il padding viene tolto. `offset` = posizione della riga (0.5 = metà).
 */
export function estensioneMacchina(offset = 0.5): Extension {
  return [offsetMacchina.of(offset), paddingMacchina, scorrimentoMacchina];
}
