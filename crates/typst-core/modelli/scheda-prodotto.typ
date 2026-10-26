// @modello: Scheda prodotto
// @descrizione: Scheda tecnica di due pagine con caratteristiche, prezzo e note
// @pagine: 2
// @campo: prodotto | testo | Lampada da tavolo Aurora
// @campo: codice | testo | LT-2041
// @campo: categoria | testo | Illuminazione
// @campo: prezzo | numero | 49.90
// @campo: descrizione | testo | Lampada a LED con braccio orientabile e luce regolabile.
// @campo: caratteristiche | elenco | Luce regolabile in tre tonalità\nBraccio orientabile\nConsumo ridotto
// @campo: note | testo | Garanzia di due anni. Immagini a scopo illustrativo.

#set page(paper: "a4", margin: 2.2cm)
#set text(lang: "it", size: 11pt)
#set par(justify: true)

#text(size: 22pt, weight: "bold")[{{prodotto}}]

#text(fill: luma(100))[Codice {{codice}} · {{categoria}}]

#line(length: 100%, stroke: 0.5pt + luma(180))

#v(0.5cm)

== Descrizione

{{descrizione}}

== Prezzo

#text(size: 16pt, weight: "bold")[{{prezzo}} €]

#pagebreak()

== Caratteristiche

{{caratteristiche}}

== Note

{{note}}

#v(1fr)

#align(center)[#text(size: 9pt, fill: luma(120))[Scheda {{codice}}]]
