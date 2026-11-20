//! `World`/`IdeWorld` dell'editor: compilazione, render, export ed IDE.
//!
//! `EditorWorld` è l'unico tipo che implementa [`typst::World`] e
//! [`typst_ide::IdeWorld`]. Il file principale (`main.typ`) è **virtuale** e
//! vive in memoria (`principale`), aggiornato in-place da `aggiorna_sorgente`
//! per abilitare il parsing incrementale di `Source::replace`. In modalità
//! progetto (dopo `apri_progetto`) i file fratelli si leggono dal disco sotto
//! una radice canonicalizzata, con barriera anti path-traversal (vedi
//! [`RadiceProgetto`]); i pacchetti `@preview`/`@local` passano dallo stesso
//! `SystemPackages` di `typst-kit` usato dal compilatore CLI ufficiale; se un
//! `@local` non è installato, ripiega sul pacchetto incorporato
//! ([`crate::pacchetti_incorporati`]).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::Serialize;
use typst::diag::{FileError, FileResult, Severity, SourceDiagnostic, Tracepoint};
use typst::foundations::{Bytes, Datetime, Duration, NativeElement, StyleChain};
use typst::introspection::{Introspector, PagedPosition};
use typst::layout::{Abs, Point};
use typst::model::HeadingElem;
use typst::syntax::package::PackageSpec;
use typst::syntax::{DiagSpan, FileId, RootedPath, Side, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_ide::{CompletionKind, IdeWorld, Jump};
use typst_kit::datetime::Time;
use typst_kit::files::{FileLoader, FileStore, FsRoot};
use typst_kit::fonts::{self, FontSource, FontStore};
use typst_kit::packages::SystemPackages;
use typst_layout::{Page, PagedDocument};

use crate::esporta::{self, OpzioniPdf};
use crate::indice::{self, VoceIndice};
use crate::pacchetti_incorporati;
use crate::sorveglianza::RisorseLette;

/// Esito di una compilazione: metadati per pagina (**senza SVG**, F5-008) più
/// i diagnostici raccolti.
///
/// L'SVG di una pagina si chiede a richiesta con [`EditorWorld::svg_pagina`]:
/// su documenti lunghi si serializza solo ciò che è in vista (DECISIONI.md
/// D-016, fase8.md §4.1). Sempre restituito con successo dal comando `compila`: gli errori fatali di
/// compilazione **non** sono un `Err` a livello di applicazione, ma
/// diagnostici con `gravita: "errore"` in questa struttura (vedi
/// DECISIONI.md F1-007).
#[derive(Debug, Clone, Serialize)]
pub struct RisultatoCompilazione {
    /// Un elemento per pagina, in ordine. Vuoto se la compilazione è fallita
    /// (l'ultima anteprima valida resta interrogabile via `svg_pagina`).
    pub pagine: Vec<MetadatiPagina>,
    /// Errori e avvisi del compilatore, con posizione se ricadono nel main.
    pub diagnostici: Vec<Diagnostico>,
}

/// Metadati di una pagina compilata: bastano al frontend per creare un
/// segnaposto delle dimensioni corrette e per sapere se l'SVG già mostrato è
/// ancora valido.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetadatiPagina {
    /// Larghezza in punti tipografici (1 unità del `viewBox` SVG = 1 pt).
    pub larghezza: f64,
    /// Altezza in punti tipografici.
    pub altezza: f64,
    /// `hash128` del contenuto della pagina (`typst::utils::hash128`), in
    /// esadecimale a 32 cifre: un `u128` non è rappresentabile in un numero JS.
    /// Se due compilazioni danno hash diversi la pagina è cambiata; a hash
    /// uguali l'SVG già mostrato resta valido.
    pub hash: String,
}

/// L'SVG di una pagina insieme all'hash del contenuto da cui deriva.
///
/// L'hash è quello della pagina **effettivamente serializzata** (l'ultimo
/// documento valido al momento della richiesta), non quello atteso dal
/// chiamante: così un frontend in ritardo di una compilazione non attribuisce
/// all'SVG un hash sbagliato.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaginaSvg {
    /// Stesso formato di [`MetadatiPagina::hash`].
    pub hash: String,
    /// Documento SVG completo della pagina.
    pub svg: String,
}

/// Copia (economica: i frame sono condivisi) di una pagina dell'ultimo
/// documento valido, pronta per essere serializzata **fuori dal lock** del
/// World (F5-008): la serializzazione SVG di una pagina non deve bloccare le
/// compilazioni successive.
#[derive(Debug, Clone)]
pub struct PaginaDaRenderizzare(Page);

impl PaginaDaRenderizzare {
    /// Serializza la pagina in SVG e ne calcola l'hash (della copia, quindi
    /// coerente con l'SVG prodotto).
    pub fn svg(&self) -> PaginaSvg {
        PaginaSvg {
            hash: hash_pagina(&self.0),
            svg: typst_svg::svg(&self.0, &typst_svg::SvgOptions::default()),
        }
    }
}

/// Pagine scelte per un export PNG/SVG (F5-011), pronte per il rendering a
/// lock rilasciato. Ogni pagina è una copia economica di quella del documento.
#[derive(Debug, Clone)]
pub struct PagineDaEsportare {
    totale: usize,
    /// (numero di pagina base 1, pagina).
    pagine: Vec<(usize, PaginaDaRenderizzare)>,
}

impl PagineDaEsportare {
    /// Pagine totali del documento (serve alla numerazione dei file).
    pub fn totale(&self) -> usize {
        self.totale
    }

    /// Numeri (base 1) delle pagine selezionate, in ordine.
    pub fn numeri(&self) -> Vec<usize> {
        self.pagine.iter().map(|(n, _)| *n).collect()
    }

    /// Rende ogni pagina in PNG alla risoluzione `ppi` (pixel per pollice; 72
    /// PPI = 1 pixel per punto tipografico). Validazione di `ppi` e del numero
    /// di pixel di ogni pagina **prima** di allocare le immagini.
    pub fn png(&self, ppi: f32) -> Result<Vec<Vec<u8>>, String> {
        let ppi = esporta::valida_ppi(ppi)?;
        for (numero, pagina) in &self.pagine {
            let dim = pagina.0.frame.size();
            let pixel = esporta::pixel_pagina(dim.x.to_pt() as f32, dim.y.to_pt() as f32, ppi);
            if pixel > esporta::PIXEL_MASSIMI {
                return Err(format!(
                    "la pagina {numero} a {ppi} PPI avrebbe {pixel} pixel (massimo {}): \
                     per questa pagina la risoluzione massima è {} PPI",
                    esporta::PIXEL_MASSIMI,
                    esporta::ppi_massimo_pagina(dim.x.to_pt() as f32, dim.y.to_pt() as f32)
                ));
            }
        }
        // Derivato da typst-cli crates/typst-cli/src/compile.rs (3ae52774b48987fc78a72ff483068cacc28e46c2), licenza Apache-2.0 — Copyright Typst contributors
        // `png_options`: pixel_per_pt = ppi / 72, senza bleed.
        let opzioni = typst_render::RenderOptions {
            pixel_per_pt: typst::utils::Scalar::new(f64::from(ppi) / 72.0),
            render_bleed: false,
        };
        self.pagine
            .iter()
            .map(|(numero, pagina)| {
                typst_render::render(&pagina.0, &opzioni)
                    .encode_png()
                    .map_err(|e| format!("codifica PNG della pagina {numero} non riuscita: {e}"))
            })
            .collect()
    }

