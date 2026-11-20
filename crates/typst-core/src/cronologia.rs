//! Cronologia locale delle versioni di un documento (F5-013, DECISIONI.md D-012).
//!
//! Gli snapshot vivono nella **cartella dati dell'app**
//! (`dirs::data_dir()/typster/cronologia/<chiave>/`), non accanto al progetto:
//! le cartelle dei documenti restano pulite. La `<chiave>` è lo SHA-256, in
//! esadecimale, del percorso assoluto normalizzato del file (su Windows in
//! minuscolo, perché il filesystem non distingue le maiuscole); ogni cartella
//! contiene gli snapshot in chiaro (`AAAAMMGG-HHMMSSZ.<estensione>`, ora **UTC**:
//! il nome non dipende dal fuso né dall'ora legale; l'elenco mostra l'ora
//! locale) e un
//! `meta.json` con il percorso originale (leggibilità, diagnostica e controllo
//! di coerenza: se il percorso registrato non coincide si usa la chiave
//! successiva, `<chiave>-1`, `<chiave>-2`, …).
//!
//! # Ordine delle scritture
//! Prima lo snapshot (creato con `create_new` sul nome definitivo: due scrittori
//! non si sovrascrivono mai), poi la rotazione: un crash a metà lascia al più
//! una versione in più, mai una in meno del previsto.
//!
//! Il modulo è puro (nessuna dipendenza da Tauri/UI): l'istante corrente è
//! passato dal chiamante, così i test controllano il tempo.

use std::io;
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::scrivi_atomico;

/// Sottocartella della cartella dati del prodotto che contiene le cronologie.
const NOME_CARTELLA: &str = "cronologia";
/// Nome del file di metadati dentro la cartella di ciascun documento.
const NOME_META: &str = "meta.json";
/// Formato della parte data-ora del nome di uno snapshot (segue la `Z` UTC).
const FORMATO_TEMPO: &str = "%Y%m%d-%H%M%S";
/// Lunghezza (caratteri) di `AAAAMMGG-HHMMSS`.
const LUNGHEZZA_DATA: usize = 15;
/// Lunghezza (caratteri) del prefisso temporale completo `AAAAMMGG-HHMMSSZ`.
const LUNGHEZZA_TEMPO: usize = LUNGHEZZA_DATA + 1;
/// Tetto ai tentativi di chiave/nome alternativi (collisioni): oltre, errore.
const MAX_ALTERNATIVE: u32 = 1000;

/// SHA-256 di `dati` in esadecimale minuscolo (64 caratteri).
pub fn sha256_hex(dati: &[u8]) -> String {
    let digest = Sha256::digest(dati);
    let mut testo = String::with_capacity(64);
    for byte in digest {
        testo.push_str(&format!("{byte:02x}"));
    }
    testo
}

/// Forma canonica testuale di un percorso, usata per la chiave e per i
/// confronti: toglie il prefisso "verbatim" di Windows (`\\?\`, `\\?\UNC\`),
/// unifica i separatori in `/`, elimina il `/` finale e, se
/// `senza_maiuscole`, porta tutto in minuscolo.
///
/// Parametrizzata sul flag (invece di `cfg!`) per poter testare entrambi i
/// comportamenti su ogni piattaforma.
pub fn normalizza_percorso(percorso: &str, senza_maiuscole: bool) -> String {
    let mut p = percorso.replace('\\', "/");
    if let Some(resto) = p.strip_prefix("//?/UNC/") {
        p = format!("//{resto}");
    } else if let Some(resto) = p.strip_prefix("//?/") {
        p = resto.to_string();
    }
    while p.len() > 1 && p.ends_with('/') {
        p.pop();
    }
    if senza_maiuscole {
        p = p.to_lowercase();
    }
    p
}

/// `true` sulle piattaforme il cui filesystem, di norma, ignora le maiuscole.
pub(crate) const fn fs_senza_maiuscole() -> bool {
    cfg!(windows)
}

