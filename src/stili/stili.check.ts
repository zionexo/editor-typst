/**
 * stili.check.ts — Controlli sui fogli di stile (testo dei CSS, nessun browser):
 * - il fuoco visibile globale non deve colpire l'area di testo di CodeMirror;
 * - l'evidenziazione del pulsante con `aria-expanded` vale solo per «Aspetto»;
 * - contrasto WCAG del commento di sintassi sulla selezione (`--rilievo`) ≥ 4,5:1;
 * - contrasto WCAG di tutti i colori di sintassi (`--sint-*`) sullo sfondo dell'editor, e della
 *   tavolozza del Classico scuro su `oneDark`, ≥ 4,5:1 (valgono anche per i tag di Typst).
 */
import { COLORI_CLASSICO_SCURO, SFONDO_ONEDARK } from "./tavolozza-typst.ts";

const { readFileSync } = (
  globalThis as unknown as {
    process: { getBuiltinModule(nome: "fs"): { readFileSync(url: URL, codifica: "utf8"): string } };
  }
).process.getBuiltinModule("fs");

function vero(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`FALLITO: ${msg}`);
  }
}

/** Luminanza relativa WCAG 2.x di un colore #rrggbb. */
function luminanza(hex: string): number {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}

/** Rapporto di contrasto WCAG fra due colori #rrggbb. */
function contrasto(a: string, b: string): number {
  const [x, y] = [luminanza(a), luminanza(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}

/** Valore `light-dark(chiaro, scuro)` del token `--nome` nel foglio: [chiaro, scuro]. */
function token(css: string, nome: string): [string, string] {
  const m = new RegExp(nome + String.raw`:\s*light-dark\(\s*(#[0-9a-fA-F]{6})\s*,\s*(#[0-9a-fA-F]{6})\s*\)`).exec(css);
  if (m === null) {
    throw new Error(`FALLITO: token ${nome} non trovato`);
  }
  return [m[1], m[2]];
}

for (const stile of ["composizione", "strumento"]) {
  const css = readFileSync(new URL(`./${stile}.css`, import.meta.url), "utf8");
  const radice = `:root[data-stile="${stile}"]`;

  // (1) Il fuoco globale esclude l'area di testo (che ha già l'anello su .cm-editor.cm-focused).
  vero(
    css.includes(`${radice} :focus-visible:not(.cm-content) {`),
    `${stile}: il fuoco visibile globale deve escludere .cm-content`,
  );
  vero(!css.includes(`${radice} :focus-visible {`), `${stile}: resta una regola :focus-visible senza esclusione`);

  // (5) L'evidenziazione di aria-expanded è solo del pulsante Aspetto.
  vero(
    css.includes(`${radice} header button#btn-aspetto[aria-expanded="true"]`),
    `${stile}: aria-expanded limitato a #btn-aspetto`,
  );
  vero(
    !css.includes(`${radice} header button[aria-expanded="true"]`),
    `${stile}: regola aria-expanded troppo larga (colpisce #btn-barra, #btn-pacchetti)`,
  );

  // (9) Commento di sintassi leggibile (AA, 4,5:1) sul colore di selezione, in chiaro e scuro.
  const [rc, rs] = token(css, "--rilievo");
  const [cc, cs] = token(css, "--sint-commento");
  const chiaro = contrasto(cc, rc);
  const scuro = contrasto(cs, rs);
  console.log(`${stile}: commento su rilievo ${chiaro.toFixed(2)}:1 chiaro, ${scuro.toFixed(2)}:1 scuro`);
  vero(chiaro >= 4.5, `${stile}: commento/rilievo chiaro ${chiaro.toFixed(2)} < 4,5`);
  vero(scuro >= 4.5, `${stile}: commento/rilievo scuro ${scuro.toFixed(2)} < 4,5`);

  // (10) Ogni colore di sintassi leggibile sul fondo dell'editor (--sfondo), chiaro e scuro.
  const [fc, fs] = token(css, "--sfondo");
  for (const nome of ["titolo", "funzione", "stringa", "numero", "commento", "math", "rif"]) {
    const [c, d] = token(css, `--sint-${nome}`);
    vero(contrasto(c, fc) >= 4.5, `${stile}: --sint-${nome} chiaro ${contrasto(c, fc).toFixed(2)} < 4,5 su --sfondo`);
    vero(contrasto(d, fs) >= 4.5, `${stile}: --sint-${nome} scuro ${contrasto(d, fs).toFixed(2)} < 4,5 su --sfondo`);
  }
}

// (11) Classico scuro: tavolozza dei documenti Typst sul fondo di oneDark.
for (const [nome, colore] of Object.entries(COLORI_CLASSICO_SCURO)) {
  const c = contrasto(colore, SFONDO_ONEDARK);
  console.log(`classico scuro: ${nome} ${colore} su ${SFONDO_ONEDARK} ${c.toFixed(2)}:1`);
  vero(c >= 4.5, `classico scuro: ${nome} ${colore} ${c.toFixed(2)} < 4,5 su oneDark`);
}
// (12) Pallino di salvataggio e riquadro errori, nei tre stili e in chiaro/scuro: i colori del
// pallino (verde, ambra = --avviso, rosso = --errore, neutro = --controllo) ≥ 3:1 sulla barra
// (--pannello, componenti UI); testo e dettagli attenuati (mix 92% di
// --testo con --errore-sfondo, come in index.html) ≥ 4,5:1 sullo sfondo del riquadro.
{
  const html = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
  vero(
    html.includes("color-mix(in srgb, var(--testo) 92%, var(--errore-sfondo))"),
    "il tono attenuato dei dettagli non è più il mix verificato qui",
  );
  const mescola = (a: string, b: string, pa: number): string => {
    const c = [1, 3, 5].map((i) => {
      const x = parseInt(a.slice(i, i + 2), 16) * pa + parseInt(b.slice(i, i + 2), 16) * (1 - pa);
      return Math.round(x).toString(16).padStart(2, "0");
    });
    return "#" + c.join("");
  };
  for (const stile of ["classico", "composizione", "strumento"]) {
    const css = stile === "classico" ? html : readFileSync(new URL(`./${stile}.css`, import.meta.url), "utf8");
    const dal = (nome: string): [string, string] => {
      try {
        return token(css, nome);
      } catch {
        return token(html, nome);
      }
    };
    for (const [i, tema] of ["chiaro", "scuro"].entries()) {
      const pannello = dal("--pannello")[i];
      for (const nome of ["--verde", "--avviso", "--errore", "--controllo"]) {
        const c = contrasto(dal(nome)[i], pannello);
        vero(c >= 3, `${stile} ${tema}: ${nome} ${c.toFixed(2)} < 3 su --pannello (pallino)`);
      }
      const sfondo = dal("--errore-sfondo")[i];
      const t = contrasto(dal("--testo")[i], sfondo);
      vero(t >= 4.5, `${stile} ${tema}: --testo ${t.toFixed(2)} < 4,5 su --errore-sfondo`);
      // Barra laterale della riga (componente UI): 3:1.
      for (const nome of ["--errore", "--avviso"]) {
        const c = contrasto(dal(nome)[i], sfondo);
        vero(c >= 3, `${stile} ${tema}: ${nome} ${c.toFixed(2)} < 3 su --errore-sfondo (barra)`);
      }
      const attenuato = mescola(dal("--testo")[i], sfondo, 0.92);
      const c = contrasto(attenuato, sfondo);
      console.log(`${stile} ${tema}: dettagli attenuati ${c.toFixed(2)}:1 su --errore-sfondo`);
      vero(c >= 4.5, `${stile} ${tema}: dettagli attenuati ${c.toFixed(2)} < 4,5`);
    }
  }
}
console.log("stili.check: OK");
