/**
 * aspetto.ts — Tema, stile grafico e font dell'editor (F5-004/F5-005).
 *
 * Applica le impostazioni utente (`tema`, `dimensione_font`) alla UI:
 * - il tema risolto ("chiaro"/"scuro") va nell'attributo `data-tema` di `<html>`
 *   (le variabili CSS in index.html) e nel compartment del tema dell'editor;
 * - lo stile grafico ("classico" | "composizione" | "strumento") va in `data-stile`
 *   di `<html>` (regole CSS in src/stili/*.css) e nell'editor (colori di sintassi);
 *   la scelta fatta prima del caricamento delle impostazioni non viene sovrascritta;
 * - la dimensione va nella variabile CSS `--font-editor`.
 * Le funzioni esportate sono registrate come comandi in `main.ts` (registro
 * `comandi.ts`, F5-006), che gestisce anche le scorciatoie Ctrl/Cmd `+`/`-`/`0`.
 * Il modulo non conosce CodeMirror: parla solo con l'interfaccia `Editor`.
 */
import {
  helpCompletamentoAttivo,
  impostaHelpCompletamento,
  impostaSuggerimentoParametri,
  suggerimentoParametriAttivo,
} from "./assistenza";
import type { Editor, TemaEditor } from "./editor";
import {
  CHIAVE_SPECCHIO_STILE,
  devePersistereStile,
  normalizzaStile,
  STILE_PREDEFINITO,
  stileDaSpecchio,
  type SceltaStile,
} from "./stile";
import {
  devePersistereMotore,
  MOTORE_PREDEFINITO,
  normalizzaMotore,
  type MotoreTypst,
} from "./motore";
import {
  caricaImpostazioni,
  leggiImpostazioni,
  PREDEFINITE,
  salvaImpostazioni,
  type Impostazioni,
} from "./impostazioni";

/** Scelta dell'utente: come `Impostazioni.tema`. */
export type SceltaTema = Impostazioni["tema"];

/** Limiti e predefinito del font, allineati al core (8..=72, default 14). */
export const FONT_MIN = 8;
export const FONT_MAX = 72;
export const FONT_PREDEFINITO = PREDEFINITE.dimensione_font;

const ORDINE: SceltaTema[] = ["sistema", "chiaro", "scuro"];

let editorAttivo: Editor | null = null;
let sceltaTema: SceltaTema = "sistema";
let dimensioneFont = FONT_PREDEFINITO;
let notifica: (msg: string) => void = () => {};
let timerFont: ReturnType<typeof setTimeout> | null = null;
// Scelte fatte dall'utente prima che il caricamento delle impostazioni finisca:
// il valore salvato, arrivando dopo, non deve sovrascriverle.
let temaToccato = false;
let fontToccato = false;
let sceltaStile: SceltaStile = STILE_PREDEFINITO;
let stileToccato = false;
// Vero quando il file delle impostazioni e' stato letto (prima di allora ogni scelta va scritta).
let impostazioniCaricate = false;
// Stile che risulta scritto su disco (letto all'avvio o salvato con successo): serve a
// non saltare un nuovo tentativo dopo un salvataggio fallito.
let stileSalvato: SceltaStile = STILE_PREDEFINITO;
// Motore di evidenziazione Typst: stessa gestione dello stile (scelta prima del caricamento
// non sovrascritta; `motoreSalvato` serve a ritentare dopo un salvataggio fallito).
let sceltaMotore: MotoreTypst = MOTORE_PREDEFINITO;
let motoreToccato = false;
let motoreSalvato: MotoreTypst = MOTORE_PREDEFINITO;
// Osservatori (il menu «Aspetto» si riallinea anche quando la scelta arriva dalla palette).
const osservatori = new Set<() => void>();

const querySchemaScuro = window.matchMedia("(prefers-color-scheme: dark)");

/** Risolve la scelta in un tema concreto (per "sistema" guarda `prefers-color-scheme`). */
function risolvi(scelta: SceltaTema): TemaEditor {
  if (scelta === "sistema") {
    return querySchemaScuro.matches ? "scuro" : "chiaro";
  }
  return scelta;
}

/** Applica la scelta corrente a DOM, editor e pulsante (senza persistere). */
function applicaTema(): void {
  const risolto = risolvi(sceltaTema);
  document.documentElement.dataset.tema = risolto;
  editorAttivo?.impostaTema(risolto);
}

/**
 * Specchio dello stile in `localStorage`, letto in testa a `main.ts` per dipingere subito
 * lo stile giusto (il file delle impostazioni arriva dopo il primo paint; niente script
 * inline: CSP). Mai fidarsi del valore: passa da `stileDaSpecchio`. Errori di accesso ignorati.
 */
