/**
 * file.ts — Pannello "File" della barra laterale (F5-021).
 *
 * Albero della cartella-progetto agganciata, **in sola consultazione**
 * (D-015): cartelle comprimibili, file `.typ`/`.md` apribili con un clic (stesso
 * percorso di apertura della modalità progetto), risorse mostrate ma non
 * aperte, documento aperto evidenziato. Senza una cartella agganciata (documento
 * nuovo) il pannello invita ad aprirne una. L'elenco viene dal core (comando
 * `albero_progetto`: ordinamento, esclusioni e confinamento nella radice sono
 * lì); si aggiorna da solo su `modifica-esterna/albero` (watcher F5-022) e con
 * il pulsante "Aggiorna".
 *
 * Accessibilità: `role="tree"` con righe `treeitem` piatte (`aria-level`,
 * `aria-expanded` sulle cartelle, `aria-selected` sul documento aperto,
 * `aria-disabled` sulle risorse), *roving tabindex* e tastiera del pattern
 * WAI-ARIA "tree" (frecce, Home/End, destra/sinistra, Invio/Spazio).
 * Nomi di file solo con `textContent`.
 */

import type { Pannello } from "../barra-laterale";
import {
  cartelleDaAprire,
  eIlDocumento,
  righeVisibili,
  type AlberoFile,
  type RigaFile,
} from "./file-logica";
import { navigaAlbero } from "./tastiera";

/** Identificatore del pannello nella barra laterale. */
export const ID_PANNELLO_FILE = "file";

/** Attesa (ms) prima di rileggere l'albero su una raffica di eventi del watcher. */
const ATTESA_AGGIORNAMENTO_MS = 300;

/** Dipendenze iniettate da `main.ts`. */
export interface OpzioniFile {
  /** Legge l'albero dal core; `null` se non c'è una cartella agganciata. */
  carica: () => Promise<AlberoFile | null>;
  /** Percorso assoluto del documento aperto, o `null` se nuovo. */
  percorsoDocumento: () => string | null;
  /** Apre il file (percorso relativo alla radice, con `/`). */
  apri: (relativo: string) => void | Promise<void>;
  /** Il file scelto è già il documento aperto: di norma si riporta il fuoco all'editor. */
  giaAperto: () => void;
  /** Fa scegliere e apre una cartella-progetto. */
  apriCartella: () => void | Promise<void>;
  /** Messaggio informativo non di errore. */
  onAvviso: (messaggio: string) => void;
  /** Errore di lettura. */
  onErrore: (errore: unknown) => void;
}

/** Pannello File, con i punti d'ingresso usati da `main.ts`. */
export interface PannelloFile extends Pannello {
  /** Rilegge l'albero (con una breve attesa che raggruppa più richieste). */
  aggiorna(): void;
  /** Da chiamare quando cambia il documento aperto: evidenzia e, se serve, rilegge. */
  documentoCambiato(percorso: string | null): void;
}

