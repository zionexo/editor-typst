// @modello: Lettera di richiesta
// @descrizione: Lettera formale di una pagina con luogo, data, destinatario e firma
// @pagine: 1
// @campo: mittente | testo | Nicola Ballari
// @campo: luogo | testo | Milano
// @campo: data | data | oggi
// @campo: destinatario | testo | Studio Bianchi
// @campo: oggetto | testo | Richiesta di informazioni
// @campo: firma | testo | Nicola Ballari

#set page(paper: "a4", margin: (x: 2.5cm, y: 2.5cm))
#set text(lang: "it", size: 11pt)
#set par(justify: true, leading: 0.7em)

#text(weight: "bold")[{{mittente}}]

#v(1.5cm)

#align(right)[
  Spett.le \
  *{{destinatario}}*
]

#v(1cm)

{{luogo}}, {{data}}

#v(0.5cm)

*Oggetto:* {{oggetto}}

#v(0.5cm)

Gentili signori,

con la presente desidero richiedere alcune informazioni in merito a quanto
indicato in oggetto. Vi sarei grato se poteste fornirmi un riscontro non
appena possibile, indicando eventuali documenti necessari.

Resto a disposizione per qualsiasi chiarimento e porgo distinti saluti.

#v(1.5cm)

#align(right)[
  {{firma}}
]
