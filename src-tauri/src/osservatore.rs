//! Comandi Tauri del controllo dei file modificati da fuori (F5-022), della
//! cronologia versioni (F5-013) e dell'invalidazione delle risorse (F5-014).
//!
//! Il guscio fa solo da tramite: la logica (watcher, batching, macchina a
//! stati, decisione, integrità, cronologia) vive in `typst_core` (moduli
//! `sorveglianza`, `esterni`, `cronologia`) ed è testata lì. Qui si emettono
//! gli eventi verso il frontend e si agisce sul `World`.
//!
//! Il lock del World si prende sempre **dopo** aver rilasciato lo stato
//! dell'osservazione (mai insieme): vedi `typst_core::sorveglianza`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use typst_core::cronologia::{ora_utc, Cronologia, EsitoRegistra, Versione};
use typst_core::esterni::{
    decidi, percorso_nella_forma_del_documento, stesso_file, Azione, Cambio, Integrita,
    RICONTROLLO_MS,
};
use typst_core::sorveglianza::{
    avvia_watcher, blocca, elabora, imposta_documento, ricontrolla, ripristina_sparito,
    Elaborazione, EsitoSparito,
};
use typst_core::EditorWorld;

pub use typst_core::sorveglianza::{scrivi_proprio, Condiviso, EsitoScrittura};

/// Nome dell'evento Tauri con cui il guscio avvisa il frontend.
const EVENTO_FRONTEND: &str = "modifica-esterna";

/// Messaggio verso il frontend (evento `modifica-esterna`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Uscita {
    /// `ricarica`, `chiedi`, `sparito`, `rinominato`, `invalida`, `albero`.
    pub azione: &'static str,
    /// Percorso interessato (per `rinominato` il vecchio).
    pub percorso: Option<String>,
    /// Nuovo percorso (solo `rinominato`).
    pub nuovo: Option<String>,
}

/// Percorso per il frontend, senza il prefisso verbatim `\\?\` di Windows.
fn testo(p: &Path) -> String {
    typst_core::albero::percorso_per_frontend(p)
}

fn uscita(azione: &'static str, percorso: Option<&Path>, nuovo: Option<&Path>) -> Uscita {
    Uscita {
        azione,
        percorso: percorso.map(testo),
        nuovo: nuovo.map(testo),
    }
}

/// Esito della traduzione di un batch di cambi in messaggi per il frontend.
#[derive(Debug, Default, PartialEq, Eq)]
struct Decisioni {
    /// Messaggi sul documento, nell'ordine dei cambi.
    uscite: Vec<Uscita>,
    /// Almeno un file fratello è cambiato: invalidare la cache del World.
    invalida: bool,
    /// L'albero dei file va riletto.
    albero: bool,
    /// Rinomine del documento da seguire in cronologia: `(vecchio, nuovo)`.
    rinomine: Vec<(PathBuf, PathBuf)>,
}

/// Percorso per un file nella grafia con cui il frontend ha aperto il documento.
fn nella_forma_del_doc(doc: Option<&Path>, p: &Path) -> PathBuf {
    match doc {
        Some(d) => percorso_nella_forma_del_documento(d, p, stesso_file),
        None => p.to_path_buf(),
    }
}

/// Logica pura di [`applica`]: decide cosa comunicare per i `cambi`.
///
/// I percorsi del **documento** (azioni `ricarica`, `chiedi`, `sparito`,
/// `rinominato`) escono nella forma di `doc`, cioè quella data dal frontend
/// (drive di rete mappato, `subst`, giunzione), non in quella canonica degli
/// eventi del watcher: il frontend li confronta in modo lessicale e altrimenti
/// li scarterebbe. Il nuovo nome di una rinomina è riscritto sotto la stessa
/// forma ([`percorso_nella_forma_del_documento`]).
fn decisioni(doc: Option<&Path>, non_salvato: bool, cambi: &[Cambio]) -> Decisioni {
    let mut out = Decisioni::default();
    for cambio in cambi {
        for azione in decidi(doc, non_salvato, cambio) {
            match (azione, cambio) {
                (Azione::Ricarica, Cambio::Contenuto(p) | Cambio::Rinominato { a: p, .. }) => {
                    let p = nella_forma_del_doc(doc, p);
                    out.uscite.push(uscita("ricarica", Some(&p), None));
                }
                (Azione::Chiedi, Cambio::Contenuto(p) | Cambio::Rinominato { a: p, .. }) => {
                    let p = nella_forma_del_doc(doc, p);
                    out.uscite.push(uscita("chiedi", Some(&p), None));
                }
                (Azione::DocumentoSparito, Cambio::Sparito(p)) => {
                    let p = nella_forma_del_doc(doc, p);
                    out.uscite.push(uscita("sparito", Some(&p), None));
                }
                (Azione::SeguiRinomina { nuovo }, Cambio::Rinominato { da, .. }) => {
                    let da = nella_forma_del_doc(doc, da);
                    let nuovo = nella_forma_del_doc(doc, &nuovo);
                    out.uscite
                        .push(uscita("rinominato", Some(&da), Some(&nuovo)));
                    out.rinomine.push((da, nuovo));
                }
                (Azione::InvalidaRisorse, _) => out.invalida = true,
                (Azione::AggiornaAlbero, _) => out.albero = true,
                _ => {}
            }
        }
    }
    out
}

