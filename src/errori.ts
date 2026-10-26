/**
 * errori.ts — Logica pura dei diagnostici di compilazione (nessun DOM, nessun CodeMirror):
 * posizione riga/colonna da un offset, ordinamento, riepilogo, testi del riquadro errori e dei
 * marcatori inline. Testata in `errori.check.ts`.
 *
 * I campi aggiunti dal core (`posizione_da_traccia`, `origine`, `traccia`, `suggerimenti`) sono
 * tutti OPZIONALI: con un core più vecchio il comportamento è quello di sempre.
 */

/** Gravità di un diagnostico. */
export type Gravita = "errore" | "avviso";

/** Forma di un diagnostico del compilatore, come arriva dal backend. */
export interface DiagnosticoBase {
  gravita: Gravita;
  messaggio: string;
  /** Offset UTF-16 nel sorgente principale, o `null` se non c'è una posizione nel main. */
  da: number | null;
  a: number | null;
  /** `true` se `da`/`a` sono il punto di CHIAMATA nel main (dalla traccia), non lo span reale. */
  posizione_da_traccia?: boolean;
  /** Dove l'errore è nato davvero (file dati o pacchetto), già in forma leggibile. */
  origine?: string | null;
  /** Passi della traccia di Typst (es. «chiamata della funzione `gantt` (riga 54)»). */
  traccia?: string[];
  /** Suggerimenti del compilatore (il `messaggio` li contiene già accodati). */
  suggerimenti?: string[];
}

/** Riga e colonna (1-based) di un offset nel testo. */
export interface Posizione {
  riga: number;
  colonna: number;
}

function eSurrogataAlta(c: number): boolean {
  return c >= 0xd800 && c <= 0xdbff;
}
function eSurrogataBassa(c: number): boolean {
  return c >= 0xdc00 && c <= 0xdfff;
}

/**
 * Converte un offset UTF-16 in riga/colonna (entrambe da 1). La colonna conta i caratteri
 * Unicode (un'emoji vale 1, non 2). Il testo dell'editor ha solo `\n` come fine riga.
 * Un offset oltre la fine è portato alla fine; negativo o non finito a 0; un offset che cade
 * a metà di una coppia surrogata è riportato all'inizio della coppia.
 */
export function posizioneDaOffset(testo: string, offset: number): Posizione {
  let fine = Number.isFinite(offset) ? Math.trunc(offset) : 0;
  fine = Math.max(0, Math.min(fine, testo.length));
  if (
    fine > 0 &&
    fine < testo.length &&
    eSurrogataBassa(testo.charCodeAt(fine)) &&
    eSurrogataAlta(testo.charCodeAt(fine - 1))
  ) {
    fine -= 1;
  }
  let riga = 1;
  let inizioRiga = 0;
  for (let i = 0; i < fine; i++) {
    if (testo.charCodeAt(i) === 10) {
      riga += 1;
      inizioRiga = i + 1;
    }
  }
  let colonna = 1;
  for (let i = inizioRiga; i < fine; i++) {
    colonna += 1;
    if (eSurrogataAlta(testo.charCodeAt(i)) && i + 1 < fine && eSurrogataBassa(testo.charCodeAt(i + 1))) {
      i += 1;
    }
  }
  return { riga, colonna };
}

/** Intervallo `da..a` navigabile nell'editor (già con `da <= a`, entro la lunghezza). */
export interface Intervallo {
  da: number;
  a: number;
}

/**
 * Intervallo dove portare il cursore per un diagnostico, o `null` se non ha posizione.
 * Con `lunghezza` l'intervallo è ristretto al documento corrente (che può essere cambiato dopo
 * la compilazione); senza, è quello grezzo.
 */
export function intervalloDiagnostico(
  d: Pick<DiagnosticoBase, "da" | "a">,
  lunghezza?: number,
): Intervallo | null {
  if (d.da === null || d.da === undefined || !Number.isFinite(d.da)) return null;
  const max = lunghezza ?? Number.MAX_SAFE_INTEGER;
  const da = Math.max(0, Math.min(Math.trunc(d.da), max));
  const aGrezzo = d.a === null || d.a === undefined || !Number.isFinite(d.a) ? da : Math.trunc(d.a);
  const a = Math.max(da, Math.min(aGrezzo, max));
  return { da, a };
}

