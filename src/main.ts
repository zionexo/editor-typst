/**
 * main.ts — Frontend Fase 1 di Typster.
 *
 * UI: editor CodeMirror 6 (sorgente) + bottone "Compila" + anteprima SVG.
 * - Editor di codice vero (CodeMirror): numeri di riga, evidenziazione,
 *   auto-indent, chiusura parentesi, tema chiaro/scuro, a capo automatico.
 *   Il setup dell'editor vive in `editor.ts`; qui se ne usa solo l'API.
 * - Compilazione automatica con debounce adattivo (base `debounce_ms`, default 300 ms) ad ogni modifica del documento.
 * - Il bottone "Compila" resta un trigger manuale immediato.
 * - Gestione delle compilazioni sovrapposte: se una è in corso, la prossima
 *   parte solo al termine con il testo più recente (flag `inCorso`/`richiestaPendente`).
 * - Prima compilazione automatica all'avvio (mostra l'anteprima del testo iniziale).
 * - Apertura/salvataggio file .typ ed export PDF (modulo `fileops.ts`), con
 *   indicatore del file corrente e delle modifiche non salvate.
 * - Ogni azione è un comando del registro (`comandi.ts`): alimenta le scorciatoie
 *   da tastiera (Ctrl/Cmd) e la palette comandi (`palette.ts`, Ctrl/Cmd+P).
 */

import { invoke } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";
import { attendiCondizione, regoleStampa } from "./stampa";
import "./stili/stampa.css";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  creaEditor,
  type Editor,
  type DiagnosticoEditor,
  type ComandoFormattazione,
  type TipoDocumento,
} from "./editor";
import {
  creaGestoreFile,
  nomeFile,
  type GestoreFile,
  type StatoFile,
} from "./fileops";
import { caricaFirme, caricaSnippet } from "./assistenza";
import { creaRiquadroErrori } from "./riquadro-errori";
import { creaPallino } from "./pallino";
import { riepilogoDiagnostici } from "./errori";
import type { StatoScrittura } from "./fileops";
import "./stili/composizione.css";
import "./stili/strumento.css";
import "./stili/menu-aspetto.css";
import "./stili/menu-opzioni.css";
import "./stili/pannello-strumenti.css";
import {
  applicaStileSpecchiato,
  inizializzaAspetto,
  impostaStile,
  impostaMotore,
  impostaTema,
  alternaTema,
  aumentaFont,
  riduciFont,
  ripristinaFont,
} from "./aspetto";
import { creaMenuAspetto, type MenuAspetto } from "./menu-aspetto";
import { creaMenuOpzioni, type MenuOpzioni } from "./menu-opzioni";
import { MOTORI, titoloComandoMotore } from "./motore";
import { STILI, etichettaStile } from "./stile";
import { creaAnteprima, type Anteprima, type MetadatiPagina, type PaginaSvg } from "./anteprima";
import { ritardoDebounce } from "./debounce";
import { caricaImpostazioni, leggiImpostazioni, PREDEFINITE, salvaImpostazioni } from "./impostazioni";
import type { EventoEsterno } from "./modificheesterne";
import { creaDialogoCronologia, type DialogoCronologia } from "./cronologia";
import { creaRegistro, formattaScorciatoia, rilevaMac, type RegistroComandi } from "./comandi";
import { creaPalette, type Palette } from "./palette";
import { creaGestorePacchetti, ID_PANNELLO_PACCHETTI, type GestorePacchetti } from "./pacchetti";
import { creaBarraLaterale, type BarraLaterale } from "./barra-laterale";
import { creaPannelloStrumenti, type PannelloStrumenti, type RisultatoModello } from "./pannelli/strumenti";
import type { ModelloInfo } from "./pannelli/strumenti-logica";
import { creaPannelloIndice, type PannelloIndice } from "./pannelli/indice";
import { creaPannelloFile, ID_PANNELLO_FILE, type PannelloFile } from "./pannelli/file";
import type { AlberoFile } from "./pannelli/file-logica";
import type { Destinazione, VoceIndice } from "./pannelli/indice-logica";
import {
  configuraSincronizzazione,
  type Sincronizzazione,
} from "./sincronizzazione";
import { creaGestoreLocal, type GestoreLocal } from "./local";
import {
  creaCicloCompilazione,
  reagisciABattuta,
  reagisciASostituzione,
  ricompilaPerWorldCambiato,
  type CicloCompilazione,
} from "./ciclocompilazione";
import { creaDialogoEsporta, type DialogoEsporta } from "./esporta";
import {
  NOME_FORMATO,
  messaggioEsito,
  testoAvvisiConversione,
  type EsitoConversione,
  type FormatoEsportazione,
} from "./opzioniesporta";

// Primo statement: lo stile salvato (specchio in localStorage) è su <html> prima di ogni altro lavoro,
// così il primo paint non passa dal Classico. Il file delle impostazioni, caricato dopo, vince.
applicaStileSpecchiato();

/** Risultato di una compilazione restituito dal comando `compila` del backend. */
interface RisultatoCompilazione {
  /**
   * Metadati per pagina (dimensioni + hash), **senza SVG** (F5-008): l'SVG si
   * chiede a richiesta con `svg_pagina`. Vuoto se errore fatale (l'ultima
   * anteprima valida resta).
   */
  pagine: MetadatiPagina[];
  /**
   * Errori e avvisi del compilatore (con o senza posizione nel sorgente). I campi
   * `posizione_da_traccia`, `origine`, `traccia`, `suggerimenti` sono opzionali: vedi errori.ts.
   */
  diagnostici: DiagnosticoEditor[];
}

// ---- Rete di sicurezza diagnostica ----
// Riversa a schermo qualunque errore non gestito (di setup o asincrono), così un
// guasto del frontend non si traduce in una finestra nera silenziosa: almeno lo
// si vede e lo si può diagnosticare. Registrata per prima, prima di ogni altra
// inizializzazione, per catturare anche errori durante la creazione dell'editor.
function mostraErroreFatale(messaggio: string): void {
  const corpo = document.body ?? document.documentElement;
  let box = document.getElementById("errore-fatale");
  if (box === null) {
    box = document.createElement("div");
    box.id = "errore-fatale";
    box.setAttribute(
      "style",
      "position:fixed;inset:0;z-index:99999;background:#2a0a0a;color:#f5b5b5;" +
        "font:13px/1.5 ui-monospace,monospace;padding:16px;white-space:pre-wrap;overflow:auto;",
    );
    corpo.appendChild(box);
  }
  box.textContent = `Errore nel frontend (vedi DevTools per i dettagli):\n\n${messaggio}`;
}

window.addEventListener("error", (ev: ErrorEvent): void => {
  mostraErroreFatale(
    `${ev.message}\n  in ${ev.filename}:${ev.lineno}:${ev.colno}` +
      (ev.error?.stack ? `\n\n${ev.error.stack}` : ""),
  );
});
window.addEventListener("unhandledrejection", (ev: PromiseRejectionEvent): void => {
  const motivo = ev.reason as { stack?: string } | string | undefined;
  mostraErroreFatale(
    typeof motivo === "object" && motivo?.stack ? motivo.stack : String(motivo),
  );
});

// ---- Elementi DOM ----

/**
 * Recupera un elemento del DOM per id, o solleva un errore esplicito.
 * Evita `TypeError` silenziosi su `null` se un id viene rinominato o manca
 * nell'HTML: il problema emerge subito all'avvio con un messaggio chiaro.
 */
function elemento<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

