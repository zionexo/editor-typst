//! Matematica Typst → rappresentazione intermedia `M` → MathML / LaTeX / testo lineare.
//!
//! La math Typst viene letta dal syntax tree (`typst-syntax`) e convertita in
//! un piccolo albero [`M`] indipendente dal formato; tre generatori ne
//! producono MathML, LaTeX e una forma lineare leggibile (Unicode).
//!
//! Sottoinsieme coperto: numeri, variabili, lettere greche, operatori e
//! relazioni, frazioni, apici/pedici (anche limiti di somme/integrali),
//! radici, delimitatori, stili (`bold`, `cal`, `bb`, …), accenti, matrici,
//! vettori, `cases`, funzioni operatore (`sin`, `lim`, …), spazi (`quad`) e
//! allineamenti (`&`, `\`). Ciò che non si riconosce diventa [`M::Grezzo`] e i
//! chiamanti ripiegano sul sorgente Typst (con avviso).

use typst_syntax::ast::{self, AstNode};

use super::ir::Avvisi;

// =======================================================================
// Tipi
// =======================================================================

/// Classe tipografica di un atomo (determina spaziatura e sizing).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Classe {
    Ordinario,
    Binario,
    Relazione,
    Apertura,
    Chiusura,
    Punteggiatura,
    /// Operatori grandi (somma, integrale, …).
    Grande,
    /// Nomi di funzione (`sin`, `lim`, …).
    Funzione,
}

/// Natura dell'atomo (determina l'elemento MathML).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tipo {
    Num,
    /// Identificatore in corsivo (variabile).
    Id,
    /// Identificatore dritto (funzioni, maiuscole greche, simboli).
    IdDritto,
    Op,
    Testo,
}

/// Atomo: un simbolo con la sua resa Unicode e LaTeX.
#[derive(Clone, Debug)]
pub(crate) struct Atomo {
    /// Resa Unicode (per MathML e forma lineare).
    pub u: String,
    /// Resa LaTeX (comando o carattere, già pronta da emettere in math mode).
    pub l: String,
    pub tipo: Tipo,
    pub classe: Classe,
    /// Gli apici/pedici vanno sopra/sotto (somma, `lim`) invece che di lato.
    pub limiti: bool,
}

/// Stile tipografico di una lettera.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Stile {
    Grassetto,
    Corsivo,
    Dritto,
    Calligrafico,
    Lavagna,
    Gotico,
    Monospazio,
}

/// Accento sopra/sotto un'espressione.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Accento {
    Cappello,
    Tilde,
    Barra,
    Punto,
    DuePunti,
    Freccia,
    Breve,
    Caron,
    SopraLinea,
    SottoLinea,
}

/// Albero della formula.
#[derive(Clone, Debug)]
pub(crate) enum M {
    A(Atomo),
    Riga(Vec<M>),
    Frac(Box<M>, Box<M>),
    Binom(Box<M>, Box<M>),
    Att {
        base: Box<M>,
        pedice: Option<Box<M>>,
        apice: Option<Box<M>>,
    },
    Rad {
        indice: Option<Box<M>>,
        radicando: Box<M>,
    },
    Delim {
        apri: String,
        chiudi: String,
        corpo: Box<M>,
    },
    Stile(Stile, Box<M>),
    Accento(Accento, Box<M>),
    Matrice {
        righe: Vec<Vec<M>>,
        apri: String,
        chiudi: String,
    },
    Casi(Vec<M>),
    /// Spazio orizzontale, in `em`.
    Spazio(f32),
    /// Punto di allineamento `&`.
    Allinea,
    /// A capo `\`.
    ACapo,
    /// Costrutto non convertibile (sorgente Typst originale).
    Grezzo(String),
}

/// Formula pronta per i generatori.
#[derive(Clone, Debug)]
pub(crate) struct Formula {
    pub m: Vec<M>,
    /// Sorgente Typst originale (senza `$`), usato come ripiego leggibile.
    pub sorgente: String,
}

impl Formula {
    pub fn ha_grezzo(&self) -> bool {
        contiene_grezzo(&self.m)
    }
}

// =======================================================================
// Tabella dei simboli
// =======================================================================

use Classe::{Apertura as AP, Binario as BI, Chiusura as CH, Grande as GR, Ordinario as OR};
use Classe::{Punteggiatura as PU, Relazione as RE};

