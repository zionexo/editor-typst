#set document(title: "La casa sul crinale", author: "Prova Typster")
#set page(paper: "a5", margin: (x: 2.2cm, top: 2.4cm, bottom: 2.2cm),
  header: context {
    if counter(page).get().first() > 1 [
      #set text(size: 8.5pt, style: "italic")
      La casa sul crinale #h(1fr) Prova di esportazione
      #line(length: 100%, stroke: 0.4pt)
    ]
  },
  footer: align(center, text(size: 9pt)[– #context counter(page).display() –]))
#set text(lang: "it", font: "Libertinus Serif", size: 10.5pt)
#set par(justify: true, first-line-indent: 1.2em, leading: 0.7em, spacing: 0.7em)
#set heading(numbering: "I")
#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  v(2.5cm)
  align(center)[
    #text(size: 9pt, tracking: 2pt)[#upper[Capitolo #counter(heading).display(it.numbering)]] \
    #v(4pt)
    #text(size: 17pt, weight: "bold", it.body)
  ]
  v(1cm)
}
#show quote.where(block: true): set block(inset: (left: 1.2em, right: 1em), above: 1em, below: 1em)
#show quote.where(block: true): set text(size: 9.8pt, style: "italic")
#show figure.caption: set text(size: 8.5pt)

#let capolettera(lettera, resto) = grid(
  columns: (auto, 1fr), column-gutter: 5pt,
  text(size: 3.6em, weight: "bold", fill: rgb("#8b1e2d"), top-edge: "cap-height", bottom-edge: "baseline")[#lettera],
  par(first-line-indent: 0pt)[#resto],
)

#v(3cm)
#align(center)[
  #text(size: 26pt, weight: "bold")[La casa sul crinale] \
  #v(6pt)
  #text(size: 12pt, style: "italic")[racconto in tre capitoli] \
  #v(1.2cm)
  #figure(image("img/stemma.svg", width: 3.2cm), caption: [Lo stemma dei Valdarni (SVG).])
]

= La salita

#capolettera("L")[a diligenza lasciò il viaggiatore ai piedi del crinale quando il sole, già stanco di quel giorno di settembre, indugiava sulle creste come un ospite che non sa decidersi a prendere congedo. Il cocchiere, un omone taciturno che per tutta la strada aveva parlato soltanto ai cavalli, gli porse la valigia con un gesto quasi cerimonioso e si toccò il cappello senza dire parola.]

Elia Montanari era nato in quelle valli, ma ne mancava da ventidue anni, e per ventidue anni aveva creduto di non rimpiangerle. Si accorse, mentre guardava il sentiero perdersi tra i castagni, che il cuore gli batteva con una sollecitudine indegna d'un uomo di quarantasei anni, avvocato stimato, vedovo e senza illusioni. Prese la valigia e cominciò a salire.

Il sentiero era quello di sempre, soltanto più stretto, come accade a tutte le cose dell'infanzia quando le si rivede da adulti. I muri a secco, che ricordava alti quanto un gigante, gli arrivavano ora a metà coscia; le more selvatiche si erano mangiate i cancelli; e dal fondo della valle saliva, sottile e continuo, il rumore del torrente, che era l'unica cosa in tutto il paesaggio a non aver cambiato né voce né umore.#footnote[Il torrente si chiamava Sorana, ma nessuno in valle l'aveva mai chiamato per nome: per tutti era «l'acqua», come se non ve ne fosse altra al mondo.]

A metà salita incontrò una vecchia che raccoglieva legna secca. Ella si raddrizzò a fatica, si fece schermo agli occhi con la mano e lo guardò a lungo.

«Voi siete il figlio del notaio», disse infine.

«Lo sono stato», rispose Elia. «Ora non lo so più.»

«Si è figli per sempre, signore», replicò la vecchia, e riprese i suoi rami come se avesse detto la cosa più semplice del mondo.

Egli proseguì, e per un tratto si sorprese a ripetere quella frase, come si ripete un motivo che non si è capito ma che ha lasciato nell'orecchio una traccia di dolcezza. Sul filo del crinale, dove il bosco si apriva e il cielo prendeva tutto lo spazio, comparve finalmente la casa.

#figure(image("img/paesaggio.png", width: 90%),
  caption: [Il crinale al tramonto, con la casa e il bosco (PNG).])

Era una casa grande e bassa, di pietra grigia, con un loggiato a tre archi e un tetto che il tempo aveva ondulato come il dorso di un animale addormentato. Le imposte del piano superiore erano chiuse; quelle del pianterreno, no, e ciò gli parve strano, perché la lettera dello zio Gaspare diceva chiaramente che la casa era disabitata da almeno due inverni.

= Il custode

Sulla soglia lo attendeva un uomo magro, con la barba bianca tagliata corta e un paio di occhiali tondi legati con lo spago. Teneva in mano una lanterna già accesa, benché fosse ancora giorno, e la reggeva con la deferenza che si riserva a un oggetto sacro.

«Sono Tobia», disse. «Il vostro zio mi ha lasciato le chiavi e un testamento da leggervi. Ma prima cenate: i testamenti, a stomaco vuoto, sembrano sempre più crudeli di quello che sono.»

