/**
 * pagina-vista.ts — Vista «Pagina» del pannello Strumenti: formato, orientamento,
 * colonne, margini, intestazione e numerazione, con un foglio in scala. Legge la
 * regola `#set page(...)` dal documento all'apertura (e dopo «Applica» o
 * «Rileggi») e la riscrive con `scriviPagina`; la logica sta in `pagina-logica.ts`.
 *
 * I controlli aggiornano lo stato e il foglio senza ridisegnare la vista, così il
 * fuoco resta dov'è. Il foglio si dimensiona con `element.style` (consentito dalla
 * CSP di produzione, come il posizionamento dei popover).
 */

import { campoForm, el, pulsante } from "./dom";
import {
  ETICHETTE_FORMATI,
  FORMATI,
  geometriaFoglio,
  leggiPaginaDaTesto,
  normalizzaLunghezza,
  scriviPagina,
  type StatoPagina,
} from "./pagina-logica";
import type { Cambio } from "./strumenti-logica";

/** Colonne selezionabili. */
const COLONNE = [1, 2, 3, 4];

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaPagina {
  testoEditor: () => string;
  sostituisci: (cambi: readonly Cambio[], selezione: { da: number; a: number }) => void;
  onAvviso: (messaggio: string) => void;
  /** Il pulsante «‹ Strumenti» già costruito. */
  indietro: () => HTMLElement;
  /** Chiede il ridisegno della vista (dopo una rilettura del documento). */
  ridisegna: () => void;
}

/** La vista: si rilegge dal documento con `carica`, produce i suoi elementi con `elementi`. */
export interface VistaPagina {
  carica(): void;
  elementi(): HTMLElement[];
}

