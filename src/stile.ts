/**
 * stile.ts — Logica pura dei tre stili grafici (nessun DOM, nessun CodeMirror).
 *
 * Gli stili sono tre: «classico» (l'aspetto storico dell'app), «composizione» e
 * «strumento». Qui stanno solo validazione/normalizzazione del valore salvato,
 * ciclo, etichette e navigazione da tastiera del gruppo di schede; applicare lo
 * stile al DOM spetta a `aspetto.ts`. Verifica: `stile.check.ts`.
 */

/** Identificatori ammessi, nello stesso formato del campo `stile` del core. */
export const STILI = ["classico", "composizione", "strumento"] as const;

/** Scelta dello stile grafico. */
export type SceltaStile = (typeof STILI)[number];

/** Stile predefinito (anche per file di impostazioni vecchi o valori sconosciuti). */
export const STILE_PREDEFINITO: SceltaStile = "classico";

const ETICHETTE: Record<SceltaStile, string> = {
  classico: "Classico",
  composizione: "Composizione",
  strumento: "Strumento",
};

const DESCRIZIONI: Record<SceltaStile, string> = {
  classico: "L'aspetto attuale: tavolozza Catppuccin, barre e pulsanti consueti.",
  composizione: "Carta e inchiostro: toni caldi, titoli con grazie, pagina color carta.",
  strumento: "Console compatta: grafite fredda, accento ciano, alta densità.",
};

/** Vero se il valore è uno degli stili noti. */
export function stileValido(valore: unknown): valore is SceltaStile {
  return typeof valore === "string" && (STILI as readonly string[]).includes(valore);
}

/** Riporta un valore qualunque (letto da disco o dal backend) a uno stile noto. */
export function normalizzaStile(valore: unknown): SceltaStile {
  return stileValido(valore) ? valore : STILE_PREDEFINITO;
}

/** Etichetta leggibile («Classico», …). */
export function etichettaStile(stile: SceltaStile): string {
  return ETICHETTE[stile];
}

/** Descrizione breve per la scheda del menu. */
export function descrizioneStile(stile: SceltaStile): string {
  return DESCRIZIONI[stile];
}

/** Stile a distanza `passo` (anche negativo) dal corrente, in ciclo. */
export function stileSuccessivo(stile: SceltaStile, passo = 1): SceltaStile {
  const n = STILI.length;
  const i = STILI.indexOf(normalizzaStile(stile));
  return STILI[(((i + passo) % n) + n) % n];
}

/**
 * Indice da selezionare dopo un tasto in un gruppo di `totale` radio (pattern
 * WAI-ARIA): frecce con ritorno ciclico, Home/End agli estremi; `null` per
 * qualunque altro tasto o per un gruppo vuoto. Usata anche dal gruppo dei temi.
 */
export function navigaIndice(totale: number, corrente: number, tasto: string): number | null {
  if (totale <= 0) {
    return null;
  }
  switch (tasto) {
    case "ArrowRight":
    case "ArrowDown":
      return (corrente + 1) % totale;
    case "ArrowLeft":
    case "ArrowUp":
      return (corrente - 1 + totale) % totale;
    case "Home":
      return 0;
    case "End":
      return totale - 1;
    default:
      return null;
  }
}

/** Stile da selezionare dopo un tasto sul gruppo di schede degli stili (`null` = nessuno spostamento). */
export function navigaStili(corrente: SceltaStile, tasto: string): SceltaStile | null {
  const i = navigaIndice(STILI.length, STILI.indexOf(normalizzaStile(corrente)), tasto);
  return i === null ? null : STILI[i];
}

/** Chiave dello specchio dello stile in `localStorage` (solo per evitare il flash al primo paint). */
export const CHIAVE_SPECCHIO_STILE = "typster.stile";

/**
 * Stile da uno specchio letto da `localStorage`: solo un valore esattamente valido
 * è applicabile, altrimenti `null` (nessun «normalizza»: un valore ignoto non deve
 * far cambiare aspetto). Il file delle impostazioni resta la fonte di verità.
 */
export function stileDaSpecchio(valore: unknown): SceltaStile | null {
  return stileValido(valore) ? valore : null;
}

/**
 * Se `impostaStile` deve scrivere su disco: solo se il valore (normalizzato) cambia,
 * oppure se le impostazioni non sono ancora state caricate (il valore salvato
 * arriverebbe dopo e la scelta resterebbe solo in memoria).
 */
export function devePersistereStile(
  corrente: SceltaStile,
  scelta: SceltaStile,
  impostazioniCaricate: boolean,
): boolean {
  return !impostazioniCaricate || normalizzaStile(scelta) !== corrente;
}

/** Invio/Spazio su un radio: lo si attiva solo se non è già selezionato (`aria-checked`). */
export function radioDaAttivare(ariaChecked: string | null): boolean {
  return ariaChecked !== "true";
}

/** Dove va il focus quando lascia un elemento del menu `Aspetto` (`null` = nessuna destinazione). */
export type DestinazioneFocus = "pannello" | "bottone" | "fuori" | null;

/**
 * Il popover (dialog non modale da menu button, APG) si chiude quando il focus
 * passa a un elemento esterno (Tab che esce, scorciatoie che focalizzano altro).
 * Con destinazione assente (clic su area non focalizzabile, finestra che perde il
 * focus) decide il `pointerdown` esterno, non il `focusout`.
 */
export function focusEsceDalMenu(destinazione: DestinazioneFocus): boolean {
  return destinazione === "fuori";
}
