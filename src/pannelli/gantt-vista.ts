/**
 * gantt-vista.ts — Scheda «Gantt» del pannello Strumenti (pacchetto `gantt-newgen`, incorporato): si
 * compilano attività (con gruppo, date, completamento e dipendenza) e pietre miliari e si
 * inserisce il diagramma alla posizione del cursore, con l'import del pacchetto solo se
 * manca. La logica sta in `gantt-logica.ts`; i valori restano fra un ridisegno e l'altro.
 *
 * Il ridisegno avviene solo quando cambia il NUMERO di righe: scrivendo nei campi si
 * aggiornano sul posto le etichette dei menu «Dipende da» e l'elenco dei gruppi, così il
 * fuoco e i clic non si perdono.
 */

import { campoForm, el, pulsante } from "./dom";
import { PACCHETTI, pianoInserimento, type PianoInserimento } from "./equazioni-logica";
import {
  ANNO_MAX,
  ANNO_MIN,
  ATTIVITA_MAX,
  INTESTAZIONI,
  OPZIONI_PREDEFINITE,
  PIETRE_MAX,
  TIPI_DIPENDENZA,
  aggiungiGiorni,
  esempioGantt,
  generaGantt,
  rimuoviAttivita,
  type Attivita,
  type OpzioniGantt,
  type PietraMiliare,
  type TipoDipendenza,
} from "./gantt-logica";
import { isoOggi } from "./strumenti-logica";