/// Percorso assoluto "più canonico possibile" di un file che potrebbe non
/// esistere più (es. cancellato o rinominato da fuori): il file stesso se
/// esiste, altrimenti la cartella genitrice canonicalizzata + il nome, altrimenti
/// il percorso così com'è.
pub(crate) fn percorso_stabile(percorso: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(percorso) {
        return c;
    }
    if let (Some(genitore), Some(nome)) = (percorso.parent(), percorso.file_name()) {
        if let Ok(c) = std::fs::canonicalize(genitore) {
            return c.join(nome);
        }
    }
    percorso.to_path_buf()
}

/// Forma testuale normalizzata del percorso di un file, per chiavi e confronti.
pub(crate) fn percorso_normalizzato(percorso: &Path) -> String {
    normalizza_percorso(
        &percorso_stabile(percorso).to_string_lossy(),
        fs_senza_maiuscole(),
    )
}

/// Chiave della cronologia di un file: SHA-256 (esadecimale) del percorso
/// normalizzato (vedi [`normalizza_percorso`]). Stabile fra esecuzioni e fra
/// versioni di Rust (a differenza di `DefaultHasher`).
pub fn chiave_percorso(percorso: &Path) -> String {
    sha256_hex(percorso_normalizzato(percorso).as_bytes())
}

/// Istante corrente in **UTC**, nel formato atteso da [`Cronologia::registra`]
/// (i nomi degli snapshot sono in UTC).
pub fn ora_utc() -> NaiveDateTime {
    chrono::Utc::now().naive_utc()
}

/// Rappresentazione leggibile, in ora **locale**, di un istante UTC (solo per la vista).
fn vista_locale(utc: NaiveDateTime) -> String {
    use chrono::TimeZone;
    chrono::Local
        .from_utc_datetime(&utc)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Registra una versione dopo un salvataggio, con i limiti delle impostazioni
/// (`cronologia_max_versioni`, `cronologia_intervallo_s`). Best-effort: un
/// errore della cronologia non deve mai far fallire il salvataggio, quindi è
/// solo segnalato su stderr.
///
/// `automatico` applica l'intervallo minimo (l'autosave non riempie la
/// cronologia); il salvataggio manuale crea sempre una versione se il
/// contenuto è cambiato.
pub fn registra_dopo_salvataggio(percorso: &Path, contenuto: &str, automatico: bool) {
    let Some(cronologia) = Cronologia::nella_cartella_dati() else {
        return;
    };
    let impostazioni = crate::config::carica_impostazioni();
    let intervallo = automatico
        .then(|| std::time::Duration::from_secs(u64::from(impostazioni.cronologia_intervallo_s)));
    if let Err(e) = cronologia.registra(
        percorso,
        contenuto,
        ora_utc(),
        impostazioni.cronologia_max_versioni as usize,
        intervallo,
    ) {
        eprintln!("Versione non registrata in cronologia: {e}");
    }
}

/// Contenuto di `meta.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Meta {
    /// Percorso (come scritto dall'app) a cui appartiene la cronologia.
    percorso: String,
    /// Percorsi precedenti, se il file è stato rinominato/spostato (D-014).
    percorsi_precedenti: Vec<String>,
}

/// Una versione conservata, come mostrata nell'elenco della cronologia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Versione {
    /// Identificatore = nome del file dello snapshot (da passare a [`Cronologia::leggi`]).
    pub id: String,
    /// Istante dello snapshot, `AAAA-MM-GG HH:MM:SS` (ora locale; il nome è in UTC).
    pub quando: String,
    /// Stesso istante in **UTC**, RFC 3339 (`AAAA-MM-GGTHH:MM:SSZ`), così il
    /// frontend lo formatta con la propria locale. Campo additivo: `quando`
    /// resta invariato.
    pub quando_utc: String,
    /// Dimensione in byte.
    pub dimensione: u64,
}