/// Applica le decisioni per i cambi: invalida la cache dei file, aggiorna
/// cronologia e stato per le rinomine seguite, ed emette gli eventi al frontend.
fn applica<R: Runtime>(app: &AppHandle<R>, condiviso: &Condiviso, elaborazione: Elaborazione) {
    // Stato del documento al momento della decisione (preso dal core nella stessa
    // sezione critica dell'elaborazione: una rinomina seguita ha già aggiornato
    // `Stato::doc`).
    let decisi = decisioni(
        elaborazione.doc.as_deref(),
        elaborazione.non_salvato,
        &elaborazione.cambi,
    );
    let Decisioni {
        mut uscite,
        invalida,
        albero,
        rinomine,
    } = decisi;
    let albero = albero || elaborazione.albero;
    // La cronologia segue il file (D-014, ri-chiavatura); il core ha già spostato
    // `Stato::doc` sul nuovo percorso.
    for (da, nuovo) in &rinomine {
        if let Some(c) = Cronologia::nella_cartella_dati() {
            if let Err(e) = c.rinomina(da, nuovo) {
                eprintln!("Cronologia non riassociata dopo la rinomina: {e}");
            }
        }
    }

    if invalida {
        // Il lock del World si prende a stato dell'osservazione rilasciato.
        let mondo = app.state::<Arc<Mutex<EditorWorld>>>();
        mondo
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .invalida_risorse();
        uscite.push(uscita("invalida", None, None));
    }
    if albero {
        uscite.push(uscita("albero", None, None));
    }
    for u in uscite {
        if let Err(e) = app.emit(EVENTO_FRONTEND, u) {
            eprintln!("Evento verso il frontend non emesso: {e}");
        }
    }

    // Ricontrolli dei file svuotati/spariti: fra RICONTROLLO_MS si rilegge il
    // disco e decide solo l'ultimo evento (generazione).
    for (percorso, generazione) in elaborazione.ricontrolli {
        let app = app.clone();
        let condiviso = Arc::clone(condiviso);
        let avvio = std::thread::Builder::new()
            .name("ricontrollo-file".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_millis(RICONTROLLO_MS));
                let el = ricontrolla(&mut blocca(&condiviso), &percorso, generazione);
                if !el.cambi.is_empty() || !el.ricontrolli.is_empty() {
                    applica(&app, &condiviso, el);
                }
            });
        if let Err(e) = avvio {
            eprintln!("Ricontrollo del file non avviato: {e}");
        }
    }
}

/// Comunica all'osservazione il documento aperto e se ha modifiche non salvate.
///
/// Da chiamare a ogni cambio di stato del file (apertura, salvataggio,
/// modifica). Ricava la cartella da osservare dal World (radice del progetto,
/// che può stare sopra l'entrypoint) o, in mancanza, dalla cartella del
/// documento; la cambia (riavviando il watcher) solo se è diversa. Ritorna
/// `true` se un watcher è attivo: se `false` il frontend ripiega
/// sull'invalidazione al focus della finestra (F5-014).
#[tauri::command]
pub(crate) async fn osserva_documento<R: Runtime>(
    percorso: Option<String>,
    non_salvato: bool,
    app: AppHandle<R>,
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
    stato: tauri::State<'_, Condiviso>,
) -> Result<bool, String> {
    let mondo = Arc::clone(&mondo);
    let condiviso = Arc::clone(&stato);
    tauri::async_runtime::spawn_blocking(move || {
        // Un solo lock del World: radice e handle dei file letti dalla
        // compilazione (filtro delle invalidazioni, rilasciato prima di
        // prendere lo stato dell'osservazione).
        let (radice_mondo, letti) = {
            let guida = mondo.lock().unwrap_or_else(|e| e.into_inner());
            (
                guida.radice_progetto().map(Path::to_path_buf),
                guida.risorse_lette(),
            )
        };
        let doc = percorso.map(PathBuf::from);
        let radice =
            radice_mondo.or_else(|| doc.as_deref().and_then(Path::parent).map(Path::to_path_buf));

        let mut guardia = blocca(&condiviso);
        guardia.letti = Some(letti);
        let cond2 = Arc::clone(&condiviso);
        imposta_documento(&mut guardia, doc, non_salvato, radice, |r| {
            let app2 = app.clone();
            avvia_watcher(r, move |eventi| {
                let el = elabora(&mut blocca(&cond2), &eventi);
                applica(&app2, &cond2, el);
            })
        })
    })
    .await
    .map_err(|e| format!("Osservazione interrotta: {e}"))
}

