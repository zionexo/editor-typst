/**
 * local.ts — Pubblicazione di pacchetti `@local` (Fase 4).
 *
 * Mostra un piccolo modale (nome + versione) e pubblica il documento corrente
 * come pacchetto riutilizzabile nella data dir standard, tramite il comando
 * `pubblica_local`. Dopo la pubblicazione il pacchetto è importabile da qualsiasi
 * documento con `#import "@local/{nome}:{versione}"`.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Editor } from "./editor";

// Vincoli coerenti col backend (difesa in profondità).
const RE_NOME = /^[a-z0-9-]+$/;
const RE_VERSIONE = /^\d+\.\d+\.\d+$/;

/** API pubblica del gestore @local. */
export interface GestoreLocal {
  /** Apre il modale di pubblicazione. */
  apri(): void;
}

const MSG_SOLO_TYPST =
  "@local pubblica codice Typst: converti prima il documento in Typst (pulsante «→ Typst»).";

/** Recupera un elemento per id o solleva un errore esplicito. */
function elem<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

/**
 * Crea il gestore della pubblicazione `@local`.
 *
 * @param editor  editor da cui prendere il contenuto del pacchetto (`lib.typ`).
 * @param onStato invocata con un messaggio di esito (es. per la barra di stato).
 * @param pubblicabile `true` se il documento corrente è Typst. Un `lib.typ` va
 *                    pubblicato come Typst: con un documento Markdown si RIFIUTA
 *                    con un messaggio (scelta più semplice e sicura rispetto a
 *                    convertire in silenzio, che pubblicherebbe un pacchetto
 *                    diverso da ciò che l'utente vede; si converte con «→ Typst»).
 */
export function creaGestoreLocal(
  editor: Editor,
  onStato: (messaggio: string) => void,
  pubblicabile: () => boolean = () => true,
): GestoreLocal {
  const overlay = elem<HTMLDivElement>("modal-local");
  const inputNome = elem<HTMLInputElement>("local-nome");
  const inputVersione = elem<HTMLInputElement>("local-versione");
  const messaggio = elem<HTMLDivElement>("local-messaggio");
  const btnAnnulla = elem<HTMLButtonElement>("local-annulla");
  const btnPubblica = elem<HTMLButtonElement>("local-pubblica");

  function apri(): void {
    if (!pubblicabile()) {
      onStato(MSG_SOLO_TYPST);
      return;
    }
    messaggio.textContent = "";
    overlay.hidden = false;
    inputNome.focus();
  }
  function chiudi(): void {
    overlay.hidden = true;
    editor.focus();
  }

  async function pubblica(): Promise<void> {
    // Guardia di re-entrancy: il pulsante viene disabilitato durante la
    // pubblicazione, ma il percorso da tastiera (Invio sui campi) chiamerebbe
    // `pubblica` di nuovo ignorando lo stato del pulsante → invoke concorrenti.
    if (btnPubblica.disabled) {
      return;
    }
    const nome = inputNome.value.trim();
    const versione = inputVersione.value.trim();
    if (!RE_NOME.test(nome)) {
      messaggio.textContent = "Nome non valido (lettere minuscole, cifre, trattini).";
      return;
    }
    if (!RE_VERSIONE.test(versione)) {
      messaggio.textContent = "Versione non valida (es. 0.1.0).";
      return;
    }
    if (!pubblicabile()) {
      // Il tipo può essere cambiato a modale aperto.
      messaggio.textContent = MSG_SOLO_TYPST;
      return;
    }
    btnPubblica.disabled = true;
    try {
      await invoke("pubblica_local", {
        nome,
        versione,
        contenuto: editor.testo(),
      });
      chiudi();
      onStato(
        `Pubblicato @local/${nome}:${versione} — importa con #import "@local/${nome}:${versione}"`,
      );
    } catch (e) {
      messaggio.textContent =
        typeof e === "string" ? e : "Errore nella pubblicazione.";
    } finally {
      btnPubblica.disabled = false;
    }
  }

  // Focus trap: mentre il modale è aperto, Tab/Shift+Tab ciclano solo tra i suoi
  // elementi (non escono verso i pulsanti dell'header sotto al modale).
  const focusabili: HTMLElement[] = [
    inputNome,
    inputVersione,
    btnAnnulla,
    btnPubblica,
  ];
  overlay.addEventListener("keydown", (ev: KeyboardEvent) => {
    // Esc sull'overlay vale per qualunque controllo (anche i pulsanti).
    if (ev.key === "Escape") {
      ev.preventDefault();
      chiudi();
      return;
    }
    if (ev.key !== "Tab") {
      return;
    }
    ev.preventDefault();
    const corrente = focusabili.indexOf(document.activeElement as HTMLElement);
    const n = focusabili.length;
    const base = corrente === -1 ? 0 : corrente;
    const prossimo = (base + (ev.shiftKey ? -1 : 1) + n) % n;
    focusabili[prossimo].focus();
  });

  btnAnnulla.addEventListener("click", () => chiudi());
  btnPubblica.addEventListener("click", () => void pubblica());
  // Clic sullo sfondo (fuori dal riquadro) → chiude.
  overlay.addEventListener("click", (ev: MouseEvent) => {
    if (ev.target === overlay) {
      chiudi();
    }
  });
  // Invio sui campi → pubblica (Esc è gestito sull'overlay).
  for (const campo of [inputNome, inputVersione]) {
    campo.addEventListener("keydown", (ev: KeyboardEvent) => {
      if (ev.key === "Enter") {
        ev.preventDefault();
        void pubblica();
      }
    });
  }

  return { apri };
}
