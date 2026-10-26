/**
 * menu-aspetto.ts — Menu grafico «Aspetto»: scelta dello stile e del tema.
 *
 * Popover ancorato al pulsante `#btn-aspetto`, riempito qui (la struttura statica
 * è solo il contenitore in index.html). Contiene due `radiogroup`:
 * - tre schede per lo stile (miniatura in CSS puro + nome + descrizione);
 * - tre radio per il tema (Chiaro / Scuro / Sistema), con la logica di `aspetto.ts`.
 * La scelta si applica subito (anteprima dal vivo) e si persiste come il tema.
 *
 * Accessibilità: `role="radiogroup"`/`radio` con tabindex "roving", frecce/Home/End
 * per spostarsi (e selezionare, come i radio nativi), Invio/Spazio per scegliere,
 * Esc per chiudere restituendo il focus al pulsante (`aria-expanded`). Pattern APG
 * «menu button» con dialog non modale (la soluzione più semplice): Tab NON è confinato,
 * anzi il popover si chiude quando il focus passa a un elemento esterno (`focusout` con
 * `relatedTarget` fuori da pannello e pulsante: Tab che esce, Ctrl+P/N/O che focalizzano
 * altro); niente `aria-modal`. Il pannello è focalizzabile (`tabindex=-1`) e un clic sul
 * suo testo riporta il focus sul radio selezionato, così la tastiera arriva sempre al menu.
 * Nessuno stile inline se non `element.style` per la posizione
 * (compatibile con la CSP di produzione, B-005).
 */
import {
  impostaStile,
  impostaTema,
  osservaAspetto,
  stileCorrente,
  temaCorrente,
  type SceltaTema,
} from "./aspetto";
import {
  descrizioneStile,
  etichettaStile,
  focusEsceDalMenu,
  navigaIndice,
  radioDaAttivare,
  STILI,
  type DestinazioneFocus,
  type SceltaStile,
} from "./stile";

/** Temi mostrati nel menu, nell'ordine richiesto (la logica è quella di `aspetto.ts`). */
const TEMI: readonly { valore: SceltaTema; etichetta: string }[] = [
  { valore: "chiaro", etichetta: "Chiaro" },
  { valore: "scuro", etichetta: "Scuro" },
  { valore: "sistema", etichetta: "Sistema" },
];

/** API del menu, usata dai comandi della palette. */
export interface MenuAspetto {
  apri(): void;
  chiudi(): void;
  alterna(): void;
  aperto(): boolean;
}

export function crea<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  classe: string,
  testo?: string,
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  el.className = classe;
  if (testo !== undefined) {
    el.textContent = testo;
  }
  return el;
}

/** Miniatura grafica di uno stile: palette a cinque colori e mini-layout (solo elementi vuoti + CSS). */
function creaMiniatura(stile: SceltaStile): HTMLElement {
  const m = crea("span", "miniatura");
  m.dataset.miniatura = stile;
  m.setAttribute("aria-hidden", "true");
  const layout = crea("span", "mn-layout");
  for (const parte of ["rail", "pannello", "editor", "anteprima"]) {
    const p = crea("i", `mn-${parte}`);
    if (parte === "editor") {
      for (let k = 0; k < 4; k++) {
        p.appendChild(crea("b", "mn-riga"));
      }
    }
    if (parte === "anteprima") {
      p.appendChild(crea("b", "mn-pagina"));
    }
    layout.appendChild(p);
  }
  const palette = crea("span", "mn-palette");
  for (let k = 1; k <= 5; k++) {
    palette.appendChild(crea("i", `mn-c${k}`));
  }
  m.append(layout, palette);
  return m;
}

/**
 * Costruisce il menu e lo aggancia al pulsante.
 *
 * @param bottone  pulsante «Aspetto» (riceve `aria-expanded`).
 * @param pannello contenitore del popover (`#menu-aspetto`, inizialmente `hidden`).
 * @param ripristinaFocus chiamata alla chiusura da tastiera se il bottone non è utilizzabile.
 */
