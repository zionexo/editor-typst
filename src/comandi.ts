/**
 * comandi.ts — Registro unico dei comandi dell'app (F5-006).
 *
 * Un solo registro `{id, titolo, scorciatoie, esegui, abilitato?}` alimenta la
 * palette comandi (`palette.ts`), le scorciatoie da tastiera e, in prospettiva,
 * i pulsanti. Il modulo NON conosce il DOM né CodeMirror: la logica di
 * scorciatoie e di ricerca è pura e verificabile da `comandi.check.ts`.
 *
 * Derivazione: i tipi `Comando`/`Sottocomando` sono ADATTATI da
 * `CommandDescriptor`/`CommandSubcommand` di MarkText (packages/desktop/src/
 * renderer/src/commands/index.ts, righe 18-54, commit
 * 34b5c2832380da3c0c24b50a4a0a203cfcde46bf), Copyright (c) MarkText
 * contributors, licenza MIT (https://github.com/marktext/marktext).
 * Modifiche: nomi in italiano, id gerarchici, titolo come stringa o funzione
 * (traduzioni F7-004), `abilitato`, scorciatoie normalizzate con `Mod` = Ctrl su
 * Win/Linux e Cmd su macOS. Ricerca fuzzy e dispatcher sono scritti ex novo.
 * Attribuzione nel NOTICE: F5-024.
 */

// Derivato da MarkText packages/desktop/src/renderer/src/commands/index.ts (34b5c28), licenza MIT — Copyright (c) MarkText contributors

/** Titolo mostrato: stringa, o funzione valutata a ogni visualizzazione (i18n, F7-004). */
export type Titolo = string | (() => string);

/** Voce di secondo livello (es. "Esporta ▸ PDF"). Non ancora usata dalla palette (F7-001). */
export interface Sottocomando {
  /** Identificatore gerarchico completo (es. `esporta.pdf`). */
  id: string;
  titolo: Titolo;
  /** Valore opzionale passato al gestore del comando padre. */
  valore?: unknown;
  esegui?: () => void | Promise<void>;
}

/** Descrittore di un comando registrato. */
export interface Comando {
  /** Identificatore univoco e gerarchico, es. `file.salva`, `esporta.pdf`. */
  id: string;
  titolo: Titolo;
  /**
   * Scorciatoie, la prima è quella mostrata. Sintassi `Mod+Shift+O`: `Mod` è
   * Ctrl o Cmd indifferentemente; un tasto punteggiatura si scrive com'è
   * (`Mod++`, `Mod+=`, `Mod+-`); vedi `analizzaScorciatoia`.
   */
  scorciatoie?: string[];
  /** Azione; può mancare in un comando che espone solo sottocomandi. */
  esegui?: () => void | Promise<void>;
  /** Se presente e falso, il comando è nascosto dalla palette e la scorciatoia non agisce. */
  abilitato?: () => boolean;
  /** Se vero la scorciatoia si ripete tenendo premuto il tasto (es. dimensione font). */
  ripetibile?: boolean;
  /** Sottocomandi (anche calcolati in modo asincrono): innesto per F7-001. */
  sottocomandi?: Sottocomando[] | (() => Promise<Sottocomando[]>);
  // Campi solo di tipo, per l'innesto di F7-001 (nessuna logica li usa ancora):
  /** Esegue un sottocomando scelto (id gerarchico, valore opzionale). */
  eseguiSottocomando?: (id: string, valore?: unknown) => void | Promise<void>;
  /** Ricerca delegata al comando (es. quick-open sui file). */
  cerca?: (query: string) => Promise<Sottocomando[]>;
  /** Rilascia le risorse caricate per mostrare i sottocomandi. */
  scarica?: () => void;
}

/** Valuta un titolo (stringa o funzione). */
export function titoloDi(t: { titolo: Titolo }): string {
  return typeof t.titolo === "function" ? t.titolo() : t.titolo;
}

// ---- Scorciatoie (logica pura) ----

/** Scorciatoia scomposta. `chiave` è minuscola ("o", "0", "+", "enter"...). */
export interface ScorciatoiaAnalizzata {
  mod: boolean;
  shift: boolean;
  alt: boolean;
  chiave: string;
}

