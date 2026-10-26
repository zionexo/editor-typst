//! Guscio Tauri di Typster.
//!
//! Espone i comandi (compilazione, file e progetto, export, pacchetti,
//! impostazioni, cronologia, osservazione dei file) al frontend e gestisce il
//! ciclo di vita dell'applicazione: navigazione della finestra, registro dei
//! crash. Non contiene logica di dominio: quella vive in `typst-core`.
//!
//! # World persistente
//! L'`EditorWorld` viene costruito una sola volta all'avvio e tenuto in
//! `tauri::State<Arc<Mutex<EditorWorld>>>`. La scansione dei font di sistema
//! non è nel costruttore: parte in un thread di sfondo (`setup`, F5-007) e la
//! prima compilazione ne attende l'esito. Ad ogni compilazione si aggiorna solo
//! il sorgente via `aggiorna_sorgente`, senza riscansionare i font.
//!
//! # Esecuzione fuori dal thread UI
//! Il comando `compila` è asincrono: la closure bloccante viene spostata
//! su un thread dedicato tramite `tauri::async_runtime::spawn_blocking`,
//! così il thread UI (e il reactor async) restano liberi durante la compilazione.

mod osservatore;

use std::path::Path;
use std::sync::{Arc, Mutex};

use typst_core::albero::AlberoFile;
use typst_core::packages::EsitoAggiornamenti;
use typst_core::{
    EditorWorld, EsitoElenco, Impostazioni, OpzioniPdf, PaginaSvg, Posizione,
    RisultatoCompilazione, RisultatoCompletamento, Snippet, VoceIndice,
};

/// Compila il sorgente Typst fornito e restituisce metadati per pagina +
/// diagnostici (F5-008: nessun SVG, che si chiede a richiesta con `svg_pagina`).
///
/// Clona l'`Arc` dallo `State` per trasferire la proprietà nel thread bloccante,
/// poi esegue il lavoro pesante (parsing + layout + rendering) fuori dal thread
/// principale tramite `tauri::async_runtime::spawn_blocking`.
///
/// Restituisce un `RisultatoCompilazione` (serializzato verso il frontend) con
/// dimensioni e hash di ogni pagina e l'elenco di errori/avvisi posizionati: gli errori
/// fatali NON sono più un `Err`, ma fanno parte del risultato di successo, così
/// il frontend può mostrare anteprima e diagnostica inline nella stessa risposta.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Typst aggiornato dall'editor.
/// - `mondo`: stato gestito da Tauri contenente il World persistente (Arc<Mutex>).
///
/// # Errori
/// Restituisce `Err(Vec<String>)` solo se la compilazione viene interrotta
/// (task abortito). Gli errori di compilazione del sorgente sono trasportati
/// dentro `RisultatoCompilazione::diagnostici`, non come `Err`.
#[tauri::command]
async fn compila(
    sorgente: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<RisultatoCompilazione, Vec<String>> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato.
        // Se una compilazione precedente è andata in panic (caso anomalo),
        // il Mutex viene "avvelenato". Lo stato interno di EditorWorld
        // (sorgente + font) resta comunque valido, quindi recuperare il guard
        // dal poison evita di murare l'editor a ogni successiva richiesta.
        let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.aggiorna_sorgente(&sorgente);
        guida.compila()
    })
    .await
    // La closure ora restituisce direttamente `RisultatoCompilazione`, quindi
    // l'await produce `Result<RisultatoCompilazione, JoinError>`. Il `map_err`
    // converte l'unico errore possibile (task abortito) nel formato atteso e
    // coincide già con il tipo di ritorno: niente `?` da propagare.
    .map_err(|e| vec![format!("Compilazione interrotta: {e}")])
}

/// Restituisce l'SVG della pagina `indice` (0-based) dell'ultima compilazione
/// riuscita, insieme all'hash della pagina serializzata (F5-008).
///
/// Non ricompila: copia la pagina dal documento in cache nel World (lock
/// breve) e la serializza a lock rilasciato, fuori dal thread UI. `None` se
/// l'indice è fuori range o non esiste ancora un documento valido.
///
/// # Errori
/// `Err(String)` solo se il task viene abortito.
#[tauri::command]
async fn svg_pagina(
    indice: usize,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Option<PaginaSvg>, String> {
    let mondo = Arc::clone(&mondo);
    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        // Il lock serve solo a copiare la pagina; l'SVG si serializza a lock
        // rilasciato, senza bloccare le compilazioni successive.
        let pagina = {
            let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.pagina_da_renderizzare(indice)
        };
        pagina.map(|p| p.svg())
    })
    .await
    .map_err(|e| format!("Rendering della pagina interrotto: {e}"))
}

/// Legge il contenuto testuale di un file dal disco e aggancia il World alla sua
/// cartella, entrando in "modalità progetto" sulla cartella che lo contiene.
///
/// Usato dal frontend dopo che l'utente ha scelto un file tramite il dialog di
/// apertura (gestito lato web dal plugin dialog). Oltre a leggere il testo,
/// questo comando ora aggancia la **cartella del file** come radice di progetto:
/// così gli `#import`/`#image` con percorso relativo al file aperto vengono
/// risolti dal disco alle compilazioni successive (limite rimosso in Fase 4).
///
/// La radice agganciata è `parent` del file; l'entrypoint relativo è il solo
/// `file_name`. Il lavoro bloccante (I/O) avviene fuori dal thread UI tramite
/// `spawn_blocking`; il lock del World viene preso **solo** per `apri_progetto`,
/// per il tempo minimo necessario.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `percorso`: percorso assoluto del file da leggere.
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile in italiano se il file
/// non è leggibile (inesistente, permessi, contenuto non UTF-8), o se non ha
/// una cartella genitrice o un nome valido. Se invece la lettura riesce ma
/// l'aggancio-progetto (`apri_progetto`) fallisce (es. `canonicalize` della
/// cartella genitrice non riuscita), l'apertura **non fallisce**: si ripiega
/// su un semplice aggiornamento del sorgente in modalità documento singolo
/// (si perde solo la risoluzione dei percorsi relativi, non l'apertura del
/// file — "un errore non blocca l'editor"). Diverso da `apri_cartella`/
/// `nuovo_da_template`, dove la modalità progetto è lo scopo stesso del
/// comando e un suo fallimento resta un errore esplicito.
#[tauri::command]
async fn apri_file(
    percorso: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<String, String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);
    let stato = Arc::clone(&stato);

    tauri::async_runtime::spawn_blocking(move || {
        // Legge il testo del file (errore esplicito se non UTF-8/illeggibile).
        let contenuto = std::fs::read_to_string(&percorso)
            .map_err(|e| format!("Impossibile aprire il file '{percorso}': {e}"))?;

        let file = Path::new(&percorso);

        // Radice = cartella del file. Senza genitore (es. percorso radice del
        // filesystem) non è agganciabile come progetto: errore esplicito.
        let radice = file
            .parent()
            .ok_or_else(|| format!("Il file '{percorso}' non ha una cartella genitrice."))?;

        // Entrypoint relativo = solo il nome del file. `to_str` (non lossy) così
        // un nome non-UTF8 dà errore esplicito invece di un path corrotto.
        let entrypoint_relativo = file
            .file_name()
            .ok_or_else(|| format!("Il percorso '{percorso}' non è un file."))?
            .to_str()
            .ok_or_else(|| "Nome del file non valido (non UTF-8)".to_string())?;

        // Ciò che l'editor riceve è la base vera del documento (usata da
        // `osserva_documento`: file illeggibile, scritto da altri nel frattempo,
        // o riapertura). Dopo le validazioni: su errore non resta una voce
        // stantia. Prima del lock del World: mai annidare i due lock.
        typst_core::sorveglianza::segna_consegnato(&stato, file, contenuto.as_bytes());

        // Aggancia il World alla cartella del file (lock per il tempo minimo).
        {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            // Il file è già stato letto con successo: un aggancio-progetto
            // fallito non deve impedire l'apertura (vedi doc del comando).
            // Ripiega su un semplice aggiornamento del sorgente in modalità
            // documento singolo: si perde solo la risoluzione dei percorsi
            // relativi (import/immagini), non l'apertura del file in sé.
            //
            // Prima del ripiego, chiude un eventuale progetto già agganciato
            // (`chiudi_progetto`): senza questo reset lo stato del World
            // resterebbe legato alla radice-progetto precedente (font store e
            // risoluzione file fratelli di un altro progetto) anche se
            // `apri_progetto` per QUESTO file è fallito. Rende il ripiego
            // deterministico: sempre "documento singolo, nessuna radice".
            if guida
                .apri_progetto(radice.to_path_buf(), entrypoint_relativo, &contenuto)
                .is_err()
            {
                guida.chiudi_progetto();
                guida.aggiorna_sorgente(&contenuto);
            }
        }

        Ok(contenuto)
    })
    .await
    .map_err(|e| format!("Apertura del file interrotta: {e}"))?
}

