/**
 * assistenza.ts — Assistenza alla scrittura (Fase 2): autocompletamento e hover.
 *
 * Fa da ponte fra CodeMirror 6 e i comandi backend `completa`/`tooltip`, che
 * usano il crate `typst-ide` embeddato (nessun sidecar). Espone le estensioni
 * CodeMirror pronte (`estensioniAssistenza`), incluse poi da `editor.ts`.
 *
 * Offset: CodeMirror conta in unità UTF-16; il backend converte in byte per
 * `typst-ide` e riconverte il `from` di ritorno in UTF-16 (gestito lato Rust).
 */

import { invoke } from "@tauri-apps/api/core";
import {
  autocompletion,
  snippetCompletion,
  type CompletionContext,
  type CompletionResult,
  type CompletionSource,
  type Completion,
} from "@codemirror/autocomplete";
import { hoverTooltip, showTooltip, ViewPlugin, type Tooltip, type ViewUpdate, EditorView } from "@codemirror/view";
import { StateEffect, StateField, type EditorState, type Extension } from "@codemirror/state";
import {
  FINESTRA,
  chiamataAttiva,
  parametroAttivo,
  pezziFirma,
  type PezzoFirma,
  tipiParametro,
  trovaFirma,
  type Firme,
} from "./firma";

/** Una voce di completamento, nel formato prodotto dal comando `completa`. */
interface Completamento {
  etichetta: string;
  tipo: string;
  applica: string | null;
  dettaglio: string | null;
}

/** Risultato del comando `completa`. */
interface RisultatoCompletamento {
  /** Offset UTF-16 da cui inizia il token da completare. */
  da: number;
  voci: Completamento[];
}

/** Ritardo (ms) prima di mostrare il tooltip dell'hover (come il debounce compile). */
const RITARDO_HOVER_MS = 300;

/**
 * Contatore di versione delle richieste di completamento: una risposta più
 * lenta resa obsoleta da una richiesta successiva (digitazione veloce) viene
 * scartata, evitando che un risultato vecchio sovrascriva quello aggiornato.
 *
 * NB: questo contatore e `opzioniSnippet` sono stato **di modulo**, condiviso —
 * corretto finché c'è una sola istanza di editor (design attuale). Se in futuro
 * si aggiungerà il multi-editor (split-view/multi-tab, cfr. il `Compartment`
 * per-istanza in `editor.ts`), andranno resi per-istanza incapsulando queste
 * estensioni in una factory, altrimenti le istanze interferirebbero sul
 * versionamento dei completamenti.
 */
let versioneCompletamento = 0;

/**
 * `false` per i documenti Markdown: `completa`/`tooltip` di typst-ide lavorano
 * su offset del sorgente Typst, che dentro un testo Markdown non esistono (il
 * Typst è generato dalla conversione). Stato di modulo come il resto (una sola
 * istanza di editor); lo imposta `editor.impostaTipo`.
 */
let assistenzaTypstAttiva = true;

/** Attiva/disattiva completamento e hover typst-ide (disattivi nei documenti Markdown). */
export function impostaAssistenzaTypst(attiva: boolean): void {
  assistenzaTypstAttiva = attiva;
  rinfrescaFirma();
  if (!attiva) {
    versioneCompletamento++; // invalida eventuali richieste in volo
  }
}

/** Interruttori del menu Opzioni (stato di modulo, come il resto: una sola istanza di editor). */
let helpCompletamento = true;
let suggerimentoParametri = true;

/** Mostra la documentazione accanto alle voci di completamento e il tipo dei parametri. */
export function impostaHelpCompletamento(attivo: boolean): void {
  helpCompletamento = attivo;
}

export function helpCompletamentoAttivo(): boolean {
  return helpCompletamento;
}

export function suggerimentoParametriAttivo(): boolean {
  return suggerimentoParametri;
}

/** Vista dell'editor (una sola istanza): serve a riaggiornare il tooltip quando cambia lo stato di modulo. */
let vista: EditorView | null = null;
const aggiornaFirma = StateEffect.define<null>();
function rinfrescaFirma(): void {
  vista?.dispatch({ effects: aggiornaFirma.of(null) });
}

/** Mostra la firma della funzione mentre si scrivono gli argomenti. */
export function impostaSuggerimentoParametri(attivo: boolean): void {
  suggerimentoParametri = attivo;
  rinfrescaFirma();
}

/**
 * Firme delle funzioni predefinite (`typstBuiltinSignatures`), caricate a parte
 * perché stanno nel chunk Lezer. Finché mancano, firma e tipi non compaiono.
 */
let firme: Firme | null = null;

