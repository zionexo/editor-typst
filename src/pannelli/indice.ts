/**
 * indice.ts — Pannello "Indice" della barra laterale (F5-020).
 *
 * Mostra l'albero dei titoli di **tutto il documento compilato** (anche dei file
 * richiamati con `#include` e dei titoli generati da codice), ricavato dal core
 * (comando `indice_documento`). Scegliere una voce porta il cursore al titolo
 * nel documento aperto, oppure apre il file che lo contiene; il cursore mosso
 * dall'utente evidenzia il titolo corrente. Dopo una compilazione fallita il
 * core continua a fornire l'ultimo indice valido, quindi il pannello non si
 * svuota mentre si scrive.
 *
 * Accessibilità: `role="tree"` con righe `treeitem` piatte (`aria-level`,
 * `aria-setsize`, `aria-posinset`), *roving tabindex*, frecce/Home/End/Invio,
 * titolo corrente segnalato con `aria-current="location"`. Testi solo con
 * `textContent` (i titoli sono contenuto del documento).
 */

import type { Pannello } from "../barra-laterale";
import {
  destinazione,
  offsetNelDocumento,
  posizioniFratelli,
  voceCorrente,
  type Destinazione,
  type VoceIndice,
} from "./indice-logica";
import { navigaAlbero } from "./tastiera";

/** Identificatore del pannello nella barra laterale. */
export const ID_PANNELLO_INDICE = "indice";

/** Dipendenze iniettate da `main.ts` (il pannello non conosce Tauri né l'editor). */
export interface OpzioniIndice {
  /** Legge l'indice dal core (`indice_documento`). */
  richiediVoci: () => Promise<VoceIndice[]>;
  /** `true` se il documento aperto è Markdown (i titoli si abbinano per testo). */
  documentoMarkdown: () => boolean;
  /** Testo dell'editor (serve solo per i documenti Markdown). */
  testoEditor: () => string;
  /** Offset UTF-16 attuale del cursore. */
  cursore: () => number;
  /** Esegue la destinazione della voce scelta. */
  vai: (destinazione: Destinazione) => void | Promise<void>;
  /** Errore di lettura dell'indice. */
  onErrore: (errore: unknown) => void;
}

/** Pannello Indice, con i due punti d'ingresso usati da `main.ts`. */
export interface PannelloIndice extends Pannello {
  /** Da chiamare dopo ogni compilazione: rilegge l'indice (se il pannello è visibile). */
  aggiorna(): void;
  /** Da chiamare quando il cursore si sposta: evidenzia il titolo corrente. */
  cursoreCambiato(offset: number): void;
}

