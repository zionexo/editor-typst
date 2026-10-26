/**
 * barra-laterale.ts — Barra laterale a pannelli, stile VS Code (F5-019).
 *
 * A sinistra dell'editor: una fila di icone (activity bar, `role="tablist"`)
 * che commuta i pannelli, il pannello attivo, e un divisore per regolarne la
 * larghezza. I pannelli si **registrano** (`registra`) con `{id, icona, titolo,
 * monta}` invece di essere cablati qui: Indice (F5-020), File
 * (F5-021) e Pacchetti (F5-019) sono i primi, la ricerca nel progetto (F7-003)
 * si aggiungerà come un altro pannello senza toccare questo modulo. Nessun
 * pannello segnaposto: se non è registrato, non compare.
 *
 * Il modulo non persiste nulla: a ogni cambio di stato invoca `onCambio` e
 * `main.ts` lo salva in `impostazioni.json`. Il DOM è quello statico di
 * `index.html` (`#barra`, `#barra-attivita`, `#barra-pannelli`,
 * `#barra-divisore`); le sole parti dinamiche sono i pulsanti e le sezioni dei
 * pannelli, creati con `createElement` (mai `innerHTML`).
 *
 * Accessibilità: activity bar come tablist verticale con *roving tabindex*
 * (frecce su/giù e Home/End spostano il fuoco, Invio/Spazio attivano, il clic
 * sul pannello già attivo chiude la barra); ogni pannello è un `tabpanel`
 * etichettato dalla sua scheda; il divisore è un `separator` focalizzabile
 * regolabile con le frecce; `Esc` dalla barra riporta il fuoco all'editor.
 */

import { naviga } from "./pannelli/tastiera.ts";

/** Larghezza minima della barra dei pannelli (px). */
export const LARGHEZZA_MIN = 160;
/** Larghezza massima assoluta della barra dei pannelli (px). */
export const LARGHEZZA_MAX = 600;
/** Quota massima della finestra occupabile dalla barra (il resto serve a editor e anteprima). */
const QUOTA_MAX = 0.5;
/** Passo di regolazione da tastiera (px). */
const PASSO_TASTIERA = 16;

/**
 * Larghezza ammessa per la barra dei pannelli: fra il minimo e il più piccolo
 * tra il massimo assoluto e la metà della larghezza `disponibile` (finestra).
 * Valori non finiti tornano al minimo. Il risultato è intero.
 */
export function limitaLarghezza(px: number, disponibile: number): number {
  if (!Number.isFinite(px)) {
    return LARGHEZZA_MIN;
  }
  const tetto = Number.isFinite(disponibile) && disponibile > 0
    ? Math.max(LARGHEZZA_MIN, Math.min(LARGHEZZA_MAX, Math.floor(disponibile * QUOTA_MAX)))
    : LARGHEZZA_MAX;
  return Math.round(Math.min(Math.max(px, LARGHEZZA_MIN), tetto));
}

/** Un pannello registrabile nella barra. */
export interface Pannello {
  /** Identificatore univoco (`[a-z0-9-]`, persistito come pannello attivo). */
  id: string;
  /** Dati del percorso SVG (`d`, viewBox 24x24, solo contorno) dell'icona. */
  icona: string;
  /** Titolo: intestazione del pannello e nome accessibile dell'icona. */
  titolo: string;
  /** Costruisce il contenuto in `contenitore`; chiamata una sola volta, alla prima visualizzazione. */
  monta(contenitore: HTMLElement): void;
  /** Il pannello è diventato visibile (attivo e barra aperta): aggiornare i dati. */
  mostrato?(): void;
  /** Il pannello non è più visibile. */
  nascosto?(): void;
  /** Porta il fuoco sul contenuto (solo su richiesta esplicita dell'utente). */
  fuoco?(): void;
}

