//! Voce «Nuova finestra» nella Jump List di Windows.
//!
//! La Jump List è il menu che appare col clic destro sull'icona dell'app nella
//! barra delle applicazioni. Qui si registra una sola attività che avvia un
//! nuovo processo dell'eseguibile corrente, senza argomenti.
//!
//! Tutto è «best effort»: ogni errore è ignorato. Se la Jump List non si può
//! creare, l'app parte lo stesso e la voce manca soltanto.
//!
//! Il modulo esiste solo su Windows (`#[cfg(windows)]` in `lib.rs`).

use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantChangeType, PROPVARIANT, PVCHF_DEFAULT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW,
    SetCurrentProcessExplicitAppUserModelID, ShellLink,
};

/// Identificatore dell'app: lo stesso `identifier` di `tauri.conf.json`.
const APP_ID: &str = "it.nexodux.typster";

/// Testo della voce nel menu.
const TITOLO: &str = "Nuova finestra";

/// Imposta l'AppUserModelID del processo e crea la Jump List.
///
/// Da chiamare all'inizio di `run()`, **prima** di creare le finestre: l'ID
/// deve essere impostato prima che compaia la prima finestra. L'ID si imposta
/// in modo sincrono; la Jump List si crea in un thread breve a parte, per non
/// toccare l'apartment COM del thread principale (usato da WebView2).
/// Ogni errore è ignorato.
pub fn imposta() {
    // SAFETY: chiamata Win32 con un puntatore a stringa valido per tutta la
    // durata della chiamata (`HSTRING` vive fino a fine istruzione).
    let _ = unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID)) };
    // Il thread non si attende: se non parte, la voce manca soltanto.
    let _ = std::thread::Builder::new()
        .name("jumplist".into())
        .spawn(nel_thread);
}

/// Corpo del thread: COM proprio, creazione della lista, rilascio COM.
fn nel_thread() {
    // SAFETY: inizializzazione COM propria del thread; `CoUninitialize` è
    // chiamata una sola volta, solo se `CoInitializeEx` è riuscita, e dopo il
    // ritorno di `crea_lista` (quindi dopo il rilascio di tutte le interfacce).
    unsafe {
        if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
            return;
        }
        let _ = crea_lista();
        CoUninitialize();
    }
}

/// Crea e registra la Jump List con la sola attività «Nuova finestra».
///
/// # Safety
/// Va chiamata da un thread con COM inizializzato. Le interfacce COM create
/// qui sono possedute da questa funzione e rilasciate al ritorno (`Drop`).
unsafe fn crea_lista() -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|_| windows::core::Error::from(E_FAIL))?;
    let percorso = HSTRING::from(exe.as_os_str());

    // Attività: collegamento all'exe, senza argomenti, icona dell'exe (indice 0).
    let collegamento: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
    collegamento.SetPath(&percorso)?;
    collegamento.SetIconLocation(&percorso, 0)?;

    // Il titolo si scrive nelle proprietà del collegamento (`PKEY_Title`).
    let proprieta: IPropertyStore = collegamento.cast()?;
    proprieta.SetValue(&PKEY_Title, &titolo_come_lpwstr()?)?;
    proprieta.Commit()?;

    let attivita: IObjectCollection =
        CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
    attivita.AddObject(&collegamento)?;

    let lista: ICustomDestinationList =
        CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
    lista.SetAppID(&HSTRING::from(APP_ID))?;
    let mut posti = 0u32;
    // Elenco delle destinazioni rimosse: non serve, ma `BeginList` lo richiede.
    let _rimosse: IObjectArray = lista.BeginList(&mut posti)?;
    lista.AddUserTasks(&*attivita)?;
    lista.CommitList()
}

/// Titolo come `PROPVARIANT` di tipo `VT_LPWSTR`, quello atteso da `PKEY_Title`.
///
/// `PROPVARIANT::from(&str)` produce un `VT_BSTR`: lo si converte.
///
/// # Safety
/// Chiama `PropVariantChangeType` con due `PROPVARIANT` validi e distinti.
unsafe fn titolo_come_lpwstr() -> windows::core::Result<PROPVARIANT> {
    let origine = PROPVARIANT::from(TITOLO);
    let mut destinazione = PROPVARIANT::default();
    PropVariantChangeType(&mut destinazione, &origine, PVCHF_DEFAULT, VT_LPWSTR)?;
    Ok(destinazione)
}
