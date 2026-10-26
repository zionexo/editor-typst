//! Ecosistema dei pacchetti Typst: indice di Typst Universe, installazione
//! `@preview`, creazione di progetti da template e pubblicazione `@local`.
//!
//! Modulo indipendente dal `World`: `installa`/`nuovo_da_template` creano un
//! proprio `SystemPackages` (con lo stesso downloader e le stesse cache
//! standard), così un download di rete non contende mai il lock del `World`
//! con `compila`/`completa`/`tooltip` (vedi DECISIONI.md F3-001).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use typst::syntax::ast;
use typst::syntax::package::{
    PackageInfo, PackageManifest, PackageSpec, PackageVersion, VersionBound, VersionlessPackageSpec,
};
use typst::syntax::{LinkedNode, SyntaxKind, VirtualPath};
use typst_kit::packages::FsPackages;

use crate::crea_system_packages;

/// URL dell'indice dei pacchetti `@preview` di Typst Universe.
const URL_INDICE: &str = "https://packages.typst.org/preview/index.json";
/// Freschezza massima della cache locale dell'indice prima di ritentare la rete.
const TTL_CACHE: Duration = Duration::from_secs(30 * 60);
/// Limite di lettura del corpo dell'indice scaricato (64 MiB): `ureq` 2.x
/// capperebbe altrimenti `into_string()` a ~10 MiB, con fallimento silenzioso
/// (degrado su cache/errore) non appena l'indice reale di Typst Universe
/// superasse quella soglia.
const LIMITE_DOWNLOAD_INDICE: u64 = 64 * 1024 * 1024;
/// Limite dimensione di un documento pubblicato come pacchetto `@local` (~10 MiB).
const LIMITE_PUBBLICAZIONE: usize = 10 * 1024 * 1024;
/// Livello di linguaggio Typst dichiarato nei pacchetti `@local` generati.
const VERSIONE_COMPILATORE: &str = "0.15";

/// Metadati di un pacchetto per il browser di Typst Universe.
#[derive(Debug, Clone, Serialize)]
pub struct PacchettoInfo {
    pub nome: String,
    pub versione: String,
    pub descrizione: String,
    pub autori: Vec<String>,
    pub licenza: Option<String>,
    pub categorie: Vec<String>,
    pub parole_chiave: Vec<String>,
    pub repository: Option<String>,
    pub e_template: bool,
}

/// Esito di [`elenca`]: pacchetti trovati + se provengono da cache stantia.
#[derive(Debug, Clone, Serialize)]
pub struct EsitoElenco {
    pub pacchetti: Vec<PacchettoInfo>,
    /// `true` se l'indice mostrato proviene da una cache scaduta perché la
    /// rete non era raggiungibile (offline-first, vedi DECISIONI.md F3-001).
    pub stantia: bool,
}

/// Progetto creato da un template: cartella radice ed entrypoint assoluto.
#[derive(Debug, Clone)]
pub struct ProgettoCreato {
    pub dir_progetto: PathBuf,
    pub entrypoint: PathBuf,
}

/// Nomi riservati dal filesystem Windows (case-insensitive): usarli come nome
/// di una cartella (anche con estensione, es. `con.txt`) fa fallire
/// `create_dir_all`/`create_dir` con un errore di sistema opaco, invece di un
/// rifiuto esplicito e comprensibile qui a monte.
const NOMI_RISERVATI_WINDOWS: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Valida un nome pacchetto (`^[a-z0-9-]+$`, come richiesto da Typst
/// Universe), rifiutando anche i nomi riservati dal filesystem Windows (vedi
/// [`NOMI_RISERVATI_WINDOWS`]): pur essendo già vincolato a `[a-z0-9-]+`, un
/// nome come `con` o `com1` diventa comunque la cartella `pubblica_in`/
/// `nuovo_da_template` provano a creare su disco, e su Windows fallirebbe
/// con un errore poco comprensibile.
fn valida_nome(nome: &str) -> Result<(), String> {
    let valido = !nome.is_empty()
        && nome
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valido {
        return Err(format!(
            "nome pacchetto non valido: '{nome}' (atteso [a-z0-9-]+)"
        ));
    }
    if NOMI_RISERVATI_WINDOWS
        .iter()
        .any(|riservato| nome.eq_ignore_ascii_case(riservato))
    {
        return Err(format!(
            "nome pacchetto non valido: '{nome}' è un nome riservato dal filesystem Windows"
        ));
    }
    Ok(())
}

/// Valida (`^\d+\.\d+\.\d+$`) e converte una versione `major.minor.patch`.
fn valida_versione(versione: &str) -> Result<PackageVersion, String> {
    let parti: Vec<&str> = versione.split('.').collect();
    let valida = parti.len() == 3
        && parti
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    if !valida {
        return Err(format!(
            "versione non valida: '{versione}' (atteso major.minor.patch)"
        ));
    }
    versione
        .parse::<PackageVersion>()
        .map_err(|e| format!("versione non valida: '{versione}': {e}"))
}

// ---------------------------------------------------------------------
// 3.1 — Indice di Typst Universe
// ---------------------------------------------------------------------

/// Restituisce l'elenco dei pacchetti `@preview` di Typst Universe.
///
/// Offline-first: usa la cache locale se fresca (TTL 30 minuti), a meno che
/// `forza` non richieda comunque un tentativo di rete. Se la rete non è
/// raggiungibile, ripiega sulla cache anche se stantia. Fallisce solo se non
/// c'è alcun dato disponibile (né rete né cache).
pub fn elenca(forza: bool) -> Result<EsitoElenco, String> {
    let percorso_cache = percorso_cache_indice()?;

    if !forza && cache_fresca(&percorso_cache) {
        let dalla_cache = std::fs::read_to_string(&percorso_cache)
            .ok()
            .and_then(|testo| analizza_indice(&testo).ok());
        if let Some(pacchetti) = dalla_cache {
            return Ok(EsitoElenco {
                pacchetti,
                stantia: false,
            });
        }
        // Cache "fresca" ma illeggibile/corrotta: si prosegue col tentativo
        // di rete qui sotto invece di fallire subito.
    }

    gestisci_esito_rete(scarica_indice(), &percorso_cache)
}

/// Interpreta l'esito del download dell'indice: scrive la cache su successo,
/// oppure ripiega su di essa (se presente e valida) su fallimento — sia un
/// fallimento di rete, sia una risposta 200 con corpo non-JSON (tipico di un
/// captive portal/proxy che intercetta la richiesta con una pagina HTML: un
/// simile corpo non è un indice valido, ma non è nemmeno un errore di rete in
/// senso stretto, quindi non deve propagare subito l'errore invece di
/// ripiegare sulla cache, cfr. DECISIONI.md F3-001 offline-first). Pura
/// rispetto all'esito di rete (passato già come `Result`): testabile senza
/// toccare la rete reale, iniettando un successo/fallimento sintetico.
fn gestisci_esito_rete(
    esito_rete: Result<String, String>,
    percorso_cache: &Path,
) -> Result<EsitoElenco, String> {
    match esito_rete {
        Ok(testo) => match analizza_indice(&testo) {
            Ok(pacchetti) => {
                // Scrittura della cache: best-effort, non deve impedire di
                // restituire il risultato appena scaricato. Atomica (temp +
                // rename, crea anche la cartella): un crash a metà non lascia
                // un JSON troncato che farebbe fallire il ripiego offline.
                let _ = crate::config::scrivi_atomico(percorso_cache, testo.as_bytes());
                Ok(EsitoElenco {
                    pacchetti,
                    stantia: false,
                })
            }
            // Risposta 200 ma corpo non analizzabile come indice: ripiega
            // sulla cache invece di propagare subito l'errore di parsing
            // (NON scrive questo corpo in cache: sovrascriverebbe una cache
            // buona esistente con una risposta corrotta).
            Err(errore_parsing) => ripiega_su_cache(percorso_cache, &errore_parsing),
        },
        Err(errore_rete) => ripiega_su_cache(percorso_cache, &errore_rete),
    }
}

/// Ripiego offline comune ai due fallimenti di [`gestisci_esito_rete`] (rete
/// irraggiungibile, o risposta 200 con corpo non valido): la cache, se
/// presente, fa da fonte anche se NON è JSON valido — in tal caso va trattata
/// come assente (non come un errore fatale "indice malformato" da
/// propagare).
fn ripiega_su_cache(percorso_cache: &Path, messaggio_errore: &str) -> Result<EsitoElenco, String> {
    let dalla_cache = std::fs::read_to_string(percorso_cache)
        .ok()
        .and_then(|testo| analizza_indice(&testo).ok());
    match dalla_cache {
        Some(pacchetti) => Ok(EsitoElenco {
            pacchetti,
            // "Stantia" solo se la cache usata è realmente oltre il TTL: con
            // `forza=true` e rete irraggiungibile, una cache ancora fresca
            // non va etichettata come stantia.
            stantia: !cache_fresca(percorso_cache),
        }),
        None => Err(format!(
            "Impossibile raggiungere Typst Universe e nessuna cache locale disponibile: {messaggio_errore}"
        )),
    }
}

/// Percorso su disco della cache dell'indice.
fn percorso_cache_indice() -> Result<PathBuf, String> {
    dirs::cache_dir()
        .map(|base| {
            base.join(crate::NOME_CARTELLA_DATI)
                .join("universe-index.json")
        })
        .ok_or_else(|| "impossibile determinare la cartella di cache dell'OS".to_string())
}

/// Vero se la cache esiste ed è stata modificata entro il TTL.
fn cache_fresca(percorso: &Path) -> bool {
    let Ok(metadati) = std::fs::metadata(percorso) else {
        return false;
    };
    let Ok(modificato) = metadati.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modificato)
        .is_ok_and(|eta| eta < TTL_CACHE)
}