    /// Serializza ogni pagina in SVG (un file per pagina).
    pub fn svg(&self) -> Vec<String> {
        self.pagine
            .iter()
            .map(|(_, p)| typst_svg::svg(&p.0, &typst_svg::SvgOptions::default()))
            .collect()
    }
}

/// Messaggio per un intervallo che non seleziona alcuna pagina.
fn nessuna_pagina(totale: usize) -> String {
    format!("nessuna pagina nell'intervallo indicato (il documento ha {totale} pagine)")
}

/// Un errore o avviso del compilatore.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostico {
    /// `"errore"` oppure `"avviso"`.
    pub gravita: String,
    /// Messaggio (con eventuali suggerimenti accodati).
    pub messaggio: String,
    /// Offset UTF-16 di inizio nel main, `None` se non posizionabile lì.
    pub da: Option<usize>,
    /// Offset UTF-16 di fine nel main, `None` se non posizionabile lì.
    pub a: Option<usize>,
    /// `true` quando `da`/`a` NON vengono dallo span reale dell'errore ma dal
    /// punto di chiamata nel main preso dalla traccia del compilatore
    /// (errore nato in un altro file: pacchetto, fratello, dato).
    pub posizione_da_traccia: bool,
    /// Dove l'errore è nato davvero quando non è nel main: per file del
    /// progetto `nome/relativo.ext, riga R, colonna C`; per pacchetti
    /// `@ns/nome:ver — percorso, riga R, colonna C`. `None` se l'errore è nel
    /// main o lo span non punta ad alcun file (es. errori di plugin).
    pub origine: Option<String>,
    /// Descrizioni in italiano dei tracepoint, dal più interno al più esterno
    /// (ordine del compilatore), con la riga del main quando disponibile.
    pub traccia: Vec<String>,
    /// Gli hint del compilatore, separati (`messaggio` li ha ancora accodati).
    pub suggerimenti: Vec<String>,
}

/// Esito dell'autocompletamento: offset da cui si applica + voci proposte.
#[derive(Debug, Clone, Serialize)]
pub struct RisultatoCompletamento {
    /// Offset UTF-16 da cui inizia il token completato.
    pub da: usize,
    pub voci: Vec<Completamento>,
}

/// Una singola voce di completamento, pronta per CodeMirror.
#[derive(Debug, Clone, Serialize)]
pub struct Completamento {
    pub etichetta: String,
    /// Tipo CodeMirror (`"function"`, `"constant"`, `"type"`, `"keyword"`, …).
    pub tipo: String,
    /// Testo da inserire, se diverso dall'etichetta (altrimenti `None`).
    pub applica: Option<String>,
    pub dettaglio: Option<String>,
}

/// Una posizione nell'anteprima renderizzata (pagina 0-based + punti tipografici).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Posizione {
    pub pagina: usize,
    pub x: f64,
    pub y: f64,
}

/// Radice di un progetto multi-file agganciata su disco.
///
/// Canonicalizzata **una sola volta** all'apertura del progetto
/// (`EditorWorld::apri_progetto`): ogni risoluzione successiva di un file
/// fratello confronta il percorso realizzato con questa radice canonica, così
/// un symlink che punta fuori dalla cartella di progetto viene rifiutato.
/// `VirtualPath::realize` da solo rifiuta già `..` e i drive-letter, ma non
/// può accorgersi di un link simbolico che punta altrove: la barriera qui è
/// quel controllo aggiuntivo (DECISIONI.md F3-002/F4-001).
#[derive(Debug, Clone)]
struct RadiceProgetto {
    /// Percorso canonico della radice del progetto.
    canonica: PathBuf,
    /// Handle typst-kit per la risoluzione lessicale dei percorsi virtuali.
    fs: FsRoot,
}

/// Percorso relativo alla radice (senza `/` iniziale) per i messaggi d'errore.
fn percorso_per_messaggio(vpath: &VirtualPath) -> PathBuf {
    PathBuf::from(vpath.get_with_slash().trim_start_matches('/'))
}

impl RadiceProgetto {
    fn nuova(canonica: PathBuf) -> Self {
        let fs = FsRoot::new(canonica.clone());
        Self { canonica, fs }
    }

    /// Risolve un percorso virtuale in un percorso reale, canonicalizzandolo
    /// e verificando che resti sotto la radice canonica.
    fn risolvi(&self, vpath: &VirtualPath) -> FileResult<PathBuf> {
        let candidato = self.fs.resolve(vpath)?;
        // Nei messaggi d'errore solo il percorso relativo alla radice: il
        // percorso assoluto (`\?\C:\...`) è rumore e rivela la struttura locale.
        let reale = std::fs::canonicalize(&candidato)
            .map_err(|e| FileError::from_io(e, &percorso_per_messaggio(vpath)))?;
        if !reale.starts_with(&self.canonica) {
            // Il percorso realizzato esce dalla radice (tipicamente tramite
            // un symlink): rifiutato come se l'accesso fosse negato.
            return Err(FileError::AccessDenied);
        }
        Ok(reale)
    }

    fn carica(&self, vpath: &VirtualPath) -> FileResult<Bytes> {
        let reale = self.risolvi(vpath)?;
        let mappa_errore =
            |e: std::io::Error| FileError::from_io(e, &percorso_per_messaggio(vpath));
        if std::fs::metadata(&reale).map_err(mappa_errore)?.is_dir() {
            return Err(FileError::IsDirectory);
        }
        std::fs::read(&reale).map(Bytes::new).map_err(mappa_errore)
    }
}

/// Carica i file fratelli (progetto) e i pacchetti `@preview`/`@local` per il
/// [`FileStore`] di [`EditorWorld`]. Il file principale non passa da qui:
/// resta in memoria (vedi `EditorWorld::principale`).
struct CaricatoreFile {
    /// Radice del progetto su disco. `None` in modalità documento singolo:
    /// nessun file fratello è risolvibile (comportamento invariato rispetto
    /// alle fasi precedenti l'introduzione della modalità progetto).
    radice_progetto: Option<RadiceProgetto>,
    /// Cache/downloader standard per i pacchetti `@preview` e `@local`.
    pacchetti: SystemPackages,
    /// File del progetto richiesti dalla compilazione (letture riuscite **e**
    /// fallite), condiviso col watcher: vedi [`RisorseLette`].
    letti: RisorseLette,
}

impl FileLoader for CaricatoreFile {
    fn load(&self, id: FileId) -> FileResult<Bytes> {
        match id.root() {
            VirtualRoot::Project => match &self.radice_progetto {
                Some(radice) => {
                    // Registrato PRIMA della lettura e anche se fallisce: la
                    // successiva comparsa del file (es. una bibliografia
                    // creata dopo l'errore) deve invalidare la cache.
                    if let Ok(candidato) = radice.fs.resolve(id.vpath()) {
                        self.letti.registra(&candidato);
                    }
                    radice.carica(id.vpath())
                }
                None => Err(FileError::NotFound(PathBuf::from(
                    id.vpath().get_with_slash(),
                ))),
            },
            VirtualRoot::Package(spec) => self.carica_da_pacchetto(spec, id.vpath()),
        }
    }
}