/** Righe massime nella scheda (la logica ne ammette `ATTIVITA_MAX`): oltre, il pannello da 260 px diventa ingestibile. */
const ATTIVITA_SCHEDA_MAX = 40;

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaGantt {
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

export interface VistaGantt {
  elementi(): HTMLElement[];
}

/** Crea la vista Gantt. */
export function creaVistaGantt(opz: OpzioniVistaGantt): VistaGantt {
  /** Nuova attività: parte il giorno dopo la fine dell'ultima (o da oggi) e dura una settimana. */
  const nuovaAttivita = (elenco: readonly Attivita[]): Attivita => {
    const ultima = elenco[elenco.length - 1];
    const inizio = ultima !== undefined && ultima.fine !== "" ? aggiungiGiorni(ultima.fine, 1) : isoOggi();
    return { nome: "", gruppo: "", inizio, fine: aggiungiGiorni(inizio, 6), fatta: "", avanzamento: null, dipende: null, tipoDip: "fs" };
  };
  let attivita: Attivita[] = [nuovaAttivita([])];
  let pietre: PietraMiliare[] = [];
  const o: OpzioniGantt = { ...OPZIONI_PREDEFINITE };

  const testo = (valore: string, id: string, etichetta: string, scrivi: (v: string) => void, extra?: (i: HTMLInputElement) => void): HTMLElement => {
    const i = el("input", "str-input");
    i.type = "text";
    i.value = valore;
    i.addEventListener("input", () => scrivi(i.value));
    extra?.(i);
    return campoForm(etichetta, i, id);
  };
  const data = (valore: string, id: string, etichetta: string, scrivi: (v: string) => void): HTMLElement => {
    const i = el("input", "str-input");
    i.type = "date";
    i.min = `${ANNO_MIN}-01-01`;
    i.max = `${ANNO_MAX}-12-31`;
    i.value = valore;
    i.addEventListener("input", () => scrivi(i.value));
    return campoForm(etichetta, i, id);
  };
  const conId = (b: HTMLButtonElement, id: string, nomeAccessibile: string): HTMLButtonElement => {
    b.id = id;
    b.setAttribute("aria-label", nomeAccessibile);
    return b;
  };

  function elementi(): HTMLElement[] {
    // Riferimenti agli elementi che dipendono dai valori digitati: si aggiornano sul posto.
    const menuDipendenze: HTMLSelectElement[] = [];
    const elencoGruppi = el("datalist");
    elencoGruppi.id = "str-gt-gruppi";
    const aggiornaGruppi = (): void => {
      elencoGruppi.replaceChildren(
        ...[...new Set(attivita.map((x) => x.gruppo.trim()).filter((g) => g !== ""))].map((g) => {
          const op = el("option");
          op.value = g;
          return op;
        }),
      );
    };
    const etichettaDipendenza = (j: number): string => `${j + 1}. ${attivita[j].nome.trim() || "senza nome"}`;
    const aggiornaMenu = (): void => {
      for (const s of menuDipendenze) {
        for (const op of Array.from(s.options)) {
          if (op.value !== "") op.textContent = etichettaDipendenza(Number(op.value));
        }
      }
    };

    const rigaAttivita = (a: Attivita, i: number): HTMLElement => {
      const riga = el("div", "str-gt-riga");
      riga.setAttribute("role", "group");
      riga.setAttribute("aria-label", `Attività ${i + 1}`);
      const testata = el("div", "str-gt-testata");
      testata.append(
        el("b", undefined, `Attività ${i + 1}`),
        conId(
          pulsante("Rimuovi", "str-link str-pericolo", () => {
            attivita = rimuoviAttivita(attivita, i);
            // Il fuoco va alla riga che ha preso il posto di questa (o alla precedente, o a «+ Attività»).
            opz.ridisegna(attivita.length === 0 ? "#str-gt-piu-att" : `#str-gt-n${Math.min(i, attivita.length - 1)}`);
          }),
          `str-gt-rim${i}`,
          `Rimuovi l'attività ${i + 1}`,
        ),
      );
      riga.append(
        testata,
        testo(a.nome, `str-gt-n${i}`, "Nome", (v) => {
          a.nome = v;
          aggiornaMenu();
        }),
        testo(
          a.gruppo,
          `str-gt-g${i}`,
          "Gruppo (facoltativo)",
          (v) => {
            a.gruppo = v;
            aggiornaGruppi();
          },
          (inp) => inp.setAttribute("list", "str-gt-gruppi"),
        ),
      );
      const date = el("div", "str-due");
      date.append(
        data(a.inizio, `str-gt-i${i}`, "Inizio", (v) => (a.inizio = v)),
        data(a.fine, `str-gt-f${i}`, "Fine", (v) => (a.fine = v)),
      );
      const av = el("input", "str-input");
      av.type = "number";
      av.min = "0";
      av.max = "100";
      av.step = "1";
      av.value = a.avanzamento === null ? "" : String(a.avanzamento);
      av.addEventListener("input", () => {
        a.avanzamento = av.value === "" ? null : Number(av.value);
      });
      riga.append(
        date,
        data(a.fatta, `str-gt-c${i}`, "Completata il (facoltativo)", (v) => (a.fatta = v)),
        campoForm("Avanzamento % (facoltativo)", av, `str-gt-p${i}`),
      );
      if (attivita.length > 1) {
        const s = el("select", "str-input");
        const nessuna = el("option", undefined, "(nessuna)");
        nessuna.value = "";
        s.append(nessuna);
        attivita.forEach((_, j) => {
          if (j === i) return;
          const op = el("option", undefined, etichettaDipendenza(j));
          op.value = String(j);
          op.selected = a.dipende === j;
          s.append(op);
        });
        const tipo = el("select", "str-input");
        for (const t of TIPI_DIPENDENZA) {
          const op = el("option", undefined, t.etichetta);
          op.value = t.id;
          op.selected = a.tipoDip === t.id;
          tipo.append(op);
        }
        tipo.disabled = a.dipende === null;
        tipo.addEventListener("change", () => {
          a.tipoDip = tipo.value as TipoDipendenza;
        });
        s.addEventListener("change", () => {
          a.dipende = s.value === "" ? null : Number(s.value);
          tipo.disabled = a.dipende === null;
        });
        menuDipendenze.push(s);
        riga.append(campoForm("Dipende da", s, `str-gt-d${i}`), campoForm("Tipo di dipendenza", tipo, `str-gt-t${i}`));
      }
      return riga;
    };

    const rigaPietra = (p: PietraMiliare, i: number): HTMLElement => {
      const riga = el("div", "str-gt-riga");
      riga.setAttribute("role", "group");
      riga.setAttribute("aria-label", `Pietra miliare ${i + 1}`);
      const testata = el("div", "str-gt-testata");
      testata.append(
        el("b", undefined, `Pietra miliare ${i + 1}`),
        conId(
          pulsante("Rimuovi", "str-link str-pericolo", () => {
            pietre = pietre.filter((_, j) => j !== i);
            opz.ridisegna(pietre.length === 0 ? "#str-gt-piu-pietra" : `#str-gt-mn${Math.min(i, pietre.length - 1)}`);
          }),
          `str-gt-rimp${i}`,
          `Rimuovi la pietra miliare ${i + 1}`,
        ),
      );
      const c = el("input");
      c.type = "checkbox";
      c.checked = p.mostraData;
      c.addEventListener("change", () => (p.mostraData = c.checked));
      const mostra = el("label", "str-casella");
      mostra.append(c, el("span", undefined, "Mostra la data"));
      riga.append(
        testata,
        testo(p.nome, `str-gt-mn${i}`, "Nome", (v) => (p.nome = v)),
        data(p.data, `str-gt-md${i}`, "Data", (v) => (p.data = v)),
        mostra,
      );
      return riga;
    };

    const inserisci = (): void => {
      const esito = generaGantt(attivita, pietre, o);
      if ("errore" in esito) {
        opz.onAvviso(esito.errore);
        return;
      }
      const piano = pianoInserimento(opz.testoEditor(), opz.selezione(), esito.codice, ["gantt-newgen"]);
      opz.sostituisci(piano.cambi, piano.selezione);
      opz.onAvviso("Diagramma di Gantt inserito");
    };

    const caricaEsempio = async (): Promise<void> => {
      const vuota = attivita.length === 1 && attivita[0].nome.trim() === "" && pietre.length === 0;
      if (!vuota && !(await opz.conferma("Sostituire le attività e le pietre miliari della scheda con l'esempio?"))) {
        return;
      }
      const es = esempioGantt(isoOggi());
      attivita = es.attivita;
      pietre = es.pietre;
      opz.ridisegna("#str-gt-n0");
    };

    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Gantt")];
    fuori.push(
      el(
        "p",
        "str-descrizione",
        "Le attività con lo stesso gruppo diventano le sotto-attività di quel gruppo. L'intervallo del diagramma comprende attività, completamenti e pietre miliari. La data di fine è inclusa nella barra.",
      ),
      elencoGruppi,
    );
    aggiornaGruppi();

    fuori.push(el("h3", "str-titolo", "Attività"));
    attivita.forEach((a, i) => fuori.push(rigaAttivita(a, i)));
    const aggiungi = conId(
      pulsante("+ Attività", "str-secondario", () => {
        attivita.push(nuovaAttivita(attivita));
        opz.ridisegna(`#str-gt-n${attivita.length - 1}`);
      }),
      "str-gt-piu-att",
      "Aggiungi un'attività",
    );
    aggiungi.disabled = attivita.length >= Math.min(ATTIVITA_MAX, ATTIVITA_SCHEDA_MAX);
    fuori.push(aggiungi);

    fuori.push(el("h3", "str-titolo", "Pietre miliari"));
    pietre.forEach((p, i) => fuori.push(rigaPietra(p, i)));
    const aggiungiPietra = conId(
      pulsante("+ Pietra miliare", "str-secondario", () => {
        const ultima = attivita[attivita.length - 1];
        pietre.push({ nome: "", data: ultima?.fine || isoOggi(), mostraData: true });
        opz.ridisegna(`#str-gt-mn${pietre.length - 1}`);
      }),
      "str-gt-piu-pietra",
      "Aggiungi una pietra miliare",
    );
    aggiungiPietra.disabled = pietre.length >= PIETRE_MAX;
    fuori.push(aggiungiPietra);

    const intestazioni = el("select", "str-input");
    for (const h of INTESTAZIONI) {
      const op = el("option", undefined, h.etichetta);
      op.value = h.id;
      op.selected = o.intestazioni === h.id;
      intestazioni.append(op);
    }
    intestazioni.addEventListener("change", () => (o.intestazioni = intestazioni.value));
    const cOggi = el("input");
    cOggi.type = "checkbox";
    cOggi.checked = o.oggi;
    cOggi.addEventListener("change", () => (o.oggi = cOggi.checked));
    const oggi = el("label", "str-casella");
    oggi.append(cOggi, el("span", undefined, "Linea «oggi» (estende l'intervallo fino a oggi)"));

    fuori.push(
      el("h3", "str-titolo", "Diagramma"),
      campoForm("Intestazioni", intestazioni, "str-gt-intest"),
      oggi,
      data(o.da, "str-gt-da", "Mostra da (facoltativo)", (v) => (o.da = v)),
      data(o.a, "str-gt-a", "Mostra fino a (facoltativo)", (v) => (o.a = v)),
      testo(o.didascalia, "str-gt-didascalia", "Didascalia (crea una figura)", (v) => (o.didascalia = v)),
      testo(o.etichetta, "str-gt-etichetta", "Etichetta (per citarla)", (v) => (o.etichetta = v), (inp) => (inp.placeholder = "fig:piano")),
      conId(pulsante("Inserisci nel documento", "str-primario", inserisci), "str-gt-inserisci", "Inserisci il diagramma nel documento"),
      conId(pulsante("Carica un esempio", "str-link", () => void caricaEsempio()), "str-gt-esempio", "Carica un esempio nella scheda"),
      el(
        "p",
        "str-descrizione",
        `Usa il pacchetto gantt-newgen ${PACCHETTI["gantt-newgen"].versione} (incorporato in Typster, licenza LGPL-3.0: nessun download). Il diagramma va alla posizione del cursore.`,
      ),
    );
    return fuori;
  }

  return { elementi };
}