/// (nome Typst, Unicode, LaTeX, classe)
const SIMBOLI: &[(&str, &str, &str, Classe)] = &[
    // --- lettere greche minuscole
    ("alpha", "α", "\\alpha", OR),
    ("beta", "β", "\\beta", OR),
    ("gamma", "γ", "\\gamma", OR),
    ("delta", "δ", "\\delta", OR),
    ("epsilon", "ϵ", "\\epsilon", OR),
    ("epsilon.alt", "ε", "\\varepsilon", OR),
    ("zeta", "ζ", "\\zeta", OR),
    ("eta", "η", "\\eta", OR),
    ("theta", "θ", "\\theta", OR),
    ("theta.alt", "ϑ", "\\vartheta", OR),
    ("iota", "ι", "\\iota", OR),
    ("kappa", "κ", "\\kappa", OR),
    ("lambda", "λ", "\\lambda", OR),
    ("mu", "μ", "\\mu", OR),
    ("nu", "ν", "\\nu", OR),
    ("xi", "ξ", "\\xi", OR),
    ("omicron", "ο", "o", OR),
    ("pi", "π", "\\pi", OR),
    ("pi.alt", "ϖ", "\\varpi", OR),
    ("rho", "ρ", "\\rho", OR),
    ("rho.alt", "ϱ", "\\varrho", OR),
    ("sigma", "σ", "\\sigma", OR),
    ("sigma.alt", "ς", "\\varsigma", OR),
    ("tau", "τ", "\\tau", OR),
    ("upsilon", "υ", "\\upsilon", OR),
    ("phi", "ϕ", "\\phi", OR),
    ("phi.alt", "φ", "\\varphi", OR),
    ("chi", "χ", "\\chi", OR),
    ("psi", "ψ", "\\psi", OR),
    ("omega", "ω", "\\omega", OR),
    // --- lettere greche maiuscole
    ("Alpha", "Α", "A", OR),
    ("Beta", "Β", "B", OR),
    ("Gamma", "Γ", "\\Gamma", OR),
    ("Delta", "Δ", "\\Delta", OR),
    ("Epsilon", "Ε", "E", OR),
    ("Zeta", "Ζ", "Z", OR),
    ("Eta", "Η", "H", OR),
    ("Theta", "Θ", "\\Theta", OR),
    ("Iota", "Ι", "I", OR),
    ("Kappa", "Κ", "K", OR),
    ("Lambda", "Λ", "\\Lambda", OR),
    ("Mu", "Μ", "M", OR),
    ("Nu", "Ν", "N", OR),
    ("Xi", "Ξ", "\\Xi", OR),
    ("Omicron", "Ο", "O", OR),
    ("Pi", "Π", "\\Pi", OR),
    ("Rho", "Ρ", "P", OR),
    ("Sigma", "Σ", "\\Sigma", OR),
    ("Tau", "Τ", "T", OR),
    ("Upsilon", "Υ", "\\Upsilon", OR),
    ("Phi", "Φ", "\\Phi", OR),
    ("Chi", "Χ", "X", OR),
    ("Psi", "Ψ", "\\Psi", OR),
    ("Omega", "Ω", "\\Omega", OR),
    // --- operatori binari
    ("plus", "+", "+", BI),
    ("minus", "−", "-", BI),
    ("plus.minus", "±", "\\pm", BI),
    ("minus.plus", "∓", "\\mp", BI),
    ("times", "×", "\\times", BI),
    ("times.o", "⊗", "\\otimes", BI),
    ("plus.o", "⊕", "\\oplus", BI),
    ("minus.o", "⊖", "\\ominus", BI),
    ("dot.c", "⋅", "\\cdot", BI),
    ("dot.op", "⋅", "\\cdot", BI),
    ("div", "÷", "\\div", BI),
    ("slash", "/", "/", BI),
    ("backslash", "\\", "\\backslash", BI),
    ("ast", "∗", "\\ast", BI),
    ("ast.op", "∗", "\\ast", BI),
    ("star.op", "⋆", "\\star", BI),
    ("compose", "∘", "\\circ", BI),
    ("circle.small", "∘", "\\circ", BI),
    ("convolve", "∗", "\\ast", BI),
    ("and", "∧", "\\wedge", BI),
    ("or", "∨", "\\vee", BI),
    ("union", "∪", "\\cup", BI),
    ("sect", "∩", "\\cap", BI),
    ("without", "∖", "\\setminus", BI),
    ("dagger", "†", "\\dagger", BI),
    // --- relazioni
    ("eq", "=", "=", RE),
    ("eq.not", "≠", "\\neq", RE),
    ("eq.triple", "≡", "\\equiv", RE),
    ("eq.def", "≝", "\\triangleq", RE),
    ("approx", "≈", "\\approx", RE),
    ("approx.eq", "≊", "\\approxeq", RE),
    ("tilde.op", "∼", "\\sim", RE),
    ("tilde.eq", "≃", "\\simeq", RE),
    ("tilde.equiv", "≅", "\\cong", RE),
    ("prop", "∝", "\\propto", RE),
    ("lt", "<", "<", RE),
    ("gt", ">", ">", RE),
    ("lt.eq", "≤", "\\leq", RE),
    ("gt.eq", "≥", "\\geq", RE),
    ("lt.double", "≪", "\\ll", RE),
    ("gt.double", "≫", "\\gg", RE),
    ("lt.eq.slant", "⩽", "\\leqslant", RE),
    ("gt.eq.slant", "⩾", "\\geqslant", RE),
    ("prec", "≺", "\\prec", RE),
    ("succ", "≻", "\\succ", RE),
    ("in", "∈", "\\in", RE),
    ("in.not", "∉", "\\notin", RE),
    ("in.rev", "∋", "\\ni", RE),
    ("subset", "⊂", "\\subset", RE),
    ("subset.eq", "⊆", "\\subseteq", RE),
    ("supset", "⊃", "\\supset", RE),
    ("supset.eq", "⊇", "\\supseteq", RE),
    ("perp", "⊥", "\\perp", RE),
    ("parallel", "∥", "\\parallel", RE),
    ("divides", "∣", "\\mid", RE),
    ("colon", ":", ":", RE),
    ("arrow.r", "→", "\\to", RE),
    ("arrow.l", "←", "\\leftarrow", RE),
    ("arrow.t", "↑", "\\uparrow", RE),
    ("arrow.b", "↓", "\\downarrow", RE),
    ("arrow.l.r", "↔", "\\leftrightarrow", RE),
    ("arrow.r.double", "⇒", "\\Rightarrow", RE),
    ("arrow.l.double", "⇐", "\\Leftarrow", RE),
    ("arrow.l.r.double", "⇔", "\\Leftrightarrow", RE),
    ("arrow.r.long", "⟶", "\\longrightarrow", RE),
    ("arrow.l.long", "⟵", "\\longleftarrow", RE),
    ("arrow.r.bar", "↦", "\\mapsto", RE),
    ("arrow.r.long.bar", "⟼", "\\longmapsto", RE),
    ("arrow.r.long.double", "⟹", "\\Longrightarrow", RE),
    ("arrow.l.long.double", "⟸", "\\Longleftarrow", RE),
    ("arrow.l.r.long.double", "⟺", "\\iff", RE),
    // --- ordinari / simboli
    ("oo", "∞", "\\infty", OR),
    ("infinity", "∞", "\\infty", OR),
    ("partial", "∂", "\\partial", OR),
    ("nabla", "∇", "\\nabla", OR),
    ("nothing", "∅", "\\emptyset", OR),
    ("emptyset", "∅", "\\emptyset", OR),
    ("forall", "∀", "\\forall", OR),
    ("exists", "∃", "\\exists", OR),
    ("exists.not", "∄", "\\nexists", OR),
    ("not", "¬", "\\neg", OR),
    ("therefore", "∴", "\\therefore", OR),
    ("because", "∵", "\\because", OR),
    ("ell", "ℓ", "\\ell", OR),
    ("planck.reduce", "ℏ", "\\hbar", OR),
    ("hbar", "ℏ", "\\hbar", OR),
    ("aleph", "ℵ", "\\aleph", OR),
    ("Re", "ℜ", "\\Re", OR),
    ("Im", "ℑ", "\\Im", OR),
    ("angle", "∠", "\\angle", OR),
    ("degree", "°", "^\\circ", OR),
    ("prime", "′", "'", OR),
    ("prime.double", "″", "''", OR),
    ("dots", "…", "\\ldots", OR),
    ("dots.h", "…", "\\ldots", OR),
    ("dots.h.c", "⋯", "\\cdots", OR),
    ("dots.v", "⋮", "\\vdots", OR),
    ("dots.down", "⋱", "\\ddots", OR),
    ("bar.v", "|", "\\vert", OR),
    ("bar.v.double", "‖", "\\Vert", OR),
    ("comma", ",", ",", PU),
    ("semi", ";", ";", PU),
    ("angle.l", "⟨", "\\langle", AP),
    ("angle.r", "⟩", "\\rangle", CH),
    ("floor.l", "⌊", "\\lfloor", AP),
    ("floor.r", "⌋", "\\rfloor", CH),
    ("ceil.l", "⌈", "\\lceil", AP),
    ("ceil.r", "⌉", "\\rceil", CH),
    ("paren.l", "(", "(", AP),
    ("paren.r", ")", ")", CH),
    ("bracket.l", "[", "[", AP),
    ("bracket.r", "]", "]", CH),
    ("brace.l", "{", "\\{", AP),
    ("brace.r", "}", "\\}", CH),
    // --- operatori grandi
    ("sum", "∑", "\\sum", GR),
    ("product", "∏", "\\prod", GR),
    ("prod", "∏", "\\prod", GR),
    ("coproduct", "∐", "\\coprod", GR),
    ("integral", "∫", "\\int", GR),
    ("integral.double", "∬", "\\iint", GR),
    ("integral.triple", "∭", "\\iiint", GR),
    ("integral.cont", "∮", "\\oint", GR),
    ("union.big", "⋃", "\\bigcup", GR),
    ("sect.big", "⋂", "\\bigcap", GR),
    ("plus.o.big", "⨁", "\\bigoplus", GR),
    ("times.o.big", "⨂", "\\bigotimes", GR),
];

/// Nomi di funzione che LaTeX conosce già come comando.
const FUNZIONI_LATEX: &[&str] = &[
    "arccos", "arcsin", "arctan", "arg", "cos", "cosh", "cot", "coth", "csc", "deg", "det", "dim",
    "exp", "gcd", "hom", "inf", "ker", "lg", "lim", "liminf", "limsup", "ln", "log", "max", "min",
    "Pr", "sec", "sin", "sinh", "sup", "tan", "tanh",
];

/// Funzioni operatore Typst non presenti in LaTeX (`\operatorname`).
const FUNZIONI_EXTRA: &[&str] = &[
    "arccot", "arcsec", "arccsc", "csch", "sech", "id", "im", "mod", "tg", "ctg",
];

/// Funzioni con limiti sopra/sotto.
const FUNZIONI_CON_LIMITI: &[&str] = &[
    "lim", "liminf", "limsup", "max", "min", "inf", "sup", "det", "gcd", "Pr",
];

fn cerca_nome(nome: &str) -> Option<&'static (&'static str, &'static str, &'static str, Classe)> {
    SIMBOLI.iter().find(|s| s.0 == nome)
}

fn cerca_carattere(c: &str) -> Option<&'static (&'static str, &'static str, &'static str, Classe)> {
    SIMBOLI.iter().find(|s| s.1 == c)
}

fn atomo_da_voce(v: &(&str, &str, &str, Classe)) -> Atomo {
    let (_, u, l, classe) = *v;
    let primo = u.chars().next().unwrap_or(' ');
    let tipo = match classe {
        Classe::Ordinario => {
            if primo.is_lowercase() && primo.is_alphabetic() {
                Tipo::Id
            } else {
                Tipo::IdDritto
            }
        }
        Classe::Funzione => Tipo::IdDritto,
        _ => Tipo::Op,
    };
    Atomo {
        u: u.to_string(),
        l: l.to_string(),
        tipo,
        classe,
        limiti: classe == Classe::Grande && matches!(u, "∑" | "∏" | "∐" | "⋃" | "⋂" | "⨁" | "⨂"),
    }
}

fn atomo_semplice(u: &str, l: &str, tipo: Tipo, classe: Classe) -> Atomo {
    Atomo {
        u: u.to_string(),
        l: l.to_string(),
        tipo,
        classe,
        limiti: false,
    }
}

/// Carattere Unicode di un simbolo Typst (`alpha`, `arrow.r`, …), per
/// `#sym.nome` nel markup.
pub(crate) fn simbolo_unicode(nome: &str) -> Option<String> {
    cerca_nome(nome).map(|v| v.1.to_string())
}

/// Carattere LaTeX sicuro in math mode.
fn latex_carattere(c: char) -> String {
    match c {
        '#' => "\\#".into(),
        '%' => "\\%".into(),
        '&' => "\\&".into(),
        '$' => "\\$".into(),
        '_' => "\\_".into(),
        '{' => "\\{".into(),
        '}' => "\\}".into(),
        '\\' => "\\backslash".into(),
        '^' => "\\hat{}".into(),
        '~' => "\\sim".into(),
        '|' => "\\vert".into(),
        other => other.to_string(),
    }
}

