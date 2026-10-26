/**
 * indice-logica.ts — Logica pura del pannello Indice (F5-020).
 *
 * Senza DOM né Tauri (verificata da `pannelli.check.ts`): a partire dalle voci
 * calcolate dal core (`VoceIndice`) decide quale titolo è quello corrente e
 * dove portare il cursore quando si sceglie una voce. Per i documenti Markdown
 * il core conosce solo il Typst generato dalla conversione: le posizioni nel
 * sorgente `.md` si ricavano qui abbinando i titoli per testo (D-015, "da
 * verificare nell'implementazione").
 */

/** Una voce dell'indice, nel formato del core (`VoceIndice` in `indice.rs`). */
export interface VoceIndice {
  /** Livello del titolo (`=` → 1). */
  livello: number;
  /** Profondità nell'albero (0 = radice); differisce dal livello se un livello è saltato. */
  profondita: number;
  /** Testo del titolo in chiaro. */
  testo: string;
  /** `true` se il titolo è nel file principale del documento compilato. */
  principale: boolean;
  /** File (relativo alla radice, con `/`), o `null` se non navigabile. */
  file: string | null;
  riga: number | null;
  colonna: number | null;
  /** Offset UTF-16 nel file della voce. */
  offset: number | null;
  /** Pagina (1-based) del titolo nel documento compilato. */
  pagina: number;
}

/** Dove portare l'utente scegliendo una voce. */
export type Destinazione =
  | { tipo: "cursore"; offset: number }
  | { tipo: "apri-file"; file: string; offset: number | null }
  | { tipo: "pagina"; pagina: number };

/**
 * Forma confrontabile di un titolo: senza marcatori di enfasi/codice/link né
 * escape, con gli spazi ridotti e in minuscolo. Serve ad abbinare il testo in
 * chiaro del compilatore a quello scritto nel sorgente Markdown.
 */
