//! Rappresentazione intermedia del documento (blocchi + inline) e costruttore
//! dal syntax tree di Typst.
//!
//! Il costruttore **non valuta** il codice Typst: interpreta il markup, le
//! chiamate di funzione note (`figure`, `table`, `quote`, `image`, `link`,
//! `footnote`, …), le regole `#set` utili (lingua, titolo, autore,
//! numerazioni) e approssima il resto. Ogni perdita è registrata negli
//! avvisi del [`Documento`].

use std::collections::{HashMap, HashSet};

use typst_syntax::ast::{self, AstNode};
use typst_syntax::{SyntaxKind, SyntaxNode};

use super::matematica::{ConvMath, Formula};

// =======================================================================
// Tipi
// =======================================================================

/// Misura di una dimensione Typst (`90%`, `3.2cm`).
#[derive(Clone, Debug)]
pub(crate) struct Misura {
    pub valore: f64,
    /// `"%"`, `"cm"`, `"mm"`, `"pt"`, `"in"`, `"em"`.
    pub unita: &'static str,
}

#[derive(Clone, Debug)]
pub(crate) struct Immagine {
    pub percorso: String,
    pub alt: Option<String>,
    pub larghezza: Option<Misura>,
}

#[derive(Clone, Debug)]
pub(crate) enum Inline {
    Testo(String),
    Spazio,
    ACapo,
    Enfasi(Vec<Inline>),
    Forte(Vec<Inline>),
    Barrato(Vec<Inline>),
    Sottolineato(Vec<Inline>),
    Apice(Vec<Inline>),
    Pedice(Vec<Inline>),
    Virgolette(Vec<Inline>),
    Codice(String),
    Link {
        url: String,
        contenuto: Vec<Inline>,
    },
    Immagine(Immagine),
    Math(Formula),
    Nota(Vec<Blocco>),
    /// Riferimento `@etichetta` (a un elemento del documento o a una fonte).
    Rif {
        bersaglio: String,
        supplemento: Option<Vec<Inline>>,
    },
    /// Punto di ancoraggio di un'etichetta `<nome>`.
    Ancora(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TipoFigura {
    Figura,
    Tabella,
    Listato,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Allineamento {
    Sinistra,
    Centro,
    Destra,
}

#[derive(Clone, Debug)]
pub(crate) struct Cella {
    pub contenuto: Vec<Inline>,
    pub colspan: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Tabella {
    pub colonne: usize,
    pub allineamenti: Vec<Allineamento>,
    pub intestazione: Vec<Vec<Cella>>,
    pub righe: Vec<Vec<Cella>>,
    pub etichetta: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum Blocco {
    Titolo {
        livello: usize,
        numero: Option<String>,
        contenuto: Vec<Inline>,
        etichetta: Option<String>,
    },
    Paragrafo(Vec<Inline>),
    Lista {
        ordinata: bool,
        inizio: u64,
        voci: Vec<Vec<Blocco>>,
    },
    Termini(Vec<(Vec<Inline>, Vec<Blocco>)>),
    Citazione {
        corpo: Vec<Blocco>,
        attribuzione: Option<Vec<Inline>>,
    },
    Codice {
        lingua: Option<String>,
        testo: String,
    },
    Figura {
        corpo: Vec<Blocco>,
        didascalia: Option<Vec<Inline>>,
        tipo: TipoFigura,
        numero: String,
        etichetta: Option<String>,
    },
    Immagine(Immagine),
    Tabella(Tabella),
    Equazione {
        formula: Formula,
        numero: Option<String>,
        etichetta: Option<String>,
    },
    Separatore,
    SaltoPagina,
    /// Elemento non convertibile (testo descrittivo).
    Segnaposto(String),
    Bibliografia {
        file: String,
        titolo: Option<Vec<Inline>>,
        stile: Option<String>,
    },
    Indice,
}

/// Tipo di elemento a cui punta un'etichetta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TipoEt {
    Titolo,
    Figura,
    Tabella,
    Listato,
    Equazione,
}

#[derive(Clone, Debug)]
pub(crate) struct InfoEtichetta {
    pub tipo: TipoEt,
    /// Numero già formattato (`"2"`, `"1.1"`, `"(3)"`); vuoto se non numerato.
    pub numero: String,
}

#[derive(Debug)]
pub(crate) struct Documento {
    pub titolo: Option<String>,
    pub autore: Option<String>,
    /// Lingua (`it`, `en`, …); `"en"` se non dichiarata.
    pub lingua: String,
    pub blocchi: Vec<Blocco>,
    pub etichette: HashMap<String, InfoEtichetta>,
    pub numerazione_titoli: Option<String>,
    pub supplemento_equazioni: Option<String>,
    pub avvisi: Vec<String>,
}

/// Titolo del documento: (livello, numero, contenuto, etichetta).
pub(crate) type VoceTitolo = (usize, Option<String>, Vec<Inline>, Option<String>);

/// Numero massimo di colonne di una tabella: oltre, il valore è ridotto con
/// un avviso. Protegge da allocazioni enormi (`columns: 100000000000`) e da
/// overflow su input di una riga.
pub(crate) const COLONNE_MAX: usize = 64;

/// Livello massimo di un titolo (come `h1`–`h6` di HTML): oltre, ridotto
/// con un avviso.
pub(crate) const LIVELLO_MAX: usize = 6;

/// Elenco di avvisi **senza duplicati** che conserva l'ordine d'inserimento.
///
/// La deduplicazione usa un `HashSet` (O(1) a inserimento) al posto di
/// `Vec::contains` (O(n) a inserimento, quindi O(n²) con molti avvisi
/// distinti, ad esempio un documento con migliaia di funzioni sconosciute).
#[derive(Debug, Default, Clone)]
pub(crate) struct Avvisi {
    elenco: Vec<String>,
    visti: HashSet<String>,
}

impl Avvisi {
    /// Aggiunge `m` se non già presente; ritorna `true` se è stato inserito.
    pub fn aggiungi(&mut self, m: impl Into<String>) -> bool {
        let m = m.into();
        if !self.visti.insert(m.clone()) {
            return false;
        }
        self.elenco.push(m);
        true
    }

    /// Accoda gli avvisi di `altri`, in ordine, saltando i già presenti.
    pub fn estendi(&mut self, altri: Avvisi) {
        for m in altri.elenco {
            self.aggiungi(m);
        }
    }

    /// Consuma l'elenco e ritorna gli avvisi nell'ordine d'inserimento.
    pub fn into_vec(self) -> Vec<String> {
        self.elenco
    }
}

/// Parole localizzate usate nei riferimenti e nei titoli automatici.
pub(crate) struct Lessico {
    pub figura: &'static str,
    pub tabella: &'static str,
    pub listato: &'static str,
    pub sezione: &'static str,
    pub equazione: &'static str,
    pub bibliografia: &'static str,
    pub indice: &'static str,
}

pub(crate) fn lessico(lingua: &str) -> Lessico {
    let l = lingua.split(['-', '_']).next().unwrap_or("en");
    match l {
        "it" => Lessico {
            figura: "Figura",
            tabella: "Tabella",
            listato: "Listato",
            sezione: "Sezione",
            equazione: "Equazione",
            bibliografia: "Bibliografia",
            indice: "Indice",
        },
        "fr" => Lessico {
            figura: "Figure",
            tabella: "Tableau",
            listato: "Liste",
            sezione: "Section",
            equazione: "Équation",
            bibliografia: "Bibliographie",
            indice: "Table des matières",
        },
        "de" => Lessico {
            figura: "Abbildung",
            tabella: "Tabelle",
            listato: "Listing",
            sezione: "Abschnitt",
            equazione: "Gleichung",
            bibliografia: "Literatur",
            indice: "Inhaltsverzeichnis",
        },
        "es" => Lessico {
            figura: "Figura",
            tabella: "Tabla",
            listato: "Listado",
            sezione: "Sección",
            equazione: "Ecuación",
            bibliografia: "Bibliografía",
            indice: "Índice",
        },
        _ => Lessico {
            figura: "Figure",
            tabella: "Table",
            listato: "Listing",
            sezione: "Section",
            equazione: "Equation",
            bibliografia: "Bibliography",
            indice: "Contents",
        },
    }
}

impl Documento {
    /// Supplemento (parola) e tipo di un'etichetta: `Some(("Figura", Figura))`.
    /// `supplemento` sostituisce la parola predefinita.
    pub fn supplemento(
        &self,
        bersaglio: &str,
        supplemento: Option<&str>,
    ) -> Option<(String, TipoEt)> {
        let info = self.etichette.get(bersaglio)?;
        let lex = lessico(&self.lingua);
        let predefinito: String = match info.tipo {
            TipoEt::Figura => lex.figura.into(),
            TipoEt::Tabella => lex.tabella.into(),
            TipoEt::Listato => lex.listato.into(),
            TipoEt::Titolo => lex.sezione.into(),
            TipoEt::Equazione => self
                .supplemento_equazioni
                .clone()
                .unwrap_or_else(|| lex.equazione.into()),
        };
        Some((
            supplemento.map(str::to_string).unwrap_or(predefinito),
            info.tipo,
        ))
    }

    /// Risolve un riferimento `@bersaglio`: `Some(testo)` se punta a un
    /// elemento del documento (es. «Figura 1»), `None` se è una fonte
    /// bibliografica o un'etichetta sconosciuta.
    pub fn risolvi(&self, bersaglio: &str, supplemento: Option<&str>) -> Option<String> {
        let (suppl, _) = self.supplemento(bersaglio, supplemento)?;
        let info = self.etichette.get(bersaglio)?;
        if info.numero.is_empty() {
            return Some(suppl);
        }
        if suppl.is_empty() {
            Some(info.numero.clone())
        } else {
            Some(format!("{suppl} {}", info.numero))
        }
    }

    /// Elenco dei titoli (livello, numero, testo semplice) per l'indice.
    pub fn titoli(&self) -> Vec<VoceTitolo> {
        self.blocchi_titolo()
            .into_iter()
            .filter_map(|b| match b {
                Blocco::Titolo {
                    livello,
                    numero,
                    contenuto,
                    etichetta,
                } => Some((
                    *livello,
                    numero.clone(),
                    contenuto.clone(),
                    etichetta.clone(),
                )),
                _ => None,
            })
            .collect()
    }

    /// Riferimenti ai blocchi `Titolo` in ordine di documento (visita in
    /// profondità), **compresi** quelli annidati in citazioni, liste, termini
    /// e figure: è lo stesso ordine in cui i generatori li rendono. I titoli
    /// dentro le note (contenuto inline) non sono inclusi.
    pub fn blocchi_titolo(&self) -> Vec<&Blocco> {
        let mut v = Vec::new();
        raccogli_titoli(&self.blocchi, &mut v);
        v
    }
}

fn raccogli_titoli<'a>(blocchi: &'a [Blocco], out: &mut Vec<&'a Blocco>) {
    for b in blocchi {
        match b {
            Blocco::Titolo { .. } => out.push(b),
            Blocco::Lista { voci, .. } => {
                for v in voci {
                    raccogli_titoli(v, out);
                }
            }
            Blocco::Termini(v) => {
                for (_, d) in v {
                    raccogli_titoli(d, out);
                }
            }
            Blocco::Citazione { corpo, .. } | Blocco::Figura { corpo, .. } => {
                raccogli_titoli(corpo, out);
            }
            _ => {}
        }
    }
}

/// Testo semplice di una sequenza inline (senza markup), usato per `title`,
/// `alt`, voci d'indice, ecc.
pub(crate) fn testo_semplice(v: &[Inline]) -> String {
    let mut s = String::new();
    for i in v {
        match i {
            Inline::Testo(t) | Inline::Codice(t) => s.push_str(t),
            Inline::Spazio | Inline::ACapo => s.push(' '),
            Inline::Enfasi(x)
            | Inline::Forte(x)
            | Inline::Barrato(x)
            | Inline::Sottolineato(x)
            | Inline::Apice(x)
            | Inline::Pedice(x)
            | Inline::Virgolette(x) => s.push_str(&testo_semplice(x)),
            Inline::Link { contenuto, .. } => s.push_str(&testo_semplice(contenuto)),
            Inline::Math(f) => s.push_str(&super::matematica::lineare(&f.m)),
            _ => {}
        }
    }
    s
}

// =======================================================================
// Numerazione
// =======================================================================

fn romano(mut n: u32, maiuscolo: bool) -> String {
    const T: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    if n == 0 {
        return "0".into();
    }
    let mut s = String::new();
    for (v, r) in T {
        while n >= *v {
            s.push_str(r);
            n -= v;
        }
    }
    if maiuscolo {
        s
    } else {
        s.to_lowercase()
    }
}

fn lettere(n: u32, maiuscolo: bool) -> String {
    if n == 0 {
        return "0".into();
    }
    let base = if maiuscolo { b'A' } else { b'a' };
    let mut n = n - 1;
    let mut v = Vec::new();
    loop {
        v.push(base + (n % 26) as u8);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    v.reverse();
    String::from_utf8(v).unwrap_or_default()
}

fn simbolo_numero(sim: char, n: u32) -> String {
    match sim {
        'a' => lettere(n, false),
        'A' => lettere(n, true),
        'i' => romano(n, false),
        'I' => romano(n, true),
        _ => n.to_string(),
    }
}

/// Formatta i contatori secondo un pattern di numerazione Typst
/// (`"1.1"`, `"I"`, `"(1)"`, `"a)"`).
pub(crate) fn formatta_numerazione(pattern: &str, numeri: &[u32]) -> String {
    let mut prefisso = String::new();
    let mut simboli: Vec<char> = Vec::new();
    let mut separatori: Vec<String> = Vec::new(); // separatore prima del simbolo k (k>=1)
    let mut suffisso = String::new();
    let mut letterale = String::new();
    for c in pattern.chars() {
        if matches!(c, '1' | 'a' | 'A' | 'i' | 'I') {
            if simboli.is_empty() {
                prefisso = std::mem::take(&mut letterale);
            } else {
                separatori.push(std::mem::take(&mut letterale));
            }
            simboli.push(c);
        } else {
            letterale.push(c);
        }
    }
    if simboli.is_empty() {
        return pattern.to_string();
    }
    suffisso.push_str(&letterale);
    let mut out = prefisso;
    for (k, n) in numeri.iter().enumerate() {
        let sim = simboli[k.min(simboli.len() - 1)];
        if k >= 1 {
            let sep = if k - 1 < separatori.len() {
                separatori[k - 1].clone()
            } else {
                separatori
                    .last()
                    .cloned()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| ".".into())
            };
            out.push_str(&sep);
        }
        out.push_str(&simbolo_numero(sim, *n));
    }
    // il suffisso si applica solo quando tutti i simboli del pattern sono usati
    if numeri.len() >= simboli.len() {
        out.push_str(&suffisso);
    }
    out
}

// =======================================================================
// Costruttore
// =======================================================================

/// Annidamento massimo del contenuto (liste, citazioni, figure, argomenti di
/// funzione, …). Oltre, la parte più interna è omessa con un avviso. Il
/// valore è basso di proposito: ogni livello consuma parecchi KiB di stack
/// (la costruzione è ricorsiva) e i documenti reali non superano la decina
/// di livelli.
const PROFONDITA_MAX: usize = 32;

struct Costruttore<'a> {
    blocchi: Vec<Blocco>,
    par: Vec<Inline>,
    attesa: Option<usize>,
    titolo: Option<String>,
    autore: Option<String>,
    lingua: Option<String>,
    etichette: HashMap<String, InfoEtichetta>,
    numerazione_titoli: Option<String>,
    numerazione_eq: Option<String>,
    supplemento_eq: Option<String>,
    cont_titoli: Vec<u32>,
    cont_fig: [u32; 3],
    cont_eq: u32,
    funzioni: HashMap<String, ()>,
    valori: HashMap<String, ast::Expr<'a>>,
    math: ConvMath,
    avvisi: Avvisi,
    regole_ignorate: usize,
    grafica_omessa: Vec<String>,
    profondita: usize,
}

/// Analizza un sorgente Typst e produce il [`Documento`].
pub(crate) fn analizza(sorgente: &str) -> Documento {
    let radice = typst_syntax::parse(sorgente);
    let mut c = Costruttore {
        blocchi: Vec::new(),
        par: Vec::new(),
        attesa: None,
        titolo: None,
        autore: None,
        lingua: None,
        etichette: HashMap::new(),
        numerazione_titoli: None,
        numerazione_eq: None,
        supplemento_eq: None,
        cont_titoli: Vec::new(),
        cont_fig: [0; 3],
        cont_eq: 0,
        funzioni: HashMap::new(),
        valori: HashMap::new(),
        math: ConvMath::nuovo(),
        avvisi: Avvisi::default(),
        regole_ignorate: 0,
        grafica_omessa: Vec::new(),
        profondita: 0,
    };
    if radice.diagnosis().errors {
        c.avvisi.aggiungi(
            "il sorgente contiene errori di sintassi: il documento è convertito al meglio",
        );
    }
    if let Some(m) = radice.cast::<ast::Markup>() {
        c.markup(m);
    }
    c.chiudi_par();
    c.concludi()
}

/// Nome puntato di un callee (`a.b.c`). Iterativo: una catena di migliaia di
/// campi non deve consumare lo stack.
fn nome_callee(e: ast::Expr) -> Option<String> {
    let mut campi: Vec<&str> = Vec::new();
    let mut corrente = e;
    loop {
        match corrente {
            ast::Expr::Ident(i) => {
                campi.push(i.as_str());
                break;
            }
            ast::Expr::FieldAccess(f) => {
                campi.push(f.field().as_str());
                corrente = f.target();
            }
            _ => return None,
        }
    }
    campi.reverse();
    Some(campi.join("."))
}

fn stringa(e: ast::Expr) -> Option<String> {
    match e {
        ast::Expr::Str(s) => Some(s.get().to_string()),
        _ => None,
    }
}

fn misura(e: ast::Expr) -> Option<Misura> {
    if let ast::Expr::Numeric(n) = e {
        let (valore, u) = n.get();
        let unita = match u {
            ast::Unit::Pt => "pt",
            ast::Unit::Mm => "mm",
            ast::Unit::Cm => "cm",
            ast::Unit::In => "in",
            ast::Unit::Em => "em",
            ast::Unit::Percent => "%",
            _ => return None,
        };
        return Some(Misura { valore, unita });
    }
    None
}

fn allineamento_da(e: ast::Expr) -> Option<Allineamento> {
    match e {
        ast::Expr::Ident(i) => match i.as_str() {
            "left" | "start" => Some(Allineamento::Sinistra),
            "center" => Some(Allineamento::Centro),
            "right" | "end" => Some(Allineamento::Destra),
            _ => None,
        },
        ast::Expr::Binary(b) => allineamento_da(b.lhs()).or_else(|| allineamento_da(b.rhs())),
        _ => None,
    }
}

/// Elenca gli argomenti `Pos` di una chiamata.
fn posizionali<'a>(args: ast::Args<'a>) -> Vec<ast::Expr<'a>> {
    args.items()
        .filter_map(|a| match a {
            ast::Arg::Pos(e) => Some(e),
            _ => None,
        })
        .collect()
}