/// Atomo per un grafema di `MathText`.
fn atomo_da_testo(g: &str) -> M {
    let Some(c) = g.chars().next() else {
        return M::Riga(Vec::new());
    };
    if c.is_ascii_digit() {
        return M::A(atomo_semplice(g, g, Tipo::Num, Classe::Ordinario));
    }
    if c.is_alphabetic() {
        return M::A(atomo_semplice(g, g, Tipo::Id, Classe::Ordinario));
    }
    let a = match c {
        '+' => atomo_semplice("+", "+", Tipo::Op, Classe::Binario),
        '-' => atomo_semplice("−", "-", Tipo::Op, Classe::Binario),
        '*' => atomo_semplice("∗", "\\ast", Tipo::Op, Classe::Binario),
        '/' => atomo_semplice("/", "/", Tipo::Op, Classe::Binario),
        '=' | '<' | '>' => atomo_semplice(g, g, Tipo::Op, Classe::Relazione),
        ':' => atomo_semplice(":", ":", Tipo::Op, Classe::Relazione),
        '(' | '[' => atomo_semplice(g, g, Tipo::Op, Classe::Apertura),
        ')' | ']' => atomo_semplice(g, g, Tipo::Op, Classe::Chiusura),
        '{' => atomo_semplice("{", "\\{", Tipo::Op, Classe::Apertura),
        '}' => atomo_semplice("}", "\\}", Tipo::Op, Classe::Chiusura),
        ',' | ';' => atomo_semplice(g, g, Tipo::Op, Classe::Punteggiatura),
        '!' => atomo_semplice("!", "!", Tipo::Op, Classe::Ordinario),
        '.' => atomo_semplice(".", ".", Tipo::Op, Classe::Ordinario),
        _ => match cerca_carattere(g) {
            Some(v) => atomo_da_voce(v),
            None => atomo_semplice(g, &latex_carattere(c), Tipo::Op, Classe::Ordinario),
        },
    };
    M::A(a)
}

/// Atomo per un carattere (shorthand, escape).
fn atomo_da_char(c: char) -> M {
    if c == '\0' {
        return M::Riga(Vec::new());
    }
    atomo_da_testo(&c.to_string())
}

// =======================================================================
// Conversione AST → M
// =======================================================================

/// Convertitore: accumula gli avvisi (simboli non riconosciuti, …).
/// Argomenti di una chiamata math: posizionali (con fine-riga `;`) e
/// nominati (nome, valore testuale, valore convertito).
type ArgomentiMath = (Vec<(M, bool)>, Vec<(String, String, M)>);

pub(crate) struct ConvMath {
    pub avvisi: Avvisi,
    /// Annidamento corrente di `espr` (limitato da [`PROFONDITA_MATH_MAX`]).
    profondita: usize,
}

/// Annidamento massimo di una formula. Oltre, il sotto-albero è sostituito
/// dal suo sorgente (`M::Grezzo`) con un avviso: i generatori (`lin_m`, `ml`,
/// `tx`) ricorrono sulla profondità dell'albero `M`, che così resta limitata
/// e non può esaurire lo stack.
pub(crate) const PROFONDITA_MATH_MAX: usize = 32;

fn nome_accesso(a: ast::MathAccess) -> String {
    match a {
        ast::MathAccess::MathIdent(i) => i.get().to_string(),
        ast::MathAccess::MathFieldAccess(f) => {
            format!("{}.{}", nome_accesso(f.target()), f.field().get())
        }
    }
}

fn riga(mut v: Vec<M>) -> M {
    if v.len() == 1 {
        v.remove(0)
    } else {
        M::Riga(v)
    }
}

impl ConvMath {
    pub fn nuovo() -> Self {
        Self {
            avvisi: Avvisi::default(),
            profondita: 0,
        }
    }

    /// Converte una `Equation` completa.
    pub fn equazione(&mut self, eq: ast::Equation) -> Formula {
        let m = self.sequenza(eq.body());
        let sorgente = eq.body().to_untyped().full_text().trim().to_string();
        Formula { m, sorgente }
    }

    fn sequenza(&mut self, math: ast::Math) -> Vec<M> {
        let mut out = Vec::new();
        for e in math.exprs() {
            if matches!(e, ast::Expr::Space(_)) {
                continue;
            }
            out.push(self.espr(e));
        }
        out
    }

    fn avvisa(&mut self, msg: String) {
        self.avvisi.aggiungi(msg);
    }

    fn ident(&mut self, nome: &str) -> M {
        if let Some(v) = cerca_nome(nome) {
            return M::A(atomo_da_voce(v));
        }
        if FUNZIONI_LATEX.contains(&nome) || FUNZIONI_EXTRA.contains(&nome) {
            let l = if FUNZIONI_LATEX.contains(&nome) {
                format!("\\{nome}")
            } else {
                format!("\\operatorname{{{nome}}}")
            };
            let mut a = atomo_semplice(nome, &l, Tipo::IdDritto, Classe::Funzione);
            a.limiti = FUNZIONI_CON_LIMITI.contains(&nome);
            return M::A(a);
        }
        match nome {
            "dif" => {
                return M::A(atomo_semplice(
                    "d",
                    "\\mathrm{d}",
                    Tipo::IdDritto,
                    Classe::Ordinario,
                ))
            }
            "Dif" => {
                return M::A(atomo_semplice(
                    "D",
                    "\\mathrm{D}",
                    Tipo::IdDritto,
                    Classe::Ordinario,
                ))
            }
            "quad" => return M::Spazio(1.0),
            "wide" => return M::Spazio(2.0),
            "thin" => return M::Spazio(1.0 / 6.0),
            "med" => return M::Spazio(2.0 / 9.0),
            "thick" => return M::Spazio(5.0 / 18.0),
            "space" | "space.nobreak" => return M::Spazio(0.25),
            _ => {}
        }
        if nome.chars().count() == 1 {
            return atomo_da_testo(nome);
        }
        self.avvisa(format!(
            "math: simbolo «{nome}» non riconosciuto (reso come testo)"
        ));
        M::A(atomo_semplice(
            nome,
            &format!("\\mathrm{{{nome}}}"),
            Tipo::Testo,
            Classe::Ordinario,
        ))
    }

    /// Converte un'espressione math, limitando la profondità di annidamento.
    fn espr(&mut self, e: ast::Expr) -> M {
        if self.profondita >= PROFONDITA_MATH_MAX {
            self.avvisa(
                "math: formula annidata troppo in profondità: la parte più interna è riportata come sorgente"
                    .to_string(),
            );
            return M::Grezzo(e.to_untyped().full_text().to_string());
        }
        self.profondita += 1;
        let m = self.espr_interna(e);
        self.profondita -= 1;
        m
    }

    fn espr_interna(&mut self, e: ast::Expr) -> M {
        match e {
            ast::Expr::Math(m) => riga(self.sequenza(m)),
            ast::Expr::MathText(t) => match t.get() {
                ast::MathTextKind::Number(n) => {
                    M::A(atomo_semplice(n, n, Tipo::Num, Classe::Ordinario))
                }
                ast::MathTextKind::Grapheme(g) => atomo_da_testo(g),
            },
            ast::Expr::MathIdent(i) => self.ident(i.as_str()),
            ast::Expr::MathFieldAccess(f) => {
                let nome = nome_accesso(ast::MathAccess::MathFieldAccess(f));
                self.ident(&nome)
            }
            ast::Expr::MathShorthand(s) => atomo_da_char(s.get()),
            ast::Expr::Escape(s) => atomo_da_char(s.get()),
            ast::Expr::Shorthand(s) => atomo_da_char(s.get()),
            ast::Expr::MathAlignPoint(_) => M::Allinea,
            ast::Expr::Linebreak(_) => M::ACapo,
            ast::Expr::Space(_) => M::Riga(Vec::new()),
            ast::Expr::Str(s) => {
                let t = s.get().to_string();
                let l = format!("\\text{{{}}}", latex_testo(&t));
                M::A(atomo_semplice(&t, &l, Tipo::Testo, Classe::Ordinario))
            }
            ast::Expr::MathCall(c) => self.chiamata(c),
            ast::Expr::MathDelimited(d) => {
                let apri = self.testo_delim(d.open());
                let chiudi = self.testo_delim(d.close());
                let corpo = riga(self.sequenza(d.body()));
                M::Delim {
                    apri,
                    chiudi,
                    corpo: Box::new(corpo),
                }
            }
            ast::Expr::MathAttach(a) => {
                let base = self.espr(a.base());
                let pedice = a.bottom().map(|e| Box::new(self.espr(e)));
                let mut apice = a.top().map(|e| self.espr(e));
                if let Some(p) = a.primes() {
                    let primi = atomo_semplice(
                        &"′".repeat(p.count()),
                        &"'".repeat(p.count()),
                        Tipo::Op,
                        Classe::Ordinario,
                    );
                    apice = Some(match apice {
                        Some(a) => M::Riga(vec![M::A(primi), a]),
                        None => M::A(primi),
                    });
                }
                M::Att {
                    base: Box::new(base),
                    pedice,
                    apice: apice.map(Box::new),
                }
            }
            ast::Expr::MathPrimes(p) => M::A(atomo_semplice(
                &"′".repeat(p.count()),
                &"'".repeat(p.count()),
                Tipo::Op,
                Classe::Ordinario,
            )),
            ast::Expr::MathFrac(f) => {
                let n = self.espr(f.num());
                let d = self.espr(f.denom());
                M::Frac(Box::new(n), Box::new(d))
            }
            ast::Expr::MathRoot(r) => {
                let indice = r.index().map(|n| {
                    Box::new(M::A(atomo_semplice(
                        &n.to_string(),
                        &n.to_string(),
                        Tipo::Num,
                        Classe::Ordinario,
                    )))
                });
                let radicando = self.espr(r.radicand());
                M::Rad {
                    indice,
                    radicando: Box::new(radicando),
                }
            }
            altro => {
                let src = altro.to_untyped().full_text().to_string();
                self.avvisa(format!("math: costrutto non convertibile «{}»", src.trim()));
                M::Grezzo(src)
            }
        }
    }

