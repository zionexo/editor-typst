#set document(title: "Moto laminare in un canale e integrali notevoli", author: "Prova Typster")
#set page(paper: "a4", margin: (x: 2.4cm, y: 2.6cm), numbering: "1 / 1",
  header: align(right, text(size: 9pt, fill: gray)[Prova di esportazione · testo scientifico]))
#set text(lang: "it", size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.1")
#set math.equation(numbering: "(1)", supplement: "Eq.")
#show figure.caption: set text(size: 9.5pt)

#align(center)[
  #text(size: 19pt, weight: "bold")[Moto laminare in un canale e integrali notevoli] \
  #v(4pt)
  #text(size: 11pt)[Nota tecnica di prova · #datetime(year: 2026, month: 10, day: 7).display("[day]/[month]/[year]")]
]

#v(8pt)
*Sommario.* Si richiamano le equazioni di Navier-Stokes per un fluido incomprimibile, si ricava il profilo di Poiseuille tra due lastre parallele e si raccolgono due risultati classici di analisi: l'integrale gaussiano e la somma dei reciproci dei quadrati. Il testo serve anche da banco di prova per formule, tabelle e figure vettoriali.

= Equazioni del moto

Per un fluido newtoniano incomprimibile di densità $rho$ e viscosità dinamica $mu$ il bilancio della quantità di moto @batchelor si scrive
$ rho (partial bold(u)) / (partial t) + rho (bold(u) dot.c nabla) bold(u) = -nabla p + mu nabla^2 bold(u) + bold(f), quad nabla dot.c bold(u) = 0 $ <eq:ns>
dove $bold(u)$ è il campo di velocità, $p$ la pressione e $bold(f)$ una forza di volume. Il numero di Reynolds $"Re" = rho U L slash mu$ misura il rapporto tra inerzia e attrito viscoso; per $"Re" << 1$ l'@eq:ns si riduce al problema di Stokes @landau.

== Profilo di Poiseuille

Tra due lastre distanti $h$, con gradiente di pressione costante $G = -dif p slash dif x$ e regime stazionario, i termini convettivi si annullano e resta $mu u'' = -G$. Con aderenza alle pareti si ottiene
$ u(y) = G / (2 mu) y (h - y), quad u_max = (G h^2) / (8 mu) $ <eq:poiseuille>
e la portata per unità di larghezza vale $Q = integral_0^h u(y) dif y = G h^3 slash (12 mu)$. La @fig:profilo mostra il profilo parabolico dell'@eq:poiseuille.

#figure(
  {
    let w = 7cm
    let h = 4.2cm
    let n = 40
    let pts = range(0, n + 1).map(i => {
      let y = i / n
      (w * 4 * y * (1 - y) * 0.9, h * (1 - y))
    })
    box(width: 9cm, height: h + 0.6cm, {
      place(dx: 0.8cm, dy: 0.2cm, rect(width: w, height: h, stroke: 0.6pt + gray))
      place(dx: 0.8cm, dy: 0.2cm, polygon(fill: rgb("#cfe3f7"), stroke: 1.2pt + blue,
        (0pt, 0pt), ..pts.map(p => (p.at(0), p.at(1))), (0pt, h)))
      for k in range(0, 6) {
        let y = h * (k + 0.5) / 6
        let yy = 1 - (k + 0.5) / 6
        let l = w * 4 * yy * (1 - yy) * 0.9
        place(dx: 0.8cm, dy: 0.2cm + y, line(length: l, stroke: 0.7pt + red))
      }
      place(dx: 0cm, dy: 0cm, text(size: 8pt)[$y$])
      place(dx: 8.2cm, dy: h + 0.2cm, text(size: 8pt)[$u$])
    })
  },
  caption: [Profilo di velocità di Poiseuille tra due lastre: la freccia è proporzionale a $u(y)$.],
) <fig:profilo>

= Due integrali notevoli

L'integrale gaussiano @gauss è
$ integral_(-oo)^(oo) e^(-x^2) dif x = sqrt(pi) $ <eq:gauss>
da cui, per $a > 0$, $integral_(-oo)^(oo) e^(-a x^2) dif x = sqrt(pi slash a)$. La serie di Basilea fornisce invece
$ sum_(n=1)^(oo) 1 / n^2 = pi^2 / 6 approx 1.6449 $ <eq:basilea>
e le somme parziali $S_N$ convergono con errore $O(1 slash N)$, come mostrato in @tab:somme e in @fig:serie.

#figure(
  table(columns: 4, align: center + horizon, stroke: 0.5pt,
    table.header[*$N$*][*$S_N$*][*Errore*][*$N dot.c "errore"$*],
    [10], [1.5498], [0.0952], [0.952],
    [100], [1.6350], [0.0099], [0.995],
    [1000], [1.6439], [0.0010], [1.000],
    [10000], [1.6448], [0.0001], [1.000],
  ),
  caption: [Somme parziali della serie di Basilea ed errore rispetto a $pi^2 slash 6$.],
) <tab:somme>

#figure(
  {
    let w = 8cm
    let h = 3.6cm
    let dati = (1.0, 1.25, 1.361, 1.4236, 1.4636, 1.4914, 1.5118, 1.5274, 1.5398, 1.5498)
    box(width: 9.5cm, height: h + 0.9cm, {
      place(dx: 1cm, dy: 0.2cm, rect(width: w, height: h, stroke: 0.6pt))
      // asintoto pi^2/6 = 1.6449 su scala [0.9, 1.7]
      let ys = v => h * (1 - (v - 0.9) / 0.8)
      place(dx: 1cm, dy: 0.2cm + ys(1.6449), line(length: w, stroke: (paint: red, dash: "dashed")))
      for (i, v) in dati.enumerate() {
        let x = w * (i + 0.5) / dati.len()
        place(dx: 1cm + x - 2pt, dy: 0.2cm + ys(v) - 2pt, circle(radius: 2pt, fill: blue, stroke: none))
        if i > 0 {
          let xp = w * (i - 0.5) / dati.len()
          let d = dati.at(i - 1)
          place(dx: 1cm + xp, dy: 0.2cm + ys(d),
            line(start: (0pt, 0pt), end: (x - xp, ys(v) - ys(d)), stroke: 0.8pt + blue))
        }
      }
      place(dx: 0cm, dy: 0.2cm, text(size: 8pt)[1.7])
      place(dx: 0cm, dy: 0.2cm + h - 8pt, text(size: 8pt)[0.9])
      place(dx: 1cm, dy: h + 0.35cm, text(size: 8pt)[$N = 1, dots, 10$])
    })
  },
  caption: [Somme parziali $S_N$ (punti) e limite $pi^2 slash 6$ (tratteggio).],
) <fig:serie>

= Conclusioni

Il profilo parabolico dell'@eq:poiseuille, l'@eq:gauss e l'@eq:basilea sono risultati chiusi che permettono di verificare rapidamente la resa tipografica di formule inline come $e^(i pi) + 1 = 0$ e display numerate. Le fonti sono in bibliografia.

#bibliography("refs.bib", title: "Bibliografia", style: "ieee")
