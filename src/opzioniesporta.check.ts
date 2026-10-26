/**
 * opzioniesporta.check.ts — Verifica eseguibile di `opzioniesporta.ts` (F5-011).
 * Stesso schema di `debounce.check.ts`; esce con errore al primo controllo fallito.
 */
import {
  CARTE_LATEX,
  CLASSI_LATEX,
  LARGHEZZA_TESTO_MAX,
  LARGHEZZA_TESTO_MIN,
  LARGHEZZA_TESTO_PREDEFINITA,
  MAX_AVVISI_MOSTRATI,
  PPI_MASSIMO,
  PPI_PREDEFINITO,
  STANDARD_UI,
  argomentiConversione,
  cartaLatexValida,
  classeLatexValida,
  comandoConversione,
  eConversione,
  filtriSalvataggio,
  leggiLarghezza,
  leggiPpi,
  messaggioEsito,
  nomePredefinito,
  opzioniPdfPerBackend,
  pagineOppureNull,
  standardValido,
  testoAvvisiConversione,
  testoConflitto,
  testoErroreEsportazione,
} from "./opzioniesporta.ts";
import type { EsitoConversione, FormatoEsportazione } from "./opzioniesporta.ts";

let n = 0;
function uguale<T>(atteso: T, reale: T, msg: string): void {
  n++;
  if (JSON.stringify(atteso) !== JSON.stringify(reale)) {
    throw new Error(`FALLITO: ${msg} (atteso ${JSON.stringify(atteso)}, ottenuto ${JSON.stringify(reale)})`);
  }
}

// Risoluzione: intero 1..PPI_MASSIMO, vuoto = predefinito, il resto è rifiutato.
uguale(PPI_PREDEFINITO, leggiPpi(""), "vuoto = predefinito");
uguale(PPI_PREDEFINITO, leggiPpi("   "), "solo spazi = predefinito");
uguale(300, leggiPpi(" 300 "), "spazi ai lati");
uguale(1, leggiPpi("1"), "minimo");
uguale(PPI_MASSIMO, leggiPpi(String(PPI_MASSIMO)), "massimo");
for (const cattivo of ["0", "-5", "1201", "12.5", "1e3", "abc", "144dpi", "NaN", "0x10"]) {
  uguale(null, leggiPpi(cattivo), `rifiutato: ${cattivo}`);
}

// Intervallo di pagine: passato così com'è (lo valida il core), vuoto = tutte.
uguale(null, pagineOppureNull(""), "pagine vuote");
uguale(null, pagineOppureNull("  "), "pagine solo spazi");
uguale("1-3, 5", pagineOppureNull(" 1-3, 5 "), "pagine con spazi");

// Standard: solo le varianti della UI (D-011), il resto ricade su "nessuno".
uguale(["nessuno", "a-2b", "a-3b", "a-4"], STANDARD_UI.map((s) => s.valore), "varianti offerte dalla UI");
uguale("a-3b", standardValido("a-3b"), "a-3b valido");
for (const cattivo of ["a-1b", "a-2a", "", "PDF/A-2b", null, undefined, 3]) {
  uguale("nessuno", standardValido(cattivo), `ripiego: ${String(cattivo)}`);
}

// Opzioni verso il backend: lista vuota senza standard, uno solo altrimenti.
uguale({ standard: [], pagine: null }, opzioniPdfPerBackend({ pagine: null, standard: "nessuno" }), "nessuno");
uguale({ standard: ["a-2b"], pagine: "2-3" }, opzioniPdfPerBackend({ pagine: " 2-3 ", standard: "a-2b" }), "a-2b + pagine");

// Nome predefinito del file.
uguale("C:\\d\\tesi.pdf", nomePredefinito("C:\\d\\tesi.typ", "pdf"), "da .typ a .pdf");
uguale("/x/nota.png", nomePredefinito("/x/nota.md", "png"), "da .md a .png");
uguale("documento.svg", nomePredefinito(null, "svg"), "senza file corrente");

