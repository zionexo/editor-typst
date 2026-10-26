/**
 * strumenti-logica.ts — Logica pura del pannello «Strumenti» (Modelli).
 *
 * Nessun DOM e nessun Tauri: valori iniziali del modulo, conversione dei valori
 * per il core, date in italiano, ricerca, e la trasformazione di una selezione
 * in campo di un modello («Salva come modello»). Verificata da
 * `pannelli.check.ts`-style in `strumenti.check.ts`.
 */

/** Tipi di campo ammessi dal core (`modelli.rs`). */
export type TipoCampo = "testo" | "data" | "numero" | "elenco";

export const TIPI_CAMPO: readonly TipoCampo[] = ["testo", "data", "numero", "elenco"];

/** Un campo di un modello, come lo restituisce il core. */
export interface CampoModello {
  nome: string;
  tipo: TipoCampo;
  /** Valore predefinito; per `data` può essere `oggi`. Assente = vuoto. */
  predefinito?: string | null;
}

/** Un modello elencato dal core. */
export interface ModelloInfo {
  id: string;
  nome: string;
  descrizione?: string | null;
  pagine?: number | null;
  campi: CampoModello[];
  origine: "integrato" | "utente";
  /** Avvisi emersi leggendo l'intestazione del file (righe mal formate, ecc.). */
  avvisi?: string[];
}

const MESI = [
  "gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno",
  "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre",
];