/// Invalida la cache dei file fratelli del World (comando "Ricarica risorse",
/// ripiego al focus della finestra; F5-014). Il frontend ricompila dopo.
#[tauri::command]
pub(crate) async fn invalida_risorse(
    mondo: tauri::State<'_, Arc<Mutex<EditorWorld>>>,
) -> Result<(), String> {
    let mondo = Arc::clone(&mondo);
    tauri::async_runtime::spawn_blocking(move || {
        mondo
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .invalida_risorse();
    })
    .await
    .map_err(|e| format!("Invalidazione interrotta: {e}"))
}

/// L'utente ha scelto di tenere la versione dell'editor davanti a una modifica
/// esterna: lo stato attuale del disco diventa il riferimento visto, così il
/// prossimo salvataggio lo sovrascrive senza altri conflitti.
#[tauri::command]
pub(crate) async fn riconosci_disco(
    percorso: String,
    stato: tauri::State<'_, Condiviso>,
) -> Result<(), String> {
    let condiviso = Arc::clone(&stato);
    tauri::async_runtime::spawn_blocking(move || {
        let cronologia = Cronologia::nella_cartella_dati();
        let max = typst_core::config::carica_impostazioni().cronologia_max_versioni as usize;
        typst_core::sorveglianza::riconosci_disco(
            &condiviso,
            Path::new(&percorso),
            cronologia.as_ref(),
            max,
        )
        .map_err(|e| format!("Stato del file non verificabile: {e}"))
    })
    .await
    .map_err(|e| format!("Verifica interrotta: {e}"))?
}

/// Elenco delle versioni del file, dalla più recente.
#[tauri::command]
pub(crate) async fn cronologia_elenca(percorso: String) -> Result<Vec<Versione>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Cronologia::nella_cartella_dati()
            .map(|c| c.elenca(Path::new(&percorso)))
            .unwrap_or_default()
    })
    .await
    .map_err(|e| format!("Lettura della cronologia interrotta: {e}"))
}

/// Testo della versione `id` del file.
#[tauri::command]
pub(crate) async fn cronologia_leggi(percorso: String, id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Cronologia::nella_cartella_dati()
            .ok_or_else(|| "cartella dati non determinabile".to_string())?
            .leggi(Path::new(&percorso), &id)
            .map_err(|e| format!("Versione non leggibile: {e}"))
    })
    .await
    .map_err(|e| format!("Lettura della cronologia interrotta: {e}"))?
}

/// Registra `contenuto` come versione del file, senza scrivere il file: da
/// chiamare **prima** di sostituire il testo dell'editor (ripristino di una
/// versione, ricarica dal disco) così le modifiche non salvate restano
/// recuperabili. Ritorna `true` se è stata creata una versione.
#[tauri::command]
pub(crate) async fn cronologia_registra(
    percorso: String,
    contenuto: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cronologia = Cronologia::nella_cartella_dati()
            .ok_or_else(|| "cartella dati non determinabile".to_string())?;
        let max = typst_core::config::carica_impostazioni().cronologia_max_versioni as usize;
        cronologia
            .registra(Path::new(&percorso), &contenuto, ora_utc(), max, None)
            .map(|e| matches!(e, EsitoRegistra::Creata { .. }))
            .map_err(|e| format!("Cronologia non aggiornata: {e}"))
    })
    .await
    .map_err(|e| format!("Scrittura della cronologia interrotta: {e}"))?
}

/// Esito del ripristino di un documento sparito, per il frontend.
#[derive(Debug, Clone, Serialize)]
pub struct EsitoRipristinoDoc {
    /// `ripristinato`, `esiste` (il file è tornato: nulla da fare),
    /// `cartella-assente` (la cartella del documento non c'è più: nulla è
    /// stato scritto), `vuoto`, `non-utf8` o `troncato` (testo non integro:
    /// nessun salvataggio).
    pub esito: &'static str,
}

