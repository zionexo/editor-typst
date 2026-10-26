//! Test di integrazione delle conversioni (Markdown, testo, HTML, LaTeX) sui
//! due documenti reali di prova (`tests/fixtures/prova-export/`, copia
//! versionata di `output/prova-export/`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use typst_core::{
    md_a_typst, typst_a_html, typst_a_latex, typst_a_md, typst_a_md_con_avvisi, typst_a_testo,
    EditorWorld, OpzioniHtml, OpzioniLatex, OpzioniTesto,
};

fn cartella() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/prova-export")
}

fn leggi(nome: &str) -> String {
    std::fs::read_to_string(cartella().join(nome)).expect("fixture mancante")
}

/// Compila `testo` come `entry` nella cartella-progetto di prova.
/// Ritorna (pagine, messaggi di errore).
fn compila(testo: &str, entry: &str) -> (usize, Vec<String>) {
    let mut m = EditorWorld::nuovo("");
    m.apri_progetto(cartella(), entry, testo)
        .expect("apri_progetto");
    let r = m.compila();
    let errori = r
        .diagnostici
        .iter()
        .filter(|d| d.gravita == "errore")
        .map(|d| d.messaggio.clone())
        .collect();
    (r.pagine.len(), errori)
}

/// Conteggio dei comandi `#nome` presenti (set, show, let, figure, …).
fn comandi(src: &str) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '#' && i + 1 < c.len() && c[i + 1].is_ascii_alphabetic() {
            let mut j = i + 1;
            while j < c.len() && (c[j].is_ascii_alphanumeric() || matches!(c[j], '_' | '-' | '.')) {
                j += 1;
            }
            let nome: String = c[i + 1..j].iter().collect();
            // `#set.` non esiste: scarta il punto finale (fine frase)
            let nome = nome.trim_end_matches('.').to_string();
            *m.entry(nome).or_insert(0) += 1;
            i = j;
        } else {
            i += 1;
        }
    }
    m
}

// ---------------------------------------------------------------------------
// Markdown: round-trip sui documenti reali
// ---------------------------------------------------------------------------

fn round_trip(nome: &str) {
    let originale = leggi(nome);
    let (pagine_orig, err_orig) = compila(&originale, nome);
    assert!(
        err_orig.is_empty(),
        "{nome} originale non compila: {err_orig:?}"
    );

    let md = typst_a_md(&originale);
    let tornato = md_a_typst(&md);
    let entry = format!("{}.roundtrip.typ", nome.trim_end_matches(".typ"));
    let (pagine_rt, err_rt) = compila(&tornato, &entry);
    assert!(
        err_rt.is_empty(),
        "{nome}: il round-trip non compila: {err_rt:?}\n--- md ---\n{md}\n--- typ ---\n{tornato}"
    );
    let diff = pagine_orig.abs_diff(pagine_rt);
    assert!(
        diff <= 1,
        "{nome}: pagine originale {pagine_orig} vs round-trip {pagine_rt}"
    );

    // Stessi comandi presenti (set/show/let/figure/footnote/…): niente
    // `#set` ridotto a testo visibile, niente comando sparito. Le funzioni
    // riconosciute dal Markdown (image/quote/link) possono cambiare conteggio
    // ma devono restare presenti.
    let co = comandi(&originale);
    let cr = comandi(&tornato);
    for (cmd, n) in &co {
        let m = cr.get(cmd).copied().unwrap_or(0);
        assert!(
            m >= 1,
            "{nome}: il comando #{cmd} (x{n}) è sparito nel round-trip\n{tornato}"
        );
    }
    for cmd in [
        "set",
        "show",
        "let",
        "figure",
        "footnote",
        "bibliography",
        "table",
    ] {
        if let Some(n) = co.get(cmd) {
            assert_eq!(
                cr.get(cmd).copied().unwrap_or(0),
                *n,
                "{nome}: conteggio di #{cmd} cambiato"
            );
        }
    }
    // Nessuna riga di sorgente ridotta a testo visibile: ogni `set text(` del
    // Markdown deve stare dentro un blocco grezzo (preceduto da `#`).
    let mut dentro_fence = false;
    for riga in md.lines() {
        let t = riga.trim_start();
        if t.starts_with("```") {
            dentro_fence = !dentro_fence;
            continue;
        }
        if dentro_fence {
            continue;
        }
        assert!(
            !(t.starts_with("set ") || t.starts_with("show ") || t.starts_with("let ")),
            "{nome}: sorgente Typst senza # nel Markdown: {riga:?}"
        );
    }
    eprintln!(
        "{nome}: pagine {pagine_orig} -> {pagine_rt}; md {} byte; comandi {:?} -> {:?}",
        md.len(),
        co,
        cr
    );
}