impl CaricatoreFile {
    /// Legge un file di un pacchetto. **Priorità all'utente**: si prova prima la
    /// risoluzione standard (`@local` installato, cache, download per
    /// `@preview`); solo se FALLISCE e il pacchetto è uno di quelli incorporati
    /// nell'eseguibile (`pacchetti_incorporati`) si serve la copia incorporata.
    /// Altrimenti si propaga l'errore originale, invariato (compresi i messaggi
    /// per `@preview`). Per `@local` inesistente `obtain` fallisce subito con
    /// `NotFound`, senza rete: il download parte solo per il namespace `preview`.
    fn carica_da_pacchetto(&self, spec: &PackageSpec, vpath: &VirtualPath) -> FileResult<Bytes> {
        match self.pacchetti.obtain(spec) {
            Ok(radice) => radice.load(vpath),
            Err(errore) => match pacchetti_incorporati::file(spec, vpath) {
                Some(esito) => esito,
                None => Err(errore.into()),
            },
        }
    }
}

/// Il mondo di compilazione persistente dell'editor.
///
/// Costruito una sola volta (font embedded + di sistema caricati al primo uso,
/// di norma anticipato da un thread di sfondo, vedi [`FontPrecaricabili`]); le compilazioni successive aggiornano solo il sorgente
/// (`aggiorna_sorgente`) per beneficiare della cache incrementale di
/// `comemo`. Implementa `typst::World` e `typst_ide::IdeWorld`.
pub struct EditorWorld {
    libreria: LazyHash<Library>,
    /// Font (embedded + sistema + progetto), costruiti **al primo uso** e non
    /// nel costruttore (F5-007): vedi [`FontPrecaricabili`].
    font: Arc<FontPrecaricabili>,
    /// Testo e albero sintattico del file **in editing**, tenuti in memoria
    /// (overlay: `source(id_principale)` serve sempre questo buffer). Di norma
    /// è anche il main della compilazione; aprendo un altro `.typ` del progetto
    /// (F5-020/F5-021) cambia solo questo campo e `id_main` resta il file del
    /// progetto, così la compilazione resta quella dell'intero progetto.
    principale: Source,
    /// Id del file in editing (quello di `principale`).
    id_principale: FileId,
    /// Id del file principale della compilazione (`World::main`). Se diverso
    /// da `id_principale` viene letto dal disco come un file fratello, con le
    /// stesse regole (e la stessa invalidazione via `invalida_risorse`) di ogni
    /// altro fratello: nessuna copia in memoria del main (una cache stantia non
    /// seguirebbe le modifiche su disco).
    id_main: FileId,
    /// Main `.typ` del progetto messo da parte quando si apre un file non Typst
    /// (`.md`) che lo sostituisce come `id_main`; `Some` solo mentre `id_main`
    /// non è un `.typ`. Tornando a un `.typ` del progetto viene ripristinato,
    /// altrimenti la sequenza main.typ → nota.md → cap1.typ perderebbe il main.
    id_main_typ_progetto: Option<FileId>,
    /// File fratelli (modalità progetto) e pacchetti.
    magazzino: FileStore<CaricatoreFile>,
    /// File del progetto richiesti dalla compilazione, condivisi col watcher
    /// (`Stato::letti`): l'handle resta lo stesso per tutta la vita del `World`,
    /// si svuota solo al cambio di progetto.
    letti: RisorseLette,
    /// Data "odierna" per `datetime.today()`, congelata al primo accesso
    /// successivo alla costruzione di questo `World` (vedi DECISIONI.md,
    /// "Limiti noti" F1-006: sessioni lunghe a cavallo di mezzanotte
    /// mostrano la data d'avvio).
    tempo: Time,
    /// Ultimo documento compilato con **successo**, usato dai comandi di
    /// sincronizzazione sorgente↔anteprima (`jump_da_click`/`jump_da_cursore`).
    /// Su un errore di compilazione resta l'ultimo valido (non viene svuotato).
    ultimo_documento: Option<PagedDocument>,
    /// Indice dei titoli dell'ultimo documento compilato con **successo**
    /// (F5-020): si costruisce insieme a `ultimo_documento` e, come lui, su un
    /// errore di compilazione resta l'ultimo valido (mai un indice misto fra un
    /// documento vecchio e un sorgente nuovo, perché gli offset sono calcolati
    /// sui sorgenti del momento della compilazione).
    ultimo_indice: Vec<VoceIndice>,
}

impl EditorWorld {
    /// Crea un nuovo `EditorWorld` in modalità documento singolo: **non**
    /// scansiona i font (lo fa [`FontPrecaricabili`] al primo uso) e imposta `sorgente` come
    /// testo del file principale virtuale `main.typ`.
    pub fn nuovo(sorgente: &str) -> Self {
        let id_principale = id_main_virtuale();
        let letti = RisorseLette::nuovo();
        Self {
            libreria: LazyHash::new(Library::default()),
            font: Arc::new(FontPrecaricabili::nuovo(None)),
            principale: Source::new(id_principale, sorgente.to_string()),
            id_principale,
            id_main: id_principale,
            id_main_typ_progetto: None,
            magazzino: FileStore::new(CaricatoreFile {
                radice_progetto: None,
                pacchetti: crate::crea_system_packages(),
                letti: letti.clone(),
            }),
            letti,
            tempo: Time::system(),
            ultimo_documento: None,
            ultimo_indice: Vec::new(),
        }
    }

    /// Handle condivisibile per precaricare i font fuori dal lock del World
    /// (F5-007): il guscio lo clona subito dopo `nuovo` e lo passa a un thread
    /// di sfondo. Dopo `apri_progetto`/`chiudi_progetto` lo store è nuovo e
    /// l'handle precedente non lo riguarda più.
    pub fn font_precaricabili(&self) -> Arc<FontPrecaricabili> {
        Arc::clone(&self.font)
    }

    /// Handle condiviso dei file del progetto richiesti dalla compilazione: il
    /// guscio lo passa al watcher (`Stato::letti`) perché invalidi la cache solo
    /// per file che Typst ha davvero usato.
    pub fn risorse_lette(&self) -> RisorseLette {
        self.letti.clone()
    }

    /// Aggiorna il testo del file principale in memoria.
    ///
    /// Usa `Source::replace` (diff prefisso/suffisso + reparsing incrementale)
    /// invece di ricreare il `Source`, così `comemo` può riusare gran parte
    /// del lavoro di valutazione fra una battitura e l'altra.
    pub fn aggiorna_sorgente(&mut self, sorgente: &str) {
        self.principale.replace(sorgente);
    }

    /// Aggancia il `World` a una cartella di progetto su disco: la radice
    /// viene canonicalizzata e l'entrypoint relativo diventa il nuovo file
    /// principale (in memoria, col testo passato in `contenuto`). Ricostruisce
    /// anche lo store dei font, includendo `radice/fonts` se presente, e lo
    /// store dei file fratelli/pacchetti (nessuna cache stantia attraversa
    /// progetti diversi).
    pub fn apri_progetto(
        &mut self,
        radice: PathBuf,
        entrypoint_relativo: &str,
        contenuto: &str,
    ) -> Result<(), String> {
        let canonica = std::fs::canonicalize(&radice).map_err(|e| {
            format!(
                "Cartella di progetto non valida '{}': {e}",
                radice.display()
            )
        })?;
        let vpath = VirtualPath::new(entrypoint_relativo)
            .map_err(|e| format!("Percorso entrypoint non valido '{entrypoint_relativo}': {e}"))?;
        let id_principale = RootedPath::new(VirtualRoot::Project, vpath).intern();

        self.font = Arc::new(FontPrecaricabili::nuovo(Some(canonica.clone())));
        self.letti.svuota();
        self.magazzino = FileStore::new(CaricatoreFile {
            radice_progetto: Some(RadiceProgetto::nuova(canonica)),
            pacchetti: crate::crea_system_packages(),
            letti: self.letti.clone(),
        });
        self.principale = Source::new(id_principale, contenuto.to_string());
        self.id_principale = id_principale;
        self.id_main = id_principale;
        self.id_main_typ_progetto = None;
        // Il documento compilato apparteneva (eventualmente) a un progetto
        // diverso: FileId e offset non sono più coerenti col nuovo main.
        self.ultimo_documento = None;
        self.ultimo_indice = Vec::new();
        Ok(())
    }