    fn testo_delim(&mut self, e: ast::Expr) -> String {
        match self.espr(e) {
            M::A(a) => a.u,
            _ => String::new(),
        }
    }

    /// Estrae argomenti posizionali (con la fine-riga `;`) e nominati
    /// (nome, valore testuale, valore convertito).
    fn argomenti(&mut self, c: ast::MathCall) -> ArgomentiMath {
        let mut pos = Vec::new();
        let mut nom = Vec::new();
        for a in c.args().arg_items() {
            let fine_riga = a.ends_in_semicolon;
            match a.arg {
                ast::Arg::Pos(e) => pos.push((self.espr(e), fine_riga)),
                ast::Arg::Named(n) => {
                    let valore = match n.expr() {
                        ast::Expr::Str(s) => s.get().to_string(),
                        ast::Expr::Bool(b) => b.get().to_string(),
                        altro => altro.to_untyped().full_text().to_string(),
                    };
                    let m = self.espr(n.expr());
                    nom.push((n.name().get().to_string(), valore, m));
                }
                ast::Arg::Spread(s) => {
                    let src = s.to_untyped().full_text().to_string();
                    self.avvisa(format!("math: argomento spread «{src}» non convertibile"));
                    pos.push((M::Grezzo(src), fine_riga));
                }
            }
        }
        (pos, nom)
    }

    fn chiamata(&mut self, c: ast::MathCall) -> M {
        let nome = nome_accesso(c.callee());
        let sorgente = c.to_untyped().full_text().to_string();
        let (pos, nom) = self.argomenti(c);
        let arg = |i: usize| -> M {
            pos.get(i)
                .map(|(m, _)| m.clone())
                .unwrap_or_else(|| M::Riga(Vec::new()))
        };
        let delim_nominato = |def: (&str, &str)| -> (String, String) {
            match nom
                .iter()
                .find(|(k, _, _)| k == "delim")
                .map(|(_, v, _)| v.as_str())
            {
                Some("[") => ("[".into(), "]".into()),
                Some("(") => ("(".into(), ")".into()),
                Some("{") => ("{".into(), "}".into()),
                Some("|") => ("|".into(), "|".into()),
                Some("||") => ("‖".into(), "‖".into()),
                Some("none") | Some("") => (String::new(), String::new()),
                _ => (def.0.into(), def.1.into()),
            }
        };
        let accento = |a: Accento, arg0: M| M::Accento(a, Box::new(arg0));
        let stile = |s: Stile, arg0: M| M::Stile(s, Box::new(arg0));
        match nome.as_str() {
            "frac" => M::Frac(Box::new(arg(0)), Box::new(arg(1))),
            "binom" => M::Binom(Box::new(arg(0)), Box::new(arg(1))),
            "sqrt" => M::Rad {
                indice: None,
                radicando: Box::new(arg(0)),
            },
            "root" => M::Rad {
                indice: Some(Box::new(arg(0))),
                radicando: Box::new(arg(1)),
            },
            "abs" => M::Delim {
                apri: "|".into(),
                chiudi: "|".into(),
                corpo: Box::new(arg(0)),
            },
            "norm" => M::Delim {
                apri: "‖".into(),
                chiudi: "‖".into(),
                corpo: Box::new(arg(0)),
            },
            "floor" => M::Delim {
                apri: "⌊".into(),
                chiudi: "⌋".into(),
                corpo: Box::new(arg(0)),
            },
            "ceil" => M::Delim {
                apri: "⌈".into(),
                chiudi: "⌉".into(),
                corpo: Box::new(arg(0)),
            },
            "round" => M::Delim {
                apri: "⌊".into(),
                chiudi: "⌉".into(),
                corpo: Box::new(arg(0)),
            },
            "lr" | "display" | "inline" | "script" | "sscript" | "limits" | "scripts" | "mid" => {
                riga(pos.iter().map(|(m, _)| m.clone()).collect())
            }
            "class" => arg(1),
            "bold" => stile(Stile::Grassetto, arg(0)),
            "italic" => stile(Stile::Corsivo, arg(0)),
            "upright" | "serif" | "sans" => stile(Stile::Dritto, arg(0)),
            "cal" | "scr" => stile(Stile::Calligrafico, arg(0)),
            "bb" => stile(Stile::Lavagna, arg(0)),
            "frak" => stile(Stile::Gotico, arg(0)),
            "mono" => stile(Stile::Monospazio, arg(0)),
            "hat" | "circumflex" => accento(Accento::Cappello, arg(0)),
            "tilde" => accento(Accento::Tilde, arg(0)),
            "macron" => accento(Accento::Barra, arg(0)),
            "dot" => accento(Accento::Punto, arg(0)),
            "dot.double" | "diaer" => accento(Accento::DuePunti, arg(0)),
            "arrow" => accento(Accento::Freccia, arg(0)),
            "breve" => accento(Accento::Breve, arg(0)),
            "caron" => accento(Accento::Caron, arg(0)),
            "overline" => accento(Accento::SopraLinea, arg(0)),
            "underline" => accento(Accento::SottoLinea, arg(0)),
            "op" => {
                let t = match arg(0) {
                    M::A(a) => a.u,
                    _ => String::new(),
                };
                let limiti = nom.iter().any(|(k, v, _)| k == "limits" && v == "true");
                let mut a = atomo_semplice(
                    &t,
                    &format!("\\operatorname{{{}}}", latex_testo(&t)),
                    Tipo::IdDritto,
                    Classe::Funzione,
                );
                a.limiti = limiti;
                M::A(a)
            }
            "text" => arg(0),
            "vec" => {
                let (apri, chiudi) = delim_nominato(("(", ")"));
                M::Matrice {
                    righe: pos.iter().map(|(m, _)| vec![m.clone()]).collect(),
                    apri,
                    chiudi,
                }
            }
            "mat" => {
                let (apri, chiudi) = delim_nominato(("(", ")"));
                let mut righe: Vec<Vec<M>> = vec![Vec::new()];
                for (m, fine) in &pos {
                    if let Some(ultima) = righe.last_mut() {
                        ultima.push(m.clone());
                    }
                    if *fine {
                        righe.push(Vec::new());
                    }
                }
                if righe.last().is_some_and(Vec::is_empty) {
                    righe.pop();
                }
                M::Matrice {
                    righe,
                    apri,
                    chiudi,
                }
            }
            "cases" => M::Casi(pos.iter().map(|(m, _)| m.clone()).collect()),
            "attach" => {
                let get = |k: &str| -> Option<Box<M>> {
                    nom.iter()
                        .find(|(n, _, _)| n == k)
                        .map(|(_, _, m)| Box::new(m.clone()))
                };
                M::Att {
                    base: Box::new(arg(0)),
                    pedice: get("b"),
                    apice: get("t"),
                }
            }
            "cancel" => {
                self.avvisa("math: `cancel` non rappresentato (barratura ignorata)".into());
                arg(0)
            }
            _ => {
                self.avvisa(format!("math: funzione «{nome}» non convertibile"));
                M::Grezzo(sorgente)
            }
        }
    }
}

