/**
 * errori.check.ts — Verifica di `errori.ts` (riga/colonna da offset, ordinamento, riepilogo,
 * descrizione dei diagnostici). Esce con errore al primo controllo fallito.
 */
import {
  NOTA_SENZA_POSIZIONE,
  contaDiagnostici,
  descriviDiagnostico,
  intervalloDiagnostico,
  messaggioMarcatore,
  ordinaDiagnostici,
  posizioneDaOffset,
  riepilogoDiagnostici,
  suggerimentiNuovi,
  type DiagnosticoBase,
} from "./errori.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

// 1. riga/colonna: inizio, fine riga, righe successive, ultima riga senza \n.
{
  const t = "ab\ncde\n\nf";
  uguali(posizioneDaOffset(t, 0), { riga: 1, colonna: 1 }, "inizio");
  uguali(posizioneDaOffset(t, 2), { riga: 1, colonna: 3 }, "prima del \\n");
  uguali(posizioneDaOffset(t, 3), { riga: 2, colonna: 1 }, "dopo il primo \\n");
  uguali(posizioneDaOffset(t, 5), { riga: 2, colonna: 3 }, "dentro la riga 2");
  uguali(posizioneDaOffset(t, 7), { riga: 3, colonna: 1 }, "riga vuota");
  uguali(posizioneDaOffset(t, 8), { riga: 4, colonna: 1 }, "ultima riga");
  uguali(posizioneDaOffset(t, 9), { riga: 4, colonna: 2 }, "fine documento");
}

// 2. Oltre la fine, negativo, non finito: stringono ai bordi, senza errori.
{
  const t = "ab\ncd";
  uguali(posizioneDaOffset(t, 999), { riga: 2, colonna: 3 }, "oltre la fine");
  uguali(posizioneDaOffset(t, -5), { riga: 1, colonna: 1 }, "negativo");
  uguali(posizioneDaOffset(t, Number.NaN), { riga: 1, colonna: 1 }, "NaN");
  uguali(posizioneDaOffset(t, Number.POSITIVE_INFINITY), { riga: 1, colonna: 1 }, "infinito");
  uguali(posizioneDaOffset(t, 2.9), { riga: 1, colonna: 3 }, "frazionario troncato");
}

// 3. Documento vuoto.
{
  uguali(posizioneDaOffset("", 0), { riga: 1, colonna: 1 }, "vuoto a 0");
  uguali(posizioneDaOffset("", 10), { riga: 1, colonna: 1 }, "vuoto oltre");
}

// 4. Emoji e surrogati: la colonna conta i caratteri Unicode; l'offset a metà coppia arretra.
{
  const t = "a😀b\n😀😀x"; // 😀 = 2 unità UTF-16
  uguali(posizioneDaOffset(t, 1), { riga: 1, colonna: 2 }, "prima dell'emoji");
  uguali(posizioneDaOffset(t, 3), { riga: 1, colonna: 3 }, "dopo l'emoji = colonna 3, non 4");
  uguali(posizioneDaOffset(t, 2), { riga: 1, colonna: 2 }, "a metà coppia: inizio della coppia");
  uguali(posizioneDaOffset(t, 4), { riga: 1, colonna: 4 }, "dopo b");
  uguali(posizioneDaOffset(t, 5), { riga: 2, colonna: 1 }, "inizio riga 2");
  uguali(posizioneDaOffset(t, 9), { riga: 2, colonna: 3 }, "dopo due emoji");
  uguali(posizioneDaOffset(t, 10), { riga: 2, colonna: 4 }, "fine");
  // Surrogata isolata (testo malformato): non lancia, conta come un carattere.
  uguali(posizioneDaOffset("\ud83dx", 1), { riga: 1, colonna: 2 }, "surrogata alta isolata");
}

// 5. CRLF già normalizzato nell'editor: il testo ha solo \n. Un \r residuo è un carattere.
{
  uguali(posizioneDaOffset("a\nb", 2), { riga: 2, colonna: 1 }, "solo \\n");
  uguali(posizioneDaOffset("a\r\nb", 3), { riga: 2, colonna: 1 }, "\\r\\n: la riga cambia dopo il \\n");
}

// 6. intervalloDiagnostico: clamp al documento corrente, da <= a, senza posizione = null.
{
  uguali(intervalloDiagnostico({ da: 3, a: 7 }), { da: 3, a: 7 }, "grezzo");
  uguali(intervalloDiagnostico({ da: 3, a: 7 }, 5), { da: 3, a: 5 }, "a oltre la fine");
  uguali(intervalloDiagnostico({ da: 30, a: 70 }, 5), { da: 5, a: 5 }, "tutto oltre la fine");
  uguali(intervalloDiagnostico({ da: 7, a: 3 }), { da: 7, a: 7 }, "a < da");
  uguali(intervalloDiagnostico({ da: 4, a: null }), { da: 4, a: 4 }, "a assente = cursore");
  uguali(intervalloDiagnostico({ da: -4, a: 2 }), { da: 0, a: 2 }, "da negativo");
  vero(intervalloDiagnostico({ da: null, a: null }) === null, "senza posizione");
  vero(intervalloDiagnostico({ da: Number.NaN, a: 1 }) === null, "da NaN");
}

const e = (da: number | null, extra: Partial<DiagnosticoBase> = {}): DiagnosticoBase => ({
  gravita: "errore",
  messaggio: "m",
  da,
  a: da === null ? null : da + 1,
  ...extra,
});
const w = (da: number | null): DiagnosticoBase => ({ ...e(da), gravita: "avviso" });