const contenitoreEditor = elemento<HTMLDivElement>("sorgente");
const barraFormattazione = elemento<HTMLDivElement>("barra-formattazione");
const btnCompila        = elemento<HTMLButtonElement>("btn-compila");
const btnNuovo          = elemento<HTMLButtonElement>("btn-nuovo");
const btnApri           = elemento<HTMLButtonElement>("btn-apri");
const btnApriCartella   = elemento<HTMLButtonElement>("btn-apri-cartella");
const btnSalva          = elemento<HTMLButtonElement>("btn-salva");
const btnEsporta        = elemento<HTMLButtonElement>("btn-esporta");
const btnStampa         = elemento<HTMLButtonElement>("btn-stampa");
const boxAvvisiConversione  = elemento<HTMLDivElement>("avvisi-conversione");
const spanAvvisiConversione = elemento<HTMLSpanElement>("avvisi-conversione-testo");
const btnPacchetti      = elemento<HTMLButtonElement>("btn-pacchetti");
const btnBarra          = elemento<HTMLButtonElement>("btn-barra");
const btnLocal          = elemento<HTMLButtonElement>("btn-local");
const btnConverti       = elemento<HTMLButtonElement>("btn-converti");
const pannelloAnteprima = elemento<HTMLDivElement>("pannello-anteprima");
const divErrori         = elemento<HTMLDivElement>("errori");
const spanStato         = elemento<HTMLSpanElement>("stato");
const spanNomeFile      = elemento<HTMLSpanElement>("nome-file");

// Riquadro errori: una riga per diagnostico, cliccabile se ha una posizione nel documento.
// `editor` è letto solo al click (è già creato).
const riquadroErrori = creaRiquadroErrori(divErrori, (i) => editor.vaiA(i.da, i.a));

// Pallino di salvataggio accanto al nome del file. Dichiarato PRIMA del gestore file, che
// chiama `mostraStatoFile` già alla creazione.
const pallino = creaPallino(
  elemento<HTMLSpanElement>("pallino-salvataggio"),
  elemento<HTMLSpanElement>("annuncio-salvataggio"),
);
let ultimoStatoFile: StatoFile = { percorso: null, modificato: false };
let ultimaScrittura: StatoScrittura = { inSalvataggio: false, errore: false };

/** Ricalcola il pallino da stato del file e stato di scrittura (nessuna scrittura in più). */
function aggiornaPallino(): void {
  pallino.aggiorna({
    modificato: ultimoStatoFile.modificato,
    inSalvataggio: ultimaScrittura.inSalvataggio,
    // Un errore di scrittura vale finché ci sono modifiche: caricato un altro documento no.
    errore: ultimaScrittura.errore && ultimoStatoFile.modificato,
    haPercorso: ultimoStatoFile.percorso !== null,
  });
}

// Testo d'esempio precaricato all'avvio.
const ESEMPIO_TYPST = `= Ciao dal mondo Typst!

Questo è un *editor desktop* scritto in #text(fill: blue)[Rust + Tauri].

== Caratteristiche della Fase 1

- Compilazione Typst _embedded_ (nessuna CLI esterna)
- Editor CodeMirror 6 (numeri di riga, evidenziazione, tema chiaro/scuro)
- Anteprima SVG con aggiornamento automatico (debounce 300 ms)
- World persistente: font caricati una sola volta all'avvio

#let autore = "nexodux"

Progetto creato da *#autore* nel #datetime.today().display().

#table(
  columns: (auto, auto),
  [*Componente*], [*Versione*],
  [Typst],  [0.15.0],
  [Tauri],  [2.x],
  [Rust],   [1.96.0],
)
`;

// ---- Stato della compilazione ----

/**
 * Ciclo di compilazione (in corso / richiesta pendente / sostituzione del
 * documento), logica pura in `ciclocompilazione.ts`. Se arriva una richiesta
 * mentre una compilazione è in volo, al termine si ricompila leggendo il testo
 * PIÙ RECENTE dall'editor (non un'istantanea catturata in anticipo).
 */
const ciclo: CicloCompilazione = creaCicloCompilazione();

/** Handle del timer di debounce (usato per annullarlo sul click manuale). */
let timerDebounce: ReturnType<typeof setTimeout> | null = null;

/**
 * Durata (ms, andata e ritorno IPC compresi) dell'ultima invocazione di
 * compilazione tornata dal backend, anche se il documento ha errori di
 * compilazione (il tempo speso vale comunque). Un errore infrastrutturale non
 * la tocca: resta l'ultima misura valida. `null` solo prima della prima
 * misura. Alimenta il ritardo adattivo di `programmaCompilazione` (F5-010).
 */
let ultimaDurataMs: number | null = null;
/** Testo dell'ultima compilazione applicata all'interfaccia (serve a stampare la versione attuale). */
let ultimoTestoCompilato: string | null = null;
/** Errori dell'ultima compilazione applicata: con errori l'anteprima mostra una versione vecchia. */
let ultimiErroriCompilazione = 0;
/** `true` mentre si prepara o si apre la stampa. */
let stampaInCorso = false;


/**
 * Tipo di documento corrente (Typst | Markdown), tenuto in sincrono col gestore
 * file tramite il callback `onCambioTipo`. Determina quale comando di anteprima
 * usare (`compila` per Typst, `anteprima_markdown` per Markdown) e come mostrare
 * i diagnostici.
 */
let tipoDocumento: TipoDocumento = "typst";

// ---- Editor e gestione file ----

/**
 * Gestore dei file (apri/salva/esporta). Dichiarato prima dell'editor perché
 * `onModifica` lo referenzia, ma assegnato subito dopo la creazione dell'editor
 * (che il gestore richiede). `onModifica` viene invocato solo su modifiche
 * dell'utente, quando il gestore è già pronto.
 */
let gestoreFile: GestoreFile;

/**
 * Sincronizzazione sorgente↔anteprima. Dichiarata prima dell'editor perché il
 * callback `onCursore` la referenzia; assegnata subito dopo (è invocata solo su
 * interazione dell'utente, quando è già pronta).
 */
let sincro: Sincronizzazione | undefined;

/**
 * Pannelli della barra laterale che reagiscono a eventi dell'editor (cursore) e
 * del documento (file cambiato). Dichiarati prima dell'editor per lo stesso
 * motivo di `sincro`: i callback li referenziano ma partono solo su azione dell'utente.
 */
let pannelloIndice: PannelloIndice | undefined;
let pannelloStrumenti: PannelloStrumenti | undefined;
let pannelloFile: PannelloFile | undefined;

/**
 * `true` dopo l'apertura di un altro file dall'indice: il nuovo documento non è
 * ancora compilato, quindi la sincronizzazione cursore → anteprima va ripetuta
 * quando arriva la prima compilazione.
 */
let ripetiSincro = false;

/**
 * Istanza dell'editor CodeMirror. Creata subito: il container `#sorgente`
 * esiste già nel DOM (lo script è caricato in fondo al body come modulo).
 * Ad ogni modifica del documento aggiorna lo stato file e avvia il debounce;
 * ad ogni spostamento del cursore sincronizza l'anteprima.
 */
const editor: Editor = creaEditor(
  contenitoreEditor,
  ESEMPIO_TYPST,
  onModifica,
  (offset) => {
    sincro?.daCursore(offset);
    pannelloIndice?.cursoreCambiato(offset);
    pannelloStrumenti?.aggiorna();
  },
);

// Le sostituzioni in blocco del documento (Nuovo/apri/ricarica/ripristina/converti)
// passano tutte da `imposta`/`sostituisci`: se cambiano il testo, i risultati di
// compilazione in volo diventano obsoleti.
{
  const impostaOrig = editor.imposta;
  const sostituisciOrig = editor.sostituisci;
  editor.imposta = (testo: string) => {
    reagisciASostituzione(ciclo, editor.testo(), testo);
    impostaOrig(testo);
  };
  editor.sostituisci = (testo: string) => {
    reagisciASostituzione(ciclo, editor.testo(), testo);
    sostituisciOrig(testo);
  };
}

gestoreFile = creaGestoreFile(
  editor,
  mostraStatoFile,
  mostraErrori,
  onCambioTipo,
  (msg) => {
    spanStato.textContent = msg;
  },
  // World cambiato (apertura con testo identico, rinomina esterna): il risultato in volo
  // del documento precedente è obsoleto, quindi si invalida e si ricompila.
  () => ricompilaPerWorldCambiato(ciclo, () => avviaCompilazione()),
  // Esportazione per conversione: avvisi sulle perdite, non fatali.
  (formato, esito) => mostraAvvisiConversione(formato, esito),
);
/** Cronologia versioni del file corrente (F5-013). */
const dialogoCronologia: DialogoCronologia = creaDialogoCronologia(gestoreFile, editor);
sincro = configuraSincronizzazione(
  editor,
  pannelloAnteprima,
  mostraErrori,
  () => tipoDocumento === "typst",
);

