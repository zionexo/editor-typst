/**
 * firma.check.ts — Verifica di `firma.ts` (chiamata attiva, parametro corrente,
 * firma composta) sulle firme reali di `codemirror-lang-typst`.
 */
import { typstBuiltinSignatures } from "codemirror-lang-typst/lezer";
import { chiamataAttiva, parametroAttivo, pezziFirma, trovaFirma } from "./firma.ts";

function vero(c: boolean, msg: string): void {
  if (!c) {
    throw new Error(`FALLITO: ${msg}`);
  }
}
function uguali<T>(a: T, b: T, msg: string): void {
  vero(JSON.stringify(a) === JSON.stringify(b), `${msg}: ${JSON.stringify(a)} != ${JSON.stringify(b)}`);
}
const alla = (t: string) => chiamataAttiva(t, t.length);

vero(alla("testo senza chiamate") === null, "fuori da una chiamata");
vero(alla("#figure(x)") === null, "chiamata chiusa");
uguali(alla("#figure(")?.chiamata, "figure", "chiamata aperta");
uguali(alla("#figure(")?.corrente, { posizione: 0 }, "primo posizionale");
uguali(alla("#figure(img, ")?.corrente, { posizione: 1 }, "secondo posizionale");
uguali(alla("#figure(caption: [a, b], ")?.corrente, { posizione: 0 }, "virgola dentro [] ignorata, dopo un nominale");
uguali(alla("#figure(caption: ")?.corrente, { nome: "caption" }, "nominale in scrittura");
uguali([...(alla("#figure(caption: [x], ali")?.usati ?? [])], ["caption"], "nominali già usati");
uguali(alla('#figure("a, (b", ')?.corrente, { posizione: 1 }, "stringhe ignorate");
uguali(alla("#figure(image(\"a.png\"), ")?.chiamata, "figure", "chiamata annidata chiusa");
uguali(alla("#figure(image(")?.chiamata, "image", "chiamata annidata aperta");
vero(alla("#let x = (1, 2, ") === null, "parentesi senza chiamata");
uguali(alla("#f(a /* ( */, ")?.corrente, { posizione: 1 }, "commento a blocco ignorato");
uguali(alla('#f("a\\"b, (", ')?.corrente, { posizione: 1 }, "escape nella stringa");
uguali(alla("#f([Fonte: https://x.org], ")?.corrente, { posizione: 1 }, "URL non è un commento");
uguali(alla("#f(`)`, ")?.corrente, { posizione: 1 }, "raw ignorato");
vero(alla("$ x_(i+1) $") === null || alla("$ x_(i+1) ")?.chiamata === undefined, "pedice in math senza chiamata");
uguali(alla("#stack(a, b, ")?.corrente, { posizione: 2 }, "variadico: posizione");
uguali(alla("#f(a // (\n, ")?.corrente, { posizione: 1 }, "commento di riga ignorato");

const figure = trovaFirma(typstBuiltinSignatures, "figure");
vero(figure !== null, "firma di figure");
uguali(parametroAttivo(figure!, { posizione: 0 }), "body", "body è il primo posizionale");
uguali(parametroAttivo(figure!, { nome: "caption" }), "caption", "nominale noto");
uguali(parametroAttivo(figure!, { nome: "inesistente" }), null, "nominale ignoto");
const stack = trovaFirma(typstBuiltinSignatures, "stack")!;
vero(parametroAttivo(stack, { posizione: 2 }) !== null, "variadico: parametro attivo");
vero(trovaFirma(typstBuiltinSignatures, "attach") !== null, "prefisso math.");
vero(trovaFirma(typstBuiltinSignatures, "nonesiste") === null, "chiamata ignota");

const testo = pezziFirma("figure", figure!, "caption", new Set(["alt"]));
const s = testo.map((p) => p.testo).join("");
vero(s.startsWith("figure(body, "), `inizio firma: ${s}`);
vero(s.includes("caption: content | none"), `tipo del nominale: ${s}`);
vero(!s.includes("alt:"), `nominale già usato omesso: ${s}`);
uguali(testo.filter((p) => p.attivo).map((p) => p.testo.split(":")[0]), ["caption"], "solo il parametro attivo in grassetto");

console.log("OK firma");