/// Legge il testo di un file dal disco **senza** toccare il World (ricarica di
/// un file modificato da fuori, F5-022): a differenza di [`apri_file`] non
/// riaggancia la radice del progetto, che resta quella corrente. Se è il
/// documento aperto, ciò che si legge diventa il contenuto "visto dall'editor"
/// (da lì un salvataggio non è più un conflitto).
///
/// # Errori
/// `Err(String)` leggibile se il file non è leggibile o non è UTF-8.
#[tauri::command]
async fn leggi_file(
    percorso: String,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<String, String> {
    let stato = Arc::clone(&stato);
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::sorveglianza::leggi_e_segna(&stato, Path::new(&percorso))
            .map_err(|e| format!("Impossibile leggere il file '{percorso}': {e}"))
    })
    .await
    .map_err(|e| format!("Lettura del file interrotta: {e}"))?
}

/// Scrive un contenuto testuale su un file del disco.
///
/// Usato dal frontend dopo che l'utente ha scelto la destinazione tramite il
/// dialog di salvataggio (gestito lato web dal plugin dialog). Il lavoro di
/// I/O bloccante viene spostato fuori dal thread UI tramite `spawn_blocking`.
/// La scrittura è **atomica** (vedi [`typst_core::scrivi_atomico`]): un'interruzione a
/// metà (crash, disco pieno) non tronca il file precedente.
///
/// # Parametri
/// - `percorso`: percorso assoluto del file da scrivere (sovrascritto se esiste).
/// - `contenuto`: testo da salvare.
/// - `automatico`: `true` per il salvataggio automatico (F5-013: la cronologia
///   applica l'intervallo minimo fra le versioni; e non si scrive mai su un
///   file diverso dal documento aperto).
///
/// La scrittura passa dal registro delle scritture proprie (F5-022): il
/// controllo dei file modificati da fuori non la scambia per una modifica
/// esterna. **Non sovrascrive** un documento cambiato su disco da altri: in
/// quel caso ritorna `"conflitto"` (o `"sparito"` se il file non c'è più) senza
/// scrivere né registrare versioni. Dopo una scrittura riuscita si registra una
/// versione in cronologia (best-effort: un suo errore non fa fallire il salvataggio).
///
/// # Ritorno
/// `"scritto"`, `"conflitto"` o `"sparito"`.
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile in italiano se la
/// scrittura fallisce (permessi, percorso non valido, disco pieno, ecc.).
#[tauri::command]
async fn salva_file(
    percorso: String,
    contenuto: String,
    automatico: Option<bool>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<&'static str, String> {
    let stato = Arc::clone(&stato);
    let automatico = automatico.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        let esito = osservatore::scrivi_proprio(
            &stato,
            Path::new(&percorso),
            contenuto.as_bytes(),
            automatico,
        )
        .map_err(|e| format!("Impossibile salvare il file '{percorso}': {e}"))?;
        Ok(match esito {
            osservatore::EsitoScrittura::Scritto => {
                typst_core::cronologia::registra_dopo_salvataggio(
                    Path::new(&percorso),
                    &contenuto,
                    automatico,
                );
                "scritto"
            }
            osservatore::EsitoScrittura::Conflitto => "conflitto",
            osservatore::EsitoScrittura::Sparito => "sparito",
        })
    })
    .await
    .map_err(|e| format!("Salvataggio del file interrotto: {e}"))?
}

/// Compila il sorgente Typst fornito ed esporta il PDF risultante su disco.
///
/// Aggiorna il World persistente con il sorgente corrente, lo compila ed
/// esporta in PDF, scrivendo i byte sul percorso indicato. Il lavoro pesante
/// (compilazione + export + I/O) avviene fuori dal thread UI tramite
/// `spawn_blocking`. La scrittura su disco è **atomica** (vedi
/// [`typst_core::scrivi_atomico`]), come `salva_file`: stesso rischio di troncamento a
/// metà su un'interruzione, seppure di severità minore (un PDF esportato è
/// rigenerabile ricompilando, a differenza del sorgente in `salva_file`).
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `percorso`: percorso assoluto del file PDF di destinazione.
/// - `sorgente`: testo Typst aggiornato dall'editor.
/// - `opzioni`: standard PDF/A e intervallo di pagine (F5-011); assenti =
///   nessuno standard, tutte le pagine.
/// - `mondo`: stato gestito da Tauri contenente il World persistente (Arc<Mutex>).
///
/// # Errori
/// Restituisce `Err(Vec<String>)` con i messaggi di diagnosi se la compilazione
/// o l'export PDF falliscono (incluse le violazioni dello standard scelto: in
/// quel caso nessun file viene scritto), oppure con un singolo messaggio se la scrittura
/// del file fallisce o il task viene interrotto.
#[tauri::command]
async fn esporta_pdf(
    percorso: String,
    sorgente: String,
    opzioni: Option<OpzioniPdf>,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<(), Vec<String>> {
    let stato = Arc::clone(&stato);
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // La destinazione non può essere il documento aperto né un sorgente
        // (.typ/.typc/.md): decisione pura nel core, prima di compilare.
        let documento = typst_core::sorveglianza::blocca(&stato).doc.clone();
        typst_core::esporta::verifica_destinazione_export(
            Path::new(&percorso),
            documento.as_deref(),
        )
        .map_err(|e| vec![e])?;
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        // Il lock del World serve solo a compilare e produrre i byte: la
        // scrittura su disco (e il lock di `Condiviso`) avviene a lock rilasciato,
        // come in `esporta_immagini`, per non bloccare le compilazioni.
        let byte = {
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.aggiorna_sorgente(&sorgente);
            guida.esporta_pdf_con(&opzioni.unwrap_or_default())?
        };

        // Scrittura atomica (vedi `typst_core::scrivi_atomico`); eventuale errore di I/O
        // mappato in Vec<String>.
        // Registrata come scrittura propria: un export dentro la cartella del
        // progetto non deve sembrare una modifica esterna (F5-022).
        osservatore::scrivi_proprio(&stato, Path::new(&percorso), &byte, false)
            .map(|_| ())
            .map_err(|e| vec![format!("Impossibile salvare il PDF '{percorso}': {e}")])
    })
    .await
    .map_err(|e| vec![format!("Export PDF interrotto: {e}")])?
}

/// Esito di [`esporta_immagini`]: file scritti, oppure conflitto da confermare.
#[derive(serde::Serialize)]
#[serde(tag = "esito")]
enum EsitoImmagini {
    /// Export completato: numero di file scritti.
    Scritti { file: usize },
    /// Nulla è stato scritto: questi file esistono già e `sovrascrivi` era `false`.
    Conflitto { esistenti: Vec<String> },
}

