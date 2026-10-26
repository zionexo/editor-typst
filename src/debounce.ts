/**
 * debounce.ts — Ritardo adattivo della compilazione automatica (F5-010).
 *
 * Logica pura, senza DOM né Tauri (verificata da `debounce.check.ts`). Il
 * valore base è `debounce_ms` delle impostazioni (F5-003); il ritardo effettivo
 * si adatta alla durata dell'ultima compilazione, così un documento pesante non
 * accumula compilazioni consecutive mentre uno leggero risponde più in fretta.
 */

/** Pavimento assoluto (ms): coincide col minimo accettato dal core per `debounce_ms`. */
export const RITARDO_MIN_ASSOLUTO = 50;

/**
 * Calcola il ritardo (ms) fra l'ultima modifica e l'avvio della compilazione.
 *
 * `ritardo = clamp(base/2 + durata, base/2, 2*base)`: a riposo mezza base
 * (150 ms con la base predefinita di 300), più la durata dell'ultima
 * compilazione perché il compilatore non occupi mai più di circa metà del
 * tempo, con tetto a due basi (600 ms). Senza misura (`null`, prima compilazione
 * dopo l'avvio) vale la base. Input non finiti o negativi
 * sono ripiegati su valori sicuri: nessun NaN arriva a `setTimeout`.
 *
 * @param baseMs   Valore base configurato (`debounce_ms`).
 * @param durataMs Durata dell'ultima compilazione tornata dal backend, se nota.
 */
export function ritardoDebounce(baseMs: number, durataMs: number | null): number {
  const base = Number.isFinite(baseMs) ? Math.max(RITARDO_MIN_ASSOLUTO, baseMs) : 300;
  if (durataMs === null || !Number.isFinite(durataMs)) {
    return Math.round(base);
  }
  const pavimento = Math.max(RITARDO_MIN_ASSOLUTO, base / 2);
  const tetto = base * 2;
  return Math.round(Math.min(tetto, Math.max(pavimento, pavimento + Math.max(0, durataMs))));
}