/** Crea il pannello File. */
export function creaPannelloFile(opz: OpzioniFile): PannelloFile {
  const radice = document.createElement("div");
  radice.className = "file-pannello";

  const azioni = document.createElement("div");
  azioni.className = "barra-azioni";
  const nomeRadice = document.createElement("span");
  nomeRadice.className = "file-radice";
  const btnAggiorna = document.createElement("button");
  btnAggiorna.type = "button";
  btnAggiorna.className = "barra-azione";
  btnAggiorna.title = "Aggiorna l'elenco dei file";
  btnAggiorna.setAttribute("aria-label", "Aggiorna l'elenco dei file");
  btnAggiorna.textContent = "↻";
  azioni.append(nomeRadice, btnAggiorna);

  const messaggio = document.createElement("div");
  messaggio.className = "pannello-vuoto";
  messaggio.setAttribute("aria-live", "polite");
  const btnCartella = document.createElement("button");
  btnCartella.type = "button";
  btnCartella.className = "barra-pulsante";
  btnCartella.textContent = "Apri cartella…";
  btnCartella.addEventListener("click", () => {
    void Promise.resolve(opz.apriCartella()).catch(opz.onErrore);
  });

  const albero = document.createElement("div");
  albero.className = "albero";
  albero.setAttribute("role", "tree");
  albero.setAttribute("aria-label", "File del progetto");

  radice.append(azioni, messaggio, albero);

  let dati: AlberoFile | null = null;
  const aperte = new Set<string>();
  let righe: RigaFile[] = [];
  let attiva = 0;
  let visibile = false;
  let ultimoDocumento: string | null | undefined;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let richiesta = 0;

  function elementiDom(): HTMLElement[] {
    return Array.from(albero.querySelectorAll<HTMLElement>(".file-riga"));
  }

  function aggiornaRoving(): void {
    elementiDom().forEach((r, i) => (r.tabIndex = i === attiva ? 0 : -1));
  }

  function disegna(): void {
    // Il fuoco, se è nell'albero, resta sulla stessa voce (per percorso) dopo il ridisegno.
    const attivo = document.activeElement;
    const percorsoConFuoco =
      attivo instanceof HTMLElement && albero.contains(attivo) ? attivo.dataset.percorso : undefined;
    albero.replaceChildren();
    const doc = opz.percorsoDocumento();

    if (dati === null) {
      nomeRadice.textContent = "";
      btnAggiorna.hidden = true;
      albero.hidden = true;
      messaggio.hidden = false;
      messaggio.replaceChildren(
        "Nessuna cartella aperta. Apri una cartella per vedere i suoi file e aprirli con un clic.",
        document.createElement("br"),
        btnCartella,
      );
      righe = [];
      return;
    }

    btnAggiorna.hidden = false;
    albero.hidden = false;
    nomeRadice.textContent = nomeCartella(dati.radice);
    nomeRadice.title = dati.radice;
    righe = righeVisibili(dati.nodi, aperte);
    if (righe.length === 0) {
      messaggio.hidden = false;
      messaggio.textContent = "La cartella non contiene file visibili.";
    } else if (dati.troncato) {
      messaggio.hidden = false;
      messaggio.textContent = "Elenco parziale: la cartella è molto grande.";
    } else {
      messaggio.hidden = true;
    }

    righe.forEach((r, i) => {
      const n = r.nodo;
      const el = document.createElement("div");
      el.className = `file-riga file-${n.tipo}`;
      el.setAttribute("role", "treeitem");
      el.setAttribute("aria-level", String(r.livello + 1));
      el.dataset.i = String(i);
      el.dataset.percorso = n.percorso;
      el.tabIndex = -1;
      el.style.paddingLeft = `${8 + r.livello * 14}px`;
      if (n.tipo === "cartella") {
        el.setAttribute("aria-expanded", String(r.aperta));
      } else if (n.tipo === "risorsa") {
        el.setAttribute("aria-disabled", "true");
        el.title = `${n.percorso} — risorsa: non si apre nell'editor`;
      } else {
        el.title = n.percorso;
      }
      if (n.tipo === "documento" && eIlDocumento(dati?.radice ?? "", n.percorso, doc)) {
        el.setAttribute("aria-selected", "true");
        el.classList.add("selezionata");
      }
      const etichetta = document.createElement("span");
      etichetta.className = "file-nome";
      etichetta.textContent = n.nome;
      el.append(etichetta);
      albero.append(el);
    });

    const conFuoco = percorsoConFuoco === undefined ? -1 : righe.findIndex((r) => r.nodo.percorso === percorsoConFuoco);
    const selezionata = righe.findIndex((r) => eIlDocumento(dati?.radice ?? "", r.nodo.percorso, doc));
    attiva = conFuoco >= 0 ? conFuoco : selezionata >= 0 ? selezionata : Math.min(attiva, Math.max(righe.length - 1, 0));
    aggiornaRoving();
    if (conFuoco >= 0) {
      elementiDom()[conFuoco]?.focus();
    }
  }

  /** Ultimo segmento di un percorso assoluto (con `/` o `\`). */
  function nomeCartella(percorso: string): string {
    const segmenti = percorso.split(/[/\\]/).filter(Boolean);
    return segmenti.length > 0 ? segmenti[segmenti.length - 1] : percorso;
  }

  async function carica(): Promise<void> {
    const mia = ++richiesta;
    let nuovo: AlberoFile | null;
    try {
      nuovo = await opz.carica();
    } catch (e) {
      opz.onErrore(e);
      return;
    }
    if (mia !== richiesta) {
      return;
    }
    dati = nuovo;
    if (dati !== null) {
      // Il documento aperto dev'essere visibile: se ne aprono le cartelle antenate.
      for (const c of cartelleDaAprire(dati.nodi, dati.radice, opz.percorsoDocumento())) {
        aperte.add(c);
      }
    }
    disegna();
  }

  function pianifica(): void {
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      void carica();
    }, ATTESA_AGGIORNAMENTO_MS);
  }

  function attivaRiga(i: number): void {
    const r = righe[i];
    if (r === undefined) {
      return;
    }
    attiva = i;
    const n = r.nodo;
    if (n.tipo === "cartella") {
      alternaCartella(i);
    } else if (n.tipo === "documento") {
      if (dati !== null && eIlDocumento(dati.radice, n.percorso, opz.percorsoDocumento())) {
        // Riaprire lo stesso file rileggerebbe il disco sopra le modifiche non salvate.
        opz.giaAperto();
        return;
      }
      void Promise.resolve(opz.apri(n.percorso)).catch(opz.onErrore);
    } else {
      opz.onAvviso(`«${n.nome}» è una risorsa: si mostra nell'elenco ma non si apre nell'editor.`);
    }
  }

  function alternaCartella(i: number, aprire?: boolean): void {
    const r = righe[i];
    if (r === undefined || r.nodo.tipo !== "cartella") {
      return;
    }
    const percorso = r.nodo.percorso;
    const daAprire = aprire ?? !aperte.has(percorso);
    if (daAprire) {
      aperte.add(percorso);
    } else {
      aperte.delete(percorso);
    }
    attiva = i;
    disegna();
    elementiDom()[i]?.focus();
  }

  albero.addEventListener("click", (ev: MouseEvent) => {
    const riga = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".file-riga") : null;
    if (riga?.dataset.i !== undefined) {
      attivaRiga(Number(riga.dataset.i));
    }
  });
  albero.addEventListener("keydown", (ev: KeyboardEvent) => {
    const dom = elementiDom();
    const i = dom.findIndex((r) => r === document.activeElement);
    const elementi = righe.map((r) => ({
      livello: r.livello,
      eCartella: r.nodo.tipo === "cartella",
      aperta: r.aperta,
    }));
    const m = navigaAlbero(elementi, i, ev.key);
    if (m === null) {
      return;
    }
    ev.preventDefault();
    switch (m.tipo) {
      case "vai":
        attiva = m.indice;
        aggiornaRoving();
        dom[m.indice]?.focus();
        break;
      case "espandi":
        alternaCartella(m.indice, true);
        break;
      case "comprimi":
        alternaCartella(m.indice, false);
        break;
      case "attiva":
        attivaRiga(m.indice);
        break;
    }
  });
  albero.addEventListener("focusin", (ev: FocusEvent) => {
    const riga = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".file-riga") : null;
    if (riga?.dataset.i !== undefined) {
      attiva = Number(riga.dataset.i);
      aggiornaRoving();
    }
  });
  btnAggiorna.addEventListener("click", () => void carica());

  messaggio.textContent = "Caricamento…";

  return {
    id: ID_PANNELLO_FILE,
    // Cartella: contorno di una cartella con linguetta.
    icona: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    titolo: "File",
    monta: (contenitore) => contenitore.append(radice),
    mostrato: () => {
      visibile = true;
      ultimoDocumento = opz.percorsoDocumento();
      void carica();
    },
    nascosto: () => {
      visibile = false;
    },
    fuoco: () => {
      const dom = elementiDom();
      (dom[attiva] ?? dom[0] ?? btnCartella).focus();
    },
    aggiorna: () => {
      if (visibile) {
        pianifica();
      }
    },
    documentoCambiato: (percorso) => {
      if (!visibile || percorso === ultimoDocumento) {
        return; // niente da fare (o `mostrato` rilegge quando torna visibile)
      }
      ultimoDocumento = percorso;
      // Un altro documento può stare in un'altra cartella (radice diversa): si rilegge.
      void carica();
    },
  };
}
