/**
 * impostazioni.ts — Impostazioni utente persistenti (F5-003).
 *
 * Legge e salva `impostazioni.json` tramite i comandi `leggi_impostazioni` /
 * `salva_impostazioni` (il file vive nella config dir dell'OS, lato Rust).
 * Qui c'è solo la cache in memoria e l'accesso; tema e font li applica aspetto.ts.
 */
import { invoke } from "@tauri-apps/api/core";
import { STILE_PREDEFINITO, normalizzaStile, type SceltaStile } from "./stile";
import { MOTORE_PREDEFINITO, normalizzaMotore, type MotoreTypst } from "./motore";

/** Impostazioni utente, nel formato del core (`Impostazioni` in config.rs). */
export interface Impostazioni {
  /** Tema dell'interfaccia. */
  tema: "sistema" | "chiaro" | "scuro";
  /** Dimensione del font dell'editor, in pixel. */
  dimensione_font: number;
  /** Ritardo (ms) fra l'ultima modifica e la compilazione automatica. */
  debounce_ms: number;
  /** Salvataggio automatico dei file aperti. */
  autosave: boolean;
  /** Ritardo (ms) fra l'ultima modifica e il salvataggio automatico (F5-013). */
  autosave_ms: number;
  /** Numero massimo di versioni per file nella cronologia locale (F5-013). */
  cronologia_max_versioni: number;
  /** Intervallo minimo (s) fra due versioni create dal salvataggio automatico (F5-013). */
  cronologia_intervallo_s: number;
  /** Ultimo standard scelto nell'export PDF: `nessuno`, `a-2b`, `a-3b` o `a-4` (F5-011). */
  standard_pdf: string;
  /** Barra laterale visibile (F5-019). */
  barra_visibile: boolean;
  /** Larghezza della barra laterale in pixel (F5-019). */
  barra_larghezza: number;
  /** Identificatore del pannello attivo della barra (F5-019). */
  pannello_attivo: string;
  /** Modalità focus: schiarisce tutto tranne il paragrafo del cursore (F5-023). */
  modalita_focus: boolean;
  /** Modalità macchina da scrivere: la riga del cursore resta a metà altezza (F5-023). */
  modalita_macchina: boolean;
  /** Stile grafico: `classico` (predefinito), `composizione` o `strumento`. */
  stile: SceltaStile;
  /** Motore di evidenziazione Typst: `lezer` (predefinito) o `wasm`. Assente nei backend vecchi. */
  motore_typst: MotoreTypst;
  /** Documentazione accanto alle voci di completamento e tipo dei parametri. */
  help_completamento: boolean;
  /** Firma della funzione mentre si scrivono gli argomenti. */
  suggerimento_parametri: boolean;
}

/** Valori di ripiego, allineati a `Impostazioni::default()` del core. */
export const PREDEFINITE: Readonly<Impostazioni> = {
  tema: "sistema",
  dimensione_font: 14,
  debounce_ms: 300,
  autosave: false,
  autosave_ms: 2000,
  cronologia_max_versioni: 50,
  cronologia_intervallo_s: 300,
  standard_pdf: "nessuno",
  barra_visibile: true,
  barra_larghezza: 260,
  pannello_attivo: "indice",
  modalita_focus: false,
  modalita_macchina: false,
  stile: STILE_PREDEFINITO,
  motore_typst: MOTORE_PREDEFINITO,
  help_completamento: true,
  suggerimento_parametri: true,
};

let correnti: Impostazioni = { ...PREDEFINITE };

/** Promise del caricamento iniziale: i salvataggi la attendono per non sovrascrivere il file con i predefiniti. */
let pronte: Promise<Impostazioni> | null = null;

/** Coda che serializza le scritture, così l'ultima richiesta è l'ultima su disco. */
let coda: Promise<unknown> = Promise.resolve();

/**
 * Carica le impostazioni dal backend (una sola volta: le chiamate successive
 * riusano la stessa promise). In caso di errore mantiene quelle correnti
 * (predefinite al primo avvio): l'assenza non blocca l'editor.
 */
export function caricaImpostazioni(): Promise<Impostazioni> {
  if (pronte === null) {
    pronte = (async () => {
      try {
        correnti = { ...PREDEFINITE, ...(await invoke<Impostazioni>("leggi_impostazioni")) };
        // Un backend senza il campo (o con un valore sconosciuto) ricade sul classico.
        correnti.stile = normalizzaStile(correnti.stile);
        // Idem per il motore: assente o sconosciuto ricade su lezer.
        correnti.motore_typst = normalizzaMotore(correnti.motore_typst);
      } catch (e) {
        console.warn("Impostazioni non caricate, uso i predefiniti:", e);
      }
      return { ...correnti };
    })();
  }
  return pronte;
}

/** Restituisce una copia delle impostazioni correnti (senza I/O). */
export function leggiImpostazioni(): Impostazioni {
  return { ...correnti };
}

/**
 * Applica una modifica parziale e la salva su disco. Attende il caricamento
 * iniziale, aggiorna la cache in modo sincrono (nessun aggiornamento perso fra
 * chiamate ravvicinate) e serializza le scritture; l'errore di scrittura è
 * propagato al chiamante.
 */
export async function salvaImpostazioni(modifica: Partial<Impostazioni>): Promise<void> {
  await caricaImpostazioni();
  correnti = { ...correnti, ...modifica };
  const istantanea = { ...correnti };
  coda = coda.catch(() => {}).then(() => invoke("salva_impostazioni", { impostazioni: istantanea }));
  await coda;
}