#[test]
fn round_trip_scientifico() {
    round_trip("scientifico.typ");
}

#[test]
fn round_trip_letteratura() {
    round_trip("letteratura.typ");
}

#[test]
fn markdown_letteratura_ha_quote_note_e_avvisi() {
    let c = typst_a_md_con_avvisi(&leggi("letteratura.typ"));
    assert!(c.contenuto.contains("[^1]"), "manca il riferimento di nota");
    assert!(
        c.contenuto.contains("[^1]: Il torrente"),
        "manca la definizione di nota"
    );
    assert!(
        c.contenuto.contains("> Lascio a mio nipote"),
        "manca la citazione"
    );
    assert!(c
        .contenuto
        .contains("— dal testamento di Gaspare Montanari"));
    assert!(
        c.avvisi.iter().any(|a| a.contains("attribuzione")),
        "attribuzione persa in silenzio: {:?}",
        c.avvisi
    );
    // il paesaggio (con width) è conservato grezzo, lo stemma sta in un align
    assert!(c.contenuto.contains("#figure(image(\"img/paesaggio.png\""));
}

// ---------------------------------------------------------------------------
// Testo semplice
// ---------------------------------------------------------------------------

#[test]
fn testo_scientifico() {
    let c = typst_a_testo(&leggi("scientifico.typ"), &OpzioniTesto::default());
    let t = &c.contenuto;
    assert!(!t.is_empty());
    assert!(
        t.contains("1 Equazioni del moto\n===================="),
        "{t}"
    );
    assert!(
        t.contains("1.1 Profilo di Poiseuille\n-------------------------"),
        "{t}"
    );
    assert!(
        t.contains("ρ") && t.contains('∂') && t.contains('∇'),
        "formule"
    );
    assert!(t.contains("∑_(n = 1)^∞"), "somma");
    assert!(t.contains("[Figura 1: Profilo di velocità"), "{t}");
    assert!(t.contains("Tabella 1: Somme parziali"), "{t}");
    // la riga «1000» della tabella ASCII ha le quattro celle attese, in ordine
    let riga = t
        .lines()
        .find(|r| r.starts_with('|') && r.contains("1000"))
        .unwrap_or_else(|| panic!("riga 1000 assente:\n{t}"));
    let celle: Vec<&str> = riga.trim_matches('|').split('|').map(str::trim).collect();
    assert_eq!(celle, ["1000", "1.6439", "0.0010", "1.000"], "{riga}");
    assert!(t.contains("Eq. (2)"), "riferimento a equazione");
    for riga in t.lines() {
        assert!(
            riga.chars().count() <= 80 + 8,
            "riga troppo lunga: {riga:?}"
        );
    }
    assert!(!c.avvisi.is_empty());
}