/// Ripristina al percorso originale il documento sparito dal disco, se il testo
/// dell'editor è integro (D-014): voce in cronologia, poi scrittura atomica
/// registrata come propria. Non scrive mai se il file esiste di nuovo.
#[tauri::command]
pub(crate) async fn ripristina_documento_sparito(
    percorso: String,
    contenuto: String,
    stato: tauri::State<'_, Condiviso>,
) -> Result<EsitoRipristinoDoc, String> {
    let condiviso = Arc::clone(&stato);
    tauri::async_runtime::spawn_blocking(move || {
        let cronologia = Cronologia::nella_cartella_dati();
        let max = typst_core::config::carica_impostazioni().cronologia_max_versioni as usize;
        let esito = ripristina_sparito(
            &condiviso,
            Path::new(&percorso),
            &contenuto,
            cronologia.as_ref(),
            max,
        )
        .map_err(|e| format!("Ripristino del file '{percorso}' non riuscito: {e}"))?;
        Ok(EsitoRipristinoDoc {
            esito: match esito {
                EsitoSparito::Ripristinato => "ripristinato",
                EsitoSparito::Esiste => "esiste",
                EsitoSparito::CartellaAssente => "cartella-assente",
                EsitoSparito::NonIntegro(Integrita::Vuoto) => "vuoto",
                EsitoSparito::NonIntegro(Integrita::NonUtf8) => "non-utf8",
                EsitoSparito::NonIntegro(_) => "troncato",
            },
        })
    })
    .await
    .map_err(|e| format!("Ripristino interrotto: {e}"))?
}

#[cfg(test)]
mod test_percorsi_emessi {
    use super::*;

    /// Cartella temporanea canonica, rimossa al drop.
    struct Cartella(PathBuf);

    impl Cartella {
        fn nuova(nome: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("typster-{nome}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("sub")).expect("cartella di prova");
            Self(std::fs::canonicalize(&dir).expect("canonicalize"))
        }
    }

    impl Drop for Cartella {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Il frontend conosce il file con un'altra grafia dello stesso percorso
    /// (stand-in di drive mappato / `subst` / giunzione): senza il prefisso
    /// verbatim `\\?\` del watcher (su Windows) e, su ogni piattaforma, passando
    /// da `sub/..` (cartella creata in [`Cartella::nuova`]). `testo()` toglie già
    /// il prefisso verbatim, quindi solo la forma con `..` è davvero diversa dopo
    /// la conversione in stringa: lessicalmente diversa, risolta dal filesystem.
    fn grafia_frontend(radice: &Path, nome: &str) -> PathBuf {
        let g = PathBuf::from(typst_core::albero::percorso_per_frontend(radice))
            .join("sub")
            .join("..")
            .join(nome);
        // Confronto fra `PathBuf` PRIMA di `testo()`.
        assert_ne!(g, radice.join(nome), "la grafia deve differire");
        assert_ne!(
            testo(&g),
            testo(&radice.join(nome)),
            "la grafia deve differire anche come testo emesso"
        );
        g
    }

    #[test]
    fn le_azioni_sul_documento_escono_nella_forma_del_frontend() {
        let c = Cartella::nuova("azioni");
        let canonico = c.0.join("main.typ");
        std::fs::write(&canonico, "x").expect("fixture");
        let doc = grafia_frontend(&c.0, "main.typ");
        for (non_salvato, azione) in [(false, "ricarica"), (true, "chiedi")] {
            let d = decisioni(
                Some(&doc),
                non_salvato,
                &[Cambio::Contenuto(canonico.clone())],
            );
            assert_eq!(d.uscite, vec![uscita(azione, Some(&doc), None)], "{azione}");
        }
        let d = decisioni(Some(&doc), false, &[Cambio::Sparito(canonico.clone())]);
        assert_eq!(d.uscite, vec![uscita("sparito", Some(&doc), None)]);
        assert!(d.albero);
    }

    #[test]
    fn la_rinomina_esce_con_vecchio_e_nuovo_nella_forma_del_frontend() {
        let c = Cartella::nuova("rinomina");
        let doc = grafia_frontend(&c.0, "main.typ");
        let da = c.0.join("main.typ");
        let a = c.0.join("nuovo.typ");
        std::fs::write(&a, "x").expect("fixture");
        let d = decisioni(
            Some(&doc),
            false,
            &[Cambio::Rinominato {
                da: da.clone(),
                a: a.clone(),
            }],
        );
        let nuovo_fe = grafia_frontend(&c.0, "nuovo.typ");
        assert_eq!(
            d.uscite,
            vec![uscita("rinominato", Some(&doc), Some(&nuovo_fe))]
        );
        assert_eq!(d.rinomine, vec![(doc, nuovo_fe)]);
        assert!(d.albero);
    }

    #[test]
    fn i_file_fratelli_non_portano_percorsi() {
        let c = Cartella::nuova("fratelli");
        let doc = grafia_frontend(&c.0, "main.typ");
        let d = decisioni(Some(&doc), false, &[Cambio::Contenuto(c.0.join("img.png"))]);
        assert!(d.uscite.is_empty() && d.invalida && !d.albero);
    }
}