fn nominato<'a>(args: ast::Args<'a>, nome: &str) -> Option<ast::Expr<'a>> {
    args.items().find_map(|a| match a {
        ast::Arg::Named(n) if n.name().as_str() == nome => Some(n.expr()),
        _ => None,
    })
}

impl<'a> Costruttore<'a> {
    fn avvisa(&mut self, msg: impl Into<String>) {
        self.avvisi.aggiungi(msg);
    }

    /// Riduce un numero di colonne al tetto [`COLONNE_MAX`] (minimo 1),
    /// avvisando se ha dovuto ridurlo.
    fn colonne_limitate(&mut self, richieste: u128) -> usize {
        if richieste > COLONNE_MAX as u128 {
            self.avvisa(format!(
                "tabella con {richieste} colonne: ridotta a {COLONNE_MAX} colonne"
            ));
            COLONNE_MAX
        } else {
            (richieste as usize).max(1)
        }
    }

    /// Riduce il livello di un titolo a `1..=`[`LIVELLO_MAX`], avvisando se
    /// ha dovuto ridurlo.
    fn livello_limitato(&mut self, richiesto: i128) -> usize {
        if richiesto > LIVELLO_MAX as i128 {
            self.avvisa(format!(
                "titolo di livello {richiesto}: ridotto al livello {LIVELLO_MAX}"
            ));
            LIVELLO_MAX
        } else {
            richiesto.max(1) as usize
        }
    }

