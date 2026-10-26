/**
 * fileops.ts — Operazioni su file dell'editor (Fase 1, esteso in Fase 6).
 *
 * Incapsula apertura, salvataggio ed export (PDF/PNG/SVG), mantenendo lo stato del file
 * corrente (percorso, flag "modificato", tipo di documento). Il dialog di sistema
 * (plugin Tauri `dialog`) serve solo a far scegliere all'utente un percorso; la
 * lettura e la scrittura effettive avvengono nei comandi Rust del backend
 * (`apri_file`, `salva_file`, `esporta_pdf`, `esporta_immagini`), che hanno accesso pieno al filesystem.
 *
 * # Tipo di documento (Fase 6)
 * Il gestore riconosce `.typ` (Typst) e `.md` (Markdown) dall'estensione: guida
 * il filtro di salvataggio e viene notificato al resto del frontend via `onTipo`,
 * così l'editor imposta l'evidenziazione e `main.ts` sceglie la pipeline di
 * anteprima (compilazione diretta per Typst, conversione md→typst per Markdown).
 *
 * # Salvataggio automatico e cronologia (F5-013)
 * Con `autosave` attivo un timer salva il file dopo `autosave_ms` dall'ultima
 * modifica (e `autosalva()`, chiamato da main.ts, al blur della finestra). Il
 * backend registra una versione in cronologia a ogni salvataggio; prima di
 * sostituire il testo dell'editor (ripristino, ricarica da disco) la versione
 * corrente viene registrata: niente si perde.
 *
 * # File modificati da fuori (F5-022)
 * Il guscio emette `modifica-esterna`; `gestisciModificaEsterna` applica la
 * decisione: ricarica se il documento è pulito, **chiede** se ha modifiche non
 * salvate (mai sovrascritte senza risposta), ripristina/segnala se il file è
 * sparito, segue una rinomina. Lo stato dell'editor è la fonte di verità: se
 * il guscio dice "ricarica" ma nel frattempo sono arrivate modifiche, si chiede.
 *
 * # Cambio di documento con modifiche non salvate
 * Prima di sostituire o chiudere il documento (`proteggiModifiche`) si chiede
 * «Salva / Non salvare / Annulla»: Salva scrive sul file (o "Salva con nome" se
 * è nuovo) e se non riesce o il file è in conflitto/sparito annulla l'operazione;
 * Non salvare copia il testo in cronologia e prosegue; Annulla non fa nulla.
 *
 * Il modulo non conosce il DOM: comunica stato, tipo ed errori tramite le
 * callback iniettate in `creaGestoreFile` (separazione presentazione/logica).
 */

import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type { Editor, TipoDocumento } from "./editor";
import {
  argomentiConversione,
  comandoConversione,
  eConversione,
  filtriSalvataggio,
  nomePredefinito,
  testoConflitto,
  opzioniPdfPerBackend,
  pagineOppureNull,
  testoErroreEsportazione,
} from "./opzioniesporta";
import type { EsitoConversione, FormatoEsportazione, OpzioniEsportazione } from "./opzioniesporta";
import { leggiImpostazioni, salvaImpostazioni } from "./impostazioni";
import { avvisoTipoDopoRinomina, azioneDopoSparito, confrontaPercorsi, discoInvariato, ricaricaSuperflua, motivoNonRipristinato, stessoPercorso } from "./modificheesterne";
import { impostaTestoConRicompilazione } from "./ciclocompilazione";
import type { EventoEsterno } from "./modificheesterne";
import { applicaFineRiga, contenutoPerCronologia, normalizzaFineRiga, rilevaFineRiga } from "./finerighe";
import type { FineRiga } from "./finerighe";

/** Filtri del dialog di apertura: Typst, Markdown, o entrambi. */
const FILTRI_APERTURA = [
  { name: "Documenti (Typst/Markdown)", extensions: ["typ", "md"] },
  { name: "Typst", extensions: ["typ"] },
  { name: "Markdown", extensions: ["md"] },
];

/** Risposta alla domanda sulle modifiche non salvate. */
type SceltaCambio = "salva" | "non-salvare" | "annulla";
const ETICHETTA_SALVA = "Salva";
const ETICHETTA_NON_SALVARE = "Non salvare";
const ETICHETTA_ANNULLA = "Annulla";

/** Esito del comando `esporta_immagini` (enum con tag `esito` nel guscio). */
type EsitoImmagini =
  | { esito: "Scritti"; file: number }
  | { esito: "Conflitto"; esistenti: string[] };

/** Deduce il tipo di documento dall'estensione del percorso (default Typst). */
export function tipoDaPercorso(percorso: string): TipoDocumento {
  return /\.md$/i.test(percorso) ? "markdown" : "typst";
}

/** Una versione della cronologia locale, come restituita da `cronologia_elenca`. */
export interface VersioneCronologia {
  /** Identificatore (nome dello snapshot). */
  id: string;
  /** Istante, `AAAA-MM-GG HH:MM:SS` (ora locale). */
  quando: string;
  /**
   * Istante UTC in RFC 3339 (campo additivo del backend, `vista_locale` resta in
   * `quando`): se presente, l'UI lo formatta nella lingua dell'interfaccia.
   */
  quando_utc?: string | null;
  /** Dimensione in byte. */
  dimensione: number;
}

/** Istantanea dello stato del file corrente, per l'aggiornamento dell'UI. */
export interface StatoFile {
  /** Percorso del file corrente su disco, o `null` se nuovo/non salvato. */
  percorso: string | null;
  /** `true` se il documento ha modifiche non salvate. */
  modificato: boolean;
}

