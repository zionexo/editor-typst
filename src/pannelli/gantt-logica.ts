/**
 * gantt-logica.ts — Logica pura della scheda «Gantt», basata sul pacchetto **gantt-newgen**
 * (sviluppato dall'utente, incorporato nell'eseguibile come `@local/gantt-newgen`; sorgenti in
 * `crates/typst-core/pacchetti/gantt-newgen`). Attività, gruppi, dipendenze e pietre miliari →
 * dizionario Typst da passare a `gantt(...)`. Nessun DOM, nessun Tauri: verificata da
 * `gantt.check.ts`.
 *
 * Formato (README e `src/schema.typ` del pacchetto; accetta anche lo schema di gantty 0.5):
 * `tasks` (`name` + `start`/`end` [+ `done`, `progress`, `id`, `dependencies`], oppure
 * `subtasks`), `milestones` (`name`, `date`, `show-date`), `headers`, `locale`, `show-today`,
 * `start`/`end` facoltativi (il pacchetto include da solo pietre miliari e completamenti nel
 * diagramma). Le date sono testo ISO `AAAA-MM-GG`; con `date-mode: "inclusive"` (predefinito)
 * il giorno di fine è coperto dalla barra.
 */

import { etichettaValida } from "./inserisci-logica.ts";
import { escapaContenuto } from "./pagina-logica.ts";
import { eDataIso } from "./strumenti-logica.ts";

export const ATTIVITA_MAX = 100;
export const PIETRE_MAX = 30;
/** Finestra di anni ammessa: oltre l'anno 9999 il compilatore va in panico (overflow di `time`). */
export const ANNO_MIN = 1900;
export const ANNO_MAX = 2200;
/** Ampiezza massima del diagramma: con intestazioni a settimane/giorni il pacchetto si ferma oltre ~27 anni (E-HDR-06). */
export const ANNI_MAX = 25;
/** Lunghezza massima di nomi e gruppi: un nome lunghissimo toglie spazio alle barre (E-LAY-01). */
export const NOME_MAX = 80;

/** Tipo di dipendenza: quale estremo del predecessore e del successore sono legati. */
export type TipoDipendenza = "fs" | "ss" | "ff" | "sf";

export const TIPI_DIPENDENZA: readonly { id: TipoDipendenza; etichetta: string; valore: string }[] = [
  { id: "fs", etichetta: "Fine → inizio (predefinita)", valore: "finish-to-start" },
  { id: "ss", etichetta: "Inizio → inizio", valore: "start-to-start" },
  { id: "ff", etichetta: "Fine → fine", valore: "finish-to-finish" },
  { id: "sf", etichetta: "Inizio → fine", valore: "start-to-finish" },
];

/** Combinazioni di intestazioni (dal livello più grossolano al più fine). */
export const INTESTAZIONI: readonly { id: string; etichetta: string; livelli: readonly string[] }[] = [
  { id: "mese-settimana", etichetta: "Mese e settimana", livelli: ["month", "week"] },
  { id: "mese-settimana-iso", etichetta: "Mese e settimana ISO", livelli: ["month", "week-iso"] },
  { id: "anno-trimestre", etichetta: "Anno e trimestre", livelli: ["year", "quarter"] },
  { id: "anno-mese", etichetta: "Anno e mese", livelli: ["year", "month"] },
  { id: "mese-giorno", etichetta: "Mese e giorno", livelli: ["month", "day"] },
  { id: "settimana-giorno", etichetta: "Settimana e giorno", livelli: ["week", "day"] },
];

/** Un'attività (riga della tabella). Le date sono `AAAA-MM-GG` (vuote = non indicate dove facoltative). */
export interface Attivita {
  nome: string;
  /** Attività con lo stesso gruppo diventano sotto-attività di un gruppo con quel nome. */
  gruppo: string;
  inizio: string;
  fine: string;
  /** Data di completamento, facoltativa. */
  fatta: string;
  /** Avanzamento in percentuale (0-100), facoltativo. */
  avanzamento: number | null;
  /** Indice (0-based) dell'attività da cui questa dipende, o `null`. */
  dipende: number | null;
  /** Tipo della dipendenza (conta solo se `dipende` non è `null`). */
  tipoDip: TipoDipendenza;
}

/** Una pietra miliare. */
export interface PietraMiliare {
  nome: string;
  data: string;
  mostraData: boolean;
}

/** Opzioni del diagramma. */
export interface OpzioniGantt {
  /** Intervallo mostrato (facoltativo; se vuoto lo ricava il pacchetto). */
  da: string;
  a: string;
  /** Id di una voce di `INTESTAZIONI`. */
  intestazioni: string;
  /** Linea «oggi» (la data è quella della compilazione): estende l'intervallo fino a oggi. */
  oggi: boolean;
  didascalia: string;
  /** Etichetta per citare la figura (senza `< >`). */
  etichetta: string;
}