/** Anteprima paginata: l'SVG di ogni pagina si carica solo quando è in vista (F5-008). */
const anteprima: Anteprima = creaAnteprima(pannelloAnteprima, {
  chiediSvg: (indice) => invoke<PaginaSvg | null>("svg_pagina", { indice }),
  onPaginaCaricata: (indice) => sincro?.paginaCaricata(indice),
  onAggiornata: () => sincro?.invalida(),
  // Errore IPC transitorio nel caricamento di una pagina: console + stato, non
  // il riquadro degli errori di compilazione (la pagina si ripropone da sola).
  onErrore: (e) => {
    console.warn("Pagina dell'anteprima non caricata:", e);
    spanStato.textContent = "anteprima: pagina non caricata";
  },
});

/** Gestore della pubblicazione `@local` (modale nome+versione). */
const gestoreLocal: GestoreLocal = creaGestoreLocal(
  editor,
  (msg) => {
    spanStato.textContent = msg;
  },
  () => tipoDocumento === "typst",
);

/**
 * Gestore del pannello pacchetti (Typst Universe). Dopo l'installazione inserisce
 * l'`#import` e avvia subito una compilazione per mostrarne l'effetto.
 */
const gestorePacchetti: GestorePacchetti = creaGestorePacchetti(
  editor,
  (percorso, contenuto) => gestoreFile.apriProgetto(percorso, contenuto),
  avviaCompilazione,
  mostraErrori,
  // Sorgente per il controllo degli `#import` non aggiornati (F5-012): solo Typst.
  () => (tipoDocumento === "typst" ? editor.testo() : null),
  // Dopo "Nuovo da template" il progetto è aperto: si mostra il suo albero dei file.
  () => barra.apri(ID_PANNELLO_FILE),
  // Prima di creare il template (che aggancia il World): Salva / Non salvare / Annulla.
  () => gestoreFile.proteggiModifiche("sostituire"),
);

// ---- Barra laterale a pannelli (F5-019/020/021) ----

/**
 * Barra laterale: activity bar + pannelli registrati. Lo stato (visibilità,
 * larghezza, pannello attivo) si salva in `impostazioni.json` a ogni cambio e si
 * ripristina all'avvio (vedi "Avvio"). `Esc` dalla barra riporta il fuoco all'editor.
 */
/** Allinea `aria-expanded` dei pulsanti che aprono la barra laterale e il pannello pacchetti. */
function aggiornaAriaBarra(visibile: boolean, pannello: string | null | undefined): void {
  btnBarra.setAttribute("aria-expanded", String(visibile));
  btnPacchetti.setAttribute(
    "aria-expanded",
    String(visibile && pannello === ID_PANNELLO_PACCHETTI),
  );
}

const barra: BarraLaterale = creaBarraLaterale(
  (stato) => {
    aggiornaAriaBarra(stato.visibile, stato.pannello);
    void salvaImpostazioni({
      barra_visibile: stato.visibile,
      barra_larghezza: stato.larghezza,
      pannello_attivo: stato.pannello ?? PREDEFINITE.pannello_attivo,
    }).catch((e: unknown) => console.warn("Stato della barra non salvato:", e));
  },
  () => editor.focus(),
);

/**
 * Esegue la scelta di una voce dell'indice (F5-020): cursore al titolo nel
 * documento aperto (la sincronizzazione sposta poi l'anteprima); apertura del
 * file che lo contiene, poi cursore; oppure solo la pagina dell'anteprima.
 */
async function vaiADestinazione(d: Destinazione): Promise<void> {
  if (d.tipo === "cursore") {
    editor.vaiA(d.offset);
  } else if (d.tipo === "apri-file") {
    if (!(await gestoreFile.apriFileProgetto(d.file))) {
      return; // rifiutato dall'utente o errore (già mostrato)
    }
    ripetiSincro = true;
    editor.vaiA(d.offset ?? 0);
  } else {
    anteprima.scorriA(d.pagina - 1);
  }
}

pannelloIndice = creaPannelloIndice({
  richiediVoci: () => invoke<VoceIndice[]>("indice_documento"),
  documentoMarkdown: () => tipoDocumento === "markdown",
  testoEditor: () => editor.testo(),
  cursore: () => editor.cursore(),
  vai: vaiADestinazione,
  onErrore: (e) => console.warn("Indice non disponibile:", e),
});
pannelloFile = creaPannelloFile({
  carica: () => invoke<AlberoFile | null>("albero_progetto"),
  percorsoDocumento: () => gestoreFile.stato().percorso,
  apri: async (relativo) => {
    await gestoreFile.apriFileProgetto(relativo);
  },
  giaAperto: () => editor.focus(),
  apriCartella: () => gestoreFile.apriCartella(),
  onAvviso: (msg) => {
    spanStato.textContent = msg;
  },
  onErrore: (e) => {
    console.warn("Elenco dei file non disponibile:", e);
    spanStato.textContent = "elenco dei file non disponibile";
  },
});
// Ordine dell'activity bar: file, indice, pacchetti.
barra.registra(pannelloFile);
barra.registra(pannelloIndice);
barra.registra(gestorePacchetti);

/** Pagine dell'ultima compilazione (serve a «Salva come modello»). */
let pagineUltimaCompilazione = 0;
pannelloStrumenti = creaPannelloStrumenti({
  elenca: () => invoke<ModelloInfo[]>("elenca_modelli"),
  compila: (id, valori) => invoke<RisultatoModello>("compila_modello", { id, valori }),
  salva: (nome, sorgente, campi, descrizione, pagine, sovrascrivi) =>
    invoke<string>("salva_come_modello", { nome, sorgente, campi, descrizione, pagine, sovrascrivi }),
  elimina: (id) => invoke<void>("elimina_modello", { id }),
  nuovoDocumento: async (testo) => {
    const creato = await gestoreFile.nuovoDa(testo);
    if (creato) {
      editor.focus();
    }
    return creato;
  },
  testoEditor: () => editor.testo(),
  inserisci: (testo) => editor.inserisci(testo),
  pagineDocumento: () => pagineUltimaCompilazione,
  selezione: () => editor.selezione(),
  sostituisci: (cambi, selezione) => editor.sostituisciIntervalli(cambi, selezione),
  conferma: (messaggio) => ask(messaggio, { title: "Modelli", kind: "warning" }),
  onAvviso: (msg) => {
    spanStato.textContent = msg;
  },
  onErrore: (e) => {
    console.warn("Strumenti:", e);
    spanStato.textContent = `strumenti: ${String(e)}`;
  },
});
barra.registra(pannelloStrumenti);

/** Modale delle opzioni di esportazione: pagine, standard PDF/A, PPI (F5-011). */
const dialogoEsporta: DialogoEsporta = creaDialogoEsporta(editor);

// ---- Funzioni principali ----

/**
 * Reagisce a ogni modifica del documento da parte dell'utente: marca il file
 * come modificato e programma la compilazione con debounce.
 */
function onModifica(): void {
  // Le battute NON invalidano la compilazione in volo (altrimenti con una
  // compilazione lenta l'anteprima non si aggiorna mai): solo le sostituzioni
  // del documento lo fanno (`reagisciASostituzione`). La regola vive in
  // `ciclocompilazione.ts` ed è protetta da `ciclocompilazione.check.ts`.
  reagisciABattuta(ciclo, {
    segnaModificato: () => gestoreFile.segnaModificato(),
    programmaCompilazione,
  });
}

/**
 * Reagisce al cambio di tipo di documento (Typst ↔ Markdown), notificato dal
 * gestore file su apertura, salvataggio con altra estensione o conversione.
 * Aggiorna l'evidenziazione dell'editor e la UI dipendente dal tipo.
 */
function onCambioTipo(tipo: TipoDocumento): void {
  tipoDocumento = tipo;
  editor.impostaTipo(tipo);
  aggiornaUiTipo();
}

/**
 * Allinea la UI al tipo di documento corrente: etichetta del pulsante di
 * conversione e visibilità della toolbar di formattazione (che inserisce markup
 * Typst, quindi ha senso solo per documenti Typst).
 */
