// @modello: Relazione breve
// @descrizione: Relazione di due pagine con titolo, sommario e conclusioni
// @pagine: 2
// @campo: titolo | testo | Relazione sullo stato di avanzamento
// @campo: autore | testo | Nicola Ballari
// @campo: data | data | oggi
// @campo: sommario | elenco | Obiettivi del periodo\nAttività svolte\nCriticità e prossimi passi
// @campo: conclusioni | testo | Il lavoro procede secondo i tempi previsti; le criticità emerse sono gestibili.

#set page(paper: "a4", margin: 2.5cm, numbering: "1")
#set text(lang: "it", size: 11pt)
#set par(justify: true)
#set heading(numbering: "1.")

#align(center)[
  #text(size: 20pt, weight: "bold")[{{titolo}}] \
  #v(0.3cm)
  {{autore}} · {{data}}
]

#v(0.8cm)

*Sommario*

{{sommario}}

= Introduzione

Questa relazione riassume in breve il lavoro svolto e i punti principali
da portare all'attenzione dei lettori.

#pagebreak()

= Conclusioni

{{conclusioni}}
