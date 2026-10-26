/**
 * sincronizzazione.ts — Sync sorgente↔anteprima (Fase 4).
 *
 * Due direzioni:
 * - **anteprima → sorgente**: clic su una pagina SVG → comando `jump_da_click`
 *   (typst-ide) → porta il cursore all'offset corrispondente nel sorgente.
 * - **sorgente → anteprima**: spostamento del cursore → comando `jump_da_cursore`
 *   → evidenzia con un marcatore le posizioni sulle pagine e ci scorre.
 *
 * Coordinate: l'SVG di `typst-svg` ha `viewBox` in punti (1 unità SVG = 1 pt),
 * quindi le coordinate user-space del clic (via `getScreenCTM().inverse()`) sono
 * direttamente i pt da passare al backend, e i pt restituiti sono direttamente
 * coordinate SVG per i marcatori.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "./editor";

/** Posizione nell'anteprima restituita da `jump_da_cursore`. */
interface Posizione {
  pagina: number; // indice 0-based
  x: number; // pt
  y: number; // pt
}

/** Namespace SVG per creare i marcatori. */
const NS_SVG = "http://www.w3.org/2000/svg";
/** Debounce (ms) della sincronizzazione cursore → anteprima. */
const RITARDO_CURSORE_MS = 150;

/** API pubblica della sincronizzazione. */
export interface Sincronizzazione {
  /** Da invocare quando il cursore nel sorgente cambia (offset UTF-16). */
  daCursore(offset: number): void;
  /**
   * Da invocare quando l'SVG della pagina `indice` è stato inserito nel DOM
   * (le pagine si caricano in ritardo, F5-008): ridisegna i marcatori
   * pendenti di quella pagina.
   */
  paginaCaricata(indice: number): void;
  /** Da invocare quando l'anteprima viene aggiornata: posizioni e marcatori disegnati sono obsoleti e vengono rimossi. */
  invalida(): void;
}

/**
 * Configura la sincronizzazione bidirezionale fra editor e pannello anteprima.
 *
 * @param editor            editor in cui spostare il cursore (anteprima→sorgente).
 * @param pannelloAnteprima contenitore delle pagine SVG (`.pagina-svg[data-pagina]`).
 * @param onErrore          invocata su errore di un comando di jump.
 * @param abilitata         `false` per i documenti Markdown: gli offset di
 *                          `jump_da_click`/`jump_da_cursore` sono del Typst
 *                          generato, non del testo Markdown nell'editor.
 */
