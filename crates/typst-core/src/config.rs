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
}

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
    /// `pannello_attivo` non valido → `"indice"`.
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
mod test {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn scrivi_atomico_conserva_i_permessi_del_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("f.txt");
        std::fs::write(&p, b"vecchio").expect("scrittura iniziale");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).expect("chmod");
        scrivi_atomico(&p, b"nuovo").expect("scrittura atomica");
        let modo = std::fs::metadata(&p).expect("meta").permissions().mode();
        assert_eq!(
            modo & 0o777,
            0o640,
            "i permessi devono restare quelli del file"
        );
        assert_eq!(std::fs::read(&p).expect("lettura"), b"nuovo");
    }

    #[test]
    #[cfg(windows)]
    fn scrivi_atomico_con_rename_fallito_su_bersaglio_readonly_non_lascia_orfani() {
        // Regressione: il temporaneo ereditava il flag read-only e, se il rename
        // falliva (bersaglio aperto senza condivisione da un altro programma),
        // la rimozione falliva lasciando `.nome.tmp-…` nella cartella.
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("ro.txt");
        std::fs::write(&p, b"originale").expect("scrittura");
        let mut perm = std::fs::metadata(&p).expect("meta").permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm).expect("readonly");
        // Apertura senza condivisione: il rename sul bersaglio non può riuscire.
        let _bloccato = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&p)
            .expect("apertura esclusiva");

        let esito = scrivi_atomico(&p, b"nuovo");
        assert!(esito.is_err(), "il rename doveva fallire: {esito:?}");

        let nomi: Vec<String> = std::fs::read_dir(dir.path())
            .expect("dir")
            .map(|e| e.expect("voce").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomi, vec!["ro.txt".to_string()], "nessun orfano: {nomi:?}");

        drop(_bloccato);
        let mut perm = std::fs::metadata(&p).expect("meta").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(&p, perm);
    }

    #[test]
    fn scrivi_atomico_su_bersaglio_readonly_non_lascia_orfani() {
        // Regressione: su Windows il temporaneo ereditava il read-only e la
        // rimozione nel ramo d'errore falliva, lasciando `.nome.tmp-…`.
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("ro.txt");
        std::fs::write(&p, b"originale").expect("scrittura");
        let mut perm = std::fs::metadata(&p).expect("meta").permissions();
        perm.set_readonly(true);
        std::fs::set_permissions(&p, perm).expect("readonly");

        let _ = scrivi_atomico(&p, b"nuovo"); // l'esito dipende dalla piattaforma

        let nomi: Vec<String> = std::fs::read_dir(dir.path())
            .expect("dir")
            .map(|e| e.expect("voce").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomi, vec!["ro.txt".to_string()], "nessun orfano: {nomi:?}");

        // pulizia: la tempdir deve poter rimuovere il file
        let mut perm = std::fs::metadata(&p).expect("meta").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perm.set_readonly(false);
        let _ = std::fs::set_permissions(&p, perm);
    }

    #[test]
    fn scrivi_atomico_non_lascia_temporanei_e_non_imposta_readonly() {
        // Dopo la scrittura su un file NON read-only il flag resta falso
        // (copia dei permessi corretta) e non restano temporanei.
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("g.txt");
        std::fs::write(&p, b"a").expect("scrittura");
        scrivi_atomico(&p, b"b").expect("atomica");
        assert!(!std::fs::metadata(&p)
            .expect("meta")
            .permissions()
            .readonly());
        let rimasti = std::fs::read_dir(dir.path()).expect("dir").count();
        assert_eq!(rimasti, 1, "nessun temporaneo residuo");
    }

    #[test]
    fn scrivi_atomico_scrive_attraverso_un_symlink() {
        let dir = tempfile::tempdir().expect("tempdir");
        let reale_dir = dir.path().join("reale");
        std::fs::create_dir_all(&reale_dir).expect("mkdir");
        let reale = reale_dir.join("dati.json");
        std::fs::write(&reale, b"vecchio").expect("scrittura");
        let link = dir.path().join("collegamento.json");
        #[cfg(unix)]
        let creato = std::os::unix::fs::symlink(&reale, &link);
        #[cfg(windows)]
        let creato = std::os::windows::fs::symlink_file(&reale, &link);
        #[cfg(not(any(unix, windows)))]
        let creato: std::io::Result<()> = Err(std::io::ErrorKind::Unsupported.into());
        if let Err(e) = creato {
            // Windows senza privilegio/Developer Mode: test non applicabile.
            eprintln!("symlink non creabile ({e}): test saltato");
            return;
        }
        scrivi_atomico(&link, b"nuovo").expect("scrittura via symlink");
        assert!(
            std::fs::symlink_metadata(&link)
                .expect("meta link")
                .file_type()
                .is_symlink(),
            "il collegamento deve restare un symlink"
        );
        assert_eq!(std::fs::read(&reale).expect("lettura reale"), b"nuovo");
        // Nessun temporaneo né accanto al link né accanto al file reale.
        assert_eq!(std::fs::read_dir(&reale_dir).expect("dir").count(), 1);
        assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 2);
    }

    #[test]
    fn crea_predefiniti_al_primo_avvio() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 6);
        assert!(
            percorso.exists(),
            "il file dei predefiniti dovrebbe essere stato scritto"
        );
    }

    #[test]
    fn scrivi_predefiniti_non_sovrascrive_e_non_lascia_temporanei() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        scrivi_predefiniti(&percorso).expect("primo avvio");
        let json = std::fs::read_to_string(&percorso).expect("lettura");
        assert_eq!(
            serde_json::from_str::<Vec<Snippet>>(&json)
                .expect("json")
                .len(),
            6
        );
        // Un file utente esistente non si tocca mai.
        std::fs::write(&percorso, "[]").expect("file utente");
        let e = scrivi_predefiniti(&percorso).expect_err("esiste già");
        assert_eq!(e.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&percorso).expect("lettura"), "[]");
        let residui = std::fs::read_dir(dir.path()).expect("dir").count();
        assert_eq!(residui, 1, "nessun temporaneo residuo");
    }

    #[test]
    fn degrada_su_json_invalido_senza_sovrascrivere_file_utente() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        std::fs::write(&percorso, "questo non è json valido").expect("scrittura fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 6); // predefiniti in memoria, degradati con grazia

        let contenuto = std::fs::read_to_string(&percorso).expect("rilettura fixture");
        assert_eq!(
            contenuto, "questo non è json valido",
            "il file utente non va sovrascritto"
        );
    }

    #[test]
    fn degrada_su_json_valido_ma_non_array_senza_sovrascrivere_file_utente() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        std::fs::write(&percorso, r#"{"non": "un array"}"#).expect("scrittura fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 6); // predefiniti in memoria, degradati con grazia

        let contenuto = std::fs::read_to_string(&percorso).expect("rilettura fixture");
        assert_eq!(
            contenuto, r#"{"non": "un array"}"#,
            "il file utente non va sovrascritto"
        );
    }

    #[test]
    fn scarta_solo_la_voce_malformata_mantenendo_le_altre() {
        // Un solo snippet malformato (qui: `prefisso` con un tipo sbagliato,
        // un numero invece di una stringa) non deve più scartare l'intero
        // file: le voci valide restano disponibili (bug critico, prima
        // `from_str::<Vec<Snippet>>` era tutto-o-niente).
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        let json = r#"[
            {"etichetta": "buono1", "prefisso": "b1", "corpo": "corpo1"},
            {"etichetta": "malformato", "prefisso": 42, "corpo": "corpo"},
            {"etichetta": "buono2", "prefisso": "b2", "corpo": "corpo2"}
        ]"#;
        std::fs::write(&percorso, json).expect("scrittura fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(
            voci.len(),
            2,
            "solo la voce malformata va scartata: {voci:?}"
        );
        assert!(voci.iter().any(|s| s.prefisso == "b1"));
        assert!(voci.iter().any(|s| s.prefisso == "b2"));
    }

    #[test]
    fn scarta_voci_con_prefisso_o_corpo_vuoti() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        let json = r#"[
            {"etichetta": "ok", "prefisso": "ok", "corpo": "corpo"},
            {"etichetta": "vuoto1", "prefisso": "", "corpo": "corpo"},
            {"etichetta": "vuoto2", "prefisso": "p", "corpo": "   "}
        ]"#;
        std::fs::write(&percorso, json).expect("scrittura fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].etichetta, "ok");
    }

    #[test]
    fn carica_snippet_utente_validi() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        let json = r#"[{"etichetta": "mio", "prefisso": "mio", "corpo": "corpo mio"}]"#;
        std::fs::write(&percorso, json).expect("scrittura fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 1);
        assert_eq!(voci[0].prefisso, "mio");
    }

    #[test]
    fn file_illeggibile_come_directory_degrada_ai_predefiniti() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Una cartella al posto del file: `read_to_string` fallisce.
        let percorso = dir.path().join(NOME_FILE_SNIPPET);
        std::fs::create_dir(&percorso).expect("crea cartella fixture");

        let voci = carica_snippet_da(&percorso);
        assert_eq!(voci.len(), 6);
    }

    #[test]
    fn migra_copia_la_vecchia_se_la_nuova_manca() {
        let dir = tempfile::tempdir().expect("tempdir");
        let vecchia = dir.path().join(CARTELLA_PRECEDENTE);
        std::fs::create_dir(&vecchia).expect("crea vecchia");
        let json = r#"[{"etichetta": "mio", "prefisso": "mio", "corpo": "corpo"}]"#;
        std::fs::write(vecchia.join(NOME_FILE_SNIPPET), json).expect("fixture");

        assert!(migra_snippet_da(dir.path()));
        let nuova = dir
            .path()
            .join(crate::NOME_CARTELLA_DATI)
            .join(NOME_FILE_SNIPPET);
        assert_eq!(std::fs::read_to_string(&nuova).expect("nuova"), json);
        // La vecchia resta intatta.
        assert_eq!(
            std::fs::read_to_string(vecchia.join(NOME_FILE_SNIPPET)).expect("vecchia"),
            json
        );
        // E il caricamento successivo vede lo snippet migrato, non i predefiniti.
        assert_eq!(carica_snippet_da(&nuova).len(), 1);
    }

    #[test]
    fn migra_non_tocca_la_nuova_gia_presente() {
        let dir = tempfile::tempdir().expect("tempdir");
        let vecchia = dir.path().join(CARTELLA_PRECEDENTE);
        let nuova_dir = dir.path().join(crate::NOME_CARTELLA_DATI);
        std::fs::create_dir(&vecchia).expect("crea vecchia");
        std::fs::create_dir(&nuova_dir).expect("crea nuova");
        std::fs::write(vecchia.join(NOME_FILE_SNIPPET), "[]").expect("fixture");
        std::fs::write(nuova_dir.join(NOME_FILE_SNIPPET), "nuova").expect("fixture");

        assert!(!migra_snippet_da(dir.path()));
        assert_eq!(
            std::fs::read_to_string(nuova_dir.join(NOME_FILE_SNIPPET)).expect("nuova"),
            "nuova"
        );
    }

    #[test]
    fn migra_con_vecchia_assente_non_fa_nulla_e_restano_i_predefiniti() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!migra_snippet_da(dir.path()));
        let nuova = dir
            .path()
            .join(crate::NOME_CARTELLA_DATI)
            .join(NOME_FILE_SNIPPET);
        assert!(!nuova.exists());
        assert_eq!(carica_snippet_da(&nuova).len(), 6);
    }

    #[test]
    fn migra_con_errore_io_degrada_ai_predefiniti_senza_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let vecchia = dir.path().join(CARTELLA_PRECEDENTE);
        std::fs::create_dir(&vecchia).expect("crea vecchia");
        std::fs::write(vecchia.join(NOME_FILE_SNIPPET), "[]").expect("fixture");
        // Un FILE al posto della cartella nuova: create_dir_all fallisce.
        std::fs::write(dir.path().join(crate::NOME_CARTELLA_DATI), "x").expect("fixture");

        assert!(!migra_snippet_da(dir.path()));
        let nuova = dir
            .path()
            .join(crate::NOME_CARTELLA_DATI)
            .join(NOME_FILE_SNIPPET);
        assert_eq!(carica_snippet_da(&nuova).len(), 6);
    }

    #[test]
    fn impostazioni_default_su_file_mancante() {
        let dir = tempfile::tempdir().expect("tempdir");
        let imp = carica_impostazioni_da(&dir.path().join(NOME_FILE_IMPOSTAZIONI));
        assert_eq!(imp, Impostazioni::default());
    }

    #[test]
    fn impostazioni_default_su_json_invalido_senza_sovrascrivere() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        std::fs::write(&percorso, "non json").expect("fixture");
        assert_eq!(carica_impostazioni_da(&percorso), Impostazioni::default());
        assert_eq!(
            std::fs::read_to_string(&percorso).expect("rilettura"),
            "non json"
        );
    }

    #[test]
    fn impostazioni_voce_sconosciuta_ignorata_e_mancanti_a_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        std::fs::write(&percorso, r#"{"tema": "scuro", "futura": [1, 2]}"#).expect("fixture");
        let imp = carica_impostazioni_da(&percorso);
        assert_eq!(imp.tema, "scuro");
        assert_eq!(imp.dimensione_font, Impostazioni::default().dimensione_font);
    }

    #[test]
    fn impostazioni_round_trip_crea_la_cartella() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join("sotto").join(NOME_FILE_IMPOSTAZIONI);
        let imp = Impostazioni {
            tema: "chiaro".to_string(),
            stile: "composizione".to_string(),
            motore_typst: "wasm".to_string(),
            dimensione_font: 18,
            debounce_ms: 500,
            autosave: true,
            autosave_ms: 4000,
            cronologia_max_versioni: 20,
            cronologia_intervallo_s: 60,
            standard_pdf: "a-2b".to_string(),
            // Tutti diversi dal default: un campo dimenticato nel parsing o nel salvataggio farebbe fallire il confronto.
            barra_visibile: false,
            barra_larghezza: 320,
            pannello_attivo: "file".to_string(),
            modalita_focus: true,
            modalita_macchina: true,
            help_completamento: false,
            suggerimento_parametri: false,
        };
        salva_impostazioni_in(&percorso, &imp).expect("salvataggio");
        assert_eq!(carica_impostazioni_da(&percorso), imp);
    }

    #[test]
    fn impostazioni_help_e_suggerimento_default_true() {
        let d = Impostazioni::default();
        assert!(d.help_completamento && d.suggerimento_parametri);
        let j = Impostazioni::da_json("{}");
        assert!(j.help_completamento && j.suggerimento_parametri);
    }

    #[test]
    fn impostazioni_help_e_suggerimento_round_trip_e_nome_serializzato() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        let imp = Impostazioni {
            help_completamento: false,
            suggerimento_parametri: false,
            ..Impostazioni::default()
        };
        salva_impostazioni_in(&percorso, &imp).expect("salvataggio");
        let letto = carica_impostazioni_da(&percorso);
        assert!(!letto.help_completamento && !letto.suggerimento_parametri);
        let v = serde_json::to_value(&imp).expect("serializza");
        assert_eq!(v["help_completamento"], false);
        assert_eq!(v["suggerimento_parametri"], false);
        // I due campi sono indipendenti fra loro.
        let a = Impostazioni::da_json(r#"{"help_completamento":false}"#);
        assert!(!a.help_completamento && a.suggerimento_parametri);
        let b = Impostazioni::da_json(r#"{"suggerimento_parametri":false}"#);
        assert!(b.help_completamento && !b.suggerimento_parametri);
    }

    #[test]
    fn impostazioni_help_e_suggerimento_file_vecchio_senza_campi_restano_true() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        std::fs::write(&percorso, r#"{"tema":"scuro","dimensione_font":20}"#).expect("fixture");
        let imp = carica_impostazioni_da(&percorso);
        assert!(imp.help_completamento && imp.suggerimento_parametri);
        assert_eq!((imp.tema.as_str(), imp.dimensione_font), ("scuro", 20));
    }

    #[test]
    fn impostazioni_help_e_suggerimento_valore_non_booleano_torna_true() {
        for cattivo in [
            r#""no""#,
            r#""false""#,
            "0",
            "1",
            "null",
            "[]",
            r#"{"a":1}"#,
        ] {
            let imp = Impostazioni::da_json(&format!(
                r#"{{"help_completamento":{cattivo},"suggerimento_parametri":{cattivo}}}"#
            ));
            assert!(imp.help_completamento, "{cattivo}");
            assert!(imp.suggerimento_parametri, "{cattivo}");
        }
    }

    #[test]
    fn impostazioni_help_e_suggerimento_indipendenti_da_altri_campi_e_futuri_ignorati() {
        let imp = Impostazioni::da_json(
            r#"{"help_completamento":false,"tema":"blu","modalita_focus":true,"futuro":{"x":1},"altro":[1]}"#,
        );
        assert!(!imp.help_completamento);
        assert!(imp.suggerimento_parametri);
        assert_eq!((imp.tema.as_str(), imp.modalita_focus), ("sistema", true));
        let imp2 = Impostazioni::da_json(r#"{"suggerimento_parametri":false,"autosave":true}"#);
        assert!(!imp2.suggerimento_parametri && imp2.help_completamento && imp2.autosave);
    }

    #[test]
    fn impostazioni_stile_default_classico() {
        assert_eq!(Impostazioni::default().stile, "classico");
        assert_eq!(Impostazioni::da_json("{}").stile, "classico");
    }

    #[test]
    fn impostazioni_stile_file_vecchio_senza_campo_resta_classico() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        std::fs::write(&percorso, r#"{"tema":"scuro","dimensione_font":20}"#).expect("fixture");
        let imp = carica_impostazioni_da(&percorso);
        assert_eq!(imp.stile, "classico");
        assert_eq!((imp.tema.as_str(), imp.dimensione_font), ("scuro", 20));
    }

    #[test]
    fn impostazioni_stile_valori_ammessi_si_conservano_e_nome_serializzato() {
        for ok in ["classico", "composizione", "strumento"] {
            let imp = Impostazioni::da_json(&format!(r#"{{"stile":"{ok}"}}"#));
            assert_eq!(imp.stile, ok);
        }
        let v = serde_json::to_value(Impostazioni {
            stile: "strumento".into(),
            ..Impostazioni::default()
        })
        .expect("serializza");
        assert_eq!(v["stile"], "strumento");
    }

    #[test]
    fn impostazioni_stile_sconosciuto_o_di_tipo_sbagliato_torna_classico() {
        for cattivo in [r#""neon""#, r#""""#, r#""Classico""#, "3", "null", "[]"] {
            let imp = Impostazioni::da_json(&format!(r#"{{"stile":{cattivo}}}"#));
            assert_eq!(imp.stile, "classico", "{cattivo}");
        }
        // Anche in salvataggio (normalizza), non solo in lettura.
        let n = Impostazioni {
            stile: "neon".into(),
            ..Impostazioni::default()
        }
        .normalizza();
        assert_eq!(n.stile, "classico");
    }

    #[test]
    fn impostazioni_stile_valido_e_tema_non_valido_sono_indipendenti() {
        let a = Impostazioni::da_json(r#"{"stile":"composizione","tema":"blu"}"#);
        assert_eq!(
            (a.stile.as_str(), a.tema.as_str()),
            ("composizione", "sistema")
        );
        let b = Impostazioni::da_json(r#"{"stile":"neon","tema":"scuro"}"#);
        assert_eq!((b.stile.as_str(), b.tema.as_str()), ("classico", "scuro"));
    }

    #[test]
    fn impostazioni_campi_futuri_ignorati_senza_perdere_stile() {
        let imp = Impostazioni::da_json(r#"{"stile":"strumento","futuro":{"x":1},"altro":[1]}"#);
        assert_eq!(imp.stile, "strumento");
    }

    #[test]
    fn impostazioni_motore_typst_default_lezer() {
        assert_eq!(Impostazioni::default().motore_typst, "lezer");
        assert_eq!(Impostazioni::da_json("{}").motore_typst, "lezer");
    }

    #[test]
    fn impostazioni_motore_typst_round_trip_wasm_e_nome_serializzato() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        let imp = Impostazioni {
            motore_typst: "wasm".into(),
            ..Impostazioni::default()
        };
        salva_impostazioni_in(&percorso, &imp).expect("salvataggio");
        assert_eq!(carica_impostazioni_da(&percorso).motore_typst, "wasm");
        let v = serde_json::to_value(&imp).expect("serializza");
        assert_eq!(v["motore_typst"], "wasm");
        for ok in MOTORI_TYPST {
            let i = Impostazioni::da_json(&format!(r#"{{"motore_typst":"{ok}"}}"#));
            assert_eq!(i.motore_typst, ok);
        }
    }

    #[test]
    fn impostazioni_motore_typst_non_ammesso_torna_lezer() {
        for cattivo in [
            r#""cm6""#,
            r#""""#,
            r#""LEZER""#,
            r#""Wasm""#,
            r#"" lezer""#,
            "3",
            "true",
            "null",
            "[]",
            r#"{"a":1}"#,
        ] {
            let imp = Impostazioni::da_json(&format!(r#"{{"motore_typst":{cattivo}}}"#));
            assert_eq!(imp.motore_typst, "lezer", "{cattivo}");
        }
        // Anche in salvataggio (normalizza), non solo in lettura.
        let n = Impostazioni {
            motore_typst: "Lezer".into(),
            ..Impostazioni::default()
        }
        .normalizza();
        assert_eq!(n.motore_typst, "lezer");
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        let sporco = Impostazioni {
            motore_typst: "xyz".into(),
            ..Impostazioni::default()
        };
        salva_impostazioni_in(&percorso, &sporco).expect("salvataggio");
        let scritto = std::fs::read_to_string(&percorso).expect("lettura");
        assert!(scritto.contains(r#""motore_typst": "lezer""#), "{scritto}");
    }

    #[test]
    fn impostazioni_motore_typst_file_vecchio_senza_campo_resta_lezer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        std::fs::write(
            &percorso,
            r#"{"tema":"scuro","stile":"strumento","dimensione_font":20}"#,
        )
        .expect("fixture");
        let imp = carica_impostazioni_da(&percorso);
        assert_eq!(imp.motore_typst, "lezer");
        assert_eq!(
            (imp.tema.as_str(), imp.stile.as_str(), imp.dimensione_font),
            ("scuro", "strumento", 20)
        );
    }

    #[test]
    fn impostazioni_motore_typst_indipendente_da_tema_e_stile() {
        let a = Impostazioni::da_json(r#"{"motore_typst":"wasm","tema":"blu","stile":"neon"}"#);
        assert_eq!(
            (a.motore_typst.as_str(), a.tema.as_str(), a.stile.as_str()),
            ("wasm", "sistema", "classico")
        );
        let b =
            Impostazioni::da_json(r#"{"motore_typst":"cm6","tema":"scuro","stile":"strumento"}"#);
        assert_eq!(
            (b.motore_typst.as_str(), b.tema.as_str(), b.stile.as_str()),
            ("lezer", "scuro", "strumento")
        );
    }

    #[test]
    fn impostazioni_campi_futuri_ignorati_senza_perdere_motore_typst() {
        let imp = Impostazioni::da_json(r#"{"motore_typst":"wasm","futuro":{"x":1},"altro":[1]}"#);
        assert_eq!(imp.motore_typst, "wasm");
    }

    #[test]
    fn impostazioni_campo_di_tipo_sbagliato_torna_al_default() {
        let imp = Impostazioni::da_json(r#"{"tema":"scuro","dimensione_font":"grande"}"#);
        assert_eq!(imp.tema, "scuro");
        assert_eq!(imp.dimensione_font, 14);
    }

    #[test]
    fn impostazioni_json_non_oggetto_da_default() {
        assert_eq!(Impostazioni::da_json("[1, 2]"), Impostazioni::default());
        assert_eq!(Impostazioni::da_json("42"), Impostazioni::default());
    }

    #[test]
    fn impostazioni_normalizza_tema_e_clamp() {
        let imp = Impostazioni::da_json(r#"{"tema":"blu","dimensione_font":1000,"debounce_ms":1}"#);
        assert_eq!(imp.tema, "sistema");
        assert_eq!(imp.dimensione_font, 72);
        assert_eq!(imp.debounce_ms, 50);
    }

    #[test]
    fn impostazioni_autosave_e_cronologia_hanno_limiti() {
        let imp = Impostazioni::da_json(
            r#"{"autosave_ms":1,"cronologia_max_versioni":0,"cronologia_intervallo_s":999999}"#,
        );
        assert_eq!(imp.autosave_ms, 500);
        assert_eq!(imp.cronologia_max_versioni, 1);
        assert_eq!(imp.cronologia_intervallo_s, 86_400);
        let grande =
            Impostazioni::da_json(r#"{"autosave_ms":9999999,"cronologia_max_versioni":99999}"#);
        assert_eq!(grande.autosave_ms, 60_000);
        assert_eq!(grande.cronologia_max_versioni, 1000);
        // Assenti: default.
        let d = Impostazioni::da_json("{}");
        assert_eq!((d.autosave_ms, d.cronologia_max_versioni), (2000, 50));
    }

    #[test]
    fn impostazioni_standard_pdf_ammette_solo_le_varianti_della_ui() {
        for ok in ["nessuno", "a-2b", "a-3b", "a-4"] {
            let imp = Impostazioni::da_json(&format!(r#"{{"standard_pdf":"{ok}"}}"#));
            assert_eq!(imp.standard_pdf, ok);
        }
        // Sconosciuto (anche se valido per il core) o di tipo sbagliato → nessuno.
        for cattivo in [r#""a-1b""#, r#""""#, "3", "null"] {
            let imp = Impostazioni::da_json(&format!(r#"{{"standard_pdf":{cattivo}}}"#));
            assert_eq!(imp.standard_pdf, "nessuno", "{cattivo}");
        }
        assert_eq!(Impostazioni::da_json("{}").standard_pdf, "nessuno");
    }

    #[test]
    fn impostazioni_barra_e_modalita_hanno_default_e_limiti() {
        // File di una versione precedente (senza i campi nuovi): default.
        let d = Impostazioni::da_json(r#"{"tema":"scuro"}"#);
        assert!(d.barra_visibile);
        assert_eq!(d.barra_larghezza, 260);
        assert_eq!(d.pannello_attivo, "indice");
        assert!(!d.modalita_focus && !d.modalita_macchina);
        // Valori validi si conservano.
        let ok = Impostazioni::da_json(
            r#"{"barra_visibile":false,"barra_larghezza":300,"pannello_attivo":"file-2","modalita_focus":true,"modalita_macchina":true}"#,
        );
        assert!(!ok.barra_visibile);
        assert_eq!(ok.barra_larghezza, 300);
        assert_eq!(ok.pannello_attivo, "file-2");
        assert!(ok.modalita_focus && ok.modalita_macchina);
        // Larghezza fuori limiti: clamp.
        assert_eq!(
            Impostazioni::da_json(r#"{"barra_larghezza":1}"#).barra_larghezza,
            160
        );
        assert_eq!(
            Impostazioni::da_json(r#"{"barra_larghezza":99999}"#).barra_larghezza,
            600
        );
        // Identificatore non ammesso (maiuscole, spazi, markup, troppo lungo, vuoto, tipo sbagliato).
        let lungo = format!(r#"{{"pannello_attivo":"{}"}}"#, "a".repeat(33));
        for cattivo in [
            r#"{"pannello_attivo":"Indice"}"#,
            r#"{"pannello_attivo":"a b"}"#,
            r#"{"pannello_attivo":"<x>"}"#,
            r#"{"pannello_attivo":""}"#,
            r#"{"pannello_attivo":5}"#,
            r#"{"barra_visibile":"si","pannello_attivo":null}"#,
            lungo.as_str(),
        ] {
            assert_eq!(
                Impostazioni::da_json(cattivo).pannello_attivo,
                "indice",
                "{cattivo}"
            );
        }
        // Tipo sbagliato del booleano: default.
        assert!(Impostazioni::da_json(r#"{"barra_visibile":"no"}"#).barra_visibile);
        // Il salvataggio normalizza come la lettura.
        let n = Impostazioni {
            barra_larghezza: 5,
            pannello_attivo: "X".into(),
            ..Impostazioni::default()
        }
        .normalizza();
        assert_eq!(
            (n.barra_larghezza, n.pannello_attivo.as_str()),
            (160, "indice")
        );
    }

    #[test]
    fn salvataggio_atomico_sostituisce_e_non_lascia_temporanei() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = dir.path().join(NOME_FILE_IMPOSTAZIONI);
        let a = Impostazioni {
            tema: "chiaro".to_string(),
            ..Impostazioni::default()
        };
        let b = Impostazioni {
            tema: "scuro".to_string(),
            ..Impostazioni::default()
        };
        salva_impostazioni_in(&percorso, &a).expect("salva A");
        salva_impostazioni_in(&percorso, &b).expect("salva B");
        assert_eq!(carica_impostazioni_da(&percorso), b);
        let nomi: Vec<String> = std::fs::read_dir(dir.path())
            .expect("read_dir")
            .map(|e| e.expect("voce").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(nomi, vec![NOME_FILE_IMPOSTAZIONI.to_string()]);
    }

    #[test]
    fn carica_snippet_in_migra_prima_di_creare_i_predefiniti() {
        let dir = tempfile::tempdir().expect("tempdir");
        let vecchia = dir.path().join(CARTELLA_PRECEDENTE);
        std::fs::create_dir(&vecchia).expect("crea vecchia");
        let json = r#"[{"etichetta": "mio", "prefisso": "mio", "corpo": "corpo"}]"#;
        std::fs::write(vecchia.join(NOME_FILE_SNIPPET), json).expect("fixture");

        let voci = carica_snippet_in(dir.path());
        assert_eq!(voci.len(), 1, "non i 6 predefiniti: {voci:?}");
        let nuova = dir
            .path()
            .join(crate::NOME_CARTELLA_DATI)
            .join(NOME_FILE_SNIPPET);
        assert_eq!(std::fs::read_to_string(nuova).expect("nuova"), json);
    }
}
