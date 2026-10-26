/**
 * ciclocompilazione.ts — Macchina a stati del ciclo di compilazione.
 *
 * Logica pura (niente DOM né Tauri), verificata da `ciclocompilazione.check.ts`.
 *
 * Un risultato di compilazione è obsoleto SOLO se il documento è stato
 * *sostituito* (Nuovo/apri/ricarica/ripristina/converti) durante il volo: le
 * semplici battute dell'utente non lo invalidano. Invalidarlo a ogni tasto
 * affamava l'anteprima: con una compilazione da ~2 s e un tasto al secondo ogni
 * risultato veniva scartato e lo stato restava «Compilazione…». I diagnostici
 * inline di un risultato «vecchio di qualche battuta» sono già protetti dal clamp
 * degli offset in `editor.ts` (`mostraDiagnostici`).
 */

/** Esito della terminazione di una compilazione. */
export interface EsitoCompilazione {
  /** Il risultato va dipinto sull'UI (pagine, diagnostici, stato). */
  applica: boolean;
  /** Va avviata subito un'altra compilazione (leggendo il testo corrente). */
  ricompila: boolean;
}

export interface CicloCompilazione {
  /** Il documento è stato sostituito: i risultati in volo sono obsoleti. */
  sostituzione(): void;
  /** Richiesta di compilazione: `avvia` se libera, `accoda` se una è in volo. */
  richiedi(): "avvia" | "accoda";
  /** Segna l'inizio di una compilazione; restituisce il token da riconsegnare. */
  inizia(): number;
  /**
   * Segna la fine. `timerInAttesa`: c'è già un debounce armato che ricompilerà
   * da solo (quindi, per un risultato scartato, non serve ricompilare subito).
   */
  termina(token: number, timerInAttesa: boolean): EsitoCompilazione;
  inCorso(): boolean;
}

export function creaCicloCompilazione(): CicloCompilazione {
  let sostituzioni = 0;
  let inCorso = false;
  let pendente = false;
  return {
    sostituzione: () => {
      sostituzioni++;
    },
    richiedi: () => {
      if (inCorso) {
        pendente = true;
        return "accoda";
      }
      return "avvia";
    },
    inizia: () => {
      inCorso = true;
      pendente = false;
      return sostituzioni;
    },
    termina: (token, timerInAttesa) => {
      inCorso = false;
      const applica = token === sostituzioni;
      const ricompila = pendente || (!applica && !timerInAttesa);
      pendente = false;
      return { applica, ricompila };
    },
    inCorso: () => inCorso,
  };
}

/** Effetti di una battuta dell'utente (iniettati: la funzione resta pura). */
export interface AzioniBattuta {
  /** Marca il documento come modificato. */
  segnaModificato(): void;
  /** Arma (o riarma) il debounce della compilazione. */
  programmaCompilazione(): void;
}

/**
 * Reazione a una battuta (modifica dell'utente): segna il file come modificato e
 * programma la compilazione. NON tocca `ciclo`: invalidare il risultato in volo
 * a ogni tasto affamerebbe l'anteprima (vedi l'intestazione del modulo). Solo
 * `reagisciASostituzione` invalida.
 */
export function reagisciABattuta(_ciclo: CicloCompilazione, azioni: AzioniBattuta): void {
  azioni.segnaModificato();
  azioni.programmaCompilazione();
}

/**
 * Reazione a una sostituzione del documento (`imposta`/`sostituisci`): se il
 * testo cambia, i risultati in volo sono obsoleti.
 */
export function reagisciASostituzione(ciclo: CicloCompilazione, prima: string, dopo: string): void {
  if (prima !== dopo) {
    ciclo.sostituzione();
  }
}

/**
 * Imposta il testo dell'editor (apertura, nuovo, adozione di un progetto) e
 * garantisce che il World venga ricompilato anche se il testo è identico:
 * CodeMirror emette `docChanged` (quindi `onModifica` e il debounce) solo se il
 * testo cambia, ma un'apertura con testo uguale può avere una nuova radice o un
 * World diverso. Se il testo cambia basta il debounce già armato: richiamare
 * `ricompila` produrrebbe una seconda compilazione inutile.
 */
export function impostaTestoConRicompilazione(
  editor: { testo(): string; imposta(testo: string): void },
  testo: string,
  ricompila: () => void,
): void {
  const identico = editor.testo() === testo;
  editor.imposta(testo);
  if (identico) {
    ricompila();
  }
}

/**
 * Il World è cambiato senza che il testo cambi (apertura/adozione con testo
 * identico, nuova radice, rinomina esterna): la compilazione in volo riguarda il
 * documento precedente, quindi il suo risultato è obsoleto (`sostituzione`) e se
 * ne avvia una nuova (`avvia` la accoda se una è ancora in volo). Se il testo
 * cambiasse, `reagisciASostituzione` ha già invalidato: la doppia chiamata è innocua.
 */
export function ricompilaPerWorldCambiato(ciclo: CicloCompilazione, avvia: () => void): void {
  ciclo.sostituzione();
  avvia();
}