function aggiornaUiTipo(): void {
  const md = tipoDocumento === "markdown";
  btnConverti.textContent = md ? "→ Typst" : "→ Markdown";
  btnConverti.title = md
    ? "Converti il documento Markdown in Typst"
    : "Converti il documento Typst in Markdown";
  barraFormattazione.style.display = md ? "none" : "";
}

/**
 * Converte il documento corrente nell'altro formato (md↔typst) tramite i comandi
 * backend, sostituisce il contenuto dell'editor e passa al nuovo tipo. La
 * conversione è dichiaratamente **lossy** (PROGETTO.md §4.5): i costrutti non
 * mappabili nel formato di destinazione possono andare persi.
 */
async function convertiDocumento(): Promise<void> {
  if (btnConverti.disabled) {
    return;
  }
  btnConverti.disabled = true;
  try {
    // Il testo attuale sta per essere sostituito: prima si chiede se salvarlo
    // (Salva / Non salvare / Annulla). Va fatto PRIMA dell'invoke, così la
    // conversione parte dal testo definitivo.
    if (!(await gestoreFile.proteggiModifiche("sostituire"))) {
      return;
    }
    const daMarkdown = tipoDocumento === "markdown";
    const comando = daMarkdown ? "converti_md_a_typst" : "converti_typst_a_md";
    const nuovoTipo: TipoDocumento = daMarkdown ? "typst" : "markdown";
    spanStato.textContent = "Conversione…";
    const sorgente = editor.testo();
    const convertito = await invoke<string>(comando, { sorgente });
    // Se l'utente ha scritto durante la conversione il risultato è obsoleto:
    // applicarlo cancellerebbe quelle modifiche.
    if (editor.testo() !== sorgente) {
      spanStato.textContent = "Conversione annullata: il testo è cambiato mentre si convertiva";
      return;
    }
    // `imposta` fa scattare onModifica (segna modificato + programma la ricompila);
    // `convertitoA` aggiorna il tipo (→ onCambioTipo) e scollega il file su disco,
    // così il prossimo Salva usa l'estensione giusta. La ricompila (col debounce)
    // parte dopo che il tipo è già cambiato, quindi usa la pipeline corretta.
    editor.imposta(convertito);
    gestoreFile.convertitoA(nuovoTipo);
    spanStato.textContent =
      nuovoTipo === "markdown" ? "Convertito in Markdown" : "Convertito in Typst";
  } catch (e) {
    mostraErrori(e);
  } finally {
    btnConverti.disabled = false;
  }
}

/**
 * Programma una compilazione con debounce adattivo (F5-010).
 *
 * Azzera il timer precedente e ne avvia uno nuovo. Il ritardo parte dal valore
 * base delle impostazioni (`debounce_ms`, letto a ogni chiamata: una modifica
 * ha effetto subito) e segue la durata dell'ultima compilazione (vedi
 * `ritardoDebounce`). Allo scadere senza ulteriori modifiche, avvia la
 * compilazione; il coalescing `inCorso`/`richiestaPendente` resta invariato.
 */
function programmaCompilazione(): void {
  if (timerDebounce !== null) {
    clearTimeout(timerDebounce);
  }
  timerDebounce = setTimeout(() => {
    timerDebounce = null;
    avviaCompilazione();
  }, ritardoDebounce(leggiImpostazioni().debounce_ms, ultimaDurataMs));
}

/**
 * Avvia una compilazione con il testo corrente dell'editor,
 * rispettando il flag anti-sovrapposizione.
 *
 * Se una compilazione è già in corso, marca solo `richiestaPendente`: al suo
 * termine si rileggerà il testo più aggiornato dall'editor.
 */
function avviaCompilazione(tentativiBridge = 0): void {
  if (ciclo.richiedi() === "accoda") {
    return;
  }
  void eseguiCompilazione(editor.testo(), tentativiBridge);
}

/**
 * Esegue la chiamata al backend e aggiorna l'UI.
 *
 * Gestisce il ciclo `inCorso`/`richiestaPendente`: se arriva una nuova richiesta
 * mentre questa è in volo, al termine ne viene avviata una sola aggiuntiva,
 * leggendo il testo più recente direttamente dall'editor.
 *
 * @param testo          Testo Typst da compilare in questa esecuzione.
 * @param tentativiBridge Retry concessi se `invoke` fallisce perché il bridge
 *                        Tauri non è ancora pronto (usato solo al primo avvio).
 */
async function eseguiCompilazione(testo: string, tentativiBridge = 0): Promise<void> {
  // Cattura tipo e generazione all'AVVIO della richiesta: il tipo va riletto qui
  // (non al ritorno dell'await, quando potrebbe essere cambiato per un "Nuovo"/
  // apertura/conversione in volo) e la generazione serve a scartare un risultato
  // ormai obsoleto.
  const md = tipoDocumento === "markdown";
  const token = ciclo.inizia();

  // Aggiorna l'interfaccia durante la compilazione.
  btnCompila.disabled = true;
  spanStato.textContent = "Compilazione…";

  let haErroreInfra = false;
  let erroreInfra: unknown = null;
  let risultato: RisultatoCompilazione | null = null;

  const inizio = performance.now();
  try {
    // Modalità Markdown: l'anteprima passa dalla conversione md→typst→compilazione
    // (comando `anteprima_markdown`); il Typst nativo usa `compila`.
    const comando = md ? "anteprima_markdown" : "compila";
    risultato = await invoke<RisultatoCompilazione>(comando, { sorgente: testo });
    ultimaDurataMs = performance.now() - inizio;
  } catch (e: unknown) {
    // Un Err dal backend è un guasto infrastrutturale (task abortito) o il bridge
    // non ancora pronto: NON è un errore di compilazione, che ora arriva sempre
    // dentro `risultato.diagnostici` con la compilazione che "va a buon fine".
    haErroreInfra = true;
    erroreInfra = e;
  } finally {
    btnCompila.disabled = false;
  }
  const esito = ciclo.termina(token, timerDebounce !== null);

  // Gestione fuori dal `finally`, per decidere fra retry del bridge e UI senza
  // intrecciarsi con i flag di stato.
  if (haErroreInfra) {
    if (tentativiBridge > 0) {
      // Bridge Tauri probabilmente non ancora pronto all'avvio: ritenta a breve
      // tramite `avviaCompilazione`, che rispetta la guardia `inCorso` (se nel
      // frattempo è partita un'altra compilazione la lascia proseguire, invece di
      // avviarne una concorrente) e rilegge comunque il testo più recente dall'editor.
      setTimeout(() => avviaCompilazione(), 300);
      return;
    }
    mostraErrori(erroreInfra);
    spanStato.textContent = "errore";
  } else if (risultato !== null && esito.applica) {
    aggiornaConRisultato(risultato, md);
    ultimoTestoCompilato = testo;
  } else if (!esito.applica) {
    // Documento sostituito durante il volo: risultato scartato. Lo stato non deve
    // restare su «Compilazione…»; la ricompilazione (sotto, o il debounce già
    // armato) lo aggiornerà.
    spanStato.textContent = "In attesa…";
  }

  // Coalescing: se è arrivata una nuova richiesta durante la compilazione (o il
  // risultato è stato scartato senza un debounce in attesa), ricompila subito
  // leggendo il testo più recente dall'editor.
  if (esito.ricompila) {
    void eseguiCompilazione(editor.testo());
  }
}

/**
 * Applica all'UI il risultato di una compilazione.
 *
 * - Diagnostici inline nell'editor (errori/avvisi posizionati).
 * - Anteprima aggiornata SOLO se sono state prodotte pagine: in caso di errore
 *   fatale (`pagine` vuoto) l'ultima anteprima valida resta visibile.
 * - Riquadro inferiore per i diagnostici senza posizione (es. dentro pacchetti),
 *   così nessun messaggio va perso.
 * - Conteggio di errori/avvisi (e pagine) nello stato.
 */
