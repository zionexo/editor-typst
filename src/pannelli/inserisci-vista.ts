/**
 * inserisci-vista.ts — Vista «Inserisci» del pannello Strumenti: tabella, figura,
 * equazione, riferimento a un'etichetta e nota a piè di pagina. Ogni modulo
 * costruisce il codice con `inserisci-logica.ts` e lo inserisce nell'editor alla
 * posizione del cursore. I valori dei campi restano fra un ridisegno e l'altro.
 */

import { campoForm, el, pulsante } from "./dom";
import {
  COLONNE_MAX,
  RIGHE_MAX,
  etichetteNelTesto,
  generaEquazione,
  esitoFigura,
  generaNota,
  generaRiferimento,
  generaTabella,
} from "./inserisci-logica";

type Tipo = "tabella" | "figura" | "equazione" | "riferimento" | "nota";

const TIPI: readonly { id: Tipo; etichetta: string }[] = [
  { id: "tabella", etichetta: "Tabella" },
  { id: "figura", etichetta: "Figura" },
  { id: "equazione", etichetta: "Equazione" },
  { id: "riferimento", etichetta: "Riferimento a un'etichetta" },
  { id: "nota", etichetta: "Nota a piè di pagina" },
];

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaInserisci {
  testoEditor: () => string;
  /** Inserisce il testo alla posizione del cursore (sostituendo la selezione). */
  inserisci: (testo: string) => void;
  onAvviso: (messaggio: string) => void;
  /** Cartella del documento aperto (per rendere relativi i percorsi delle immagini), `null` se non salvato. */
  cartellaDocumento?: () => string | null;
  /** Il pulsante «‹ Strumenti» già costruito. */
  indietro: () => HTMLElement;
  /** Chiede il ridisegno della vista (cambio di tipo). */
  ridisegna: () => void;
}

export interface VistaInserisci {
  elementi(): HTMLElement[];
}

