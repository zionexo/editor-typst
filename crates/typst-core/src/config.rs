//! Configurazione esterna di Typster: snippet riutilizzabili e impostazioni.
//!
//! I file `snippets.json` e `impostazioni.json` vivono nella config dir
//! standard dell'OS (`dirs::config_dir()/typster/`): dati esterni e
//! versionabili dall'utente, non hardcodati (vedi PROGETTO.md §2).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Nome del file degli snippet dentro la cartella dati del prodotto.
const NOME_FILE_SNIPPET: &str = "snippets.json";

/// Nome del file delle impostazioni dentro la cartella dati del prodotto.
const NOME_FILE_IMPOSTAZIONI: &str = "impostazioni.json";

/// Cartella dati precedente alla rinomina in Typster (D-009): sorgente della
/// migrazione una tantum di `snippets.json`.
const CARTELLA_PRECEDENTE: &str = "editor-typst";

/// Uno snippet riutilizzabile, offerto come completamento in CodeMirror.
///
/// `corpo` usa la sintassi snippet di CodeMirror (`${campo}` per i
/// segnaposto navigabili a Tab, `${0}` per lo stop finale del cursore dopo
/// l'espansione).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    pub etichetta: String,
    pub prefisso: String,
    pub corpo: String,
    #[serde(default)]
    pub descrizione: Option<String>,
}

/// Carica gli snippet dal file di configurazione standard dell'OS.
///
/// **Infallibile**: crea i 6 predefiniti al primo avvio (scrittura atomica),
/// e degrada con grazia ai predefiniti in memoria se il file manca, non è
/// leggibile o non è un array JSON valido — senza mai sovrascrivere un file
/// utente già presente ma temporaneamente illeggibile. Se il file è un array
/// JSON valido, ogni voce è validata **singolarmente** (parsing tollerante,
/// come per l'indice pacchetti di Typst Universe, F3-001): una voce
/// malformata viene scartata senza perdere le altre. Scarta anche le voci
/// (ben formate) con prefisso o corpo vuoti.
pub fn carica_snippet() -> Vec<Snippet> {
    match dirs::config_dir() {
        Some(base) => carica_snippet_in(&base),
        // Nessuna config dir determinabile per l'OS corrente: degrada ai
        // predefiniti senza tentare alcuna scrittura su disco.
        None => predefiniti(),
    }
}

/// Carica gli snippet dalla config dir `base` (testabile su cartelle temporanee).
///
/// La migrazione va PRIMA del caricamento: `carica_snippet_da` crea i
/// predefiniti se il file nuovo manca, e dopo non si potrebbe più distinguere
/// "primo avvio" da "già migrato".
fn carica_snippet_in(base: &Path) -> Vec<Snippet> {
    migra_snippet_da(base);
    carica_snippet_da(&base.join(crate::NOME_CARTELLA_DATI).join(NOME_FILE_SNIPPET))
}

/// Migrazione una tantum di `snippets.json` dalla vecchia cartella dati alla
/// nuova (D-009 punto 2), parametrizzata sulla config dir `base`.
///
/// Se il file nuovo manca e quello vecchio esiste, lo **copia** (scrittura
/// atomica); la cartella vecchia resta intatta. Ritorna `true` solo se ha
/// copiato. Degrado silenzioso: ogni errore di I/O lascia le cose come stanno
/// (si parte dai predefiniti) e l'esito è solo loggato su stderr.
fn migra_snippet_da(base: &Path) -> bool {
    let nuovo = base.join(crate::NOME_CARTELLA_DATI).join(NOME_FILE_SNIPPET);
    let vecchio = base.join(CARTELLA_PRECEDENTE).join(NOME_FILE_SNIPPET);
    if nuovo.exists() || !vecchio.is_file() {
        return false;
    }
    let esito = std::fs::read(&vecchio).and_then(|dati| scrivi_atomico(&nuovo, &dati));
    match esito {
        Ok(()) => {
            eprintln!("Migrati gli snippet da {}", vecchio.display());
            true
        }
        Err(e) => {
            eprintln!("Migrazione degli snippet non riuscita: {e}");
            false
        }
    }
}

