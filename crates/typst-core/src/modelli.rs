//! Modelli di documento (contratto «Strumenti nella barra laterale e Modelli»,
//! DECISIONI.md, 2026-10-16).
//!
//! Un modello è un file `.typ` che compila da solo: in testa ha righe di
//! intestazione che sono commenti Typst (`// @…`), poi il corpo con segnaposto
//! `{{nome}}`.
//!
//! ```text
//! // @modello: Lettera di richiesta
//! // @descrizione: Lettera formale di una pagina
//! // @pagine: 1
//! // @campo: destinatario | testo | Studio Bianchi
//! // @campo: data | data | oggi
//! ```
//!
//! # Regole
//! - Campo: `<nome> | <tipo> | <predefinito>`; nome `[a-z][a-z0-9_]*` (max 40),
//!   tipo `testo | data | numero | elenco`, predefinito facoltativo. Per il tipo
//!   `data`, `oggi` vale la data locale odierna in formato ISO `AAAA-MM-GG`.
//! - `{{nome}}` inserisce il valore **come testo** (caratteri speciali del
//!   markup preceduti da `\`); `{{!nome}}` lo inserisce **verbatim** (codice
//!   Typst). Il tipo `elenco` trasforma ogni riga non vuota in `- riga`.
//! - Segnaposto non dichiarato o campo mai usato: **avviso**, non errore.
//!   Un segnaposto non dichiarato resta nel testo così com'è.
//! - Valore assente → predefinito → stringa vuota. Un valore presente ma
//!   vuoto resta vuoto (l'utente ha svuotato il campo di proposito).
//! - Le righe `// @…` della testata non finiscono nel documento generato.
//!
//! # Scelte di design (oltre il contratto)
//! - **Escaping**: oltre ai caratteri del contratto (`\ # $ * _ @ < > ` ~ [ ] " '`)
//!   si preceda con `\` anche ciò che, a inizio riga, creerebbe struttura
//!   (`= `, `- `, `+ `, `/ `, `1. `) e la prima `/` di una coppia `//` (che
//!   aprirebbe un commento e cancellerebbe il resto della riga).
//! - **Predefiniti multi-riga**: nella riga `@campo` la sequenza `\n` vale a
//!   capo e `\\` vale `\` (serve per i predefiniti degli `elenco`).
//! - **Data**: `oggi` si risolve con `chrono::Local` (già nelle dipendenze).
//! - **Precedenza**: un modello utente con lo stesso id sostituisce l'integrato;
//!   eliminarlo ripristina l'integrato.
//!
//! # Sicurezza
//! Id validati (`[a-z0-9_-]`, 1..=60, nomi riservati Windows rifiutati), percorso
//! risolto e verificato dentro la cartella dei modelli (anche contro symlink),
//! scrittura atomica ([`crate::config::scrivi_atomico`]), tetto 256 KiB per
//! modello e 64 campi.
//!
//! Il modulo è puro: l'I/O avviene solo nelle funzioni che ricevono la cartella
//! come parametro; le varianti `*_sistema` usano
//! `dirs::config_dir()/typster/modelli/`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Tetto in byte di un modello (file utente e sorgente da salvare).
pub const MAX_BYTES_MODELLO: usize = 256 * 1024;
/// Numero massimo di campi per modello.
pub const MAX_CAMPI: usize = 64;
/// Lunghezza massima del nome di un campo.
pub const MAX_NOME_CAMPO: usize = 40;
/// Lunghezza massima di un id (e del nome file senza estensione).
pub const MAX_ID: usize = 60;
/// Lunghezza massima del nome visibile di un modello salvato.
const MAX_NOME_MODELLO: usize = 80;
/// Tetto del sorgente generato da [`compila_modello`].
const MAX_BYTES_GENERATO: usize = 4 * 1024 * 1024;
/// Byte esaminati dopo `{{` alla ricerca di `}}` (limita il costo a O(n)).
const FINESTRA_SEGNAPOSTO: usize = 256;
/// Numero massimo di file utente esaminati da [`elenca_modelli`].
const MAX_FILE_UTENTE: usize = 500;
/// Estensione dei file modello.
const ESTENSIONE: &str = "typ";
/// Sottocartella di `config_dir()/typster/` con i modelli utente.
const NOME_SOTTOCARTELLA: &str = "modelli";

/// Modelli integrati: `(id, sorgente)`, incorporati con `include_str!`.
const INTEGRATI: &[(&str, &str)] = &[
    ("lettera", include_str!("../modelli/lettera.typ")),
    (
        "scheda-prodotto",
        include_str!("../modelli/scheda-prodotto.typ"),
    ),
    (
        "relazione-breve",
        include_str!("../modelli/relazione-breve.typ"),
    ),
];

/// Tipo di un campo del modello. Serializzato in minuscolo
/// (`"testo" | "data" | "numero" | "elenco"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoCampo {
    /// Testo libero (escapato come markup).
    Testo,
    /// Data (il predefinito `oggi` si risolve nella data odierna ISO).
    Data,
    /// Numero (si avvisa se il valore non lo è).
    Numero,
    /// Elenco: una voce per riga non vuota, resa come `- voce`.
    Elenco,
}

impl TipoCampo {
    fn da_str(s: &str) -> Option<Self> {
        match s {
            "testo" => Some(Self::Testo),
            "data" => Some(Self::Data),
            "numero" => Some(Self::Numero),
            "elenco" => Some(Self::Elenco),
            _ => None,
        }
    }

    fn come_str(self) -> &'static str {
        match self {
            Self::Testo => "testo",
            Self::Data => "data",
            Self::Numero => "numero",
            Self::Elenco => "elenco",
        }
    }
}

/// Un campo dichiarato dal modello.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampoModello {
    /// Nome del campo (`[a-z][a-z0-9_]*`, max 40).
    pub nome: String,
    /// Tipo del campo.
    pub tipo: TipoCampo,
    /// Valore predefinito. In [`ModelloInfo`] è già risolto (`oggi` → data
    /// ISO) e con i `\n` decodificati; in ingresso a [`salva_come_modello`] può
    /// valere `oggi` (tipo `data`) e può contenere a capo reali.
    #[serde(default)]
    pub predefinito: String,
}

/// Provenienza di un modello. Serializzata in minuscolo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrigineModello {
    /// Incorporato nel programma.
    Integrato,
    /// File `.typ` nella cartella dei modelli dell'utente.
    Utente,
}

/// Scheda di un modello per l'elenco del frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelloInfo {
    /// Id = nome file senza estensione.
    pub id: String,
    /// Nome visibile (`@modello`, ripiego: l'id).
    pub nome: String,
    /// Descrizione (`@descrizione`, può essere vuota).
    pub descrizione: String,
    /// Numero di pagine dichiarato (`@pagine`), `null` se non indicato.
    pub pagine: Option<u32>,
    /// Campi nell'ordine di dichiarazione.
    pub campi: Vec<CampoModello>,
    /// Integrato o utente.
    pub origine: OrigineModello,
    /// Avvisi di parsing/coerenza (righe mal formate, segnaposto orfani…).
    pub avvisi: Vec<String>,
}

/// Esito di [`compila_modello`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RisultatoModello {
    /// Sorgente Typst generato (senza righe di testata).
    pub sorgente: String,
    /// Avvisi non bloccanti.
    pub avvisi: Vec<String>,
}

/// Errori delle operazioni sui modelli.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroreModello {
    /// Id non ammesso (motivo nel testo).
    IdNonValido(String),
    /// Nessun modello con questo id.
    NonTrovato(String),
    /// Tentativo di eliminare un modello integrato.
    IntegratoNonEliminabile(String),
    /// Dati in ingresso non validi (nome, campi, sorgente…).
    DatiNonValidi(String),
    /// Esiste già un modello **utente** con questo id e non è stata chiesta la
    /// sovrascrittura. Il messaggio inizia con «Esiste già un modello»: il
    /// frontend lo riconosce dal prefisso e chiede conferma.
    GiaEsistente(String),
    /// Superato un tetto (dimensione o numero di campi).
    Limite(String),
    /// Errore di I/O o percorso fuori dalla cartella dei modelli.
    Io(String),
}

impl fmt::Display for ErroreModello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdNonValido(m) => write!(f, "Identificativo del modello non valido: {m}"),
            Self::NonTrovato(id) => write!(f, "Modello «{id}» non trovato"),
            Self::IntegratoNonEliminabile(id) => {
                write!(f, "Il modello «{id}» è integrato e non si può eliminare")
            }
            Self::DatiNonValidi(m) => write!(f, "Dati del modello non validi: {m}"),
            Self::GiaEsistente(id) => write!(
                f,
                "Esiste già un modello «{id}»: conferma per sovrascriverlo"
            ),
            Self::Limite(m) => write!(f, "Limite superato: {m}"),
            Self::Io(m) => write!(f, "Errore sui modelli: {m}"),
        }
    }
}

impl std::error::Error for ErroreModello {}

// ---------------------------------------------------------------------------
// Validazione
// ---------------------------------------------------------------------------

/// Nomi riservati di Windows (confronto sull'id intero, già minuscolo).
fn nome_riservato_windows(id: &str) -> bool {
    if matches!(id, "con" | "prn" | "aux" | "nul") {
        return true;
    }
    for prefisso in ["com", "lpt"] {
        if let Some(n) = id.strip_prefix(prefisso) {
            if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() {
                return true;
            }
        }
    }
    false
}

/// Verifica un id: solo `[a-z0-9_-]`, 1..=[`MAX_ID`] caratteri, niente nomi
/// riservati di Windows. Rifiuta di fatto `..`, separatori e percorsi.
pub fn valida_id(id: &str) -> Result<(), ErroreModello> {
    let errore = |m: &str| Err(ErroreModello::IdNonValido(m.to_string()));
    if id.is_empty() {
        return errore("vuoto");
    }
    if id.len() > MAX_ID {
        return errore("troppo lungo (max 60 caratteri)");
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return errore("sono ammessi solo a-z, 0-9, «_» e «-»");
    }
    if nome_riservato_windows(id) {
        return errore("nome riservato dal sistema");
    }
    Ok(())
}

