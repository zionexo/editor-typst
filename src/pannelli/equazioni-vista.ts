/**
 * equazioni-vista.ts — Vista «Equazioni» del pannello Strumenti: si compone una
 * formula aggiungendo strutture (frazione, matrice, derivata…) con un modulo, la
 * si inserisce nel documento, e si impostano numerazione e `equate`. I pacchetti
 * (physica, mitex, equate) si importano in cima al documento solo se mancano,
 * nella stessa transazione dell'inserimento. La logica sta in `equazioni-logica.ts`.
 */

import { campoForm, el, pulsante } from "./dom";
import {
  PACCHETTI,
  STRUTTURE,
  generaFormula,
  generaMitex,
  generaStruttura,
  leggiImpostazioni,
  pianoInserimento,
  scriviBlocco,
  strutturaPerId,
  valoriIniziali,
  type IdPacchetto,
  type ImpostazioniEquazioni,
  type PianoInserimento,
  type Struttura,
} from "./equazioni-logica";

/** Dipendenze della vista (le fornisce il pannello Strumenti). */
export interface OpzioniVistaEquazioni {
  testoEditor: () => string;
  selezione: () => { da: number; a: number };
  sostituisci: (cambi: PianoInserimento["cambi"], selezione: { da: number; a: number }) => void;
  onAvviso: (messaggio: string) => void;
  /** Il pulsante «‹ Strumenti» già costruito. */
  indietro: () => HTMLElement;
  /** Chiede il ridisegno della vista. */
  ridisegna: () => void;
}

export interface VistaEquazioni {
  /** Rilegge dal documento le impostazioni di numerazione. */
  carica(): void;
  elementi(): HTMLElement[];
}

