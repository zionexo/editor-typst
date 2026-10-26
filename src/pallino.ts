/**
 * pallino.ts — Pallino di salvataggio accanto al nome del file (solo DOM; la logica pura è in
 * `salvataggio.ts`). Lo stato è su `data-stato`: il CSS dà a ciascuno una forma diversa, oltre
 * al colore. Il nome accessibile (`aria-label`) e il `title` ripetono lo stato in parole; gli
 * esiti significativi (salvato / non salvato) sono annunciati una volta sola nella regione
 * `aria-live="polite"` dedicata.
 */
import {
  annunciare,
  avanza,
  statoSalvataggio,
  titoloSalvataggio,
  type IngressiSalvataggio,
  type Mostrato,
  type StatoSalvataggio,
} from "./salvataggio";

export interface Pallino {
  /** Ricalcola lo stato voluto dagli ingressi e lo mostra (l'ambra resta almeno ~400 ms). */
  aggiorna(ingressi: IngressiSalvataggio): void;
}

/**
 * @param elemento  il pallino (riceve `role="img"`, `aria-label`, `title`, `data-stato`).
 * @param annuncio  regione `aria-live="polite"` per gli annunci.
 */
export function creaPallino(elemento: HTMLElement, annuncio: HTMLElement): Pallino {
  elemento.setAttribute("role", "img");
  let voluto: StatoSalvataggio = "nuovo";
  let mostrato: Mostrato | null = null;
  let disegnato: StatoSalvataggio | null = null;
  let ultimoAnnunciato: StatoSalvataggio | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function valuta(): void {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    const r = avanza(mostrato, voluto, Date.now());
    mostrato = r.mostrato;
    disegna(r.mostrato.stato);
    if (r.attesaMs !== null) {
      timer = setTimeout(valuta, r.attesaMs);
    }
  }

  function disegna(stato: StatoSalvataggio): void {
    if (disegnato === stato) return;
    const primo = disegnato === null;
    disegnato = stato;
    const titolo = titoloSalvataggio(stato);
    elemento.dataset.stato = stato;
    elemento.setAttribute("aria-label", titolo);
    elemento.title = titolo;
    if (primo) {
      // Il primo disegno non si annuncia, ma fa da riferimento per i successivi.
      if (stato === "salvato" || stato === "non-salvato") ultimoAnnunciato = stato;
      return;
    }
    if (annunciare(ultimoAnnunciato, stato)) {
      ultimoAnnunciato = stato;
      annuncio.textContent = titolo;
    }
  }

  return {
    aggiorna(ingressi) {
      voluto = statoSalvataggio(ingressi);
      valuta();
    },
  };
}