#[test]
fn testo_letteratura_note_e_citazioni() {
    let c = typst_a_testo(&leggi("letteratura.typ"), &OpzioniTesto { larghezza: 60 });
    let t = &c.contenuto;
    assert!(t.contains("I La salita\n==========="), "{t}");
    assert!(t.contains("La diligenza lasciò"), "capolettera perso");
    assert!(t.contains("[1]"), "marcatore di nota");
    assert!(t.contains("\n[1] Il torrente"), "nota in coda: {t}");
    assert!(t.contains("    Lascio a mio nipote"), "citazione rientrata");
    assert!(t.contains("— dal testamento di Gaspare Montanari"));
    assert!(t.contains("[Figura 2: Il crinale al tramonto"), "{t}");
    for riga in t.lines() {
        assert!(riga.chars().count() <= 60 + 4, "riga oltre 60: {riga:?}");
    }
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

const VUOTI: &[&str] = &["meta", "br", "hr", "img", "link", "input", "col", "source"];

/// Verifica sommaria: tag bilanciati, nessun `<script`, nessun attributo `on…`.
fn html_ben_formato(h: &str) -> Result<(), String> {
    let low = h.to_ascii_lowercase();
    if low.contains("<script") {
        return Err("contiene <script".into());
    }
    let mut pila: Vec<String> = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    let mut i = 0;
    let b = h.as_bytes();
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if h[i..].starts_with("<!--") {
            return Err(format!("commento HTML inatteso a {i}"));
        }
        let fine = h[i..].find('>').ok_or("tag non chiuso")? + i;
        let tag = &h[i + 1..fine];
        i = fine + 1;
        if tag.starts_with('!') {
            continue;
        }
        if let Some(nome) = tag.strip_prefix('/') {
            let nome = nome.trim().to_ascii_lowercase();
            match pila.pop() {
                Some(a) if a == nome => {}
                altro => return Err(format!("</{nome}> inatteso (aperto: {altro:?})")),
            }
            continue;
        }
        let nome: String = tag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        // attributi: nessun gestore di evento
        let resto = &tag[nome.len()..];
        let mut k = 0;
        let rb = resto.as_bytes();
        while k < rb.len() {
            while k < rb.len() && (rb[k] as char).is_whitespace() {
                k += 1;
            }
            let ini = k;
            while k < rb.len() && rb[k] != b'=' && !(rb[k] as char).is_whitespace() && rb[k] != b'/'
            {
                k += 1;
            }
            let attr = resto[ini..k].to_ascii_lowercase();
            if attr.starts_with("on") && attr.len() > 2 {
                return Err(format!("attributo evento {attr} in <{nome}>"));
            }
            if k < rb.len() && rb[k] == b'=' {
                k += 1;
                if k < rb.len() && rb[k] == b'"' {
                    k += 1;
                    let v0 = k;
                    while k < rb.len() && rb[k] != b'"' {
                        k += 1;
                    }
                    if attr == "id" {
                        let id = &resto[v0..k.min(resto.len())];
                        if ids.contains(&id) {
                            return Err(format!("id duplicato {id:?}"));
                        }
                        ids.push(id);
                    }
                    k += 1;
                } else {
                    while k < rb.len() && !(rb[k] as char).is_whitespace() {
                        k += 1;
                    }
                }
            } else if k < rb.len() && rb[k] == b'/' {
                k += 1;
            }
            if k == ini {
                k += 1;
            }
        }
        if nome == "style" {
            // il CSS contiene '>' nei selettori: salta fino alla chiusura
            let c = h[i..].find("</style>").ok_or("<style> non chiuso")? + i;
            i = c + "</style>".len();
            continue;
        }
        if !VUOTI.contains(&nome.as_str()) && !tag.ends_with('/') {
            pila.push(nome);
        }
    }
    if pila.is_empty() {
        Ok(())
    } else {
        Err(format!("tag non chiusi: {pila:?}"))
    }
}

#[test]
fn html_scientifico_ben_formato_con_mathml() {
    let c = typst_a_html(&leggi("scientifico.typ"), &OpzioniHtml::default());
    let h = &c.contenuto;
    html_ben_formato(h).unwrap_or_else(|e| panic!("HTML mal formato: {e}"));
    assert!(h.starts_with("<!doctype html>"));
    assert!(h.contains("<html lang=\"it\">"));
    assert!(h.contains("<title>Moto laminare in un canale e integrali notevoli</title>"));
    assert!(h.contains("<meta name=\"viewport\""));
    assert!(h.contains("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\""));
    assert!(h.contains("<mfrac>") && h.contains("<munderover>") && h.contains("<msqrt>"));
    assert!(h.contains("<figure id=\"et-fig_3a_profilo\">"));
    assert!(h.contains("<figcaption>Figura 1: Profilo"));
    assert!(h.contains("<table>") && h.contains("<thead>") && h.contains("<th scope=\"col\""));
    assert!(h.contains("<a href=\"#et-eq_3a_ns\">Eq. (1)</a>"));
    assert!(h.contains("<h1 id=\"sez-1\"><span class=\"num\">1</span>"));
    assert!(!h.to_ascii_lowercase().contains("<script"));
}