fn nome_campo_valido(nome: &str) -> bool {
    let mut it = nome.bytes();
    nome.len() <= MAX_NOME_CAMPO
        && it.next().is_some_and(|b| b.is_ascii_lowercase())
        && it.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Ricava un id dal nome visibile (minuscole, accenti italiani semplificati,
/// il resto → `-`, troncato a [`MAX_ID`]). Errore se non resta nulla di valido.
pub fn id_da_nome(nome: &str) -> Result<String, ErroreModello> {
    let mut id = String::new();
    let mut ultimo_trattino = true; // evita trattino iniziale e doppi
    for c in nome.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            altro => altro,
        };
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
            id.push(c);
            ultimo_trattino = false;
        } else if !ultimo_trattino {
            id.push('-');
            ultimo_trattino = true;
        }
    }
    let mut id = id.trim_end_matches('-').to_string();
    id.truncate(MAX_ID); // ASCII: nessun rischio di tagliare un carattere
    let id = id.trim_end_matches('-').to_string();
    if id.is_empty() {
        return Err(ErroreModello::DatiNonValidi(
            "il nome non contiene lettere o cifre utilizzabili per l'identificativo".into(),
        ));
    }
    valida_id(&id)?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// Escaping
// ---------------------------------------------------------------------------

/// Caratteri che il markup Typst interpreta ovunque (contratto).
fn carattere_speciale(c: char) -> bool {
    matches!(
        c,
        '\\' | '#' | '$' | '*' | '_' | '@' | '<' | '>' | '`' | '~' | '[' | ']' | '"' | '\''
    )
}

/// Normalizza a `\n` tutti i terminatori di riga che Typst considera a capo:
/// `\r\n`, `\r`, VT (`U+000B`), FF (`U+000C`), NEL (`U+0085`), LS (`U+2028`) e
/// PS (`U+2029`). Senza, un valore con `U+2028` seguito da `= Titolo` creerebbe
/// un titolo pur non contenendo `\n` per il nostro `split`.
fn normalizza_a_capo(testo: &str) -> String {
    let mut out = String::with_capacity(testo.len());
    let mut it = testo.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push('\n');
            }
            '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}' => out.push('\n'),
            altro => out.push(altro),
        }
    }
    out
}

/// Rende `testo` sicuro come **testo** in markup Typst: ogni carattere
/// speciale è preceduto da `\`; a inizio riga si escapano anche i marcatori di
/// struttura (`=`, `-`, `+`, `/` seguiti da spazio/fine riga, `1.`), e la
/// prima `/` di `//`. Gli a capo (anche Unicode, vedi [`normalizza_a_capo`])
/// sono normalizzati a `\n`. Una sequenza `==`, `===`… seguita da spazio o
/// fine riga è un titolo: se ne escapa il primo `=`.
pub fn escapa_markup(testo: &str) -> String {
    let norm = normalizza_a_capo(testo);
    let mut out = String::with_capacity(norm.len() + 8);
    for (n, riga) in norm.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let c: Vec<char> = riga.chars().collect();
        let primo = c.iter().position(|ch| !ch.is_whitespace());
        // Posizione del punto di un marcatore `1.` a inizio riga.
        let punto_enumerato = primo.and_then(|p| {
            let cifre = c[p..].iter().take_while(|ch| ch.is_ascii_digit()).count();
            let dopo = p + cifre;
            let a_capo_o_spazio = c.get(dopo + 1).is_none_or(|ch| ch.is_whitespace());
            (cifre > 0 && c.get(dopo) == Some(&'.') && a_capo_o_spazio).then_some(dopo)
        });
        // Titolo `=`, `==`, `===`…: si escapa il primo `=` se dopo l'intera
        // sequenza c'è spazio o fine riga.
        let titolo = primo.is_some_and(|p| {
            c[p] == '='
                && c[p..]
                    .iter()
                    .find(|&&ch| ch != '=')
                    .is_none_or(|ch| ch.is_whitespace())
        });
        for (i, &ch) in c.iter().enumerate() {
            let a_inizio = primo == Some(i);
            let segue_spazio = c.get(i + 1).is_none_or(|x| x.is_whitespace());
            let da_escapare = carattere_speciale(ch)
                || (a_inizio && ch == '=' && titolo)
                || (a_inizio && matches!(ch, '-' | '+' | '/') && segue_spazio)
                || (ch == '/' && c.get(i + 1) == Some(&'/'))
                || punto_enumerato == Some(i);
            if da_escapare {
                out.push('\\');
            }
            out.push(ch);
        }
    }
    out
}

/// Resa di un `elenco` per `{{nome}}`: `- voce` per ogni riga non vuota.
fn rendi_elenco(valore: &str) -> String {
    normalizza_a_capo(valore)
        .split('\n')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(|r| format!("- {}", escapa_markup(r)))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Risultato dell'analisi di un file modello.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Analisi {
    nome: Option<String>,
    descrizione: String,
    pagine: Option<u32>,
    /// Campi con predefinito **grezzo** (decodificato ma `oggi` non risolto).
    campi: Vec<CampoModello>,
    corpo: String,
    avvisi: Vec<String>,
}

