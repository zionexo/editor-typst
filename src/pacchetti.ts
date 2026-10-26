/**
 * pacchetti.ts — Browser dei pacchetti di Typst Universe (Fase 3).
 *
 * Recupera l'indice dei pacchetti dal backend (comando `elenca_pacchetti`, con
 * cache e degradazione offline lato Rust), li mostra in un pannello con ricerca,
 * e permette di installarli (`installa_pacchetto` → pre-download nella cache
 * `@preview`) inserendo automaticamente l'`#import` nell'editor.
 *
 * Dalla F5-019 il browser è un pannello della barra laterale (`barra-laterale.ts`):
 * il gestore implementa `Pannello` (monta il proprio DOM nel contenitore che la
 * barra gli dà, si aggiorna quando diventa visibile) invece di essere un drawer
 * a scomparsa a destra. Funzioni invariate: ricerca, badge di aggiornamento,
 * installazione con import, nuovo progetto da template.
 *
 * Sicurezza: i metadati dei pacchetti provengono dalla rete (non fidati). Il
 * rendering usa SEMPRE `textContent`/`createElement`, MAI `innerHTML`, per
 * impedire l'iniezione di markup da descrizioni/nomi remoti.
 */

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Editor } from "./editor";
import type { Pannello } from "./barra-laterale";

/** Metadati di un pacchetto, nel formato prodotto da `elenca_pacchetti`. */
interface PacchettoInfo {
  nome: string;
  versione: string;
  descrizione: string;
  autori: string[];
  licenza: string | null;
  categorie: string[];
  parole_chiave: string[];
  repository: string | null;
  e_template: boolean;
}

/** Pacchetto con una versione più recente disponibile (F5-012, `AggiornamentoPacchetto` del core). */
interface AggiornamentoPacchetto {
  nome: string;
  attuale: string;
  ultima: string;
}

/** Esito di `aggiornamenti_pacchetti`: pacchetti installati e import del documento. */
interface EsitoAggiornamenti {
  installati: AggiornamentoPacchetto[];
  documento: AggiornamentoPacchetto[];
}

/** Esito di `elenca_pacchetti`: lista + se proviene dalla cache stantia (offline). */
interface EsitoElenco {
  pacchetti: PacchettoInfo[];
  stantia: boolean;
}

// Vincoli sui nomi/versioni dei pacchetti (difesa in profondità: anche il backend
// valida). Impediscono che un nome/versione manomesso inietti markup Typst quando
// vengono interpolati nell'`#import`.
const RE_NOME = /^[a-z0-9-]+$/;
const RE_VERSIONE = /^\d+\.\d+\.\d+$/;

/** Vero se nome e versione rispettano i vincoli di Typst Universe. */
function pacchettoValido(nome: string, versione: string): boolean {
  return RE_NOME.test(nome) && RE_VERSIONE.test(versione);
}

/**
 * Pannello dei pacchetti per la barra laterale: `mostrato` carica l'indice alla
 * prima visualizzazione e rifà il confronto degli aggiornamenti alle successive.
 */
export type GestorePacchetti = Pannello;

/** Identificatore del pannello nella barra laterale. */
export const ID_PANNELLO_PACCHETTI = "pacchetti";

/**
 * Crea il gestore del pannello pacchetti.
 *
 * @param editor      editor in cui inserire l'`#import` dopo l'installazione.
 * @param onProgetto  invocata dopo "Nuovo da template" col percorso entrypoint e
 *                    il contenuto, per adottare il progetto come documento corrente.
 * @param onRicompila invocata dopo install/template (per aggiornare l'anteprima).
 * @param onErrore    invocata in caso di errore di rete/installazione.
 * @param sorgenteTypst sorgente Typst corrente (o `null` se non è un documento
 *                    Typst), per segnalare gli `#import` non aggiornati (F5-012).
 * @param primaDiSostituire chiamata dopo la scelta della cartella e PRIMA di creare il
 *                    progetto (che aggancia il World alla cartella del template):
 *                    chiede di salvare le modifiche; `false` (Annulla) = non si fa nulla.
 * @param onCreato    invocata dopo "Nuovo da template" riuscito (la barra può
 *                    passare al pannello dei file).
 */
