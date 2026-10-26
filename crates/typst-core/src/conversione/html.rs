//! Generatore HTML5 autonomo dal documento intermedio.
//!
//! Pagina completa (`<!doctype html>`, `lang`, CSS inline sobrio con regole
//! responsive e di stampa), struttura semantica, formule in MathML con
//! ripiego `<code>`. **Nessun testo del sorgente è emesso senza escape** e
//! gli URL con schemi pericolosi (`javascript:`, `data:` non immagine, …)
//! sono rifiutati.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use super::ir::{
    lessico, testo_semplice, Allineamento, Avvisi, Blocco, Cella, Documento, Immagine, Inline,
    Misura, Tabella, TipoFigura,
};
use super::matematica::{lineare, mathml};
use super::OpzioniHtml;

const CSS: &str = r#":root{color-scheme:light dark;--fg:#1d1d1f;--bg:#fff;--mut:#5b5b66;--line:#d0d0d8;--acc:#1a56a0}
@media (prefers-color-scheme:dark){:root{--fg:#e8e8ec;--bg:#17171a;--mut:#a0a0ab;--line:#3a3a44;--acc:#7fb0ee}}
body{margin:0;background:var(--bg);color:var(--fg);font:1.0625rem/1.65 Georgia,"Times New Roman",serif}
main{max-width:42rem;margin:0 auto;padding:2rem 1.25rem 4rem}
h1,h2,h3,h4,h5,h6{line-height:1.25;margin:2em 0 .6em;font-family:system-ui,-apple-system,"Segoe UI",sans-serif}
h1{font-size:1.9rem}h2{font-size:1.5rem}h3{font-size:1.25rem}h4,h5,h6{font-size:1.05rem}
p{margin:.8em 0}
a{color:var(--acc)}
ul,ol{padding-left:1.6em}
blockquote{margin:1.2em 0;padding:.1em 1.1em;border-left:3px solid var(--line);color:var(--mut)}
blockquote footer{font-size:.9em;margin:.4em 0 .6em}
figure{margin:1.5em 0;text-align:center}
figure img,p>img{max-width:100%;height:auto}
figcaption{font-size:.9em;color:var(--mut);margin-top:.5em}
.tabella{overflow-x:auto;margin:1em 0}
table{border-collapse:collapse;margin:0 auto;font-size:.95em}
th,td{border:1px solid var(--line);padding:.35em .7em;vertical-align:top}
thead th{background:rgba(127,127,127,.12)}
pre{overflow-x:auto;padding:.8em 1em;border:1px solid var(--line);border-radius:4px;font-size:.9em;line-height:1.45}
code{font-family:ui-monospace,Consolas,"DejaVu Sans Mono",monospace;font-size:.92em}
.eq{display:grid;grid-template-columns:1fr auto;align-items:center;gap:1rem;margin:1em 0;overflow-x:auto}
.eq>math,.eq>code{justify-self:center}
.eq .num{color:var(--mut)}
math[display=block]{margin:1em 0}
.formula{white-space:pre-wrap}
.note{border-top:1px solid var(--line);margin-top:3rem;padding-top:.4rem;font-size:.9em;color:var(--mut)}
.note p{margin:.2em 0}
sup a{text-decoration:none}
.segnaposto{border:1px dashed var(--line);padding:.8em;color:var(--mut);text-align:center;font-size:.9em}
.indice ol{list-style:none;padding-left:0}
.indice .l2{margin-left:1.2em}.indice .l3{margin-left:2.4em}.indice .l4,.indice .l5,.indice .l6{margin-left:3.6em}
hr{border:0;border-top:1px solid var(--line);margin:2em 0}
@media (max-width:40rem){body{font-size:1rem}main{padding:1.2rem .9rem 3rem}}
@media print{body{background:#fff;color:#000;font-size:11pt}main{max-width:none;padding:0}a{color:inherit}figure,table,blockquote,pre,.eq{break-inside:avoid}hr.salto-pagina{break-after:page;border:0}}
"#;

pub(crate) struct Rend<'d> {
    doc: &'d Documento,
    opz: &'d OpzioniHtml,
    note: Vec<Vec<Blocco>>,
    pub avvisi: Avvisi,
    formule_ripiego: usize,
    /// Identificatori già assegnati (gli `id` HTML devono essere unici).
    ids: HashSet<String>,
    /// `id` di ogni titolo del documento (anche annidato), in ordine di
    /// documento; lo stesso ordine di `Documento::blocchi_titolo`.
    id_titoli: Vec<String>,
    /// Indirizzo del blocco `Titolo` → posizione in `id_titoli`. Si associa
    /// per **identità** del blocco (non per conteggio): i titoli annidati in
    /// citazioni, liste o figure ricevono l'`id` giusto qualunque sia l'ordine
    /// di resa.
    indice_titolo: HashMap<usize, usize>,
    /// Testo alternativo di ripiego per le immagini senza `alt`: la didascalia
    /// (in testo semplice) della figura che le contiene.
    alt_figura: Option<String>,
    /// Contatore per gli `id` dei titoli non pre-assegnati (dentro le note).
    titoli_extra: usize,
}

/// Chiave d'identità di un blocco (indirizzo): vale finché `Documento` vive.
fn chiave(b: &Blocco) -> usize {
    b as *const Blocco as usize
}

/// Escape HTML per testo e valori di attributo.
pub(crate) fn esc(t: &str) -> String {
    let mut o = String::with_capacity(t.len() + 8);
    esc_in(t, &mut o);
    o
}

fn esc_in(t: &str, o: &mut String) {
    for c in t.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '\0' => {}
            c => o.push(c),
        }
    }
}

/// Identificatore HTML derivato da un'etichetta: solo `[A-Za-z0-9_-]`, gli
/// altri caratteri sono codificati `_xx` (iniettivo).
fn id_da_etichetta(e: &str) -> String {
    let mut s = String::from("et-");
    for c in e.chars() {
        if c.is_ascii_alphanumeric() || c == '-' {
            s.push(c);
        } else {
            let _ = write!(s, "_{:x}_", c as u32);
        }
    }
    s
}

/// Verifica lo schema di un URL: `Some(url)` se sicuro (http, https, mailto,
/// tel, ftp o relativo).
fn url_sicuro(u: &str) -> Option<&str> {
    let u = u.trim();
    let normalizzato: String = u.chars().filter(|c| *c > ' ' && *c != '\u{7f}').collect();
    let fine_schema = normalizzato.find([':', '/', '?', '#']);
    match fine_schema {
        Some(i) if normalizzato.as_bytes()[i] == b':' => {
            let schema = normalizzato[..i].to_ascii_lowercase();
            if matches!(schema.as_str(), "http" | "https" | "mailto" | "tel" | "ftp") {
                Some(u)
            } else {
                None
            }
        }
        _ => Some(u),
    }
}

fn stile_misura(m: &Misura) -> Option<String> {
    if !m.valore.is_finite() || m.valore <= 0.0 {
        return None;
    }
    let unita = match m.unita {
        "%" | "cm" | "mm" | "pt" | "in" | "em" => m.unita,
        _ => return None,
    };
    Some(format!(
        "width:{}{}",
        (m.valore * 100.0).round() / 100.0,
        unita
    ))
}

fn classe_lingua(l: &str) -> Option<String> {
    let ok = !l.is_empty()
        && l.len() <= 24
        && l.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '#' | '_'));
    ok.then(|| l.to_string())
}

