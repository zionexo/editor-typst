/**
 * strumenti.ts — Pannello «Strumenti» della barra laterale.
 *
 * Una lista di strumenti (Modelli, Pagina, Inserisci, Equazioni, Grafici e Gantt) e, scelto uno strumento, il suo pannello con «‹ Strumenti» per
 * tornare indietro. Vista **Modelli**: si sceglie un modello, si compila il
 * modulo e si crea un documento non salvato. Vista **Salva come modello**: si
 * trasformano in campi le parti del documento aperto che cambiano ogni volta.
 *
 * Il pannello non conosce Tauri né l'editor: `main.ts` gli passa le funzioni
 * (`OpzioniStrumenti`). Testi solo con `textContent`; nessuno stile inline
 * (CSP di produzione, B-005). Ogni ridisegno conserva il fuoco (e la posizione
 * del cursore nei campi) perché le risposte asincrone possono arrivare mentre
 * l'utente scrive.
 */

import type { Pannello } from "../barra-laterale";
import { campoForm, el, pulsante } from "./dom";
import { creaVistaEquazioni } from "./equazioni-vista";
import { creaVistaGantt } from "./gantt-vista";
import { creaVistaGrafici } from "./grafici-vista";
import { creaVistaInserisci } from "./inserisci-vista";
import { creaVistaPagina } from "./pagina-vista";
import {
  TIPI_CAMPO,
  abbrevia,
  campiNelSorgente,
  contaOccorrenze,
  convertiInCampo,
  eGiaCampo,
  filtraModelli,
  isoOggi,
  nomeCampoSuggerito,
  nomeCampoValido,
  riepilogoModello,
  sostituzioneMultiplaSensata,
  tipoInputData,
  valoriIniziali,
  valoriPerCore,
  type Cambio,
  type CampoModello,
  type ModelloInfo,
  type TipoCampo,
} from "./strumenti-logica";

/** Identificatore del pannello nella barra laterale. */
export const ID_PANNELLO_STRUMENTI = "strumenti";

/** Icona (viewBox 24, solo contorno): una scintilla con una piccola. */
const ICONA_STRUMENTI =
  "M11 3l1.9 5.1L18 10l-5.1 1.9L11 17l-1.9-5.1L4 10l5.1-1.9z M18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z";

/** Prefisso con cui il core segnala che un modello utente con quel nome esiste già. */
const PREFISSO_ESISTE_GIA = "Esiste già un modello";

/** Ritardo (ms) con cui la vista «Salva come modello» segue cursore e testo. */
const RITARDO_AGGIORNA_MS = 150;

/** Esito di `compila_modello`. */
export interface RisultatoModello {
  sorgente: string;
  avvisi: string[];
}

/** Dipendenze iniettate da `main.ts`. */
export interface OpzioniStrumenti {
  elenca: () => Promise<ModelloInfo[]>;
  compila: (id: string, valori: Record<string, string>) => Promise<RisultatoModello>;
  /** Salva il modello e restituisce l'id; con `sovrascrivi` falso rifiuta un modello utente già esistente. */
  salva: (
    nome: string,
    sorgente: string,
    campi: CampoModello[],
    descrizione: string,
    pagine: number | undefined,
    sovrascrivi: boolean,
  ) => Promise<string>;
  elimina: (id: string) => Promise<void>;
  /** Apre un documento nuovo, non salvato, con questo testo; `false` se non è stato creato. */
  nuovoDocumento: (testo: string) => Promise<boolean>;
  testoEditor: () => string;
  /** Inserisce testo alla posizione del cursore (sostituisce la selezione). */
  inserisci: (testo: string) => void;
  /** Pagine dell'ultima compilazione (0 se sconosciuto). */
  pagineDocumento: () => number;
  selezione: () => { da: number; a: number };
  /** Applica i cambi in una sola transazione e seleziona `selezione` nel testo risultante. */
  sostituisci: (cambi: readonly Cambio[], selezione: { da: number; a: number }) => void;
  /** Chiede conferma all'utente (finestra di dialogo). */
  conferma: (messaggio: string) => Promise<boolean>;
  onAvviso: (messaggio: string) => void;
  onErrore: (errore: unknown) => void;
  /** Cartella del documento aperto, `null` se non salvato. */
  cartellaDocumento?: () => string | null;
}

