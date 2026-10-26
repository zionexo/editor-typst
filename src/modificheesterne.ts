/**
 * modificheesterne.ts — Logica pura del frontend per i file modificati da fuori
 * (F5-022) e per la sostituzione del testo dell'editor (F5-013).
 *
 * Senza DOM né Tauri (verificata da `modificheesterne.check.ts`). Le decisioni
 * vere (ricarica/chiedi/sparito/segui) le prende il core in Rust; qui restano
 * i pezzi che servono solo lato editor.
 */

import { rilevaWindows } from "./comandi.ts";
import { normalizzaFineRiga } from "./finerighe.ts";

/** Evento `modifica-esterna` emesso dal guscio (vedi `osservatore.rs`). */
export interface EventoEsterno {
  /** `ricarica`, `chiedi`, `sparito`, `rinominato`, `invalida` o `albero`. */
  azione: "ricarica" | "chiedi" | "sparito" | "rinominato" | "invalida" | "albero";
  /** Percorso interessato (per `rinominato` il vecchio). */
  percorso: string | null;
  /** Nuovo percorso (solo `rinominato`). */
  nuovo: string | null;
}

/** Sostituzione minima che trasforma un testo in un altro. */
export interface Sostituzione {
  /** Offset UTF-16 iniziale del tratto da sostituire. */
  da: number;
  /** Offset UTF-16 finale (esclusivo) del tratto da sostituire, nel testo vecchio. */
  a: number;
  /** Testo da inserire al posto del tratto. */
  inserisci: string;
}

/** `true` se `codice` è una unità UTF-16 "alta" di una coppia surrogata. */
function eSurrogataAlta(codice: number): boolean {
  return codice >= 0xd800 && codice <= 0xdbff;
}

/** `true` se `codice` è una unità UTF-16 "bassa" di una coppia surrogata. */
function eSurrogataBassa(codice: number): boolean {
  return codice >= 0xdc00 && codice <= 0xdfff;
}

/**
 * Calcola la sostituzione minima (prefisso e suffisso comuni tolti) fra due testi.
 *
 * Serve a ricaricare il file da disco senza perdere cursore e scorrimento: con
 * una sostituzione dell'intero documento CodeMirror riporterebbe la selezione
 * all'inizio, mentre con una modifica limitata al tratto cambiato la selezione
 * si sposta da sé. I tagli non spezzano mai una coppia surrogata (emoji).
 */
export function sostituzioneMinima(vecchio: string, nuovo: string): Sostituzione {
  const max = Math.min(vecchio.length, nuovo.length);
  let prefisso = 0;
  while (prefisso < max && vecchio.charCodeAt(prefisso) === nuovo.charCodeAt(prefisso)) {
    prefisso++;
  }
  // Non tagliare in mezzo a una coppia surrogata: se l'ultima unità comune è
  // "alta", il carattere intero non è comune.
  if (prefisso > 0 && eSurrogataAlta(vecchio.charCodeAt(prefisso - 1))) {
    prefisso--;
  }
  let suffisso = 0;
  while (
    suffisso < max - prefisso &&
    vecchio.charCodeAt(vecchio.length - 1 - suffisso) === nuovo.charCodeAt(nuovo.length - 1 - suffisso)
  ) {
    suffisso++;
  }
  if (suffisso > 0 && eSurrogataBassa(vecchio.charCodeAt(vecchio.length - suffisso))) {
    suffisso--;
  }
  return {
    da: prefisso,
    a: vecchio.length - suffisso,
    inserisci: nuovo.slice(prefisso, nuovo.length - suffisso),
  };
}

/**
 * Forma normalizzata di un percorso per confronti, come fa il core in Rust:
 * - **Windows** (`windows`): separatori `/`, senza prefisso `\?\`, senza `/` finale
 *   e in minuscolo (il file system non distingue le maiuscole);
 * - **macOS/Linux**: il percorso resta com'è (`\` è un carattere valido di un nome,
 *   le maiuscole contano), tolto solo un `/` finale.
 *
 * @param windows piattaforma di riferimento; di default quella corrente (`rilevaWindows`).
 */
export function normalizzaPercorso(percorso: string, windows: boolean = rilevaWindows()): string {
  if (!windows) {
    let p = percorso;
    while (p.length > 1 && p.endsWith("/")) {
      p = p.slice(0, -1);
    }
    return p;
  }
  let p = percorso.split(String.fromCharCode(92)).join("/");
  // Prefissi verbatim/dispositivo (`\\?\`, `\\.\`), anche `UNC` in minuscolo.
  let unc = false;
  const mUnc = /^\/\/[?.]\/unc\//i.exec(p);
  if (mUnc !== null) {
    p = p.slice(mUnc[0].length);
    unc = true;
  } else {
    const mVerb = /^\/\/[?.]\//.exec(p);
    if (mVerb !== null) {
      p = p.slice(mVerb[0].length);
    } else if (/^\/\/[^/]/.test(p)) {
      unc = true; // `//server/condivisione/...`
      p = p.slice(2);
    }
  }
  const radiceSemplice = !unc && p.startsWith("/");
  // Separatori ripetuti, segmenti `.` e `..` (un `..` non esce dalla radice).
  const pila: string[] = [];
  for (const seg of p.split("/")) {
    if (seg === "" || seg === ".") {
      continue;
    }
    if (seg === "..") {
      if (pila.length > 0 && !(pila.length === 1 && /^[A-Za-z]:$/.test(pila[0]!))) {
        pila.pop();
      }
      continue;
    }
    pila.push(seg);
  }
  const radice = unc ? "//" : radiceSemplice ? "/" : "";
  return (radice + pila.join("/")).toLowerCase();
}

