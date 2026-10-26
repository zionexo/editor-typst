/**
 * salvataggio.check.ts — Verifica di `salvataggio.ts` (stato del pallino, ambra minima,
 * annunci). Esce con errore al primo controllo fallito.
 */
import {
  AMBRA_MIN_MS,
  annunciare,
  avanza,
  statoSalvataggio,
  titoloSalvataggio,
  type IngressiSalvataggio,
  type Mostrato,
  type StatoSalvataggio,
} from "./salvataggio.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

// 1. Tabella completa: tutte le 16 combinazioni.
{
  const atteso = (i: IngressiSalvataggio): StatoSalvataggio => {
    if (i.inSalvataggio) return "in-corso";
    if (i.errore || i.modificato) return "non-salvato";
    return i.haPercorso ? "salvato" : "nuovo";
  };
  let n = 0;
  for (const modificato of [false, true]) {
    for (const inSalvataggio of [false, true]) {
      for (const errore of [false, true]) {
        for (const haPercorso of [false, true]) {
          const i = { modificato, inSalvataggio, errore, haPercorso };
          uguali(statoSalvataggio(i), atteso(i), JSON.stringify(i));
          n += 1;
        }
      }
    }
  }
  vero(n === 16, "16 combinazioni");
}

// 2. Casi chiave, uno per uno.
{
  const base = { modificato: false, inSalvataggio: false, errore: false, haPercorso: true };
  uguali(statoSalvataggio(base), "salvato", "salvato");
  uguali(statoSalvataggio({ ...base, modificato: true }), "non-salvato", "modificato");
  uguali(statoSalvataggio({ ...base, modificato: true, inSalvataggio: true }), "in-corso", "in scrittura");
  uguali(statoSalvataggio({ ...base, modificato: true, errore: true }), "non-salvato", "errore");
  uguali(statoSalvataggio({ ...base, modificato: true, errore: true, inSalvataggio: true }), "in-corso", "nuovo tentativo dopo errore");
  uguali(statoSalvataggio({ ...base, haPercorso: false }), "nuovo", "nuovo documento intatto");
  uguali(statoSalvataggio({ ...base, haPercorso: false, modificato: true }), "non-salvato", "nuovo documento con contenuto");
  uguali(statoSalvataggio({ ...base, haPercorso: false, inSalvataggio: true }), "in-corso", "salva con nome in corso");
}

// 3. Titoli: parole distinte per ogni stato.
{
  uguali(titoloSalvataggio("salvato"), "Salvato", "titolo salvato");
  uguali(titoloSalvataggio("in-corso"), "Salvataggio in corso", "titolo in corso");
  uguali(titoloSalvataggio("non-salvato"), "Modifiche non salvate", "titolo non salvato");
  uguali(titoloSalvataggio("nuovo"), "Nuovo documento, non ancora salvato", "titolo nuovo");
  const tutti = (["salvato", "in-corso", "non-salvato", "nuovo"] as const).map(titoloSalvataggio);
  vero(new Set(tutti).size === 4, "titoli distinti");
}

// 4. Ambra minima: non si lascia prima di AMBRA_MIN_MS; poi sì.
{
  vero(AMBRA_MIN_MS >= 400, "almeno 400 ms");
  const primo = avanza(null, "non-salvato", 1000);
  uguali(primo.mostrato, { stato: "non-salvato", dal: 1000 }, "primo disegno");
  vero(primo.attesaMs === null, "primo disegno senza attesa");
  // Entrata nell'ambra: immediata.
  const dentro = avanza(primo.mostrato, "in-corso", 2000);
  uguali(dentro.mostrato, { stato: "in-corso", dal: 2000 }, "ambra subito");
  // Fine scrittura dopo 50 ms: si trattiene per il resto.
  const presto = avanza(dentro.mostrato, "salvato", 2050);
  uguali(presto.mostrato.stato, "in-corso", "trattenuta");
  uguali(presto.attesaMs, AMBRA_MIN_MS - 50, "attesa residua");
  uguali(presto.mostrato.dal, 2000, "l'istante di inizio non si sposta");
  // Allo scadere: passa allo stato voluto.
  const scaduto = avanza(presto.mostrato, "salvato", 2000 + AMBRA_MIN_MS);
  uguali(scaduto.mostrato, { stato: "salvato", dal: 2000 + AMBRA_MIN_MS }, "dopo il minimo");
  vero(scaduto.attesaMs === null, "nessuna ulteriore attesa");
  // Scrittura lunga: nessun trattenimento.
  const lunga = avanza(dentro.mostrato, "non-salvato", 5000);
  uguali(lunga.mostrato.stato, "non-salvato", "scrittura lunga, esito subito");
  // Stesso stato: nulla cambia e l'istante resta.
  const stesso = avanza({ stato: "salvato", dal: 10 }, "salvato", 999);
  uguali(stesso.mostrato, { stato: "salvato", dal: 10 }, "stesso stato");
  // Solo l'ambra si trattiene: altri stati passano subito.
  const altro = avanza({ stato: "non-salvato", dal: 100 } as Mostrato, "salvato", 101);
  uguali(altro.mostrato.stato, "salvato", "il rosso non si trattiene");
  // Cambi intermedi durante la trattenuta: conta l'ultimo voluto all'ora di rivalutare.
  const a = avanza(dentro.mostrato, "salvato", 2100);
  const b = avanza(a.mostrato, "non-salvato", 2200);
  uguali(b.mostrato.stato, "in-corso", "ancora trattenuta");
  const c = avanza(b.mostrato, "non-salvato", 2000 + AMBRA_MIN_MS);
  uguali(c.mostrato.stato, "non-salvato", "alla scadenza va l'ultimo voluto");
}

// 5. Annunci: solo esiti, solo se diversi dall'ultimo annunciato.
{
  vero(!annunciare("salvato", "salvato"), "stesso esito");
  vero(annunciare("salvato", "non-salvato"), "salvato → non salvato");
  vero(annunciare("non-salvato", "salvato"), "non salvato → salvato");
  vero(!annunciare("non-salvato", "in-corso"), "l'ambra non si annuncia");
  vero(!annunciare("salvato", "nuovo"), "il neutro non si annuncia");
  vero(!annunciare("non-salvato", "non-salvato"), "ciclo autosave: resta non salvato, nessun annuncio");
  vero(annunciare(null, "salvato"), "nessun annuncio precedente");
}

console.log("salvataggio.check: OK");
