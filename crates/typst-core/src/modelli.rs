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
#[path = "../tests/unit/modelli.rs"]
mod test;