export function creaGestorePacchetti(
  editor: Editor,
  onProgetto: (percorso: string, contenuto: string) => void | Promise<void>,
  onRicompila: () => void,
  onErrore: (errore: unknown) => void,
  sorgenteTypst: () => string | null = () => null,
  onCreato: () => void = () => {},
  primaDiSostituire: () => Promise<boolean> = () => Promise.resolve(true),
): GestorePacchetti {
  // DOM del pannello: creato subito, agganciato al contenitore da `monta`.
  const radice = document.createElement("div");
  radice.className = "pkg-pannello";
  const riga = document.createElement("div");
  riga.className = "pkg-ricerca-riga";
  const inputRicerca = document.createElement("input");
  inputRicerca.id = "ricerca-pacchetti";
  inputRicerca.type = "search";
  inputRicerca.placeholder = "Cerca pacchetti…";
  inputRicerca.setAttribute("aria-label", "Cerca pacchetti");
  const btnAggiorna = document.createElement("button");
  btnAggiorna.type = "button";
  btnAggiorna.id = "pkg-aggiorna";
  btnAggiorna.className = "barra-azione";
  btnAggiorna.title = "Aggiorna l'indice";
  btnAggiorna.setAttribute("aria-label", "Aggiorna l'indice dei pacchetti");
  btnAggiorna.textContent = "↻";
  riga.append(inputRicerca, btnAggiorna);
  const divStato = document.createElement("div");
  divStato.id = "stato-pacchetti";
  divStato.setAttribute("aria-live", "polite");
  const divAggDocumento = document.createElement("div");
  divAggDocumento.id = "agg-documento";
  divAggDocumento.hidden = true;
  const divLista = document.createElement("div");
  divLista.id = "lista-pacchetti";
  radice.append(riga, divStato, divAggDocumento, divLista);

  /** Indice completo caricato in memoria (filtrato a video dalla ricerca). */
  let tutti: PacchettoInfo[] = [];
  let caricato = false;
  /** `true` se l'indice mostrato proviene dalla cache locale stantia (offline). */
  let stantia = false;
  /** `true` mentre è in corso un'installazione (guardia anti doppio click). */
  let inCorso = false;
  /** Aggiornamenti disponibili per i pacchetti installati: nome → versioni (F5-012). */
  let aggiornabili = new Map<string, AggiornamentoPacchetto>();

  /** Carica l'indice dal backend e lo mostra. */
  async function carica(forza: boolean): Promise<void> {
    divStato.textContent = forza ? "Aggiornamento dell'indice…" : "Caricamento…";
    try {
      const esito = await invoke<EsitoElenco>("elenca_pacchetti", { forza });
      tutti = esito.pacchetti;
      stantia = esito.stantia;
      caricato = true;
      await controllaAggiornamenti();
      mostra(filtra(inputRicerca.value));
    } catch (e) {
      divStato.textContent = "Indice non disponibile (sei offline e senza cache?).";
      onErrore(e);
    }
  }

  /**
   * Confronta pacchetti installati e `#import` del documento con l'indice già
   * in cache (nessuna rete, F5-012). Un errore non blocca il pannello: si
   * omettono semplicemente i badge.
   */
  async function controllaAggiornamenti(): Promise<void> {
    aggiornabili = new Map();
    divAggDocumento.replaceChildren();
    divAggDocumento.hidden = true;
    try {
      const esito = await invoke<EsitoAggiornamenti>("aggiornamenti_pacchetti", {
        sorgente: sorgenteTypst(),
      });
      aggiornabili = new Map(esito.installati.map((a) => [a.nome, a]));
      if (esito.documento.length > 0) {
        const titolo = document.createElement("strong");
        titolo.textContent = "Il documento usa pacchetti non aggiornati:";
        divAggDocumento.append(titolo);
        for (const a of esito.documento) {
          const riga = document.createElement("div");
          riga.textContent = `${a.nome}: ${a.attuale} → ${a.ultima}`;
          divAggDocumento.append(riga);
        }
        divAggDocumento.hidden = false;
      }
    } catch (e) {
      console.warn("Controllo aggiornamenti non disponibile:", e);
    }
  }

  /** Restituisce i pacchetti che corrispondono alla query (nome/descrizione/categorie/keyword). */
  function filtra(query: string): PacchettoInfo[] {
    const q = query.trim().toLowerCase();
    if (q === "") {
      return tutti;
    }
    return tutti.filter((p) => {
      const campi = [
        p.nome,
        p.descrizione,
        ...p.categorie,
        ...p.parole_chiave,
      ]
        .join(" ")
        .toLowerCase();
      return campi.includes(q);
    });
  }

  /** Renderizza la lista dei pacchetti (rendering sicuro, solo textContent). */
  function mostra(lista: PacchettoInfo[]): void {
    divLista.replaceChildren();
    const suffisso = stantia ? " — cache locale (offline)" : "";
    divStato.textContent = `${lista.length} pacchetto/i${suffisso}`;

    for (const p of lista) {
      // Difesa in profondità: non mostrare voci con nome/versione fuori vincolo.
      if (!pacchettoValido(p.nome, p.versione)) {
        continue;
      }
      const card = document.createElement("div");
      card.className = "pkg-card";

      const titolo = document.createElement("div");
      titolo.className = "pkg-titolo";
      const nome = document.createElement("span");
      nome.className = "pkg-nome";
      nome.textContent = p.nome;
      const versione = document.createElement("span");
      versione.className = "pkg-versione";
      versione.textContent = p.versione;
      titolo.append(nome, versione);
      const agg = aggiornabili.get(p.nome);
      if (agg !== undefined) {
        // Testo da rete/cache locale: solo textContent, come il resto del pannello.
        const badge = document.createElement("span");
        badge.className = "pkg-agg";
        badge.textContent = `aggiornamento disponibile (installata ${agg.attuale})`;
        titolo.append(badge);
      }
      if (p.e_template) {
        const badge = document.createElement("span");
        badge.className = "pkg-badge";
        badge.textContent = "template";
        titolo.append(badge);
      }

      const desc = document.createElement("div");
      desc.className = "pkg-desc";
      desc.textContent = p.descrizione;

      const azioni = document.createElement("div");
      azioni.className = "pkg-azioni";
      if (p.categorie.length > 0) {
        const cat = document.createElement("span");
        cat.className = "pkg-categorie";
        cat.textContent = p.categorie.join(", ");
        azioni.append(cat);
      }
      const btn = document.createElement("button");
      btn.className = "pkg-installa";
      btn.textContent = "Installa e importa";
      btn.dataset.nome = p.nome;
      btn.dataset.versione = p.versione;
      // Più schede hanno pulsanti col testo identico: il nome accessibile li distingue.
      btn.setAttribute("aria-label", `Installa e importa ${p.nome} ${p.versione}`);
      azioni.append(btn);

      if (p.e_template) {
        const btnT = document.createElement("button");
        btnT.className = "pkg-template";
        btnT.textContent = "Nuovo da template";
        btnT.dataset.nome = p.nome;
        btnT.dataset.versione = p.versione;
        btnT.setAttribute("aria-label", `Nuovo da template ${p.nome} ${p.versione}`);
        azioni.append(btnT);
      }

      card.append(titolo, desc, azioni);
      divLista.append(card);
    }

    // Se un'installazione/creazione è in corso, i pulsanti appena (ri)creati devono
    // restare disabilitati: la ricerca ricrea le card anche durante l'operazione.
    if (inCorso) {
      abilitaInstalla(false);
    }
  }

  /** Abilita/disabilita i pulsanti d'azione delle card (durante un'operazione). */
  function abilitaInstalla(abilita: boolean): void {
    for (const b of divLista.querySelectorAll<HTMLButtonElement>(
      "button.pkg-installa, button.pkg-template",
    )) {
      b.disabled = !abilita;
    }
  }

  /** Installa un pacchetto (pre-download) e inserisce l'import nell'editor. */
  async function installa(nome: string, versione: string): Promise<void> {
    if (inCorso) {
      return;
    }
    // Difesa in profondità: non interpolare nell'`#import` valori fuori vincolo.
    if (!pacchettoValido(nome, versione)) {
      divStato.textContent = "Pacchetto non valido: installazione annullata.";
      return;
    }
    // Un documento Markdown non ha `#import`: inserirlo sporcherebbe il testo.
    // `sorgenteTypst` vale `null` proprio quando il documento non è Typst.
    if (sorgenteTypst() === null) {
      divStato.textContent =
        "Il documento è Markdown: l'#import di Typst non si può inserire. " +
        "Passa a un documento Typst per installare e importare il pacchetto.";
      return;
    }
    inCorso = true;
    abilitaInstalla(false);
    divStato.textContent = `Installazione di ${nome}…`;
    try {
      await invoke("installa_pacchetto", { nome, versione });
      editor.inserisci(`#import "@preview/${nome}:${versione}": *\n`);
      onRicompila();
      divStato.textContent = `Installato ${nome}:${versione}`;
    } catch (e) {
      divStato.textContent = `Errore nell'installazione di ${nome}.`;
      onErrore(e);
    } finally {
      inCorso = false;
      abilitaInstalla(true);
    }
  }

  /** Crea un nuovo progetto da un template: scelta cartella, download, copia, apertura. */
  async function nuovoDaTemplate(nome: string, versione: string): Promise<void> {
    if (inCorso) {
      return;
    }
    if (!pacchettoValido(nome, versione)) {
      divStato.textContent = "Pacchetto non valido: operazione annullata.";
      return;
    }
    let destinazione: string | null;
    try {
      destinazione = await open({
        directory: true,
        multiple: false,
        title: "Scegli la cartella in cui creare il progetto",
      });
    } catch (e) {
      // Un rigetto del dialog non deve diventare una unhandledrejection (overlay
      // di errore fatale a tutto schermo): instradalo sul riquadro errori inline.
      onErrore(e);
      return;
    }
    if (destinazione === null) {
      return; // annullato
    }
    inCorso = true;
    abilitaInstalla(false);
    try {
      if (!(await primaDiSostituire())) {
        divStato.textContent = "";
        return; // Annulla: niente viene creato né agganciato
      }
      divStato.textContent = `Creazione del progetto da ${nome}…`;
      const ris = await invoke<{ entrypoint: string; contenuto: string }>(
        "nuovo_da_template",
        { nome, versione, destinazione },
      );
      // L'ordine conta: si attende `onProgetto` (imposta il contenuto nell'editor),
      // poi `onRicompila` rilegge dall'editor il testo già aggiornato.
      await onProgetto(ris.entrypoint, ris.contenuto);
      onRicompila();
      divStato.textContent = `Progetto creato da ${nome}`;
      onCreato();
    } catch (e) {
      divStato.textContent = `Errore nella creazione del progetto da ${nome}.`;
      onErrore(e);
    } finally {
      inCorso = false;
      abilitaInstalla(true);
    }
  }

  // ---- Eventi ----
  inputRicerca.addEventListener("input", () => {
    if (caricato) {
      mostra(filtra(inputRicerca.value));
    }
  });
  btnAggiorna.addEventListener("click", () => void carica(true));
  // Delega: i pulsanti "Installa" sono creati dinamicamente.
  divLista.addEventListener("click", (ev: MouseEvent) => {
    if (!(ev.target instanceof HTMLElement)) {
      return;
    }
    const btn = ev.target.closest<HTMLButtonElement>(
      "button.pkg-installa, button.pkg-template",
    );
    if (btn === null) {
      return;
    }
    const { nome, versione } = btn.dataset;
    if (nome === undefined || versione === undefined) {
      return;
    }
    if (btn.classList.contains("pkg-template")) {
      void nuovoDaTemplate(nome, versione);
    } else {
      void installa(nome, versione);
    }
  });

  /** Il pannello è visibile: carica l'indice o rifà il confronto degli aggiornamenti. */
  function mostrato(): void {
    if (!caricato) {
      void carica(false);
    } else {
      // Il documento può essere cambiato dall'ultima visualizzazione: rifà il confronto.
      void controllaAggiornamenti().then(() => mostra(filtra(inputRicerca.value)));
    }
  }

  return {
    id: ID_PANNELLO_PACCHETTI,
    // Scatola: contorno di un parallelepipedo (pacchetto).
    icona: "M12 3l8 4.5v9L12 21l-8-4.5v-9L12 3zM4 7.5l8 4.5 8-4.5M12 12v9",
    titolo: "Pacchetti",
    monta: (contenitore) => contenitore.append(radice),
    mostrato,
    fuoco: () => inputRicerca.focus(),
  };
}
