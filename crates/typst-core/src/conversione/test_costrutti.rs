//! Test unitari per costrutto delle tre esportazioni (testo, HTML, LaTeX).

use super::*;

fn txt(s: &str) -> Conversione {
    typst_a_testo(s, &OpzioniTesto::default())
}
fn txt_w(s: &str, w: usize) -> Conversione {
    typst_a_testo(s, &OpzioniTesto { larghezza: w })
}
fn html(s: &str) -> Conversione {
    typst_a_html(s, &OpzioniHtml::default())
}
fn tex(s: &str) -> Conversione {
    typst_a_latex(s, &OpzioniLatex::default())
}

// ---------------------------------------------------------------------------
// testo semplice
// ---------------------------------------------------------------------------

#[test]
fn testo_titoli_sottolineati_e_numerati() {
    let c = txt("#set heading(numbering: \"1.1\")\n= Uno\n== Due\n=== Tre\n==== Quattro\n");
    let t = c.contenuto;
    assert!(t.contains("1 Uno\n====="), "{t}");
    assert!(t.contains("1.1 Due\n-------"), "{t}");
    assert!(t.contains("1.1.1 Tre\n~~~~~~~~~"), "{t}");
    assert!(t.contains("1.1.1.1 Quattro\n"), "{t}");
    // senza numerazione
    let t = txt("= Solo titolo\n").contenuto;
    assert!(t.starts_with("Solo titolo\n===========\n"), "{t}");
}

#[test]
fn testo_a_capo_a_larghezza_configurabile() {
    let src = "una due tre quattro cinque sei sette otto nove dieci undici dodici";
    let t = txt_w(src, 20).contenuto;
    for riga in t.lines() {
        assert!(riga.chars().count() <= 20, "{riga:?}");
    }
    assert!(t.lines().count() >= 3);
    let t80 = txt(src).contenuto;
    assert_eq!(t80.lines().count(), 1);
    // larghezza minima imposta
    assert!(txt_w(src, 1)
        .contenuto
        .lines()
        .all(|r| r.chars().count() <= 20));
}

#[test]
fn testo_liste_annidate() {
    let t = txt("- a\n  - b\n  - c\n- d\n\n+ x\n+ y\n").contenuto;
    assert!(t.contains("- a\n  - b\n  - c\n- d"), "{t}");
    assert!(t.contains("1. x\n2. y"), "{t}");
}

#[test]
fn testo_termini_citazione_codice() {
    let t = txt("/ Mela: frutto rosso\n\n#quote(block: true, attribution: [Autore])[Parole sagge.]\n\n```rust\nfn f() {}\n```\n").contenuto;
    assert!(t.contains("Mela\n    frutto rosso"), "{t}");
    assert!(t.contains("    Parole sagge.\n\n    — Autore"), "{t}");
    assert!(t.contains("    fn f() {}"), "{t}");
}

