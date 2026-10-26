//! Logica pura dell'esportazione (F5-011): intervalli di pagine, standard PDF
//! (PDF/A incluso), limiti del PNG e nomi dei file per pagina.
//!
//! Nessun I/O e nessuna dipendenza dal `World`: tutto è testabile senza
//! compilare documenti. Il rendering vero e proprio (PNG/SVG/PDF) sta in
//! `world.rs`, che usa i tipi di questo modulo. Le parti derivate da
//! `typst-cli` v0.15.0 (Apache-2.0) portano il marcatore `// Derivato da`
//! richiesto da fase8.md §0 (F5-024).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use typst::foundations::Datetime;
use typst::layout::PageRanges;
use typst_pdf::{PdfOptions, PdfStandards, Timestamp};

/// Risoluzione predefinita del PNG (pixel per pollice), come `typst-cli`.
pub const PPI_PREDEFINITO: f32 = 144.0;
/// Risoluzione massima accettata: oltre, un foglio normale supera comunque il
/// limite di [`PIXEL_MASSIMI`]; il tetto evita valori assurdi e input non finiti.
pub const PPI_MASSIMO: f32 = 1200.0;
/// Numero massimo di pixel di una singola pagina PNG (100 Mpx ≈ 400 MB di
/// RGBA in memoria): protegge da esaurimento della RAM con pagine grandi o DPI alti.
pub const PIXEL_MASSIMI: u64 = 100_000_000;

// ---------------------------------------------------------------------
// Intervallo di pagine
// ---------------------------------------------------------------------

// Derivato da typst-cli crates/typst-cli/src/args.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
// Adattato: messaggi in italiano, elenco separato da virgole, testo vuoto = tutte le pagine.

/// Un intervallo `1-3`, `4`, `5-` o `-2` (numeri di pagina a partire da 1).
type Intervallo = std::ops::RangeInclusive<Option<NonZeroUsize>>;

/// Analizza un singolo intervallo (`1-3`, `4`, `5-`, `-2`).
fn analizza_intervallo(valore: &str) -> Result<Intervallo, &'static str> {
    match valore
        .split('-')
        .map(str::trim)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | [""] => Err("intervallo di pagine vuoto"),
        [singola] => {
            let numero = analizza_numero_pagina(singola)?;
            Ok(Some(numero)..=Some(numero))
        }
        ["", ""] => Err("l'intervallo deve avere un inizio o una fine"),
        [inizio, ""] => Ok(Some(analizza_numero_pagina(inizio)?)..=None),
        ["", fine] => Ok(None..=Some(analizza_numero_pagina(fine)?)),
        [inizio, fine] => {
            let inizio = analizza_numero_pagina(inizio)?;
            let fine = analizza_numero_pagina(fine)?;
            if inizio > fine {
                Err("l'intervallo deve finire in una pagina successiva all'inizio")
            } else {
                Ok(Some(inizio)..=Some(fine))
            }
        }
        [_, _, _, ..] => Err("l'intervallo deve contenere un solo trattino"),
    }
}

/// Numero di pagina valido: intero positivo (le pagine partono da 1).
fn analizza_numero_pagina(valore: &str) -> Result<NonZeroUsize, &'static str> {
    if valore == "0" {
        Err("le pagine partono da 1")
    } else {
        NonZeroUsize::from_str(valore).map_err(|_| "numero di pagina non valido")
    }
}

/// Analizza un elenco di intervalli separati da virgola (`1-3, 5, 8-`).
///
/// Testo vuoto (o solo spazi) = nessuna restrizione → `Ok(None)`. Un elemento
/// vuoto dentro l'elenco (`1,,3`) è un errore, non viene ignorato in silenzio.
pub fn analizza_pagine(testo: &str) -> Result<Option<PageRanges>, String> {
    if testo.trim().is_empty() {
        return Ok(None);
    }
    let mut intervalli = Vec::new();
    for pezzo in testo.split(',') {
        let intervallo = analizza_intervallo(pezzo)
            .map_err(|e| format!("intervallo di pagine non valido '{}': {e}", pezzo.trim()))?;
        intervalli.push(intervallo);
    }
    Ok(Some(PageRanges::new(intervalli)))
}

/// Indici (base 0) delle pagine selezionate su `totale`. `None` = tutte.
pub fn pagine_selezionate(totale: usize, intervalli: Option<&PageRanges>) -> Vec<usize> {
    (0..totale)
        .filter(|&i| intervalli.is_none_or(|r| r.includes_page_index(i)))
        .collect()
}

// ---------------------------------------------------------------------
// Standard PDF
// ---------------------------------------------------------------------

