/**
 * finerighe.ts — Fine riga dei file su disco (LF / CRLF), logica pura.
 *
 * CodeMirror ricompone il documento con `\n` (`doc.toString()`), quindi un file
 * CRLF aperto e salvato verrebbe riscritto LF: diff rumorosi e, per i file
 * versionati, ogni riga "modificata". Il gestore file rileva la fine riga alla
 * lettura e la riapplica al salvataggio; l'editor e il compilatore continuano
 * a vedere solo `\n`.
 */

/** Fine riga di un file: `\n` (Unix) o `\r\n` (Windows). */
export type FineRiga = "\n" | "\r\n";

/**
 * Rileva la fine riga prevalente di `testo`. Un `\n` preceduto da `\r` conta
 * come CRLF, gli altri come LF. File misti: vince la maggioranza; a parità
 * (compreso il testo senza righe) si resta su `\n`. I `\r` isolati non contano.
 */
export function rilevaFineRiga(testo: string): FineRiga {
  let crlf = 0;
  let lf = 0;
  let i = testo.indexOf("\n");
  while (i !== -1) {
    if (i > 0 && testo.charCodeAt(i - 1) === 13) {
      crlf++;
    } else {
      lf++;
    }
    i = testo.indexOf("\n", i + 1);
  }
  return crlf > lf ? "\r\n" : "\n";
}

/**
 * Riscrive `testo` (nella forma dell'editor, con soli `\n`) con la fine riga
 * indicata. Normalizza prima ogni `\r\n` a `\n`, così non nascono `\r\r\n` se un
 * `\r` fosse rimasto nel testo.
 */
export function applicaFineRiga(testo: string, fine: FineRiga): string {
  if (fine === "\n") {
    return testo;
  }
  return testo.replace(/\r\n/g, "\n").replace(/\n/g, "\r\n");
}

/**
 * Forma «dell'editor» di un testo: solo `\n` (CodeMirror normalizza così ogni
 * fine riga). Va passata a `editor.sostituisci`/`imposta` DOPO aver rilevato la
 * fine riga originale con `rilevaFineRiga`: confrontare un testo CRLF con quello
 * dell'editor farebbe risultare «diversa» ogni riga.
 */
export function normalizzaFineRiga(testo: string): string {
  return testo.replace(/\r\n?/g, "\n");
}

/**
 * Testo da registrare in cronologia per il documento corrente: la forma che
 * `salva_file` scrive su disco (testo dell'editor con la fine riga del file
 * riapplicata). Con soli `\n` su un file CRLF la versione non risulterebbe
 * «Identica» a quella salvata (duplicati a ogni ricarica) e la verifica di
 * integrità vedrebbe un troncamento.
 */
export function contenutoPerCronologia(testoEditor: string, fine: FineRiga): string {
  return applicaFineRiga(testoEditor, fine);
}