/// Decodifica il predefinito della riga `@campo`: `\n` → a capo, `\\` → `\`.
fn decodifica_predefinito(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.peek() {
                Some('n') => {
                    out.push('\n');
                    it.next();
                }
                Some('\\') => {
                    out.push('\\');
                    it.next();
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Inverso di [`decodifica_predefinito`] per la scrittura della testata.
fn codifica_predefinito(s: &str) -> String {
    normalizza_a_capo(s)
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
}

/// Se la riga (già `trim`) è `// @…`, restituisce ciò che segue la `@`.
fn direttiva(riga: &str) -> Option<&str> {
    riga.strip_prefix("//")?.trim_start().strip_prefix('@')
}

/// Estrae il nome da `nome` o `!nome` (spazi interni ammessi).
fn nome_segnaposto(interno: &str) -> Option<(bool, &str)> {
    let t = interno.trim();
    let (verbatim, nome) = match t.strip_prefix('!') {
        Some(resto) => (true, resto.trim_start()),
        None => (false, t),
    };
    nome_campo_valido(nome).then_some((verbatim, nome))
}

/// Scorre il corpo sostituendo i segnaposto: per ciascuno chiama `f(nome,
/// verbatim)`; `None` lascia il testo originale. Il valore sostituito non
/// viene riesaminato.
///
/// `limite` è il tetto in byte dell'output: la lunghezza prevista è controllata
/// **prima** di ogni `push_str`, quindi nulla supera mai `limite` e si risponde
/// [`ErroreModello::Limite`] senza allocare oltre. La chiusura restituisce un
/// riferimento a un valore già pronto (reso una sola volta per campo).
///
/// Il testo tra `{{` e `}}` è cercato solo nei primi [`FINESTRA_SEGNAPOSTO`]
/// byte: un nome valido (max 40) più `!` e spazi sta largamente dentro, e così
/// una sequenza di `{{` senza chiusura costa O(n), non O(n²).
fn sostituisci_segnaposto<'v>(
    corpo: &str,
    limite: usize,
    mut f: impl FnMut(&str, bool) -> Option<&'v str>,
) -> Result<String, ErroreModello> {
    let troppo_grande = || {
        ErroreModello::Limite(format!(
            "il documento generato supera i {} MiB",
            limite >> 20
        ))
    };
    let mut out = String::with_capacity(corpo.len().min(limite));
    let mut resto = corpo;
    while let Some(i) = resto.find("{{") {
        if out.len() + i > limite {
            return Err(troppo_grande());
        }
        out.push_str(&resto[..i]);
        let dopo = &resto[i + 2..];
        let finestra = &dopo[..dopo.floor_char_boundary(FINESTRA_SEGNAPOSTO)];
        if let Some(fine) = finestra.find("}}") {
            if let Some((verbatim, nome)) = nome_segnaposto(&dopo[..fine]) {
                let originale = &resto[i..i + 2 + fine + 2];
                let testo = f(nome, verbatim).unwrap_or(originale);
                if out.len() + testo.len() > limite {
                    return Err(troppo_grande());
                }
                out.push_str(testo);
                resto = &dopo[fine + 2..];
                continue;
            }
        }
        // Non è un segnaposto: emetti una sola «{» e riprova dalla successiva
        // (così `{{{x}}}` trova comunque `{{x}}`).
        if out.len() + 1 > limite {
            return Err(troppo_grande());
        }
        out.push('{');
        resto = &resto[i + 1..];
    }
    if out.len() + resto.len() > limite {
        return Err(troppo_grande());
    }
    out.push_str(resto);
    Ok(out)
}

/// Analizza testata e corpo; non fallisce mai: i problemi diventano avvisi.
fn analizza(testo: &str) -> Analisi {
    let testo = testo.strip_prefix('\u{feff}').unwrap_or(testo);
    let mut a = Analisi {
        nome: None,
        descrizione: String::new(),
        pagine: None,
        campi: Vec::new(),
        corpo: String::with_capacity(testo.len()),
        avvisi: Vec::new(),
    };
    let mut in_testata = true;
    let mut descrizione_vista = false;

    for (n, riga) in testo.split_inclusive('\n').enumerate() {
        let num = n + 1;
        if in_testata {
            let t = riga.trim();
            if let Some(dir) = direttiva(t) {
                applica_direttiva(&mut a, &mut descrizione_vista, num, dir);
                continue;
            }
            if !(t.is_empty() || t.starts_with("//")) {
                in_testata = false;
            }
        }
        a.corpo.push_str(riga);
    }

    // Coerenza segnaposto <-> campi.
    let dichiarati: BTreeSet<String> = a.campi.iter().map(|c| c.nome.clone()).collect();
    let mut usati = BTreeSet::new();
    let mut orfani = BTreeSet::new();
    // Nessun tetto: non si copia nulla oltre il corpo stesso (già limitato).
    let _ = sostituisci_segnaposto(&a.corpo, usize::MAX, |nome, _| {
        if dichiarati.contains(nome) {
            usati.insert(nome.to_string());
        } else {
            orfani.insert(nome.to_string());
        }
        None
    });
    for nome in orfani {
        a.avvisi.push(format!(
            "segnaposto «{{{{{nome}}}}}» senza campo dichiarato"
        ));
    }
    for c in &a.campi {
        if !usati.contains(&c.nome) {
            a.avvisi
                .push(format!("campo «{}» dichiarato ma mai usato", c.nome));
        }
    }
    a
}

fn applica_direttiva(a: &mut Analisi, descrizione_vista: &mut bool, num: usize, dir: &str) {
    let Some((chiave, valore)) = dir.split_once(':') else {
        a.avvisi.push(format!(
            "riga {num}: intestazione «@{}» senza «:»",
            dir.trim()
        ));
        return;
    };
    let chiave = chiave.trim().to_lowercase();
    let valore = valore.trim();
    match chiave.as_str() {
        "modello" => {
            if a.nome.is_some() {
                a.avvisi
                    .push(format!("riga {num}: «@modello» ripetuto, ignorato"));
            } else if valore.is_empty() {
                a.avvisi
                    .push(format!("riga {num}: «@modello» vuoto, ignorato"));
            } else {
                a.nome = Some(valore.to_string());
            }
        }
        "descrizione" => {
            if *descrizione_vista {
                a.avvisi
                    .push(format!("riga {num}: «@descrizione» ripetuta, ignorata"));
            } else {
                *descrizione_vista = true;
                a.descrizione = valore.to_string();
            }
        }
        "pagine" => match valore.parse::<u32>() {
            Ok(p) if (1..=9999).contains(&p) => {
                if a.pagine.is_some() {
                    a.avvisi
                        .push(format!("riga {num}: «@pagine» ripetuto, ignorato"));
                } else {
                    a.pagine = Some(p);
                }
            }
            _ => a.avvisi.push(format!(
                "riga {num}: «@pagine» non è un numero valido ({valore:?}), ignorato"
            )),
        },
        "campo" => applica_campo(a, num, valore),
        altro => a
            .avvisi
            .push(format!("riga {num}: intestazione «@{altro}» sconosciuta")),
    }
}

fn applica_campo(a: &mut Analisi, num: usize, valore: &str) {
    let mut parti = valore.splitn(3, '|');
    let nome = parti.next().unwrap_or("").trim();
    let tipo = parti.next().map(str::trim);
    let predefinito = parti.next().map(str::trim).unwrap_or("");

    if !nome_campo_valido(nome) {
        a.avvisi.push(format!(
            "riga {num}: nome di campo non valido ({nome:?}); atteso [a-z][a-z0-9_]* (max 40)"
        ));
        return;
    }
    let Some(tipo) = tipo.filter(|t| !t.is_empty()) else {
        a.avvisi
            .push(format!("riga {num}: campo «{nome}» senza tipo, ignorato"));
        return;
    };
    let Some(tipo) = TipoCampo::da_str(tipo) else {
        a.avvisi.push(format!(
            "riga {num}: tipo «{tipo}» sconosciuto per il campo «{nome}» (testo|data|numero|elenco), ignorato"
        ));
        return;
    };
    if a.campi.iter().any(|c| c.nome == nome) {
        a.avvisi.push(format!(
            "riga {num}: campo «{nome}» dichiarato due volte, ignorato"
        ));
        return;
    }
    if a.campi.len() >= MAX_CAMPI {
        a.avvisi.push(format!(
            "riga {num}: oltre {MAX_CAMPI} campi, «{nome}» ignorato"
        ));
        return;
    }
    let predefinito = decodifica_predefinito(predefinito);
    if tipo == TipoCampo::Numero && !predefinito.is_empty() && !e_numero(&predefinito) {
        a.avvisi.push(format!(
            "riga {num}: il predefinito di «{nome}» non è un numero ({predefinito:?})"
        ));
    }
    a.campi.push(CampoModello {
        nome: nome.to_string(),
        tipo,
        predefinito,
    });
}

/// Vero se `s` è un numero (solo per l'avviso: il valore resta testo). Accetta
/// anche la virgola decimale italiana (`12,5`): una sola virgola e nessun punto.
fn e_numero(s: &str) -> bool {
    let t = s.trim();
    let n = if t.matches(',').count() == 1 && !t.contains('.') {
        t.replace(',', ".")
    } else {
        t.to_string()
    };
    n.parse::<f64>().is_ok_and(f64::is_finite)
}

// ---------------------------------------------------------------------------
// Compilazione (sostituzione)
// ---------------------------------------------------------------------------

/// Data locale odierna `AAAA-MM-GG`.
fn oggi_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Predefinito risolto (`oggi` per i campi `data`).
fn predefinito_risolto(c: &CampoModello, oggi: &str) -> String {
    if c.tipo == TipoCampo::Data && c.predefinito.trim() == "oggi" {
        oggi.to_string()
    } else {
        c.predefinito.clone()
    }
}

fn compila_analisi(
    analisi: &Analisi,
    valori: &BTreeMap<String, String>,
    oggi: &str,
) -> Result<RisultatoModello, ErroreModello> {
    let mut avvisi = analisi.avvisi.clone();

    // Valori risolti: assente → predefinito → "".
    let mut risolti: BTreeMap<&str, (&CampoModello, String)> = BTreeMap::new();
    for c in &analisi.campi {
        let v = match valori.get(&c.nome) {
            Some(v) => v.clone(),
            None => predefinito_risolto(c, oggi),
        };
        if v.len() > MAX_BYTES_MODELLO {
            return Err(ErroreModello::Limite(format!(
                "il valore di «{}» supera i 256 KiB",
                c.nome
            )));
        }
        if c.tipo == TipoCampo::Numero && !v.trim().is_empty() && !e_numero(&v) {
            avvisi.push(format!("il valore di «{}» non è un numero", c.nome));
        }
        risolti.insert(c.nome.as_str(), (c, v));
    }
    for chiave in valori.keys() {
        if !risolti.contains_key(chiave.as_str()) {
            avvisi.push(format!(
                "valore per il campo sconosciuto «{chiave}», ignorato"
            ));
        }
    }

    // Resa di ogni campo calcolata UNA volta (non a ogni occorrenza); il
    // verbatim usa direttamente il valore.
    let rese: BTreeMap<&str, String> = risolti
        .iter()
        .map(|(&nome, (campo, valore))| {
            let resa = if campo.tipo == TipoCampo::Elenco {
                rendi_elenco(valore)
            } else {
                escapa_markup(valore)
            };
            (nome, resa)
        })
        .collect();

    let sorgente = sostituisci_segnaposto(&analisi.corpo, MAX_BYTES_GENERATO, |nome, verbatim| {
        let (_, valore) = risolti.get(nome)?;
        if verbatim {
            Some(valore.as_str())
        } else {
            rese.get(nome).map(String::as_str)
        }
    })?;
    Ok(RisultatoModello { sorgente, avvisi })
}

fn info_da_analisi(id: &str, mut a: Analisi, origine: OrigineModello, oggi: &str) -> ModelloInfo {
    if a.nome.is_none() {
        a.avvisi
            .push("manca «@modello»: si usa l'identificativo come nome".into());
    }
    let campi = a
        .campi
        .iter()
        .map(|c| CampoModello {
            nome: c.nome.clone(),
            tipo: c.tipo,
            predefinito: predefinito_risolto(c, oggi),
        })
        .collect();
    ModelloInfo {
        id: id.to_string(),
        nome: a.nome.unwrap_or_else(|| id.to_string()),
        descrizione: a.descrizione,
        pagine: a.pagine,
        campi,
        origine,
        avvisi: a.avvisi,
    }
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

/// Cartella dei modelli utente: `dirs::config_dir()/typster/modelli`.
pub fn cartella_modelli() -> Option<PathBuf> {
    dirs::config_dir().map(|b| b.join(crate::NOME_CARTELLA_DATI).join(NOME_SOTTOCARTELLA))
}

fn io(contesto: &str, e: std::io::Error) -> ErroreModello {
    ErroreModello::Io(format!("{contesto}: {e}"))
}

/// Percorso `<cartella>/<id>.typ` (id validato).
fn percorso_modello(cartella: &Path, id: &str) -> Result<PathBuf, ErroreModello> {
    valida_id(id)?;
    Ok(cartella.join(format!("{id}.{ESTENSIONE}")))
}

/// Verifica che `percorso` (esistente) risolva dentro `cartella`.
fn verifica_dentro(cartella: &Path, percorso: &Path) -> Result<PathBuf, ErroreModello> {
    let base = std::fs::canonicalize(cartella).map_err(|e| io("cartella dei modelli", e))?;
    let reale = std::fs::canonicalize(percorso).map_err(|e| io("percorso del modello", e))?;
    if reale.parent() != Some(base.as_path()) {
        return Err(ErroreModello::Io(
            "il file del modello risolve fuori dalla cartella dei modelli".into(),
        ));
    }
    Ok(reale)
}

/// Legge il modello utente `id`; `Ok(None)` se il file non esiste.
fn leggi_utente(cartella: &Path, id: &str) -> Result<Option<String>, ErroreModello> {
    let percorso = percorso_modello(cartella, id)?;
    match std::fs::symlink_metadata(&percorso) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io("lettura del modello", e)),
    }
    let reale = verifica_dentro(cartella, &percorso)?;
    let file = std::fs::File::open(&reale).map_err(|e| io("apertura del modello", e))?;
    if !file
        .metadata()
        .map_err(|e| io("lettura del modello", e))?
        .is_file()
    {
        return Err(ErroreModello::Io("il modello non è un file".into()));
    }
    let mut buf = Vec::new();
    file.take(MAX_BYTES_MODELLO as u64 + 1)
        .read_to_end(&mut buf)
        .map_err(|e| io("lettura del modello", e))?;
    if buf.len() > MAX_BYTES_MODELLO {
        return Err(ErroreModello::Limite(format!(
            "il modello «{id}» supera i 256 KiB"
        )));
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|_| ErroreModello::DatiNonValidi(format!("il modello «{id}» non è UTF-8")))
}

fn integrato(id: &str) -> Option<&'static str> {
    INTEGRATI.iter().find(|(i, _)| *i == id).map(|(_, s)| *s)
}