/// Standard PDF selezionabili (versioni, PDF/A, PDF/UA-1).
///
/// Il core li mappa **tutti** verso `typst_pdf::PdfStandard` (D-011): la UI ne
/// mostra solo alcuni (PDF/A-2b, 3b, 4) e potrà esporre gli altri senza toccare
/// questo tipo. I nomi serializzati coincidono con quelli di `typst-cli`
/// (`a-2b`, `1.7`, `ua-1`…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum StandardPdf {
    #[serde(rename = "1.4")]
    V1_4,
    #[serde(rename = "1.5")]
    V1_5,
    #[serde(rename = "1.6")]
    V1_6,
    #[serde(rename = "1.7")]
    V1_7,
    #[serde(rename = "2.0")]
    V2_0,
    #[serde(rename = "a-1b")]
    A1b,
    #[serde(rename = "a-1a")]
    A1a,
    #[serde(rename = "a-2b")]
    A2b,
    #[serde(rename = "a-2u")]
    A2u,
    #[serde(rename = "a-2a")]
    A2a,
    #[serde(rename = "a-3b")]
    A3b,
    #[serde(rename = "a-3u")]
    A3u,
    #[serde(rename = "a-3a")]
    A3a,
    #[serde(rename = "a-4")]
    A4,
    #[serde(rename = "a-4f")]
    A4f,
    #[serde(rename = "a-4e")]
    A4e,
    #[serde(rename = "ua-1")]
    Ua1,
}

impl StandardPdf {
    /// Nome leggibile per i messaggi (`PDF/A-2b`, `PDF 1.7`, `PDF/UA-1`).
    pub fn nome(self) -> &'static str {
        match self {
            Self::V1_4 => "PDF 1.4",
            Self::V1_5 => "PDF 1.5",
            Self::V1_6 => "PDF 1.6",
            Self::V1_7 => "PDF 1.7",
            Self::V2_0 => "PDF 2.0",
            Self::A1b => "PDF/A-1b",
            Self::A1a => "PDF/A-1a",
            Self::A2b => "PDF/A-2b",
            Self::A2u => "PDF/A-2u",
            Self::A2a => "PDF/A-2a",
            Self::A3b => "PDF/A-3b",
            Self::A3u => "PDF/A-3u",
            Self::A3a => "PDF/A-3a",
            Self::A4 => "PDF/A-4",
            Self::A4f => "PDF/A-4f",
            Self::A4e => "PDF/A-4e",
            Self::Ua1 => "PDF/UA-1",
        }
    }

    /// `true` se lo standard impone i tag di accessibilità (varianti `a` di
    /// PDF/A e PDF/UA-1): incompatibile con un intervallo di pagine, che
    /// spegne i tag (D-016, fase8.md §3.2).
    pub fn richiede_tag(self) -> bool {
        matches!(self, Self::A1a | Self::A2a | Self::A3a | Self::Ua1)
    }
}

// Derivato da typst-cli crates/typst-cli/src/compile.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
// Mappatura `From<PdfStandard> for typst_pdf::PdfStandard`, con i nomi di varianti del core.
impl From<StandardPdf> for typst_pdf::PdfStandard {
    fn from(standard: StandardPdf) -> Self {
        use typst_pdf::PdfStandard as P;
        match standard {
            StandardPdf::V1_4 => P::V_1_4,
            StandardPdf::V1_5 => P::V_1_5,
            StandardPdf::V1_6 => P::V_1_6,
            StandardPdf::V1_7 => P::V_1_7,
            StandardPdf::V2_0 => P::V_2_0,
            StandardPdf::A1b => P::A_1b,
            StandardPdf::A1a => P::A_1a,
            StandardPdf::A2b => P::A_2b,
            StandardPdf::A2u => P::A_2u,
            StandardPdf::A2a => P::A_2a,
            StandardPdf::A3b => P::A_3b,
            StandardPdf::A3u => P::A_3u,
            StandardPdf::A3a => P::A_3a,
            StandardPdf::A4 => P::A_4,
            StandardPdf::A4f => P::A_4f,
            StandardPdf::A4e => P::A_4e,
            StandardPdf::Ua1 => P::Ua_1,
        }
    }
}

/// Opzioni d'esportazione PDF ricevute dal guscio (deserializzabili dal JSON
/// del frontend). Il valore predefinito equivale a `PdfOptions::default()`:
/// nessuno standard, tutte le pagine, PDF taggato.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OpzioniPdf {
    /// Standard richiesti. La UI ne invia al più uno; il tipo è una lista per
    /// lasciare a `typst-pdf` la validazione delle combinazioni (es. un
    /// PDF/A più una versione PDF compatibile).
    pub standard: Vec<StandardPdf>,
    /// Intervallo di pagine (`1-3, 5`); `None` o vuoto = tutte.
    pub pagine: Option<String>,
}

