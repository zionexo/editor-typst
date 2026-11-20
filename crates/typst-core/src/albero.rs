//! Albero dei file della cartella-progetto, in sola consultazione (F5-021, D-015).
//!
//! Funzione pura di elenco: legge la cartella, non modifica nulla e non
//! conosce Tauri né il `World`. Regole:
//! - **ordinamento**: cartelle prima, poi file; dentro ogni gruppo l'ordine è
//!   alfabetico senza distinzione fra maiuscole e minuscole e *naturale*
//!   (`cap2` prima di `cap10`);
//! - **esclusioni** in un solo posto ([`CARTELLE_RUMORE`], [`CARTELLE_SOLO_ALBERO`],
//!   [`FILE_RUMORE`]): ogni voce nascosta (nome che inizia con `.`), le cartelle
//!   di artefatti noti e i file di sistema. Le stesse liste servono al watcher
//!   (`esterni::percorso_ignorato`), con la distinzione documentata sotto;
//! - **niente uscite dalla radice**: i collegamenti simbolici (e le giunzioni
//!   di Windows) non si seguono e non compaiono; percorsi con nomi non UTF-8
//!   sono ignorati (non rappresentabili verso il frontend);
//! - **limiti**: profondità e numero di voci sono limitati (`troncato` lo
//!   segnala), così una cartella enorme non blocca l'interfaccia.

use std::cmp::Ordering;
use std::path::Path;

use serde::Serialize;

/// Cartelle di puro rumore (mai risorse di un documento Typst), escluse **sia**
/// dall'albero **sia** dal watcher (oltre alle nascoste).
pub const CARTELLE_RUMORE: &[&str] = &["node_modules", "__pycache__"];

/// Cartelle nascoste **solo nell'albero** (decisione 2026-10-08, vedi
/// DECISIONI.md): `target/` è l'output di Cargo, rumore da non elencare, ma può
/// anche essere una cartella legittima di un progetto utente che contiene
/// risorse lette dal documento (`#image("target/fig.png")`). Il watcher quindi
/// NON la ignora (un evento su una risorsa vera deve invalidare la cache); evita
/// solo di richiedere il ricaricamento dell'albero, vedi
/// `esterni::percorso_fuori_dall_albero`.
pub const CARTELLE_SOLO_ALBERO: &[&str] = &["target"];

/// File di sistema, esclusi dall'albero e dal watcher (oltre ai nascosti).
pub const FILE_RUMORE: &[&str] = &["Thumbs.db", "desktop.ini"];

/// `true` se il nome di file è un file di sistema da ignorare (senza
/// distinguere le maiuscole, come i filesystem di Windows/macOS).
pub fn e_file_rumore(nome: &str) -> bool {
    FILE_RUMORE.iter().any(|f| f.eq_ignore_ascii_case(nome))
}

/// Profondità massima di discesa (la radice è a profondità 0).
pub const PROFONDITA_MASSIMA: usize = 32;

/// Numero massimo di voci elencate in totale.
pub const NODI_MASSIMI: usize = 20_000;

/// Tipo di una voce dell'albero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoNodo {
    /// Cartella (comprimibile).
    Cartella,
    /// File `.typ` o `.md`: apribile nell'editor.
    Documento,
    /// Qualunque altro file (immagini, dati, font…): mostrato, non aperto.
    Risorsa,
}

/// Una voce dell'albero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodoFile {
    /// Nome (ultimo segmento).
    pub nome: String,
    /// Percorso relativo alla radice, con separatore `/`.
    pub percorso: String,
    /// Cartella, documento o risorsa.
    pub tipo: TipoNodo,
    /// Figli (solo per le cartelle), già ordinati.
    pub figli: Vec<NodoFile>,
}

/// Albero dei file di una cartella-progetto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlberoFile {
    /// Percorso assoluto della radice elencata (per mostrarne il nome).
    pub radice: String,
    /// Voci al primo livello, già ordinate.
    pub nodi: Vec<NodoFile>,
    /// `true` se l'elenco è stato interrotto per i limiti di profondità/voci.
    pub troncato: bool,
}

/// Confronto "naturale" e senza distinzione fra maiuscole/minuscole di due
/// nomi: le sequenze di cifre si confrontano per valore (`cap2` < `cap10`).
/// A parità si ripiega sul confronto esatto, così l'ordine è totale e stabile.
pub fn confronta_nomi(a: &str, b: &str) -> Ordering {
    let mut ia = a.chars().peekable();
    let mut ib = b.chars().peekable();
    loop {
        match (ia.peek().copied(), ib.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) if ca.is_ascii_digit() && cb.is_ascii_digit() => {
                let na = prendi_cifre(&mut ia);
                let nb = prendi_cifre(&mut ib);
                // Senza zeri iniziali: più cifre = numero più grande; a pari
                // lunghezza il confronto lessicografico coincide con quello numerico.
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(ca), Some(cb)) => {
                let ord = ca.to_lowercase().cmp(cb.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                ia.next();
                ib.next();
            }
        }
    }
}