export function normalizzaTitolo(testo: string): string {
  return testo
    .replace(/[*_`~\\#[\]()<>]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

/** Un titolo trovato nel sorgente Markdown. */
export interface TitoloMarkdown {
  /** Testo normalizzato (vedi [`normalizzaTitolo`]). */
  testo: string;
  /** Offset UTF-16 dell'inizio della riga del titolo. */
  offset: number;
}

/**
 * Titoli ATX (`# Titolo`) di un sorgente Markdown, in ordine, ignorando i blocchi
 * di codice recintati (tre o più apici inversi, o tre o più tilde). Non
 * riconosce i titoli sottolineati (setext).
 */
export function titoliMarkdown(testo: string): TitoloMarkdown[] {
  const titoli: TitoloMarkdown[] = [];
  let recinto: string | null = null;
  let inizio = 0;
  while (inizio <= testo.length) {
    let fine = testo.indexOf("\n", inizio);
    if (fine === -1) {
      fine = testo.length;
    }
    const riga = testo.slice(inizio, fine).replace(/\r$/, "");
    const apertura = /^ {0,3}(`{3,}|~{3,})/.exec(riga);
    if (recinto !== null) {
      if (apertura !== null && apertura[1][0] === recinto[0] && apertura[1].length >= recinto.length) {
        recinto = null;
      }
    } else if (apertura !== null) {
      recinto = apertura[1];
    } else {
      const m = /^ {0,3}#{1,6}[ \t]+(.*?)(?:[ \t]+#+)?[ \t]*$/.exec(riga);
      if (m !== null && m[1] !== "") {
        titoli.push({ testo: normalizzaTitolo(m[1]), offset: inizio });
      }
    }
    inizio = fine + 1;
  }
  return titoli;
}

/**
 * Offset nel sorgente Markdown di ogni voce dell'indice, abbinando i titoli per
 * testo: l'n-esima voce con un dato titolo corrisponde all'n-esimo titolo
 * Markdown con lo stesso testo. `null` se non si trova (titolo generato dalla
 * conversione o non riconosciuto).
 */
export function offsetPerMarkdown(testo: string, voci: readonly VoceIndice[]): (number | null)[] {
  const trovati = new Map<string, number[]>();
  for (const t of titoliMarkdown(testo)) {
    const lista = trovati.get(t.testo) ?? [];
    lista.push(t.offset);
    trovati.set(t.testo, lista);
  }
  const usati = new Map<string, number>();
  return voci.map((v) => {
    const chiave = normalizzaTitolo(v.testo);
    const n = usati.get(chiave) ?? 0;
    usati.set(chiave, n + 1);
    return trovati.get(chiave)?.[n] ?? null;
  });
}

/**
 * Offset di ogni voce nel **documento aperto nell'editor**: per Typst quello
 * del core (solo per le voci del file principale, che è ciò che l'editor
 * mostra); per Markdown quello ricavato dal testo. `null` per le altre.
 */
export function offsetNelDocumento(
  voci: readonly VoceIndice[],
  markdown: boolean,
  testoEditor: string,
): (number | null)[] {
  if (markdown) {
    return offsetPerMarkdown(testoEditor, voci);
  }
  return voci.map((v) => (v.principale ? v.offset : null));
}

/**
 * Indice della voce "corrente": l'ultima con offset ≤ cursore, fra quelle che
 * hanno un offset nel documento aperto. `-1` se il cursore precede il primo
 * titolo o nessuna voce è nel documento.
 */
export function voceCorrente(offsets: readonly (number | null)[], cursore: number): number {
  let corrente = -1;
  let migliore = -1;
  offsets.forEach((o, i) => {
    // A parità di offset vince l'ultima; gli offset non sono ordinati fra file diversi.
    if (o !== null && o <= cursore && o >= migliore) {
      migliore = o;
      corrente = i;
    }
  });
  return corrente;
}

/**
 * Destinazione della scelta di una voce.
 *
 * - titolo nel documento aperto con offset noto → cursore lì (la sincronizzazione
 *   sposta poi l'anteprima);
 * - titolo in un altro file del progetto → apri quel file, poi cursore;
 * - altrimenti (pacchetto, titolo non trovabile in un `.md`) → solo la pagina
 *   dell'anteprima.
 */
export function destinazione(voce: VoceIndice, offsetNelDoc: number | null): Destinazione {
  if (offsetNelDoc !== null) {
    return { tipo: "cursore", offset: offsetNelDoc };
  }
  if (!voce.principale && voce.file !== null) {
    return { tipo: "apri-file", file: voce.file, offset: voce.offset };
  }
  return { tipo: "pagina", pagina: voce.pagina };
}

/**
 * Dimensione e posizione di ogni voce fra i suoi fratelli (`aria-setsize` e
 * `aria-posinset`): l'albero è reso "piatto" (righe non annidate nel DOM), quindi
 * queste informazioni vanno dichiarate. I fratelli sono le voci con lo stesso
 * padre, cioè con la stessa profondità e nessuna voce meno profonda in mezzo.
 */
export function posizioniFratelli(
  voci: readonly { profondita: number }[],
): { dimensione: number; posizione: number }[] {
  // Per ogni profondità, il gruppo di fratelli ancora aperto (contiene le voci già viste).
  const aperti: number[][] = [];
  const gruppoDi: number[][] = [];
  const posizioni: number[] = [];
  voci.forEach((v, i) => {
    const d = v.profondita;
    // Una voce meno profonda chiude i gruppi più profondi (e il proprio se è un nuovo padre).
    aperti.length = Math.min(aperti.length, d + 1);
    aperti[d] ??= [];
    aperti[d].push(i);
    gruppoDi[i] = aperti[d];
    posizioni[i] = aperti[d].length;
  });
  return voci.map((_, i) => ({ dimensione: gruppoDi[i].length, posizione: posizioni[i] }));
}
