/**
 * caricatorepagine.ts — Macchina a stati del caricamento degli SVG delle pagine
 * dell'anteprima (estratta da `anteprima.ts`, senza DOM né Tauri).
 *
 * Tiene lo `StatoPagina` di ogni pagina e decide quando richiedere l'SVG, cosa
 * fare al ritorno della risposta (pagina sostituita nel frattempo, hash cambiato
 * in volo, errore, reimpostazione delle definizioni condivise). Le operazioni sul
 * DOM sono iniettate: `anteprima.ts` le implementa, `caricatorepagine.check.ts`
 * le simula.
 */

/** SVG di una pagina con l'hash della pagina effettivamente serializzata (`PaginaSvg`). */
export interface PaginaSvg {
  hash: string;
  svg: string;
}

/** Stato di una pagina dell'anteprima. */
export interface StatoPagina {
  /** Hash atteso, dall'ultima compilazione. */
  atteso: string;
  /** Hash dell'SVG oggi nel DOM, `null` se la pagina non è ancora stata caricata. */
  caricato: string | null;
  /** `true` se la pagina è nella zona di precaricamento. */
  visibile: boolean;
  /** `true` se una richiesta di SVG è in volo. */
  inVolo: boolean;
}

/**
 * Decide se serve (ri)chiedere l'SVG di una pagina: solo se è in zona di
 * precaricamento, non c'è già una richiesta in volo e ciò che è nel DOM non
 * corrisponde all'hash atteso (mai caricata, oppure contenuto cambiato).
 */
export function haBisognoDiSvg(s: Readonly<StatoPagina>): boolean {
  return s.visibile && !s.inVolo && s.caricato !== s.atteso;
}

/**
 * `true` se TUTTE le pagine hanno nel DOM l'SVG corrispondente all'hash atteso
 * (serve alla stampa, che non può aspettare che le pagine entrino in vista).
 */
export function tuttePagineCaricate(stato: readonly Readonly<StatoPagina>[]): boolean {
  return stato.length > 0 && stato.every((s) => s.caricato === s.atteso);
}

/** Dipendenze (DOM e IPC) del caricatore. */
export interface DipendenzeCaricatore {
  chiediSvg: (indice: number) => Promise<PaginaSvg | null>;
  /** Inserisce l'SVG nel DOM della pagina `indice`. */
  inserisci: (indice: number, pagina: PaginaSvg) => void;
  /** Svuota il DOM della pagina `indice` (dopo la reimpostazione delle definizioni). */
  svuotaPagina: (indice: number) => void;
  /** Svuota il contenitore delle definizioni condivise. */
  svuotaDefs: () => void;
  /** Numero di definizioni oggi nel contenitore condiviso. */
  numeroDefs: () => number;
  /** Oltre questo numero di definizioni si reimposta il contenitore. */
  limiteDefs: number;
  onPaginaCaricata: (indice: number) => void;
  onErrore: (errore: unknown) => void;
}

export interface CaricatorePagine {
  /** Stato delle pagine (indice = numero di pagina); il chiamante aggiunge/toglie pagine. */
  readonly stato: StatoPagina[];
  /** Richiede l'SVG della pagina `i` se serve. */
  carica(i: number): void;
}

export function creaCaricatorePagine(dip: DipendenzeCaricatore): CaricatorePagine {
  const stato: StatoPagina[] = [];

  /** Svuota le definizioni e tutte le pagine tranne `salta`: le pagine in vista si ricaricano. */
  function reimpostaDefs(salta: number): void {
    dip.svuotaDefs();
    stato.forEach((st, j) => {
      if (j !== salta) {
        st.caricato = null;
        dip.svuotaPagina(j);
      }
    });
  }

  function carica(i: number): void {
    const s = stato[i];
    if (s === undefined || !haBisognoDiSvg(s)) {
      return;
    }
    s.inVolo = true;
    const attesoAllaRichiesta = s.atteso;
    /**
     * Se un `aggiorna` ha cambiato l'hash atteso mentre la richiesta era in volo
     * (e la pagina è ancora la stessa) si richiede di nuovo: vale anche dopo un
     * errore o un `null`, altrimenti la pagina resterebbe stantia fino al
     * prossimo `aggiorna`. Se l'atteso non è cambiato non si riprova qui (niente
     * cicli su un errore persistente): ripartirà da `aggiorna` o dall'osservatore.
     */
    const riprovaSeCambiato = (): void => {
      if (stato[i] === s && s.atteso !== attesoAllaRichiesta) {
        carica(i);
      }
    };
    void dip
      .chiediSvg(i)
      .then((pagina) => {
        s.inVolo = false;
        // La pagina può essere stata rimossa o sostituita da un `aggiorna` nel frattempo.
        if (stato[i] !== s) {
          return;
        }
        if (pagina === null) {
          riprovaSeCambiato();
          return;
        }
        const reimposta = dip.numeroDefs() > dip.limiteDefs;
        if (reimposta) {
          reimpostaDefs(i);
        }
        dip.inserisci(i, pagina);
        s.caricato = pagina.hash;
        dip.onPaginaCaricata(i);
        if (reimposta) {
          stato.forEach((_, j) => carica(j));
        }
        // Se invece l'SVG ricevuto è più recente dell'atteso (compilazione già
        // finita, risultato non ancora applicato) si aspetta `aggiorna`.
        riprovaSeCambiato();
      })
      .catch((e: unknown) => {
        s.inVolo = false;
        dip.onErrore(e);
        riprovaSeCambiato();
      });
  }

  return { stato, carica };
}
