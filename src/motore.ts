/**
 * motore.ts — Logica pura della scelta del motore di evidenziazione Typst (nessun DOM,
 * nessun CodeMirror).
 *
 * I motori sono due: «lezer» (predefinito: `typst_lezer()`, parser JavaScript nativo, senza
 * WASM) e «wasm» (`typst()` di `codemirror-lang-typst`, evidenziatore incrementale nel modulo
 * WASM di typst-syntax). Cambia solo l'evidenziazione/modifica dei file Typst, non la
 * compilazione. Qui stanno validazione del valore salvato, etichette, descrizioni e decisione
 * di persistenza; applicare il motore all'editor spetta a `aspetto.ts`/`editor.ts`.
 * Verifica: `motore.check.ts`.
 */

/** Identificatori ammessi, nello stesso formato del campo `motore_typst` del core. */
export const MOTORI = ["lezer", "wasm"] as const;

/** Scelta del motore di evidenziazione Typst. */
export type MotoreTypst = (typeof MOTORI)[number];

/** Motore predefinito (anche per file di impostazioni vecchi o valori sconosciuti). */
export const MOTORE_PREDEFINITO: MotoreTypst = "lezer";

const ETICHETTE: Record<MotoreTypst, string> = {
  lezer: "Lezer (predefinito)",
  wasm: "WASM",
};

const DESCRIZIONI: Record<MotoreTypst, string> = {
  wasm: "Evidenziatore incrementale nel modulo WASM di typst-syntax",
  lezer:
    "Parser JavaScript senza WASM, con indentazione, piegatura, completamento dei built-in e controllo sintattico",
};

/** Titoli dei comandi della palette (`motore.wasm`, `motore.lezer`). */
const TITOLI_COMANDO: Record<MotoreTypst, string> = {
  wasm: "Motore Typst: WASM",
  lezer: "Motore Typst: Lezer",
};

/** Vero se il valore è uno dei motori noti. */
export function motoreValido(valore: unknown): valore is MotoreTypst {
  return typeof valore === "string" && (MOTORI as readonly string[]).includes(valore);
}

/** Riporta un valore qualunque (letto da disco o dal backend) a un motore noto. */
export function normalizzaMotore(valore: unknown): MotoreTypst {
  return motoreValido(valore) ? valore : MOTORE_PREDEFINITO;
}

/** Etichetta leggibile per il radio del menu. */
export function etichettaMotore(motore: MotoreTypst): string {
  return ETICHETTE[motore];
}

/** Descrizione breve per il radio del menu. */
export function descrizioneMotore(motore: MotoreTypst): string {
  return DESCRIZIONI[motore];
}

/** Titolo del comando della palette. */
export function titoloComandoMotore(motore: MotoreTypst): string {
  return TITOLI_COMANDO[motore];
}

/**
 * Se `impostaMotore` deve scrivere su disco: solo se il valore (normalizzato) differisce da
 * quello risultante su disco, oppure se le impostazioni non sono ancora state caricate (il
 * valore salvato arriverebbe dopo e la scelta resterebbe solo in memoria).
 *
 * @param salvato  motore che risulta scritto su disco (letto o salvato con successo).
 */
export function devePersistereMotore(
  salvato: MotoreTypst,
  scelta: unknown,
  impostazioniCaricate: boolean,
): boolean {
  return !impostazioniCaricate || normalizzaMotore(scelta) !== salvato;
}