/// Esito di [`Cronologia::registra`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EsitoRegistra {
    /// Snapshot creato (`id`); `eliminate` = versioni rimosse dalla rotazione.
    Creata { id: String, eliminate: usize },
    /// Contenuto identico all'ultima versione: nulla da fare.
    Identica,
    /// Ultima versione troppo recente rispetto all'intervallo richiesto.
    Saltata,
}

/// Cronologia locale, radicata in una cartella (la cartella dati dell'app in
/// produzione, una cartella temporanea nei test).
#[derive(Debug, Clone)]
pub struct Cronologia {
    radice: PathBuf,
}

/// Istante UTC nel nome di uno snapshot (`AAAAMMGG-HHMMSSZ…`), se ha la forma attesa.
fn tempo_da_nome(nome: &str) -> Option<NaiveDateTime> {
    let prefisso = nome.get(..LUNGHEZZA_TEMPO)?;
    let data = prefisso.strip_suffix('Z')?;
    NaiveDateTime::parse_from_str(data, FORMATO_TEMPO).ok()
}

/// Progressivo anti-collisione di un nome (`…-HHMMSS-2.typ` → 2; assente → 1).
fn progressivo_da_nome(nome: &str) -> u32 {
    let resto = &nome[LUNGHEZZA_TEMPO.min(nome.len())..];
    let stem = resto.split('.').next().unwrap_or("");
    stem.strip_prefix('-')
        .and_then(|n| n.parse().ok())
        .unwrap_or(1)
}

impl Cronologia {
    /// Cronologia nella cartella dati dell'OS (`data_dir()/typster/cronologia`),
    /// se determinabile.
    pub fn nella_cartella_dati() -> Option<Self> {
        dirs::data_dir().map(|base| Self {
            radice: base.join(crate::NOME_CARTELLA_DATI).join(NOME_CARTELLA),
        })
    }

    /// Cronologia radicata in una cartella esplicita (test, o cartella dati custom).
    pub fn in_radice(radice: PathBuf) -> Self {
        Self { radice }
    }

    /// Trova la cartella di un documento partendo dalla chiave base.
    ///
    /// Scorre `chiave`, `chiave-1`, `chiave-2`, …: la prima cartella il cui
    /// `meta.json` dichiara lo stesso percorso normalizzato (attuale **o**
    /// fra i `percorsi_precedenti`, per una rinomina interrotta a metà) è quella giusta
    /// (`Ok(Some)`); la prima **non esistente** è il posto libero
    /// (`Ok(None)` con `libera` valorizzata). Le cartelle occupate da un
    /// altro percorso (collisione della chiave) si saltano.
    fn cerca(&self, chiave: &str, norma: &str) -> io::Result<(Option<PathBuf>, PathBuf)> {
        for n in 0..MAX_ALTERNATIVE {
            let nome = if n == 0 {
                chiave.to_string()
            } else {
                format!("{chiave}-{n}")
            };
            let cartella = self.radice.join(nome);
            if !cartella.exists() {
                return Ok((None, cartella));
            }
            if let Some(meta) = leggi_meta(&cartella) {
                let uguale = |p: &str| normalizza_percorso(p, fs_senza_maiuscole()) == norma;
                if uguale(&meta.percorso) || meta.percorsi_precedenti.iter().any(|p| uguale(p)) {
                    return Ok((Some(cartella.clone()), cartella));
                }
            }
        }
        Err(io::Error::other(
            "troppe collisioni di chiave della cronologia",
        ))
    }

    /// Cartella esistente del documento (nessuna creazione).
    fn cartella_esistente(&self, percorso: &Path) -> Option<PathBuf> {
        let norma = percorso_normalizzato(percorso);
        let chiave = sha256_hex(norma.as_bytes());
        self.cerca(&chiave, &norma).ok().and_then(|(t, _)| t)
    }

    /// Cartella del documento, creata (con `meta.json`) se manca.
    fn cartella_o_crea(&self, percorso: &Path) -> io::Result<PathBuf> {
        let norma = percorso_normalizzato(percorso);
        let chiave = sha256_hex(norma.as_bytes());
        self.cartella_con_chiave(&chiave, percorso, &norma)
    }

