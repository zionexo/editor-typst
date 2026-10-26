/**
 * comandi.check.ts — Verifica eseguibile della logica pura di `comandi.ts` (F5-006).
 *
 * Nel repo non c'è un runner di test TypeScript: questo è un controllo minimo
 * senza dipendenze, eseguibile con Node >= 22.18 (che rimuove i tipi da solo);
 * con Node 22.6-22.17 serve `node --experimental-strip-types src/comandi.check.ts`:
 *
 *   node src/comandi.check.ts
 *
 * Esce con codice 1 al primo controllo fallito. Non è importato dall'app.
 */
import {
  analizzaScorciatoia,
  canonica,
  corrisponde,
  type EventoTasto,
  creaRegistro,
  filtraFuzzy,
  formattaScorciatoia,
  punteggioFuzzy,
  type TastoEvento,
} from "./comandi.ts";

let n = 0;
function ok(cond: boolean, msg: string): void {
  n++;
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

function tasto(p: Partial<TastoEvento>): TastoEvento {
  return { key: "", code: "", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...p };
}
const sc = analizzaScorciatoia;
/** `corrisponde` su Windows/Linux (Mod = Ctrl) o macOS (Mod = Cmd). */
const win = (a: ReturnType<typeof sc>, e: TastoEvento): boolean => corrisponde(a, e, false);
const mac = (a: ReturnType<typeof sc>, e: TastoEvento): boolean => corrisponde(a, e, true);

// Scorciatoie: Ctrl e Cmd equivalenti, Shift/Alt esatti per le lettere.
ok(win(sc("Mod+S"), tasto({ key: "s", code: "KeyS", ctrlKey: true })), "Ctrl+S");
ok(mac(sc("Mod+S"), tasto({ key: "s", code: "KeyS", metaKey: true })), "Cmd+S su mac");
ok(!mac(sc("Mod+S"), tasto({ key: "s", code: "KeyS", ctrlKey: true })), "Ctrl+S non è Mod su mac");
ok(!win(sc("Mod+S"), tasto({ key: "s", code: "KeyS", metaKey: true })), "Win+S non è Mod su Windows");
ok(!win(sc("Mod+S"), tasto({ key: "s", code: "KeyS", ctrlKey: true, metaKey: true })), "Ctrl+Meta escluso");
ok(
  !win(sc("Mod+S"), tasto({ key: "S", code: "KeyS", ctrlKey: true, shiftKey: true })),
  "Ctrl+Shift+S diverso da Ctrl+S",
);
ok(
  win(sc("Mod+Shift+O"), tasto({ key: "O", code: "KeyO", ctrlKey: true, shiftKey: true })),
  "Ctrl+Shift+O",
);
ok(!win(sc("Mod+S"), tasto({ key: "s", code: "KeyS" })), "S senza Mod");
ok(
  !win(sc("Mod+S"), tasto({ key: "s", code: "KeyS", ctrlKey: true, altKey: true })),
  "Alt esatto",
);
ok(win(sc("Mod+S"), tasto({ key: "ы", code: "KeyS", ctrlKey: true })), "layout cirillico");
// Font: zero da code, punteggiatura con Shift ignorato.
ok(win(sc("Mod+0"), tasto({ key: "0", code: "Numpad0", ctrlKey: true })), "Ctrl+0 tastierino");
ok(
  win(sc("Mod++"), tasto({ key: "+", code: "Equal", ctrlKey: true, shiftKey: true })),
  "Ctrl++",
);
ok(win(sc("Mod+="), tasto({ key: "=", code: "Equal", ctrlKey: true })), "Ctrl+=");
ok(mac(sc("Mod+-"), tasto({ key: "-", code: "Minus", metaKey: true })), "Cmd+- su mac");
ok(analizzaScorciatoia("Mod++").chiave === "+", "parsing Mod++");
ok(canonica(sc("Mod+Shift++")) === canonica(sc("Mod++")), "canonica: Shift ignorato su punteggiatura");
ok(canonica(sc("Mod+Shift+O")) !== canonica(sc("Mod+O")), "canonica: Shift conta sulle lettere");
ok(formattaScorciatoia("Mod+Shift+O", false) === "Ctrl+Shift+O", "formato Win");
ok(formattaScorciatoia("Mod+Shift+O", true) === "⇧⌘O", "formato mac");
ok(formattaScorciatoia("Mod++", false) === "Ctrl++", "formato Ctrl++");
function lancia(fn: () => void): boolean {
  try {
    fn();
    return false;
  } catch {
    return true;
  }
}
ok(lancia(() => analizzaScorciatoia("Ctl+S")), "modificatore sconosciuto rifiutato");

// Fuzzy: sottostringa > sottosequenza, diacritici, AND sulle parole, ordine stabile.
ok(
  (punteggioFuzzy("sal", "Salva il documento") ?? 0) >
    (punteggioFuzzy("sld", "Salva il documento") ?? 0),
  "sottostringa batte sottosequenza",
);
ok(punteggioFuzzy("xyz", "Salva") === null, "nessuna corrispondenza");
ok(punteggioFuzzy("cartella", "Apri cartella progetto") !== null, "parola interna");
ok(punteggioFuzzy("apri cart", "Apri cartella") !== null, "più parole");
ok(punteggioFuzzy("apri zzz", "Apri cartella") === null, "AND fra parole");
ok(punteggioFuzzy("e", "è") !== null, "diacritici");
ok(punteggioFuzzy("", "qualsiasi") === 0, "query vuota");
const voci = ["Esporta PDF", "Apri", "Apri cartella", "Salva"];
ok(filtraFuzzy(voci, "", (v) => v).join() === voci.join(), "query vuota: ordine invariato");
ok(filtraFuzzy(voci, "apri", (v) => v).join() === "Apri,Apri cartella", "filtro e ordine stabile");
ok(
  filtraFuzzy(["Nuova cartella", "Apri cartella"], "apri", (v) => v).join() === "Apri cartella",
  "solo le corrispondenti",
);
ok(filtraFuzzy(["Esporta il documento", "Salva"], "sal", (v) => v)[0] === "Salva", "sottostringa batte sottosequenza");

// Registro: duplicati, abilitazione, errori asincroni.
const errori: unknown[] = [];
const r = creaRegistro((e) => errori.push(e), false);
let eseguito = 0;
r.registra({ id: "a.uno", titolo: "Uno", scorciatoie: ["Mod+U"], esegui: () => void eseguito++ });
r.registra({ id: "a.due", titolo: () => "Due", abilitato: () => false, esegui: () => void eseguito++ });
ok(lancia(() => r.registra({ id: "a.uno", titolo: "x" })), "id duplicato");
ok(
  lancia(() => r.registra({ id: "a.tre", titolo: "x", scorciatoie: ["Mod+U"] })),
  "scorciatoia duplicata",
);
ok(r.abilitati().length === 1, "abilitati esclude i disabilitati");
r.esegui("a.due");
ok(eseguito === 0, "comando disabilitato non eseguito");
r.esegui("a.uno");
ok(eseguito === 1, "comando eseguito");
r.registra({ id: "a.ko", titolo: "Ko", esegui: () => Promise.reject(new Error("boom")) });
r.esegui("a.ko");
await Promise.resolve();
await Promise.resolve();
ok(errori.length === 1, "rifiuto asincrono inoltrato a onErrore");

// Dispatcher: eseguiSolo blocca l'esecuzione ma NON il preventDefault.
function evento(p: Partial<EventoTasto>): EventoTasto & { bloccato: boolean } {
  const e = {
    ...tasto({ key: "u", code: "KeyU", ctrlKey: true }),
    repeat: false,
    isComposing: false,
    bloccato: false,
    preventDefault() {
      e.bloccato = true;
    },
    ...p,
  };
  return e;
}
eseguito = 0;
const e1 = evento({});
ok(r.gestisciTasto(e1) && e1.bloccato && eseguito === 1, "gestisciTasto esegue e blocca");
const e2 = evento({});
ok(r.gestisciTasto(e2, "altro.id") && e2.bloccato && eseguito === 1, "eseguiSolo: blocca senza eseguire");
const e3 = evento({});
ok(r.gestisciTasto(e3, "a.uno") && eseguito === 2, "eseguiSolo uguale all'id: esegue");
const e4 = evento({ repeat: true });
ok(r.gestisciTasto(e4) && e4.bloccato && eseguito === 2, "tasto ripetuto: blocca senza rieseguire");
const e5 = evento({ key: "x", code: "KeyX" });
ok(!r.gestisciTasto(e5) && !e5.bloccato, "nessuna corrispondenza: evento intatto");

console.log(`comandi.check.ts: ${n} controlli OK`);