/// Esporta le pagine scelte del documento in PNG o SVG (F5-011).
///
/// Compila sotto lock (breve), poi renderizza e scrive **a lock rilasciato**:
/// il rendering PNG di molte pagine ad alta risoluzione non blocca le
/// compilazioni dell'anteprima. Una sola pagina → esattamente `percorso`; più
/// pagine → `<nome>-<NN>.<estensione>` accanto a `percorso` (vedi
/// `typst_core::esporta::percorsi_pagine`). Ogni file è scritto in modo atomico.
///
/// Il file `percorso` ha già la conferma di sovrascrittura del dialog di
/// sistema; i file `nome-NN.ext` no. Se `sovrascrivi` è `false` e almeno uno
/// di questi esiste già, il comando **non scrive nulla** (e non renderizza) e
/// restituisce `Conflitto`: il frontend chiede conferma e rinvoca con `true`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `formato`: `"png"` o `"svg"`.
/// - `percorso`: file scelto dall'utente nel dialog di salvataggio.
/// - `sorgente`: testo Typst aggiornato dall'editor.
/// - `pagine`: intervallo (`1-3, 5`); assente/vuoto = tutte le pagine.
/// - `ppi`: risoluzione del PNG (predefinita 144); ignorata per l'SVG.
/// - `sovrascrivi`: `true` dopo la conferma dell'utente sui file già presenti.
///
/// # Ritorno
/// `Scritti { file }` o `Conflitto { esistenti }`. `Err(Vec<String>)` con i
/// messaggi se la compilazione, l'intervallo, la risoluzione o la scrittura
/// falliscono. Un errore a metà serie **non annulla** i file già scritti: il
/// messaggio dice quanti ne sono stati scritti su quanti.
// 6 argomenti applicativi + 2 `State` iniettati da Tauri: non raggruppabili senza
// cambiare il contratto `invoke` con il frontend.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn esporta_immagini(
    formato: String,
    percorso: String,
    sorgente: String,
    pagine: Option<String>,
    ppi: Option<f32>,
    sovrascrivi: bool,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<EsitoImmagini, Vec<String>> {
    let stato = Arc::clone(&stato);
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        if formato != "png" && formato != "svg" {
            return Err(vec![format!(
                "Formato di export non supportato: '{formato}'"
            )]);
        }
        // Destinazione valida (non il documento aperto, non un sorgente) prima
        // di compilare. Vale anche per i nomi `nome-NN.ext` generati sotto.
        let documento = typst_core::sorveglianza::blocca(&stato).doc.clone();
        typst_core::esporta::verifica_destinazione_export(
            Path::new(&percorso),
            documento.as_deref(),
        )
        .map_err(|e| vec![e])?;
        let scelte = {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.aggiorna_sorgente(&sorgente);
            guida.pagine_da_esportare(pagine.as_deref())?
        };

        let scelto = Path::new(&percorso);
        let percorsi =
            typst_core::esporta::percorsi_pagine(scelto, &scelte.numeri(), scelte.totale());
        for destinazione in &percorsi {
            typst_core::esporta::verifica_destinazione_export(destinazione, documento.as_deref())
                .map_err(|e| vec![e])?;
        }
        if !sovrascrivi {
            let esistenti = typst_core::esporta::percorsi_esistenti(&percorsi, scelto);
            if !esistenti.is_empty() {
                return Ok(EsitoImmagini::Conflitto {
                    esistenti: esistenti
                        .iter()
                        .map(|p| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| p.display().to_string())
                        })
                        .collect(),
                });
            }
        }

        let contenuti: Vec<Vec<u8>> = if formato == "png" {
            scelte
                .png(ppi.unwrap_or(typst_core::esporta::PPI_PREDEFINITO))
                .map_err(|e| vec![e])?
        } else {
            scelte.svg().into_iter().map(String::into_bytes).collect()
        };

        let totale_file = percorsi.len();
        for (scritti, (destinazione, byte)) in percorsi.iter().zip(&contenuti).enumerate() {
            osservatore::scrivi_proprio(&stato, destinazione, byte, false).map_err(|e| {
                vec![format!(
                    "Impossibile salvare '{}': {e}. Scritti {scritti} file su {totale_file} \
                     prima dell'errore; i file già scritti restano su disco.",
                    destinazione.display()
                )]
            })?;
        }
        Ok(EsitoImmagini::Scritti { file: totale_file })
    })
    .await
    .map_err(|e| vec![format!("Export interrotto: {e}")])?
}

/// Esito di un'esportazione per conversione (testo, HTML, LaTeX, Markdown).
#[derive(serde::Serialize)]
struct EsitoConversione {
    /// File scritto.
    percorso: String,
    /// Perdite e approssimazioni della conversione (elenco compatto, non fatale).
    avvisi: Vec<String>,
    /// `true` se il sorgente Typst ha errori di sintassi (conversione al meglio).
    errori_sintassi: bool,
}

/// Convalida la destinazione, converte e scrive in modo atomico.
///
/// Le conversioni sono pure e non toccano il World: nessun lock, il lavoro
/// gira interamente in `spawn_blocking`. `converti` riceve la cartella del
/// documento aperto (per incorporare le immagini), se il documento è su disco.
async fn esporta_conversione(
    nome_formato: &'static str,
    percorso: String,
    sorgente: String,
    formato_md: bool,
    stato: osservatore::Condiviso,
    converti: impl FnOnce(Option<&Path>) -> typst_core::Conversione + Send + 'static,
) -> Result<EsitoConversione, Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || {
        let documento = typst_core::sorveglianza::blocca(&stato).doc.clone();
        typst_core::verifica_destinazione_conversione(
            Path::new(&percorso),
            documento.as_deref(),
            formato_md,
        )
        .map_err(|e| vec![e])?;
        let errori_sintassi = typst_core::ha_errori_sintassi(&sorgente);
        let radice = documento.as_deref().and_then(Path::parent);
        let risultato = converti(radice);
        osservatore::scrivi_proprio(
            &stato,
            Path::new(&percorso),
            risultato.contenuto.as_bytes(),
            false,
        )
        .map_err(|e| vec![format!("Impossibile salvare il file '{percorso}': {e}")])?;
        Ok(EsitoConversione {
            percorso,
            avvisi: risultato.avvisi,
            errori_sintassi,
        })
    })
    .await
    .map_err(|e| vec![format!("Export {nome_formato} interrotto: {e}")])?
}

/// Esporta il sorgente Typst come **testo semplice** (`.txt`).
///
/// `larghezza`: colonne di a capo (predefinita 80, limitata dal core a 20–500).
#[tauri::command]
async fn esporta_testo(
    percorso: String,
    sorgente: String,
    larghezza: Option<usize>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<EsitoConversione, Vec<String>> {
    let stato = Arc::clone(&stato);
    let src = sorgente.clone();
    esporta_conversione("testo", percorso, sorgente, false, stato, move |_| {
        typst_core::typst_a_testo(
            &src,
            &typst_core::OpzioniTesto {
                larghezza: larghezza.unwrap_or(80),
            },
        )
    })
    .await
}

/// Esporta il sorgente Typst come pagina **HTML5**.
///
/// Con `autonomo` le immagini sono incorporate come data-URI, lette **solo**
/// sotto la radice del progetto aperto (o la cartella del documento se non c'è
/// un progetto): `/x.png` dalla radice, gli altri dalla cartella del documento,
/// `..` solo se il file resta nel progetto; niente percorsi assoluti o symlink
/// verso l'esterno; tetto per immagine e sul totale. Le non incorporabili
/// restano percorsi relativi con un avviso. Senza `autonomo` (o con documento
/// non salvato) i percorsi restano relativi.
#[tauri::command]
async fn esporta_html(
    percorso: String,
    sorgente: String,
    autonomo: Option<bool>,
    stato: tauri::State<'_, osservatore::Condiviso>,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<EsitoConversione, Vec<String>> {
    let stato = Arc::clone(&stato);
    let mondo = Arc::clone(&mondo);
    let src = sorgente.clone();
    let autonomo = autonomo.unwrap_or(false);
    esporta_conversione(
        "HTML",
        percorso,
        sorgente,
        false,
        stato,
        move |radice| match (autonomo, radice) {
            (true, Some(cartella)) => {
                // Radice del progetto del World (lock breve, rilasciato prima della
                // conversione): `/img/x.png` e `../img/x.png` valgono come in Typst.
                let radice_progetto = {
                    let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
                    guida.radice_progetto().map(Path::to_path_buf)
                };
                let (opz, mancate) =
                    typst_core::opzioni_html_progetto(radice_progetto.as_deref(), cartella);
                let mut c = typst_core::typst_a_html(&src, &opz);
                c.avvisi.extend(
                    mancate
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .iter()
                        .cloned(),
                );
                c
            }
            (true, None) => {
                let mut c = typst_core::typst_a_html(&src, &typst_core::OpzioniHtml::default());
                c.avvisi.push(
                    "il documento non è salvato su disco: le immagini non sono state incorporate"
                        .into(),
                );
                c
            }
            _ => typst_core::typst_a_html(&src, &typst_core::OpzioniHtml::default()),
        },
    )
    .await
}

/// Esporta il sorgente Typst come documento **LaTeX**.
///
/// `classe` (`article`/`report`/`book`), `carta` (`a4paper`/`a5paper`/
/// `letterpaper`) e `dimensione_pt` (10/11/12): valori fuori elenco sono
/// rifiutati con un errore chiaro.
#[tauri::command]
async fn esporta_latex(
    percorso: String,
    sorgente: String,
    classe: Option<String>,
    carta: Option<String>,
    dimensione_pt: Option<u8>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<EsitoConversione, Vec<String>> {
    let mut opz = typst_core::OpzioniLatex::default();
    if let Some(c) = classe {
        if !["article", "report", "book"].contains(&c.as_str()) {
            return Err(vec![format!("Classe LaTeX non supportata: '{c}'")]);
        }
        opz.classe = c;
    }
    if let Some(c) = carta {
        if !["a4paper", "a5paper", "letterpaper"].contains(&c.as_str()) {
            return Err(vec![format!("Formato carta non supportato: '{c}'")]);
        }
        opz.carta = c;
    }
    if let Some(p) = dimensione_pt {
        if ![10, 11, 12].contains(&p) {
            return Err(vec![format!("Corpo del testo non supportato: {p} pt")]);
        }
        opz.dimensione_pt = p;
    }
    let stato = Arc::clone(&stato);
    let src = sorgente.clone();
    esporta_conversione("LaTeX", percorso, sorgente, false, stato, move |_| {
        typst_core::typst_a_latex(&src, &opz)
    })
    .await
}

/// Esporta il sorgente Typst come **Markdown**, con l'elenco delle perdite.
///
/// A differenza degli altri export accetta l'estensione `.md` (ma mai sul
/// documento aperto).
#[tauri::command]
async fn esporta_markdown(
    percorso: String,
    sorgente: String,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<EsitoConversione, Vec<String>> {
    let stato = Arc::clone(&stato);
    let src = sorgente.clone();
    esporta_conversione("Markdown", percorso, sorgente, true, stato, move |_| {
        typst_core::typst_a_md_con_avvisi(&src)
    })
    .await
}

/// Calcola le proposte di autocompletamento per la posizione del cursore.
///
/// Aggiorna il World persistente col sorgente corrente (così l'analisi IDE vede
/// l'ultimo testo dell'editor), poi delega a `EditorWorld::completa`. Il lavoro
/// di parsing/analisi avviene fuori dal thread UI tramite `spawn_blocking`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Typst aggiornato dall'editor.
/// - `cursore`: offset **UTF-16** del cursore (CodeMirror lavora in UTF-16).
/// - `esplicito`: `true` se il completamento è richiesto esplicitamente
///   dall'utente (es. Ctrl+Space).
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Ritorno
/// `Ok(Some(..))` con le voci proposte, `Ok(None)` se non c'è nulla da
/// completare nel contesto corrente. `Err(String)` solo se il task viene
/// interrotto (abortito).
#[tauri::command]
async fn completa(
    sorgente: String,
    cursore: usize,
    esplicito: bool,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Option<RisultatoCompletamento>, String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.aggiorna_sorgente(&sorgente);
        guida.completa(cursore, esplicito)
    })
    .await
    .map_err(|e| format!("Autocompletamento interrotto: {e}"))
}

/// Restituisce il testo di hover (tooltip) per l'elemento sotto il cursore.
///
/// Aggiorna il World persistente col sorgente corrente, poi delega a
/// `EditorWorld::tooltip`. Il lavoro avviene fuori dal thread UI tramite
/// `spawn_blocking`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Typst aggiornato dall'editor.
/// - `cursore`: offset **UTF-16** del cursore.
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Ritorno
/// `Ok(Some(testo))` con la descrizione, `Ok(None)` se non c'è nulla da
/// mostrare. `Err(String)` solo se il task viene interrotto (abortito).
#[tauri::command]
async fn tooltip(
    sorgente: String,
    cursore: usize,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Option<String>, String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.aggiorna_sorgente(&sorgente);
        guida.tooltip(cursore)
    })
    .await
    .map_err(|e| format!("Hover interrotto: {e}"))
}