/** Il pannello, con i punti d'ingresso usati da `main.ts`. */
export interface PannelloStrumenti extends Pannello {
  /** Da chiamare quando cambiano testo o selezione dell'editor: la vista «Salva come modello» si riallinea. */
  aggiorna(): void;
  /** Mostra direttamente la vista Modelli (comando «Nuovo da modello…»). */
  apriModelli(): void;
}

type Vista = "lista" | "modelli" | "salva" | "pagina" | "inserisci" | "equazioni" | "grafici" | "gantt";

/** Messaggio di un errore qualsiasi (Tauri restituisce stringhe). */
function testoErrore(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** Crea il pannello Strumenti. */
export function creaPannelloStrumenti(opz: OpzioniStrumenti): PannelloStrumenti {
  const radice = el("div", "str-pannello");
  let vista: Vista = "lista";
  let modelli: ModelloInfo[] = [];
  let caricati = false;
  let ricerca = "";
  let scelto: string | null = null;
  let valori: Record<string, string> = {};
  /** Tipi e predefiniti dei campi del modello che si sta creando, per nome (si azzera solo dopo il salvataggio). */
  const nuoviCampi = new Map<string, { tipo: TipoCampo; predefinito: string }>();
  let nomeNuovo = "";
  let nomeCampoProposto = "";
  let tutteLeOccorrenze = true;
  /** Frammento per cui è stata decisa l'impostazione di «tutte le occorrenze». */
  let ultimoFrammento = "";
  let occupato = false;
  let contenitore: HTMLElement | null = null;
  let progressivo = 0;
  let timerAggiorna: ReturnType<typeof setTimeout> | null = null;

  const modelloScelto = (): ModelloInfo | undefined => modelli.find((m) => m.id === scelto);

  const equazioniVista = creaVistaEquazioni({
    testoEditor: opz.testoEditor,
    selezione: opz.selezione,
    sostituisci: opz.sostituisci,
    onAvviso: opz.onAvviso,
    indietro: () => indietro("Strumenti", "lista"),
    ridisegna: () => disegna(),
  });

  const ganttVista = creaVistaGantt({
    testoEditor: opz.testoEditor,
    selezione: opz.selezione,
    sostituisci: opz.sostituisci,
    onAvviso: opz.onAvviso,
    conferma: opz.conferma,
    indietro: () => indietro("Strumenti", "lista"),
    ridisegna: (fuocoSu) => disegna(fuocoSu),
  });

  const graficiVista = creaVistaGrafici({
    testoEditor: opz.testoEditor,
    selezione: opz.selezione,
    sostituisci: opz.sostituisci,
    onAvviso: opz.onAvviso,
    indietro: () => indietro("Strumenti", "lista"),
    ridisegna: () => disegna(),
  });

  const inserisciVista = creaVistaInserisci({
    testoEditor: opz.testoEditor,
    inserisci: opz.inserisci,
    onAvviso: opz.onAvviso,
    cartellaDocumento: opz.cartellaDocumento,
    indietro: () => indietro("Strumenti", "lista"),
    ridisegna: () => disegna(),
  });

  const paginaVista = creaVistaPagina({
    testoEditor: opz.testoEditor,
    sostituisci: opz.sostituisci,
    onAvviso: opz.onAvviso,
    indietro: () => indietro("Strumenti", "lista"),
    ridisegna: () => disegna(),
  });

  function vai(v: Vista): void {
    vista = v;
    disegna();
  }

  async function carica(): Promise<void> {
    const mio = ++progressivo;
    try {
      const m = await opz.elenca();
      if (mio !== progressivo) {
        return;
      }
      modelli = m;
      caricati = true;
      if (scelto === null || modelloScelto() === undefined) {
        scelto = modelli[0]?.id ?? null;
        valori = valoriIniziali(modelloScelto()?.campi ?? [], isoOggi());
      }
    } catch (e) {
      opz.onErrore(e);
    }
    disegna();
  }

  // ---- Vista: lista ----
  function vistaLista(): HTMLElement[] {
    const lista = el("div", "str-strumenti");
    const voce = (icona: string, nome: string, descr: string, azione: (() => void) | null): HTMLElement => {
      const b = el("button", "str-strumento");
      b.type = "button";
      if (azione === null) {
        b.disabled = true;
      }
      b.append(el("span", "str-icona", icona));
      const testo = el("span", "str-testo");
      testo.append(el("b", undefined, nome), el("small", undefined, descr));
      b.append(testo, el("span", "str-freccia", azione === null ? "" : "›"));
      if (azione !== null) {
        b.addEventListener("click", azione);
      }
      return b;
    };
    lista.append(
      voce("❐", "Modelli", "Nuovo documento da un modello", apriModelli),
      voce("▭", "Pagina", "Formato, margini, intestazione", () => {
        paginaVista.carica();
        vai("pagina");
      }),
      voce("+", "Inserisci", "Tabella, figura, equazione, riferimento, nota", () => vai("inserisci")),
      voce("↗", "Grafici", "Linee, punti e barre da una tabella", () => vai("grafici")),
      voce("▤", "Gantt", "Attività, dipendenze e pietre miliari", () => vai("gantt")),
      voce("∑", "Equazioni", "Strutture, pacchetti, numerazione", () => {
        equazioniVista.carica();
        vai("equazioni");
      }),
    );
    return [lista];
  }

  function apriModelli(): void {
    vai("modelli");
    void carica();
  }

  // ---- Vista: modelli ----
  function indietro(testo: string, verso: Vista): HTMLElement {
    return pulsante(`‹ ${testo}`, "str-indietro", () => vai(verso));
  }

  function vistaModelli(): HTMLElement[] {
    const fuori: HTMLElement[] = [indietro("Strumenti", "lista"), el("h3", "str-titolo", "Modelli")];
    const cerca = el("input", "str-ricerca");
    cerca.type = "search";
    cerca.placeholder = "Cerca un modello…";
    cerca.setAttribute("aria-label", "Cerca un modello");
    cerca.value = ricerca;
    cerca.addEventListener("input", () => {
      ricerca = cerca.value;
      disegna();
    });
    fuori.push(cerca);

    if (!caricati) {
      fuori.push(el("p", "pannello-vuoto", "Caricamento dei modelli…"));
      return fuori;
    }
    const visibili = filtraModelli(modelli, ricerca);
    if (visibili.length === 0) {
      fuori.push(
        el("p", "pannello-vuoto", modelli.length === 0 ? "Nessun modello disponibile." : "Nessun modello corrisponde."),
      );
    }
    const gruppo = el("div", "str-modelli");
    gruppo.setAttribute("role", "radiogroup");
    gruppo.setAttribute("aria-label", "Modelli disponibili");
    // Se il filtro nasconde il modello scelto, il primo visibile deve restare raggiungibile da tastiera.
    const sceltoVisibile = visibili.some((v) => v.id === scelto);
    visibili.forEach((m, i) => {
      const r = el("div", "str-modello");
      r.setAttribute("role", "radio");
      const attivo = m.id === scelto;
      r.setAttribute("aria-checked", String(attivo));
      r.tabIndex = attivo || (!sceltoVisibile && i === 0) ? 0 : -1;
      r.dataset.id = m.id;
      const testo = el("span", "str-testo");
      testo.append(
        el("b", undefined, m.nome + (m.origine === "utente" ? " · tuo" : "")),
        el("small", undefined, riepilogoModello(m)),
      );
      r.append(el("span", "str-mini"), testo);
      const scegli = (modello: ModelloInfo): void => {
        scelto = modello.id;
        valori = valoriIniziali(modello.campi, isoOggi());
        disegna(`[data-id="${CSS.escape(modello.id)}"]`);
      };
      r.addEventListener("click", () => scegli(m));
      r.addEventListener("keydown", (ev: KeyboardEvent) => {
        if (ev.key === "Enter" || ev.key === " ") {
          ev.preventDefault();
          scegli(m);
        } else if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
          // Come il radiogroup APG: la freccia sposta E seleziona.
          ev.preventDefault();
          const k = ev.key === "ArrowDown" ? 1 : -1;
          scegli(visibili[(i + k + visibili.length) % visibili.length]);
        }
      });
      gruppo.append(r);
    });
    fuori.push(gruppo);

    const m = modelloScelto();
    if (m !== undefined) {
      if (m.descrizione) {
        fuori.push(el("p", "str-descrizione", m.descrizione));
      }
      for (const avviso of m.avvisi ?? []) {
        fuori.push(el("p", "str-descrizione str-avviso", `⚠ ${avviso}`));
      }
      fuori.push(el("h3", "str-titolo", "Campi"));
      for (const c of m.campi) {
        fuori.push(campoModulo(c));
      }
      const crea = pulsante("Crea documento", "str-primario", () => void creaDocumento(m));
      crea.disabled = occupato;
      fuori.push(crea);
      if (m.origine === "utente") {
        fuori.push(pulsante("Elimina questo modello", "str-link str-pericolo", () => void eliminaModello(m)));
      }
    }
    fuori.push(pulsante("Salva il documento aperto come modello…", "str-link", () => vai("salva")));
    return fuori;
  }

  /** Controllo del modulo per un campo del modello scelto. */
  function campoModulo(c: CampoModello): HTMLElement {
    const id = `str-v-${c.nome}`;
    let ctrl: HTMLInputElement | HTMLTextAreaElement;
    if (c.tipo === "elenco") {
      const t = el("textarea", "str-input");
      t.rows = 4;
      t.placeholder = "Una voce per riga";
      ctrl = t;
    } else {
      const i = el("input", "str-input");
      const iniziale = valori[c.nome] ?? "";
      // Un predefinito non ISO in un campo data (es. «12 marzo 2026») non va perso: campo di testo.
      i.type = c.tipo === "data" ? tipoInputData(iniziale) : "text";
      if (c.tipo === "numero") {
        i.inputMode = "decimal";
      }
      ctrl = i;
    }
    ctrl.value = valori[c.nome] ?? "";
    ctrl.addEventListener("input", () => {
      valori[c.nome] = ctrl.value;
    });
    return campoForm(c.nome.replace(/_/g, " "), ctrl, id);
  }

  async function creaDocumento(m: ModelloInfo): Promise<void> {
    if (occupato) {
      return;
    }
    occupato = true;
    disegna();
    try {
      const r = await opz.compila(m.id, valoriPerCore(m.campi, valori));
      if (await opz.nuovoDocumento(r.sorgente)) {
        opz.onAvviso(
          r.avvisi.length > 0
            ? `Documento creato dal modello «${m.nome}» — ${r.avvisi.join(" · ")}`
            : `Documento creato dal modello «${m.nome}»`,
        );
      }
    } catch (e) {
      opz.onErrore(e);
    } finally {
      occupato = false;
      disegna();
    }
  }

  async function eliminaModello(m: ModelloInfo): Promise<void> {
    if (!(await opz.conferma(`Eliminare il modello «${m.nome}»? Il file viene cancellato.`))) {
      return;
    }
    try {
      await opz.elimina(m.id);
      scelto = null;
      await carica();
      opz.onAvviso(`Modello «${m.nome}» eliminato`);
    } catch (e) {
      opz.onErrore(e);
    }
  }

  // ---- Vista: salva come modello ----
  function vistaSalva(): HTMLElement[] {
    const testo = opz.testoEditor();
    const sel = opz.selezione();
    const frammento = sel.da < sel.a ? testo.slice(sel.da, sel.a) : "";
    const fuori: HTMLElement[] = [indietro("Modelli", "modelli"), el("h3", "str-titolo", "Nuovo modello")];

    const salvaBtn = pulsante("Salva tra i modelli", "str-primario", () => void salvaModello());
    const puoSalvare = (): boolean =>
      !occupato && nomeNuovo.trim() !== "" && campiNelSorgente(opz.testoEditor()).length > 0;
    salvaBtn.disabled = !puoSalvare();

    const nome = el("input", "str-input");
    nome.type = "text";
    nome.value = nomeNuovo;
    nome.addEventListener("input", () => {
      nomeNuovo = nome.value;
      salvaBtn.disabled = !puoSalvare();
    });
    fuori.push(campoForm("Nome del modello", nome, "str-nome-modello"));

    fuori.push(el("h3", "str-titolo", "Parti che cambiano"));
    if (frammento === "") {
      fuori.push(el("p", "str-descrizione", "Seleziona nell'editor il testo che cambia da un documento all'altro."));
    } else if (eGiaCampo(frammento)) {
      fuori.push(el("p", "str-descrizione", `«${abbrevia(frammento, 40)}» è già un campo del modello.`));
    } else {
      fuori.push(el("p", "str-descrizione", `Selezione: «${abbrevia(frammento, 40)}»`));
      if (frammento !== ultimoFrammento) {
        ultimoFrammento = frammento;
        nomeCampoProposto = nomeCampoSuggerito(frammento);
        tutteLeOccorrenze = sostituzioneMultiplaSensata(frammento);
      }
      const nc = el("input", "str-input");
      nc.type = "text";
      nc.value = nomeCampoProposto;
      nc.addEventListener("input", () => {
        nomeCampoProposto = nc.value.trim();
      });
      fuori.push(campoForm("Nome del campo", nc, "str-nome-campo"));
      const occorrenze = contaOccorrenze(testo, frammento);
      if (occorrenze > 1) {
        const casella = el("input");
        casella.type = "checkbox";
        casella.checked = tutteLeOccorrenze;
        casella.addEventListener("change", () => {
          tutteLeOccorrenze = casella.checked;
        });
        const riga = el("label", "str-casella");
        riga.append(casella, el("span", undefined, `Sostituisci tutte le occorrenze (${occorrenze})`));
        fuori.push(riga);
      }
      fuori.push(pulsante("Rendi la selezione un campo", "str-secondario", () => rendiCampo(frammento, nc)));
    }

    fuori.push(el("h3", "str-titolo", "Campi del modello"));
    const nomi = campiNelSorgente(testo);
    if (nomi.length === 0) {
      fuori.push(el("p", "str-descrizione", "Ancora nessun campo."));
    }
    const lista = el("div", "str-campi");
    for (const n of nomi) {
      const info = nuoviCampi.get(n) ?? { tipo: "testo" as TipoCampo, predefinito: "" };
      nuoviCampi.set(n, info);
      const riga = el("div", "str-riga-campo");
      riga.append(el("span", "str-chip", n));
      const sc = el("select", "str-tipo");
      sc.setAttribute("aria-label", `Tipo del campo ${n}`);
      for (const t of TIPI_CAMPO) {
        const o = el("option", undefined, t);
        o.value = t;
        o.selected = t === info.tipo;
        sc.append(o);
      }
      sc.addEventListener("change", () => {
        info.tipo = sc.value as TipoCampo;
        if (info.tipo === "data" && info.predefinito !== "" && !/^\d{4}-\d{2}-\d{2}$/.test(info.predefinito)) {
          info.predefinito = "oggi";
        }
      });
      riga.append(sc);
      lista.append(riga);
    }
    fuori.push(lista, salvaBtn);
    return fuori;
  }

  /** «Rendi la selezione un campo»: rilegge testo e selezione al momento del clic (non quelli del disegno). */
  function rendiCampo(frammentoMostrato: string, nc: HTMLInputElement): void {
    const testo = opz.testoEditor();
    const sel = opz.selezione();
    if (testo.slice(sel.da, sel.a) !== frammentoMostrato) {
      opz.onAvviso("La selezione è cambiata: selezionala di nuovo");
      disegna();
      return;
    }
    const nomeC = (nc.value || nomeCampoProposto).trim();
    if (!nomeCampoValido(nomeC)) {
      opz.onAvviso("Nome del campo non valido: minuscole, cifre e _, che inizia con una lettera");
      return;
    }
    const esito = convertiInCampo(testo, sel.da, sel.a, nomeC, tutteLeOccorrenze);
    if (esito === null) {
      opz.onAvviso("Questa selezione non può diventare un campo");
      return;
    }
    if (!nuoviCampi.has(nomeC)) {
      nuoviCampi.set(nomeC, { tipo: "testo", predefinito: frammentoMostrato });
    }
    opz.sostituisci(esito.cambi, esito.selezione);
    nomeCampoProposto = "";
    ultimoFrammento = "";
    disegna();
  }

  async function salvaModello(): Promise<void> {
    if (occupato) {
      return;
    }
    const testo = opz.testoEditor();
    const nomi = campiNelSorgente(testo);
    const campi: CampoModello[] = nomi.map((n) => {
      const i = nuoviCampi.get(n) ?? { tipo: "testo" as TipoCampo, predefinito: "" };
      return { nome: n, tipo: i.tipo, predefinito: i.predefinito === "" ? undefined : i.predefinito };
    });
    const nome = nomeNuovo.trim();
    const pagine = opz.pagineDocumento() > 0 ? opz.pagineDocumento() : undefined;
    occupato = true;
    disegna();
    try {
      let id: string;
      try {
        id = await opz.salva(nome, testo, campi, "", pagine, false);
      } catch (e) {
        if (!testoErrore(e).startsWith(PREFISSO_ESISTE_GIA)) {
          throw e;
        }
        if (!(await opz.conferma("Esiste già un tuo modello con questo nome. Sostituirlo con il documento aperto?"))) {
          return;
        }
        id = await opz.salva(nome, testo, campi, "", pagine, true);
      }
      opz.onAvviso(`Modello «${nome}» salvato`);
      scelto = id;
      nuoviCampi.clear();
      nomeNuovo = "";
      occupato = false;
      vai("modelli");
      await carica();
    } catch (e) {
      opz.onErrore(e);
    } finally {
      occupato = false;
      disegna();
    }
  }

  // ---- Disegno ----
  /** Selettore stabile dell'elemento del pannello che ha il fuoco, se ce l'ha. */
  function chiaveFuoco(): string | null {
    const a = document.activeElement;
    if (!(a instanceof HTMLElement) || !radice.contains(a)) {
      return null;
    }
    if (a.id !== "") {
      return `#${CSS.escape(a.id)}`;
    }
    if (a.dataset.id !== undefined) {
      return `[data-id="${CSS.escape(a.dataset.id)}"]`;
    }
    for (const c of ["str-ricerca", "str-indietro", "str-primario", "str-secondario"]) {
      if (a.classList.contains(c)) {
        return `.${c}`;
      }
    }
    return null;
  }

  /**
   * Ricostruisce la vista. Se il fuoco era nel pannello lo restituisce allo stesso
   * elemento (stesso `id`/`data-id`/classe) con la posizione del cursore; `fuocoSu`
   * forza un elemento diverso (es. il modello appena scelto).
   */
  function disegna(fuocoSu?: string): void {
    const chiave = fuocoSu ?? chiaveFuoco();
    const a = document.activeElement;
    const inTesto = a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement;
    const inizio = inTesto ? a.selectionStart : null;
    const fine = inTesto ? a.selectionEnd : null;
    const parti =
      vista === "lista"
        ? vistaLista()
        : vista === "modelli"
          ? vistaModelli()
          : vista === "pagina"
            ? paginaVista.elementi()
            : vista === "inserisci"
              ? inserisciVista.elementi()
              : vista === "equazioni"
                ? equazioniVista.elementi()
                : vista === "grafici"
                  ? graficiVista.elementi()
                  : vista === "gantt"
                    ? ganttVista.elementi()
                    : vistaSalva();
    radice.replaceChildren(...parti);
    if (chiave !== null) {
      const nuovo = radice.querySelector<HTMLElement>(chiave);
      if (nuovo !== null) {
        nuovo.focus();
        if (fuocoSu === undefined && inizio !== null && fine !== null && "setSelectionRange" in nuovo) {
          try {
            (nuovo as HTMLInputElement).setSelectionRange(inizio, fine);
          } catch {
            // Tipo di campo senza selezione (date, checkbox): niente da ripristinare.
          }
        }
      }
    }
  }

  return {
    id: ID_PANNELLO_STRUMENTI,
    icona: ICONA_STRUMENTI,
    titolo: "Strumenti",
    monta(c: HTMLElement): void {
      contenitore = c;
      c.append(radice);
      disegna();
    },
    mostrato(): void {
      // Alla riapertura: Modelli ricarica la lista; «Salva come modello» rilegge selezione e campi.
      if (vista === "modelli") {
        void carica();
      } else if (vista === "pagina") {
        paginaVista.carica();
        disegna();
      } else if (vista === "equazioni") {
        equazioniVista.carica();
        disegna();
      } else {
        disegna();
      }
    },
    nascosto(): void {
      if (timerAggiorna !== null) {
        clearTimeout(timerAggiorna);
        timerAggiorna = null;
      }
    },
    fuoco(): void {
      (contenitore?.querySelector("button, input, [tabindex='0']") as HTMLElement | null)?.focus();
    },
    aggiorna(): void {
      if (vista !== "salva") {
        return;
      }
      if (timerAggiorna !== null) {
        clearTimeout(timerAggiorna);
      }
      timerAggiorna = setTimeout(() => {
        timerAggiorna = null;
        if (vista === "salva" && !occupato) {
          disegna();
        }
      }, RITARDO_AGGIORNA_MS);
    },
    apriModelli,
  };
}
