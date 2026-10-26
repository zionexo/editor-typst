/**
 * riquadro-errori.ts — Riquadro dei diagnostici sotto l'editor (`#errori`).
 *
 * Una riga per diagnostico: gravità (parola, non solo colore), messaggio, «riga R, colonna C»
 * quando c'è una posizione nel documento corrente, poi in tono attenuato origine, traccia e
 * suggerimenti. Le righe con posizione sono pulsanti: portano il cursore sull'intervallo.
 * La logica pura sta in `errori.ts`; qui solo il DOM.
 */
import {
  contaDiagnostici,
  descriviDiagnostico,
  ordinaDiagnostici,
  riepilogoDiagnostici,
  type DiagnosticoBase,
  type Intervallo,
  type VoceErrore,
} from "./errori";

export interface RiquadroErrori {
  /**
   * Mostra l'elenco dei diagnostici. `testo` = sorgente corrente dell'editor; `markdown` = gli
   * offset sono del Typst generato, quindi nessuna posizione e nessuna riga cliccabile.
   */
  mostra(diagnostici: readonly DiagnosticoBase[], testo: string, markdown: boolean): void;
  /** Mostra un messaggio libero (errori di I/O, di comandi): annunciato subito. */
  mostraTesto(testo: string): void;
  /** Svuota e nasconde il riquadro. */
  pulisci(): void;
}

/**
 * Crea il riquadro dentro `contenitore` (di norma `#errori`).
 * @param vaiA chiamata con l'intervallo da selezionare quando si attiva una riga con posizione.
 */
export function creaRiquadroErrori(
  contenitore: HTMLElement,
  vaiA: (intervallo: Intervallo) => void,
): RiquadroErrori {
  const doc = contenitore.ownerDocument;
  // Intestazione: unico elemento "live" dell'elenco, aggiornata solo se il testo cambia.
  const intestazione = doc.createElement("div");
  intestazione.className = "errori-intestazione";
  intestazione.setAttribute("role", "status");
  const elenco = doc.createElement("ul");
  elenco.className = "errori-elenco";
  const libero = doc.createElement("div");
  libero.className = "errori-testo";
  libero.setAttribute("role", "alert");
  contenitore.append(intestazione, elenco, libero);

  let chiave = "";

  function imposta(el: HTMLElement, testo: string): void {
    if (el.textContent !== testo) el.textContent = testo;
  }

  function nascondi(): void {
    contenitore.classList.remove("visibile");
  }

  function riga(v: VoceErrore): HTMLLIElement {
    const li = doc.createElement("li");
    li.className = "errore-voce";
    li.dataset.gravita = v.gravita;
    const cliccabile = v.intervallo !== null;
    const corpo = doc.createElement(cliccabile ? "button" : "div");
    corpo.className = "errore-corpo";
    if (corpo instanceof HTMLButtonElement) {
      corpo.type = "button";
      corpo.title = "Vai alla posizione nell'editor";
    }
    const g = doc.createElement("span");
    g.className = "errore-gravita";
    g.textContent = v.etichetta;
    const m = doc.createElement("span");
    m.className = "errore-messaggio";
    m.textContent = v.messaggio;
    corpo.append(g, " ", m); // lo spazio tiene separate le parole per i lettori di schermo
    if (v.posizione !== null) {
      const p = doc.createElement("span");
      p.className = "errore-posizione";
      p.textContent = v.posizione + (v.puntoDiChiamata ? " (punto di chiamata)" : "");
      corpo.append(p);
    }
    for (const d of v.dettagli) {
      const e = doc.createElement("span");
      e.className = "errore-dettaglio";
      e.textContent = d;
      corpo.append(e);
    }
    if (cliccabile && v.intervallo !== null) {
      const intervallo = v.intervallo;
      // `button` risponde già a Invio e Spazio con un click.
      corpo.addEventListener("click", () => vaiA(intervallo));
    }
    li.append(corpo);
    return li;
  }

  return {
    mostra(diagnostici, testo, markdown) {
      if (diagnostici.length === 0) {
        this.pulisci();
        return;
      }
      const voci = ordinaDiagnostici(diagnostici).map((d) => descriviDiagnostico(d, testo, markdown));
      const { errori, avvisi } = contaDiagnostici(diagnostici);
      const riepilogo = riepilogoDiagnostici(errori, avvisi);
      const nota = markdown ? "Diagnostica (dal Typst generato dalla conversione): " : "";
      const nuova = JSON.stringify([riepilogo, nota, voci]);
      libero.textContent = "";
      if (nuova !== chiave) {
        chiave = nuova;
        elenco.replaceChildren(...voci.map(riga));
      }
      imposta(intestazione, nota + riepilogo);
      // Con una sola voce l'intestazione resta per i lettori di schermo ma non si vede.
      intestazione.classList.toggle("visually-hidden", voci.length === 1 && !markdown);
      contenitore.classList.add("visibile");
    },
    mostraTesto(testo) {
      chiave = "";
      elenco.replaceChildren();
      imposta(intestazione, "");
      libero.textContent = testo;
      contenitore.classList.add("visibile");
    },
    pulisci() {
      chiave = "";
      elenco.replaceChildren();
      imposta(intestazione, "");
      libero.textContent = "";
      nascondi();
    },
  };
}