    /// Come [`Self::cartella_o_crea`] con la chiave base imposta (per i test
    /// della gestione delle collisioni).
    fn cartella_con_chiave(
        &self,
        chiave: &str,
        percorso: &Path,
        norma: &str,
    ) -> io::Result<PathBuf> {
        let (trovata, libera) = self.cerca(chiave, norma)?;
        if let Some(cartella) = trovata {
            return Ok(cartella);
        }
        std::fs::create_dir_all(&libera)?;
        let meta = Meta {
            percorso: percorso_stabile(percorso).to_string_lossy().into_owned(),
            percorsi_precedenti: Vec::new(),
        };
        scrivi_meta(&libera, &meta)?;
        Ok(libera)
    }

    /// Registra `contenuto` come nuova versione del file `percorso`.
    ///
    /// - `Identica` se il contenuto coincide con l'ultima versione;
    /// - `Saltata` se `intervallo_minimo` è `Some` e l'ultima versione è più
    ///   recente di quell'intervallo rispetto a `ora` (uso: salvataggio
    ///   automatico; il manuale passa `None`);
    /// - altrimenti scrive lo snapshot (atomico) e applica la rotazione a
    ///   `max_versioni` (almeno 1).
    pub fn registra(
        &self,
        percorso: &Path,
        contenuto: &str,
        ora: NaiveDateTime,
        max_versioni: usize,
        intervallo_minimo: Option<std::time::Duration>,
    ) -> io::Result<EsitoRegistra> {
        let cartella = self.cartella_o_crea(percorso)?;
        self.registra_in(
            &cartella,
            percorso,
            contenuto,
            ora,
            max_versioni,
            intervallo_minimo,
        )
    }

