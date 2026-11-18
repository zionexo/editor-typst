/**
 * grafici-vista.ts — Vista «Grafici» del pannello Strumenti (v1): si incolla una
 * tabella (prima colonna X, le altre serie), si sceglie linee, punti o barre e si
 * inserisce il grafico (`lilaq` o `cetz`) alla posizione del cursore. L'import del pacchetto
 * si aggiunge in cima al documento solo se manca. Logica in `grafici-logica.ts`.
 */

import { campoForm, el, pulsante } from "./dom";
import { PACCHETTI, pianoInserimento, type PianoInserimento } from "./equazioni-logica";
import {
  MOTORI_GRAFICO,
  TIPI_GRAFICO,
  analizzaTabella,
  generaGrafico,
  type MotoreGrafico,
  type OpzioniGrafico,
  type TipoGrafico,
} from "./grafici-logica";

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaGrafici {
  testoEditor: () => string;
  selezione: () => { da: number; a: number };
  sostituisci: (cambi: PianoInserimento["cambi"], selezione: { da: number; a: number }) => void;
  onAvviso: (messaggio: string) => void;
  /** Il pulsante «‹ Strumenti» già costruito. */
  indietro: () => HTMLElement;
  /** Chiede il ridisegno della vista. */
  ridisegna: () => void;
}

export interface VistaGrafici {
  elementi(): HTMLElement[];
}

/** Crea la vista Grafici. */
export function creaVistaGrafici(opz: OpzioniVistaGrafici): VistaGrafici {
  let tabella = "";
  const o: OpzioniGrafico = {
    motore: "lilaq",
    tipo: "linee",
    titolo: "",
    etichettaX: "",
    etichettaY: "",
    legenda: true,
    didascalia: "",
    etichetta: "",
  };

  function testo(valore: string, id: string, etichetta: string, scrivi: (v: string) => void, segnaposto = ""): HTMLElement {
    const i = el("input", "str-input");
    i.type = "text";
    i.value = valore;
    i.placeholder = segnaposto;
    i.addEventListener("input", () => scrivi(i.value));
    return campoForm(etichetta, i, id);
  }

  function inserisci(): void {
    const analisi = analizzaTabella(tabella);
    if ("errore" in analisi) {
      opz.onAvviso(analisi.errore);
      return;
    }
    const codice = generaGrafico(analisi.dati, o);
    if (codice === null) {
      opz.onAvviso("L'etichetta deve iniziare con una lettera (es. fig:andamento)");
      return;
    }
    const piano = pianoInserimento(opz.testoEditor(), opz.selezione(), codice, MOTORI_GRAFICO.find((m) => m.id === o.motore)?.pacchetti ?? ["lilaq"]);
    opz.sostituisci(piano.cambi, piano.selezione);
    opz.onAvviso("Grafico inserito");
  }

  function elementi(): HTMLElement[] {
    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Grafici")];

    // Dati
    const t = el("textarea", "str-input str-formula");
    t.rows = 6;
    t.value = tabella;
    t.placeholder = "tempo;temperatura\n0;20\n1;22\n2;21";
    t.setAttribute("spellcheck", "false");
    const stato = el("p", "str-descrizione");
    const aggiornaStato = (): void => {
      if (tabella.trim() === "") {
        stato.textContent = "La prima colonna è X, le altre sono le serie. Separatori: tab, ; o spazi.";
        return;
      }
      const r = analizzaTabella(tabella);
      stato.textContent =
        "errore" in r
          ? r.errore
          : `${r.dati.x.length} righe, ${r.dati.serie.length} serie`;
    };
    t.addEventListener("input", () => {
      tabella = t.value;
      aggiornaStato();
    });
    aggiornaStato();
    fuori.push(campoForm("Dati (incolla una tabella)", t, "str-gr-tabella"), stato);

    // Libreria: lilaq per i grafici di dati standard, CeTZ per quelli da ritoccare a mano
    const motore = el("select", "str-input");
    for (const m of MOTORI_GRAFICO) {
      const op = el("option", undefined, m.etichetta);
      op.value = m.id;
      op.selected = m.id === o.motore;
      motore.append(op);
    }
    const nota = el("p", "str-descrizione");
    const aggiornaNota = (): void => {
      const m = MOTORI_GRAFICO.find((x) => x.id === o.motore);
      const versioni = (m?.pacchetti ?? []).map((p) => `${p} ${PACCHETTI[p].versione}`).join(" + ");
      nota.textContent = `${m?.descrizione ?? ""} Pacchetti: ${versioni} (si scaricano alla prima compilazione; serve la rete).`;
    };
    motore.addEventListener("change", () => {
      o.motore = motore.value as MotoreGrafico;
      aggiornaNota();
    });
    aggiornaNota();
    fuori.push(campoForm("Libreria", motore, "str-gr-motore"), nota);

    // Tipo
    const tipo = el("select", "str-input");
    for (const g of TIPI_GRAFICO) {
      const op = el("option", undefined, g.etichetta);
      op.value = g.id;
      op.selected = g.id === o.tipo;
      tipo.append(op);
    }
    tipo.addEventListener("change", () => {
      o.tipo = tipo.value as TipoGrafico;
    });
    fuori.push(campoForm("Tipo di grafico", tipo, "str-gr-tipo"));

    // Testi
    fuori.push(
      testo(o.titolo, "str-gr-titolo", "Titolo", (v) => (o.titolo = v)),
      testo(o.etichettaX, "str-gr-x", "Etichetta asse X", (v) => (o.etichettaX = v), "Tempo (s)"),
      testo(o.etichettaY, "str-gr-y", "Etichetta asse Y", (v) => (o.etichettaY = v), "Temperatura (°C)"),
    );
    const c = el("input");
    c.type = "checkbox";
    c.checked = o.legenda;
    c.addEventListener("change", () => {
      o.legenda = c.checked;
    });
    const riga = el("label", "str-casella");
    riga.append(c, el("span", undefined, "Legenda con i nomi delle serie"));
    fuori.push(
      riga,
      testo(o.didascalia, "str-gr-didascalia", "Didascalia (crea una figura)", (v) => (o.didascalia = v)),
      testo(o.etichetta, "str-gr-etichetta", "Etichetta (per citarla)", (v) => (o.etichetta = v), "fig:andamento"),
      pulsante("Inserisci nel documento", "str-primario", inserisci),
      el("p", "str-descrizione", "Il grafico va alla posizione del cursore."),
    );
    return fuori;
  }

  return { elementi };
}
