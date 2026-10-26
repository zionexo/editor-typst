/**
 * palette.ts — Palette comandi (Ctrl/Cmd+P) su registro di comandi (F5-006).
 *
 * Modale accessibile in stile `local.ts`: campo di ricerca (combobox) + elenco
 * (listbox/option) con `aria-activedescendant`; il focus resta sul campo e la
 * voce attiva si sposta con le frecce. Invio esegue, Esc chiude, e alla chiusura
 * il focus torna all'editor. La logica di filtro/ordinamento è in `comandi.ts`.
 *
 * Derivazione (UX): il modello "il comando selezionato si esegue con Invio"
 * segue la palette di MarkText (commandPalette/index.vue, MIT, vedi
 * `comandi.ts`); il codice è scritto ex novo. Non ancora gestita la discesa nei
 * sottocomandi (F7-001): i comandi si eseguono con `esegui`.
 */

import type { Editor } from "./editor";
import { filtraFuzzy, formattaScorciatoia, titoloDi, type Comando, type RegistroComandi } from "./comandi";

/** Id del comando che apre la palette: escluso dall'elenco (sarebbe autoreferenziale). */
const ID_PALETTE = "palette.apri";

/** API pubblica della palette. */
export interface Palette {
  /** Apre la palette con la query vuota (no-op se già aperta). */
  apri(): void;
  /** Chiude la palette e riporta il focus dov'era (o all'editor). */
  chiudi(): void;
  /** `true` se la palette è visibile. */
  aperta(): boolean;
}

/** Recupera un elemento per id o solleva un errore esplicito. */
function elem<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

/**
 * Crea la palette.
 *
 * @param registro registro da cui leggere i comandi abilitati.
 * @param editor   editor a cui restituire il focus alla chiusura.
 */