/** Data locale `AAAA-MM-GG` (quella che vuole `<input type="date">`). */
export function isoOggi(d: Date = new Date()): string {
  const due = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${due(d.getMonth() + 1)}-${due(d.getDate())}`;
}

/** `2026-10-16` → `16 ottobre 2026`; stringa vuota se non è una data valida. */
export function dataEstesa(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso.trim());
  if (m === null) {
    return "";
  }
  const [anno, mese, giorno] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const d = new Date(anno, mese - 1, giorno);
  if (d.getFullYear() !== anno || d.getMonth() !== mese - 1 || d.getDate() !== giorno) {
    return "";
  }
  return `${giorno} ${MESI[mese - 1]} ${anno}`;
}

/** `true` se `v` è una data `AAAA-MM-GG` reale. */
export function eDataIso(v: string): boolean {
  return dataEstesa(v) !== "";
}

/** Tipo di `<input>` per un campo data: `date` solo se il valore è vuoto o ISO, altrimenti `text` (non si perde il predefinito). */
export function tipoInputData(valore: string): "date" | "text" {
  return valore === "" || eDataIso(valore) ? "date" : "text";
}

/** Valori iniziali del modulo: predefiniti del modello, `oggi` risolto. */
export function valoriIniziali(campi: readonly CampoModello[], oggi: string = isoOggi()): Record<string, string> {
  const valori: Record<string, string> = {};
  for (const c of campi) {
    const p = c.predefinito ?? "";
    valori[c.nome] = c.tipo === "data" && p.trim().toLowerCase() === "oggi" ? oggi : p;
  }
  return valori;
}

/**
 * Valori da mandare al core: le date diventano `16 ottobre 2026`, gli altri
 * campi si ripuliscono dagli spazi ai bordi (l'`elenco` conserva le righe).
 */
export function valoriPerCore(
  campi: readonly CampoModello[],
  valori: Readonly<Record<string, string>>,
): Record<string, string> {
  const fuori: Record<string, string> = {};
  for (const c of campi) {
    const v = valori[c.nome] ?? "";
    // Una data ISO diventa «16 ottobre 2026»; un testo libero (predefinito non ISO) passa com'è.
    fuori[c.nome] = c.tipo === "data" ? dataEstesa(v) || v.trim() : c.tipo === "elenco" ? v.replace(/\s+$/u, "") : v.trim();
  }
  return fuori;
}

/** Riepilogo per la lista: «1 pagina · 4 campi». */
export function riepilogoModello(m: Pick<ModelloInfo, "pagine" | "campi">): string {
  const parti: string[] = [];
  if (m.pagine !== undefined && m.pagine !== null && m.pagine > 0) {
    parti.push(m.pagine === 1 ? "1 pagina" : `${m.pagine} pagine`);
  }
  parti.push(m.campi.length === 1 ? "1 campo" : `${m.campi.length} campi`);
  return parti.join(" · ");
}

/** Filtra i modelli per nome o descrizione (senza distinzione di maiuscole e accenti). */
export function filtraModelli(modelli: readonly ModelloInfo[], ricerca: string): ModelloInfo[] {
  const q = normalizza(ricerca);
  if (q === "") {
    return [...modelli];
  }
  return modelli.filter((m) => normalizza(`${m.nome} ${m.descrizione ?? ""}`).includes(q));
}

function normalizza(t: string): string {
  return t.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase().trim();
}

// ---- Salva come modello ----

/** Nome di campo ammesso dal core: `[a-z][a-z0-9_]{0,39}`. */
export function nomeCampoValido(nome: string): boolean {
  return /^[a-z][a-z0-9_]{0,39}$/.test(nome);
}

/** Propone un nome di campo dal testo selezionato («Studio Bianchi» → `studio_bianchi`). */
export function nomeCampoSuggerito(testo: string): string {
  let s = normalizza(testo).replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");
  if (s === "") {
    return "campo";
  }
  if (!/^[a-z]/.test(s)) {
    s = `campo_${s}`;
  }
  return s.slice(0, 40).replace(/_+$/g, "") || "campo";
}

/** `{{nome}}` — il segnaposto che il core sostituisce. */
export function segnaposto(nome: string): string {
  return `{{${nome}}}`;
}

/** Una sostituzione nel documento: intervallo `da..a` (offset del testo originale) → `testo`. */
export interface Cambio {
  da: number;
  a: number;
  testo: string;
}

/** Esito della trasformazione di un frammento in campo. */
export interface EsitoCampo {
  /** Sostituzioni da applicare al testo originale, in ordine e non sovrapposte. */
  cambi: Cambio[];
  /** Selezione da proporre nel testo RISULTANTE (sul primo segnaposto). */
  selezione: { da: number; a: number };
}

/** Quante volte `frammento` compare in `testo` (senza sovrapposizioni). */
export function contaOccorrenze(testo: string, frammento: string): number {
  return posizioniOccorrenze(testo, frammento).length;
}

/** Posizioni di inizio delle occorrenze non sovrapposte di `frammento`. */
function posizioniOccorrenze(testo: string, frammento: string): number[] {
  const pos: number[] = [];
  if (frammento === "") {
    return pos;
  }
  for (let i = testo.indexOf(frammento); i !== -1; i = testo.indexOf(frammento, i + frammento.length)) {
    pos.push(i);
  }
  return pos;
}

/** `true` se `frammento` è già un segnaposto intero (`{{nome}}` o `{{!nome}}`). */
export function eGiaCampo(frammento: string): boolean {
  return /^\{\{!?[a-z][a-z0-9_]*\}\}$/.test(frammento.trim());
}

/** «Tutte le occorrenze» ha senso come predefinito solo per frammenti di almeno 3 caratteri non vuoti. */
export function sostituzioneMultiplaSensata(frammento: string): boolean {
  return frammento.trim().length >= 3;
}

/**
 * Trasforma in campo il frammento `testo[da..a]`: con `tutte` sostituisce ogni
 * occorrenza identica (se la selezione stessa non è fra quelle non sovrapposte,
 * solo la selezione), altrimenti solo quella selezionata. `null` se la selezione
 * è vuota, fatta di soli spazi o fuori dal testo, se il nome non è valido, o se
 * il frammento contiene già un segnaposto (`{{`/`}}`).
 */
export function convertiInCampo(
  testo: string,
  da: number,
  a: number,
  nome: string,
  tutte: boolean,
): EsitoCampo | null {
  if (!nomeCampoValido(nome) || da < 0 || a > testo.length || da >= a) {
    return null;
  }
  const frammento = testo.slice(da, a);
  if (frammento.trim() === "" || frammento.includes("{{") || frammento.includes("}}")) {
    return null;
  }
  const sp = segnaposto(nome);
  let posizioni = tutte ? posizioniOccorrenze(testo, frammento) : [da];
  if (!posizioni.includes(da)) {
    posizioni = [da];
  }
  const cambi = posizioni.map((p) => ({ da: p, a: p + frammento.length, testo: sp }));
  // Il primo cambio non ha niente di modificato prima di sé: il suo offset vale anche nel testo risultante.
  const primo = cambi[0].da;
  return { cambi, selezione: { da: primo, a: primo + sp.length } };
}

/** Applica i cambi a `testo` (per i controlli e per l'anteprima). */
export function applicaCambi(testo: string, cambi: readonly Cambio[]): string {
  let fuori = "";
  let da = 0;
  for (const c of cambi) {
    fuori += testo.slice(da, c.da) + c.testo;
    da = c.a;
  }
  return fuori + testo.slice(da);
}

/** Abbrevia un testo a `max` caratteri senza spezzare coppie surrogate. */
export function abbrevia(testo: string, max: number): string {
  const c = Array.from(testo);
  return c.length > max ? `${c.slice(0, max).join("")}\u2026` : testo;
}

/** Nomi dei campi usati nel sorgente (`{{nome}}` e `{{!nome}}`), senza duplicati, in ordine. */
export function campiNelSorgente(testo: string): string[] {
  const visti = new Set<string>();
  for (const m of testo.matchAll(/\{\{!?([a-z][a-z0-9_]*)\}\}/g)) {
    visti.add(m[1]);
  }
  return [...visti];
}
