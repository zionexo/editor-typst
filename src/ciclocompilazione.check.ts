/**
 * ciclocompilazione.check.ts — Verifica di `ciclocompilazione.ts` (correzione
 * «anteprima affamata»). Esce con errore al primo controllo fallito.
 */
import {
  creaCicloCompilazione,
  impostaTestoConRicompilazione,
  reagisciABattuta,
  reagisciASostituzione,
  ricompilaPerWorldCambiato,
} from "./ciclocompilazione.ts";
import { ritardoDebounce } from "./debounce.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

// 1. Modifiche durante la compilazione NON la rendono obsoleta.
{
  const c = creaCicloCompilazione();
  vero(c.richiedi() === "avvia", "prima richiesta avvia");
  const t = c.inizia();
  vero(c.richiedi() === "accoda", "richiesta in volo si accoda");
  const e = c.termina(t, false);
  vero(e.applica, "il risultato va applicato anche se si è digitato");
  vero(e.ricompila, "la richiesta accodata ricompila");
  vero(!c.inCorso(), "a fine ciclo non è in corso");
}

// 2. Una sostituzione del documento invalida il risultato, e ricompila.
{
  const c = creaCicloCompilazione();
  c.richiedi();
  const t = c.inizia();
  c.sostituzione();
  const e = c.termina(t, false);
  vero(!e.applica, "dopo una sostituzione il risultato è scartato");
  vero(e.ricompila, "scartato senza timer: ricompila subito (stato mai fermo)");
  const t2 = c.inizia();
  c.sostituzione();
  const e2 = c.termina(t2, true);
  vero(!e2.applica && !e2.ricompila, "scartato con timer di debounce in attesa: ci pensa il timer");
}

// 3. Simulazione: compilazione da 2 s, un tasto al secondo per 20 s.
{
  const DURATA = 2000;
  const c = creaCicloCompilazione();
  let timerScade: number | null = null;
  let fineCompilazione: number | null = null;
  let tokenInVolo = 0;
  let applicati = 0;
  let ultimaDurata: number | null = null;
  let tNow = 0;
  const avvia = (): void => {
    const t = c.inizia();
    tokenInVolo = t;
    fineCompilazione = tNow + DURATA;
  };
  const richiediCompilazione = (): void => {
    if (c.richiedi() === "avvia") {
      avvia();
    }
  };
  for (tNow = 0; tNow <= 30000; tNow += 10) {
    if (tNow <= 20000 && tNow % 1000 === 0) {
      // Tasto: la reazione è quella REALE di main.ts (`reagisciABattuta`); se tornasse
      // a invalidare il ciclo a ogni battuta, nessun risultato verrebbe più applicato.
      reagisciABattuta(c, {
        segnaModificato: () => {},
        programmaCompilazione: () => {
          timerScade = tNow + ritardoDebounce(300, ultimaDurata);
        },
      });
    }
    if (timerScade !== null && tNow >= timerScade) {
      timerScade = null;
      richiediCompilazione();
    }
    if (fineCompilazione !== null && tNow >= fineCompilazione) {
      fineCompilazione = null;
      ultimaDurata = DURATA;
      const e = c.termina(tokenInVolo, timerScade !== null);
      if (e.applica) {
        applicati++;
      }
      if (e.ricompila) {
        avvia();
      }
    }
  }
  vero(applicati >= 5, `l'anteprima si aggiorna durante la digitazione (aggiornamenti: ${applicati})`);
  vero(!c.inCorso(), "a riposo nessuna compilazione in corso");
}

// 4. Una sostituzione con testo identico non invalida; con testo diverso sì.
{
  const c = creaCicloCompilazione();
  c.richiedi();
  const t = c.inizia();
  reagisciASostituzione(c, "uguale", "uguale");
  vero(c.termina(t, false).applica, "sostituzione con testo identico: il risultato resta valido");
  const t2 = c.inizia();
  reagisciASostituzione(c, "prima", "dopo");
  vero(!c.termina(t2, false).applica, "sostituzione con testo diverso: risultato obsoleto");
}

// 5. Una battuta non tocca il ciclo (controllo diretto, non solo via simulazione).
{
  const c = creaCicloCompilazione();
  c.richiedi();
  const t = c.inizia();
  let chiamate = "";
  for (let i = 0; i < 50; i++) {
    reagisciABattuta(c, {
      segnaModificato: () => (chiamate += "m"),
      programmaCompilazione: () => (chiamate += "p"),
    });
  }
  vero(chiamate === "mp".repeat(50), "ogni battuta segna e programma, in quest'ordine");
  vero(c.termina(t, false).applica, "50 battute in volo non invalidano il risultato");
}

// 6. Apertura con testo identico: la ricompilazione parte comunque (una sola volta).
{
  let testo = "= Titolo";
  let richiesteDebounce = 0;
  const editorFinto = {
    testo: () => testo,
    imposta: (nuovo: string) => {
      const cambiato = nuovo !== testo;
      testo = nuovo;
      if (cambiato) {
        richiesteDebounce++; // docChanged -> onModifica -> programmaCompilazione
      }
    },
  };
  let esplicite = 0;
  impostaTestoConRicompilazione(editorFinto, "= Titolo", () => esplicite++);
  vero(esplicite === 1 && richiesteDebounce === 0, "testo identico: ricompilazione esplicita, nessun debounce");
  impostaTestoConRicompilazione(editorFinto, "= Altro", () => esplicite++);
  vero(esplicite === 1 && richiesteDebounce === 1, "testo diverso: ci pensa il debounce, niente doppia compilazione");
  impostaTestoConRicompilazione(editorFinto, "", () => esplicite++);
  vero(testo === "" && richiesteDebounce === 2 && esplicite === 1, "da testo pieno a vuoto: è una modifica");
  impostaTestoConRicompilazione(editorFinto, "", () => esplicite++);
  vero(esplicite === 2, "Nuovo su documento già vuoto: ricompila (World da azzerare)");
}

// 7. Compilazione in volo del documento precedente + riapertura con testo IDENTICO:
//    il risultato vecchio non va applicato e se ne avvia uno nuovo (World cambiato).
{
  const c = creaCicloCompilazione();
  let testo = "= Titolo";
  // Come main.ts: editor.imposta passa da reagisciASostituzione.
  const editorFinto = {
    testo: () => testo,
    imposta: (nuovo: string) => {
      reagisciASostituzione(c, testo, nuovo);
      testo = nuovo;
    },
  };
  let avvii = 0;
  const avvia = (): void => {
    if (c.richiedi() === "avvia") {
      avvii++;
    }
  };
  vero(c.richiedi() === "avvia", "prima compilazione: avvia");
  const t = c.inizia(); // compilazione in volo del documento precedente
  impostaTestoConRicompilazione(editorFinto, "= Titolo", () => ricompilaPerWorldCambiato(c, avvia));
  const esito = c.termina(t, false);
  vero(!esito.applica, "testo identico, World cambiato: il risultato in volo NON si applica");
  vero(esito.ricompila, "testo identico, World cambiato: si ricompila subito dopo");
  vero(avvii === 0, "durante il volo la nuova richiesta si accoda (nessuna compilazione sovrapposta)");
  // A riposo: la richiesta avvia direttamente.
  impostaTestoConRicompilazione(editorFinto, "= Titolo", () => ricompilaPerWorldCambiato(c, avvia));
  vero(avvii === 1, "a riposo: la ricompilazione parte");
}

console.log("ciclocompilazione.check.ts: OK");