/// Costruisce l'agent HTTP con TLS di sistema e proxy da ambiente.
///
/// Il default di `ureq` (rustls + radici Mozilla incorporate) non conosce le CA
/// aziendali installate nello store di Windows e fallisce con `UnknownIssuer`
/// dietro ispezione TLS. Qui si usa `native-tls` (store di sistema) e il proxy
/// da variabili d'ambiente, come il downloader di typst-kit per i pacchetti.
///
/// Funzione senza I/O di rete: testabile in isolamento.
// Derivato da typst-kit src/downloader.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
fn costruisci_agent(url: &str) -> Result<ureq::Agent, String> {
    let mut builder = ureq::AgentBuilder::new();

    if let Some(proxy) = env_proxy::for_url_str(url)
        .to_url()
        .and_then(|u| ureq::Proxy::new(u).ok())
    {
        builder = builder.proxy(proxy);
    }

    let connettore = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| format!("inizializzazione TLS di sistema fallita: {e}"))?;
    Ok(builder
        .tls_connector(std::sync::Arc::new(connettore))
        .build())
}

/// Scarica il testo grezzo dell'indice via HTTP (ureq, sincrono).
///
/// Legge il corpo dal `Read` grezzo (`into_reader`) invece di usare
/// `Response::into_string`, che in `ureq` 2.x cappa la lettura a ~10 MiB: un
/// indice reale più grande farebbe fallire silenziosamente il parse e
/// degradare sempre su cache/errore. Il limite qui (`LIMITE_DOWNLOAD_INDICE`)
/// è deliberatamente ampio, solo a difesa da una risposta abnorme o senza
/// `Content-Length`.
fn scarica_indice() -> Result<String, String> {
    use std::io::Read;

    let risposta = costruisci_agent(URL_INDICE)?
        .get(URL_INDICE)
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| e.to_string())?;

    let mut testo = String::new();
    risposta
        .into_reader()
        .take(LIMITE_DOWNLOAD_INDICE)
        .read_to_string(&mut testo)
        .map_err(|e| e.to_string())?;
    Ok(testo)
}

/// Voce grezza dell'indice: solo i campi che ci servono, con parsing
/// tollerante voce-per-voce (una voce non deserializzabile viene scartata,
/// non blocca l'analisi delle altre).
#[derive(Debug, Deserialize)]
struct VoceIndiceGrezza {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    license: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    repository: Option<String>,
    /// Presenza (non-null) ⇒ il pacchetto è un template. Il contenuto esatto
    /// non ci serve qui: solo il flag `e_template` verso il frontend.
    #[serde(default)]
    template: Option<serde_json::Value>,
}

/// Analizza il testo JSON dell'indice: parsing tollerante voce-per-voce,
/// validazione nome/versione, raggruppamento per nome tenendo la versione
/// massima. Pura (nessun I/O): testabile con un fixture JSON in memoria.
fn analizza_indice(testo: &str) -> Result<Vec<PacchettoInfo>, String> {
    let grezzo: Vec<serde_json::Value> =
        serde_json::from_str(testo).map_err(|e| format!("indice malformato: {e}"))?;

    let mut per_nome: HashMap<String, (PackageVersion, VoceIndiceGrezza)> = HashMap::new();
    for valore in grezzo {
        let Ok(voce) = serde_json::from_value::<VoceIndiceGrezza>(valore) else {
            continue;
        };
        if valida_nome(&voce.name).is_err() {
            continue;
        }
        let Ok(versione) = valida_versione(&voce.version) else {
            continue;
        };

        match per_nome.get(&voce.name) {
            Some((esistente, _)) if *esistente >= versione => {}
            _ => {
                per_nome.insert(voce.name.clone(), (versione, voce));
            }
        }
    }

    let mut pacchetti: Vec<PacchettoInfo> = per_nome
        .into_values()
        .map(|(versione, voce)| PacchettoInfo {
            nome: voce.name,
            versione: versione.to_string(),
            descrizione: voce.description.unwrap_or_default(),
            autori: voce.authors,
            licenza: voce.license,
            categorie: voce.categories,
            parole_chiave: voce.keywords,
            repository: voce.repository,
            e_template: voce.template.is_some(),
        })
        .collect();

    pacchetti.sort_by(|a, b| a.nome.cmp(&b.nome));
    Ok(pacchetti)
}

// ---------------------------------------------------------------------
// Installazione (pre-download nella cache @preview)
// ---------------------------------------------------------------------

/// Pre-scarica un pacchetto `@preview` nella cache standard, senza toccare
/// il `World`.
///
/// Dopo l'ottenimento invalida la cache `comemo` (come `pubblica_local`,
/// F4-004): una precedente risoluzione fallita (`NotFound`) di quel pacchetto
/// — es. una compilazione tentata prima dell'installazione — potrebbe essere
/// rimasta memoizzata, nascondendo il pacchetto appena scaricato fino al
/// prossimo sfratto naturale della cache.
pub fn installa(nome: &str, versione: &str) -> Result<(), String> {
    valida_nome(nome)?;
    let versione = valida_versione(versione)?;
    let spec = PackageSpec {
        namespace: "preview".into(),
        name: nome.into(),
        version: versione,
    };
    crea_system_packages()
        .obtain(&spec)
        .map_err(|e| e.to_string())?;
    crate::invalida_cache();
    Ok(())
}

// ---------------------------------------------------------------------
// 3.3 — "Nuovo da template"
// ---------------------------------------------------------------------

