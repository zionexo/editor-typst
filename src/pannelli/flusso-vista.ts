/**
 * flusso-vista.ts — Scheda «Diagramma di flusso» del pannello Strumenti (pacchetto `diagraph`,
 * Graphviz): si compilano nodi e archi e si inserisce il diagramma alla posizione del cursore, con
 * l'import del pacchetto solo se manca. La logica sta in `flusso-logica.ts`; i valori restano fra
 * un ridisegno e l'altro.
 *
 * Il ridisegno avviene solo quando cambia il NUMERO di righe: scrivendo nei campi si aggiornano sul
 * posto le etichette dei menu «Da» e «A» e gli elenchi di riquadri e livelli, così il fuoco e i
 * clic non si perdono.
 */

import { campoForm, el, pulsante } from "./dom";
import { PACCHETTI, pianoInserimento, type PianoInserimento } from "./equazioni-logica";
import {
  ARCHI_MAX,
  COLORI,
  DIREZIONI,
  FORME,
  NODI_MAX,
  OPZIONI_PREDEFINITE,
  esempioFlusso,
  generaFlusso,
  rimuoviNodo,
  type Arco,
  type Direzione,
  type Forma,
  type Nodo,
  type OpzioniFlusso,
} from "./flusso-logica";

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaFlusso {
  testoEditor: () => string;
  selezione: () => { da: number; a: number };
  sostituisci: (cambi: PianoInserimento["cambi"], selezione: { da: number; a: number }) => void;
  onAvviso: (messaggio: string) => void;
  /** Chiede conferma all'utente (finestra di dialogo). */
  conferma: (messaggio: string) => Promise<boolean>;
  /** Il pulsante «‹ Strumenti» già costruito. */
  indietro: () => HTMLElement;
  /** Chiede il ridisegno della vista; `fuocoSu` è un selettore CSS dell'elemento che deve avere il fuoco. */
  ridisegna: (fuocoSu?: string) => void;
}

export interface VistaFlusso {
  elementi(): HTMLElement[];
}

const nuovoNodo = (): Nodo => ({ testo: "", forma: "processo", colore: "", riquadro: "", livello: "" });