    // ---- contenitori --------------------------------------------------

    fn chiudi_par(&mut self) {
        let par = std::mem::take(&mut self.par);
        let par = normalizza(par);
        if par.is_empty() {
            return;
        }
        if par.len() == 1 {
            if let Inline::Immagine(img) = &par[0] {
                self.blocchi.push(Blocco::Immagine(img.clone()));
                return;
            }
        }
        self.blocchi.push(Blocco::Paragrafo(par));
    }

    fn push_blocco(&mut self, b: Blocco) {
        self.chiudi_par();
        self.blocchi.push(b);
    }

    /// Come `push_blocco`, ma il blocco può ricevere un'etichetta `<nome>`
    /// subito dopo.
    fn push_etichettabile(&mut self, b: Blocco) {
        self.push_blocco(b);
        self.attesa = Some(self.blocchi.len() - 1);
    }

    fn sotto(&mut self, f: impl FnOnce(&mut Self)) -> Vec<Blocco> {
        self.sotto_livello(true, f)
    }

    /// Converte l'argomento `e` in un contesto separato **senza** aggiungere un
    /// livello di profondità: lo aggiunge già [`Costruttore::contenuto_espr`].
    /// (Con `sotto` si contava due volte e il tetto effettivo era la metà.)
    fn sotto_espr(&mut self, e: ast::Expr<'a>) -> Vec<Blocco> {
        self.sotto_livello(false, |s| s.contenuto_espr(e))
    }

    fn sotto_livello(&mut self, conta: bool, f: impl FnOnce(&mut Self)) -> Vec<Blocco> {
        let blocchi = std::mem::take(&mut self.blocchi);
        let par = std::mem::take(&mut self.par);
        let attesa = self.attesa.take();
        let livello = usize::from(conta);
        self.profondita += livello;
        f(self);
        self.chiudi_par();
        self.profondita -= livello;
        let risultato = std::mem::replace(&mut self.blocchi, blocchi);
        self.par = par;
        self.attesa = attesa;
        risultato
    }

    fn corpo_inline(&mut self, m: ast::Markup<'a>) -> Vec<Inline> {
        let b = self.sotto(|s| s.markup(m));
        normalizza(appiattisci(b))
    }

    fn espr_inline(&mut self, e: ast::Expr<'a>) -> Vec<Inline> {
        let b = self.sotto_espr(e);
        normalizza(appiattisci(b))
    }

    /// Contenuto di un argomento: blocco `[..]`, stringa o chiamata.
    ///
    /// Incrementa la profondità: ogni argomento è un livello di annidamento,
    /// anche quando non passa da [`Costruttore::sotto`] (ad esempio una
    /// funzione sconosciuta annidata `#f[#f[#f[…]]]`, che altrimenti
    /// ricorrerebbe senza limite).
    fn contenuto_espr(&mut self, e: ast::Expr<'a>) {
        if self.profondita > PROFONDITA_MAX {
            self.avvisa("contenuto annidato troppo in profondità: parte omessa");
            return;
        }
        self.profondita += 1;
        self.contenuto_espr_interno(e);
        self.profondita -= 1;
    }

    fn contenuto_espr_interno(&mut self, e: ast::Expr<'a>) {
        match e {
            ast::Expr::ContentBlock(cb) => self.markup(cb.body()),
            ast::Expr::Str(s) => self.testo(s.get().as_str()),
            // `#figure(```rust …```, caption: …)`: il listato è l'argomento stesso
            ast::Expr::Raw(_) => self.espr_markup(e),
            ast::Expr::FuncCall(_)
            | ast::Expr::Parenthesized(_)
            | ast::Expr::FieldAccess(_)
            | ast::Expr::Contextual(_) => self.espr_hash(e),
            ast::Expr::Int(_) | ast::Expr::Float(_) => self.espr_hash(e),
            ast::Expr::Ident(i) if self.valori.contains_key(i.as_str()) => self.espr_hash(e),
            _ => {}
        }
    }

