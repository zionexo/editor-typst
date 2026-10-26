/**
 * svgdedup.ts — Dedup a valle delle definizioni SVG condivise fra pagine (F5-009).
 *
 * `typst-svg` (0.15, `finalize`) scrive in coda a ogni pagina **fino a sette**
 * blocchi `<defs>…</defs>` consecutivi, uno per categoria (id con prefisso
 * `g` glifi, `c` clip path, `f`/`r`/`s` gradienti, `t`/`p` tiling), con i glifi
 * (`<symbol>`), i clip path, i gradienti e i tiling usati dalla pagina.
 * Gli id sono `hash128` del contenuto (`fase8.md` §4.2): **stabili fra pagine e
 * fra compilazioni**, quindi lo stesso glifo ha lo stesso id ovunque. Con più
 * pagine nello stesso DOM basta tenere ogni definizione una sola volta in un
 * contenitore condiviso: `separaDefs` toglie da una pagina le definizioni già
 * viste e restituisce solo quelle nuove da aggiungere al contenitore.
 *
 * Logica pura, senza DOM (verificata da `svgdedup.check.ts`). Se la struttura
 * non è quella attesa la pagina viene lasciata intatta (`null`): il dedup è
 * un'ottimizzazione, mai un rischio per la resa.
 */

/** Esito della separazione delle definizioni di una pagina. */
export interface DefsSeparate {
  /** La pagina senza i suoi blocchi `<defs>` (i riferimenti `#id` restano validi col contenitore). */
  svg: string;
  /** Solo le definizioni non ancora presenti (markup da aggiungere al contenitore). */
  nuove: string;
  /** Gli id di `nuove`, da aggiungere all'insieme dei già visti. */
  idNuovi: string[];
}

/**
 * Un elemento di primo livello di un `<defs>`: `<tag id="…" …/>` (es. i
 * gradienti di riferimento `<linearGradient id="r…" href="#f…"/>`) oppure
 * `<tag id="…" …>…</tag>` (nessun annidamento dello stesso tag). Lo sticky `y`
 * lo ancora alla posizione corrente.
 */
const ELEMENTO_DEFS =
  /<(symbol|clipPath|linearGradient|radialGradient|pattern)\s[^>]*?\bid="([^"]+)"[^>]*?(?:\/>|>[\s\S]*?<\/\1>)/y;

/**
 * Separa **tutti** i blocchi `<defs>` finali di una pagina SVG di `typst-svg`.
 *
 * @param svg   Documento SVG di una pagina.
 * @param visti Id delle definizioni già presenti nel contenitore condiviso.
 * @returns Pagina senza `<defs>` finali + definizioni nuove, oppure `null` se
 *          la pagina non ha la forma attesa (nessun `<defs>` finale, elementi
 *          non riconosciuti): in tal caso va inserita così com'è.
 */
export function separaDefs(svg: string, visti: ReadonlySet<string>): DefsSeparate | null {
  const CHIUSURA = "</svg>";
  if (!svg.endsWith(CHIUSURA)) {
    return null;
  }
  // Stacca dalla coda tutti i blocchi <defs> consecutivi.
  let resto = svg.slice(0, svg.length - CHIUSURA.length);
  const corpi: string[] = [];
  while (resto.endsWith("</defs>")) {
    const inizio = resto.lastIndexOf("<defs>");
    if (inizio < 0) {
      return null;
    }
    corpi.unshift(resto.slice(inizio + "<defs>".length, resto.length - "</defs>".length));
    resto = resto.slice(0, inizio);
  }
  if (corpi.length === 0) {
    return null;
  }

  let nuove = "";
  const idNuovi: string[] = [];
  const localiVisti = new Set<string>();
  for (const corpo of corpi) {
    let pos = 0;
    while (pos < corpo.length) {
      ELEMENTO_DEFS.lastIndex = pos;
      const m = ELEMENTO_DEFS.exec(corpo);
      if (m === null) {
        return null; // contenuto non riconosciuto: nessun rischio, nessun dedup
      }
      const id = m[2];
      if (!visti.has(id) && !localiVisti.has(id)) {
        nuove += m[0];
        idNuovi.push(id);
        localiVisti.add(id);
      }
      pos += m[0].length;
    }
  }
  return { svg: resto + CHIUSURA, nuove, idNuovi };
}