/** Sottoinsieme di `KeyboardEvent` usato per il confronto (permette il test senza DOM). */
export interface TastoEvento {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/**
 * Scompone una stringa `Mod+Shift+O`. L'ultimo elemento è il tasto; se la stringa
 * finisce con `+` il tasto è il carattere `+` (es. `Mod++`).
 * @throws Error se la sintassi è invalida (errore di programmazione: fail-fast).
 */
export function analizzaScorciatoia(s: string): ScorciatoiaAnalizzata {
  const finisceConPiu = s.endsWith("+");
  const parti = (finisceConPiu ? s.slice(0, -1) : s).split("+");
  let chiave: string;
  if (finisceConPiu) {
    // "Mod++" → "Mod+" → ["Mod", ""]: l'elemento vuoto è il separatore prima del tasto "+".
    if (parti[parti.length - 1] !== "") {
      throw new Error(`Scorciatoia non valida: "${s}"`);
    }
    parti.pop();
    chiave = "+";
  } else {
    chiave = (parti.pop() ?? "").toLowerCase();
  }
  const out: ScorciatoiaAnalizzata = { mod: false, shift: false, alt: false, chiave };
  for (const m of parti) {
    switch (m.toLowerCase()) {
      case "mod":
        out.mod = true;
        break;
      case "shift":
        out.shift = true;
        break;
      case "alt":
        out.alt = true;
        break;
      default:
        throw new Error(`Modificatore sconosciuto "${m}" in "${s}"`);
    }
  }
  if (chiave === "") {
    throw new Error(`Scorciatoia senza tasto: "${s}"`);
  }
  return out;
}

/**
 * Forma canonica (per rilevare scorciatoie duplicate). Per un tasto punteggiatura a un
 * carattere Shift è ignorato dal confronto (vedi `corrisponde`), quindi qui è azzerato:
 * `Mod+Shift++` e `Mod++` sono la stessa scorciatoia.
 */
export function canonica(s: ScorciatoiaAnalizzata): string {
  const shift = s.shift && !(s.chiave.length === 1 && /[^a-z0-9]/.test(s.chiave));
  return `${s.mod ? "Mod+" : ""}${s.alt ? "Alt+" : ""}${shift ? "Shift+" : ""}${s.chiave}`;
}

/**
 * Vero se `ev` corrisponde alla scorciatoia. `Mod` è Cmd su macOS (`mac`) e Ctrl altrove:
 * il modificatore dell'altra piattaforma premuto da solo esclude la corrispondenza
 * (Ctrl+S su Mac non è Cmd+S, Win+S su Windows non è Ctrl+S).
 */
export function corrisponde(sc: ScorciatoiaAnalizzata, ev: TastoEvento, mac: boolean): boolean {
  const modPremuto = mac ? ev.metaKey : ev.ctrlKey;
  const altroPremuto = mac ? ev.ctrlKey : ev.metaKey;
  if (altroPremuto || modPremuto !== sc.mod || ev.altKey !== sc.alt) {
    return false;
  }
  const c = sc.chiave;
  if (/^[0-9]$/.test(c)) {
    // Cifre dal tasto fisico (riga alta o tastierino, layout diversi); Shift esatto.
    return ev.shiftKey === sc.shift && (ev.code === `Digit${c}` || ev.code === `Numpad${c}`);
  }
  if (/^[a-z]$/.test(c)) {
    if (ev.shiftKey !== sc.shift) {
      return false;
    }
    if (/^[a-z]$/i.test(ev.key)) {
      return ev.key.toLowerCase() === c;
    }
    // Layout non latino (es. cirillico): si ripiega sul tasto fisico.
    return ev.code === `Key${c.toUpperCase()}`;
  }
  if (c.length === 1) {
    // Punteggiatura: Shift dipende dal layout (`+` richiede Shift su molte tastiere), lo si ignora.
    return ev.key === c;
  }
  // Tasti con nome (Enter, Escape, F1...): Shift esatto.
  return ev.shiftKey === sc.shift && ev.key.toLowerCase() === c;
}

/**
 * Testo di una scorciatoia per la UI. Su macOS simboli (`⌘⇧O`), altrove `Ctrl+Shift+O`.
 */
export function formattaScorciatoia(s: string, mac: boolean): string {
  const a = analizzaScorciatoia(s);
  const tasto =
    a.chiave.length === 1 ? a.chiave.toUpperCase() : a.chiave[0].toUpperCase() + a.chiave.slice(1);
  if (mac) {
    return `${a.alt ? "⌥" : ""}${a.shift ? "⇧" : ""}${a.mod ? "⌘" : ""}${tasto}`;
  }
  return [a.mod ? "Ctrl" : "", a.alt ? "Alt" : "", a.shift ? "Shift" : "", tasto]
    .filter((p) => p !== "")
    .join("+");
}

// ---- Ricerca fuzzy (logica pura) ----

/** Minuscolo e senza segni diacritici, così "e" trova "è". */
function normalizza(s: string): string {
  return s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** `true` se in posizione `i` inizia una parola (inizio stringa o dopo un non alfanumerico). */
function inizioParola(t: string, i: number): boolean {
  return i === 0 || !/[\p{L}\p{N}]/u.test(t[i - 1]);
}

/** Punteggio di un token (già normalizzato) su un testo (già normalizzato), o `null`. */
function punteggioToken(token: string, t: string): number | null {
  const idx = t.indexOf(token);
  if (idx >= 0) {
    // Sottostringa: sempre meglio di una semplice sottosequenza (1000 > qualsiasi somma sotto).
    return 1000 + (idx === 0 ? 60 : inizioParola(t, idx) ? 40 : 0) - Math.min(idx, 30);
  }
  // Sottosequenza: i caratteri compaiono in ordine, con bonus per contiguità e inizi parola.
  let pos = 0;
  let punti = 0;
  let precedente = -2;
  for (const c of token) {
    const i = t.indexOf(c, pos);
    if (i < 0) {
      return null;
    }
    punti += i === precedente + 1 ? 5 : 1;
    if (inizioParola(t, i)) {
      punti += 3;
    }
    precedente = i;
    pos = i + 1;
  }
  return punti;
}

/**
 * Punteggio di `testo` per `query` (più alto = migliore), o `null` se non corrisponde.
 * La query è divisa in parole separate da spazi: tutte devono corrispondere (AND).
 * Query vuota: punteggio 0 (tutto corrisponde).
 */
export function punteggioFuzzy(query: string, testo: string): number | null {
  const t = normalizza(testo);
  let totale = 0;
  for (const token of normalizza(query).split(/\s+/).filter((p) => p !== "")) {
    const p = punteggioToken(token, t);
    if (p === null) {
      return null;
    }
    totale += p;
  }
  return totale;
}

/**
 * Filtra e ordina `voci` per `query`: solo le corrispondenti, dal punteggio più alto;
 * a parità resta l'ordine originale (ordinamento stabile). Query vuota: tutte, invariate.
 */
export function filtraFuzzy<T>(voci: readonly T[], query: string, testo: (v: T) => string): T[] {
  const conPunti: { v: T; p: number; i: number }[] = [];
  voci.forEach((v, i) => {
    const p = punteggioFuzzy(query, testo(v));
    if (p !== null) {
      conPunti.push({ v, p, i });
    }
  });
  conPunti.sort((a, b) => b.p - a.p || a.i - b.i);
  return conPunti.map((x) => x.v);
}

/**
 * Nome della piattaforma dichiarato dalla webview (`""` se non disponibile).
 * Unico punto di lettura di `navigator`, condiviso da `rilevaMac` e `rilevaWindows`.
 */
function nomePiattaforma(): string {
  const nav = globalThis.navigator as (Navigator & { userAgentData?: { platform?: string } }) | undefined;
  return nav?.userAgentData?.platform ?? nav?.platform ?? "";
}

/**
 * Rileva macOS (unico punto di rilevamento, condiviso da dispatcher e palette).
 * Da chiamare a runtime nel frontend: usa `navigator`, non disponibile in altri ambienti.
 */
export function rilevaMac(): boolean {
  return /Mac|iPhone|iPad/i.test(nomePiattaforma());
}

/**
 * Rileva Windows (percorsi: separatore `\`, nomi insensibili alle maiuscole).
 * Ancorato all'inizio: "Darwin"/"MacIntel" non devono contenere "win" per caso.
 */
export function rilevaWindows(): boolean {
  return /^Win/i.test(nomePiattaforma());
}

// ---- Registro ----

/** Sottoinsieme di `KeyboardEvent` usato dal dispatcher (strutturale: testabile senza DOM). */
export interface EventoTasto extends TastoEvento {
  repeat: boolean;
  isComposing: boolean;
  preventDefault(): void;
}

/** Registro dei comandi con dispatcher delle scorciatoie. */
export interface RegistroComandi {
  /** Piattaforma con cui il registro interpreta `Mod` (Cmd se `true`, Ctrl altrimenti). */
  readonly mac: boolean;
  /**
   * Registra un comando.
   * @throws Error su id duplicato o scorciatoia già assegnata (errore di programmazione).
   */
  registra(comando: Comando): void;
  /** Comando per id, se registrato. */
  ottieni(id: string): Comando | undefined;
  /** Comandi attualmente abilitati (quelli mostrati dalla palette). */
  abilitati(): Comando[];
  /** Esegue un comando per id se esiste ed è abilitato; gli errori vanno a `onErrore`. */
  esegui(id: string): void;
  /**
   * Cerca un comando per la scorciatoia dell'evento. Se c'è una corrispondenza fa SEMPRE
   * `preventDefault` e restituisce `true` (anche se il comando è disabilitato o escluso:
   * Ctrl+P non deve stampare la webview, Ctrl+S/O non devono aprire i dialoghi nativi).
   * Il comando parte solo se abilitato e se `eseguiSolo` è assente o uguale al suo id.
   */
  gestisciTasto(ev: EventoTasto, eseguiSolo?: string): boolean;
}

/**
 * Crea un registro vuoto.
 * @param onErrore invocata se l'esecuzione di un comando fallisce (sincrona o asincrona).
 * @param mac `true` se `Mod` è Cmd (macOS), `false` se è Ctrl (vedi `rilevaMac`).
 */
export function creaRegistro(onErrore: (e: unknown) => void, mac: boolean): RegistroComandi {
  const comandi: Comando[] = [];
  const perId = new Map<string, Comando>();
  const perScorciatoia = new Map<string, { sc: ScorciatoiaAnalizzata; id: string }>();

  function abilitato(c: Comando): boolean {
    return c.abilitato === undefined || c.abilitato();
  }

  function lancia(c: Comando): void {
    try {
      // `Promise.resolve` uniforma esegui sincrono e asincrono: il rifiuto non va perso.
      Promise.resolve(c.esegui?.()).catch(onErrore);
    } catch (e) {
      onErrore(e);
    }
  }

  return {
    mac,
    registra(comando) {
      if (perId.has(comando.id)) {
        throw new Error(`Comando già registrato: ${comando.id}`);
      }
      const nuove = (comando.scorciatoie ?? []).map((s) => analizzaScorciatoia(s));
      for (const sc of nuove) {
        const altro = perScorciatoia.get(canonica(sc));
        if (altro !== undefined) {
          throw new Error(
            `Scorciatoia ${canonica(sc)} di "${comando.id}" già usata da "${altro.id}"`,
          );
        }
      }
      for (const sc of nuove) {
        perScorciatoia.set(canonica(sc), { sc, id: comando.id });
      }
      comandi.push(comando);
      perId.set(comando.id, comando);
    },
    ottieni: (id) => perId.get(id),
    abilitati: () => comandi.filter(abilitato),
    esegui(id) {
      const c = perId.get(id);
      if (c !== undefined && abilitato(c)) {
        lancia(c);
      }
    },
    gestisciTasto(ev, eseguiSolo) {
      if (ev.isComposing) {
        return false;
      }
      for (const { sc, id } of perScorciatoia.values()) {
        if (!corrisponde(sc, ev, mac)) {
          continue;
        }
        const c = perId.get(id);
        if (c === undefined) {
          continue;
        }
        ev.preventDefault();
        // Un tasto tenuto premuto non rilancia il comando (es. Salva), salvo `ripetibile`.
        const consentito = eseguiSolo === undefined || eseguiSolo === id;
        if (consentito && (!ev.repeat || c.ripetibile === true) && abilitato(c)) {
          lancia(c);
        }
        return true;
      }
      return false;
    },
  };
}