impl<'d> Rend<'d> {
    pub fn nuovo(doc: &'d Documento, opz: &'d OpzioniHtml) -> Self {
        // Gli id dei titoli si assegnano per primi: l'indice li referenzia e
        // un'etichetta ripetuta non deve duplicarli.
        let mut ids = HashSet::new();
        let mut id_titoli = Vec::new();
        let mut indice_titolo = HashMap::new();
        for (k, b) in doc.blocchi_titolo().into_iter().enumerate() {
            let et = match b {
                Blocco::Titolo { etichetta, .. } => etichetta.as_deref(),
                _ => None,
            };
            let da_etichetta = et.map(id_da_etichetta);
            let id = match da_etichetta {
                Some(id) if ids.insert(id.clone()) => id,
                _ => format!("sez-{}", k + 1),
            };
            ids.insert(id.clone());
            id_titoli.push(id);
            indice_titolo.insert(chiave(b), k);
        }
        let titoli_extra = id_titoli.len();
        Self {
            doc,
            opz,
            note: Vec::new(),
            avvisi: Avvisi::default(),
            formule_ripiego: 0,
            ids,
            id_titoli,
            indice_titolo,
            alt_figura: None,
            titoli_extra,
        }
    }

    /// `id` HTML di un blocco `Titolo`: quello pre-assegnato, oppure (titolo
    /// dentro una nota, copia del blocco originale) uno nuovo e unico.
    fn id_titolo(&mut self, b: &Blocco) -> String {
        if let Some(&k) = self.indice_titolo.get(&chiave(b)) {
            return self.id_titoli[k].clone();
        }
        loop {
            self.titoli_extra += 1;
            let id = format!("sez-{}", self.titoli_extra);
            if self.ids.insert(id.clone()) {
                return id;
            }
        }
    }