/** Crea il pannello Indice. */
export function creaPannelloIndice(opz: OpzioniIndice): PannelloIndice {
  const radice = document.createElement("div");
  radice.className = "indice-pannello";
  const messaggio = document.createElement("p");
  messaggio.className = "pannello-vuoto";
  const albero = document.createElement("div");
  albero.className = "albero";
  albero.setAttribute("role", "tree");
  albero.setAttribute("aria-label", "Indice del documento");
  radice.append(messaggio, albero);

  let voci: VoceIndice[] = [];
  let offsets: (number | null)[] = [];
  /** Riga corrente (titolo sotto il cursore), -1 se nessuna. */
  let corrente = -1;
  /** Riga con il fuoco "roving". */
  let attiva = 0;
  let visibile = false;
  /** Richiesta più recente: una risposta più vecchia arrivata dopo si scarta. */
  let richiesta = 0;
  /** Chiave strutturale dell'ultimo albero disegnato (per non ricostruire il DOM inutilmente). */
  let chiaveDisegnata: string | null = null;

  function righe(): HTMLElement[] {
    return Array.from(albero.querySelectorAll<HTMLElement>(".indice-riga"));
  }

  /** Chiave che cambia solo se cambia ciò che si vede (non gli offset). */
  function chiave(v: readonly VoceIndice[]): string {
    return JSON.stringify(v.map((x) => [x.livello, x.profondita, x.testo, x.pagina]));
  }

  function disegna(): void {
    const ciDeveFuoco = albero.contains(document.activeElement);
    albero.replaceChildren();
    messaggio.hidden = voci.length > 0;
    if (voci.length === 0) {
      messaggio.textContent =
        "Nessun titolo nel documento. I titoli (= Titolo in Typst, # Titolo in Markdown) compaiono qui.";
      chiaveDisegnata = chiave(voci);
      return;
    }
    const fratelli = posizioniFratelli(voci);
    voci.forEach((v, i) => {
      const riga = document.createElement("div");
      riga.className = "indice-riga";
      riga.setAttribute("role", "treeitem");
      riga.setAttribute("aria-level", String(v.profondita + 1));
      riga.setAttribute("aria-setsize", String(fratelli[i].dimensione));
      riga.setAttribute("aria-posinset", String(fratelli[i].posizione));
      riga.dataset.i = String(i);
      riga.tabIndex = -1;
      riga.style.paddingLeft = `${8 + v.profondita * 14}px`;
      const testo = document.createElement("span");
      testo.className = "indice-testo";
      testo.textContent = v.testo === "" ? "(senza titolo)" : v.testo;
      const pagina = document.createElement("span");
      pagina.className = "indice-pagina";
      pagina.textContent = `p. ${v.pagina}`;
      riga.title = `${v.testo} — pagina ${v.pagina}`;
      riga.append(testo, pagina);
      albero.append(riga);
    });
    chiaveDisegnata = chiave(voci);
    attiva = Math.min(Math.max(attiva, 0), voci.length - 1);
    aggiornaRoving();
    if (ciDeveFuoco) {
      righe()[attiva]?.focus();
    }
    marcaCorrente(false);
  }

  /** Solo la riga `attiva` è raggiungibile con Tab. */
  function aggiornaRoving(): void {
    righe().forEach((r, i) => (r.tabIndex = i === attiva ? 0 : -1));
  }

  /** Evidenzia la riga `corrente`; `scorri` la porta in vista (se il fuoco non è nell'albero). */
  function marcaCorrente(scorri: boolean): void {
    const tutte = righe();
    tutte.forEach((r, i) => {
      if (i === corrente) {
        r.setAttribute("aria-current", "location");
        r.classList.add("corrente");
      } else {
        r.removeAttribute("aria-current");
        r.classList.remove("corrente");
      }
    });
    if (scorri && corrente >= 0 && !albero.contains(document.activeElement)) {
      tutte[corrente]?.scrollIntoView({ block: "nearest" });
    }
  }

  function ricalcolaCorrente(cursore: number): boolean {
    const nuovo = voceCorrente(offsets, cursore);
    const cambiato = nuovo !== corrente;
    corrente = nuovo;
    return cambiato;
  }

  async function carica(): Promise<void> {
    const mia = ++richiesta;
    let nuove: VoceIndice[];
    try {
      nuove = await opz.richiediVoci();
    } catch (e) {
      opz.onErrore(e);
      return;
    }
    if (mia !== richiesta) {
      return; // una richiesta più recente è già in corso o conclusa
    }
    voci = nuove;
    offsets = offsetNelDocumento(voci, opz.documentoMarkdown(), opz.testoEditor());
    if (chiave(voci) !== chiaveDisegnata) {
      disegna();
    }
    ricalcolaCorrente(opz.cursore());
    marcaCorrente(true);
  }

  function attivaRiga(i: number): void {
    const v = voci[i];
    if (v === undefined) {
      return;
    }
    attiva = i;
    aggiornaRoving();
    void Promise.resolve(opz.vai(destinazione(v, offsets[i] ?? null))).catch(opz.onErrore);
  }

  albero.addEventListener("click", (ev: MouseEvent) => {
    const riga = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".indice-riga") : null;
    if (riga?.dataset.i !== undefined) {
      attivaRiga(Number(riga.dataset.i));
    }
  });
  albero.addEventListener("keydown", (ev: KeyboardEvent) => {
    const tutte = righe();
    const i = tutte.findIndex((r) => r === document.activeElement);
    const elementi = voci.map((v) => ({ livello: v.profondita, eCartella: false, aperta: false }));
    const m = navigaAlbero(elementi, i, ev.key);
    if (m === null) {
      return;
    }
    ev.preventDefault();
    if (m.tipo === "vai") {
      attiva = m.indice;
      aggiornaRoving();
      tutte[m.indice]?.focus();
    } else if (m.tipo === "attiva") {
      attivaRiga(m.indice);
    }
  });
  albero.addEventListener("focusin", (ev: FocusEvent) => {
    const riga = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".indice-riga") : null;
    if (riga?.dataset.i !== undefined) {
      attiva = Number(riga.dataset.i);
      aggiornaRoving();
    }
  });

  messaggio.textContent = "Compila il documento per vedere l'indice.";

  return {
    id: ID_PANNELLO_INDICE,
    // Elenco: tre righe di lunghezza decrescente.
    icona: "M4 6h16M4 12h16M4 18h10",
    titolo: "Indice",
    monta: (contenitore) => contenitore.append(radice),
    mostrato: () => {
      visibile = true;
      // Un aggiornamento arrivato mentre il pannello era nascosto si recupera ora.
      void carica();
    },
    nascosto: () => {
      visibile = false;
    },
    fuoco: () => {
      const r = righe();
      (r[attiva] ?? r[0])?.focus();
    },
    aggiorna: () => {
      // Nascosto: niente lavoro; `mostrato` rilegge l'indice appena il pannello torna visibile.
      if (visibile) {
        void carica();
      }
    },
    cursoreCambiato: (offset) => {
      if (visibile && ricalcolaCorrente(offset)) {
        marcaCorrente(true);
      }
    },
  };
}