/// Sincronizzazione **anteprima → sorgente**: dato un clic sull'anteprima,
/// restituisce l'offset (UTF-16) corrispondente nella sorgente principale.
///
/// Non aggiorna il sorgente: opera sull'**ultimo documento compilato con
/// successo** già conservato dal World (vedi `EditorWorld::compila`). Il lavoro
/// (risoluzione del clic sul frame della pagina) avviene fuori dal thread UI
/// tramite `spawn_blocking`; il lock è preso in sola lettura sullo stato del World.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `pagina`: indice **0-based** della pagina cliccata.
/// - `x`/`y`: coordinate del clic nel frame della pagina, in punti tipografici
///   (pt), coincidenti con le coordinate user-space dell'SVG (reso senza scala).
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Ritorno
/// `Ok(Some(offset))` con l'offset UTF-16 nella sorgente quando il clic ricade
/// su un elemento del file principale; `Ok(None)` se non c'è anteprima valida,
/// il clic non colpisce nulla di jumpabile, o il salto punta altrove (altro file
/// o URL). `Err(String)` solo se il task viene interrotto (abortito).
#[tauri::command]
async fn jump_da_click(
    pagina: usize,
    x: f64,
    y: f64,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Option<usize>, String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.jump_da_click(pagina, x, y)
    })
    .await
    .map_err(|e| format!("Jump da click interrotto: {e}"))
}

/// Sincronizzazione **sorgente → anteprima**: data la posizione del cursore,
/// restituisce le posizioni corrispondenti nell'anteprima renderizzata.
///
/// Non aggiorna il sorgente: opera sull'**ultimo documento compilato con
/// successo** già conservato dal World (vedi `EditorWorld::compila`). Il lavoro
/// avviene fuori dal thread UI tramite `spawn_blocking`; il lock è preso in
/// sola lettura sullo stato del World.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `cursore`: offset **UTF-16** del cursore (CodeMirror lavora in UTF-16).
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Ritorno
/// `Ok(Vec<Posizione>)` con le posizioni (pagina 0-based + coordinate in pt) su
/// cui evidenziare l'anteprima; vuoto se non c'è anteprima valida o il cursore
/// non è su testo mappabile. `Err(String)` solo se il task viene interrotto.
#[tauri::command]
async fn jump_da_cursore(
    cursore: usize,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Vec<Posizione>, String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.jump_da_cursore(cursore)
    })
    .await
    .map_err(|e| format!("Jump da cursore interrotto: {e}"))
}

/// Restituisce l'elenco degli snippet riutilizzabili per l'editor.
///
/// Comando volutamente **stateless**: a differenza di `compila`, `completa` e
/// `tooltip`, NON riceve né tocca lo stato gestito (`Arc<Mutex<EditorWorld>>`).
/// Si limita a leggere la configurazione dal disco, quindi non concorre sul lock
/// del World e non ne richiede un parametro `State`.
///
/// Delega a `typst_core::config::carica_snippet`, che legge il file esterno
/// `snippets.json` (creandolo coi predefiniti alla prima esecuzione) e degrada
/// sempre con grazia ai predefiniti in caso di problemi. Il lavoro bloccante
/// (lettura file, parsing JSON) viene spostato fuori dal thread UI tramite
/// `spawn_blocking`.
///
/// # Ritorno
/// `Ok(Vec<Snippet>)` con gli snippet caricati. Il ripiego ai predefiniti
/// scatta solo se il file utente manca, non è leggibile o non è un array
/// JSON: un array valido (anche vuoto, `[]`) viene restituito così com'è,
/// quindi il risultato **può essere vuoto** se l'utente ha svuotato
/// deliberatamente `snippets.json`. `Err(String)` solo se il task viene
/// interrotto (abortito).
#[tauri::command]
async fn elenca_snippet() -> Result<Vec<Snippet>, String> {
    tauri::async_runtime::spawn_blocking(typst_core::config::carica_snippet)
        .await
        .map_err(|e| format!("Caricamento degli snippet interrotto: {e}"))
}

/// Elenca i modelli di documento (integrati + utente in
/// `config_dir()/typster/modelli/`). Stateless come `elenca_snippet`; infallibile
/// lato core (file illeggibili saltati); `Err` solo se il task viene interrotto.
#[tauri::command]
async fn elenca_modelli() -> Result<Vec<typst_core::modelli::ModelloInfo>, String> {
    tauri::async_runtime::spawn_blocking(typst_core::modelli::elenca_modelli_sistema)
        .await
        .map_err(|e| format!("Elenco dei modelli interrotto: {e}"))
}

/// Genera il sorgente del modello `id` con i `valori` (campo → testo).
///
/// # Errori
/// `Err(String)` (messaggio italiano) per id non valido/inesistente, tetti
/// superati o task interrotto.
#[tauri::command]
async fn compila_modello(
    id: String,
    valori: std::collections::BTreeMap<String, String>,
) -> Result<typst_core::modelli::RisultatoModello, String> {
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::modelli::compila_modello_sistema(&id, &valori).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Compilazione del modello interrotta: {e}"))?
}