/** Opzioni predefinite. */
export const OPZIONI_PREDEFINITE: Readonly<OpzioniGantt> = {
  da: "",
  a: "",
  intestazioni: "mese-settimana",
  oggi: false,
  didascalia: "",
  etichetta: "",
};

/** Aggiunge `giorni` a una data ISO (calendario gregoriano, in UTC: nessun salto d'ora legale). */
export function aggiungiGiorni(iso: string, giorni: number): string {
  const [a, m, g] = iso.split("-").map(Number);
  // setUTCFullYear: Date.UTC trasformerebbe gli anni 0-99 in 1900-1999.
  const d = new Date(0);
  d.setUTCFullYear(a, m - 1, g + giorni);
  const due = (n: number): string => String(n).padStart(2, "0");
  return `${String(d.getUTCFullYear()).padStart(4, "0")}-${due(d.getUTCMonth() + 1)}-${due(d.getUTCDate())}`;
}

/** Stringa Typst letterale (`\` e `"` escapati). */
function lett(s: string): string {
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
}

const haACapo = (s: string): boolean => /[\r\n]/.test(s);

/** `true` se `iso` è una data valida con l'anno nella finestra ammessa. */
function dataAmmessa(iso: string): boolean {
  if (!eDataIso(iso)) {
    return false;
  }
  const anno = Number(iso.slice(0, 4));
  return anno >= ANNO_MIN && anno <= ANNO_MAX;
}

/** Intervallo effettivo del diagramma: quello indicato o, in mancanza, quello ricavato dalle date. */
export function intervalloEffettivo(
  attivita: readonly Attivita[],
  pietre: readonly PietraMiliare[],
  opz: Pick<OpzioniGantt, "da" | "a">,
): { da: string; a: string } {
  const inizi = [...attivita.map((x) => x.inizio), ...pietre.map((p) => p.data)];
  const fini = [...attivita.map((x) => x.fine), ...attivita.map((x) => x.fatta).filter((f) => f !== ""), ...pietre.map((p) => p.data)];
  return {
    da: opz.da !== "" ? opz.da : inizi.reduce((m, d) => (d < m ? d : m), inizi[0] ?? ""),
    a: opz.a !== "" ? opz.a : fini.reduce((m, d) => (d > m ? d : m), fini[0] ?? ""),
  };
}

/** Giorni fra due date ISO (b - a), in UTC. */
function giorniTra(a: string, b: string): number {
  const t = (iso: string): number => {
    const [y, m, g] = iso.split("-").map(Number);
    return Date.UTC(y, m - 1, g);
  };
  return Math.round((t(b) - t(a)) / 86_400_000);
}