/** Stato della barra da ricordare fra le sessioni. */
export interface StatoBarra {
  visibile: boolean;
  /** Larghezza in px dell'area dei pannelli. */
  larghezza: number;
  /** Pannello attivo, o `null` se nessuno è registrato. */
  pannello: string | null;
}

/** API della barra laterale verso il resto del frontend. */
export interface BarraLaterale {
  /** Registra un pannello (id già presente: errore). */
  registra(pannello: Pannello): void;
  /** Pannelli registrati, nell'ordine di registrazione. */
  pannelli(): readonly { id: string; titolo: string }[];
  /** Mostra la barra sul pannello `id` e gli dà il fuoco. Ignora id sconosciuti. */
  apri(id: string): void;
  /** Come `apri`, ma se il pannello è già visibile chiude la barra. */
  alterna(id: string): void;
  /** Mostra o nasconde la barra. */
  alternaVisibilita(): void;
  /** `true` se la barra è visibile. */
  visibile(): boolean;
  /**
   * Applica uno stato salvato (nessun fuoco e nessun `onCambio`): un pannello
   * sconosciuto ripiega sul primo registrato, la larghezza è riportata nei limiti.
   */
  ripristina(stato: Partial<StatoBarra>): void;
}

/** Elemento per id o errore esplicito (come in `main.ts`). */
function elem<T extends HTMLElement>(id: string): T {
  const el = document.getElementById(id);
  if (el === null) {
    throw new Error(`Elemento DOM mancante: #${id}`);
  }
  return el as T;
}

const NS_SVG = "http://www.w3.org/2000/svg";

/** Icona SVG a contorno (eredita il colore dal testo). */
function creaIcona(percorso: string): SVGSVGElement {
  const svg = document.createElementNS(NS_SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "22");
  svg.setAttribute("height", "22");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  svg.setAttribute("class", "barra-icona");
  const p = document.createElementNS(NS_SVG, "path");
  p.setAttribute("d", percorso);
  svg.append(p);
  return svg;
}

/** Voce interna: pannello registrato + suoi elementi DOM. */
interface Voce {
  pannello: Pannello;
  scheda: HTMLButtonElement;
  sezione: HTMLElement;
  corpo: HTMLElement;
  montato: boolean;
}

/**
 * Crea la barra laterale sul DOM di `index.html`.
 *
 * @param onCambio invocata a ogni cambio di visibilità, pannello attivo o
 *                 larghezza fatto dall'utente (per persistere lo stato).
 * @param onEsci   invocata con `Esc` dalla barra (di norma: fuoco all'editor).
 */