/// Scrive `contenuto` su `percorso` in modo atomico: file temporaneo nella
/// **stessa cartella** (creata se manca), `sync_all` e poi `rename`, così il
/// bersaglio è sempre o il vecchio contenuto intatto o il nuovo completo,
/// anche dopo un crash. Su errore il temporaneo viene rimosso.
///
/// Se `percorso` è un **symlink** (o ne attraversa uno) il bersaglio vero viene
/// risolto con `canonicalize` e il temporaneo nasce accanto al file reale: il
/// `rename` sostituisce il file puntato e il collegamento resta intatto (prima
/// lo avrebbe rimpiazzato con un file normale). I **permessi** del file
/// preesistente sono copiati sul temporaneo prima del rename; su Windows gli
/// ACL non sono copiabili con la sola std: il nuovo file eredita quelli della
/// cartella (limite noto, D-026).
pub fn scrivi_atomico(percorso: &Path, contenuto: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    // Bersaglio reale: solo se il file esiste (altrimenti non c'è nulla da
    // risolvere; un symlink penzolante resta com'è, come prima).
    let reale = std::fs::canonicalize(percorso).unwrap_or_else(|_| percorso.to_path_buf());
    let permessi = std::fs::metadata(&reale).ok().map(|m| m.permissions());
    let percorso = reale.as_path();

    let cartella = percorso
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(cartella)?;
    let nome_file = percorso
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let temporaneo = cartella.join(format!(".{nome_file}.tmp-{}", crate::suffisso_univoco()));

    let esito = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporaneo)?;
        file.write_all(contenuto)?;
        if let Some(p) = &permessi {
            file.set_permissions(p.clone())?;
        }
        file.sync_all()?;
        drop(file); // chiuso prima del rename (necessario su Windows)
        std::fs::rename(&temporaneo, percorso)
    })();
    if esito.is_err() {
        // Su Windows un temporaneo con il flag read-only (copiato dal bersaglio)
        // non si rimuove: va azzerato prima, altrimenti resta un orfano
        // `.nome.tmp-…` nella cartella del documento.
        #[cfg(windows)]
        if let Ok(meta) = std::fs::metadata(&temporaneo) {
            let mut perm = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = std::fs::set_permissions(&temporaneo, perm);
        }
        let _ = std::fs::remove_file(&temporaneo);
    }
    esito
}

/// Impostazioni utente persistenti (`impostazioni.json`, F5-003).
///
/// Parsing tollerante (vedi [`Impostazioni::da_json`]): ogni campo mancante o
/// di tipo sbagliato prende il default e le voci sconosciute sono ignorate,
/// così file scritti da versioni diverse o editati a mano restano leggibili.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Impostazioni {
    /// Tema dell'interfaccia: `"sistema"`, `"chiaro"` o `"scuro"`.
    pub tema: String,
    /// Stile grafico dell'interfaccia, indipendente dal tema: `"classico"`
    /// (predefinito, l'aspetto storico), `"composizione"` o `"strumento"`.
    /// Un file preesistente senza il campo ricade su `"classico"`.
    pub stile: String,
    /// Motore di evidenziazione Typst dell'editor: `"lezer"` (predefinito, parser
    /// Lezer nativo, senza WASM) o `"wasm"` (modulo WASM di `codemirror-lang-typst`).
    /// Valore non ammesso o campo assente → `"lezer"`.
    pub motore_typst: String,
    /// Dimensione del font dell'editor, in pixel.
    pub dimensione_font: u32,
    /// Ritardo (ms) fra l'ultima modifica e la compilazione automatica.
    pub debounce_ms: u32,
    /// Salvataggio automatico dei file aperti.
    pub autosave: bool,
    /// Ritardo (ms) fra l'ultima modifica e il salvataggio automatico (F5-013);
    /// il salvataggio scatta anche quando la finestra perde il focus.
    pub autosave_ms: u32,
    /// Numero massimo di versioni conservate per file nella cronologia
    /// locale (F5-013): le più vecchie vengono eliminate (rotazione).
    pub cronologia_max_versioni: u32,
    /// Intervallo minimo (secondi) fra due versioni create dal salvataggio
    /// **automatico** (F5-013): il salvataggio manuale ne crea sempre una.
    /// Evita che l'autosave riempia la cronologia di micro-versioni.
    pub cronologia_intervallo_s: u32,
    /// Ultimo standard scelto nell'export PDF (F5-011, D-011): `"nessuno"`,
    /// `"a-2b"`, `"a-3b"` o `"a-4"`. Preselezionato alla prossima esportazione.
    pub standard_pdf: String,
    /// Barra laterale visibile (F5-019).
    pub barra_visibile: bool,
    /// Larghezza della barra laterale in pixel (F5-019).
    pub barra_larghezza: u32,
    /// Identificatore del pannello attivo della barra (F5-019): solo una
    /// parola `[a-z0-9-]`; se il frontend non lo conosce ripiega sul primo.
    pub pannello_attivo: String,
    /// Modalità focus: si schiarisce tutto tranne il paragrafo del cursore (F5-023).
    pub modalita_focus: bool,
    /// Modalità macchina da scrivere: la riga del cursore resta a metà altezza (F5-023).
    pub modalita_macchina: bool,
    /// Aiuto al completamento: documentazione/anteprima accanto ai suggerimenti
    /// dell'editor. Predefinito `true`; campo assente o non booleano → `true`.
    pub help_completamento: bool,
    /// Suggerimento dei parametri durante la digitazione di una chiamata.
    /// Predefinito `true`; campo assente o non booleano → `true`.
    pub suggerimento_parametri: bool,
    /// Percorsi assoluti dei documenti o delle cartelle aperti di recente, il
    /// più recente per primo, al massimo [`MAX_RECENTI`]. Predefinito: vuoto.
    pub recenti: Vec<String>,
}