// Messaggi di esito.
uguale("PDF esportato", messaggioEsito("pdf", 1), "esito pdf");
uguale("PNG esportato", messaggioEsito("png", 1), "esito png singolo");
uguale("5 file SVG esportati", messaggioEsito("svg", 5), "esito svg multiplo");

// Conferma di sovrascrittura: elenco troncato a 5 nomi.
uguale(
  "Questi file esistono già e verranno sovrascritti:\na-1.png\na-2.png",
  testoConflitto(["a-1.png", "a-2.png"]),
  "conflitto: pochi nomi",
);
uguale(
  "Questi file esistono già e verranno sovrascritti:\n1\n2\n3\n4\n5\ne altri 2",
  testoConflitto(["1", "2", "3", "4", "5", "6", "7"]),
  "conflitto: elenco troncato",
);
uguale(true, !testoConflitto(["1", "2", "3", "4", "5"]).includes("altri"), "esattamente 5: nessun troncamento");

// Errore di esportazione: il motivo del backend arriva intero, col formato.
uguale(
  "Esportazione PDF non riuscita: destinazione rifiutata",
  testoErroreEsportazione("pdf", "destinazione rifiutata"),
  "errore stringa",
);
uguale(
  "Esportazione SVG non riuscita: a\nb",
  testoErroreEsportazione("svg", ["a", "b"]),
  "errore array",
);
uguale(
  "Esportazione PNG non riuscita: boom",
  testoErroreEsportazione("png", new Error("boom")),
  "errore Error",
);
uguale(
  "Esportazione PNG non riuscita: errore sconosciuto",
  testoErroreEsportazione("png", null),
  "errore ignoto",
);

// ---- Conversioni (testo semplice, HTML, LaTeX, Markdown) ----

// Quali formati sono conversioni del sorgente e non compilazioni.
for (const f of ["txt", "html", "tex", "md"] as const) uguale(true, eConversione(f), `conversione: ${f}`);
for (const f of ["pdf", "png", "svg"] as const) uguale(false, eConversione(f), `compilazione: ${f}`);

// Filtri del dialogo «Salva»: estensione corretta per ogni formato.
uguale([{ name: "Testo semplice", extensions: ["txt"] }], filtriSalvataggio("txt"), "filtro txt");
uguale([{ name: "HTML", extensions: ["html"] }], filtriSalvataggio("html"), "filtro html");
uguale([{ name: "LaTeX", extensions: ["tex"] }], filtriSalvataggio("tex"), "filtro tex");
uguale([{ name: "Markdown", extensions: ["md"] }], filtriSalvataggio("md"), "filtro md");

// Nomi suggeriti: l'estensione sostituisce .typ/.md; un .md da .md non coincide col documento.
uguale("C:\\d\\tesi.html", nomePredefinito("C:\\d\\tesi.typ", "html"), "typ -> html");
uguale("/x/tesi.tex", nomePredefinito("/x/tesi.typ", "tex"), "typ -> tex");
uguale("/x/tesi.txt", nomePredefinito("/x/tesi.md", "txt"), "md -> txt");
uguale("/x/tesi.md", nomePredefinito("/x/tesi.typ", "md"), "typ -> md");
uguale("/x/tesi-esportato.md", nomePredefinito("/x/tesi.md", "md"), "md -> md diverso dal documento");
uguale("documento.tex", nomePredefinito(null, "tex"), "senza file: tex");
uguale("/x/nota.txt.txt", nomePredefinito("/x/nota.txt", "txt"), "estensione estranea: si aggiunge, non è il sorgente");
uguale("/x/bozza.pdf", nomePredefinito("/x/bozza", "pdf"), "senza estensione");
uguale("/x/a.typc.html", nomePredefinito("/x/a.typc", "html"), "altra estensione Typst");

