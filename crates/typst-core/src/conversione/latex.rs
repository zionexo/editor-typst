//! Generatore LaTeX (.tex) dal documento intermedio.
//!
//! Documento completo per `pdflatex` (`\documentclass`, pacchetti caricati
//! solo se servono, `babel` secondo la lingua). I caratteri speciali del
//! testo sono sempre scappati (`# $ % & _ { } ~ ^ \`, più `< > |` e le
//! virgolette tipografiche).

use std::fmt::Write as _;

use super::ir::{
    lessico, testo_semplice, Allineamento, Avvisi, Blocco, Documento, Immagine, Inline, Misura,
    Tabella, TipoEt, TipoFigura,
};
use super::matematica::{latex as math_latex, richiede_allineamento, senza_allineamenti};
use super::OpzioniLatex;

/// Valore massimo accettato da TeX per un contatore (`\setcounter`).
const CONTATORE_MAX: i128 = 2_000_000_000;

#[derive(Default)]
struct Requisiti {
    math: bool,
    grafica: bool,
    ulem: bool,
    link: bool,
    cite: bool,
    /// Serve l'ambiente flottante `listato` (pacchetto `float`).
    listato: bool,
    /// Serve `ptionof` (pacchetto `caption`): figure in una voce di lista.
    captionof: bool,
}

pub(crate) struct Rend<'d> {
    doc: &'d Documento,
    opz: &'d OpzioniLatex,
    req: Requisiti,
    pub avvisi: Avvisi,
    /// Profondità delle liste `enumerate` (contatore `enumi…`).
    profondita_enum: usize,
    formule_ripiego: usize,
    /// `true` mentre si scrive il contenuto di una cella `tabular`: lì un
    /// `\\` terminerebbe la riga della tabella.
    in_cella: bool,
    /// Quante voci di lista (`itemize`/`enumerate`/`description`) sono aperte:
    /// lì un ambiente flottante (`figure`, `table`) non è ammesso.
    in_lista: usize,
}

/// Escape LaTeX per testo ordinario.
pub(crate) fn esc(t: &str) -> String {
    let mut o = String::with_capacity(t.len() + 8);
    for c in t.chars() {
        match c {
            '\\' => o.push_str("\\textbackslash{}"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '$' => o.push_str("\\$"),
            '&' => o.push_str("\\&"),
            '#' => o.push_str("\\#"),
            '%' => o.push_str("\\%"),
            '_' => o.push_str("\\_"),
            '^' => o.push_str("\\textasciicircum{}"),
            '~' => o.push_str("\\textasciitilde{}"),
            '<' => o.push_str("\\textless{}"),
            '>' => o.push_str("\\textgreater{}"),
            '|' => o.push_str("\\textbar{}"),
            '"' => o.push_str("\\textquotedbl{}"),
            '\u{00A0}' => o.push('~'),
            '\u{00AD}' => o.push_str("\\-"),
            '\u{2009}' | '\u{202F}' => o.push_str("\\,"),
            '“' => o.push_str("``"),
            '”' => o.push_str("''"),
            '‘' => o.push('`'),
            '’' => o.push('\''),
            '…' => o.push_str("\\ldots{}"),
            '−' => o.push_str("$-$"),
            '×' => o.push_str("$\\times$"),
            '±' => o.push_str("$\\pm$"),
            '≈' => o.push_str("$\\approx$"),
            '≤' => o.push_str("$\\leq$"),
            '≥' => o.push_str("$\\geq$"),
            '≠' => o.push_str("$\\neq$"),
            '→' => o.push_str("$\\rightarrow$"),
            '←' => o.push_str("$\\leftarrow$"),
            '∞' => o.push_str("$\\infty$"),
            '\r' | '\0' => {}
            c => o.push(c),
        }
    }
    o
}

/// Etichetta LaTeX sicura (`\label`/`\ref`/`\eqref`).
fn etichetta(e: &str) -> String {
    let mut s = String::from("et:");
    for c in e.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '.') {
            s.push(c);
        } else {
            s.push('_');
        }
    }
    s
}