export function creaPalette(registro: RegistroComandi, editor: Editor): Palette {
  const overlay = elem<HTMLDivElement>("palette");
  const input = elem<HTMLInputElement>("palette-input");
  const lista = elem<HTMLUListElement>("palette-lista");
  const stato = elem<HTMLDivElement>("palette-stato");

  /** Comandi mostrati (filtrati e ordinati) e indice della voce attiva. */
  let visibili: Comando[] = [];
  let attivo = 0;
  /** Elemento col focus prima dell'apertura, da ripristinare alla chiusura. */
  let precedente: HTMLElement | null = null;

  /** Testo su cui cerca il filtro: titolo, poi id (per trovare anche `esporta.pdf`). */
  function testoRicerca(c: Comando): string {
    return `${titoloDi(c)} ${c.id}`;
  }

  function idOpzione(i: number): string {
    return `palette-opz-${i}`;
  }

  /** Ricostruisce l'elenco dalla query corrente. I titoli sono riletti a ogni apertura/digitazione. */
  function aggiorna(): void {
    visibili = filtraFuzzy(
      registro.abilitati().filter((c) => c.id !== ID_PALETTE),
      input.value,
      testoRicerca,
    );
    attivo = 0;
    lista.textContent = "";
    visibili.forEach((c, i) => {
      const li = document.createElement("li");
      li.id = idOpzione(i);
      li.setAttribute("role", "option");
      li.dataset.indice = String(i);
      const titolo = document.createElement("span");
      titolo.className = "palette-titolo";
      titolo.textContent = titoloDi(c);
      li.appendChild(titolo);
      const sc = c.scorciatoie?.[0];
      if (sc !== undefined) {
        const kbd = document.createElement("kbd");
        kbd.textContent = formattaScorciatoia(sc, registro.mac);
        li.appendChild(kbd);
      }
      lista.appendChild(li);
    });
    stato.textContent =
      visibili.length === 0 ? "Nessun comando trovato" : `${visibili.length} comandi`;
    evidenzia();
  }

  /**
   * Riflette l'indice attivo su `aria-selected` e `aria-activedescendant`; con `scorri`
   * porta la voce in vista (non da mousemove: farebbe scorrere la lista sotto il mouse).
   */
  function evidenzia(scorri = true): void {
    const opzioni = lista.children;
    for (let i = 0; i < opzioni.length; i++) {
      opzioni[i].setAttribute("aria-selected", i === attivo ? "true" : "false");
    }
    if (visibili.length > 0) {
      input.setAttribute("aria-activedescendant", idOpzione(attivo));
      if (scorri) {
        opzioni[attivo].scrollIntoView({ block: "nearest" });
      }
    } else {
      input.removeAttribute("aria-activedescendant");
    }
  }

  function sposta(delta: number): void {
    if (visibili.length === 0) {
      return;
    }
    attivo = (attivo + delta + visibili.length) % visibili.length;
    evidenzia();
  }

  function apri(): void {
    if (!overlay.hidden) {
      return;
    }
    precedente = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    input.value = "";
    overlay.hidden = false;
    aggiorna();
    input.focus();
  }

  function chiudi(): void {
    if (overlay.hidden) {
      return;
    }
    overlay.hidden = true;
    const daRipristinare = precedente;
    precedente = null;
    // Torna dov'era il focus se l'elemento esiste ancora; altrimenti (o se era il body) all'editor.
    if (daRipristinare !== null && daRipristinare.isConnected && daRipristinare !== document.body) {
      daRipristinare.focus();
      // L'elemento può essere nascosto nel frattempo (es. il menu «Aspetto» chiuso dal
      // cambio di focus): il focus() è allora un no-op e il focus resterebbe sul body.
      if (document.activeElement !== daRipristinare) {
        editor.focus();
      }
    } else {
      editor.focus();
    }
  }

  /** Esegue la voce `i`: prima chiude e rida il focus all'editor, poi lancia il comando
   *  (così un comando che apre un modale o un dialogo può prendersi il focus). */
  function eseguiVoce(i: number): void {
    const c = visibili[i];
    if (c === undefined) {
      return;
    }
    chiudi();
    registro.esegui(c.id);
  }

  input.addEventListener("input", () => aggiorna());
  input.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.isComposing) {
      return; // composizione IME in corso: Invio/frecce appartengono all'IME
    }
    switch (ev.key) {
      case "ArrowDown":
        ev.preventDefault();
        sposta(1);
        break;
      case "ArrowUp":
        ev.preventDefault();
        sposta(-1);
        break;
      // Home/End spostano la lista solo a campo vuoto; altrimenti muovono il cursore nel testo.
      case "Home":
        if (visibili.length > 0 && input.value === "") {
          ev.preventDefault();
          attivo = 0;
          evidenzia();
        }
        break;
      case "End":
        if (visibili.length > 0 && input.value === "") {
          ev.preventDefault();
          attivo = visibili.length - 1;
          evidenzia();
        }
        break;
      case "Enter":
        ev.preventDefault();
        eseguiVoce(attivo);
        break;
    }
  });

  // Esc e Tab sull'overlay (non solo sul campo): se il focus finisce altrove nel modale
  // (es. dopo un clic sul riquadro) la palette resta chiudibile e con focus intrappolato.
  overlay.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      chiudi();
    } else if (ev.key === "Tab") {
      // Nella palette c'è un solo elemento focalizzabile: il campo.
      ev.preventDefault();
      input.focus();
    }
  });

  // Il campo non deve perdere il focus cliccando sul riquadro (voci, spazi vuoti), salvo
  // sul campo stesso, dove il mousedown serve a posizionare il cursore e selezionare.
  overlay.addEventListener("mousedown", (ev: MouseEvent) => {
    if (ev.target !== input && ev.target !== overlay) {
      ev.preventDefault();
    }
  });
  lista.addEventListener("mousemove", (ev: MouseEvent) => {
    const li = (ev.target as HTMLElement).closest<HTMLElement>("li[data-indice]");
    const i = li === null ? -1 : Number(li.dataset.indice);
    if (i >= 0 && i !== attivo) {
      attivo = i;
      evidenzia(false);
    }
  });
  lista.addEventListener("click", (ev: MouseEvent) => {
    const li = (ev.target as HTMLElement).closest<HTMLElement>("li[data-indice]");
    if (li !== null) {
      eseguiVoce(Number(li.dataset.indice));
    }
  });
  // Clic sullo sfondo (fuori dal riquadro) → chiude.
  overlay.addEventListener("click", (ev: MouseEvent) => {
    if (ev.target === overlay) {
      chiudi();
    }
  });

  return { apri, chiudi, aperta: () => !overlay.hidden };
}
