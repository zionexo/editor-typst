/**
 * esporta.ts — Modale delle opzioni di esportazione (F5-011).
 *
 * Raccoglie pagine, standard PDF/A (solo PDF), risoluzione (solo PNG) e, per le
 * conversioni, larghezza del testo (txt), HTML autonomo (html), classe e carta
 * (tex); il Markdown non ha opzioni e non apre la modale. Restituisce
 * restituisce le scelte con una promise: `null` se l'utente annulla. Non
 * scrive file né chiama il backend (lo fa `fileops.ts`). L'ultimo standard
 * scelto per il PDF è ricordato in `impostazioni.json` (F5-003).
 *
 * La logica pura (validazione PPI, nomi, mappatura verso il backend) sta in
 * `opzioniesporta.ts`, verificata da `opzioniesporta.check.ts`.
 */
import type { Editor } from "./editor";
import { leggiImpostazioni } from "./impostazioni";
import {
  LARGHEZZA_TESTO_MAX,
  LARGHEZZA_TESTO_MIN,
  OPZIONI_CONVERSIONE_PREDEFINITE,
  PPI_MASSIMO,
  PPI_PREDEFINITO,
  STANDARD_UI,
  cartaLatexValida,
  classeLatexValida,
  leggiLarghezza,
  leggiPpi,
  pagineOppureNull,
  standardValido,
} from "./opzioniesporta";
import type { FormatoEsportazione, OpzioniEsportazione } from "./opzioniesporta";

/** API pubblica del dialogo verso il resto del frontend. */
export interface DialogoEsporta {
  /** Mostra la modale per `formato`; `null` se annullata o già aperta. */
  chiedi(formato: FormatoEsportazione): Promise<OpzioniEsportazione | null>;
}

/** Recupera un elemento per id o solleva un errore esplicito. */
function elem<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

const TITOLI: Record<FormatoEsportazione, string> = {
  pdf: "Esporta in PDF",
  png: "Esporta in PNG",
  svg: "Esporta in SVG",
  txt: "Esporta come testo semplice",
  html: "Esporta come HTML",
  tex: "Esporta come LaTeX",
  md: "Esporta come Markdown",
};

