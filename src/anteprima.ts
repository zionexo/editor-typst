/**
 * anteprima.ts — Pannello anteprima con render della sola pagina visibile (F5-008).
 *
 * Il backend restituisce solo i **metadati** per pagina (dimensioni + hash): qui
 * si crea per ogni pagina un segnaposto delle dimensioni corrette e l'SVG si
 * chiede (`svg_pagina`) soltanto quando la pagina entra nella zona vicina al
 * viewport (± 1 altezza del pannello, `IntersectionObserver`). Una pagina già
 * caricata si richiede di nuovo solo se il suo hash è cambiato. Su errore di
 * compilazione il chiamante non invoca `aggiorna`: l'ultima anteprima resta.
 *
 * Le definizioni SVG (glifi, clip path, gradienti) sono deduplicate fra le pagine
 * in un contenitore condiviso nel `body` (F5-009, vedi `svgdedup.ts`).
 *
 * Il modulo non importa Tauri: la richiesta dell'SVG è iniettata, così la
 * logica pura (`haBisognoDiSvg`) è verificabile da `anteprima.check.ts`.
 */

import { separaDefs } from "./svgdedup.ts";
import { creaCaricatorePagine, haBisognoDiSvg, tuttePagineCaricate, type PaginaSvg, type StatoPagina } from "./caricatorepagine.ts";
import { attendiCondizione, type DimensioniPagina } from "./stampa.ts";

/** Metadati di una pagina, nel formato del core (`MetadatiPagina` in world.rs). */
export interface MetadatiPagina {
  /** Larghezza in pt. */
  larghezza: number;
  /** Altezza in pt. */
  altezza: number;
  /** `hash128` del contenuto (esadecimale). */
  hash: string;
}

export { haBisognoDiSvg };
export type { PaginaSvg, StatoPagina };

/** Dipendenze iniettate da `main.ts`. */
export interface OpzioniAnteprima {
  /** Richiede l'SVG della pagina `indice` (`svg_pagina`); `null` se non disponibile. */
  chiediSvg: (indice: number) => Promise<PaginaSvg | null>;
  /** Invocata dopo l'inserimento dell'SVG di una pagina (per i marcatori di sincronizzazione). */
  onPaginaCaricata: (indice: number) => void;
  /** Invocata a ogni `aggiorna`, prima di toccare il DOM. */
  onAggiornata: () => void;
  /** Errore (di norma transitorio) di una richiesta di SVG: da loggare, non da mostrare come errore di compilazione. */
  onErrore: (errore: unknown) => void;
}

/** API pubblica dell'anteprima. */
export interface Anteprima {
  /**
   * Applica i metadati dell'ultima compilazione riuscita. Con l'elenco vuoto
   * mostra il segnaposto "documento vuoto".
   */
  aggiorna(pagine: readonly MetadatiPagina[]): void;
  /**
   * Porta in vista la pagina `indice` (0-based): dall'indice del documento quando
   * il titolo non ha una posizione nel sorgente aperto. Indice fuori range: nulla.
   */
  scorriA(indice: number): void;
  /**
   * Prima di stampare: carica TUTTE le pagine (di norma si caricano solo quelle vicine alla
   * vista), al massimo `RICHIESTE_PARALLELE_STAMPA` per volta, e riprova quelle fallite.
   * Restituisce le dimensioni di ogni pagina (pt), oppure il motivo per cui non si può
   * stampare. Da chiudere con `finisciStampa`.
   */
  preparaStampa(): Promise<EsitoPreparazioneStampa>;
  /** Dopo la stampa: ripristina il caricamento in base alle pagine in vista. Idempotente. */
  finisciStampa(): void;
}

/** Esito di `preparaStampa`. */
export type EsitoPreparazioneStampa =
  | { ok: true; pagine: DimensioniPagina[] }
  | { ok: false; motivo: "vuoto" | "scaduto" };

/** Richieste di SVG contemporanee durante il caricamento di tutte le pagine per la stampa. */
const RICHIESTE_PARALLELE_STAMPA = 8;

/** Tempo massimo per caricare tutte le pagine prima di stampare (ms). */
const SCADENZA_CARICAMENTO_STAMPA_MS = 30_000;