/// Numero massimo di voci conservate in `recenti`.
pub const MAX_RECENTI: usize = 10;

impl Default for Impostazioni {
    fn default() -> Self {
        Self {
            tema: "sistema".to_string(),
            stile: "classico".to_string(),
            motore_typst: "lezer".to_string(),
            dimensione_font: 14,
            debounce_ms: 300,
            autosave: false,
            autosave_ms: 2000,
            cronologia_max_versioni: 50,
            cronologia_intervallo_s: 300,
            standard_pdf: "nessuno".to_string(),
            barra_visibile: true,
            barra_larghezza: 260,
            pannello_attivo: "indice".to_string(),
            modalita_focus: false,
            modalita_macchina: false,
            help_completamento: true,
            suggerimento_parametri: true,
            recenti: Vec::new(),
        }
    }
}

/// Valori ammessi per `tema`.
const TEMI: [&str; 3] = ["sistema", "chiaro", "scuro"];
/// Valori ammessi per `stile` (il primo è il predefinito).
const STILI: [&str; 3] = ["classico", "composizione", "strumento"];
/// Valori ammessi per `motore_typst` (il primo è il predefinito).
pub const MOTORI_TYPST: [&str; 2] = ["lezer", "wasm"];
/// Valori ammessi per `standard_pdf` (le varianti offerte dalla UI, D-011).
const STANDARD_PDF: [&str; 4] = ["nessuno", "a-2b", "a-3b", "a-4"];

impl Impostazioni {
    /// Riporta i valori in un intervallo sensato: tema sconosciuto →
    /// `"sistema"`, stile sconosciuto → `"classico"`, `motore_typst` non ammesso → `"lezer"`, `dimensione_font` in 8..=72, `debounce_ms` in 50..=5000,
    /// `autosave_ms` in 500..=60000, `cronologia_max_versioni` in 1..=1000,
    /// `cronologia_intervallo_s` in 0..=86400, `barra_larghezza` in 160..=600,
    /// `pannello_attivo` non valido → `"indice"`, `recenti` senza voci vuote e
    /// al massimo [`MAX_RECENTI`] (si tengono le prime).
    /// Applicata in lettura e in salvataggio.
    pub fn normalizza(mut self) -> Self {
        if !TEMI.contains(&self.tema.as_str()) {
            self.tema = Self::default().tema;
        }
        if !STILI.contains(&self.stile.as_str()) {
            self.stile = Self::default().stile;
        }
        if !MOTORI_TYPST.contains(&self.motore_typst.as_str()) {
            self.motore_typst = Self::default().motore_typst;
        }
        if !STANDARD_PDF.contains(&self.standard_pdf.as_str()) {
            self.standard_pdf = Self::default().standard_pdf;
        }
        self.dimensione_font = self.dimensione_font.clamp(8, 72);
        self.debounce_ms = self.debounce_ms.clamp(50, 5000);
        self.autosave_ms = self.autosave_ms.clamp(500, 60_000);
        self.cronologia_max_versioni = self.cronologia_max_versioni.clamp(1, 1000);
        self.cronologia_intervallo_s = self.cronologia_intervallo_s.min(86_400);
        self.barra_larghezza = self.barra_larghezza.clamp(160, 600);
        if !id_pannello_valido(&self.pannello_attivo) {
            self.pannello_attivo = Self::default().pannello_attivo;
        }
        self.recenti.retain(|voce| !voce.trim().is_empty());
        self.recenti.truncate(MAX_RECENTI);
        self
    }