#[test]
fn html_letteratura_ben_formato_con_note_e_citazioni() {
    let c = typst_a_html(&leggi("letteratura.typ"), &OpzioniHtml::default());
    let h = &c.contenuto;
    html_ben_formato(h).unwrap_or_else(|e| panic!("HTML mal formato: {e}"));
    assert!(h.contains("<blockquote>"));
    assert!(
        h.contains("<footer>\u{2014} <cite>dal testamento di Gaspare Montanari</cite></footer>")
    );
    assert!(h.contains("<sup class=\"nota\"><a id=\"rif-1\" href=\"#nota-1\">1</a></sup>"));
    assert!(h.contains("<li id=\"nota-2\">"));
    assert!(h.contains("<img src=\"img/paesaggio.png\""));
    // lo stemma (SVG, larghezza 3.2cm) è reso come <img> con la sua larghezza
    let img = h
        .split("<img ")
        .find(|p| p.starts_with("src=\"img/stemma.svg\""))
        .unwrap_or_else(|| panic!("<img> dello stemma assente:\n{h}"));
    let tag = img.split('>').next().unwrap();
    assert!(tag.contains("style=\"width:3.2cm\""), "{tag}");
}

#[test]
fn html_immagini_data_uri_opzionali() {
    use std::sync::Arc;
    let opz = OpzioniHtml {
        risolvi_immagine: Some(Arc::new(|p: &str| {
            Some(typst_core::data_uri("image/png", p.as_bytes()))
        })),
        ..Default::default()
    };
    let c = typst_a_html("#image(\"a.png\", alt: \"x\")", &opz);
    assert!(
        c.contenuto.contains("src=\"data:image/png;base64,"),
        "{}",
        c.contenuto
    );
    html_ben_formato(&c.contenuto).unwrap();
}