#[test]
fn testo_note_in_coda_e_link() {
    let t =
        txt(r#"Vedi#footnote[Nota uno.] il #link("https://a.it")[sito] e https://b.it."#).contenuto;
    assert!(
        t.contains("Vedi[1] il sito (https://a.it) e https://b.it."),
        "{t}"
    );
    assert!(t.trim_end().ends_with("[1] Nota uno."), "{t}");
}

#[test]
fn testo_tabella_ascii_con_colspan_e_allineamento() {
    let t = txt("#table(columns: 2, align: right, table.header[A][B], [1], [22], table.cell(colspan: 2)[unito])").contenuto;
    // la prima riga è un bordo `+-----+-----+`, poi l'intestazione `| A | B |`
    let righe: Vec<&str> = t.lines().collect();
    assert!(righe[0].starts_with('+') && righe[0].ends_with('+'), "{t}");
    assert!(
        righe[1].starts_with('|') && righe[1].contains('A') && righe[1].contains('B'),
        "{t}"
    );
    assert!(t.contains("A |") && t.contains("B |"), "{t}");
    assert!(t.contains("+=====+=====+"), "{t}");
    assert!(t.contains("unito"), "{t}");
    for riga in t.lines() {
        assert!(riga.chars().count() <= 80, "{riga:?}");
    }
    // tutte le righe di bordo hanno la stessa lunghezza
    let larghezze: Vec<usize> = t
        .lines()
        .filter(|r| r.starts_with('+'))
        .map(|r| r.chars().count())
        .collect();
    assert!(larghezze.windows(2).all(|w| w[0] == w[1]), "{larghezze:?}");
}

#[test]
fn testo_formule_lineari_e_figure() {
    let t = txt("#set text(lang: \"it\")\n#figure(image(\"a.png\"), caption: [Un grafico]) <g>\n\nVedi @g. $sum_(i=1)^n i = (n(n+1))/2$").contenuto;
    assert!(t.contains("[Figura 1: Un grafico]"), "{t}");
    assert!(t.contains("Vedi Figura 1."), "{t}");
    assert!(t.contains("∑_(i = 1)ⁿ i = (n(n + 1))/2"), "{t}");
}

#[test]
fn testo_avvisi_su_perdite() {
    let c = txt("#for i in range(3) [x]\n#lorem(5)\n#unknownfn(1)[corpo]");
    assert!(
        c.avvisi.iter().any(|a| a.contains("if/for/while")),
        "{:?}",
        c.avvisi
    );
    assert!(
        c.avvisi.iter().any(|a| a.contains("lorem")),
        "{:?}",
        c.avvisi
    );
    assert!(
        c.avvisi.iter().any(|a| a.contains("unknownfn")),
        "{:?}",
        c.avvisi
    );
    assert!(c.contenuto.contains("corpo"));
}

#[test]
fn testo_senza_panico_su_input_degenere() {
    for src in [
        "",
        "   ",
        "#",
        "$",
        "= ",
        "#let",
        "$ ( $",
        "#table(",
        "[[[",
        "\u{0}",
        "#figure()",
        "#quote()",
    ] {
        let _ = txt(src);
        let _ = html(src);
        let _ = tex(src);
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

#[test]
fn html_struttura_semantica_di_base() {
    let c = html("#set text(lang: \"it\")\n= Titolo\n\nTesto *forte* _enfasi_ `cod` e #strike[via].\n\n- a\n- b\n\n+ uno\n+ due\n");
    let h = c.contenuto;
    assert!(h.starts_with("<!doctype html>\n<html lang=\"it\">"), "{h}");
    assert!(h.contains("<h1 id=\"sez-1\">Titolo</h1>"));
    assert!(h.contains("<strong>forte</strong> <em>enfasi</em> <code>cod</code> e <s>via</s>."));
    assert!(h.contains("<ul>\n<li>a</li>\n<li>b</li>\n</ul>"));
    assert!(h.contains("<ol>\n<li>uno</li>\n<li>due</li>\n</ol>"));
    assert!(h.contains("<style>") && h.contains("@media print") && h.contains("max-width"));
    assert!(h.contains("<meta name=\"viewport\""));
}

#[test]
fn html_citazione_con_attribuzione_figure_tabelle_e_note() {
    let h = html(
        "#quote(block: true, attribution: [Dante])[Nel mezzo.]\n\n\
         #figure(image(\"a.png\"), caption: [Cap]) <f>\n\n\
         #table(columns: 2, table.header[H1][H2], [a], [b])\n\n\
         Nota#footnote[Testo nota] qui. Vedi @f.",
    )
    .contenuto;
    assert!(h.contains("<blockquote>\n<p>Nel mezzo.</p>\n<footer>\u{2014} <cite>Dante</cite></footer>\n</blockquote>"), "{h}");
    assert!(h.contains("<figure id=\"et-f\">\n<p><img src=\"a.png\" alt=\"Cap\" loading=\"lazy\"></p>\n<figcaption>Figure 1: Cap</figcaption>"), "{h}");
    assert!(
        h.contains(
            "<thead>\n<tr><th scope=\"col\">H1</th><th scope=\"col\">H2</th></tr>\n</thead>"
        ),
        "{h}"
    );
    assert!(h.contains("<tbody>\n<tr><td>a</td><td>b</td></tr>"), "{h}");
    assert!(
        h.contains("Nota<sup class=\"nota\"><a id=\"rif-1\" href=\"#nota-1\">1</a></sup> qui."),
        "{h}"
    );
    assert!(h.contains("<li id=\"nota-1\">Testo nota"), "{h}");
    assert!(h.contains("<a href=\"#et-f\">Figure 1</a>"), "{h}");
}

#[test]
fn html_formule_mathml_e_ripiego_code() {
    let c = html("Inline $a/b$ e $foo.bar(x)$.\n\n$ sum_(i=1)^n i $\n");
    let h = c.contenuto;
    assert!(h.contains("<mfrac><mi>a</mi><mi>b</mi></mfrac>"), "{h}");
    assert!(
        h.contains("<code class=\"formula\">foo.bar(x)</code>"),
        "{h}"
    );
    assert!(h.contains("<munderover>"), "{h}");
    assert!(
        c.avvisi
            .iter()
            .any(|a| a.contains("formule non convertibili")),
        "{:?}",
        c.avvisi
    );
}

#[test]
fn html_titoli_oltre_il_sesto_livello_e_lingua_forzata() {
    let h = typst_a_html(
        "======= Sette",
        &OpzioniHtml {
            lingua: Some("fr".into()),
            titolo: Some("T <&>".into()),
            ..Default::default()
        },
    )
    .contenuto;
    assert!(h.contains("<h6 id=\"sez-1\">Sette</h6>"), "{h}");
    assert!(h.contains("<html lang=\"fr\">"));
    assert!(h.contains("<title>T &lt;&amp;&gt;</title>"));
}

#[test]
fn html_lingua_non_valida_ripiega_su_en() {
    let h = typst_a_html(
        "x",
        &OpzioniHtml {
            lingua: Some("it\"><script>".into()),
            ..Default::default()
        },
    )
    .contenuto;
    assert!(h.contains("<html lang=\"en\">"), "{h}");
}

#[test]
fn html_id_unici_con_etichette_ripetute() {
    let h = html("= A <x>\n\n= B <x>\n\ntesto <x>\n").contenuto;
    assert_eq!(h.matches("id=\"et-x\"").count(), 1, "{h}");
}

#[test]
fn html_indice_e_bibliografia() {
    let h = html("#outline()\n= Uno\n= Due\n#bibliography(\"r.bib\")\n").contenuto;
    assert!(h.contains("<nav class=\"indice\""), "{h}");
    assert!(
        h.contains("<a href=\"#sez-1\">Uno</a>") && h.contains("<a href=\"#sez-2\">Due</a>"),
        "{h}"
    );
    assert!(h.contains("r.bib"));
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

#[test]
fn latex_pacchetti_solo_se_servono() {
    let t = tex("Solo testo.").contenuto;
    assert!(t.contains("\\documentclass[11pt,a4paper]{article}"));
    for no in ["amsmath", "graphicx", "hyperref", "ulem"] {
        assert!(!t.contains(no), "{no} non richiesto:\n{t}");
    }
    let t = tex("$x$ #image(\"a.png\") #link(\"https://a.it\")[l] #strike[x]").contenuto;
    for si in ["amsmath", "graphicx", "hyperref", "ulem"] {
        assert!(t.contains(si), "{si} richiesto:\n{t}");
    }
}

#[test]
fn latex_titoli_numerati_o_con_asterisco() {
    let t = tex("= Uno\n== Due\n").contenuto;
    assert!(
        t.contains("\\section*{Uno}") && t.contains("\\subsection*{Due}"),
        "{t}"
    );
    let t = tex(
        "#set heading(numbering: \"1.\")\n= Uno\n== Due\n=== Tre\n==== Quattro\n===== Cinque\n",
    )
    .contenuto;
    assert!(
        t.contains("\\section{Uno}") && t.contains("\\subsubsection{Tre}"),
        "{t}"
    );
    assert!(
        t.contains("\\paragraph{Quattro}") && t.contains("\\subparagraph{Cinque}"),
        "{t}"
    );
}

#[test]
fn latex_classe_report_usa_capitoli() {
    let t = typst_a_latex(
        "#set heading(numbering: \"1.\")\n= Uno\n== Due\n",
        &OpzioniLatex {
            classe: "report".into(),
            dimensione_pt: 12,
            ..Default::default()
        },
    )
    .contenuto;
    assert!(t.contains("\\documentclass[12pt,a4paper]{report}"));
    assert!(
        t.contains("\\chapter{Uno}") && t.contains("\\section{Due}"),
        "{t}"
    );
}

#[test]
fn latex_enfasi_liste_citazioni_note() {
    let t = tex("*forte* _enfasi_ `cod`\n\n- a\n  - b\n\n+ x\n+ y\n\n#quote(block: true)[Citata.]\n\nTesto#footnote[Nota.]\n").contenuto;
    assert!(
        t.contains("\\textbf{forte} \\emph{enfasi} \\texttt{cod}"),
        "{t}"
    );
    assert!(
        t.contains("\\begin{itemize}\n\\item a\n\\begin{itemize}\n\\item b"),
        "{t}"
    );
    assert!(
        t.contains("\\begin{enumerate}\n\\item x\n\\item y\n\\end{enumerate}"),
        "{t}"
    );
    assert!(t.contains("\\begin{quote}\nCitata.\n\n\\end{quote}"), "{t}");
    assert!(t.contains("Testo\\footnote{Nota.}"), "{t}");
}

#[test]
fn latex_quotation_per_citazioni_lunghe_e_attribuzione() {
    let t = tex("#quote(block: true, attribution: [Autore])[Uno.\n\nDue.]").contenuto;
    assert!(
        t.contains("\\begin{quotation}") && t.contains("\\end{quotation}"),
        "{t}"
    );
    assert!(t.contains("\\hfill\\textemdash\\ Autore"), "{t}");
}

#[test]
fn latex_figura_con_includegraphics_didascalia_ed_etichetta() {
    let t = tex("#figure(image(\"img/a.png\", width: 50%), caption: [Cap *x*]) <f>\n\nVedi @f.")
        .contenuto;
    assert!(t.contains("\\begin{figure}[htbp]\n\\centering\n\\includegraphics[width=0.5\\linewidth]{img/a.png}"), "{t}");
    assert!(
        t.contains("\\caption{Cap \\textbf{x}}\n\\label{et:f}"),
        "{t}"
    );
    assert!(t.contains("Figure~\\ref{et:f}"), "{t}");
}

#[test]
fn latex_tabella_con_header_e_colspan() {
    let t = tex("#table(columns: 3, align: center, table.header[A][B][C], [1], table.cell(colspan: 2)[unito])").contenuto;
    assert!(t.contains("\\begin{tabular}{|c|c|c|}"), "{t}");
    assert!(
        t.contains("\\textbf{A} & \\textbf{B} & \\textbf{C} \\\\ \\hline"),
        "{t}"
    );
    assert!(
        t.contains("1 & \\multicolumn{2}{|c|}{unito} \\\\ \\hline"),
        "{t}"
    );
}

#[test]
fn latex_equazioni_numerate_allineate_e_cases() {
    let t = tex("#set math.equation(numbering: \"(1)\")\n$ a &= b \\ c &= d $ <e>\n\n$ f(x) = cases(1 \"se\" x > 0, 0 \"no\") $\n\nVedi @e.").contenuto;
    assert!(
        t.contains("\\begin{equation}\n\\label{et:e}\n\\begin{aligned}"),
        "{t}"
    );
    assert!(t.contains("a & = b \\\\\nc & = d"), "{t}");
    assert!(t.contains("\\begin{cases}"), "{t}");
    assert!(t.contains("Equation~\\eqref{et:e}"), "{t}");
    let t = tex("$ x = 1 $").contenuto;
    assert!(t.contains("\\begin{equation*}"), "{t}");
}

#[test]
fn latex_ambienti_bilanciati_in_documento_composito() {
    let t = tex("= T\n- a\n  + b\n\n#quote(block: true)[q]\n\n#table(columns: 1, [a])\n\n```\ncodice\n```\n\n$ x $").contenuto;
    let mut pila: Vec<String> = Vec::new();
    for riga in t.lines() {
        if let Some(r) = riga.strip_prefix("\\begin{") {
            pila.push(r.split('}').next().unwrap().to_string());
        }
        if let Some(r) = riga.strip_prefix("\\end{") {
            let n = r.split('}').next().unwrap();
            assert_eq!(pila.pop().as_deref(), Some(n), "{t}");
        }
    }
    assert!(pila.is_empty(), "{pila:?}");
}

#[test]
fn latex_codice_non_chiude_verbatim() {
    let t = tex("```\nx \\end{verbatim} y\n```").contenuto;
    assert_eq!(t.matches("\\end{verbatim}").count(), 1, "{t}");
}

#[test]
fn latex_babel_secondo_la_lingua() {
    assert!(tex("#set text(lang: \"it\")\nx")
        .contenuto
        .contains("\\usepackage[italian]{babel}"));
    assert!(tex("#set text(lang: \"de\")\nx")
        .contenuto
        .contains("\\usepackage[ngerman]{babel}"));
    let c = tex("#set text(lang: \"xx\")\nx");
    assert!(!c.contenuto.contains("babel"));
    assert!(c.avvisi.iter().any(|a| a.contains("babel")));
}

#[test]
fn latex_titolo_automatico_opzionale() {
    let src = "#set document(title: \"Il titolo\", author: \"Io\")\ntesto";
    let t = tex(src).contenuto;
    assert!(!t.contains("\\maketitle"));
    assert!(t.contains("pdftitle={Il titolo}"));
    let t = typst_a_latex(
        src,
        &OpzioniLatex {
            titolo_automatico: true,
            ..Default::default()
        },
    )
    .contenuto;
    assert!(
        t.contains("\\title{Il titolo}") && t.contains("\\author{Io}") && t.contains("\\maketitle"),
        "{t}"
    );
}

#[test]
fn latex_link_url_con_caratteri_speciali() {
    let t = tex("#link(\"https://a.it/x?y=1&z=%20#top\")[vai]").contenuto;
    assert!(
        t.contains("\\href{https://a.it/x?y=1&z=\\%20\\#top}{vai}"),
        "{t}"
    );
}

#[test]
fn avvisi_unici_e_non_vuoti_per_funzioni_sconosciute() {
    let c = tex("#foo[a] #foo[b] #foo[c]");
    let n = c.avvisi.iter().filter(|a| a.contains("`foo`")).count();
    assert_eq!(n, 1, "{:?}", c.avvisi);
}

// ---------------------------------------------------------------------------
// Correzioni 2026-10-08: input limite (nessun panico, nessuna allocazione enorme)
// ---------------------------------------------------------------------------

/// Esegue le tre conversioni e ritorna (testo, html, latex).
fn tutte(s: &str) -> (Conversione, Conversione, Conversione) {
    (txt(s), html(s), tex(s))
}

#[test]
fn nessun_panico_colonne_enormi() {
    for src in [
        "#table(columns: 9223372036854775807, [a])",
        "#table(columns: -5, [a])",
        "#table(columns: 3, table.cell(colspan: 9223372036854775807)[a], table.cell(colspan: 9223372036854775807)[b], table.cell(colspan: 9223372036854775807)[c])",
    ] {
        let (t, h, l) = tutte(src);
        // verifica indipendente: la specifica di colonne LaTeX non supera il tetto
        let spec = l
            .contenuto
            .lines()
            .find(|r| r.starts_with("\\begin{tabular}"))
            .unwrap_or_else(|| panic!("tabular assente:\n{}", l.contenuto));
        let n_col = spec.matches('|').count().saturating_sub(1);
        assert!(
            (1..=64).contains(&n_col),
            "colonne fuori tetto ({n_col}) per {src}: {spec}"
        );
        // HTML: nessun colspan oltre il tetto
        for pezzo in h.contenuto.split("colspan=\"").skip(1) {
            let v: usize = pezzo.split('"').next().unwrap().parse().unwrap();
            assert!(v <= 64, "colspan {v} per {src}");
        }
        assert!(!t.contenuto.is_empty());
    }
}

#[test]
fn colonne_oltre_il_tetto_sono_ridotte_con_avviso() {
    let src = "#table(columns: 100000000000, [a])";
    let (t, h, l) = tutte(src);
    for c in [&t, &h, &l] {
        assert!(
            c.avvisi.iter().any(|a| a.contains("colonne")),
            "avviso di clamp mancante: {:?}",
            c.avvisi
        );
    }
    // il tetto è 64: la tabella LaTeX ha esattamente 64 colonne
    let spec = l
        .contenuto
        .lines()
        .find(|r| r.starts_with("\\begin{tabular}"))
        .unwrap();
    assert_eq!(spec.matches("|l").count(), 64, "{spec}");
}

#[test]
fn nessun_panico_livello_titolo_enorme() {
    for src in [
        "#heading(level: 9223372036854775807)[x]\n#outline()",
        "#heading(level: -3)[x]\n#outline()",
        &format!("{} x\n\n#outline()", "=".repeat(5000)),
    ] {
        let (t, h, l) = tutte(src);
        for riga in t.contenuto.lines() {
            let rientro = riga.len() - riga.trim_start().len();
            assert!(rientro <= 12, "rientro {rientro} in {riga:?} per {src:.40}");
        }
        assert!(h.contenuto.contains("<h"), "{src:.40}");
        assert!(l.contenuto.contains("\\subparagraph") || l.contenuto.contains("\\section"));
    }
    let c = txt("#heading(level: 9223372036854775807)[x]\n#outline()");
    assert!(
        c.avvisi.iter().any(|a| a.contains("livello")),
        "{:?}",
        c.avvisi
    );
}

#[test]
fn nessun_panico_numero_iniziale_di_lista_estremo() {
    // `0.` : in LaTeX il contatore parte da -1 (non deve sottrarre da un u64 a zero)
    let t = tex("0. zero\n1. uno").contenuto;
    assert!(t.contains("\\setcounter{enumi}{-1}"), "{t}");
    // numero massimo: aritmetica saturante nel testo semplice
    let massimo = "18446744073709551615";
    let c = txt(&format!("{massimo}. a\n+ b"));
    assert!(
        c.contenuto.starts_with(&format!("{massimo}. a")),
        "{}",
        c.contenuto
    );
    assert!(c.contenuto.contains("\n"), "{}", c.contenuto);
    // LaTeX non accetta contatori oltre 2^31-1: ridotto con avviso
    let c = tex(&format!("{massimo}. a\n+ b"));
    let riga = c
        .contenuto
        .lines()
        .find(|r| r.starts_with("\\setcounter{enumi}"))
        .unwrap_or_else(|| panic!("{}", c.contenuto));
    let n: i64 = riga
        .trim_start_matches("\\setcounter{enumi}{")
        .trim_end_matches('}')
        .parse()
        .unwrap();
    assert!(n <= i64::from(i32::MAX), "{riga}");
    // HTML: attributo start dichiarato, nessun panico
    let _ = html(&format!("{massimo}. a\n+ b"));
}

#[test]
fn nessun_panico_fuzz_deterministico_su_troncamenti() {
    // Corpus di costrutti "a rischio", tagliato a ogni lunghezza e con
    // un generatore pseudo-casuale a seme fisso (nessuna dipendenza esterna).
    let semi = [
        "#table(columns: (1fr, 1fr), table.header[A][B], [x], table.cell(colspan: 2)[y])\n",
        "#figure(image(\"a.png\"), caption: [C]) <f>\n\nVedi @f.\n",
        "= Titolo <t>\n== Sotto\n#outline()\n- a\n  - b\n0. z\n1. u\n",
        "$ a &= b \\ c &= frac(1, 2) $ e $x^2$ #footnote[nota *forte*]\n",
        "/ Termine: descrizione\n#quote(block: true, attribution: [A])[q]\n```rust\nfn f() {}\n```\n",
    ];
    let mut stato: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut prossimo = || {
        stato ^= stato << 13;
        stato ^= stato >> 7;
        stato ^= stato << 17;
        stato
    };
    for s in semi {
        let carattere: Vec<char> = s.chars().collect();
        for fine in 0..=carattere.len() {
            let parte: String = carattere[..fine].iter().collect();
            let _ = tutte(&parte);
        }
        for _ in 0..40 {
            // mutazione: sostituisce un carattere con uno di una lista "ostile"
            let ostili = [
                '#', '$', '[', ']', '(', ')', '`', '"', '\\', '*', '_', '=', '9', '-',
            ];
            let mut v = carattere.clone();
            let k = (prossimo() % v.len() as u64) as usize;
            v[k] = ostili[(prossimo() % ostili.len() as u64) as usize];
            let m: String = v.iter().collect();
            let _ = tutte(&m);
        }
    }
    // valori limite in posizione numerica
    for n in [
        "0",
        "1",
        "63",
        "64",
        "65",
        "2147483647",
        "2147483648",
        "9223372036854775807",
        "9223372036854775808",
        "18446744073709551615",
        "18446744073709551616",
    ] {
        for modello in [
            "#table(columns: N, [a], [b])",
            "#heading(level: N)[x]\n#outline()",
            "N. a\n+ b",
            "#table(columns: 2, table.cell(colspan: N)[a], [b])",
            "#table(columns: 2, table.cell(rowspan: N)[a], [b])",
            "#image(\"a.png\", width: Npt)",
        ] {
            let _ = tutte(&modello.replace('N', n));
        }
    }
}