impl OpzioniPdf {
    /// Intervallo di pagine analizzato (`None` = tutte).
    pub fn intervalli(&self) -> Result<Option<PageRanges>, String> {
        analizza_pagine(self.pagine.as_deref().unwrap_or(""))
    }

    /// Converte in `PdfOptions` di `typst-pdf`, validando **prima** di
    /// esportare:
    /// - intervallo di pagine ben formato;
    /// - regola `tagged = pagine.is_none()` (come typst-cli e Tinymist);
    ///   quindi con un intervallo gli standard che richiedono i tag (varianti
    ///   `a`, PDF/UA-1) sono rifiutati con un messaggio leggibile;
    /// - combinazione di standard ammessa (`PdfStandards::new`), con i
    ///   suggerimenti di `typst-pdf` accodati al messaggio.
    ///
    /// Con uno standard richiesto imposta anche la data di creazione (ora
    /// UTC): PDF/A la esige ("missing document date") e `typst-pdf` non la
    /// inventa. Senza standard il PDF resta identico a quello di prima.
    pub fn a_opzioni_typst(&self) -> Result<PdfOptions, String> {
        let ora = if self.standard.is_empty() {
            None
        } else {
            timestamp_adesso()
        };
        self.con_timestamp(ora)
    }

    /// Come [`Self::a_opzioni_typst`] con il timestamp esplicito (testabile).
    fn con_timestamp(&self, timestamp: Option<Timestamp>) -> Result<PdfOptions, String> {
        let intervalli = self.intervalli()?;

        if intervalli.is_some() {
            if let Some(s) = self.standard.iter().find(|s| s.richiede_tag()) {
                return Err(format!(
                    "{} richiede i tag di accessibilità, che con un intervallo di pagine \
                     non si possono scrivere: esporta tutte le pagine oppure scegli \
                     PDF/A-2b, PDF/A-3b o PDF/A-4",
                    s.nome()
                ));
            }
        }

        let elenco: Vec<typst_pdf::PdfStandard> =
            self.standard.iter().copied().map(Into::into).collect();
        let standards = PdfStandards::new(&elenco).map_err(|e| {
            let mut testo = format!("standard PDF non applicabile: {}", e.message());
            for suggerimento in e.hints() {
                testo.push_str(&format!(" ({suggerimento})"));
            }
            testo
        })?;

        Ok(PdfOptions {
            timestamp,
            page_ranges: intervalli.clone(),
            standards,
            tagged: intervalli.is_none(),
            ..PdfOptions::default()
        })
    }
}

// ---------------------------------------------------------------------
// Data di creazione
// ---------------------------------------------------------------------

/// Timestamp UTC dell'istante corrente (`None` se non rappresentabile).
fn timestamp_adesso() -> Option<Timestamp> {
    use chrono::{Datelike, Timelike, Utc};
    let ora = Utc::now();
    Datetime::from_ymd_hms(
        ora.year(),
        u8::try_from(ora.month()).ok()?,
        u8::try_from(ora.day()).ok()?,
        u8::try_from(ora.hour()).ok()?,
        u8::try_from(ora.minute()).ok()?,
        u8::try_from(ora.second()).ok()?,
    )
    .map(Timestamp::new_utc)
}

// ---------------------------------------------------------------------
// PNG: limiti
// ---------------------------------------------------------------------

/// Valida la risoluzione (finita, positiva, non oltre [`PPI_MASSIMO`]).
pub fn valida_ppi(ppi: f32) -> Result<f32, String> {
    if !ppi.is_finite() || !(1.0..=PPI_MASSIMO).contains(&ppi) {
        return Err(format!(
            "risoluzione non valida: {ppi} PPI (ammessi da 1 a {PPI_MASSIMO})"
        ));
    }
    Ok(ppi)
}

/// Pixel di una pagina `larghezza_pt` × `altezza_pt` (punti tipografici) alla
/// risoluzione `ppi`. Come `typst_render::render` calcola in `f32`
/// (`pixel_per_pt * dimensione`, arrotondato, minimo 1 px per lato).
pub fn pixel_pagina(larghezza_pt: f32, altezza_pt: f32, ppi: f32) -> u64 {
    let scala = ppi / 72.0;
    let w = (scala * larghezza_pt).round().max(1.0) as u64;
    let h = (scala * altezza_pt).round().max(1.0) as u64;
    w.saturating_mul(h)
}

