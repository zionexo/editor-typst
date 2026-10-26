/**
 * editor.ts — Configurazione dell'editor CodeMirror 6 (Fase 1, esteso in Fase 6).
 *
 * Incapsula la creazione dell'`EditorView` ed espone al resto del frontend
 * un'API minima e stabile ([`Editor`]): leggere il testo, sostituirlo e dare il
 * focus. Il `main.ts` non conosce CodeMirror: dipende solo da questa interfaccia.
 *
 * # Evidenziazione per tipo di documento (Fase 6)
 * L'editor può evidenziare sia Typst sia Markdown: il linguaggio attivo vive in
 * un `Compartment` riconfigurabile a caldo via [`Editor.impostaTipo`], così
 * l'apertura di un `.md` o una conversione md↔typst cambia grammatica senza
 * ricreare l'istanza.
 *
 * La grammatica Typst è fornita da `codemirror-lang-typst` (0.6) in due motori, scelti
 * dall'utente (menu «Opzioni», `Editor.impostaMotoreTypst`) e caricati con **import
 * dinamico** in `try/catch`, in chunk separati:
 * - "lezer" (predefinito): `typst_lezer()`, parser Lezer nativo, senza WASM (il chunk non lo
 *   contiene);
 * - "wasm": `typst()`, evidenziatore a decorazioni nel modulo WASM di `typst-syntax`.
 * Se l'import o l'inizializzazione fallisce (es. CSP che blocca `wasm-unsafe-eval`),
 * l'editor resta pienamente funzionante con l'evidenziazione di default (principio
 * "un errore non blocca l'editor", PROGETTO.md §9). La grammatica Markdown
 * (`@codemirror/lang-markdown`) è invece una dipendenza stabile importata
 * staticamente.
 */

import { EditorView, basicSetup } from "codemirror";
import { EditorState, EditorSelection, Compartment, type Extension } from "@codemirror/state";
import { LanguageSupport } from "@codemirror/language";
import { oneDark } from "@codemirror/theme-one-dark";
import { setDiagnostics, lintGutter, type Diagnostic } from "@codemirror/lint";
import { markdown } from "@codemirror/lang-markdown";
import { eseguiComando, type ComandoFormattazione } from "./formattazione";
import { messaggioMarcatore, type DiagnosticoBase } from "./errori";
import { estensioniAssistenza, impostaAssistenzaTypst } from "./assistenza";
import { sostituzioneMinima } from "./modificheesterne";
import { centraSuTesta, estensioneFocus, estensioneMacchina } from "./modalita";
import { estensioniEvidenziazioneTypst, estensioniStile, type TagTypst } from "./stili/editor-stile";
import { STILE_PREDEFINITO, type SceltaStile } from "./stile";
import { MOTORE_PREDEFINITO, normalizzaMotore, type MotoreTypst } from "./motore";

export type { ComandoFormattazione } from "./formattazione";

/**
 * Tipo di documento in modifica: sorgente Typst nativo o Markdown (Fase 6).
 * Determina l'evidenziazione dell'editor e, a monte, quale pipeline di anteprima
 * usa il frontend (compilazione diretta vs conversione md→typst→compilazione).
 */
export type TipoDocumento = "typst" | "markdown";

/**
 * Diagnostico da mostrare nell'editor, nel formato prodotto dal backend.
 * `da`/`a` sono offset UTF-16 nella sorgente; `null` quando il diagnostico non
 * appartiene al documento principale (es. errore dentro un pacchetto) e quindi
 * non ha una posizione mostrabile inline.
 */
export type DiagnosticoEditor = DiagnosticoBase;