/// Argomento di `\href`/`\url`: `%` e `#` scappati, caratteri non sicuri
/// codificati in percentuale.
fn url_latex(u: &str) -> String {
    let mut s = String::new();
    for c in u.trim().chars() {
        match c {
            '%' => s.push_str("\\%"),
            '#' => s.push_str("\\#"),
            '\\' | '{' | '}' | ' ' | '"' | '^' | '`' | '<' | '>' => {
                let mut b = [0u8; 4];
                for byte in c.encode_utf8(&mut b).bytes() {
                    let _ = write!(s, "%{byte:02X}");
                }
            }
            c if c.is_control() => {}
            c => s.push(c),
        }
    }
    s
}

fn misura_latex(m: &Misura) -> Option<String> {
    if !m.valore.is_finite() || m.valore <= 0.0 {
        return None;
    }
    let v = (m.valore * 1000.0).round() / 1000.0;
    Some(match m.unita {
        "%" => format!("{}\\linewidth", (v / 100.0 * 1000.0).round() / 1000.0),
        "cm" | "mm" | "pt" | "in" | "em" => format!("{v}{}", m.unita),
        _ => return None,
    })
}

fn nome_babel(l: &str) -> Option<&'static str> {
    Some(match l.split(['-', '_']).next().unwrap_or("") {
        "it" => "italian",
        "en" => "english",
        "fr" => "french",
        "de" => "ngerman",
        "es" => "spanish",
        "pt" => "portuguese",
        "nl" => "dutch",
        "ca" => "catalan",
        _ => return None,
    })
}

/// Comando LaTeX per il contatore delle sezioni (da pattern Typst `"I"`, …).
fn formato_sezione(pattern: &str) -> Option<&'static str> {
    let simboli: Vec<char> = pattern
        .chars()
        .filter(|c| matches!(c, '1' | 'a' | 'A' | 'i' | 'I'))
        .collect();
    match simboli.first() {
        Some('I') => Some("\\Roman"),
        Some('i') => Some("\\roman"),
        Some('A') => Some("\\Alph"),
        Some('a') => Some("\\alph"),
        _ => None,
    }
}

impl<'d> Rend<'d> {
    pub fn nuovo(doc: &'d Documento, opz: &'d OpzioniLatex) -> Self {
        Self {
            doc,
            opz,
            req: Requisiti::default(),
            avvisi: Avvisi::default(),
            profondita_enum: 0,
            formule_ripiego: 0,
            in_cella: false,
            in_lista: 0,
        }
    }

    fn avvisa(&mut self, m: String) {
        self.avvisi.aggiungi(m);
    }