    /// Chiude la modalità progetto, riportando il `World` allo stato
    /// "documento singolo, nessun progetto": inverso di [`Self::apri_progetto`]
    /// per la parte radice/font (font store senza i font di progetto,
    /// nessuna radice agganciata per la risoluzione dei file fratelli).
    ///
    /// Il testo del file principale **non viene toccato**: resta quello
    /// corrente (il comando Tauri `chiudi_progetto` non riceve un nuovo
    /// contenuto, vedi DECISIONI.md gap F4-001), cambia solo il suo
    /// `FileId`, che torna a essere il virtuale `main.typ` non agganciato ad
    /// alcuna radice su disco (coerente con [`EditorWorld::nuovo`]).
    pub fn chiudi_progetto(&mut self) {
        let id_principale = id_main_virtuale();
        self.font = Arc::new(FontPrecaricabili::nuovo(None));
        self.letti.svuota();
        self.magazzino = FileStore::new(CaricatoreFile {
            radice_progetto: None,
            pacchetti: crate::crea_system_packages(),
            letti: self.letti.clone(),
        });
        // Mantiene il testo corrente: cambia solo l'id, non più legato
        // all'entrypoint su disco del progetto appena chiuso.
        self.principale = Source::new(id_principale, self.principale.text().to_string());
        self.id_principale = id_principale;
        self.id_main = id_principale;
        self.id_main_typ_progetto = None;
        // Il documento compilato apparteneva (eventualmente) al progetto
        // appena chiuso: FileId e offset non sono più coerenti col nuovo main.
        self.ultimo_documento = None;
        self.ultimo_indice = Vec::new();
    }

    /// Radice (canonica) del progetto agganciato, se in modalità progetto.
    ///
    /// Serve al guscio per sapere quale cartella osservare (F5-022): la radice
    /// può essere più in alto dell'entrypoint (es. `typst.toml` con entrypoint
    /// in una sottocartella).
    pub fn radice_progetto(&self) -> Option<&Path> {
        self.magazzino
            .loader()
            .radice_progetto
            .as_ref()
            .map(|r| r.canonica.as_path())
    }