function leggiSpecchioStile(): SceltaStile | null {
  try {
    return stileDaSpecchio(window.localStorage.getItem(CHIAVE_SPECCHIO_STILE));
  } catch {
    return null;
  }
}

function scriviSpecchioStile(): void {
  try {
    window.localStorage.setItem(CHIAVE_SPECCHIO_STILE, sceltaStile);
  } catch {
    // localStorage non disponibile: si perde solo la mitigazione del flash.
  }
}

/**
 * Applica subito lo stile dello specchio (se valido) a `data-stile`, PRIMA di
 * `inizializzaAspetto`. Non conta come scelta dell'utente: il file, una volta
 * caricato, vince.
 */
export function applicaStileSpecchiato(): void {
  const s = leggiSpecchioStile();
  if (s !== null) {
    sceltaStile = s;
    document.documentElement.dataset.stile = s;
  }
}

/** Applica lo stile corrente: attributo `data-stile` su <html> (le regole CSS sono scopate lì), editor e specchio. */
function applicaStile(): void {
  document.documentElement.dataset.stile = sceltaStile;
  editorAttivo?.impostaStile(sceltaStile);
  scriviSpecchioStile();
}

/** Avvisa gli osservatori che tema o stile sono cambiati. */
function notificaCambio(): void {
  for (const cb of osservatori) {
    cb();
  }
}

/** Limita e arrotonda una dimensione al range ammesso. */
function limita(px: number): number {
  return Math.min(FONT_MAX, Math.max(FONT_MIN, Math.round(px)));
}

/** Applica la dimensione corrente (variabile CSS) e fa rimisurare l'editor. */
function applicaFont(): void {
  document.documentElement.style.setProperty("--font-editor", `${dimensioneFont}px`);
  editorAttivo?.rimisura();
}

/** Persiste in background; l'errore è mostrato tramite la callback di notifica. */
function persisti(modifica: Partial<Impostazioni>): Promise<boolean> {
  return salvaImpostazioni(modifica).then(
    () => true,
    (e: unknown) => {
      console.warn("Impostazioni non salvate:", e);
      notifica(`Impostazioni non salvate: ${String(e)}`);
      return false;
    },
  );
}

/**
 * Imposta il tema ("sistema" | "chiaro" | "scuro"): applica a caldo e persiste.
 * Riusabile dalla palette comandi.
 */
export function impostaTema(scelta: SceltaTema): void {
  temaToccato = true;
  sceltaTema = scelta;
  applicaTema();
  persisti({ tema: scelta });
  notificaCambio();
}

/** Tema scelto dall'utente ("sistema" incluso, non risolto). */
export function temaCorrente(): SceltaTema {
  return sceltaTema;
}

/** Stile grafico corrente. */
export function stileCorrente(): SceltaStile {
  return sceltaStile;
}

/**
 * Registra un osservatore chiamato quando cambiano tema o stile; restituisce la
 * funzione che lo rimuove.
 */
export function osservaAspetto(cb: () => void): () => void {
  osservatori.add(cb);
  return () => {
    osservatori.delete(cb);
  };
}

/**
 * Imposta lo stile grafico ("classico" | "composizione" | "strumento"): applica a
 * caldo (nessun ricaricamento) e persiste come il tema. Un valore sconosciuto
 * ricade sul classico. Riusabile dalla palette comandi e dal menu «Aspetto».
 */
export function impostaStile(scelta: SceltaStile): void {
  const scrivi = devePersistereStile(stileSalvato, scelta, impostazioniCaricate);
  stileToccato = true;
  sceltaStile = normalizzaStile(scelta);
  applicaStile();
  if (scrivi) {
    const valore = sceltaStile;
    void persisti({ stile: valore }).then((ok) => {
      if (ok) {
        stileSalvato = valore;
      }
    });
  }
  notificaCambio();
}

/** Motore di evidenziazione Typst corrente. */
export function motoreCorrente(): MotoreTypst {
  return sceltaMotore;
}

/**
 * Imposta il motore di evidenziazione Typst ("lezer" | "wasm"): applica a caldo
 * (nessun ricaricamento) e persiste come lo stile. Un valore sconosciuto ricade su lezer.
 * Riusabile dalla palette comandi e dal menu «Opzioni».
 */
export function impostaMotore(scelta: MotoreTypst): void {
  const scrivi = devePersistereMotore(motoreSalvato, scelta, impostazioniCaricate);
  motoreToccato = true;
  sceltaMotore = normalizzaMotore(scelta);
  editorAttivo?.impostaMotoreTypst(sceltaMotore);
  if (scrivi) {
    const valore = sceltaMotore;
    void persisti({ motore_typst: valore }).then((ok) => {
      if (ok) {
        motoreSalvato = valore;
      }
    });
  }
  notificaCambio();
}

