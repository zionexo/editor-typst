/**
 * menu-opzioni.ts — Menu «Opzioni»: motore di evidenziazione Typst e assistenza alla scrittura.
 *
 * Popover ancorato al pulsante `#btn-opzioni`, riempito qui (in index.html c'è solo il
 * contenitore). Contiene un `radiogroup` «Evidenziazione Typst» con due motori (WASM,
 * Lezer). La scelta si applica subito all'editor e si persiste come lo stile.
 *
 * Accessibilità e tastiera come `menu-aspetto.ts` (pattern APG «menu button» con dialog
 * non modale): tabindex "roving", frecce/Home/End spostano e selezionano, Invio/Spazio
 * scelgono, Esc chiude restituendo il focus al pulsante (`aria-expanded`); Tab NON è
 * confinato: il menu si chiude quando il focus passa a un elemento esterno (`focusout`,
 * più `focusin` come rete di sicurezza) o con un `pointerdown` esterno. Aprire «Opzioni»
 * chiude «Aspetto» (e viceversa) per la stessa regola: il focus passa all'altro menu.
 * Nessuno stile inline se non `element.style` per la posizione (CSP di produzione, B-005).
 */
import {
  helpCompletamentoAttivo,
  impostaHelp,
  impostaMotore,
  impostaSuggerimento,
  motoreCorrente,
  osservaAspetto,
  suggerimentoParametriAttivo,
} from "./aspetto";
import { crea } from "./menu-aspetto";
import { descrizioneMotore, etichettaMotore, MOTORI, type MotoreTypst } from "./motore";
import { focusEsceDalMenu, navigaIndice, radioDaAttivare, type DestinazioneFocus } from "./stile";

/** API del menu, usata dai comandi della palette. */
export interface MenuOpzioni {
  apri(): void;
  chiudi(): void;
  alterna(): void;
  aperto(): boolean;
}

/**
 * Costruisce il menu e lo aggancia al pulsante.
 *
 * @param bottone  pulsante «Opzioni» (riceve `aria-expanded`).
 * @param pannello contenitore del popover (`#menu-opzioni`, inizialmente `hidden`).
 * @param ripristinaFocus chiamata alla chiusura da tastiera se il bottone non è utilizzabile.
 */