/// Risoluzione massima intera per cui la pagina resta entro [`PIXEL_MASSIMI`]
/// (e comunque non oltre [`PPI_MASSIMO`]); almeno 1.
pub fn ppi_massimo_pagina(larghezza_pt: f32, altezza_pt: f32) -> u32 {
    let mut ppi = PPI_MASSIMO as u32;
    while ppi > 1 && pixel_pagina(larghezza_pt, altezza_pt, ppi as f32) > PIXEL_MASSIMI {
        ppi -= 1;
    }
    ppi
}

// ---------------------------------------------------------------------
// Nomi dei file per pagina
// ---------------------------------------------------------------------

/// Percorsi di destinazione delle pagine esportate.
///
/// `pagine` sono i numeri (base 1) delle pagine scelte, `totale` le pagine del
/// documento. Una sola pagina → esattamente `destinazione` (il file che l'utente
/// ha confermato nel dialog di salvataggio). Più pagine → `<nome>-<0p>.<estensione>`
/// nella stessa cartella, con numero riempito di zeri (`doc-01.png`).
pub fn percorsi_pagine(destinazione: &Path, pagine: &[usize], totale: usize) -> Vec<PathBuf> {
    if pagine.len() == 1 {
        return vec![destinazione.to_path_buf()];
    }
    let cartella = destinazione.parent().unwrap_or_else(|| Path::new(""));
    // Il nome scelto dall'utente si usa alla lettera (anche con graffe): la
    // numerazione si costruisce direttamente, senza passare dai segnaposto.
    let nome = destinazione
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "pagina".to_string());
    let estensione = destinazione
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let larghezza = 1 + totale.checked_ilog10().unwrap_or(0) as usize;
    pagine
        .iter()
        .map(|p| cartella.join(format!("{nome}-{p:0larghezza$}{estensione}")))
        .collect()
}