    /// Apre come documento un file **della cartella-progetto già agganciata**
    /// (F5-020/F5-021: clic su un titolo dell'indice o su un file dell'albero).
    ///
    /// Il file diventa il nuovo file principale mantenendo la **stessa radice**
    /// (a differenza di `apri_file`, che riparte dalla cartella del file): gli
    /// `#import` relativi a cartelle superiori continuano a risolversi. Il
    /// percorso è quello relativo alla radice, con `/`; è risolto con la stessa
    /// barriera di ogni altro file fratello (`..`, link simbolici verso
    /// l'esterno e percorsi assoluti sono rifiutati).
    ///
    /// Restituisce il percorso assoluto (canonico) e il testo del file.
    ///
    /// # Errori
    /// `Err` con messaggio leggibile se non c'è un progetto agganciato, il
    /// percorso non è valido o esce dalla radice, il file non è `.typ`/`.md`,
    /// non esiste o non è UTF-8. In caso di errore il `World` non cambia.
    pub fn apri_file_del_progetto(&mut self, relativo: &str) -> Result<(PathBuf, String), String> {
        let radice = self
            .magazzino
            .loader()
            .radice_progetto
            .clone()
            .ok_or_else(|| {
                "Nessuna cartella-progetto aperta: apri prima una cartella.".to_string()
            })?;
        let vpath = VirtualPath::new(relativo)
            .map_err(|e| format!("Percorso non valido '{relativo}': {e}"))?;
        let reale = radice
            .risolvi(&vpath)
            .map_err(|e| format!("Il file '{relativo}' non è raggiungibile nel progetto: {e}"))?;
        let estensione_ok = reale
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("typ") || e.eq_ignore_ascii_case("md"));
        if !estensione_ok {
            return Err(format!(
                "Il file '{relativo}' non è un documento .typ o .md."
            ));
        }
        let contenuto = std::fs::read_to_string(&reale)
            .map_err(|e| format!("Impossibile aprire il file '{relativo}': {e}"))?;
        // Il FileId deriva dal percorso relativo **canonico su disco** (maiuscole
        // incluse): su Windows `cap1.typ` e `Cap1.typ` sono lo stesso file ma
        // FileId diversi, e l'overlay in editing non verrebbe visto dal main che
        // lo include con la grafia del disco.
        // LIMITE NOTO: se è il *main* a includerlo con una grafia diversa da
        // quella su disco (`#include "Cap1.typ"` per `cap1.typ`), i due id
        // restano distinti e la compilazione legge la copia da disco.
        let vpath = reale
            .strip_prefix(&radice.canonica)
            .ok()
            .and_then(|rel| {
                let parti: Option<Vec<&str>> =
                    rel.components().map(|c| c.as_os_str().to_str()).collect();
                VirtualPath::new(parti?.join("/")).ok()
            })
            .unwrap_or(vpath);
        let e_typ = |id: FileId| {
            Path::new(&id.vpath().get_with_slash().to_string())
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("typ"))
        };
        let nuovo_id = RootedPath::new(VirtualRoot::Project, vpath.clone()).intern();
        // Main `.typ` del progetto: quello corrente, oppure quello messo da parte
        // quando un `.md` ne ha preso il posto.
        let main_typ = if e_typ(self.id_main) {
            Some(self.id_main)
        } else {
            self.id_main_typ_progetto
        };
        if let (true, Some(main_typ)) = (e_typ(nuovo_id), main_typ) {
            // Overlay: cambia solo il file in editing. Main, font, radice, ultimo
            // documento e indice restano quelli del progetto; la cache dei
            // fratelli si svuota perché il file appena lasciato torni dal disco
            // (il main, se non è il file in editing, passa da lì come ogni fratello).
            // Se si arriva da un `.md` il main del progetto viene ripristinato.
            self.principale = Source::new(nuovo_id, contenuto.clone());
            self.id_principale = nuovo_id;
            self.id_main = main_typ;
            self.id_main_typ_progetto = None;
            self.magazzino.reset();
        } else {
            // Markdown (main = testo convertito) o main non Typst: il file
            // aperto diventa il main. La radice è la stessa del progetto già
            // agganciato: font store (scansione lenta, da non rifare sotto lock)
            // e cache dei pacchetti restano; cambiano solo i tre id e il testo.
            // Il main `.typ` corrente si conserva per ripristinarlo in seguito.
            if e_typ(self.id_main) {
                self.id_main_typ_progetto = Some(self.id_main);
            }
            self.principale = Source::new(nuovo_id, contenuto.clone());
            self.id_principale = nuovo_id;
            self.id_main = nuovo_id;
            self.magazzino.reset();
            // FileId e offset del documento precedente non sono più coerenti.
            self.ultimo_documento = None;
            self.ultimo_indice = Vec::new();
        }
        Ok((reale, contenuto))
    }

    /// Invalida la cache dei file fratelli letti dal disco (F5-014).
    ///
    /// Il [`FileStore`] memorizza i file fratelli alla prima lettura e, senza
    /// reset, non li rilegge mai: un file modificato da un altro programma
    /// resterebbe stantio fino alla riapertura del progetto. Dopo il reset la
    /// prossima compilazione rilegge dal disco (come `world.reset()` nel ciclo
    /// di `typst watch`); la memoizzazione di `comemo` resta valida perché
    /// traccia il contenuto letto, non il percorso. Il sorgente principale è
    /// in memoria e non è toccato.
    pub fn invalida_risorse(&mut self) {
        self.magazzino.reset();
    }

    /// Compila il documento corrente. Condivisa da `compila` ed `esporta_pdf`;
    /// non ha effetti collaterali sullo stato del `World`.
    fn compila_documento(&self) -> typst::diag::Warned<typst::diag::SourceResult<PagedDocument>> {
        typst::compile::<PagedDocument>(self)
    }

    /// Compila e restituisce i metadati per pagina (dimensioni + hash),
    /// raccogliendo diagnostici. **Non** serializza alcun SVG (F5-008): vedi
    /// [`Self::svg_pagina`].
    ///
    /// Su successo aggiorna `ultimo_documento` (per i comandi di jump e per
    /// `svg_pagina`); su errore lo lascia invariato, così l'ultima anteprima
    /// valida resta disponibile. Evita la cache `comemo` dopo la compilazione
    /// per limitarne la crescita nel tempo.
    pub fn compila(&mut self) -> RisultatoCompilazione {
        let esito = self.compila_documento();
        let mut diagnostici: Vec<Diagnostico> = esito
            .warnings
            .iter()
            .map(|d| self.diagnostico_da(d))
            .collect();

        let pagine = match esito.output {
            Ok(documento) => {
                let pagine = documento.pages().iter().map(metadati_pagina).collect();
                self.ultimo_indice = self.costruisci_indice(&documento);
                self.ultimo_documento = Some(documento);
                pagine
            }
            Err(errori) => {
                diagnostici.extend(errori.iter().map(|d| self.diagnostico_da(d)));
                Vec::new()
            }
        };

        // Limita la crescita della cache comemo dopo ogni ciclo compila+render.
        typst::comemo::evict(30);

        RisultatoCompilazione {
            pagine,
            diagnostici,
        }
    }

    /// Copia della pagina `indice` (0-based) dell'**ultimo documento compilato
    /// con successo**, o `None` se l'indice è fuori range o non c'è ancora un
    /// documento valido. Non ricompila. Il chiamante serializza con
    /// [`PaginaDaRenderizzare::svg`] dopo aver rilasciato il lock (F5-008).
    pub fn pagina_da_renderizzare(&self, indice: usize) -> Option<PaginaDaRenderizzare> {
        let pagina = self.ultimo_documento.as_ref()?.pages().get(indice)?;
        Some(PaginaDaRenderizzare(pagina.clone()))
    }

    /// Comodità per test e strumenti: `pagina_da_renderizzare(indice)` seguita
    /// subito da `svg()`.
    pub fn svg_pagina(&self, indice: usize) -> Option<PaginaSvg> {
        self.pagina_da_renderizzare(indice).map(|p| p.svg())
    }

    /// Compila ed esporta il documento corrente in PDF, senza standard e con
    /// tutte le pagine (equivale a [`OpzioniPdf::default`]).
    pub fn esporta_pdf(&mut self) -> Result<Vec<u8>, Vec<String>> {
        self.esporta_pdf_con(&OpzioniPdf::default())
    }

    /// Compila ed esporta il documento corrente in PDF con le opzioni date
    /// (standard PDF/A, intervallo di pagine; F5-011).
    ///
    /// Le opzioni si validano **prima** di compilare (intervallo malformato,
    /// standard incompatibili con i tag). Se il documento viola lo standard
    /// scelto, `typst-pdf` restituisce diagnostica di errore: qui diventa
    /// `Err` con i messaggi e nessun byte prodotto (nessun panic).
    pub fn esporta_pdf_con(&mut self, opzioni: &OpzioniPdf) -> Result<Vec<u8>, Vec<String>> {
        let opzioni_typst = opzioni.a_opzioni_typst().map_err(|e| vec![e])?;
        let esito = self.compila_documento();
        let risultato = match esito.output {
            Ok(documento) => {
                let totale = documento.pages().len();
                if opzioni_typst.page_ranges.is_some()
                    && esporta::pagine_selezionate(totale, opzioni_typst.page_ranges.as_ref())
                        .is_empty()
                {
                    Err(vec![nessuna_pagina(totale)])
                } else {
                    // Gli errori ripetuti (es. "missing alt text", uno per
                    // occorrenza) si raggruppano in un solo messaggio con
                    // conteggio, posizioni e suggerimento (punto 10, D-026).
                    let pdf = typst_pdf::pdf(&documento, &opzioni_typst).map_err(|errori| {
                        esporta::raggruppa_errori_pdf(
                            errori.iter().map(|d| self.errore_pdf(d)).collect(),
                        )
                    });
                    // L'export non tocca `ultimo_documento`/`ultimo_indice`: si
                    // aggiornano insieme, solo da `compila` (mai uno senza l'altro).
                    pdf
                }
            }
            Err(errori) => Err(errori.iter().map(|d| d.message.to_string()).collect()),
        };
        typst::comemo::evict(30);
        risultato
    }

    /// Compila il documento e prepara le pagine scelte per un export PNG/SVG
    /// (F5-011). `pagine` è un intervallo (`1-3, 5`), vuoto/`None` = tutte.
    ///
    /// Il lock del `World` serve solo fin qui: il rendering vero (lento) si fa
    /// dopo, su [`PagineDaEsportare`], a lock rilasciato. Su errore di
    /// compilazione o intervallo senza pagine restituisce i messaggi.
    pub fn pagine_da_esportare(
        &mut self,
        pagine: Option<&str>,
    ) -> Result<PagineDaEsportare, Vec<String>> {
        let intervalli = esporta::analizza_pagine(pagine.unwrap_or("")).map_err(|e| vec![e])?;
        let esito = self.compila_documento();
        let risultato = match esito.output {
            Ok(documento) => {
                let totale = documento.pages().len();
                let scelte = esporta::pagine_selezionate(totale, intervalli.as_ref());
                if scelte.is_empty() {
                    Err(vec![nessuna_pagina(totale)])
                } else {
                    let elenco = scelte
                        .into_iter()
                        .map(|i| (i + 1, PaginaDaRenderizzare(documento.pages()[i].clone())))
                        .collect();
                    // Come per il PDF: `ultimo_documento`/`ultimo_indice` restano
                    // quelli dell'ultima `compila`, coerenti fra loro.
                    Ok(PagineDaEsportare {
                        totale,
                        pagine: elenco,
                    })
                }
            }
            Err(errori) => Err(errori.iter().map(|d| d.message.to_string()).collect()),
        };
        typst::comemo::evict(30);
        risultato
    }

    /// Propone completamenti alla posizione del cursore (offset UTF-16).
    ///
    /// Usa l'ultimo documento compilato con successo, se presente, per
    /// arricchire i completamenti (es. label): non è un requisito, funziona
    /// comunque con `None` (funzioni, parametri, tipi, simboli, font).
    pub fn completa(&self, cursore: usize, esplicito: bool) -> Option<RisultatoCompletamento> {
        let byte = self.principale.lines().utf16_to_byte(cursore)?;
        let output = self.ultimo_documento.as_ref();
        let (da_byte, voci) =
            typst_ide::autocomplete(self, output, &self.principale, byte, esplicito)?;
        let da = self.principale.lines().byte_to_utf16(da_byte)?;
        let voci = voci
            .into_iter()
            .map(|c| Completamento {
                etichetta: c.label.to_string(),
                tipo: tipo_completamento(&c.kind),
                applica: c
                    .apply
                    .as_deref()
                    .filter(|a| *a != c.label.as_str())
                    .map(str::to_string),
                dettaglio: c.detail.as_deref().map(str::to_string),
            })
            .collect();
        Some(RisultatoCompletamento { da, voci })
    }

    /// Restituisce il testo di hover per l'elemento sotto il cursore (offset
    /// UTF-16). `Side::After`: l'elemento hoverato è quello che segue
    /// immediatamente l'offset (coerente col comportamento atteso al passaggio
    /// del mouse).
    pub fn tooltip(&self, cursore: usize) -> Option<String> {
        let byte = self.principale.lines().utf16_to_byte(cursore)?;
        let output = self.ultimo_documento.as_ref();
        let tooltip = typst_ide::tooltip(self, output, &self.principale, byte, Side::After)?;
        Some(match tooltip {
            typst_ide::Tooltip::Text(testo) | typst_ide::Tooltip::Code(testo) => testo.to_string(),
        })
    }

    /// Sincronizzazione anteprima→sorgente: clic in `(x, y)` pt sulla pagina
    /// `pagina` (0-based) → offset UTF-16 nel main, se il salto ricade lì.
    pub fn jump_da_click(&self, pagina: usize, x: f64, y: f64) -> Option<usize> {
        let documento = self.ultimo_documento.as_ref()?;
        // Difesa in profondità: un indice di pagina fuori range (frontend
        // disallineato dall'ultima anteprima, click su un frame stantio)
        // potrebbe far indicizzare `document.pages()` fuori bounds a
        // `typst_ide::jump_from_click` se questa non usasse un accesso
        // validato internamente — validare qui a monte evita di doverne
        // dipendere.
        if pagina >= documento.pages().len() {
            return None;
        }
        let pagina_1based = NonZeroUsize::new(pagina.checked_add(1)?)?;
        let posizione = PagedPosition {
            page: pagina_1based,
            point: Point::new(Abs::pt(x), Abs::pt(y)),
        };
        match typst_ide::jump_from_click(self, documento, &posizione)? {
            Jump::File(id, offset) if id == self.id_principale => {
                self.principale.lines().byte_to_utf16(offset)
            }
            // Salto verso un altro file, un URL, o una posizione senza uno
            // span nel main: nulla di jumpabile per l'editor.
            _ => None,
        }
    }

    /// Sincronizzazione sorgente→anteprima: dal cursore (offset UTF-16) nel
    /// main alle posizioni corrispondenti nell'ultima anteprima compilata.
    pub fn jump_da_cursore(&self, cursore: usize) -> Vec<Posizione> {
        let Some(documento) = self.ultimo_documento.as_ref() else {
            return Vec::new();
        };
        let Some(byte) = self.principale.lines().utf16_to_byte(cursore) else {
            return Vec::new();
        };
        typst_ide::jump_from_cursor(documento, &self.principale, byte)
            .into_iter()
            .map(|posizione: PagedPosition| Posizione {
                pagina: posizione.page.get() - 1,
                x: posizione.point.x.to_pt(),
                y: posizione.point.y.to_pt(),
            })
            .collect()
    }

    /// Indice dei titoli dell'**ultimo documento compilato con successo**
    /// (F5-020): tutti i file richiamati con `#include` e i titoli generati da
    /// codice, in ordine di documento. Dopo una compilazione fallita resta
    /// l'ultimo indice valido; vuoto finché non c'è una compilazione riuscita.
    ///
    /// Il flag `principale` di ogni voce si riferisce al file **aperto ora
    /// nell'editor** (può cambiare senza ricompilare, aprendo un capitolo).
    pub fn indice_documento(&self) -> Vec<VoceIndice> {
        let radice = self.radice_progetto().is_some();
        let rel_editor = self.percorso_relativo_editor();
        self.ultimo_indice
            .iter()
            .cloned()
            .map(|mut v| {
                if radice {
                    v.principale = v.file.is_some() && v.file == rel_editor;
                }
                v
            })
            .collect()
    }

    /// Percorso relativo alla radice (con `/`) del file in editing, se in un progetto.
    fn percorso_relativo_editor(&self) -> Option<String> {
        self.radice_progetto()?;
        Some(
            self.id_principale
                .vpath()
                .get_with_slash()
                .trim_start_matches('/')
                .to_string(),
        )
    }

    /// Raccoglie i titoli del documento compilato (`HeadingElem` via
    /// introspettore) e li dispone ad albero con [`indice::appiattisci`].
    ///
    /// Titolo = testo in chiaro del corpo; se lo span dell'elemento è
    /// *detached* si usa quello del corpo (Tinymist `typst-preview`
    /// `outline.rs`, `f3b00eb553d6b7a35b5d59cce529c8d0d774cfce`); pagina da `introspector.position`.
    fn costruisci_indice(&self, documento: &PagedDocument) -> Vec<VoceIndice> {
        // Derivato da Tinymist crates/typst-preview/src/outline.rs (f3b00eb553d6b7a35b5d59cce529c8d0d774cfce), licenza Apache-2.0 — Copyright Myriad-Dreamin e contributori Tinymist
        let introspettore: &dyn Introspector = documento.introspector().as_ref();
        let elementi = introspettore.query(&HeadingElem::ELEM.select());
        let piatti: Vec<_> = elementi
            .iter()
            .filter_map(|e| e.to_packed::<HeadingElem>())
            .map(|titolo| {
                let livello = titolo.resolve_level(StyleChain::default());
                let incluso = titolo.outlined.get(StyleChain::default());
                (titolo, livello, incluso)
            })
            .collect();

        indice::appiattisci(piatti)
            .into_iter()
            .map(|(titolo, livello, profondita)| {
                let testo = titolo.body.plain_text().trim().to_string();
                let span = titolo.span();
                let span = if span.is_detached() {
                    titolo.body.span()
                } else {
                    span
                };
                let pagina = titolo
                    .location()
                    .and_then(|posizione| introspettore.position(posizione))
                    .map(|p| p.as_paged_or_default().page.get())
                    .unwrap_or(1);
                let (principale, file, riga, colonna, offset) = self.posizione_titolo(span);
                VoceIndice {
                    livello,
                    profondita,
                    testo,
                    principale,
                    file,
                    riga,
                    colonna,
                    offset,
                    pagina,
                }
            })
            .collect()
    }

    /// File, riga, colonna e offset UTF-16 di uno span di titolo.
    ///
    /// I file di pacchetto non sono navigabili (`file` e posizioni `None`); i
    /// file del progetto danno il percorso relativo alla radice, con `/`.
    #[allow(clippy::type_complexity)]
    fn posizione_titolo(
        &self,
        span: typst::syntax::Span,
    ) -> (
        bool,
        Option<String>,
        Option<usize>,
        Option<usize>,
        Option<usize>,
    ) {
        let Some(id) = span.id() else {
            return (false, None, None, None, None);
        };
        let principale = id == self.id_principale;
        if !principale && !matches!(id.root(), VirtualRoot::Project) {
            return (false, None, None, None, None);
        }
        // Senza una radice agganciata solo il main virtuale è un "file": un
        // fratello non può esistere (e comunque non sarebbe apribile).
        let file = if principale && self.radice_progetto().is_none() {
            None
        } else {
            Some(
                id.vpath()
                    .get_with_slash()
                    .trim_start_matches('/')
                    .to_string(),
            )
        };
        let Ok(sorgente) = self.source(id) else {
            return (principale, file, None, None, None);
        };
        let Some(intervallo) = self.range(span) else {
            return (principale, file, None, None, None);
        };
        let righe = sorgente.lines();
        (
            principale,
            file,
            righe.byte_to_line(intervallo.start),
            righe.byte_to_column(intervallo.start),
            // CodeMirror normalizza `\r\n` in un solo carattere di riga: l'offset
            // per l'editor toglie un carattere per ogni `\r\n` che precede.
            righe.byte_to_utf16(intervallo.start).map(|o| {
                let crlf = sorgente.text()[..intervallo.start].matches("\r\n").count();
                o - crlf
            }),
        )
    }

    /// Converte un diagnostico del compilatore, aggiungendo la posizione UTF-16
    /// nel main quando disponibile (vedi `intervallo_utf16`).
    fn diagnostico_da(&self, diagnostica: &SourceDiagnostic) -> Diagnostico {
        let gravita = match diagnostica.severity {
            Severity::Error => "errore",
            Severity::Warning => "avviso",
        }
        .to_string();

        let mut messaggio = diagnostica.message.to_string();
        for suggerimento in &diagnostica.hints {
            messaggio.push_str("\nsuggerimento: ");
            messaggio.push_str(&suggerimento.v);
        }

        // Span reale nel main: si usa com'è. Altrimenti il punto di chiamata
        // nel main preso dalla traccia (F1-007 resta: mai mappare lo span di
        // un altro file sulle righe dell'editor).
        let (da, a, posizione_da_traccia) = match self.intervallo_utf16(diagnostica.span) {
            Some((da, a)) => (Some(da), Some(a), false),
            None => match self.intervallo_da_traccia(diagnostica) {
                Some((da, a)) => (Some(da), Some(a), true),
                None => (None, None, false),
            },
        };

        // L'origine ha senso solo quando l'errore è nato fuori dal main.
        let origine = if diagnostica.span.id() == Some(self.id_principale) {
            None
        } else {
            self.origine_span(diagnostica.span)
        };

        let traccia = diagnostica
            .trace
            .iter()
            .map(|punto| {
                let mut testo = descrivi_tracepoint(&punto.v);
                if let Some(dove) = self.dove_traccia(punto.span) {
                    testo.push_str(&format!(" ({dove})"));
                }
                testo
            })
            .collect();

        let suggerimenti = diagnostica.hints.iter().map(|h| h.v.to_string()).collect();

        Diagnostico {
            gravita,
            messaggio,
            da,
            a,
            posizione_da_traccia,
            origine,
            traccia,
            suggerimenti,
        }
    }

    /// Intervallo UTF-16 nel main del **primo** tracepoint (il compilatore li
    /// accumula dal più interno al più esterno) il cui span cade nel main:
    /// il punto di chiamata più vicino al codice dell'utente.
    fn intervallo_da_traccia(&self, diagnostica: &SourceDiagnostic) -> Option<(usize, usize)> {
        diagnostica
            .trace
            .iter()
            .find_map(|punto| self.intervallo_utf16(punto.span.into()))
    }

    /// Riga e colonna (1-based) di uno span, se il file è leggibile dal
    /// magazzino (file già letti per la compilazione: nessuna scansione).
    fn riga_colonna(&self, span: DiagSpan) -> Option<(usize, usize)> {
        let id = span.id()?;
        let intervallo = self.range(span)?;
        let sorgente = self.source(id).ok()?;
        let (riga, colonna) = sorgente.lines().byte_to_line_column(intervallo.start)?;
        Some((riga + 1, colonna + 1))
    }

    /// `file, riga R, colonna C` (o `@pacchetto — file, …`) di uno span fuori
    /// dal main; `None` se lo span non punta ad alcun file.
    fn origine_span(&self, span: DiagSpan) -> Option<String> {
        let id = span.id()?;
        let mut testo = etichetta_file(id);
        if let Some((riga, colonna)) = self.riga_colonna(span) {
            testo.push_str(&format!(", riga {riga}, colonna {colonna}"));
        }
        Some(testo)
    }

    /// Dove si trova un tracepoint: `riga R` nel main, `file, riga R` altrove.
    fn dove_traccia(&self, span: typst::syntax::Span) -> Option<String> {
        let id = span.id()?;
        let riga = self.riga_colonna(span.into()).map(|(riga, _)| riga);
        match (id == self.id_principale, riga) {
            (true, Some(riga)) => Some(format!("riga {riga}")),
            (true, None) => None,
            (false, Some(riga)) => Some(format!("{}, riga {riga}", etichetta_file(id))),
            (false, None) => Some(etichetta_file(id)),
        }
    }

    /// Errore di export PDF con riga/colonna (1-based) nel file in editing,
    /// quando lo span appartiene a esso (per gli altri file la posizione manca,
    /// per lo stesso motivo di [`Self::intervallo_utf16`]).
    fn errore_pdf(&self, diagnostica: &SourceDiagnostic) -> esporta::ErrorePdf {
        // Span reale nel main; altrimenti il primo tracepoint nel main (stessa
        // regola di `diagnostico_da`).
        let span_nel_main = std::iter::once(diagnostica.span)
            .chain(diagnostica.trace.iter().map(|p| DiagSpan::from(p.span)))
            .find(|s| s.id() == Some(self.id_principale));
        let posizione = span_nel_main
            .and_then(|s| self.range(s))
            .and_then(|r| self.principale.lines().byte_to_line_column(r.start))
            .map(|(riga, colonna)| (riga + 1, colonna + 1));
        esporta::ErrorePdf {
            messaggio: diagnostica.message.to_string(),
            posizione,
        }
    }

    /// Byte range → UTF-16 dello span, **solo** se appartiene al file
    /// principale: gli span di pacchetti `@preview`/file fratelli avrebbero
    /// offset validi ma nel file sbagliato, mappati per errore sulle righe
    /// dell'editor (vedi DECISIONI.md F1-007).
    fn intervallo_utf16(&self, span: DiagSpan) -> Option<(usize, usize)> {
        if span.id() != Some(self.id_principale) {
            return None;
        }
        let intervallo = self.range(span)?;
        let righe = self.principale.lines();
        let da = righe.byte_to_utf16(intervallo.start)?;
        let a = righe.byte_to_utf16(intervallo.end)?;
        Some((da, a))
    }
}