/** API pubblica del gestore file verso il resto del frontend. */
export interface GestoreFile {
  /**
   * Crea un documento nuovo e vuoto (Typst nativo), scollegato da qualsiasi file
   * su disco: azzera l'editor, resetta il percorso e il flag "modificato", e
   * scollega la radice-progetto sul backend (comando `chiudi_progetto`).
   */
  nuovo(): Promise<void>;
  /**
   * Come `nuovo`, ma il documento parte con `testo` (es. generato da un modello):
   * resta non salvato e «modificato», quindi la chiusura chiede se salvarlo.
   * `true` se il documento è stato creato; `false` se l'utente ha rifiutato di sostituire il
   * documento aperto o c'è stato un errore (già segnalato): in quel caso non cambia nulla.
   */
  nuovoDa(testo: string): Promise<boolean>;
  /** Apre un .typ/.md scelto dall'utente e ne carica il contenuto nell'editor. */
  apri(): Promise<void>;
  /**
   * Apre una cartella-progetto: rileva l'entrypoint, aggancia la radice (così
   * import/risorse relativi si risolvono) e ne carica il contenuto nell'editor.
   */
  apriCartella(): Promise<void>;
  /**
   * Apre come documento un file `.typ`/`.md` della cartella-progetto già
   * agganciata, mantenendone la radice (indice e albero dei file, F5-020/F5-021).
   * `relativo` è il percorso relativo alla radice, con `/`. Prima mette al sicuro
   * le modifiche non salvate (come le altre aperture). `true` se il file è stato
   * aperto; `false` se l'utente ha rifiutato o c'è stato un errore (già mostrato).
   */
  apriFileProgetto(relativo: string): Promise<boolean>;
  /** Salva sul percorso corrente; se assente, chiede "salva con nome". */
  salva(): Promise<void>;
  /**
   * Salvataggio automatico: scrive sul percorso corrente se `autosave` è attivo,
   * il documento ha modifiche e non c'è un conflitto in sospeso. Mai un dialog.
   */
  autosalva(): Promise<void>;
  /** Applica un evento `modifica-esterna` del guscio (ricarica, chiedi, sparito, rinomina). */
  gestisciModificaEsterna(evento: EventoEsterno): Promise<void>;
  /** Versioni del file corrente nella cronologia locale (vuoto se non salvato). */
  versioni(): Promise<VersioneCronologia[]>;
  /** Testo di una versione della cronologia, senza applicarlo. */
  testoVersione(id: string): Promise<string>;
  /**
   * Ripristina una versione nell'editor. Prima registra il testo corrente in
   * cronologia (se non si può, non sostituisce nulla); il risultato resta
   * "modificato" finché l'utente non salva.
   */
  ripristinaVersione(id: string): Promise<boolean>;
  /** `true` se il guscio sta osservando la cartella del documento (F5-022). */
  osservazioneAttiva(): boolean;
  /**
   * Esporta il documento corrente (PDF, PNG, SVG oppure, per conversione del
   * sorgente, testo semplice, HTML, LaTeX o Markdown) su un percorso scelto
   * dall'utente, con le opzioni già raccolte dalla modale (F5-011). Per le
   * conversioni gli avvisi sulle perdite arrivano a `onConversione`.
   * @param formato formato di export.
   * @param opzioni pagine, standard PDF e risoluzione PNG.
   * @param sorgenteTypst sorgente Typst da compilare; se assente usa il testo
   *        dell'editor. In modalità Markdown il chiamante passa qui il Typst
   *        già convertito (l'export compila sempre Typst).
   * @returns numero di file scritti (0 se annullato o in errore).
   */
  esporta(
    formato: FormatoEsportazione,
    opzioni: OpzioniEsportazione,
    sorgenteTypst?: string,
  ): Promise<number>;
  /** Segnala che il documento è stato modificato (da chiamare ad ogni edit). */
  segnaModificato(): void;
  /**
   * Registra il testo corrente dell'editor in cronologia nella stessa forma che
   * scrive `salva_file` (fine riga del file riapplicata). No-op senza percorso.
   */
  registraInCronologia(): Promise<void>;
  /**
   * Adotta come documento corrente un progetto già caricato (es. da template):
   * imposta il contenuto nell'editor e il percorso, senza segnare modifiche.
   * NON chiede nulla sulle modifiche non salvate: il chiamante ha già chiamato
   * `proteggiModifiche` PRIMA di creare il progetto (evita la doppia domanda).
   */
  apriProgetto(percorso: string, contenuto: string): Promise<void>;
  /**
   * Da chiamare prima di sostituire (o chiudere) il documento: se ha modifiche
   * non salvate chiede «Salva / Non salvare / Annulla». Salva: salvataggio
   * normale (Salva con nome se il documento è nuovo); se fallisce o finisce in
   * conflitto/sparito l'operazione è annullata. Non salvare: il testo va in
   * cronologia (se il percorso è noto) e si prosegue. `false` = non procedere.
   */
  proteggiModifiche(contesto: "sostituire" | "chiudere"): Promise<boolean>;
  /**
   * Registra che il documento è stato convertito in un altro formato (Fase 6):
   * cambia il tipo, lo scollega dal file su disco (il formato è cambiato) e lo
   * marca come nuovo documento non salvato, così il prossimo salvataggio chiede
   * un nome con l'estensione giusta.
   */
  convertitoA(tipo: TipoDocumento): void;
  /** Restituisce lo stato corrente del file. */
  stato(): StatoFile;
  /**
   * Registra l'ascoltatore dello stato di scrittura (pallino di salvataggio): è chiamato
   * quando una scrittura parte e quando finisce, con l'esito dell'ultima (`errore`).
   * Solo osservazione: non cambia né aggiunge scritture.
   */
  suSalvataggio(ascoltatore: (s: StatoScrittura) => void): void;
  /** Stato di scrittura corrente (per il primo disegno del pallino). */
  statoScrittura(): StatoScrittura;
}

/** Stato della scrittura su disco: in volo adesso, ed esito dell'ultima conclusa. */
export interface StatoScrittura {
  /** `true` mentre una richiesta di salvataggio (manuale o automatica) è in volo. */
  inSalvataggio: boolean;
  /** `true` se l'ultima scrittura conclusa è fallita (si azzera a una riuscita o non applicata). */
  errore: boolean;
}

/**
 * Crea il gestore dei file per un editor.
 *
 * @param editor     istanza dell'editor da cui leggere/in cui caricare il testo.
 * @param onStato    invocata ad ogni cambio di stato (percorso o flag modificato).
 * @param onErrore   invocata in caso di errore di I/O o di export (per mostrarlo).
 * @param onTipo     invocata ad ogni cambio di tipo di documento (Typst/Markdown).
 * @param onAvviso   messaggi informativi non di errore (es. "file ripristinato").
 * @param onRicompila richiesta di ricompilazione immediata (dopo che il World è
 *                   stato riagganciato a un nuovo percorso, es. rinomina esterna).
 * @param onConversione esito di un'esportazione per conversione (avvisi, errori
 *                   di sintassi) da mostrare all'utente in modo non fatale.
 */
