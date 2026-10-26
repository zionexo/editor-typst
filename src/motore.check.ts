/**
 * motore.check.ts — Verifica eseguibile di `motore.ts`. Esce con errore al primo
 * controllo fallito (stesso schema degli altri `*.check.ts`).
 */
import {
  MOTORE_PREDEFINITO,
  MOTORI,
  descrizioneMotore,
  devePersistereMotore,
  etichettaMotore,
  motoreValido,
  normalizzaMotore,
  titoloComandoMotore,
} from "./motore.ts";

function uguale<T>(atteso: T, reale: T, msg: string): void {
  if (atteso !== reale) {
    throw new Error(`FALLITO: ${msg} (atteso ${String(atteso)}, ottenuto ${String(reale)})`);
  }
}

// Valori ammessi e predefinito.
uguale(2, MOTORI.length, "due motori");
uguale("lezer", MOTORE_PREDEFINITO, "il predefinito è lezer");
uguale("lezer", MOTORI[0], "lezer è il primo");
uguale("wasm", MOTORI[1], "wasm è il secondo");

// Validazione / normalizzazione: tutto ciò che non è un motore noto cade su lezer.
for (const m of MOTORI) {
  uguale(true, motoreValido(m), `${m} è valido`);
  uguale(m, normalizzaMotore(m), `${m} resta invariato`);
}
for (const v of ["", "WASM", "Lezer", " lezer", "altro", null, undefined, 3, true, {}, []]) {
  uguale(false, motoreValido(v), `valore non valido: ${JSON.stringify(v)}`);
  uguale("lezer", normalizzaMotore(v), `normalizza ${JSON.stringify(v)}`);
}

// Etichette, descrizioni e titoli dei comandi: presenti, non vuoti e distinti.
const viste = new Set<string>();
for (const m of MOTORI) {
  uguale(true, etichettaMotore(m).length > 0, `etichetta di ${m}`);
  uguale(true, descrizioneMotore(m).length > 10, `descrizione di ${m}`);
  uguale(true, titoloComandoMotore(m).startsWith("Motore Typst: "), `titolo del comando di ${m}`);
  viste.add(etichettaMotore(m));
  viste.add(descrizioneMotore(m));
  viste.add(titoloComandoMotore(m));
}
uguale(6, viste.size, "etichette, descrizioni e titoli distinti");
uguale("WASM", etichettaMotore("wasm"), "etichetta wasm");
uguale("Lezer (predefinito)", etichettaMotore("lezer"), "etichetta lezer");
uguale("Motore Typst: WASM", titoloComandoMotore("wasm"), "titolo comando wasm");
uguale("Motore Typst: Lezer", titoloComandoMotore("lezer"), "titolo comando lezer");

// Persistenza: solo se il valore cambia rispetto al salvato, o se le impostazioni non sono caricate.
uguale(false, devePersistereMotore("wasm", "wasm", true), "stesso motore a impostazioni caricate: niente scrittura");
uguale(true, devePersistereMotore("lezer", "wasm", true), "motore diverso: scrive");
uguale(true, devePersistereMotore("lezer", "lezer", false), "prima del caricamento: scrive comunque");
uguale(false, devePersistereMotore("lezer", "valore-ignoto", true), "valore ignoto = lezer = invariato");
uguale(true, devePersistereMotore("wasm", "valore-ignoto", true), "valore ignoto cade su lezer: cambia");
// Dopo un salvataggio fallito `salvato` resta quello vecchio: la scelta uguale si riprova.
uguale(true, devePersistereMotore("lezer", "wasm", true), "salvataggio fallito: nuovo tentativo");

console.log("motore.check.ts: OK");
