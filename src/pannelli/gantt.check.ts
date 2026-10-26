/**
 * gantt.check.ts — Verifica di `gantt-logica.ts` (scheda Gantt, pacchetto gantt-newgen).
 * Esce con errore al primo controllo fallito.
 */
import {
  INTESTAZIONI,
  OPZIONI_PREDEFINITE,
  aggiungiGiorni,
  esempioGantt,
  generaGantt,
  rimuoviAttivita,
  validaGantt,
  type Attivita,
  type OpzioniGantt,
  type PietraMiliare,
} from "./gantt-logica.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}

const a = (nome: string, inizio: string, fine: string, extra: Partial<Attivita> = {}): Attivita => ({
  nome, gruppo: "", inizio, fine, fatta: "", avanzamento: null, dipende: null, tipoDip: "fs", ...extra,
});
const opz: OpzioniGantt = { ...OPZIONI_PREDEFINITE };
const codice = (att: Attivita[], pietre: PietraMiliare[] = [], o: OpzioniGantt = opz): string => {
  const r = generaGantt(att, pietre, o);
  vero("codice" in r, `generazione: ${JSON.stringify(r)}`);
  return (r as { codice: string }).codice;
};
const errore = (att: Attivita[], pietre: PietraMiliare[] = [], o: OpzioniGantt = opz): string => {
  const r = generaGantt(att, pietre, o);
  vero("errore" in r, `atteso un errore per ${JSON.stringify(att)}`);
  return (r as { errore: string }).errore;
};

// date
uguali(aggiungiGiorni("2026-01-30", 3), "2026-02-02", "cambio di mese");
uguali(aggiungiGiorni("2026-12-31", 1), "2027-01-01", "cambio di anno");
uguali(aggiungiGiorni("2024-02-28", 1), "2024-02-29", "anno bisestile");
uguali(aggiungiGiorni("2026-03-10", -10), "2026-02-28", "giorni negativi");

// minimo: lingua italiana, intestazioni e linea «oggi» espliciti; l'intervallo lo ricava il pacchetto
uguali(
  codice([a("A", "2026-01-05", "2026-01-12")]),
  '#gantt((\n  locale: "it",\n  headers: ("month", "week"),\n  show-today: false,\n  tasks: (\n    (name: "A", start: "2026-01-05", end: "2026-01-12"),\n  ),\n))\n',
  "una sola attività",
);
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, oggi: true }).includes("  show-today: true,"), "linea «oggi» accesa");
vero(!codice([a("A", "2026-01-05", "2026-01-12")]).includes("  start:"), "senza intervallo indicato non si scrive start/end del diagramma");

// opzioni: intestazioni, «oggi» (spento di default)
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, intestazioni: "anno-trimestre", oggi: false }).includes('headers: ("year", "quarter"),\n  show-today: false,'), "intestazioni anno/trimestre, oggi spento");
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, intestazioni: "mese-settimana-iso" }).includes('headers: ("month", "week-iso"),'), "settimana ISO");
vero(INTESTAZIONI.every((h) => codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, intestazioni: h.id }).includes(`headers: (${h.livelli.map((l) => `"${l}"`).join(", ")}),`)), "ogni combinazione di intestazioni è generata");

// gruppi, completamento, avanzamento, dipendenze (id solo per chi è citato), tipi, pietre miliari
const att = [
  a("Analisi", "2026-01-05", "2026-01-10", { gruppo: "Prep", fatta: "2026-01-09", avanzamento: 100 }),
  a("Progetto", "2026-01-11", "2026-01-20", { gruppo: "Prep", dipende: 0, avanzamento: 40 }),
  a("Sviluppo", "2026-01-21", "2026-02-20", { dipende: 1, tipoDip: "ss" }),
  a("Collaudo", "2026-02-21", "2026-03-01", { gruppo: "Prep" }),
];
const c = codice(att, [{ nome: "Consegna", data: "2026-03-01", mostraData: false }]);
vero(c.includes('(name: "Prep", subtasks: ('), "gruppo con sotto-attività");
vero(c.split('name: "Prep"').length === 2, "le attività dello stesso gruppo stanno in un solo gruppo (anche se non contigue)");
vero(c.includes('(name: "Analisi", start: "2026-01-05", end: "2026-01-10", done: "2026-01-09", progress: 100, id: "a1")'), "done, progress e id su chi è citato");
vero(c.includes('progress: 40, id: "a2", dependencies: ((id: "a1"),)'), "dipendenza di default senza `type` (virgola finale obbligatoria)");
vero(c.includes('dependencies: ((id: "a2", type: "start-to-start"),)'), "tipo di dipendenza non predefinito");
vero(!c.includes('id: "a3"') && !c.includes('id: "a4"'), "nessun id inutile");
vero(c.includes('(name: "Consegna", date: "2026-03-01", show-date: false)'), "pietra miliare senza data");
vero(c.indexOf('name: "Prep"') < c.indexOf('name: "Sviluppo"'), "ordine di prima comparsa");
for (const [t, v] of [["ff", "finish-to-finish"], ["sf", "start-to-finish"]] as const) {
  vero(codice([a("A", "2026-01-01", "2026-01-02"), a("B", "2026-01-03", "2026-01-04", { dipende: 0, tipoDip: t })]).includes(`type: "${v}"`), `tipo ${t}`);
}