impl World for EditorWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.libreria
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.font.store().book()
    }

    fn main(&self) -> FileId {
        self.id_main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.id_principale {
            Ok(self.principale.clone())
        } else {
            self.magazzino.source(id)
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.id_principale {
            // Byte UTF-8 espliciti del main (non un'ottimizzazione "furba" via
            // stringa condivisa): il main può essere richiesto anche come file
            // grezzo (es. `read()` con `encoding: none`), e vogliamo che il
            // contenuto rispecchi esattamente il testo in memoria.
            Ok(Bytes::new(self.principale.text().as_bytes().to_vec()))
        } else {
            self.magazzino.file(id)
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.font.store().font(index)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        self.tempo.today(offset)
    }
}

impl IdeWorld for EditorWorld {
    fn upcast(&self) -> &dyn World {
        self
    }
}

/// Etichetta leggibile di un file: percorso relativo alla radice con `/` per
/// i file del progetto; `@ns/nome:versione — percorso` per quelli di pacchetto.
fn etichetta_file(id: FileId) -> String {
    let percorso = id
        .vpath()
        .get_with_slash()
        .trim_start_matches('/')
        .to_string();
    match id.root() {
        VirtualRoot::Project => percorso,
        VirtualRoot::Package(spec) => format!("{spec} — {percorso}"),
    }
}

/// Descrizione in italiano di un tracepoint del compilatore.
fn descrivi_tracepoint(punto: &Tracepoint) -> String {
    match punto {
        Tracepoint::Call(Some(nome)) => format!("chiamata della funzione `{nome}`"),
        Tracepoint::Call(None) => "chiamata di una funzione".to_string(),
        Tracepoint::Show(nome) => format!("regola show su `{nome}`"),
        Tracepoint::Import(nome) => format!("importazione del modulo `{nome}`"),
        Tracepoint::Include(nome) => format!("inclusione del file `{nome}`"),
    }
}

/// Hash del contenuto di una pagina, in esadecimale a 32 cifre.
fn hash_pagina(pagina: &Page) -> String {
    format!("{:032x}", typst::utils::hash128(pagina))
}

/// Metadati (dimensioni in pt + hash) di una pagina compilata.
fn metadati_pagina(pagina: &Page) -> MetadatiPagina {
    let dimensione = pagina.frame.size();
    MetadatiPagina {
        larghezza: dimensione.x.to_pt(),
        altezza: dimensione.y.to_pt(),
        hash: hash_pagina(pagina),
    }
}

/// Id del file principale virtuale in modalità documento singolo.
fn id_main_virtuale() -> FileId {
    // "main.typ" è un letterale costante: la validazione di `VirtualPath::new`
    // (rifiuta solo backslash e path vuoti malformati) non può mai fallire su
    // di esso, quindi l'`expect` non riguarda input esterno.
    let vpath = VirtualPath::new("main.typ").expect("\"main.typ\" è un percorso letterale valido");
    RootedPath::new(VirtualRoot::Project, vpath).intern()
}

/// Store dei font costruito **pigramente**, condivisibile fra thread (F5-007).
///
/// La scansione dei font di sistema è la parte lenta dell'avvio. Il
/// costruttore di [`EditorWorld`] non la esegue: la esegue chi chiama per
/// primo [`FontPrecaricabili::store`], mentre un thread di sfondo può
/// anticiparla con [`EditorWorld::font_precaricabili`] + [`Self::precarica`]
/// **senza tenere il lock del World**. `OnceLock::get_or_init` garantisce che
/// una compilazione lanciata durante la scansione *attenda* il risultato
/// (mai uno store parziale/vuoto) e che la scansione avvenga una volta sola.
pub struct FontPrecaricabili {
    /// Cartella di progetto da cui leggere anche `fonts/` (già canonica).
    radice_progetto: Option<PathBuf>,
    store: OnceLock<FontStore>,
}

impl FontPrecaricabili {
    fn nuovo(radice_progetto: Option<PathBuf>) -> Self {
        Self {
            radice_progetto,
            store: OnceLock::new(),
        }
    }

    /// Restituisce lo store, costruendolo se non ancora pronto (bloccante
    /// finché la scansione, eventualmente in corso su un altro thread, non
    /// termina).
    fn store(&self) -> &FontStore {
        self.store
            .get_or_init(|| costruisci_font_store(self.radice_progetto.as_deref()))
    }

    /// Forza la costruzione dello store (idempotente). Pensata per un thread
    /// di sfondo lanciato all'avvio.
    pub fn precarica(&self) {
        self.store();
    }

    /// `true` se la scansione è già stata completata.
    pub fn pronto(&self) -> bool {
        self.store.get().is_some()
    }
}

/// Voce di font condivisa fra tutti gli store del processo: la sorgente e il
/// font già caricato (lazy) restano in un solo posto.
struct VoceFontBase {
    sorgente: Box<dyn FontSource>,
    caricato: OnceLock<Option<Font>>,
    info: FontInfo,
}

/// Adattatore [`FontSource`] che punta a una [`VoceFontBase`] condivisa: i font
/// di sistema già letti dal disco non si rileggono a ogni apertura di progetto.
struct FontCondiviso(Arc<VoceFontBase>);

impl FontSource for FontCondiviso {
    fn load(&self) -> Option<Font> {
        self.0
            .caricato
            .get_or_init(|| self.0.sorgente.load())
            .clone()
    }
}

/// Font embedded + di sistema, costruiti **una sola volta per processo**.
///
/// La scansione dei font di sistema è la parte lenta: prima si ripeteva a ogni
/// `apri_progetto`/`chiudi_progetto` (sotto il lock del World, alla prima
/// compilazione successiva). Ora ogni store di progetto riusa queste voci e
/// aggiunge solo `radice/fonts`. Limite noto: un font di sistema installato a
/// programma aperto compare al riavvio (D-026).
fn font_base() -> &'static [Arc<VoceFontBase>] {
    static BASE: OnceLock<Vec<Arc<VoceFontBase>>> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut voci: Vec<Arc<VoceFontBase>> = Vec::new();
        let mut aggiungi = |sorgente: Box<dyn FontSource>, info: FontInfo| {
            voci.push(Arc::new(VoceFontBase {
                sorgente,
                caricato: OnceLock::new(),
                info,
            }));
        };
        for (font, info) in fonts::embedded() {
            aggiungi(Box::new(font), info);
        }
        for (percorso, info) in fonts::system() {
            aggiungi(Box::new(percorso), info);
        }
        voci
    })
}