/** API pubblica dell'editor verso il resto del frontend. */
export interface Editor {
  /** Restituisce il testo sorgente corrente. */
  testo(): string;
  /** Sostituisce l'intero contenuto del documento (per "apri file"). */
  imposta(testo: string): void;
  /**
   * Sostituisce il contenuto modificando solo il tratto che differisce (ricarica
   * da disco, ripristino di una versione): cursore e scorrimento si conservano
   * per quanto possibile, a differenza di [`Editor.imposta`].
   */
  sostituisci(testo: string): void;
  /** Dà il focus all'area di editing. */
  focus(): void;
  /** Inserisce del testo alla posizione del cursore (es. una riga `#import`). */
  inserisci(testo: string): void;
  /**
   * Porta il cursore all'offset UTF-16 indicato e lo scorre in vista (sync). Con `fine`
   * seleziona l'intervallo `offset..fine`. Gli offset sono ristretti al documento corrente.
   */
  vaiA(offset: number, fine?: number): void;
  /** Mostra inline i diagnostici posizionati (sottolineature + gutter). */
  mostraDiagnostici(diagnostici: DiagnosticoEditor[]): void;
  /** Applica un comando di formattazione (inserisce il markup Typst). */
  formatta(comando: ComandoFormattazione): void;
  /** Cambia il tipo di documento e la relativa evidenziazione (Typst/Markdown). */
  impostaTipo(tipo: TipoDocumento): void;
  /** Cambia a caldo il tema dell'editor (F5-004); contenuto e cursore restano intatti. */
  impostaTema(tema: TemaEditor): void;
  /**
   * Cambia a caldo lo stile grafico: il classico mantiene il tema storico (chiaro
   * di default / oneDark), gli altri usano i colori di sintassi dello stile.
   */
  impostaStile(stile: SceltaStile): void;
  /**
   * Cambia a caldo il motore di evidenziazione Typst ("lezer" | "wasm"). Riconfigura il
   * linguaggio solo se il documento attivo è Typst; il motore resta memorizzato per i
   * successivi `impostaTipo`. Testo e cursore restano intatti.
   */
  impostaMotoreTypst(motore: MotoreTypst): void;
  /** Chiede a CodeMirror di rimisurare (dopo un cambio di font-size via CSS, F5-005). */
  rimisura(): void;
  /** Offset UTF-16 attuale del cursore (testa della selezione principale). */
  cursore(): number;
  /** Selezione principale (offset UTF-16, `da <= a`); vuota se `da === a`. */
  selezione(): { da: number; a: number };
  /**
   * Applica più sostituzioni in un'unica transazione (un solo passo di annullamento). Gli intervalli
   * sono in offset UTF-16 del documento ATTUALE, non sovrapposti; vengono ristretti al documento.
   * `selezione` (offset nel documento RISULTANTE) è applicata nella stessa transazione.
   */
  sostituisciIntervalli(
    cambi: readonly { da: number; a: number; testo: string }[],
    selezione?: { da: number; a: number },
  ): void;
  /** Modalità focus (F5-023): schiarisce tutto tranne il paragrafo del cursore. Testo e selezione restano intatti. */
  impostaFocus(attiva: boolean): void;
  /** Modalità macchina da scrivere (F5-023): la riga del cursore resta a metà altezza. Testo e selezione restano intatti. */
  impostaMacchina(attiva: boolean): void;
}

/** Tema risolto dell'editor: "sistema" è risolto a monte (vedi aspetto.ts). */
export type TemaEditor = "chiaro" | "scuro";

/**
 * Crea l'editor CodeMirror dentro il container indicato.
 *
 * @param parent          elemento DOM che ospita l'editor.
 * @param valoreIniziale  testo iniziale del documento.
 * @param onModifica      invocata ad ogni modifica del documento; il chiamante
 *                        decide il debounce e l'avvio della compilazione.
 * @param onCursore       invocata quando la selezione/cursore cambia (offset UTF-16
 *                        della testa), per la sincronizzazione con l'anteprima.
 * @returns l'API [`Editor`] per interagire con l'istanza.
 */