export function creaBarraLaterale(
  onCambio: (stato: StatoBarra) => void,
  onEsci: () => void,
): BarraLaterale {
  const barra = elem<HTMLElement>("barra");
  const attivita = elem<HTMLElement>("barra-attivita");
  const area = elem<HTMLElement>("barra-pannelli");
  const divisore = elem<HTMLElement>("barra-divisore");

  const voci: Voce[] = [];
  let idAttivo: string | null = null;
  let mostraBarra = true;
  let larghezza = 260;
  /** Pannello per cui è stato chiamato `mostrato` (per chiamare `nascosto` una volta sola). */
  let idMostrato: string | null = null;
  /** `false` finché `ripristina` non ha applicato lo stato salvato: la barra resta nascosta (niente sfarfallio). */
  let pronta = false;

  function stato(): StatoBarra {
    return { visibile: mostraBarra, larghezza, pannello: idAttivo };
  }

  function voce(id: string | null): Voce | undefined {
    return voci.find((v) => v.pannello.id === id);
  }

  /** Larghezza massima consentita ora (in funzione della finestra). */
  function limita(px: number): number {
    return limitaLarghezza(px, window.innerWidth);
  }

  function applicaLarghezza(): void {
    area.style.width = `${larghezza}px`;
    divisore.setAttribute("aria-valuenow", String(larghezza));
    divisore.setAttribute("aria-valuemin", String(LARGHEZZA_MIN));
    divisore.setAttribute("aria-valuemax", String(limita(LARGHEZZA_MAX)));
  }

  /** Allinea il DOM (visibilità, scheda e sezione attive) allo stato e notifica i pannelli. */
  function aggiorna(): void {
    barra.hidden = !pronta;
    divisore.hidden = !pronta || !mostraBarra;
    area.hidden = !mostraBarra;
    // L'activity bar resta sempre visibile: è il modo per riaprire la barra col mouse.
    for (const v of voci) {
      const attivo = v.pannello.id === idAttivo;
      v.scheda.setAttribute("aria-selected", String(attivo));
      v.scheda.classList.toggle("attiva", attivo && mostraBarra);
      v.sezione.hidden = !attivo;
    }
    const daMostrare = pronta && mostraBarra ? voce(idAttivo) : undefined;
    const nuovoId = daMostrare?.pannello.id ?? null;
    if (idMostrato !== null && idMostrato !== nuovoId) {
      voce(idMostrato)?.pannello.nascosto?.();
    }
    if (daMostrare !== undefined) {
      if (!daMostrare.montato) {
        daMostrare.montato = true;
        daMostrare.pannello.monta(daMostrare.corpo);
      }
      if (idMostrato !== nuovoId) {
        daMostrare.pannello.mostrato?.();
      }
    }
    idMostrato = nuovoId;
  }

  function cambiato(): void {
    aggiorna();
    onCambio(stato());
  }

  function attiva(id: string, conFuoco: boolean): void {
    if (voce(id) === undefined) {
      return;
    }
    const giaMostrato = idMostrato === id;
    idAttivo = id;
    mostraBarra = true;
    cambiato();
    if (conFuoco) {
      // Richiesta esplicita su un pannello già visibile: i dati si rileggono comunque
      // (es. "Controlla aggiornamenti dei pacchetti" a pannello aperto).
      if (giaMostrato) {
        voce(id)?.pannello.mostrato?.();
      }
      voce(id)?.pannello.fuoco?.();
    }
  }

  function registra(pannello: Pannello): void {
    if (voce(pannello.id) !== undefined) {
      throw new Error(`Pannello già registrato: ${pannello.id}`);
    }
    const scheda = document.createElement("button");
    scheda.type = "button";
    scheda.id = `tab-${pannello.id}`;
    scheda.className = "barra-scheda";
    scheda.setAttribute("role", "tab");
    scheda.setAttribute("aria-controls", `pannello-${pannello.id}`);
    scheda.setAttribute("aria-label", pannello.titolo);
    scheda.title = pannello.titolo;
    scheda.tabIndex = voci.length === 0 ? 0 : -1;
    scheda.append(creaIcona(pannello.icona));
    scheda.addEventListener("click", () => {
      if (idAttivo === pannello.id && mostraBarra) {
        mostraBarra = false;
        cambiato();
      } else {
        attiva(pannello.id, true);
      }
    });
    attivita.append(scheda);

    const sezione = document.createElement("section");
    sezione.id = `pannello-${pannello.id}`;
    sezione.className = "barra-sezione";
    sezione.setAttribute("role", "tabpanel");
    sezione.setAttribute("aria-labelledby", scheda.id);
    sezione.hidden = true;
    const titolo = document.createElement("h2");
    titolo.className = "barra-titolo";
    titolo.textContent = pannello.titolo;
    const corpo = document.createElement("div");
    corpo.className = "barra-corpo";
    sezione.append(titolo, corpo);
    area.append(sezione);

    voci.push({ pannello, scheda, sezione, corpo, montato: false });
    // Il primo pannello registrato è quello di ripiego finché non se ne sceglie un altro.
    if (idAttivo === null) {
      idAttivo = pannello.id;
    }
    aggiorna();
  }

  // ---- Tastiera sull'activity bar (tablist verticale, attivazione manuale) ----
  attivita.addEventListener("keydown", (ev: KeyboardEvent) => {
    const schede = voci.map((v) => v.scheda);
    const corrente = schede.findIndex((s) => s === document.activeElement);
    const prossimo = naviga(schede.length, corrente, ev.key);
    if (prossimo === null) {
      return;
    }
    ev.preventDefault();
    schede.forEach((s, i) => (s.tabIndex = i === prossimo ? 0 : -1));
    schede[prossimo].focus();
  });
  // Il tabindex "attivo" dell'activity bar segue il fuoco (anche col clic).
  attivita.addEventListener("focusin", (ev: FocusEvent) => {
    for (const v of voci) {
      v.scheda.tabIndex = v.scheda === ev.target ? 0 : -1;
    }
  });

  // Esc da qualunque punto della barra: si torna all'editor.
  barra.addEventListener("keydown", (ev: KeyboardEvent) => {
    if (ev.key === "Escape" && !ev.defaultPrevented) {
      ev.preventDefault();
      onEsci();
    }
  });

  // ---- Divisore: trascinamento e tastiera ----
  divisore.addEventListener("pointerdown", (ev: PointerEvent) => {
    ev.preventDefault();
    divisore.setPointerCapture(ev.pointerId);
    const x0 = ev.clientX;
    const l0 = larghezza;
    const sposta = (e: PointerEvent): void => {
      larghezza = limita(l0 + (e.clientX - x0));
      applicaLarghezza();
    };
    const fine = (): void => {
      divisore.removeEventListener("pointermove", sposta);
      divisore.removeEventListener("pointerup", fine);
      divisore.removeEventListener("pointercancel", fine);
      onCambio(stato());
    };
    divisore.addEventListener("pointermove", sposta);
    divisore.addEventListener("pointerup", fine);
    divisore.addEventListener("pointercancel", fine);
  });
  divisore.addEventListener("keydown", (ev: KeyboardEvent) => {
    let nuova: number | null = null;
    if (ev.key === "ArrowLeft") {
      nuova = larghezza - PASSO_TASTIERA;
    } else if (ev.key === "ArrowRight") {
      nuova = larghezza + PASSO_TASTIERA;
    } else if (ev.key === "Home") {
      nuova = LARGHEZZA_MIN;
    } else if (ev.key === "End") {
      nuova = LARGHEZZA_MAX;
    }
    if (nuova === null) {
      return;
    }
    ev.preventDefault();
    larghezza = limita(nuova);
    applicaLarghezza();
    onCambio(stato());
  });
  // Restringendo la finestra la barra non deve mangiare tutto lo spazio.
  window.addEventListener("resize", () => {
    const l = limita(larghezza);
    if (l !== larghezza) {
      larghezza = l;
      applicaLarghezza();
    }
  });

  applicaLarghezza();

  return {
    registra,
    pannelli: () => voci.map((v) => ({ id: v.pannello.id, titolo: v.pannello.titolo })),
    apri: (id) => attiva(id, true),
    alterna: (id) => {
      if (idAttivo === id && mostraBarra) {
        mostraBarra = false;
        cambiato();
      } else {
        attiva(id, true);
      }
    },
    alternaVisibilita: () => {
      mostraBarra = !mostraBarra;
      cambiato();
    },
    visibile: () => mostraBarra,
    ripristina: (s) => {
      pronta = true;
      if (s.larghezza !== undefined) {
        larghezza = limita(s.larghezza);
        applicaLarghezza();
      }
      if (s.pannello !== undefined && s.pannello !== null && voce(s.pannello) !== undefined) {
        idAttivo = s.pannello;
      } else if (voce(idAttivo) === undefined) {
        idAttivo = voci[0]?.pannello.id ?? null;
      }
      if (s.visibile !== undefined) {
        mostraBarra = s.visibile;
      }
      aggiorna();
    },
  };
}