/** Esito del confronto fra due percorsi. */
export type EsitoConfrontoPercorsi = "uguale" | "incerto" | "diverso";

/**
 * Confronta due percorsi in tre esiti:
 * - `uguale`: coincidono dopo la normalizzazione;
 * - `incerto`: forma diversa ma stesso nome di file (unità di rete mappate, `subst`,
 *   giunzioni, cartelle collegate: solo il file system sa se è lo stesso file).
 *   Un evento riferito al documento corrente NON va scartato in silenzio: si
 *   mostra la domanda;
 * - `diverso`: nome diverso (o percorso assente).
 */
export function confrontaPercorsi(
  a: string | null,
  b: string | null,
  windows: boolean = rilevaWindows(),
): EsitoConfrontoPercorsi {
  if (a === null || b === null) {
    return "diverso";
  }
  const na = normalizzaPercorso(a, windows);
  const nb = normalizzaPercorso(b, windows);
  if (na === nb) {
    return "uguale";
  }
  const nomeA = na.slice(na.lastIndexOf("/") + 1);
  const nomeB = nb.slice(nb.lastIndexOf("/") + 1);
  return nomeA !== "" && nomeA === nomeB ? "incerto" : "diverso";
}

/** `true` se i due percorsi indicano con certezza lo stesso file (vedi [`confrontaPercorsi`]). */
export function stessoPercorso(
  a: string | null,
  b: string | null,
  windows: boolean = rilevaWindows(),
): boolean {
  return confrontaPercorsi(a, b, windows) === "uguale";
}

/** Cosa fare dopo `ripristina_documento_sparito`. */
export type AzioneDopoSparito = "ripristinato" | "ricomparso" | "ignora" | "non-ripristinato";

/**
 * Decide la reazione all'esito di `ripristina_documento_sparito`.
 * Con `incerto` (percorso dell'evento in forma diversa, stesso nome: potrebbe essere
 * un omonimo in un'altra cartella, o lo stato punta ancora al documento precedente)
 * l'esito «esiste» è un no-op silenzioso: il documento non è mai sparito, quindi non
 * va segnato modificato né accompagnato dall'avviso «ricomparso».
 */
export function azioneDopoSparito(esito: string, incerto: boolean): AzioneDopoSparito {
  if (esito === "ripristinato") {
    return "ripristinato";
  }
  if (esito === "esiste") {
    return incerto ? "ignora" : "ricomparso";
  }
  return "non-ripristinato";
}

/**
 * `true` se, per un evento `ricarica` con percorso `incerto`, il disco ha lo stesso
 * contenuto dell'editor (a meno della fine riga): ricaricare e avvisare sarebbe un
 * falso allarme. Con percorso certo vale il comportamento di sempre (`false`).
 */
export function ricaricaSuperflua(contenutoDisco: string, testoEditor: string, incerto: boolean): boolean {
  return incerto && normalizzaFineRiga(contenutoDisco) === testoEditor;
}

/**
 * `true` se, per un evento `chiedi`/`ricarica` con percorso `incerto` e testo già
 * modificato, il disco contiene ancora ESATTAMENTE l'ultima versione nota del file del
 * documento (`testoDisco`, solo LF, o `null` se ignota): falso allarme (omonimo in altra
 * cartella), non si chiede nulla. Con `testoDisco === null` non si può escludere un
 * conflitto vero, quindi `false`.
 */
export function discoInvariato(contenutoDisco: string, testoDisco: string | null): boolean {
  return testoDisco !== null && normalizzaFineRiga(contenutoDisco) === testoDisco;
}

/** Testo del motivo per cui il documento sparito non è stato ripristinato. */
export function motivoNonRipristinato(esito: string): string {
  switch (esito) {
    case "vuoto":
      return "il testo nell'editor è vuoto";
    case "non-utf8":
      return "il testo nell'editor non è UTF-8 valido";
    case "troncato":
      return "il testo nell'editor sembra troncato rispetto all'ultima versione nota";
    case "cartella-assente":
      return "la cartella che conteneva il file non esiste più";
    default:
      return "il testo nell'editor non ha superato la verifica";
  }
}

/**
 * Avviso da mostrare se una rinomina esterna cambia l'estensione fra Typst
 * (`.typ`) e Markdown (`.md`), altrimenti `null`. Il tipo del documento NON
 * cambia da solo: il testo nell'editor è quello di prima e reinterpretarlo
 * (anteprima, evidenziazione, pipeline di compilazione) lo romperebbe in
 * silenzio. Il tipo cambia solo con una conversione o un «Salva con nome».
 *
 * @param tipoCorrente tipo del documento aperto.
 * @param nuovoPercorso percorso dopo la rinomina.
 * @param nome nome da citare nell'avviso (file rinominato).
 */
export function avvisoTipoDopoRinomina(
  tipoCorrente: "typst" | "markdown",
  nuovoPercorso: string,
  nome: string,
): string | null {
  const tipoNuovo = /\.md$/i.test(nuovoPercorso) ? "markdown" : "typst";
  if (tipoNuovo === tipoCorrente) {
    return null;
  }
  const attuale = tipoCorrente === "typst" ? "Typst" : "Markdown";
  const estensione = tipoNuovo === "markdown" ? ".md" : ".typ";
  return (
    `«${nome}» ha ora estensione ${estensione}, ma il documento resta ${attuale}: ` +
    "il tipo non cambia da solo (usa la conversione, o «Salva con nome» per sceglierlo)."
  );
}