export async function caricaFirme(): Promise<void> {
  try {
    firme = (await import("codemirror-lang-typst/lezer")).typstBuiltinSignatures;
    rinfrescaFirma();
  } catch (e) {
    // Nessuna firma: il completamento typst-ide resta com'è.
    console.warn("Firme delle funzioni non caricate:", e);
  }
}

/** Uno snippet riutilizzabile, nel formato prodotto dal comando `elenca_snippet`. */
interface Snippet {
  etichetta: string;
  prefisso: string;
  corpo: string;
  descrizione: string | null;
}

/**
 * Opzioni di completamento generate dagli snippet caricati. Popolata da
 * `caricaSnippet` all'avvio; vuota finché i dati non arrivano (l'editor è subito
 * usabile e i suggerimenti snippet compaiono appena pronti).
 */
let opzioniSnippet: Completion[] = [];

/**
 * Carica gli snippet dal backend e li converte in completamenti CodeMirror con
 * placeholder navigabili (`snippetCompletion`). In caso di errore lascia la lista
 * vuota: l'assenza di snippet non blocca l'editor.
 */
export async function caricaSnippet(): Promise<void> {
  let snippet: Snippet[];
  try {
    snippet = await invoke<Snippet[]>("elenca_snippet");
  } catch {
    // Lascia invariata l'eventuale lista già caricata (non azzerarla su un
    // errore di una chiamata successiva, es. un futuro "ricarica snippet").
    return;
  }
  opzioniSnippet = snippet.map((s) =>
    snippetCompletion(s.corpo, {
      label: s.prefisso,
      detail: s.etichetta,
      info: s.descrizione ?? undefined,
      type: "text",
    }),
  );
}

/**
 * Sorgente di completamento per gli snippet: propone gli snippet quando si digita
 * una parola, espandendo i placeholder. Restituisce `null` se non c'è nessuno
 * snippet, se non si sta digitando una parola, o se la parola è preceduta da `#`.
 */
const fonteSnippet: CompletionSource = (context: CompletionContext) => {
  if (opzioniSnippet.length === 0) {
    return null;
  }
  // Parola (prefisso) prima del cursore.
  const parola = context.matchBefore(/[\p{L}\p{N}_-]+/u);
  if (parola === null) {
    return null;
  }
  // Subito dopo '#' lavora il completamento typst-ide (funzioni Typst): lì NON
  // si propongono gli snippet, perché il loro corpo inizia già con '#' e si
  // otterrebbe un cancelletto doppio (`##figure(...)`).
  const precedente =
    parola.from > 0 ? context.state.sliceDoc(parola.from - 1, parola.from) : "";
  if (precedente === "#") {
    return null;
  }
  return {
    from: parola.from,
    options: opzioniSnippet,
    validFor: /^[\p{L}\p{N}_-]*$/u,
  };
};

/**
 * Sorgente di completamento per CodeMirror: interroga il backend Typst.
 * Restituisce `null` (nessun popup) se il backend non offre completamenti o se
 * la chiamata fallisce — l'assenza di suggerimenti non deve mai rompere l'editor.
 * (Anche un cursore non convertibile in byte lato Rust produce `null`/nessun popup.)
 */
async function fonteCompletamento(
  context: CompletionContext,
): Promise<CompletionResult | null> {
  if (!assistenzaTypstAttiva) {
    return null;
  }
  const versione = ++versioneCompletamento;
  const sorgente = context.state.doc.toString();
  let risultato: RisultatoCompletamento | null;
  try {
    risultato = await invoke<RisultatoCompletamento | null>("completa", {
      sorgente,
      cursore: context.pos, // offset UTF-16
      esplicito: context.explicit,
    });
  } catch {
    return null;
  }
  // Scarta la risposta se nel frattempo è partita una richiesta più recente.
  if (versione !== versioneCompletamento) {
    return null;
  }
  if (risultato === null || risultato.voci.length === 0) {
    return null;
  }

  // Parametri della funzione di cui si stanno scrivendo gli argomenti (per i tipi).
  const chiamata = helpCompletamento && firme !== null
    ? chiamataAttiva(sorgente, risultato.da)
    : null;
  const params = chiamata && firme ? trovaFirma(firme, chiamata.chiamata) : null;

  const opzioni: Completion[] = risultato.voci.map((v) => {
    const p = v.tipo === "property" ? params?.find((q) => q[0] === v.etichetta) : undefined;
    return {
      label: v.etichetta,
      type: v.tipo,
      // Con l'help attivo la documentazione va nel pannello laterale (`info`) e
      // accanto alla voce resta solo il tipo del parametro; altrimenti com'era.
      detail: helpCompletamento ? (p ? tipiParametro(p) : undefined) : (v.dettaglio ?? undefined),
      info: helpCompletamento && v.dettaglio !== null ? v.dettaglio : undefined,
      // Usa `applica` solo se non contiene segnaposto snippet (`${…}`), che
      // CodeMirror inserirebbe alla lettera; altrimenti inserisce l'etichetta
      // (nome pulito). Il supporto agli snippet è un refinement futuro.
      apply: v.applica !== null && !v.applica.includes("${") ? v.applica : undefined,
    };
  });

  return {
    from: risultato.da,
    options: opzioni,
    // Finché l'utente digita caratteri di parola, CodeMirror filtra localmente
    // la lista senza re-interrogare il backend; su altri caratteri ri-chiede.
    validFor: /^[\p{L}\p{N}_\-]*$/u,
  };
}