/** Margine di precaricamento: una volta l'altezza del pannello sopra e sotto (≈ ± 1 pagina). */
const MARGINE_PRECARICAMENTO = "100% 0px";

/**
 * Oltre questo numero di definizioni nel contenitore condiviso lo si svuota (le
 * pagine si ricaricano dalla vista): limita la crescita in sessioni lunghe,
 * dove i glifi non più usati resterebbero per sempre.
 */
const LIMITE_ID_DEFS = 5000;

/** Crea l'anteprima paginata dentro `pannello`. */
export function creaAnteprima(pannello: HTMLElement, opzioni: OpzioniAnteprima): Anteprima {
  const wrapper: HTMLElement[] = [];
  /** Metadati dell'ultima compilazione applicata (servono al formato carta della stampa). */
  let ultimiMeta: readonly MetadatiPagina[] = [];
  /** `true` fra `preparaStampa` e `finisciStampa`. */
  let inStampa = false;

  // Contenitore condiviso delle definizioni (F5-009). Non `display:none`: nei
  // browser Chromium/Firefox gradienti e clip path definiti in un SVG non
  // renderizzato possono non risolversi; qui è a dimensione zero, fuori flusso.
  const NS = "http://www.w3.org/2000/svg";
  const contenitoreDefs = document.createElementNS(NS, "svg");
  contenitoreDefs.setAttribute("width", "0");
  contenitoreDefs.setAttribute("height", "0");
  contenitoreDefs.setAttribute("aria-hidden", "true");
  contenitoreDefs.style.cssText = "position:absolute;width:0;height:0;overflow:hidden";
  const defsCondivise = document.createElementNS(NS, "defs");
  contenitoreDefs.appendChild(defsCondivise);
  document.body.appendChild(contenitoreDefs);
  /** Id delle definizioni già presenti in `defsCondivise`. */
  const idVisti = new Set<string>();

  /** Inserisce l'SVG di una pagina in `w`, spostando nel contenitore le sole definizioni nuove. */
  function inserisciSvg(w: HTMLElement, svg: string): void {
    // Inserimento come innerHTML: l'SVG è prodotto dal compilatore Typst locale, ma
    // ne fanno parte anche contenuti del documento (link `<a>`, immagini incluse
    // anche da file o pacchetti), quindi NON è codice fidato. Gli script sono
    // bloccati dalla CSP (`script-src 'self'`) e i link non navigano la webview
    // (`sincronizzazione.ts` ne annulla il clic).
    const d = separaDefs(svg, idVisti);
    if (d === null) {
      w.innerHTML = svg; // forma inattesa: pagina intatta, con le sue definizioni
      return;
    }
    // Prima le definizioni, poi la pagina: i riferimenti `#id` non restano mai pendenti.
    if (d.nuove !== "") {
      defsCondivise.insertAdjacentHTML("beforeend", d.nuove);
      for (const id of d.idNuovi) {
        idVisti.add(id);
      }
    }
    w.innerHTML = d.svg;
  }

  // Macchina a stati del caricamento (pura, verificata da `caricatorepagine.check.ts`).
  const caricatore = creaCaricatorePagine({
    chiediSvg: opzioni.chiediSvg,
    inserisci: (i, pagina) => inserisciSvg(wrapper[i], pagina.svg),
    svuotaPagina: (j) => {
      wrapper[j].innerHTML = "";
    },
    svuotaDefs: () => {
      defsCondivise.replaceChildren();
      idVisti.clear();
    },
    numeroDefs: () => idVisti.size,
    // Durante la stampa non si reimposta il contenitore: ricaricare le pagine già pronte
    // le svuoterebbe mentre la finestra di stampa è aperta.
    get limiteDefs(): number {
      return inStampa ? Number.POSITIVE_INFINITY : LIMITE_ID_DEFS;
    },
    onPaginaCaricata: opzioni.onPaginaCaricata,
    onErrore: opzioni.onErrore,
  });
  const stato = caricatore.stato;
  const carica = caricatore.carica;

  const osservatore = new IntersectionObserver(
    (voci) => {
      for (const voce of voci) {
        const i = Number((voce.target as HTMLElement).dataset.pagina);
        if (!Number.isInteger(i) || stato[i] === undefined) {
          continue;
        }
        stato[i].visibile = voce.isIntersecting;
        carica(i);
      }
    },
    { root: pannello, rootMargin: MARGINE_PRECARICAMENTO },
  );

  function svuota(): void {
    for (const w of wrapper) {
      osservatore.unobserve(w);
    }
    wrapper.length = 0;
    stato.length = 0;
    pannello.innerHTML = "";
  }

  function aggiorna(pagine: readonly MetadatiPagina[]): void {
    opzioni.onAggiornata();
    ultimiMeta = pagine;

    if (pagine.length === 0) {
      svuota();
      pannello.innerHTML =
        '<p id="placeholder-anteprima">Documento vuoto: nessuna pagina prodotta.</p>';
      return;
    }
    // Il segnaposto testuale (primo avvio o documento vuoto) non è una pagina.
    if (wrapper.length === 0) {
      pannello.innerHTML = "";
    }

    pagine.forEach((meta, i) => {
      if (i >= wrapper.length) {
        const w = document.createElement("div");
        w.className = "pagina-svg";
        // Indice di pagina per la sincronizzazione (clic anteprima → sorgente e
        // marcatori sorgente → anteprima).
        w.dataset.pagina = String(i);
        wrapper.push(w);
        // Durante la stampa anche le pagine nuove si caricano: la finestra di stampa le aspetta.
        stato.push({ atteso: meta.hash, caricato: null, visibile: inStampa, inVolo: false });
        pannello.appendChild(w);
        osservatore.observe(w);
      }
      // Segnaposto delle dimensioni corrette (1 pt = 1/72 in; il CSS scala a max-width 100%).
      const w = wrapper[i];
      w.style.width = `${meta.larghezza}pt`;
      w.style.aspectRatio = `${meta.larghezza} / ${meta.altezza}`;
      stato[i].atteso = meta.hash;
    });

    // Pagine in eccesso (il documento si è accorciato).
    while (wrapper.length > pagine.length) {
      const w = wrapper.pop();
      stato.pop();
      if (w !== undefined) {
        osservatore.unobserve(w);
        w.remove();
      }
    }

    // Le pagine già in zona non generano un nuovo evento dell'osservatore: le
    // controlla `aggiorna` (le altre partiranno quando entrano in vista).
    if (inStampa) {
      stato.forEach((st) => {
        st.visibile = true;
      });
    }
    for (let i = 0; i < stato.length; i++) {
      carica(i);
    }
  }

  function scorriA(indice: number): void {
    // Il caricamento dell'SVG parte da solo quando il segnaposto entra in vista.
    wrapper[indice]?.scrollIntoView({ block: "start", behavior: "smooth" });
  }

  async function preparaStampa(): Promise<EsitoPreparazioneStampa> {
    if (wrapper.length === 0 || ultimiMeta.length === 0) {
      return { ok: false, motivo: "vuoto" };
    }
    inStampa = true;
    stato.forEach((st) => {
      st.visibile = true;
    });
    // A ogni passo avvia le pagine che servono (con un tetto di richieste in volo) e riprova
    // quelle la cui richiesta è fallita: `carica` non fa nulla se la pagina è già a posto.
    const pronte = await attendiCondizione(() => {
      let inVolo = stato.filter((st) => st.inVolo).length;
      for (let i = 0; i < stato.length && inVolo < RICHIESTE_PARALLELE_STAMPA; i++) {
        if (haBisognoDiSvg(stato[i])) {
          carica(i);
          inVolo++;
        }
      }
      return tuttePagineCaricate(stato);
    }, SCADENZA_CARICAMENTO_STAMPA_MS);
    if (!pronte) {
      finisciStampa();
      return { ok: false, motivo: "scaduto" };
    }
    return { ok: true, pagine: ultimiMeta.map((m) => ({ larghezza: m.larghezza, altezza: m.altezza })) };
  }

  function finisciStampa(): void {
    if (!inStampa) {
      return;
    }
    inStampa = false;
    // Ri-osservare fa rivalutare a ogni pagina se è davvero in zona di precaricamento
    // (l'osservatore notifica lo stato corrente alla prima osservazione).
    osservatore.disconnect();
    wrapper.forEach((w) => osservatore.observe(w));
  }

  return { aggiorna, scorriA, preparaStampa, finisciStampa };
}