/// `true` se l'albero contiene nodi non convertibili.
pub(crate) fn contiene_grezzo(seq: &[M]) -> bool {
    seq.iter().any(grezzo_in)
}

fn grezzo_in(m: &M) -> bool {
    match m {
        M::Grezzo(_) => true,
        M::A(_) | M::Spazio(_) | M::Allinea | M::ACapo => false,
        M::Riga(v) => contiene_grezzo(v),
        M::Frac(a, b) | M::Binom(a, b) => grezzo_in(a) || grezzo_in(b),
        M::Att {
            base,
            pedice,
            apice,
        } => {
            grezzo_in(base)
                || pedice.as_deref().is_some_and(grezzo_in)
                || apice.as_deref().is_some_and(grezzo_in)
        }
        M::Rad { indice, radicando } => {
            indice.as_deref().is_some_and(grezzo_in) || grezzo_in(radicando)
        }
        M::Delim { corpo, .. } => grezzo_in(corpo),
        M::Stile(_, m) | M::Accento(_, m) => grezzo_in(m),
        M::Matrice { righe, .. } => righe.iter().any(|r| contiene_grezzo(r)),
        M::Casi(v) => contiene_grezzo(v),
    }
}

/// Se la sequenza usa `&` o `\`, la divide in righe → celle → elementi.
pub(crate) fn righe_allineate(seq: &[M]) -> Option<Vec<Vec<Vec<M>>>> {
    if !seq.iter().any(|m| matches!(m, M::Allinea | M::ACapo)) {
        return None;
    }
    let mut righe: Vec<Vec<Vec<M>>> = vec![vec![Vec::new()]];
    for m in seq {
        match m {
            M::ACapo => righe.push(vec![Vec::new()]),
            M::Allinea => {
                if let Some(r) = righe.last_mut() {
                    r.push(Vec::new());
                }
            }
            altro => {
                if let Some(cella) = righe.last_mut().and_then(|r| r.last_mut()) {
                    cella.push(altro.clone());
                }
            }
        }
    }
    // Una riga finale vuota (a capo in coda) non deve produrre una riga.
    if righe.last().is_some_and(|r| r.iter().all(Vec::is_empty)) && righe.len() > 1 {
        righe.pop();
    }
    Some(righe)
}

// =======================================================================
// Utilità di testo
// =======================================================================

/// Escape LaTeX per testo (usato in `\text{}`/`\operatorname{}`).
pub(crate) fn latex_testo(t: &str) -> String {
    let mut o = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '\\' => o.push_str("\\textbackslash{}"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '#' | '$' | '%' | '&' | '_' => {
                o.push('\\');
                o.push(c);
            }
            '~' => o.push_str("\\textasciitilde{}"),
            '^' => o.push_str("\\textasciicircum{}"),
            c => o.push(c),
        }
    }
    o
}

fn esc_xml(t: &str, out: &mut String) {
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

/// Mappa una lettera/cifra ASCII nello stile Unicode richiesto.
fn carattere_stile(stile: Stile, c: char) -> char {
    let (maiu, minu, cifre, ecc): (u32, u32, Option<u32>, &[(char, char)]) = match stile {
        Stile::Grassetto => (0x1D400, 0x1D41A, Some(0x1D7CE), &[]),
        Stile::Corsivo => (0x1D434, 0x1D44E, None, &[('h', '\u{210E}')]),
        Stile::Calligrafico => (
            0x1D49C,
            0x1D4B6,
            None,
            &[
                ('B', '\u{212C}'),
                ('E', '\u{2130}'),
                ('F', '\u{2131}'),
                ('H', '\u{210B}'),
                ('I', '\u{2110}'),
                ('L', '\u{2112}'),
                ('M', '\u{2133}'),
                ('R', '\u{211B}'),
                ('e', '\u{212F}'),
                ('g', '\u{210A}'),
                ('o', '\u{2134}'),
            ],
        ),
        Stile::Lavagna => (
            0x1D538,
            0x1D552,
            Some(0x1D7D8),
            &[
                ('C', '\u{2102}'),
                ('H', '\u{210D}'),
                ('N', '\u{2115}'),
                ('P', '\u{2119}'),
                ('Q', '\u{211A}'),
                ('R', '\u{211D}'),
                ('Z', '\u{2124}'),
            ],
        ),
        Stile::Gotico => (
            0x1D504,
            0x1D51E,
            None,
            &[
                ('C', '\u{212D}'),
                ('H', '\u{210C}'),
                ('I', '\u{2111}'),
                ('R', '\u{211C}'),
                ('Z', '\u{2128}'),
            ],
        ),
        Stile::Monospazio => (0x1D670, 0x1D68A, Some(0x1D7F6), &[]),
        Stile::Dritto => return c,
    };
    if let Some((_, r)) = ecc.iter().find(|(k, _)| *k == c) {
        return *r;
    }
    let base = if c.is_ascii_uppercase() {
        maiu + (c as u32 - 'A' as u32)
    } else if c.is_ascii_lowercase() {
        minu + (c as u32 - 'a' as u32)
    } else if c.is_ascii_digit() {
        match cifre {
            Some(b) => b + (c as u32 - '0' as u32),
            None => return c,
        }
    } else {
        return c;
    };
    char::from_u32(base).unwrap_or(c)
}

fn stile_su_stringa(stile: Stile, s: &str) -> String {
    s.chars().map(|c| carattere_stile(stile, c)).collect()
}

/// Riscrive gli atomi di `m` con lo stile Unicode (per MathML e forma lineare).
fn applica_stile(stile: Stile, m: &M) -> M {
    match m {
        M::A(a) if matches!(a.tipo, Tipo::Id | Tipo::IdDritto | Tipo::Num) => {
            let mut n = a.clone();
            n.u = stile_su_stringa(stile, &a.u);
            n.tipo = if a.tipo == Tipo::Num {
                Tipo::Num
            } else {
                Tipo::IdDritto
            };
            M::A(n)
        }
        M::Riga(v) => M::Riga(v.iter().map(|x| applica_stile(stile, x)).collect()),
        M::Att {
            base,
            pedice,
            apice,
        } => M::Att {
            base: Box::new(applica_stile(stile, base)),
            pedice: pedice.as_ref().map(|p| Box::new(applica_stile(stile, p))),
            apice: apice.as_ref().map(|p| Box::new(applica_stile(stile, p))),
        },
        M::Stile(s, inner) => M::Stile(*s, Box::new(applica_stile(stile, inner))),
        altro => altro.clone(),
    }
}

/// Converte `s` in apice Unicode se ogni carattere è rappresentabile.
pub(crate) fn a_apice(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    s.chars().map(apice_unicode).collect()
}

/// Converte `s` in pedice Unicode se ogni carattere è rappresentabile.
pub(crate) fn a_pedice(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    s.chars().map(pedice_unicode).collect()
}

fn apice_unicode(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

fn pedice_unicode(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' | '−' => '₋',
        '(' => '₍',
        ')' => '₎',
        _ => return None,
    })
}

// =======================================================================
// Forma lineare (testo Unicode)
// =======================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum K {
    Parola,
    Bin,
    Rel,
    Punt,
    Apre,
    Chiude,
    Grande,
    Fun,
    Gruppo,
    Spazio,
}