La cucina era l'unica stanza in cui il tempo sembrasse non aver fatto danni. Il camino ardeva, la tavola era apparecchiata per due, e sulla mensola un orologio a pendolo scandiva i secondi con l'ostinazione di chi sa di avere ragione. Elia sedette, e mangiò zuppa di castagne e pane scuro con un appetito che non provava da anni. Tobia lo osservava dall'altra parte della tavola senza dire nulla, e solo a metà della cena si decise a parlare.

«Vostro zio diceva sempre che questa casa non si eredita: si accetta.»

«È una distinzione sottile.»

«È la sola che conti», disse Tobia. «Chi eredita fa l'inventario. Chi accetta fa la legna.»

Elia sorrise, e il sorriso gli costò uno sforzo, perché da molto non lo usava. Poi il custode estrasse da una cartella di cuoio un foglio piegato in quattro e lo lesse a voce alta, con la lentezza di chi sillaba una preghiera:

#quote(block: true, attribution: [dal testamento di Gaspare Montanari])[
  Lascio a mio nipote Elia la casa del crinale, con gli ulivi, i libri e il silenzio. Non gli lascio denaro, perché ne ha già troppo e ne sa troppo poco; gli lascio invece un anno di tempo, durante il quale dovrà abitarla senza vendere nulla. Se allo scadere dell'anno vorrà ancora andarsene, sia libero di farlo, e il notaio gli darà questo foglio come ricevuta del mio affetto.
]

«Un anno», ripeté Elia.

«Un anno, un mese e un giorno, per essere esatti. Vostro zio amava le cifre storte: diceva che sono più oneste di quelle tonde.»

Per qualche istante nella cucina non si udì che il pendolo e il crepitio dei ceppi. Elia pensò alla sua città, allo studio con le pareti di noce, alle cause che lo aspettavano e che, ne era certo, avrebbero saputo fare a meno di lui; pensò a sua moglie, morta in primavera, e al modo in cui la casa di lei, a sera, si era fatta improvvisamente troppo grande. Poi, senza decidere nulla, disse:

«Resterò, per ora.»

«"Per ora" è un buon principio», approvò Tobia, e riempì i bicchieri.#footnote[Il vino era quello delle vigne basse, che in valle si beveva giovane e senza cerimonie; lo zio Gaspare ne conservava tuttavia alcune bottiglie per le grandi occasioni, e ne aveva dichiarate occasioni grandi ben poche.]

= La biblioteca

Nei giorni che seguirono Elia esplorò la casa come si esplora un paese straniero, con curiosità e un po' di sospetto. Al piano superiore trovò camere dalle lenzuola ripiegate in cassettoni profumati di lavanda; in soffitta, un baule di giocattoli che aveva dimenticato di possedere; e in fondo al corridoio, dietro una porta che gridava a ogni spinta, la biblioteca.

Era una sala lunga, con tre finestre sul versante della valle e scaffali che salivano fino al soffitto. Vi erano libri di ogni sorta: trattati di agraria e poemi cavallereschi, almanacchi, erbari, una Divina Commedia con le note di un commentatore che pareva avercela personalmente con Dante. Sul leggio, aperto a metà, giaceva un quaderno rilegato in tela. Era il diario dello zio.

Lo lesse quella sera stessa, alla luce della lampada, e continuò per molte sere, un poco alla volta, come si assaporano le cose che si temono di finire. Gaspare scriveva con una calligrafia minuta e ironica; parlava del tempo, dei raccolti, delle visite del medico; parlava di sé il meno possibile, e proprio in quel poco Elia credette di riconoscere il ritratto d'un uomo che aveva scelto la solitudine non per misantropia, ma per un eccesso di riguardo verso il prossimo. Una pagina, soprattutto, gli rimase impressa:

#quote(block: true)[
  Ho imparato che la pazienza non è l'arte di aspettare, ma quella di non smettere di vivere mentre si aspetta. Il contadino che semina non attende il grano: sta seminando, ed è già tutto lì.
]

Alla sesta settimana Elia si accorse di non guardare più l'orologio. Aveva preso a svegliarsi con l'alba, a camminare per i filari, a conversare con Tobia di cose minime: della grandine, dei muli, d'una trave del loggiato che cedeva. Rispondeva alle lettere dello studio con crescente brevità, e una mattina si sorprese a dettare, in un biglietto al socio, una frase che non avrebbe mai pensato di scrivere: «Non tornerò prima della primavera».

Una sera di novembre, mentre la nebbia risaliva dalla valle e inghiottiva a uno a uno i castagni, Tobia lo trovò in biblioteca, intento a rimettere in ordine uno scaffale.

«Allora, signor avvocato», disse, «avete già deciso se la casa si eredita o si accetta?»

Elia posò il volume che teneva in mano, e ci pensò con la serietà che avrebbe dedicato a un parere legale.

«Credo», rispose, «che si accetti. E credo anche che, accettandola, mi stia accettando lei.»

«È la stessa cosa che disse vostro zio, il primo inverno.»

«E poi?»

«E poi non se ne andò più», concluse Tobia, e uscì a chiudere le imposte, lasciando nella sala il profumo della lanterna e la strana, quieta certezza che i discorsi più importanti sono quelli che nessuno ha bisogno di finire.

Fuori la nebbia saliva ancora. Ma dentro, nella luce bassa dei vecchi scaffali, Elia Montanari aprì il quaderno alla prima pagina bianca, intinse la penna e cominciò, per la prima volta in ventidue anni, a scrivere di sé.