/// Crea un nuovo progetto in `destinazione/{nome}` copiando i file del
/// template dichiarato dal pacchetto `@preview/{nome}:{versione}`.
pub fn nuovo_da_template(
    nome: &str,
    versione: &str,
    destinazione: &Path,
) -> Result<ProgettoCreato, String> {
    valida_nome(nome)?;
    let versione_valida = valida_versione(versione)?;
    let spec = PackageSpec {
        namespace: "preview".into(),
        name: nome.into(),
        version: versione_valida,
    };

    let radice_pacchetto = crea_system_packages()
        .obtain(&spec)
        .map_err(|e| e.to_string())?;

    let testo_manifesto = std::fs::read_to_string(radice_pacchetto.path().join("typst.toml"))
        .map_err(|e| format!("'typst.toml' del pacchetto '{nome}' non leggibile: {e}"))?;
    let manifesto: PackageManifest = toml::from_str(&testo_manifesto)
        .map_err(|e| format!("'typst.toml' del pacchetto '{nome}' non valido: {e}"))?;
    let template = manifesto
        .template
        .ok_or_else(|| format!("il pacchetto '{nome}' non è un template"))?;

    // Percorsi del manifesto validati come relativi "puliti" (niente `..`).
    let vpath_dir = VirtualPath::new(template.path.as_str())
        .map_err(|e| format!("percorso template non valido '{}': {e}", template.path))?;
    let vpath_entrypoint = VirtualPath::new(template.entrypoint.as_str()).map_err(|e| {
        format!(
            "entrypoint template non valido '{}': {e}",
            template.entrypoint
        )
    })?;

    let dir_sorgente = vpath_dir
        .realize(radice_pacchetto.path())
        .map_err(|e| format!("percorso template non valido '{}': {e}", template.path))?;
    if !dir_sorgente.is_dir() {
        return Err(format!(
            "il pacchetto '{nome}' non contiene la cartella template '{}'",
            template.path
        ));
    }

    let dir_progetto = destinazione.join(nome);
    let entrypoint = crea_progetto_da_cartella(
        &dir_sorgente,
        &dir_progetto,
        &vpath_entrypoint,
        template.entrypoint.as_str(),
    )?;

    Ok(ProgettoCreato {
        dir_progetto,
        entrypoint,
    })
}

/// Crea `dir_progetto` (che non deve esistere) copiandoci `dir_sorgente` e
/// verifica che l'entrypoint dichiarato esista; restituisce il suo percorso.
///
/// Se la copia o la verifica falliscono la cartella appena creata viene
/// **rimossa** (solo se l'ha creata questa chiamata): altrimenti un secondo
/// tentativo fallirebbe con «impossibile creare» su una cartella a metà.
fn crea_progetto_da_cartella(
    dir_sorgente: &Path,
    dir_progetto: &Path,
    vpath_entrypoint: &VirtualPath,
    entrypoint_dichiarato: &str,
) -> Result<PathBuf, String> {
    std::fs::create_dir(dir_progetto)
        .map_err(|e| format!("impossibile creare '{}': {e}", dir_progetto.display()))?;
    let esito = (|| -> Result<PathBuf, String> {
        copia_ricorsiva(dir_sorgente, dir_progetto)
            .map_err(|e| format!("copia del template fallita: {e}"))?;
        let entrypoint = vpath_entrypoint.realize(dir_progetto).map_err(|e| {
            format!("entrypoint template non valido '{entrypoint_dichiarato}': {e}")
        })?;
        if !entrypoint.is_file() {
            return Err(format!(
                "il template dichiara l'entrypoint '{entrypoint_dichiarato}', ma il file non esiste dopo la copia"
            ));
        }
        Ok(entrypoint)
    })();
    if esito.is_err() {
        if let Err(e) = std::fs::remove_dir_all(dir_progetto) {
            eprintln!(
                "Cartella parziale '{}' non rimossa: {e}",
                dir_progetto.display()
            );
        }
    }
    esito
}