    /// Comando che ridefinisce il titolo della bibliografia: `\refname` in
    /// `article`, `\bibname` in `report`/`book` (l'altro non esiste e un
    /// `\renewcommand` su un comando non definito è un errore).
    fn comando_titolo_bibliografia(&self) -> &'static str {
        if self.con_capitoli() {
            "\\bibname"
        } else {
            "\\refname"
        }
    }

    fn con_capitoli(&self) -> bool {
        matches!(self.opz.classe.as_str(), "report" | "book")
    }

    pub fn documento(&mut self) -> String {
        let doc = self.doc;
        let mut corpo = String::new();
        self.blocchi(&doc.blocchi, &mut corpo);
        if self.formule_ripiego > 0 {
            let n = self.formule_ripiego;
            self.avvisa(format!(
                "{n} formule non convertibili in LaTeX: riportato il sorgente Typst come testo"
            ));
        }

        let lingua = self
            .opz
            .lingua
            .clone()
            .unwrap_or_else(|| doc.lingua.clone());
        let classe = if self.opz.classe.chars().all(|c| c.is_ascii_alphanumeric())
            && !self.opz.classe.is_empty()
        {
            self.opz.classe.clone()
        } else {
            "article".to_string()
        };
        let pt = match self.opz.dimensione_pt {
            10..=12 => self.opz.dimensione_pt,
            _ => 11,
        };
        let carta = if self.opz.carta.chars().all(|c| c.is_ascii_alphanumeric())
            && !self.opz.carta.is_empty()
        {
            self.opz.carta.clone()
        } else {
            "a4paper".to_string()
        };

        let mut out = String::new();
        let _ = writeln!(out, "\\documentclass[{pt}pt,{carta}]{{{classe}}}");
        out.push_str("\\usepackage[T1]{fontenc}\n\\usepackage[utf8]{inputenc}\n");
        match nome_babel(&lingua) {
            Some(b) => {
                let _ = writeln!(out, "\\usepackage[{b}]{{babel}}");
            }
            None => self.avvisa(format!(
                "lingua «{lingua}» senza pacchetto babel noto: nessuna localizzazione"
            )),
        }
        if self.req.math {
            out.push_str("\\usepackage{amsmath,amssymb}\n");
        }
        if self.req.grafica {
            out.push_str("\\usepackage{graphicx}\n");
        }
        if self.req.ulem {
            out.push_str("\\usepackage[normalem]{ulem}\n");
        }
        if self.req.listato {
            let lex = lessico(&lingua);
            out.push_str("\\usepackage{float}\n\\newfloat{listato}{htbp}{lol}\n");
            let _ = writeln!(out, "\\floatname{{listato}}{{{}}}", esc(lex.listato));
        }
        if self.req.captionof {
            out.push_str("\\usepackage{caption}\n");
        }
        let titolo = doc.titolo.clone();
        let hyperref = self.req.link
            || self.req.cite
            || !doc.etichette.is_empty()
            || titolo.is_some()
            || doc.autore.is_some()
            || doc.blocchi.iter().any(|b| matches!(b, Blocco::Indice));
        if hyperref {
            out.push_str("\\usepackage{hyperref}\n");
            let mut opzioni = Vec::new();
            if let Some(t) = &titolo {
                opzioni.push(format!("pdftitle={{{}}}", esc(t)));
            }
            if let Some(a) = &doc.autore {
                opzioni.push(format!("pdfauthor={{{}}}", esc(a)));
            }
            if !opzioni.is_empty() {
                let _ = writeln!(out, "\\hypersetup{{{}}}", opzioni.join(","));
            }
        }
        if let Some(p) = &doc.numerazione_titoli {
            if let Some(f) = formato_sezione(p) {
                let cmd = if self.con_capitoli() {
                    "chapter"
                } else {
                    "section"
                };
                let _ = writeln!(out, "\\renewcommand{{\\the{cmd}}}{{{f}{{{cmd}}}}}");
            }
        }
        if self.opz.titolo_automatico {
            if let Some(t) = &titolo {
                let _ = writeln!(out, "\\title{{{}}}", esc(t));
                let _ = writeln!(
                    out,
                    "\\author{{{}}}",
                    esc(doc.autore.as_deref().unwrap_or(""))
                );
                out.push_str("\\date{}\n");
            }
        }
        out.push_str("\n\\begin{document}\n");
        if self.opz.titolo_automatico && titolo.is_some() {
            out.push_str("\\maketitle\n\n");
        }
        out.push_str(&corpo);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\\end{document}\n");
        out
    }

    fn blocchi(&mut self, bl: &[Blocco], out: &mut String) {
        for b in bl {
            self.blocco(b, out);
        }
    }

    /// `false` = contesto fragile (titoli, didascalie): `\protect\footnote`.
    fn inline(&mut self, v: &[Inline], out: &mut String, fragile: bool) {
        for i in v {
            match i {
                Inline::Testo(t) => out.push_str(&esc(t)),
                Inline::Spazio => out.push(' '),
                // In una cella `tabular` `\\` chiuderebbe la riga: lì è uno spazio.
                Inline::ACapo if self.in_cella => out.push(' '),
                Inline::ACapo => out.push_str("\\\\ "),
                Inline::Enfasi(x) => self.avvolto("\\emph", x, out, fragile),
                Inline::Forte(x) => self.avvolto("\\textbf", x, out, fragile),
                Inline::Barrato(x) => {
                    self.req.ulem = true;
                    self.avvolto("\\sout", x, out, fragile)
                }
                Inline::Sottolineato(x) => {
                    self.req.ulem = true;
                    self.avvolto("\\uline", x, out, fragile)
                }
                Inline::Apice(x) => self.avvolto("\\textsuperscript", x, out, fragile),
                Inline::Pedice(x) => self.avvolto("\\textsubscript", x, out, fragile),
                Inline::Virgolette(x) => {
                    out.push_str("``");
                    self.inline(x, out, fragile);
                    out.push_str("''");
                }
                Inline::Codice(t) => {
                    let _ = write!(out, "\\texttt{{{}}}", esc(t));
                }
                Inline::Link { url, contenuto } => {
                    self.req.link = true;
                    let u = url_latex(url);
                    let testo = testo_semplice(contenuto);
                    if testo == *url {
                        let _ = write!(out, "\\href{{{u}}}{{\\nolinkurl{{{u}}}}}");
                    } else {
                        let _ = write!(out, "\\href{{{u}}}{{");
                        self.inline(contenuto, out, true);
                        out.push('}');
                    }
                }
                Inline::Immagine(img) => self.immagine(img, out),
                Inline::Math(f) => {
                    if f.ha_grezzo() {
                        self.formule_ripiego += 1;
                        let _ = write!(out, "\\texttt{{{}}}", esc(&f.sorgente));
                    } else {
                        self.req.math = true;
                        // `&` e `\\` fuori da un ambiente di allineamento sono
                        // errori LaTeX: nella math in linea vengono tolti.
                        let m = senza_allineamenti(&f.m);
                        let _ = write!(out, "${}$", math_latex(&m).replace('\n', " "));
                    }
                }
                Inline::Nota(b) => {
                    if fragile {
                        out.push_str("\\protect");
                    }
                    out.push_str("\\footnote{");
                    let mut interno = String::new();
                    for (k, blocco) in b.iter().enumerate() {
                        if k > 0 {
                            interno.push_str("\n\n");
                        }
                        match blocco {
                            Blocco::Paragrafo(v) => self.inline(v, &mut interno, true),
                            altro => {
                                let a = testo_blocco(altro);
                                interno.push_str(&esc(&a));
                            }
                        }
                    }
                    out.push_str(&interno);
                    out.push('}');
                }
                Inline::Rif {
                    bersaglio,
                    supplemento,
                } => {
                    let suppl = supplemento.as_ref().map(|v| testo_semplice(v));
                    match self.doc.supplemento(bersaglio, suppl.as_deref()) {
                        Some((s, tipo)) => {
                            self.req.link = true;
                            // Titolo non numerato: `\ref` darebbe il numero della
                            // sezione precedente; si cita col solo testo (come gli
                            // altri formati), con collegamento all'àncora.
                            let senza_numero = tipo == TipoEt::Titolo
                                && self
                                    .doc
                                    .etichette
                                    .get(bersaglio)
                                    .is_some_and(|i| i.numero.is_empty());
                            if senza_numero {
                                let testo = if s.is_empty() { bersaglio.clone() } else { s };
                                let _ = write!(
                                    out,
                                    "\\hyperref[{}]{{{}}}",
                                    etichetta(bersaglio),
                                    esc(&testo)
                                );
                                continue;
                            }
                            let comando = if tipo == TipoEt::Equazione {
                                "\\eqref"
                            } else {
                                "\\ref"
                            };
                            if s.is_empty() {
                                let _ = write!(out, "{comando}{{{}}}", etichetta(bersaglio));
                            } else {
                                let _ = write!(
                                    out,
                                    "{}~{comando}{{{}}}",
                                    esc(&s),
                                    etichetta(bersaglio)
                                );
                            }
                        }
                        None => {
                            self.req.cite = true;
                            let chiave: String = bersaglio
                                .chars()
                                .map(|c| {
                                    if c == ',' || c.is_whitespace() || c == '{' || c == '}' {
                                        '_'
                                    } else {
                                        c
                                    }
                                })
                                .collect();
                            let _ = write!(out, "\\cite{{{chiave}}}");
                        }
                    }
                }
                Inline::Ancora(_) => {}
            }
        }
    }

    fn avvolto(&mut self, cmd: &str, x: &[Inline], out: &mut String, fragile: bool) {
        out.push_str(cmd);
        out.push('{');
        self.inline(x, out, fragile);
        out.push('}');
    }

    fn immagine(&mut self, i: &Immagine, out: &mut String) {
        self.req.grafica = true;
        let mut percorso = String::new();
        let mut sostituito = false;
        for c in i.percorso.replace('\\', "/").chars() {
            if matches!(c, '%' | '#' | '{' | '}' | '$' | '&' | '^' | '~') || c.is_control() {
                percorso.push('_');
                sostituito = true;
            } else {
                percorso.push(c);
            }
        }
        if sostituito {
            self.avvisa(format!(
                "percorso immagine «{}» con caratteri speciali: sostituiti con «_»",
                i.percorso
            ));
        }
        if percorso.to_ascii_lowercase().ends_with(".svg") {
            self.avvisa(format!(
                "immagine «{}» in formato SVG: pdflatex non la include, convertirla in PDF/PNG",
                i.percorso
            ));
        }
        out.push_str("\\includegraphics");
        if let Some(w) = i.larghezza.as_ref().and_then(misura_latex) {
            let _ = write!(out, "[width={w}]");
        }
        let _ = write!(out, "{{{percorso}}}");
    }

    fn comando_titolo(&mut self, livello: usize) -> &'static str {
        let capitoli = self.con_capitoli();
        let l = if capitoli { livello } else { livello + 1 };
        match l {
            1 => "chapter",
            2 => "section",
            3 => "subsection",
            4 => "subsubsection",
            5 => "paragraph",
            _ => {
                if l > 6 {
                    self.avvisa("titoli oltre il livello 5/6 resi come \\subparagraph".into());
                }
                "subparagraph"
            }
        }
    }

    fn blocco(&mut self, b: &Blocco, out: &mut String) {
        match b {
            Blocco::Titolo {
                livello,
                numero,
                contenuto,
                etichetta: et,
            } => {
                let cmd = self.comando_titolo(*livello);
                let numerato = numero.is_some();
                let _ = write!(out, "\\{cmd}{}{{", if numerato { "" } else { "*" });
                self.inline(contenuto, out, true);
                out.push('}');
                if let Some(e) = et {
                    if !numerato {
                        // `\section*` non avanza alcun contatore: `\phantomsection`
                        // (hyperref) crea l'àncora a cui punta il `\label`, così
                        // `\hyperref[…]` non resta «??». Il numero NON si cita.
                        out.push_str("\\phantomsection");
                    }
                    let _ = write!(out, "\\label{{{}}}", etichetta(e));
                }
                out.push_str("\n\n");
            }
            Blocco::Paragrafo(v) => {
                self.inline(v, out, false);
                out.push_str("\n\n");
            }
            Blocco::Lista {
                ordinata,
                inizio,
                voci,
            } => {
                let env = if *ordinata { "enumerate" } else { "itemize" };
                let _ = writeln!(out, "\\begin{{{env}}}");
                if *ordinata {
                    self.profondita_enum += 1;
                    if *inizio != 1 {
                        let nome = ["enumi", "enumii", "enumiii", "enumiv"]
                            [(self.profondita_enum - 1).min(3)];
                        // Il contatore precede la prima voce di uno: per `0.`
                        // vale -1 (TeX ammette contatori negativi). Aritmetica
                        // con segno e valore limitato al massimo di TeX.
                        let mut valore = i128::from(*inizio) - 1;
                        if valore > CONTATORE_MAX {
                            self.avvisa(format!(
                                "numero iniziale di lista {inizio}: ridotto al massimo ammesso da LaTeX"
                            ));
                            valore = CONTATORE_MAX;
                        }
                        let _ = writeln!(out, "\\setcounter{{{nome}}}{{{valore}}}");
                    }
                    if self.profondita_enum > 4 {
                        self.avvisa(
                            "liste numerate annidate oltre 4 livelli: LaTeX non le supporta".into(),
                        );
                    }
                }
                for v in voci {
                    let mut corpo = String::new();
                    self.voce(v, &mut corpo);
                    // `\item [x]` leggerebbe «[x]» come argomento opzionale:
                    // un gruppo vuoto davanti lo impedisce.
                    out.push_str(if corpo.starts_with('[') {
                        "\\item{} "
                    } else {
                        "\\item "
                    });
                    out.push_str(&corpo);
                    out.push('\n');
                }
                if *ordinata {
                    self.profondita_enum -= 1;
                }
                let _ = writeln!(out, "\\end{{{env}}}\n");
            }
            Blocco::Termini(v) => {
                out.push_str("\\begin{description}\n");
                for (t, d) in v {
                    // Il termine sta fra graffe: un `]` al suo interno
                    // chiuderebbe in anticipo l'argomento opzionale.
                    out.push_str("\\item[{");
                    self.inline(t, out, true);
                    out.push_str("}] ");
                    self.voce(d, out);
                    out.push('\n');
                }
                out.push_str("\\end{description}\n\n");
            }
            Blocco::Citazione {
                corpo,
                attribuzione,
            } => {
                let paragrafi = corpo
                    .iter()
                    .filter(|b| matches!(b, Blocco::Paragrafo(_)))
                    .count();
                let env = if paragrafi > 1 { "quotation" } else { "quote" };
                let _ = writeln!(out, "\\begin{{{env}}}");
                self.blocchi(corpo, out);
                if let Some(a) = attribuzione {
                    out.push_str("\\hfill\\textemdash\\ ");
                    self.inline(a, out, false);
                    out.push('\n');
                }
                let _ = writeln!(out, "\\end{{{env}}}\n");
            }
            Blocco::Codice { testo, .. } => {
                out.push_str("\\begin{verbatim}\n");
                out.push_str(&testo.replace("\\end{verbatim}", "\\end {verbatim}"));
                out.push_str("\n\\end{verbatim}\n\n");
            }
            Blocco::Figura {
                corpo,
                didascalia,
                tipo,
                etichetta: et,
                ..
            } => self.figura(corpo, didascalia.as_deref(), *tipo, et.as_deref(), out),
            Blocco::Immagine(i) => {
                out.push_str("\\begin{center}\n");
                self.immagine(i, out);
                out.push_str("\n\\end{center}\n\n");
            }
            Blocco::Tabella(t) => {
                out.push_str("\\begin{center}\n");
                self.tabular(t, out);
                out.push_str("\\end{center}\n\n");
            }
            Blocco::Equazione {
                formula,
                numero,
                etichetta: et,
            } => {
                if formula.ha_grezzo() {
                    self.formule_ripiego += 1;
                    let _ = writeln!(
                        out,
                        "\\begin{{center}}\n\\texttt{{{}}}\n\\end{{center}}\n",
                        esc(&formula.sorgente)
                    );
                    return;
                }
                self.req.math = true;
                let env = if numero.is_some() {
                    "equation"
                } else {
                    "equation*"
                };
                let _ = writeln!(out, "\\begin{{{env}}}");
                if let Some(e) = et {
                    if numero.is_some() {
                        let _ = writeln!(out, "\\label{{{}}}", etichetta(e));
                    }
                }
                if richiede_allineamento(&formula.m) {
                    let _ = writeln!(
                        out,
                        "\\begin{{aligned}}\n{}\n\\end{{aligned}}",
                        math_latex(&formula.m)
                    );
                } else {
                    let _ = writeln!(out, "{}", math_latex(&formula.m));
                }
                let _ = writeln!(out, "\\end{{{env}}}\n");
            }
            Blocco::Separatore => out.push_str("\\noindent\\rule{\\linewidth}{0.4pt}\n\n"),
            Blocco::SaltoPagina => out.push_str("\\newpage\n\n"),
            Blocco::Segnaposto(t) => {
                let _ = writeln!(out, "\\begin{{center}}\n[{}]\n\\end{{center}}\n", esc(t));
            }
            Blocco::Bibliografia {
                file,
                titolo,
                stile,
            } => {
                self.req.cite = true;
                // `article` definisce `\refname`, `report`/`book` `\bibname`:
                // ridefinire quello sbagliato è un errore («undefined»).
                let comando = self.comando_titolo_bibliografia();
                if let Some(v) = titolo {
                    if !v.is_empty() {
                        let t = {
                            let mut s = String::new();
                            self.inline(v, &mut s, true);
                            s
                        };
                        let _ = writeln!(out, "\\renewcommand{{{comando}}}{{{t}}}");
                    }
                } else {
                    let lex = lessico(&self.doc.lingua);
                    let _ = writeln!(
                        out,
                        "\\renewcommand{{{comando}}}{{{}}}",
                        esc(lex.bibliografia)
                    );
                }
                let stile_tex = match stile.as_deref() {
                    Some("ieee") => "ieeetran",
                    Some("apa") => "apalike",
                    _ => "plain",
                };
                let _ = writeln!(out, "\\bibliographystyle{{{stile_tex}}}");
                let nomi: Vec<String> = file
                    .split(',')
                    .map(|f| {
                        let f = f.trim();
                        let f = f.strip_suffix(".bib").unwrap_or(f);
                        f.chars()
                            .map(|c| {
                                if matches!(c, '{' | '}' | '%' | '#' | '\\' | ' ') {
                                    '_'
                                } else {
                                    c
                                }
                            })
                            .collect::<String>()
                    })
                    .filter(|f| !f.is_empty())
                    .collect();
                let _ = writeln!(out, "\\bibliography{{{}}}\n", nomi.join(","));
            }
            Blocco::Indice => {
                self.req.link = true;
                out.push_str("\\tableofcontents\n\n");
            }
        }
    }

    /// Contenuto di una voce di lista: primo paragrafo in linea.
    fn voce(&mut self, bl: &[Blocco], out: &mut String) {
        self.in_lista += 1;
        self.voce_interna(bl, out);
        self.in_lista -= 1;
    }

    fn voce_interna(&mut self, bl: &[Blocco], out: &mut String) {
        for (k, b) in bl.iter().enumerate() {
            match b {
                Blocco::Paragrafo(v) if k == 0 => self.inline(v, out, false),
                Blocco::Paragrafo(v) => {
                    out.push_str("\n\n");
                    self.inline(v, out, false);
                }
                altro => {
                    out.push('\n');
                    self.blocco(altro, out);
                }
            }
        }
    }

    fn figura(
        &mut self,
        corpo: &[Blocco],
        didascalia: Option<&[Inline]>,
        tipo: TipoFigura,
        et: Option<&str>,
        out: &mut String,
    ) {
        // I listati hanno un ambiente (e quindi un contatore) proprio, come in
        // Typst («Listato N» numera separatamente da «Figura N»).
        let ambiente = match tipo {
            TipoFigura::Tabella => "table",
            TipoFigura::Listato => {
                self.req.listato = true;
                "listato"
            }
            TipoFigura::Figura => "figure",
        };
        // In una voce di lista un ambiente flottante non è ammesso: si usa un
        // blocco `center` con `\captionof` (pacchetto `caption`), che numera
        // con lo stesso contatore ma resta nel punto in cui compare.
        let in_lista = self.in_lista > 0;
        if in_lista {
            self.req.captionof = true;
            out.push_str("\\begin{center}\n");
        } else {
            let _ = writeln!(out, "\\begin{{{ambiente}}}[htbp]\n\\centering");
        }
        let mut caption = String::new();
        if let Some(d) = didascalia {
            self.inline(d, &mut caption, true);
        }
        // tabelle: didascalia sopra
        let didascalia_sopra = tipo == TipoFigura::Tabella;
        let scrivi_caption = |out: &mut String| {
            if !caption.is_empty() {
                if in_lista {
                    let _ = writeln!(out, "\\captionof{{{ambiente}}}{{{caption}}}");
                } else {
                    let _ = writeln!(out, "\\caption{{{caption}}}");
                }
            } else {
                // Senza `\caption` il contatore non avanza e un `\label` si
                // riferirebbe all'elemento precedente: lo si fa avanzare a mano
                // (Typst numera comunque la figura).
                let _ = writeln!(out, "\\refstepcounter{{{ambiente}}}");
            }
            if let Some(e) = et {
                let _ = writeln!(out, "\\label{{{}}}", etichetta(e));
            }
        };
        if didascalia_sopra {
            scrivi_caption(out);
        }
        let mut scritto = false;
        for b in corpo {
            match b {
                Blocco::Immagine(i) => {
                    self.immagine(i, out);
                    out.push('\n');
                    scritto = true;
                }
                Blocco::Tabella(t) => {
                    self.tabular(t, out);
                    scritto = true;
                }
                Blocco::Codice { testo, .. } => {
                    out.push_str("\\begin{verbatim}\n");
                    out.push_str(&testo.replace("\\end{verbatim}", "\\end {verbatim}"));
                    out.push_str("\n\\end{verbatim}\n");
                    scritto = true;
                }
                Blocco::Paragrafo(v) => {
                    self.inline(v, out, true);
                    out.push('\n');
                    scritto = true;
                }
                Blocco::Segnaposto(t) => {
                    let _ = writeln!(
                        out,
                        "\\fbox{{\\parbox{{0.8\\linewidth}}{{\\centering [{}]}}}}",
                        esc(t)
                    );
                    scritto = true;
                }
                altro => {
                    self.blocco(altro, out);
                    scritto = true;
                }
            }
        }
        if !scritto {
            self.avvisa(
                "figura generata da codice Typst non convertibile: inserito un riquadro segnaposto"
                    .into(),
            );
            out.push_str("\\fbox{\\parbox{0.8\\linewidth}{\\centering [figura non convertita]}}\n");
        }
        if !didascalia_sopra {
            scrivi_caption(out);
        }
        if in_lista {
            out.push_str("\\end{center}\n\n");
        } else {
            let _ = writeln!(out, "\\end{{{ambiente}}}\n");
        }
    }

    fn tabular(&mut self, t: &Tabella, out: &mut String) {
        let colonne = t.colonne.max(1);
        let spec: String = (0..colonne)
            .map(|i| {
                match t
                    .allineamenti
                    .get(i)
                    .copied()
                    .unwrap_or(Allineamento::Sinistra)
                {
                    Allineamento::Sinistra => 'l',
                    Allineamento::Centro => 'c',
                    Allineamento::Destra => 'r',
                }
            })
            .map(|c| format!("|{c}"))
            .collect::<String>()
            + "|";
        let _ = writeln!(out, "\\begin{{tabular}}{{{spec}}}\n\\hline");
        let scrivi_righe =
            |s: &mut Self, righe: &[Vec<super::ir::Cella>], intest: bool, out: &mut String| {
                for r in righe {
                    let mut celle: Vec<String> = Vec::new();
                    let mut col = 0usize;
                    for c in r {
                        let mut testo = String::new();
                        s.in_cella = true;
                        s.inline(&c.contenuto, &mut testo, true);
                        s.in_cella = false;
                        if intest && !matches!(c.contenuto.as_slice(), [Inline::Forte(_)]) {
                            testo = format!("\\textbf{{{testo}}}");
                        }
                        let span = c.colspan.max(1).min(colonne);
                        if span > 1 {
                            let al = match t
                                .allineamenti
                                .get(col)
                                .copied()
                                .unwrap_or(Allineamento::Sinistra)
                            {
                                Allineamento::Sinistra => 'l',
                                Allineamento::Centro => 'c',
                                Allineamento::Destra => 'r',
                            };
                            celle.push(format!("\\multicolumn{{{span}}}{{|{al}|}}{{{testo}}}"));
                        } else {
                            celle.push(testo);
                        }
                        col += span;
                    }
                    let _ = writeln!(out, "{} \\\\ \\hline", celle.join(" & "));
                }
            };
        scrivi_righe(self, &t.intestazione, true, out);
        scrivi_righe(self, &t.righe, false, out);
        out.push_str("\\end{tabular}\n");
    }
}