/// Forma lineare leggibile della formula (una riga per `\`).
pub(crate) fn lineare(seq: &[M]) -> String {
    if let Some(righe) = righe_allineate(seq) {
        return righe
            .iter()
            .map(|r| {
                r.iter()
                    .map(|cella| lin_seq(cella))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    lin_seq(seq)
}

fn kind_di(m: &M) -> K {
    match m {
        M::A(a) => match a.classe {
            Classe::Binario => K::Bin,
            Classe::Relazione => K::Rel,
            Classe::Punteggiatura => K::Punt,
            Classe::Apertura => K::Apre,
            Classe::Chiusura => K::Chiude,
            Classe::Grande => K::Grande,
            Classe::Funzione => K::Fun,
            Classe::Ordinario => K::Parola,
        },
        M::Att { base, .. } => kind_di(base),
        M::Stile(_, i) | M::Accento(_, i) => kind_di(i),
        M::Delim { .. } => K::Gruppo,
        M::Spazio(_) => K::Spazio,
        _ => K::Parola,
    }
}

fn lin_seq(seq: &[M]) -> String {
    let mut out = String::new();
    let mut prec: Option<K> = None;
    let mut segno = false; // l'ultimo elemento era un segno unario
    for m in seq {
        if matches!(m, M::Allinea) {
            continue;
        }
        let k = kind_di(m);
        let testo = lin_m(m);
        if testo.is_empty() {
            continue;
        }
        let unario =
            k == K::Bin && matches!(prec, None | Some(K::Apre | K::Bin | K::Rel | K::Punt));
        let spazio = match prec {
            None => false,
            Some(_) if segno => false,
            Some(p) => {
                if k == K::Punt {
                    false
                } else if p == K::Punt {
                    true
                } else if p == K::Apre || k == K::Chiude || p == K::Spazio || k == K::Spazio {
                    false
                } else if k == K::Bin || k == K::Rel || p == K::Bin || p == K::Rel {
                    true
                } else if k == K::Apre && matches!(p, K::Parola | K::Chiude | K::Fun) {
                    false
                } else {
                    // f(x), sin(x): nessuno spazio prima di un gruppo
                    !(k == K::Gruppo && matches!(p, K::Parola | K::Chiude | K::Gruppo | K::Fun))
                }
            }
        };
        if spazio {
            out.push(' ');
        }
        out.push_str(&testo);
        segno = unario;
        prec = Some(k);
    }
    out
}

fn semplice(t: &str) -> bool {
    !t.is_empty()
        && t.chars().all(|c| {
            c.is_alphanumeric() || matches!(c, '.' | '′' | '″' | '_' | '∞' | '⁻' | '²' | '³')
        })
}

fn racchiudi(t: String) -> String {
    if semplice(&t) {
        t
    } else {
        format!("({t})")
    }
}

fn lin_m(m: &M) -> String {
    match m {
        M::A(a) => a.u.clone(),
        M::Riga(v) => lin_seq(v),
        M::Frac(n, d) => format!("{}/{}", racchiudi(lin_m(n)), racchiudi(lin_m(d))),
        M::Binom(n, k) => format!("C({}, {})", lin_m(n), lin_m(k)),
        M::Att {
            base,
            pedice,
            apice,
        } => {
            let mut s = lin_m(base);
            if let Some(p) = pedice {
                let t = lin_m(p);
                let conv: Option<String> = t.chars().map(pedice_unicode).collect();
                match conv {
                    Some(c) if !t.is_empty() => s.push_str(&c),
                    _ if semplice(&t) => {
                        s.push('_');
                        s.push_str(&t);
                    }
                    _ => {
                        s.push_str("_(");
                        s.push_str(&t);
                        s.push(')');
                    }
                }
            }
            if let Some(a) = apice {
                let t = lin_m(a);
                let conv: Option<String> = t.chars().map(apice_unicode).collect();
                if !t.is_empty() && t.chars().all(|c| c == '′' || c == '″') {
                    s.push_str(&t);
                } else {
                    match conv {
                        Some(c) if !t.is_empty() => s.push_str(&c),
                        _ if semplice(&t) => {
                            s.push('^');
                            s.push_str(&t);
                        }
                        _ => {
                            s.push_str("^(");
                            s.push_str(&t);
                            s.push(')');
                        }
                    }
                }
            }
            s
        }
        M::Rad { indice, radicando } => {
            let r = lin_m(radicando);
            let r = racchiudi(r);
            match indice.as_deref() {
                None => format!("√{r}"),
                Some(M::A(a)) if a.u == "3" => format!("∛{r}"),
                Some(M::A(a)) if a.u == "4" => format!("∜{r}"),
                Some(i) => {
                    let t = lin_m(i);
                    let conv: Option<String> = t.chars().map(apice_unicode).collect();
                    match conv {
                        Some(c) => format!("{c}√{r}"),
                        None => format!("({t})√{r}"),
                    }
                }
            }
        }
        M::Delim {
            apri,
            chiudi,
            corpo,
        } => format!("{apri}{}{chiudi}", lin_m(corpo)),
        M::Stile(s, i) => lin_m(&applica_stile(*s, i)),
        M::Accento(a, i) => {
            let base = lin_m(i);
            let comb = match a {
                Accento::Cappello => '\u{0302}',
                Accento::Tilde => '\u{0303}',
                Accento::Barra => '\u{0304}',
                Accento::SopraLinea => '\u{0305}',
                Accento::Breve => '\u{0306}',
                Accento::Punto => '\u{0307}',
                Accento::DuePunti => '\u{0308}',
                Accento::Caron => '\u{030C}',
                Accento::Freccia => '\u{20D7}',
                Accento::SottoLinea => '\u{0332}',
            };
            if base.chars().count() == 1 {
                format!("{base}{comb}")
            } else {
                // più caratteri: il segno combinante si applica a tutti
                base.chars().flat_map(|c| [c, comb]).collect()
            }
        }
        M::Matrice {
            righe,
            apri,
            chiudi,
        } => {
            let r: Vec<String> = righe
                .iter()
                .map(|riga| riga.iter().map(lin_m).collect::<Vec<_>>().join(", "))
                .collect();
            format!("{apri}{}{chiudi}", r.join("; "))
        }
        M::Casi(v) => {
            let r: Vec<String> = v.iter().map(lin_m).collect();
            format!("{{ {} }}", r.join("; "))
        }
        M::Spazio(_) => " ".into(),
        M::Allinea => String::new(),
        M::ACapo => "\n".into(),
        M::Grezzo(s) => s.clone(),
    }
}

// =======================================================================
// MathML
// =======================================================================

/// Genera l'elemento `<math>` MathML (senza ripiego: i chiamanti controllano
/// prima [`contiene_grezzo`]).
pub(crate) fn mathml(seq: &[M], display: bool, alt: &str) -> String {
    let mut o = String::new();
    o.push_str("<math xmlns=\"http://www.w3.org/1998/Math/MathML\"");
    if display {
        o.push_str(" display=\"block\"");
    }
    o.push_str(" alttext=\"");
    esc_xml(alt, &mut o);
    o.push_str("\">");
    if let Some(righe) = righe_allineate(seq) {
        o.push_str("<mtable>");
        for r in &righe {
            o.push_str("<mtr>");
            for cella in r {
                o.push_str("<mtd>");
                ml_seq(cella, &mut o);
                o.push_str("</mtd>");
            }
            o.push_str("</mtr>");
        }
        o.push_str("</mtable>");
    } else {
        o.push_str("<mrow>");
        ml_seq(seq, &mut o);
        o.push_str("</mrow>");
    }
    o.push_str("</math>");
    o
}

fn ml_seq(seq: &[M], o: &mut String) {
    for m in seq {
        ml(m, o);
    }
}

fn ml_riga(m: &M, o: &mut String) {
    // racchiude in <mrow> (necessario per argomenti multi-elemento)
    match m {
        M::Riga(v) if v.len() != 1 => {
            o.push_str("<mrow>");
            ml_seq(v, o);
            o.push_str("</mrow>");
        }
        altro => ml(altro, o),
    }
}

fn ml_atomo(a: &Atomo, o: &mut String) {
    let (tag, attr) = match a.tipo {
        Tipo::Num => ("mn", ""),
        Tipo::Id => ("mi", ""),
        Tipo::IdDritto => ("mi", " mathvariant=\"normal\""),
        Tipo::Op => ("mo", ""),
        Tipo::Testo => ("mtext", ""),
    };
    o.push('<');
    o.push_str(tag);
    o.push_str(attr);
    if tag == "mo" {
        match a.classe {
            Classe::Apertura | Classe::Chiusura => o.push_str(" fence=\"true\" stretchy=\"false\""),
            Classe::Grande if a.limiti => o.push_str(" movablelimits=\"true\""),
            _ => {}
        }
    }
    o.push('>');
    esc_xml(&a.u, o);
    o.push_str("</");
    o.push_str(tag);
    o.push('>');
}

fn ml(m: &M, o: &mut String) {
    match m {
        M::A(a) => ml_atomo(a, o),
        M::Riga(v) => {
            if v.len() == 1 {
                ml(&v[0], o);
            } else {
                o.push_str("<mrow>");
                ml_seq(v, o);
                o.push_str("</mrow>");
            }
        }
        M::Frac(n, d) => {
            o.push_str("<mfrac>");
            ml_riga(n, o);
            ml_riga(d, o);
            o.push_str("</mfrac>");
        }
        M::Binom(n, k) => {
            o.push_str("<mrow><mo fence=\"true\">(</mo><mfrac linethickness=\"0\">");
            ml_riga(n, o);
            ml_riga(k, o);
            o.push_str("</mfrac><mo fence=\"true\">)</mo></mrow>");
        }
        M::Att {
            base,
            pedice,
            apice,
        } => {
            let sopra_sotto = matches!(&**base, M::A(a) if a.limiti);
            let (aperto, chiuso) = match (pedice.is_some(), apice.is_some(), sopra_sotto) {
                (true, true, true) => ("<munderover>", "</munderover>"),
                (true, false, true) => ("<munder>", "</munder>"),
                (false, true, true) => ("<mover>", "</mover>"),
                (true, true, false) => ("<msubsup>", "</msubsup>"),
                (true, false, false) => ("<msub>", "</msub>"),
                (false, true, false) => ("<msup>", "</msup>"),
                (false, false, _) => ("", ""),
            };
            o.push_str(aperto);
            ml_riga(base, o);
            if let Some(p) = pedice {
                ml_riga(p, o);
            }
            if let Some(a) = apice {
                ml_riga(a, o);
            }
            o.push_str(chiuso);
        }
        M::Rad { indice, radicando } => match indice {
            None => {
                o.push_str("<msqrt>");
                ml_riga(radicando, o);
                o.push_str("</msqrt>");
            }
            Some(i) => {
                o.push_str("<mroot>");
                ml_riga(radicando, o);
                ml_riga(i, o);
                o.push_str("</mroot>");
            }
        },
        M::Delim {
            apri,
            chiudi,
            corpo,
        } => {
            o.push_str("<mrow>");
            if !apri.is_empty() {
                o.push_str("<mo fence=\"true\" stretchy=\"true\">");
                esc_xml(apri, o);
                o.push_str("</mo>");
            }
            ml_riga(corpo, o);
            if !chiudi.is_empty() {
                o.push_str("<mo fence=\"true\" stretchy=\"true\">");
                esc_xml(chiudi, o);
                o.push_str("</mo>");
            }
            o.push_str("</mrow>");
        }
        M::Stile(s, i) => ml(&applica_stile(*s, i), o),
        M::Accento(a, i) => {
            let (tag, car) = match a {
                Accento::Cappello => ("mover", "^"),
                Accento::Tilde => ("mover", "~"),
                Accento::Barra => ("mover", "¯"),
                Accento::SopraLinea => ("mover", "‾"),
                Accento::Breve => ("mover", "˘"),
                Accento::Punto => ("mover", "˙"),
                Accento::DuePunti => ("mover", "¨"),
                Accento::Caron => ("mover", "ˇ"),
                Accento::Freccia => ("mover", "→"),
                Accento::SottoLinea => ("munder", "_"),
            };
            o.push('<');
            o.push_str(tag);
            o.push_str(" accent=\"true\">");
            ml_riga(i, o);
            o.push_str("<mo>");
            esc_xml(car, o);
            o.push_str("</mo></");
            o.push_str(tag);
            o.push('>');
        }
        M::Matrice {
            righe,
            apri,
            chiudi,
        } => {
            o.push_str("<mrow>");
            if !apri.is_empty() {
                o.push_str("<mo fence=\"true\" stretchy=\"true\">");
                esc_xml(apri, o);
                o.push_str("</mo>");
            }
            o.push_str("<mtable>");
            for r in righe {
                o.push_str("<mtr>");
                for c in r {
                    o.push_str("<mtd>");
                    ml_riga(c, o);
                    o.push_str("</mtd>");
                }
                o.push_str("</mtr>");
            }
            o.push_str("</mtable>");
            if !chiudi.is_empty() {
                o.push_str("<mo fence=\"true\" stretchy=\"true\">");
                esc_xml(chiudi, o);
                o.push_str("</mo>");
            }
            o.push_str("</mrow>");
        }
        M::Casi(v) => {
            o.push_str(
                "<mrow><mo fence=\"true\" stretchy=\"true\">{</mo><mtable columnalign=\"left\">",
            );
            for r in v {
                o.push_str("<mtr><mtd>");
                ml_riga(r, o);
                o.push_str("</mtd></mtr>");
            }
            o.push_str("</mtable></mrow>");
        }
        M::Spazio(w) => {
            o.push_str(&format!("<mspace width=\"{:.2}em\"></mspace>", w));
        }
        M::Allinea | M::ACapo => {}
        M::Grezzo(s) => {
            o.push_str("<mtext>");
            esc_xml(s, o);
            o.push_str("</mtext>");
        }
    }
}

// =======================================================================
// LaTeX
// =======================================================================

/// Genera il codice LaTeX (modalità matematica) della formula. Con `&`/`\`
/// restituisce il corpo per un ambiente `align`.
pub(crate) fn latex(seq: &[M]) -> String {
    if let Some(righe) = righe_allineate(seq) {
        return righe
            .iter()
            .map(|r| {
                r.iter()
                    .map(|cella| tx_seq(cella))
                    .collect::<Vec<_>>()
                    .join(" & ")
            })
            .collect::<Vec<_>>()
            .join(" \\\\\n");
    }
    tx_seq(seq)
}

/// `true` se la formula richiede un ambiente di allineamento (`align`).
pub(crate) fn richiede_allineamento(seq: &[M]) -> bool {
    seq.iter().any(|m| matches!(m, M::Allinea | M::ACapo))
}

/// Copia della sequenza **senza** punti di allineamento (`&`) né a capo
/// (`\`), a qualunque profondità: i primi spariscono, i secondi diventano uno
/// spazio. Serve alla math *in linea*, dove `&` e `\\` fuori da un ambiente
/// di allineamento sono errori LaTeX («Misplaced alignment tab»).
pub(crate) fn senza_allineamenti(seq: &[M]) -> Vec<M> {
    seq.iter()
        .filter(|m| !matches!(m, M::Allinea))
        .map(ripulisci_allineamenti)
        .collect()
}

fn ripulisci_allineamenti(m: &M) -> M {
    let b = |x: &M| Box::new(ripulisci_allineamenti(x));
    let ob = |x: &Option<Box<M>>| x.as_deref().map(|y| Box::new(ripulisci_allineamenti(y)));
    match m {
        M::Allinea => M::Riga(Vec::new()),
        M::ACapo => M::Spazio(1.0),
        M::A(_) | M::Spazio(_) | M::Grezzo(_) => m.clone(),
        M::Riga(v) => M::Riga(senza_allineamenti(v)),
        M::Frac(x, y) => M::Frac(b(x), b(y)),
        M::Binom(x, y) => M::Binom(b(x), b(y)),
        M::Att {
            base,
            pedice,
            apice,
        } => M::Att {
            base: b(base),
            pedice: ob(pedice),
            apice: ob(apice),
        },
        M::Rad { indice, radicando } => M::Rad {
            indice: ob(indice),
            radicando: b(radicando),
        },
        M::Delim {
            apri,
            chiudi,
            corpo,
        } => M::Delim {
            apri: apri.clone(),
            chiudi: chiudi.clone(),
            corpo: b(corpo),
        },
        M::Stile(s, x) => M::Stile(*s, b(x)),
        M::Accento(a, x) => M::Accento(*a, b(x)),
        M::Matrice {
            righe,
            apri,
            chiudi,
        } => M::Matrice {
            righe: righe.iter().map(|r| senza_allineamenti(r)).collect(),
            apri: apri.clone(),
            chiudi: chiudi.clone(),
        },
        M::Casi(v) => M::Casi(senza_allineamenti(v)),
    }
}

fn tx_seq(seq: &[M]) -> String {
    seq.iter()
        .map(tx)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn tx_gruppo(m: &M) -> String {
    format!("{{{}}}", tx(m))
}

fn tx_delim(d: &str) -> String {
    match d {
        "" => ".".into(),
        "{" => "\\{".into(),
        "}" => "\\}".into(),
        "|" => "|".into(),
        "‖" => "\\|".into(),
        "⌊" => "\\lfloor".into(),
        "⌋" => "\\rfloor".into(),
        "⌈" => "\\lceil".into(),
        "⌉" => "\\rceil".into(),
        "⟨" => "\\langle".into(),
        "⟩" => "\\rangle".into(),
        altro => altro.to_string(),
    }
}

fn tx(m: &M) -> String {
    match m {
        M::A(a) => a.l.clone(),
        M::Riga(v) => tx_seq(v),
        M::Frac(n, d) => format!("\\frac{}{}", tx_gruppo(n), tx_gruppo(d)),
        M::Binom(n, k) => format!("\\binom{}{}", tx_gruppo(n), tx_gruppo(k)),
        M::Att {
            base,
            pedice,
            apice,
        } => {
            let b = match &**base {
                M::A(_) | M::Delim { .. } => tx(base),
                altro => tx_gruppo(altro),
            };
            let mut s = b;
            if let Some(p) = pedice {
                s.push_str(&format!("_{}", tx_gruppo(p)));
            }
            if let Some(a) = apice {
                s.push_str(&format!("^{}", tx_gruppo(a)));
            }
            s
        }
        M::Rad { indice, radicando } => match indice {
            None => format!("\\sqrt{}", tx_gruppo(radicando)),
            Some(i) => format!("\\sqrt[{}]{}", tx(i), tx_gruppo(radicando)),
        },
        M::Delim {
            apri,
            chiudi,
            corpo,
        } => format!(
            "\\left{} {} \\right{}",
            tx_delim(apri),
            tx(corpo),
            tx_delim(chiudi)
        ),
        M::Stile(s, i) => {
            let cmd = match s {
                Stile::Grassetto => "\\mathbf",
                Stile::Corsivo => "\\mathit",
                Stile::Dritto => "\\mathrm",
                Stile::Calligrafico => "\\mathcal",
                Stile::Lavagna => "\\mathbb",
                Stile::Gotico => "\\mathfrak",
                Stile::Monospazio => "\\mathtt",
            };
            format!("{cmd}{}", tx_gruppo(i))
        }
        M::Accento(a, i) => {
            let cmd = match a {
                Accento::Cappello => "\\hat",
                Accento::Tilde => "\\tilde",
                Accento::Barra => "\\bar",
                Accento::Punto => "\\dot",
                Accento::DuePunti => "\\ddot",
                Accento::Freccia => "\\vec",
                Accento::Breve => "\\breve",
                Accento::Caron => "\\check",
                Accento::SopraLinea => "\\overline",
                Accento::SottoLinea => "\\underline",
            };
            format!("{cmd}{}", tx_gruppo(i))
        }
        M::Matrice {
            righe,
            apri,
            chiudi,
        } => {
            let env = match (apri.as_str(), chiudi.as_str()) {
                ("(", ")") => "pmatrix",
                ("[", "]") => "bmatrix",
                ("{", "}") => "Bmatrix",
                ("|", "|") => "vmatrix",
                ("‖", "‖") => "Vmatrix",
                _ => "matrix",
            };
            let corpo: Vec<String> = righe
                .iter()
                .map(|r| r.iter().map(tx).collect::<Vec<_>>().join(" & "))
                .collect();
            format!("\\begin{{{env}}} {} \\end{{{env}}}", corpo.join(" \\\\ "))
        }
        M::Casi(v) => {
            let corpo: Vec<String> = v.iter().map(tx).collect();
            format!("\\begin{{cases}} {} \\end{{cases}}", corpo.join(" \\\\ "))
        }
        M::Spazio(w) => {
            if *w >= 1.5 {
                "\\qquad".into()
            } else if *w >= 0.9 {
                "\\quad".into()
            } else if *w >= 0.3 {
                "\\;".into()
            } else if *w >= 0.2 {
                "\\:".into()
            } else {
                "\\,".into()
            }
        }
        M::Allinea => "&".into(),
        M::ACapo => "\\\\".into(),
        M::Grezzo(s) => format!("\\text{{{}}}", latex_testo(s)),
    }
}

// =======================================================================
// Test
// =======================================================================

#[cfg(test)]
mod test {
    use super::*;

    /// Converte la prima equazione del sorgente.
    fn formula(src: &str) -> (Formula, Vec<String>) {
        let radice = typst_syntax::parse(src);
        let mut trovata = None;
        radice.children().for_each(|n| {
            if let Some(eq) = n.cast::<ast::Equation>() {
                if trovata.is_none() {
                    trovata = Some(eq);
                }
            }
        });
        let mut c = ConvMath::nuovo();
        let f = c.equazione(trovata.expect("equazione"));
        (f, c.avvisi.into_vec())
    }

    fn lin(src: &str) -> String {
        lineare(&formula(src).0.m)
    }
    fn tex(src: &str) -> String {
        latex(&formula(src).0.m)
    }
    fn mml(src: &str) -> String {
        mathml(&formula(src).0.m, false, "")
    }

    #[test]
    fn frazione_lineare_latex_mathml() {
        assert_eq!(lin("$a / b$"), "a/b");
        assert_eq!(tex("$a / b$"), "\\frac{a}{b}");
        assert!(mml("$a / b$").contains("<mfrac><mi>a</mi><mi>b</mi></mfrac>"));
    }

    #[test]
    fn apici_e_pedici() {
        assert_eq!(lin("$x^2$"), "x²");
        assert_eq!(lin("$x_1$"), "x₁");
        assert_eq!(lin("$u_max$"), "u_max");
        assert_eq!(tex("$x_1^2$"), "x_{1}^{2}");
        assert!(mml("$x_1^2$").contains("<msubsup>"));
    }

    #[test]
    fn radici() {
        assert_eq!(lin("$sqrt(pi)$"), "√π");
        assert_eq!(tex("$sqrt(x + 1)$"), "\\sqrt{x + 1}");
        assert!(mml("$root(3, x)$").contains("<mroot>"));
    }

    #[test]
    fn sommatoria_con_limiti() {
        let l = lin("$sum_(n=1)^(oo) 1 / n^2$");
        assert!(
            l.starts_with("∑_(n = 1)^∞") || l.starts_with("∑_(n=1)^∞"),
            "{l}"
        );
        assert!(l.contains("1/n²"), "{l}");
        let t = tex("$sum_(n=1)^(oo) 1 / n^2$");
        assert!(t.contains("\\sum_{n = 1}^{\\infty}"), "{t}");
        assert!(mml("$sum_(n=1)^(oo) x$").contains("<munderover>"));
    }

    #[test]
    fn integrale_con_limiti_laterali() {
        let m = mml("$integral_0^h u dif y$");
        assert!(m.contains("<msubsup><mo>∫</mo>"), "{m}");
        assert!(m.contains("<mi mathvariant=\"normal\">d</mi>"), "{m}");
    }

    #[test]
    fn greche_e_operatori() {
        assert_eq!(lin("$alpha + beta <= gamma$"), "α + β ≤ γ");
        assert_eq!(tex("$mu nabla^2 bold(u)$"), "\\mu \\nabla^{2} \\mathbf{u}");
        assert_eq!(lin("$a dot.c b$"), "a ⋅ b");
        assert_eq!(lin("$sin x$"), "sin x");
    }

    #[test]
    fn stili_unicode_e_latex() {
        assert_eq!(lin("$bold(u)$"), "𝐮");
        assert_eq!(lin("$bb(R)$"), "ℝ");
        assert_eq!(tex("$cal(L)$"), "\\mathcal{L}");
    }

    #[test]
    fn matrice_e_cases() {
        let t = tex("$mat(1, 2; 3, 4)$");
        assert!(
            t.contains("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}"),
            "{t}"
        );
        let c = tex("$cases(1 \"se\" x > 0, 0 \"altrimenti\")$");
        assert!(c.contains("\\begin{cases}"), "{c}");
        assert!(c.contains("\\text{se}"), "{c}");
        assert!(mml("$mat(1, 2; 3, 4)$").contains("<mtable>"));
    }

    #[test]
    fn delimitatori_e_chiamate_funzione() {
        assert_eq!(lin("$f(x)$"), "f(x)");
        assert!(tex("$abs(x)$").contains("\\left| x \\right|"));
        assert!(tex("$(a + b)^2$").contains("\\left( a + b \\right)^{2}"));
    }

    #[test]
    fn allineamento_righe() {
        let (f, _) = formula("$ a &= b \\ c &= d $");
        assert!(richiede_allineamento(&f.m));
        let t = latex(&f.m);
        assert!(t.contains("a & = b"), "{t}");
        assert!(t.contains("\\\\"), "{t}");
        assert!(mathml(&f.m, true, "").contains("<mtr>"));
    }

    #[test]
    fn stringhe_come_testo() {
        assert_eq!(lin("$\"Re\" << 1$"), "Re ≪ 1");
        assert!(tex("$\"Re\"$").contains("\\text{Re}"));
    }

    #[test]
    fn funzione_ignota_diventa_grezzo_con_avviso() {
        let (f, av) = formula("$foo.bar(x)$");
        assert!(f.ha_grezzo());
        assert!(!av.is_empty());
    }

    #[test]
    fn primi_e_accenti() {
        assert_eq!(lin("$f'$"), "f′");
        assert_eq!(tex("$hat(x)$"), "\\hat{x}");
        assert!(mml("$arrow(v)$").contains("<mover accent=\"true\">"));
    }

    #[test]
    fn xml_escape_negli_operatori() {
        let m = mml("$a < b$");
        assert!(m.contains("<mo>&lt;</mo>"), "{m}");
    }

    #[test]
    fn formula_scientifico_navier_stokes() {
        let src = "$ rho (partial bold(u)) / (partial t) + rho (bold(u) dot.c nabla) bold(u) = -nabla p + mu nabla^2 bold(u) + bold(f), quad nabla dot.c bold(u) = 0 $";
        let (f, av) = formula(src);
        assert!(!f.ha_grezzo(), "{av:?}");
        let l = lineare(&f.m);
        assert!(l.contains('ρ') && l.contains('∂') && l.contains('∇'), "{l}");
        let t = latex(&f.m);
        assert!(t.contains("\\frac{\\partial"), "{t}");
        assert!(t.contains("\\quad"), "{t}");
    }
}