/// Copia ricorsivamente `sorgente` in `destinazione`, saltando i symlink
/// (evita traversal e cicli tramite link che puntano fuori dal pacchetto).
fn copia_ricorsiva(sorgente: &Path, destinazione: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destinazione)?;
    for voce in std::fs::read_dir(sorgente)? {
        let voce = voce?;
        let tipo = voce.file_type()?;
        let percorso_sorgente = voce.path();
        let percorso_dest = destinazione.join(voce.file_name());
        if tipo.is_symlink() {
            continue;
        } else if tipo.is_dir() {
            copia_ricorsiva(&percorso_sorgente, &percorso_dest)?;
        } else if tipo.is_file() {
            std::fs::copy(&percorso_sorgente, &percorso_dest)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Fase 4 — pubblicazione @local
// ---------------------------------------------------------------------

/// Soglia di età oltre la quale una cartella `.tmp-`/`.old-` è considerata
/// un residuo orfano (di un crash passato) invece che l'appoggio vivo di una
/// pubblicazione concorrente. Ampia di proposito: una pubblicazione, anche
/// lenta, si conclude in secondi, non ore.
const SOGLIA_RESIDUI_ORFANI: Duration = Duration::from_secs(60 * 60);

/// Ripulisce eventuali cartelle di appoggio orfane (prefisso `.tmp-` o
/// `.old-`) nella cartella di un pacchetto: residui possibili di un crash
/// durante una pubblicazione precedente, a metà fra la creazione della
/// temporanea e la sua rimozione.
///
/// Rimuove un residuo **solo se il suo tempo di modifica è più vecchio di
/// [`SOGLIA_RESIDUI_ORFANI`]**: una pubblicazione concorrente dello stesso
/// pacchetto, ancora in volo, crea cartelle con lo stesso schema di nome
/// (`.tmp-{versione}-{univoco}` / `.old-{versione}-{univoco}`), ma fresche
/// (secondi, non ore). Cancellarle a prescindere dall'età — come faceva una
/// versione precedente di questa funzione — è un hazard: il `rename` finale
/// di quell'altra pubblicazione troverebbe la cartella già sparita e
/// fallirebbe, lasciando la versione assente. Un vero residuo di crash è
/// invece vecchio quanto il crash stesso, quindi ben oltre la soglia.
///
/// Best-effort in ogni sua parte: qualunque errore nel leggere metadati o
/// mtime, o un mtime nel futuro (clock skew, per cui `duration_since`
/// fallisce), fa saltare quel residuo senza cancellarlo e senza propagare
/// l'errore — la pulizia è una comodità, non deve mai far fallire la
/// pubblicazione corrente.
fn pulisci_residui_orfani(cartella_nome: &Path) {
    let Ok(voci) = std::fs::read_dir(cartella_nome) else {
        return;
    };
    let adesso = SystemTime::now();
    for voce in voci.flatten() {
        let nome_voce = voce.file_name();
        let nome_voce = nome_voce.to_string_lossy();
        if !nome_voce.starts_with(".tmp-") && !nome_voce.starts_with(".old-") {
            continue;
        }
        let Ok(metadati) = voce.metadata() else {
            continue;
        };
        let Ok(modificato) = metadati.modified() else {
            continue;
        };
        if residuo_e_da_rimuovere(modificato, adesso) {
            let _ = std::fs::remove_dir_all(voce.path());
        }
    }
}

/// Logica pura (isolata per testabilità) della decisione "questo residuo va
/// rimosso?": sì solo se la sua età rispetto ad `adesso` è pari o superiore a
/// [`SOGLIA_RESIDUI_ORFANI`]. Qualunque anomalia nel calcolo dell'età —
/// incluso un `modificato` nel futuro rispetto ad `adesso` per clock skew,
/// per cui `duration_since` fallisce — fa propendere per **non** rimuovere:
/// nel dubbio si preserva il residuo (al più si accumula, ma non si rischia
/// di cancellare l'appoggio di una pubblicazione concorrente in corso).
fn residuo_e_da_rimuovere(modificato: SystemTime, adesso: SystemTime) -> bool {
    match adesso.duration_since(modificato) {
        Ok(eta) => eta >= SOGLIA_RESIDUI_ORFANI,
        Err(_) => false,
    }
}

/// Pubblica `contenuto` come pacchetto `{base}/{nome}/{versione}/` (struttura
/// standard di una radice `@local`), con scrittura **atomica**.
///
/// Pura rispetto alla directory di base: testabile senza toccare la data dir
/// reale dell'utente (vedi [`pubblica_local`], che calcola `base`).
fn pubblica_in(
    base: &Path,
    nome: &str,
    versione: &str,
    contenuto: &str,
) -> Result<PathBuf, String> {
    valida_nome(nome)?;
    let versione_valida = valida_versione(versione)?;
    if contenuto.len() > LIMITE_PUBBLICAZIONE {
        return Err(format!(
            "documento troppo grande per la pubblicazione locale (limite {} MiB)",
            LIMITE_PUBBLICAZIONE / (1024 * 1024)
        ));
    }

    let mut manifesto = PackageManifest::new(PackageInfo::new(nome, versione_valida, "lib.typ"));
    manifesto.package.compiler = Some(
        VERSIONE_COMPILATORE
            .parse::<VersionBound>()
            .map_err(|e| format!("versione compilatore interna non valida: {e}"))?,
    );
    let toml_manifesto = toml::to_string_pretty(&manifesto)
        .map_err(|e| format!("generazione di 'typst.toml' fallita: {e}"))?;

    // Percorso su disco costruito dalla versione **normalizzata**
    // (`versione_valida.to_string()`), non dalla stringa grezza in ingresso:
    // Typst risolve `@local/{nome}:{versione}` cercando la cartella con la
    // forma normalizzata (es. `1.0.0`), quindi una versione grezza non
    // canonica (es. `01.0.0`) finirebbe in una cartella (`.../01.0.0/`) che
    // il compilatore non troverebbe mai, con un `NotFound` silenzioso.
    let versione_normalizzata = versione_valida.to_string();
    let cartella_nome = base.join(nome);
    let dir_pacchetto = cartella_nome.join(&versione_normalizzata);
    std::fs::create_dir_all(&cartella_nome)
        .map_err(|e| format!("impossibile creare '{}': {e}", cartella_nome.display()))?;

    // Pulizia di eventuali cartelle di appoggio orfane (`.tmp-*`/`.old-*`)
    // lasciate da un crash durante una pubblicazione precedente. Rimuove solo
    // i residui più vecchi di `SOGLIA_RESIDUI_ORFANI`: una pubblicazione
    // concorrente dello stesso pacchetto, ancora in volo, ha le proprie
    // cartelle di appoggio fresche (vedi doc di `pulisci_residui_orfani`) e
    // non va toccata. Best-effort e non strettamente necessaria per la
    // correttezza di QUESTA esecuzione (la cartella di appoggio qui sotto è
    // resa univoca da `univoco`, quindi non collide mai con un residuo), ma
    // evita che simili residui si accumulino indefinitamente nella cartella
    // del pacchetto.
    pulisci_residui_orfani(&cartella_nome);

    // Scrittura atomica: prepara i file in una cartella temporanea accanto
    // alla destinazione finale, poi la scambia con un `rename`. Se la
    // versione era già stata pubblicata, viene sostituita per intero (mai
    // modificata sul posto): una ripubblicazione dopo una modifica non lascia
    // mai un pacchetto a metà scritto.
    //
    // Il nome della cartella temporanea include, oltre al PID, i nanosecondi
    // correnti: il solo PID potrebbe ripresentarsi (riuso da parte dell'OS)
    // e collidere con un residuo lasciato da un crash precedente.
    let univoco = crate::suffisso_univoco();
    let temporanea = cartella_nome.join(format!(".tmp-{versione_normalizzata}-{univoco}"));
    let vecchia_da_parte = cartella_nome.join(format!(".old-{versione_normalizzata}-{univoco}"));

    let esito = (|| -> std::io::Result<()> {
        // Pulizia difensiva di un eventuale residuo con lo stesso nome
        // (in pratica escluso dal componente temporale, ma innocuo da fare).
        let _ = std::fs::remove_dir_all(&temporanea);
        std::fs::create_dir_all(&temporanea)?;
        std::fs::write(temporanea.join("lib.typ"), contenuto.as_bytes())?;
        std::fs::write(temporanea.join("typst.toml"), toml_manifesto.as_bytes())?;

        if dir_pacchetto.exists() {
            // Scambio sicuro: la versione precedente viene spostata da parte
            // (mai cancellata prima che la nuova sia al sicuro sul posto
            // giusto), poi la nuova la sostituisce, e solo allora la vecchia
            // viene eliminata. Così un `rename` fallito non lascia MAI la
            // cartella del pacchetto vuota: nel caso peggiore resta la
            // versione precedente, mai "nessuna versione".
            //
            // **Limite noto**: fra i due `rename` c'è comunque una finestra
            // (piccola ma non nulla) in cui `dir_pacchetto` non esiste
            // affatto — su Windows non esiste un'operazione che sostituisca
            // atomicamente una directory con un'altra (a differenza del
            // rename-over-file POSIX). Il rischio residuo è quindi una
            // lettura concorrente che vede `NotFound` per un istante, non
            // corruzione o perdita di dati: se il secondo `rename` fallisce,
            // la versione precedente viene comunque ripristinata subito
            // sotto.
            std::fs::rename(&dir_pacchetto, &vecchia_da_parte)?;
            match std::fs::rename(&temporanea, &dir_pacchetto) {
                Ok(()) => {
                    let _ = std::fs::remove_dir_all(&vecchia_da_parte);
                    Ok(())
                }
                Err(errore) => {
                    // Ripristina la versione precedente: meglio la vecchia
                    // pubblicazione intatta che nessuna.
                    let _ = std::fs::rename(&vecchia_da_parte, &dir_pacchetto);
                    Err(errore)
                }
            }
        } else {
            std::fs::rename(&temporanea, &dir_pacchetto)
        }
    })();

    if esito.is_err() {
        let _ = std::fs::remove_dir_all(&temporanea);
    }
    esito.map_err(|e| format!("pubblicazione fallita: {e}"))?;

    Ok(dir_pacchetto)
}

/// Pubblica il documento corrente come pacchetto `@local` riutilizzabile,
/// nella data dir standard dell'OS (`dirs::data_dir()/typst/packages/local`):
/// la stessa radice che `typst-kit` risolve già per `@local`, quindi nessuna
/// modifica al `World` è necessaria per renderlo importabile.
pub fn pubblica_local(nome: &str, versione: &str, contenuto: &str) -> Result<PathBuf, String> {
    let base = dirs::data_dir()
        .map(|dir| dir.join("typst").join("packages").join("local"))
        .ok_or_else(|| "impossibile determinare la data dir dell'OS".to_string())?;
    pubblica_in(&base, nome, versione, contenuto)
}

// ---------------------------------------------------------------------
// Aggiornamenti dei pacchetti (F5-012)
// ---------------------------------------------------------------------

/// Un pacchetto `@preview` con una versione più recente nell'indice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AggiornamentoPacchetto {
    pub nome: String,
    /// Versione oggi installata (o importata dal documento).
    pub attuale: String,
    /// Ultima versione nell'indice di Typst Universe.
    pub ultima: String,
}

/// Esito di [`controlla_aggiornamenti`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EsitoAggiornamenti {
    /// Pacchetti installati nella cache `@preview` con una versione più recente.
    pub installati: Vec<AggiornamentoPacchetto>,
    /// Import `@preview` del documento corrente non aggiornati.
    pub documento: Vec<AggiornamentoPacchetto>,
}

