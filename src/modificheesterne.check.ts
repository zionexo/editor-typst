/**
 * modificheesterne.check.ts — Verifica eseguibile di `modificheesterne.ts` (F5-022).
 * Stesso schema di `debounce.check.ts`; esce con errore al primo controllo fallito.
 */
import { rilevaWindows } from "./comandi.ts";
import {
  azioneDopoSparito,
  discoInvariato,
  ricaricaSuperflua,
  avvisoTipoDopoRinomina,
  confrontaPercorsi,
  motivoNonRipristinato,
  normalizzaPercorso,
  sostituzioneMinima,
  stessoPercorso,
} from "./modificheesterne.ts";

function uguale<T>(atteso: T, reale: T, msg: string): void {
  if (JSON.stringify(atteso) !== JSON.stringify(reale)) {
    throw new Error(
      `FALLITO: ${msg} (atteso ${JSON.stringify(atteso)}, ottenuto ${JSON.stringify(reale)})`,
    );
  }
}

/** Applica una sostituzione a `testo`. */
function applica(testo: string, s: { da: number; a: number; inserisci: string }): string {
  return testo.slice(0, s.da) + s.inserisci + testo.slice(s.a);
}

// Sostituzione minima: applicandola al vecchio si ottiene sempre il nuovo.
const casi: [string, string][] = [
  ["", ""],
  ["", "nuovo"],
  ["vecchio", ""],
  ["uguale", "uguale"],
  ["= Titolo\nprimo\nterzo", "= Titolo\nprimo\nsecondo\nterzo"],
  ["aaa", "aaaa"],
  ["abcabc", "abc"],
  ["x😀y", "x😃y"], // emoji diverse: stessa unità alta, bassa diversa
  ["😀", "😃"],
  ["a😀", "a😀😀"],
  ["ciao mondo", "ciao a tutti mondo"],
];
for (const [v, n] of casi) {
  uguale(n, applica(v, sostituzioneMinima(v, n)), `applica(${JSON.stringify(v)} → ${JSON.stringify(n)})`);
}

// È davvero minima: cambia solo il tratto differente (cursore altrove intatto).
uguale(
  { da: 15, a: 15, inserisci: "secondo\n" },
  sostituzioneMinima("= Titolo\nprimo\nterzo", "= Titolo\nprimo\nsecondo\nterzo"),
  "inserimento in mezzo",
);
uguale({ da: 6, a: 6, inserisci: "" }, sostituzioneMinima("uguale", "uguale"), "testi uguali: intervallo vuoto, nulla da inserire");

// Nessun taglio in mezzo a una coppia surrogata.
const s = sostituzioneMinima("x😀y", "x😃y");
uguale({ da: 1, a: 3, inserisci: "😃" }, s, "emoji sostituita per intero, mai a metà");