function aggiornaConRisultato(r: RisultatoCompilazione, md: boolean): void {
  // In Markdown i diagnostici si riferiscono al Typst GENERATO dalla conversione:
  // i loro offset non hanno posizione valida nel sorgente Markdown, quindi niente
  // inline (si mostrano solo nel riquadro inferiore, con una nota).
  editor.mostraDiagnostici(md ? [] : r.diagnostici);

  const errori = r.diagnostici.filter((d) => d.gravita === "errore");
  ultimiErroriCompilazione = errori.length;
  const avvisi = r.diagnostici.filter((d) => d.gravita === "avviso");

  pagineUltimaCompilazione = r.pagine.length;
  if (r.pagine.length > 0) {
    anteprima.aggiorna(r.pagine);
  } else if (errori.length === 0) {
    // Documento legittimamente vuoto (compilato senza errori, 0 pagine): pulisci
    // l'anteprima mostrando il placeholder. In caso di errore fatale (0 pagine CON
    // errori) si conserva invece l'ultima anteprima valida (non entra in questo ramo).
    anteprima.aggiorna([]);
  }

  // Riquadro inferiore: TUTTI i diagnostici, ordinati (errori prima, per posizione), con
  // riga/colonna calcolate sul testo corrente; le righe con posizione portano al punto
  // nell'editor. In Markdown gli offset sono del Typst generato: nessuna posizione.
  riquadroErrori.mostra(r.diagnostici, editor.testo(), md);

  // Stato: conteggio errori/avvisi e, se non ci sono errori, numero di pagine.
  const parti: string[] = [];
  const riepilogo = riepilogoDiagnostici(errori.length, avvisi.length);
  if (riepilogo !== "") parti.push(riepilogo);
  if (errori.length === 0) parti.push(`${r.pagine.length} pagina/e — Pronto`);
  spanStato.textContent = parti.join(", ");

  // Indice del documento (F5-020): il core conserva l'ultimo valido anche se questa
  // compilazione è fallita, quindi si rilegge comunque (se il pannello è visibile).
  pannelloIndice?.aggiorna();
  if (ripetiSincro) {
    // Primo risultato dopo l'apertura di un file dall'indice: l'anteprima va sul titolo.
    ripetiSincro = false;
    sincro?.daCursore(editor.cursore());
  }
}

/**
 * Mostra i messaggi di errore nel riquadro dedicato.
 *
 * @param errori Valore lanciato da Tauri: array di stringhe o stringa singola.
 */
function mostraErrori(errori: unknown): void {
  let testo: string;

  if (Array.isArray(errori)) {
    testo = (errori as string[]).join("\n");
  } else if (typeof errori === "string") {
    testo = errori;
  } else if (errori instanceof Error) {
    // Eccezione JS nativa (es. errore I/O dal plugin dialog, non un Err Tauri).
    testo = errori.message;
  } else {
    testo = "Errore sconosciuto.";
  }

  riquadroErrori.mostraTesto(testo);
}

/**
 * Aggiorna l'indicazione del file corrente nell'header e nel titolo finestra.
 * Il pallino accanto al nome (verde / ambra / rosso, con forme diverse) segnala lo stato di
 * salvataggio; nel titolo finestra resta "•" per le modifiche non salvate.
 *
 * @param stato istantanea del file corrente (percorso + flag modificato).
 */
function mostraStatoFile(stato: StatoFile): void {
  const nome = stato.percorso ? nomeFile(stato.percorso) : "(nuovo documento)";
  const etichetta = nome + (stato.modificato ? " •" : "");
  spanNomeFile.textContent = nome;
  spanNomeFile.title = stato.percorso ?? "";
  document.title = `Typster — ${etichetta}`;
  ultimoStatoFile = stato;
  aggiornaPallino();
  // L'albero dei file evidenzia il documento aperto (e si rilegge se è cambiato).
  pannelloFile?.documentoCambiato(stato.percorso);
}

/**
 * Mostra gli avvisi di una conversione (testo, HTML, LaTeX, Markdown) in un
 * riquadro chiudibile sotto l'editor; nessun avviso = riquadro nascosto. Con
 * errori di sintassi nel sorgente l'avviso è sempre in testa.
 */
function mostraAvvisiConversione(formato: FormatoEsportazione, esito: EsitoConversione): void {
  const testo = testoAvvisiConversione(formato, esito);
  if (testo === null) {
    boxAvvisiConversione.classList.remove("visibile");
    return;
  }
  spanAvvisiConversione.textContent = testo;
  boxAvvisiConversione.classList.add("visibile");
}

/**
 * Esporta in PDF/PNG/SVG o per conversione (testo, HTML, LaTeX, Markdown)
 * con feedback nello stato e guardia anti doppio-click.
 *
 * Prima si chiedono le opzioni (pagine, standard PDF/A, PPI) con la modale,
 * poi il percorso con il dialog di sistema. L'export può essere lento su
 * documenti complessi: si disabilita il pulsante per l'intera operazione
 * (evita due export concorrenti) e si aggiorna lo stato. Gli errori sono già
 * mostrati dal gestore (`mostraErrori`).
 */
async function esportaConFeedback(formato: FormatoEsportazione = "pdf"): Promise<void> {
  if (btnEsporta.disabled) {
    return; // operazione già in corso
  }
  btnEsporta.disabled = true;
  const statoPrecedente = spanStato.textContent;

  let esportati = 0;
  try {
    const opzioni = await dialogoEsporta.chiedi(formato);
    if (opzioni !== null) {
      spanStato.textContent = `Esportazione ${NOME_FORMATO[formato]}…`;
      if (tipoDocumento === "markdown") {
        // L'export compila sempre Typst: in modalità Markdown si converte prima
        // il documento in Typst e si esporta il risultato della conversione.
        const typ = await invoke<string>("converti_md_a_typst", {
          sorgente: editor.testo(),
        });
        esportati = await gestoreFile.esporta(formato, opzioni, typ);
      } else {
        esportati = await gestoreFile.esporta(formato, opzioni);
      }
    }
  } catch (e) {
    mostraErrori(e);
  }

  btnEsporta.disabled = false;
  spanStato.textContent = esportati > 0 ? messaggioEsito(formato, esportati) : (statoPrecedente ?? "");
}

// ---- Collegamento eventi ----

/**
 * Click sul bottone "Compila": trigger manuale immediato.
 *
 * Azzera il timer di debounce in corso (evita una doppia compilazione)
 * e avvia subito la compilazione con il testo corrente.
 */
btnCompila.addEventListener("click", (): void => compilaOra());

/**
 * Invalida la cache dei file fratelli (inclusi, importati, immagini) e ricompila:
 * comando "Ricarica risorse" e ripiego al focus se il watcher non è attivo (F5-014).
 */
async function ricaricaRisorse(): Promise<void> {
  await invoke("invalida_risorse");
  compilaOra();
  spanStato.textContent = "Risorse ricaricate";
}

/** Mostra o nasconde la barra laterale; nascondendola il fuoco torna all'editor. */
function alternaBarra(): void {
  barra.alternaVisibilita();
  if (!barra.visibile()) {
    editor.focus();
  }
}

/** `true` se la modalità focus è attiva (paragrafo del cursore in evidenza, F5-023). */
let modalitaFocus = false;
/** `true` se la macchina da scrivere è attiva (riga del cursore a metà altezza, F5-023). */
let modalitaMacchina = false;

/** Accende/spegne la modalità focus; `salva` = false all'avvio (si applica lo stato già salvato). */
function impostaModalitaFocus(attiva: boolean, salva = true): void {
  modalitaFocus = attiva;
  editor.impostaFocus(attiva);
  if (salva) {
    spanStato.textContent = attiva ? "Modalità focus attiva" : "Modalità focus disattivata";
    void salvaImpostazioni({ modalita_focus: attiva }).catch((e: unknown) =>
      console.warn("Modalità focus non salvata:", e),
    );
  }
}

/** Accende/spegne la macchina da scrivere; `salva` = false all'avvio. */
function impostaModalitaMacchina(attiva: boolean, salva = true): void {
  modalitaMacchina = attiva;
  editor.impostaMacchina(attiva);
  if (salva) {
    spanStato.textContent = attiva
      ? "Macchina da scrivere attiva"
      : "Macchina da scrivere disattivata";
    void salvaImpostazioni({ modalita_macchina: attiva }).catch((e: unknown) =>
      console.warn("Macchina da scrivere non salvata:", e),
    );
  }
}

