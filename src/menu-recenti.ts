/**
 * menu-recenti.ts — Menu a tendina dei documenti aperti di recente.
 *
 * Popover ancorato al pulsante `#btn-recenti` (la freccia accanto ad «Apri»), con una
 * voce per ogni percorso di `Impostazioni.recenti`. Le voci si ricostruiscono a ogni
 * apertura, così l'elenco è sempre quello salvato. Un clic (o Invio) sulla voce chiude
 * il menu e passa il percorso a `apriPercorso`.
 *
 * Accessibilità e tastiera come `menu-opzioni.ts` (pattern APG «menu button»): `role="menu"`
 * con `menuitem` a tabindex «roving», frecce/Home/End per spostarsi, Invio/Spazio per
 * scegliere, Esc chiude restituendo il focus al pulsante, Freccia giù sul pulsante apre.
 * Tab NON è confinato: il menu si chiude quando il focus esce (`focusout`, `focusin`) o con
 * un `pointerdown` esterno. Nessuno stile inline se non `element.style` per la posizione
 * (CSP di produzione, B-005).
 */
import { leggiImpostazioni } from "./impostazioni";
import { crea } from "./menu-aspetto";
import { cartellaVoce, eSorgente, nomeVoce } from "./recenti";
import { focusEsceDalMenu, navigaIndice, type DestinazioneFocus } from "./stile";

/** API del menu. */
export interface MenuRecenti {
  apri(): void;
  chiudi(): void;
  aperto(): boolean;
}

/**
 * Costruisce il menu e lo aggancia al pulsante.
 *
 * @param bottone  pulsante ▾ (riceve `aria-expanded`).
 * @param pannello contenitore del popover (`#menu-recenti`, inizialmente `hidden`).
 * @param apriPercorso chiamata con il percorso scelto (file o cartella-progetto).
 */
export function creaMenuRecenti(
  bottone: HTMLButtonElement,
  pannello: HTMLElement,
  apriPercorso: (percorso: string) => void,
): MenuRecenti {
  pannello.tabIndex = -1;

  /** Voci che si possono scegliere, nell'ordine di visualizzazione. */
  function voci(): HTMLElement[] {
    return Array.from(pannello.querySelectorAll<HTMLElement>('[role="menuitem"]:not([aria-disabled="true"])'));
  }

  /** Ricostruisce le voci dall'elenco salvato. */
  function riempi(): void {
    const recenti = leggiImpostazioni().recenti;
    if (recenti.length === 0) {
      const vuoto = crea("div", "mr-vuoto", "Nessun documento recente");
      vuoto.setAttribute("role", "menuitem");
      vuoto.setAttribute("aria-disabled", "true");
      pannello.replaceChildren(vuoto);
      return;
    }
    pannello.replaceChildren(
      ...recenti.map((percorso) => {
        const voce = crea("div", "mr-voce");
        voce.setAttribute("role", "menuitem");
        voce.tabIndex = -1;
        voce.dataset.percorso = percorso;
        voce.title = percorso;
        const nome = crea("span", "mr-nome", nomeVoce(percorso));
        if (!eSorgente(percorso)) {
          nome.append(crea("span", "mr-tipo", "cartella"));
        }
        voce.append(nome, crea("span", "mr-cartella", cartellaVoce(percorso)));
        return voce;
      }),
    );
  }

  /** Posiziona il popover sotto il pulsante, dentro la finestra. */
  function posiziona(): void {
    const r = bottone.getBoundingClientRect();
    const larghezza = pannello.offsetWidth;
    const sinistra = Math.max(8, Math.min(r.left, window.innerWidth - larghezza - 8));
    pannello.style.top = `${Math.round(r.bottom + 6)}px`;
    pannello.style.left = `${Math.round(sinistra)}px`;
  }

  function aperto(): boolean {
    return !pannello.hidden;
  }

  /** Sposta il focus (roving tabindex) sulla voce `i`. */
  function focalizza(i: number): void {
    const lista = voci();
    lista.forEach((v, k) => (v.tabIndex = k === i ? 0 : -1));
    (lista[i] ?? pannello).focus();
  }

  function apri(): void {
    if (aperto()) {
      return;
    }
    riempi();
    pannello.hidden = false;
    bottone.setAttribute("aria-expanded", "true");
    posiziona();
    focalizza(0);
  }

  function chiudi(restituisciFocus = true): void {
    if (!aperto()) {
      return;
    }
    pannello.hidden = true;
    bottone.setAttribute("aria-expanded", "false");
    if (restituisciFocus) {
      bottone.focus();
    }
  }

  /** Sceglie la voce: chiude il menu e apre il percorso. */
  function scegli(voce: HTMLElement): void {
    const percorso = voce.dataset.percorso;
    chiudi();
    if (percorso !== undefined) {
      apriPercorso(percorso);
    }
  }

  pannello.addEventListener("click", (ev) => {
    const voce = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".mr-voce") : null;
    if (voce !== null) {
      scegli(voce);
    }
  });

  // Tastiera: frecce/Home/End spostano, Invio/Spazio scelgono, Esc chiude, Tab esce e chiude.
  pannello.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.isComposing) {
      return;
    }
    if (ev.key === "Escape") {
      ev.preventDefault();
      ev.stopPropagation();
      chiudi();
      return;
    }
    if (ev.key === "Tab") {
      chiudi();
      return;
    }
    const lista = voci();
    const voce = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".mr-voce") : null;
    if (voce !== null && (ev.key === "Enter" || ev.key === " ")) {
      ev.preventDefault();
      scegli(voce);
      return;
    }
    const prossimo = navigaIndice(lista.length, voce === null ? -1 : lista.indexOf(voce), ev.key);
    if (prossimo !== null) {
      ev.preventDefault();
      focalizza(prossimo);
    }
  });

  // Il focus che esce verso un elemento esterno chiude il popover.
  function alFocusPerso(ev: FocusEvent): void {
    const dest = ev.relatedTarget;
    let d: DestinazioneFocus = null;
    if (dest instanceof Node) {
      d = pannello.contains(dest) ? "pannello" : bottone.contains(dest) ? "bottone" : "fuori";
    }
    if (aperto() && focusEsceDalMenu(d)) {
      chiudi(false);
    }
  }
  pannello.addEventListener("focusout", alFocusPerso);
  bottone.addEventListener("focusout", alFocusPerso);
  document.addEventListener("focusin", (ev) => {
    const t = ev.target;
    if (aperto() && t instanceof Node) {
      const d: DestinazioneFocus = pannello.contains(t) ? "pannello" : bottone.contains(t) ? "bottone" : "fuori";
      if (focusEsceDalMenu(d)) {
        chiudi(false);
      }
    }
  });

  // Il pulsante alterna (Freccia giù lo apre); un clic fuori dal popover lo chiude senza rubare il focus.
  bottone.addEventListener("click", () => (aperto() ? chiudi() : apri()));
  bottone.addEventListener("keydown", (ev) => {
    if (ev.key === "ArrowDown" && !aperto()) {
      ev.preventDefault();
      apri();
    }
  });
  document.addEventListener(
    "pointerdown",
    (ev) => {
      if (aperto() && ev.target instanceof Node && !pannello.contains(ev.target) && !bottone.contains(ev.target)) {
        chiudi(false);
      }
    },
    true,
  );
  window.addEventListener("resize", () => {
    if (aperto()) {
      posiziona();
    }
  });

  return { apri, chiudi: () => chiudi(), aperto };
}