// intervallo e figura
uguali(
  codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, da: "2026-01-01", a: "2026-02-01", didascalia: "Piano [v1]", etichetta: "fig:piano" }),
  '#figure(\n  gantt((\n    locale: "it",\n    headers: ("month", "week"),\n    show-today: false,\n    start: "2026-01-01",\n    end: "2026-02-01",\n    tasks: (\n      (name: "A", start: "2026-01-05", end: "2026-01-12"),\n    ),\n  )),\n  caption: [Piano \\[v1\\]],\n) <fig:piano>\n',
  "figura con didascalia ed etichetta e intervallo",
);
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, etichetta: "solo" }).startsWith("#figure("), "solo etichetta: figura");
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, da: "2026-01-01" }).includes('start: "2026-01-01"') && !codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, da: "2026-01-01" }).includes("  end:"), "solo «Mostra da»: si scrive solo start");

// escape nei nomi e nelle didascalie
vero(codice([a('Il "grande" \\ passo', "2026-01-05", "2026-01-12")]).includes('name: "Il \\"grande\\" \\\\ passo"'), "virgolette e barra rovesciata nei nomi");
vero(codice([a("Con # e $", "2026-01-05", "2026-01-12")]).includes('name: "Con # e $"'), "# e $ restano letterali nella stringa");
vero(codice([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, didascalia: "Fasi // bozza /* x */" }).includes("caption: [Fasi \\/\\/ bozza \\/\\* x \\*\\/]"), "la barra viene escapata nella didascalia");

// una attività di un solo giorno è ammessa (end inclusivo)
uguali(validaGantt([a("A", "2026-01-05", "2026-01-05")], [], opz), null, "inizio = fine è ammesso");
uguali(validaGantt([a("A", "2026-01-05", "2026-01-05")], [], { ...opz, da: "2026-01-05", a: "2026-01-05" }), null, "intervallo di un solo giorno");

// errori
vero(errore([]).includes("almeno un'attività"), "nessuna attività");
vero(errore([a("", "2026-01-05", "2026-01-12")]).includes("manca il nome"), "nome vuoto");
vero(errore([a("  ", "2026-01-05", "2026-01-12")]).includes("manca il nome"), "nome di soli spazi");
vero(errore([a("A\nB", "2026-01-05", "2026-01-12")]).includes("a capo"), "a capo nel nome");
vero(errore([a("A", "2026-02-30", "2026-03-01")]).includes("inizio non valida"), "data inesistente");
vero(errore([a("A", "2026-01-05", "")]).includes("fine non valida"), "fine mancante");
vero(errore([a("A", "2026-01-12", "2026-01-05")]).includes("precede"), "fine prima dell'inizio");
vero(errore([a("A", "2026-01-05", "2026-01-12", { fatta: "ieri" })]).includes("completamento"), "completamento non valido");
vero(errore([a("A", "2026-01-05", "2026-01-12", { fatta: "2026-01-01" })]).includes("completamento precede"), "completamento prima dell'inizio");
vero(errore([a("A", "2026-01-05", "2026-01-12", { avanzamento: 101 })]).includes("da 0 a 100"), "avanzamento oltre 100");
vero(errore([a("A", "2026-01-05", "2026-01-12", { avanzamento: -1 })]).includes("da 0 a 100"), "avanzamento negativo");
vero(errore([a("A", "2026-01-05", "2026-01-12", { avanzamento: 12.5 })]).includes("da 0 a 100"), "avanzamento non intero");
uguali(validaGantt([a("A", "2026-01-05", "2026-01-12", { avanzamento: 0 })], [], opz), null, "avanzamento 0 ammesso");
vero(errore([a("A", "2026-01-05", "2026-01-12", { dipende: 0 })]).includes("se stessa"), "dipendenza da sé");
vero(errore([a("A", "2026-01-05", "2026-01-12", { dipende: 5 })]).includes("dipendenza non valida"), "dipendenza fuori range");
vero(errore([a("A", "2026-01-01", "2026-01-02"), a("B", "2026-01-03", "2026-01-04", { dipende: 0, tipoDip: "xx" as never })]).includes("tipo di dipendenza"), "tipo di dipendenza ignoto");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [{ nome: "", data: "2026-01-05", mostraData: true }]).includes("Pietra miliare 1"), "pietra senza nome");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [{ nome: "M", data: "x", mostraData: true }]).includes("data non valida"), "pietra con data errata");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, da: "2026-02-01", a: "2026-01-01" }).includes("Intervallo"), "intervallo invertito");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, da: "2026-02-01" }).includes("dopo la fine"), "«Mostra da» dopo tutte le fini");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, a: "2026-01-01" }).includes("Mostra fino a"), "«Mostra fino a» prima di tutte le attività");
// la linea «oggi» allarga l'intervallo: 1950 + oggi supera i 25 anni, senza oggi no
vero(errore([a("A", "1950-01-01", "1951-01-01")], [], { ...opz, oggi: true }).includes("più di"), "«oggi» conta nell'ampiezza");
uguali(validaGantt([a("A", "1950-01-01", "1951-01-01")], [], opz), null, "senza «oggi» l'ampiezza è piccola");
uguali(validaGantt([a("A", "1950-01-01", "1951-01-01")], [], { ...opz, oggi: true, a: "1951-06-01" }, "2026-10-23"), null, "«Mostra fino a» fisso: «oggi» non allarga quel lato")
uguali(aggiungiGiorni("0050-01-10", 1), "0050-01-11", "anni 0-99 non diventano 19xx");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, etichetta: "1x" }).includes("etichetta"), "etichetta non valida");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, intestazioni: "boh" }).includes("Intestazioni"), "intestazioni ignote");
vero(errore(Array.from({ length: 101 }, () => a("A", "2026-01-05", "2026-01-12"))).includes("Al massimo"), "troppe attività");