/** Crea la vista Inserisci. */
export function creaVistaInserisci(opz: OpzioniVistaInserisci): VistaInserisci {
  let tipo: Tipo = "tabella";
  const tabella = { righe: 3, colonne: 3, intestazione: true };
  const figura = { percorso: "", didascalia: "", etichetta: "" };
  const equazione = { testo: "", visualizzata: true };
  let riferimento = "";
  let erroreFigura = "";
  let nota = "";

  const testo = (valore: string, id: string, etichetta: string, scrivi: (v: string) => void, segnaposto = ""): HTMLElement => {
    const i = el("input", "str-input");
    i.type = "text";
    i.value = valore;
    i.placeholder = segnaposto;
    i.addEventListener("input", () => scrivi(i.value));
    return campoForm(etichetta, i, id);
  };
  const numero = (valore: number, id: string, etichetta: string, max: number, scrivi: (v: number) => void): HTMLElement => {
    const i = el("input", "str-input");
    i.type = "number";
    i.min = "1";
    i.max = String(max);
    i.value = String(valore);
    i.addEventListener("input", () => scrivi(Number(i.value)));
    return campoForm(etichetta, i, id);
  };
  const casella = (valore: boolean, etichetta: string, scrivi: (v: boolean) => void): HTMLElement => {
    const c = el("input");
    c.type = "checkbox";
    c.checked = valore;
    c.addEventListener("change", () => scrivi(c.checked));
    const riga = el("label", "str-casella");
    riga.append(c, el("span", undefined, etichetta));
    return riga;
  };

  function modulo(): { campi: HTMLElement[]; genera: () => string | null; errore: string } {
    switch (tipo) {
      case "tabella": {
        const due = el("div", "str-due");
        due.append(
          numero(tabella.righe, "str-ins-righe", "Righe", RIGHE_MAX, (v) => (tabella.righe = v)),
          numero(tabella.colonne, "str-ins-colonne", "Colonne", COLONNE_MAX, (v) => (tabella.colonne = v)),
        );
        return {
          campi: [due, casella(tabella.intestazione, "Riga di intestazione", (v) => (tabella.intestazione = v))],
          genera: () => generaTabella(tabella.righe, tabella.colonne, tabella.intestazione),
          errore: `Righe da 1 a ${RIGHE_MAX}, colonne da 1 a ${COLONNE_MAX}`,
        };
      }
      case "figura":
        return {
          campi: [
            testo(figura.percorso, "str-ins-percorso", "Immagine (percorso)", (v) => (figura.percorso = v), "img/grafico.png"),
            testo(figura.didascalia, "str-ins-didascalia", "Didascalia", (v) => (figura.didascalia = v)),
            testo(figura.etichetta, "str-ins-etichetta", "Etichetta", (v) => (figura.etichetta = v), "fig:grafico"),
          ],
          genera: () => {
            const r = esitoFigura(figura.percorso, figura.didascalia, figura.etichetta, opz.cartellaDocumento?.() ?? null);
            erroreFigura = "errore" in r ? r.errore : "";
            return "codice" in r ? r.codice : null;
          },
          // Letto solo dopo `genera`, che ne scrive il motivo.
          get errore() {
            return erroreFigura;
          },
        };
      case "equazione":
        return {
          campi: [
            testo(equazione.testo, "str-ins-equazione", "Formula", (v) => (equazione.testo = v), "x^2 + y^2 = r^2"),
            casella(equazione.visualizzata, "Su una riga propria", (v) => (equazione.visualizzata = v)),
          ],
          genera: () => generaEquazione(equazione.testo, equazione.visualizzata),
          errore: "La formula non può contenere $ né andare a capo",
        };
      case "riferimento": {
        const etichette = etichetteNelTesto(opz.testoEditor());
        if (riferimento === "" || !etichette.includes(riferimento)) {
          riferimento = etichette[0] ?? "";
        }
        const s = el("select", "str-input");
        for (const e of etichette) {
          const o = el("option", undefined, e);
          o.value = e;
          o.selected = e === riferimento;
          s.append(o);
        }
        s.addEventListener("change", () => (riferimento = s.value));
        return {
          campi:
            etichette.length === 0
              ? [el("p", "str-descrizione", "Nel documento non ci sono etichette (<nome>) a cui riferirsi.")]
              : [
                  campoForm("Etichetta", s, "str-ins-rif"),
                  el(
                    "p",
                    "str-descrizione",
                    'Titoli ed equazioni si possono citare solo se numerati (#set heading(numbering: "1."), #set math.equation(numbering: "(1)")).',
                  ),
                ],
          genera: () => generaRiferimento(riferimento),
          errore: "Nessuna etichetta da inserire",
        };
      }
      case "nota":
        return {
          campi: [testo(nota, "str-ins-nota", "Testo della nota", (v) => (nota = v))],
          genera: () => generaNota(nota),
          errore: "Scrivi il testo della nota",
        };
    }
  }

  function elementi(): HTMLElement[] {
    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Inserisci")];
    const scelta = el("select", "str-input");
    for (const t of TIPI) {
      const o = el("option", undefined, t.etichetta);
      o.value = t.id;
      o.selected = t.id === tipo;
      scelta.append(o);
    }
    scelta.addEventListener("change", () => {
      tipo = scelta.value as Tipo;
      opz.ridisegna();
    });
    fuori.push(campoForm("Cosa inserire", scelta, "str-ins-tipo"));

    const m = modulo();
    fuori.push(...m.campi);
    fuori.push(
      pulsante("Inserisci nel documento", "str-primario", () => {
        const codice = m.genera();
        if (codice === null) {
          opz.onAvviso(m.errore);
          return;
        }
        opz.inserisci(codice);
      }),
      el(
        "p",
        "str-descrizione",
        "Il codice va alla posizione del cursore (sostituisce l'eventuale selezione) e vale nel testo normale, non dentro codice o formule.",
      ),
    );
    return fuori;
  }

  return { elementi };
}
