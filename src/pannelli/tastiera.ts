/**
 * tastiera.ts — Navigazione da tastiera dei componenti della barra laterale (F5-019/020/021).
 *
 * Logica pura, senza DOM (verificata da `pannelli.check.ts`): dato lo stato di
 * una lista o di un albero e il tasto premuto, dice cosa fare. I componenti
 * (activity bar, indice, albero dei file) applicano il risultato al DOM con la
 * tecnica del *roving tabindex* dei pattern WAI-ARIA (tablist, tree).
 */

/** Riga visibile di un albero, per la navigazione. */
export interface ElementoAlbero {
  /** Livello di annidamento (0 = radice). */
  livello: number;
  /** `true` se è una cartella (può aprirsi e chiudersi). */
  eCartella: boolean;
  /** `true` se la cartella è aperta. */
  aperta: boolean;
}

/** Azione richiesta da un tasto su un albero. */
export type Movimento =
  | { tipo: "vai"; indice: number }
  | { tipo: "espandi"; indice: number }
  | { tipo: "comprimi"; indice: number }
  | { tipo: "attiva"; indice: number };

/**
 * Nuovo indice attivo in una lista di `totale` elementi (frecce, Home, End).
 * `verticale` sceglie le frecce su/giù, altrimenti sinistra/destra. `null` se il
 * tasto non riguarda la navigazione o la lista è vuota. Non si torna a capo: ai
 * bordi resta l'elemento estremo.
 */
export function naviga(
  totale: number,
  corrente: number,
  tasto: string,
  verticale = true,
): number | null {
  if (totale <= 0) {
    return null;
  }
  const massimo = totale - 1;
  const base = Math.min(Math.max(corrente, 0), massimo);
  const [indietro, avanti] = verticale ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
  switch (tasto) {
    case indietro:
      return Math.max(base - 1, 0);
    case avanti:
      return Math.min(base + 1, massimo);
    case "Home":
      return 0;
    case "End":
      return massimo;
    default:
      return null;
  }
}

/**
 * Azione per un tasto su un albero (pattern WAI-ARIA "tree"): su/giù/Home/End
 * spostano il fuoco fra le righe visibili; destra apre una cartella chiusa o
 * scende al primo figlio; sinistra chiude una cartella aperta o sale al padre;
 * Invio e Spazio attivano la riga (apre il file o alterna la cartella).
 */
export function navigaAlbero(
  elementi: readonly ElementoAlbero[],
  corrente: number,
  tasto: string,
): Movimento | null {
  if (elementi.length === 0 || corrente < 0 || corrente >= elementi.length) {
    return null;
  }
  const riga = elementi[corrente];
  switch (tasto) {
    case "ArrowUp":
    case "ArrowDown":
    case "Home":
    case "End": {
      const indice = naviga(elementi.length, corrente, tasto);
      return indice === null ? null : { tipo: "vai", indice };
    }
    case "ArrowRight":
      if (!riga.eCartella) {
        return null;
      }
      if (!riga.aperta) {
        return { tipo: "espandi", indice: corrente };
      }
      // Cartella aperta: il primo figlio è la riga successiva, se più profonda.
      return corrente + 1 < elementi.length && elementi[corrente + 1].livello > riga.livello
        ? { tipo: "vai", indice: corrente + 1 }
        : null;
    case "ArrowLeft": {
      if (riga.eCartella && riga.aperta) {
        return { tipo: "comprimi", indice: corrente };
      }
      for (let i = corrente - 1; i >= 0; i--) {
        if (elementi[i].livello < riga.livello) {
          return { tipo: "vai", indice: i };
        }
      }
      return null;
    }
    case "Enter":
    case " ":
      return { tipo: "attiva", indice: corrente };
    default:
      return null;
  }
}