    // ---- markup -------------------------------------------------------

    fn markup(&mut self, markup: ast::Markup<'a>) {
        if self.profondita > PROFONDITA_MAX {
            self.avvisa("contenuto annidato troppo in profondità: parte omessa");
            return;
        }
        let figli: Vec<&'a SyntaxNode> = markup.to_untyped().children().collect();
        let mut i = 0;
        while i < figli.len() {
            let n = figli[i];
            let k = n.kind();
            if k != SyntaxKind::Space && k != SyntaxKind::Label {
                self.attesa = None;
            }
            match k {
                SyntaxKind::Hash => {
                    if let Some(e) = figli.get(i + 1).and_then(|n| n.cast::<ast::Expr>()) {
                        self.espr_hash(e);
                        i += 2;
                        if figli.get(i).map(|n| n.kind()) == Some(SyntaxKind::Semicolon) {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                    continue;
                }
                SyntaxKind::ListItem | SyntaxKind::EnumItem | SyntaxKind::TermItem => {
                    i = self.elenco(&figli, i);
                    continue;
                }
                SyntaxKind::LineComment | SyntaxKind::BlockComment => {}
                SyntaxKind::Space => self.par.push(Inline::Spazio),
                _ => {
                    if let Some(e) = n.cast::<ast::Expr>() {
                        self.espr_markup(e);
                    }
                }
            }
            i += 1;
        }
    }

    fn ultimo_carattere(&self) -> Option<char> {
        match self.par.last() {
            Some(Inline::Testo(t)) => t.chars().last(),
            Some(Inline::Spazio) | Some(Inline::ACapo) | None => None,
            Some(_) => Some('x'),
        }
    }

    fn testo(&mut self, t: &str) {
        if t.is_empty() {
            return;
        }
        self.par.push(Inline::Testo(t.to_string()));
    }

    fn espr_markup(&mut self, e: ast::Expr<'a>) {
        match e {
            ast::Expr::Text(t) => self.testo(t.get().as_str()),
            ast::Expr::Space(_) => self.par.push(Inline::Spazio),
            ast::Expr::Linebreak(_) => self.par.push(Inline::ACapo),
            ast::Expr::Parbreak(_) => self.chiudi_par(),
            ast::Expr::Escape(x) => self.testo(&x.get().to_string()),
            ast::Expr::Shorthand(x) => self.testo(&x.get().to_string()),
            ast::Expr::SmartQuote(q) => {
                let apre = match self.ultimo_carattere() {
                    None => true,
                    Some(c) => c.is_whitespace() || matches!(c, '(' | '[' | '{' | '—' | '–' | '«'),
                };
                let c = match (q.double(), apre) {
                    (true, true) => '“',
                    (true, false) => '”',
                    (false, true) => '‘',
                    (false, false) => '’',
                };
                self.testo(&c.to_string());
            }
            ast::Expr::Strong(s) => {
                let v = self.corpo_inline(s.body());
                self.par.push(Inline::Forte(v));
            }
            ast::Expr::Emph(s) => {
                let v = self.corpo_inline(s.body());
                self.par.push(Inline::Enfasi(v));
            }
            ast::Expr::Raw(r) => {
                let testo = r
                    .lines()
                    .map(|l| l.get().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                if r.block() {
                    let lingua = r.lang().map(|l| l.get().to_string());
                    self.push_blocco(Blocco::Codice { lingua, testo });
                } else {
                    self.par.push(Inline::Codice(testo));
                }
            }
            ast::Expr::Link(l) => {
                let url = l.get().to_string();
                self.par.push(Inline::Link {
                    contenuto: vec![Inline::Testo(url.clone())],
                    url,
                });
            }
            ast::Expr::Label(l) => self.etichetta(l.get().to_string()),
            ast::Expr::Ref(r) => {
                let supplemento = r.supplement().map(|cb| self.corpo_inline(cb.body()));
                self.par.push(Inline::Rif {
                    bersaglio: r.target().to_string(),
                    supplemento,
                });
            }
            ast::Expr::Heading(h) => self.titolo(h),
            ast::Expr::Equation(eq) => self.equazione(eq),
            _ => {}
        }
    }

    // ---- etichette ----------------------------------------------------

    fn etichetta(&mut self, nome: String) {
        match self.attesa.take() {
            Some(idx) if idx < self.blocchi.len() => self.assegna_etichetta(idx, nome),
            _ => self.par.push(Inline::Ancora(nome)),
        }
    }

    fn assegna_etichetta(&mut self, idx: usize, nome: String) {
        let info = match &mut self.blocchi[idx] {
            Blocco::Titolo {
                numero, etichetta, ..
            } => {
                *etichetta = Some(nome.clone());
                InfoEtichetta {
                    tipo: TipoEt::Titolo,
                    numero: numero.clone().unwrap_or_default(),
                }
            }
            Blocco::Figura {
                tipo,
                numero,
                etichetta,
                ..
            } => {
                *etichetta = Some(nome.clone());
                InfoEtichetta {
                    tipo: match tipo {
                        TipoFigura::Figura => TipoEt::Figura,
                        TipoFigura::Tabella => TipoEt::Tabella,
                        TipoFigura::Listato => TipoEt::Listato,
                    },
                    numero: numero.clone(),
                }
            }
            Blocco::Tabella(t) => {
                t.etichetta = Some(nome.clone());
                InfoEtichetta {
                    tipo: TipoEt::Tabella,
                    numero: String::new(),
                }
            }
            Blocco::Equazione {
                numero, etichetta, ..
            } => {
                *etichetta = Some(nome.clone());
                InfoEtichetta {
                    tipo: TipoEt::Equazione,
                    numero: numero.clone().unwrap_or_default(),
                }
            }
            _ => return,
        };
        self.etichette.insert(nome, info);
    }

    // ---- titoli, elenchi, equazioni -----------------------------------

    fn titolo(&mut self, h: ast::Heading<'a>) {
        let livello = self.livello_limitato(h.depth().get() as i128);
        let mut contenuto = self.corpo_inline(h.body());
        let mut etichetta = None;
        if let Some(Inline::Ancora(_)) = contenuto.last() {
            if let Some(Inline::Ancora(n)) = contenuto.pop() {
                etichetta = Some(n);
            }
            contenuto = normalizza(contenuto);
        }
        self.cont_titoli.truncate(livello);
        self.cont_titoli.resize(livello, 0);
        self.cont_titoli[livello - 1] += 1;
        let numero = self
            .numerazione_titoli
            .as_deref()
            .map(|p| formatta_numerazione(p, &self.cont_titoli));
        if let Some(n) = &etichetta {
            self.etichette.insert(
                n.clone(),
                InfoEtichetta {
                    tipo: TipoEt::Titolo,
                    numero: numero.clone().unwrap_or_default(),
                },
            );
        }
        self.push_etichettabile(Blocco::Titolo {
            livello,
            numero,
            contenuto,
            etichetta,
        });
    }

    /// Raggruppa le voci consecutive dello stesso tipo; ritorna l'indice del
    /// primo nodo non consumato.
    fn elenco(&mut self, figli: &[&'a SyntaxNode], da: usize) -> usize {
        let tipo = figli[da].kind();
        let mut voci_ul: Vec<Vec<Blocco>> = Vec::new();
        let mut termini: Vec<(Vec<Inline>, Vec<Blocco>)> = Vec::new();
        let mut inizio: Option<u64> = None;
        let mut j = da;
        let mut fine = da;
        while j < figli.len() {
            let n = figli[j];
            if n.kind() != tipo {
                break;
            }
            match n.cast::<ast::Expr>() {
                Some(ast::Expr::ListItem(v)) => {
                    let b = self.sotto(|s| s.markup(v.body()));
                    voci_ul.push(b);
                }
                Some(ast::Expr::EnumItem(v)) => {
                    if inizio.is_none() {
                        inizio = Some(v.number().unwrap_or(1));
                    }
                    let b = self.sotto(|s| s.markup(v.body()));
                    voci_ul.push(b);
                }
                Some(ast::Expr::TermItem(v)) => {
                    let t = self.corpo_inline(v.term());
                    let d = self.sotto(|s| s.markup(v.description()));
                    termini.push((t, d));
                }
                _ => {}
            }
            fine = j + 1;
            // salta spazi/interruzioni e controlla che segua la stessa voce
            let mut k = j + 1;
            while k < figli.len()
                && matches!(figli[k].kind(), SyntaxKind::Space | SyntaxKind::Parbreak)
            {
                k += 1;
            }
            if k < figli.len() && figli[k].kind() == tipo {
                j = k;
            } else {
                break;
            }
        }
        match tipo {
            SyntaxKind::TermItem => self.push_blocco(Blocco::Termini(termini)),
            SyntaxKind::EnumItem => self.push_blocco(Blocco::Lista {
                ordinata: true,
                inizio: inizio.unwrap_or(1),
                voci: voci_ul,
            }),
            _ => self.push_blocco(Blocco::Lista {
                ordinata: false,
                inizio: 1,
                voci: voci_ul,
            }),
        }
        fine
    }

    fn equazione(&mut self, eq: ast::Equation<'a>) {
        let formula = self.math.equazione(eq);
        if eq.block() {
            let numero = self.numerazione_eq.clone().map(|p| {
                self.cont_eq += 1;
                formatta_numerazione(&p, &[self.cont_eq])
            });
            self.push_etichettabile(Blocco::Equazione {
                formula,
                numero,
                etichetta: None,
            });
        } else {
            self.par.push(Inline::Math(formula));
        }
    }

    // ---- espressioni con '#' ------------------------------------------

    fn espr_hash(&mut self, e: ast::Expr<'a>) {
        if self.profondita > PROFONDITA_MAX {
            self.avvisa("contenuto annidato troppo in profondità: parte omessa");
            return;
        }
        match e {
            ast::Expr::FuncCall(c) => self.chiamata(c),
            ast::Expr::SetRule(r) => self.regola_set(r),
            ast::Expr::ShowRule(_) => self.regole_ignorate += 1,
            ast::Expr::LetBinding(l) => self.definizione(l),
            ast::Expr::ContentBlock(cb) => self.markup(cb.body()),
            ast::Expr::Parenthesized(p) => {
                // `#((((x))))`: ogni parentesi è un livello di ricorsione
                self.profondita += 1;
                self.espr_hash(p.expr());
                self.profondita -= 1;
            }
            ast::Expr::Str(s) => self.testo(s.get().as_str()),
            ast::Expr::Int(i) => self.testo(&i.get().to_string()),
            ast::Expr::Float(f) => self.testo(&f.get().to_string()),
            ast::Expr::Ident(i) => {
                let nome = i.as_str();
                if let Some(v) = self.valori.get(nome).copied() {
                    self.profondita += 1;
                    self.espr_hash(v);
                    self.profondita -= 1;
                } else {
                    self.avvisa(format!("variabile «{nome}» non valutata (omessa)"));
                }
            }
            ast::Expr::FieldAccess(f) => {
                let nome = nome_callee(ast::Expr::FieldAccess(f)).unwrap_or_default();
                match super::matematica::simbolo_unicode(
                    nome.strip_prefix("sym.")
                        .or(nome.strip_prefix("emoji."))
                        .unwrap_or(&nome),
                ) {
                    Some(s) if nome.starts_with("sym.") => self.testo(&s),
                    _ => self.avvisa(format!("accesso a campo «{nome}» non valutato (omesso)")),
                }
            }
            ast::Expr::CodeBlock(_) => {
                self.avvisa("blocco di codice #{…} non valutato (omesso)");
            }
            ast::Expr::Contextual(_) => {
                self.avvisa("espressione `context` non valutata (omessa)");
            }
            ast::Expr::Conditional(_) | ast::Expr::ForLoop(_) | ast::Expr::WhileLoop(_) => {
                self.avvisa("costrutto di controllo (if/for/while) non valutato (omesso)");
            }
            ast::Expr::ModuleInclude(_) => {
                self.avvisa("`#include` non seguito: il file incluso non è convertito");
            }
            _ => {}
        }
    }

    fn definizione(&mut self, l: ast::LetBinding<'a>) {
        match l.kind() {
            ast::LetBindingKind::Closure(nome) => {
                self.funzioni.insert(nome.as_str().to_string(), ());
            }
            ast::LetBindingKind::Normal(p) => {
                if let (Some(init), [nome]) = (l.init(), p.bindings().as_slice()) {
                    self.valori.insert(nome.as_str().to_string(), init);
                }
            }
        }
    }

    fn regola_set(&mut self, r: ast::SetRule<'a>) {
        let bersaglio = nome_callee(r.target()).unwrap_or_default();
        let args = r.args();
        match bersaglio.as_str() {
            "document" => {
                if let Some(t) = nominato(args, "title") {
                    let s = match stringa(t) {
                        Some(s) => Some(s),
                        None => {
                            let v = self.espr_inline(t);
                            let s = testo_semplice(&v);
                            (!s.is_empty()).then_some(s)
                        }
                    };
                    self.titolo = s;
                }
                if let Some(a) = nominato(args, "author") {
                    self.autore = match a {
                        ast::Expr::Array(arr) => {
                            let v: Vec<String> = arr
                                .items()
                                .filter_map(|it| match it {
                                    ast::ArrayItem::Pos(e) => stringa(e),
                                    _ => None,
                                })
                                .collect();
                            (!v.is_empty()).then(|| v.join(", "))
                        }
                        e => stringa(e),
                    };
                }
            }
            "text" => {
                if let Some(l) = nominato(args, "lang").and_then(stringa) {
                    self.lingua = Some(l);
                }
            }
            "heading" => {
                if let Some(n) = nominato(args, "numbering") {
                    self.numerazione_titoli = stringa(n);
                }
            }
            "math.equation" => {
                if let Some(n) = nominato(args, "numbering") {
                    self.numerazione_eq = stringa(n);
                }
                if let Some(s) = nominato(args, "supplement") {
                    self.supplemento_eq = stringa(s);
                }
            }
            _ => self.regole_ignorate += 1,
        }
    }

    // ---- chiamate di funzione -----------------------------------------

    fn chiamata(&mut self, c: ast::FuncCall<'a>) {
        // `datetime(...).display(...)`
        if let ast::Expr::FieldAccess(f) = c.callee() {
            if let ast::Expr::FuncCall(inner) = f.target() {
                if nome_callee(inner.callee()).as_deref() == Some("datetime")
                    && f.field().as_str() == "display"
                {
                    self.data_formattata(inner, c.args());
                    return;
                }
            }
        }
        let Some(nome) = nome_callee(c.callee()) else {
            self.avvisa("chiamata di funzione non riconoscibile (omessa)");
            return;
        };
        let args = c.args();
        match nome.as_str() {
            "figure" => self.figura(args),
            "table" => {
                if let Some(t) = self.tabella(args) {
                    self.push_etichettabile(Blocco::Tabella(t));
                }
            }
            "image" => {
                if let Some(img) = self.immagine(args) {
                    self.par.push(Inline::Immagine(img));
                }
            }
            "link" => self.link(args),
            "footnote" => {
                let cont = posizionali(args)
                    .into_iter()
                    .next()
                    .map(|e| self.sotto_espr(e))
                    .unwrap_or_default();
                self.par.push(Inline::Nota(cont));
            }
            "quote" => self.citazione(args),
            "raw" => {
                let testo = posizionali(args).into_iter().next().and_then(stringa);
                if let Some(t) = testo {
                    let blocco = nominato(args, "block")
                        .map(|e| matches!(e, ast::Expr::Bool(b) if b.get()))
                        .unwrap_or(false);
                    if blocco {
                        let lingua = nominato(args, "lang").and_then(stringa);
                        self.push_blocco(Blocco::Codice { lingua, testo: t });
                    } else {
                        self.par.push(Inline::Codice(t));
                    }
                }
            }
            "emph" | "strong" | "strike" | "underline" | "overline" | "highlight" | "sub"
            | "super" | "smallcaps" | "upper" | "lower" | "text" => self.stile_inline(&nome, args),
            "align" | "block" | "box" | "pad" | "par" | "place" | "grid" | "stack" | "columns"
            | "rotate" | "scale" | "move" | "rect" | "square" | "circle" | "ellipse" | "layout"
            | "repeat" | "grid.cell" | "style" => {
                let ha_contenuto = posizionali(args).into_iter().any(|e| {
                    matches!(
                        e,
                        ast::Expr::ContentBlock(_) | ast::Expr::Str(_) | ast::Expr::FuncCall(_)
                    )
                });
                if !ha_contenuto
                    && matches!(nome.as_str(), "rect" | "square" | "circle" | "ellipse")
                {
                    self.grafica_omessa.push(nome.clone());
                    return;
                }
                if matches!(
                    nome.as_str(),
                    "block" | "grid" | "stack" | "columns" | "align"
                ) {
                    self.chiudi_par();
                }
                for e in posizionali(args) {
                    self.contenuto_espr(e);
                }
                if matches!(
                    nome.as_str(),
                    "block" | "grid" | "stack" | "columns" | "align"
                ) {
                    self.chiudi_par();
                }
            }
            "hide" => {}
            "v" | "colbreak" | "parbreak" => self.chiudi_par(),
            "h" => self.par.push(Inline::Spazio),
            "linebreak" => self.par.push(Inline::ACapo),
            "pagebreak" => self.push_blocco(Blocco::SaltoPagina),
            "line" => self.push_blocco(Blocco::Separatore),
            "polygon" | "curve" | "path" | "polyline" | "line.arrow" => {
                self.grafica_omessa.push(nome.clone());
            }
            "bibliography" => {
                let file = posizionali(args)
                    .into_iter()
                    .next()
                    .map(|e| match e {
                        ast::Expr::Array(a) => a
                            .items()
                            .filter_map(|it| match it {
                                ast::ArrayItem::Pos(e) => stringa(e),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                        e => stringa(e).unwrap_or_default(),
                    })
                    .unwrap_or_default();
                let titolo = match nominato(args, "title") {
                    Some(ast::Expr::None(_)) => Some(Vec::new()),
                    Some(ast::Expr::Auto(_)) | None => None,
                    Some(e) => Some(self.espr_inline(e)),
                };
                let stile = nominato(args, "style").and_then(stringa);
                self.avvisa(format!(
                    "bibliografia «{file}»: il file non viene letto, l'elenco delle fonti non è generato"
                ));
                self.push_blocco(Blocco::Bibliografia {
                    file,
                    titolo,
                    stile,
                });
            }
            "outline" => self.push_blocco(Blocco::Indice),
            "cite" => {
                for e in posizionali(args) {
                    if let ast::Expr::Label(l) = e {
                        self.par.push(Inline::Rif {
                            bersaglio: l.get().to_string(),
                            supplemento: None,
                        });
                    }
                }
            }
            "heading" => {
                let richiesto = nominato(args, "level")
                    .and_then(|e| match e {
                        ast::Expr::Int(i) => Some(i128::from(i.get())),
                        _ => None,
                    })
                    .unwrap_or(1);
                let livello = self.livello_limitato(richiesto);
                let cont = posizionali(args)
                    .into_iter()
                    .next()
                    .map(|e| self.espr_inline(e))
                    .unwrap_or_default();
                self.push_blocco(Blocco::Titolo {
                    livello,
                    numero: None,
                    contenuto: cont,
                    etichetta: None,
                });
            }
            "lorem" => self.avvisa("`lorem(…)` non espanso (omesso)"),
            "counter" | "numbering" | "state" | "query" | "locate" | "datetime" | "read"
            | "json" | "csv" | "yaml" | "toml" | "xml" => {
                self.avvisa(format!("funzione `{nome}` non valutata (omessa)"));
            }
            "linebreak_" => {}
            _ => {
                if self.funzioni.contains_key(&nome) {
                    self.avvisa(format!(
                        "funzione `{nome}` definita nel documento non valutata: resi solo i suoi argomenti"
                    ));
                } else if nome.starts_with("sym.") || nome.starts_with("emoji.") {
                    // simbolo chiamato come funzione: ignorato
                } else {
                    self.avvisa(format!(
                        "funzione `{nome}` sconosciuta: resi solo i suoi argomenti"
                    ));
                }
                for e in posizionali(args) {
                    self.contenuto_espr(e);
                }
            }
        }
    }

    fn data_formattata(&mut self, datetime: ast::FuncCall<'a>, display: ast::Args<'a>) {
        let intero = |nome: &str| -> Option<i64> {
            nominato(datetime.args(), nome).and_then(|e| match e {
                ast::Expr::Int(i) => Some(i.get()),
                _ => None,
            })
        };
        let (Some(anno), Some(mese), Some(giorno)) =
            (intero("year"), intero("month"), intero("day"))
        else {
            self.avvisa("data `datetime` con argomenti non letterali non valutata (omessa)");
            return;
        };
        let formato = posizionali(display)
            .into_iter()
            .next()
            .and_then(stringa)
            .unwrap_or_else(|| "[year]-[month]-[day]".into());
        let testo = formato
            .replace("[year]", &format!("{anno:04}"))
            .replace("[month]", &format!("{mese:02}"))
            .replace("[day]", &format!("{giorno:02}"));
        self.testo(&testo);
    }

    fn stile_inline(&mut self, nome: &str, args: ast::Args<'a>) {
        let mut v: Vec<Inline> = Vec::new();
        for e in posizionali(args) {
            v.extend(self.espr_inline(e));
        }
        let v = normalizza(v);
        if v.is_empty() {
            return;
        }
        let avvolto = match nome {
            "emph" => Inline::Enfasi(v),
            "strong" => Inline::Forte(v),
            "strike" => Inline::Barrato(v),
            "underline" | "overline" => Inline::Sottolineato(v),
            "sub" => Inline::Pedice(v),
            "super" => Inline::Apice(v),
            "upper" => {
                let mut v = v;
                trasforma_testo(&mut v, &|t| t.to_uppercase());
                self.par.extend(v);
                return;
            }
            "lower" => {
                let mut v = v;
                trasforma_testo(&mut v, &|t| t.to_lowercase());
                self.par.extend(v);
                return;
            }
            "text" => {
                let grassetto = nominato(args, "weight").is_some_and(|e| match e {
                    ast::Expr::Str(s) => matches!(
                        s.get().as_str(),
                        "bold" | "semibold" | "extrabold" | "black"
                    ),
                    ast::Expr::Int(i) => i.get() >= 600,
                    _ => false,
                });
                let corsivo = nominato(args, "style")
                    .and_then(stringa)
                    .is_some_and(|s| s == "italic" || s == "oblique");
                let mut v = v;
                if corsivo {
                    v = vec![Inline::Enfasi(v)];
                }
                if grassetto {
                    v = vec![Inline::Forte(v)];
                }
                self.par.extend(v);
                return;
            }
            _ => {
                self.par.extend(v);
                return;
            }
        };
        self.par.push(avvolto);
    }

    fn immagine(&mut self, args: ast::Args<'a>) -> Option<Immagine> {
        let percorso = posizionali(args).into_iter().next().and_then(stringa)?;
        Some(Immagine {
            percorso,
            alt: nominato(args, "alt").and_then(stringa),
            larghezza: nominato(args, "width").and_then(misura),
        })
    }

    fn link(&mut self, args: ast::Args<'a>) {
        let pos = posizionali(args);
        let Some(primo) = pos.first().copied() else {
            return;
        };
        let corpo: Vec<Inline> = pos
            .iter()
            .skip(1)
            .flat_map(|e| self.espr_inline(*e))
            .collect();
        match primo {
            ast::Expr::Str(s) => {
                let url = s.get().to_string();
                let contenuto = if corpo.is_empty() {
                    vec![Inline::Testo(url.clone())]
                } else {
                    normalizza(corpo)
                };
                self.par.push(Inline::Link { url, contenuto });
            }
            _ => {
                // destinazione non letterale (etichetta, posizione): solo il testo
                self.par.extend(corpo);
            }
        }
    }

    fn citazione(&mut self, args: ast::Args<'a>) {
        let blocco =
            nominato(args, "block").is_some_and(|e| matches!(e, ast::Expr::Bool(b) if b.get()));
        let attribuzione = nominato(args, "attribution").and_then(|e| match e {
            ast::Expr::None(_) | ast::Expr::Auto(_) => None,
            ast::Expr::Label(l) => Some(vec![Inline::Testo(l.get().to_string())]),
            e => {
                let v = self.espr_inline(e);
                (!v.is_empty()).then_some(v)
            }
        });
        let corpo_expr = posizionali(args).into_iter().next();
        let corpo = match corpo_expr {
            Some(e) => self.sotto_espr(e),
            None => Vec::new(),
        };
        if blocco {
            self.push_blocco(Blocco::Citazione {
                corpo,
                attribuzione,
            });
        } else {
            let mut v = appiattisci(corpo);
            if let Some(a) = attribuzione {
                v.push(Inline::Testo(" — ".into()));
                v.extend(a);
            }
            self.par.push(Inline::Virgolette(normalizza(v)));
        }
    }

    fn figura(&mut self, args: ast::Args<'a>) {
        let corpo_expr = posizionali(args).into_iter().next();
        let corpo = match corpo_expr {
            Some(e) => self.sotto_espr(e),
            None => Vec::new(),
        };
        let mut corpo = corpo;
        // `#figure(image(..), alt: "…")`: il testo alternativo della figura vale
        // per le sue immagini che non ne hanno uno proprio.
        if let Some(alt) = nominato(args, "alt").and_then(stringa) {
            for b in &mut corpo {
                match b {
                    Blocco::Immagine(i) if i.alt.is_none() => i.alt = Some(alt.clone()),
                    Blocco::Paragrafo(v) => {
                        for x in v {
                            if let Inline::Immagine(i) = x {
                                if i.alt.is_none() {
                                    i.alt = Some(alt.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if corpo.is_empty() {
            self.avvisa("figura con corpo generato da codice Typst non convertibile: sostituita da un segnaposto");
            corpo.push(Blocco::Segnaposto(
                "figura generata da codice Typst, non convertita".into(),
            ));
        }
        let didascalia = nominato(args, "caption").and_then(|e| match e {
            ast::Expr::None(_) => None,
            e => {
                let v = self.espr_inline(e);
                (!v.is_empty()).then_some(v)
            }
        });
        let mut tipo = if corpo.iter().any(|b| matches!(b, Blocco::Tabella(_))) {
            TipoFigura::Tabella
        } else if corpo.iter().any(|b| matches!(b, Blocco::Codice { .. })) {
            TipoFigura::Listato
        } else {
            TipoFigura::Figura
        };
        if let Some(k) = nominato(args, "kind") {
            let nome = match k {
                ast::Expr::Ident(i) => Some(i.as_str().to_string()),
                ast::Expr::Str(s) => Some(s.get().to_string()),
                _ => None,
            };
            match nome.as_deref() {
                Some("table") => tipo = TipoFigura::Tabella,
                Some("raw") => tipo = TipoFigura::Listato,
                Some("image") | Some("figure") => tipo = TipoFigura::Figura,
                _ => {}
            }
        }
        let idx = match tipo {
            TipoFigura::Figura => 0,
            TipoFigura::Tabella => 1,
            TipoFigura::Listato => 2,
        };
        self.cont_fig[idx] += 1;
        let numero = self.cont_fig[idx].to_string();
        self.push_etichettabile(Blocco::Figura {
            corpo,
            didascalia,
            tipo,
            numero,
            etichetta: None,
        });
    }

    fn tabella(&mut self, args: ast::Args<'a>) -> Option<Tabella> {
        let mut colonne = 1usize;
        let mut allineamenti: Vec<Allineamento> = Vec::new();
        let mut alline_unico: Option<Allineamento> = None;
        if let Some(c) = nominato(args, "columns") {
            let richieste = match c {
                ast::Expr::Int(i) => i.get().max(1) as u128,
                ast::Expr::Array(a) => a.items().count().max(1) as u128,
                _ => 1,
            };
            colonne = self.colonne_limitate(richieste);
        }
        if let Some(a) = nominato(args, "align") {
            match a {
                ast::Expr::Array(arr) => {
                    for it in arr.items() {
                        if let ast::ArrayItem::Pos(e) = it {
                            allineamenti.push(allineamento_da(e).unwrap_or(Allineamento::Sinistra));
                        }
                    }
                }
                e => alline_unico = allineamento_da(e),
            }
        }
        let mut intest: Vec<Cella> = Vec::new();
        let mut corpo: Vec<Cella> = Vec::new();
        for a in args.items() {
            match a {
                ast::Arg::Pos(e) => self.celle_da(e, false, &mut intest, &mut corpo),
                ast::Arg::Spread(_) => {
                    self.avvisa("tabella con celle generate da codice (`..`): celle omesse");
                }
                ast::Arg::Named(_) => {}
            }
        }
        if intest.is_empty() && corpo.is_empty() {
            return None;
        }
        let mut al = allineamenti;
        if al.is_empty() {
            al = vec![alline_unico.unwrap_or(Allineamento::Sinistra); colonne];
        }
        al.resize(colonne, Allineamento::Sinistra);
        let mut troppo_largo = 0usize;
        let intestazione = a_righe(intest, colonne, &mut troppo_largo);
        let righe = a_righe(corpo, colonne, &mut troppo_largo);
        if troppo_largo > 0 {
            self.avvisa(format!(
                "tabella: una cella con colspan {troppo_largo} supera le {colonne} colonne: ridotto a {colonne}"
            ));
        }
        Some(Tabella {
            colonne,
            allineamenti: al,
            intestazione,
            righe,
            etichetta: None,
        })
    }

    fn push_cella(
        &mut self,
        ex: ast::Expr<'a>,
        colspan: usize,
        intestazione: bool,
        intest: &mut Vec<Cella>,
        corpo: &mut Vec<Cella>,
    ) {
        let contenuto = self.espr_inline(ex);
        // `colspan` oltre il tetto delle colonne non ha senso (e, riportato
        // alla lettera in HTML, farebbe traboccare i conteggi): ridotto.
        let colspan = if colspan > COLONNE_MAX {
            self.avvisa(format!(
                "cella con colspan {colspan}: ridotto a {COLONNE_MAX} colonne"
            ));
            COLONNE_MAX
        } else {
            colspan
        };
        let c = Cella {
            contenuto,
            colspan: colspan.max(1),
        };
        if intestazione {
            intest.push(c);
        } else {
            corpo.push(c);
        }
    }

    fn celle_da(
        &mut self,
        e: ast::Expr<'a>,
        intestazione: bool,
        intest: &mut Vec<Cella>,
        corpo: &mut Vec<Cella>,
    ) {
        match e {
            ast::Expr::FuncCall(c) => {
                let nome = nome_callee(c.callee()).unwrap_or_default();
                match nome.as_str() {
                    "table.header" | "table.footer" => {
                        let e_header = nome == "table.header";
                        for x in posizionali(c.args()) {
                            self.celle_da(x, e_header, intest, corpo);
                        }
                    }
                    "table.cell" => {
                        let intero = |n: &str| {
                            nominato(c.args(), n)
                                .and_then(|e| match e {
                                    ast::Expr::Int(i) => Some(i.get().max(1) as usize),
                                    _ => None,
                                })
                                .unwrap_or(1)
                        };
                        let (colspan, rowspan) = (intero("colspan"), intero("rowspan"));
                        if rowspan > 1 {
                            self.avvisa(
                                "tabella: `rowspan` non rappresentato (cella su una sola riga)",
                            );
                        }
                        if let Some(x) = posizionali(c.args()).into_iter().next() {
                            self.push_cella(x, colspan, intestazione, intest, corpo);
                        }
                    }
                    "table.hline" | "table.vline" => {}
                    _ => self.push_cella(e, 1, intestazione, intest, corpo),
                }
            }
            ast::Expr::ContentBlock(_)
            | ast::Expr::Str(_)
            | ast::Expr::Int(_)
            | ast::Expr::Float(_) => self.push_cella(e, 1, intestazione, intest, corpo),
            _ => {
                self.avvisa("tabella: cella non valutabile (omessa)");
            }
        }
    }

    // ---- conclusione --------------------------------------------------

    fn concludi(mut self) -> Documento {
        let mut avvisi = std::mem::take(&mut self.avvisi);
        avvisi.estendi(std::mem::take(&mut self.math.avvisi));
        if !self.grafica_omessa.is_empty() {
            let mut nomi = self.grafica_omessa.clone();
            nomi.sort();
            nomi.dedup();
            avvisi.aggiungi(format!(
                "{} elementi grafici vettoriali omessi ({})",
                self.grafica_omessa.len(),
                nomi.join(", ")
            ));
        }
        if self.regole_ignorate > 0 {
            avvisi.aggiungi(format!(
                "{} regole di stile (#set/#show) non applicate: sono interpretati solo lingua, titolo, autore e numerazioni",
                self.regole_ignorate
            ));
        }
        let lingua = self.lingua.clone().unwrap_or_else(|| "en".to_string());
        Documento {
            titolo: self.titolo,
            autore: self.autore,
            lingua,
            blocchi: self.blocchi,
            etichette: self.etichette,
            numerazione_titoli: self.numerazione_titoli,
            supplemento_equazioni: self.supplemento_eq,
            avvisi: avvisi.into_vec(),
        }
    }
}

/// Distribuisce le celle in righe tenendo conto di `colspan`.
/// `troppo_largo` riceve il `colspan` massimo che eccedeva le colonne (0 se nessuno),
/// perché il chiamante possa avvisare della riduzione.
fn a_righe(celle: Vec<Cella>, colonne: usize, troppo_largo: &mut usize) -> Vec<Vec<Cella>> {
    let mut righe: Vec<Vec<Cella>> = Vec::new();
    let mut riga: Vec<Cella> = Vec::new();
    let mut occupate = 0usize;
    for mut c in celle {
        if c.colspan > colonne {
            *troppo_largo = (*troppo_largo).max(c.colspan);
        }
        let span = c.colspan.min(colonne).max(1);
        // lo span memorizzato non eccede mai le colonne della tabella
        c.colspan = span;
        if occupate + span > colonne && !riga.is_empty() {
            righe.push(std::mem::take(&mut riga));
            occupate = 0;
        }
        occupate += span;
        riga.push(c);
        if occupate >= colonne {
            righe.push(std::mem::take(&mut riga));
            occupate = 0;
        }
    }
    if !riga.is_empty() {
        righe.push(riga);
    }
    righe
}

// =======================================================================
// Normalizzazione e utilità sugli inline
// =======================================================================

/// Fonde i `Testo` adiacenti, comprime gli spazi e rimuove quelli ai bordi.
pub(crate) fn normalizza(v: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::with_capacity(v.len());
    for i in v {
        let i = match i {
            Inline::Enfasi(x) => Inline::Enfasi(normalizza(x)),
            Inline::Forte(x) => Inline::Forte(normalizza(x)),
            Inline::Barrato(x) => Inline::Barrato(normalizza(x)),
            Inline::Sottolineato(x) => Inline::Sottolineato(normalizza(x)),
            Inline::Apice(x) => Inline::Apice(normalizza(x)),
            Inline::Pedice(x) => Inline::Pedice(normalizza(x)),
            Inline::Virgolette(x) => Inline::Virgolette(normalizza(x)),
            altro => altro,
        };
        match (out.last_mut(), i) {
            (Some(Inline::Testo(a)), Inline::Testo(b)) => a.push_str(&b),
            (Some(Inline::Spazio), Inline::Spazio) => {}
            (Some(Inline::ACapo), Inline::Spazio) => {}
            (Some(Inline::Spazio), Inline::ACapo) => {
                out.pop();
                out.push(Inline::ACapo);
            }
            (_, i) => out.push(i),
        }
    }
    while matches!(out.first(), Some(Inline::Spazio) | Some(Inline::ACapo)) {
        out.remove(0);
    }
    while matches!(out.last(), Some(Inline::Spazio) | Some(Inline::ACapo)) {
        out.pop();
    }
    out
}

/// Appiattisce blocchi in una sequenza inline (per didascalie, celle, …).
pub(crate) fn appiattisci(blocchi: Vec<Blocco>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for b in blocchi {
        if !out.is_empty() {
            out.push(Inline::Spazio);
        }
        match b {
            Blocco::Paragrafo(v) => out.extend(v),
            Blocco::Titolo { contenuto, .. } => out.extend(contenuto),
            Blocco::Equazione { formula, .. } => out.push(Inline::Math(formula)),
            Blocco::Citazione { corpo, .. } => out.extend(appiattisci(corpo)),
            Blocco::Lista { voci, .. } => {
                for v in voci {
                    out.extend(appiattisci(v));
                    out.push(Inline::Spazio);
                }
            }
            Blocco::Immagine(i) => out.push(Inline::Immagine(i)),
            Blocco::Codice { testo, .. } => out.push(Inline::Codice(testo)),
            _ => {}
        }
    }
    out
}

fn trasforma_testo(v: &mut [Inline], f: &dyn Fn(&str) -> String) {
    for i in v {
        match i {
            Inline::Testo(t) => *t = f(t),
            Inline::Enfasi(x)
            | Inline::Forte(x)
            | Inline::Barrato(x)
            | Inline::Sottolineato(x)
            | Inline::Apice(x)
            | Inline::Pedice(x)
            | Inline::Virgolette(x) => trasforma_testo(x, f),
            _ => {}
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn numerazione_pattern() {
        assert_eq!(formatta_numerazione("1.1", &[2]), "2");
        assert_eq!(formatta_numerazione("1.1", &[2, 3]), "2.3");
        assert_eq!(formatta_numerazione("1.1", &[2, 3, 4]), "2.3.4");
        assert_eq!(formatta_numerazione("I", &[4]), "IV");
        assert_eq!(formatta_numerazione("(1)", &[7]), "(7)");
        assert_eq!(formatta_numerazione("a)", &[2]), "b)");
        assert_eq!(formatta_numerazione("1.", &[3]), "3.");
    }

    #[test]
    fn titoli_numerati_e_riferimenti() {
        let d = analizza(
            "#set text(lang: \"it\")\n#set heading(numbering: \"1.1\")\n= Uno <a>\n== Due <b>\nVedi @a e @b.\n",
        );
        assert_eq!(d.lingua, "it");
        assert_eq!(d.risolvi("a", None).as_deref(), Some("Sezione 1"));
        assert_eq!(d.risolvi("b", None).as_deref(), Some("Sezione 1.1"));
        assert!(d.risolvi("zzz", None).is_none());
    }

    #[test]
    fn figura_con_didascalia_ed_etichetta() {
        let d = analizza("#figure(image(\"a.png\"), caption: [Ciao]) <f>\n");
        assert!(matches!(d.blocchi[0], Blocco::Figura { .. }));
        assert_eq!(d.risolvi("f", None).as_deref(), Some("Figure 1"));
    }

    #[test]
    fn elenchi_raggruppati() {
        let d = analizza("- a\n- b\n\n+ uno\n+ due\n");
        assert!(
            matches!(&d.blocchi[0], Blocco::Lista { ordinata: false, voci, .. } if voci.len() == 2)
        );
        assert!(
            matches!(&d.blocchi[1], Blocco::Lista { ordinata: true, voci, .. } if voci.len() == 2)
        );
    }

    #[test]
    fn virgolette_intelligenti() {
        let d = analizza("Disse \"ciao\" e l'altro.");
        let Blocco::Paragrafo(v) = &d.blocchi[0] else {
            panic!()
        };
        let t = testo_semplice(v);
        assert_eq!(t, "Disse “ciao” e l’altro.");
    }

    #[test]
    fn funzione_utente_rende_gli_argomenti() {
        let d = analizza("#let cap(a, b) = a\n#cap(\"L\")[a diligenza]\n");
        let Blocco::Paragrafo(v) = &d.blocchi[0] else {
            panic!("{:?}", d.blocchi)
        };
        assert_eq!(testo_semplice(v), "La diligenza");
        assert!(d.avvisi.iter().any(|a| a.contains("cap")));
    }
}
