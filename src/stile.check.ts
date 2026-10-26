/**
 * stile.check.ts — Verifica eseguibile di `stile.ts`. Esce con errore al primo
 * controllo fallito (stesso schema degli altri `*.check.ts`).
 */
import {
  STILE_PREDEFINITO,
  STILI,
  descrizioneStile,
  etichettaStile,
  navigaIndice,
  navigaStili,
  normalizzaStile,
  stileSuccessivo,
  stileValido,
  stileDaSpecchio,
  devePersistereStile,
  radioDaAttivare,
  focusEsceDalMenu,
} from "./stile.ts";

function uguale<T>(atteso: T, reale: T, msg: string): void {
  if (atteso !== reale) {
    throw new Error(`FALLITO: ${msg} (atteso ${String(atteso)}, ottenuto ${String(reale)})`);
  }
}

// Valori ammessi e predefinito.
uguale(3, STILI.length, "tre stili");
uguale("classico", STILE_PREDEFINITO, "il predefinito è il classico");
uguale("classico", STILI[0], "il classico è il primo");

// Validazione / normalizzazione: tutto ciò che non è uno stile noto cade sul classico.
for (const s of STILI) {
  uguale(true, stileValido(s), `${s} è valido`);
  uguale(s, normalizzaStile(s), `${s} resta invariato`);
}
for (const v of ["", "Classico", " strumento", "altro", null, undefined, 3, {}, []]) {
  uguale(false, stileValido(v), `valore non valido: ${JSON.stringify(v)}`);
  uguale("classico", normalizzaStile(v), `normalizza ${JSON.stringify(v)}`);
}

// Ciclo: avanti, indietro, passi grandi.
uguale("composizione", stileSuccessivo("classico"), "classico → composizione");
uguale("strumento", stileSuccessivo("composizione"), "composizione → strumento");
uguale("classico", stileSuccessivo("strumento"), "strumento → classico (giro)");
uguale("strumento", stileSuccessivo("classico", -1), "classico ← strumento (indietro)");
uguale("classico", stileSuccessivo("composizione", -1), "composizione ← classico");
uguale("composizione", stileSuccessivo("classico", 4), "passo 4 = passo 1");
uguale("strumento", stileSuccessivo("classico", -4), "passo -4 = passo -1");
uguale("classico", stileSuccessivo("classico", 3), "tre passi = giro intero");

// Etichette e descrizioni: presenti, non vuote e distinte.
const viste = new Set<string>();
for (const s of STILI) {
  uguale(true, etichettaStile(s).length > 0, `etichetta di ${s}`);
  uguale(true, descrizioneStile(s).length > 10, `descrizione di ${s}`);
  viste.add(etichettaStile(s));
  viste.add(descrizioneStile(s));
}
uguale(6, viste.size, "etichette e descrizioni distinte");
uguale("Classico", etichettaStile("classico"), "etichetta classico");

// Navigazione da tastiera del gruppo di schede.
uguale("composizione", navigaStili("classico", "ArrowRight"), "→");
uguale("composizione", navigaStili("classico", "ArrowDown"), "↓");
uguale("strumento", navigaStili("classico", "ArrowLeft"), "← dal primo torna all'ultimo");
uguale("strumento", navigaStili("classico", "ArrowUp"), "↑ dal primo torna all'ultimo");
uguale("classico", navigaStili("strumento", "ArrowRight"), "→ dall'ultimo torna al primo");
uguale("classico", navigaStili("strumento", "Home"), "Home");
uguale("strumento", navigaStili("classico", "End"), "End");
uguale(null, navigaStili("classico", "Enter"), "Invio non sposta");
uguale(null, navigaStili("classico", "a"), "una lettera non sposta");

// Navigazione generica (gruppo dei temi, 3 voci).
uguale(1, navigaIndice(3, 0, "ArrowRight") ?? -1, "indice →");
uguale(2, navigaIndice(3, 0, "ArrowLeft") ?? -1, "indice ← dal primo");
uguale(0, navigaIndice(3, 2, "ArrowDown") ?? -1, "indice ↓ dall'ultimo");
uguale(0, navigaIndice(3, 1, "Home") ?? -1, "indice Home");
uguale(2, navigaIndice(3, 1, "End") ?? -1, "indice End");
uguale(null, navigaIndice(3, 1, " "), "spazio non sposta");
uguale(null, navigaIndice(0, 0, "ArrowRight"), "gruppo vuoto");

// (6) Specchio dello stile in localStorage: si fida solo di valori validi (mai del testo grezzo).
for (const s of STILI) {
  uguale(s, stileDaSpecchio(s) ?? "nessuno", `specchio valido ${s}`);
}
for (const v of ["", "Classico", "altro", "<script>", null, undefined, 3, {}]) {
  uguale(null, stileDaSpecchio(v), `specchio non valido: ${JSON.stringify(v)} non si applica`);
}

// (7) Persistenza: solo se il valore cambia, o se le impostazioni non sono ancora caricate.
uguale(false, devePersistereStile("composizione", "composizione", true), "stesso stile a impostazioni caricate: niente scrittura");
uguale(true, devePersistereStile("composizione", "strumento", true), "stile diverso: scrive");
uguale(true, devePersistereStile("classico", "classico", false), "prima del caricamento: scrive comunque");
uguale(false, devePersistereStile("classico", "valore-ignoto" as never, true), "valore ignoto = classico = invariato");
uguale(true, devePersistereStile("strumento", "valore-ignoto" as never, true), "valore ignoto cade sul classico: cambia");
uguale(false, radioDaAttivare("true"), "Invio sul radio già selezionato: niente clic");
uguale(true, radioDaAttivare("false"), "Invio su radio non selezionato: clic");
uguale(true, radioDaAttivare(null), "senza aria-checked: clic");

// (3) Focus che esce dal popover: si chiude solo se la destinazione è un elemento esterno.
uguale(true, focusEsceDalMenu("fuori"), "destinazione esterna: chiude");
uguale(false, focusEsceDalMenu("pannello"), "destinazione nel pannello: resta");
uguale(false, focusEsceDalMenu("bottone"), "destinazione sul pulsante: resta (alterna lui)");
uguale(false, focusEsceDalMenu(null), "nessuna destinazione (clic su body/finestra): decide il pointerdown");

console.log("stile.check.ts: OK");