/// Salva un sorgente con segnaposto come modello utente; restituisce l'id.
///
/// `sovrascrivi` (default `false`): se esiste già un modello **utente** con lo
/// stesso id e vale `false`, `Err` con messaggio che INIZIA con «Esiste già un
/// modello» (il frontend lo riconosce, chiede conferma e riprova con `true`).
///
/// # Errori
/// `Err(String)` per dati non validi, modello già esistente, tetti superati,
/// I/O o task interrotto.
#[tauri::command]
async fn salva_come_modello(
    nome: String,
    sorgente: String,
    campi: Vec<typst_core::modelli::CampoModello>,
    descrizione: Option<String>,
    pagine: Option<u32>,
    sovrascrivi: Option<bool>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::modelli::salva_come_modello_sistema(
            &nome,
            &sorgente,
            &campi,
            descrizione.as_deref().unwrap_or(""),
            pagine,
            sovrascrivi.unwrap_or(false),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Salvataggio del modello interrotto: {e}"))?
}

/// Elimina un modello **utente**; i modelli integrati sono rifiutati.
///
/// # Errori
/// `Err(String)` se l'id non è valido, il modello è integrato/inesistente o
/// l'I/O fallisce.
#[tauri::command]
async fn elimina_modello(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::modelli::elimina_modello_sistema(&id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Eliminazione del modello interrotta: {e}"))?
}

/// Restituisce le impostazioni utente (`impostazioni.json`, F5-003).
///
/// Stateless come `elenca_snippet`. Infallibile lato core (file mancante o
/// invalido -> default); `Err` solo se il task viene interrotto.
#[tauri::command]
async fn leggi_impostazioni() -> Result<Impostazioni, String> {
    tauri::async_runtime::spawn_blocking(typst_core::config::carica_impostazioni)
        .await
        .map_err(|e| format!("Lettura delle impostazioni interrotta: {e}"))
}

/// Salva le impostazioni utente con scrittura atomica.
///
/// # Errori
/// `Err(String)` se la scrittura fallisce o il task viene interrotto.
#[tauri::command]
async fn salva_impostazioni(impostazioni: Impostazioni) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::config::salva_impostazioni(&impostazioni)
            .map_err(|e| format!("Impossibile salvare le impostazioni: {e}"))
    })
    .await
    .map_err(|e| format!("Salvataggio delle impostazioni interrotto: {e}"))?
}

/// Restituisce l'elenco dei pacchetti di Typst Universe per il browser (3.1).
///
/// Comando volutamente **stateless**: come `elenca_snippet`, non riceve né tocca
/// lo stato gestito (`Arc<Mutex<EditorWorld>>`), perché la consultazione
/// dell'indice è indipendente dal World e non deve concorrere sul suo lock.
///
/// Delega a `typst_core::packages::elenca`, che è offline-first (cache su disco
/// con TTL, ripiego su copia locale se la rete è assente). Il lavoro bloccante
/// (rete + I/O + parsing del JSON) viene spostato fuori dal thread UI tramite
/// `spawn_blocking`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `forza`: se `true`, ignora la freschezza della cache e tenta sempre il
///   download aggiornato (pulsante "aggiorna" della UI).
///
/// # Ritorno
/// `Ok(EsitoElenco)` con i pacchetti (uno per nome, versione più recente) e il
/// flag `stantia` (true se mostrati da cache scaduta in assenza di rete).
/// `Err(String)` se non c'è alcun dato disponibile (rete assente e nessuna
/// cache) o se il task viene interrotto.
#[tauri::command]
async fn elenca_pacchetti(forza: bool) -> Result<EsitoElenco, String> {
    tauri::async_runtime::spawn_blocking(move || typst_core::packages::elenca(forza))
        .await
        .map_err(|e| format!("Caricamento dei pacchetti interrotto: {e}"))?
}

/// Controlla gli aggiornamenti dei pacchetti `@preview` (F5-012): quelli
/// installati nella cache standard e, se `sorgente` è dato, gli `#import` del
/// documento corrente. Stateless e **senza rete**: confronta con l'indice di
/// Typst Universe già in cache (`Err` se non è mai stato scaricato).
///
/// # Errori
/// `Err(String)` se l'indice non è in cache o il task viene interrotto.
#[tauri::command]
async fn aggiornamenti_pacchetti(sorgente: Option<String>) -> Result<EsitoAggiornamenti, String> {
    tauri::async_runtime::spawn_blocking(move || {
        typst_core::packages::controlla_aggiornamenti(sorgente.as_deref())
    })
    .await
    .map_err(|e| format!("Controllo degli aggiornamenti interrotto: {e}"))?
}

/// Esito dell'apertura/creazione di un progetto: percorso assoluto del suo
/// entrypoint su disco e il relativo contenuto testuale, pronto a essere
/// caricato nell'editor.
///
/// Condiviso da `nuovo_da_template` (3.3) e da `apri_cartella` (Fase 4): in
/// entrambi i casi il frontend riceve un entrypoint da aprire e il suo testo,
/// quindi una sola struct evita duplicati con gli stessi campi.
#[derive(serde::Serialize)]
struct RisultatoProgetto {
    /// Percorso assoluto dell'entrypoint del progetto.
    entrypoint: String,
    /// Contenuto testuale dell'entrypoint, da mostrare subito nell'editor.
    contenuto: String,
}

/// Crea un nuovo progetto a partire da un template di Typst Universe (3.3).
///
/// Scarica il template (se non in cache), ne copia i file in una sottocartella
/// di `destinazione` e aggancia il World alla nuova radice di progetto, così che
/// i file fratelli dell'entrypoint (immagini, include, dati) vengano risolti dal
/// disco alle compilazioni successive. Restituisce al frontend il percorso e il
/// contenuto dell'entrypoint da aprire nell'editor.
///
/// A differenza dei comandi stateless del browser pacchetti, questo comando
/// **tocca lo stato gestito** (`Arc<Mutex<EditorWorld>>`): dopo la copia su disco
/// chiama `apri_progetto`, che ridefinisce il file principale del World e la sua
/// radice. Il lavoro bloccante (rete + estrazione + copia ricorsiva + I/O) è
/// spostato fuori dal thread UI tramite `spawn_blocking`; il lock del World viene
/// acquisito **solo** alla fine, per il tempo minimo necessario ad agganciare il
/// progetto (il download lungo avviene prima, senza tenere il lock).
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `nome`: nome del pacchetto template senza namespace (es. `"charged-ieee"`).
/// - `versione`: versione esatta `"major.minor.patch"`.
/// - `destinazione`: cartella scelta dall'utente; il progetto finisce in una sua
///   sottocartella omonima del pacchetto.
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile se: il template non
/// esiste/non è scaricabile, il pacchetto non è un template, la copia dei file
/// fallisce, l'entrypoint non è leggibile, o il task viene interrotto.
#[tauri::command]
async fn nuovo_da_template(
    nome: String,
    versione: String,
    destinazione: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<RisultatoProgetto, String> {
    use std::path::Component;

    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);
    let stato = Arc::clone(&stato);

    tauri::async_runtime::spawn_blocking(move || {
        // Scarica il template e copia i file nel progetto; ottiene radice del
        // progetto (dir_progetto) ed entrypoint assoluto creati su disco.
        let progetto =
            typst_core::packages::nuovo_da_template(&nome, &versione, Path::new(&destinazione))?;
        let dir_progetto = progetto.dir_progetto;
        let entrypoint = progetto.entrypoint;

        // Legge il contenuto dell'entrypoint da mostrare subito nell'editor.
        let contenuto = std::fs::read_to_string(&entrypoint).map_err(|e| {
            format!(
                "Template creato ma entrypoint '{}' non leggibile: {e}",
                entrypoint.display()
            )
        })?;

        // C-2: la radice del progetto è `dir_progetto` (NON `entrypoint.parent()`,
        // sbagliato per entrypoint annidati come `src/main.typ`). L'entrypoint
        // relativo si ricava togliendo il prefisso della radice.
        let relativo = entrypoint.strip_prefix(&dir_progetto).map_err(|_| {
            format!(
                "Entrypoint '{}' fuori dalla radice di progetto '{}'",
                entrypoint.display(),
                dir_progetto.display()
            )
        })?;

        // `VirtualPath` usa separatori `/`: ricostruisci il relativo unendo i
        // soli componenti "normali" con `/`. C-8: `to_str()` (non `to_string_lossy`)
        // così un componente non-UTF8 dà errore esplicito, non un path corrotto.
        let mut segmenti: Vec<&str> = Vec::new();
        for componente in relativo.components() {
            match componente {
                Component::Normal(s) => {
                    let s = s
                        .to_str()
                        .ok_or_else(|| "Entrypoint con nome non valido (non UTF-8)".to_string())?;
                    segmenti.push(s);
                }
                // CurDir (`.`) ignorato; qualunque altro componente (`..`, radice,
                // prefisso) non deve comparire in un relativo dentro la radice.
                Component::CurDir => {}
                _ => {
                    return Err(format!(
                        "Percorso entrypoint relativo non valido: '{}'",
                        relativo.display()
                    ));
                }
            }
        }
        let entrypoint_relativo = segmenti.join("/");

        // C-8: percorso assoluto dell'entrypoint come stringa, con errore
        // esplicito se non è UTF-8 (invece di una stringa lossy/corrotta).
        let entrypoint_str = typst_core::albero::normalizza_verbatim(
            entrypoint
                .to_str()
                .ok_or_else(|| "Percorso entrypoint non valido (non UTF-8)".to_string())?,
        );

        // Aggancia il World al nuovo progetto (lock per il tempo minimo).
        {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.apri_progetto(dir_progetto, &entrypoint_relativo, &contenuto)?;
        }
        // Lock del World già rilasciato (mai annidare): il contenuto consegnato
        // all'editor è la base vera per `osserva_documento`.
        typst_core::sorveglianza::segna_consegnato(&stato, &entrypoint, contenuto.as_bytes());

        Ok(RisultatoProgetto {
            entrypoint: entrypoint_str,
            contenuto,
        })
    })
    .await
    .map_err(|e| format!("Creazione da template interrotta: {e}"))?
}