    /// Attributo ` id="…"` per un'etichetta, se l'identificatore è ancora libero.
    fn attr_id(&mut self, etichetta: &str) -> String {
        let id = id_da_etichetta(etichetta);
        if self.ids.insert(id.clone()) {
            format!(" id=\"{id}\"")
        } else {
            String::new()
        }
    }

    fn avvisa(&mut self, m: String) {
        self.avvisi.aggiungi(m);
    }

    pub fn documento(&mut self) -> String {
        let mut corpo = String::new();
        let doc = self.doc;
        self.blocchi(&doc.blocchi, &mut corpo);
        // note
        let mut note_html = String::new();
        let mut i = 0;
        while i < self.note.len() {
            let blocchi = self.note[i].clone();
            let n = i + 1;
            let _ = write!(note_html, "<li id=\"nota-{n}\">");
            self.contenuto_voce(&blocchi, &mut note_html);
            let _ = writeln!(
                note_html,
                " <a class=\"ritorno\" href=\"#rif-{n}\" aria-label=\"Torna al testo\">\u{21A9}</a></li>"
            );
            i += 1;
        }
        if !note_html.is_empty() {
            corpo.push_str("<section class=\"note\" aria-label=\"Note\">\n<ol>\n");
            corpo.push_str(&note_html);
            corpo.push_str("</ol>\n</section>\n");
        }
        if self.formule_ripiego > 0 {
            let n = self.formule_ripiego;
            self.avvisa(format!(
                "{n} formule non convertibili in MathML: mostrato il sorgente Typst come codice"
            ));
        }

        let lingua = self
            .opz
            .lingua
            .clone()
            .unwrap_or_else(|| self.doc.lingua.clone());
        let lingua = if !lingua.is_empty()
            && lingua.len() <= 16
            && lingua
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            lingua
        } else {
            "en".to_string()
        };
        let titolo = self
            .opz
            .titolo
            .clone()
            .or_else(|| self.doc.titolo.clone())
            .or_else(|| {
                self.doc
                    .titoli()
                    .first()
                    .map(|(_, _, c, _)| testo_semplice(c))
            })
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "Documento".to_string());

        let mut out = String::with_capacity(corpo.len() + CSS.len() + 512);
        out.push_str("<!doctype html>\n");
        let _ = writeln!(out, "<html lang=\"{}\">", esc(&lingua));
        out.push_str("<head>\n<meta charset=\"utf-8\">\n");
        out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        let _ = writeln!(out, "<title>{}</title>", esc(&titolo));
        if let Some(a) = &self.doc.autore {
            let _ = writeln!(out, "<meta name=\"author\" content=\"{}\">", esc(a));
        }
        out.push_str("<meta name=\"generator\" content=\"Typster\">\n");
        out.push_str("<style>\n");
        out.push_str(CSS);
        out.push_str("</style>\n</head>\n<body>\n<main>\n");
        out.push_str(&corpo);
        out.push_str("</main>\n</body>\n</html>\n");
        out
    }

    fn blocchi(&mut self, bl: &[Blocco], out: &mut String) {
        for b in bl {
            self.blocco(b, out);
        }
    }

    /// Contenuto di una voce di lista/nota: il primo paragrafo senza `<p>`.
    fn contenuto_voce(&mut self, bl: &[Blocco], out: &mut String) {
        for (k, b) in bl.iter().enumerate() {
            match b {
                Blocco::Paragrafo(v) if k == 0 => self.inline(v, out),
                altro => {
                    out.push('\n');
                    self.blocco(altro, out);
                }
            }
        }
    }

    fn blocco(&mut self, b: &Blocco, out: &mut String) {
        match b {
            Blocco::Titolo {
                livello,
                numero,
                contenuto,
                ..
            } => {
                let l = (*livello).clamp(1, 6);
                let id = self.id_titolo(b);
                let _ = write!(out, "<h{l} id=\"{id}\">");
                if let Some(n) = numero {
                    if !n.is_empty() {
                        let _ = write!(out, "<span class=\"num\">{}</span> ", esc(n));
                    }
                }
                self.inline(contenuto, out);
                let _ = writeln!(out, "</h{l}>");
            }
            Blocco::Paragrafo(v) => {
                out.push_str("<p>");
                self.inline(v, out);
                out.push_str("</p>\n");
            }
            Blocco::Lista {
                ordinata,
                inizio,
                voci,
            } => {
                if *ordinata {
                    if *inizio != 1 {
                        let _ = writeln!(out, "<ol start=\"{inizio}\">");
                    } else {
                        out.push_str("<ol>\n");
                    }
                } else {
                    out.push_str("<ul>\n");
                }
                for v in voci {
                    out.push_str("<li>");
                    self.contenuto_voce(v, out);
                    out.push_str("</li>\n");
                }
                out.push_str(if *ordinata { "</ol>\n" } else { "</ul>\n" });
            }
            Blocco::Termini(v) => {
                out.push_str("<dl>\n");
                for (t, d) in v {
                    out.push_str("<dt>");
                    self.inline(t, out);
                    out.push_str("</dt>\n<dd>");
                    self.contenuto_voce(d, out);
                    out.push_str("</dd>\n");
                }
                out.push_str("</dl>\n");
            }
            Blocco::Citazione {
                corpo,
                attribuzione,
            } => {
                out.push_str("<blockquote>\n");
                self.blocchi(corpo, out);
                if let Some(a) = attribuzione {
                    out.push_str("<footer>\u{2014} <cite>");
                    self.inline(a, out);
                    out.push_str("</cite></footer>\n");
                }
                out.push_str("</blockquote>\n");
            }
            Blocco::Codice { lingua, testo } => {
                out.push_str("<pre><code");
                if let Some(c) = lingua.as_deref().and_then(classe_lingua) {
                    let _ = write!(out, " class=\"language-{}\"", esc(&c));
                }
                out.push('>');
                esc_in(testo, out);
                out.push_str("</code></pre>\n");
            }
            Blocco::Figura {
                corpo,
                didascalia,
                tipo,
                numero,
                etichetta,
            } => {
                out.push_str("<figure");
                if let Some(e) = etichetta {
                    let a = self.attr_id(e);
                    out.push_str(&a);
                }
                out.push_str(">\n");
                let precedente = self.alt_figura.take();
                self.alt_figura = didascalia
                    .as_deref()
                    .map(testo_semplice)
                    .filter(|t| !t.trim().is_empty());
                self.blocchi(corpo, out);
                self.alt_figura = precedente;
                let lex = lessico(&self.doc.lingua);
                let parola = match tipo {
                    TipoFigura::Figura => lex.figura,
                    TipoFigura::Tabella => lex.tabella,
                    TipoFigura::Listato => lex.listato,
                };
                let _ = write!(out, "<figcaption>{} {}", esc(parola), esc(numero));
                if let Some(d) = didascalia {
                    out.push_str(": ");
                    self.inline(d, out);
                }
                out.push_str("</figcaption>\n</figure>\n");
            }
            Blocco::Immagine(i) => {
                out.push_str("<p>");
                self.immagine(i, out);
                out.push_str("</p>\n");
            }
            Blocco::Tabella(t) => self.tabella(t, out),
            Blocco::Equazione {
                formula,
                numero,
                etichetta,
            } => {
                out.push_str("<div class=\"eq\"");
                if let Some(e) = etichetta {
                    let a = self.attr_id(e);
                    out.push_str(&a);
                }
                out.push('>');
                if formula.ha_grezzo() {
                    self.formule_ripiego += 1;
                    let _ = write!(
                        out,
                        "<code class=\"formula\">{}</code>",
                        esc(&formula.sorgente)
                    );
                } else {
                    out.push_str(&mathml(&formula.m, true, &lineare(&formula.m)));
                }
                if let Some(n) = numero {
                    let _ = write!(out, "<span class=\"num\">{}</span>", esc(n));
                }
                out.push_str("</div>\n");
            }
            Blocco::Separatore => out.push_str("<hr>\n"),
            Blocco::SaltoPagina => out.push_str("<hr class=\"salto-pagina\">\n"),
            Blocco::Segnaposto(t) => {
                let _ = writeln!(out, "<p class=\"segnaposto\">[{}]</p>", esc(t));
            }
            Blocco::Bibliografia { file, titolo, .. } => {
                let lex = lessico(&self.doc.lingua);
                out.push_str("<section class=\"bibliografia\">\n");
                match titolo {
                    Some(v) if v.is_empty() => {}
                    Some(v) => {
                        out.push_str("<h2>");
                        self.inline(v, out);
                        out.push_str("</h2>\n");
                    }
                    None => {
                        let _ = writeln!(out, "<h2>{}</h2>", esc(lex.bibliografia));
                    }
                }
                let _ = writeln!(
                    out,
                    "<p class=\"segnaposto\">[Fonti bibliografiche: {} \u{2013} elenco non incluso]</p>\n</section>",
                    esc(file)
                );
            }
            Blocco::Indice => {
                let lex = lessico(&self.doc.lingua);
                let _ = writeln!(
                    out,
                    "<nav class=\"indice\" aria-label=\"{0}\">\n<h2>{0}</h2>\n<ol>",
                    esc(lex.indice)
                );
                let doc = self.doc;
                for t in doc.blocchi_titolo() {
                    let Blocco::Titolo {
                        livello,
                        numero,
                        contenuto,
                        ..
                    } = t
                    else {
                        continue;
                    };
                    let id = self.id_titolo(t);
                    let _ = write!(
                        out,
                        "<li class=\"l{}\"><a href=\"#{id}\">",
                        (*livello).clamp(1, 6)
                    );
                    if let Some(n) = numero {
                        if !n.is_empty() {
                            let _ = write!(out, "{} ", esc(n));
                        }
                    }
                    // testo semplice (come nel testo piano): niente `<a>`
                    // annidati né note ripetute dentro la voce d'indice
                    esc_in(&testo_semplice(contenuto), out);
                    out.push_str("</a></li>\n");
                }
                out.push_str("</ol>\n</nav>\n");
            }
        }
    }

    fn immagine(&mut self, i: &Immagine, out: &mut String) {
        let da_callback = self
            .opz
            .risolvi_immagine
            .as_ref()
            .and_then(|f| f(&i.percorso))
            .filter(|u| {
                let m = u.trim().to_ascii_lowercase();
                m.starts_with("data:image/") || url_sicuro(u).is_some()
            });
        let src = match da_callback {
            Some(u) => u,
            None => match url_sicuro(&i.percorso) {
                Some(u) => u.to_string(),
                None => {
                    self.avvisa(format!(
                        "immagine «{}» con schema non consentito: omessa",
                        i.percorso
                    ));
                    return;
                }
            },
        };
        // `alt:` dell'immagine (o della figura), altrimenti la didascalia
        let alt = i
            .alt
            .clone()
            .or_else(|| self.alt_figura.clone())
            .unwrap_or_default();
        let _ = write!(
            out,
            "<img src=\"{}\" alt=\"{}\" loading=\"lazy\"",
            esc(&src),
            esc(&alt)
        );
        if let Some(s) = i.larghezza.as_ref().and_then(stile_misura) {
            let _ = write!(out, " style=\"{}\"", esc(&s));
        }
        out.push('>');
    }

    fn cella(&mut self, c: &Cella, intest: bool, al: Allineamento, out: &mut String) {
        let tag = if intest { "th" } else { "td" };
        let _ = write!(out, "<{tag}");
        if intest {
            out.push_str(" scope=\"col\"");
        }
        if c.colspan > 1 {
            let _ = write!(out, " colspan=\"{}\"", c.colspan);
        }
        match al {
            Allineamento::Centro => out.push_str(" style=\"text-align:center\""),
            Allineamento::Destra => out.push_str(" style=\"text-align:right\""),
            Allineamento::Sinistra => {}
        }
        out.push('>');
        self.inline(&c.contenuto, out);
        let _ = write!(out, "</{tag}>");
    }

    fn tabella(&mut self, t: &Tabella, out: &mut String) {
        out.push_str("<div class=\"tabella\"");
        if let Some(e) = &t.etichetta {
            let a = self.attr_id(e);
            out.push_str(&a);
        }
        out.push_str(">\n<table>\n");
        let al = |col: usize| {
            t.allineamenti
                .get(col)
                .copied()
                .unwrap_or(Allineamento::Sinistra)
        };
        if !t.intestazione.is_empty() {
            out.push_str("<thead>\n");
            for r in &t.intestazione {
                out.push_str("<tr>");
                let mut col = 0;
                for c in r {
                    self.cella(c, true, al(col), out);
                    col += c.colspan.max(1);
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</thead>\n");
        }
        out.push_str("<tbody>\n");
        for r in &t.righe {
            out.push_str("<tr>");
            let mut col = 0;
            for c in r {
                self.cella(c, false, al(col), out);
                col += c.colspan.max(1);
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n</table>\n</div>\n");
    }

    fn inline(&mut self, v: &[Inline], out: &mut String) {
        for i in v {
            match i {
                Inline::Testo(t) => esc_in(t, out),
                Inline::Spazio => out.push(' '),
                Inline::ACapo => out.push_str("<br>"),
                Inline::Enfasi(x) => self.avvolto("em", x, out),
                Inline::Forte(x) => self.avvolto("strong", x, out),
                Inline::Barrato(x) => self.avvolto("s", x, out),
                Inline::Sottolineato(x) => self.avvolto("u", x, out),
                Inline::Apice(x) => self.avvolto("sup", x, out),
                Inline::Pedice(x) => self.avvolto("sub", x, out),
                Inline::Virgolette(x) => self.avvolto("q", x, out),
                Inline::Codice(t) => {
                    out.push_str("<code>");
                    esc_in(t, out);
                    out.push_str("</code>");
                }
                Inline::Link { url, contenuto } => match url_sicuro(url) {
                    Some(u) => {
                        let _ = write!(out, "<a href=\"{}\">", esc(u));
                        self.inline(contenuto, out);
                        out.push_str("</a>");
                    }
                    None => {
                        self.avvisa(format!(
                            "collegamento con schema non consentito «{url}»: reso come testo"
                        ));
                        self.inline(contenuto, out);
                    }
                },
                Inline::Immagine(img) => self.immagine(img, out),
                Inline::Math(f) => {
                    if f.ha_grezzo() {
                        self.formule_ripiego += 1;
                        let _ = write!(out, "<code class=\"formula\">{}</code>", esc(&f.sorgente));
                    } else {
                        out.push_str(&mathml(&f.m, false, &lineare(&f.m)));
                    }
                }
                Inline::Nota(b) => {
                    self.note.push(b.clone());
                    let n = self.note.len();
                    let _ = write!(
                        out,
                        "<sup class=\"nota\"><a id=\"rif-{n}\" href=\"#nota-{n}\">{n}</a></sup>"
                    );
                }
                Inline::Rif {
                    bersaglio,
                    supplemento,
                } => {
                    let suppl = supplemento.as_ref().map(|v| testo_semplice(v));
                    match self.doc.risolvi(bersaglio, suppl.as_deref()) {
                        Some(t) => {
                            let _ = write!(
                                out,
                                "<a href=\"#{}\">{}</a>",
                                id_da_etichetta(bersaglio),
                                esc(&t)
                            );
                        }
                        None => {
                            let _ = write!(out, "<cite class=\"rif\">[{}]</cite>", esc(bersaglio));
                        }
                    }
                }
                Inline::Ancora(e) => {
                    let a = self.attr_id(e);
                    if !a.is_empty() {
                        let _ = write!(out, "<span{a}></span>");
                    }
                }
            }
        }
    }

    fn avvolto(&mut self, tag: &str, x: &[Inline], out: &mut String) {
        let _ = write!(out, "<{tag}>");
        self.inline(x, out);
        let _ = write!(out, "</{tag}>");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn escape_completo() {
        assert_eq!(
            esc("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn url_pericolosi_rifiutati() {
        assert!(url_sicuro("javascript:alert(1)").is_none());
        assert!(url_sicuro("  JaVa\tScRiPt:alert(1)").is_none());
        assert!(url_sicuro("data:text/html;base64,AAAA").is_none());
        assert!(url_sicuro("vbscript:x").is_none());
        assert_eq!(url_sicuro("https://a.it/x?y=1"), Some("https://a.it/x?y=1"));
        assert_eq!(url_sicuro("img/a.png"), Some("img/a.png"));
        assert_eq!(url_sicuro("#ancora"), Some("#ancora"));
        assert_eq!(url_sicuro("mailto:a@b.it"), Some("mailto:a@b.it"));
    }

    #[test]
    fn id_etichetta_iniettivo_e_sicuro() {
        assert_eq!(id_da_etichetta("fig:profilo"), "et-fig_3a_profilo");
        assert_ne!(id_da_etichetta("a:b"), id_da_etichetta("a-b"));
        assert!(!id_da_etichetta("\"><script>").contains('<'));
    }
}
