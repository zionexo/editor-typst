/**
 * dipendenze.check.ts — Le librerie importate direttamente dal codice devono essere
 * dichiarate in `package.json` (non solo transitive) e rispecchiate nella radice di
 * `package-lock.json`, con lo stesso intervallo e una versione già risolta nel lockfile.
 */
const { readFileSync } = (
  globalThis as unknown as {
    process: { getBuiltinModule(nome: "fs"): { readFileSync(url: URL, codifica: "utf8"): string } };
  }
).process.getBuiltinModule("fs");

type Pacchetto = { dependencies?: Record<string, string>; version?: string };
type Lock = { packages: Record<string, Pacchetto> };

const pkg = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")) as Pacchetto;
const lock = JSON.parse(readFileSync(new URL("../package-lock.json", import.meta.url), "utf8")) as Lock;

// Dipendenze importate direttamente da `src/` (non solo transitive): devono essere dichiarate.
const ATTESE = ["@codemirror/language", "@lezer/highlight"];

/** `versione` soddisfa `^base` (stesso major, >= base) per versioni x.y.z. */
function soddisfaCaret(versione: string, intervallo: string): boolean {
  const v = versione.split(".").map(Number);
  const b = intervallo.replace(/^\^/, "").split(".").map(Number);
  if (v.length !== 3 || b.length !== 3 || [...v, ...b].some((n) => !Number.isInteger(n))) {
    return false;
  }
  if (v[0] !== b[0]) {
    return false;
  }
  return v[1] > b[1] || (v[1] === b[1] && v[2] >= b[2]);
}

for (const nome of ATTESE) {
  const intervallo = pkg.dependencies?.[nome];
  if (intervallo === undefined || !intervallo.startsWith("^")) {
    throw new Error(`FALLITO: ${nome} non dichiarata con intervallo ^ in package.json`);
  }
  const radice = lock.packages[""].dependencies?.[nome];
  if (radice !== intervallo) {
    throw new Error(`FALLITO: package-lock radice: ${nome} atteso ${intervallo}, ottenuto ${String(radice)}`);
  }
  const risolta = lock.packages[`node_modules/${nome}`]?.version;
  if (risolta === undefined || !soddisfaCaret(risolta, intervallo)) {
    throw new Error(`FALLITO: ${nome} risolta nel lockfile a ${String(risolta)}, fuori da ${intervallo}`);
  }
}
if (soddisfaCaret("6.11.0", "^6.12.4") || !soddisfaCaret("6.12.4", "^6.12.4") || soddisfaCaret("7.0.0", "^6.12.4")) {
  throw new Error("FALLITO: soddisfaCaret non distingue gli intervalli");
}
// Le dipendenze dirette di package.json e della radice del lock coincidono.
const a = JSON.stringify(Object.entries(pkg.dependencies ?? {}).sort());
const b = JSON.stringify(Object.entries(lock.packages[""].dependencies ?? {}).sort());
if (a !== b) {
  throw new Error("FALLITO: dependencies di package.json e del lock (radice) divergono");
}
console.log("dipendenze.check: OK");