/// Apre una cartella-progetto: rileva l'entrypoint, ne legge il contenuto e
/// aggancia il World alla cartella come radice di progetto (Fase 4).
///
/// Riusa la "modalità progetto" già esistente: a differenza dell'apertura di un
/// singolo file (che aggancia la sola cartella genitrice del file scelto),
/// `apri_cartella` deduce l'entrypoint dall'euristica di
/// [`typst_core::rileva_entrypoint`] (`main.typ` → `[package].entrypoint` →
/// unico `.typ`), poi aggancia la **cartella scelta** come radice. Così import e
/// immagini con percorso relativo vengono risolti dal disco anche per entrypoint
/// annidati (es. `src/main.typ`, dove la radice è la cartella, non `.../src`).
///
/// Come `nuovo_da_template`, **tocca lo stato gestito** (`Arc<Mutex<EditorWorld>>`)
/// chiamando `apri_progetto`. Il lavoro bloccante (rilevamento + I/O) è spostato
/// fuori dal thread UI tramite `spawn_blocking`; il lock del World viene acquisito
/// **solo** alla fine, per il tempo minimo necessario ad agganciare il progetto.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `percorso`: percorso assoluto della cartella scelta dall'utente.
/// - `mondo`: World persistente condiviso (Arc<Mutex>).
///
/// # Ritorno
/// `Ok(RisultatoProgetto)` con il percorso assoluto dell'entrypoint e il suo
/// contenuto testuale, da aprire nell'editor.
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile in italiano se: la
/// cartella non esiste o non contiene un entrypoint individuabile (nessun `.typ`,
/// oppure più `.typ` ambigui); l'entrypoint non è leggibile o fuori dalla radice;
/// `apri_progetto` fallisce; o il task viene interrotto.
#[tauri::command]
async fn apri_cartella(
    percorso: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<RisultatoProgetto, String> {
    use std::path::Component;

    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);
    let stato = Arc::clone(&stato);

    tauri::async_runtime::spawn_blocking(move || {
        // La cartella scelta è la radice del progetto. `rileva_entrypoint` la
        // canonicalizza UNA sola volta e restituisce sia la radice canonica sia
        // l'entrypoint assoluto sotto di essa: si riusa quella radice per
        // `strip_prefix`/`apri_progetto` senza ricanonicalizzare, eliminando la
        // finestra TOCTOU (un symlink modificato dopo non può più disallineare
        // radice ed entrypoint, che sono coerenti per costruzione).
        let typst_core::EntrypointRilevato { radice, entrypoint } =
            typst_core::rileva_entrypoint(Path::new(&percorso))?;

        // Legge il contenuto dell'entrypoint da mostrare subito nell'editor.
        let contenuto = std::fs::read_to_string(&entrypoint)
            .map_err(|e| format!("Entrypoint '{}' non leggibile: {e}", entrypoint.display()))?;

        // Entrypoint relativo alla radice (stessa logica di `nuovo_da_template`):
        // togli il prefisso della radice e ricomponi con `/` per `VirtualPath`.
        let relativo = entrypoint.strip_prefix(&radice).map_err(|_| {
            format!(
                "Entrypoint '{}' fuori dalla radice di progetto '{}'",
                entrypoint.display(),
                radice.display()
            )
        })?;

        // `VirtualPath` usa separatori `/`: unisci i soli componenti "normali".
        // C-8: `to_str()` (non `to_string_lossy`) così un componente non-UTF8 dà
        // errore esplicito, non un path corrotto.
        let mut segmenti: Vec<&str> = Vec::new();
        for componente in relativo.components() {
            match componente {
                Component::Normal(s) => {
                    let s = s
                        .to_str()
                        .ok_or_else(|| "Entrypoint con nome non valido (non UTF-8)".to_string())?;
                    segmenti.push(s);
                }
                // CurDir (`.`) ignorato; qualunque altro componente (`..`, radice,
                // prefisso) non deve comparire in un relativo dentro la radice.
                Component::CurDir => {}
                _ => {
                    return Err(format!(
                        "Percorso entrypoint relativo non valido: '{}'",
                        relativo.display()
                    ));
                }
            }
        }
        let entrypoint_relativo = segmenti.join("/");

        // C-8: percorso assoluto dell'entrypoint come stringa, con errore
        // esplicito se non è UTF-8 (invece di una stringa lossy/corrotta).
        let entrypoint_str = typst_core::albero::normalizza_verbatim(
            entrypoint
                .to_str()
                .ok_or_else(|| "Percorso entrypoint non valido (non UTF-8)".to_string())?,
        );

        // Aggancia il World al progetto (lock per il tempo minimo).
        {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.apri_progetto(radice, &entrypoint_relativo, &contenuto)?;
        }
        // Lock del World già rilasciato (mai annidare): il contenuto consegnato
        // all'editor è la base vera per `osserva_documento`.
        typst_core::sorveglianza::segna_consegnato(&stato, &entrypoint, contenuto.as_bytes());

        Ok(RisultatoProgetto {
            entrypoint: entrypoint_str,
            contenuto,
        })
    })
    .await
    .map_err(|e| format!("Apertura della cartella interrotta: {e}"))?
}

/// Indice dei titoli dell'ultimo documento compilato con successo (F5-020).
///
/// Non compila: copia l'indice conservato dal World (lock brevissimo). Dopo una
/// compilazione fallita è l'ultimo indice valido; vuoto finché non ce n'è una
/// riuscita. Il frontend lo richiede dopo ogni compilazione.
///
/// # Errori
/// `Err(String)` solo se il task viene interrotto.
#[tauri::command]
async fn indice_documento(
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Vec<VoceIndice>, String> {
    let mondo = Arc::clone(&mondo);
    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.indice_documento()
    })
    .await
    .map_err(|e| format!("Lettura dell'indice interrotta: {e}"))
}