export function creaEditor(
  parent: HTMLElement,
  valoreIniziale: string,
  onModifica: () => void,
  onCursore?: (offset: number) => void,
): Editor {
  // Compartment locale all'istanza (non di modulo): se in futuro si creassero
  // più editor — split-view, multi-tab — ognuno riconfigura il proprio linguaggio
  // senza interferenze. Parte vuoto: l'editor è subito usabile.
  const linguaggio = new Compartment();
  // Tema (F5-004): compartment separato, riconfigurabile senza toccare lo stato
  // del documento. Il tema chiaro è quello predefinito di CodeMirror (nessuna
  // estensione); lo scuro è oneDark.
  const tema = new Compartment();
  // Modalità di scrittura (F5-023): un compartment ciascuna, spente all'avvio;
  // `main.ts` applica lo stato salvato in `impostazioni.json`.
  const modalitaFocus = new Compartment();
  const modalitaMacchina = new Compartment();
  // Colori dei tag propri di Typst (dipendono dal motore caricato, dallo stile e dal tema).
  const evidenziazioneTypst = new Compartment();

  // Versione (per-istanza) delle richieste di cambio linguaggio: l'attivazione
  // Typst passa da un `await import` del chunk WASM, quindi una richiesta più
  // lenta può risolvere DOPO una scelta più recente (es. apertura di un `.md`
  // mentre l'import Typst a freddo è ancora in volo) e sovrascriverla. Il
  // contatore fa sì che solo l'ultima richiesta applichi la sua `reconfigure`.
  let versioneLinguaggio = 0;

  // Listener idiomatico: `docChanged` per la compilazione, `selectionSet` per la
  // sincronizzazione con l'anteprima (entrambi distinti da scroll/re-misurazioni).
  const ascoltatore = EditorView.updateListener.of((update) => {
    if (update.docChanged) {
      onModifica();
    }
    if (update.selectionSet && onCursore) {
      onCursore(update.state.selection.main.head);
    }
  });

  const state = EditorState.create({
    doc: valoreIniziale,
    extensions: [
      basicSetup, // numeri riga, history, chiusura parentesi, auto-indent, ricerca, fold
      tema.of(oneDark), // tema scuro all'avvio; aspetto.ts lo riallinea subito
      EditorView.lineWrapping, // a capo automatico per righe lunghe
      lintGutter(), // colonnina con i marcatori di errore/avviso accanto alle righe
      ...estensioniAssistenza(), // autocompletamento + hover via typst-ide (override)
      linguaggio.of([]), // segnaposto: riempito async con la grammatica del tipo attivo
      evidenziazioneTypst.of([]),
      modalitaFocus.of([]),
      modalitaMacchina.of([]),
      ascoltatore,
    ],
  });

  const view = new EditorView({ state, parent });

  // Tema risolto e stile correnti: il compartment del tema li combina (il classico
  // conserva chiaro/oneDark, gli altri stili usano le variabili CSS dello stile).
  let temaCorrente: TemaEditor = "scuro";
  let stileCorrente: SceltaStile = STILE_PREDEFINITO;
  // Tipo e motore correnti, e tag di Typst del motore caricato (null finché non è pronto).
  let tipoCorrente: TipoDocumento = "typst";
  let motoreCorrente: MotoreTypst = MOTORE_PREDEFINITO;
  let tagTypst: TagTypst | null = null;
  // Ultimo supporto di linguaggio applicato: serve a ricreare il plugin del motore wasm.
  let supportoCorrente: Extension = [];
  const effettoEvidenziazioneTypst = () =>
    evidenziazioneTypst.reconfigure(
      tipoCorrente === "typst" && tagTypst !== null
        ? estensioniEvidenziazioneTypst(tagTypst, stileCorrente === "classico", temaCorrente === "scuro")
        : [],
    );
  const riconfiguraTema = (): void =>
    view.dispatch({
      effects: [
        tema.reconfigure(
          stileCorrente === "classico"
            ? temaCorrente === "scuro"
              ? oneDark
              : []
            : estensioniStile(temaCorrente === "scuro"),
        ),
        effettoEvidenziazioneTypst(),
      ],
    });

  /**
   * Attiva l'evidenziazione del tipo/motore correnti. Solo l'ultima richiesta applica la
   * sua `reconfigure` (guardia `versioneLinguaggio`): un import lento non deve
   * sovrascrivere una scelta più recente.
   */
  const attivaCorrente = (): void => {
    const versione = ++versioneLinguaggio;
    void attivaLinguaggio(tipoCorrente, motoreCorrente, (supporto, tag) => {
      if (versione !== versioneLinguaggio) {
        return;
      }
      tagTypst = tag;
      supportoCorrente = supporto;
      view.dispatch({ effects: [linguaggio.reconfigure(supporto), effettoEvidenziazioneTypst()] });
    });
  };

  // Tipo iniziale: Typst col motore predefinito (attivato in modo non bloccante).
  attivaCorrente();

  return {
    testo: () => view.state.doc.toString(),
    imposta: (testo: string) => {
      // Sostituzione tramite transazione (`dispatch(changes)`), NON tramite
      // `view.setState`: la transazione conserva compartment, history e listener
      // e segnala la modifica al chiamante.
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: testo },
      });
      // Motore wasm (codemirror-lang-typst 0.6.0): l'aggiornamento incrementale delle
      // evidenziazioni dopo una sostituzione dell'intero documento perde gran parte dei
      // colori (misurato: fuzz su 300 sostituzioni totali, 294 diverse dal ricalcolo
      // completo). Ricreare il plugin (via vuoto e ritorno) lo ricostruisce dal testo.
      if (motoreCorrente === "wasm" && tipoCorrente === "typst" && tagTypst !== null) {
        view.dispatch({ effects: linguaggio.reconfigure([]) });
        view.dispatch({ effects: linguaggio.reconfigure(supportoCorrente) });
      }
    },
    sostituisci: (testo: string) => {
      // Stessa regola di `imposta`: sempre una transazione. Il cambiamento si
      // limita al tratto diverso, così la selezione viene rimappata da CodeMirror.
      const s = sostituzioneMinima(view.state.doc.toString(), testo);
      if (s.da === s.a && s.inserisci === "") {
        return; // testi identici: nessuna transazione, nessun segnale di modifica
      }
      view.dispatch({ changes: { from: s.da, to: s.a, insert: s.inserisci } });
    },
    focus: () => view.focus(),
    inserisci: (testo: string) => {
      // Sostituisce la selezione (o inserisce al cursore) col testo, via transazione.
      view.dispatch(view.state.replaceSelection(testo));
      view.focus();
    },
    vaiA: (offset: number, fine?: number) => {
      // Clamp difensivo: l'offset (UTF-16, come `doc.length` in CodeMirror) arriva
      // dal backend e il documento può essere cambiato nel frattempo.
      const lunghezza = view.state.doc.length;
      const pos = Math.max(0, Math.min(offset, lunghezza));
      const fin = fine === undefined ? pos : Math.max(pos, Math.min(fine, lunghezza));
      view.dispatch({
        selection: fin === pos ? EditorSelection.cursor(pos) : EditorSelection.range(pos, fin),
        scrollIntoView: true,
      });
      view.focus();
    },
    mostraDiagnostici: (diagnostici: DiagnosticoEditor[]) => {
      const lunghezza = view.state.doc.length;
      const cm: Diagnostic[] = [];
      for (const d of diagnostici) {
        // Solo i diagnostici con posizione nel documento principale sono inline.
        if (d.da === null || d.a === null) {
          continue;
        }
        // Clamp obbligatorio: tra compilazione e risposta il documento può essere
        // cambiato; offset oltre la fine farebbero lanciare un RangeError a CodeMirror.
        const da = Math.max(0, Math.min(d.da, lunghezza));
        const a = Math.max(da, Math.min(d.a, lunghezza));
        cm.push({
          from: da,
          to: a,
          severity: d.gravita === "errore" ? "error" : "warning",
          message: messaggioMarcatore(d),
          source: "typst",
        });
      }
      view.dispatch(setDiagnostics(view.state, cm));
    },
    impostaTema: (t: TemaEditor) => {
      temaCorrente = t;
      riconfiguraTema();
    },
    impostaStile: (s: SceltaStile) => {
      stileCorrente = s;
      riconfiguraTema();
    },
    rimisura: () => view.requestMeasure(),
    cursore: () => view.state.selection.main.head,
    selezione: () => ({ da: view.state.selection.main.from, a: view.state.selection.main.to }),
    sostituisciIntervalli: (cambi, selezione) => {
      const n = view.state.doc.length;
      const changes = cambi.map((c) => {
        const d = Math.max(0, Math.min(c.da, n));
        return { from: d, to: Math.max(d, Math.min(c.a, n)), insert: c.testo };
      });
      const nuovo = changes.reduce((len, c) => len - (c.to - c.from) + c.insert.length, n);
      view.dispatch({
        changes,
        selection:
          selezione === undefined
            ? undefined
            : { anchor: Math.min(selezione.da, nuovo), head: Math.min(selezione.a, nuovo) },
        scrollIntoView: selezione !== undefined,
      });
    },
    impostaFocus: (attiva: boolean) =>
      view.dispatch({ effects: modalitaFocus.reconfigure(attiva ? estensioneFocus() : []) }),
    impostaMacchina: (attiva: boolean) => {
      view.dispatch({ effects: modalitaMacchina.reconfigure(attiva ? estensioneMacchina() : []) });
      if (attiva) {
        // Al momento dell'accensione la riga corrente si porta subito al centro.
        window.requestAnimationFrame(() => centraSuTesta(view));
      }
    },
    formatta: (comando: ComandoFormattazione) => eseguiComando(view, comando),
    impostaTipo: (tipo: TipoDocumento) => {
      // Completamento/hover typst-ide solo su Typst: in Markdown gli offset non
      // corrisponderebbero al sorgente Typst generato.
      impostaAssistenzaTypst(tipo === "typst");
      tipoCorrente = tipo;
      attivaCorrente();
    },
    impostaMotoreTypst: (motore: MotoreTypst) => {
      const nuovo = normalizzaMotore(motore);
      if (nuovo === motoreCorrente) {
        return;
      }
      motoreCorrente = nuovo;
      // Con un documento Markdown il motore resta memorizzato e si applica al prossimo Typst.
      if (tipoCorrente === "typst") {
        attivaCorrente();
      }
    },
  };
}