export function creaGestoreFile(
  editor: Editor,
  onStato: (stato: StatoFile) => void,
  onErrore: (errore: unknown) => void,
  onTipo: (tipo: TipoDocumento) => void,
  onAvviso: (messaggio: string) => void = () => {},
  onRicompila: () => void = () => {},
  onConversione: (formato: FormatoEsportazione, esito: EsitoConversione) => void = () => {},
): GestoreFile {
  let percorso: string | null = null;
  let modificato = false;
  let tipoCorrente: TipoDocumento = "typst";
  // Fine riga del file su disco (LF/CRLF): l'editor lavora solo con `\n`, quindi
  // si rileva alla lettura e si riapplica a ogni scrittura (vedi finerighe.ts).
  let fineRiga: FineRiga = "\n";
  // `true` durante un caricamento da disco: sopprime il segnale "modificato"
  // emesso in modo sincrono da `editor.imposta` (non è una modifica dell'utente).
  let caricamento = false;
  // Timer del salvataggio automatico (riarmato a ogni modifica).
  let timerAutosave: ReturnType<typeof setTimeout> | null = null;
  // `true` mentre una richiesta di salvataggio è in volo (niente scritture sovrapposte).
  let salvataggioInCorso = false;
  // Esito dell'ultima scrittura (solo per il pallino): `true` se è fallita.
  let erroreScrittura = false;
  let ascoltatoreScrittura: (s: StatoScrittura) => void = () => {};
  const statoScrittura = (): StatoScrittura => ({
    inSalvataggio: salvataggioInCorso,
    errore: erroreScrittura,
  });
  // Salvataggio in volo (si risolve sempre): chi deve cambiare documento lo attende.
  let attesaSalvataggio: Promise<unknown> = Promise.resolve();
  // `true` dopo aver avvisato che il salvataggio automatico fallisce (un avviso solo).
  let avvisoAutosaveDato = false;
  // `true` mentre è aperta la domanda «Salva / Non salvare / Annulla».
  let domandaCambioAperta = false;
  // `true` mentre è aperta la domanda "quale versione tenere": niente autosave né altre domande.
  let conflittoAperto = false;
  // Testo (solo LF) dell'ultima lettura/scrittura riuscita del file del documento, o `null` se
  // ignoto (nuovo documento, conversione, dopo `riconosci_disco`). Serve a riconoscere gli eventi
  // `incerto` di un omonimo altrove senza leggere il disco in modo che nasconda un conflitto vero.
  let testoDisco: string | null = null;
  // `true` se il file è sparito e il testo non è stato ripristinato: l'autosave non
  // deve ricreare un file di cui non si è verificata l'integrità (solo Salva esplicito).
  let fileSparito = false;
  // Esito dell'ultima `osserva_documento`: se `false` main.ts ripiega sul focus (F5-014).
  let osservazione = false;
  // Coda che serializza `osserva_documento`, così l'ultimo stato comunicato è l'ultimo vero.
  let codaOsserva: Promise<unknown> = Promise.resolve();
  // Coda degli eventi esterni: uno alla volta, per non aprire due domande insieme.
  let codaEventi: Promise<void> = Promise.resolve();

  /** Notifica all'UI lo stato corrente e all'osservazione dei file quello del documento. */
  function notifica(): void {
    onStato({ percorso, modificato });
    const args = { percorso, nonSalvato: modificato };
    codaOsserva = codaOsserva
      .catch(() => {})
      .then(() => invoke<boolean>("osserva_documento", args))
      .then((attiva) => {
        osservazione = attiva as boolean;
      })
      .catch((e: unknown) => console.warn("Osservazione dei file non aggiornata:", e));
  }

  /** Imposta il tipo di documento e lo notifica (editor + main). */
  function impostaTipo(tipo: TipoDocumento): void {
    if (tipo !== tipoCorrente) {
      tipoCorrente = tipo;
      onTipo(tipo);
    }
  }

  /** Crea un documento nuovo e vuoto: azzera l'editor e scollega il file su disco. */
  async function nuovo(): Promise<void> {
    try {
      if (!(await proteggiModifiche("sostituire"))) {
        return;
      }
      await creaNuovo();
    } catch (e) {
      onErrore(e);
    }
  }

  /** Nuovo documento con un testo iniziale (vedi `GestoreFile.nuovoDa`). */
  async function nuovoDa(testo: string): Promise<boolean> {
    try {
      if (!(await proteggiModifiche("sostituire"))) {
        return false;
      }
      await creaNuovo();
      // `caricamento` è già tornato a false: l'editor segnala la modifica come per un edit
      // dell'utente, e la compilazione parte col solito debounce.
      impostaTestoConRicompilazione(editor, testo, onRicompila);
      return true;
    } catch (e) {
      onErrore(e);
      return false;
    }
  }

  /** Corpo di `nuovo` dopo la protezione delle modifiche. */
  async function creaNuovo(): Promise<void> {
    // Un nuovo documento è Typst nativo, come lo stato iniziale dell'editor.
    impostaTipo("typst");
    // Scollega la radice-progetto sul backend PRIMA di azzerare l'editor: senza
    // questo, dopo un "Apri cartella" il World continuerebbe a risolvere import e
    // immagini relativi (e i font) dalla cartella-progetto precedente, anche se il
    // documento mostrato è nuovo. Un errore qui non deve bloccare la creazione del
    // nuovo documento (principio "un errore non blocca l'editor").
    try {
      await invoke("chiudi_progetto");
    } catch (e) {
      onErrore(e);
    }
    // `caricamento` sopprime il segnale "modificato" emesso in modo sincrono da
    // `editor.imposta`: un documento appena creato è vuoto e "non modificato",
    // non una modifica dell'utente. La ricompila (anteprima vuota) parte comunque.
    caricamento = true;
    impostaTestoConRicompilazione(editor, "", onRicompila);
    caricamento = false;
    fineRiga = "\n";
    testoDisco = null;
    percorso = null;
    modificato = false;
    fileSparito = false;
    notifica();
  }

  /**
   * Prima di sostituire o chiudere il documento con modifiche non salvate chiede
   * «Salva / Non salvare / Annulla» (vedi intestazione del modulo). `true` = si
   * può procedere.
   */
  async function proteggiModifiche(contesto: "sostituire" | "chiudere"): Promise<boolean> {
    if (!modificato) {
      return true;
    }
    if (domandaCambioAperta) {
      return false; // una domanda è già aperta: un secondo comando non ne apre un'altra
    }
    domandaCambioAperta = true;
    try {
      const scelta = await chiediSalvataggio(contesto);
      if (scelta === "annulla") {
        return false;
      }
      return scelta === "salva" ? await salvaPerCambio() : await scartaInCronologia(contesto);
    } catch (e) {
      if (contesto === "chiudere") {
        throw e; // un guasto della domanda non deve intrappolare l'utente: decide main.ts
      }
      onErrore(e);
      return false; // nel dubbio non si perde nulla
    } finally {
      domandaCambioAperta = false;
    }
  }

  /**
   * La domanda a tre vie. Usa `message` con pulsanti personalizzati (permesso
   * `dialog:allow-message`); se non è disponibile ripiega su `ask` a due pulsanti
   * («Non salvare» / «Annulla»), sempre con il testo in cronologia.
   */
  async function chiediSalvataggio(contesto: "sostituire" | "chiudere"): Promise<SceltaCambio> {
    const nome = percorso !== null ? `«${nomeFile(percorso)}»` : "Il documento (mai salvato)";
    const azione = contesto === "chiudere" ? "chiudere Typster" : "sostituire il documento";
    const nota =
      percorso !== null
        ? "\n\nCon «Non salvare» le modifiche restano comunque nella cronologia versioni."
        : "\n\nCon «Non salvare» le modifiche andranno perse.";
    const testo = `${nome} ha modifiche non salvate.\n\nVuoi salvarle prima di ${azione}?${nota}`;
    try {
      const r = await message(testo, {
        title: "Modifiche non salvate",
        kind: "warning",
        buttons: { yes: ETICHETTA_SALVA, no: ETICHETTA_NON_SALVARE, cancel: ETICHETTA_ANNULLA },
      });
      if (r === ETICHETTA_SALVA || r === "Yes") {
        return "salva";
      }
      return r === ETICHETTA_NON_SALVARE || r === "No" ? "non-salvare" : "annulla";
    } catch (e) {
      console.warn("Dialogo a tre pulsanti non disponibile, ripiego a due:", e);
    }
    const scarta = await ask(
      `${testo}\n\n(Il salvataggio da questa domanda non è disponibile: annulla e usa Salva.)`,
      {
        title: "Modifiche non salvate",
        kind: "warning",
        okLabel: ETICHETTA_NON_SALVARE,
        cancelLabel: ETICHETTA_ANNULLA,
      },
    );
    return scarta ? "non-salvare" : "annulla";
  }

  /**
   * «Salva» dalla domanda: salvataggio normale senza ulteriori domande. `true`
   * solo se il testo è tutto su disco; un errore, un conflitto o un file sparito
   * annullano l'operazione (si decide con un Salva esplicito).
   */
  async function salvaPerCambio(): Promise<boolean> {
    await attesaSalvataggio; // un autosave in volo finisce prima
    if (!modificato) {
      return true;
    }
    const esito =
      percorso !== null ? await scriviSu(percorso, false, true) : await salvaConNome(undefined, true);
    if (esito === "scritto" && !modificato) {
      return true;
    }
    if (esito === "conflitto" || esito === "sparito") {
      onAvviso(
        "Salvataggio non eseguito: il file su disco è cambiato o sparito. " +
          "Operazione annullata: usa Salva per scegliere quale versione tenere.",
      );
    } else if (esito === "scritto") {
      onAvviso("Il testo è cambiato durante il salvataggio: operazione annullata.");
    } else if (esito !== null) {
      onAvviso("Salvataggio non riuscito: operazione annullata.");
    }
    return false;
  }

  /** «Non salvare»: il testo va in cronologia (se il percorso è noto) e si prosegue. */
  async function scartaInCronologia(contesto: "sostituire" | "chiudere"): Promise<boolean> {
    try {
      await registraInCronologia(editor.testo());
      return true;
    } catch (e) {
      onErrore(e);
      // La cronologia non è raggiungibile: nulla garantisce il recupero, si chiede.
      const azione = contesto === "chiudere" ? "chiudere" : "sostituire il testo";
      return ask(
        `Non è stato possibile registrare le modifiche non salvate nella cronologia. Vuoi ${azione} comunque?`,
        { title: "Modifiche non salvate", kind: "warning", okLabel: "Continua", cancelLabel: "Annulla" },
      );
    }
  }

  /** Apre un sorgente Typst/Markdown e lo carica nell'editor. */
  async function apri(): Promise<void> {
    try {
      // Con `multiple: false` il dialog ritorna `string | null` (null se annullato).
      const scelto = await open({
        multiple: false,
        directory: false,
        filters: FILTRI_APERTURA,
      });
      if (scelto === null) {
        return; // annullato dall'utente
      }
      const percorsoScelto = scelto as string;
      // Prima di toccare il World e l'editor: le modifiche non salvate sono al sicuro.
      if (!(await proteggiModifiche("sostituire"))) {
        return;
      }
      const contenuto = await invoke<string>("apri_file", {
        percorso: percorsoScelto,
      });
      // Il tipo va impostato PRIMA di caricare il testo, così l'evidenziazione è
      // già quella giusta quando il contenuto compare.
      impostaTipo(tipoDaPercorso(percorsoScelto));
      // `imposta` sostituisce il documento e fa scattare in modo sincrono
      // `onModifica`. Poiché un caricamento da disco NON è una modifica
      // dell'utente, lo si racchiude in `caricamento` per sopprimere il segnale
      // "modificato"; la compilazione dell'anteprima avviene comunque (voluta).
      // Fine riga rilevata sul testo del disco; all'editor va la forma normalizzata (solo LF).
      fineRiga = rilevaFineRiga(contenuto);
      caricamento = true;
      // Con testo identico CodeMirror non emette `docChanged`: la ricompilazione (nuova
      // radice/World) la chiede `impostaTestoConRicompilazione`.
      impostaTestoConRicompilazione(editor, normalizzaFineRiga(contenuto), onRicompila);
      caricamento = false;
      percorso = percorsoScelto;
      testoDisco = normalizzaFineRiga(contenuto);
      modificato = false;
      fileSparito = false;
      notifica();
    } catch (e) {
      onErrore(e);
    }
  }

  /** Apre un file della cartella-progetto già agganciata (radice invariata). */
  async function apriFileProgetto(relativo: string): Promise<boolean> {
    try {
      if (!(await proteggiModifiche("sostituire"))) {
        return false;
      }
      const ris = await invoke<{ entrypoint: string; contenuto: string }>("apri_file_progetto", {
        relativo,
      });
      adotta(ris.entrypoint, ris.contenuto);
      return true;
    } catch (e) {
      onErrore(e);
      return false;
    }
  }

  /** Apre una cartella-progetto: rileva l'entrypoint e lo adotta come documento. */
  async function apriCartella(): Promise<void> {
    try {
      const cartella = await open({
        multiple: false,
        directory: true,
        title: "Apri una cartella-progetto",
      });
      if (cartella === null) {
        return; // annullato dall'utente
      }
      if (!(await proteggiModifiche("sostituire"))) {
        return;
      }
      const ris = await invoke<{ entrypoint: string; contenuto: string }>(
        "apri_cartella",
        { percorso: cartella },
      );
      // Adotta l'entrypoint come documento corrente (compilazione dal debounce).
      adotta(ris.entrypoint, ris.contenuto);
    } catch (e) {
      onErrore(e);
    }
  }

  /**
   * Scrive il testo corrente sul percorso indicato e aggiorna lo stato.
   * `automatico` = salvataggio automatico (la cronologia applica l'intervallo
   * minimo fra le versioni).
   *
   * Il backend **non sovrascrive** un file cambiato da altri (`conflitto`) né
   * ricrea uno sparito (`sparito`). Automatico: niente dialog, il documento
   * resta "modificato". Manuale: si chiede quale versione tenere (o si verifica
   * l'integrità del testo se il file non c'è più); scegliendo di tenere quella
   * dell'editor si salva di nuovo.
   */
  async function scriviSu(
    destinazione: string,
    automatico = false,
    senzaDomande = false,
  ): Promise<string> {
    // Il documento a cui la richiesta si riferisce: se cambia mentre si scrive
    // (apri/nuovo), l'esito non va applicato al documento nuovo.
    const prima = percorso;
    // Il testo si cattura UNA volta: ciò che si scrive e ciò che si confronta dopo
    // l'attesa sono lo stesso, così le modifiche arrivate durante la scrittura
    // non vengono date per salvate.
    const testo = editor.testo();
    let esito = "scritto";
    let risolviAttesa: () => void = () => {};
    attesaSalvataggio = new Promise<void>((ok) => {
      risolviAttesa = ok;
    });
    salvataggioInCorso = true;
    ascoltatoreScrittura(statoScrittura());
    try {
      esito = await invoke<string>("salva_file", {
        percorso: destinazione,
        contenuto: applicaFineRiga(testo, fineRiga),
        automatico,
      });
      erroreScrittura = false;
      if (percorso !== prima) {
        return "scartato";
      }
      if (esito === "scritto") {
        avvisoAutosaveDato = false;
        percorso = destinazione;
        testoDisco = testo;
        fileSparito = false;
        // Salvando con un'altra estensione il tipo può cambiare (es. .typ → .md).
        // Solo con «Salva con nome» verso un altro file: un Salva sul percorso
        // corrente (anche rinominato da fuori in .md/.typ) non cambia il tipo.
        if (!stessoPercorso(destinazione, prima)) {
          impostaTipo(tipoDaPercorso(destinazione));
        }
        modificato = editor.testo() !== testo;
        notifica();
      }
    } catch (e) {
      // Il salvataggio automatico non disturba: l'errore (file in uso, disco
      // pieno…) va solo in console, `modificato` resta com'è e si riprova.
      esito = "errore";
      erroreScrittura = true;
      if (automatico) {
        console.warn("Salvataggio automatico non riuscito:", e);
        // Un solo avviso finché l'errore persiste (il flag si azzera a salvataggio riuscito).
        if (!avvisoAutosaveDato) {
          avvisoAutosaveDato = true;
          onAvviso(
            "Salvataggio automatico non riuscito: riprova con Salva o modifica il documento.",
          );
        }
      } else {
        onErrore(e);
      }
      return "errore";
    } finally {
      salvataggioInCorso = false;
      ascoltatoreScrittura(statoScrittura());
      risolviAttesa();
      // Modifiche arrivate durante la scrittura (o scrittura non riuscita): il
      // timer va riarmato, altrimenti restano non salvate fino alla prossima
      // battuta. Non si riarma dopo conflitto/sparito (serve l'utente) né dopo un
      // errore: ritentare a ciclo ogni `autosave_ms` martellerebbe un disco che
      // fallisce; ci pensa la prossima modifica (`segnaModificato`).
      if (esito !== "conflitto" && esito !== "sparito" && esito !== "errore") {
        riarmaAutosave();
      }
    }
    // `senzaDomande` (salvataggio da «Salva / Non salvare»): l'esito va al chiamante,
    // che annulla l'operazione invece di aprire la domanda sul conflitto.
    if (esito === "scritto" || automatico || senzaDomande) {
      return esito;
    }
    if (esito === "sparito") {
      await documentoSparito();
    } else if ((await chiediVersione()) === "editor") {
      return scriviSu(destinazione);
    }
    return esito;
  }

  /** Chiede dove salvare (dialog di sistema) e salva lì; `suggerito` = percorso proposto. */
  async function salvaConNome(suggerito?: string, senzaDomande = false): Promise<string | null> {
    try {
      const md = tipoCorrente === "markdown";
      const destinazione = await save({
        defaultPath: suggerito ?? (md ? "documento.md" : "documento.typ"),
        filters: md
          ? [{ name: "Markdown", extensions: ["md"] }]
          : [{ name: "Typst", extensions: ["typ"] }],
      });
      if (destinazione === null) {
        return null; // annullato
      }
      // Scegliere nel dialog di sistema lo STESSO file del documento (anche se è
      // sparito o cambiato) è una scelta esplicita di sovrascriverlo: si
      // riconosce lo stato attuale del disco, così non si rientra nel ciclo
      // "sparito / non integro" o "conflitto".
      if (stessoPercorso(destinazione, percorso)) {
        await invoke("riconosci_disco", { percorso: destinazione });
        testoDisco = null;
      }
      return await scriviSu(destinazione, false, senzaDomande);
    } catch (e) {
      onErrore(e);
      return "errore";
    }
  }

  /** Salva sul percorso corrente, oppure chiede dove salvare se non c'è. */
  async function salva(): Promise<void> {
    // Un autosave in volo finisce prima: due scritture sovrapposte si pesterebbero
    // (e la seconda vedrebbe un falso conflitto).
    await attesaSalvataggio;
    if (percorso !== null) {
      await scriviSu(percorso);
      return;
    }
    await salvaConNome();
  }

  /** Cancella il timer del salvataggio automatico, se armato. */
  function annullaAutosave(): void {
    if (timerAutosave !== null) {
      clearTimeout(timerAutosave);
      timerAutosave = null;
    }
  }

  /** Riarma il timer del salvataggio automatico se serve (modificato, attivo, con percorso). */
  function riarmaAutosave(): void {
    if (leggiImpostazioni().autosave && modificato && percorso !== null && !caricamento) {
      annullaAutosave();
      timerAutosave = setTimeout(() => void autosalva(), leggiImpostazioni().autosave_ms);
    }
  }

  /** Salvataggio automatico (timer dopo la modifica, blur della finestra). */
  async function autosalva(): Promise<void> {
    annullaAutosave();
    if (
      !leggiImpostazioni().autosave ||
      percorso === null ||
      !modificato ||
      caricamento ||
      conflittoAperto ||
      domandaCambioAperta || // il blur causato dal dialog nativo non deve scrivere prima della risposta
      fileSparito
    ) {
      return;
    }
    if (salvataggioInCorso) {
      // Un salvataggio è già in volo: lo si attende (la chiusura della finestra
      // deve vederne l'esito); le modifiche arrivate dopo riarmano il timer.
      await attesaSalvataggio;
      return;
    }
    await scriviSu(percorso, true);
  }

  // ---- Cronologia versioni (F5-013) ----

  /**
   * Registra `testo` (forma dell'editor, solo LF; di default il testo corrente) come
   * versione del file corrente (no-op se il documento non ha percorso). Il
   * contenuto ha la fine riga del file riapplicata, come in `salva_file`: con LF
   * grezzo su un file CRLF la versione non risulterebbe «Identica» a quella
   * salvata e la verifica di integrità vedrebbe un troncamento.
   */
  async function registraInCronologia(testo: string = editor.testo()): Promise<void> {
    if (percorso === null) {
      return;
    }
    await invoke<boolean>("cronologia_registra", {
      percorso,
      contenuto: contenutoPerCronologia(testo, fineRiga),
    });
  }

  async function versioni(): Promise<VersioneCronologia[]> {
    if (percorso === null) {
      return [];
    }
    return invoke<VersioneCronologia[]>("cronologia_elenca", { percorso });
  }

  async function testoVersione(id: string): Promise<string> {
    if (percorso === null) {
      throw new Error("Il documento non è ancora stato salvato: nessuna cronologia.");
    }
    return invoke<string>("cronologia_leggi", { percorso, id });
  }

  async function ripristinaVersione(id: string): Promise<boolean> {
    try {
      const testo = await testoVersione(id);
      // La cronologia salva PRIMA di sostituire: se non ci riesce, il testo
      // corrente resta dov'è.
      await registraInCronologia(editor.testo());
      // Non sotto `caricamento`: è una modifica dell'utente (segna "modificato",
      // ricompila) e non tocca il disco finché non si salva.
      editor.sostituisci(normalizzaFineRiga(testo));
      return true;
    } catch (e) {
      onErrore(e);
      return false;
    }
  }

  // ---- File modificati da fuori (F5-022) ----

  /** Sostituisce il testo con `contenuto` letto da disco: non è una modifica dell'utente. */
  function sostituisciDaDisco(contenuto: string): void {
    // Fine riga rilevata sul testo del disco; l'editor ha solo LF: confrontare il
    // testo CRLF grezzo farebbe sostituire quasi tutto (cursore e scorrimento persi).
    fineRiga = rilevaFineRiga(contenuto);
    caricamento = true;
    try {
      editor.sostituisci(normalizzaFineRiga(contenuto));
    } finally {
      caricamento = false;
    }
    testoDisco = normalizzaFineRiga(contenuto);
    modificato = false;
    fileSparito = false;
    notifica();
  }

  /**
   * Ricarica il file corrente dal disco conservando cursore e scorrimento.
   * Il testo dell'editor va prima in cronologia. Se nel frattempo l'utente ha
   * scritto (e `forzata` è falso) non si sostituisce nulla: si chiede.
   */
  async function ricaricaDaDisco(forzata: boolean, incerto: boolean = false): Promise<void> {
    const daCaricare = percorso;
    if (daCaricare === null) {
      return;
    }
    const prima = editor.testo();
    try {
      await registraInCronologia(prima);
      // Se durante la registrazione l'utente ha scritto, si chiede PRIMA di
      // leggere: `leggi_file` segna il contenuto letto come "visto dall'editor" e
      // la versione esterna non deve risultare vista senza una risposta.
      if (!forzata && editor.testo() !== prima) {
        await chiediVersione();
        return;
      }
      const contenuto = await invoke<string>("leggi_file", { percorso: daCaricare });
      if (percorso !== daCaricare) {
        return; // il documento è cambiato nel frattempo
      }
      if (!forzata && editor.testo() !== prima) {
        // Ha scritto proprio durante la lettura: non si sostituisce nulla e si
        // chiede (la scelta dell'utente decide; il salvataggio non parte da solo).
        await chiediVersione();
        return;
      }
      if (ricaricaSuperflua(contenuto, prima, incerto)) {
        // Percorso incerto e contenuto identico: falso allarme (omonimo altrove), si tace.
        return;
      }
      sostituisciDaDisco(contenuto);
      onAvviso(`«${nomeFile(daCaricare)}» ricaricato: era stato modificato da un altro programma.`);
    } catch (e) {
      onErrore(e);
    }
  }

  /**
   * Il file è cambiato su disco ma l'editor ha modifiche non salvate: chiede
   * quale versione tenere. Nulla viene sovrascritto prima della risposta.
   * Il tasto di chiusura/Esc del dialog vale "tieni la mia" (la scelta sicura).
   */
  async function chiediVersione(): Promise<"disco" | "editor" | null> {
    if (conflittoAperto || percorso === null) {
      return null;
    }
    const file = percorso;
    let scelta: "disco" | "editor" | null = null;
    conflittoAperto = true;
    annullaAutosave();
    try {
      const usaDisco = await ask(
        `Il file «${nomeFile(percorso)}» è stato modificato da un altro programma, ma nell'editor ci sono ` +
          "modifiche non salvate.\n\nQuale versione vuoi tenere?\n" +
          "(La versione dell'editor viene comunque registrata nella cronologia.)",
        {
          title: "File modificato da fuori",
          kind: "warning",
          okLabel: "Carica quella su disco",
          cancelLabel: "Tieni quella dell'editor",
        },
      );
      if (usaDisco) {
        await ricaricaDaDisco(true);
        scelta = "disco";
      } else {
        // Lo stato attuale del disco diventa quello "visto": il prossimo
        // salvataggio lo sovrascrive senza chiedere di nuovo.
        await invoke("riconosci_disco", { percorso: file });
        testoDisco = null;
        scelta = "editor";
        fileSparito = false;
        onAvviso("Tenuta la versione dell'editor: salvandola sostituirai quella su disco.");
      }
    } catch (e) {
      onErrore(e);
    } finally {
      conflittoAperto = false;
    }
    return scelta;
  }

  /**
   * Evento `incerto` con testo modificato: legge il disco e chiede quale versione tenere
   * solo se è diverso dall'ultima versione nota (`testoDisco`); uguale = omonimo altrove,
   * si tace. Il flag `conflittoAperto` copre anche la lettura (nessun autosave concorrente
   * scrive) ed è sempre ripristinato. La versione esterna va in cronologia PRIMA della
   * domanda: la lettura l'ha già segnata «vista» e non deve andare persa.
   */
  async function chiediSeDiscoCambiato(): Promise<void> {
    const file = percorso;
    if (file === null || conflittoAperto) {
      return;
    }
    let daChiedere = false;
    conflittoAperto = true;
    annullaAutosave();
    try {
      const c = await invoke<string>("leggi_file", { percorso: file });
      if (percorso !== file) {
        return; // il documento è cambiato nel frattempo
      }
      if (discoInvariato(c, testoDisco)) {
        return;
      }
      await invoke<boolean>("cronologia_registra", {
        percorso: file,
        contenuto: contenutoPerCronologia(normalizzaFineRiga(c), rilevaFineRiga(c)),
      });
      daChiedere = true;
    } catch (e) {
      // Non si riesce a confrontare: si ripiega sul comportamento prudente (chiedere).
      console.warn("Confronto con il disco non riuscito:", e);
      daChiedere = percorso === file;
    } finally {
      conflittoAperto = false;
    }
    if (daChiedere && percorso === file) {
      await chiediVersione();
    }
  }

  /**
   * Il file aperto è sparito dal disco (cancellato o spostato). Il backend
   * ripristina solo se il testo dell'editor è integro (voce in cronologia +
   * scrittura al percorso originale); altrimenti il documento resta "non
   * salvato" e si chiede dove salvarlo.
   */
  async function documentoSparito(incerto: boolean = false): Promise<void> {
    const file = percorso;
    if (file === null) {
      return;
    }
    const testo = editor.testo();
    try {
      const r = await invoke<{ esito: string }>("ripristina_documento_sparito", {
        percorso: file,
        contenuto: applicaFineRiga(testo, fineRiga),
      });
      if (percorso !== file) {
        return;
      }
      const azione = azioneDopoSparito(r.esito, incerto);
      if (azione === "ignora") {
        // Percorso incerto e il documento esiste: l'evento riguardava un altro file.
        return;
      }
      if (azione === "ripristinato") {
        testoDisco = testo;
        fileSparito = false;
        if (editor.testo() === testo) {
          modificato = false;
          notifica();
        }
        onAvviso("File eliminato dall'esterno: ripristinato da Typster.");
      } else if (azione === "non-ripristinato") {
        // Non integro (o altro): nessun salvataggio, il testo resta "non salvato".
        fileSparito = true;
        modificato = true;
        notifica();
        const salvaAltrove = await ask(
          `Il file «${nomeFile(file)}» è stato eliminato o spostato da un altro programma e non è stato ` +
            `ripristinato: ${motivoNonRipristinato(r.esito)}.\n\n` +
            "Il testo resta nell'editor come non salvato. Vuoi scegliere dove salvarlo?",
          {
            title: "File non più su disco",
            kind: "warning",
            okLabel: "Salva con nome…",
            cancelLabel: "Più tardi",
          },
        );
        if (salvaAltrove) {
          await salvaConNome(file);
        }
      }
      else {
        // "esiste" (percorso certo): il file è tornato (o non è leggibile): non si tocca. Il testo
        // resta com'è e il prossimo salvataggio confronta il disco e, se serve, chiede.
        fileSparito = false;
        modificato = true;
        notifica();
        onAvviso("Il file è ricomparso sul disco: salva di nuovo per decidere quale versione tenere.");
      }
    } catch (e) {
      onErrore(e);
    }
  }

  /**
   * Riaggancia il World al nuovo percorso dopo una rinomina esterna, così
   * `#image`/`#include` relativi si risolvono dalla cartella giusta. Il testo
   * ritornato da `apri_file` si scarta: l'editor tiene il proprio (modifiche
   * non salvate e cursore intatti). Un errore non è fatale: avviso e basta.
   */
  async function riagganciaWorld(nuovo: string): Promise<void> {
    try {
      await invoke<string>("apri_file", { percorso: nuovo });
      if (percorso === nuovo) {
        onRicompila();
      }
    } catch (e) {
      onAvviso(`Percorso aggiornato, ma i file collegati potrebbero non risolversi: ${String(e)}`);
    }
  }

  /** Applica un singolo evento `modifica-esterna` (uno alla volta, vedi `codaEventi`). */
  async function elaboraEvento(ev: EventoEsterno): Promise<void> {
    switch (ev.azione) {
      case "ricarica":
      case "chiedi":
        // `incerto` (stesso nome, forma diversa: unità mappate, subst, giunzioni) NON si
        // scarta: ricaricaDaDisco/chiediVersione lavorano sul percorso del documento e
        // proteggono comunque le modifiche non salvate.
        {
          const esito = confrontaPercorsi(ev.percorso, percorso);
          if (esito === "diverso") {
            return;
          }
          // Lo stato dell'editor decide, non l'etichetta arrivata dal guscio: fra
          // l'evento e qui l'utente potrebbe aver scritto. Con `incerto` e testo non
          // modificato si legge e si confronta (identico: nessun avviso); con testo
          // modificato si confronta il disco con l'ultima versione nota (`testoDisco`).
          if (modificato) {
            if (esito === "incerto" && testoDisco !== null) {
              await chiediSeDiscoCambiato();
            } else {
              await chiediVersione();
            }
          } else {
            await ricaricaDaDisco(false, esito === "incerto");
          }
        }
        return;
      case "sparito":
        // `documentoSparito` verifica sul disco (esito «esiste» se il file c'è): con un
        // percorso `incerto` il controllo è innocuo, scartare l'evento no.
        {
          const esito = confrontaPercorsi(ev.percorso, percorso);
          if (esito !== "diverso") {
            await documentoSparito(esito === "incerto");
          }
        }
        return;
      case "rinominato":
        if (ev.nuovo !== null && confrontaPercorsi(ev.percorso, percorso) === "incerto") {
          // Non si riaggancia il documento a un percorso che potrebbe riguardare un altro
          // file, ma nemmeno si ignora l'evento in silenzio.
          onAvviso(
            `Un file chiamato «${nomeFile(ev.percorso ?? "")}» è stato rinominato in «${nomeFile(ev.nuovo)}»: ` +
              "il suo percorso è scritto in forma diversa da quello del documento, quindi non è stato aggiornato. " +
              "Se è il documento aperto, usa «Salva con nome».",
          );
          return;
        }
        if (ev.nuovo !== null && confrontaPercorsi(ev.percorso, percorso) === "uguale") {
          const nuovo = ev.nuovo;
          percorso = nuovo;
          // Il tipo NON si deduce dalla nuova estensione: se differisce lo si
          // mantiene e si avvisa (vedi `avvisoTipoDopoRinomina`).
          const avvisoTipo = avvisoTipoDopoRinomina(tipoCorrente, nuovo, nomeFile(nuovo));
          notifica();
          await riagganciaWorld(nuovo);
          onAvviso(
            `File rinominato dall'esterno in «${nomeFile(nuovo)}».` +
              (avvisoTipo === null ? "" : ` ${avvisoTipo}`),
          );
        }
        return;
      default:
        return; // `invalida` e `albero` riguardano anteprima e albero: main.ts
    }
  }

  function gestisciModificaEsterna(ev: EventoEsterno): Promise<void> {
    codaEventi = codaEventi.then(() => elaboraEvento(ev)).catch((e: unknown) => onErrore(e));
    return codaEventi;
  }

  /** Esporta in PDF/PNG/SVG su un percorso scelto dall'utente. */
  async function esporta(
    formato: FormatoEsportazione,
    opzioni: OpzioniEsportazione,
    sorgenteTypst?: string,
  ): Promise<number> {
    try {
      const destinazione = await save({
        defaultPath: nomePredefinito(percorso, formato),
        filters: filtriSalvataggio(formato),
      });
      if (destinazione === null) {
        return 0; // annullato
      }
      // L'export compila sempre Typst: in modalità Markdown il chiamante passa
      // il Typst già convertito in `sorgenteTypst`; in modalità Typst si usa il
      // testo corrente dell'editor.
      const sorgente = sorgenteTypst ?? editor.testo();
      if (eConversione(formato)) {
        const esito = await invoke<EsitoConversione>(comandoConversione(formato), {
          percorso: destinazione,
          sorgente,
          ...argomentiConversione(formato, opzioni),
        });
        onConversione(formato, esito);
        return 1;
      }
      if (formato === "pdf") {
        await invoke("esporta_pdf", {
          percorso: destinazione,
          sorgente,
          opzioni: opzioniPdfPerBackend(opzioni),
        });
        // Ricorda lo standard solo dopo un export riuscito; un errore qui non annulla il PDF scritto.
        if (opzioni.standard !== leggiImpostazioni().standard_pdf) {
          salvaImpostazioni({ standard_pdf: opzioni.standard }).catch((e: unknown) =>
            console.warn("Standard PDF non salvato:", e),
          );
        }
        return 1;
      }
      const args = {
        formato,
        percorso: destinazione,
        sorgente,
        pagine: pagineOppureNull(opzioni.pagine ?? ""),
        ppi: formato === "png" ? opzioni.ppi : null,
      };
      let esito = await invoke<EsitoImmagini>("esporta_immagini", { ...args, sovrascrivi: false });
      if (esito.esito === "Conflitto") {
        // I file `nome-NN.ext` non hanno la conferma del dialog: una sola domanda.
        const ok = await ask(testoConflitto(esito.esistenti), {
          title: "Sovrascrivere i file?",
          kind: "warning",
        });
        if (!ok) {
          return 0;
        }
        esito = await invoke<EsitoImmagini>("esporta_immagini", { ...args, sovrascrivi: true });
      }
      return esito.esito === "Scritti" ? esito.file : 0;
    } catch (e) {
      // Il backend può rifiutare la destinazione (es. il documento aperto, o un
      // file `.typ`/`.md`): messaggio chiaro con il motivo, mai un blocco. Va nel
      // riquadro errori e in un messaggio esplicito (il riquadro si svuota alla
      // prossima compilazione).
      const testo = testoErroreEsportazione(formato, e);
      onErrore(testo);
      try {
        await message(testo, { title: "Esportazione non riuscita", kind: "error" });
      } catch (m) {
        console.warn("Messaggio di errore non mostrato:", m);
      }
      return 0;
    }
  }

  /** Marca il documento come modificato (notifica solo sul fronte di salita). */
  function segnaModificato(): void {
    if (caricamento) {
      return; // modifica programmatica da caricamento file, non dell'utente
    }
    if (!modificato) {
      modificato = true;
      notifica();
    }
    // Salvataggio automatico: si riarma a ogni modifica (scatta a riposo).
    if (leggiImpostazioni().autosave && percorso !== null) {
      annullaAutosave();
      timerAutosave = setTimeout(() => void autosalva(), leggiImpostazioni().autosave_ms);
    }
  }

  /** Adotta un progetto già caricato; le modifiche del documento sono già state protette dal chiamante. */
  async function apriProgetto(nuovoPercorso: string, contenuto: string): Promise<void> {
    try {
      adotta(nuovoPercorso, contenuto);
    } catch (e) {
      onErrore(e);
    }
  }

  /** Adotta un progetto già caricato (contenuto + percorso noti, es. da template). */
  function adotta(nuovoPercorso: string, contenuto: string): void {
    // I progetti/template sono Typst nativi: imposta il tipo dall'entrypoint.
    impostaTipo(tipoDaPercorso(nuovoPercorso));
    // `caricamento` sopprime il segnale "modificato" emesso da `editor.imposta`.
    fineRiga = rilevaFineRiga(contenuto);
    caricamento = true;
    impostaTestoConRicompilazione(editor, normalizzaFineRiga(contenuto), onRicompila);
    caricamento = false;
    percorso = nuovoPercorso;
    testoDisco = normalizzaFineRiga(contenuto);
    modificato = false;
    fileSparito = false;
    notifica();
  }

  /** Registra una conversione di formato: nuovo documento non salvato del tipo dato. */
  function convertitoA(tipo: TipoDocumento): void {
    impostaTipo(tipo);
    fineRiga = "\n"; // testo convertito: nuovo documento, fine riga predefinita
    // Il formato è cambiato: il file su disco non corrisponde più. Si scollega e
    // si marca come modificato, così il prossimo Salva chiede un nome nuovo con
    // l'estensione corretta (evita di salvare Markdown in un `.typ` e viceversa).
    percorso = null;
    testoDisco = null;
    modificato = true;
    fileSparito = false;
    notifica();
  }

  return {
    nuovo,
    nuovoDa,
    apri,
    apriCartella,
    apriFileProgetto,
    salva,
    autosalva,
    gestisciModificaEsterna,
    versioni,
    testoVersione,
    ripristinaVersione,
    osservazioneAttiva: () => osservazione,
    esporta,
    segnaModificato,
    registraInCronologia: () => registraInCronologia(),
    apriProgetto,
    proteggiModifiche,
    convertitoA,
    stato: () => ({ percorso, modificato }),
    suSalvataggio: (a) => {
      ascoltatoreScrittura = a;
    },
    statoScrittura,
  };
}

/**
 * Estrae il nome file da un percorso assoluto (gestisce separatori `/` e `\`).
 *
 * @param percorso percorso completo del file.
 * @returns l'ultimo segmento (nome file con estensione).
 */
export function nomeFile(percorso: string): string {
  const segmenti = percorso.split(/[/\\]/).filter(Boolean);
  return segmenti.length > 0 ? segmenti[segmenti.length - 1] : percorso;
}