#[test]
fn html_nessuna_iniezione_dal_sorgente() {
    let sorgente = r#"
#set document(title: "</title><script>alert(1)</script>")
= Titolo <script>
Testo <script>alert(1)</script> e "virgolette" & <b>tag</b>.

#link("javascript:alert(1)")[clic]
#link("https://ok.it/?a=1&b=\"2\"")[ok]
#image("x\" onerror=\"alert(1)")
#figure(image("a.png", alt: "\" onmouseover=\"x"), caption: [<img src=x onerror=alert(1)>])
`<script>alert(2)</script>`

```html
<script>alert(3)</script>
```

- <script>
- #footnote[<script>alert(4)</script>]

#table(columns: 1, [<script>alert(5)</script>])
$ "<script>" $
@x<y>
"#;
    let c = typst_a_html(sorgente, &OpzioniHtml::default());
    let h = &c.contenuto;
    html_ben_formato(h).unwrap_or_else(|e| panic!("HTML mal formato: {e}\n{h}"));
    assert!(!h.contains("href=\"javascript:"));
    assert!(h.contains("&lt;script&gt;"));
    assert!(
        c.avvisi.iter().any(|a| a.contains("schema non consentito")),
        "{:?}",
        c.avvisi
    );
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

/// Ambienti `\begin{x}`/`\end{x}` bilanciati (e annidati correttamente).
fn ambienti_bilanciati(tex: &str) -> Result<(), String> {
    let mut pila: Vec<String> = Vec::new();
    let mut resto = tex;
    while let Some(p) = resto.find('\\') {
        resto = &resto[p..];
        for (prefisso, apre) in [("\\begin{", true), ("\\end{", false)] {
            if let Some(r) = resto.strip_prefix(prefisso) {
                let f = r.find('}').ok_or("graffa mancante")?;
                let nome = r[..f].to_string();
                if apre {
                    pila.push(nome);
                } else {
                    match pila.pop() {
                        Some(a) if a == nome => {}
                        altro => {
                            return Err(format!("\\end{{{nome}}} inatteso (aperto {altro:?})"))
                        }
                    }
                }
            }
        }
        resto = &resto[1..];
    }
    if pila.is_empty() {
        Ok(())
    } else {
        Err(format!("ambienti non chiusi: {pila:?}"))
    }
}

/// Graffe `{}` bilanciate ignorando quelle scappate.
fn graffe_bilanciate(tex: &str) -> bool {
    let mut livello: i64 = 0;
    let mut prec = ' ';
    let mut prec2 = ' ';
    for c in tex.chars() {
        let scappata = prec == '\\' && prec2 != '\\';
        match c {
            '{' if !scappata => livello += 1,
            '}' if !scappata => livello -= 1,
            _ => {}
        }
        if livello < 0 {
            return false;
        }
        prec2 = prec;
        prec = c;
    }
    livello == 0
}

fn compilatore_latex() -> Option<(&'static str, Vec<&'static str>)> {
    for (cmd, args) in [
        (
            "pdflatex",
            vec!["-interaction=nonstopmode", "-halt-on-error"],
        ),
        (
            "xelatex",
            vec!["-interaction=nonstopmode", "-halt-on-error"],
        ),
        ("tectonic", vec![]),
    ] {
        if Command::new(cmd).arg("--version").output().is_ok() {
            return Some((cmd, args));
        }
    }
    None
}

fn prova_compilazione_latex(nome: &str, tex: &str) {
    let Some((cmd, args)) = compilatore_latex() else {
        // Non passare muti: il test resta "verde" ma l'esito è tracciato in modo
        // evidente. Con `TYPSTER_RICHIEDI_LATEX=1` (CI) l'assenza diventa un errore.
        let msg = format!(
            "SALTATO: compilazione LaTeX di {nome} NON eseguita \
             (nessun pdflatex/xelatex/tectonic nel PATH): il .tex è stato \
             verificato solo staticamente"
        );
        if std::env::var_os("TYPSTER_RICHIEDI_LATEX").is_some() {
            panic!("{msg} (TYPSTER_RICHIEDI_LATEX è impostata)");
        }
        eprintln!("\n*** {msg} ***\n");
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("doc.tex"), tex).unwrap();
    // immagini relative usate dai documenti
    let src_img = cartella().join("img");
    let dst_img = dir.path().join("img");
    std::fs::create_dir_all(&dst_img).unwrap();
    for e in std::fs::read_dir(src_img).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), dst_img.join(e.file_name())).unwrap();
    }
    let out = Command::new(cmd)
        .args(&args)
        .arg("doc.tex")
        .current_dir(dir.path())
        .output()
        .expect("esecuzione compilatore");
    assert!(
        out.status.success(),
        "{cmd} ha fallito su {nome}:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn latex_scientifico() {
    let c = typst_a_latex(&leggi("scientifico.typ"), &OpzioniLatex::default());
    let t = &c.contenuto;
    ambienti_bilanciati(t).unwrap_or_else(|e| panic!("{e}\n{t}"));
    assert!(graffe_bilanciate(t), "graffe sbilanciate");
    assert!(t.starts_with("\\documentclass[11pt,a4paper]{article}"));
    assert!(t.contains("\\usepackage[italian]{babel}"));
    assert!(t.contains("\\usepackage{amsmath,amssymb}"));
    assert!(
        !t.contains("\\usepackage{graphicx}"),
        "graphicx non serve qui"
    );
    assert!(t.contains("\\section{Equazioni del moto}"));
    assert!(t.contains("\\subsection{Profilo di Poiseuille}"));
    assert!(t.contains("\\begin{equation}") && t.contains("\\label{et:eq:ns}"));
    assert!(t.contains("\\frac{G}{2 \\mu}"));
    assert!(t.contains("\\int_{- \\infty}^{\\infty}"));
    assert!(t.contains("\\sum_{n = 1}^{\\infty}"));
    assert!(t.contains("\\sqrt{\\pi}"));
    assert!(t.contains("Eq.~\\eqref{et:eq:ns}"));
    assert!(t.contains("Figura~\\ref{et:fig:profilo}"));
    assert!(t.contains("\\cite{batchelor}"));
    assert!(t.contains("\\begin{tabular}{|c|c|c|c|}"));
    assert!(t.contains("\\begin{table}") && t.contains("\\caption{Somme parziali"));
    assert!(t.contains("\\bibliography{refs}"));
    assert!(t.ends_with("\\end{document}\n"));
    prova_compilazione_latex("scientifico", t);
}

