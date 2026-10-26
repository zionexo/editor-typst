/**
 * finerighe.check.ts — Verifica eseguibile di `finerighe.ts` (fine riga LF/CRLF).
 * Stesso schema di `modalita.check.ts`; esce con errore al primo controllo fallito.
 */
import { applicaFineRiga, contenutoPerCronologia, normalizzaFineRiga, rilevaFineRiga } from "./finerighe.ts";
import { sostituzioneMinima } from "./modificheesterne.ts";

function uguale<T>(atteso: T, reale: T, msg: string): void {
  if (JSON.stringify(atteso) !== JSON.stringify(reale)) {
    throw new Error(`FALLITO: ${msg} (atteso ${JSON.stringify(atteso)}, ottenuto ${JSON.stringify(reale)})`);
  }
}

// Rilevamento.
uguale("\n", rilevaFineRiga(""), "vuoto -> LF");
uguale("\n", rilevaFineRiga("una riga"), "senza a capo -> LF");
uguale("\n", rilevaFineRiga("a\nb\nc"), "LF");
uguale("\r\n", rilevaFineRiga("a\r\nb\r\nc"), "CRLF");
uguale("\r\n", rilevaFineRiga("a\r\nb\r\nc\nd"), "misto, maggioranza CRLF");
uguale("\n", rilevaFineRiga("a\nb\nc\r\nd"), "misto, maggioranza LF");
uguale("\n", rilevaFineRiga("a\r\nb\nc"), "parità -> LF");
uguale("\n", rilevaFineRiga("a\rb\rc"), "\r isolati ignorati");
uguale("\r\n", rilevaFineRiga("\r\n"), "solo un CRLF");

// Riapplicazione.
uguale("a\nb", applicaFineRiga("a\nb", "\n"), "LF invariato");
uguale("a\r\nb\r\n", applicaFineRiga("a\nb\n", "\r\n"), "LF -> CRLF");
uguale("a\r\nb", applicaFineRiga("a\r\nb", "\r\n"), "niente \r\r\n");

// Round-trip di un file CRLF passando dalla forma dell'editor (soli \n).
const originale = "riga 1\r\nriga 2\r\n\r\nfine\r\n";
const nelEditor = originale.replace(/\r\n?|\n/g, "\n");
uguale(originale, applicaFineRiga(nelEditor, rilevaFineRiga(originale)), "round-trip CRLF");

// Normalizzazione verso la forma dell'editor.
uguale("a\nb\nc\n", normalizzaFineRiga("a\r\nb\rc\n"), "CRLF e CR -> LF");
uguale("a\n\nb", normalizzaFineRiga("a\r\n\r\nb"), "righe vuote CRLF");
uguale("", normalizzaFineRiga(""), "vuoto");
// Il ricaricamento di un file CRLF uguale all'editor non deve produrre differenze.
uguale(nelEditor, normalizzaFineRiga(originale), "originale CRLF normalizzato = testo dell'editor");

// Regressione: ricaricare da disco un file CRLF (modificato in coda) deve
// toccare solo il tratto diverso, non quasi tutto il documento.
{
  const vecchio = "riga uno\nriga due\nriga tre\n";
  const disco = "riga uno\r\nriga due\r\nriga tre\r\nriga quattro\r\n";
  const grezzo = sostituzioneMinima(vecchio, disco);
  const norm = sostituzioneMinima(vecchio, normalizzaFineRiga(disco));
  uguale(true, grezzo.a - grezzo.da > 0, "(documentazione) il testo grezzo CRLF sostituisce parte del documento");
  uguale({ da: vecchio.length, a: vecchio.length, inserisci: "riga quattro\n" }, norm, "normalizzato: solo la riga aggiunta");
}

// Coerenza con `salva_file`: la cronologia registra ESATTAMENTE i byte che
// finiscono su disco (testo dell'editor con la fine riga del file riapplicata).
// Con LF grezzo su un file CRLF la versione non risulterebbe «Identica» e
// `verifica_integrita` vedrebbe un troncamento.
{
  const disco = "uno\r\ndue\r\ntre\r\n";
  const fine = rilevaFineRiga(disco);
  const editor = normalizzaFineRiga(disco);
  const sulDisco = applicaFineRiga(editor, fine); // ciò che scrive salva_file
  uguale(sulDisco, contenutoPerCronologia(editor, fine), "cronologia = byte scritti su disco (CRLF)");
  uguale(disco, contenutoPerCronologia(editor, fine), "ricarica/«Non salvare»: stessa versione del disco");
  uguale(true, contenutoPerCronologia(editor, fine) !== editor, "il testo LF dell'editor NON è ciò che va in cronologia su un CRLF");
  uguale("a\nb\n", contenutoPerCronologia("a\nb\n", "\n"), "LF: invariato");
}

console.log("finerighe.check.ts: OK");