/** Crea la vista «Diagramma di flusso». */
export function creaVistaFlusso(opz: OpzioniVistaFlusso): VistaFlusso {
  let nodi: Nodo[] = [nuovoNodo()];
  let archi: Arco[] = [];
  const o: OpzioniFlusso = { ...OPZIONI_PREDEFINITE };

  const testo = (valore: string, id: string, etichetta: string, scrivi: (v: string) => void, extra?: (i: HTMLInputElement) => void): HTMLElement => {
    const i = el("input", "str-input");
    i.type = "text";
    i.value = valore;
    i.addEventListener("input", () => scrivi(i.value));
    extra?.(i);
    return campoForm(etichetta, i, id);
  };
  const menu = <T extends string>(valore: T, scelte: readonly { id: T; etichetta: string }[], id: string, etichetta: string, scrivi: (v: T) => void): HTMLElement => {
    const s = el("select", "str-input");
    for (const c of scelte) {
      const op = el("option", undefined, c.etichetta);
      op.value = c.id;
      op.selected = valore === c.id;
      s.append(op);
    }
    s.addEventListener("change", () => scrivi(s.value as T));
    return campoForm(etichetta, s, id);
  };
  /** Dà un id al pulsante; il nome accessibile serve solo dove il testo visibile non basta (es. «Rimuovi»). */
  const conId = (b: HTMLButtonElement, id: string, nomeAccessibile?: string): HTMLButtonElement => {
    b.id = id;
    if (nomeAccessibile !== undefined) {
      b.setAttribute("aria-label", nomeAccessibile);
    }
    return b;
  };

  function elementi(): HTMLElement[] {
    // Riferimenti agli elementi che dipendono dai valori digitati: si aggiornano sul posto.
    const menuNodi: HTMLSelectElement[] = [];
    const elencoRiquadri = el("datalist");
    elencoRiquadri.id = "str-fl-riquadri";
    const elencoLivelli = el("datalist");
    elencoLivelli.id = "str-fl-livelli";
    const valoriDistinti = (campo: "riquadro" | "livello"): HTMLOptionElement[] =>
      [...new Set(nodi.map((n) => n[campo].trim()).filter((v) => v !== ""))].map((v) => {
        const op = el("option");
        op.value = v;
        return op;
      });
    const aggiornaElenchi = (): void => {
      elencoRiquadri.replaceChildren(...valoriDistinti("riquadro"));
      elencoLivelli.replaceChildren(...valoriDistinti("livello"));
    };
    const etichettaNodo = (j: number): string => `${j + 1}. ${nodi[j].testo.trim() || "senza testo"}`;
    /** Aggiorna, in ogni menu «Da»/«A», la sola opzione del nodo `i` (a ogni tasto: niente riscritture inutili). */
    const aggiornaMenu = (i: number): void => {
      for (const s of menuNodi) {
        s.options[i].textContent = etichettaNodo(i);
      }
    };

    const rigaNodo = (n: Nodo, i: number): HTMLElement => {
      const riga = el("div", "str-gt-riga");
      riga.setAttribute("role", "group");
      riga.setAttribute("aria-label", `Nodo ${i + 1}`);
      const testata = el("div", "str-gt-testata");
      testata.append(
        el("b", undefined, `Nodo ${i + 1}`),
        conId(
          pulsante("Rimuovi", "str-link str-pericolo", () => {
            ({ nodi, archi } = rimuoviNodo(nodi, archi, i));
            // Il fuoco va al nodo che ha preso il posto di questo (o al precedente, o a «+ Nodo»).
            opz.ridisegna(nodi.length === 0 ? "#str-fl-piu-nodo" : `#str-fl-t${Math.min(i, nodi.length - 1)}`);
          }),
          `str-fl-rim${i}`,
          `Rimuovi il nodo ${i + 1} e i suoi archi`,
        ),
      );
      const colori = [{ id: "", etichetta: "(nessuno)" }, ...COLORI];
      riga.append(
        testata,
        testo(n.testo, `str-fl-t${i}`, "Testo", (v) => {
          n.testo = v;
          aggiornaMenu(i);
        }),
        menu<Forma>(n.forma, FORME, `str-fl-f${i}`, "Forma", (v) => (n.forma = v)),
        menu(n.colore, colori, `str-fl-c${i}`, "Colore", (v) => (n.colore = v)),
        testo(
          n.riquadro,
          `str-fl-r${i}`,
          "Riquadro (facoltativo)",
          (v) => {
            n.riquadro = v;
            aggiornaElenchi();
          },
          (inp) => inp.setAttribute("list", "str-fl-riquadri"),
        ),
        testo(
          n.livello,
          `str-fl-l${i}`,
          "Stesso livello (facoltativo)",
          (v) => {
            n.livello = v;
            aggiornaElenchi();
          },
          (inp) => inp.setAttribute("list", "str-fl-livelli"),
        ),
      );
      return riga;
    };

    const menuEstremo = (valore: number, id: string, etichetta: string, scrivi: (k: number) => void): HTMLElement => {
      const s = el("select", "str-input");
      nodi.forEach((_, j) => {
        const op = el("option", undefined, etichettaNodo(j));
        op.value = String(j);
        op.selected = valore === j;
        s.append(op);
      });
      s.addEventListener("change", () => scrivi(Number(s.value)));
      menuNodi.push(s);
      return campoForm(etichetta, s, id);
    };

    const rigaArco = (a: Arco, i: number): HTMLElement => {
      const riga = el("div", "str-gt-riga");
      riga.setAttribute("role", "group");
      riga.setAttribute("aria-label", `Arco ${i + 1}`);
      const testata = el("div", "str-gt-testata");
      testata.append(
        el("b", undefined, `Arco ${i + 1}`),
        conId(
          pulsante("Rimuovi", "str-link str-pericolo", () => {
            archi = archi.filter((_, j) => j !== i);
            opz.ridisegna(archi.length === 0 ? "#str-fl-piu-arco" : `#str-fl-ad${Math.min(i, archi.length - 1)}`);
          }),
          `str-fl-rima${i}`,
          `Rimuovi l'arco ${i + 1}`,
        ),
      );
      const tratteggio = el("input");
      tratteggio.type = "checkbox";
      tratteggio.checked = a.tratteggiato;
      tratteggio.addEventListener("change", () => (a.tratteggiato = tratteggio.checked));
      const casella = el("label", "str-casella");
      casella.append(tratteggio, el("span", undefined, "Tratteggiato"));
      const estremi = el("div", "str-due");
      estremi.append(
        menuEstremo(a.da, `str-fl-ad${i}`, "Da", (k) => (a.da = k)),
        menuEstremo(a.a, `str-fl-aa${i}`, "A", (k) => (a.a = k)),
      );
      riga.append(
        testata,
        estremi,
        testo(a.etichetta, `str-fl-ae${i}`, "Etichetta (facoltativa)", (v) => (a.etichetta = v), (inp) => (inp.placeholder = "Sì / No")),
        casella,
      );
      return riga;
    };

    const inserisci = (): void => {
      const esito = generaFlusso(nodi, archi, o);
      if ("errore" in esito) {
        opz.onAvviso(esito.errore);
        return;
      }
      const piano = pianoInserimento(opz.testoEditor(), opz.selezione(), esito.codice, ["diagraph"]);
      opz.sostituisci(piano.cambi, piano.selezione);
      opz.onAvviso("Diagramma di flusso inserito");
    };

    const caricaEsempio = async (): Promise<void> => {
      const vuota = nodi.length === 1 && nodi[0].testo.trim() === "" && archi.length === 0;
      if (!vuota && !(await opz.conferma("Sostituire i nodi e gli archi della scheda con l'esempio?"))) {
        return;
      }
      ({ nodi, archi } = esempioFlusso());
      opz.ridisegna("#str-fl-t0");
    };

    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Diagramma di flusso")];
    fuori.push(
      el(
        "p",
        "str-descrizione",
        "Disegna nodi e frecce: la disposizione la decide Graphviz, che evita per quanto può le intersezioni. Più nodi con lo stesso «livello» stanno sulla stessa riga (rami paralleli); lo stesso «riquadro» li racchiude in un riquadro con titolo. Un nodo non può avere entrambi.",
      ),
      elencoRiquadri,
      elencoLivelli,
    );
    aggiornaElenchi();

    fuori.push(el("h3", "str-titolo", "Nodi"));
    nodi.forEach((n, i) => fuori.push(rigaNodo(n, i)));
    const aggiungiNodo = conId(
      pulsante("+ Nodo", "str-secondario", () => {
        nodi.push(nuovoNodo());
        opz.ridisegna(`#str-fl-t${nodi.length - 1}`);
      }),
      "str-fl-piu-nodo",
    );
    aggiungiNodo.disabled = nodi.length >= NODI_MAX;
    fuori.push(aggiungiNodo);

    fuori.push(el("h3", "str-titolo", "Archi (frecce)"));
    archi.forEach((a, i) => fuori.push(rigaArco(a, i)));
    const aggiungiArco = conId(
      pulsante("+ Arco", "str-secondario", () => {
        const ultimo = nodi.length - 1;
        archi.push({ da: Math.max(0, ultimo - 1), a: ultimo, etichetta: "", tratteggiato: false });
        opz.ridisegna(`#str-fl-ad${archi.length - 1}`);
      }),
      "str-fl-piu-arco",
    );
    // Servono due nodi: con uno solo «+ Arco» creerebbe un arco da un nodo a se stesso che nessuno ha chiesto.
    aggiungiArco.disabled = nodi.length < 2 || archi.length >= ARCHI_MAX;
    fuori.push(aggiungiArco);

    const larghezza = el("input", "str-input");
    larghezza.type = "number";
    larghezza.min = "10";
    larghezza.max = "100";
    larghezza.step = "1";
    larghezza.placeholder = "naturale";
    larghezza.value = o.larghezza === null ? "" : String(o.larghezza);
    larghezza.addEventListener("input", () => {
      // Un testo non numerico (`12e`) dà `value === ""` ma `badInput`: NaN, così la validazione lo segnala.
      o.larghezza = larghezza.value === "" ? (larghezza.validity.badInput ? Number.NaN : null) : Number(larghezza.value);
    });

    fuori.push(
      el("h3", "str-titolo", "Diagramma"),
      menu<Direzione>(o.direzione, DIREZIONI, "str-fl-direzione", "Direzione", (v) => (o.direzione = v)),
      campoForm("Larghezza in % della pagina (vuota = naturale)", larghezza, "str-fl-larghezza"),
      conId(pulsante("Inserisci nel documento", "str-primario", inserisci), "str-fl-inserisci"),
      conId(pulsante("Carica un esempio", "str-link", () => void caricaEsempio()), "str-fl-esempio"),
      el(
        "p",
        "str-descrizione",
        `Usa il pacchetto diagraph ${PACCHETTI.diagraph.versione} di Typst Universe (si scarica la prima volta: serve internet). Il codice inserito è DOT di Graphviz e si può modificare a mano. Il diagramma non compare nelle esportazioni Markdown, HTML e LaTeX.`,
      ),
    );
    return fuori;
  }

  return { elementi };
}