    /// Parsing tollerante anche sui **tipi**: il testo deve essere un oggetto
    /// JSON (altrimenti default) e ogni campo è ricavato singolarmente, con
    /// ripiego al default se manca o ha il tipo sbagliato. Il risultato è
    /// normalizzato.
    fn da_json(testo: &str) -> Self {
        let Ok(serde_json::Value::Object(mappa)) = serde_json::from_str(testo) else {
            return Self::default();
        };
        let d = Self::default();
        Self {
            tema: campo(&mappa, "tema", d.tema),
            stile: campo(&mappa, "stile", d.stile),
            motore_typst: campo(&mappa, "motore_typst", d.motore_typst),
            dimensione_font: campo(&mappa, "dimensione_font", d.dimensione_font),
            debounce_ms: campo(&mappa, "debounce_ms", d.debounce_ms),
            autosave: campo(&mappa, "autosave", d.autosave),
            autosave_ms: campo(&mappa, "autosave_ms", d.autosave_ms),
            cronologia_max_versioni: campo(
                &mappa,
                "cronologia_max_versioni",
                d.cronologia_max_versioni,
            ),
            cronologia_intervallo_s: campo(
                &mappa,
                "cronologia_intervallo_s",
                d.cronologia_intervallo_s,
            ),
            standard_pdf: campo(&mappa, "standard_pdf", d.standard_pdf),
            barra_visibile: campo(&mappa, "barra_visibile", d.barra_visibile),
            barra_larghezza: campo(&mappa, "barra_larghezza", d.barra_larghezza),
            pannello_attivo: campo(&mappa, "pannello_attivo", d.pannello_attivo),
            modalita_focus: campo(&mappa, "modalita_focus", d.modalita_focus),
            modalita_macchina: campo(&mappa, "modalita_macchina", d.modalita_macchina),
            help_completamento: campo(&mappa, "help_completamento", d.help_completamento),
            suggerimento_parametri: campo(
                &mappa,
                "suggerimento_parametri",
                d.suggerimento_parametri,
            ),
            // Elenco tollerante: gli elementi non stringa si scartano uno a
            // uno (un `campo` tutto-o-niente perderebbe l'intero elenco).
            recenti: mappa
                .get("recenti")
                .and_then(serde_json::Value::as_array)
                .map(|voci| {
                    voci.iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or(d.recenti),
        }
        .normalizza()
    }
}

/// `true` se `id` è un identificatore di pannello accettabile: 1..=32 caratteri
/// fra minuscole ASCII, cifre e trattino (il valore finisce in un attributo DOM).
fn id_pannello_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Estrae e deserializza `chiave` da `mappa`; `predefinito` se manca o ha il
/// tipo sbagliato.
fn campo<T: serde::de::DeserializeOwned>(
    mappa: &serde_json::Map<String, serde_json::Value>,
    chiave: &str,
    predefinito: T,
) -> T {
    mappa
        .get(chiave)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or(predefinito)
}

/// Percorso di `impostazioni.json` nella config dir dell'OS, se determinabile.
fn percorso_impostazioni() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|base| {
        base.join(crate::NOME_CARTELLA_DATI)
            .join(NOME_FILE_IMPOSTAZIONI)
    })
}

/// Carica le impostazioni dalla config dir standard dell'OS.
///
/// **Infallibile**: default se la config dir non è determinabile.
pub fn carica_impostazioni() -> Impostazioni {
    match percorso_impostazioni() {
        Some(percorso) => carica_impostazioni_da(&percorso),
        None => Impostazioni::default(),
    }
}

/// Salva le impostazioni nella config dir standard (scrittura atomica).
pub fn salva_impostazioni(impostazioni: &Impostazioni) -> Result<(), String> {
    let percorso = percorso_impostazioni()
        .ok_or_else(|| "impossibile determinare la cartella di configurazione".to_string())?;
    salva_impostazioni_in(&percorso, impostazioni)
}

/// Implementazione testabile di [`carica_impostazioni`]: file mancante,
/// illeggibile o JSON invalido → default (il file non viene mai toccato).
fn carica_impostazioni_da(percorso: &Path) -> Impostazioni {
    std::fs::read_to_string(percorso)
        .map(|testo| Impostazioni::da_json(&testo))
        .unwrap_or_default()
}