/// Ultima versione per nome nell'indice. Le versioni non valide si scartano;
/// il confronto è quello di `PackageVersion` (`Ord` su major, minor, patch:
/// nessun crate `semver`, D-016).
fn ultime_versioni(indice: &[PacchettoInfo]) -> HashMap<&str, PackageVersion> {
    let mut mappa: HashMap<&str, PackageVersion> = HashMap::new();
    for voce in indice {
        let Ok(versione) = valida_versione(&voce.versione) else {
            continue;
        };
        mappa
            .entry(voce.nome.as_str())
            .and_modify(|v| *v = (*v).max(versione))
            .or_insert(versione);
    }
    mappa
}

/// Ultima versione installata di ogni pacchetto `@preview` nella cache
/// `cache` (`<cache>/preview/<nome>/<versione>/`), ordinata per nome.
/// Usa `FsPackages::latest_version` di `typst-kit` (nessuna copia): le
/// cartelle che non sono nomi validi o senza versioni leggibili si ignorano.
fn installate_in(cache: &FsPackages) -> Vec<(String, PackageVersion)> {
    let mut trovate: Vec<(String, PackageVersion)> =
        std::fs::read_dir(cache.path().join("preview"))
            .into_iter()
            .flatten()
            .filter_map(|voce| voce.ok())
            .filter(|voce| voce.path().is_dir())
            .filter_map(|voce| {
                let nome = voce.file_name().to_string_lossy().into_owned();
                valida_nome(&nome).ok()?;
                let spec = VersionlessPackageSpec {
                    namespace: "preview".into(),
                    name: nome.as_str().into(),
                };
                Some((nome, cache.latest_version(&spec)?))
            })
            .collect();
    trovate.sort();
    trovate
}

/// Segnala i pacchetti la cui versione `(nome, versione)` è inferiore
/// all'ultima dell'indice. Pura: niente I/O. Ordinata per nome e versione,
/// senza duplicati; i pacchetti assenti dall'indice non si segnalano.
fn aggiornamenti_da(
    usati: &[(String, PackageVersion)],
    indice: &[PacchettoInfo],
) -> Vec<AggiornamentoPacchetto> {
    let ultime = ultime_versioni(indice);
    let mut esito: Vec<(String, PackageVersion, PackageVersion)> = usati
        .iter()
        .filter_map(|(nome, attuale)| {
            let ultima = *ultime.get(nome.as_str())?;
            (ultima > *attuale).then(|| (nome.clone(), *attuale, ultima))
        })
        .collect();
    esito.sort();
    esito.dedup();
    esito
        .into_iter()
        .map(|(nome, attuale, ultima)| AggiornamentoPacchetto {
            nome,
            attuale: attuale.to_string(),
            ultima: ultima.to_string(),
        })
        .collect()
}

// Derivato da tinymist crates/tinymist-query/src/package.rs (f3b00eb553d6b7a35b5d59cce529c8d0d774cfce), licenza Apache-2.0 — Copyright Myriad-Dreamin e contributori Tinymist
// `parse_package_import`: da un nodo stringa di un `#import` al `PackageSpec`.

/// Se `nodo` è la stringa-sorgente di un `#import "@ns/nome:versione"`,
/// restituisce lo spec del pacchetto.
fn import_di_pacchetto(nodo: &LinkedNode) -> Option<PackageSpec> {
    if !matches!(nodo.kind(), SyntaxKind::Str) {
        return None;
    }
    let import = nodo.parent()?.cast::<ast::ModuleImport>()?;
    let ast::Expr::Str(sorgente) = import.source() else {
        return None;
    };
    let testo = sorgente.get();
    if testo.starts_with('@') {
        testo.parse().ok()
    } else {
        None
    }
}

/// Pacchetti `@preview` importati dal sorgente Typst (visita dell'intero
/// albero sintattico, quindi anche gli import dentro blocchi di codice).
pub fn import_preview_da_sorgente(sorgente: &str) -> Vec<(String, PackageVersion)> {
    fn visita(nodo: &LinkedNode, fuori: &mut Vec<(String, PackageVersion)>) {
        if let Some(spec) = import_di_pacchetto(nodo) {
            if spec.namespace == "preview" {
                fuori.push((spec.name.to_string(), spec.version));
            }
        }
        for figlio in nodo.children() {
            visita(&figlio, fuori);
        }
    }
    let radice = typst::syntax::parse(sorgente);
    let mut trovati = Vec::new();
    visita(&LinkedNode::new(&radice), &mut trovati);
    trovati
}

/// Cuore testabile di [`controlla_aggiornamenti`]: cache e indice espliciti.
fn controlla_con(
    cache: &FsPackages,
    indice: &[PacchettoInfo],
    sorgente: Option<&str>,
) -> EsitoAggiornamenti {
    let documento = sorgente
        .map(|s| aggiornamenti_da(&import_preview_da_sorgente(s), indice))
        .unwrap_or_default();
    EsitoAggiornamenti {
        installati: aggiornamenti_da(&installate_in(cache), indice),
        documento,
    }
}

/// Indice di Typst Universe **già in cache**, di qualunque età e senza rete
/// (offline-first: il controllo aggiornamenti non deve mai bloccarsi sul
/// download). `Err` se non è mai stato scaricato o è illeggibile.
fn indice_da_cache() -> Result<Vec<PacchettoInfo>, String> {
    let percorso = percorso_cache_indice()?;
    let testo = std::fs::read_to_string(&percorso).map_err(|_| {
        "indice di Typst Universe non ancora scaricato: apri il pannello Pacchetti".to_string()
    })?;
    analizza_indice(&testo)
}

/// Confronta i pacchetti `@preview` installati (cache standard di Typst) e,
/// se dato, gli `#import` di `sorgente` con l'ultima versione nell'indice in
/// cache. Nessuna richiesta di rete.
pub fn controlla_aggiornamenti(sorgente: Option<&str>) -> Result<EsitoAggiornamenti, String> {
    let cache = FsPackages::system_cache()
        .ok_or_else(|| "impossibile determinare la cache dei pacchetti dell'OS".to_string())?;
    let indice = indice_da_cache()?;
    Ok(controlla_con(&cache, &indice, sorgente))
}

#[cfg(test)]
mod test {
    /// L'agent con TLS di sistema si costruisce senza rete e senza panic.
    #[test]
    fn agent_tls_di_sistema_si_costruisce() {
        assert!(super::costruisci_agent("https://packages.typst.org/preview/index.json").is_ok());
    }

    use super::*;