    fn registra_in(
        &self,
        cartella: &Path,
        percorso: &Path,
        contenuto: &str,
        ora: NaiveDateTime,
        max_versioni: usize,
        intervallo_minimo: Option<std::time::Duration>,
    ) -> io::Result<EsitoRegistra> {
        let esistenti = elenca_cartella(cartella);
        if let Some(ultima) = esistenti.first() {
            let testo = std::fs::read_to_string(cartella.join(&ultima.id)).ok();
            if testo.as_deref() == Some(contenuto) {
                return Ok(EsitoRegistra::Identica);
            }
            let minimo = intervallo_minimo.and_then(|d| chrono::Duration::from_std(d).ok());
            if let (Some(min), Some(t)) = (minimo, tempo_da_nome(&ultima.id)) {
                if ora.signed_duration_since(t) < min {
                    return Ok(EsitoRegistra::Saltata);
                }
            }
        }

        let estensione = percorso
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| {
                !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric())
            })
            .unwrap_or("txt")
            .to_ascii_lowercase();
        let prefisso = format!("{}Z", ora.format(FORMATO_TEMPO));
        let mut id = format!("{prefisso}.{estensione}");
        let mut n = 1;
        loop {
            // `create_new` direttamente sul nome definitivo: nessuna finestra fra
            // "il nome è libero" e la scrittura, nemmeno con due processi.
            match scrivi_nuovo(&cartella.join(&id), contenuto.as_bytes()) {
                Ok(()) => break,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    n += 1;
                    if n > MAX_ALTERNATIVE {
                        return Err(io::Error::other("troppe versioni nello stesso secondo"));
                    }
                    id = format!("{prefisso}-{n}.{estensione}");
                }
                Err(e) => return Err(e),
            }
        }

        let eliminate = ruota(cartella, max_versioni.max(1));
        Ok(EsitoRegistra::Creata { id, eliminate })
    }

    /// Versioni del file, dalla più recente alla più vecchia (vuoto se nessuna).
    pub fn elenca(&self, percorso: &Path) -> Vec<Versione> {
        self.cartella_esistente(percorso)
            .map(|c| elenca_cartella(&c))
            .unwrap_or_default()
    }

    /// Testo della versione `id` del file.
    ///
    /// `id` deve essere il nome di uno snapshot (`AAAAMMGG-HHMMSS…`): separatori
    /// di percorso, `..` e nomi estranei sono rifiutati, così l'identificatore
    /// arrivato dal frontend non può uscire dalla cartella della cronologia.
    pub fn leggi(&self, percorso: &Path, id: &str) -> io::Result<String> {
        let valido = tempo_da_nome(id).is_some() && !id.contains(['/', '\\']) && !id.contains("..");
        if !valido {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("identificatore di versione non valido: '{id}'"),
            ));
        }
        let cartella = self.cartella_esistente(percorso).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "nessuna cronologia per questo file",
            )
        })?;
        std::fs::read_to_string(cartella.join(id))
    }

    /// Testo dell'ultima versione registrata, se c'è.
    pub fn ultima(&self, percorso: &Path) -> Option<String> {
        let cartella = self.cartella_esistente(percorso)?;
        let prima = elenca_cartella(&cartella).into_iter().next()?;
        std::fs::read_to_string(cartella.join(prima.id)).ok()
    }

    /// Associa la cronologia di `vecchio` al nuovo percorso `nuovo` (ri-chiavatura,
    /// D-014): la cartella `<chiave-vecchia>` diventa `<chiave-nuova>` e
    /// `meta.json` registra il vecchio percorso in `percorsi_precedenti`. Se
    /// `nuovo` ha già una cronologia, gli snapshot si **fondono** (i nomi in
    /// collisione ricevono un progressivo) e la cartella vecchia sparisce.
    /// Nessuna cronologia per `vecchio` → non fa nulla.
    pub fn rinomina(&self, vecchio: &Path, nuovo: &Path) -> io::Result<()> {
        let Some(da) = self.cartella_esistente(vecchio) else {
            return Ok(());
        };
        let norma_nuova = percorso_normalizzato(nuovo);
        let chiave_nuova = sha256_hex(norma_nuova.as_bytes());
        let (esistente, libera) = self.cerca(&chiave_nuova, &norma_nuova)?;

        let mut meta = leggi_meta(&da).unwrap_or_default();
        let precedente = meta.percorso.clone();
        meta.percorso = percorso_stabile(nuovo).to_string_lossy().into_owned();
        // Il percorso "precedente" non è mai quello nuovo (rinomina ripetuta dopo
        // un'interruzione: il meta dichiara già il nuovo nome).
        let e_nuovo = normalizza_percorso(&precedente, fs_senza_maiuscole())
            == normalizza_percorso(&meta.percorso, fs_senza_maiuscole());
        if !precedente.is_empty() && !e_nuovo && !meta.percorsi_precedenti.contains(&precedente) {
            meta.percorsi_precedenti.push(precedente);
        }
        // Rinomina A→B→A: il percorso attuale non è mai un "precedente" di se stesso.
        togli_percorso_attuale(&mut meta);
        // Il `meta.json` si scrive PRIMA di spostare la cartella: se il rename
        // fallisce, la cartella vecchia resta ritrovabile dal vecchio percorso
        // (`percorsi_precedenti`) e non c'è mai una cartella con un meta obsoleto.
        if esistente.is_none() {
            scrivi_meta(&da, &meta)?;
        }

        match esistente {
            Some(a) if a == da => scrivi_meta(&da, &meta),
            Some(a) => {
                // Fusione: gli snapshot vecchi entrano nella cartella nuova.
                let mut meta_a = leggi_meta(&a).unwrap_or_default();
                for p in meta.percorsi_precedenti.iter().chain([&meta.percorso]) {
                    if *p != meta_a.percorso && !meta_a.percorsi_precedenti.contains(p) {
                        meta_a.percorsi_precedenti.push(p.clone());
                    }
                }
                meta_a.percorso = meta.percorso;
                togli_percorso_attuale(&mut meta_a);
                for versione in elenca_cartella(&da) {
                    sposta_snapshot(&da, &a, &versione.id)?;
                }
                scrivi_meta(&a, &meta_a)?;
                std::fs::remove_dir_all(&da)
            }
            None => {
                if let Some(padre) = libera.parent() {
                    std::fs::create_dir_all(padre)?;
                }
                std::fs::rename(&da, &libera)
            }
        }
    }
}