/** Foglio con le regole `@page` della stampa, costruito da codice (nessuno stile inline: CSP, B-005). */
const foglioStampa = new CSSStyleSheet();
document.adoptedStyleSheets = [...document.adoptedStyleSheets, foglioStampa];
/** `true` da quando si apre la finestra di stampa a quando si chiude. */
let dialogoStampaAperto = false;

/** Chiusa la finestra di stampa si torna al caricamento normale delle pagine (idempotente). */
function chiudiStampa(): void {
  if (dialogoStampaAperto) {
    dialogoStampaAperto = false;
    anteprima.finisciStampa();
  }
}
window.addEventListener("afterprint", chiudiStampa);
// Se `afterprint` non arrivasse, il ritorno del fuoco alla finestra segnala la fine della stampa.
window.addEventListener("focus", chiudiStampa);

/** `true` se non c'è una compilazione in corso né in attesa del debounce. */
const compilazioneFerma = (): boolean => !ciclo.inCorso() && timerDebounce === null;

/**
 * Stampa il documento formattato: compila la versione attuale (se serve), carica tutte le
 * pagine dell'anteprima, dà a ogni pagina il suo formato carta e apre la finestra di stampa di
 * Windows (`window.print()` di WebView2: stampante, copie, intervallo, «Salva come PDF»).
 * Con errori di compilazione non stampa: l'anteprima mostrerebbe una versione vecchia.
 */
async function stampaDocumento(): Promise<void> {
  if (stampaInCorso) {
    return;
  }
  stampaInCorso = true;
  btnStampa.disabled = true;
  try {
    if (editor.testo() !== ultimoTestoCompilato) {
      spanStato.textContent = "Compilazione prima della stampa…";
      compilaOra();
      await attendiCondizione(compilazioneFerma, 30_000, 100);
      if (editor.testo() !== ultimoTestoCompilato) {
        spanStato.textContent = "Stampa annullata: la compilazione non è riuscita o il documento è cambiato. Riprova.";
        return;
      }
    }
    if (ultimiErroriCompilazione > 0) {
      spanStato.textContent = "Stampa annullata: correggi prima gli errori del documento.";
      return;
    }
    const statoPrecedente = spanStato.textContent ?? "";
    spanStato.textContent = "Preparazione della stampa…";
    const esito = await anteprima.preparaStampa();
    if (!esito.ok) {
      spanStato.textContent =
        esito.motivo === "vuoto"
          ? "Niente da stampare: il documento non ha pagine."
          : "Stampa annullata: le pagine non si sono caricate in tempo. Riprova.";
      return;
    }
    // Mentre si caricavano le pagine il documento può essere cambiato (compilazione in arrivo).
    if (editor.testo() !== ultimoTestoCompilato || !compilazioneFerma()) {
      anteprima.finisciStampa();
      spanStato.textContent = "Stampa annullata: il documento è cambiato durante la preparazione. Riprova.";
      return;
    }
    const regole = regoleStampa(esito.pagine);
    if (regole === null) {
      anteprima.finisciStampa();
      spanStato.textContent = "Stampa annullata: formato delle pagine non valido.";
      return;
    }
    foglioStampa.replaceSync(regole.join("\n"));
    spanStato.textContent = statoPrecedente;
    dialogoStampaAperto = true;
    window.print();
  } catch (e) {
    mostraErrori(e);
    dialogoStampaAperto = false;
    anteprima.finisciStampa();
  } finally {
    stampaInCorso = false;
    btnStampa.disabled = false;
  }
}

/** Compilazione manuale immediata (bottone "Compila" e comando `compila`). */
function compilaOra(): void {
  if (timerDebounce !== null) {
    clearTimeout(timerDebounce);
    timerDebounce = null;
  }
  avviaCompilazione();
}

// Operazioni su file: i gestori sono async ma gestiscono gli errori al loro
// interno (callback `mostraErrori`), perciò qui si scarta la Promise con `void`.
btnNuovo.addEventListener("click", (): void => {
  void gestoreFile.nuovo();
  editor.focus();
});
btnApri.addEventListener("click", (): void => void gestoreFile.apri());
btnApriCartella.addEventListener("click", (): void => void gestoreFile.apriCartella());
btnSalva.addEventListener("click", (): void => void gestoreFile.salva());
btnEsporta.addEventListener("click", (): void => void esportaConFeedback("pdf"));
btnStampa.addEventListener("click", (): void => void stampaDocumento());
elemento<HTMLButtonElement>("avvisi-conversione-chiudi").addEventListener("click", (): void => {
  boxAvvisiConversione.classList.remove("visibile");
});
btnPacchetti.addEventListener("click", (): void => barra.alterna(ID_PANNELLO_PACCHETTI));
btnBarra.addEventListener("click", (): void => alternaBarra());
btnLocal.addEventListener("click", (): void => gestoreLocal.apri());
btnConverti.addEventListener("click", (): void => void convertiDocumento());

// Toolbar di formattazione: un solo listener delega in base all'attributo
// `data-comando` del pulsante premuto. Ogni comando modifica il documento, quindi
// fa scattare `onModifica` (segna il file modificato e ricompila l'anteprima).
barraFormattazione.addEventListener("click", (ev: MouseEvent): void => {
  if (!(ev.target instanceof HTMLElement)) {
    return;
  }
  const pulsante = ev.target.closest<HTMLButtonElement>("button[data-comando]");
  if (pulsante === null) {
    return;
  }
  const comando = pulsante.dataset.comando;
  if (comando === undefined) {
    return;
  }
  // `eseguiComando` ignora silenziosamente un comando non riconosciuto (switch
  // senza default): un eventuale refuso nell'HTML è un no-op innocuo.
  editor.formatta(comando as ComandoFormattazione);
});

// ---- Registro comandi e palette (F5-006) ----

/**
 * Registro unico dei comandi: alimenta le scorciatoie e la palette. `Mod` nelle
 * scorciatoie è Ctrl su Windows/Linux e Cmd su macOS (vedi `comandi.ts`). Gli
 * errori di un comando finiscono nel riquadro errori come per i pulsanti.
 */
const registro: RegistroComandi = creaRegistro(mostraErrori, rilevaMac());
const palette: Palette = creaPalette(registro, editor);

/** Comandi di formattazione: inseriscono markup Typst, quindi solo per documenti Typst. */
const FORMATTAZIONE: [ComandoFormattazione, string][] = [
  ["grassetto", "Grassetto"],
  ["corsivo", "Corsivo"],
  ["codice", "Codice inline"],
  ["titolo", "Titolo"],
  ["lista", "Lista puntata"],
  ["lista-numerata", "Lista numerata"],
  ["link", "Collegamento"],
  ["immagine", "Immagine"],
  ["tabella", "Tabella"],
];