/**
 * Sorgente dell'hover: chiede al backend il tooltip per la posizione indicata.
 * Restituisce `null` se non c'è nulla da mostrare o se la chiamata fallisce.
 */
async function fonteHover(view: EditorView, pos: number): Promise<Tooltip | null> {
  if (!assistenzaTypstAttiva) {
    return null;
  }
  let testo: string | null;
  try {
    testo = await invoke<string | null>("tooltip", {
      sorgente: view.state.doc.toString(),
      cursore: pos, // offset UTF-16
    });
  } catch {
    return null;
  }
  if (testo === null || testo === "" || !assistenzaTypstAttiva) {
    return null;
  }
  const messaggio = testo;
  return {
    pos,
    create: () => {
      const dom = document.createElement("div");
      dom.className = "cm-tooltip-typst";
      dom.textContent = messaggio;
      return { dom };
    },
  };
}

/** Dati della firma sotto il cursore (il DOM sta in `creaFirma`, identico fra un aggiornamento e l'altro). */
interface DatiFirma {
  pos: number;
  pezzi: PezzoFirma[];
}

function datiFirma(state: EditorState): DatiFirma | null {
  const sel = state.selection.main;
  if (!suggerimentoParametri || !assistenzaTypstAttiva || firme === null || !sel.empty) {
    return null;
  }
  const da = Math.max(0, sel.head - FINESTRA);
  const attiva = chiamataAttiva(state.doc.sliceString(da, sel.head), sel.head - da);
  const params = attiva ? trovaFirma(firme, attiva.chiamata) : null;
  if (!attiva || !params) {
    return null;
  }
  const attivo = parametroAttivo(params, attiva.corrente);
  return { pos: sel.head, pezzi: pezziFirma(attiva.chiamata, params, attivo, attiva.usati) };
}

const campoFirma = StateField.define<DatiFirma | null>({
  create: datiFirma,
  update: (valore, tr) =>
    tr.docChanged || tr.selection || tr.effects.some((e) => e.is(aggiornaFirma))
      ? datiFirma(tr.state)
      : valore,
  provide: (campo) =>
    showTooltip.from(campo, (d): Tooltip | null =>
      d === null ? null : { pos: d.pos, above: true, create: creaFirma },
    ),
});

/** Una sola funzione `create`: CodeMirror riusa il DOM e chiama `update` invece di ricrearlo. */
function creaFirma(view: EditorView) {
  const dom = document.createElement("div");
  dom.className = "cm-tooltip-typst cm-firma-typst";
  const disegna = (state: EditorState) => {
    dom.replaceChildren();
    for (const pezzo of state.field(campoFirma)?.pezzi ?? []) {
      const el = document.createElement(pezzo.attivo ? "strong" : "span");
      el.textContent = pezzo.testo;
      dom.append(el);
    }
  };
  disegna(view.state);
  return { dom, update: (u: ViewUpdate) => disegna(u.state) };
}

/**
 * Estensioni CodeMirror per l'assistenza alla scrittura: completamento (con la
 * sorgente Typst come `override`) e hover. Incluse da `editor.ts`.
 */
export function estensioniAssistenza(): Extension[] {
  return [
    autocompletion({
      // Due sorgenti: completamenti del compilatore (typst-ide) e snippet.
      // CodeMirror le interroga entrambe e fonde i risultati nel menu.
      override: [fonteCompletamento, fonteSnippet],
      activateOnTyping: true,
    }),
    hoverTooltip(fonteHover, { hoverTime: RITARDO_HOVER_MS }),
    campoFirma,
    ViewPlugin.define((v) => {
      vista = v;
      return {
        destroy: () => {
          if (vista === v) vista = null;
        },
      };
    }),
  ];
}