/// Frase che spiega come ripristinare l'integrato nascosto da un file guasto.
const RIMEDIO_GUASTO: &str =
    "il file utente nasconde il modello integrato: eliminalo per ripristinarlo";

/// Scheda di un file utente guasto che nasconde un integrato.
fn info_guasto(id: &str, e: &ErroreModello) -> ModelloInfo {
    ModelloInfo {
        id: id.to_string(),
        nome: id.to_string(),
        descrizione: String::new(),
        pagine: None,
        campi: Vec::new(),
        origine: OrigineModello::Utente,
        avvisi: vec![format!("file non utilizzabile ({e}); {RIMEDIO_GUASTO}")],
    }
}

/// Aggiunge il rimedio al messaggio di un errore di lettura di un file utente
/// che ha l'id di un integrato (stessa variante, testo esteso).
fn con_rimedio(e: ErroreModello) -> ErroreModello {
    let s = |m: String| format!("{m}; {RIMEDIO_GUASTO}");
    match e {
        ErroreModello::Limite(m) => ErroreModello::Limite(s(m)),
        ErroreModello::DatiNonValidi(m) => ErroreModello::DatiNonValidi(s(m)),
        ErroreModello::Io(m) => ErroreModello::Io(s(m)),
        altro => altro,
    }
}