/**
 * Carica la grammatica per il tipo di documento (e, per Typst, il motore) e la consegna a
 * `applica` insieme ai tag propri di Typst del motore (`null` per Markdown). Per "typst":
 * - wasm → `typst()` da `codemirror-lang-typst` (porta con sé il chunk WASM);
 * - lezer → `typst_lezer()` da `codemirror-lang-typst/lezer` (chunk separato, nessun WASM),
 *   privato del linter sintattico (vedi sotto).
 * In caso di errore consegna un supporto vuoto (evidenziazione di default): l'editor resta
 * sempre usabile. Sta a `applica` scartare le risposte obsolete.
 */
async function attivaLinguaggio(
  tipo: TipoDocumento,
  motore: MotoreTypst,
  applica: (supporto: Extension, tag: TagTypst | null) => void,
): Promise<void> {
  try {
    if (tipo === "markdown") {
      applica(markdown(), null);
      return;
    }
    if (motore === "lezer") {
      const m = await import("codemirror-lang-typst/lezer");
      // `typst_lezer()` installa anche un linter sintattico. Ogni linter, a editor fermo,
      // chiama `setDiagnostics`, che SOSTITUISCE tutti i diagnostici: cancellerebbe quelli
      // del compilatore (`mostraDiagnostici`) e viceversa. Il compilatore segnala già gli
      // errori di sintassi, quindi il linter viene tolto dal supporto.
      const supporto = m.typst_lezer();
      const base = Array.isArray(supporto.support) ? (supporto.support as Extension[]) : [];
      const senzaLinter = base.filter((e) => e !== m.typstLezerLinter);
      if (senzaLinter.length === base.length) {
        console.warn("Linter di typst_lezer non individuato: i diagnostici potrebbero sovrapporsi.");
      }
      applica(new LanguageSupport(supporto.language, senzaLinter), m.typstTags);
      return;
    }
    const m = await import("codemirror-lang-typst");
    applica(m.typst(), m.typstTags);
  } catch (e) {
    applica([], null);
    console.warn(`Evidenziazione ${tipo} non disponibile: uso l'evidenziazione di default.`, e);
  }
}