/** Crea il dialogo. Lo standard scelto si salva dopo l'export riuscito (fileops.ts). */
export function creaDialogoEsporta(editor: Editor): DialogoEsporta {
  const overlay = elem<HTMLDivElement>("modal-esporta");
  const titolo = elem<HTMLElement>("modal-esporta-titolo");
  const inputPagine = elem<HTMLInputElement>("esporta-pagine");
  const rigaPagine = elem<HTMLLabelElement>("esporta-riga-pagine");
  const rigaLarghezza = elem<HTMLLabelElement>("esporta-riga-larghezza");
  const inputLarghezza = elem<HTMLInputElement>("esporta-larghezza");
  const rigaAutonomo = elem<HTMLLabelElement>("esporta-riga-autonomo");
  const inputAutonomo = elem<HTMLInputElement>("esporta-autonomo");
  const aiutoHtml = elem<HTMLElement>("esporta-aiuto-html");
  const rigaClasse = elem<HTMLLabelElement>("esporta-riga-classe");
  const selectClasse = elem<HTMLSelectElement>("esporta-classe");
  const rigaCarta = elem<HTMLLabelElement>("esporta-riga-carta");
  const selectCarta = elem<HTMLSelectElement>("esporta-carta");
  const rigaStandard = elem<HTMLLabelElement>("esporta-riga-standard");
  const selectStandard = elem<HTMLSelectElement>("esporta-standard");
  const aiutoStandard = elem<HTMLElement>("esporta-aiuto-standard");
  const rigaPpi = elem<HTMLLabelElement>("esporta-riga-ppi");
  const inputPpi = elem<HTMLInputElement>("esporta-ppi");
  const aiutoFile = elem<HTMLElement>("esporta-aiuto-file");
  const messaggio = elem<HTMLDivElement>("esporta-messaggio");
  const btnAnnulla = elem<HTMLButtonElement>("esporta-annulla");
  const btnConferma = elem<HTMLButtonElement>("esporta-conferma");

  // Le opzioni della select vengono dalla stessa tabella usata dai controlli.
  for (const s of STANDARD_UI) {
    const o = document.createElement("option");
    o.value = s.valore;
    o.textContent = s.etichetta;
    selectStandard.append(o);
  }
  inputPpi.max = String(PPI_MASSIMO);
  inputLarghezza.min = String(LARGHEZZA_TESTO_MIN);
  inputLarghezza.max = String(LARGHEZZA_TESTO_MAX);

  let formatoCorrente: FormatoEsportazione = "pdf";
  /** Funzione che chiude la promise in corso; `null` se la modale è chiusa. */
  let risolvi: ((o: OpzioniEsportazione | null) => void) | null = null;
  /** Elemento che aveva il focus all'apertura della modale. */
  let focusPrecedente: HTMLElement | null = null;

  function aggiornaAiuto(): void {
    const scelto = STANDARD_UI.find((s) => s.valore === selectStandard.value);
    aiutoStandard.textContent = scelto?.aiuto ?? "";
  }

  function chiudi(esito: OpzioniEsportazione | null): void {
    overlay.hidden = true;
    const fine = risolvi;
    risolvi = null;
    // Il focus torna dov'era all'apertura (pulsante, palette → editor), se esiste ancora
    // ed è focalizzabile: il pulsante Esporta è disabilitato per tutta l'operazione,
    // quindi in quel caso si ripiega sull'editor.
    if (
      focusPrecedente?.isConnected &&
      focusPrecedente !== document.body &&
      !focusPrecedente.matches(":disabled")
    ) {
      focusPrecedente.focus();
    } else {
      editor.focus();
    }
    focusPrecedente = null;
    fine?.(esito);
  }

  function conferma(): void {
    const ppi = leggiPpi(inputPpi.value);
    if (formatoCorrente === "png" && ppi === null) {
      messaggio.textContent = `Risoluzione non valida (intero da 1 a ${PPI_MASSIMO}).`;
      return;
    }
    const larghezza = leggiLarghezza(inputLarghezza.value);
    if (formatoCorrente === "txt" && larghezza === null) {
      messaggio.textContent = `Larghezza non valida (intero da ${LARGHEZZA_TESTO_MIN} a ${LARGHEZZA_TESTO_MAX}).`;
      return;
    }
    const standard = standardValido(selectStandard.value);
    chiudi({
      pagine: pagineOppureNull(inputPagine.value),
      standard: formatoCorrente === "pdf" ? standard : "nessuno",
      ppi: ppi ?? PPI_PREDEFINITO,
      larghezza: larghezza ?? OPZIONI_CONVERSIONE_PREDEFINITE.larghezza,
      htmlAutonomo: formatoCorrente === "html" && inputAutonomo.checked,
      latexClasse: classeLatexValida(selectClasse.value),
      latexCarta: cartaLatexValida(selectCarta.value),
    });
  }

  function chiedi(formato: FormatoEsportazione): Promise<OpzioniEsportazione | null> {
    if (risolvi !== null) {
      return Promise.resolve(null); // già aperta (es. scorciatoia premuta due volte)
    }
    if (formato === "md") {
      // Nessuna opzione da chiedere: si procede con i valori predefiniti.
      return Promise.resolve({
        pagine: null,
        standard: "nessuno",
        ppi: PPI_PREDEFINITO,
        ...OPZIONI_CONVERSIONE_PREDEFINITE,
      });
    }
    formatoCorrente = formato;
    focusPrecedente = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    titolo.textContent = TITOLI[formato];
    const daCompilazione = formato === "pdf" || formato === "png" || formato === "svg";
    rigaPagine.hidden = !daCompilazione;
    rigaStandard.hidden = formato !== "pdf";
    aiutoStandard.hidden = formato !== "pdf";
    rigaPpi.hidden = formato !== "png";
    aiutoFile.hidden = formato !== "png" && formato !== "svg";
    rigaLarghezza.hidden = formato !== "txt";
    rigaAutonomo.hidden = formato !== "html";
    aiutoHtml.hidden = formato !== "html";
    rigaClasse.hidden = formato !== "tex";
    rigaCarta.hidden = formato !== "tex";
    messaggio.textContent = "";
    selectStandard.value = standardValido(leggiImpostazioni().standard_pdf);
    aggiornaAiuto();
    if (inputPpi.value === "") {
      inputPpi.value = String(PPI_PREDEFINITO);
    }
    overlay.hidden = false;
    (daCompilazione ? inputPagine : formato === "txt" ? inputLarghezza : btnConferma).focus();
    return new Promise((ok) => {
      risolvi = ok;
    });
  }

  // Focus trap come nel modale @local: Tab cicla solo fra i controlli visibili.
  overlay.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.key === "Escape") {
      ev.preventDefault();
      chiudi(null);
      return;
    }
    if (ev.key === "Enter" && ev.target !== btnAnnulla && !(ev.target instanceof HTMLSelectElement)) {
      ev.preventDefault();
      conferma();
      return;
    }
    if (ev.key !== "Tab") {
      return;
    }
    ev.preventDefault();
    const campi: HTMLElement[] = [
      inputPagine,
      selectStandard,
      inputPpi,
      inputLarghezza,
      inputAutonomo,
      selectClasse,
      selectCarta,
      btnAnnulla,
      btnConferma,
    ].filter(
      (c) => c.offsetParent !== null,
    );
    const corrente = campi.indexOf(document.activeElement as HTMLElement);
    const n = campi.length;
    const base = corrente === -1 ? 0 : corrente;
    campi[(base + (ev.shiftKey ? -1 : 1) + n) % n]?.focus();
  });
  selectStandard.addEventListener("change", aggiornaAiuto);
  btnAnnulla.addEventListener("click", () => chiudi(null));
  btnConferma.addEventListener("click", () => conferma());
  // Clic sullo sfondo (fuori dal riquadro) → annulla.
  overlay.addEventListener("click", (ev: MouseEvent) => {
    if (ev.target === overlay) {
      chiudi(null);
    }
  });

  return { chiedi };
}