// Larghezza del testo: 20..500, vuoto = 80.
uguale(LARGHEZZA_TESTO_PREDEFINITA, leggiLarghezza(""), "larghezza vuota");
uguale(20, leggiLarghezza("20"), "larghezza minima");
uguale(500, leggiLarghezza(" 500 "), "larghezza massima");
for (const cattivo of ["19", "501", "-1", "8.5", "ottanta", "1e2"]) {
  uguale(null, leggiLarghezza(cattivo), `larghezza rifiutata: ${cattivo}`);
}
uguale([20, 500], [LARGHEZZA_TESTO_MIN, LARGHEZZA_TESTO_MAX], "limiti larghezza");

// LaTeX: solo classi e carte offerte, il resto ripiega sui predefiniti.
uguale("report", classeLatexValida("report"), "classe valida");
uguale("article", classeLatexValida("memoir"), "classe sconosciuta");
uguale("article", classeLatexValida(undefined), "classe assente");
uguale("letterpaper", cartaLatexValida("letterpaper"), "carta valida");
uguale("a4paper", cartaLatexValida("b5paper"), "carta sconosciuta");
uguale(["article", "report", "book"], [...CLASSI_LATEX], "classi offerte");
uguale(["a4paper", "a5paper", "letterpaper"], [...CARTE_LATEX], "carte offerte");

// Comando e argomenti per formato.
uguale("esporta_testo", comandoConversione("txt"), "comando txt");
uguale("esporta_html", comandoConversione("html"), "comando html");
uguale("esporta_latex", comandoConversione("tex"), "comando tex");
uguale("esporta_markdown", comandoConversione("md"), "comando md");
const base = { larghezza: 72, htmlAutonomo: true, latexClasse: "book", latexCarta: "a5paper" } as const;
uguale({ larghezza: 72 }, argomentiConversione("txt", base), "argomenti txt");
uguale({ autonomo: true }, argomentiConversione("html", base), "argomenti html");
uguale({ classe: "book", carta: "a5paper" }, argomentiConversione("tex", base), "argomenti tex");
uguale({}, argomentiConversione("md", base), "argomenti md");

// Messaggi di esito e di errore per le conversioni.
uguale("Esportato come HTML", messaggioEsito("html", 1), "esito html");
uguale("Esportato come testo semplice", messaggioEsito("txt", 1), "esito txt");
uguale(
  "Esportazione LaTeX non riuscita: boom",
  testoErroreEsportazione("tex", new Error("boom")),
  "errore tex",
);

// Avvisi: nulla da dire = nessun riquadro; elenco compatto; errori di sintassi in evidenza.
const esito = (avvisi: string[], errori_sintassi = false): EsitoConversione => ({ percorso: "x", avvisi, errori_sintassi });
const fmt: FormatoEsportazione = "html";
uguale(null, testoAvvisiConversione(fmt, esito([])), "nessun avviso");
uguale(
  "Esportazione HTML: 2 avvisi sulla conversione\n• a\n• b",
  testoAvvisiConversione(fmt, esito(["a", "b"])),
  "due avvisi",
);
const molti = Array.from({ length: MAX_AVVISI_MOSTRATI + 3 }, (_, i) => `v${i}`);
const t = testoAvvisiConversione("tex", esito(molti)) ?? "";
uguale(true, t.includes("… e altri 3") && !t.includes(`v${MAX_AVVISI_MOSTRATI}`), "elenco troncato");
const s1 = testoAvvisiConversione("txt", esito(["il sorgente contiene errori di sintassi: x", "b"], true)) ?? "";
uguale(true, s1.includes("errori di sintassi: la conversione è approssimata.") && s1.includes("• b"), "errori di sintassi in evidenza");
uguale(false, s1.includes("il sorgente contiene errori di sintassi: x"), "avviso del core non duplicato");
uguale(
  true,
  (testoAvvisiConversione("md", esito([], true)) ?? "").includes("errori di sintassi"),
  "solo errori di sintassi",
);

console.log(`opzioniesporta.check.ts: ${n} controlli OK`);