registro.registra({
  id: "palette.apri",
  titolo: "Palette comandi",
  // Ctrl+P è anche la stampa della webview: il dispatcher fa preventDefault.
  scorciatoie: ["Mod+P", "Mod+Shift+P"],
  esegui: () => (palette.aperta() ? palette.chiudi() : palette.apri()),
});
registro.registra({
  id: "file.nuovo",
  titolo: "Nuovo documento",
  scorciatoie: ["Mod+N"],
  esegui: async () => {
    await gestoreFile.nuovo();
    editor.focus();
  },
});
registro.registra({
  id: "strumenti.modelli",
  titolo: "Nuovo da modello…",
  esegui: () => {
    barra.apri("strumenti");
    pannelloStrumenti?.apriModelli();
  },
});
registro.registra({
  id: "file.apri",
  titolo: "Apri file…",
  scorciatoie: ["Mod+O"],
  esegui: () => gestoreFile.apri(),
});
registro.registra({
  id: "file.apriCartella",
  titolo: "Apri cartella-progetto…",
  scorciatoie: ["Mod+Shift+O"],
  esegui: () => gestoreFile.apriCartella(),
});
registro.registra({
  id: "file.salva",
  titolo: "Salva",
  scorciatoie: ["Mod+S"],
  esegui: () => gestoreFile.salva(),
});
registro.registra({
  id: "file.cronologia",
  titolo: "Cronologia versioni…",
  esegui: () => dialogoCronologia.apri(),
});
registro.registra({
  id: "file.autosave",
  titolo: () =>
    leggiImpostazioni().autosave
      ? "Salvataggio automatico: disattiva"
      : "Salvataggio automatico: attiva",
  esegui: async () => {
    const attivo = !leggiImpostazioni().autosave;
    await salvaImpostazioni({ autosave: attivo });
    spanStato.textContent = attivo ? "Salvataggio automatico attivo" : "Salvataggio automatico disattivato";
    if (attivo) {
      void gestoreFile.autosalva();
    }
  },
});
registro.registra({
  id: "progetto.ricaricaRisorse",
  titolo: "Ricarica risorse del progetto",
  esegui: ricaricaRisorse,
});
registro.registra({
  id: "esporta.pdf",
  titolo: "Esporta in PDF…",
  scorciatoie: ["Mod+E"],
  esegui: () => esportaConFeedback("pdf"),
});
registro.registra({
  id: "esporta.png",
  titolo: "Esporta PNG…",
  esegui: () => esportaConFeedback("png"),
});
registro.registra({
  id: "esporta.svg",
  titolo: "Esporta SVG…",
  esegui: () => esportaConFeedback("svg"),
});
registro.registra({
  id: "esporta.testo",
  titolo: "Esporta come testo semplice…",
  esegui: () => esportaConFeedback("txt"),
});
registro.registra({
  id: "esporta.html",
  titolo: "Esporta come HTML…",
  esegui: () => esportaConFeedback("html"),
});
registro.registra({
  id: "esporta.latex",
  titolo: "Esporta come LaTeX…",
  esegui: () => esportaConFeedback("tex"),
});
registro.registra({
  id: "esporta.markdown",
  titolo: "Esporta come Markdown…",
  abilitato: () => tipoDocumento === "typst",
  esegui: () => esportaConFeedback("md"),
});
registro.registra({ id: "compila", titolo: "Compila ora", esegui: compilaOra });
registro.registra({
  id: "stampa",
  titolo: "Stampa il documento…",
  // Nessuna scorciatoia: Ctrl+P è della palette e le combinazioni con Alt (AltGr) sono escluse.
  abilitato: () => !stampaInCorso,
  esegui: stampaDocumento,
});
registro.registra({
  id: "documento.converti",
  titolo: () =>
    tipoDocumento === "markdown" ? "Converti in Typst" : "Converti in Markdown",
  abilitato: () => !btnConverti.disabled,
  esegui: () => convertiDocumento(),
});
registro.registra({
  id: "pacchetti.alterna",
  titolo: "Pacchetti Typst Universe: mostra/nascondi",
  esegui: () => barra.alterna(ID_PANNELLO_PACCHETTI),
});
registro.registra({
  id: "pacchetti.aggiornamenti",
  titolo: "Controlla aggiornamenti dei pacchetti",
  // Apre il pannello: a indice già caricato rifà il confronto (installati + import del documento).
  esegui: () => barra.apri(ID_PANNELLO_PACCHETTI),
});
// Barra laterale e pannelli (F5-019/020/021): i comandi dei pannelli nascono dal registro della barra.
registro.registra({
  id: "vista.barra",
  titolo: () => (barra.visibile() ? "Barra laterale: nascondi" : "Barra laterale: mostra"),
  // Mod+B resta libero per il grassetto; Mod+Shift+B (VS Code: build) non è di CodeMirror.
  scorciatoie: ["Mod+Shift+B"],
  esegui: alternaBarra,
});
for (const { id, titolo } of barra.pannelli()) {
  registro.registra({
    id: `vista.pannello.${id}`,
    titolo: `Barra laterale: pannello ${titolo}`,
    // Ctrl+Shift+E come l'Esplora risorse di VS Code; gli altri pannelli dalla palette o dall'activity bar.
    scorciatoie: id === ID_PANNELLO_FILE ? ["Mod+Shift+E"] : undefined,
    esegui: () => barra.apri(id),
  });
}
registro.registra({
  id: "vista.focus",
  titolo: () => (modalitaFocus ? "Modalità focus: disattiva" : "Modalità focus: attiva"),
  // Mod+Shift+F è riservata alla ricerca nel progetto (F7-003); Mod+Shift+D non è di CodeMirror.
  scorciatoie: ["Mod+Shift+D"],
  esegui: () => impostaModalitaFocus(!modalitaFocus),
});
registro.registra({
  id: "vista.macchina",
  titolo: () =>
    modalitaMacchina ? "Macchina da scrivere: disattiva" : "Macchina da scrivere: attiva",
  // F9 resta come alternativa, ma su macOS richiede Fn: la principale è Mod+Shift+T.
  scorciatoie: ["Mod+Shift+T", "F9"],
  esegui: () => impostaModalitaMacchina(!modalitaMacchina),
});
registro.registra({
  id: "pacchetti.pubblicaLocal",
  titolo: "Pubblica come pacchetto @local…",
  esegui: () => gestoreLocal.apri(),
});
// Stile grafico (classico / composizione / strumento) e menu «Aspetto». Nessuna scorciatoia: le
// combinazioni Mod+Shift+lettera libere rispetto a CodeMirror e a pannelli.check.ts sono poche,
// e il menu è raggiungibile dal pulsante «Aspetto» e dalla palette.
let menuAspetto: MenuAspetto | null = null;
registro.registra({
  id: "aspetto.menu",
  titolo: "Aspetto…",
  esegui: () => {
    menuAspetto?.apri();
  },
});
for (const stile of STILI) {
  registro.registra({
    id: `stile.${stile}`,
    titolo: `Stile: ${etichettaStile(stile)}`,
    esegui: () => impostaStile(stile),
  });
}
// Menu «Opzioni» e motore di evidenziazione Typst (wasm / lezer). Anche qui nessuna scorciatoia.
let menuOpzioni: MenuOpzioni | null = null;
registro.registra({
  id: "opzioni.menu",
  titolo: "Opzioni…",
  esegui: () => {
    menuOpzioni?.apri();
  },
});
for (const motore of MOTORI) {
  registro.registra({
    id: `motore.${motore}`,
    titolo: titoloComandoMotore(motore),
    esegui: () => impostaMotore(motore),
  });
}
registro.registra({ id: "tema.alterna", titolo: "Tema: cambia (sistema, chiaro, scuro)", esegui: alternaTema });
registro.registra({ id: "tema.sistema", titolo: "Tema: sistema", esegui: () => impostaTema("sistema") });
registro.registra({ id: "tema.chiaro", titolo: "Tema: chiaro", esegui: () => impostaTema("chiaro") });
registro.registra({ id: "tema.scuro", titolo: "Tema: scuro", esegui: () => impostaTema("scuro") });
// Font: `=` e `_` sono i tasti senza Shift/con Shift di `+` e `-` (accettati come prima).
registro.registra({
  id: "font.aumenta",
  titolo: "Aumenta dimensione font",
  scorciatoie: ["Mod++", "Mod+="],
  ripetibile: true,
  esegui: aumentaFont,
});
registro.registra({
  id: "font.riduci",
  titolo: "Riduci dimensione font",
  scorciatoie: ["Mod+-", "Mod+_"],
  ripetibile: true,
  esegui: riduciFont,
});
registro.registra({
  id: "font.ripristina",
  titolo: "Ripristina dimensione font",
  scorciatoie: ["Mod+0"],
  esegui: ripristinaFont,
});
for (const [comando, titolo] of FORMATTAZIONE) {
  registro.registra({
    id: `formato.${comando}`,
    titolo: `Formato: ${titolo}`,
    abilitato: () => tipoDocumento === "typst",
    esegui: () => {
      editor.focus();
      editor.formatta(comando);
    },
  });
}

/** Id inesistente: passato come `eseguiSolo` non fa partire nessun comando (il tasto è comunque consumato). */
const NESSUN_COMANDO = "\0";

/**
 * Scorciatoie da tastiera: un solo listener sul registro, in fase di cattura così
 * arriva prima delle mappe tasti di CodeMirror. Con la palette aperta agisce solo
 * la sua scorciatoia (per chiuderla): gli altri tasti vanno al campo di ricerca.
 * Con un altro modale aperto (esporta, cronologia, @local) le scorciatoie sono
 * consumate ma non eseguono nulla: Ctrl+O non deve cambiare il documento sotto
 * un export in corso, né la palette aprirsi sopra il modale.
 */