#[test]
fn latex_letteratura() {
    let c = typst_a_latex(&leggi("letteratura.typ"), &OpzioniLatex::default());
    let t = &c.contenuto;
    ambienti_bilanciati(t).unwrap_or_else(|e| panic!("{e}\n{t}"));
    assert!(graffe_bilanciate(t), "graffe sbilanciate");
    assert!(t.contains("\\usepackage{graphicx}"));
    assert!(t.contains("\\footnote{Il torrente"));
    assert!(t.contains("\\begin{quote}"));
    assert!(t.contains("dal testamento di Gaspare Montanari"));
    assert!(t.contains("\\includegraphics[width=0.9\\linewidth]{img/paesaggio.png}"));
    assert!(t.contains("\\caption{Il crinale al tramonto"));
    assert!(t.contains("\\renewcommand{\\thesection}{\\Roman{section}}"));
    // l'SVG non è includibile da pdflatex: deve esserci l'avviso
    assert!(c.avvisi.iter().any(|a| a.contains("SVG")), "{:?}", c.avvisi);
    prova_compilazione_latex("letteratura", t);
}

#[test]
fn latex_caratteri_speciali_scappati() {
    let c = typst_a_latex(
        r"Prezzo: 5\$ \#1 100% a&b x_y {z} \~ ^ \\ < > |",
        &OpzioniLatex::default(),
    );
    let t = &c.contenuto;
    for atteso in [
        "\\$",
        "\\#",
        "\\%",
        "\\&",
        "\\_",
        "\\{z\\}",
        "\\textasciitilde{}",
        "\\textasciicircum{}",
        "\\textbackslash{}",
        "\\textless{}",
        "\\textgreater{}",
        "\\textbar{}",
    ] {
        assert!(t.contains(atteso), "manca {atteso} in:\n{t}");
    }
    assert!(graffe_bilanciate(t));
}

// ---------------------------------------------------------------------------
// Prestazioni (soglia larga: rileva regressioni grossolane, non micro-tempi)
// ---------------------------------------------------------------------------

#[test]
fn conversioni_veloci() {
    use std::time::Instant;
    let s = leggi("scientifico.typ");
    let l = leggi("letteratura.typ");
    let t0 = Instant::now();
    for _ in 0..5 {
        for src in [&s, &l] {
            let _ = typst_a_testo(src, &OpzioniTesto::default());
            let _ = typst_a_html(src, &OpzioniHtml::default());
            let _ = typst_a_latex(src, &OpzioniLatex::default());
            let _ = typst_a_md(src);
        }
    }
    let ms = t0.elapsed().as_millis();
    eprintln!("40 conversioni in {ms} ms");
    assert!(ms < 10_000, "troppo lente: {ms} ms");
}

// ---------------------------------------------------------------------------
// Robustezza: nessun panico su sorgenti troncati o con Unicode insolito
// ---------------------------------------------------------------------------

#[test]
fn nessun_panico_su_sorgenti_troncati() {
    for nome in ["scientifico.typ", "letteratura.typ"] {
        let src = leggi(nome);
        let mut tagli: Vec<usize> = src.char_indices().map(|(i, _)| i).step_by(61).collect();
        tagli.push(src.len());
        for t in tagli {
            let parte = &src[..t];
            let _ = typst_a_md_con_avvisi(parte);
            let _ = md_a_typst(&typst_a_md(parte));
            let _ = typst_a_testo(parte, &OpzioniTesto::default());
            let h = typst_a_html(parte, &OpzioniHtml::default());
            assert!(!h.contenuto.to_ascii_lowercase().contains("<script"));
            let _ = typst_a_latex(parte, &OpzioniLatex::default());
        }
    }
}

#[test]
fn unicode_e_caratteri_di_controllo() {
    let src = "= Ünïcödé 日本語 🙂\n\n\u{0}\u{7f}\u{202e}testo *a\u{300}* $ α ⊕ β → γ $ #\"😀\"";
    let c = typst_a_html(src, &OpzioniHtml::default());
    html_ben_formato(&c.contenuto).unwrap();
    let _ = typst_a_latex(src, &OpzioniLatex::default());
    let _ = typst_a_testo(src, &OpzioniTesto::default());
    let _ = typst_a_md(src);
}

// ---------------------------------------------------------------------------
// Annidamento profondo: nessun overflow dello stack (thread con stack ridotto)
// ---------------------------------------------------------------------------