export function creaMenuAspetto(
  bottone: HTMLButtonElement,
  pannello: HTMLElement,
  ripristinaFocus: () => void = () => {},
): MenuAspetto {
  pannello.tabIndex = -1;
  const schedeStile = new Map<SceltaStile, HTMLElement>();
  const radioTema = new Map<SceltaTema, HTMLElement>();

  // --- Gruppo degli stili ---
  const titoloStile = crea("div", "ma-titolo", "Stile grafico");
  titoloStile.id = "ma-titolo-stile";
  const gruppoStile = crea("div", "ma-schede");
  gruppoStile.setAttribute("role", "radiogroup");
  gruppoStile.setAttribute("aria-labelledby", titoloStile.id);
  for (const s of STILI) {
    const scheda = crea("div", "ma-scheda");
    scheda.setAttribute("role", "radio");
    scheda.dataset.valore = s;
    const nome = crea("span", "ma-nome", etichettaStile(s));
    nome.id = `ma-nome-${s}`;
    const desc = crea("span", "ma-desc", descrizioneStile(s));
    desc.id = `ma-desc-${s}`;
    scheda.setAttribute("aria-labelledby", nome.id);
    scheda.setAttribute("aria-describedby", desc.id);
    scheda.append(creaMiniatura(s), nome, desc);
    gruppoStile.appendChild(scheda);
    schedeStile.set(s, scheda);
  }

  // --- Gruppo dei temi ---
  const titoloTema = crea("div", "ma-titolo", "Tema");
  titoloTema.id = "ma-titolo-tema";
  const gruppoTema = crea("div", "ma-temi");
  gruppoTema.setAttribute("role", "radiogroup");
  gruppoTema.setAttribute("aria-labelledby", titoloTema.id);
  for (const { valore, etichetta } of TEMI) {
    const r = crea("div", "ma-tema", etichetta);
    r.setAttribute("role", "radio");
    r.dataset.valore = valore;
    gruppoTema.appendChild(r);
    radioTema.set(valore, r);
  }

  const aiuto = crea("p", "ma-aiuto", "Frecce per scegliere, Esc per chiudere. La scelta è applicata subito.");
  pannello.replaceChildren(titoloStile, gruppoStile, titoloTema, gruppoTema, aiuto);

  /** Allinea aria-checked, tabindex e classe allo stato di `aspetto.ts`. */
  function sincronizza(): void {
    const stile = stileCorrente();
    for (const [s, el] of schedeStile) {
      const attivo = s === stile;
      el.setAttribute("aria-checked", String(attivo));
      el.tabIndex = attivo ? 0 : -1;
    }
    const tema = temaCorrente();
    for (const [t, el] of radioTema) {
      const attivo = t === tema;
      el.setAttribute("aria-checked", String(attivo));
      el.tabIndex = attivo ? 0 : -1;
    }
  }
  sincronizza();
  osservaAspetto(sincronizza);

  /** Posiziona il popover sotto il pulsante, dentro la finestra (element.style: ok con la CSP). */
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

  function apri(): void {
    if (aperto()) {
      return;
    }
    pannello.hidden = false;
    bottone.setAttribute("aria-expanded", "true");
    posiziona();
    schedeStile.get(stileCorrente())?.focus();
  }

  function chiudi(restituisciFocus = true): void {
    if (!aperto()) {
      return;
    }
    pannello.hidden = true;
    bottone.setAttribute("aria-expanded", "false");
    if (restituisciFocus) {
      bottone.focus();
      if (document.activeElement !== bottone) {
        ripristinaFocus();
      }
    }
  }

  function alterna(): void {
    if (aperto()) {
      chiudi();
    } else {
      apri();
    }
  }

  /** Radio focalizzabili del popover (roving tabindex), nell'ordine di Tab. */
  function focalizzabili(): HTMLElement[] {
    return Array.from(pannello.querySelectorAll<HTMLElement>('[role="radio"][tabindex="0"]'));
  }

  /** Riporta il focus sulla scheda dello stile selezionata (o sul pannello). */
  function focalizzaSelezionato(): void {
    (schedeStile.get(stileCorrente()) ?? pannello).focus();
  }

  // Selezione per clic.
  gruppoStile.addEventListener("click", (ev) => {
    const scheda = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".ma-scheda") : null;
    if (scheda?.dataset.valore !== undefined) {
      impostaStile(scheda.dataset.valore as SceltaStile);
      scheda.focus();
    }
  });
  gruppoTema.addEventListener("click", (ev) => {
    const r = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".ma-tema") : null;
    if (r?.dataset.valore !== undefined) {
      impostaTema(r.dataset.valore as SceltaTema);
      r.focus();
    }
  });

  // Tastiera: frecce (spostano e selezionano), Invio/Spazio (scelgono), Esc; Tab esce dal menu e lo chiude.
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
      // Tab scorre i radio del popover; dall'ultimo (Maiusc+Tab dal primo) esce: il menu si
      // chiude e il focus passa al pulsante, da cui la Tab prosegue verso l'elemento successivo.
      const lista = focalizzabili();
      const i = lista.indexOf(document.activeElement as HTMLElement);
      if (lista.length === 0 || (ev.shiftKey ? i <= 0 : i === lista.length - 1)) {
        chiudi();
      }
      return;
    }
    const radio = ev.target instanceof Element ? ev.target.closest<HTMLElement>('[role="radio"]') : null;
    if (radio === null) {
      // Focus sul pannello stesso (clic su un'area vuota): le frecce rientrano nei radio.
      if (ev.target === pannello && ev.key.startsWith("Arrow")) {
        ev.preventDefault();
        focalizzaSelezionato();
      }
      return;
    }
    const inStile = radio.classList.contains("ma-scheda");
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      if (radioDaAttivare(radio.getAttribute("aria-checked"))) {
        radio.click();
      }
      return;
    }
    const elementi = inStile ? Array.from(schedeStile.values()) : Array.from(radioTema.values());
    const prossimo = navigaIndice(elementi.length, elementi.indexOf(radio), ev.key);
    if (prossimo !== null) {
      ev.preventDefault();
      elementi[prossimo].click();
    }
  });

  // Un clic su titolo/descrizione/aiuto non deve portare il focus su body: resta sul menu.
  pannello.addEventListener("mousedown", (ev) => {
    const t = ev.target;
    if (t instanceof Element && t !== pannello && t.closest('[role="radio"]') === null) {
      ev.preventDefault();
      if (!pannello.contains(document.activeElement)) {
        focalizzaSelezionato();
      }
    }
  });

  // Il focus che esce verso un elemento esterno chiude il popover (Tab, scorciatoie globali).
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
  // Rete di sicurezza: un focus che arriva altrove da «nessun elemento» (relatedTarget nullo,
  // es. dopo una Tab fuori dal documento) chiude comunque il menu.
  document.addEventListener("focusin", (ev) => {
    const t = ev.target;
    if (aperto() && t instanceof Node) {
      const d: DestinazioneFocus = pannello.contains(t) ? "pannello" : bottone.contains(t) ? "bottone" : "fuori";
      if (focusEsceDalMenu(d)) {
        chiudi(false);
      }
    }
  });

  // Il pulsante alterna; un clic fuori dal popover lo chiude senza rubare il focus.
  bottone.addEventListener("click", alterna);
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

  return { apri, chiudi: () => chiudi(), alterna, aperto };
}