/** Primo errore di validazione in italiano, o `null` se i dati sono corretti. */
export function validaGantt(
  attivita: readonly Attivita[],
  pietre: readonly PietraMiliare[],
  opz: OpzioniGantt,
  oggiIso: string = new Date().toISOString().slice(0, 10),
): string | null {
  if (attivita.length === 0) {
    return "Aggiungi almeno un'attività";
  }
  if (attivita.length > ATTIVITA_MAX || pietre.length > PIETRE_MAX) {
    return `Al massimo ${ATTIVITA_MAX} attività e ${PIETRE_MAX} pietre miliari`;
  }
  const intervalloAnni = `tra il ${ANNO_MIN} e il ${ANNO_MAX}`;
  for (let i = 0; i < attivita.length; i++) {
    const a = attivita[i];
    const n = `Attività ${i + 1}`;
    if (a.nome.trim() === "") return `${n}: manca il nome`;
    if (haACapo(a.nome) || haACapo(a.gruppo)) return `${n}: nome e gruppo non possono andare a capo`;
    if (a.nome.trim().length > NOME_MAX || a.gruppo.trim().length > NOME_MAX) return `${n}: nome e gruppo al massimo ${NOME_MAX} caratteri`;
    if (!eDataIso(a.inizio)) return `${n}: data di inizio non valida`;
    if (!eDataIso(a.fine)) return `${n}: data di fine non valida`;
    if (!dataAmmessa(a.inizio) || !dataAmmessa(a.fine)) return `${n}: le date devono essere ${intervalloAnni}`;
    if (a.fine < a.inizio) return `${n}: la fine precede l'inizio`;
    if (a.fatta !== "") {
      if (!eDataIso(a.fatta)) return `${n}: data di completamento non valida`;
      if (!dataAmmessa(a.fatta)) return `${n}: le date devono essere ${intervalloAnni}`;
      if (a.fatta < a.inizio) return `${n}: il completamento precede l'inizio`;
    }
    if (a.avanzamento !== null && (!Number.isInteger(a.avanzamento) || a.avanzamento < 0 || a.avanzamento > 100)) {
      return `${n}: l'avanzamento è un numero intero da 0 a 100`;
    }
    if (a.dipende !== null) {
      if (!Number.isInteger(a.dipende) || a.dipende < 0 || a.dipende >= attivita.length) return `${n}: dipendenza non valida`;
      if (a.dipende === i) return `${n}: non può dipendere da se stessa`;
      if (!TIPI_DIPENDENZA.some((t) => t.id === a.tipoDip)) return `${n}: tipo di dipendenza non valido`;
    }
  }
  // Dipendenze circolari (A dopo B dopo A): ogni attività ha al più una dipendenza, basta seguire la catena.
  for (let i = 0; i < attivita.length; i++) {
    const visti = new Set<number>([i]);
    for (let j = attivita[i].dipende; j !== null; j = attivita[j].dipende) {
      if (visti.has(j)) return `Attività ${i + 1}: dipendenza circolare`;
      visti.add(j);
    }
  }
  for (let i = 0; i < pietre.length; i++) {
    const p = pietre[i];
    const n = `Pietra miliare ${i + 1}`;
    if (p.nome.trim() === "") return `${n}: manca il nome`;
    if (haACapo(p.nome)) return `${n}: il nome non può andare a capo`;
    if (p.nome.trim().length > NOME_MAX) return `${n}: nome al massimo ${NOME_MAX} caratteri`;
    if (!eDataIso(p.data)) return `${n}: data non valida`;
    if (!dataAmmessa(p.data)) return `${n}: la data deve essere ${intervalloAnni}`;
  }
  if (opz.da !== "" && !eDataIso(opz.da)) return "Intervallo: data iniziale non valida";
  if (opz.a !== "" && !eDataIso(opz.a)) return "Intervallo: data finale non valida";
  if ((opz.da !== "" && !dataAmmessa(opz.da)) || (opz.a !== "" && !dataAmmessa(opz.a))) return `Intervallo: le date devono essere ${intervalloAnni}`;
  if (opz.da !== "" && opz.a !== "" && opz.a < opz.da) return "Intervallo: la data finale precede l'iniziale";
  // Il pacchetto rifiuta un intervallo che finisce prima di cominciare («Mostra da» dopo tutte le fini).
  const eff = intervalloEffettivo(attivita, pietre, opz);
  if (eff.a < eff.da) {
    return opz.da !== ""
      ? "Intervallo: «Mostra da» è dopo la fine di tutte le attività"
      : "Intervallo: «Mostra fino a» è prima dell'inizio di tutte le attività";
  }
  // La linea «oggi» allarga l'intervallo fino a oggi dal lato non fissato (come fa il pacchetto).
  const largo = {
    da: opz.oggi && opz.da === "" && oggiIso < eff.da ? oggiIso : eff.da,
    a: opz.oggi && opz.a === "" && oggiIso > eff.a ? oggiIso : eff.a,
  };
  if (giorniTra(largo.da, largo.a) > ANNI_MAX * 366) return `Il diagramma copre più di ${ANNI_MAX} anni: restringi le date o l'intervallo mostrato`;
  if (!INTESTAZIONI.some((h) => h.id === opz.intestazioni)) return "Intestazioni non valide";
  if (opz.etichetta.trim() !== "" && !etichettaValida(opz.etichetta.trim())) {
    return "L'etichetta deve iniziare con una lettera (es. fig:piano)";
  }
  return null;
}

/** Id dell'attività `i` usato nelle dipendenze. */
const idAttivita = (i: number): string => `a${i + 1}`;

/** Esito della generazione. */
export type EsitoGantt = { codice: string } | { errore: string };

/**
 * Codice Typst del diagramma: `#gantt((...))`, oppure `#figure(gantt((...)), ...)` con didascalia
 * o etichetta. Le attività con lo stesso gruppo diventano sotto-attività; l'ordine è quello di
 * prima comparsa. Solo le attività da cui dipende qualcuna ricevono un `id`.
 */
