/**
 * file-logica.ts — Logica pura del pannello File (F5-021).
 *
 * Senza DOM né Tauri (verificata da `pannelli.check.ts`): trasforma l'albero
 * restituito dal core in righe visibili (in base alle cartelle aperte) e ritrova
 * il percorso relativo di un file assoluto.
 */

import { normalizzaPercorso } from "../modificheesterne.ts";

/** Tipo di una voce (`TipoNodo` in `albero.rs`). */
export type TipoNodo = "cartella" | "documento" | "risorsa";

/** Una voce dell'albero, nel formato del core (`NodoFile`). */
export interface NodoFile {
  nome: string;
  /** Percorso relativo alla radice, con `/`. */
  percorso: string;
  tipo: TipoNodo;
  figli: NodoFile[];
}

/** Albero dei file (`AlberoFile` del core). */
export interface AlberoFile {
  /** Percorso assoluto della radice. */
  radice: string;
  nodi: NodoFile[];
  /** `true` se l'elenco è stato interrotto per i limiti del core. */
  troncato: boolean;
}

/** Riga visibile dell'albero. */
export interface RigaFile {
  nodo: NodoFile;
  /** Livello di annidamento (0 = radice). */
  livello: number;
  /** `true` se è una cartella aperta. */
  aperta: boolean;
}

/** Righe visibili: i figli di una cartella compaiono solo se è in `aperte`. */
export function righeVisibili(nodi: readonly NodoFile[], aperte: ReadonlySet<string>): RigaFile[] {
  const righe: RigaFile[] = [];
  const visita = (lista: readonly NodoFile[], livello: number): void => {
    for (const nodo of lista) {
      const aperta = nodo.tipo === "cartella" && aperte.has(nodo.percorso);
      righe.push({ nodo, livello, aperta });
      if (aperta) {
        visita(nodo.figli, livello + 1);
      }
    }
  };
  visita(nodi, 0);
  return righe;
}

/** Cartelle antenate di un percorso relativo (`a/b/c.typ` → `a`, `a/b`). */
export function cartelleAntenate(percorso: string): string[] {
  const segmenti = percorso.split("/");
  const antenate: string[] = [];
  for (let i = 1; i < segmenti.length; i++) {
    antenate.push(segmenti.slice(0, i).join("/"));
  }
  return antenate;
}

/**
 * Percorso di `assoluto` relativo a `radice` (con `/`, **normalizzato**: in
 * minuscolo, senza prefisso `\\?\`), o `null` se non sta sotto la radice. Va
 * confrontato con `normalizzaPercorso` del percorso relativo del nodo.
 */
export function percorsoRelativo(radice: string, assoluto: string): string | null {
  const r = normalizzaPercorso(radice);
  const a = normalizzaPercorso(assoluto);
  if (a.length <= r.length || !a.startsWith(r + "/")) {
    return null;
  }
  return a.slice(r.length + 1);
}

/** `true` se il nodo `relativo` è il documento aperto `assolutoAperto`. */
export function eIlDocumento(radice: string, relativo: string, assolutoAperto: string | null): boolean {
  if (assolutoAperto === null) {
    return false;
  }
  return percorsoRelativo(radice, assolutoAperto) === normalizzaPercorso(relativo);
}

/**
 * Cartelle da aprire perché il documento aperto sia visibile nell'albero:
 * gli antenati del suo percorso, riconosciuti sui nodi reali (il percorso
 * relativo normalizzato è in minuscolo, i nodi conservano le maiuscole).
 */
export function cartelleDaAprire(nodi: readonly NodoFile[], radice: string, assolutoAperto: string | null): string[] {
  if (assolutoAperto === null) {
    return [];
  }
  const relativo = percorsoRelativo(radice, assolutoAperto);
  if (relativo === null) {
    return [];
  }
  const antenati = new Set(cartelleAntenate(relativo));
  const trovate: string[] = [];
  const visita = (lista: readonly NodoFile[]): void => {
    for (const n of lista) {
      if (n.tipo === "cartella" && antenati.has(normalizzaPercorso(n.percorso))) {
        trovate.push(n.percorso);
        visita(n.figli);
      }
    }
  };
  visita(nodi);
  return trovate;
}
