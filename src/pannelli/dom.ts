/**
 * dom.ts — Costruttori DOM minimi condivisi dalle viste del pannello «Strumenti».
 * Solo `createElement` e `textContent` (mai `innerHTML`), come il resto del frontend.
 */

/** Crea un elemento con classe e testo. */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  classe?: string,
  testo?: string,
): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (classe !== undefined) {
    e.className = classe;
  }
  if (testo !== undefined) {
    e.textContent = testo;
  }
  return e;
}

/** Pulsante di tipo `button` con classe e azione. */
export function pulsante(testo: string, classe: string, click: () => void): HTMLButtonElement {
  const b = el("button", classe, testo);
  b.type = "button";
  b.addEventListener("click", click);
  return b;
}

/** Etichetta + controllo, collegati da `for`/`id`. */
export function campoForm(etichetta: string, controllo: HTMLElement, id: string): HTMLElement {
  controllo.id = id;
  const l = el("label", "str-campo");
  l.htmlFor = id;
  l.append(el("span", "str-etichetta", etichetta), controllo);
  return l;
}