// anni e ampiezza: l'anno 9999 manda in panico il compilatore; oltre ~27 anni le intestazioni fine si fermano
vero(errore([a("A", "0100-01-01", "0100-02-01")]).includes("1900"), "anno 100");
vero(errore([a("A", "2026-01-01", "9999-12-31")]).includes("2200"), "anno 9999");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [{ nome: "M", data: "2500-01-01", mostraData: true }]).includes("Pietra miliare 1"), "pietra miliare nel 2500");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [], { ...opz, a: "3000-01-01" }).includes("Intervallo"), "intervallo nel 3000");
uguali(validaGantt([a("A", "1900-01-01", "1900-02-01")], [], opz), null, "il limite inferiore (1900) è ammesso");
uguali(validaGantt([a("A", "2200-11-01", "2200-12-31")], [], opz), null, "il limite superiore (2200) è ammesso");
vero(errore([a("A", "2000-01-01", "2026-06-01")]).includes("più di 25 anni"), "ampiezza oltre 25 anni");
uguali(validaGantt([a("A", "2000-01-01", "2024-12-31")], [], opz), null, "24 anni e 364 giorni sono ammessi");
vero(errore([a("A", "2026-01-05", "2026-01-12")], [{ nome: "M", data: "2060-01-01", mostraData: true }]).includes("più di 25 anni"), "una pietra miliare lontana allarga l'intervallo oltre il limite");

// nomi lunghi
vero(errore([a("x".repeat(81), "2026-01-05", "2026-01-12")]).includes("80"), "nome di 81 caratteri");
uguali(validaGantt([a("x".repeat(80), "2026-01-05", "2026-01-12")], [], opz), null, "nome di 80 caratteri");

// dipendenze circolari
vero(errore([a("A", "2026-01-01", "2026-01-02", { dipende: 1 }), a("B", "2026-01-03", "2026-01-04", { dipende: 0 })]).includes("circolare"), "A↔B");
vero(errore([a("A", "2026-01-01", "2026-01-02", { dipende: 2 }), a("B", "2026-01-03", "2026-01-04", { dipende: 0 }), a("C", "2026-01-05", "2026-01-06", { dipende: 1 })]).includes("circolare"), "ciclo di tre");
uguali(validaGantt([a("A", "2026-01-01", "2026-01-02"), a("B", "2026-01-03", "2026-01-04", { dipende: 0 }), a("C", "2026-01-05", "2026-01-06", { dipende: 0 })], [], opz), null, "due attività che dipendono dalla stessa non sono un ciclo");

// rimozione di un'attività: le dipendenze si riallineano (e i tipi restano)
{
  const base = [a("A", "2026-01-01", "2026-01-02"), a("B", "2026-01-03", "2026-01-04", { dipende: 0 }), a("C", "2026-01-05", "2026-01-06", { dipende: 1, tipoDip: "ff" }), a("D", "2026-01-07", "2026-01-08", { dipende: 2 })];
  uguali(rimuoviAttivita(base, 1).map((x) => [x.nome, x.dipende]), [["A", null], ["C", null], ["D", 1]], "toglie B: C perde la dipendenza, D scala da 2 a 1");
  uguali(rimuoviAttivita(base, 0).map((x) => [x.nome, x.dipende]), [["B", null], ["C", 0], ["D", 1]], "toglie A: B perde la dipendenza, gli altri scalano");
  uguali(rimuoviAttivita(base, 3).map((x) => [x.nome, x.dipende]), [["A", null], ["B", 0], ["C", 1]], "toglie D: nessun cambiamento sugli altri");
  uguali(rimuoviAttivita(base, 0)[1].tipoDip, "ff", "il tipo di dipendenza si conserva");
  uguali(base[1].dipende, 0, "l'originale non viene modificato");
}

// l'esempio è valido e usa gruppi, dipendenze, avanzamento e una pietra miliare
const es = esempioGantt("2026-06-01");
uguali(validaGantt(es.attivita, es.pietre, opz), null, "l'esempio è valido");
vero(codice(es.attivita, es.pietre).includes("dependencies") && codice(es.attivita, es.pietre).includes("progress: 40"), "l'esempio ha dipendenze e avanzamento");

console.log("OK gantt");