// 7. Ordinamento: errori prima degli avvisi, con posizione prima di senza, per offset, stabile.
{
  const elenco = [
    { ...w(1), messaggio: "w1" },
    { ...e(null), messaggio: "e-senza-1" },
    { ...e(30), messaggio: "e30" },
    { ...e(null), messaggio: "e-senza-2" },
    { ...e(10), messaggio: "e10" },
    { ...w(0), messaggio: "w0" },
    { ...e(10), messaggio: "e10bis" },
  ];
  const copia = elenco.slice();
  uguali(
    ordinaDiagnostici(elenco).map((d) => d.messaggio),
    ["e10", "e10bis", "e30", "e-senza-1", "e-senza-2", "w0", "w1"],
    "ordine",
  );
  uguali(elenco, copia, "l'input non è modificato");
  uguali(ordinaDiagnostici([]), [], "vuoto");
}

// 8. Riepilogo e conteggi.
{
  uguali(riepilogoDiagnostici(2, 1), "2 errori, 1 avviso", "plurale/singolare");
  uguali(riepilogoDiagnostici(1, 3), "1 errore, 3 avvisi", "singolare/plurale");
  uguali(riepilogoDiagnostici(0, 2), "2 avvisi", "solo avvisi");
  uguali(riepilogoDiagnostici(1, 0), "1 errore", "solo un errore");
  uguali(riepilogoDiagnostici(0, 0), "", "nulla");
  uguali(contaDiagnostici([e(1), e(null), w(2)]), { errori: 2, avvisi: 1 }, "conteggio");
}

// 9. Descrizione: nel main (riga/colonna dal testo corrente), senza etichetta di chiamata.
{
  const testo = "riga uno\nriga due\nterza";
  const v = descriviDiagnostico(e(9), testo);
  uguali(v.posizione, "riga 2, colonna 1", "posizione nel main");
  vero(!v.puntoDiChiamata, "non è un punto di chiamata");
  uguali(v.intervallo, { da: 9, a: 10 }, "intervallo");
  uguali(v.etichetta, "Errore", "etichetta errore");
  uguali(v.dettagli, [], "nessun dettaglio né nota");
  uguali(descriviDiagnostico(w(0), testo).etichetta, "Avviso", "etichetta avviso");
}

// 10. Descrizione: posizione da traccia con origine, traccia e suggerimento nuovo.
{
  const testo = "x\n".repeat(60);
  const d = e(108, {
    posizione_da_traccia: true,
    messaggio: "string contains invalid digits\nhint: controlla i dati",
    origine: "gantt.yaml, riga 3, colonna 7",
    traccia: ["chiamata della funzione `gantt` (riga 54)"],
    suggerimenti: ["controlla i dati", "usa numeri interi"],
  });
  const v = descriviDiagnostico(d, testo);
  uguali(v.posizione, "riga 55, colonna 1", "riga dal testo");
  vero(v.puntoDiChiamata, "etichetta punto di chiamata");
  uguali(
    v.dettagli,
    [
      "Origine: gantt.yaml, riga 3, colonna 7",
      "chiamata della funzione `gantt` (riga 54)",
      "Suggerimento: usa numeri interi",
    ],
    "dettagli: il suggerimento già nel messaggio non si ripete",
  );
}

// 11. Senza posizione: con origine niente nota; senza origine la nota; mai persi.
{
  const conOrigine = descriviDiagnostico(e(null, { origine: "@preview/gantty:0.5.1 — src/lib.typ, riga 12, colonna 5" }), "abc");
  vero(conOrigine.intervallo === null && conOrigine.posizione === null, "senza posizione");
  uguali(conOrigine.dettagli, ["Origine: @preview/gantty:0.5.1 — src/lib.typ, riga 12, colonna 5"], "solo origine");
  const nuda = descriviDiagnostico(e(null), "abc");
  uguali(nuda.dettagli, [NOTA_SENZA_POSIZIONE], "nota esplicita");
  vero(NOTA_SENZA_POSIZIONE.startsWith("Posizione non disponibile"), "testo della nota");
  vero(NOTA_SENZA_POSIZIONE.includes("file letti da questa chiamata"), "suggerimento sui file letti");
  // Origine vuota o solo spazi = assente.
  uguali(descriviDiagnostico(e(null, { origine: "  " }), "").dettagli, [NOTA_SENZA_POSIZIONE], "origine vuota");
  // Markdown: gli offset non valgono, anche se presenti.
  const md = descriviDiagnostico(e(2), "abc", true);
  vero(md.intervallo === null && md.posizione === null && !md.puntoDiChiamata, "markdown: nessuna posizione");
  uguali(md.dettagli, [NOTA_SENZA_POSIZIONE], "markdown: nota");
}

// 12. Core vecchio: campi assenti, comportamento invariato; offset oltre la fine = clamp.
{
  const v = descriviDiagnostico({ gravita: "errore", messaggio: "x", da: 500, a: 600 }, "ab\ncd");
  uguali(v.intervallo, { da: 5, a: 5 }, "clamp al documento cambiato");
  uguali(v.posizione, "riga 2, colonna 3", "posizione clampata");
  uguali(v.dettagli, [], "nessun dettaglio");
  uguali(suggerimentiNuovi({ gravita: "errore", messaggio: "x", da: null, a: null }), [], "senza suggerimenti");
}

// 13. Messaggio del marcatore inline: origine e punto di chiamata.
{
  uguali(messaggioMarcatore(e(1)), "m", "semplice");
  uguali(
    messaggioMarcatore(e(1, { posizione_da_traccia: true, origine: "gantt.yaml, riga 3, colonna 7" })),
    "m\n(punto di chiamata)\nOrigine: gantt.yaml, riga 3, colonna 7",
    "da traccia con origine",
  );
  uguali(messaggioMarcatore(e(1, { origine: "f.typ, riga 1" })), "m\nOrigine: f.typ, riga 1", "solo origine");
}

console.log("errori.check: OK");