// Percorsi: dipendono dalla piattaforma, passata esplicitamente (windows = true/false),
// come nel core Rust: minuscolo e backslash → `/` solo su Windows.
// Percorsi in stile Windows: si costruiscono dai separatori `/` per non dipendere dagli escape.
const win = (p: string): string => p.split("/").join(String.fromCharCode(92));
// -- Windows: forme diverse dello stesso file coincidono.
uguale("c:/prog/doc.typ", normalizzaPercorso(win("//?/C:/Prog/Doc.typ"), true), "win: prefisso verbatim e maiuscole");
uguale("//srv/cond/f.typ", normalizzaPercorso(win("//?/UNC/srv/cond/f.typ"), true), "win: UNC verbatim");
uguale("c:/prog", normalizzaPercorso(win("C:/Prog/"), true), "win: barra finale tolta");
uguale(true, stessoPercorso(win("C:/Prog/doc.typ"), "c:/prog/DOC.typ", true), "win: stesso file");
uguale(false, stessoPercorso(win("C:/Prog/doc.typ"), win("C:/Prog/altro.typ"), true), "win: file diversi");
// -- macOS e Linux: maiuscole e backslash contano, nessuna conversione.
const conBs = "/home/u/a" + String.fromCharCode(92) + "b.typ"; // nome con un backslash
for (const [nome, windows] of [["linux", false], ["macOS", false]] as const) {
  uguale("/Home/Utente/Doc.typ", normalizzaPercorso("/Home/Utente/Doc.typ", windows), `${nome}: maiuscole conservate`);
  uguale(conBs, normalizzaPercorso(conBs, windows), `${nome}: backslash è un carattere del nome`);
  uguale("/home/u", normalizzaPercorso("/home/u/", windows), `${nome}: barra finale tolta`);
  uguale("/", normalizzaPercorso("/", windows), `${nome}: la radice resta`);
  uguale(false, stessoPercorso("/home/u/Doc.typ", "/home/u/doc.typ", windows), `${nome}: Doc.typ e doc.typ sono file diversi`);
  uguale(true, stessoPercorso("/home/u/doc.typ", "/home/u/doc.typ/", windows), `${nome}: stesso file`);
  uguale(false, stessoPercorso(conBs, "/home/u/a/b.typ", windows), `${nome}: a-backslash-b e a/b sono diversi`);
}
// -- Il default segue la piattaforma rilevata: coincide con l'esplicito.
uguale(
  stessoPercorso("/tmp/A.typ", "/tmp/a.typ", rilevaWindows()),
  stessoPercorso("/tmp/A.typ", "/tmp/a.typ"),
  "default = piattaforma rilevata",
);
uguale(false, stessoPercorso(null, win("C:/a"), true), "null non coincide con nulla");
uguale(false, stessoPercorso(null, null, false), "null e null non coincidono");

// -- Windows, casi limite (punto 2 dell'ultimo giro): stesso file in forme diverse.
const eq = (a: string, b: string): boolean => stessoPercorso(win(a), win(b), true);
uguale(true, eq("//?/unc/Srv/Cond/f.typ", "//srv/cond/F.typ"), "win: prefisso UNC verbatim in minuscolo");
uguale(true, eq("//./C:/p/f.typ", "c:/p/f.typ"), "win: prefisso di dispositivo");
uguale(true, eq("C://p///f.typ", "c:/p/f.typ"), "win: separatori ripetuti");
uguale(true, eq("C:/p/./sub/../f.typ", "c:/p/f.typ"), "win: segmenti . e ..");
uguale(true, eq("c:/p/f.typ/", "C:/P/F.TYP"), "win: barra finale e maiuscole");
uguale(true, eq("//srv/cond//p/f.typ", "//?/UNC/srv/cond/p/f.typ"), "win: UNC con barra doppia interna");
uguale(false, eq("//srv/cond/f.typ", "/srv/cond/f.typ"), "win: UNC e radice semplice non coincidono");
uguale(false, eq("C:/p/f.typ", "D:/p/f.typ"), "win: unità diverse");
uguale(false, eq("C:/../../f.typ", "C:/f2.typ"), "win: .. non esce dalla radice, ma il file è altro");
uguale(true, eq("C:/../f.typ", "C:/f.typ"), "win: .. oltre la radice si ferma alla radice");
// Tre esiti: uguale / incerto (forma diversa, stesso nome: si chiede, non si ignora) / diverso.
uguale("uguale", confrontaPercorsi(win("//?/C:/P/a.typ"), win("c:/p/a.typ"), true), "esito: uguale");
uguale(
  "incerto",
  confrontaPercorsi(win("Z:/progetti/a.typ"), win("//?/UNC/srv/share/progetti/a.typ"), true),
  "esito: unità di rete mappata vs UNC (stesso nome, forma diversa) è incerto, non diverso",
);
uguale("incerto", confrontaPercorsi(win("X:/a.typ"), win("C:/lavoro/a.typ"), true), "esito: subst/giunzione: incerto");
uguale("diverso", confrontaPercorsi(win("C:/p/a.typ"), win("C:/p/b.typ"), true), "esito: nomi diversi");
uguale("diverso", confrontaPercorsi(null, win("C:/p/a.typ"), true), "esito: null è diverso");
uguale("diverso", confrontaPercorsi("/home/u/Doc.typ", "/home/u/doc.typ", false), "esito unix: maiuscole contano");
uguale("incerto", confrontaPercorsi("/mnt/x/doc.typ", "/home/u/doc.typ", false), "esito unix: stesso nome, cartella diversa");
uguale(true, stessoPercorso(win("C:/p/a.typ"), win("c:/p/a.typ"), true), "stessoPercorso resta booleano stretto");
uguale(false, stessoPercorso(win("Z:/progetti/a.typ"), win("//srv/share/progetti/a.typ"), true), "stessoPercorso: incerto non è uguale");