/// Sposta lo snapshot `id` da `da` ad `a`; se il nome esiste già gli dà un
/// progressivo libero, senza mai sovrascrivere.
fn sposta_snapshot(da: &Path, a: &Path, id: &str) -> io::Result<()> {
    let (stem, estensione) = match id.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (id.to_string(), String::new()),
    };
    // Il progressivo esistente (`-2`) va tolto dallo stem prima di riassegnarlo.
    let base = stem.get(..LUNGHEZZA_TEMPO).unwrap_or(&stem).to_string();
    let mut destinazione = a.join(id);
    let mut n = 1;
    while destinazione.exists() {
        n += 1;
        if n > MAX_ALTERNATIVE {
            return Err(io::Error::other("troppe collisioni durante la fusione"));
        }
        destinazione = a.join(format!("{base}-{n}{estensione}"));
    }
    std::fs::rename(da.join(id), destinazione)
}

/// Snapshot presenti in `cartella`, dal più recente al più vecchio.
fn elenca_cartella(cartella: &Path) -> Vec<Versione> {
    let Ok(voci) = std::fs::read_dir(cartella) else {
        return Vec::new();
    };
    let mut versioni: Vec<(NaiveDateTime, u32, Versione)> = voci
        .filter_map(|v| v.ok())
        .filter_map(|voce| {
            let nome = voce.file_name().to_string_lossy().into_owned();
            let t = tempo_da_nome(&nome)?;
            let dimensione = voce.metadata().ok()?.len();
            let versione = Versione {
                quando: vista_locale(t),
                quando_utc: t.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                id: nome.clone(),
                dimensione,
            };
            Some((t, progressivo_da_nome(&nome), versione))
        })
        .collect();
    // Dal più recente: prima il tempo, poi il progressivo dello stesso secondo.
    versioni.sort_by_key(|v| std::cmp::Reverse((v.0, v.1)));
    versioni.into_iter().map(|(_, _, v)| v).collect()
}

/// Elimina gli snapshot più vecchi oltre `max`; ritorna quanti ne ha rimossi.
fn ruota(cartella: &Path, max: usize) -> usize {
    let mut eliminate = 0;
    for versione in elenca_cartella(cartella).into_iter().skip(max) {
        if std::fs::remove_file(cartella.join(&versione.id)).is_ok() {
            eliminate += 1;
        }
    }
    eliminate
}

/// Crea `percorso` con `create_new` e vi scrive `contenuto` (con `sync_all`);
/// se la scrittura fallisce a metà rimuove il file parziale.
fn scrivi_nuovo(percorso: &Path, contenuto: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(percorso)?;
    let esito = file.write_all(contenuto).and_then(|()| file.sync_all());
    if esito.is_err() {
        drop(file);
        let _ = std::fs::remove_file(percorso);
    }
    esito
}

fn leggi_meta(cartella: &Path) -> Option<Meta> {
    let testo = std::fs::read_to_string(cartella.join(NOME_META)).ok()?;
    serde_json::from_str(&testo).ok()
}

fn scrivi_meta(cartella: &Path, meta: &Meta) -> io::Result<()> {
    let testo = serde_json::to_string_pretty(meta).map_err(io::Error::other)?;
    scrivi_atomico(&cartella.join(NOME_META), testo.as_bytes())
}

/// Rimuove da `percorsi_precedenti` le voci uguali (normalizzate) al
/// percorso attuale del `meta`.
fn togli_percorso_attuale(meta: &mut Meta) {
    let attuale = normalizza_percorso(&meta.percorso, fs_senza_maiuscole());
    meta.percorsi_precedenti
        .retain(|p| normalizza_percorso(p, fs_senza_maiuscole()) != attuale);
}

#[cfg(test)]
#[path = "../tests/unit/cronologia.rs"]
mod test;