/// Albero dei file della cartella-progetto agganciata, in sola consultazione
/// (F5-021). `None` in modalità documento singolo (nessuna radice).
///
/// La radice è quella del World, mai un percorso dato dal frontend: nulla può
/// far elencare altre cartelle. La lettura del disco avviene a lock rilasciato.
///
/// # Errori
/// `Err(String)` se la radice non è leggibile o il task viene interrotto.
#[tauri::command]
async fn albero_progetto(
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<Option<AlberoFile>, String> {
    let mondo = Arc::clone(&mondo);
    tauri::async_runtime::spawn_blocking(move || {
        let radice = {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.radice_progetto().map(Path::to_path_buf)
        };
        radice
            .map(|r| typst_core::albero::elenca_albero(&r))
            .transpose()
    })
    .await
    .map_err(|e| format!("Lettura dell'albero interrotta: {e}"))?
}

/// Apre come documento un file `.typ`/`.md` della cartella-progetto già
/// agganciata, mantenendone la radice (F5-020/F5-021: clic su un titolo
/// dell'indice o su un file dell'albero).
///
/// `relativo` è il percorso relativo alla radice, con `/`, come lo forniscono
/// indice e albero; il core lo risolve con la stessa barriera dei file
/// fratelli (niente `..`, niente link verso l'esterno). Il frontend deve aver
/// già messo al sicuro le modifiche non salvate: il World passa al nuovo file.
///
/// # Errori
/// `Err(String)` leggibile se non c'è un progetto, il percorso non è valido o
/// il file non è leggibile; in tal caso il World non cambia.
#[tauri::command]
async fn apri_file_progetto(
    relativo: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<RisultatoProgetto, String> {
    let mondo = Arc::clone(&mondo);
    let stato = Arc::clone(&stato);
    tauri::async_runtime::spawn_blocking(move || {
        let (percorso, contenuto) = {
            // W-001: recupero da lock avvelenato (vedi commento in `compila`).
            let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            guida.apri_file_del_progetto(&relativo)?
        };
        let entrypoint = typst_core::albero::normalizza_verbatim(
            percorso
                .to_str()
                .ok_or_else(|| "Percorso del file non valido (non UTF-8)".to_string())?,
        );
        // World rilasciato (mai annidare Condiviso dentro World): il contenuto
        // consegnato all'editor è la base vera per `osserva_documento`. Dopo la
        // validazione `to_str`: su errore non resta una voce stantia.
        typst_core::sorveglianza::segna_consegnato(&stato, &percorso, contenuto.as_bytes());
        Ok(RisultatoProgetto {
            entrypoint,
            contenuto,
        })
    })
    .await
    .map_err(|e| format!("Apertura del file interrotta: {e}"))?
}

/// Chiude la modalità progetto, riportando il World a "documento singolo,
/// nessun progetto" (Fase 4, gap noto: mancava un modo per uscire dalla
/// modalità progetto in vista di un futuro "Nuovo documento").
///
/// Nessun argomento applicativo: solo lo `State` del World. Delega a
/// `EditorWorld::chiudi_progetto`, che rimuove la radice di progetto e i font
/// specifici del progetto senza toccare il testo corrente dell'editor (si
/// perde solo la risoluzione dei percorsi relativi, non il contenuto). Il
/// lavoro (ricostruzione dello store dei font) avviene fuori dal thread UI
/// tramite `spawn_blocking`.
///
/// # Errori
/// Restituisce `Err(String)` solo se il task viene interrotto (abortito).
#[tauri::command]
async fn chiudi_progetto(mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>) -> Result<(), String> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);

    tauri::async_runtime::spawn_blocking(move || {
        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.chiudi_progetto();
    })
    .await
    .map_err(|e| format!("Chiusura del progetto interrotta: {e}"))
}

/// Scarica e pre-installa un pacchetto di Typst Universe nella cache (3.1).
///
/// Comando volutamente **stateless**: NON riceve né tocca lo stato gestito
/// (`Arc<Mutex<EditorWorld>>`). Il download (lungo, di rete) avverrebbe altrimenti
/// tenendo il lock del World e bloccherebbe `compila`/`completa`/`tooltip` per
/// decine di secondi. Delega a `typst_core::packages::installa`, che crea un
/// proprio downloader+`SystemPackages` sulla **stessa** cache standard `@preview`:
/// la successiva compilazione del World trova quindi i file già estratti, senza
/// alcun lock condiviso. Il lavoro bloccante (rete + estrazione tar.gz + I/O)
/// viene spostato fuori dal thread UI tramite `spawn_blocking`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `nome`: nome del pacchetto senza namespace (es. `"cetz"`).
/// - `versione`: versione esatta `"major.minor.patch"` (es. `"0.2.2"`).
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile se l'installazione
/// fallisce (versione non valida, pacchetto inesistente, rete, archivio
/// corrotto) o se il task viene interrotto.
#[tauri::command]
async fn installa_pacchetto(nome: String, versione: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || typst_core::packages::installa(&nome, &versione))
        .await
        .map_err(|e| format!("Installazione del pacchetto interrotta: {e}"))?
}

/// Pubblica il documento corrente come pacchetto `@local` riutilizzabile (Fase 4).
///
/// Comando volutamente **stateless**: come `installa_pacchetto`, NON riceve né
/// tocca lo stato gestito (`Arc<Mutex<EditorWorld>>`). Scrive su disco un
/// pacchetto Typst minimale (`lib.typ` + `typst.toml`) nella data dir standard,
/// sotto `<data_dir>/typst/packages/local/{nome}/{versione}/`: la stessa radice
/// `@local` che il World risolve già tramite `SystemPackages`. Dopo la
/// pubblicazione il documento è importabile come `@local/{nome}:{versione}` senza
/// alcuna modifica al World. Delega a `typst_core::packages::pubblica_local`,
/// scartando il `PathBuf` di ritorno (al frontend basta sapere se è andata a buon
/// fine). Il lavoro bloccante (I/O su disco) è spostato fuori dal thread UI
/// tramite `spawn_blocking`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `nome`: nome del pacchetto senza namespace (`^[a-z0-9-]+$`).
/// - `versione`: versione esatta `"major.minor.patch"`.
/// - `contenuto`: testo Typst del documento, salvato come `lib.typ` del pacchetto.
///
/// # Invalidazione cache (comemo)
/// Dopo una pubblicazione **riuscita** chiama `typst_core::invalida_cache()`: se
/// il documento corrente importava già `@local/{nome}:{versione}` e una
/// compilazione precedente era fallita perché il pacchetto non esisteva ancora,
/// `comemo` ha memoizzato quell'esito di risoluzione (`NotFound`) per il `FileId`.
/// Svuotare la cache forza la ricompilazione a ri-risolvere il `FileId` e a
/// trovare il pacchetto appena scritto su disco (vedi `invalida_cache`).
///
/// # Errori
/// Restituisce `Err(String)` con un messaggio leggibile se nome/versione non sono
/// validi, se il contenuto è troppo grande, se la data dir non è determinabile, se
/// la scrittura su disco fallisce, o se il task viene interrotto.
#[tauri::command]
async fn pubblica_local(nome: String, versione: String, contenuto: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Scarta il PathBuf: al frontend interessa solo l'esito (Ok/Err).
        typst_core::packages::pubblica_local(&nome, &versione, &contenuto).map(|_| ())?;

        // Pubblicazione riuscita: invalida la cache comemo così la prossima
        // compilazione ri-risolve il FileId del pacchetto @local appena creato
        // (altrimenti un eventuale `NotFound` memoizzato resterebbe stantio).
        typst_core::invalida_cache();
        Ok(())
    })
    .await
    .map_err(|e| format!("Pubblicazione del pacchetto @local interrotta: {e}"))?
}

/// Converte un sorgente Markdown in markup Typst equivalente (Fase 6).
///
/// Comando volutamente **stateless**: come `elenca_snippet`, NON riceve né
/// tocca lo stato gestito (`Arc<Mutex<EditorWorld>>`). La conversione è pura
/// (`typst_core::md_a_typst`) e non richiede il World; il lavoro (parsing
/// Markdown + generazione del markup) è comunque spostato fuori dal thread UI
/// tramite `spawn_blocking`, per coerenza con gli altri comandi.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Markdown da convertire.
///
/// # Errori
/// Restituisce `Err(String)` solo se il task viene interrotto (abortito): la
/// conversione stessa è infallibile ma **lossy** — i costrutti Markdown senza
/// equivalente Typst diretto sono resi al meglio (vedi `typst_core::markdown`).
#[tauri::command]
async fn converti_md_a_typst(
    sorgente: String,
    stato: tauri::State<'_, osservatore::Condiviso>,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<String, String> {
    let stato = Arc::clone(&stato);
    let mondo = Arc::clone(&mondo);
    tauri::async_runtime::spawn_blocking(move || {
        let opzioni = opzioni_md_del_documento(&stato, &mondo);
        typst_core::md_a_typst_con_opzioni(&sorgente, &opzioni)
    })
    .await
    .map_err(|e| format!("Conversione Markdown→Typst interrotta: {e}"))
}

/// Opzioni md→Typst per il documento aperto: il Typst grezzo `{=typst}` (che può
/// contenere `#read`/`#include`) vale solo per i `.md` dentro la radice del
/// progetto aperto; un file esterno, scaricato o non salvato lo vede come codice
/// letterale. I due lock (stato dell'osservazione, World) sono presi uno alla
/// volta, mai insieme.
fn opzioni_md_del_documento(
    stato: &osservatore::Condiviso,
    mondo: &Arc<Mutex<EditorWorld>>,
) -> typst_core::OpzioniMd {
    let documento = typst_core::sorveglianza::blocca(stato).doc.clone();
    let radice = {
        let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.radice_progetto().map(Path::to_path_buf)
    };
    typst_core::opzioni_md_per_documento(documento.as_deref(), radice.as_deref())
}

/// Converte un sorgente Typst in Markdown (CommonMark) equivalente (Fase 6).
///
/// Comando stateless, speculare a `converti_md_a_typst`. Delega a
/// `typst_core::typst_a_md`.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Typst da convertire.
///
/// # Errori
/// Restituisce `Err(String)` solo se il task viene interrotto (abortito): la
/// conversione stessa è infallibile ma **lossy** — i costrutti Typst senza
/// equivalente CommonMark diretto (funzioni, math, set/show, …) sono
/// riprodotti come testo grezzo (vedi `typst_core::markdown`).
#[tauri::command]
async fn converti_typst_a_md(sorgente: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || typst_core::typst_a_md(&sorgente))
        .await
        .map_err(|e| format!("Conversione Typst→Markdown interrotta: {e}"))
}