export function creaMenuOpzioni(
  bottone: HTMLButtonElement,
  pannello: HTMLElement,
  ripristinaFocus: () => void = () => {},
): MenuOpzioni {
  pannello.tabIndex = -1;
  const radioMotore = new Map<MotoreTypst, HTMLElement>();

  const titolo = crea("div", "ma-titolo", "Evidenziazione Typst");
  titolo.id = "mo-titolo-motore";
  const gruppo = crea("div", "mo-gruppo");
  gruppo.setAttribute("role", "radiogroup");
  gruppo.setAttribute("aria-labelledby", titolo.id);
  for (const m of MOTORI) {
    const radio = crea("div", "mo-radio");
    radio.setAttribute("role", "radio");
    radio.dataset.valore = m;
    const nome = crea("span", "mo-nome", etichettaMotore(m));
    nome.id = `mo-nome-${m}`;
    const desc = crea("span", "mo-desc", descrizioneMotore(m));
    desc.id = `mo-desc-${m}`;
    radio.setAttribute("aria-labelledby", nome.id);
    radio.setAttribute("aria-describedby", desc.id);
    radio.append(crea("span", "mo-spunta"), nome, desc);
    gruppo.appendChild(radio);
    radioMotore.set(m, radio);
  }
  const nota = crea(
    "p",
    "ma-aiuto mo-nota",
    "Cambia solo l'evidenziazione e la modifica dei file Typst, non la compilazione.",
  );
  const aiuto = crea("p", "ma-aiuto", "Frecce per scegliere, Esc per chiudere. Tab per le caselle, Spazio per attivarle. La scelta è applicata subito.");
  const titoloHelp = crea("div", "ma-titolo", "Assistenza alla scrittura");
  const casella = (testo: string, leggi: () => boolean, scrivi: (v: boolean) => void) => {
    const etichetta = crea("label", "mo-casella");
    const input = document.createElement("input");
    input.type = "checkbox";
    input.addEventListener("change", () => scrivi(input.checked));
    etichetta.append(input, crea("span", "mo-nome", testo));
    return { etichetta, input, leggi };
  };
  const caselle = [
    casella("Documentazione accanto ai suggerimenti", helpCompletamentoAttivo, impostaHelp),
    casella("Firma della funzione mentre scrivi gli argomenti", suggerimentoParametriAttivo, impostaSuggerimento),
  ];
  pannello.replaceChildren(titolo, gruppo, nota, titoloHelp, ...caselle.map((c) => c.etichetta), aiuto);

  /** Allinea aria-checked e tabindex allo stato di `aspetto.ts`. */
  function sincronizza(): void {
    const corrente = motoreCorrente();
    for (const [m, el] of radioMotore) {
      const attivo = m === corrente;
      el.setAttribute("aria-checked", String(attivo));
      el.tabIndex = attivo ? 0 : -1;
    }
    for (const c of caselle) {
      c.input.checked = c.leggi();
    }
  }
  sincronizza();
  osservaAspetto(sincronizza);

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

  function apri(): void {
    if (aperto()) {
      return;
    }
    pannello.hidden = false;
    bottone.setAttribute("aria-expanded", "true");
    posiziona();
    radioMotore.get(motoreCorrente())?.focus();
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

  function focalizzaSelezionato(): void {
    (radioMotore.get(motoreCorrente()) ?? pannello).focus();
  }

  // Selezione per clic.
  gruppo.addEventListener("click", (ev) => {
    const r = ev.target instanceof Element ? ev.target.closest<HTMLElement>(".mo-radio") : null;
    if (r?.dataset.valore !== undefined) {
      impostaMotore(r.dataset.valore as MotoreTypst);
      r.focus();
    }
  });

  // Tastiera: frecce (spostano e selezionano), Invio/Spazio (scelgono), Esc; Tab esce e chiude.
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
      // Focalizzabili: il radio selezionato e le caselle. Tab fra loro resta nel menu;
      // oltre il primo/l'ultimo esce e chiude.
      const ordine = [radioMotore.get(motoreCorrente()), ...caselle.map((c) => c.input)];
      const i = ordine.findIndex((e) => e === ev.target);
      const j = i + (ev.shiftKey ? -1 : 1);
      if (i === -1 || j < 0 || j >= ordine.length) {
        chiudi();
      }
      return;
    }
    if (ev.target instanceof HTMLInputElement) {
      if (ev.key === "Enter") {
        ev.preventDefault();
        ev.target.click(); // come i radio: Invio sceglie
      }
      return; // Spazio sulla casella: comportamento nativo.
    }
    const radio = ev.target instanceof Element ? ev.target.closest<HTMLElement>('[role="radio"]') : null;
    if (radio === null) {
      if (ev.target === pannello && ev.key.startsWith("Arrow")) {
        ev.preventDefault();
        focalizzaSelezionato();
      }
      return;
    }
    if (ev.key === "Enter" || ev.key === " ") {
      ev.preventDefault();
      if (radioDaAttivare(radio.getAttribute("aria-checked"))) {
        radio.click();
      }
      return;
    }
    const elementi = Array.from(radioMotore.values());
    const prossimo = navigaIndice(elementi.length, elementi.indexOf(radio), ev.key);
    if (prossimo !== null) {
      ev.preventDefault();
      elementi[prossimo].click();
    }
  });

  // Un clic su titolo/descrizione/nota non deve portare il focus su body: resta sul menu.
  pannello.addEventListener("mousedown", (ev) => {
    const t = ev.target;
    if (t instanceof Element && t !== pannello && t.closest('[role="radio"]') === null && t.closest(".mo-casella") === null) {
      ev.preventDefault();
      if (!pannello.contains(document.activeElement)) {
        focalizzaSelezionato();
      }
    }
  });

  // Il focus che esce verso un elemento esterno chiude il popover (Tab, scorciatoie globali,
  // apertura dell'altro menu).
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