    #[test]
    fn creazione_fallita_ripulisce_la_cartella_e_il_secondo_tentativo_riesce() {
        let sorgente = tempfile::tempdir().expect("sorgente");
        std::fs::write(sorgente.path().join("altro.typ"), "x").expect("fixture");
        let dest = tempfile::tempdir().expect("destinazione");
        let progetto = dest.path().join("modello");
        let vpath = VirtualPath::new("main.typ").expect("vpath");
        // Entrypoint dichiarato ma assente nel template: fallisce dopo la copia.
        let err = crea_progetto_da_cartella(sorgente.path(), &progetto, &vpath, "main.typ")
            .expect_err("entrypoint mancante");
        assert!(err.contains("non esiste dopo la copia"), "{err}");
        assert!(!progetto.exists(), "cartella parziale da rimuovere");
        // Secondo tentativo con il file presente: non fallisce con «impossibile creare».
        std::fs::write(sorgente.path().join("main.typ"), "= Ciao").expect("main");
        let ep = crea_progetto_da_cartella(sorgente.path(), &progetto, &vpath, "main.typ")
            .expect("secondo tentativo");
        assert!(ep.is_file());
        // Una cartella preesistente NON viene toccata se la creazione fallisce subito.
        let err = crea_progetto_da_cartella(sorgente.path(), &progetto, &vpath, "main.typ")
            .expect_err("esiste già");
        assert!(err.contains("impossibile creare"), "{err}");
        assert!(progetto.join("main.typ").is_file(), "non va cancellata");
    }

    #[test]
    fn nome_valido_accetta_minuscole_numeri_trattini() {
        assert!(valida_nome("cetz").is_ok());
        assert!(valida_nome("charged-ieee").is_ok());
        assert!(valida_nome("pkg2").is_ok());
    }

    #[test]
    fn nome_non_valido_rifiuta_maiuscole_e_simboli() {
        assert!(valida_nome("").is_err());
        assert!(valida_nome("Cetz").is_err());
        assert!(valida_nome("cetz_pkg").is_err());
        assert!(valida_nome("../evil").is_err());
        assert!(valida_nome("cetz/../x").is_err());
    }

    #[test]
    fn nome_rifiuta_device_riservati_windows() {
        assert!(valida_nome("con").is_err());
        assert!(valida_nome("prn").is_err());
        assert!(valida_nome("aux").is_err());
        assert!(valida_nome("nul").is_err());
        assert!(valida_nome("com1").is_err());
        assert!(valida_nome("com9").is_err());
        assert!(valida_nome("lpt1").is_err());
        assert!(valida_nome("lpt9").is_err());
        // Nessun falso positivo su nomi solo simili.
        assert!(valida_nome("console").is_ok());
        assert!(valida_nome("com10").is_ok());
    }

    #[test]
    fn versione_valida_esatta_major_minor_patch() {
        let v = valida_versione("0.2.2").expect("versione valida");
        assert_eq!(
            v,
            PackageVersion {
                major: 0,
                minor: 2,
                patch: 2
            }
        );
    }

    #[test]
    fn versione_non_valida_rifiuta_componenti_extra_o_non_numerici() {
        assert!(valida_versione("1.2").is_err());
        assert!(valida_versione("1.2.3.4").is_err());
        assert!(valida_versione("1.2.x").is_err());
        assert!(valida_versione("1.2.-3").is_err());
        assert!(valida_versione("").is_err());
    }

    #[test]
    fn analizza_indice_tollerante_e_raggruppa_versione_massima() {
        let json = r#"[
            {"name": "cetz", "version": "0.1.0", "description": "d1"},
            {"name": "cetz", "version": "0.2.2", "description": "d2", "categories": ["visualization"]},
            {"name": "cetz", "version": "0.2.0", "description": "d3"},
            {"name": "BadName", "version": "1.0.0"},
            {"name": "senza-versione-valida", "version": "1.0"},
            {"qualcosa": "senza nome, voce scartata"},
            {"name": "template-esempio", "version": "1.0.0", "template": {"path": "template", "entrypoint": "main.typ"}}
        ]"#;

        let pacchetti = analizza_indice(json).expect("analisi riuscita");
        assert_eq!(pacchetti.len(), 2);

        let cetz = pacchetti
            .iter()
            .find(|p| p.nome == "cetz")
            .expect("cetz presente");
        assert_eq!(cetz.versione, "0.2.2");
        assert_eq!(cetz.descrizione, "d2");
        assert_eq!(cetz.categorie, vec!["visualization".to_string()]);
        assert!(!cetz.e_template);