window.addEventListener(
  "keydown",
  (ev: KeyboardEvent): void => {
    const altroModale = document.querySelector(".modal-overlay:not([hidden]):not(#palette)") !== null;
    registro.gestisciTasto(
      ev,
      altroModale ? NESSUN_COMANDO : palette.aperta() ? "palette.apri" : undefined,
    );
  },
  true,
);

// Tooltip dei pulsanti con la scorciatoia della piattaforma (Ctrl+… o ⌘…): i title
// statici di index.html mostrano Ctrl e qui vengono riscritti dal registro.
for (const [id, descrizione, comando] of [
  ["btn-barra", "Mostra o nascondi la barra laterale", "vista.barra"],
  ["btn-nuovo", "Nuovo documento", "file.nuovo"],
  ["btn-apri", "Apri un file .typ o .md", "file.apri"],
  ["btn-apri-cartella", "Apri una cartella-progetto", "file.apriCartella"],
  ["btn-salva", "Salva il documento", "file.salva"],
] as const) {
  const sc = registro.ottieni(comando)?.scorciatoie?.[0];
  if (sc !== undefined) {
    elemento<HTMLButtonElement>(id).title = `${descrizione} (${formattaScorciatoia(sc, registro.mac)})`;
  }
}
const scEsporta = registro.ottieni("esporta.pdf")?.scorciatoie?.[0];
if (scEsporta !== undefined) {
  btnEsporta.title =
    `Esporta in PDF (${formattaScorciatoia(scEsporta, registro.mac)}); PNG, SVG, testo, HTML, LaTeX e Markdown dalla palette comandi`;
}

// ---- Avvio ----

// Mostra subito lo stato iniziale del file ("(nuovo documento)").
mostraStatoFile(gestoreFile.stato());
// Il pallino segue anche la scrittura su disco (in volo / esito), senza altre notifiche.
ultimaScrittura = gestoreFile.statoScrittura();
gestoreFile.suSalvataggio((s) => {
  ultimaScrittura = s;
  aggiornaPallino();
});
aggiornaPallino();

// Allinea la UI al tipo iniziale (Typst): etichetta del pulsante di conversione,
// visibilità della toolbar di formattazione.
aggiornaUiTipo();

// Carica gli snippet in background: appena pronti compaiono nel completamento.
void caricaSnippet();
void caricaFirme();
// Stato salvato della barra laterale e delle modalità di scrittura (F5-019/F5-023):
// si applica appena le impostazioni sono lette; fino ad allora la barra resta nascosta.
void caricaImpostazioni().then((imp) => {
  barra.ripristina({
    visibile: imp.barra_visibile,
    larghezza: imp.barra_larghezza,
    pannello: imp.pannello_attivo,
  });
  aggiornaAriaBarra(imp.barra_visibile, imp.pannello_attivo);
  impostaModalitaFocus(imp.modalita_focus, false);
  impostaModalitaMacchina(imp.modalita_macchina, false);
});
// Carica le impostazioni utente (F5-003) e applica tema e font (F5-004/005).
inizializzaAspetto(editor, (msg) => {
  spanStato.textContent = msg;
});
menuAspetto = creaMenuAspetto(
  elemento<HTMLButtonElement>("btn-aspetto"),
  elemento<HTMLElement>("menu-aspetto"),
  () => editor.focus(),
);
menuOpzioni = creaMenuOpzioni(
  elemento<HTMLButtonElement>("btn-opzioni"),
  elemento<HTMLElement>("menu-opzioni"),
  () => editor.focus(),
);

// File modificati da fuori (F5-022): il guscio decide, qui si applica. `invalida`
// (un file fratello è cambiato: il guscio ha già svuotato la cache) → si ricompila;
// il resto riguarda il documento aperto e lo gestisce il gestore file.
void listen<EventoEsterno>("modifica-esterna", (evento): void => {
  const ev = evento.payload;
  // File creati/eliminati/rinominati da fuori: l'albero dei file si rilegge (F5-021).
  if (ev.azione === "albero" || ev.azione === "sparito" || ev.azione === "rinominato") {
    pannelloFile?.aggiorna();
  }
  if (ev.azione === "invalida") {
    compilaOra();
  } else {
    void gestoreFile.gestisciModificaEsterna(ev);
  }
}).catch((e: unknown) => console.warn("Ascolto delle modifiche esterne non attivo:", e));

// Chiusura della finestra (F5-013): con modifiche non salvate si chiede «Salva / Non
// salvare / Annulla» (Non salvare le copia in cronologia). Annulla, o un salvataggio
// non riuscito, annulla la chiusura. Se la domanda stessa fallisce si tenta di mettere
// il testo in cronologia e la prima volta si resta aperti con un avviso; alla seconda
// richiesta di chiusura (`chiusuraForzata`) l'app si chiude comunque.
/** `true` mentre il controllo alla chiusura è in corso (un secondo clic non ne apre un altro). */
let chiusuraInCorso = false;
/** `true` dopo un fallimento della domanda di chiusura: la richiesta successiva chiude senza chiedere. */
let chiusuraForzata = false;
try {
  void getCurrentWindow()
    .onCloseRequested(async (evento): Promise<void> => {
      if (chiusuraInCorso) {
        evento.preventDefault(); // la domanda è già aperta
        return;
      }
      if (chiusuraForzata) {
        return; // secondo tentativo dopo un guasto della domanda: si chiude
      }
      chiusuraInCorso = true;
      try {
        // Con il salvataggio automatico attivo si attende un salvataggio prima di
        // chiedere: se va a buon fine non c'è nulla da chiedere. Le regole
        // esistenti restano: su conflitto o file sparito `autosalva` NON forza
        // nulla e la domanda «Salva / Non salvare / Annulla» resta.
        if (leggiImpostazioni().autosave && gestoreFile.stato().percorso !== null) {
          await gestoreFile.autosalva();
        }
        if (!(await gestoreFile.proteggiModifiche("chiudere"))) {
          evento.preventDefault();
        }
      } catch (e) {
        // La domanda è fallita e ci sono modifiche non salvate: prima si prova a
        // metterle in cronologia (se il percorso è noto); poi, al PRIMO fallimento, si
        // resta aperti con un avviso, e alla richiesta successiva si lascia chiudere
        // (un guasto non deve mai intrappolare l'utente).
        mostraErrori(e);
        const stato = gestoreFile.stato();
        if (stato.modificato) {
          if (stato.percorso !== null) {
            try {
              // Dal gestore file: stessa forma (fine riga del file) che scrive `salva_file`.
              await gestoreFile.registraInCronologia();
            } catch (e2) {
              console.warn("Cronologia non raggiunta alla chiusura:", e2);
            }
          }
          chiusuraForzata = true;
          evento.preventDefault();
          spanStato.textContent =
            "Impossibile chiedere conferma: premi di nuovo X per chiudere";
        }
      } finally {
        chiusuraInCorso = false;
      }
    })
    .catch((e: unknown) => console.warn("Controllo alla chiusura non attivo:", e));
} catch (e) {
  console.warn("Controllo alla chiusura non attivo:", e);
}

// Salvataggio automatico anche quando la finestra perde il focus (F5-013).
window.addEventListener("blur", (): void => void gestoreFile.autosalva());

// Ripiego di F5-014: se il watcher non è attivo, al ritorno nella finestra si
// invalidano i file fratelli e si ricompila (un altro programma può averli cambiati).
window.addEventListener("focus", (): void => {
  if (!gestoreFile.osservazioneAttiva() && gestoreFile.stato().percorso !== null) {
    void ricaricaRisorse().catch(mostraErrori);
  }
});

// La prima compilazione è agganciata all'evento "load": il DOM è completo e il
// bridge Tauri è quasi sempre già pronto. Per coprire il caso (raro, in dev) in
// cui non lo sia ancora, si concede 1 retry tramite `tentativiBridge`.
window.addEventListener("load", (): void => {
  // Passa da `avviaCompilazione` (guardia `ciclo.richiedi()`): una chiamata diretta
  // a `eseguiCompilazione` poteva sovrapporsi a una compilazione già partita.
  avviaCompilazione(1);
});
