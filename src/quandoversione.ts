/**
 * quandoversione.ts — Formattazione dell'istante di una versione della cronologia.
 *
 * Il backend fornisce `quando` (ora locale già formattata, `AAAA-MM-GG HH:MM:SS`)
 * e, dove disponibile, `quando_utc` (istante UTC in RFC 3339). Il campo UTC si
 * formatta qui con `Intl.DateTimeFormat` nella lingua dell'interfaccia e nel
 * fuso del sistema; se manca o non è valido si ripiega su `quando`.
 */

/** Campi di una versione che servono alla formattazione. */
export interface QuandoVersione {
  quando: string;
  quando_utc?: string | null;
}

/**
 * Testo da mostrare per l'istante della versione.
 *
 * @param v      versione (campo additivo `quando_utc` facoltativo).
 * @param lingua tag BCP 47 dell'interfaccia (es. `it`); vuoto/non valido = predefinita del sistema.
 */
export function formattaQuando(v: QuandoVersione, lingua: string): string {
  const utc = v.quando_utc;
  if (typeof utc !== "string" || utc === "") {
    return v.quando;
  }
  const data = new Date(utc);
  if (Number.isNaN(data.getTime())) {
    return v.quando;
  }
  const opzioni: Intl.DateTimeFormatOptions = { dateStyle: "medium", timeStyle: "medium" };
  try {
    return new Intl.DateTimeFormat(lingua === "" ? undefined : lingua, opzioni).format(data);
  } catch {
    // Tag di lingua non valido: lingua predefinita del sistema.
    return new Intl.DateTimeFormat(undefined, opzioni).format(data);
  }
}