        let template = pacchetti
            .iter()
            .find(|p| p.nome == "template-esempio")
            .expect("template-esempio presente");
        assert!(template.e_template);
    }

    #[test]
    fn analizza_indice_su_json_malformato_e_errore() {
        assert!(analizza_indice("non è json").is_err());
    }

    #[test]
    fn pubblica_in_scrive_lib_e_manifest_validi() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = pubblica_in(dir.path(), "mio-pacchetto", "0.1.0", "#let x = 1")
            .expect("pubblicazione riuscita");

        assert_eq!(percorso, dir.path().join("mio-pacchetto").join("0.1.0"));

        let lib = std::fs::read_to_string(percorso.join("lib.typ")).expect("lib.typ leggibile");
        assert_eq!(lib, "#let x = 1");

        let toml_testo =
            std::fs::read_to_string(percorso.join("typst.toml")).expect("typst.toml leggibile");
        let manifesto: PackageManifest =
            toml::from_str(&toml_testo).expect("typst.toml generato è TOML valido");
        assert_eq!(manifesto.package.name.as_str(), "mio-pacchetto");
        assert_eq!(manifesto.package.entrypoint.as_str(), "lib.typ");
        assert_eq!(
            manifesto.package.compiler.map(|c| c.to_string()),
            Some("0.15".to_string())
        );
    }

    #[test]
    fn pubblica_in_normalizza_versione_nel_percorso() {
        // Versione grezza non normalizzata ("01.0.0"): il percorso su disco
        // deve usare la forma normalizzata ("1.0.0"), quella che Typst
        // risolve per `@local/{nome}:{versione}` — altrimenti un
        // `NotFound` silenzioso (bug ~riga 438, revisione).
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso = pubblica_in(dir.path(), "norm", "01.0.0", "#let x = 1")
            .expect("pubblicazione riuscita");
        assert_eq!(
            percorso,
            dir.path().join("norm").join("1.0.0"),
            "il percorso su disco deve usare la versione normalizzata, non la stringa grezza"
        );
    }

    #[test]
    fn pubblica_in_rifiuta_nome_o_versione_non_validi() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(pubblica_in(dir.path(), "Nome-Non-Valido", "0.1.0", "x").is_err());
        assert!(pubblica_in(dir.path(), "nome", "0.1", "x").is_err());
    }

    #[test]
    fn pubblica_in_rifiuta_documento_troppo_grande() {
        let dir = tempfile::tempdir().expect("tempdir");
        let grande = "a".repeat(LIMITE_PUBBLICAZIONE + 1);
        assert!(pubblica_in(dir.path(), "grande", "0.1.0", &grande).is_err());
    }

    #[test]
    fn pubblica_in_sovrascrive_una_pubblicazione_precedente() {
        let dir = tempfile::tempdir().expect("tempdir");
        pubblica_in(dir.path(), "ripubblica", "0.1.0", "primo contenuto")
            .expect("prima pubblicazione riuscita");
        let percorso = pubblica_in(dir.path(), "ripubblica", "0.1.0", "secondo contenuto")
            .expect("ripubblicazione riuscita");

        let lib = std::fs::read_to_string(percorso.join("lib.typ")).expect("lib.typ leggibile");
        assert_eq!(lib, "secondo contenuto");
    }

    #[test]
    fn pubblica_in_non_lascia_cartelle_di_appoggio_dopo_la_ripubblicazione() {
        let dir = tempfile::tempdir().expect("tempdir");
        pubblica_in(dir.path(), "pulizia", "0.1.0", "primo").expect("prima pubblicazione riuscita");
        pubblica_in(dir.path(), "pulizia", "0.1.0", "secondo").expect("ripubblicazione riuscita");

        let voci: Vec<String> = std::fs::read_dir(dir.path().join("pulizia"))
            .expect("lettura cartella pacchetto")
            .filter_map(|v| v.ok())
            .map(|v| v.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            voci,
            vec!["0.1.0".to_string()],
            "nessuna cartella .tmp-/.old- residua: {voci:?}"
        );
    }

    #[test]
    fn pulisci_residui_orfani_non_tocca_un_residuo_fresco() {
        // Un residuo appena creato (mtime ≈ ora) simula l'appoggio vivo di
        // una pubblicazione concorrente dello stesso pacchetto ancora in
        // volo: non va cancellato, pena corrompere il suo `rename` finale
        // (vedi doc di `pulisci_residui_orfani`).
        let dir = tempfile::tempdir().expect("tempdir");
        let cartella_pacchetto = dir.path().join("concorrente");
        let residuo_fresco = cartella_pacchetto.join(".tmp-0.1.0-1234-1");
        std::fs::create_dir_all(&residuo_fresco).expect("creazione residuo fresco");

        pulisci_residui_orfani(&cartella_pacchetto);

        assert!(
            residuo_fresco.exists(),
            "un residuo fresco non va cancellato: potrebbe appartenere a una pubblicazione concorrente"
        );
    }

    #[test]
    fn pulisci_residui_orfani_ignora_le_cartelle_senza_prefisso_noto() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cartella_pacchetto = dir.path().join("qualsiasi");
        let versione_pubblicata = cartella_pacchetto.join("0.1.0");
        std::fs::create_dir_all(&versione_pubblicata).expect("creazione cartella versione");

        pulisci_residui_orfani(&cartella_pacchetto);

        assert!(
            versione_pubblicata.exists(),
            "una cartella di versione pubblicata non va mai toccata"
        );
    }

    #[test]
    fn residuo_e_da_rimuovere_solo_oltre_la_soglia_di_eta() {
        let adesso = SystemTime::now();

        // Fresco (pubblicazione concorrente plausibile): non va rimosso.
        let fresco = adesso - Duration::from_secs(5);
        assert!(!residuo_e_da_rimuovere(fresco, adesso));

        // Appena sotto la soglia: ancora non va rimosso.
        let poco_sotto_soglia = adesso - (SOGLIA_RESIDUI_ORFANI - Duration::from_secs(1));
        assert!(!residuo_e_da_rimuovere(poco_sotto_soglia, adesso));

        // Esattamente alla soglia o oltre (residuo di un crash passato): va rimosso.
        let alla_soglia = adesso - SOGLIA_RESIDUI_ORFANI;
        assert!(residuo_e_da_rimuovere(alla_soglia, adesso));

        let molto_vecchio = adesso - SOGLIA_RESIDUI_ORFANI - Duration::from_secs(3600);
        assert!(residuo_e_da_rimuovere(molto_vecchio, adesso));
    }

    #[test]
    fn residuo_e_da_rimuovere_falso_su_mtime_nel_futuro() {
        // Clock skew: `duration_since` fallisce, la funzione deve propendere
        // per NON rimuovere (nel dubbio si preserva il residuo).
        let adesso = SystemTime::now();
        let nel_futuro = adesso + Duration::from_secs(60);
        assert!(!residuo_e_da_rimuovere(nel_futuro, adesso));
    }

    #[test]
    fn gestisci_esito_rete_scrive_cache_su_successo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        let json = r#"[{"name": "cetz", "version": "0.2.2"}]"#;

        let esito =
            gestisci_esito_rete(Ok(json.to_string()), &percorso_cache).expect("dovrebbe riuscire");
        assert!(!esito.stantia);
        assert_eq!(esito.pacchetti.len(), 1);
        assert!(
            percorso_cache.exists(),
            "la cache andrebbe scritta su successo"
        );
    }

    #[test]
    fn gestisci_esito_rete_scrive_la_cache_in_modo_atomico() {
        // Cartella inesistente: viene creata; sovrascrittura completa e nessun
        // temporaneo residuo accanto alla cache (prova della scrittura atomica).
        let dir = tempfile::tempdir().expect("tempdir");
        let cartella = dir.path().join("sotto").join("cache");
        let percorso_cache = cartella.join("indice.json");
        let vecchio = r#"[{"name": "a", "version": "0.1.0"}]"#;
        let nuovo = r#"[{"name": "cetz", "version": "0.2.2"}]"#;
        gestisci_esito_rete(Ok(vecchio.to_string()), &percorso_cache).expect("primo");
        gestisci_esito_rete(Ok(nuovo.to_string()), &percorso_cache).expect("secondo");
        assert_eq!(
            std::fs::read_to_string(&percorso_cache).expect("lettura"),
            nuovo
        );
        let file: Vec<_> = std::fs::read_dir(&cartella).expect("dir").collect();
        assert_eq!(file.len(), 1, "nessun temporaneo residuo: {file:?}");
    }

    #[test]
    fn gestisci_esito_rete_ripiega_su_cache_fresca_senza_marcarla_stantia() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        std::fs::write(&percorso_cache, r#"[{"name": "cetz", "version": "0.2.2"}]"#)
            .expect("scrittura fixture");

        let esito = gestisci_esito_rete(Err("rete offline".to_string()), &percorso_cache)
            .expect("dovrebbe ripiegare sulla cache");
        assert!(
            !esito.stantia,
            "una cache ancora fresca non va marcata stantia"
        );
        assert_eq!(esito.pacchetti.len(), 1);
    }

    #[test]
    fn gestisci_esito_rete_ripiega_su_cache_stantia_oltre_ttl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        std::fs::write(&percorso_cache, r#"[{"name": "cetz", "version": "0.2.2"}]"#)
            .expect("scrittura fixture");
        // Retrodata il file oltre il TTL, per simulare una cache scaduta.
        let vecchia = std::time::SystemTime::now() - TTL_CACHE - Duration::from_secs(60);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&percorso_cache)
            .expect("apertura fixture in scrittura");
        file.set_modified(vecchia).expect("impostazione mtime");

        let esito = gestisci_esito_rete(Err("rete offline".to_string()), &percorso_cache)
            .expect("dovrebbe ripiegare sulla cache stantia");
        assert!(esito.stantia, "una cache oltre il TTL va marcata stantia");
    }

    #[test]
    fn gestisci_esito_rete_cache_non_json_trattata_come_assente() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        std::fs::write(&percorso_cache, "questo non è json valido").expect("scrittura fixture");

        let errore = gestisci_esito_rete(Err("rete offline".to_string()), &percorso_cache)
            .expect_err("una cache non parsabile va trattata come assente");
        assert!(errore.contains("nessuna cache locale disponibile"));
    }

    #[test]
    fn gestisci_esito_rete_senza_cache_e_errore_esplicito() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("non-esiste.json");

        let errore = gestisci_esito_rete(Err("rete offline".to_string()), &percorso_cache)
            .expect_err("senza rete e senza cache non c'è nulla da mostrare");
        assert!(errore.contains("nessuna cache locale disponibile"));
    }

    #[test]
    fn gestisci_esito_rete_risposta_200_non_json_ripiega_su_cache_fresca() {
        // Risposta 200 ma corpo non-JSON (es. captive portal/proxy che
        // intercetta la richiesta con una pagina HTML): non deve propagare
        // l'errore di parsing se una cache fresca è disponibile.
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        std::fs::write(&percorso_cache, r#"[{"name": "cetz", "version": "0.2.2"}]"#)
            .expect("scrittura fixture");

        let esito = gestisci_esito_rete(
            Ok("<html>portale captive</html>".to_string()),
            &percorso_cache,
        )
        .expect("dovrebbe ripiegare sulla cache invece di propagare l'errore di parsing");
        assert!(
            !esito.stantia,
            "una cache ancora fresca non va marcata stantia"
        );
        assert_eq!(esito.pacchetti.len(), 1);
    }

    #[test]
    fn gestisci_esito_rete_risposta_200_non_json_senza_cache_e_errore() {
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("non-esiste.json");

        let errore = gestisci_esito_rete(
            Ok("<html>portale captive</html>".to_string()),
            &percorso_cache,
        )
        .expect_err("senza cache, una risposta non valida deve restituire un errore esplicito");
        assert!(errore.contains("nessuna cache locale disponibile"));
    }

    #[test]
    fn gestisci_esito_rete_risposta_200_non_json_non_sovrascrive_cache_buona() {
        // Una risposta 200 corrotta non deve mai sovrascrivere una cache
        // buona esistente sul disco.
        let dir = tempfile::tempdir().expect("tempdir");
        let percorso_cache = dir.path().join("indice.json");
        let buona = r#"[{"name": "cetz", "version": "0.2.2"}]"#;
        std::fs::write(&percorso_cache, buona).expect("scrittura fixture");

        let _ = gestisci_esito_rete(
            Ok("<html>portale captive</html>".to_string()),
            &percorso_cache,
        );

        let contenuto = std::fs::read_to_string(&percorso_cache).expect("rilettura cache");
        assert_eq!(contenuto, buona, "la cache buona non va sovrascritta");
    }

    // ---- F5-012: aggiornamenti ----

    fn voce(nome: &str, versione: &str) -> PacchettoInfo {
        PacchettoInfo {
            nome: nome.into(),
            versione: versione.into(),
            descrizione: String::new(),
            autori: vec![],
            licenza: None,
            categorie: vec![],
            parole_chiave: vec![],
            repository: None,
            e_template: false,
        }
    }

    fn v(testo: &str) -> PackageVersion {
        valida_versione(testo).expect("versione di test")
    }

    /// Cache finta `<radice>/preview/<nome>/<versione>/`.
    fn cache_finta(pacchetti: &[(&str, &str)]) -> (tempfile::TempDir, FsPackages) {
        let dir = tempfile::tempdir().expect("tempdir");
        for (nome, versione) in pacchetti {
            std::fs::create_dir_all(dir.path().join("preview").join(nome).join(versione))
                .expect("cartella");
        }
        let cache = FsPackages::new(dir.path());
        (dir, cache)
    }

    #[test]
    fn confronto_semver_numerico_non_lessicografico() {
        // 0.10.0 > 0.9.0 (con un confronto fra stringhe sarebbe il contrario).
        let indice = [voce("cetz", "0.10.0")];
        let agg = aggiornamenti_da(&[("cetz".into(), v("0.9.0"))], &indice);
        assert_eq!(
            agg,
            vec![AggiornamentoPacchetto {
                nome: "cetz".into(),
                attuale: "0.9.0".into(),
                ultima: "0.10.0".into()
            }]
        );
        // Stessa versione o più recente dell'indice: nessuna segnalazione.
        assert!(aggiornamenti_da(&[("cetz".into(), v("0.10.0"))], &indice).is_empty());
        assert!(aggiornamenti_da(&[("cetz".into(), v("1.0.0"))], &indice).is_empty());
        // Patch e major.
        assert_eq!(
            aggiornamenti_da(&[("cetz".into(), v("0.9.9"))], &indice).len(),
            1
        );
        let indice = [voce("a", "2.0.0")];
        assert_eq!(
            aggiornamenti_da(&[("a".into(), v("1.99.99"))], &indice).len(),
            1
        );
    }

    #[test]
    fn ultima_versione_e_il_massimo_per_nome_dell_indice() {
        let indice = [
            voce("x", "1.2.0"),
            voce("x", "1.10.0"),
            voce("x", "1.9.0"),
            voce("x", "rotta"),
        ];
        let agg = aggiornamenti_da(&[("x".into(), v("1.9.0"))], &indice);
        assert_eq!(agg[0].ultima, "1.10.0");
    }

    #[test]
    fn pacchetto_assente_dall_indice_non_si_segnala() {
        let agg = aggiornamenti_da(&[("locale".into(), v("0.1.0"))], &[voce("altro", "9.0.0")]);
        assert!(agg.is_empty());
    }

    #[test]
    fn installate_prende_la_versione_massima_e_ignora_il_resto() {
        let (dir, cache) = cache_finta(&[
            ("cetz", "0.2.2"),
            ("cetz", "0.10.1"),
            ("cetz", "0.9.0"),
            ("tablex", "0.0.8"),
            ("Nome Maiuscolo", "1.0.0"),
        ]);
        // Un file sparso e una cartella senza versioni non devono rompere nulla.
        std::fs::write(dir.path().join("preview").join("file.txt"), "x").expect("file");
        std::fs::create_dir_all(dir.path().join("preview").join("vuoto")).expect("vuoto");
        let trovate = installate_in(&cache);
        assert_eq!(
            trovate,
            vec![
                ("cetz".to_string(), v("0.10.1")),
                ("tablex".to_string(), v("0.0.8"))
            ]
        );
    }

    #[test]
    fn installate_cache_assente_e_vuota() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(installate_in(&FsPackages::new(dir.path().join("non-esiste"))).is_empty());
    }

    #[test]
    fn import_del_documento_trova_solo_gli_import_preview() {
        let src = "#import \"@preview/cetz:0.2.2\": canvas\n\
                   #import \"@local/mio:1.0.0\": *\n\
                   #import \"file.typ\": x\n\
                   #let s = \"@preview/finto:9.9.9\"\n\
                   #{ import \"@preview/tablex:0.0.8\": * }\n\
                   #import \"@preview/cetz:0.2.2\": draw\n";
        assert_eq!(
            import_preview_da_sorgente(src),
            vec![
                ("cetz".to_string(), v("0.2.2")),
                ("tablex".to_string(), v("0.0.8")),
                ("cetz".to_string(), v("0.2.2")),
            ]
        );
        assert!(import_preview_da_sorgente("= Solo testo").is_empty());
        // Specifica malformata: ignorata, nessun panic.
        assert!(import_preview_da_sorgente("#import \"@preview/cetz\": *").is_empty());
    }

    #[test]
    fn controlla_con_combina_cache_e_documento_senza_duplicati() {
        let (_dir, cache) = cache_finta(&[("cetz", "0.2.2"), ("tablex", "0.0.8")]);
        let indice = [
            voce("cetz", "0.3.0"),
            voce("tablex", "0.0.8"),
            voce("touying", "0.5.0"),
        ];
        let src = "#import \"@preview/cetz:0.2.2\": *\n\
                   #import \"@preview/cetz:0.2.2\": draw\n\
                   #import \"@preview/touying:0.4.0\": *\n\
                   #import \"@preview/tablex:0.0.8\": *";
        let esito = controlla_con(&cache, &indice, Some(src));
        assert_eq!(
            esito.installati,
            vec![AggiornamentoPacchetto {
                nome: "cetz".into(),
                attuale: "0.2.2".into(),
                ultima: "0.3.0".into()
            }]
        );
        let nomi: Vec<_> = esito
            .documento
            .iter()
            .map(|a| (a.nome.as_str(), a.attuale.as_str()))
            .collect();
        assert_eq!(nomi, vec![("cetz", "0.2.2"), ("touying", "0.4.0")]);
        // Senza sorgente il controllo del documento è vuoto.
        assert!(controlla_con(&cache, &indice, None).documento.is_empty());
    }
}