// Motivi leggibili.
uguale(true, motivoNonRipristinato("vuoto").includes("vuoto"), "motivo vuoto");
uguale(true, motivoNonRipristinato("troncato").includes("troncato"), "motivo troncato");
uguale(true, motivoNonRipristinato("cartella-assente").includes("cartella"), "motivo cartella");
// Il motivo generico è distinto da quelli specifici e vale per ogni esito sconosciuto.
const generico = motivoNonRipristinato("altro");
uguale(true, generico.includes("verifica"), "motivo generico: dice che la verifica non è passata");
uguale(
  false,
  ["vuoto", "non-utf8", "troncato", "cartella-assente"].some((e) => motivoNonRipristinato(e) === generico),
  "motivo generico diverso dai specifici",
);
uguale(generico, motivoNonRipristinato("esito-mai-visto"), "esito sconosciuto → motivo generico");
uguale(true, motivoNonRipristinato("non-utf8").includes("UTF-8"), "motivo non-utf8");

// Rinomina esterna: il tipo non cambia in silenzio.
uguale(null, avvisoTipoDopoRinomina("typst", "C:/p/b.typ", "b.typ"), "typ→typ: nessun avviso");
uguale(null, avvisoTipoDopoRinomina("markdown", "C:/p/b.MD", "b.MD"), "md→md (maiuscolo): nessun avviso");
uguale(true, (avvisoTipoDopoRinomina("typst", "C:/p/b.md", "b.md") ?? "").includes("resta Typst"), "typ→md: avviso");
uguale(true, (avvisoTipoDopoRinomina("markdown", "C:/p/b.typ", "b.typ") ?? "").includes("resta Markdown"), "md→typ: avviso");

// Evento «sparito» con percorso incerto (omonimo in un'altra cartella, o documento
// precedente ancora nello stato): «esiste» non deve segnare il documento modificato.
uguale("ignora", azioneDopoSparito("esiste", true), "sparito incerto + esiste: no-op silenzioso");
uguale("ricomparso", azioneDopoSparito("esiste", false), "sparito certo + esiste: ricomparso");
uguale("ripristinato", azioneDopoSparito("ripristinato", true), "sparito incerto + ripristinato");
uguale("ripristinato", azioneDopoSparito("ripristinato", false), "sparito certo + ripristinato");
uguale("non-ripristinato", azioneDopoSparito("troncato", true), "sparito incerto + troncato: chiede dove salvare");
uguale("non-ripristinato", azioneDopoSparito("vuoto", false), "sparito certo + vuoto");

// Ricarica/chiedi con percorso incerto: a contenuto identico (anche CRLF) si tace.
uguale(true, ricaricaSuperflua("a\r\nb\r\n", "a\nb\n", true), "incerto + identico (CRLF): superflua");
uguale(true, ricaricaSuperflua("a\nb", "a\nb", true), "incerto + identico: superflua");
uguale(false, ricaricaSuperflua("a\nc", "a\nb", true), "incerto + diverso: si ricarica");
uguale(false, ricaricaSuperflua("a\nb", "a\nb", false), "certo + identico: avviso come prima");

// Incerto + testo modificato: il disco uguale all'ultima versione nota (anche CRLF) è un
// falso allarme; diverso, o versione ignota, resta un possibile conflitto vero.
uguale(true, discoInvariato("a\r\nb\r\n", "a\nb\n"), "disco = ultimo noto (CRLF): invariato");
uguale(true, discoInvariato("", ""), "disco vuoto = noto vuoto: invariato");
uguale(false, discoInvariato("a\nc\n", "a\nb\n"), "disco diverso dal noto: conflitto possibile");
uguale(false, discoInvariato("a\nb\n", null), "versione nota ignota: si chiede");
uguale(false, discoInvariato("", null), "ignota + vuoto: si chiede");

console.log("modificheesterne.check.ts: ok");