/// Percorsi di `percorsi` che esistono già su disco, **escluso** `scelto`: il
/// file scelto nel dialog di salvataggio ha già avuto la conferma di
/// sovrascrittura dal sistema, gli altri (`nome-NN.ext`) no.
pub fn percorsi_esistenti(percorsi: &[PathBuf], scelto: &Path) -> Vec<PathBuf> {
    percorsi
        .iter()
        .filter(|p| p.as_path() != scelto && p.exists())
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------
// Destinazione dell'export
// ---------------------------------------------------------------------

/// Estensioni dei file sorgente, mai ammesse come destinazione di un export
/// (sovrascriverebbero il lavoro dell'utente con un PDF/PNG/SVG).
const ESTENSIONI_SORGENTE: [&str; 3] = ["typ", "typc", "md"];

/// Decide se `destinazione` può ricevere un export.
///
/// Rifiuta (con messaggio in italiano) il **documento aperto** (`documento`,
/// confronto robusto a maiuscole, separatori e prefisso verbatim) e qualunque
/// file con estensione sorgente `.typ`/`.typc`/`.md`.
pub fn verifica_destinazione_export(
    destinazione: &Path,
    documento: Option<&Path>,
) -> Result<(), String> {
    if documento.is_some_and(|d| crate::esterni::stesso_file(d, destinazione)) {
        return Err(format!(
            "La destinazione '{}' è il documento aperto: scegli un altro nome, \
             l'export lo sovrascriverebbe.",
            destinazione.display()
        ));
    }
    verifica_nome_destinazione(destinazione, &ESTENSIONI_SORGENTE)
}

/// `true` se il nome di file (ultimo componente) è insidioso su Windows: il
/// sistema scarta punti e spazi finali (`capitolo.typ.` apre `capitolo.typ`) e
/// `:` introduce un flusso alternativo (`capitolo.typ::$DATA` è `capitolo.typ`).
/// Il filtro sull'estensione, fatto sul testo del nome, verrebbe aggirato mentre
/// la scrittura colpisce il file sorgente.
pub fn nome_insidioso(nome: &str) -> bool {
    nome.ends_with(['.', ' ']) || nome.contains(':')
}

/// Controlla il **nome** di una destinazione di scrittura: rifiuta i nomi
/// [`nome_insidioso`] (su Windows) e verifica l'estensione sia sul nome scelto
/// sia sul percorso **canonico** a cui la scrittura arriverà davvero (un
/// collegamento `out.pdf` che punta a `capitolo.typ` sovrascriverebbe il
/// sorgente: [`crate::config::scrivi_atomico`] scrive sul bersaglio risolto).
///
/// `vietate`: estensioni (minuscole, senza punto) non ammesse.
pub fn verifica_nome_destinazione(destinazione: &Path, vietate: &[&str]) -> Result<(), String> {
    if cfg!(windows) {
        let nome = destinazione
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if nome_insidioso(&nome) {
            return Err(format!(
                "Il nome '{nome}' non è ammesso come destinazione: non può finire con \
                 un punto o uno spazio né contenere ':'."
            ));
        }
    }
    let canonico = crate::cronologia::percorso_stabile(destinazione);
    for candidato in [destinazione, canonico.as_path()] {
        let estensione = candidato
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if let Some(e) = estensione {
            if vietate.contains(&e.as_str()) {
                return Err(format!(
                    "La destinazione '{}' ha estensione .{e} (file '{}'), riservata ai \
                     file sorgente: scegli un'estensione di export (.pdf, .png, .svg).",
                    destinazione.display(),
                    candidato.display()
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Errori ripetuti dell'export PDF
// ---------------------------------------------------------------------

/// Quante posizioni (riga/colonna) elencare al massimo per un errore ripetuto.
pub const MAX_POSIZIONI_ELENCATE: usize = 5;

/// Errore di export PDF: messaggio e, se noto, posizione `(riga, colonna)`
/// 1-based nel documento aperto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorePdf {
    /// Messaggio del compilatore.
    pub messaggio: String,
    /// Riga e colonna (1-based) nel file in editing, se lo span vi appartiene.
    pub posizione: Option<(usize, usize)>,
}

/// Raggruppa gli errori con lo stesso messaggio in un solo testo: conteggio,
/// prime [`MAX_POSIZIONI_ELENCATE`] posizioni e, per il testo alternativo
/// mancante degli standard con tag (A-1a, A-2a, A-3a, UA-1), il suggerimento
/// `alt:`. L'ordine è quello della prima comparsa di ciascun messaggio.
pub fn raggruppa_errori_pdf(errori: Vec<ErrorePdf>) -> Vec<String> {
    // (messaggio, occorrenze, posizioni)
    type Gruppo = (String, usize, Vec<(usize, usize)>);
    let mut gruppi: Vec<Gruppo> = Vec::new();
    for e in errori {
        let indice = match gruppi.iter().position(|g| g.0 == e.messaggio) {
            Some(i) => i,
            None => {
                gruppi.push((e.messaggio, 0, Vec::new()));
                gruppi.len() - 1
            }
        };
        gruppi[indice].1 += 1;
        if let Some(p) = e.posizione {
            gruppi[indice].2.push(p);
        }
    }
    gruppi
        .into_iter()
        .map(|(messaggio, n, posizioni)| {
            let elenco = posizioni
                .iter()
                .take(MAX_POSIZIONI_ELENCATE)
                .map(|(r, c)| format!("riga {r}, colonna {c}"))
                .collect::<Vec<_>>()
                .join("; ");
            let mut testo = messaggio.clone();
            if n > 1 {
                testo.push_str(&format!(" ({n} occorrenze"));
                if !elenco.is_empty() {
                    testo.push_str(&format!("; prime posizioni: {elenco}"));
                }
                testo.push(')');
            } else if !elenco.is_empty() {
                testo.push_str(&format!(" ({elenco})"));
            }
            if messaggio.contains("alt text") {
                testo.push_str(
                    ". Suggerimento: aggiungi l'argomento `alt:` a immagini, figure ed \
                     equazioni (es. `#image(\"a.png\", alt: \"descrizione\")`).",
                );
            }
            testo
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    fn incluse(testo: &str, totale: usize) -> Vec<usize> {
        let r = analizza_pagine(testo).expect("intervallo valido");
        pagine_selezionate(totale, r.as_ref())
            .into_iter()
            .map(|i| i + 1)
            .collect()
    }

    #[test]
    fn intervalli_forme_supportate() {
        assert_eq!(incluse("2", 5), vec![2]);
        assert_eq!(incluse("2-4", 5), vec![2, 3, 4]);
        assert_eq!(incluse("4-", 6), vec![4, 5, 6]);
        assert_eq!(incluse("-2", 6), vec![1, 2]);
        assert_eq!(incluse("1-2, 5 , 7-", 8), vec![1, 2, 5, 7, 8]);
        // Sovrapposizioni e fuori range: nessun duplicato, nessun panic.
        assert_eq!(incluse("1-3,2-4,9", 4), vec![1, 2, 3, 4]);
        assert_eq!(incluse("9-12", 4), Vec::<usize>::new());
    }

    #[test]
    fn intervallo_vuoto_significa_tutte_le_pagine() {
        assert!(analizza_pagine("").expect("ok").is_none());
        assert!(analizza_pagine("   ").expect("ok").is_none());
        assert_eq!(pagine_selezionate(3, None), vec![0, 1, 2]);
    }

    #[test]
    fn intervalli_non_validi_sono_errori_leggibili() {
        for cattivo in [
            "0",
            "0-3",
            "3-1",
            "-",
            "1-2-3",
            "a",
            "1,,3",
            "1,",
            ",2",
            "1.5",
            "-0",
            "2--3",
            "99999999999999999999999",
        ] {
            let e = analizza_pagine(cattivo).expect_err(cattivo);
            assert!(
                e.contains("intervallo di pagine non valido"),
                "{cattivo}: {e}"
            );
        }
    }

    #[test]
    fn percorsi_una_pagina_usa_il_nome_scelto() {
        let d = Path::new("C:/x/mio.png");
        assert_eq!(percorsi_pagine(d, &[4], 20), vec![d.to_path_buf()]);
    }

    #[test]
    fn percorsi_piu_pagine_numerati_con_zeri() {
        let d = Path::new("cartella/doc.svg");
        let p = percorsi_pagine(d, &[2, 10], 12);
        assert_eq!(
            p,
            vec![
                Path::new("cartella").join("doc-02.svg"),
                Path::new("cartella").join("doc-10.svg")
            ]
        );
    }

    #[test]
    fn percorsi_nome_con_graffe_non_e_un_segnaposto() {
        let p = percorsi_pagine(Path::new("a{p}.png"), &[1, 2], 2);
        assert_eq!(p[0], Path::new("a{p}-1.png"));
        assert_eq!(p[1], Path::new("a{p}-2.png"));
        let t = percorsi_pagine(Path::new("x{t}{0p}.svg"), &[3, 4], 10);
        assert_eq!(t[0], Path::new("x{t}{0p}-03.svg"));
    }

    #[test]
    fn timestamp_solo_con_standard() {
        assert!(OpzioniPdf::default()
            .a_opzioni_typst()
            .expect("ok")
            .timestamp
            .is_none());
        let con = OpzioniPdf {
            standard: vec![StandardPdf::A2b],
            pagine: None,
        };
        assert!(con.a_opzioni_typst().expect("ok").timestamp.is_some());
    }

    #[test]
    fn percorsi_esistenti_esclude_il_file_scelto() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scelto = dir.path().join("doc.png");
        let percorsi = percorsi_pagine(&scelto, &[1, 2, 3], 3);
        // Nessun file: nessun conflitto.
        assert!(percorsi_esistenti(&percorsi, &scelto).is_empty());
        // Esiste il secondo e il "scelto" stesso (confermato dal dialog).
        std::fs::write(&percorsi[1], "x").expect("scrittura");
        std::fs::write(&scelto, "x").expect("scrittura");
        assert_eq!(
            percorsi_esistenti(&percorsi, &scelto),
            vec![percorsi[1].clone()]
        );
        // Con una sola pagina il percorso coincide col file scelto: mai conflitto.
        assert!(percorsi_esistenti(std::slice::from_ref(&scelto), &scelto).is_empty());
    }

    #[test]
    fn ppi_validazione() {
        assert!(valida_ppi(144.0).is_ok());
        for cattivo in [0.0, -5.0, f32::NAN, f32::INFINITY, PPI_MASSIMO + 1.0] {
            assert!(valida_ppi(cattivo).is_err(), "{cattivo}");
        }
    }

    #[test]
    fn pixel_pagina_a4() {
        // A4 = 595.28 × 841.89 pt; a 144 PPI (2 px/pt) → 1191 × 1684.
        assert_eq!(pixel_pagina(595.28, 841.89, 144.0), 1191 * 1684);
        // A4: il massimo per pagina è ben sotto il tetto assoluto di 1200 PPI.
        let max = ppi_massimo_pagina(595.28, 841.89);
        assert!(max < 1200 && max > 800, "{max}");
        assert!(pixel_pagina(595.28, 841.89, max as f32) <= PIXEL_MASSIMI);
        assert!(pixel_pagina(595.28, 841.89, (max + 1) as f32) > PIXEL_MASSIMI);
        // Pagina piccola: vale il tetto assoluto.
        assert_eq!(ppi_massimo_pagina(100.0, 100.0), 1200);
        // Pagina minuscola: almeno 1 px per lato, come typst-render.
        assert_eq!(pixel_pagina(0.0, 0.0, 72.0), 1);
    }

    #[test]
    fn standard_pdf_mappatura_completa_e_nomi() {
        let tutti = [
            StandardPdf::V1_4,
            StandardPdf::V1_5,
            StandardPdf::V1_6,
            StandardPdf::V1_7,
            StandardPdf::V2_0,
            StandardPdf::A1b,
            StandardPdf::A1a,
            StandardPdf::A2b,
            StandardPdf::A2u,
            StandardPdf::A2a,
            StandardPdf::A3b,
            StandardPdf::A3u,
            StandardPdf::A3a,
            StandardPdf::A4,
            StandardPdf::A4f,
            StandardPdf::A4e,
            StandardPdf::Ua1,
        ];
        for s in tutti {
            let _: typst_pdf::PdfStandard = s.into();
            assert!(!s.nome().is_empty());
        }
        let tag: Vec<_> = tutti
            .iter()
            .filter(|s| s.richiede_tag())
            .map(|s| s.nome())
            .collect();
        assert_eq!(tag, ["PDF/A-1a", "PDF/A-2a", "PDF/A-3a", "PDF/UA-1"]);
    }

    #[test]
    fn opzioni_da_json_del_frontend() {
        let o: OpzioniPdf =
            serde_json::from_str(r#"{"standard":["a-2b"],"pagine":"1-2"}"#).expect("json");
        assert_eq!(o.standard, vec![StandardPdf::A2b]);
        assert_eq!(o.pagine.as_deref(), Some("1-2"));
        let vuote: OpzioniPdf = serde_json::from_str("{}").expect("json");
        assert!(vuote.standard.is_empty() && vuote.pagine.is_none());
        assert!(serde_json::from_str::<OpzioniPdf>(r#"{"standard":["a-9z"]}"#).is_err());
    }

    #[test]
    fn opzioni_predefinite_equivalgono_a_pdf_options_default() {
        let o = OpzioniPdf::default().a_opzioni_typst().expect("ok");
        assert!(o.tagged, "senza intervallo il PDF resta taggato");
        assert!(o.page_ranges.is_none());
    }

    #[test]
    fn tagged_segue_la_presenza_dell_intervallo() {
        let o = OpzioniPdf {
            standard: vec![StandardPdf::A2b],
            pagine: Some("1-2".into()),
        }
        .a_opzioni_typst()
        .expect("A-2b è compatibile con gli intervalli");
        assert!(!o.tagged);
        assert!(o.page_ranges.is_some());
    }

    #[test]
    fn standard_con_tag_rifiutati_con_intervallo() {
        for s in [
            StandardPdf::A1a,
            StandardPdf::A2a,
            StandardPdf::A3a,
            StandardPdf::Ua1,
        ] {
            let e = OpzioniPdf {
                standard: vec![s],
                pagine: Some("2".into()),
            }
            .a_opzioni_typst()
            .expect_err("deve essere rifiutato a monte");
            assert!(e.contains(s.nome()) && e.contains("tag"), "{e}");
            // Senza intervallo lo stesso standard è ammesso.
            assert!(OpzioniPdf {
                standard: vec![s],
                pagine: None
            }
            .a_opzioni_typst()
            .is_ok());
        }
    }

    #[test]
    fn due_pdfa_insieme_errore_leggibile() {
        let e = OpzioniPdf {
            standard: vec![StandardPdf::A2b, StandardPdf::A3b],
            pagine: None,
        }
        .a_opzioni_typst()
        .expect_err("due PDF/A non sono ammessi");
        assert!(
            e.contains("standard PDF non applicabile") && e.contains("at most one"),
            "{e}"
        );
    }

    #[test]
    fn pdfa_con_versione_incompatibile_errore_leggibile() {
        // PDF/A-4 richiede PDF 2.0: la versione 1.4 è incompatibile.
        let e = OpzioniPdf {
            standard: vec![StandardPdf::A4, StandardPdf::V1_4],
            pagine: None,
        }
        .a_opzioni_typst()
        .expect_err("PDF/A-4 con PDF 1.4 è incompatibile");
        assert!(e.contains("standard PDF non applicabile"), "{e}");
        // La stessa famiglia con versione compatibile è ammessa.
        assert!(OpzioniPdf {
            standard: vec![StandardPdf::A4, StandardPdf::V2_0],
            pagine: None
        }
        .a_opzioni_typst()
        .is_ok());
    }

    #[test]
    fn intervallo_malformato_blocca_prima_di_tutto() {
        let e = OpzioniPdf {
            standard: vec![],
            pagine: Some("x".into()),
        }
        .a_opzioni_typst()
        .expect_err("intervallo non valido");
        assert!(e.contains("intervallo di pagine non valido"), "{e}");
    }

    #[test]
    fn destinazione_export_rifiuta_documento_e_sorgenti() {
        let doc = Path::new("C:/Lavoro/tesi.typ");
        let msg = verifica_destinazione_export(doc, Some(doc)).unwrap_err();
        assert!(msg.contains("documento aperto"), "{msg}");
        let msg = verifica_destinazione_export(Path::new("/x/a.typ"), None).unwrap_err();
        assert!(msg.contains("sorgente"), "{msg}");
        for nome in ["a.typ", "b.TYP", "c.typc", "d.md", "e.MD"] {
            assert!(
                verifica_destinazione_export(Path::new(nome), None).is_err(),
                "{nome}"
            );
        }
        for nome in ["a.pdf", "b.png", "c.svg", "senza_estensione", "d.typ.pdf"] {
            assert!(
                verifica_destinazione_export(Path::new(nome), Some(doc)).is_ok(),
                "{nome}"
            );
        }
    }

    #[test]
    fn nomi_insidiosi_di_windows() {
        for n in [
            "capitolo.typ.",
            "capitolo.typ ",
            "capitolo.typ::$DATA",
            "a.pdf:flusso",
            "x.",
        ] {
            assert!(nome_insidioso(n), "{n:?}");
        }
        for n in ["capitolo.pdf", "d.typ.pdf", "senza_estensione", ".nascosto"] {
            assert!(!nome_insidioso(n), "{n:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn destinazione_con_nome_insidioso_non_aggira_il_filtro_sul_sorgente() {
        let d = tempfile::tempdir().unwrap();
        let sorgente = d.path().join("capitolo.typ");
        std::fs::write(&sorgente, "= Titolo").unwrap();
        for nome in ["capitolo.typ.", "capitolo.typ ", "capitolo.typ::$DATA"] {
            let dest = d.path().join(nome);
            assert!(
                verifica_destinazione_export(&dest, None).is_err(),
                "{nome:?} doveva essere rifiutato"
            );
        }
        assert_eq!(std::fs::read_to_string(&sorgente).unwrap(), "= Titolo");
    }

    #[test]
    fn destinazione_che_si_risolve_su_un_sorgente_e_rifiutata() {
        // Il nome scelto è innocuo (`.pdf`) ma il percorso canonico no: collegamento
        // simbolico `out.pdf` → `capitolo.typ` (se i privilegi lo permettono).
        let d = tempfile::tempdir().unwrap();
        let sorgente = d.path().join("capitolo.typ");
        std::fs::write(&sorgente, "= Titolo").unwrap();
        let link = d.path().join("out.pdf");
        #[cfg(unix)]
        let creato = std::os::unix::fs::symlink(&sorgente, &link).is_ok();
        #[cfg(windows)]
        let creato = std::os::windows::fs::symlink_file(&sorgente, &link).is_ok();
        #[cfg(not(any(unix, windows)))]
        let creato = false;
        if creato {
            let msg = verifica_destinazione_export(&link, None).unwrap_err();
            assert!(msg.contains("capitolo.typ"), "{msg}");
        } else {
            eprintln!("*** SALTATO: collegamento simbolico non creabile ***");
        }
    }

    #[test]
    fn errori_pdf_ripetuti_raggruppati_con_conteggio_posizioni_e_suggerimento() {
        let alt = "PDF/UA-1 missing alt text";
        let mut errori: Vec<ErrorePdf> = (1..=30)
            .map(|i| ErrorePdf {
                messaggio: alt.to_string(),
                posizione: Some((i, 3)),
            })
            .collect();
        errori.push(ErrorePdf {
            messaggio: "altro problema".into(),
            posizione: None,
        });
        let out = raggruppa_errori_pdf(errori);
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].contains("30 occorrenze"), "{}", out[0]);
        assert!(out[0].contains("riga 1, colonna 3"), "{}", out[0]);
        assert!(out[0].contains("riga 5, colonna 3"), "{}", out[0]);
        assert!(!out[0].contains("riga 6,"), "solo le prime: {}", out[0]);
        assert!(out[0].contains("alt:"), "{}", out[0]);
        assert_eq!(out[1], "altro problema");
    }

    #[test]
    fn errore_pdf_singolo_resta_leggibile() {
        let out = raggruppa_errori_pdf(vec![ErrorePdf {
            messaggio: "errore x".into(),
            posizione: Some((4, 2)),
        }]);
        assert_eq!(out, vec!["errore x (riga 4, colonna 2)".to_string()]);
        assert!(raggruppa_errori_pdf(Vec::new()).is_empty());
    }
}
