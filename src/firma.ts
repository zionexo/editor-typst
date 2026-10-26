/**
 * firma.ts — Suggerimento dei parametri nelle chiamate Typst (logica pura).
 *
 * Dato il testo e il cursore trova la chiamata `f(` aperta più interna e il
 * parametro che si sta scrivendo, e compone la firma da mostrare usando i
 * metadati di `typstBuiltinSignatures` (`codemirror-lang-typst`). Nessun DOM:
 * la presentazione è in `assistenza.ts`.
 */

/** Come `TypstBuiltinParameter`: [nome, flag, tipi]. Flag: 1 posiz., 2 nominale, 4 variadico, 8 obbligatorio, 16 impostabile. */
export type Parametro = readonly [nome: string, flag: number, tipi: readonly string[]];
export type Firme = Readonly<Record<string, readonly Parametro[]>>;

/** Chiamata aperta sotto il cursore. */
export interface ChiamataAttiva {
  /** Nome come scritto nel sorgente (`figure`, `calc.pow`). */
  chiamata: string;
  /** Parametri nominali già scritti. */
  usati: ReadonlySet<string>;
  /** Parametro in scrittura: nome (argomento nominale) o indice fra i posizionali. */
  corrente: { nome: string } | { posizione: number };
}

interface Cornice {
  chiamata: string | null;
  posizionali: number;
  usati: Set<string>;
  nomeCorrente: string | null;
  /** Il primo token dell'argomento corrente è già stato visto (serve a riconoscere `nome:`). */
  inizioArg: number;
}

// Scansione in avanti sugli ultimi 20 000 caratteri; un documento enorme con una
// stringa aperta a metà finestra può dare un suggerimento sbagliato (ceiling noto;
// alternativa: l'albero Lezer, solo motore lezer).
export const FINESTRA = 20000;

/** Trova la chiamata aperta più interna alla posizione `cursore` (offset UTF-16). */
export function chiamataAttiva(testo: string, cursore: number): ChiamataAttiva | null {
  const da = Math.max(0, cursore - FINESTRA);
  const pila: Cornice[] = [];
  const nuova = (chiamata: string | null, i: number): Cornice => ({
    chiamata,
    posizionali: 0,
    usati: new Set(),
    nomeCorrente: null,
    inizioArg: i,
  });
  let i = da;
  while (i < cursore) {
    const c = testo[i];
    const cima = pila[pila.length - 1];
    if (c === "\\") {
      i += 2; // escape (`\"`, `\(`…): il carattere dopo non conta
    } else if (c === "`") {
      const delim = /^`+/.exec(testo.slice(i, i + 8))![0];
      const f = testo.indexOf(delim, i + delim.length);
      i = f === -1 || f >= cursore ? cursore : f + delim.length;
    } else if (c === '"') {
      i++;
      while (i < cursore && testo[i] !== '"') i += testo[i] === "\\" ? 2 : 1;
      i++;
    } else if (c === "/" && testo[i + 1] === "/" && testo[i - 1] !== ":") {
      while (i < cursore && testo[i] !== "\n") i++;
    } else if (c === "/" && testo[i + 1] === "*") {
      const f = testo.indexOf("*/", i + 2);
      i = f === -1 || f >= cursore ? cursore : f + 2;
    } else if (c === "(") {
      const m = /[\p{L}_][\p{L}\p{N}_.-]*$/u.exec(testo.slice(Math.max(da, i - 80), i));
      pila.push(nuova(m ? m[0] : null, i + 1));
      i++;
    } else if (c === "[" || c === "{") {
      pila.push(nuova(null, i + 1));
      i++;
    } else if (c === ")" || c === "]" || c === "}") {
      pila.pop();
      i++;
    } else if (c === "," && cima) {
      if (cima.nomeCorrente === null) cima.posizionali++;
      cima.nomeCorrente = null;
      cima.inizioArg = i + 1;
      i++;
    } else if (c === ":" && cima && cima.nomeCorrente === null) {
      const nome = testo.slice(cima.inizioArg, i).trim();
      if (/^[\p{L}_][\p{L}\p{N}_-]*$/u.test(nome)) {
        cima.nomeCorrente = nome;
        cima.usati.add(nome);
      }
      i++;
    } else {
      i++;
    }
  }
  const cima = pila[pila.length - 1];
  if (!cima || cima.chiamata === null) return null;
  const usati = new Set(cima.usati);
  if (cima.nomeCorrente !== null) usati.delete(cima.nomeCorrente);
  return {
    chiamata: cima.chiamata,
    usati,
    corrente:
      cima.nomeCorrente !== null
        ? { nome: cima.nomeCorrente }
        : { posizione: cima.posizionali },
  };
}

/** Cerca la firma: nome esatto, poi con prefisso `math.` (chiamate in modalità matematica). */
export function trovaFirma(firme: Firme, chiamata: string): readonly Parametro[] | null {
  return firme[chiamata] ?? firme[`math.${chiamata}`] ?? null;
}

/** `type:content` → `content`, `value:"column"` → `"column"`, `any` → `any`. */
export function tipoLeggibile(t: string): string {
  const i = t.indexOf(":");
  return i === -1 ? t : t.slice(i + 1);
}

/** Tipi di un parametro, uniti con `|`. */
export function tipiParametro(p: Parametro): string {
  return p[2].map(tipoLeggibile).join(" | ");
}

/** Nome del parametro attivo nella firma, o `null` se non determinabile. */
export function parametroAttivo(
  params: readonly Parametro[],
  corrente: ChiamataAttiva["corrente"],
): string | null {
  if ("nome" in corrente) {
    return params.some((p) => p[0] === corrente.nome) ? corrente.nome : null;
  }
  const posizionali = params.filter((p) => (p[1] & 1) !== 0);
  const p = posizionali[corrente.posizione] ?? posizionali.find((q) => (q[1] & 4) !== 0);
  return p ? p[0] : null;
}

/** Pezzo di firma: `attivo` va in grassetto. */
export interface PezzoFirma {
  testo: string;
  attivo: boolean;
}

/**
 * Compone `figure(body, caption: content | none, …)`. Mostra i posizionali e i
 * nominali; i nominali già usati sono omessi, tranne quello attivo.
 */
export function pezziFirma(
  chiamata: string,
  params: readonly Parametro[],
  attivo: string | null,
  usati: ReadonlySet<string> = new Set(),
): PezzoFirma[] {
  const pezzi: PezzoFirma[] = [{ testo: `${chiamata}(`, attivo: false }];
  let primo = true;
  for (const p of params) {
    const [nome, flag] = p;
    const nominale = (flag & 2) !== 0;
    const soloPosizionale = (flag & 1) !== 0 && !nominale;
    if (nominale && usati.has(nome) && nome !== attivo) continue;
    if (!primo) pezzi.push({ testo: ", ", attivo: false });
    primo = false;
    const prefisso = (flag & 4) !== 0 ? ".." : "";
    const resto = soloPosizionale ? nome : `${nome}: ${tipiParametro(p)}`;
    const opz = soloPosizionale && (flag & 8) === 0 && (flag & 4) === 0 ? "?" : "";
    pezzi.push({ testo: `${prefisso}${resto}${opz}`, attivo: nome === attivo });
  }
  pezzi.push({ testo: ")", attivo: false });
  return pezzi;
}