/// Consuma e restituisce la sequenza di cifre ASCII all'inizio dell'iteratore.
fn prendi_cifre(it: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut cifre = String::new();
    while let Some(c) = it.peek().copied() {
        if !c.is_ascii_digit() {
            break;
        }
        cifre.push(c);
        it.next();
    }
    cifre
}

/// `true` se una voce con questo nome va nascosta.
fn da_escludere(nome: &str, e_cartella: bool) -> bool {
    if nome.starts_with('.') {
        return true;
    }
    if e_cartella {
        CARTELLE_RUMORE.contains(&nome) || CARTELLE_SOLO_ALBERO.contains(&nome)
    } else {
        e_file_rumore(nome)
    }
}

/// Tipo di un file dal suo nome (estensione `.typ`/`.md`, senza distinguere le maiuscole).
fn tipo_file(nome: &str) -> TipoNodo {
    let documento = Path::new(nome)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("typ") || e.eq_ignore_ascii_case("md"));
    if documento {
        TipoNodo::Documento
    } else {
        TipoNodo::Risorsa
    }
}

/// Forma di un percorso adatta al frontend: toglie il prefisso verbatim
/// `\\?\` che `canonicalize` produce su Windows, **solo** per i percorsi con
/// lettera di unità (`\\?\C:\...` → `C:\...`). I percorsi UNC (`\\?\UNC\server\...`)
/// e gli altri verbatim (`\\?\Volume{...}`) restano invariati: toglierne il
/// prefisso li renderebbe percorsi diversi. Pura e testabile su ogni piattaforma.
pub fn normalizza_verbatim(percorso: &str) -> String {
    if let Some(resto) = percorso.strip_prefix(r"\\?\") {
        let b = resto.as_bytes();
        let con_unita = b.len() >= 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b.len() == 2 || b[2] == b'\\');
        if con_unita {
            return resto.to_string();
        }
    }
    percorso.to_string()
}

/// [`normalizza_verbatim`] applicata a un `Path` (conversione lossy, come
/// `display`). Da usare per ogni percorso serializzato verso il frontend.
pub fn percorso_per_frontend(percorso: &Path) -> String {
    normalizza_verbatim(&percorso.to_string_lossy())
}

/// Elenca l'albero di `radice`.
///
/// # Errori
/// `Err` solo se la radice stessa non è leggibile. Una sottocartella non
/// leggibile (permessi, rimossa nel frattempo) viene saltata: l'albero
/// mostra ciò che si riesce a leggere.
pub fn elenca_albero(radice: &Path) -> Result<AlberoFile, String> {
    let mut conteggio = 0usize;
    let mut troncato = false;
    let nodi = leggi_cartella(radice, "", 0, &mut conteggio, &mut troncato).map_err(|e| {
        format!(
            "Impossibile leggere la cartella del progetto '{}': {e}",
            radice.display()
        )
    })?;
    Ok(AlberoFile {
        radice: percorso_per_frontend(radice),
        nodi,
        troncato,
    })
}

/// Legge una cartella (`prefisso` = percorso relativo con `/` finale, o vuoto).
fn leggi_cartella(
    cartella: &Path,
    prefisso: &str,
    profondita: usize,
    conteggio: &mut usize,
    troncato: &mut bool,
) -> std::io::Result<Vec<NodoFile>> {
    let mut nodi = Vec::new();
    for voce in std::fs::read_dir(cartella)? {
        let Ok(voce) = voce else { continue };
        let Some(nome) = voce.file_name().to_str().map(str::to_string) else {
            continue;
        };
        // `file_type` non segue i collegamenti: symlink e giunzioni si riconoscono
        // e si scartano, così non si esce mai dalla radice.
        let Ok(tipo) = voce.file_type() else { continue };
        if tipo.is_symlink() {
            continue;
        }
        let e_cartella = tipo.is_dir();
        if !e_cartella && !tipo.is_file() {
            continue;
        }
        if da_escludere(&nome, e_cartella) {
            continue;
        }
        if *conteggio >= NODI_MASSIMI {
            *troncato = true;
            break;
        }
        *conteggio += 1;
        let percorso = format!("{prefisso}{nome}");
        if e_cartella {
            let figli = if profondita >= PROFONDITA_MASSIMA {
                *troncato = true;
                Vec::new()
            } else {
                // Sottocartella non leggibile: si mostra vuota, l'albero prosegue.
                leggi_cartella(
                    &voce.path(),
                    &format!("{percorso}/"),
                    profondita + 1,
                    conteggio,
                    troncato,
                )
                .unwrap_or_default()
            };
            nodi.push(NodoFile {
                nome,
                percorso,
                tipo: TipoNodo::Cartella,
                figli,
            });
        } else {
            let tipo = tipo_file(&nome);
            nodi.push(NodoFile {
                nome,
                percorso,
                tipo,
                figli: Vec::new(),
            });
        }
    }
    nodi.sort_by(|a, b| {
        let cartella = |n: &NodoFile| n.tipo == TipoNodo::Cartella;
        cartella(b)
            .cmp(&cartella(a))
            .then_with(|| confronta_nomi(&a.nome, &b.nome))
    });
    Ok(nodi)
}

#[cfg(test)]
#[path = "../tests/unit/albero.rs"]
mod test;