fn elenca_con(cartella: Option<&Path>, oggi: &str) -> Vec<ModelloInfo> {
    let mut mappa: BTreeMap<String, ModelloInfo> = INTEGRATI
        .iter()
        .map(|(id, src)| {
            (
                id.to_string(),
                info_da_analisi(id, analizza(src), OrigineModello::Integrato, oggi),
            )
        })
        .collect();

    if let Some(dir) = cartella {
        if let Ok(voci) = std::fs::read_dir(dir) {
            // Il tetto conta solo i `.typ`: file di altro tipo non lo consumano.
            let file_typ = voci.flatten().map(|v| v.path()).filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(ESTENSIONE))
            });
            for p in file_typ.take(MAX_FILE_UTENTE) {
                let Some(id) = p.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if valida_id(id).is_err() {
                    continue;
                }
                match leggi_utente(dir, id) {
                    Ok(Some(testo)) => {
                        mappa.insert(
                            id.to_string(),
                            info_da_analisi(id, analizza(&testo), OrigineModello::Utente, oggi),
                        );
                    }
                    // File guasto (troppo grande, non UTF-8, fuori cartella…)
                    // con l'id di un integrato: `compila` fallirebbe, quindi
                    // l'elenco lo mostra com'è (utente, senza campi, con
                    // avviso) e l'UI offre «Elimina» per ripristinare
                    // l'integrato. Con id non integrato il file è saltato.
                    Err(e) if integrato(id).is_some() => {
                        mappa.insert(id.to_string(), info_guasto(id, &e));
                    }
                    _ => {}
                }
            }
        }
    }

    let mut v: Vec<ModelloInfo> = mappa.into_values().collect();
    v.sort_by(|a, b| {
        a.nome
            .to_lowercase()
            .cmp(&b.nome.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    v
}

fn compila_con(
    cartella: Option<&Path>,
    id: &str,
    valori: &BTreeMap<String, String>,
    oggi: &str,
) -> Result<RisultatoModello, ErroreModello> {
    valida_id(id)?;
    let utente = match cartella {
        Some(dir) => leggi_utente(dir, id).map_err(|e| {
            if integrato(id).is_some() {
                con_rimedio(e)
            } else {
                e
            }
        })?,
        None => None,
    };
    let testo = match utente {
        Some(t) => t,
        None => integrato(id)
            .ok_or_else(|| ErroreModello::NonTrovato(id.to_string()))?
            .to_string(),
    };
    compila_analisi(&analizza(&testo), valori, oggi)
}

/// Elenca i modelli: integrati più quelli in `cartella`. Un modello utente con
/// lo stesso id sostituisce l'integrato. File illeggibili, troppo grandi o con
/// id non valido sono saltati. Ordinati per nome (senza distinguere maiuscole).
pub fn elenca_modelli(cartella: &Path) -> Vec<ModelloInfo> {
    elenca_con(Some(cartella), &oggi_iso())
}

/// Come [`elenca_modelli`] sulla cartella di sistema (solo integrati se
/// non determinabile).
pub fn elenca_modelli_sistema() -> Vec<ModelloInfo> {
    elenca_con(cartella_modelli().as_deref(), &oggi_iso())
}

/// Genera il sorgente del modello `id` con i `valori` (campo → testo).
///
/// # Errori
/// Id non valido o inesistente, file illeggibile/oltre tetto, documento
/// generato oltre 4 MiB.
pub fn compila_modello(
    cartella: &Path,
    id: &str,
    valori: &BTreeMap<String, String>,
) -> Result<RisultatoModello, ErroreModello> {
    compila_con(Some(cartella), id, valori, &oggi_iso())
}

/// Come [`compila_modello`] sulla cartella di sistema.
pub fn compila_modello_sistema(
    id: &str,
    valori: &BTreeMap<String, String>,
) -> Result<RisultatoModello, ErroreModello> {
    compila_con(cartella_modelli().as_deref(), id, valori, &oggi_iso())
}

/// Salva `sorgente` (con segnaposto `{{campo}}`) come modello utente e ne
/// restituisce l'id (ricavato da `nome`). Stesso id di un integrato: il modello
/// utente lo sostituisce senza conferma. Stesso id di un modello **utente**
/// esistente: [`ErroreModello::GiaEsistente`] salvo `sovrascrivi == true`.
/// Eventuali righe `// @…` in testa a `sorgente` sono scartate: la testata è
/// rigenerata da `nome`, `descrizione`, `pagine` e `campi`. Scrittura atomica.
///
/// # Errori
/// Nome/campi/sorgente non validi (anche corpo vuoto dopo aver tolto la
/// testata), modello utente già esistente senza `sovrascrivi`, tetti superati,
/// percorso fuori cartella, I/O.
pub fn salva_come_modello(
    cartella: &Path,
    nome: &str,
    sorgente: &str,
    campi: &[CampoModello],
    descrizione: &str,
    pagine: Option<u32>,
    sovrascrivi: bool,
) -> Result<String, ErroreModello> {
    let nome = nome.trim();
    let invalido = |m: &str| Err(ErroreModello::DatiNonValidi(m.to_string()));
    if nome.is_empty() || nome.chars().count() > MAX_NOME_MODELLO {
        return invalido("il nome deve avere da 1 a 80 caratteri");
    }
    if nome.chars().any(char::is_control) {
        return invalido("il nome non può contenere caratteri di controllo");
    }
    if let Some(p) = pagine {
        if !(1..=9999).contains(&p) {
            return invalido("le pagine devono essere tra 1 e 9999");
        }
    }
    if campi.len() > MAX_CAMPI {
        return Err(ErroreModello::Limite(format!("più di {MAX_CAMPI} campi")));
    }
    let mut visti = BTreeSet::new();
    for c in campi {
        if !nome_campo_valido(&c.nome) {
            return Err(ErroreModello::DatiNonValidi(format!(
                "nome di campo non valido: {:?}",
                c.nome
            )));
        }
        if !visti.insert(c.nome.as_str()) {
            return Err(ErroreModello::DatiNonValidi(format!(
                "campo «{}» duplicato",
                c.nome
            )));
        }
    }
    let id = id_da_nome(nome)?;

    // Testata rigenerata; il corpo è il sorgente senza eventuale testata.
    let corpo = analizza(sorgente).corpo;
    // Il corpo (non il sorgente): righe di sola testata non fanno un modello.
    if corpo.trim().is_empty() {
        return invalido("il sorgente è vuoto (la testata «// @…» non conta)");
    }
    let descrizione: String = descrizione
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let mut contenuto = format!("// @modello: {nome}\n");
    if !descrizione.trim().is_empty() {
        contenuto.push_str(&format!("// @descrizione: {}\n", descrizione.trim()));
    }
    if let Some(p) = pagine {
        contenuto.push_str(&format!("// @pagine: {p}\n"));
    }
    for c in campi {
        let pred = codifica_predefinito(&c.predefinito);
        contenuto.push_str(&format!(
            "// @campo: {} | {} | {}\n",
            c.nome,
            c.tipo.come_str(),
            pred.trim()
        ));
    }
    contenuto.push('\n');
    contenuto.push_str(corpo.trim_start_matches(['\r', '\n']));
    if !contenuto.ends_with('\n') {
        contenuto.push('\n');
    }
    if contenuto.len() > MAX_BYTES_MODELLO {
        return Err(ErroreModello::Limite("il modello supera i 256 KiB".into()));
    }

    valida_id(&id)?;
    std::fs::create_dir_all(cartella).map_err(|e| io("creazione della cartella dei modelli", e))?;
    // Da qui si usa solo il percorso canonico (esiste: l'abbiamo appena creata).
    let cartella = std::fs::canonicalize(cartella).map_err(|e| io("cartella dei modelli", e))?;
    let percorso = percorso_modello(&cartella, &id)?;
    // Se esiste già (anche come symlink) deve risolvere dentro la cartella:
    // `scrivi_atomico` segue i collegamenti e scriverebbe fuori.
    if std::fs::symlink_metadata(&percorso).is_ok() {
        let reale = verifica_dentro(&cartella, &percorso)?;
        if !reale.is_file() {
            return Err(ErroreModello::Io("il bersaglio non è un file".into()));
        }
        // Esiste un modello UTENTE con questo id: niente sovrascrittura muta.
        if !sovrascrivi {
            return Err(ErroreModello::GiaEsistente(id));
        }
    }
    crate::config::scrivi_atomico(&percorso, contenuto.as_bytes())
        .map_err(|e| io("scrittura del modello", e))?;
    Ok(id)
}

/// Come [`salva_come_modello`] sulla cartella di sistema.
pub fn salva_come_modello_sistema(
    nome: &str,
    sorgente: &str,
    campi: &[CampoModello],
    descrizione: &str,
    pagine: Option<u32>,
    sovrascrivi: bool,
) -> Result<String, ErroreModello> {
    let cartella = cartella_modelli()
        .ok_or_else(|| ErroreModello::Io("cartella di configurazione non disponibile".into()))?;
    salva_come_modello(
        &cartella,
        nome,
        sorgente,
        campi,
        descrizione,
        pagine,
        sovrascrivi,
    )
}

/// Elimina il modello **utente** `id`. Se non esiste un file utente ma l'id è
/// di un integrato: [`ErroreModello::IntegratoNonEliminabile`].
pub fn elimina_modello(cartella: &Path, id: &str) -> Result<(), ErroreModello> {
    let percorso = percorso_modello(cartella, id)?;
    match std::fs::symlink_metadata(&percorso) {
        Ok(meta) => {
            // Se è un collegamento si rimuove il collegamento, non il bersaglio;
            // un file regolare deve comunque stare nella cartella.
            if !meta.file_type().is_symlink() {
                verifica_dentro(cartella, &percorso)?;
            }
            std::fs::remove_file(&percorso).map_err(|e| io("eliminazione del modello", e))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if integrato(id).is_some() {
                Err(ErroreModello::IntegratoNonEliminabile(id.to_string()))
            } else {
                Err(ErroreModello::NonTrovato(id.to_string()))
            }
        }
        Err(e) => Err(io("lettura del modello", e)),
    }
}

/// Come [`elimina_modello`] sulla cartella di sistema.
pub fn elimina_modello_sistema(id: &str) -> Result<(), ErroreModello> {
    match cartella_modelli() {
        Some(dir) => elimina_modello(&dir, id),
        None => {
            valida_id(id)?;
            if integrato(id).is_some() {
                Err(ErroreModello::IntegratoNonEliminabile(id.to_string()))
            } else {
                Err(ErroreModello::NonTrovato(id.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::EditorWorld;

    const OGGI: &str = "2026-10-16";

    fn valori(coppie: &[(&str, &str)]) -> BTreeMap<String, String> {
        coppie
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn compila_testo(testo: &str, v: &[(&str, &str)]) -> Result<RisultatoModello, ErroreModello> {
        compila_analisi(&analizza(testo), &valori(v), OGGI)
    }

    fn campo(nome: &str, tipo: TipoCampo, pred: &str) -> CampoModello {
        CampoModello {
            nome: nome.into(),
            tipo,
            predefinito: pred.into(),
        }
    }

    // --- parsing ---

    #[test]
    fn parsing_completo() {
        let a = analizza(
            "// @modello: Lettera\n// @descrizione: Una pagina\n// @pagine: 1\n\
             // @campo: dest | testo | Studio Bianchi\n\
             // @campo: oggetto | testo |\n\
             // @campo: data | data | oggi\n\
             // @campo: importo | numero | 12.5\n\
             // @campo: voci | elenco | a\\nb\n\
             \n= {{dest}} {{oggetto}} {{data}} {{importo}}\n{{voci}}\n",
        );
        assert_eq!(a.nome.as_deref(), Some("Lettera"));
        assert_eq!(a.descrizione, "Una pagina");
        assert_eq!(a.pagine, Some(1));
        assert_eq!(a.campi.len(), 5);
        assert_eq!(
            a.campi[0],
            campo("dest", TipoCampo::Testo, "Studio Bianchi")
        );
        assert_eq!(a.campi[1], campo("oggetto", TipoCampo::Testo, ""));
        assert_eq!(a.campi[2].tipo, TipoCampo::Data);
        assert_eq!(a.campi[3].tipo, TipoCampo::Numero);
        assert_eq!(a.campi[4].predefinito, "a\nb");
        assert!(a.avvisi.is_empty(), "{:?}", a.avvisi);
        assert!(!a.corpo.contains("@"));
    }

    #[test]
    fn parsing_tollerante_righe_mal_formate() {
        let a = analizza(
            "// @modello: M\n\
             // @campo: Maiuscolo | testo | x\n\
             // @campo: senza_tipo\n\
             // @campo: tipo_strano | colore | x\n\
             // @campo: ok | testo | v\n\
             // @campo: ok | testo | doppio\n\
             // @pagine: molte\n\
             // @sconosciuta: x\n\
             // @senzaduepunti\n\
             // @modello: Secondo\n\
             {{ok}}\n",
        );
        assert_eq!(a.nome.as_deref(), Some("M"));
        assert_eq!(a.campi.len(), 1);
        assert_eq!(a.campi[0].predefinito, "v");
        assert_eq!(a.pagine, None);
        assert_eq!(a.avvisi.len(), 8, "{:#?}", a.avvisi);
        assert_eq!(a.corpo, "{{ok}}\n");
    }

    #[test]
    fn parsing_non_va_in_panico_su_input_strani() {
        for s in [
            "",
            "// @",
            "// @campo:",
            "// @campo: |||",
            "\u{feff}// @modello: Bom\ncorpo",
            "{{",
            "}}{{",
            "{{{{x}}",
            "// @campo: é | testo |\n",
            "//@campo: a | testo | \\",
            "{{!}}{{ }}{{é}}",
        ] {
            let a = analizza(s);
            let _ = compila_analisi(&a, &BTreeMap::new(), OGGI);
        }
        assert_eq!(
            analizza("\u{feff}// @modello: Bom\n").nome.as_deref(),
            Some("Bom")
        );
    }

    #[test]
    fn la_testata_finisce_al_primo_rigo_di_corpo() {
        let a =
            analizza("// @modello: M\n// commento normale\n#set page()\n// @campo: x | testo |\n");
        // Il commento normale resta nel corpo; la direttiva dopo il corpo pure.
        assert_eq!(
            a.corpo,
            "// commento normale\n#set page()\n// @campo: x | testo |\n"
        );
        assert!(a.campi.is_empty());
    }

    #[test]
    fn tetto_dei_campi_in_parsing() {
        let mut t = String::new();
        for i in 0..70 {
            t.push_str(&format!("// @campo: c{i} | testo |\n"));
        }
        let a = analizza(&t);
        assert_eq!(a.campi.len(), MAX_CAMPI);
        assert!(a.avvisi.iter().any(|m| m.contains("oltre 64 campi")));
    }

    #[test]
    fn nome_campo_troppo_lungo_rifiutato() {
        let lungo = "a".repeat(41);
        let a = analizza(&format!("// @campo: {lungo} | testo |\n"));
        assert!(a.campi.is_empty());
        assert!(nome_campo_valido(&"a".repeat(40)));
    }

    // --- predefiniti e oggi ---

    #[test]
    fn predefiniti_assente_e_oggi() {
        let t = "// @campo: a | testo | pre\n// @campo: d | data | oggi\n// @campo: v | testo |\n{{a}}|{{d}}|{{v}}";
        assert_eq!(compila_testo(t, &[]).unwrap().sorgente, "pre|2026-10-16|");
        // Un valore presente ma vuoto resta vuoto.
        assert_eq!(
            compila_testo(t, &[("a", "")]).unwrap().sorgente,
            "|2026-10-16|"
        );
        assert_eq!(
            compila_testo(t, &[("a", "x"), ("d", "1/2/2020")])
                .unwrap()
                .sorgente,
            "x|1/2/2020|"
        );
    }

    #[test]
    fn oggi_vale_solo_per_il_tipo_data() {
        let t = "// @campo: a | testo | oggi\n{{a}}";
        assert_eq!(compila_testo(t, &[]).unwrap().sorgente, "oggi");
    }

    #[test]
    fn oggi_iso_ha_formato_corretto() {
        let s = oggi_iso();
        let b = s.as_bytes();
        assert_eq!(s.len(), 10, "{s}");
        assert!(b[4] == b'-' && b[7] == b'-');
        assert!(s.replace('-', "").bytes().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn info_risolve_oggi_nell_elenco() {
        let v = elenca_con(None, OGGI);
        let lettera = v.iter().find(|m| m.id == "lettera").unwrap();
        let data = lettera.campi.iter().find(|c| c.nome == "data").unwrap();
        assert_eq!(data.predefinito, OGGI);
    }

    // --- escaping ---

    #[test]
    fn escaping_di_ogni_carattere_speciale() {
        for c in [
            '\\', '#', '$', '*', '_', '@', '<', '>', '`', '~', '[', ']', '"', '\'',
        ] {
            assert_eq!(escapa_markup(&format!("a{c}b")), format!("a\\{c}b"), "{c}");
        }
        assert_eq!(escapa_markup("normale 123 àè"), "normale 123 àè");
    }

    #[test]
    fn escaping_strutture_a_inizio_riga_e_commenti() {
        assert_eq!(escapa_markup("= titolo"), "\\= titolo");
        assert_eq!(escapa_markup("- voce"), "\\- voce");
        assert_eq!(escapa_markup("+ voce"), "\\+ voce");
        assert_eq!(escapa_markup("/ termine"), "\\/ termine");
        assert_eq!(escapa_markup("1. primo"), "1\\. primo");
        assert_eq!(escapa_markup("a\n- b"), "a\n\\- b");
        assert_eq!(escapa_markup("-5"), "-5");
        assert_eq!(escapa_markup("http://x"), "http:\\//x");
        assert_eq!(escapa_markup("a\r\nb"), "a\nb");
    }

    #[test]
    fn segnaposto_normale_verbatim_ed_elenco() {
        let t = "// @campo: x | testo | a*b\n// @campo: y | numero | 2\n// @campo: l | elenco |\n\
                 [{{x}}] [{{!x}}] {{!y}} {{ x }}\n{{l}}\n";
        let r = compila_testo(t, &[("l", "uno\n\n  - due  \r\ntre #\n")]).unwrap();
        assert_eq!(
            r.sorgente,
            "[a\\*b] [a*b] 2 a\\*b\n- uno\n- \\- due\n- tre \\#\n"
        );
        assert!(r.avvisi.is_empty(), "{:?}", r.avvisi);
        // Elenco vuoto -> nessuna riga.
        let r = compila_testo(t, &[("l", " \n")]).unwrap();
        assert!(r.sorgente.ends_with("\n\n"), "{:?}", r.sorgente);
    }

    #[test]
    fn il_valore_non_viene_riesaminato() {
        let t = "// @campo: a | testo |\n// @campo: b | testo | B\n{{a}}";
        let r = compila_testo(t, &[("a", "{{b}}")]).unwrap();
        assert_eq!(r.sorgente, "{{b}}");
    }

    #[test]
    fn graffe_extra_attorno_al_segnaposto() {
        let t = "// @campo: a | testo | X\n{{{a}}} {{ {a}} {{";
        let r = compila_testo(t, &[]).unwrap();
        assert_eq!(r.sorgente, "{X} {{ {a}} {{");
    }

    // --- avvisi di coerenza ---

    #[test]
    fn avvisi_segnaposto_non_dichiarato_e_campo_inutilizzato() {
        let t = "// @modello: M\n// @campo: usato | testo |\n// @campo: inutile | testo |\n{{usato}} {{orfano}} {{!orfano}}";
        let r = compila_testo(t, &[]).unwrap();
        assert_eq!(r.avvisi.len(), 2, "{:?}", r.avvisi);
        assert!(r.avvisi.iter().any(|m| m.contains("orfano")));
        assert!(r.avvisi.iter().any(|m| m.contains("inutile")));
        // Il segnaposto orfano resta com'è nel testo.
        assert!(r.sorgente.contains("{{orfano}}") && r.sorgente.contains("{{!orfano}}"));
    }

    #[test]
    fn avvisi_sui_valori() {
        let t = "// @campo: n | numero | 1\n{{n}}";
        let r = compila_testo(t, &[("n", "abc"), ("fantasma", "x")]).unwrap();
        assert!(r.avvisi.iter().any(|m| m.contains("non è un numero")));
        assert!(r.avvisi.iter().any(|m| m.contains("fantasma")));
        let r = compila_testo(t, &[("n", "3.5")]).unwrap();
        assert!(r.avvisi.is_empty());
    }

    #[test]
    fn la_testata_non_finisce_nel_documento() {
        let r = compila_testo(
            "// @modello: M\n// @descrizione: d\n// @pagine: 1\n// @campo: a | testo | x\n\n= {{a}}\n",
            &[],
        )
        .unwrap();
        assert!(!r.sorgente.contains("@modello") && !r.sorgente.contains("@campo"));
        assert_eq!(r.sorgente, "\n= x\n");
    }

    // --- id ---

    #[test]
    fn id_validi_e_non_validi() {
        for ok in ["lettera", "a", "x_y-z", "a1", &"a".repeat(60)] {
            assert!(valida_id(ok).is_ok(), "{ok}");
        }
        let lungo = "a".repeat(61);
        for ko in [
            "", "..", ".", "../x", "a/b", "a\\b", "a.typ", "A", "è", "a b", "c:", "con", "nul",
            "aux", "prn", "com1", "lpt9", "com0x", &lungo, "a\0b",
        ] {
            let e = valida_id(ko);
            // `com0x` non è riservato: è valido.
            if ko == "com0x" {
                assert!(e.is_ok());
            } else {
                assert!(matches!(e, Err(ErroreModello::IdNonValido(_))), "{ko:?}");
            }
        }
    }

    #[test]
    fn id_da_nome_normalizza() {
        assert_eq!(id_da_nome("Lettera di Prova").unwrap(), "lettera-di-prova");
        assert_eq!(id_da_nome("  Perché? Sì!  ").unwrap(), "perche-si");
        assert_eq!(id_da_nome("../../x").unwrap(), "x");
        assert!(id_da_nome("???").is_err());
        assert!(id_da_nome("CON").is_err());
        assert!(id_da_nome("日本語").is_err());
        let id = id_da_nome(&"ab ".repeat(40)).unwrap();
        assert!(id.len() <= MAX_ID && !id.ends_with('-'));
    }

    #[test]
    fn id_non_valido_nelle_operazioni() {
        let d = tempfile::tempdir().unwrap();
        for ko in ["../x", "a/b", "CON", ""] {
            assert!(matches!(
                compila_modello(d.path(), ko, &BTreeMap::new()),
                Err(ErroreModello::IdNonValido(_))
            ));
            assert!(matches!(
                elimina_modello(d.path(), ko),
                Err(ErroreModello::IdNonValido(_))
            ));
        }
    }

    // --- I/O ---

    #[test]
    fn elenco_con_soli_integrati_e_cartella_inesistente() {
        let d = tempfile::tempdir().unwrap();
        let v = elenca_modelli(&d.path().join("non-esiste"));
        let ids: BTreeSet<&str> = v.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            BTreeSet::from(["lettera", "scheda-prodotto", "relazione-breve"])
        );
        assert!(v.iter().all(|m| m.origine == OrigineModello::Integrato));
        for m in &v {
            assert!(m.avvisi.is_empty(), "{}: {:?}", m.id, m.avvisi);
            assert!(m.pagine.is_some() && !m.campi.is_empty() && !m.descrizione.is_empty());
        }
    }

    #[test]
    fn campi_degli_integrati() {
        let v = elenca_con(None, OGGI);
        let n = |id: &str| v.iter().find(|m| m.id == id).unwrap();
        assert_eq!(n("lettera").pagine, Some(1));
        assert_eq!(n("scheda-prodotto").pagine, Some(2));
        assert_eq!(n("relazione-breve").pagine, Some(2));
        assert!(n("scheda-prodotto").campi.len() >= 7);
        assert!(n("relazione-breve").campi.len() >= 5);
    }

    #[test]
    fn scrittura_sovrascrittura_e_nessun_temporaneo_residuo() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("modelli");
        let campi = [campo("x", TipoCampo::Testo, "uno")];
        let id = salva_come_modello(
            &dir,
            "Mio Modello",
            "Ciao {{x}}",
            &campi,
            "d",
            Some(1),
            false,
        )
        .unwrap();
        assert_eq!(id, "mio-modello");
        let id2 =
            salva_come_modello(&dir, "Mio modello", "Ciao2 {{x}}", &campi, "", None, true).unwrap();
        assert_eq!(id2, id);
        let r = compila_modello(&dir, &id, &BTreeMap::new()).unwrap();
        assert_eq!(r.sorgente, "\nCiao2 uno\n");
        let nomi: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomi, vec!["mio-modello.typ".to_string()]);
    }

    #[test]
    fn round_trip_salva_elenca_compila() {
        let d = tempfile::tempdir().unwrap();
        let campi = vec![
            campo("titolo", TipoCampo::Testo, "T | con barra e \\ backslash"),
            campo("quando", TipoCampo::Data, "oggi"),
            campo("n", TipoCampo::Numero, "7"),
            campo("voci", TipoCampo::Elenco, "a\nb"),
        ];
        let sorg = "// @modello: scartata\n= {{titolo}}\n{{quando}} {{!n}}\n{{voci}}\n";
        let id = salva_come_modello(d.path(), "Rapporto", sorg, &campi, "Descr", Some(3), false)
            .unwrap();

        let el = elenca_modelli(d.path());
        let m = el.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.origine, OrigineModello::Utente);
        assert_eq!(m.nome, "Rapporto");
        assert_eq!(m.descrizione, "Descr");
        assert_eq!(m.pagine, Some(3));
        assert!(m.avvisi.is_empty(), "{:?}", m.avvisi);
        assert_eq!(m.campi.len(), 4);
        assert_eq!(m.campi[0].predefinito, "T | con barra e \\ backslash");
        assert_eq!(m.campi[3].predefinito, "a\nb");
        assert_eq!(m.campi[1].predefinito, oggi_iso());

        let r = compila_con(Some(d.path()), &id, &BTreeMap::new(), OGGI).unwrap();
        assert_eq!(
            r.sorgente,
            "\n= T | con barra e \\\\ backslash\n2026-10-16 7\n- a\n- b\n"
        );
        let file = std::fs::read_to_string(d.path().join("rapporto.typ")).unwrap();
        assert!(file.contains("// @campo: quando | data | oggi"));
        assert!(!file.contains("scartata"));
    }

    #[test]
    fn precedenza_utente_su_integrato_e_ripristino() {
        let d = tempfile::tempdir().unwrap();
        let id = salva_come_modello(
            d.path(),
            "Lettera",
            "Mia lettera {{a}}",
            &[campo("a", TipoCampo::Testo, "z")],
            "",
            None,
            false,
        )
        .unwrap();
        assert_eq!(id, "lettera");
        let el = elenca_modelli(d.path());
        assert_eq!(el.iter().filter(|m| m.id == "lettera").count(), 1);
        assert_eq!(
            el.iter().find(|m| m.id == "lettera").unwrap().origine,
            OrigineModello::Utente
        );
        assert_eq!(
            compila_modello(d.path(), "lettera", &BTreeMap::new())
                .unwrap()
                .sorgente,
            "\nMia lettera z\n"
        );
        // Eliminando l'utente torna l'integrato.
        elimina_modello(d.path(), "lettera").unwrap();
        let el = elenca_modelli(d.path());
        assert_eq!(
            el.iter().find(|m| m.id == "lettera").unwrap().origine,
            OrigineModello::Integrato
        );
        assert!(matches!(
            elimina_modello(d.path(), "lettera"),
            Err(ErroreModello::IntegratoNonEliminabile(_))
        ));
    }

    #[test]
    fn eliminazione_e_non_trovato() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(
            elimina_modello(d.path(), "inesistente"),
            Err(ErroreModello::NonTrovato(_))
        ));
        assert!(matches!(
            elimina_modello(d.path(), "scheda-prodotto"),
            Err(ErroreModello::IntegratoNonEliminabile(_))
        ));
        assert!(matches!(
            compila_modello(d.path(), "inesistente", &BTreeMap::new()),
            Err(ErroreModello::NonTrovato(_))
        ));
        let id = salva_come_modello(d.path(), "Tmp", "x", &[], "", None, false).unwrap();
        elimina_modello(d.path(), &id).unwrap();
        assert!(!d.path().join("tmp.typ").exists());
    }

    #[test]
    fn salvataggio_rifiuta_dati_non_validi() {
        let d = tempfile::tempdir().unwrap();
        let ok = [campo("a", TipoCampo::Testo, "")];
        let e = |r: Result<String, ErroreModello>| r.unwrap_err();
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "  ",
                "x",
                &ok,
                "",
                None,
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "a\nb",
                "x",
                &ok,
                "",
                None,
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "con",
                "x",
                &ok,
                "",
                None,
                false
            )),
            ErroreModello::IdNonValido(_)
        ));
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "n",
                " \n",
                &ok,
                "",
                None,
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "n",
                "x",
                &ok,
                "",
                Some(0),
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        let doppi = [ok[0].clone(), ok[0].clone()];
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "n",
                "x",
                &doppi,
                "",
                None,
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        let cattivo = [campo("A b", TipoCampo::Testo, "")];
        assert!(matches!(
            e(salva_come_modello(
                d.path(),
                "n",
                "x",
                &cattivo,
                "",
                None,
                false
            )),
            ErroreModello::DatiNonValidi(_)
        ));
        assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 0);
    }

    #[test]
    fn tetti_di_dimensione_e_campi() {
        let d = tempfile::tempdir().unwrap();
        let grande = "x".repeat(MAX_BYTES_MODELLO + 1);
        assert!(matches!(
            salva_come_modello(d.path(), "g", &grande, &[], "", None, false),
            Err(ErroreModello::Limite(_))
        ));
        let molti: Vec<CampoModello> = (0..=MAX_CAMPI)
            .map(|i| campo(&format!("c{i}"), TipoCampo::Testo, ""))
            .collect();
        assert!(matches!(
            salva_come_modello(d.path(), "m", "x", &molti, "", None, false),
            Err(ErroreModello::Limite(_))
        ));
        // File utente oltre il tetto: compila rifiuta, elenca salta.
        std::fs::write(d.path().join("enorme.typ"), &grande).unwrap();
        assert!(matches!(
            compila_modello(d.path(), "enorme", &BTreeMap::new()),
            Err(ErroreModello::Limite(_))
        ));
        assert!(elenca_modelli(d.path()).iter().all(|m| m.id != "enorme"));
        // Valore troppo grande.
        assert!(matches!(
            compila_modello(d.path(), "lettera", &valori(&[("oggetto", &grande)])),
            Err(ErroreModello::Limite(_))
        ));
        // Esattamente al tetto: accettato.
        assert!(salva_come_modello(
            d.path(),
            "ok",
            &"y".repeat(MAX_BYTES_MODELLO - 200),
            &[],
            "",
            None,
            false,
        )
        .is_ok());
    }

    #[test]
    fn elenco_salta_file_con_id_non_valido_e_non_typ() {
        let d = tempfile::tempdir().unwrap();
        for nome in [
            "Maiuscolo.typ",
            "con.typ",
            "valido.txt",
            "ok.typ",
            "a b.typ",
        ] {
            std::fs::write(d.path().join(nome), "// @modello: X\nciao").unwrap();
        }
        std::fs::create_dir(d.path().join("dir.typ")).unwrap();
        let ids: Vec<String> = elenca_modelli(d.path())
            .into_iter()
            .filter(|m| m.origine == OrigineModello::Utente)
            .map(|m| m.id)
            .collect();
        assert_eq!(ids, vec!["ok".to_string()]);
    }

    #[test]
    fn modello_senza_nome_usa_l_id_con_avviso() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("senza.typ"), "ciao").unwrap();
        let el = elenca_modelli(d.path());
        let m = el.iter().find(|m| m.id == "senza").unwrap();
        assert_eq!(m.nome, "senza");
        assert!(m.avvisi.iter().any(|a| a.contains("@modello")));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_fuori_cartella_rifiutato() {
        let d = tempfile::tempdir().unwrap();
        let fuori = tempfile::tempdir().unwrap();
        std::fs::write(fuori.path().join("x.typ"), "segreto").unwrap();
        std::os::unix::fs::symlink(fuori.path().join("x.typ"), d.path().join("x.typ")).unwrap();
        assert!(compila_modello(d.path(), "x", &BTreeMap::new()).is_err());
        assert!(salva_come_modello(d.path(), "x", "nuovo", &[], "", None, false).is_err());
        assert_eq!(
            std::fs::read_to_string(fuori.path().join("x.typ")).unwrap(),
            "segreto"
        );
    }

    #[test]
    fn serializzazione_json_per_il_frontend() {
        let v = elenca_con(None, OGGI);
        let lettera = v.iter().find(|m| m.id == "lettera").unwrap();
        let j = serde_json::to_value(lettera).unwrap();
        assert_eq!(j["origine"], "integrato");
        assert_eq!(j["pagine"], 1);
        assert_eq!(j["campi"][0]["tipo"], "testo");
        assert!(j["campi"][0].get("predefinito").is_some());
        assert!(j["avvisi"].is_array());
        let c: CampoModello = serde_json::from_str(r#"{"nome":"a","tipo":"elenco"}"#).unwrap();
        assert_eq!(c.predefinito, "");
        let r = serde_json::to_value(RisultatoModello {
            sorgente: "s".into(),
            avvisi: vec![],
        })
        .unwrap();
        assert_eq!(r["sorgente"], "s");
    }

    // --- compilazione Typst reale ---

    fn compila_typst(sorgente: &str) -> usize {
        let mut mondo = EditorWorld::nuovo(sorgente);
        let r = mondo.compila();
        assert!(
            r.diagnostici.iter().all(|d| d.gravita != "errore"),
            "errori Typst: {:?}\n--- sorgente ---\n{sorgente}",
            r.diagnostici
        );
        r.pagine.len()
    }

    #[test]
    fn i_tre_integrati_compilano_con_i_predefiniti() {
        let attese = [
            ("lettera", 1),
            ("scheda-prodotto", 2),
            ("relazione-breve", 2),
        ];
        for (id, pagine) in attese {
            let r = compila_con(None, id, &BTreeMap::new(), OGGI).unwrap();
            assert!(r.avvisi.is_empty(), "{id}: {:?}", r.avvisi);
            assert!(!r.sorgente.contains("{{"), "{id}: segnaposto residui");
            assert!(!r.sorgente.contains("@modello"));
            assert_eq!(compila_typst(&r.sorgente), pagine, "{id}");
        }
    }

    #[test]
    fn integrati_compilano_con_valori_ostili_e_vuoti() {
        let ostile =
            "*a* _b_ #c $d$ @e <f> `g` ~h [i] \"j\" 'k' \\ // x /* y */ = t\n- v\n+ w\n/ z\n1. u";
        for m in elenca_con(None, OGGI) {
            let mut v = BTreeMap::new();
            for c in &m.campi {
                let val = if c.tipo == TipoCampo::Numero {
                    "-3.5".to_string()
                } else {
                    ostile.to_string()
                };
                v.insert(c.nome.clone(), val);
            }
            let r = compila_modello_con_cartella_nulla(&m.id, &v);
            compila_typst(&r.sorgente);
            // Tutti vuoti.
            let vuoti: BTreeMap<String, String> = m
                .campi
                .iter()
                .map(|c| (c.nome.clone(), String::new()))
                .collect();
            let r = compila_modello_con_cartella_nulla(&m.id, &vuoti);
            compila_typst(&r.sorgente);
        }
    }

    fn compila_modello_con_cartella_nulla(
        id: &str,
        v: &BTreeMap<String, String>,
    ) -> RisultatoModello {
        compila_con(None, id, v, OGGI).unwrap()
    }

    #[test]
    fn il_testo_escapato_compare_letteralmente_nel_documento() {
        // Il testo ostile deve rendersi come testo, non come markup.
        let t = "// @campo: x | testo |\n{{x}}\n";
        let ostile = "*a* #let y = 1 $z$ // coda";
        let r = compila_testo(t, &[("x", ostile)]).unwrap();
        let mut mondo = EditorWorld::nuovo(&r.sorgente);
        let c = mondo.compila();
        assert!(c.diagnostici.iter().all(|d| d.gravita != "errore"));
        assert_eq!(c.pagine.len(), 1);
    }

    // --- correzioni dopo revisione ---

    /// Allocatore di test: ricorda, per thread, la più grande allocazione
    /// singola richiesta (serve a provare che il tetto agisce PRIMA di
    /// costruire la stringa, non dopo).
    struct ContaAlloc;

    thread_local! {
        static MAX_ALLOC: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    fn registra(n: usize) {
        let _ = MAX_ALLOC.try_with(|m| {
            if n > m.get() {
                m.set(n);
            }
        });
    }

    // SAFETY: delega tutto a `System` senza alterare layout né puntatori;
    // in più aggiorna un contatore thread-local (const-init, senza Drop).
    unsafe impl std::alloc::GlobalAlloc for ContaAlloc {
        unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
            registra(l.size());
            unsafe { std::alloc::System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
            registra(l.size());
            unsafe { std::alloc::System.alloc_zeroed(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
            unsafe { std::alloc::System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
            registra(n);
            unsafe { std::alloc::System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static ALLOCATORE_TEST: ContaAlloc = ContaAlloc;

    /// Esegue `f` e restituisce (esito, massima allocazione singola in byte).
    fn con_max_alloc<T>(f: impl FnOnce() -> T) -> (T, usize) {
        MAX_ALLOC.with(|m| m.set(0));
        let r = f();
        (r, MAX_ALLOC.with(std::cell::Cell::get))
    }

    /// Ripetizioni del segnaposto nel test del tetto (50.000 = caso reale
    /// segnalato; abbassato solo per provare il codice vecchio senza abort).
    const RIPETIZIONI: usize = 50_000;

    #[test]
    fn tetto_applicato_prima_di_costruire_la_stringa() {
        let t = format!("// @campo: a | testo |\n{}", "{{a}}".repeat(RIPETIZIONI));
        assert!(t.len() < MAX_BYTES_MODELLO);
        let grande = "x".repeat(MAX_BYTES_MODELLO);
        let (r, max) = con_max_alloc(|| compila_testo(&t, &[("a", &grande)]));
        assert!(matches!(r, Err(ErroreModello::Limite(_))), "{r:?}");
        assert!(
            max < 16 * 1024 * 1024,
            "allocazione singola di {max} byte: il tetto va controllato prima"
        );
    }

    #[test]
    fn il_valore_si_rende_una_sola_volta_per_campo() {
        // Sotto il tetto: 3000 occorrenze di un valore con caratteri speciali.
        let t = format!("// @campo: a | testo |\n{}", "{{a}}".repeat(3000));
        let r = compila_testo(&t, &[("a", "*_#")]).unwrap();
        assert_eq!(r.sorgente, "\\*\\_\\#".repeat(3000));
        // Verbatim e normale insieme, entrambi con tetto.
        let t = "// @campo: a | testo |\n{{a}}{{!a}}";
        assert_eq!(compila_testo(t, &[("a", "*")]).unwrap().sorgente, "\\**");
    }

    #[test]
    fn titoli_con_piu_uguali_sono_escapati() {
        assert_eq!(escapa_markup("== x"), "\\== x");
        assert_eq!(escapa_markup("=== x"), "\\=== x");
        assert_eq!(escapa_markup("=="), "\\==");
        assert_eq!(escapa_markup("a\n=== x"), "a\n\\=== x");
        assert_eq!(escapa_markup("  == x"), "  \\== x");
        // Senza spazio dopo la sequenza non è un titolo.
        assert_eq!(escapa_markup("==x"), "==x");
        assert_eq!(escapa_markup("= x"), "\\= x");
        // Il risultato compila e non crea titoli: una sola pagina di testo.
        let r = compila_testo("// @campo: x | testo |\n{{x}}\n", &[("x", "== x")]).unwrap();
        assert_eq!(compila_typst(&r.sorgente), 1);
    }

    #[test]
    fn a_capo_unicode_normalizzati_in_escaping_ed_elenco() {
        assert_eq!(escapa_markup("x\u{2028}= Titolo"), "x\n\\= Titolo");
        assert_eq!(escapa_markup("x\u{85}- voce"), "x\n\\- voce");
        assert_eq!(escapa_markup("x\u{2029}1. voce"), "x\n1\\. voce");
        assert_eq!(escapa_markup("x\u{0B}+ v"), "x\n\\+ v");
        assert_eq!(escapa_markup("x\u{0C}== t"), "x\n\\== t");
        assert_eq!(escapa_markup("a\r\n\rb"), "a\n\nb");
        assert_eq!(rendi_elenco("a\u{2028}b\u{85}\u{2029}c"), "- a\n- b\n- c");
        assert_eq!(rendi_elenco("a\u{2028}- b"), "- a\n- \\- b");
    }

    #[test]
    fn file_utente_guasto_con_id_di_integrato_e_coerente() {
        let d = tempfile::tempdir().unwrap();
        // Oltre il tetto.
        std::fs::write(
            d.path().join("lettera.typ"),
            "x".repeat(MAX_BYTES_MODELLO + 1),
        )
        .unwrap();
        // Non UTF-8.
        std::fs::write(d.path().join("scheda-prodotto.typ"), [0xff, 0xfe, 0x00]).unwrap();
        let el = elenca_modelli(d.path());
        for id in ["lettera", "scheda-prodotto"] {
            let m = el.iter().find(|m| m.id == id).unwrap();
            assert_eq!(m.origine, OrigineModello::Utente, "{id}");
            assert!(m.campi.is_empty(), "{id}");
            assert!(!m.avvisi.is_empty(), "{id}");
            assert!(m.avvisi[0].contains("elimin"), "{:?}", m.avvisi);
            let e = compila_modello(d.path(), id, &BTreeMap::new()).unwrap_err();
            assert!(e.to_string().contains("elimin"), "{id}: {e}");
        }
        // Un integrato non toccato resta integrato.
        let r = el.iter().find(|m| m.id == "relazione-breve").unwrap();
        assert_eq!(r.origine, OrigineModello::Integrato);
        // Eliminando il file guasto l'integrato torna utilizzabile.
        elimina_modello(d.path(), "lettera").unwrap();
        assert!(compila_modello(d.path(), "lettera", &BTreeMap::new()).is_ok());
    }

    #[test]
    fn salva_non_sovrascrive_in_silenzio_un_modello_utente() {
        let d = tempfile::tempdir().unwrap();
        salva_come_modello(d.path(), "Mio", "uno", &[], "", None, false).unwrap();
        let e = salva_come_modello(d.path(), "Mio", "due", &[], "", None, false).unwrap_err();
        assert!(e.to_string().starts_with("Esiste già un modello"), "{e}");
        let r = compila_modello(d.path(), "mio", &BTreeMap::new()).unwrap();
        assert_eq!(r.sorgente, "\nuno\n");
        assert!(matches!(e, ErroreModello::GiaEsistente(_)));
        // Con `sovrascrivi` il file viene sostituito.
        salva_come_modello(d.path(), "Mio", "due", &[], "", None, true).unwrap();
        let r = compila_modello(d.path(), "mio", &BTreeMap::new()).unwrap();
        assert_eq!(r.sorgente, "\ndue\n");
        // Sostituire un integrato non chiede conferma.
        assert!(salva_come_modello(d.path(), "Lettera", "mia", &[], "", None, false).is_ok());
    }

    #[test]
    fn salva_rifiuta_corpo_vuoto_con_sola_testata() {
        let d = tempfile::tempdir().unwrap();
        let e = salva_come_modello(
            d.path(),
            "Vuoto",
            "// @modello: x\n// @campo: a | testo |\n\n",
            &[],
            "",
            None,
            false,
        )
        .unwrap_err();
        assert!(matches!(e, ErroreModello::DatiNonValidi(_)), "{e:?}");
        assert!(!d.path().join("vuoto.typ").exists());
    }

    #[test]
    fn graffe_aperte_senza_chiusura_non_sono_quadratiche() {
        let senza_chiusura = "{{".repeat(MAX_BYTES_MODELLO / 2);
        let con_chiusura_in_fondo = format!("{}}}}}", "{{".repeat(MAX_BYTES_MODELLO / 2 - 1));
        for corpo in [senza_chiusura, con_chiusura_in_fondo] {
            let inizio = std::time::Instant::now();
            let a = analizza(&corpo);
            let r = compila_analisi(&a, &BTreeMap::new(), OGGI).unwrap();
            assert_eq!(r.sorgente, corpo);
            assert!(
                inizio.elapsed() < std::time::Duration::from_secs(2),
                "troppo lento: {:?}",
                inizio.elapsed()
            );
        }
    }

    #[test]
    fn numero_accetta_la_virgola_decimale() {
        assert!(e_numero("12,5"));
        assert!(e_numero(" -3,25 "));
        assert!(e_numero("12.5"));
        assert!(!e_numero("1,2,3"));
        assert!(!e_numero("1,2.3"));
        assert!(!e_numero("abc"));
        let t = "// @campo: n | numero | 1\n{{n}}";
        let r = compila_testo(t, &[("n", "12,5")]).unwrap();
        assert!(r.avvisi.is_empty(), "{:?}", r.avvisi);
        assert_eq!(r.sorgente, "12,5");
    }

    #[test]
    fn il_tetto_dei_file_conta_solo_i_typ() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..(MAX_FILE_UTENTE + 100) {
            std::fs::write(d.path().join(format!("a{i:04}.txt")), "x").unwrap();
        }
        std::fs::write(d.path().join("zzz.typ"), "// @modello: Z\nciao").unwrap();
        let el = elenca_modelli(d.path());
        assert!(el.iter().any(|m| m.id == "zzz"));
    }

    #[test]
    fn cartella_non_esistente_viene_creata_e_usata_canonica() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("a").join("..").join("b");
        let id = salva_come_modello(&dir, "Nuovo", "x", &[], "", None, false).unwrap();
        assert!(d.path().join("b").join(format!("{id}.typ")).is_file());
        elimina_modello(&dir, &id).unwrap();
        assert!(!d.path().join("b").join("nuovo.typ").exists());
    }
}