/** Crea la vista Pagina. */
export function creaVistaPagina(opz: OpzioniVistaPagina): VistaPagina {
  let stato: StatoPagina = leggiPaginaDaTesto("");
  /** Testo digitato nei margini (può essere non valido, mentre `stato` tiene l'ultimo valido). */
  let testoY = "";
  let testoX = "";

  function carica(): void {
    stato = leggiPaginaDaTesto(opz.testoEditor());
    testoY = stato.margineY;
    testoX = stato.margineX;
  }

  const margineValido = (t: string): boolean => normalizzaLunghezza(t) !== null;

  function applica(): void {
    const y = normalizzaLunghezza(testoY);
    const x = normalizzaLunghezza(testoX);
    if (y === null || x === null) {
      opz.onAvviso("Margine non valido: usa un numero e un'unità (cm, mm, pt, in)");
      return;
    }
    const esito = scriviPagina(opz.testoEditor(), { ...stato, margineY: y, margineX: x });
    if (esito === null) {
      opz.onAvviso("Niente da applicare: il documento usa già le impostazioni predefinite");
      return;
    }
    opz.sostituisci([esito.cambio], esito.selezione);
    opz.onAvviso("Impostazioni di pagina applicate");
    carica();
    opz.ridisegna();
  }

  function elementi(): HTMLElement[] {
    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Pagina")];

    // Foglio in scala
    const foglio = el("div", "str-foglio");
    foglio.setAttribute("role", "img");
    const area = el("div", "str-foglio-area");
    const testata = el("span", "str-foglio-testata");
    const piede = el("span", "str-foglio-piede");
    foglio.append(testata, area, piede);
    const aggiornaFoglio = (): void => {
      const g = geometriaFoglio({
        ...stato,
        margineY: normalizzaLunghezza(testoY) ?? "",
        margineX: normalizzaLunghezza(testoX) ?? "",
      });
      foglio.style.aspectRatio = String(g.rapporto);
      area.style.inset = `${g.alto}% ${g.destra}% ${g.basso}% ${g.sinistra}%`;
      testata.textContent = stato.intestazione;
      piede.textContent = stato.numerazione || stato.numerazioneAvanzata ? "1" : "";
      foglio.setAttribute(
        "aria-label",
        `Anteprima della pagina ${ETICHETTE_FORMATI[stato.formato] ?? stato.formato}, ${stato.orizzontale ? "orizzontale" : "verticale"}`,
      );
    };

    // Formato e colonne
    const formato = el("select", "str-input");
    for (const f of Object.keys(FORMATI)) {
      const o = el("option", undefined, ETICHETTE_FORMATI[f] ?? f);
      o.value = f;
      o.selected = f === stato.formato;
      formato.append(o);
    }
    formato.addEventListener("change", () => {
      stato.formato = formato.value;
      aggiornaFoglio();
    });
    const colonne = el("select", "str-input");
    for (const n of COLONNE) {
      const o = el("option", undefined, String(n));
      o.value = String(n);
      o.selected = n === stato.colonne;
      colonne.append(o);
    }
    if (!COLONNE.includes(stato.colonne)) {
      const o = el("option", undefined, String(stato.colonne));
      o.value = String(stato.colonne);
      o.selected = true;
      colonne.append(o);
    }
    colonne.addEventListener("change", () => {
      stato.colonne = Number(colonne.value);
    });
    const due = el("div", "str-due");
    due.append(campoForm("Formato", formato, "str-pag-formato"), campoForm("Colonne", colonne, "str-pag-colonne"));
    fuori.push(due);

    // Orientamento
    const gruppo = el("div", "str-seg");
    gruppo.setAttribute("role", "radiogroup");
    gruppo.setAttribute("aria-label", "Orientamento");
    const bottoni: [boolean, HTMLButtonElement][] = [
      [false, pulsante("Verticale", "str-seg-voce", () => imposta(false))],
      [true, pulsante("Orizzontale", "str-seg-voce", () => imposta(true))],
    ];
    const sincronizzaOrientamento = (): void => {
      for (const [valore, b] of bottoni) {
        b.setAttribute("role", "radio");
        b.setAttribute("aria-checked", String(valore === stato.orizzontale));
      }
    };
    const imposta = (orizzontale: boolean): void => {
      stato.orizzontale = orizzontale;
      sincronizzaOrientamento();
      aggiornaFoglio();
    };
    gruppo.append(...bottoni.map(([, b]) => b));
    sincronizzaOrientamento();
    fuori.push(gruppo, foglio);

    // Margini
    if (stato.margineAvanzato) {
      fuori.push(
        el("p", "str-descrizione", "I margini di questo documento usano una forma avanzata (es. top/left): non si modificano da qui."),
      );
    } else {
      const campoMargine = (etichetta: string, id: string, valore: string, scrivi: (t: string) => void): HTMLElement => {
        const i = el("input", "str-input");
        i.type = "text";
        i.placeholder = "2,5 cm";
        i.value = valore;
        i.addEventListener("input", () => {
          scrivi(i.value);
          const ok = margineValido(i.value);
          i.setAttribute("aria-invalid", String(!ok));
          aggiornaFoglio();
        });
        return campoForm(etichetta, i, id);
      };
      const m = el("div", "str-due");
      m.append(
        campoMargine("Alto/basso", "str-pag-my", testoY, (t) => (testoY = t)),
        campoMargine("Sinistra/destra", "str-pag-mx", testoX, (t) => (testoX = t)),
      );
      fuori.push(m);
    }

    // Intestazione
    if (stato.intestazioneAvanzata) {
      fuori.push(el("p", "str-descrizione", "L'intestazione usa contenuto avanzato: non si modifica da qui."));
    } else {
      const i = el("input", "str-input");
      i.type = "text";
      i.value = stato.intestazione;
      i.addEventListener("input", () => {
        stato.intestazione = i.value;
        aggiornaFoglio();
      });
      fuori.push(campoForm("Intestazione", i, "str-pag-intestazione"));
    }

    // Numerazione
    if (stato.numerazioneAvanzata) {
      fuori.push(el("p", "str-descrizione", "La numerazione usa un formato avanzato: non si modifica da qui."));
    } else {
      const c = el("input");
      c.type = "checkbox";
      c.checked = stato.numerazione;
      c.addEventListener("change", () => {
        stato.numerazione = c.checked;
        aggiornaFoglio();
      });
      const riga = el("label", "str-casella");
      riga.append(c, el("span", undefined, "Numero di pagina"));
      fuori.push(riga);
    }

    aggiornaFoglio();
    fuori.push(
      pulsante("Applica al documento", "str-primario", applica),
      pulsante("Rileggi dal documento", "str-link", () => {
        carica();
        opz.ridisegna();
      }),
    );
    return fuori;
  }

  return { carica, elementi };
}
