/**
 * salvataggio.ts — Logica pura del pallino di salvataggio accanto al nome del file.
 * Nessun DOM: testata in `salvataggio.check.ts`.
 *
 * Stati (forma diversa per ciascuno, oltre al colore):
 * - `salvato`     verde, cerchio pieno;
 * - `in-corso`    ambra, cerchio con anello e punto (scrittura su disco in volo);
 * - `non-salvato` rosso, rombo (modifiche non salvate, o ultima scrittura fallita);
 * - `nuovo`       neutro, anello vuoto (documento senza file e senza modifiche).
 */

export type StatoSalvataggio = "salvato" | "in-corso" | "non-salvato" | "nuovo";

/** Ingressi della funzione di stato. */
export interface IngressiSalvataggio {
  /** Modifiche non salvate rispetto al disco (flag `modificato` del gestore file). */
  modificato: boolean;
  /** Una scrittura su disco è in volo (Ctrl+S o autosave). */
  inSalvataggio: boolean;
  /** L'ultima scrittura conclusa è fallita. */
  errore: boolean;
  /** Il documento è collegato a un file su disco. */
  haPercorso: boolean;
}

/**
 * Stato del pallino. Precedenza: scrittura in volo (ambra, anche dopo un errore: è un nuovo
 * tentativo) > errore > modificato > documento senza file ancora intatto (neutro) > salvato.
 *
 * Regola per il documento senza file: neutro finché è intatto (appena creato, vuoto); appena
 * ha modifiche è rosso, perché chiudere l'app le perderebbe.
 */
export function statoSalvataggio(i: IngressiSalvataggio): StatoSalvataggio {
  if (i.inSalvataggio) return "in-corso";
  if (i.errore || i.modificato) return "non-salvato";
  if (!i.haPercorso) return "nuovo";
  return "salvato";
}

/** Testo (titolo e nome accessibile) di ciascuno stato. */
export function titoloSalvataggio(s: StatoSalvataggio): string {
  switch (s) {
    case "salvato":
      return "Salvato";
    case "in-corso":
      return "Salvataggio in corso";
    case "non-salvato":
      return "Modifiche non salvate";
    case "nuovo":
      return "Nuovo documento, non ancora salvato";
  }
}

/** Durata minima di visibilità dell'ambra, in ms (evita il lampeggio). */
export const AMBRA_MIN_MS = 400;

/** Stato mostrato e istante in cui è stato mostrato. */
export interface Mostrato {
  stato: StatoSalvataggio;
  dal: number;
}

/** Esito di un aggiornamento: nuovo stato mostrato e, se serve, attesa prima di rivalutare. */
export interface Avanzamento {
  mostrato: Mostrato;
  /** ms dopo i quali richiamare `avanza` con lo stato voluto più recente; `null` = nessuna attesa. */
  attesaMs: number | null;
}

/**
 * Decide cosa mostrare dato lo stato voluto adesso. Lo stato ambra resta visibile almeno
 * `minimo` ms: se lo stato voluto cambia prima, si trattiene l'ambra e si chiede di rivalutare
 * allo scadere (con l'ultimo stato voluto, quindi cambi intermedi non lampeggiano).
 */
export function avanza(
  mostrato: Mostrato | null,
  voluto: StatoSalvataggio,
  ora: number,
  minimo: number = AMBRA_MIN_MS,
): Avanzamento {
  if (mostrato === null || mostrato.stato === voluto) {
    return { mostrato: mostrato ?? { stato: voluto, dal: ora }, attesaMs: null };
  }
  if (mostrato.stato === "in-corso") {
    const trascorso = ora - mostrato.dal;
    if (trascorso < minimo) {
      return { mostrato, attesaMs: Math.max(0, minimo - trascorso) };
    }
  }
  return { mostrato: { stato: voluto, dal: ora }, attesaMs: null };
}

/**
 * `true` se mostrare `dopo` merita un annuncio vocale: solo gli esiti (salvato / non salvato),
 * e solo se differiscono dall'ULTIMO ANNUNCIATO (`null` = nessuno ancora, il primo disegno
 * non si annuncia). Così l'ambra transitoria e i cicli autosave durante la digitazione
 * (non salvato, ambra, non salvato) non producono raffiche.
 */
export function annunciare(ultimoAnnunciato: StatoSalvataggio | null, dopo: StatoSalvataggio): boolean {
  if (dopo !== "salvato" && dopo !== "non-salvato") return false;
  return ultimoAnnunciato !== dopo;
}