export function configuraSincronizzazione(
  editor: Editor,
  pannelloAnteprima: HTMLElement,
  onErrore: (errore: unknown) => void,
  abilitata: () => boolean = () => true,
): Sincronizzazione {
  // Quando un clic sull'anteprima sposta il cursore, il conseguente cambio di
  // selezione NON deve ri-sincronizzare l'anteprima (eviterebbe un rimbalzo).
  let ignoraProssimoCursore = false;
  let timerCursore: ReturnType<typeof setTimeout> | null = null;
  // Posizioni dell'ultima evidenziazione, per disegnare i marcatori sulle pagine
  // il cui SVG arriva dopo (caricamento in ritardo). Svuotate da `invalida`.
  let posizioniPendenti: Posizione[] = [];
  // Numero dell'ultima richiesta `jump_da_cursore`: scarta le risposte in ritardo
  // (schema di `indice.ts`), che disegnerebbero marcatori di una posizione vecchia.
  let richiesta = 0;

  // Difesa in fase di cattura: l'SVG del documento può contenere link (`<a>`); né il
  // clic né il clic centrale devono far navigare la webview fuori dall'app. Non si
  // ferma la propagazione: la sincronizzazione del clic sotto prosegue.
  for (const tipo of ["click", "auxclick"] as const) {
    pannelloAnteprima.addEventListener(
      tipo,
      (ev: MouseEvent): void => {
        if (ev.target instanceof Element && ev.target.closest("a") !== null) {
          ev.preventDefault();
        }
      },
      true,
    );
  }

  // ---- Anteprima → sorgente (clic) ----
  pannelloAnteprima.addEventListener("click", (ev: MouseEvent): void => {
    void gestisciClic(ev);
  });

  async function gestisciClic(ev: MouseEvent): Promise<void> {
    if (!(ev.target instanceof Element)) {
      return;
    }
    // Un link dell'SVG non deve navigare la webview (il jump al sorgente prosegue).
    if (ev.target.closest("a") !== null) {
      ev.preventDefault();
    }
    const svg = ev.target.closest("svg");
    const wrapper = svg?.closest<HTMLElement>(".pagina-svg");
    if (svg === null || wrapper?.dataset.pagina === undefined) {
      return;
    }
    if (!abilitata()) {
      return; // Markdown: nessun salto al sorgente (offset non confrontabili)
    }
    const pagina = Number(wrapper.dataset.pagina);
    if (!Number.isFinite(pagina)) {
      return; // data-pagina malformato: niente jump
    }
    const ctm = (svg as SVGSVGElement).getScreenCTM();
    if (ctm === null) {
      return;
    }
    // Trasforma le coordinate del clic in user-space SVG (= pt del frame).
    const punto = (svg as SVGSVGElement).createSVGPoint();
    punto.x = ev.clientX;
    punto.y = ev.clientY;
    const u = punto.matrixTransform(ctm.inverse());

    try {
      const offset = await invoke<number | null>("jump_da_click", {
        pagina,
        x: u.x,
        y: u.y,
      });
      if (offset !== null) {
        ignoraProssimoCursore = true;
        editor.vaiA(offset);
      }
    } catch (e) {
      onErrore(e);
    }
  }

  // ---- Sorgente → anteprima (cursore) ----
  function daCursore(offset: number): void {
    if (!abilitata()) {
      return; // Markdown: l'indice abbina per testo, qui nessuna evidenziazione
    }
    if (ignoraProssimoCursore) {
      ignoraProssimoCursore = false;
      return; // il cambio cursore deriva da un clic sull'anteprima: non rimbalzare
    }
    if (timerCursore !== null) {
      clearTimeout(timerCursore);
    }
    timerCursore = setTimeout(() => {
      timerCursore = null;
      void evidenzia(offset);
    }, RITARDO_CURSORE_MS);
  }

  async function evidenzia(offset: number): Promise<void> {
    // Pulisce subito i marcatori vecchi: meglio nulla che marcatori obsoleti
    // mentre si attende la risposta del backend (jump_da_cursore può essere lento).
    pulisciMarcatori();
    posizioniPendenti = [];
    const mia = ++richiesta;
    let posizioni: Posizione[];
    try {
      posizioni = await invoke<Posizione[]>("jump_da_cursore", { cursore: offset });
    } catch (e) {
      if (mia === richiesta) {
        onErrore(e);
      }
      return;
    }
    if (mia !== richiesta) {
      return; // risposta superata da una richiesta più recente (o da `invalida`)
    }
    posizioniPendenti = posizioni;
    let primo: HTMLElement | null = null;
    for (const p of posizioni) {
      const wrapper = pannelloAnteprima.querySelector<HTMLElement>(
        `.pagina-svg[data-pagina="${p.pagina}"]`,
      );
      if (wrapper == null) {
        continue;
      }
      // Pagina non ancora caricata (segnaposto): il marcatore si disegna in
      // `paginaCaricata`; intanto si scorre al segnaposto, che ne provoca il caricamento.
      const svg = wrapper.querySelector("svg");
      if (svg !== null) {
        aggiungiMarcatore(svg as SVGSVGElement, p.y);
      }
      if (primo === null) {
        primo = wrapper;
      }
    }
    // Scorre la prima posizione in vista (senza rubare il focus all'editor).
    primo?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  /** Aggiunge una barra orizzontale evidenziatrice alla riga `y` (in pt) dell'SVG. */
  function aggiungiMarcatore(svg: SVGSVGElement, y: number): void {
    const larghezza = svg.viewBox.baseVal.width || 0;
    const barra = document.createElementNS(NS_SVG, "rect");
    barra.setAttribute("class", "marcatore-sync");
    barra.setAttribute("x", "0");
    barra.setAttribute("y", String(y - 6));
    barra.setAttribute("width", String(larghezza));
    barra.setAttribute("height", "12");
    // Colore e `pointer-events: none` (il clic passa al jump anteprima→sorgente
    // sotto) stanno nel CSS della classe `.marcatore-sync` (index.html).
    svg.appendChild(barra);
  }

  /** Rimuove i marcatori esistenti (prima di disegnarne di nuovi). */
  function pulisciMarcatori(): void {
    for (const m of pannelloAnteprima.querySelectorAll(".marcatore-sync")) {
      m.remove();
    }
  }

  function paginaCaricata(indice: number): void {
    const svg = pannelloAnteprima.querySelector<SVGSVGElement>(
      `.pagina-svg[data-pagina="${indice}"] svg`,
    );
    if (svg === null) {
      return;
    }
    for (const p of posizioniPendenti) {
      if (p.pagina === indice) {
        aggiungiMarcatore(svg, p.y);
      }
    }
  }

  function invalida(): void {
    richiesta++; // una risposta in volo si riferisce all'anteprima precedente
    posizioniPendenti = [];
    pulisciMarcatori();
  }

  return { daCursore, paginaCaricata, invalida };
}