export function generaGantt(
  attivita: readonly Attivita[],
  pietre: readonly PietraMiliare[],
  opz: OpzioniGantt,
): EsitoGantt {
  const errore = validaGantt(attivita, pietre, opz);
  if (errore !== null) {
    return { errore };
  }
  const referenziate = new Set(attivita.map((a) => a.dipende).filter((d): d is number => d !== null));

  const campiAttivita = (a: Attivita, i: number): string => {
    const campi = [`name: ${lett(a.nome.trim())}`, `start: ${lett(a.inizio)}`, `end: ${lett(a.fine)}`];
    if (a.fatta !== "") campi.push(`done: ${lett(a.fatta)}`);
    if (a.avanzamento !== null) campi.push(`progress: ${a.avanzamento}`);
    if (referenziate.has(i)) campi.push(`id: ${lett(idAttivita(i))}`);
    if (a.dipende !== null) {
      const tipo = a.tipoDip === "fs" ? "" : `, type: ${lett(TIPI_DIPENDENZA.find((t) => t.id === a.tipoDip)!.valore)}`;
      campi.push(`dependencies: ((id: ${lett(idAttivita(a.dipende))}${tipo}),)`);
    }
    return `(${campi.join(", ")})`;
  };

  // Voci di primo livello nell'ordine di prima comparsa: attività sole o gruppi.
  type Voce = { gruppo: string | null; righe: number[] };
  const voci: Voce[] = [];
  const perGruppo = new Map<string, Voce>();
  attivita.forEach((a, i) => {
    const g = a.gruppo.trim();
    if (g === "") {
      voci.push({ gruppo: null, righe: [i] });
      return;
    }
    let v = perGruppo.get(g);
    if (v === undefined) {
      v = { gruppo: g, righe: [] };
      perGruppo.set(g, v);
      voci.push(v);
    }
    v.righe.push(i);
  });

  const righeTasks = voci.map((v) => {
    if (v.gruppo === null) {
      return `    ${campiAttivita(attivita[v.righe[0]], v.righe[0])},`;
    }
    const sub = v.righe.map((i) => `      ${campiAttivita(attivita[i], i)},`).join("\n");
    return `    (name: ${lett(v.gruppo)}, subtasks: (\n${sub}\n    )),`;
  });

  const livelli = INTESTAZIONI.find((h) => h.id === opz.intestazioni)!.livelli;
  const parti: string[] = [
    `  locale: "it",`,
    `  headers: (${livelli.map(lett).join(", ")}),`,
    `  show-today: ${opz.oggi},`,
  ];
  // L'intervallo lo ricava il pacchetto (include anche pietre miliari e completamenti): si scrive solo se l'utente lo fissa.
  if (opz.da !== "") parti.push(`  start: ${lett(opz.da)},`);
  if (opz.a !== "") parti.push(`  end: ${lett(opz.a)},`);
  parti.push(`  tasks: (\n${righeTasks.join("\n")}\n  ),`);
  if (pietre.length > 0) {
    const m = pietre
      .map((p) => `    (name: ${lett(p.nome.trim())}, date: ${lett(p.data)}, show-date: ${p.mostraData}),`)
      .join("\n");
    parti.push(`  milestones: (\n${m}\n  ),`);
  }
  const dizionario = `(\n${parti.join("\n")}\n)`;

  const d = opz.didascalia.trim();
  const e = opz.etichetta.trim();
  if (d === "" && e === "") {
    return { codice: `#gantt(${dizionario})\n` };
  }
  const interno = `gantt(${dizionario})`.replace(/^/gm, "  ");
  const didascalia = d !== "" ? `  caption: [${escapaContenuto(d)}],\n` : "";
  return { codice: `#figure(\n${interno},\n${didascalia})${e !== "" ? ` <${e}>` : ""}\n` };
}

/** Dati d'esempio (a partire da `oggi`) per mostrare come si compila la tabella. */
export function esempioGantt(oggi: string): { attivita: Attivita[]; pietre: PietraMiliare[] } {
  const g = (n: number): string => aggiungiGiorni(oggi, n);
  const a = (nome: string, gruppo: string, i: number, f: number, extra: Partial<Attivita> = {}): Attivita => ({
    nome, gruppo, inizio: g(i), fine: g(f), fatta: "", avanzamento: null, dipende: null, tipoDip: "fs", ...extra,
  });
  return {
    attivita: [
      a("Analisi dei requisiti", "Preparazione", 0, 6, { fatta: g(5), avanzamento: 100 }),
      a("Progettazione", "Preparazione", 7, 20, { dipende: 0, avanzamento: 40 }),
      a("Sviluppo", "", 21, 55, { dipende: 1 }),
      a("Collaudo", "", 56, 70, { dipende: 2 }),
    ],
    pietre: [{ nome: "Consegna", data: g(70), mostraData: true }],
  };
}

/**
 * Toglie l'attività `i` e riallinea le dipendenze: chi dipendeva da lei non dipende più da
 * nessuno, chi dipendeva da un'attività successiva scala di un posto.
 */
export function rimuoviAttivita(attivita: readonly Attivita[], i: number): Attivita[] {
  return attivita
    .filter((_, j) => j !== i)
    .map((a) => ({
      ...a,
      dipende: a.dipende === null || a.dipende === i ? null : a.dipende > i ? a.dipende - 1 : a.dipende,
    }));
}