// Come tema/stile: una scelta fatta prima del caricamento non va sovrascritta.
let helpToccato = false;
let suggerimentoToccato = false;

export { helpCompletamentoAttivo, suggerimentoParametriAttivo };

/** Attiva/disattiva l'help del completamento (pannello documentazione, tipi dei parametri) e persiste. */
export function impostaHelp(attivo: boolean): void {
  helpToccato = true;
  impostaHelpCompletamento(attivo);
  void persisti({ help_completamento: attivo });
  notificaCambio();
}

/** Attiva/disattiva il suggerimento dei parametri nelle chiamate e persiste. */
export function impostaSuggerimento(attivo: boolean): void {
  suggerimentoToccato = true;
  impostaSuggerimentoParametri(attivo);
  void persisti({ suggerimento_parametri: attivo });
  notificaCambio();
}

/** Passa al tema successivo nel ciclo sistema, chiaro, scuro. */
export function alternaTema(): void {
  impostaTema(ORDINE[(ORDINE.indexOf(sceltaTema) + 1) % ORDINE.length]);
}

/** Imposta la dimensione del font (px, arrotondata e limitata a 8..72) e persiste. */
export function impostaDimensioneFont(px: number): void {
  if (!Number.isFinite(px)) {
    return;
  }
  fontToccato = true;
  dimensioneFont = limita(px);
  applicaFont();
  // Un tasto tenuto premuto genera molte variazioni: si scrive su disco solo a raffica finita.
  if (timerFont !== null) {
    clearTimeout(timerFont);
  }
  timerFont = setTimeout(scriviFontPendente, 300);
}

/** Scrive subito il font se c'è un salvataggio in attesa (anche alla chiusura/nascondimento). */
function scriviFontPendente(): void {
  if (timerFont === null) {
    return;
  }
  clearTimeout(timerFont);
  timerFont = null;
  persisti({ dimensione_font: dimensioneFont });
}

/** Aumenta il font di 1px. */
export function aumentaFont(): void {
  impostaDimensioneFont(dimensioneFont + 1);
}

/** Riduce il font di 1px. */
export function riduciFont(): void {
  impostaDimensioneFont(dimensioneFont - 1);
}

/** Riporta il font al valore predefinito (14px). */
export function ripristinaFont(): void {
  impostaDimensioneFont(FONT_PREDEFINITO);
}

/**
 * Collega l'aspetto all'editor: applica subito il tema di sistema (niente flash
 * in attesa del file), poi quello salvato; segue `prefers-color-scheme` a caldo
 * quando la scelta è "sistema". Il cambio di tema/stile da interfaccia sta nel menu
 * «Aspetto» (menu-aspetto.ts) e nei comandi della palette.
 *
 * @param editor  editor da configurare.
 * @param avvisa  callback per messaggi (es. errore di salvataggio).
 */
export function inizializzaAspetto(
  editor: Editor,
  avvisa: (msg: string) => void,
): void {
  editorAttivo = editor;
  notifica = avvisa;
  applicaStile();
  applicaTema();
  applicaFont();
  querySchemaScuro.addEventListener("change", () => {
    if (sceltaTema === "sistema") {
      applicaTema();
      notificaCambio();
    }
  });
  // Il salvataggio del font in attesa non deve perdersi chiudendo l'app.
  window.addEventListener("pagehide", scriviFontPendente);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      scriviFontPendente();
    }
  });
  void caricaImpostazioni().then(() => {
    impostazioniCaricate = true;
    const imp = leggiImpostazioni();
    stileSalvato = normalizzaStile(imp.stile);
    motoreSalvato = normalizzaMotore(imp.motore_typst);
    if (!helpToccato) {
      impostaHelpCompletamento(imp.help_completamento !== false);
    }
    if (!suggerimentoToccato) {
      impostaSuggerimentoParametri(imp.suggerimento_parametri !== false);
    }
    if (!motoreToccato) {
      sceltaMotore = motoreSalvato;
      editorAttivo?.impostaMotoreTypst(sceltaMotore);
    }
    if (!temaToccato) {
      sceltaTema = imp.tema;
      applicaTema();
    }
    if (!stileToccato) {
      sceltaStile = normalizzaStile(imp.stile);
      applicaStile();
    }
    notificaCambio();
    if (!fontToccato) {
      dimensioneFont = limita(imp.dimensione_font);
      applicaFont();
    }
  });
}