/// Costruisce lo store dei font: base condivisa (embedded + sistema) più, se
/// presente, `radice_progetto/fonts` — best-effort, non fallisce se la cartella
/// manca. Ordine invariato: embedded, sistema, progetto.
fn costruisci_font_store(radice_progetto: Option<&Path>) -> FontStore {
    let mut store = FontStore::new();
    for voce in font_base() {
        store.push((FontCondiviso(Arc::clone(voce)), voce.info.clone()));
    }
    if let Some(radice) = radice_progetto {
        let cartella_font = radice.join("fonts");
        if cartella_font.is_dir() {
            store.extend(fonts::scan(&cartella_font));
        }
    }
    store
}

/// Mappa il `CompletionKind` di `typst-ide` a una stringa di tipo CodeMirror.
///
/// Non tutte le varianti hanno un equivalente ovvio nel set standard di
/// CodeMirror (`class`, `constant`, `enum`, `function`, `interface`,
/// `keyword`, `method`, `namespace`, `property`, `text`, `type`, `variable`):
/// dove manca una corrispondenza netta si ripiega su `"text"`.
fn tipo_completamento(kind: &CompletionKind) -> String {
    match kind {
        CompletionKind::Syntax => "keyword",
        CompletionKind::Func => "function",
        CompletionKind::Type => "type",
        CompletionKind::Param => "property",
        CompletionKind::Constant => "constant",
        CompletionKind::Package => "namespace",
        CompletionKind::Path | CompletionKind::Label | CompletionKind::Font => "text",
        CompletionKind::Symbol(_) => "text",
    }
    .to_string()
}

#[cfg(test)]
#[path = "../tests/unit/world.rs"]
mod test;