/// Stack del thread di prova (KiB): un quarto dei 2 MiB di un thread normale,
/// così una ricorsione senza limite si manifesta subito (verificato: senza i
/// limiti di profondità questo test va in overflow già a 50 livelli). Non può
/// scendere molto: in debug il solo parser di `typst-syntax` ha bisogno di
/// circa 600 KiB per le parentesi annidate al massimo della sua profondità
/// (256), per questo `annidati` limita alcuni casi.
const STACK_RIDOTTO_KIB: usize = 512;

/// Esegue `f` in un thread con stack ridotto e ne propaga i panici.
fn con_stack_ridotto<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(STACK_RIDOTTO_KIB * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("il thread di prova è andato in panico");
}

/// Sorgenti Typst annidate `n` volte per ciascun costrutto ricorsivo.
fn annidati(n: usize) -> Vec<(&'static str, String)> {
    let rip = |a: &str, b: &str, centro: &str| format!("{}{centro}{}", a.repeat(n), b.repeat(n));
    vec![
        ("strong()", rip("#strong[", "]", "x")),
        ("quote", rip("#quote(block: true)[", "]", "x")),
        ("footnote", rip("#footnote[", "]", "x")),
        ("figure", rip("#figure(", ", caption: [c])", "[x]")),
        ("blocco", rip("#[", "]", "x")),
        ("funzione ignota", rip("#f[", "]", "x")),
        (
            "parentesi",
            format!("#{}1{}", "(".repeat(n.min(100)), ")".repeat(n.min(100))),
        ),
        // catena di accessi a campo: l'albero del parser upstream è profondo
        // quanto la catena e il parser stesso non ha limite; oltre qualche
        // centinaio va in overflow prima di arrivare ai convertitori.
        ("campi", format!("#{}", vec!["a"; n.min(300)].join("."))),
        ("enfasi", rip("*_", "_*", "x")),
        (
            "lista",
            (0..n.min(400))
                .map(|k| format!("{}- v\n", "  ".repeat(k)))
                .collect(),
        ),
        (
            "math parentesi",
            format!("${}x{}$", "(".repeat(n), ")".repeat(n)),
        ),
        (
            "math frac",
            format!("${}x{}$", "frac(1, ".repeat(n), ")".repeat(n)),
        ),
        (
            "math apice",
            format!("${}x{}$", "a^(".repeat(n), ")".repeat(n)),
        ),
        (
            "math chiamate",
            format!("$ {}x{} $", "bold(".repeat(n), ")".repeat(n)),
        ),
        ("titolo+raw", rip("= #strong[", "]", "x\n```\nz\n```")),
        ("link", rip("#link(\"u\")[", "]", "x")),
        ("tabella", rip("#table(columns: 1, [", "])", "x")),
    ]
}

/// Esegue tutte le conversioni (nei due versi per il Markdown) su `src`.
fn tutte_le_conversioni(src: &str) {
    let _ = typst_a_testo(src, &OpzioniTesto::default());
    let _ = typst_a_html(src, &OpzioniHtml::default());
    let _ = typst_a_latex(src, &OpzioniLatex::default());
    let md = typst_a_md(src);
    let _ = md_a_typst(&md);
}

#[test]
fn nessun_overflow_di_stack_con_annidamento_di_migliaia_di_livelli() {
    con_stack_ridotto(|| {
        for n in [50, 300, 3000] {
            for (nome, src) in annidati(n) {
                eprintln!("annidamento {nome} x{n}");
                tutte_le_conversioni(&src);
            }
        }
    });
}

#[test]
fn nessun_overflow_di_stack_con_markdown_molto_annidato() {
    con_stack_ridotto(|| {
        let n = 5000;
        let citazioni = format!("{} x\n", ">".repeat(n));
        let liste: String = (0..400)
            .map(|k| format!("{}- v\n", "  ".repeat(k)))
            .collect();
        let enfasi = format!("{}x{}", "*a ".repeat(n), "*".repeat(n));
        let link = format!("{}x{}", "[a ".repeat(n), "](u)".repeat(n));
        for md in [citazioni, liste, enfasi, link] {
            let t = md_a_typst(&md);
            let _ = typst_a_md(&t);
            let _ = typst_a_testo(&t, &OpzioniTesto::default());
        }
    });
}