/// Testo semplice di un blocco non paragrafo (usato nelle note).
fn testo_blocco(b: &Blocco) -> String {
    match b {
        Blocco::Paragrafo(v) => testo_semplice(v),
        Blocco::Codice { testo, .. } => testo.clone(),
        Blocco::Titolo { contenuto, .. } => testo_semplice(contenuto),
        Blocco::Lista { voci, .. } => voci
            .iter()
            .map(|v| v.iter().map(testo_blocco).collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("; "),
        Blocco::Citazione { corpo, .. } => {
            corpo.iter().map(testo_blocco).collect::<Vec<_>>().join(" ")
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn escape_speciali() {
        assert_eq!(esc("a#b$c%d&e_f{g}h~i^j\\k"), "a\\#b\\$c\\%d\\&e\\_f\\{g\\}h\\textasciitilde{}i\\textasciicircum{}j\\textbackslash{}k");
        assert_eq!(esc("“ciao” l’altro"), "``ciao'' l'altro");
        assert_eq!(esc("a<b>c|d"), "a\\textless{}b\\textgreater{}c\\textbar{}d");
    }

    #[test]
    fn url_e_label_sicuri() {
        assert_eq!(
            url_latex("https://a.it/x%20y#z"),
            "https://a.it/x\\%20y\\#z"
        );
        assert!(!etichetta("fig:a b}\\").contains(['}', '\\', ' ']));
    }
}