/** Crea la vista Equazioni. */
export function creaVistaEquazioni(opz: OpzioniVistaEquazioni): VistaEquazioni {
  let strutturaId = STRUTTURE[0].id;
  let valori: Record<string, string> = valoriIniziali(STRUTTURE[0]);
  let formula = "";
  let visualizzata = true;
  let etichetta = "";
  let latex = "";
  let latexInLinea = false;
  const pacchettiUsati = new Set<IdPacchetto>();
  let imp: ImpostazioniEquazioni = { numerazione: false, equate: false };

  function carica(): void {
    imp = leggiImpostazioni(opz.testoEditor());
  }

  function applicaPiano(piano: PianoInserimento): void {
    opz.sostituisci(piano.cambi, piano.selezione);
  }

  function casella(valore: boolean, testo: string, scrivi: (v: boolean) => void): HTMLElement {
    const c = el("input");
    c.type = "checkbox";
    c.checked = valore;
    c.addEventListener("change", () => scrivi(c.checked));
    const riga = el("label", "str-casella");
    riga.append(c, el("span", undefined, testo));
    return riga;
  }

  function campoStruttura(s: Struttura): HTMLElement[] {
    return s.campi.map((c) => {
      const id = `str-eq-${s.id}-${c.id}`;
      let ctrl: HTMLInputElement | HTMLSelectElement;
      if (c.scelte !== undefined) {
        const sel = el("select", "str-input");
        for (const v of c.scelte) {
          const o = el("option", undefined, v);
          o.value = v;
          o.selected = v === valori[c.id];
          sel.append(o);
        }
        ctrl = sel;
      } else {
        const i = el("input", "str-input");
        i.type = c.intero !== undefined ? "number" : "text";
        if (c.intero !== undefined) {
          i.min = String(c.intero[0]);
          i.max = String(c.intero[1]);
        }
        i.value = valori[c.id] ?? "";
        ctrl = i;
      }
      ctrl.addEventListener("input", () => {
        valori[c.id] = ctrl.value;
      });
      return campoForm(c.etichetta, ctrl, id);
    });
  }

  function aggiungiAllaFormula(s: Struttura): void {
    const r = generaStruttura(s, valori);
    if ("errore" in r) {
      opz.onAvviso(r.errore);
      return;
    }
    formula = formula.trim() === "" ? r.codice : `${formula.trimEnd()} ${r.codice}`;
    if (s.pacchetto !== null) {
      pacchettiUsati.add(s.pacchetto);
    }
    opz.ridisegna();
  }

  function inserisciFormula(): void {
    const codice = generaFormula(formula, { visualizzata, etichetta });
    if (codice === null) {
      opz.onAvviso("Formula vuota o non valida: niente $, e l'etichetta deve iniziare con una lettera");
      return;
    }
    applicaPiano(pianoInserimento(opz.testoEditor(), opz.selezione(), codice, [...pacchettiUsati]));
    formula = "";
    etichetta = "";
    pacchettiUsati.clear();
    opz.onAvviso("Formula inserita");
    opz.ridisegna();
  }

  function inserisciLatex(): void {
    const codice = generaMitex(latex, latexInLinea);
    if (codice === null) {
      opz.onAvviso("Scrivi la formula LaTeX (senza il carattere ` )");
      return;
    }
    applicaPiano(pianoInserimento(opz.testoEditor(), opz.selezione(), codice, ["mitex"]));
    latex = "";
    opz.onAvviso("Formula LaTeX inserita");
    opz.ridisegna();
  }

  function applicaImpostazioni(): void {
    const piano = scriviBlocco(opz.testoEditor(), imp);
    if (piano === null) {
      opz.onAvviso("Niente da cambiare nel documento");
      return;
    }
    applicaPiano(piano);
    opz.onAvviso("Impostazioni delle equazioni applicate");
    carica();
    opz.ridisegna();
  }

  function elementi(): HTMLElement[] {
    const fuori: HTMLElement[] = [opz.indietro(), el("h3", "str-titolo", "Equazioni")];
    const s = strutturaPerId(strutturaId) ?? STRUTTURE[0];

    // 1. Struttura
    fuori.push(el("h3", "str-titolo", "Aggiungi una struttura"));
    const scelta = el("select", "str-input");
    const gruppoBase = el("optgroup");
    gruppoBase.label = "Base";
    const gruppoFisica = el("optgroup");
    gruppoFisica.label = `Fisica (pacchetto physica ${PACCHETTI.physica.versione})`;
    for (const st of STRUTTURE) {
      const o = el("option", undefined, st.etichetta);
      o.value = st.id;
      o.selected = st.id === s.id;
      (st.pacchetto === null ? gruppoBase : gruppoFisica).append(o);
    }
    scelta.append(gruppoBase, gruppoFisica);
    scelta.addEventListener("change", () => {
      strutturaId = scelta.value;
      const nuova = strutturaPerId(strutturaId);
      valori = nuova === undefined ? {} : valoriIniziali(nuova);
      opz.ridisegna();
    });
    fuori.push(campoForm("Struttura", scelta, "str-eq-struttura"), ...campoStruttura(s));
    if (s.pacchetto !== null) {
      fuori.push(
        el("p", "str-descrizione", `Usa il pacchetto ${s.pacchetto} (si scarica alla prima compilazione; serve la rete).`),
      );
    }
    fuori.push(pulsante("Aggiungi alla formula", "str-secondario", () => aggiungiAllaFormula(s)));

    // 2. Formula
    fuori.push(el("h3", "str-titolo", "Formula"));
    const f = el("textarea", "str-input str-formula");
    f.rows = 3;
    f.value = formula;
    f.placeholder = "Aggiungi strutture o scrivi la formula in Typst";
    f.setAttribute("spellcheck", "false");
    f.addEventListener("input", () => {
      formula = f.value;
    });
    fuori.push(campoForm("Formula (sintassi matematica di Typst)", f, "str-eq-formula"));
    fuori.push(casella(visualizzata, "Su una riga propria", (v) => (visualizzata = v)));
    const e = el("input", "str-input");
    e.type = "text";
    e.value = etichetta;
    e.placeholder = "eq:energia";
    e.addEventListener("input", () => {
      etichetta = e.value;
    });
    fuori.push(campoForm("Etichetta (per citarla)", e, "str-eq-etichetta"));
    if (pacchettiUsati.size > 0) {
      fuori.push(
        el("p", "str-descrizione", `Aggiungerà l'import di: ${[...pacchettiUsati].join(", ")} (se manca nel documento).`),
      );
    }
    fuori.push(pulsante("Inserisci nel documento", "str-primario", inserisciFormula));

    // 3. Da LaTeX
    const dettagli = el("details", "str-dettagli");
    dettagli.append(el("summary", undefined, `Incolla da LaTeX (mitex ${PACCHETTI.mitex.versione})`));
    const l = el("textarea", "str-input str-formula");
    l.rows = 3;
    l.value = latex;
    l.placeholder = "\\frac{a}{b} + \\sqrt{x}";
    l.setAttribute("spellcheck", "false");
    l.addEventListener("input", () => {
      latex = l.value;
    });
    dettagli.append(
      campoForm("Formula LaTeX", l, "str-eq-latex"),
      casella(latexInLinea, "Nel testo (non su una riga propria)", (v) => (latexInLinea = v)),
      pulsante("Inserisci formula LaTeX", "str-secondario", inserisciLatex),
    );
    fuori.push(dettagli);

    // 4. Numerazione
    fuori.push(el("h3", "str-titolo", "Numerazione delle equazioni"));
    fuori.push(
      casella(imp.numerazione || imp.equate, "Numera le equazioni (1)", (v) => {
        imp.numerazione = v;
        if (!v) imp.equate = false;
        opz.ridisegna();
      }),
      casella(imp.equate, `Numerazione per riga e formule su più pagine (equate ${PACCHETTI.equate.versione})`, (v) => {
        imp.equate = v;
        opz.ridisegna();
      }),
      pulsante("Applica al documento", "str-secondario", applicaImpostazioni),
    );
    return fuori;
  }

  return { carica, elementi };
}
