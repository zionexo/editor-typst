/**
 * cronologia.ts — Modale della cronologia versioni locale (F5-013).
 *
 * Elenca le versioni del file corrente (dalla più recente), mostra il testo
 * della versione scelta e la ripristina nell'editor. Il ripristino non scrive
 * su disco: il testo corrente viene prima registrato in cronologia
 * (`ripristinaVersione` di `fileops.ts`) e il documento resta "modificato"
 * finché l'utente non salva. Non chiama direttamente il backend.
 */
import type { Editor } from "./editor";
import type { GestoreFile, VersioneCronologia } from "./fileops";
import { formattaQuando } from "./quandoversione";

/** API pubblica della modale verso il resto del frontend. */
export interface DialogoCronologia {
  /** Apre la modale per il file corrente. */
  apri(): Promise<void>;
}

/** Anteprima: oltre questo numero di caratteri il testo si tronca (la versione resta intera). */
const MAX_ANTEPRIMA = 20_000;

/** Recupera un elemento per id o solleva un errore esplicito. */
function elem<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

/** Dimensione leggibile (B / KB). */
function dimensioneLeggibile(byte: number): string {
  return byte < 1024 ? `${byte} B` : `${(byte / 1024).toFixed(1)} KB`;
}

/** Crea la modale. `gestore` fornisce elenco, testo e ripristino delle versioni. */
export function creaDialogoCronologia(gestore: GestoreFile, editor: Editor): DialogoCronologia {
  const overlay = elem<HTMLDivElement>("modal-cronologia");
  const descrizione = elem<HTMLElement>("cronologia-descrizione");
  const elenco = elem<HTMLSelectElement>("cronologia-elenco");
  const anteprima = elem<HTMLPreElement>("cronologia-anteprima");
  const messaggio = elem<HTMLDivElement>("cronologia-messaggio");
  const btnChiudi = elem<HTMLButtonElement>("cronologia-chiudi");
  const btnRipristina = elem<HTMLButtonElement>("cronologia-ripristina");

  let focusPrecedente: HTMLElement | null = null;
  /** Versione dell'ultima anteprima richiesta: scarta le risposte in ritardo. */
  let richiesta = 0;
  /** `true` mentre `apri` attende l'elenco: una seconda apertura concorrente non deve duplicare le opzioni. */
  let aperturaInCorso = false;

  function chiudi(): void {
    overlay.hidden = true;
    richiesta++;
    if (focusPrecedente?.isConnected && focusPrecedente !== document.body) {
      focusPrecedente.focus();
    } else {
      editor.focus();
    }
    focusPrecedente = null;
  }

  async function mostraAnteprima(): Promise<void> {
    const id = elenco.value;
    btnRipristina.disabled = id === "";
    if (id === "") {
      anteprima.textContent = "";
      return;
    }
    const mia = ++richiesta;
    try {
      const testo = await gestore.testoVersione(id);
      if (mia !== richiesta) {
        return;
      }
      anteprima.textContent =
        testo.length > MAX_ANTEPRIMA
          ? `${testo.slice(0, MAX_ANTEPRIMA)}\n… (anteprima troncata; la versione ripristinata è completa)`
          : testo;
      messaggio.textContent = "";
    } catch (e) {
      if (mia === richiesta) {
        anteprima.textContent = "";
        messaggio.textContent = String(e instanceof Error ? e.message : e);
      }
    }
  }

  async function ripristina(): Promise<void> {
    const id = elenco.value;
    if (id === "") {
      return;
    }
    btnRipristina.disabled = true;
    // Il modale si chiude solo se il ripristino è riuscito (l'errore resta
    // visibile e si può riprovare con un'altra versione).
    const riuscito = await gestore.ripristinaVersione(id);
    if (riuscito) {
      chiudi();
    } else {
      btnRipristina.disabled = false;
    }
  }

  async function apri(): Promise<void> {
    if (!overlay.hidden || aperturaInCorso) {
      return; // già aperta (o in apertura: l'elenco arriva dopo un await)
    }
    aperturaInCorso = true;
    try {
      await apriModale();
    } finally {
      aperturaInCorso = false;
    }
  }

  async function apriModale(): Promise<void> {
    focusPrecedente = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    elenco.replaceChildren();
    anteprima.textContent = "";
    messaggio.textContent = "";
    btnRipristina.disabled = true;
    const percorso = gestore.stato().percorso;
    let versioni: VersioneCronologia[] = [];
    if (percorso === null) {
      descrizione.textContent =
        "Il documento non è ancora stato salvato: la cronologia parte dal primo salvataggio.";
    } else {
      try {
        versioni = await gestore.versioni();
      } catch (e) {
        messaggio.textContent = String(e instanceof Error ? e.message : e);
      }
      descrizione.textContent =
        versioni.length === 0
          ? "Nessuna versione registrata per questo file: se ne crea una a ogni salvataggio."
          : "Ripristinare una versione sostituisce il testo dell'editor (quello attuale viene " +
            "registrato prima). Il file su disco cambia quando salvi (o quando lo fa il " +
            "salvataggio automatico, se è attivo).";
    }
    for (const v of versioni) {
      const o = document.createElement("option");
      o.value = v.id;
      o.textContent = `${formattaQuando(v, document.documentElement.lang)} — ${dimensioneLeggibile(v.dimensione)}`;
      elenco.append(o);
    }
    overlay.hidden = false;
    if (versioni.length > 0) {
      elenco.selectedIndex = 0;
      void mostraAnteprima();
    }
    elenco.focus();
  }

  elenco.addEventListener("change", () => void mostraAnteprima());
  btnChiudi.addEventListener("click", chiudi);
  btnRipristina.addEventListener("click", () => void ripristina());
  overlay.addEventListener("click", (ev: MouseEvent) => {
    if (ev.target === overlay) {
      chiudi();
    }
  });
  overlay.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      chiudi();
      return;
    }
    if (ev.key !== "Tab") {
      return;
    }
    // Focus trap come in `esporta.ts`: Tab cicla solo fra i controlli attivi del modale.
    ev.preventDefault();
    const campi: HTMLElement[] = [elenco, btnChiudi, btnRipristina].filter(
      (c) => !(c instanceof HTMLButtonElement && c.disabled) && c.offsetParent !== null,
    );
    const n = campi.length;
    if (n === 0) {
      return;
    }
    const corrente = campi.indexOf(document.activeElement as HTMLElement);
    const base = corrente === -1 ? 0 : corrente;
    campi[(base + (ev.shiftKey ? -1 : 1) + n) % n]?.focus();
  });

  return { apri };
}