/// Implementazione testabile di [`salva_impostazioni`].
fn salva_impostazioni_in(percorso: &Path, impostazioni: &Impostazioni) -> Result<(), String> {
    let contenuto = serde_json::to_string_pretty(&impostazioni.clone().normalizza())
        .map_err(|e| format!("serializzazione delle impostazioni: {e}"))?;
    scrivi_atomico(percorso, contenuto.as_bytes())
        .map_err(|e| format!("scrittura di '{}': {e}", percorso.display()))
}

/// Implementazione testabile: opera su un percorso esplicito. I test usano
/// sempre una cartella temporanea (`tempfile`), mai la config dir reale.
fn carica_snippet_da(percorso: &Path) -> Vec<Snippet> {
    if !percorso.exists() {
        // Primo avvio: scrittura atomica (`create_new`) dei predefiniti. Un
        // eventuale fallimento (permessi, cartella non creabile) degrada
        // comunque ai predefiniti tenuti in memoria per questa sessione.
        let _ = scrivi_predefiniti(percorso);
        return predefiniti();
    }

    let Ok(testo) = std::fs::read_to_string(percorso) else {
        return predefiniti();
    };
    // Il file deve essere un array JSON: se non lo è (o non è JSON affatto)
    // si degrada per intero ai predefiniti, come prima. Se invece È un
    // array, ogni elemento viene deserializzato singolarmente: uno snippet
    // malformato (campo mancante/tipo sbagliato) viene scartato senza far
    // fallire l'intero file, a differenza del precedente
    // `from_str::<Vec<Snippet>>` tutto-o-niente.
    let Ok(grezzo) = serde_json::from_str::<Vec<serde_json::Value>>(&testo) else {
        return predefiniti();
    };

    grezzo
        .into_iter()
        .filter_map(|valore| serde_json::from_value::<Snippet>(valore).ok())
        .filter(|s| !s.prefisso.trim().is_empty() && !s.corpo.trim().is_empty())
        .collect()
}

/// Scrive i predefiniti nel percorso indicato **solo se il file non esiste**
/// (non sovrascrive mai nulla: `AlreadyExists` altrimenti), con scrittura
/// atomica ([`scrivi_atomico`]): un crash non lascia un JSON troncato che il
/// caricamento scarterebbe come illeggibile.
fn scrivi_predefiniti(percorso: &Path) -> std::io::Result<()> {
    if percorso.exists() {
        return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    }
    let contenuto =
        serde_json::to_string_pretty(&predefiniti()).unwrap_or_else(|_| "[]".to_string());
    scrivi_atomico(percorso, contenuto.as_bytes())
}

/// I 6 snippet predefiniti offerti al primo avvio.
fn predefiniti() -> Vec<Snippet> {
    vec![
        Snippet {
            etichetta: "Figura".to_string(),
            prefisso: "figura".to_string(),
            corpo: "#figure(\n  image(\"${percorso}\", width: ${80%}),\n  caption: [${didascalia}],\n)${0}"
                .to_string(),
            descrizione: Some("Immagine con didascalia".to_string()),
        },
        Snippet {
            etichetta: "Tabella".to_string(),
            prefisso: "tabella".to_string(),
            corpo: "#table(\n  columns: ${2},\n  [${a}], [${b}],\n)${0}".to_string(),
            descrizione: Some("Tabella con intestazione".to_string()),
        },
        Snippet {
            etichetta: "Equazione".to_string(),
            prefisso: "equazione".to_string(),
            corpo: "$ ${a} = ${b} $${0}".to_string(),
            descrizione: Some("Equazione in display math".to_string()),
        },
        Snippet {
            etichetta: "Collegamento".to_string(),
            prefisso: "link".to_string(),
            corpo: "#link(\"${url}\")[${testo}]${0}".to_string(),
            descrizione: Some("Collegamento ipertestuale".to_string()),
        },
        Snippet {
            etichetta: "Codice".to_string(),
            prefisso: "codice".to_string(),
            corpo: "```${lingua}\n${codice}\n```${0}".to_string(),
            descrizione: Some("Blocco di codice con evidenziazione".to_string()),
        },
        Snippet {
            etichetta: "Intestazione".to_string(),
            prefisso: "intestazione".to_string(),
            corpo: "= ${titolo}${0}".to_string(),
            descrizione: Some("Titolo di sezione".to_string()),
        },
    ]
}

#[cfg(test)]
#[path = "../tests/unit/config.rs"]
mod test;