/** Ordine: errori prima degli avvisi, poi con posizione, poi per posizione (stabile). */
export function ordinaDiagnostici<T extends DiagnosticoBase>(elenco: readonly T[]): T[] {
  return elenco
    .map((d, i) => ({ d, i }))
    .sort((x, y) => {
      const g = (x.d.gravita === "errore" ? 0 : 1) - (y.d.gravita === "errore" ? 0 : 1);
      if (g !== 0) return g;
      const px = intervalloDiagnostico(x.d);
      const py = intervalloDiagnostico(y.d);
      if (px === null && py !== null) return 1;
      if (px !== null && py === null) return -1;
      if (px !== null && py !== null && px.da !== py.da) return px.da - py.da;
      return x.i - y.i;
    })
    .map((e) => e.d);
}

/** «N errori, M avvisi» (singolare dove serve; le parti a zero si omettono). */
export function riepilogoDiagnostici(errori: number, avvisi: number): string {
  const parti: string[] = [];
  if (errori > 0) parti.push(errori === 1 ? "1 errore" : `${errori} errori`);
  if (avvisi > 0) parti.push(avvisi === 1 ? "1 avviso" : `${avvisi} avvisi`);
  return parti.join(", ");
}

/** Conta errori e avvisi di un elenco. */
export function contaDiagnostici(elenco: readonly DiagnosticoBase[]): { errori: number; avvisi: number } {
  let errori = 0;
  let avvisi = 0;
  for (const d of elenco) {
    if (d.gravita === "errore") errori += 1;
    else avvisi += 1;
  }
  return { errori, avvisi };
}

/** Nota mostrata quando un diagnostico non ha né posizione né origine. */
export const NOTA_SENZA_POSIZIONE =
  "Posizione non disponibile: controlla i file letti da questa chiamata (dati, pacchetti importati).";

/** Voce pronta da disegnare nel riquadro errori. */
export interface VoceErrore {
  gravita: Gravita;
  etichetta: string;
  messaggio: string;
  /** «riga R, colonna C», o `null` senza posizione. */
  posizione: string | null;
  /** `true` se la posizione è il punto di chiamata (da traccia). */
  puntoDiChiamata: boolean;
  /** Intervallo da selezionare nell'editor, o `null` (voce non cliccabile). */
  intervallo: Intervallo | null;
  /** Righe attenuate: «Origine: …», passi della traccia, «Suggerimento: …», nota. */
  dettagli: string[];
}

/** Suggerimenti non già presenti nel messaggio (il core li accoda al `messaggio`). */
export function suggerimentiNuovi(d: DiagnosticoBase): string[] {
  const visti = new Set<string>();
  const out: string[] = [];
  for (const s of d.suggerimenti ?? []) {
    const t = typeof s === "string" ? s.trim() : "";
    if (t === "" || visti.has(t) || d.messaggio.includes(t)) continue;
    visti.add(t);
    out.push(t);
  }
  return out;
}

/**
 * Descrive un diagnostico per il riquadro. `testo` = sorgente corrente dell'editor (per
 * riga/colonna); `senzaPosizione` ignora `da`/`a` (documenti Markdown: gli offset sono del
 * Typst generato, non del sorgente mostrato).
 */
export function descriviDiagnostico(
  d: DiagnosticoBase,
  testo: string,
  senzaPosizione = false,
): VoceErrore {
  const intervallo = senzaPosizione ? null : intervalloDiagnostico(d, testo.length);
  let posizione: string | null = null;
  if (intervallo !== null) {
    const p = posizioneDaOffset(testo, intervallo.da);
    posizione = `riga ${p.riga}, colonna ${p.colonna}`;
  }
  const origine = typeof d.origine === "string" && d.origine.trim() !== "" ? d.origine.trim() : null;
  const dettagli: string[] = [];
  if (origine !== null) dettagli.push(`Origine: ${origine}`);
  for (const t of d.traccia ?? []) {
    if (typeof t === "string" && t.trim() !== "") dettagli.push(t.trim());
  }
  for (const s of suggerimentiNuovi(d)) dettagli.push(`Suggerimento: ${s}`);
  if (intervallo === null && origine === null) dettagli.push(NOTA_SENZA_POSIZIONE);
  return {
    gravita: d.gravita,
    etichetta: d.gravita === "errore" ? "Errore" : "Avviso",
    messaggio: d.messaggio,
    posizione,
    puntoDiChiamata: intervallo !== null && d.posizione_da_traccia === true,
    intervallo,
    dettagli,
  };
}

/** Testo del marcatore inline (tooltip/gutter): messaggio più punto di chiamata e origine. */
export function messaggioMarcatore(d: DiagnosticoBase): string {
  const righe = [d.messaggio];
  const origine = typeof d.origine === "string" ? d.origine.trim() : "";
  if (d.posizione_da_traccia === true) righe.push("(punto di chiamata)");
  if (origine !== "") righe.push(`Origine: ${origine}`);
  return righe.join("\n");
}