/// Anteprima dal vivo di un documento Markdown (Fase 6).
///
/// Converte il Markdown in Typst (`typst_core::md_a_typst`) e riusa la
/// pipeline di compilazione esistente esattamente come `compila`: aggiorna il
/// World persistente col Typst convertito e lo compila. Nessun secondo motore
/// di rendering (vedi DECISIONI.md D-006, "orchestrare, non duplicare").
///
/// # Limite noto (lossy)
/// I diagnostici restituiti si riferiscono al Typst **generato dalla
/// conversione**, non alle righe del `.md` originale (le posizioni non
/// corrispondono fra i due testi): il frontend non li mostra inline in
/// modalità Markdown, coerentemente con questo limite.
///
/// # Parametri (frontend → Rust via `invoke`)
/// - `sorgente`: testo Markdown corrente dell'editor.
/// - `mondo`: stato gestito da Tauri contenente il World persistente (Arc<Mutex>).
///
/// # Errori
/// Restituisce `Err(Vec<String>)` solo se la compilazione viene interrotta
/// (task abortito), come `compila`.
#[tauri::command]
async fn anteprima_markdown(
    sorgente: String,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, osservatore::Condiviso>,
) -> Result<RisultatoCompilazione, Vec<String>> {
    // Clona l'Arc per trasferire la proprietà nella closure Send + 'static.
    let mondo = Arc::clone(&mondo);
    let stato = Arc::clone(&stato);

    tauri::async_runtime::spawn_blocking(move || {
        // `{=typst}` solo per i .md del progetto (lock rilasciati prima di quello
        // di compilazione).
        let opzioni = opzioni_md_del_documento(&stato, &mondo);
        let typst = typst_core::md_a_typst_con_opzioni(&sorgente, &opzioni);

        // W-001: recupero da lock avvelenato (vedi commento in `compila`).
        let mut guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
        guida.aggiorna_sorgente(&typst);
        guida.compila()
    })
    .await
    .map_err(|e| vec![format!("Anteprima Markdown interrotta: {e}")])
}

/// Installa il panic hook che scrive `<config_dir>/typster/crash.log`.
///
/// Con `panic = "abort"` e sottosistema `windows` un panic non lascia altra
/// traccia: il messaggio, la posizione e l'ora UTC finiscono nel registro
/// (best effort, vedi [`typst_core::registra_crash`]) e poi si chiama l'hook
/// predefinito, che stampa su stderr come prima.
fn installa_panic_hook() {
    let predefinito = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let posizione = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "posizione sconosciuta".to_string());
        let carico = info.payload();
        let messaggio = carico
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| carico.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic senza messaggio".to_string());
        typst_core::registra_crash(&posizione, &messaggio);
        predefinito(info);
    }));
}

/// `true` se la finestra può navigare verso `url`.
///
/// Consentite solo le origini dell'app: `tauri://localhost` e
/// `http(s)://tauri.localhost` (produzione, per piattaforma), `about:blank` e,
/// in sviluppo, l'origine di `devUrl`. Tutto il resto (in particolare i link
/// `<a href>` http/https che `typst-svg` mette nell'anteprima) è bloccato:
/// navigare fuori dall'app perderebbe l'editor e il lavoro non salvato. Gli URL
/// bloccati **non** si aprono all'esterno.
fn navigazione_consentita(url: &tauri::Url, dev_url: Option<&tauri::Url>) -> bool {
    let origine = |u: &tauri::Url| {
        (
            u.scheme().to_string(),
            u.host_str().map(str::to_owned),
            u.port_or_known_default(),
        )
    };
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) | ("http" | "https", Some("tauri.localhost")) => true,
        ("about", None) => url.as_str() == "about:blank",
        _ => dev_url.is_some_and(|d| origine(d) == origine(url)),
    }
}

/// Plugin che applica [`navigazione_consentita`] a ogni navigazione della
/// finestra (difesa nel guscio; il frontend intercetta comunque i clic).
fn blocca_navigazione<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri::Manager;
    tauri::plugin::Builder::new("blocca-navigazione")
        .on_navigation(|webview, url| {
            // `devUrl` conta solo in sviluppo (`tauri dev`), mai in produzione.
            let dev_url = if tauri::is_dev() {
                webview.app_handle().config().build.dev_url.clone()
            } else {
                None
            };
            navigazione_consentita(url, dev_url.as_ref())
        })
        .build()
}

/// Costruisce e avvia l'applicazione Tauri.
///
/// Costruisce il World persistente **senza** scansionare i font (F5-007) e
/// avvia il loop degli eventi; la scansione dei font di sistema parte in un
/// thread di sfondo da `setup`, senza lock del World: una compilazione che
/// arrivi prima del termine attende lo store (vedi `FontPrecaricabili`).
/// Il World viene avvolto in `Arc<Mutex>` così il comando asincrono può
/// clonarne un handle da spostare nel thread bloccante.
/// Registra i comandi esposti al frontend.
/// In caso di errore irreversibile stampa il messaggio su stderr ed esce
/// con codice non-zero (evita `expect`/`panic` su percorso di produzione).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    installa_panic_hook();
    let mondo = EditorWorld::nuovo("");
    // Handle sui font, preso prima di consegnare il World al Mutex.
    let font = mondo.font_precaricabili();
    if let Err(e) = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(blocca_navigazione())
        .manage(Arc::new(Mutex::new(mondo)))
        .manage(osservatore::Condiviso::default())
        .setup(move |_app| {
            // F5-007: scansione dei font fuori dal percorso di avvio. Un
            // fallimento di `spawn` non è fatale: i font si costruiranno al
            // primo uso (prima compilazione).
            let _ = std::thread::Builder::new()
                .name("precarico-font".into())
                .spawn(move || font.precarica());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            compila,
            svg_pagina,
            apri_file,
            leggi_file,
            apri_cartella,
            apri_file_progetto,
            indice_documento,
            albero_progetto,
            chiudi_progetto,
            salva_file,
            esporta_pdf,
            esporta_immagini,
            esporta_testo,
            esporta_html,
            esporta_latex,
            esporta_markdown,
            completa,
            tooltip,
            jump_da_click,
            jump_da_cursore,
            elenca_snippet,
            elenca_modelli,
            compila_modello,
            salva_come_modello,
            elimina_modello,
            leggi_impostazioni,
            salva_impostazioni,
            elenca_pacchetti,
            aggiornamenti_pacchetti,
            installa_pacchetto,
            pubblica_local,
            nuovo_da_template,
            converti_md_a_typst,
            converti_typst_a_md,
            anteprima_markdown,
            osservatore::osserva_documento,
            osservatore::invalida_risorse,
            osservatore::riconosci_disco,
            osservatore::cronologia_elenca,
            osservatore::cronologia_leggi,
            osservatore::cronologia_registra,
            osservatore::ripristina_documento_sparito
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("Errore irreversibile all'avvio di Tauri: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod test_navigazione {
    use super::navigazione_consentita;
    use tauri::Url;

    fn u(s: &str) -> Url {
        Url::parse(s).expect("url di test")
    }

    #[test]
    fn origini_dell_app_consentite() {
        for s in [
            "tauri://localhost/",
            "tauri://localhost/index.html",
            "http://tauri.localhost/",
            "https://tauri.localhost/index.html",
            "about:blank",
        ] {
            assert!(navigazione_consentita(&u(s), None), "{s}");
        }
    }

    #[test]
    fn link_esterni_bloccati() {
        for s in [
            "https://example.com/",
            "http://example.com/x",
            "http://localhost:5173/",
            "https://tauri.localhost.evil.com/",
            "http://evil.com/tauri.localhost",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "data:text/html,x",
            "tauri://altro/",
        ] {
            assert!(!navigazione_consentita(&u(s), None), "{s}");
        }
    }

    #[test]
    fn dev_url_consentito_solo_se_indicato() {
        let dev = u("http://localhost:5173");
        assert!(navigazione_consentita(
            &u("http://localhost:5173/src/main.ts"),
            Some(&dev)
        ));
        assert!(!navigazione_consentita(
            &u("http://localhost:9999/"),
            Some(&dev)
        ));
        assert!(!navigazione_consentita(
            &u("https://example.com/"),
            Some(&dev)
        ));
    }
}
