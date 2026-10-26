# Log decisioni di design e bug noti

> Registro cronologico delle decisioni tecniche e dei bug noti, come richiesto da
> [PROGETTO.md](PROGETTO.md) (§2 "Iterazione documentata"). Vedi
> [PIANO.md](PIANO.md) per il piano complessivo.

---

## Decisioni

### D-001 — Stack tecnologico (Fase 0)
**Data:** 2026-06-28
**Decisione:** Opzione A — Tauri 2 + compilatore Typst embeddato come crate.
Architettura ibrida: crate Typst embeddati per compilazione/anteprima/sync;
Tinymist come *sidecar* LSP a partire dalla Fase 2 (non linkato come crate).
**Motivazione:** massima fluidità dell'anteprima e un solo artefatto leggero
(coerente con §6 di PROGETTO.md e con le convenzioni di progetto). Tinymist come
sidecar evita di importare il suo sistema di `cargo patch` nel workspace.
> *Nota (2026-09-28):* **superata da D-010 per la parte Tinymist** (Tinymist
> non è più sidecar dello stack di base, ma extra caricabile opzionale).

### D-002 — Versioni bloccate (Fase 0)
**Data:** 2026-06-28
**Decisione:** sottoinsieme Typst pinnato a `=0.15.0`. In uso dalla Fase 0:
`typst`, `typst-layout` (fornisce `PagedDocument`, non riesportato da `typst`),
`typst-kit` (feature reali: `embedded-fonts`, `scan-fonts`, `system-packages`,
`universe-packages`, `system-downloader`, `datetime`), `typst-svg`; `comemo` 0.4.
Pinnati ma introdotti più avanti: `typst-pdf` (Fase 1), `typst-render` (Fase 1/5),
`typst-ide` (Fase 2/4). Toolchain Rust 1.96.0 bloccata in `rust-toolchain.toml`
(con componenti `rustfmt`, `clippy`).
**Motivazione:** il versioning dei crate Typst è sincronizzato; il pin esatto
evita derive del linguaggio non volute. La versione di Tinymist (Fase 2) andrà
allineata a questo livello di linguaggio (vedi rischio in PIANO.md §7).

### D-003 — Confine core/presentazione (Fase 0)
**Data:** 2026-06-28
**Decisione:** crate `typst-core` puro, senza alcuna dipendenza da `tauri`.
Il guscio `src-tauri` e il frontend dipendono da `typst-core`, mai il contrario.
**Motivazione:** materializza la separazione richiesta da §5 di PROGETTO.md e
rende il core testabile headless.

---

## Ambiente di sviluppo accertato (2026-06-28)

| Componente | Versione |
|------------|----------|
| Rust (rustc/cargo) | 1.96.0 (toolchain `stable-x86_64-pc-windows-msvc`) |
| Linker C++ | MSVC `link.exe` 14.52.x (VS Community 2026) + Windows SDK 10.0.28000 |
| Node / npm | 24.16.0 / 11.13.0 |
| WebView2 runtime | 149.x |

> **Nota (2026-07-16):** questa tabella NON vale per la copia di lavoro
> corrente, arrivata senza toolchain Rust/MSVC — vedi **B-004**: qui core E
> guscio si compilano su toolchain **GNU** col bundle portabile
> `code_tools\rust_bundle` (+ `CARGO_TARGET_DIR` senza spazi); MSVC non è
> necessario.

---

## Esiti revisione Fase 0 (2026-06-28)

Revisione `code-reviewer` sul walking skeleton. Correzioni **applicate**:
CSP restrittiva (blocco script inline nell'SVG iniettato), `file()` del main che
restituisce byte UTF-8 espliciti (`Bytes::new`), `comemo::evict(30)` spostato dopo
il rendering, componenti toolchain, gestione errore d'avvio Tauri non-panic,
limite chunk Vite al default, attributi di accessibilità, `[profile.release]`
predisposto (commentato).

**Carry-over alla Fase 1** (non risolti in Fase 0, per scelta di scope):
- **W-001 — `World` persistente.** Oggi `compila_in_svg` ricrea `EditorWorld` a
  ogni chiamata, riscansionando i font di sistema e azzerando il beneficio di
  `comemo`. In Fase 1 va tenuto come `tauri::State<Mutex<EditorWorld>>` e
  aggiornato solo nel sorgente (`aggiorna_sorgente`, già presente). Cfr.
  PIANO.md §4 Fase 1.
- **W-002 — Warning non esposti.** I `SourceDiagnostic` di tipo `Warning` sono
  scartati nel ramo `Ok`. La Fase 1 (diagnostica inline) estenderà il contratto
  del comando `compila` per restituire anche gli avvisi. *(**Risolto in F1-007**:
  `compila` restituisce errori e avvisi con posizione.)*

---

## Fase 1 — avanzamento (2026-06-28)

### F1-001 — Compilazione automatica (anteprima dal vivo)
**Fatto.** Il frontend ricompila da solo con debounce ~300 ms sull'evento di
modifica del documento, con coalescing anti-sovrapposizione (`inCorso`/
`richiestaPendente`) e prima compilazione automatica su `window load`. Bottone
"Compila" mantenuto come trigger manuale immediato. *(La sorgente dell'evento è
passata da `input` della textarea a `EditorView.updateListener` con CodeMirror —
vedi F1-005.)*

### F1-002 — `World` persistente *(risolve W-001)*
**Fatto.** `EditorWorld` è ora costruito una sola volta all'avvio e tenuto in
`tauri::State<Arc<Mutex<EditorWorld>>>`; ogni compilazione aggiorna solo il
sorgente (`aggiorna_sorgente` → `Source::replace`, parsing incrementale). I font
di sistema non vengono più riscansionati a ogni battitura.

### F1-003 — Compilazione fuori dal thread UI
**Fatto.** Il comando `compila` è `async` e svolge il lavoro pesante in
`tauri::async_runtime::spawn_blocking`, così il thread principale resta reattivo
durante l'anteprima dal vivo. Recupero da lock avvelenato con `into_inner` per non
"murare" l'editor dopo un eventuale panic di compilazione.
> *Nota (2026-09-28):* con `panic = "abort"` nel profilo release (**D-013**,
> F5-015) questo recupero è irraggiungibile in release; resta valido in
> debug/test.

### F1-004 — Anteprima come SVG inline + CSP *(decisione)*
**Decisione:** l'SVG renderizzato è inserito nel DOM come **markup inline**
(`innerHTML`), non come `<img data:>`. La sicurezza è garantita dalla CSP
(`script-src 'self'` senza `unsafe-inline`): gli script iniettati via `innerHTML`
non vengono eseguiti (spec HTML) e gli handler inline `on*` sono bloccati dalla
CSP. L'SVG inline è inoltre **necessario** per la futura sincronizzazione
click-anteprima → cursore (Fase 4, `typst-ide::jump_from_click`), che richiede
accesso ai nodi SVG nel DOM.

### F1-005 — Editor CodeMirror 6 (sostituzione textarea)
**Data:** 2026-06-28
**Fatto.** La `<textarea>` è sostituita da un editor **CodeMirror 6** (`basicSetup`:
numeri di riga, history, chiusura parentesi, auto-indent, ricerca, fold) con tema
`oneDark` e a capo automatico. Il setup è isolato in `src/editor.ts`, che espone
un'interfaccia `Editor` minima (`testo`/`imposta`/`focus`): `main.ts` non conosce
CodeMirror — la separazione presentazione resta netta. La sorgente dell'evento di
ricompilazione è ora `EditorView.updateListener` (`docChanged`); debounce e
coalescing restano in `main.ts`.

**Evidenziazione Typst (decisione).** Grammatica da `codemirror-lang-typst`
(sperimentale, WASM su `typst-syntax`). Per non legare l'editor a una libreria
instabile (rischio PIANO.md §7) è caricata con **import dinamico in `try/catch`**
dentro un `Compartment` locale all'istanza: se import o init WASM falliscono,
l'editor resta pienamente usabile con l'evidenziazione di default (principio "un
errore non blocca l'editor"). Il chunk WASM è separato e caricato on-demand
(verificato dal build). *Da confermare a runtime nella webview Tauri.*

**CSP.** Aggiunto `'wasm-unsafe-eval'` a `script-src` in `tauri.conf.json` per
consentire l'instanziazione del WASM. È il minimo necessario (non reintroduce
`unsafe-inline`); il codice WASM proviene solo dal bundle Vite self-hosted, non da
remoto. Richiesto da `codemirror-lang-typst`, non da Tauri.
*(Aggiornamento 2026-07-18: la nota "non reintroduce `unsafe-inline`" resta vera per
`script-src`. In build di produzione, però, gli stili che CodeMirror inietta a runtime
richiedono una gestione a parte di `style-src` — vedi B-005.)*

**Build.** Aggiunti i plugin dev `vite-plugin-wasm` e `vite-plugin-top-level-await`
(l'import ESM del `.wasm` target `bundler` non è gestito da Vite nativamente).
`@codemirror/state` dichiarato come dipendenza diretta (importato in `editor.ts`).

### F1-006 — Export PDF e apertura/salvataggio file
**Data:** 2026-06-28
**Fatto.**
- **Core (`typst-core`):** estratto `compila_documento` privato condiviso; nuovo
  `esporta_pdf() -> Result<Vec<u8>, Vec<String>>` via `typst_pdf::pdf` con
  `PdfOptions::default()`. `comemo::evict(30)` resta nei chiamanti. Test
  `esporta_pdf_sorgente_semplice` (verifica intestazione `%PDF-`).
- **Guscio (`src-tauri`):** plugin `tauri-plugin-dialog` registrato; comandi async
  `apri_file`/`salva_file`/`esporta_pdf` (tutti `spawn_blocking`, recupero da lock
  avvelenato). La lettura/scrittura su disco la fa **Rust con `std::fs`**: il plugin
  `dialog` serve solo a scegliere il percorso, quindi **niente plugin `fs`**.
  Capabilities: `dialog:allow-open`, `dialog:allow-save`.
- **Frontend:** modulo `src/fileops.ts` (apri/salva/salva-con-nome/esporta, stato
  percorso + flag "modificato"; non tocca il DOM, usa callback). `main.ts`: pulsanti
  Apri/Salva/Esporta, indicatore file in header e `document.title`, scorciatoie
  Ctrl+O/S/E, feedback ed anti doppio-click sull'export.

**Decisione (architettura I/O).** Percorso scelto via dialog lato JS, I/O eseguita
dai comandi Rust (FS pieno, fidato). Coerente con la separazione core/presentazione
e con la trasparenza dei dati (i progetti restano `.typ` + risorse).

**Limiti noti (accettati per la Fase 1, da documentare per le fasi successive):**
- **Percorsi relativi:** il World compila sempre come `main.typ` virtuale senza
  radice su disco, quindi `#include`/`#image` con percorso relativo al file aperto
  non sono risolti (previsto in Fase 4, multi-file/risorse).
- **PDF da testo corrente:** l'export usa il testo dell'editor al momento del click,
  non l'ultima anteprima mostrata (possono divergere se il debounce non è scattato).
- **`Mutex` (non `RwLock`):** `compila`/`esporta_pdf` serializzano sullo stesso lock;
  ok per documento singolo, da rivedere se servirà concorrenza (es. lint+render).
- **`datetime.today()` congelato:** `Time::system()` è catturato alla costruzione del
  World persistente; sessioni lunghe a cavallo di mezzanotte mostrano la data d'avvio.

### F1-007 — Diagnostica inline (risolve W-002 / F1-R3)
**Data:** 2026-06-28
**Fatto.** Errori E avvisi del compilatore mostrati sulle righe dell'editor.
- **Contratto:** il comando `compila` ora restituisce `RisultatoCompilazione
  { pagine: Vec<String>, diagnostici: Vec<Diagnostico> }` (sempre `Ok`; l'`Err`
  resta solo per task abortito). `Diagnostico { gravita: "errore"|"avviso",
  messaggio, da: Option<usize>, a: Option<usize> }` con offset **UTF-16**.
- **Core (`typst-core`):** `compila()` raccoglie i `warnings` (sempre) + gli errori;
  *(nota 2026-10-14: la scelta resta valida; per gli span altrui il core ora usa il
  punto di chiamata nel main dalla traccia, vedi «Posizione degli errori dalla
  traccia 2026-10-14»)* helper `diagnostico_da` risolve la posizione via `WorldExt::range` SOLO se lo span
  appartiene alla sorgente principale (`d.span.id() == Some(id_principale)`) — gli
  span dei pacchetti `@preview` resterebbero altrimenti mappati su offset errati —
  e converte byte→UTF-16 con `Source::lines().byte_to_utf16`. Hints accodati al
  messaggio. Aggiunta dipendenza `serde` al crate (resta puro da Tauri).
- **Frontend:** `editor.ts` aggiunge `lintGutter()` + `mostraDiagnostici` (mappa a
  `@codemirror/lint`, **clampa** gli offset alla lunghezza del doc per evitare il
  `RangeError` noto, mostra inline solo i diagnostici posizionati). `main.ts`
  aggiorna l'anteprima solo se `pagine` non è vuoto (errore fatale → resta l'ultima
  valida), mette nel riquadro inferiore i diagnostici senza posizione (es. da
  pacchetti), e conta errori/avvisi nello stato.
- **Verifica:** `cargo test -p typst-core` (incl. test posizione: `#non_esiste`
  → `da=5, a=15`), `cargo check`, clippy `--all-targets -D warnings`, `tsc`+Vite:
  tutti verdi.

### Refinement ancora aperti per la Fase 1
- **F1-R1 — Scan font all'avvio sul thread principale.** `EditorWorld::nuovo("")`
  in `run()` scansiona i font di sistema prima di mostrare la finestra (ritardo
  d'avvio una-tantum). Valutare costruzione in `setup`/thread o init lazy.
- **F1-R2 — Streaming pagina-per-pagina via `Channel`.** Per documenti grandi,
  rilasciare il lock e inviare gli SVG via `Channel<String>` (PIANO.md §4).
- **F1-R3 — Diagnostica inline (W-002).** ✅ **Fatto** in F1-007.
- **F1-R4 — Sincronizzazione parser WASM su "apri file".** `Editor.imposta` usa una
  transazione (non `setState`) per restare sincronizzato col parser Typst; quando
  si implementerà l'apertura file con documenti grandi, verificare il caso di
  sostituzione totale (eventuale `clearParser`).
- **F1-R5 — Diagnostica/feedback aggiuntivi.** Test per `esporta_pdf` su sorgente
  non compilante; eventuale sanitizzazione SVG (DOMPurify) se in Fase 3 si renderanno
  pacchetti di terze parti.
### Fase 1 — stato DoD (2026-06-28)
**Tutti gli elementi della Fase 1 sono implementati** (CodeMirror F1-005, export
PDF e file I/O F1-006, diagnostica inline F1-007; più anteprima dal vivo, World
persistente e affidabilità delle fasi precedenti). Restano come *refinement* non
bloccanti per la DoD: F1-R1 (scan font all'avvio), F1-R2 (streaming `Channel`),
F1-R4/R5 (note future). **Confermati a vista** (smoke test 2026-06-28):
evidenziazione Typst sotto CSP, dialoghi file (apri/salva), diagnostici inline,
anteprima dal vivo. *(Export PDF su disco: da confermare esplicitamente.)*
Verifica automatica (build + test Rust + clippy + tsc/Vite): verde.

## Fase 2 — avanzamento (2026-06-28)

### D-004 — Incremento di partenza Fase 2
**Decisione:** iniziare la Fase 2 dalla **toolbar di formattazione** (frontend puro,
nessun binario esterno, rischio basso, valore UX immediato per i principianti).
Il sidecar Tinymist (LSP) e il completamento via `typst-ide` restano per gli
incrementi successivi; per il sidecar va prima risolto l'allineamento versioni
Typst↔Tinymist e prototipato in isolamento il transport LSP su Tauri (PIANO.md §7).

### F2-001 — Toolbar di formattazione
**Fatto.** Pulsanti che inseriscono il markup Typst corretto: grassetto `*`,
corsivo `_`, codice `` ` ``, titolo `= `, liste `- `/`+ `, link, immagine, tabella.
- **Architettura:** modulo `src/formattazione.ts` con trasformazioni CodeMirror
  pure (`changeByRange`, `EditorSelection`); `editor.ts` espone solo `formatta(comando)`
  (view incapsulata); `main.ts` cabla i pulsanti per `data-comando` con delega eventi.
- **Comportamento:** `avvolgi` gestisce selezione vuota (cursore tra i marcatori) e
  testo (ri-selezione); `prefiggiRighe` prefissa ogni riga toccata (escludendo la riga
  finale se la selezione termina al suo inizio); blocchi (tabella/immagine) sulla sola
  selezione primaria; `scrollIntoView` dopo ogni comando. Ogni formattazione fa
  scattare `onModifica` → segna modificato + ricompila.
- **Refinement noti:** wrapping su selezione multi-paragrafo può produrre markup non
  reso (l'anteprima lo evidenzia); comportamento "toggle" sui prefissi (rimuovere se
  già presente) rimandato; navigazione tastiera della toolbar (roving tabindex) futura.
- **Verifica:** `tsc`+Vite verde; review applicata (2 critici + fix); **toolbar
  confermata a vista** (smoke test 2026-06-28).

### F2-002 — Autocompletamento + hover via `typst-ide`
**Fatto.** Assistenza alla scrittura con il crate `typst-ide` embeddato (NIENTE
sidecar Tinymist).
- **Core (`typst-core`):** `impl typst_ide::IdeWorld for EditorWorld` (`upcast`);
  metodi `completa(cursore_utf16, esplicito) -> Option<RisultatoCompletamento>` e
  `tooltip(cursore_utf16) -> Option<String>` via `typst_ide::autocomplete`/`tooltip`,
  con conversione offset UTF-16↔byte e mapping `CompletionKind`→stringa CM6.
  `output = None` (no documento): funzionano funzioni/parametri/tipi/simboli/font;
  i completamenti/tooltip di **label** (`#ref`) richiedono il documento → refinement.
- **Guscio:** comandi async `completa`/`tooltip` (stesso pattern di `compila`).
- **Frontend:** `src/assistenza.ts` con sorgente `autocompletion({override})` e
  `hoverTooltip`, che chiamano i comandi; degradazione graziosa (errore/None → niente
  popup); guardia di versione contro risposte obsolete; `apply` snippet → fallback
  all'etichetta. `editor.ts` include le estensioni.
- **Limiti noti:** contesa sul `Mutex` con `compila` (completamento attende una
  compilazione in corso) — accettata per la v1, mitigabile con `RwLock` (vedi limite
  in F1-006); supporto snippet `${…}` e completamenti di label rimandati.
- **Verifica:** `cargo test -p typst-core` (6 test) + clippy + `tsc`/Vite verdi;
  **confermati a vista** (smoke test 2026-06-28): autocompletamento (menu dopo `#`
  / Ctrl+Spazio) e hover funzionanti. Risolto C-1: `autocompletion({override})`
  convive con quello (vuoto) di `basicSetup` senza conflitti `combineConfig`.

### F2-003 — Snippet riutilizzabili da file esterno
**Fatto.** Snippet personalizzabili offerti come completamenti con placeholder
navigabili a Tab.
- **Config esterna (`config.rs`):** nuovo modulo puro; `Snippet { etichetta,
  prefisso, corpo, descrizione }` letti da `snippets.json` nella config dir standard
  (Windows: `%APPDATA%\editor-typst\snippets.json`). `carica_snippet()` infallibile:
  crea i predefiniti al primo avvio, degrada con grazia su file mancante/illeggibile/
  JSON invalido senza sovrascrivere il file utente (scrittura atomica `create_new`);
  scarta voci con prefisso/corpo vuoti. 6 snippet predefiniti (figura, tabella,
  equazione, collegamento, codice, intestazione) in sintassi snippet CodeMirror.
- **Guscio:** comando async `elenca_snippet()` (stateless, solo I/O config).
- **Frontend:** `caricaSnippet()` mappa a `snippetCompletion`; seconda sorgente
  `fonteSnippet` accanto a quella typst-ide. Si **disattiva subito dopo `#`** (lì
  lavora typst-ide), evitando il doppio cancelletto; degradazione graziosa.
- **Personalizzazione:** l'utente edita `snippets.json` (sintassi `${campo}`,
  `${0}` stop finale); ricaricato all'avvio.
- **Verifica:** `cargo test -p typst-core` (11 test) + clippy + `tsc`/Vite verdi;
  review applicata (C-2 doppio `#` + altri); **confermati a vista** (smoke test
  2026-06-28): snippet che compaiono ed espandono i placeholder; `snippets.json`
  creato in `%APPDATA%\editor-typst\`.

### Fase 2 — stato DoD (2026-06-28)
**DoD raggiunta e confermata a vista.** (PIANO §4): toolbar che produce markup
compilabile (F2-001) ✓; completamento di funzioni/parametri e hover (F2-002) ✓;
uno snippet personalizzato inseribile (F2-003) ✓ — tutti verificati a runtime nello
smoke test del 2026-06-28. Il **sidecar Tinymist** (LSP completo) resta un
*enhancement* oltre il minimo della fase (il PIANO prevede `typst-ide` come base
sufficiente): da affrontare come pass successivo o saltare.

## Fase 3 — avanzamento (2026-06-28)

Decomposta in: **3.1** browser pacchetti + installazione · **3.2** (assorbita in
3.1: install via `obtain`) · **3.3** "Nuovo da template".

### F3-001 — Browser pacchetti Typst Universe + installazione
**Fatto.** Ricerca, navigazione e installazione di pacchetti da Typst Universe.
- **Indice (`packages.rs`, puro):** fetch di `packages.typst.org/preview/index.json`
  via `ureq` 2.x, **offline-first** con cache su disco (`cache_dir()/editor-typst/
  universe-index.json`, TTL 30 min, ripiego su cache stantia se la rete è KO).
  Parsing tollerante voce-per-voce; raggruppamento per nome tenendo la versione
  massima (`PackageVersion`); validazione nome `^[a-z0-9-]+$` + versione semver
  (scarta voci fuori vincolo). Esito `EsitoElenco { pacchetti, stantia }`.
- **Installazione:** `installa_pacchetto` **stateless** (un `SystemPackages`
  dedicato, NON il World: niente contesa col `Mutex` durante il download di rete);
  `obtain(&spec)` pre-scarica nella cache standard `@preview`.
- **Frontend (`pacchetti.ts`):** pannello drawer con ricerca client-side; rendering
  **sicuro** (solo `createElement`/`textContent`: i metadati sono remoti non fidati);
  "Installa e importa" → pre-download + inserimento `#import "@preview/{nome}:{ver}": *`
  + ricompila. Validazione nome/versione anche lato TS (difesa in profondità);
  indicatore "cache locale (offline)"; pulsanti disabilitati durante l'install.
- **Sicurezza (PROGETTO §11):** cache standard ufficiale; validazione contro
  injection di markup via nome/versione manomessi.
- **Verifica:** `cargo test -p typst-core` (16 test) + clippy + `tsc`/Vite verdi;
  review applicata (C-1 injection, C-2 lock di rete, D-1/D-2/D-3 robustezza/offline);
  **confermato a vista con rete reale** (smoke test 2026-06-28): ricerca, filtri,
  download e installazione di pacchetti Universe funzionanti, l'`#import` compare e
  compila. **Chiude il 1° punto della DoD Fase 3.**

### F3-002 — "Nuovo da template" + risoluzione file di progetto dal disco
**Fatto.** Creazione di un progetto da un template di Universe e compilazione.
- **World modalità progetto (`world.rs`):** nuovo campo `radice_progetto: Option<FsRoot>`
  (`None` = documento-singolo, comportamento INVARIATO). `apri_progetto(radice,
  entrypoint_relativo, contenuto)` aggancia la radice su disco e imposta l'entrypoint
  come file principale (in memoria). In modalità progetto, `source`/`file` per
  `VirtualRoot::Project` leggono i file fratelli dal disco via **`FsRoot::load`**
  (risoluzione sicura: doppia barriera `VirtualPath::new` + `realize`, rifiuta `..`,
  drive-letter, e — dopo review — symlink e `canonicalize` della radice).
- **`nuovo_da_template` (`packages.rs`):** `obtain` del template → parse `typst.toml
  [template]` (path/entrypoint, validati relativi senza `..`) → copia ricorsiva (che
  **salta i symlink**) di `[template].path` in `destinazione/{nome}` (errore se già
  esistente) → entrypoint assoluto. Lock del World tenuto SOLO per `apri_progetto`
  (non durante download/copia).
- **Frontend:** pulsante "Nuovo da template" sulle card template → dialog cartella →
  `nuovo_da_template` → `fileops.apriProgetto(entrypoint, contenuto)` (adotta come
  documento corrente, Save scrive sull'entrypoint) → ricompila → chiude il pannello.
- **Sicurezza (PROGETTO §11):** review applicata (C-1 symlink traversal, C-2 entrypoint
  annidato, C-6 path traversal da `typst.toml`, canonicalize, no-overwrite).
- **Limite noto (Fase 4):** l'apertura *manuale* di un singolo file non aggancia ancora
  la radice (solo il flusso template lo fa); edit live dei file fratelli richiederà
  invalidazione comemo.
- **Verifica:** `cargo test -p typst-core` (27 test) + clippy + `tsc`/Vite verdi;
  **confermato a vista con rete reale** (smoke test 2026-06-29): creazione di un
  progetto da un template di Universe, apertura dell'entrypoint e compilazione.

### Fase 3 — stato DoD (2026-06-29)
**DoD raggiunta e confermata a vista** (PIANO §4): cerca un pacchetto, installalo,
l'import compare e compila (F3-001) ✓; crea un progetto nuovo da un template della
rete e compilalo (F3-002) ✓. La Fase 3 (ecosistema integrato, "cuore del progetto")
è completa. Limite portato in Fase 4: aggancio radice-progetto anche all'apertura
manuale di file/cartelle e invalidazione comemo per l'edit live dei file fratelli.

## Fase 4 — avanzamento (2026-06-29)

### D-005 — Incremento di partenza Fase 4
**Decisione:** iniziare dalla **apertura cartella-progetto**, che estende la modalità
progetto già introdotta in F3-002 (radice + `apri_progetto`). Le altre voci della
Fase 4 (sync sorgente↔anteprima, `@local`, salvataggio automatico/cronologia) e il
sidecar Tinymist restano per gli incrementi successivi.

### F4-001 — Apertura cartella-progetto (+ aggancio radice all'apertura file)
**Fatto.** Apertura di una cartella multi-file come progetto; i file fratelli
(`include`, immagini, dati) si risolvono dal disco.
- **Core (`progetto.rs`, nuovo, puro):** `rileva_entrypoint(cartella)` con euristica
  `main.typ` → `[package].entrypoint` del `typst.toml` → unico `.typ` → Err; valida
  l'entrypoint del manifest come relativo pulito; canonicalizza (una sola volta, dopo
  review). Test offline con `tempfile`.
- **Guscio:** comando `apri_cartella` (rileva entrypoint → `apri_progetto`). Inoltre
  **`apri_file` ora aggancia la radice** alla cartella del file aperto → rimosso il
  limite noto (l'apertura manuale risolve import/risorse relativi). `RisultatoProgetto`
  condiviso con `nuovo_da_template`.
- **Frontend:** pulsante "Apri cartella" + scorciatoia **Ctrl+Shift+O**; riusa
  `fileops.apriProgetto`.
- **Gap noto (futuro "Nuovo documento"):** non esiste ancora un modo per USCIRE dalla
  modalità progetto (il World resta agganciato alla radice). Nessun pulsante "Nuovo"
  oggi, quindi non è un problema pratico; servirà un `chiudi_progetto` quando si
  aggiungerà "Nuovo documento".
- **Verifica:** `cargo test -p typst-core` (35 test) + clippy + `tsc`/Vite verdi;
  review applicata (#1 TOCTOU canonicalize, #2 symlink, #6 toml malformato);
  **confermato a vista** (smoke test 2026-06-29): la cartella si apre, l'entrypoint
  compila. *(Sui font vedi F4-002.)*

### F4-002 — Scansione dei font della cartella di progetto
**Fatto.** I template/progetti che includono i propri font (in una sottocartella
**`fonts/`**) si rendono senza installare i font nel sistema.
- **Causa risolta:** il `World` caricava solo font embedded + di sistema; un
  documento che usa un font non installato/non embedded mostrava l'avviso "unknown
  font family" e ripiegava sul fallback.
- **Core (`world.rs`):** helper `costruisci_font_store(radice_progetto)` = embedded
  + system + (se presente) `typst_kit::fonts::scan(radice/fonts)`. `apri_progetto`
  ricostruisce lo store (così ri-scansiona anche i font di **sistema** → un font
  installato dopo l'avvio è preso riaprendo il progetto). Best-effort (non fallisce
  se `fonts/` manca). Cambiando il `book()`, comemo si invalida da sé.
- **Scelta (dopo review):** si scansiona SOLO `radice/fonts` (convenzione Typst,
  `--font-path`), non l'intero progetto ricorsivamente (evita `.git`/`node_modules`).
- **Uso:** metti i `.ttf/.otf` necessari in `fonts/` nella cartella del progetto.
  Per font che devono stare nel sistema, installali (poi riapri il progetto).
- **Verifica:** `cargo test -p typst-core` (40 test) + clippy verdi; review applicata
  (#1 assegnazioni atomiche, #2 Err su entrypoint invalido, #3 scan solo `fonts/`,
  #4 niente rumore); **confermato a vista** (smoke test 2026-06-29): font messo in
  `fonts/` → riapertura → resa corretta, avviso "unknown font family" sparito.

### F4-003 — Sincronizzazione sorgente↔anteprima
**Fatto.** Clic sull'anteprima → cursore nel sorgente, e viceversa (SyncTeX-like).
- **Core (`world.rs`):** cache `ultimo_documento: Option<PagedDocument>` aggiornata
  da `compila` (ora `&mut self`) solo su successo (su errore resta l'ultimo render
  valido). Metodi `jump_da_click(pagina, x, y) -> Option<usize>` (via
  `typst_ide::jump_from_click` + `Jump::File` sul file principale → offset UTF-16) e
  `jump_da_cursore(cursore) -> Vec<Posizione{pagina,x,y}>` (via `jump_from_cursor`).
  Comandi read-only (NON toccano il sorgente; usano l'ultimo documento compilato).
- **Coordinate:** l'SVG di `typst-svg` ha viewBox in pt (1 unità SVG = 1 pt), quindi
  clic→`getScreenCTM().inverse()`=pt e pt→coordinate del marcatore, senza scala.
- **Frontend:** `editor.ts` `vaiA(offset)` + notifica `onCursore` (selectionSet);
  `sincronizzazione.ts` gestisce le due direzioni con debounce, marcatori `<rect>`
  sull'SVG e **guardia anti-loop** (un clic che sposta il cursore non ri-sincronizza
  l'anteprima).
- **Limiti noti:** durante l'editing, prima del recompile, il jump usa l'ultimo
  documento + sorgente corrente → posizioni approssimate (best-effort di typst-ide);
  contesa col `Mutex` di `compila` (accettata, mitigata dal debounce).
- **Verifica:** `cargo test -p typst-core` (43 test) + clippy + `tsc`/Vite verdi;
  review applicata (D-4 pulizia marcatori, S-2 validazione pagina); **confermato a
  vista** (smoke test 2026-06-29): clic sull'anteprima → cursore, e cursore → barra
  evidenziatrice nell'anteprima. Chiude il 2° dei 3 punti DoD Fase 4 (resta `@local`).

### F4-004 — Pubblicazione pacchetti `@local`
**Fatto.** Il documento corrente si pubblica come pacchetto riutilizzabile,
importabile come `#import "@local/{nome}:{versione}"`.
- **Core (`packages.rs`):** `pubblica_in(base, nome, versione, contenuto)` (valida
  nome/versione; scrive `lib.typ` + `typst.toml` generato via serde+`toml`; scrittura
  **atomica** via cartella temporanea + `rename`; campo `compiler="0.15"`; limite
  ~10 MiB). `pubblica_local` calcola `dirs::data_dir()/typst/packages/local` (percorso
  `@local` standard, confermato contro typst-kit `FsPackages::system_data`). `SystemPackages`
  risolve già `@local` → nessuna modifica al World.
- **Cache:** dopo la pubblicazione il comando chiama `typst_core::invalida_cache()`
  (`comemo::evict(0)`): senza, una precedente risoluzione `NotFound` memoizzata
  nasconderebbe il pacchetto appena creato fino allo sfratto della cache.
- **Frontend (`local.ts`):** modale nome+versione (validazione, focus trap, aria,
  pattern), `invoke("pubblica_local", …)` col testo dell'editor; messaggio d'esito
  con la stringa di import. Pulsante "@local" nell'header.
- **Verifica:** `cargo test -p typst-core` (48 test) + clippy + `tsc`/Vite verdi;
  review applicata (#1 scrittura atomica, #3 limite dimensione, #4 evict comemo,
  #9 compiler); **confermato a vista** (smoke test 2026-06-29): pubblicato un `@local`
  e importato (`#import "@local/test:0.1.0"`) → compila.

### Fase 4 — stato DoD (2026-06-29)
**DoD raggiunta e confermata a vista** (PIANO §4): progetto multi-file compila
(F4-001/F4-002, con risoluzione risorse e font di progetto) ✓; clic sull'anteprima
sposta il cursore e viceversa (F4-003) ✓; un pacchetto `@local` creato dall'app è
importabile (F4-004) ✓. La Fase 4 (progetti e produttività) è completa. Restano come
refinement non bloccanti: salvataggio automatico + cronologia versioni; uscita dalla
modalità progetto per un futuro "Nuovo documento"; invalidazione comemo per l'edit
live dei file fratelli.

## Fase 6 — pianificazione (2026-07-16)

> Fase aggiunta a [PROGETTO.md](PROGETTO.md) §4.5/§10 e [PIANO.md](PIANO.md) §4:
> **editor Markdown e interoperabilità md↔typst**. In questa data è stata svolta
> dapprima la **sola progettazione**, con implementazione rinviata al ripristino
> dei sorgenti di `typst-core` (B-003); il core è poi stato **ricostruito in
> giornata** (B-003 risolto) e la fase **implementata e completata** — vedi
> "Ricostruzione del core" e "Fase 6 — avanzamento" più sotto.

### D-006 — Consapevolezza del tipo di documento (`.typ` | `.md`)
**Data:** 2026-07-16
**Decisione:** l'app diventa consapevole del **tipo di documento**: `.typ`
(nativo) vs `.md` (Markdown). È un concetto a cavallo tra dominio (core) e
presentazione, e guida tre comportamenti: (a) la modalità di evidenziazione
dell'editor (`@codemirror/lang-markdown` per i `.md`, grammatica Typst per i
`.typ`); (b) se convertire md→typst prima di compilare l'anteprima; (c) il
formato di salvataggio. L'anteprima dal vivo del Markdown **non introduce un
secondo motore di rendering**: si converte md→typst e si riusa la pipeline
esistente (`EditorWorld::compila` → SVG).
**Motivazione:** coerenza con i principi di PROGETTO.md §2 — "non reinventare
Typst" e leggerezza (un solo artefatto): l'editor orchestra la pipeline che
già possiede invece di duplicarla con un renderer Markdown dedicato.

### D-007 — Strategia di conversione md↔typst *(CONFERMATA e attuata)*
**Data:** 2026-07-16
**Stato:** **CONFERMATA dall'utente (2026-07-16): Opzione B — conversione
nativa in `typst-core`.** Attuata lo stesso giorno in F6-001 (modulo
`markdown.rs`: `md_a_typst` via `pulldown-cmark = "=0.13.4"`, `typst_a_md`
sull'AST di `typst-syntax = "=0.15.0"`, resa dipendenza **diretta** del core);
lossy come dichiarato, limiti documentati nella doc di modulo. Registrata
anche in PIANO.md §6, punto 5.

**Opzione A — Pandoc come sidecar.** Alta fedeltà su costrutti complessi nel
verso md→typst, e coerente col pattern "Tinymist come sidecar" (D-001). MA:
introduce un binario esterno **pesante**, in tensione col vincolo di leggerezza
(PROGETTO.md §2 e §9); e soprattutto Pandoc ha un *writer* Typst ma **non** un
*reader* Typst maturo → il verso typst→md resterebbe comunque da implementare
a mano. Più invasivo del sidecar Tinymist (che è opzionale e non tocca il
percorso dati dei documenti).

**Opzione B — conversione nativa in `typst-core` (RACCOMANDATA).**
- `md → typst`: parsing Markdown con il crate **`pulldown-cmark`** (maturo,
  puro Rust, CommonMark) → emissione di markup Typst (`=` titoli,
  `*grassetto*`, `_corsivo_`, liste, `#link`, `#image`, blocchi codice con
  raw, tabelle → `#table`).
- `typst → md`: percorrere l'AST di **`typst-syntax`** (crate **GIÀ presente**
  nell'albero delle dipendenze via lo stack Typst 0.15) → emettere CommonMark
  per il sottoinsieme mappabile.
- **Vantaggi:** nessun binario esterno; convertitori nel core, puri e
  testabili headless (coerente con D-003, separazione core/presentazione);
  un solo artefatto leggero.
- **Limite dichiarato:** conversione **lossy** sui costrutti avanzati
  (funzioni Typst arbitrarie, math complessa, layout) — la conversione punta
  al **sottoinsieme di markup comune**, non alla fedeltà totale; costrutti non
  mappati segnalati all'utente; round-trip non garantito (rischi in PIANO.md
  §7 e PROGETTO.md §11).

**Raccomandazione:** Opzione B, per aderenza ai principi (leggerezza +
non-duplicazione + separazione core/presentazione). Decisione finale
all'utente. *(Esito: confermata Opzione B — vedi Stato sopra.)*

## Ricostruzione del core (2026-07-16)

I 5 moduli di `crates/typst-core/src/` persi dalla copia di lavoro (B-003)
sono stati **ricostruiti da zero** usando come contratto: (a) questo log
(voci F1…F4); (b) l'API pubblica attesa fissata da `src-tauri/src/lib.rs`
(rimasto intatto); (c) le firme reali dei crate Typst 0.15, lette dai
sorgenti nel registry Cargo. **Verifica:** `cargo build`, `cargo clippy
--all-targets -D warnings`, `cargo fmt` verdi + **41 test unitari** superati;
poi revisione statica (nessun rilievo CRITICO) con fix di robustezza di rete
applicati. Dettagli sulla chiusura in B-003; unico scostamento importante dal
codice perso in D-008.

### D-008 — `invalida_cache` via `typst::comemo::evict` (ricostruzione core)
**Data:** 2026-07-16
**Contesto:** durante la ricostruzione è emerso che `typst` 0.15 traccia il
proprio `World` con **comemo 0.5.1** (ri-esportato come `typst::comemo`),
mentre la dipendenza diretta `comemo = "0.4"` del `Cargo.toml` risolve a
un'istanza **separata** (0.4.0) con cache propria.
**Decisione:** `invalida_cache()` — e ogni `evict` del core — delega a
`typst::comemo::evict`, NON al crate `comemo` diretto: sfrattare l'istanza
0.4 sarebbe un **no-op silenzioso** sulla cache che conta davvero (quella che
memoizza le risoluzioni del World, es. il `NotFound` di un pacchetto `@local`
appena pubblicato — F4-004).
**Motivazione:** correttezza dell'invalidazione; il ri-export di `typst`
garantisce per costruzione l'allineamento di versione col compilatore.
Documentato anche nel commento di `crates/typst-core/src/lib.rs`.
**Alternative scartate:** usare il crate `comemo = "0.4"` diretto (istanza di
cache sbagliata); aggiornare il pin diretto a `0.5` (ridondante: nel codice
ricostruito tutti gli usi passano dal ri-export — la voce in `Cargo.toml`
resta come retaggio di D-002).

## Fase 6 — avanzamento (2026-07-16)

> Implementazione avviata dopo la ricostruzione del core (B-003 risolto) e la
> conferma di D-007 (Opzione B). Verifica: build + clippy (`-D warnings`) +
> fmt + **74 test** `typst-core` verdi; `npm run build` (tsc+Vite) verde.
> Il guscio si compila anche su questa macchina (toolchain GNU, vedi B-004);
> lo smoke test **interattivo** dell'app completa resta in capo all'utente.

### F6-001 — Conversione nativa md↔typst nel core (`markdown.rs`)
**Fatto.** Modulo `crates/typst-core/src/markdown.rs`, puro (`&str -> String`,
nessun I/O), come da D-007 Opzione B.
- **`md_a_typst`:** percorre gli `Event` di **`pulldown-cmark = "=0.13.4"`**
  (`default-features = false`: serve solo il parser, non il renderer HTML;
  opzioni `ENABLE_TABLES` + `ENABLE_STRIKETHROUGH`) ed emette markup Typst:
  `=` titoli, `*`/`_` enfasi, liste `-`/`+` (annidamento a indentazione),
  `#link(..)[…]`, `#image("…")`, `#strike[…]`, `#quote(block: true)[…]`,
  `#table(columns: N, […], …)`, blocchi codice raw, `#line(length: 100%)`.
- **`typst_a_md`:** percorre l'**AST tipato di `typst-syntax = "=0.15.0"`**
  (resa dipendenza **diretta** del core, prima solo transitiva) ed emette
  CommonMark per il sottoinsieme mappabile; riconosce esplicitamente le
  `FuncCall` idiomatiche `link`/`image`/`strike`/`quote` nella forma emessa da
  `md_a_typst` e le mappa sui costrutti Markdown nativi (round-trip
  strutturale; forma inattesa → fallback grezzo).
- **Escaping in entrambe le direzioni** sul testo letterale: verso Typst
  ``\ * _ # $ < > @ ` [ ] ~`` (senza `[`/`]` un `]` nel testo di un link
  chiuderebbe in anticipo il content block generato); verso Markdown i
  metacaratteri md (``\ ` * _ # [ ] < >``). Codice inline con **delimitatori a
  backtick dinamici** (run interna più lunga + 1, padding a spazi), con la
  particolarità che verso Typst il conteggio salta 2 → 3: il lexer di
  `typst-syntax` tratta **due backtick come raw span sempre vuoto**.
- **LOSSY per scelta** (D-007): costrutti Typst avanzati (funzioni arbitrarie,
  math, `set`/`show`/`let`, cicli/condizionali, `@ref`/`<label>`) riprodotti
  **verbatim** via `SyntaxNode::full_text()`, non convertiti. **Limite noto
  documentato:** le shorthand trattino/ellissi (`--`/`---`/`...`) NON vengono
  scappate verso Typst (richiederebbe contesto sui caratteri adiacenti).
  Tutti i limiti sono elencati nella doc di modulo.
- **Verifica:** build/clippy/fmt verdi; **74 test** totali nel core (33 del
  modulo: heading, enfasi, liste annidate, tabelle, backtick dinamici,
  escaping, round-trip, fallback lossy).

### F6-002 — Comandi Tauri di conversione e anteprima (`src-tauri/src/lib.rs`)
**Fatto.** Tre nuovi comandi, stesso pattern dei precedenti (`async` +
`spawn_blocking`, recupero da lock avvelenato), registrati in
`generate_handler!`:
- `converti_md_a_typst` / `converti_typst_a_md`: stateless, delegano al core;
  `Err` solo per task abortito (la conversione è infallibile ma lossy).
- `anteprima_markdown`: **riusa la pipeline esistente** (D-006, "orchestrare,
  non duplicare") — converte md→typst poi `aggiorna_sorgente` + `compila` sul
  World persistente, come `compila`. Limite documentato: i diagnostici si
  riferiscono al **Typst generato**, non alle righe del `.md` (vedi F6-003).

### F6-003 — Frontend: tipo di documento, evidenziazione, conversione
**Fatto.** Il tipo di documento `.typ` | `.md` (D-006) attraversa
`src/editor.ts`, `src/fileops.ts`, `src/main.ts` e `index.html`.
- **`editor.ts`:** tipo `TipoDocumento = "typst" | "markdown"` +
  `impostaTipo()`; evidenziazione **commutabile** in un `Compartment`:
  grammatica Typst (WASM opzionale, invariata) per `.typ`,
  **`@codemirror/lang-markdown`** (^6.5.1, ufficiale, stabile, import
  statico) per `.md`.
- **`fileops.ts`:** riconoscimento tipo dall'estensione (`tipoDaPercorso`),
  filtri dialog `.typ`/`.md`, apertura/salvataggio `.md`; `convertitoA(tipo)`
  azzera il percorso dopo una conversione (evita di salvare Markdown in un
  `.typ` e viceversa); l'**export PDF compila sempre Typst** (in modalità md
  il chiamante passa il Typst già convertito).
- **`main.ts`:** anteprima dal vivo del `.md` via `anteprima_markdown`
  (md→typst→compile, stesso debounce); pulsante di **conversione
  bidirezionale** ("→ Typst" / "→ Markdown") con avviso lossy; in modalità
  Markdown i **diagnostici NON sono inline** (gli offset si riferiscono al
  Typst generato) ma tutti nel riquadro inferiore, preceduti dalla nota
  "dal Typst generato dalla conversione"; **toolbar di formattazione nascosta**
  in md (inserisce markup Typst).
- **Verifica:** `npm run build` (tsc + Vite) verde.

### Fase 6 — stato DoD (2026-07-16)
**DoD raggiunta** (PIANO §4): apertura e salvataggio di un `.md` con
evidenziazione Markdown (F6-003) ✓; anteprima dal vivo di un `.md` via
md→typst→compile (F6-002/F6-003) ✓; comando `converti` nei due versi
(F6-001/F6-002) ✓; conversione documentata come **lossy** con avviso
all'utente e limiti in doc di modulo (F6-001) ✓. **Nota:** la conferma a
vista (smoke test interattivo dell'app completa) è **rinviata** all'utente;
il guscio si compila anche su questa macchina su toolchain GNU
(`editor-tauri.exe` prodotto, MSVC non necessario — vedi B-004; per
eseguirlo serve il runtime WebView2, di norma presente su Win11).

## Fase 5 — pianificazione (2026-09-28)

> Fase 5 (**Rifinitura**) scomposta in task **F5-001…F5-018** (poi estese a
> **F5-023** con D-014) in
> [PIANO.md](PIANO.md) §4, "Fase 5 — Rifinitura" (blocchi: identità e
> contratto · impostazioni/aspetto/palette · prestazioni · funzionalità
> mancanti · distribuzione), con ordine consigliato e DoD ampliata. Fonti:
> voci già scritte della Fase 5; rinomina in Typster (D-009); refinement
> rimandati (F1-R1, F1-R2, auto-save/cronologia e edit live dei file
> fratelli da "Fase 4 — stato DoD"); due lacune accertate — export PNG/SVG
> promesso da PROGETTO.md §4.4 ma assente nel guscio (F5-011) e
> contraddizione D-001 (Tinymist sidecar) vs `src/assistenza.ts` (`typst-ide`
> embeddato, "nessun sidecar") → F5-002 (**risolta**: D-010). Le voci di
> SPEC-UNIFICATA.md e altre ispirazioni erano raccolte come proposte
> P5-A…P5-I "da confermare": ora **tutte accettate** (D-014), vedi sotto.
> Sola pianificazione: nessun codice modificato.
>
> *Aggiornamento 2026-09-28 (decisioni dell'utente, solo documenti):* D-009
> punti 1 e 2 decisi; D-010 (Tinymist extra caricabile, F5-002 fatta, nuova
> voce X-001 fuori dalle fasi); D-011 (PDF/A in F5-011); D-012 (cronologia
> nella cartella dati dell'app, F5-013); D-013 (`panic = "abort"` in release,
> F5-015).
>
> *Secondo aggiornamento 2026-09-28 (decisioni dell'utente, solo documenti):*
> **D-014** — proposte P5-A…P5-I **accettate**: P5-C (barra laterale), P5-D
> (file modificati da fuori) e P5-G (focus/macchina da scrivere) entrano in
> Fase 5 come **F5-019…F5-023**; P5-A, P5-B, P5-E, P5-F, P5-H, P5-I e il
> menu `/` formano la nuova **Fase 7 — Estensioni** (**F7-001…F7-007**).
> Chiusi i punti ancora aperti: migrazione di `snippets.json` per **copia**,
> vecchia cartella intatta, nessun avviso (D-009 punto 2, F5-001); PDF/A in
> UI solo **A-2b, A-3b, A-4** (D-011, F5-011); chiave della cronologia con
> **`sha2`**, radice `dirs::data_dir()/typster/cronologia/` confermata
> (D-012, F5-013).
>
> *Terzo aggiornamento 2026-09-28 (decisioni dell'utente, solo documenti):*
> **D-016** — accettate le 10 proposte dell'analista sul catalogo di riuso
> [`fase8.md`](fase8.md) (ex `sorgenti/riuso-fase5-7.md`); nuova task
> **F5-024** (licenze del codice derivato), che porta la Fase 5 a
> **F5-001…F5-024**; chiuso il punto aperto di D-015 su F7-007
> (`sys.inputs` salvate anche per il documento singolo, decisione
> provvisoria).

### D-009 — Nome del prodotto: Typster
**Data:** 2026-09-28
**Stato:** nome **deciso dall'utente**; attuazione nel codice = task **F5-001**
(PIANO.md §4). I due punti prima in sospeso sono **decisi** (2026-09-28, sotto):
identifier `it.nexodux.typster`; cartelle dati del core `…/typster/`.
**Contesto:** il programma non aveva un nome di prodotto: ovunque compare la
descrizione "Editor Typst". Stato attuale nel codice (verificato 2026-09-28):
- `package.json`: `"name": "editor-typst"`;
- `src-tauri/tauri.conf.json`: `"productName": "Editor Typst"`,
  `"identifier": "it.nexodux.editor-typst"`, titolo finestra `"Editor Typst"`;
- `src-tauri/Cargo.toml`: crate/bin `editor-tauri`, lib `editor_tauri_lib`
  (richiamata da `src-tauri/src/main.rs`);
- `Cargo.toml` radice: commento `# Workspace Cargo — Editor Typst (Tauri 2)`;
- `index.html`: `<title>` e `<h1>` "Editor Typst";
- inoltre: `src/main.ts` (`document.title = \`Editor Typst — …\``),
  `src-tauri/capabilities/default.json` (descrizione),
  `crates/typst-core/src/lib.rs` (`AGENTE_UTENTE = "editor-typst/0.1"`),
  cartelle dati del core `dirs::config_dir()/editor-typst/` (`config.rs`,
  `snippets.json`) e `dirs::cache_dir()/editor-typst/` (`packages.rs`,
  `universe-index.json`).
**Decisione:** il prodotto si chiama **Typster**. Si rinominano nome visibile,
titoli e metadati (`productName`, finestra, `<title>`/`<h1>`,
`document.title`, `package.json`, crate del guscio → proposta `typster` /
`typster_lib`). **Non** si rinominano: la cartella del repo (`editor-typst/`)
e il crate `typst-core` (nome tecnico). Nei documenti di contratto cambiano
per ora solo i titoli di PROGETTO.md e PIANO.md; le voci storiche di questo
log restano com'erano.
**Motivazione:** un nome di prodotto proprio, distinto dal linguaggio Typst,
serve prima della pacchettizzazione (installer, voce nel menu Start/Applicazioni).

**Punti in sospeso (decide l'utente) — entrambi DECISI il 2026-09-28:**
1. **Cambiare anche l'`identifier`** (`it.nexodux.editor-typst` →
   es. `it.nexodux.typster`)? In Tauri l'identifier determina le cartelle dati
   dell'app (`app_data_dir`/`app_config_dir`/…) e la cartella dati del
   webview (su Windows il profilo WebView2 sotto `%LOCALAPPDATA%\<identifier>`,
   dove vivono `localStorage`, IndexedDB e cache): cambiarlo fa ripartire da
   zero tutto ciò che vi è salvato. **Verificato:** oggi `src/*.ts` e
   `index.html` **non** usano `localStorage`/`sessionStorage`/IndexedDB e il
   guscio non usa i path API di Tauri, quindi il costo attuale è ~nullo (solo
   cache del webview). Diventerà costoso quando le impostazioni di F5-003 o
   un primo installer esisteranno (l'identifier entra anche nel bundle id
   macOS e nei metadati degli installer). *Proposta:* cambiarlo **ora**, in
   F5-001, prima di F5-003/F5-016.
   **→ DECISO (2026-09-28): `it.nexodux.typster`**, da applicare in F5-001
   (`src-tauri/tauri.conf.json`, campo `identifier`). Nessuna migrazione dei
   dati del webview (oggi ~nulli).
2. **Cambiare le cartelle dati del core** (`…/editor-typst/` → `…/typster/`)?
   Queste NON dipendono dall'identifier (sono path fissi via `dirs` in
   `config.rs`/`packages.rs`). Cambiarle senza migrazione farebbe **perdere lo
   `snippets.json` personalizzato** dall'utente (la cache dell'indice invece
   si rigenera). *Proposta:* rinominare con migrazione una tantum (se la
   nuova cartella manca e la vecchia esiste, copiarne il contenuto),
   oppure lasciare `editor-typst` come nome tecnico delle cartelle.
   **→ DECISO (2026-09-28): rinomina in `…/typster/`** sia per
   `dirs::config_dir()` (`config.rs`, `carica_snippet()`) sia per
   `dirs::cache_dir()` (`packages.rs`, `percorso_cache_indice()`). Migrazione
   una tantum di `snippets.json` (copia dalla cartella vecchia, se la nuova
   manca, prima della creazione dei predefiniti; cache non migrata, si
   rigenera) inserita in F5-001.
   **→ Migrazione DECISA (2026-09-28):** `snippets.json` si **copia** nella
   nuova cartella `…/typster/`; la vecchia cartella `editor-typst` resta
   **intatta** (nessuno spostamento, nessuna pulizia) e **nessun avviso** in
   UI (esito solo loggato). La cache dell'indice **non** si migra.
   Rientra nella rinomina anche `AGENTE_UTENTE` → `"typster/0.1"` (`lib.rs`).

### D-010 — Tinymist fuori dallo stack: extra caricabile *(supera D-001 per la parte Tinymist)*
**Data:** 2026-09-28
**Stato:** deciso dall'utente; esito di **F5-002** (riallineamento documentale
fatto, vedi sotto).
**Contesto:** D-001 prevedeva Tinymist come *sidecar* LSP dalla Fase 2; nei
fatti la Fase 2 si è chiusa con `typst-ide` embeddato (F2-002, "NIENTE sidecar
Tinymist"; "Fase 2 — stato DoD"), `src/assistenza.ts` dichiara "nessun
sidecar", `src-tauri/Cargo.toml` non ha `tauri-plugin-shell` e
`src-tauri/binaries/` non esiste. Contratto e codice erano in contraddizione.
**Decisione:** **Tinymist esce dal programma** e diventa un **extra
caricabile**:
- l'assistenza integrata è e resta `typst-ide` embeddato nel core (stato
  attuale di `src/assistenza.ts`);
- Tinymist **non** fa parte dello stack di base né del bundle/installer (niente
  `externalBin`, niente `src-tauri/binaries/`, niente `tauri-plugin-shell` nel
  perimetro base);
- diventa un componente aggiuntivo **opzionale** che l'utente installa per conto
  proprio e indica all'app (es. percorso dell'eseguibile nelle impostazioni);
  se presente e compatibile, l'app lo carica come server LSP su processo
  separato (proxy JSON-RPC su stdio nel guscio), altrimenti ricade su
  `typst-ide` senza perdita di funzioni di base.
L'attuazione è la voce **X-001** (PIANO.md §4, "Estensioni opzionali (fuori
dalle fasi)"): **non** è nella Fase 5 né nella sua DoD, e si avvia solo su
richiesta esplicita.
**Motivazione:** coerenza col codice esistente (la Fase 2 ha già raggiunto la
DoD senza Tinymist); leggerezza (§9: niente binario esterno nel bundle,
target di dimensione "pochi MB" senza "+ sidecar"); i due rischi aperti di
PIANO.md §7 (allineamento versioni Typst↔Tinymist, transport LSP su Tauri non
collaudato) escono dal percorso critico e restano confinati all'estensione. Con
Tinymist opzionale è l'estensione ad adattarsi al core: **non** si pinnano i
crate embeddati sulla versione di Tinymist (resta valido D-002); la
compatibilità si verifica al caricamento.
**Alternative scartate:** (a) abbandono totale di Tinymist — scartata: l'utente
vuole poterlo usare come extra; (c) integrazione come sidecar in bundle in
Fase 5 — scartata: costo alto (rischi §7) e peso del binario, contro il
vincolo di leggerezza.
**Effetti sui documenti:** D-001 annotata ("superata da D-010 per la parte
Tinymist"); la frase su Tinymist nella motivazione di D-002 vale ora solo per
X-001. PIANO.md §0, §1, §2, §3, Fase 2, §5, §6, §7, F5-002 e DoD Fase 5
riallineati; PROGETTO.md §4.1, §5, §6 (Opzione A) e §10 (Fase 2) con nota breve.

### F5-002 — Riallineamento Tinymist (documentale) *(FATTO 2026-09-28)*
**Fatto.** Esito deciso: D-010. Solo documenti (DECISIONI.md, PIANO.md,
PROGETTO.md), nessun codice toccato: `src/assistenza.ts` è già coerente.
**Verifica:** nessuna descrizione residua di Tinymist come parte fissa
dell'architettura nei tre documenti di contratto (le voci storiche di questo
log restano com'erano, come da prassi).

### D-011 — Export PDF/A in F5-011
**Data:** 2026-09-28
**Stato:** deciso dall'utente; attuazione in **F5-011** (PIANO.md §4).
**Contesto:** PROGETTO.md §4.4 promette l'export PDF "con scelta di …
profilo"; F5-011 lasciava il profilo PDF/A "solo se confermato".
**Decisione:** il profilo **PDF/A si fa**: l'export PDF offre la scelta
dello standard PDF/A tramite `typst_pdf::PdfOptions::standards`
(`PdfStandards::new(&[PdfStandard])`). Standard PDF/A supportati da
`typst-pdf` `=0.15.0` (verificato sul sorgente del crate, `src/lib.rs`):
PDF/A-1b, 1a, 2b, 2u, 2a, 3b, 3u, 3a, 4, 4f, 4e; al massimo uno per export.
PDF/UA-1 (anch'esso presente) resta fuori dalla decisione salvo richiesta.
**Motivazione:** chiude una promessa di PROGETTO.md con API già presenti nel
crate pinnato: nessuna dipendenza nuova (`typst-pdf` è già in uso).
**Verifica prevista:** metadati XMP `pdfaid` coerenti con lo standard scelto
(test core), errori di conformità mostrati all'utente senza panic, gate
manuale con validatore esterno (es. veraPDF) e confronto con la CLI 0.15
(dettaglio in F5-011).
**Varianti in UI — DECISO (2026-09-28):** l'interfaccia offre solo
**PDF/A-2b, PDF/A-3b, PDF/A-4** (oltre a "nessuno standard", predefinito).
Le altre varianti supportate da `typst-pdf` restano disponibili in futuro
**senza cambiare il core**: il tipo di opzioni del core mappa tutte le
varianti, la restrizione è solo nella UI.

### D-012 — Cronologia versioni nella cartella dati dell'app (F5-013)
**Data:** 2026-09-28
**Stato:** deciso dall'utente; attuazione in **F5-013**.
**Contesto:** F5-013 lasciava aperta la posizione della cronologia: cartella
dell'app oppure `.typster/` accanto al progetto (impatto sulla "trasparenza
dei dati" di PROGETTO.md §9).
**Decisione:** la cronologia vive nella **cartella dati dell'app**, sotto la
cartella `typster` rinominata con D-009 punto 2 — proposta
`dirs::data_dir()/typster/cronologia/` (su Windows `%APPDATA%\typster\`,
accanto a `snippets.json`). **Non** in `.typster/` accanto al progetto.
*Dettaglio implementativo proposto:* ogni file si associa alla sua
cronologia con una chiave = hash stabile (non `DefaultHasher`) del percorso
assoluto canonicalizzato, sottocartella `cronologia/<chiave>/` con snapshot
di testo leggibili e `meta.json` col percorso originale (anti-collisione).
**→ DECISO (2026-09-28):** la chiave è lo **SHA-256 (crate `sha2`)** in
esadecimale del percorso canonicalizzato (minuscolo su Windows); `sha2` entra
nelle dipendenze bloccate (PIANO.md §3), versione esatta fissata
all'implementazione con `=`. Scartato l'FNV-1a scritto in casa. Radice
**confermata**: `dirs::data_dir()/typster/cronologia/`.
**Motivazione:** le cartelle di progetto restano "solo `.typ` + risorse"
(nessun file nascosto da ignorare nel versionamento del progetto); un unico
posto per i dati dell'app, allineato con impostazioni e snippet.
**Alternative scartate:** `.typster/` accanto al progetto — sporca le
cartelle dei progetti; path API di Tauri (`app_data_dir`) — il core non
dipende da `tauri` (D-003) e il path dipenderebbe dall'identifier.
**Conseguenze accettate:** file rinominato/spostato = nuova cronologia; la
cronologia non viaggia con il progetto se lo si copia su un'altra macchina.

### D-013 — `panic = "abort"` nel profilo release (F5-015)
**Data:** 2026-09-28
**Stato:** approvato dall'utente; attuazione in **F5-015**.
**Decisione:** il `[profile.release]` del `Cargo.toml` radice include
`panic = "abort"` (insieme a `lto = true`, `codegen-units = 1`,
`opt-level = "s"`, `strip = true`).
**Motivazione:** binario più piccolo (niente tabelle di unwinding), coerente
con l'obiettivo di leggerezza §9.
**Conseguenza esplicita:** in **release** il recupero da lock avvelenato di
**F1-003** (`into_inner` dopo un panic di compilazione) diventa
**irraggiungibile**: un panic termina il processo prima che un `Mutex` possa
risultare avvelenato. Il recupero **resta valido in debug/test** (profili con
`unwind`) e non va rimosso. Un panic interno a Typst in release chiude l'app
invece di lasciarla degradata: rischio accettato (gli errori di compilazione
ordinari sono `Result` e restano gestiti).
*Integrata da D-023 (2026-09-30): panic hook con `crash.log`.*

### D-014 — Accettazione proposte P5-A…P5-I e istituzione della Fase 7
**Data:** 2026-09-28
**Stato:** deciso dall'utente; attuazione nelle task F5-019…F5-023 (Fase 5)
e F7-001…F7-007 (Fase 7) di PIANO.md §4.
**Contesto:** la pianificazione della Fase 5 raccoglieva in PIANO.md una
sottosezione "Proposte opzionali (da confermare)" (P5-A…P5-I, da
SPEC-UNIFICATA.md §3/§5 e da altri editor), esplicitamente fuori dalla DoD.
Serviva decidere quali accettare e dove collocarle senza gonfiare la Fase 5.
**Decisione:** **tutte** le proposte sono **accettate**, più il **menu `/`**
di SPEC-UNIFICATA §3.1 ("Slash menu / quick-insert"). Collocazione: **Fase 5
base + Fase 7 estensioni**.

| Proposta | Contenuto | Collocazione | Task |
|----------|-----------|--------------|------|
| P5-C | Barra laterale: struttura a pannelli + browser pacchetti (spostato da F3-001) | Fase 5 | F5-019 |
| P5-C | Indice del documento (titoli `=` / `#`) | Fase 5 | F5-020 |
| P5-C | Albero dei file del progetto | Fase 5 | F5-021 |
| P5-D | Controllo file modificati da fuori (`notify`) + anti-loop dei salvataggi propri | Fase 5 | F5-022 |
| P5-G | Modalità focus e macchina da scrivere (CodeMirror 6) | Fase 5 | F5-023 |
| P5-A | Palette comandi a sottomenu (su registro F5-006) | Fase 7 | F7-001 |
| P5-B | Base comune menu/popup con `@floating-ui/dom` | Fase 7 | F7-002 |
| P5-E | Ricerca nel testo del progetto, pannello della barra laterale, in Rust | Fase 7 | F7-003 |
| P5-F | Interfaccia multilingua: italiano + inglese | Fase 7 | F7-004 |
| P5-H | Scheda parametri delle funzioni | Fase 7 | F7-005 |
| — | Menu `/` (quick-insert) su F7-002 e snippet F2-003 | Fase 7 | F7-006 |
| P5-I | Pannello variabili `sys.inputs` salvate nel progetto | Fase 7 | F7-007 |

Dettagli decisi con l'accettazione:
- **P5-C / ricerca:** la ricerca vive nel pannello della barra laterale ma è
  in Fase 7 (F7-003). In Fase 5 la barra ha un **registro di pannelli**
  (principio plugin/registry) e **nessun** pannello "Ricerca" vuoto: F7-003
  vi si registra senza toccare la struttura.
- **P5-D:** file aperto **senza** modifiche non salvate → ricarica
  automatica; **con** modifiche non salvate → si chiede all'utente quale
  versione tenere. Collegato a F5-014 (edit live dei file fratelli: F5-022 ne
  automatizza l'invalidazione) e a F5-013 (l'anti-loop protegge l'auto-save).
  **File aperto che sparisce dal disco** (cancellato o rinominato/spostato),
  deciso dall'utente: *"avviso, e il testo resta nell'editor come 'non
  salvato'. Verifica il file: se non è compromesso salvalo."*
  *Interpretazione registrata, criteri di integrità confermati con D-024
  (2026-09-30):* avviso +
  testo "non salvato"; verifica di integrità del buffer (non vuoto se il file
  salvato non lo era, UTF-8 valido, nessun troncamento rispetto all'ultima
  versione nota — ultima voce di cronologia F5-013 / impronta dell'ultimo
  salvataggio; la compilabilità **non** è un criterio); se superata, voce in
  cronologia e poi **ripristino automatico al percorso originale** tramite
  l'anti-loop, con avviso "File eliminato dall'esterno: ripristinato da
  Typster"; se fallita, nessun salvataggio e richiesta del percorso
  all'utente. **Rinomina/spostamento rilevato — DECISO (2026-09-28): Typster
  segue il nuovo nome.** Con l'evento `notify` che fornisce vecchio e nuovo
  percorso, l'editor aggiorna il percorso del documento (titolo, scheda,
  radice/progetto se serve), senza ripristino al vecchio nome né duplicati,
  con avviso informativo ("File rinominato dall'esterno in «nome»"). Evento
  come cancellazione + creazione non abbinabili, o nuova posizione non
  rilevabile (es. fuori dalla cartella osservata) → caso "cancellato"
  (verifica di integrità → ripristino al percorso originale). La cronologia
  F5-013 (chiave derivata dal percorso) si associa al nuovo percorso:
  *dettaglio implementativo proposto* ri-chiavatura della sottocartella con
  `percorsi_precedenti` in `meta.json` (alternativa: alias). Scartato il
  ripristino al vecchio nome: dopo una rinomina voluta (es. `git mv`)
  creerebbe un duplicato. Dettaglio in PIANO.md F5-022 e F5-013.
- **P5-A:** il registro di F5-006 si progetta **già** compatibile con
  sottomenu e titoli tradotti (annotato in F5-006).
- **P5-B:** `@floating-ui/dom` da bloccare in PIANO.md §3 al momento
  dell'implementazione.
- **P5-E:** implementazione in Rust nel core/guscio con il crate `grep` di
  ripgrep o equivalente, scelta con `research-scout`; **niente sidecar
  `rg`** (coerente con D-010).
- **P5-F:** italiano + **inglese**, lingua nelle impostazioni (F5-003); altre
  lingue aggiungibili con un file per lingua.
- **P5-H:** subordinata alla verifica di cosa offre `typst-ide` 0.15
  (`research-scout` come prima attività della task). Prima ricognizione sul
  sorgente: nessuna "signature help" esposta; costruibile nel core con
  `analyze_expr` + `Func::params()` — da confermare.
- **P5-I:** variabili **salvate nel progetto** (file nella cartella-progetto,
  versionabile), passate al `World` di compilazione: punto di aggancio
  `Library::default()` in `EditorWorld::nuovo`
  (`crates/typst-core/src/world.rs`) → `Library::builder().with_inputs(...)`.
  Eccezione deliberata alla "cartella di progetto pulita" di D-012: il dato
  appartiene al progetto.

**Motivazione:** la Fase 5 resta centrata su rifinitura e distribuzione e
accoglie solo ciò che l'utente vuole nella prima versione distribuita (barra
laterale, file esterni, scrittura concentrata); il resto, che dipende da basi
della Fase 5 (registro comandi, impostazioni, barra laterale) o richiede
ricognizioni tecniche (ricerca, scheda parametri), forma una fase propria
che non ritarda gli installer.
**Alternative scartate:** tutto in Fase 5 — DoD troppo ampia, installer
rinviati; proposte lasciate fuori dalle fasi come X-001 — scartato: sono
accettate e pianificate, non estensioni facoltative; ricerca in Fase 5 —
scartato su indicazione dell'utente (Fase 5 = barra, file esterni,
focus/macchina da scrivere).
**Effetti sui documenti:** PIANO.md — Fase 5 (stato, obiettivo, nuovo Blocco
F con F5-019…F5-023, note in F5-003/F5-006/F5-013/F5-014, ordine
consigliato, DoD), tabella di tracciabilità al posto della sottosezione
"Proposte opzionali", nuova sezione "Fase 7 — Estensioni", §3 (dipendenze
previste `sha2`, `notify`, `@floating-ui/dom`, crate di ricerca); PROGETTO.md
§10 (voci Fase 5 e nuova Fase 7).

### D-015 — Perimetro di indice, albero file, ricerca, lingue e menu `/`; backlog "Dopo la Fase 7"
**Data:** 2026-09-28
**Stato:** deciso dall'utente (punti 1–5); punto 6 **chiuso con D-016**
(2026-09-28, decisione provvisoria).
**Contesto:** le task di D-014 lasciavano aperti alcuni dettagli di
perimetro (F5-020, F5-021, F7-003, F7-004, F7-006, F7-007).
**Decisioni:**
1. **F5-020 Indice → tutto il documento**, ricavato dal **documento
   compilato**: titoli di tutti i file richiamati con `#include` (e generati
   da codice); clic → apre il file giusto al punto giusto con la mappatura
   span→sorgente già usata da F4-003. Verificato sui crate 0.15:
   `EditorWorld::ultimo_documento` (`world.rs` ~183) →
   `PagedDocument::introspector()` (`typst-layout`, `document.rs` ~43) →
   `Introspector::query(&Selector)` sugli `HeadingElem` (`typst-library`,
   `introspection/introspector.rs`), con `Content::span()`,
   `plain_text()`, `resolve_level`, `outlined`. Compilazione fallita →
   **ultimo indice valido**.
2. **F5-021 Albero dei file → solo consultazione** in Fase 5. Le operazioni
   (crea/rinomina/elimina/sposta con aggiornamento degli `#include`) vanno
   nel backlog "Dopo la Fase 7" (B-OP-01).
3. **F7-003 Ricerca:** nei file con modifiche non salvate si cerca nel
   **testo dell'editor** (confermato). "Sostituisci in tutto il progetto" →
   backlog (B-OP-02).
4. **F7-004 Lingue:** in Fase 7 si traducono **solo i testi
   dell'interfaccia** (frontend); i messaggi di core/guscio Rust restano in
   italiano ("poi vedremo" → backlog B-OP-03). Cambio lingua **immediato,
   senza riavvio**.
5. **F7-006 Menu `/`:** confermata la regola "`/` seguito da spazio chiude il
   menu e la riga resta una lista di termini". Aggiunta una nota di
   correzione in SPEC-UNIFICATA.md §3.1 (che dava `/` per libero nel markup;
   la tabella originale resta invariata).
6. **F7-007 `sys.inputs` — chiuso con D-016 punto 4** *(testo originale
   conservato per traccia)*: alla proposta di dettaglio (file
   `typster-inputs.json` con oggetto nome→valore stringa; in documento
   singolo variabili solo di sessione con avviso) l'utente ha risposto "no"
   senza precisare a quale parte. Resta decisa solo la persistenza **nel
   progetto** (D-014); nome/formato del file e comportamento in documento
   singolo sono da chiarire prima di avviare la task. → Chiarito il
   2026-09-28 (D-016 punto 4): in documento singolo le variabili restano
   salvate anche dopo la chiusura (non solo di sessione); nei progetti file
   `typster-inputs.json` creato solo alla prima variabile.

**Motivazione:** la Fase 5 resta contenuta (albero in sola lettura, nessuna
operazione distruttiva sui file del progetto); l'indice dal documento
compilato è l'unico che segue davvero `#include` e titoli generati, e riusa
dati che il core già conserva; la Fase 7 traduce ciò che l'utente vede senza
toccare il contratto dei messaggi del core.
**Alternative scartate:** indice dall'AST del solo file corrente (non segue
gli `#include`, ignora i titoli generati); operazioni sui file già in Fase 5
(rischio di perdita dati, aggiornamento degli `#include` non banale);
traduzione dei messaggi Rust in Fase 7 (richiede di cambiare il contratto
core→frontend in codici + parametri); cambio lingua al riavvio.
**Effetti sui documenti:** PIANO.md F5-020, F5-021, F7-003, F7-004, F7-006,
F7-007 e nuova sezione "Dopo la Fase 7 (backlog)" (B-OP-01…03);
SPEC-UNIFICATA.md §3.1 (nota di correzione).

### D-016 — Catalogo di riuso `fase8.md`: proposte dell'analista accettate, licenze (F5-024), `sys.inputs` in documento singolo
**Data:** 2026-09-28
**Stato:** deciso dall'utente; attuazione nelle task di PIANO.md §4 citate
sotto. Punto 4 = **decisione provvisoria, da rivedere dopo l'uso**.
> *Nota (2026-09-29):* **punto 1.1 superato da D-017** (un solo watcher
> `notify` ricorsivo nel core; niente feature `"watcher"` di typst-kit né
> `notify` nel guscio).
**Contesto:** a contratto definito, il subagent `analyst` ha analizzato il
codice di altri programmi (Typst CLI/kit/pdf/svg/ide 0.15.0, Tinymist,
ripgrep, MarkText, deathau, davisriedel, Typstify, TyX, Muya, Typora) e ha
prodotto un catalogo di estratti riusabili con URL, commit e licenza, una
tabella task → verdetto COPIA/ADATTA/NIENTE, gli obblighi di licenza per
fonte e 10 proposte di modifica al contratto. Restava inoltre aperto il
punto 6 di D-015 (F7-007).
**Decisioni:**
1. **Accettate le 10 proposte dell'analista** (riflesse nelle task, ciascuna
   con la sezione di [`fase8.md`](fase8.md) da cui deriva):
   1. **F5-022** (§7): tolta la ricognizione `research-scout` "notify vs
      debouncer". Feature `"watcher"` di `typst-kit` (notify 8, batching
      integrato) per i file fratelli via `FileStore::dependencies()` (già in
      `world.rs`); secondo watcher nel guscio (cartella, ricorsivo) per
      rinomina/albero, con filtro e batching copiati da typst-kit; anti-loop
      per **contenuto** (macchina a stati di Tinymist) + registro SHA-256
      delle scritture proprie; finestra temporale + `mtime` solo come
      ripiego; su Windows le rinomine arrivano come `From`/`To` separati →
      abbinarli nel batch, altrimenti caso "sparito" (D-014). Nessun crate
      debouncer.
   2. **F5-009** (§4.2): valutazione chiusa a favore del **dedup a valle**
      per id glifo stabili (`svg_merged` scartato). **F5-008** (§4.1): il
      core restituisce **metadati per pagina** (dimensioni + `hash128`) e
      l'SVG solo a richiesta.
   3. **F5-011** (§3): regola `tagged = page_ranges.is_none()` e rifiuto a
      monte delle varianti `A_*a`/UA-1 con un intervallo di pagine; copia di
      `Pages`/`output_template`/`png_options` da typst-cli v0.15.0;
      `typst-render = "=0.15.0"` senza `tiny-skia` diretto.
   4. **F7-005** (§11): ricognizione `research-scout` **già fatta**.
      Perimetro: funzioni native → nomi, tipi da `CastInfo`, documentazione,
      default, parametro corrente; funzioni `let` → nomi + pos/named/sink +
      default dei nominali; senza compilazione valida ripiego sui globali.
   5. **F7-003** (§10): `grep = "=0.4.1"` (o `grep-regex = "=0.1.14"` +
      `grep-searcher = "=0.1.17"`) + `ignore = "=0.4.33"`; annullamento via
      `AtomicBool` nel `Sink`; `search_slice` per i buffer non salvati;
      protocollo su `Channel` con `searchId`. PIANO.md §3 aggiornato.
   6. **F5-012** (§5): `FsPackages::system_cache().latest_version` + indice
      già in cache + `parse_package_import` (Tinymist) per gli `#import`;
      niente crate `semver`.
   7. **F5-020** (§6): `OutlineNode::build_tree` con `outlined`; ripiego
      `body.span()` per gli span *detached*; test "livello saltato".
   8. **F5-023** (§8): `extension.ts` di deathau (MIT) nel `Compartment`
      della macchina da scrivere; focus scritto in casa con
      `Decoration.line`.
   9. **F5-006 / F7-001 / F7-004** (§9): registro modellato su
      `CommandDescriptor` di MarkText + `run`/`unload`/`search` asincroni;
      evento unico "lingua cambiata"; "Backspace su campo vuoto = risali".
   10. **F7-007** (§12): parsing `chiave=valore` come `parse_sys_input_pair`
       (divisione al primo `=`, `trim`); file tollerante (invalido →
       nessuna variabile, avviso non bloccante).
2. **Nuova task F5-024 — licenze del codice derivato** (Blocco G di
   PIANO.md, nell'ordine consigliato prima di F5-016, e nella DoD della Fase
   5): file `NOTICE` unico in radice ("Contiene codice derivato da Typst /
   Tinymist, Apache-2.0" + MIT/Unlicense dove serve), cartella `licenses/`
   con i testi Apache-2.0 e MIT, commento
   `// Derivato da <progetto> <file> (<commit>), licenza <X>` in testa a
   ogni blocco copiato. Verifica: ogni blocco COPIA di `fase8.md` copiato
   nei sorgenti ha il suo commento; `NOTICE` incluso nell'installer
   (F5-016). Obblighi per fonte da `fase8.md` §0.
3. **Il catalogo di riuso diventa `fase8.md`** in radice (versionato), per
   indicazione dell'utente ("riscrivilo con il nome fase8.md"); prima era
   `sorgenti/riuso-fase5-7.md`, non versionato perché `sorgenti/` è in
   `.gitignore`. Contenuto invariato salvo l'intestazione. Il nome **non**
   indica una "Fase 8" del piano: il catalogo serve le Fasi 5 e 7.
4. **F7-007, documento singolo** (chiude D-015 punto 6) — l'utente sceglie
   "b, per ora, poi vediamo come si comporta il sistema": con un documento
   singolo (fuori da un progetto) le variabili **non** valgono solo per la
   sessione ma **restano salvate** dopo la chiusura. *Dettaglio
   implementativo proposto:* salvate nella cartella dati dell'app
   (`…/typster/`), associate al file con la **stessa chiave SHA-256 del
   percorso** della cronologia (D-012, riuso), ri-chiavate alla rinomina
   come F5-022/D-014. Per i progetti resta il file nel progetto,
   `typster-inputs.json`, **creato solo alla prima variabile** (come
   Typstify, che però lo chiama `sys-inputs.json`). **Decisione
   provvisoria, da rivedere dopo l'uso.**

**Motivazione:** le proposte dell'analista si basano su API e sorgenti
verificati sulla versione 0.15.0 pinnata e riducono codice scritto in casa e
ricognizioni residue (due `research-scout` eliminati: F5-022, F7-005; uno
evitato per F5-009 e F7-003); il riuso di codice Apache-2.0/MIT impone
obblighi che vanno pianificati prima del primo installer (F5-024). Per il
documento singolo, la chiave SHA-256 già prevista per la cronologia evita un
secondo meccanismo di associazione file → dati e mantiene pulita la cartella
del documento (D-012).
**Alternative scartate:** crate `notify-debouncer-*` (il batching di
typst-kit è già collaudato e senza dipendenze in più); `svg_merged`
(ricostruzione integrale a ogni pagina, incompatibile con F5-008); crate
`semver` (`PackageVersion` è già `Ord`); sidecar `rg` (D-010); variabili di
sola sessione per il documento singolo (scartate dall'utente); file
`sys-inputs.json` accanto al documento singolo (sporcherebbe la cartella,
contro D-012); catalogo lasciato in `sorgenti/` (non versionato, i dev non lo
troverebbero nel repo).
**Effetti sui documenti:** PIANO.md — stato Fase 5, §3 (feature `watcher`
di typst-kit, `notify` 8 senza debouncer, `grep`/`ignore` bloccati, nota su
`typst-render`/`semver`), F5-006, F5-008, F5-009, F5-011, F5-012, F5-016,
F5-020, F5-022, F5-023, nuova F5-024 (Blocco G), ordine consigliato e DoD
della Fase 5; F7-001, F7-003, F7-004, F7-005, F7-007 (marcatura APERTO
tolta), ordine e DoD della Fase 7. D-015 punto 6 chiuso. PROGETTO.md §10
(voce `sys.inputs`). `fase8.md` creato in radice da
`sorgenti/riuso-fase5-7.md` (eliminato).

### Passaggio a develop (2026-09-29)

**Ciclo di vita:** da `setup` a `develop`, su dichiarazione esplicita
dell'utente. **Motivo:** avvio dell'implementazione della Fase 5, a partire da
F5-001 (rinomina) e F5-003 (impostazioni), poi completamento della fase.
**Effetti sui documenti:** PIANO.md — campo `Ciclo di vita: develop` in testa.

## Fase 5 — avanzamento (2026-09-29)

> Implementazione delle task F5-001, F5-003…F5-014 e F5-022. Verifica:
> build + test + clippy (`-D warnings`) + `tsc` verdi. Lo **smoke test
> sull'exe di produzione** resta in capo all'utente e **non è ancora fatto**:
> ogni task qui sotto è quindi *implementata — in attesa di smoke di
> produzione*, non completata.
>
> *Aggiornamento 2026-09-30:* implementate anche F5-015, F5-016,
> F5-019…F5-021, F5-023 e F5-024 (stesso stato). Restano F5-017 e F5-018;
> smoke di produzione e gate veraPDF (F5-011) aperti.
>
> *Aggiornamento 2026-09-30 (bis):* implementata F5-017 e misurata F5-018
> (stato DoD sotto): **tutte** le task F5-001…F5-024 sono implementate
> (F5-002 documentale). Aperti per la chiusura: smoke dell'utente sull'exe di
> produzione, veraPDF (F5-011), smoke macOS/Linux (F5-017).

### F5-001 — Rinomina in Typster *(implementata — in attesa di smoke di produzione)*
Nome del prodotto Typster (D-009); costante `NOME_CARTELLA_DATI` per la
cartella dati `…/typster/`; migrazione di `snippets.json` **per copia** dalla
vecchia cartella; eseguibile `typster.exe`.

### F5-003 — Impostazioni persistenti *(implementata — in attesa di smoke di produzione)*
File `impostazioni.json` con parsing **tollerante per campo** (un campo
invalido torna al predefinito senza scartare gli altri) e `normalizza()`.
Scrittura atomica `scrivi_atomico` (file temporaneo + `sync_all` + rinomina),
**condivisa** tra core e guscio.

### F5-004 — Tema sistema/chiaro/scuro *(implementata — in attesa di smoke di produzione)*
Tema commutabile in un `Compartment` di CodeMirror; variabili CSS con
`light-dark()`; contrasto WCAG ≥ 4,5:1 nel tema chiaro.

### F5-005 — Dimensione del font *(implementata — in attesa di smoke di produzione)*
Intervallo 8..72; scorciatoie Ctrl/Cmd `+` / `-` / `0`.

### F5-006 — Registro comandi e palette *(implementata — in attesa di smoke di produzione)*
Registro in `src/comandi.ts` + palette accessibile; `Mod` = Cmd su macOS,
Ctrl altrove. Derivazione da MarkText (MIT, fase8.md §9; obblighi in F5-024).

### F5-007 — Font precaricati *(implementata — in attesa di smoke di produzione)*
Caricamento dei font in background: `EditorWorld::nuovo` da 138 a 4,4 ms.

### F5-008 — Anteprima della sola pagina visibile *(implementata — in attesa di smoke di produzione)*
Il core restituisce metadati + hash per pagina; SVG a richiesta col comando
`svg_pagina`; pagine caricate con `IntersectionObserver`. Documento di 60
pagine: da 120,7 a 12,7 ms per battitura.

### F5-009 — Deduplicazione SVG *(implementata — in attesa di smoke di produzione)*
Dedup a valle di **tutti** i blocchi `<defs>`, con id `typst-svg` = hash del
contenuto. Riduzione: −20,5% su lorem, −70,7% su documento ricco.

### F5-010 — Debounce adattivo *(implementata — in attesa di smoke di produzione)*
Ritardo = `clamp(base/2 + durata, base/2, 2·base)`, con `durata` = tempo
dell'ultima compilazione.

### F5-011 — Export PNG/SVG e PDF/A *(implementata — in attesa di smoke di produzione; gate esterni aperti)*
PNG/SVG con intervallo di pagine e PPI ≤ 1200 / limite 100 Mpx; PDF/A in UI
A-2b / A-3b / A-4 (D-011); timestamp UTC via `chrono`, richiesto da PDF/A;
**un'unica conferma** prima di sovrascrivere i file `nome-NN.ext`.
**ANCORA DA FARE:** gate manuale veraPDF e confronto con la CLI `typst` 0.15.

### F5-012 — Aggiornamenti dei pacchetti *(implementata — in attesa di smoke di produzione)*
Versioni più recenti dall'indice già in cache; confronto con `PackageVersion`
(`Ord`, niente `semver`); badge + rilevazione degli import del documento.

### F5-013 — Salvataggio automatico e cronologia *(implementata — in attesa di smoke di produzione)*
Autosave + cronologia in `%APPDATA%\typster\cronologia\<sha256>\` (D-012) con
`meta.json`, rotazione delle versioni, nomi degli snapshot in UTC.

### F5-014 — Ricarica delle risorse del progetto *(implementata — in attesa di smoke di produzione)*
`invalida_risorse` azzera il `FileStore`; comando "Ricarica risorse del
progetto" nel registro.

### F5-022 — Controllo dei file modificati da fuori *(implementata — in attesa di smoke di produzione)*
Watcher `notify` ricorsivo unico (D-017); registro SHA-256 delle scritture
proprie (anti-loop); `Batcher` 100/500 ms; decisione ricarica / chiedi /
sparito (D-014). **Protezione perdita dati emersa in revisione:** la
scrittura del documento è **rifiutata con `Conflitto`** se il contenuto su
disco differisce dall'ultimo contenuto noto.
Criteri di integrità del file sparito confermati come implementati: D-024.

### F5-015 — Profilo Cargo di release *(implementata — in attesa di smoke di produzione)*
`[profile.release]` nel `Cargo.toml` radice: `lto = true`,
`codegen-units = 1`, `opt-level = "s"`, `strip = true`, `panic = "abort"`
(D-013; crash log in D-023). `typster.exe`: **114.223.104 → 40.569.856 byte
(−64%)**. Build completa con LTO ≈ **8 min**.

### F5-016 — Pacchettizzazione *(implementata — in attesa di smoke di produzione)*
**Windows:** installer Inno Setup portabile (`tools/installer.ps1`, D-019)
`setup-typster-0.1.0.exe` **15,8 MB**, provato: installa, avvia, disinstalla;
`WebView2Loader.dll` inclusa accanto all'exe (B-004). NSIS/MSI (Tauri) e
macOS/Linux **solo configurati** in `tauri.conf.json`, non verificati qui.
**Da rifare:** l'installer provato è stato prodotto **prima** di `NOTICE` e
`licenses/` (F5-024): va ricostruito. Icona ancora segnaposto (D-020).

### F5-019 / F5-020 / F5-021 — Barra laterale, indice, albero file *(implementate — in attesa di smoke di produzione)*
Barra laterale a **registro di pannelli** (`src/barra-laterale.ts`,
`src/pannelli/`), browser pacchetti come primo pannello. Indice da
`OutlineNode::build_tree` (core `indice.rs`, fase8.md §6). Albero file in
**sola consultazione**, confinato alla radice del progetto (core
`albero.rs`). Overlay **"file in editing ≠ main"**: aprire un capitolo
dall'albero o dall'indice lo porta nell'editor senza cambiare il file
principale della compilazione del progetto.

### F5-023 — Modalità focus e macchina da scrivere *(implementata — in attesa di smoke di produzione)*
Focus scritto in casa (`Decoration.line`); macchina da scrivere derivata da
deathau/cm-typewriter-scroll-obsidian (MIT, commento di derivazione, F5-024).
Scorciatoie da rivedere con D-022.

### F5-024 — Licenze del codice derivato *(implementata — in attesa di smoke di produzione)*
`NOTICE` in radice + `licenses/` con Apache-2.0 e MIT. I testi sono quelli
**canonici** con le righe di copyright di fase8.md §0, perché i sorgenti
originali delle fonti non sono disponibili in locale. `bundle.resources` in
`tauri.conf.json`; `tools/installer.ps1` **esige** `NOTICE` e `licenses/`
(fallisce se mancano).

### F5-017 — Portabilità multipiattaforma *(implementata — in attesa di smoke di produzione (Windows) e smoke macOS/Linux (macchine non disponibili))*
- **Scorciatoie:** `Mod` = Cmd su macOS / Ctrl altrove per **tutte** le
  scorciatoie (`rilevaMac` / `rilevaWindows` in `src/comandi.ts`,
  rilevamento unico con ancoraggio `^Win`). Tabella definitiva in D-022.
- **Percorsi nel frontend:** `normalizzaPercorso` in
  `src/modificheesterne.ts` dipende dalla piattaforma **come nel core**
  (minuscole e `\` → `/` **solo su Windows**).
- **Cartelle dati/cache:** tramite il crate `dirs`.
- **Dialog:** filtri coerenti fra Apri, Salva con nome ed Esporta.

**CHECKLIST di smoke** (da ripetere su Windows, macOS e Linux):
1. Scorciatoie: `Mod+N/O/S/E/P`, `Mod+Shift+O`, `Mod+Shift+B`,
   `Mod+Shift+E`, `Mod+Shift+D`, `Mod+Shift+T` e `F9` fanno ciò che dice la
   tabella (D-022); `Mod++` / `Mod+-` / `Mod+0` cambiano il font; su macOS
   `Ctrl+lettera` non agisce; su Windows con tastiera italiana nessuna
   combinazione richiede `AltGr`.
2. `Mod+B` non apre/chiude la barra (libera per il grassetto);
   `Mod+Shift+F` non attiva la modalità focus.
3. Con export o cronologia aperti, `Mod+O/N/P` non fanno nulla; `Esc`
   chiude il modale.
4. Tooltip e palette: `Ctrl+…` su Windows/Linux, `⌘…` su macOS.
5. Apri, Salva con nome ed Esporta PDF/PNG/SVG: filtri corretti, percorsi
   con spazi e accenti, estensione rispettata.
6. Modifica non salvata poi Nuovo / Apri / Chiudi finestra / Nuovo da
   template: compare "Salva / Non salvare / Annulla" (D-018); verificare
   Salva su file esistente e su documento nuovo, Non salvare (versione in
   cronologia), Annulla; Salva con file modificato da fuori (deve annullare)
   e con file cancellato.
7. Modifica esterna del documento (ricarica / chiedi / sparito / rinomina) e
   cambio di maiuscole nel percorso: su Windows stesso file, su Linux/macOS
   file distinti; cartella con link simbolico su macOS/Linux.
8. Cartelle config/cache nella posizione standard dell'OS; ripristino di
   barra, tema e modalità dopo il riavvio.
9. Clic (anche tasto centrale) su un link nell'anteprima: non naviga fuori
   dall'app; il clic sulla pagina sposta ancora il cursore.
10. Smoke sull'exe di produzione (CSP e asset, cfr. B-005).

### F5-018 — Misure NFR e stato DoD *(misurata 2026-09-30)*
Misure del 2026-09-30 sul PC di sviluppo (Windows 11), con script
ripetibili `tools/misura_nfr.ps1` + `tools/installer.ps1` e
`CARGO_TARGET_DIR` impostata.

**Fase 5 — stato DoD**

| Voce | Misura |
|---|---|
| `typster.exe` | 38,77 MB (40.649.216 B) |
| setup Inno | 15,13 MB (15.864.753 B), include `NOTICE` + `licenses/` (3 testi) — installazione di prova silenziosa in `%TEMP%`: installa / avvia / disinstalla ok |
| avvio → finestra visibile | mediana 141 ms su 5 avvii caldi (132–143); primo avvio 1031 ms (non davvero a freddo: cache WebView2 presumibilmente calda) |
| memoria a riposo (10 s) | Working Set 374,2 MB / Private 215,5 MB in 8 processi; `typster.exe` WS 30,5 MB / Private 11,1 MB; 7 processi `msedgewebview2` WS 343,7 MB / Private 204,4 MB — dominata da WebView2 |
| latenza anteprima (compilazione + 3 pagine per battitura, release) | 1 pagina mediana 2,7 ms; 60 pagine mediana 16,8 ms |
| offline | nessuna rete all'avvio né per compilare documenti senza pacchetti: `ureq` solo in `packages.rs` `scarica_indice` (comando `elenca_pacchetti`, chiamato quando il pannello pacchetti viene mostrato; offline-first con cache e ripiego su cache stantia); `typst-kit` scarica solo `@preview` mancanti |
| test | `cargo test --workspace` 267 + 3 (guscio: navigazione) verdi; `npm run check` verde (8 gruppi) |

**Esito DoD:** requisiti non funzionali di PROGETTO.md §9 soddisfatti per
leggerezza relativa (D-021), reattività (ben sotto "poche centinaia di ms"),
affidabilità (ultima anteprima valida), offline-first; **portabilità
verificata solo su Windows**.
**Aperti per la CHIUSURA della Fase 5:** smoke interattivo dell'utente
sull'exe di produzione; veraPDF + confronto con la CLI `typst` (F5-011);
smoke macOS/Linux (F5-017); icona definitiva (D-020 — bloccante per il
rilascio, non per la fase).

### Deviazioni di implementazione Fase 5 (2026-09-30)
- **Debounce adattivo (F5-010):** `clamp(base/2 + durata, base/2, 2·base)`
  invece di un valore fisso affinato.
- **Dedup SVG (F5-009):** nel **frontend**, su **tutti** i tipi di `<defs>`
  (non solo i glifi).
- **Standard PDF (F5-011):** lo standard scelto si salva nelle impostazioni
  **solo dopo un export riuscito**.
- **Test del frontend:** file `*.check.ts` in `src/` eseguiti con
  `node --experimental-strip-types` (Node ≥ 22.6, `npm run check`) —
  deviazione dalla convenzione `tests/`.
- **Test unitari del guscio impossibili su questa macchina** (0xC0000139:
  l'exe di test non ha il manifest comctl32 v6) → bug **B-006**; la logica
  testabile vive nel core (vedi D-017).
- **Exe GNU:** `typster.exe` richiede `WebView2Loader.dll` accanto
  (integrazione in B-004).

### Audit completo 2026-09-30 *(corretto)*
Bug trovati e correzione applicata:

- link cliccati nell'anteprima che navigano fuori dall'app → link bloccati
  nel frontend + `on_navigation` nel guscio;
- file principale congelato in memoria all'apertura di un capitolo → il
  main si legge dal `FileStore`, senza copia congelata;
- scorciatoie attive sotto i modali → consumate finché un modale è aperto;
- chiusura della finestra con autosave → autosave **atteso** alla chiusura +
  domanda "Salva / Non salvare / Annulla" (D-018); se la domanda fallisce,
  ripiego a due pulsanti e secondo clic su X per chiudere;
- scansione dei font all'apertura di un `.md` → apertura dall'albero senza
  ricreare i font;
- export PDF eseguito tenendo il lock del `World` → export a lock
  rilasciato;
- marcatori di sincronizzazione stantii → contatore delle richieste di sync;
- `snippets.json` scritto in modo non atomico → scrittura atomica;
- `@codemirror/view` usato ma non dichiarato → dichiarato in
  `package.json`; `comemo` diretto inutilizzato rimosso (PIANO.md §3).

Correzioni aggiuntive nello stesso giro:
- panic hook con `crash.log` (D-023);
- `Rescan` del watcher su overflow degli eventi;
- rinomina seguita **solo** a contenuto uguale;
- `FileId` con la grafia su disco (limite residuo: `#include` con grafia
  diversa da quella su disco);
- "Nuovo da template" chiede (D-018) **prima** di agganciare il progetto;
- permesso `dialog:allow-message` aggiunto alle capability.

### D-017 — Watcher unico nel core per F5-022 *(deviazione da D-016 punto 1.1)*
**Data:** 2026-09-29
**Stato:** approvata dall'utente il 2026-09-29; attuata in **F5-022**.
**Contesto:** D-016 prevedeva la feature `"watcher"` di `typst-kit` per i
file fratelli + un secondo watcher `notify` nel guscio per cartella/rinomine.
**Decisione:** **un solo** watcher `notify` ricorsivo sulla radice, **nel
core** (`crates/typst-core/src/sorveglianza.rs`); il guscio inoltra gli
eventi. **Niente ripiego su `mtime`**: il confronto per contenuto copre anche
le riemissioni ritardate.
**Motivazione:** `typst_kit::watcher::Watcher::wait()` blocca senza timeout
né annullamento e non dice quale file è cambiato (thread appeso al cambio di
progetto); logica nel core perché i test unitari del guscio su questa
macchina vanno in crash (0xC0000139: l'exe di test non ha il manifest
comctl32 v6), quindi solo nel core è testabile.
**Perimetro escluso:** dipendenze fuori dalla radice (es. pacchetti `@local`)
→ comando "Ricarica risorse del progetto" (F5-014).
**Alternative scartate:** feature `"watcher"` di typst-kit + secondo watcher
nel guscio (D-016) — per i motivi sopra; ripiego su `mtime` — superfluo.
**Effetti sui documenti:** PIANO.md §3 (`notify = "=8.2.0"` nel core, tolti
feature `"watcher"` e `notify` nel guscio).

> **Decisioni del 2026-09-30 (D-018…D-025):** proposte presentate all'utente
> e **accettate in blocco** il 2026-09-30.

### D-018 — Cambio di documento con modifiche non salvate: "Salva / Non salvare / Annulla"
**Data:** 2026-09-30
**Stato:** approvata dall'utente; attuazione nel frontend (`src/fileops.ts`,
`src/main.ts`).
**Contesto:** con F5-013 il cambio di documento con modifiche non salvate
copiava il buffer in cronologia e proseguiva senza chiedere: nessuna perdita
di dati, ma il file su disco restava alla versione precedente senza che
l'utente lo decidesse.
**Decisione:** domanda a tre scelte.
- **Salva** → salvataggio normale; se fallisce o va in `Conflitto` (F5-022)
  l'operazione **si annulla**. Documento mai salvato: Salva = **Salva con
  nome** (annullare il dialog annulla l'operazione).
- **Non salvare** → copia in cronologia (F5-013) e si prosegue.
- **Annulla** → nessun effetto.

Vale per: apri, apri cartella, nuovo, apertura da albero/indice, conversione
md↔typst, chiusura della finestra.
**Motivazione:** comportamento atteso da qualunque editor; la copia in
cronologia resta come rete di sicurezza per "Non salvare".
**Alternative scartate:** "copia in cronologia e prosegui" di F5-013
(**sostituito**): l'utente non sceglie; salvataggio automatico silenzioso —
scriverebbe su disco senza consenso.
**Effetti sui documenti:** PIANO.md F5-013 (nota).

### D-019 — Canale installer Windows: Inno Setup portabile
**Data:** 2026-09-30
**Stato:** approvata dall'utente; attuata in **F5-016** (`tools/installer.ps1`).
**Contesto:** i bundler NSIS/MSI di Tauri non sono verificabili su questa
macchina (toolchain GNU, niente MSVC, B-004); serviva un canale Windows
provato.
**Decisione:**
- canale **ufficiale** Windows = **Inno Setup portabile** con
  `tools/installer.ps1` (offline, compatibile con la toolchain GNU);
- NSIS/MSI restano in `tauri.conf.json` **solo per CI/altre macchine**, non
  verificati qui;
- **WebView2** = prerequisito **documentato** del setup Inno (di serie su
  Windows 11; su Windows 10 può mancare) finché non si aggiunge un
  bootstrapper;
- **AppId esplicito e fisso**, per non legare l'identità dell'installazione
  al nome visibile. **AppId:** `628922B9-55E0-4255-9A4E-35A5049F89FD`,
  fissato in `tools/installer.ps1` e passato a Inno con `/DAppId=`.

**Motivazione:** unico canale verificabile end-to-end in locale (installa,
avvia, disinstalla: F5-016); AppId stabile = aggiornamenti e disinstallazione
coerenti anche se cambia il nome.
**Alternative scartate:** NSIS/MSI di Tauri come canale ufficiale (non
verificabili qui); bootstrapper WebView2 subito (rimandato).

### D-020 — Icona definitiva prima del rilascio
**Data:** 2026-09-30
**Stato:** approvata dall'utente; **azione dell'utente**, aperta.
**Decisione:** l'icona attuale in `src-tauri/icons/` è un **segnaposto**.
Serve l'icona definitiva prima del rilascio: **bloccante per il rilascio**,
**non** per la chiusura della Fase 5.
**Motivazione:** l'identità visiva è una scelta dell'utente; le icone per
tutte le piattaforme si rigenerano con `tauri icon` in un passo.

### D-021 — Obiettivo di dimensione rivisto sui valori misurati
**Data:** 2026-09-30
**Stato:** approvata dall'utente; valore definitivo con **F5-018**.
**Contesto:** la DoD della Fase 5 indicava "pochi MB" come target indicativo.
**Decisione:** target rivisto sui valori misurati il 2026-09-29:
`typster.exe` release **40,6 MB**, setup Inno **15,8 MB**. Il valore
definitivo si registra con F5-018.
**Motivazione:** il compilatore Typst embeddato con i font incorporati pesa
già decine di MB; "pochi MB" non è raggiungibile senza togliere funzioni.
**Effetti sui documenti:** PIANO.md DoD Fase 5.

### D-022 — Vincoli sulle scorciatoie da tastiera
**Data:** 2026-09-30
**Stato:** approvata dall'utente; attuazione in **F5-017**.
**Decisione:**
- modalità focus **non** su `Mod+Shift+F` (riservata alla ricerca nel
  progetto, F7-003);
- `Mod+B` libero per il **grassetto** (la barra laterale si sposta);
- la macchina da scrivere non dipende solo da `F9` (su macOS richiede `Fn`);
- **nessuna** combinazione con `Alt` (su Windows `AltGr` = `Ctrl+Alt`,
  conflitti con i caratteri delle tastiere europee).

**Tabella definitiva** (attuata in F5-017, 2026-09-30):

| Comando | Scorciatoia |
|---|---|
| Palette comandi | `Mod+P`, `Mod+Shift+P` |
| Nuovo | `Mod+N` |
| Apri | `Mod+O` |
| Apri cartella | `Mod+Shift+O` |
| Salva | `Mod+S` |
| Esporta PDF | `Mod+E` |
| Barra laterale | `Mod+Shift+B` (era `Mod+B`) |
| Pannello file | `Mod+Shift+E` |
| Modalità focus | `Mod+Shift+D` (era `Mod+Shift+F`) |
| Macchina da scrivere | `Mod+Shift+T` e `F9` |
| Font: aumenta / riduci / ripristina | `Mod++` / `Mod+=`, `Mod+-` / `Mod+_`, `Mod+0` |

Nessuna combinazione con `Alt`. Verificate contro i keymap di CodeMirror
(commands, search, lint, language, autocomplete): nessun conflitto.
`Mod+B` e `Mod+Shift+F` restano **riservati** (grassetto, ricerca nel
progetto).
**Motivazione:** convenzioni diffuse degli editor e portabilità fra OS e
layout di tastiera.

### D-023 — Log dei crash con `panic = "abort"`
**Data:** 2026-09-30
**Stato:** approvata dall'utente; integra **D-013** senza cambiarlo.
**Decisione:** si installa un **panic hook** che appende messaggio,
posizione e ora UTC in `<config_dir>/typster/crash.log`, prima dell'abort.
**Motivazione:** con `panic = "abort"` un panic chiude l'app senza traccia;
il log rende diagnosticabili i crash in produzione.
**Alternative scartate:** tornare a `unwind` (perde il guadagno di D-013).

### D-024 — Criteri di integrità del file sparito (F5-022)
**Data:** 2026-09-30
**Stato:** approvata dall'utente; **confermati come implementati**.
**Contesto:** D-014 e PIANO.md F5-022 registravano i criteri di integrità
come "da confermare".
**Decisione:** valgono i criteri implementati in `verifica_integrita`
(`crates/typst-core/src/esterni.rs`): buffer **vuoto** o **troncato**
rispetto all'ultima versione nota → **non** si ricrea il file, si chiede
all'utente dove salvare; altrimenti voce in cronologia e ripristino al
percorso originale (D-014). La compilabilità non è un criterio.
**Effetti sui documenti:** tolti i "da confermare" in D-014 e in PIANO.md
F5-022.

### D-025 — Proposte di SPEC-UNIFICATA mai decise: accettate nel backlog
**Data:** 2026-09-30
**Stato:** approvata dall'utente; **non pianificate** (backlog).
**Contesto:** SPEC-UNIFICATA.md §3/§5 conteneva proposte mai decise, non
coperte da D-014.
**Decisione:** accettate **nella direzione** e messe nel backlog "Dopo la
Fase 7" di PIANO.md: reveal-markup-on-focus (B-OP-04), auto-pair
contestuale + Tab "esci dal formato" (B-OP-05), toggle della toolbar
(B-OP-06), input rules a inizio riga (B-OP-07), fence speciale con
anteprima (B-OP-08), cursore `{key, offset}` + sentinelle (B-OP-09).
**Motivazione:** chiude lo stato "proposta" della spec senza gonfiare le
fasi pianificate.
**Effetti sui documenti:** PIANO.md "Dopo la Fase 7"; SPEC-UNIFICATA.md
intestazione e §5.

## Bug noti

### B-001 — `tauri dev` non avviava Vite *(risolto 2026-06-28)*
Allo smoke test, `tauri dev` restava in `Waiting for your frontend dev server`
perché in `tauri.conf.json` mancavano `build.beforeDevCommand` e
`build.beforeBuildCommand`. Aggiunti `"npm run dev"` e `"npm run build"`: l'app
ora compila e si avvia in modo pulito (exit 0).

### B-002 — Finestra nera al primo avvio in `tauri dev` *(mitigato 2026-06-28)*
Allo smoke test della Fase 1, la finestra appariva nera al primo caricamento pur
con Vite attivo su :5173 e i moduli serviti correttamente (200, nessun overlay
d'errore Vite). Salvando un file (reload HMR) la UI si renderizzava correttamente
("si vede tutto"): si tratta di un **glitch di tempismo di primo caricamento** in
dev (il webview naviga su `devUrl` prima che Vite/HMR siano del tutto pronti). Non
riguarda le build di produzione (`tauri build`), che caricano asset embeddati senza
dev server.
**Mitigazione applicata:** aggiunta in `main.ts` una rete di sicurezza diagnostica
(`window.addEventListener('error'|'unhandledrejection')` → overlay a schermo) che
trasforma un eventuale guasto silenzioso in un errore visibile. *(Pro-memoria:
distinguere questo caso dal vecchio sintomo "lanciato il .exe di debug senza dev
server" — vedi modo d'avvio corretto: `npm run tauri dev`.)*

### B-003 — Sorgenti di `typst-core` persi dalla copia di lavoro *(RISOLTO 2026-07-16 — core ricostruito)*
La cartella `crates/typst-core/src/` risultava **vuota**: i 5 sorgenti del core
(`lib.rs`, `world.rs`, `packages.rs`, `config.rs`, `progetto.rs`) erano andati
persi da questa copia di lavoro. Il guscio `src-tauri/src/lib.rs`, intatto,
fissava l'API pubblica attesa del core (`EditorWorld` con
`aggiorna_sorgente`/`compila`/`esporta_pdf`/`completa`/`tooltip`/
`jump_da_click`/`jump_da_cursore`/`apri_progetto`, `config::carica_snippet`,
`packages::{elenca, installa, nuovo_da_template, pubblica_local}`,
`rileva_entrypoint`, `invalida_cache`, tipi `RisultatoCompilazione`,
`RisultatoCompletamento`, `EsitoElenco`, `Posizione`, `Snippet`,
`EntrypointRilevato`).
**Risoluzione (2026-07-16):** i 5 moduli sono stati **RICOSTRUITI da zero** da
tre fonti: questo log (DECISIONI.md), l'API pubblica fissata da
`src-tauri/src/lib.rs`, e le firme reali di Typst 0.15 lette dai sorgenti nel
registry Cargo. **Verificato:** `cargo build`/`clippy --all-targets -D
warnings`/`fmt` verdi + **41 test unitari** superati; poi revisione statica
(nessun rilievo CRITICO) con fix di robustezza di rete applicati.
**Scostamento importante dal codice perso** (documentato in
`crates/typst-core/src/lib.rs`): `invalida_cache` usa `typst::comemo::evict`
(il comemo **0.5.1** ri-esportato da `typst`) e NON il `comemo = "0.4"`
diretto del `Cargo.toml`, perché typst 0.15 traccia il proprio World con
comemo 0.5.1 — sfrattare l'altra istanza sarebbe un no-op. Vedi D-008.
Con la risoluzione, l'implementazione della Fase 6 è stata sbloccata e
completata (F6-001…F6-003).

### B-004 — Ambiente della copia di lavoro corrente: il guscio si compila su toolchain GNU con `code_tools\rust_bundle` — MSVC NON necessario *(RISOLTO 2026-07-16)*
Questa copia di lavoro è arrivata **senza** toolchain Rust, senza registry
Cargo e senza MSVC (l'ambiente accertato il 2026-06-28, tabella sopra, NON
vale qui). Un primo provisioning provvisorio (rustup host GNU + LLVM-MinGW,
`~/.cargo/config.toml` globale sul linker `-gnu`) aveva mostrato un crash
all'avvio dei test sul target `-gnu` (`STATUS_ACCESS_VIOLATION` in
`std::rt::init`, commistione di due distribuzioni MinGW nell'asm di
`psm`/`stacker`), aggirato col target `gnullvm`, e aveva portato alla
conclusione — **poi smentita** — che il guscio Tauri non fosse compilabile
senza MSVC (hand-off alla macchina dell'utente).
**Fatto nuovo verificato (2026-07-16): l'intera app si compila su toolchain
GNU, senza MSVC**, usando il bundle portabile **`code_tools\rust_bundle`**
(Rust 1.97 target `x86_64-pc-windows-gnu` + winlibs MinGW GCC 16.1.0 +
`rust-lld`). Verificato:
- `cargo build`/`clippy`/`test -p typst-core` verdi con **74/74 test sul
  target `-gnu` NATIVO**: il bundle è internamente coerente (una sola
  distribuzione MinGW), niente crash `psm`/`stacker` → il target `gnullvm`
  era solo un workaround della toolchain rustup mista e **non serve più**;
- **`cargo build -p typster` produce `typster.exe`** (all'epoca pacchetto
  ed exe si chiamavano `editor-tauri`, rinominati con F5-001)
  (`webview2-com`/`tao` compilano su gnu).
**Ricetta:** attivare il bundle (`code_tools\rust_bundle` + relativo script
di activate) e impostare **`CARGO_TARGET_DIR` a un percorso SENZA spazi**
(es. `C:\Users\<utente>\et_target`): `windres` (compilazione risorse Tauri
via `embed-resource`) invoca `cc1` sul file `.rc` e non gestisce gli spazi
del percorso (il progetto vive sotto `C:\Local Storage\`, con spazio). Il
progetto può restare dov'è: va spostato solo il target dir; l'icona con
percorso-con-spazi referenziata dentro il `.rc` va bene.
**Restano:** per **eseguire** l'app serve il runtime **WebView2** installato
(di norma presente su Win11); lo **smoke test interattivo** dell'app
(Fase 6) resta da svolgere da parte dell'utente.
**Integrazione (2026-09-30):** l'exe prodotto dalla toolchain GNU importa
dinamicamente **`WebView2Loader.dll`**, che deve stare **accanto a
`typster.exe`** (non è linkata staticamente come con MSVC). L'installer Inno
(F5-016, D-019) la include; per avviare l'exe dalla cartella di build va
copiata lì.

### B-005 — Editor CodeMirror senza stile nella build di produzione: la CSP con nonce/hash disattiva `'unsafe-inline'` *(RISOLTO 2026-07-18)*
**Sintomo.** Nell'exe di **produzione** (`tauri build` / `tauri build --no-bundle`)
l'editor CodeMirror appariva rotto: i numeri di riga (gutter) impilati **sopra** il
testo invece che affiancati, nessuno sfondo del gutter e nessun colore di sintassi
(tema `oneDark` assente). Il resto della UI (header, pulsanti, pannelli) era invece
stilizzato correttamente. In `tauri dev` il problema **non** si manifestava.

**Causa (CONFERMATA).** CodeMirror 6 non usa un CSS statico: inietta il proprio tema
base — incluso `.cm-scroller { display: flex }`, che affianca gutter e contenuto — e
`oneDark` come `<style>` **a runtime** (via StyleModule). La CSP dichiarata era
`style-src 'self' 'unsafe-inline'`. In build di produzione Tauri **modifica
automaticamente** la CSP degli asset iniettando nel `style-src` un nonce
(placeholder `__TAURI_STYLE_NONCE__`) e gli hash `'sha256-…'` degli `<style>` statici.
Per la **specifica CSP**, la presenza di un nonce o di un hash in una direttiva fa
**ignorare `'unsafe-inline'`**. Quindi in produzione gli stili statici (hashati)
passavano, ma quelli iniettati a runtime da CodeMirror — privi di nonce/hash — venivano
**bloccati**, e l'editor perdeva layout flex e colori. In `tauri dev` Tauri non modifica
la CSP, perciò `'unsafe-inline'` restava effettivo e non si vedeva nulla di rotto.
Verificato ispezionando l'exe: prima della fix `style-src` conteneva
`__TAURI_STYLE_NONCE__` + 2 hash `sha256`.

**Fix.** In `src-tauri/tauri.conf.json`, `app.security`:
`"dangerousDisableAssetCspModification": ["style-src"]`. Dice a Tauri di **non**
modificare la sola direttiva `style-src`: resta `'self' 'unsafe-inline'` senza
nonce/hash → `'unsafe-inline'` torna effettivo e gli stili runtime di CodeMirror sono
ammessi. `script-src` **non** è nella lista, quindi continua a ricevere il nonce di
Tauri e resta protetto (nessuna reintroduzione di `'unsafe-inline'` per gli script).
Verificato: ricostruito l'exe con `tauri build --no-bundle`, nel binario `style-src` è
ora servito senza hash/nonce; **smoke test interattivo dell'utente 2026-07-18: editor a
posto, tema scuro e sintassi colorata.**

**Perché non un hash statico.** Non praticabile: (a) gli stili di CodeMirror sono
generati a runtime (contenuto non fisso, variabile tra versioni della libreria) e non
hashabili in anticipo; (b) `index.html` usa anche attributi `style="…"` inline, non
copribili da hash CSP (servirebbe `'unsafe-hashes'`). Un percorso a nonce sarebbe
fragile (Tauri genera il nonce a build/run-time; CodeMirror dovrebbe riceverlo esatto).
Riabilitare `'unsafe-inline'` per la sola `style-src` è il compromesso corretto.

**Trade-off di sicurezza (accettabile).** Il rischio residuo di `'unsafe-inline'` sugli
stili è solo CSS (defacement / UI-redressing), non esecuzione di codice: `script-src`
resta protetto dal nonce (coerente con F1-004), e i canali esterni sono chiusi da
`default-src 'self'`, `img-src` (niente `url()` remoti) e `connect-src`. Nota per il
futuro (cfr. F1-R5): con `'unsafe-inline'` effettivo, un `<style>` dentro un SVG di un
pacchetto di terze parti verrebbe applicato (solo CSS) — un motivo in più per la
sanitizzazione SVG quando si renderanno pacchetti non fidati.

**Preesistente, da valutare (non parte di questa fix).** La CSP di produzione include
ancora le origini del dev-server Vite in `connect-src`
(`ws://localhost:5173 http://localhost:5173`): andrebbero spostate in
`app.security.devCsp` (solo-dev) per non allargare `connect-src` in release. Segnalato
da due revisioni indipendenti. → Attuato nella revisione del 2026-07-18 (sotto).

### B-006 — Test unitari del guscio non eseguibili su questa macchina *(APERTO, aggirato)*
**Sintomo:** gli exe di test di `src-tauri` terminano all'avvio con
**0xC0000139** (entry point non trovato).
**Causa:** l'exe di test non incorpora il manifest **comctl32 v6** che l'exe
dell'app riceve da `tauri-build`; le dipendenze di Tauri importano simboli
presenti solo in comctl32 v6.
**Aggiramento:** la logica testabile vive nel **core** (`typst-core`, test
headless verdi); il guscio resta sottile (vedi D-017). Verifica del guscio
tramite build + clippy + smoke sull'exe di produzione.
**Da valutare:** manifest incorporato anche negli exe di test, o test del
guscio su una macchina con toolchain MSVC/CI.

### Revisione completa del codice (2026-07-18) — bug trovati e risolti
Dopo la fix B-005, revisione sistematica dell'intero codice (frontend TS + core Rust
+ guscio Tauri) con più revisori in parallelo, seguita da correzione e seconda
revisione delle diff. Verificato: **core `typst-core` 95/95 test verdi** (74
preesistenti + 21 nuovi), `clippy -D warnings` pulito su core e guscio, `tsc --noEmit`
verde, exe di produzione ricostruito.

**Integrità dei dati (le più importanti).**
- **Salvataggio non atomico** (`src-tauri/src/lib.rs`, `salva_file`/`esporta_pdf`):
  `std::fs::write` troncava il file prima di riscriverlo → un salvataggio interrotto
  (disco pieno/crash/unità staccata) corrompeva il documento. Introdotta
  `scrivi_atomico` (temp nella stessa cartella + `rename`, come `pubblica_in`): il
  target non è mai troncato; su Windows `rename` di un file sovrascrive
  (MOVEFILE_REPLACE_EXISTING). Bonus: un file bloccato da un'altra app fa fallire il
  solo `rename`, lasciando l'originale intatto.
- **Conversione md→typst, escape incompleto** (`markdown.rs`, `escapa_typst`): non
  scappava `/` né `=`; `//` è un commento di riga Typst e `=` a inizio riga un titolo →
  testo come `http://x` o `and/or` perdeva tutto dopo `//`, e paragrafi che iniziano
  con `=` diventavano H1. Aggiunti `\/` e `\=` (solo su testo letterale).
- **Fence dei blocchi di codice fisso a 3 backtick** (`markdown.rs`, entrambe le
  direzioni): un blocco contenente una riga di tre backtick chiudeva il fence in
  anticipo → troncamento/iniezione. Ora il fence è dinamico (run massima di backtick +
  1, min 3), come già l'inline.
- **`@local`: versione non normalizzata nel percorso** (`packages.rs`, `pubblica_in`):
  versione `01.0.0` finiva in `.../01.0.0/` mentre Typst risolve in `.../1.0.0/` →
  pacchetto pubblicato ma non importabile (fallimento silenzioso). Il percorso usa ora
  la versione normalizzata.
- **typst→md: destinazioni link/immagine grezze** (`markdown.rs`): percorsi/URL con
  spazi o parentesi (es. `logo bianco.png`, tipico su Windows) rompevano il Markdown.
  Nuova `formatta_destinazione_md` (racchiude in `<...>`, percent-encode di `<`/`>`).

**Integrazione del pulsante "Nuovo" e conversioni.**
- **`nuovo()` non resettava la radice-progetto sul backend** (gap previsto in F4-001):
  dopo "Apri cartella" il World continuava a risolvere import/immagini relativi e font
  dalla cartella precedente. Introdotto il comando `chiudi_progetto` (backend
  `EditorWorld::chiudi_progetto`: `radice_progetto=None`, font store ricostruito senza
  font di progetto, ritorno al `FileId` virtuale `main.typ`); `fileops.nuovo()` è ora
  `async` e lo invoca prima di azzerare l'editor.
- **Race sul tipo documento** (`main.ts`): `tipoDocumento` era letto due volte (avvio
  vs ritorno dell'await) → un risultato in volo poteva essere interpretato col tipo
  sbagliato. Ora `md` è catturato una sola volta all'avvio; aggiunta anche una
  **guardia di generazione** (`generazioneDoc`) che scarta i risultati di una
  compilazione il cui documento è stato sostituito nel frattempo.
- **Race sul compartment del linguaggio** (`editor.ts`): l'import WASM Typst (lento a
  freddo) poteva risolvere dopo una scelta Markdown più recente e sovrascriverla.
  Aggiunta una guardia di versione per-istanza.
- **Documento vuoto non ripuliva l'anteprima** (`main.ts`): 0 pagine senza errori mostra
  ora il placeholder (distinto dall'errore fatale, che conserva l'ultima anteprima).
- **Un solo snippet malformato scartava l'intero file** (`config.rs`): parsing ora
  tollerante voce-per-voce (scarta solo le voci difettose; ripiego ai predefiniti solo
  se il file manca/non è leggibile/non è un array).
- **Indice pacchetti: 200 non-JSON scavalcava la cache** (`packages.rs`): dietro un
  captive portal l'errore di parsing propagava senza tentare la cache; ora ripiega
  (offline-first, F3-001).

**Robustezza / hardening.**
- Dialog `open()`/`save()` racchiusi in `try/catch` (`fileops.ts`, `pacchetti.ts`): un
  rigetto non apre più l'overlay di errore fatale a tutto schermo.
- `apri_file`: un file leggibile si apre comunque anche se l'aggancio-progetto fallisce
  (ripiego su `chiudi_progetto` + `aggiorna_sorgente`, stato deterministico).
- Retry del bridge via `avviaCompilazione` (rispetta `inCorso`); guardia re-entrancy su
  `pubblica` @local (Invio ripetuto); ricerca durante un'installazione non riabilita i
  pulsanti card; `jump_da_click` valida l'indice pagina; `valida_nome` rifiuta i nomi
  device riservati Windows; `installa` invalida la cache comemo; `prova_unico_typ`
  confronta l'estensione case-insensitive; tag lingua del fence validato; liste
  annidate non spezzate da riga vuota.
- **CSP di produzione**: origini del dev-server Vite (`ws/http://localhost:5173`)
  spostate da `csp` a `app.security.devCsp` (solo-dev); la produzione non le espone più
  in `connect-src` (il fetch dell'indice pacchetti avviene in Rust via `ureq`, non nella
  webview). Chiude il punto lasciato aperto in B-005.
- **Pulizia residui `@local`**: la rimozione dei residui orfani `.tmp-`/`.old-` è
  **basata sull'età** (soglia 1 ora), per non cancellare i temp di una pubblicazione
  concorrente (hazard che una prima versione della fix aveva introdotto, poi corretto).

**Deferito (documentato).** Stato di modulo condiviso in `assistenza.ts`
(`versioneCompletamento`/`opzioniSnippet`): corretto per l'editor singolo attuale; da
rendere per-istanza solo se si aggiungerà il multi-editor (annotato nel codice).

### Smoke test Fase 1 (2026-06-28)
**Esito: superato.** Build Rust completo (dev profile, ~6 min, con `typst-pdf`,
`tauri-plugin-dialog`, `krilla`, `rfd`, ecc.), binario avviato, UI renderizzata.
Da confermare interattivamente in una sessione successiva: evidenziazione Typst
(WASM sotto CSP), dialoghi apri/salva, export PDF su disco.

## Correzioni 2026-10-07 — frontend

- **Markdown senza sync/assistenza typst-ide.** `completa`/`tooltip` (`assistenza.ts`, flag di modulo `impostaAssistenzaTypst`, impostato da `editor.impostaTipo`) e la sincronizzazione cursore↔anteprima (`jump_da_click`/`daCursore`, parametro `abilitata` di `configuraSincronizzazione`) sono spenti quando `tipoDocumento === "markdown"`: gli offset sono del Typst generato, non del testo nell'editor. L'indice continua ad abbinare per testo.
- **Fine riga (LF/CRLF).** Nuovo modulo puro `finerighe.ts` (+ `finerighe.check.ts`): l'editor e il compilatore vedono solo `\n`; `fileops.ts` rileva la fine riga prevalente a apertura, adozione e ricarica da disco (file misti: maggioranza; parità e `\r` isolati → LF) e la riapplica a `salva_file` e `ripristina_documento_sparito`. Nuovo documento e conversione → LF. Non si usa `EditorState.lineSeparator`: avrebbe cambiato la numerazione degli offset passati al backend.
- **Rinomina esterna.** Dopo l'evento `rinominato` si chiama `apri_file(nuovo)` solo per riagganciare il World (il testo restituito si scarta: modifiche non salvate e cursore restano) e si ricompila. Il tipo di documento NON si deduce più dalla nuova estensione: se passa da `.typ` a `.md` (o viceversa) si mantiene il tipo e si avvisa (`avvisoTipoDopoRinomina`). Per coerenza `scriviSu` cambia il tipo solo su «Salva con nome» verso un altro file, non su un Salva del percorso corrente. Limite noto: `apri_file` imposta la radice del World alla cartella del file, come l'apertura normale.
- **Cronologia.** `VersioneCronologia.quando_utc?` (RFC 3339, additivo; nome del campo da allineare col backend) è formattato con `Intl.DateTimeFormat` nella lingua di `<html lang>` (`quandoversione.ts`); se assente o non valido si usa `quando` (`vista_locale`).
- **Errori di export.** Il rifiuto della destinazione da parte del backend mostra «Esportazione X non riuscita: <motivo>» sia nel riquadro errori sia in un messaggio di dialogo (`testoErroreEsportazione`); nessun overlay fatale, l'editor resta usabile.
- **Type-check.** `npm run build` esegue già `tsc`; aggiunto lo script `typecheck` (`tsc --noEmit -p tsconfig.json`). Nessun errore di tipo presente.
- **Chunk da ~976 kB: non toccato.** La grammatica Typst/WASM è già un import dinamico; il resto è CodeMirror, necessario all'avvio. Un `manualChunks` non ridurrebbe il caricamento (asset locali nell'app Tauri) e rischierebbe CSP/WASM: l'avviso resta cosmetico.

## Correzioni 2026-10-07 — core/guscio

Revisione del codice: correzioni su `config`, `sorveglianza`, `world`, `packages`,
`cronologia`, `esporta`, `albero` e guscio. Ogni punto ha test nel core (i test
unitari del guscio non girano su questa macchina, B-006).

### D-026 — Scelte non banali

1. **`scrivi_atomico` e symlink/permessi.** Il bersaglio si risolve con
   `canonicalize` (se esiste) e il temporaneo nasce accanto al file *reale*: il
   `rename` sostituisce il file puntato e il symlink resta. I permessi del file
   preesistente si copiano sul temporaneo prima del rename. Limite noto: su
   Windows gli ACL non sono copiabili con la sola std, il nuovo file eredita
   quelli della cartella. Il test del symlink si autosalta se la piattaforma
   non lo permette (qui: errore 1314, privilegio mancante), quindi su questa
   macchina il ramo symlink non è stato eseguito.
2. **Punto 11, `tempfile` non adottato.** È solo dev-dependency; promuoverla a
   dipendenza di runtime non semplifica: serve comunque copiare i permessi
   (`NamedTempFile` crea 0600) e la pulizia su errore è già di poche righe.
   `scrivi_atomico` resta a mano.
3. **Rinomina del documento: finestra temporale.** `recenti` conserva l'istante
   di comparsa; un candidato oltre `ETA_MAX_RECENTE` (5 s) non è più la nuova
   casa del documento. L'istante è iniettabile (`elabora_a`/`ricontrolla_a`).
4. **Invalidazione risorse limitata ai file letti.** Un'allow-list per
   estensione era sbagliata (Typst 0.15 legge anche `.pdf` e qualunque file via
   `read()`). Il caricatore del World registra ogni file del progetto
   richiesto, **anche se la lettura fallisce** (una bibliografia creata dopo un
   errore deve invalidare), in `RisorseLette`, condiviso col watcher
   (`Stato::letti`, impostato da `osserva_documento`). Se `letti` è `None` non
   si filtra (comportamento precedente). L'aggiornamento dell'albero non
   dipende dal filtro. Lo svuotamento avviene su `apri_progetto` /
   `chiudi_progetto`.
5. **Font: base condivisa per processo.** Embedded + sistema si costruiscono
   una sola volta (`font_base`, voci con `OnceLock` condivise); ogni store di
   progetto vi aggiunge solo `radice/fonts`. Limite: un font di sistema
   installato a programma aperto compare al riavvio. Misura (profilo debug,
   `cargo test misura_apri_progetto -- --ignored --nocapture`): `apri_progetto`
   + prima compilazione da ~490-620 ms a ~8-10 ms; avvio a freddo invariato
   (~540 ms, una volta).
6. **Cache indice pacchetti** scritta con `scrivi_atomico` (best-effort).
7. **Cronologia**: campo additivo `quando_utc` (RFC 3339, UTC) accanto a `quando`.
8. **Export**: `verifica_destinazione_export` (core) rifiuta il documento aperto
   e le estensioni `.typ/.typc/.md` (confronto case-insensitive); il guscio la
   applica a `esporta_pdf` e a ogni file di `esporta_immagini`.
9. **Percorsi verbatim**: `albero::normalizza_verbatim` toglie `\?\` solo per
   i percorsi con lettera di unità (non UNC, non `\?\Volume{..}`); il guscio la
   usa per entrypoint, radice dell'albero ed eventi `modifica-esterna`. I
   percorsi interni restano canonici (i confronti usano `stesso_file`).
10. **CSP**: aggiunti `object-src 'none'; base-uri 'self'; frame-src 'none'` a
    `csp` e `devCsp` (il frontend non usa iframe/object/embed/base). Verifica
    sull'exe di produzione (B-005) **non eseguita** da questo intervento:
    richiede smoke interattivo.
11. **Errori PDF ripetuti**: `raggruppa_errori_pdf` unisce i messaggi identici
    (conteggio, prime 5 posizioni riga/colonna nel file in editing, suggerimento
    `alt:` per "alt text").


## Esportazioni testo/HTML/LaTeX 2026-10-07

Due lavori nel core (`crates/typst-core`), entrambi **puri** (`&str -> …`, nessun I/O,
nessuna dipendenza da GUI/Tauri). Il collegamento al guscio e alla GUI è rimandato
a una fase successiva.

### Bug corretto: `typst_a_md` perdeva il `#` delle espressioni di codice

**Causa.** In `typst-syntax` il `#` che introduce un'espressione di codice è un nodo
`Hash` *separato* dall'`Expr` che lo segue; `Markup::exprs()` lo salta. Il ramo
`altro => full_text()` e il ripiego di `funccall_a_md` riemettevano quindi il sorgente
**senza** `#`: `#set`, `#show`, `#let`, `#figure`, `#footnote`, `#align`, … comparivano
come testo visibile (`set text(lang: "it")`) nel Markdown.

**Correzione.** La visita non usa più `exprs()` ma i *figli* del `Markup`, con il contesto
dei fratelli (`Hash` + espressione, spazi con a capo, etichette adiacenti).

**Meccanismo di conservazione: attributo «raw» di Pandoc `{=typst}`.**
- blocco: ```` ```{=typst} … ``` ````; span in linea: `` `…`{=typst} ``;
- `md_a_typst` li riconosce e riemette il contenuto **invariato**, quindi
  `typ → md → typ` non perde `#set`/`#show`/`#let`, math, `@riferimenti`, `<etichette>`,
  `#align`, `#v`, figure complesse, …;
- il delimitatore è sempre più lungo di ogni sequenza di backtick contenuta;
- blocchi grezzi consecutivi (separati da soli a capo) sono **raggruppati** in un unico
  fence (il preambolo `#set …` di un documento resta un solo blocco);
- un'espressione grezza in mezzo al testo diventa uno span in linea (gli a capo interni
  sono compattati; se contiene `//` — che in uno span diventerebbe un commento fuori
  posto — si emette un blocco, con avviso).
*Alternative scartate:* commenti HTML (invisibili ma non ripristinabili in modo sicuro
e perdono il contenuto nei renderer); fence con lingua `typst` (ambiguo con il codice
Typst *mostrato*, che non va riemesso come sorgente); `#` nudo nel testo (viene
escapato dall'altro verso).

**Mappature leggibili conservate** (solo se la forma è esprimibile senza perdere nulla,
altrimenti si ripiega sul grezzo): `link`, `image` (con `alt:`), `strike`/`strong`/`emph`,
`quote`, `footnote`, `figure(image(..), caption: [testo semplice])`.
- `quote`: è un blockquote se ha `block: true` **o** sta su riga propria; una citazione
  in linea nel testo resta grezza. L'`attribution:` non sparisce più: diventa una riga
  finale `> — attribuzione` **con avviso** (la conversione inversa non la rimette in
  `attribution:`: perdita dichiarata).
- `footnote` ↔ nota Markdown `[^N]` (con `Options::ENABLE_FOOTNOTES`); le definizioni
  stanno in coda.
- `figure` ↔ immagine Markdown: in `md→typ` un'immagine **sola** nel paragrafo con testo
  alternativo diventa `#figure(image(..), caption: [alt])`; con alt in mezzo al testo
  `#image(.., alt: "…")`. Cambia il comportamento precedente (alt scartato): i due test
  corrispondenti sono stati aggiornati.
- commenti Typst ↔ commenti HTML (`<!-- … -->`).
- liste annidate con rientro corretto; marcatori letterali a inizio riga (`\-`, `1\.`) protetti.

**API.** `typst_a_md(&str) -> String` invariata (usata dal guscio); nuova
`typst_a_md_con_avvisi(&str) -> Conversione` (`crate::conversione::Conversione`, esportata da
`lib.rs`) con gli avvisi: attribuzione, errori di sintassi del sorgente, numero di costrutti
conservati come Typst grezzo.

**Limiti noti.** HTML grezzo diverso dai commenti è ancora omesso in `md→typ`; una
didascalia di figura con markup (math, riferimenti, enfasi) non è esprimibile come testo
alternativo e la figura resta grezza; `quote` con `attribution` non rientra in
`attribution:` al ritorno; uno span grezzo multiriga con `//` spezza il paragrafo.

### Nuove esportazioni: testo semplice, HTML5, LaTeX (`src/conversione/`)

API pubblica (ri-esportata da `lib.rs`): `typst_a_testo(&str, &OpzioniTesto)`,
`typst_a_html(&str, &OpzioniHtml)`, `typst_a_latex(&str, &OpzioniLatex)` →
`Conversione { contenuto, avvisi }`; più `data_uri(mime, &[u8])`.

**Architettura.** Una sola rappresentazione intermedia (`ir.rs`: blocchi + inline +
etichette numerate + avvisi) costruita dal syntax tree **senza eseguire** il codice Typst;
tre generatori (`testo.rs`, `html.rs`, `latex.rs`). La math ha un proprio albero `M`
(`matematica.rs`) con tre backend (MathML, LaTeX, forma lineare Unicode): un solo parser,
nessuna divergenza fra formati.

**Scelte non banali.**
1. *Niente valutazione.* Funzioni definite nel documento (`#let capolettera(..)`) sono rese
   con i soli argomenti (stringhe e contenuti, in ordine): il capolettera `"L"` + `[a diligenza…]`
   dà «La diligenza»; avviso. `datetime(..).display(..)` è calcolato solo con argomenti
   letterali. `if/for/while/context`, variabili non letterali, `lorem`, figure disegnate
   con codice → omessi o sostituiti da un segnaposto, **sempre con avviso**.
2. *Regole di stile.* Si interpretano solo `#set document(title, author)`, `text(lang)`,
   `heading(numbering)`, `math.equation(numbering, supplement)`; le altre `#set`/`#show` sono
   conteggiate in un unico avviso.
3. *Numerazioni e riferimenti.* Titoli, figure, tabelle, listati ed equazioni sono numerati
   nel rispetto dei pattern Typst (`"1.1"`, `"I"`, `"(1)"`, `"a)"`); `@etichetta` si risolve in
   «Figura 1» / «Eq. (2)» / «Sezione 1.1» (parola localizzata it/en/fr/de/es o supplemento
   esplicito); chiavi non presenti come etichetta = citazioni bibliografiche (`[chiave]`,
   `\cite{chiave}`). La bibliografia **non viene letta** (nessun I/O): segnaposto + avviso;
   in LaTeX è emessa la vera `\bibliography{..}`.
4. *Lingua predefinita `en`* se il documento non dichiara `lang` (come Typst), con
   `OpzioniHtml.lingua`/`OpzioniLatex.lingua` per forzarla.
5. *Immagini HTML.* `src` è il percorso relativo; per incorporarle il **chiamante** (che può
   leggere i file) passa `OpzioniHtml.risolvi_immagine` (callback → data-URI, `data_uri()` fa
   il base64): il core resta senza I/O. Il risultato della callback è accettato solo se è un
   `data:image/…` o un URL sicuro.
6. *Sicurezza HTML.* Escape di `& < > " '` ovunque (testo, attributi, `<title>`, `alttext`);
   URL con schema non in {http, https, mailto, tel, ftp, relativo} rifiutati (gli spazi/tab
   inseriti per aggirare il controllo sono ignorati) → testo semplice + avviso; `id` derivati
   dalle etichette con codifica iniettiva `_xx_`, **unici** nella pagina; `lang` validato; classe
   linguistica del codice validata; nessuno `<script>` né attributo `on…` può derivare dal sorgente.
7. *MathML con ripiego.* Una formula con anche un solo costrutto non convertibile è emessa per
   intero come `<code class="formula">sorgente Typst</code>` (+ avviso aggregato), non come MathML
   parziale: più leggibile e valida. In LaTeX il ripiego è `\texttt{..}` (centrato se display).
   Le lettere in stile (`bold`, `bb`, `cal`) usano i blocchi Unicode «Mathematical Alphanumeric»
   in MathML/testo e `\mathbf`/`\mathbb`/`\mathcal` in LaTeX.
8. *LaTeX.* Pacchetti **solo se servono** (`amsmath,amssymb` con math, `graphicx` con immagini,
   `ulem` con barrato/sottolineato, `hyperref` con link/riferimenti/etichette/metadati, `babel`
   dalla lingua; lingua senza babel → avviso). `\maketitle` **non** è emesso di default
   (`titolo_automatico: false`: i documenti Typst hanno già il frontespizio); i metadati vanno in
   `\hypersetup`. Equazioni: `equation`/`equation*` secondo la numerazione, con `aligned` quando
   ci sono `&`/`\` (un solo numero per equazione, come in Typst). I riferimenti usano `\ref`/`\eqref`
   di LaTeX: i numeri sono quelli di LaTeX (liste e figure condividono contatori diversi da Typst
   per i listati). Titoli senza numerazione Typst → forma con asterisco; `report`/`book` usano
   `\chapter`. SVG non includibile da pdflatex → avviso. Caratteri speciali
   `# $ % & _ { } ~ ^ \` e `< > |`, virgolette tipografiche e NBSP sono scappati.
9. *Testo semplice.* Titoli sottolineati (`=`, `-`, `~`), tabelle ASCII a griglia con riduzione
   delle colonne alla larghezza richiesta, citazioni e codice rientrati di 4 spazi, note `[n]`
   in coda, formule lineari (`∑_(n = 1)^∞ 1/n² = π²/6`). L'enfasi non è rappresentabile e non è
   segnalata riga per riga.

**Verifiche.** Nessun compilatore LaTeX (`pdflatex`/`xelatex`/`tectonic`) è presente su questa
macchina: il `.tex` **non è stato compilato**, è verificato solo strutturalmente (ambienti
`\begin/\end` e graffe bilanciati, pacchetti, comandi attesi). I test di integrazione e
`prova_export` tentano la compilazione se un compilatore è nel PATH, altrimenti dichiarano
«SALTATO».

**Test aggiunti** (`cargo test -p typst-core`): 15 unitari math (`matematica.rs`), 6 IR
(`ir.rs`), 30 per costrutto delle tre esportazioni (`conversione/test_costrutti.rs`), 3 su
escape/URL/id (`html.rs`, `latex.rs`), 1 base64; 23 nuovi in `markdown.rs` (bug del `#`, note,
figure, quote/attribuzione, commenti, liste, round-trip); 15 di integrazione
(`tests/conversioni_documenti.rs`) sulle copie versionate di `scientifico.typ` e
`letteratura.typ` (`tests/fixtures/prova-export/`): round-trip md (stesso numero di pagine,
stessi comandi, compila senza errori), testo/HTML/LaTeX con formule, figure, citazioni e note
attese, HTML ben formato (tag bilanciati, nessun `<script>`, nessun attributo `on…`, `id`
unici), iniezioni dal sorgente, sorgenti troncati (nessun panico), Unicode, prestazioni.

**Prestazioni** (release, `prova_export`, mediana su 5): `typst_a_md` 0,2–0,3 ms,
`typst_a_testo` 0,3–0,6 ms, `typst_a_html` 0,2–0,5 ms (0,6 ms con data-URI), `typst_a_latex`
0,3–0,5 ms per documenti di 5–10 KB (7 pagine). Round-trip md: 2 → 2 pagine (scientifico),
7 → 7 (letteratura).

### Collegamento alla app delle esportazioni testo/HTML/LaTeX/Markdown (2026-10-07)

- **Comandi Tauri** (`src-tauri/src/lib.rs`): `esporta_testo(percorso, sorgente, larghezza?)`,
  `esporta_html(.., autonomo?)`, `esporta_latex(.., classe?, carta?, dimensione_pt?)`,
  `esporta_markdown(..)`. Tutti restituiscono `{percorso, avvisi, errori_sintassi}`; la
  conversione è pura e gira in `spawn_blocking` **senza lock del World**; la scrittura passa da
  `osservatore::scrivi_proprio` (atomica, registrata come scrittura propria).
- **Destinazione**: `typst_core::verifica_destinazione_conversione` (nuova, nel core, testata) =
  `verifica_destinazione_export` (rifiuta il documento aperto e `.typ/.typc/.md`) con una sola
  eccezione: l'export **Markdown** può chiamarsi `.md`, ma mai come il documento aperto.
  Classe/carta/corpo LaTeX fuori elenco sono rifiutati con errore chiaro.
- **HTML autonomo**: `typst_core::conversione::file` (unico codice con I/O del modulo).
  Le immagini sono lette **solo** sotto la cartella del documento aperto: percorso relativo (o
  `/…` = radice), solo componenti normali (no `..`, no assoluti, no `:`), estensioni
  png/jpg/gif/webp/svg, `canonicalize` + `starts_with(radice)` (blocca anche i symlink verso
  l'esterno), solo file regolari, tetto 8 MiB per immagine e 32 MiB totali. Le non incorporabili
  restano percorsi relativi e generano un avviso con il motivo. Con documento non salvato
  l'incorporamento non è possibile (avviso). Non si è riusato `RadiceProgetto::risolvi` perché è
  privato di `world.rs`: stessa strategia (canonicalizzazione + prefisso), duplicata in una
  funzione piccola e testata.
- **Errori di sintassi**: la conversione prosegue al meglio, ma `errori_sintassi: true` e un avviso
  in testa al riquadro avvertono che il risultato è approssimato (non si rifiuta l'export di un
  documento in corso d'opera).
- **UI**: voci in palette «Esporta come testo semplice / HTML / LaTeX / Markdown…»
  (`esporta.testo|html|latex|markdown`); la modale di export mostra solo i campi del formato
  (larghezza testo 20–500; HTML autonomo sì/no; classe e carta LaTeX); il Markdown non ha opzioni e
  non apre la modale (e non è abilitato su un documento già Markdown). Filtri del dialogo «Salva»
  con l'estensione corretta (`filtriSalvataggio`). Gli avvisi compaiono in un riquadro non fatale
  e chiudibile sotto l'editor (`#avvisi-conversione`, max 6 voci + «e altri N»). In modalità
  Markdown, testo/HTML/LaTeX convertono prima md→typ come già fatto per PDF/PNG/SVG.
- **Permessi/CSP**: nessuna capability né CSP nuova (i comandi sono `invoke` sul core; il dialogo
  «Salva» usa `dialog:allow-save` già presente). `index.html` ha solo nuovo markup/CSS inline: la
  verifica dell'exe di produzione (B-005, hash dello `<style>`) richiede lo smoke interattivo.
- **Test**: 6 nuovi nel core (`conversione/file.rs`: traversal, assoluti, estensioni, tetti,
  symlink su unix, HTML autonomo con mancate, destinazioni, errori di sintassi) e 40+ controlli
  in `opzioniesporta.check.ts` (formati, filtri, nomi, larghezza, comandi/argomenti, avvisi).
  I comandi Tauri sono sottili (nessuna logica propria oltre alla validazione dei parametri):
  i test unitari del guscio non sono eseguibili qui (B-006).

## Correzioni frontend 2026-10-08

Revisione del frontend (`src/**`, `index.html`). Tutte le modifiche sono in TypeScript; logica
nuova in moduli puri con `*.check.ts` (aggiunti a `npm run check`).

- **Anteprima «affamata»** (`main.ts`, nuovo `ciclocompilazione.ts`): `generazioneDoc++` a ogni
  tasto scartava ogni risultato di una compilazione lenta (2 s con un tasto al secondo) e lasciava
  lo stato su «Compilazione…». Ora solo le *sostituzioni* del documento (`editor.imposta` /
  `editor.sostituisci` che cambiano il testo: Nuovo, apri, ricarica, ripristina, converti,
  adotta) invalidano un risultato in volo; le battute no. I diagnostici inline di un risultato
  «vecchio di qualche battuta» restano protetti dal clamp di `editor.mostraDiagnostici`. Un
  risultato scartato imposta lo stato «In attesa…» e, se non c'è un debounce già armato,
  ricompila subito: lo stato non resta mai su «Compilazione…». `inCorso`/`richiestaPendente`
  sono ora dentro `creaCicloCompilazione`. Controllo: simulazione a tempo virtuale (compilazione
  2 s, tasto/s per 20 s) con ≥ 5 aggiornamenti applicati.
- **CRLF** (`fileops.ts`, `finerighe.ts`): `sostituisciDaDisco`, `ripristinaVersione`, `apri`,
  `adotta` passano all'editor `normalizzaFineRiga(testo)` (solo LF) dopo `rilevaFineRiga`;
  `sostituzioneMinima` non vede più «tutto diverso» e cursore/scorrimento si conservano.
- **Autosave in errore persistente**: nel `catch` `esito = "errore"`; un solo avviso via `onAvviso`
  (flag azzerato al primo salvataggio riuscito); il timer non si riarma finché non c'è una nuova
  modifica (`segnaModificato`).
- **`@local` con Markdown** (`local.ts`, `main.ts`): scelta la via più semplice e sicura, il
  **rifiuto** con messaggio («converti prima in Typst, pulsante → Typst») sia all'apertura del
  modale sia alla pubblicazione (il tipo può cambiare a modale aperto). Non si converte in
  silenzio: si pubblicherebbe un pacchetto diverso da ciò che l'utente vede.
- **Ctrl+S durante autosave in volo**: `salva()` attende `attesaSalvataggio` prima di scrivere.
- **Cronologia**: `ripristinaVersione` restituisce `boolean`; il modale si chiude solo se riuscito.
- **Nome predefinito export**: percorso senza `.typ/.md` → si aggiunge l'estensione
  (`nota.txt` → `nota.txt.txt`), non si propone più il sorgente stesso.
- **Accessibilità**: `aria-expanded` di `#btn-barra` e `#btn-pacchetti` aggiornato a ogni cambio
  della barra (e al ripristino dello stato); `#btn-pacchetti` con `aria-controls="barra"`; pulsanti
  delle schede pacchetto con `aria-label` «azione nome versione»; `#cronologia-descrizione` come
  `aria-describedby` del modale.
- **Test**: `caricatorepagine.ts` (macchina a stati di `anteprima.ts`: `stato[i] !== s`,
  `riprovaSeCambiato`, `reimpostaDefs`) estratta e testata (19 controlli);
  `quandoversione.check.ts` ora usa i getter di `Date` come oracolo indipendente da `Intl`.
- **Promemoria (B-005)**: `index.html` è cambiato (solo attributi, nessun CSS), ma il rebuild
  dell'exe di produzione e lo smoke interattivo restano da fare per chiudere il lavoro.

## Correzioni core 2026-10-08

Revisione del core (`config`, `sorveglianza`, `esterni`, `world`, `packages`, `albero`,
`cronologia`); ogni punto ha un test nello stesso modulo.

1. **`scrivi_atomico` e bersaglio read-only (Windows)**: il flag read-only continua a essere
   copiato sul temporaneo (il file finale conserva i permessi), ma nel ramo d'errore, su
   Windows, viene azzerato prima di `remove_file`, così non resta l'orfano `.nome.tmp-PID-ns`.
   *Nota onesta*: con questa toolchain (Rust 1.97) la `remove_file` std ignora già il read-only,
   quindi il test (`..._rename_fallito_su_bersaglio_readonly_non_lascia_orfani`, bersaglio aperto
   in modo esclusivo + read-only) è una guardia di regressione che passa anche senza la
   mitigazione (verificato con mutazione); la mitigazione resta come difesa su std/filesystem
   diversi.
2. **`leggi_e_segna`**: il contenuto si valida come UTF-8 **prima** di `imposta_noto`. Un file
   riscritto da fuori in UTF-16 e ricaricato fallisce senza far avanzare noto/base, quindi il
   salvataggio successivo resta un Conflitto invece di sovrascrivere la versione esterna.
3. **`apri_file_del_progetto`**: nuovo campo `id_main_typ_progetto` (Some solo mentre `id_main`
   non è `.typ`). Aprendo un `.md` il main `.typ` si mette da parte; tornando a un `.typ` del
   progetto viene ripristinato come `id_main` (prima il `.typ` aperto diventava il main).
4. **`Lettura::Illeggibile` è transitoria**: l'`Osservatore` risponde `Esito::Ricontrolla`
   (tetto `MAX_TENTATIVI_ILLEGGIBILE` = 6 riletture per evento, azzerato a ogni nuovo evento e
   da `imposta_noto`). `ricontrolla_a` ora riporta i ricontrolli successivi in
   `Elaborazione::ricontrolli`; il guscio (`osservatore.rs`, una riga) chiama `applica` anche
   quando ci sono solo ricontrolli e non cambi. Per i file non-documento (registro) non c'è
   una voce su cui ricontrollare: se `letta`, `Illeggibile` produce un'invalidazione prudente.
5. **Eco dei file del registro (PDF esportati)**: anche il ramo `registro.ha_voce` applica il
   filtro `letta`: l'eco continua a essere riconosciuta, ma un file non letto dalla compilazione
   (PDF riscritto da un visualizzatore) non emette più cambi.
6. **`destinazione_rinomina`**: prima di leggere/hashare un candidato si confronta
   `metadata().len()` con `Impronta::lunghezza` (`lunghezza_compatibile`): niente letture inutili
   sotto il lock di `Stato`.
7. **Errori di caricamento file**: `FileError::from_io` riceve il percorso relativo alla radice
   (`percorso_per_messaggio`), non il percorso assoluto `\?\...`.
8. **`nuovo_da_template`**: logica estratta in `crea_progetto_da_cartella`; se copia o verifica
   dell'entrypoint falliscono la cartella appena creata (e solo quella) viene rimossa, quindi
   il secondo tentativo non fallisce più con «impossibile creare».
9. **Liste di esclusione unificate** in `albero.rs` (`CARTELLE_RUMORE`, `CARTELLE_SOLO_ALBERO`,
   `FILE_RUMORE`, `e_file_rumore`), usate anche dal watcher. Decisione su `target/`: può essere
   una cartella legittima di risorse dell'utente (`#image("target/fig.png")`), quindi **il
   watcher non la ignora** (gli eventi invalidano comunque le risorse effettivamente lette, filtro
   `letta`), mentre l'albero la nasconde e **non viene riletto** per eventi che toccano solo
   `target/` (`percorso_fuori_dall_albero`), evitando refresh a raffica durante un `cargo build`.
   `node_modules`, `__pycache__`, `Thumbs.db`, `desktop.ini` sono rumore per entrambi (prima il
   watcher conosceva solo `node_modules`). Il confronto dei file di sistema ignora le maiuscole.
10. **`Cronologia::rinomina` A→B→A**: il percorso attuale viene tolto da `percorsi_precedenti`
    (anche nella fusione), `togli_percorso_attuale`.

Nota di processo: `config.rs`, `sorveglianza.rs`, `world.rs`, `cronologia.rs` hanno fine riga CRLF
nella working copy (gli altri LF): le modifiche hanno rispettato la convenzione di ciascun file.

## Correzioni conversioni 2026-10-08

Revisione di `conversione/*`, `markdown.rs` e dei test di integrazione. Ogni bug è stato
riprodotto con un test; dove possibile la verifica è indipendente dal generatore (parser
`typst-syntax` per il Typst prodotto, `pulldown-cmark` per il Markdown, simulazione dei contatori
LaTeX, scansione degli `id`/`href` dell'HTML, confronto fra backend). I test aggiunti sono stati
controllati per mutazione (reintroducendo il difetto il test fallisce).

**Input limite (critici).**
1. Tetti in `ir.rs`: `COLONNE_MAX = 64` e `LIVELLO_MAX = 6`, con avviso quando si riduce
   (`columns: 9223372036854775807` andava in panico, `100000000000` tentava ~100 GB; `level: i64::MAX`
   con `#outline()` ripeteva uno spazio 2^63 volte). Anche `colspan` è ridotto alle colonne della
   tabella (in HTML veniva scritto alla lettera e sommato senza controllo).
2. `\setcounter` di LaTeX usa aritmetica con segno (`0.` → `-1`, valore valido per TeX; la
   `saturating_sub(1)` suggerita avrebbe fatto partire da 1) e limita a 2 000 000 000 con avviso;
   il testo semplice usa `saturating_add` per i numeri di lista.

**Markdown.**
3. Con 3 o più backtick Typst legge l'identificatore dopo i delimitatori come lingua: se il
   contenuto inizia con lettera o `_` si aggiunge uno spazio **iniziale** (Typst lo toglie; quello
   finale resterebbe nel testo, quindi non si aggiunge).
4. Nuovo contatore `Ctx::dentro_inline`: nel testo di link, enfasi, titoli e attribuzioni il Typst
   grezzo è sempre uno span `` `…`{=typst} ``, mai un blocco; una chiamata riconosciuta che produce
   un blocco (citazione, figura) dentro un inline resta Typst grezzo (prova «a secco» senza
   toccare il contesto, per non lasciare note orfane).
5. `escapa_typst_ctx`: `-`, `+` e `N.` a inizio riga, a inizio di un content block `[`, o dopo un
   marcatore di lista scritto da noi, sono scappati (`\- x`, `1\. x`, `\+ x` restano testo).
   Il controllo del contesto scatta solo se il testo inizia con `- + . cifra` (nessun costo
   quadratico sui paragrafi lunghi).
6. `*`/`_` a metà parola non sono markup in Typst: si usa `#strong[…]`/`#emph[…]` quando
   apertura o chiusura cadrebbero fra caratteri alfanumerici (pila `enfasi_funzione`).
13. Il `;` che chiude `#espr;` entra nel Typst grezzo (`con_punto_e_virgola`).
14. `funccall_a_md(.., solo_forma)`: `candidato_blocco_raw` e `hash_a_md` chiedono solo SE una
    chiamata è riconosciuta, senza convertirne i corpi; prima il costo era 2^profondità.
15. Un'espressione multiriga in mezzo al testo è compattata solo se è codice «piano» (nessun
    backtick, virgolette, quadre, `$`, `\`, `//`, `/*`); altrimenti è un blocco con avviso. Dentro un
    inline il blocco è impossibile: resta lo span compattato, con avviso di formattazione persa.
16. `&` è scappata quando può aprire un'entità (`&nome;`, `&#35;`) o chiude il nodo di testo.
19. Gli spazi ai bordi di `* a *`, `#strong[ a ]`, `#strike[ a ]` escono dai delimitatori; lo spazio
    non si raddoppia (gli `Space` sono idempotenti).
20. Una lista ordinata che parte da `N ≠ 1` scrive `N. ` sulla prima voce (poi `+`); `1.` resta `+`.
21. **Sicurezza di `{=typst}`**: in md→typ un blocco/span `{=typst}` è riemesso invariato, quindi un
    `.md` non fidato può iniettare `#read`/`#include`. Documentato nel modulo e aggiunta
    `md_a_typst_con_opzioni(md, &OpzioniMd { consenti_typst_grezzo })`: con `false` blocchi e span
    diventano codice letterale. Il default di `md_a_typst` resta permissivo (round-trip). **Da fare
    fuori dal mio perimetro**: `lib.rs` non riesporta ancora `md_a_typst_con_opzioni`/`OpzioniMd`, e
    il guscio dovrebbe usare l'opzione sicura per i `.md` che non sono del progetto dell'utente.

**LaTeX.**
7. Math in linea: `senza_allineamenti` toglie `&` e trasforma `\` in spazio a ogni profondità.
8. Titolo della bibliografia: `\refname` in `article`, `\bibname` in `report`/`book` (l'altro non
   esiste e `\renewcommand` darebbe errore).
9. Figura senza didascalia: `\refstepcounter{ambiente}` prima del `\label`; viene scritto anche senza
   etichetta, per tenere allineata la numerazione con Typst.
10. `\item{} ` se il contenuto inizia con `[`; termini come `\item[{…}]`; `Inline::ACapo` in una cella
    `tabular` diventa spazio (un `\\` chiuderebbe la riga della tabella).
12. I listati usano l'ambiente `listato` (`\usepackage{float}`, `\newfloat{listato}{htbp}{lol}`,
    `\floatname` localizzato): contatore separato dalle figure, come in Typst. Il pacchetto `float`
    fa parte di TeX Live/MiKTeX standard. Correlato: `#figure(```raw```, …)` (raw come argomento
    diretto) produceva un segnaposto perché `contenuto_espr` non gestiva `Expr::Raw`.

**HTML.**
11. `Documento::blocchi_titolo()` visita i titoli in profondità (anche in citazioni, liste, termini,
    figure). L'`id` si associa al blocco per **identità** (indirizzo), non per conteggio d'ordine; i
    titoli dentro le note (copie) ricevono un `sez-N` nuovo e unico. L'indice usa `testo_semplice`
    (niente note duplicate né `<a>` annidati).

**Profondità e avvisi.**
17. `PROFONDITA_MAX` (IR) = 32, `PROFONDITA_MATH_MAX` = 32, `PROFONDITA_MD_MAX` = 32; `contenuto_espr`
    e le parentesi `#(((…)))` ora incrementano la profondità; `nome_callee` è iterativo; oltre il
    limite la parte interna è omessa (IR, math → `Grezzo`) o conservata come Typst grezzo (md), sempre
    con avviso. Misura in debug: il solo parser upstream usa ~600 KiB di stack per 256 livelli di
    parentesi e una catena `#a.a.a…` di 3000 accessi manda in overflow **il parser stesso** (nessun
    limite upstream sui postfissi): non risolvibile qui; il test limita quel caso a 300.
    I test usano un thread da 512 KiB; senza i limiti vanno in overflow già a 50 livelli.
18. `Avvisi` (Vec + HashSet) in `ir.rs`, usato da IR, math, HTML, LaTeX, `unisci`, `Ctx` del Markdown
    e dall'elenco delle immagini mancanti; l'ordine d'inserimento è conservato.

**Test.** Rafforzati i test che non potevano fallire (`contains("\n\n")`, `|| contains("1000")`,
`|| contains("stemma")`, `|| "[a"`, `||` ridondante su `#let`, `replace` no-op, `[l](url)` non è
Typst, `contains("+")`); rimosso il codice morto (`tutte_semplici`, `col_prossima`). La compilazione
LaTeX nei test di integrazione, se manca il compilatore, stampa `*** SALTATO: … ***` e fallisce se
`TYPSTER_RICHIEDI_LATEX` è impostata. Sulla macchina non c'è alcun compilatore LaTeX (né nel PATH né
in `code_tools`): i `.tex` sono verificati solo staticamente e per simulazione dei contatori.

## Ultimo giro frontend 2026-10-08

1. **CRLF sulla cronologia** (`fileops.ts`, `main.ts`, `finerighe.ts`): `salva_file` registra il
   testo con la fine riga del file riapplicata, mentre `registraInCronologia` passava il testo LF
   dell'editor: su file CRLF la versione non risultava «Identica» (duplicati a ogni ricarica o
   «Non salvare») e `verifica_integrita` vedeva un troncamento. Nuova funzione pura
   `contenutoPerCronologia(testo, fineRiga)`; `registraInCronologia` la usa per tutti i chiamanti
   (salvataggio-scarto, ripristino versione, ricarica). Il gestore file espone
   `registraInCronologia()` e `main.ts` (chiusura finestra con domanda fallita) passa da lì invece
   di `invoke("cronologia_registra", editor.testo())`. Controllo in `finerighe.check.ts`.
2. **Percorsi non canonici** (`modificheesterne.ts`, `fileops.ts`): `normalizzaPercorso` su Windows
   ora gestisce `\?\` e `\.\` (anche `UNC` in minuscolo → `\server\...`), separatori ripetuti,
   segmenti `.`/`..` (senza uscire dalla radice), barre finali, maiuscole. Nuovo
   `confrontaPercorsi` a tre esiti `uguale | incerto | diverso`: `incerto` = forma diversa ma stesso
   nome di file (unità di rete mappate, `subst`, giunzioni). `stessoPercorso` resta `true` solo per
   `uguale`. In `elaboraEvento` un evento `incerto` non è più scartato: `ricarica`/`chiedi` passano
   da `ricaricaDaDisco`/`chiediVersione` (lavorano sul percorso del documento e proteggono le
   modifiche non salvate), `sparito` da `documentoSparito` (verifica sul disco), `rinominato` mostra
   un avviso senza riagganciare il documento (potrebbe essere un altro file). Compromesso noto: un
   file omonimo in un'altra cartella può causare un ricaricamento/avviso superfluo, mai la perdita
   silenziosa di un evento. La lunghezza del file non è disponibile nel frontend: il criterio è il nome.
3. **Doppio avvio** (`main.ts`): il `load` chiama `avviaCompilazione(1)` (guardia
   `ciclo.richiedi()` + retry del bridge) invece di `eseguiCompilazione` diretta.
4. **Apertura con testo identico** (`ciclocompilazione.ts`, `fileops.ts`): nuova
   `impostaTestoConRicompilazione`: se il testo è identico, CodeMirror non emette `docChanged` e
   nessuna compilazione partirebbe (nuova radice/World); si chiama `onRicompila()`. Se il testo
   cambia, basta il debounce (niente doppia compilazione). Usata in `apri`, `adotta`, `creaNuovo`.
   Non cambiata `apriFileProgetto` (non letta in questo giro: stesso rischio se imposta lo stesso testo).
5. **Check del ciclo** (`ciclocompilazione.ts/.check.ts`): estratte `reagisciABattuta` e
   `reagisciASostituzione`, usate da `main.ts`; il check simula le battute con la funzione reale.
   Verificato per mutazione: aggiungendo `ciclo.sostituzione()` in `reagisciABattuta` il check
   fallisce («l'anteprima si aggiorna durante la digitazione (aggiornamenti: 2)»); ripristinato.
6. Nessuna modifica all'UI delle opzioni di export né all'accessibilità dell'HTML esportato.

Promemoria (B-005): `index.html` non toccato in questo giro, ma `src/**` sì: restano rebuild
dell'exe di produzione e smoke interattivo (apri file CRLF, ricarica da fuori, drive mappato).

## Ultimo giro core 2026-10-08

1. **F5-022 sui percorsi non canonici (drive di rete mappato, `subst`, giunzione)**. Il watcher
   emette il percorso canonico della radice del World, il frontend confronta in modo lessicale con
   quello del dialogo: le azioni sul documento venivano scartate in silenzio. Ora
   `esterni::percorso_nella_forma_del_documento(doc, p, stesso)` (pura, `stesso` iniettabile)
   riscrive `p` nella grafia di `doc` (il file stesso, oppure il più vicino antenato in comune: la
   rinomina in un'altra cartella dello stesso volume resta coerente). Il guscio
   (`osservatore.rs::decisioni`, funzione pura testata) la applica a `ricarica`/`chiedi`/`sparito`/
   `rinominato` (vecchio e nuovo) e alla riassociazione della cronologia; nel core `ricontrolla_a`
   sposta `Stato::doc` sul nuovo nome **nella grafia del frontend**, e `imposta_documento` adotta la
   grafia ricevuta quando indica lo stesso file. Nessun campo serializzato cambia (`Uscita` uguale):
   cambia solo il valore testuale di `percorso`/`nuovo`. *Bug nei miei primi test*: `PathBuf::join("..")`
   su un percorso verbatim `\?\` si normalizza da solo, quindi la grafia alternativa «con `..`» non era
   alternativa e i test passavano anche con la mutazione; ora la grafia alternativa è quella senza
   prefisso `\?\` (verificato con mutazione: i due test falliscono se si toglie il fix).
2. **Conflitto spurio con file illeggibile all'aggancio**. `imposta_documento` con `Illeggibile` segna la
   base come *incerta* (`Osservatore::imposta_noto_illeggibile`): il primo contenuto leggibile
   in `scrivi_proprio` viene adottato come noto/base (`adotta_base_incerta`) invece di dare `Conflitto`.
   Scelta rispetto al ricontrollo programmato: niente stato nuovo da propagare al guscio e nessun
   `Modificato` spurio. L'incertezza cade appena il watcher segnala una modifica esterna (`su_evento` →
   `Modificato`) o la base viene impostata (`imposta_noto`): in quel caso il conflitto vero resta
   un conflitto. Limite noto: una modifica esterna avvenuta nella finestra di illeggibilità, senza alcun
   evento del watcher, viene sovrascritta al primo salvataggio.
3. **Tetti delle immagini incorporate**: `opzioni_html_autonomo_con_tetti(radice, base, max_immagine,
   max_totale)`; il test con due immagini da 700 byte (tetto singolo 1000, totale 1200) fallisce se si
   toglie `residuo` (mutazione verificata). Il totale ora conta i byte reali (payload base64 · 3/4, senza il
   prefisso `data:…`).
4. **HTML autonomo in un progetto**: `opzioni_html_progetto(radice_progetto, cartella_documento)`. In
   Typst `/x.png` è relativo alla radice del progetto, gli altri percorsi alla cartella del file:
   `incorpora_immagine_da(radice, base, rel, max)` fa lo stesso e ammette `..` solo se il file
   **canonico** resta sotto la radice (solo file regolari, estensione immagine, tetti). Se manca
   il progetto, o la cartella del documento non gli appartiene, vale la cartella del documento. Il guscio
   legge `radice_progetto()` dal World con un lock breve, rilasciato prima della conversione
   (`esporta_html` riceve anche lo stato `Arc<Mutex<EditorWorld>>`: nessun cambio lato frontend).
5. **Estensione e file reale (Windows)**: `esporta::verifica_nome_destinazione` rifiuta nomi con punto/spazio
   finale o `:` (flussi `::$DATA`), e controlla l'estensione vietata sia sul nome scelto sia su
   `percorso_stabile` (un collegamento `out.pdf` → `capitolo.typ` ha come bersaglio il sorgente: `scrivi_atomico`
   scrive sul canonico). Usata da `verifica_destinazione_export` e dal ramo `.md` di
   `verifica_destinazione_conversione` (qui vieta `.typ`/`.typc`). Il test del collegamento simbolico si
   salta con avviso se i privilegi mancano.
6. **`{=typst}`**: `typst_core` riesporta `OpzioniMd` e `md_a_typst_con_opzioni`; nuove
   `md_dentro_il_progetto` e `opzioni_md_per_documento` (pure, in `conversione/file.rs`). Il guscio
   (`converti_md_a_typst`, `anteprima_markdown`) decide dal documento aperto (`Stato::doc`) e dalla radice del
   progetto del World (due lock presi uno alla volta): `consenti_typst_grezzo = true` solo se il `.md` sta
   sotto la radice (percorsi canonici: `..`, symlink, prefissi simili come `progetto-bis` non contano); un
   file fuori progetto, un file singolo senza progetto o un testo non salvato vedono `{=typst}` come
   codice letterale. *Conseguenze*: un `.md` standalone con `{=typst}` perde il Typst grezzo (round-trip
   solo dentro un progetto); alla prima anteprima subito dopo l'apertura, prima che il frontend invii
   `osserva_documento`, `Stato::doc` può essere ancora il documento precedente (la decisione si
   corregge alla successiva). Nessun nuovo parametro dei comandi.
7. **CSP**: tolto `asset:` da `img-src` (CSP e devCsp; nessun `convertFileSrc`/`asset:` in `src/`, nessun
   `assetProtocol` in config) e aggiunto `form-action 'none'`. Nessun hash; `dangerousDisableAssetCspModification:
   ["style-src"]` resta (B-005). **Da fare**: rebuild dell'exe di produzione e smoke interattivo
   (la CSP non è verificata da test).

## Ultimo giro conversioni 2026-10-08

Riesame finale di `markdown.rs` e `conversione/*`. Ogni punto ha un test scritto prima del fix (o
subito dopo, dove il fix era banale) e controllato per mutazione.

1. **Marcatori di lista dopo un `[` aperto.** `contesto_inizio_markup` calcola ora «inizio riga» e
   «solo cifre» sulla **coda dopo l'ultimo `[` non scappato** (`ultima_quadra_aperta`), non sull'intera
   riga: `[1\. x](u)`, `a**1\. x**b` (`#strong[`), `a~~1\. x~~b` (`#strike[`) e le celle di tabella
   restano testo. Verificato col parser `typst-syntax` (nessun `EnumItem`/`ListItem` spurio).
2. **`#{ … }` multiriga.** `{` e `}` escono dalla compattazione «codice piano»: in mezzo al testo
   diventa un blocco grezzo (con avviso). Dentro un inline (dove il blocco è impossibile) lo span è
   prodotto da `compatta_in_una_riga`, che prova prima a unire con spazi e, se il parser dice che non
   è valido, separa le istruzioni con `; ` (solo fra token che chiudono e token che aprono
   un'istruzione, mai prima di `else`, `}`, `)`, `.`, `,`).
3. **Commenti `//` nello span compattato.** Diventano `/* … */` (con `*/` interno spezzato in `* /`)
   leggendo le foglie del parser, non il testo; l'avviso lo dichiara, e dichiara anche se il risultato
   non è valido Typst (nessuna delle due forme si legge senza errori).
4. **`{=typst}` con Typst grezzo vietato.** L'attributo è consumato e scartato comunque
   (`consuma_attributo`); il codice resta letterale e il testo seguente non mostra più `{\=typst}`.
5. **Profondità IR: un livello per annidamento.** `sotto` + `contenuto_espr` contavano due volte
   (`quote`, `figure`, `footnote`, `espr_inline`: tetto effettivo 16 invece di 32). Nuovo
   `sotto_espr` (stesso contesto separato, nessun +1 proprio: lo aggiunge `contenuto_espr`).
   Il test usa il thread da 512 KiB: 24 citazioni annidate non perdono più nulla, 200 sì (con avviso).
6. **`colspan` oltre le colonne.** `a_righe` ritorna il colspan massimo ridotto e `tabella` avvisa
   («una cella con colspan N supera le C colonne: ridotto»); l'avviso del tetto a 64 resta.
7. **`Avvisi` condiviso.** `conversione` riesporta `pub(crate) use ir::Avvisi`; `Ctx` di `markdown.rs`
   non duplica più Vec + HashSet. La nota 18 di «Correzioni conversioni» ora è vera anche per il Markdown.
8. **Test a tempo.** `avvisi_distinti_scalano_in_modo_lineare` è `#[ignore]` (dipende dal carico della
   macchina; si lancia a mano con `--ignored`). Il comportamento deterministico (nessun duplicato,
   ordine conservato, 8 000 avvisi) è in `avvisi_distinti_sono_tutti_conservati_in_ordine`. Nota:
   questo test NON rileva da solo una regressione quadratica; la rileva solo la misura a mano.
9. **Indice HTML con `#outline()` prima dei titoli.** Nuovo test: ogni `href` dell'indice ha il suo
   `<hN id>` e quel titolo ha il testo della voce; nessun id duplicato. Era già corretto (id per
   identità): il test è una guardia, non ha rilevato un difetto.
10. **LaTeX.** (a) Titolo non numerato con etichetta: `\section*{T}\phantomsection\label{et:x}`; il
    riferimento è `\hyperref[et:x]{Section}` (solo testo, come negli altri formati) perché `\ref`
    avrebbe stampato il numero della sezione precedente. Con la numerazione resta `\ref`.
    (b) Figura/tabella/listato dentro una voce di lista: niente `figure`/`table` flottanti (non
    ammessi nelle liste) ma `center` + `\captionof{ambiente}{…}` (pacchetto `caption`, caricato solo
    se serve) o `\refstepcounter` senza didascalia; il contatore è lo stesso del flottante. Il
    controllo strutturale dei test scandisce gli ambienti aperti. **Non verificato con un compilatore
    LaTeX** (assente sulla macchina): `caption` + `float` (`\captionof{listato}`) e `\phantomsection`
    sono usi documentati ma non provati.
11. **HTML, accessibilità.** `alt` dell'`<img>`: `alt:` dell'immagine, poi `alt:` della figura (copiato
    sulle immagini del corpo in IR), poi il testo semplice della didascalia; vuoto solo senza nulla.
    Rischio noto: alt = didascalia è ridondante per chi usa uno screen reader (la didascalia viene
    letta due volte); è la scelta richiesta, preferibile all'`alt=""` che segnala «decorativa».
12. **Bibliografia nel testo semplice.** Nessuna modifica: i riferimenti restano chiavi `[chiave]`
    (il `.bib` non è letto; già avvisato).

## Minigiro finale frontend 2026-10-08

1. **Evento `incerto` non innocuo** (`modificheesterne.ts`, `fileops.ts`). Nuove funzioni pure
   `azioneDopoSparito(esito, incerto)` e `ricaricaSuperflua(disco, editor, incerto)`, testate in
   `modificheesterne.check.ts`. `sparito` + `incerto` + esito `esiste` = no-op silenzioso (prima:
   `modificato = true` e avviso «ricomparso» per un documento mai sparito); con percorso certo
   nulla cambia. `ricarica`/`chiedi` incerto con testo non modificato: `ricaricaDaDisco(false, true)`
   confronta `normalizzaFineRiga(contenuto) === prima` e tace se identico (nessun «ricaricato»).
   **Non risolto**: `chiedi` incerto con testo già modificato chiede comunque «quale versione
   tenere»: per confrontare servirebbe leggere il disco, ma `leggi_file` segna il contenuto come
   «visto» e nasconderebbe un conflitto vero; servirebbe un comando di sola lettura nel guscio.
2. **Compilazione in volo + riapertura con testo identico** (`ciclocompilazione.ts`, `main.ts`).
   Nuova `ricompilaPerWorldCambiato(ciclo, avvia)` = `ciclo.sostituzione()` + avvio; `main.ts` la usa
   come `onRicompila` del gestore file (quindi nel ramo «identico» di
   `impostaTestoConRicompilazione` e nella rinomina esterna: anche lì il World cambia). Il risultato
   del documento precedente non è più applicato e se ne accoda uno nuovo. Se il testo cambia,
   `reagisciASostituzione` ha già invalidato (doppia chiamata innocua). Check n. 7 con simulazione;
   verificato per mutazione (tolta `ciclo.sostituzione()`: il check fallisce; ripristinato).
3. **`apriFileProgetto`**: già coperta, senza modifiche: chiama `adotta`, che usa
   `impostaTestoConRicompilazione` con `onRicompila` (punto 2). Nessun test dedicato (dipende da Tauri).

Promemoria (B-005): `index.html` non toccato; restano rebuild dell'exe di produzione e smoke
interattivo (riapri lo stesso file durante una compilazione lenta; evento da percorso omonimo).

## Minigiro finale 2026-10-08

1. **`adotta_base_incerta` e scritture esterne non ancora elaborate.** Il difetto: con il file
   `Illeggibile` all'aggancio la base era «da stabilire» e il primo contenuto leggibile veniva adottato,
   quindi una versione esterna scritta nella finestra BATCH_MS/STARVE_MS/RICONTROLLO_MS (prima che il
   watcher la segnalasse) veniva sovrascritta senza passare dalla cronologia. Soluzione scelta (la
   preferita, **senza cambiare l'IPC**): `apri_file` registra nello stato condiviso l'impronta di ciò che ha
   consegnato all'editor (`sorveglianza::segna_consegnato`, nuovo parametro *di stato Tauri*, non del
   comando); `imposta_documento` la usa come base **vera** se `leggi` dà `Illeggibile` e il percorso è lo
   stesso file (consumata al primo aggancio). Una versione diversa letta dopo è così un `Conflitto`. Resta
   la base incerta (comportamento del giro precedente) solo senza contenuto consegnato (file creato/aperto
   per altre vie). **Limite noto (CHIUSO dal «Micro-giro consegnato 2026-10-08», punto 1)**: nella stessa finestra ma con file *leggibile* all'aggancio, la base era
   ancora il disco letto da `imposta_documento`, non il consegnato (allora non toccato: cambiava il significato
   di «apri» per tutti i file, non solo per quelli illeggibili). Test: 3 nuovi (conflitto reale; consegnato
   uguale al disco → `Scritto`; consegnato di un altro file ignorato); mutazione verificata (disattivando
   l'uso del consegnato il primo test fallisce con `Scritto` invece di `Conflitto`).
2. **`compatta_in_una_riga`: doppio `; `.** `*a #{⏎ let x = 1⏎ // c⏎ let y = 2⏎} b*` dava
   `let x = 1; /* c*/; let y = 2` (non valido: il fallback tornava alla forma con soli spazi, anch'essa
   non valida). Flag `separato` (azzerato a ogni foglia significativa): un solo `; ` dopo `prec`. Il
   riscontro sul parser è nel test (`let x = 1; /* c*/ let y = 2`, due `LetBinding`, nessun errore, anche
   dopo il giro `typ → md → typ`). Mutazione verificata (senza `!separato` il test fallisce).
7. **Stringhe con `//` o `/*` in espressioni multiriga inline.** Nuovo test: `"http://x"` e
   `"/* non è un commento */"` restano stringhe (il compattatore legge le foglie del parser, non il testo),
   nessun commento convertito, risultato valido. Era già corretto: è una guardia, non ha rilevato un difetto.
4. **Test delle azioni nella forma del frontend (guscio).** `grafia_frontend` ora produce
   `<radice senza \?\>/sub/../nome` (cartella `sub` creata da `Cartella::nuova`): lessicalmente diversa
   anche *dopo* `testo()` (che toglie già `\?\`) e su ogni piattaforma, ma risolta dal filesystem. Assert
   su `PathBuf` e su testo emesso. *Bug del test precedente*: non poteva fallire. Mutazione verificata
   (`nella_forma_del_doc` che restituisce `p` invariato: falliscono entrambi i test, azioni e rinomina).
6. **`stesso_file` sotto il lock di `Stato` nella rinomina seguita.** `percorso_nella_forma_del_documento`
   canonicalizza gli antenati (O(d²) syscall) e girava dentro `ricontrolla_a`, quindi con `scrivi_proprio`
   bloccato su unità di rete lente. Il nuovo nome non è noto prima del lock (lo decide
   `destinazione_rinomina`), quindi «fuori lock» non è realizzabile: nuova via veloce lessicale
   `esterni::nuovo_nome_nella_forma_del_documento(doc, vecchio, nuovo)` per la rinomina **nella stessa
   cartella** (caso comune: `doc.parent()/nome`, zero syscall). Lo spostamento in un'altra cartella usa
   ancora la via completa sotto lock (raro). `e_doc(percorso)` canonicalizza comunque due percorsi sotto
   lock (invariato). Comportamento invariato (test esistente `rinomina_seguita_mantiene_la_forma…` verde);
   unit test sul caso stessa cartella/cartella diversa.

**Cambi IPC verso il frontend: nessuno.** `apri_file` ha un parametro di *stato* in più
(`tauri::State<Condiviso>`), che Tauri inietta: gli argomenti dell'`invoke` sono quelli di prima.

## Micro-giro consegnato frontend 2026-10-08

1. **`chiedi`/`ricarica` incerto con testo modificato** (`fileops.ts`, `modificheesterne.ts`). Chiuso il caso
   rimasto aperto del «Minigiro finale frontend», senza nuovo IPC. `creaGestoreFile` tiene `testoDisco`
   (testo LF dell'ultima lettura/scrittura del file del documento): impostato in `apri`, `adotta`,
   `sostituisciDaDisco`, `scriviSu` (esito «scritto») e `documentoSparito` (ripristinato); azzerato dopo
   `riconosci_disco` (in `chiediVersione` e `salvaConNome`), in `convertitoA` e `creaNuovo`.
   Nel ramo incerto+modificato con `testoDisco !== null`, `chiediSeDiscoCambiato`: `conflittoAperto = true` +
   `annullaAutosave()` (nessun autosave/blur scrive durante l'await; `finally` lo ripristina sempre), poi
   `leggi_file`; se `discoInvariato(c, testoDisco)` (pura, in `modificheesterne.ts`) → no-op silenzioso (il
   «visto» di `leggi_e_segna` è idempotente); altrimenti `cronologia_registra` della versione esterna (con
   `contenutoPerCronologia`, fine riga del file letto) e solo dopo `chiediVersione()`. Con `testoDisco === null`
   o percorso certo: si chiede come prima. Errore di lettura/registrazione: ripiego prudente (si chiede).
   Check: 5 casi su `discoInvariato`; mutazione verificata (senza il confronto `normalizzaFineRiga(...) ===
   testoDisco` il check fallisce: «disco diverso dal noto … ottenuto true»; ripristinato).
   **Limite**: se il file è cambiato da fuori senza evento già elaborato e poi arriva un evento omonimo, il disco
   differisce da `testoDisco` e si chiede (corretto, prudente). Nessuna prova end-to-end (dipende da Tauri).

## Micro-giro consegnato 2026-10-08

1. **File leggibile all'aggancio con consegnato diverso dal disco** (chiude il «limite noto» del punto 1
   del Minigiro finale). Scenario: `apri_file` legge v1, un altro programma scrive v2 prima di
   `osserva_documento`, il file è leggibile: il ramo `Lettura::Contenuto` ignorava il consegnato (già
   consumato con `take()`), quindi base e `prev` = v2, nessun `ricarica` e il salvataggio `Scritto` perdeva
   v2 in silenzio, senza cronologia. Ora `imposta_documento` imposta `prev` = disco (coerente col watcher: nessun
   evento spurio) e, se il consegnato differisce dal disco, `base` = consegnato (`imposta_base`): il salvataggio
   dà `Conflitto` e `riconosci_disco` mette v2 in cronologia. Per i file normali (consegnato = disco) nessun
   effetto. Test: `aggancio_leggibile_con_consegnato_diverso_dal_disco_e_un_conflitto_con_cronologia`
   (fallito prima: `Scritto` invece di `Conflitto`); guardia `aggancio_leggibile_con_consegnato_uguale_al_disco_non_cambia_nulla`.
   Il test «illeggibile, consegnato uguale al disco → Scritto» non distingueva «consegnato ignorato» (con la base
   incerta il risultato era identico): ora asserisce anche `base == consegnato` prima del salvataggio (con il
   consegnato ignorato la base sarebbe `None`). Mutazione verificata (`filter(|_| false)`: il test fallisce).
2. *(non assegnato in questo giro)*
3. **`apri_file_progetto`, `apri_cartella`, `nuovo_da_template` non registravano il consegnato**: l'apertura
   dall'albero/cartella/template restava a base incerta. Ora ricevono `stato: State<Condiviso>` (iniettato da
   Tauri: **nessun cambio degli argomenti `invoke`**) e chiamano `segna_consegnato` con il contenuto
   effettivamente consegnato all'editor, **dopo** il rilascio del lock del World (in `apri_file_progetto` il
   World era tenuto per tutta la closure: ora il blocco è ristretto a `apri_file_del_progetto`; MAI annidare
   `Condiviso` dentro World). Su errore di `apri_progetto` (`?`) la chiamata non avviene. Nessun test
   automatico: i comandi dipendono da `tauri::State` e il guscio non ha un harness (nessun test di comando
   esistente); verificato da compilazione/clippy e dal test del core sul comportamento a valle (punti 1 e 4).
4. **Riapertura dello stesso file** (`cambiato` falso): se il disco è a v2 e il watcher non l'ha ancora
   elaborato, `apri_file` consegna v2 all'editor ma la base restava v1 → «ricaricato» spurio. Nel ramo
   `else if doc.is_some()`, se il consegnato è dello stesso file (`stesso_file`), lo si consuma e
   `imposta_noto(doc, Some(consegnato))`; un consegnato di un altro file resta intatto per il suo aggancio
   (lock e ordine invariati). Test: `riapertura_dello_stesso_file_usa_il_consegnato_come_noto` (fallito
   prima: noto v1 invece di v2) e `…ignora_il_consegnato_di_un_altro_file`. Mutazione verificata
   (`if true || stesso_file`: il secondo test fallisce; senza il blocco fallisce il primo).
5. **`apri_file`: `segna_consegnato` dopo le validazioni** (`parent`/`file_name`/UTF-8 del nome), prima del
   lock del World: su errore non resta una voce stantia.

**Cambi IPC verso il frontend: nessuno** (solo parametri di *stato* Tauri aggiunti a tre comandi).

## Correzione consegnato stantio 2026-10-08

**Regressione del «Micro-giro consegnato» (punto 4)**, trovata al riesame. Scenario: documento pulito,
rinomina da Explorer → `elaboraEvento("rinominato")` accoda `osserva_documento(nuovo)` e poi
`riagganciaWorld` → `apri_file(nuovo)` → `segna_consegnato(nuovo, vA)` (testo scartato dal frontend). Se
`osserva_documento` passa prima del `segna_consegnato`, il consegnato resta pendente; quando un altro programma
scrive vB, `ricarica` → `leggi_e_segna` (noto = base = vB) → `osserva_documento` sullo stesso file consumava il
consegnato e riportava `noto`/`base` a vA mentre editor e disco erano a vB: `Conflitto` spurio al salvataggio
(autosave bloccato in silenzio; «Carica quella su disco» buttava le modifiche).

1. **Fix (core, nessun cambio IPC).** `Stato::azzera_consegnato(percorso)` scarta il consegnato solo se è dello
   STESSO file (`stesso_file`); è chiamato da `leggi_e_segna`, `riconosci_disco`, `ripristina_sparito` (solo
   nel ramo `Ripristinato`: negli altri esiti non cambia nulla) e dal ramo `e_doc` di `scrivi_proprio` (dopo la
   scrittura riuscita). Sono dichiarazioni più recenti e autorevoli di ciò che l'editor aveva visto alla
   consegna. Il consegnato di un altro file resta intatto per il suo aggancio.
2. **Test** (`sorveglianza.rs`): `leggi_e_segna_/riconosci_disco_/ripristina_sparito_/scrivi_proprio_azzera_il_consegnato_stantio_dello_stesso_file`
   (consegnato v1, disco v2/ricreato/scritto, poi `imposta_documento` sullo stesso doc → `noto` = valore
   dichiarato, non v1; il primo asserisce anche `base`) e
   `le_dichiarazioni_sul_documento_non_toccano_il_consegnato_di_un_altro_file`. Falliti PRIMA del fix (noto v1
   invece di v2 / del contenuto scritto); mutazione verificata: togliendo l'azzeramento da ciascuna delle 4
   funzioni fallisce esattamente il test relativo; con `|| true` nel filtro per file fallisce il test dell'altro file.
3. **`apri_file_progetto`** (`src-tauri/src/lib.rs`): `segna_consegnato` ora segue la validazione `to_str()`
   (come `apri_file`): su `Err` non resta una voce stantia. `apri_cartella` e `nuovo_da_template` erano già a
   posto (validano `entrypoint.to_str()` prima di `segna_consegnato`, che è l'ultimo passo prima di `Ok`).
4. **Limite noto (non risolto, richiede conferma dell'utente).** Con consegnato ≠ disco, `imposta_documento`
   non emette alcun evento: l'utente vede v1 mentre il disco è v2 e lo scopre solo salvando (`Conflitto`, con
   v2 in cronologia al riconoscimento). Chiuderlo richiederebbe un esito enum da `imposta_documento` o un nuovo
   evento verso il frontend: non fatto.

**Cambi IPC verso il frontend: nessuno.**

## Stile grafico: campo impostazioni 2026-10-09

**Decisione.** Nuovo campo `stile: String` in `Impostazioni` (`crates/typst-core/src/config.rs`),
accanto a `tema` e indipendente da esso. Valori ammessi (costante `STILI`): `"classico"`
(predefinito, aspetto storico), `"composizione"`, `"strumento"`. Nome serializzato esattamente `stile`
(il frontend lo legge come `Impostazioni.stile`).

**Comportamento.**
- Valore sconosciuto, vuoto o di tipo sbagliato → `"classico"` (in lettura e in salvataggio, via `normalizza`).
- File `impostazioni.json` preesistente senza il campo → `"classico"`: nessuna migrazione, nessuna rottura.
- Ogni campo è normalizzato in modo indipendente (`stile` valido + `tema` non valido conserva lo stile).
- Campi futuri sconosciuti ignorati senza perdere gli altri; salvataggio atomico invariato.

**Guscio Tauri.** `src-tauri/src/lib.rs` non modificato: `leggi_impostazioni`/`salva_impostazioni`
(de)serializzano l'intera struct con serde, quindi il campo transita da solo.

**Test** (`config.rs`): default, file vecchio senza campo, valori ammessi + nome serializzato, sconosciuto/tipo
sbagliato (lettura e `normalizza`), indipendenza da `tema`, campi futuri, round-trip (stile non predefinito).
Falliti PRIMA del fix (`stile` restava `"neon"` invece di `"classico"`): `..._sconosciuto_o_di_tipo_sbagliato_torna_classico`
e `..._valido_e_tema_non_valido_sono_indipendenti`; gli altri passavano già col solo campo+default.

## Tre stili grafici e menu Aspetto 2026-10-09

**Cosa.** Scelta fra tre stili da un menu grafico: «Classico» (aspetto storico), «Composizione» (proposta A) e
«Strumento» (proposta B), da `output/anteprime-stile/proposta-{A,B}.html`. Ogni stile è scopato su
`:root[data-stile="..."]`; `src/aspetto.ts` scrive `document.documentElement.dataset.stile`.

**File.** `src/stile.ts` + `stile.check.ts` (logica pura: validazione, ciclo, etichette, navigazione da tastiera; nello
script `check`), `src/aspetto.ts` (`impostaStile`, `applicaStile`, `stileCorrente`, `temaCorrente`, `osservaAspetto`),
`src/menu-aspetto.ts` (popover), `src/stili/{composizione,strumento,menu-aspetto}.css`, `src/stili/editor-stile.ts`
(tema CodeMirror), `src/editor.ts` (`impostaStile`), `src/impostazioni.ts` (campo `stile`), `src/main.ts` (CSS importati,
comandi), `index.html` (token di default, pulsante, contenitore del menu).

**Scelte.**
- *Classico invariato*: i nuovi token in `index.html` hanno valori che riproducono l'aspetto storico (`--carta: white`,
  `--sans`/`--mono` identici alle stringhe precedenti) e i fogli degli stili non si applicano senza l'attributo. Verifica
  automatica: confronto di `getComputedStyle` (40 proprietà) + rettangoli su tutti i ~270 elementi, build nuova col
  Classico vs stessa app con l'HTML pre-modifica, chiaro e scuro: unico scarto la larghezza di 4 elementi dell'header, perché
  «Aspetto» (più corto) prende il posto di «Tema: sistema». Non è un confronto pixel-per-pixel su un binario di riferimento.
- *Pulsante «Aspetto» al posto di «Tema»* (`#btn-tema` rimosso): aggiungerlo accanto faceva andare a capo l'header a 1280 px
  (larghezza della finestra, `tauri.conf.json`). Il ciclo del tema resta nel menu (Chiaro/Scuro/Sistema) e nei comandi
  `tema.*` della palette; `inizializzaAspetto(editor, avvisa)` non riceve più il pulsante.
- *Scelta utente prima del caricamento*: `stileToccato`, come `temaToccato`; il valore salvato non la sovrascrive. Un valore
  sconosciuto o assente (backend vecchio) si normalizza a `classico`. In memoria funziona comunque anche se il salvataggio fallisce.
- *Editor*: il Classico conserva chiaro predefinito/`oneDark`. Gli altri stili sostituiscono il tema CodeMirror con
  `estensioniStile` (solo `var(--...)`) e una `HighlightStyle` sui token `--sint-*`; il tema chiaro/scuro cambia via CSS
  (`light-dark`), il flag `dark` serve solo al tema base. Contrasti: quelli WCAG AA/AAA delle tabelle in testa alle proposte
  (valori copiati identici; non ricalcolati). `@codemirror/language` e `@lezer/highlight` sono importati direttamente ma sono
  dipendenze transitive già installate (non aggiunte a `package.json`: nessuna installazione).
- *Anteprima*: `.pagina-svg` usa `var(--carta)` (bianca nel Classico; crema/avorio in Composizione, bianca/grigio-notte in Strumento).
  Il testo delle SVG Typst è nero (fondo trasparente se la pagina non ha `fill`): leggibile su tutti i fogli.
- *Menu*: popover (`role="dialog"`) con due `radiogroup`; tabindex "roving"; frecce/Home/End spostano **e** selezionano (come i
  radio nativi); Invio/Spazio scelgono; Esc chiude e rida il focus al pulsante; Tab confinato; `aria-expanded`; clic fuori chiude.
  Miniature in solo CSS (palette a 5 colori + mini-layout) con tavolozze fisse `--m-*` dello stile, quindi seguono chiaro/scuro.
  Stato selezionato = bordo + spunta + nome in accento (non solo colore).
- *Comandi palette*: `stile.classico|composizione|strumento` («Stile: …») e `aspetto.menu` («Aspetto…»). **Nessuna scorciatoia**:
  non ne serve una e le Mod+Shift+lettera libere sono poche (`pannelli.check.ts` riserva le combinazioni di CodeMirror).
- *CSP (B-005)*: fogli CSS importati da Vite (file `self`), nessuno stile inline dinamico oltre `element.style.top/left` per
  posizionare il popover, nessuna risorsa esterna, font di sistema. `prefers-reduced-motion`: le transizioni del menu sono dentro
  `@media (prefers-reduced-motion: no-preference)`. Focus visibile: `outline: 2px solid var(--fuoco)`.

**Portato.** Token completi (UI + `--sint-*` + carta/inchiostro/fuoco) in chiaro e scuro; tipografia (serif su titolo, indice,
nome file, formattazione per Composizione); indice come sommario con puntini guida e numero di pagina; colonna di lettura
centrata (`72ch`) e interlinea 1.75 nell'editor (Composizione); densità (UI 12,5 px, righe da 22 px, controlli da 24 px, cifre
tabulari, rail da 36 px, titoli a banda, palette a righe da 28 px) per Strumento; palette e modali per stile.

**Rinviato** (richiede modifiche strutturali o JS, non solo CSS):
- Composizione: *margine annotazioni* per le diagnostiche (gutter/widget CodeMirror per riga), collasso del margine, barra di
  stato con pallino pulsante/rombo (l'app non ha una barra di stato: lo stato è nell'header, `#stato`), modalità focus/macchina
  su righe `.riga` del mockup (restano quelle di `modalita.ts`).
- Strumento: pannelli a scomparsa CSS-only (checkbox + `:has`) e sezioni `<details>` impilabili, barra di stato a segmenti, linea
  di avanzamento della compilazione, barra a margine per righe modificate/aggiunte, righello dei diagnostici, selettore carta
  (`data-carta`) e zoom dell'anteprima, palette a gruppi con prompt `>`.
- Dimensione del font dell'editor per stile (13 px in Strumento): `--font-editor` è scritta inline da `aspetto.ts` (14 px o scelta
  utente) e prevale su qualunque foglio; si cambia col font stepper.

**Da fare prima di dare per chiuso (B-005).** Rebuild dell'exe di produzione e smoke interattivo: la verifica è stata fatta su
`vite build` servito staticamente con Edge headless e un shim di `invoke` solo nello scratchpad, senza CSP di produzione né Tauri.

## Correzioni menu Aspetto 2026-10-09

**Cosa.** Riesame dei tre stili e del menu «Aspetto»: nove difetti (punto 8 assorbito dal 3). Controlli nuovi: `src/stili/stili.check.ts`
(testo dei CSS + contrasto WCAG), `src/dipendenze.check.ts` (package.json/lock), nuovi casi in `src/stile.check.ts`; tutti nello script `check`.

**Scelte.**
- *(1) Bordo inset sull'area di testo.* `:root[data-stile=…] :focus-visible` (0,3,0) batteva `.ͼ1 .cm-content { outline: none }`. Ora
  `:focus-visible:not(.cm-content)` in Composizione e Strumento (il focus resta visibile altrove; l'anello dell'editor è `.cm-editor.cm-focused`).
  Il Classico non è toccato. Misurato in Edge: `outline-style` di `.cm-content` con focus, prima `solid` (Composizione, Strumento), ora `none`; Classico `none` prima e dopo.
- *(2) Tastiera dopo un clic nel popover.* Il pannello è focalizzabile (`tabIndex = -1`); `mousedown` su titolo/descrizione/aiuto (non sui radio, non sul
  pannello stesso, per non rompere la barra di scorrimento) fa `preventDefault` e tiene il focus sul menu; le frecce premute col focus sul pannello rientrano nei radio.
- *(3 e 8) Uscita del focus.* Soluzione APG più semplice: popover da *menu button* = `dialog` **non modale**, quindi **Tab non è confinato** e non c'è `aria-modal`.
  Tab scorre i radio (roving); dall'ultimo (Maiusc+Tab dal primo) chiude il menu e il focus passa al pulsante, da cui la Tab prosegue. In più il menu si chiude
  su `focusout` (relatedTarget fuori da pannello e pulsante) e su `focusin` esterno (rete di sicurezza: Tab che esce dal documento ha relatedTarget nullo;
  osservato in Edge: senza il `focusin` il menu restava aperto con `aria-expanded="true"`). Con relatedTarget nullo da solo non si chiude (clic su area non
  focalizzabile, finestra che perde il focus): decide il `pointerdown` esterno già esistente. Decisione pura in `stile.ts` (`focusEsceDalMenu`). Non si è toccato il
  dispatcher globale di `main.ts`: Ctrl+P apre la palette e il menu si chiude perché il focus passa al suo campo (osservato: prima menu e palette entrambi aperti).
- *(4) Dipendenze dirette.* `@codemirror/language ^6.12.4` e `@lezer/highlight ^1.2.3` in `dependencies` e in `packages[""].dependencies` del lock, a mano,
  senza installare nulla (già risolte: 6.12.4 / 1.2.3). `npm ls` ora le mostra senza `UNMET`/`extraneous`.
- *(5)* `header button#btn-aspetto[aria-expanded="true"]` (non più `#btn-barra`/`#btn-pacchetti`).
- *(6) Flash al primo paint.* Specchio dello stile in `localStorage` (chiave `typster.stile`, `CHIAVE_SPECCHIO_STILE`), compatibile con la CSP (nessuno script inline).
  `applicaStileSpecchiato()` è il primo statement di `main.ts` dopo gli import; il valore passa da `stileDaSpecchio` (solo valori esatti, altrimenti ignorato, mai
  `normalizza`). Lo specchio si riscrive a ogni `applicaStile` (cambio utente e caricamento del file); `try/catch` su ogni accesso. La fonte di verità resta il file:
  dopo il caricamento, se `!stileToccato`, vince il file (osservato: specchio `composizione`, file `strumento` → storia `classico, composizione, strumento`).
  Lo specchio non conta come «scelta dell'utente». Limite: il primissimo avvio, o con `localStorage` svuotato, passa ancora dal Classico.
- *(7) Scrittura ridondante.* Invio/Spazio su un radio con `aria-checked="true"` non fanno clic (`radioDaAttivare`); `impostaStile` persiste solo se il valore normalizzato
  cambia **o** se le impostazioni non sono ancora caricate (`devePersistereStile`: prima del caricamento una scelta uguale al default va comunque scritta, altrimenti
  il file vecchio la sovrascriverebbe). Osservato: 3 scritture su Invio/Invio/Spazio prima, 0 ora; la freccia che cambia stile scrive 1.
- *(9) Contrasto commento/selezione.* WCAG: L = 0,2126 R + 0,7152 G + 0,0722 B (canali linearizzati), contrasto = (L1+0,05)/(L2+0,05), soglia 4,5:1 (`stili.check.ts`).
  Composizione chiaro: `#696257` su `--rilievo #cec6b9` = 3,56:1 → `#575145` = **4,65:1** (su `--sfondo #f7f4ec` 7,16:1; stesso tono caldo, più scuro).
  Il controllo ha mostrato lo stesso difetto altrove, corretto con lo stesso criterio: Composizione scuro `#9f978b` su `#423c34` = 3,77:1 → `#b0a89c` = 4,63:1;
  Strumento chiaro `#5d646c` su `#c6cbd1` = 3,67:1 → `#4a5159` = 4,92:1; Strumento scuro `#8f969e` su `#33383f` = 3,95:1 → `#a3aab2` = 5,03:1.

**Verifica.** `npm run check`, `typecheck`, `build` a 0. Edge headless (profilo temporaneo, porta 9447, `Input.dispatch*` reali, `Emulation.setFocusEmulationEnabled`)
su build nuova contro build precedente (stesso sorgente pre-modifica compilato a parte): tutti i comportamenti sopra confermati. Mutazioni (check rossi, sorgenti ripristinati
con SHA-256 identico): `:not(.cm-content)` tolto (entrambi gli stili), `aria-expanded` largo (entrambi), tre colori commento ripristinati, dipendenza tolta da package.json e dal lock,
specchio non validato, persistenza sempre, radio sempre attivato, chiusura su destinazione sbagliata; in browser: senza `tabIndex`/`mousedown`/`focusin`/`focusout`/uscita con Tab,
senza chiamata dello specchio e con scrittura incondizionata il driver mostra di nuovo i comportamenti difettosi.

**Da fare (B-005).** Rebuild dell'exe di produzione e smoke interattivo con la CSP di Tauri (test fatto su `vite build` servito staticamente con un shim di `invoke`).

## Motore di evidenziazione Typst: campo impostazioni 2026-10-14

**Decisione.** Nuovo campo `motore_typst: String` in `Impostazioni` (`crates/typst-core/src/config.rs`), accanto a
`tema` e `stile` (stesso modello). Valori ammessi (costante pubblica `MOTORI_TYPST`): `"wasm"` (predefinito,
evidenziazione via modulo WASM di `codemirror-lang-typst`) e `"lezer"` (parser Lezer nativo `typst_lezer()`, senza
WASM). Nome serializzato esattamente `motore_typst`.

**Comportamento.**
- Valore sconosciuto, vuoto, null, di tipo sbagliato o con maiuscole/spazi diversi (`"LEZER"`, `" lezer"`) → `"wasm"`,
  in lettura (`da_json`) e in salvataggio (`normalizza`): il confronto è esatto, nessuna conversione di maiuscole.
- File `impostazioni.json` preesistente senza il campo → `"wasm"`, senza toccare gli altri campi.
- Ogni campo è normalizzato per conto suo (`motore_typst` valido con `tema`/`stile` non validi, e viceversa).
- Campi futuri sconosciuti ignorati; salvataggio atomico invariato.

**Guscio Tauri.** `src-tauri/src/lib.rs` non modificato: `leggi_impostazioni`/`salva_impostazioni` (de)serializzano
l'intera struct con serde, il campo transita da solo.

**Test** (`config.rs`, prefisso `impostazioni_motore_typst_`): default, round-trip con `"lezer"` + nome serializzato,
non ammesso (lettura, `normalizza`, file scritto), file vecchio senza campo, indipendenza da `tema`/`stile`, campi
futuri. Falliti PRIMA del fix (campo e parsing presenti, normalizzazione assente: `"cm6"` restava `"cm6"` invece di
`"wasm"`): `..._non_ammesso_torna_wasm` e `..._indipendente_da_tema_e_stile`; gli altri passavano già.

## Motore Typst selezionabile e menu Opzioni 2026-10-14

**Cosa.** Nuovo menu «Opzioni» (pulsante `#btn-opzioni`, popover `#menu-opzioni`) con un `radiogroup` «Evidenziazione Typst»:
«WASM (predefinito)» = `typst()` di `codemirror-lang-typst`, «Lezer (nativo, sperimentale)» = `typst_lezer()` da
`codemirror-lang-typst/lezer`. Cambia solo evidenziazione/modifica dei file Typst, non la compilazione. Applicazione a caldo,
senza ricaricare; persistenza nel campo `motore_typst` di `Impostazioni` (valori `"wasm"` predefinito, `"lezer"`; assente o
sconosciuto torna `wasm`, come per `stile`).

**Dipendenza.** `codemirror-lang-typst` `^0.4.0` → `^0.6.0` (installata 0.6.0, `NODE_OPTIONS=--use-system-ca`). La 0.6.0 sincronizza
da sé il parser WASM (`edit_many` + controllo lunghezza + parser ricreato su errore): il nostro modulo resiliente non serve più.

**Rimosso e perché.** `src/grammatica-typst.ts`, `src/sincronizzaparser.ts`, `src/sincronizzaparser.check.ts` (usavano
`typstHighlight`/`TypstParser` col costruttore della 0.4.0, non più compilabili) e la voce dallo script `check`. `@lezer/common`
non è più importato direttamente da `src/`: tolto da `dependencies`, dalla radice del lock e da `ATTESE` in `dipendenze.check.ts`
(resta come dipendenza transitiva). `ATTESE` = `@codemirror/language`, `@lezer/highlight`.

**File.** Nuovi: `src/motore.ts` + `motore.check.ts` (logica pura: tipo, predefinito, `normalizzaMotore`, etichette, titoli dei comandi,
`devePersistereMotore`; nello script `check`), `src/menu-opzioni.ts`, `src/stili/menu-opzioni.css`, `src/stili/tavolozza-typst.ts`.
Modificati: `editor.ts` (`impostaMotoreTypst`, caricamento per motore), `aspetto.ts` (`impostaMotore`, `motoreCorrente`, stessa gestione
di tema/stile: scelta prima del caricamento non sovrascritta, `motoreSalvato` per ritentare dopo un fallimento, scrittura solo se
cambia o a impostazioni non caricate), `impostazioni.ts`, `main.ts` (comandi `motore.wasm`, `motore.lezer`, `opzioni.menu`, nessuna
scorciatoia nuova), `index.html`, `stili/editor-stile.ts`, `stili/stili.check.ts`, `stili/composizione.css` e `strumento.css`
(evidenziazione di `aria-expanded` anche su `#btn-opzioni`), `menu-aspetto.ts` (esporta `crea`), `package.json`, `package-lock.json`.

**Scelte.**
- *Chunk.* `import("codemirror-lang-typst")` (wasm) e `import("codemirror-lang-typst/lezer")` sono chunk distinti: il build produce
  `index-*.js` (wasm, 222 kB, l'unico che riferisce il `.wasm`), `lezer-*.js` (138 kB, senza `.wasm`) e il principale. All'avvio l'editor parte col
  motore predefinito; caricate le impostazioni passa a quello salvato se diverso (in quel caso il chunk wasm si scarica comunque).
- *Guardia di versione.* `impostaTipo` e `impostaMotoreTypst` passano da un'unica `attivaCorrente()` con `versioneLinguaggio`; col
  documento Markdown il motore resta memorizzato e vale al prossimo Typst. Stesso motore: nessuna riconfigurazione.
- *Linter sintattico di `typst_lezer()` rimosso.* Ogni `linter()` di `@codemirror/lint` chiama `setDiagnostics`, che SOSTITUISCE tutti i
  diagnostici: a editor fermo (750 ms) cancellava quelli del compilatore (`mostraDiagnostici`). Misurato in Edge: col linter il range del
  compilatore sparisce e resta solo quello sintattico; senza, resta quello del compilatore. Il supporto viene ricostruito togliendo per
  identità `typstLezerLinter` da `LanguageSupport.support` (con `console.warn` se non lo trova). Il compilatore segnala già gli errori
  di sintassi. Nota: la descrizione richiesta per la voce Lezer cita il «controllo sintattico», che nell'app è quello del compilatore.
- *Completamento.* `estensioniAssistenza()` usa `override` (typst-ide + snippet): le fonti da language data, quindi il completamento dei
  built-in di `typst_lezer()`, non vengono interrogate. Vince typst-ide (invariato nei due motori); indentazione, piegatura e Invio nelle liste di Lezer restano attivi.
  Il motore wasm non è degradato.
- *Colori dei tag nuovi.* `typstTags` (delimitatori di formula, marcatori di lista, interpolazioni `#`) sono istanze diverse per ciascun
  motore (definite in entrambi i moduli), quindi la loro mappatura sta in un compartment separato (`evidenziazioneTypst`) riempito dopo
  l'import, con `Prec.highest`: stili «Composizione»/«Strumento» → mappa completa su `--sint-*` più i tag Typst; Classico chiaro →
  `TypstHighlightSytle` (4,6–6,7:1 su bianco, invariato); Classico scuro → tavolozza Catppuccin in `tavolozza-typst.ts`, perché i colori di
  `TypstHighlightSytle` sul fondo di `oneDark` (#282c34) misurano 2,1–3,1:1. Mappa estesa: `t.null`/`t.atom` (none/auto) come parole chiave,
  `t.propertyName` come math, `t.escape` (in Typst anche etichette e riferimenti) ora `--sint-rif` invece di stringa (cambia anche gli escape Markdown).
  Contrasti (WCAG, `stili.check.ts`): tutti i `--sint-*` di Composizione e Strumento ≥ 4,5:1 su `--sfondo` in chiaro e scuro; tavolozza del Classico
  scuro 4,96–9,42:1 su #282c34 (commento 4,96). Verificato in Edge con `getComputedStyle` per quattro combinazioni tema×stile e i due motori.
- *Menu.* Come «Aspetto»: dialog non modale, `radiogroup`, roving tabindex, frecce/Home/End spostano e selezionano, Invio/Spazio scelgono (senza
  riscrivere se già selezionato), Esc chiude e ridà il focus al pulsante, `aria-expanded`, chiusura su `focusout`/`focusin` esterno/`pointerdown`
  esterno, Tab non confinato (con un solo radio focalizzabile Tab e Maiusc+Tab chiudono). Aprire uno chiude l'altro (il focus passa all'altro menu).
  Selezionato = cerchio con spunta + bordo + nome in accento. Osservato: 0 scritture su Invio/Spazio sul radio già scelto, 1 per ogni cambio.
- *Pulsante a icona.* A 1280 px l'etichetta «Opzioni» faceva peggiorare l'header (altezza Classico 59→85, Composizione 40→55). Ora è un'icona
  (cursori) con `aria-label`/`title` «Opzioni»: header Classico 59 (identico a prima), Strumento 31 (identico), Composizione 41 (prima 40, padding ridotto).
  Il Classico a 1280 px andava già su due righe in tre pulsanti prima di questo lavoro.

**Limite grave del motore WASM 0.6.0 (non risolto upstream).** L'aggiornamento incrementale delle evidenziazioni (`patchHighlights`) dà risultati diversi dal
ricalcolo completo: fuzz in Node su 300 casi per tipo, confronto colore per carattere: modifiche singole 37/300 diverse, modifiche multiple 70/300,
**sostituzione dell'intero documento 294/300** (in Edge, dopo «seleziona tutto e incolla» le evidenziazioni risultano 0). Mitigazione parziale: `Editor.imposta`
(apertura file) ricrea il plugin del motore wasm via `reconfigure([])` e ritorno; `sostituisci` e la digitazione normale non sono coperti. Il motore Lezer non ha
il problema (albero Lezer incrementale). Da segnalare upstream, o da risolvere scegliendo Lezer come predefinito dopo un periodo di prova.

**Non verificato.** Rebuild dell'exe di produzione con la CSP di Tauri e smoke interattivo (B-005): le verifiche sono su `vite build` servito staticamente con Edge
headless e uno shim di `invoke` solo nello scratchpad. Non provato: la scelta fatta prima del caricamento delle impostazioni (logica identica allo stile, coperta solo
da `devePersistereMotore`), l'effetto reale di `motore_typst` finché il core non ha il campo, `Editor.imposta` in app (verificato solo il percorso equivalente da tastiera),
blocchi raw in altri linguaggi (`codeLanguages` non configurato: non c'è `@codemirror/language-data`, nessun pacchetto nuovo).

## Posizione degli errori dalla traccia 2026-10-14

**Problema.** Un errore nato dentro un pacchetto o un file fratello/dato (caso reale: `#gantt(yaml("gantt.yaml"))` con «string contains invalid digits» dentro il plugin
WASM di gantty) aveva `da/a = None`: finiva nel riquadro in basso senza dire dove correggere (F1-007 vieta di mappare lo span di un altro file sulle righe del main).

**Decisione.** F1-007 resta: lo span reale si usa solo se è nel main (`posizione_da_traccia = false`). Altrimenti `diagnostico_da` prende dalla `SourceDiagnostic.trace`
(`Vec<Spanned<Tracepoint>>`, riempita da `Trace::trace` mentre l'errore risale: **dal più interno al più esterno**, verificato in `typst-library-0.15.0/src/diag.rs`)
il **primo** tracepoint con span nel main, cioè il punto di chiamata più vicino al codice dell'utente; se nessuno, `None` come prima. Il compilatore scarta da solo i
tracepoint che racchiudono l'errore, quindi per un errore nel main la traccia è vuota. Campi nuovi di `Diagnostico` (serde, snake_case; i vecchi restano):
- `posizione_da_traccia: bool`: `da/a` vengono dal tracepoint e non dallo span reale.
- `origine: Option<String>`: dove l'errore è nato, solo se non è nel main: `rotto.yaml, riga 2, colonna 2` (progetto, percorso relativo con `/`) oppure
  `@preview/gantty:0.5.1 — src/lib.typ, riga R, colonna C` (pacchetto); riga/colonna 1-based dal `Source` nel magazzino (file già letti, nessuna scansione), altrimenti
  solo il percorso; `None` se lo span è detached (errori di plugin senza span: resta `None`).
- `traccia: Vec<String>`: tracepoint in italiano, ordine del compilatore (interno → esterno), p.es. «chiamata della funzione `yaml` (riga 3)»; fuori dal main
  «(lib.typ, riga 2)». `Import`/`Include` → «importazione del modulo `x`» / «inclusione del file `x`»; `Show` → «regola show su `x`».
- `suggerimenti: Vec<String>`: gli hint separati; `messaggio` li ha ancora accodati («\nsuggerimento: …») per compatibilità.
`errore_pdf` (errori di export raggruppati) usa la stessa regola: span nel main, altrimenti primo tracepoint nel main.

**Osservazioni.** Errore di dati (YAML rotto): lo span è un `DiagSpan::Range` nel file dati, quindi `origine` indica riga/colonna del YAML e `da/a` coprono la chiamata
`yaml("…")` nel main. Se il main stesso è un file in modifica ma l'errore è in un capitolo aperto, "main" resta il file in editing (`id_principale`), come per F1-007.

**Verifica.** Test in `world.rs`: errore in fratello (`panic`) e di tipo (`int("12x")`), errore nel main (flag false, nessuna origine), YAML rotto, più punti nel main
(vince il più interno), modulo annidato main→a.typ→b.typ (si scarta il punto dentro a.typ), hint separati, forma JSON, etichette file/pacchetto, descrizioni.
Prima del fix 6 test su 11 nuovi fallivano (`da: None`, `traccia: []`). Mutazioni: (1) trace ignorata → 6 test falliti; (2) ordine invertito (`.rev()`, più esterno) →
fallisce `con_piu_punti_nel_main_vince_il_piu_interno`. `cargo test -p typst-core -p typster` verde (477+17+6), clippy `-D warnings` e build `typster` puliti.
**Non verificato:** pacchetto reale `@preview` (richiede rete; l'etichetta `@ns/nome:ver — percorso` è coperta solo da test unitario su `FileId` sintetico) e il caso
plugin WASM di gantty con span detached; nessun test del frontend (competenza dell'altro agente).

## Errori con posizione e pallino di salvataggio 2026-10-14

### Lavoro 1 — Riquadro errori con posizione, cliccabile

**Problema.** Un errore nato in un pacchetto o in un file dati (`#gantt(yaml("gantt.yaml"))`) compariva in fondo come «[ERRORE] string contains invalid digits», senza riga né file.

**Contratto col core (altro agente, retro-compatibile).** Ogni diagnostico può avere, in più: `posizione_da_traccia: boolean` (`da`/`a` sono il punto di CHIAMATA nel main, non lo span reale), `origine: string | null`, `traccia: string[]`, `suggerimenti: string[]`. `messaggio` continua ad avere gli hint accodati. Il frontend li tratta come OPZIONALI (`DiagnosticoBase` in `src/errori.ts`; `DiagnosticoEditor` ne è un alias): con un core vecchio il comportamento resta quello di prima, salvo il nuovo riquadro.

**Cosa.**
- `src/errori.ts` (logica pura, testata in `errori.check.ts`): `posizioneDaOffset` (offset UTF-16 → riga/colonna da 1; la colonna conta i caratteri Unicode, un'emoji vale 1; offset oltre la fine/negativo/NaN stretti ai bordi; offset a metà di una coppia surrogata arretra all'inizio della coppia; il testo dell'editor ha solo `\n`), `intervalloDiagnostico` (clamp al documento corrente, `da <= a`), `ordinaDiagnostici` (errori prima, con posizione prima di senza, per offset, stabile), `riepilogoDiagnostici` («N errori, M avvisi», singolare dove serve), `descriviDiagnostico`, `messaggioMarcatore`.
- `src/riquadro-errori.ts` (DOM): una riga per diagnostico dentro `#errori` (ora `role="region"` con nome). La riga contiene la parola «Errore»/«Avviso» (mai solo colore, in grassetto e col testo normale), il messaggio, «riga R, colonna C» (con «(punto di chiamata)» se `posizione_da_traccia`), poi in tono attenuato «Origine: …», i passi della traccia e «Suggerimento: …». I suggerimenti già contenuti nel `messaggio` non si ripetono (il core li accoda). Le righe con posizione sono `<button>` (Invio e Spazio nativi): `Editor.vaiA(da, a)` (nuovo parametro `fine` opzionale: seleziona `da..a`, clamp al documento corrente, scorre in vista, focus all'editor).
- Intestazione «N errori, M avvisi» con `role="status"`: unico elemento live, aggiornato solo se il testo cambia; con una sola voce è `visually-hidden` (resta per i lettori di schermo). L'elenco si ricostruisce solo se contenuto o posizioni cambiano (evita di perdere il focus e lo sfarfallio a ogni compilazione). Altezza massima `min(40vh, 260px)` con scorrimento. La riga di stato mantiene il conteggio, ora con lo stesso riepilogo («2 errori, 1 avviso» al posto di «2 errore/i, 1 avviso/i»).
- Il riquadro mostra ORA tutti i diagnostici (prima solo quelli senza posizione, perché gli altri erano inline). In Markdown gli offset sono del Typst generato: nessuna posizione né righe cliccabili, con la nota «Diagnostica (dal Typst generato dalla conversione): …» nell'intestazione.
- Senza posizione né origine: riga comunque mostrata, con «Posizione non disponibile: controlla i file letti da questa chiamata (dati, pacchetti importati).». Con origine ma senza posizione: solo l'origine (niente nota).
- Marcatori inline (`mostraDiagnostici`): usano già `da`/`a` (con la posizione da traccia il core mette il punto di chiamata); il messaggio ora è `messaggioMarcatore`: messaggio, «(punto di chiamata)», «Origine: …».
- `mostraErrori` (errori di I/O e di comandi) passa dal riquadro (`mostraTesto`, `role="alert"`).

**Colori e contrasti (riusati i token, nessuno nuovo).** Testo normale e dettagli su `--errore-sfondo`; la gravità è una barra laterale 3 px (`--errore`/`--avviso`, componente UI ≥ 3:1) più la parola. Prima l'intero riquadro era `--errore` su `--errore-sfondo`: nel Classico scuro misura 3,94:1 (sotto AA), ora il testo è `--testo`. I dettagli usano `color-mix(in srgb, var(--testo) 92%, var(--errore-sfondo))` (82% dava 4,33:1 nel Classico chiaro). `stili.check.ts` (12) verifica nei tre stili e in chiaro/scuro: `--testo` ≥ 4,5:1 e dettagli attenuati ≥ 4,5:1 su `--errore-sfondo` (minimo 5,46:1), barre ≥ 3:1, e che `index.html` usi ancora quel mix.

### Lavoro 2 — Pallino di salvataggio

**Stati** (forma diversa, oltre al colore; nome accessibile e `title` identici):

| Stato | Forma | Colore | Testo |
|---|---|---|---|
| `salvato` | cerchio pieno | `--verde` | «Salvato» |
| `in-corso` | cerchio con anello e punto | `--avviso` | «Salvataggio in corso» |
| `non-salvato` | rombo pieno | `--errore` | «Modifiche non salvate» |
| `nuovo` | anello vuoto | `--controllo` | «Nuovo documento, non ancora salvato» |

**Regola** (`statoSalvataggio` in `src/salvataggio.ts`, funzione pura di `modificato, inSalvataggio, errore, haPercorso`; tabella a 16 combinazioni in `salvataggio.check.ts`): scrittura in volo > errore > modificato > senza file ancora intatto (neutro) > salvato. Documento senza file: neutro finché è intatto (appena creato), ROSSO appena ha modifiche (chiudere l'app le perderebbe; anche dopo conversione, che lo marca modificato). Un nuovo tentativo dopo un errore torna ambra, poi esce rosso se fallisce ancora (con l'avviso già esistente di `fileops.ts`, invariato).

**Ambra minima.** `avanza` (pura) trattiene l'ambra per almeno 400 ms (`AMBRA_MIN_MS`) e rivaluta allo scadere con lo stato voluto più recente: cambi intermedi non lampeggiano; l'entrata nell'ambra è immediata, l'uscita da altri stati pure. Il timer sta in `src/pallino.ts` (DOM).

**Annunci.** Regione `aria-live="polite"` (`#annuncio-salvataggio`) separata dal pallino; si annunciano solo gli esiti «Salvato» / «Modifiche non salvate» e solo se diversi dall'ULTIMO ANNUNCIATO (l'ambra e il neutro mai; il primo disegno no). Durante la digitazione con autosave (non salvato → ambra → non salvato) non c'è raffica: un solo annuncio al primo tasto dopo un salvataggio. `#nome-file` resta `aria-live="polite"` ma ora cambia solo col file (il «•» è sparito dal testo; resta nel titolo finestra).

**Cablaggio senza nuove scritture.** `fileops.ts`: `GestoreFile.suSalvataggio(ascoltatore)` e `statoScrittura()` (tipo `StatoScrittura { inSalvataggio, errore }`). L'ascoltatore è chiamato in `scriviSu` solo dove `salvataggioInCorso` cambia (inizio e `finally`); `erroreScrittura` si imposta nel `catch` e si azzera quando la richiesta torna senza eccezione. NON si è toccato `notifica()` (che chiama `osserva_documento`): nessuna chiamata al backend in più, nessun cambio di semantica del salvataggio. In `main.ts` l'errore conta solo con `modificato` (caricato un altro documento non resta rosso).

**Layout a 1280 px.** Il pallino (+18 px) mandava a capo la barra: Composizione 41→55 (zero margine), Classico 59→85. Corretto con `gap` ridotto (Classico 12→8 px in `index.html`, Composizione 8→6 px in `composizione.css`; Strumento 4 px invariato) e `#nome-file` con `max-width: 24ch` + puntini (`title` = percorso intero) e `flex: none` (a cedere spazio sono gli altri elementi, come prima). Misurato: altezza barra 59 / 41 / 31 px, identica a prima, nei tre stili.

**Accessibilità/movimento.** Nessuna animazione (nulla da disattivare con `prefers-reduced-motion`); `forced-colors: active` → `forced-color-adjust: none` sul pallino per conservare i colori. Contrasto componenti ≥ 3:1 su `--pannello` verificato in `stili.check.ts` (12) per `--verde`, `--avviso`, `--errore`, `--controllo`, nei tre stili e in chiaro/scuro.

### Verifica

- `npm.cmd run check` (include `errori.check.ts` e `salvataggio.check.ts`, nuovi nello script), `typecheck`, `build`: exit 0.
- Mutazioni (8, tutte uccise, file ripristinati): colonna senza salto della coppia surrogata, ordine errori/avvisi invertito, assenza del clamp, suggerimenti non deduplicati, nota senza posizione mostrata anche con origine, ambra senza minimo, precedenza ambra/errore scambiata, annuncio sempre vero.
- Edge headless (profilo temporaneo, processi chiusi per PID, 0 rimasti) su `vite build` servito staticamente con shim di `invoke`: 3 stili × chiaro/scuro con 5 diagnostici (main, da traccia con origine, senza posizione, avviso nel main, avviso con sola origine): righe/colonne corrette (es. «riga 54, colonna 1 (punto di chiamata)» con emoji nelle righe precedenti), click e Invio/Spazio selezionano l'intervallo, scorrono e riportano il focus all'editor, documento accorciato dopo la compilazione → clamp senza eccezioni. Pallino: nuovo → rosso al primo tasto → ambra a 20 ms da Ctrl+S → ancora ambra a 170 ms con scrittura finita da 90 ms → verde a 520 ms → rosso alla modifica → rosso dopo scrittura fallita → ambra per scrittura lunga → verde.

### Non verificato

- Rebuild dell'exe di produzione con la CSP di Tauri e smoke interattivo (B-005): a carico del coordinatore.
- Il core con i nuovi campi non c'era durante la prova: i diagnostici di esempio sono stati forniti dallo shim.
- Lettori di schermo reali (annunci `aria-live`, nomi accessibili): verificati solo gli attributi nel DOM.
- Autosave reale e `salva_file` vero (lo shim risponde «scritto» o lancia): il flusso è provato con Ctrl+S → «Salva con nome» (dialog simulato).
- Alto contrasto di Windows (`forced-colors`): regola aggiunta, non provata.

## Motore Typst predefinito → lezer 2026-10-15

**Decisione.** Il motore di evidenziazione Typst predefinito diventa `lezer` (prima `wasm`). `MOTORI_TYPST` in `config.rs` e `MOTORI` in `src/motore.ts` sono ora
`["lezer", "wasm"]` (il primo è il predefinito); valore assente, non ammesso o file di impostazioni vecchio → `lezer`. L'etichetta «(predefinito)» passa a Lezer.

**Motivo.** Il fuzz sui motori ha dato 0 differenze su 9600 transazioni per Lezer, contro l'1,6%-98% di colori non aggiornati del WASM 0.6.0 (vedi la voce
«Motore Typst selezionabile e menu Opzioni»). `wasm` resta selezionabile da Opzioni. Chi aveva già `motore_typst` scritto su disco mantiene la propria scelta.

**Test.** `config.rs`: i test `impostazioni_motore_typst_*` assumono `lezer` come predefinito/ripiego e usano `wasm` come valore non predefinito; `motore.check.ts` allineato.

## Strumenti nella barra laterale e Modelli — contratto 2026-10-16

**Decisione.** Gli strumenti di creazione pagina vivono in un pannello **«Strumenti»** della barra laterale (icona nella colonna delle attività, accanto a Indice/File/Pacchetti), non in un gruppo della barra in alto. Il pannello mostra una lista (Modelli, Pagina, Inserisci…) e, scelto uno strumento, il suo pannello con «‹ Strumenti» per tornare. La barra in alto non cambia (solo «Nuovo» avrà una freccia «Nuovo da modello…»). Ordine dei lavori: **1. Modelli**, 2. Pagina, 3. Inserisci. Mock approvato: `output/anteprime-menu/strumenti.html` (stile «Scheda»).

**Modelli — formato.** Un modello è un file `.typ` che compila da solo: le righe di intestazione sono commenti Typst (`//`) in testa al file, prima del corpo.
```
// @modello: Lettera di richiesta
// @descrizione: Lettera formale di una pagina
// @pagine: 1
// @campo: destinatario | testo | Studio Bianchi
// @campo: oggetto | testo |
// @campo: data | data | oggi
// @campo: firma | testo | Nicola Ballari
```
Righe `// @campo: <nome> | <tipo> | <predefinito>`; nome = `[a-z][a-z0-9_]*` (max 40), tipo ∈ `testo` | `data` | `numero` | `elenco`; predefinito facoltativo (`oggi` per `data` = data odierna ISO `AAAA-MM-GG`). Il corpo contiene segnaposto `{{nome}}`.

**Sostituzione.** `{{nome}}` inserisce il valore **come testo** in markup Typst: i caratteri speciali (`\ # $ * _ @ < > ` ~ [ ] " '`) vengono preceduti da `\`. `{{!nome}}` inserisce il valore verbatim (codice Typst: per `numero` o per il chi sa cosa fa). Tipo `elenco`: ogni riga non vuota diventa `- riga` (escapata). Un segnaposto senza campo dichiarato, o un campo dichiarato mai usato, produce un avviso (non un errore). Valore mancante → predefinito → stringa vuota. Le righe `// @…` di intestazione **non** finiscono nel documento generato.

**Dove stanno.** Modelli integrati (3: lettera, scheda prodotto, relazione breve) incorporati nel core con `include_str!`; modelli dell'utente in `dirs::config_dir()/typster/modelli/*.typ` (dati esterni, copiabili e versionabili). Id = nome file senza estensione; un modello utente con lo stesso id **sostituisce** l'integrato. Gli integrati non si cancellano; quelli utente sì.

**Sicurezza.** Id e nomi file validati (solo `[a-z0-9_-]`, max 60, niente separatori, nomi riservati Windows rifiutati); percorso risolto e verificato dentro la cartella dei modelli; scrittura atomica; tetto di 256 KiB per modello e 64 campi.

**Architettura.** Core puro `crates/typst-core/src/modelli.rs` (parsing, sostituzione, escaping; I/O solo nelle funzioni che ricevono la cartella come parametro, per i test), comandi Tauri sottili: `elenca_modelli`, `compila_modello(id, valori)`, `salva_come_modello(nome, sorgente, campi)`, `elimina_modello(id)`. Frontend: pannello `src/pannelli/strumenti.ts` + logica pura `strumenti-logica.ts`/`modelli-logica.ts` con `*.check.ts`.

**Non incluso in questo passo.** Pannelli Pagina e Inserisci; «Nuovo da modello…» nella palette (dopo il pannello); modelli di pacchetto Typst Universe.

### Modelli — scelte del core (implementazione 2026-10-16)

- **Escaping oltre il contratto.** Oltre a `\ # $ * _ @ < > ` ~ [ ] " '`, a inizio riga si escapano `=`, `-`, `+`, `/` se seguiti da spazio o fine riga, e il punto di `1.`; si escapa anche la prima `/` di `//` (commento). Senza, il valore `-5` o `http://x` romperebbe la struttura. `-5` a inizio riga resta com'è.
- **Predefiniti multi-riga.** Nella riga `@campo`, `\n` = a capo e `\` = `\` (serve agli `elenco` e al round-trip).
- **Segnaposto non dichiarato**: resta nel testo (`{{x}}`) con un avviso; `{{…}}` con contenuto che non è un nome valido resta intatto senza avviso. Campo `numero` non numerico: avviso, valore inserito comunque.
- **`oggi`** si risolve con `chrono::Local` (data locale), solo per il tipo `data`.
- **Intestazione**: blocco iniziale di righe vuote o `//`; si rimuovono solo le direttive `// @…` lì; commenti normali e direttive dopo il corpo restano. Righe mal formate → avviso con numero di riga, mai panic.
- **Tetti**: 256 KiB per modello e per valore, 64 campi (in lettura ieccedenti si ignorano con avviso, in salvataggio è errore), documento generato max 4 MiB, id max 60, nome campo max 40, nome visibile max 80, 500 file esaminati.
- **Percorsi**: id validato prima di costruire il percorso; per file esistenti il percorso reale (symlink risolti) deve stare nella cartella; nomi riservati Windows rifiutati; scrittura via `config::scrivi_atomico`. Il test sui symlink è `#[cfg(unix)]` (non eseguito su Windows).
- **JSON dei comandi**: `ModelloInfo {id, nome, descrizione, pagine, campi[{nome,tipo,predefinito}], origine, avvisi}`; `compila_modello → {sorgente, avvisi}`; `salva_come_modello → id`; `elimina_modello → void`; errori `Err(String)` in italiano.
- **Frontend**: `src/pannelli/strumenti.ts` (+ `strumenti-logica.ts`, `strumenti.check.ts`, `src/stili/pannello-strumenti.css`); `Editor.selezione()` e `sostituisciIntervallo()`; `GestoreFile.nuovoDa(testo)`; comando di palette «Nuovo da modello…». Non ancora fatti: freccia accanto a «Nuovo», pannelli Pagina e Inserisci.

#### Modelli — correzioni dopo revisione (2026-10-16)

- **Tetto sul sorgente generato (critico).** `sostituisci_segnaposto` riceve il tetto (4 MiB) e controlla la lunghezza prevista PRIMA di ogni `push_str`: un modello da 256 KiB con ~50.000 `{{a}}` e un valore da 256 KiB ora risponde `Err(Limite)` dopo poche occorrenze invece di allocare decine di GB. La resa del valore (`escapa_markup`/`rendi_elenco`) si calcola una sola volta per campo; il verbatim usa il valore direttamente. Test con allocatore contatore (massima allocazione singola < 16 MiB).
- **Titoli `==`, `===`…** a inizio riga: se la sequenza di `=` è seguita da spazio o fine riga si escapa il primo `=`; `==x` (senza spazio) resta com'è.
- **A capo Unicode.** `normalizza_a_capo` porta a `\n` tutti i terminatori che Typst riconosce (`\r\n`, `\r`, VT, FF, NEL, LS, PS) prima di dividere in righe: in `escapa_markup`, `rendi_elenco` e `codifica_predefinito`.
- **Elenco/compilazione coerenti.** Un file utente guasto (oltre 256 KiB, non UTF-8, fuori cartella) con l'id di un integrato compare in elenco come `origine: utente`, `campi: []`, con un avviso che descrive l'errore e invita a eliminare il file (l'UI offre «Elimina» e l'integrato torna visibile); `compila_modello` dà lo stesso errore, con la stessa frase. I file guasti con id NON integrato restano saltati (invariato).
- **Sovrascrittura esplicita.** `salva_come_modello(…, pagine, sovrascrivi: bool)`; se esiste già un modello UTENTE con lo stesso id e `sovrascrivi` è `false` → nuova variante `ErroreModello::GiaEsistente`, messaggio che INIZIA con «Esiste già un modello» (il frontend lo riconosce dal prefisso, chiede conferma e riprova con `true`). Sostituire un integrato resta consentito senza conferma. Comando Tauri: `sovrascrivi: Option<bool>` (default `false`).
- **Corpo vuoto.** `salva` controlla `corpo.trim().is_empty()` (dopo aver tolto la testata), non il sorgente: un sorgente di sole righe `// @…` non salva un modello vuoto.
- **Costo O(n) con `{{` senza chiusura.** Il `}}` si cerca solo nei primi 256 byte dopo `{{` (`FINESTRA_SEGNAPOSTO`): un nome valido (max 40, più `!` e spazi) ci sta largamente; un segnaposto con oltre 256 byte tra le graffe non è più riconosciuto (resta testo). Test di prestazione su 256 KiB di `{{`.
- **`numero`** accetta la virgola decimale (`12,5`, una sola virgola e nessun punto) per l'avviso; il valore resta testo.
- **Tetto dei file.** Le 500 voci esaminate sono contate dopo il filtro per estensione `.typ`.
- **Percorsi.** In salvataggio la cartella è canonicalizzata dopo `create_dir_all` e quel percorso è usato ovunque; `elimina` fa una sola `symlink_metadata`.

## Strumento «Pagina» — contratto 2026-10-16

**Decisione.** Seconda voce del pannello Strumenti: legge e scrive la regola `#set page(...)` del documento aperto. Solo frontend (TypeScript puro, nessun comando nuovo): `src/pannelli/pagina-logica.ts` (analisi e riscrittura, verificata da `pagina.check.ts`) e `src/pannelli/pagina-vista.ts` (vista).

**Cosa gestisce.** Argomenti `paper` (formati a4, a3, a5, us-letter, us-legal), `flipped`, `margin` (un solo valore, oppure `(x: …, y: …)`), `columns`, `header` (contenuto semplice `[testo]`), `numbering` (numerazione di pagina: `"1"`). Lunghezze `numero + cm|mm|pt|in|em` (la virgola decimale si normalizza in punto).

**Cosa NON tocca.** Ogni altro argomento della chiamata (`fill`, `footer`, `margin` con `top/left/…`, `header` complesso, ecc.) si conserva così com'è, nello stesso ordine; i campi corrispondenti nel pannello sono disabilitati con una nota. Se nel documento non c'è `#set page(...)`, la regola si inserisce in cima. Si considera solo la **prima** `#set page(` che inizia una riga (fuori da stringhe e commenti).

**Scrittura.** Una sola sostituzione dell'intervallo della chiamata (un solo passo di annullamento), formattata su più righe se ha più di due argomenti. Dopo «Applica» la regola resta selezionata nell'editor.

## Strumento «Inserisci» — contratto 2026-10-16

**Decisione.** Terza voce del pannello Strumenti: moduli che producono codice Typst da inserire alla posizione del cursore (sostituisce l'eventuale selezione, come `Editor.inserisci`). Solo frontend: `src/pannelli/inserisci-logica.ts` (generatori puri, verificati da `inserisci.check.ts`) e `src/pannelli/inserisci-vista.ts`.

**Elementi.** Tabella (righe 1-50, colonne 1-12, riga di intestazione facoltativa → `#table(columns: N, table.header[…][…], [], …)`); Figura (percorso immagine relativo, didascalia, etichetta → `#figure(image("…"), caption: […]) <etichetta>`); Equazione (testo, in linea `$x$` o visualizzata `$ x $`; rifiutato un `$` nel testo); Riferimento (un menu delle etichette `<nome>` presenti nel documento → `@nome`); Nota a piè di pagina (`#footnote[…]`).

**Regole.** Il testo che finisce in contenuto `[…]` si escapa (`escapaContenuto`); il percorso dell'immagine diventa una stringa Typst (barre normali, `\` e `"` escapati); le etichette devono essere `[A-Za-z_][\w:.-]*`. Il codice generato usa il prefisso `#`, quindi vale in modalità markup: dentro codice o formule va adattato a mano. Nessuna finestra di scelta file (percorso digitato); nessun comando Tauri nuovo.

## Strumento «Equazioni» — contratto 2026-10-16

**Decisione.** Quarta voce del pannello Strumenti. Solo frontend: `src/pannelli/equazioni-logica.ts` (generatori puri + gestione dei pacchetti e del blocco di impostazioni, verificati da `equazioni.check.ts`) e `src/pannelli/equazioni-vista.ts`. La voce «Equazione» di Inserisci resta (formula semplice).

**Flusso.** Si compone una formula nel campo «Formula» aggiungendo strutture con un modulo (ogni struttura ha i suoi campi e produce codice matematico) o scrivendola a mano; poi opzioni (su una riga propria, etichetta `eq:nome`) e «Inserisci nel documento».

**Strutture native** (nessun pacchetto): frazione `frac(a, b)`, radice `sqrt(x)`/`root(n, x)`, somma/produttoria `sum_(i = 1)^(n) …`, integrale `integral_(a)^(b) f dif x`, limite `lim_(x -> 0) f`, matrice `mat(…; …)` righe×colonne, sistema `cases(…)`, vettore `vec(…)`, binomiale `binom(n, k)`, accento `hat/tilde/bar/dot/arrow(x)`.

**Pacchetti (versioni dall'indice di Typst Universe del 2026-10-16, da riverificare):** `physica 0.9.8` (derivata `dv(f, x)`, parziale `pdv(f, x)`, differenziale `dd(x)`, `ket`/`bra`/`braket`, `grad`/`div`/`curl`/`laplacian`), `mitex 0.2.7` («Incolla da LaTeX» → `#mitex(\`…\`)`, il LaTeX non può contenere il backtick), `equate 0.3.3` (numerazione per riga e formule su più pagine). Usare una struttura di un pacchetto aggiunge `#import "@preview/<nome>:<versione>": *` in cima al documento **solo se** non c'è già un import di quel pacchetto (in una sola transazione con l'inserimento). I pacchetti si scaricano alla prima compilazione (serve la rete).

**Impostazioni delle equazioni.** Un blocco gestito, delimitato da `// typster:equazioni` e `// fine typster:equazioni`, in cima al documento: numerazione `#set math.equation(numbering: "(1)")` e, se richiesto, `equate`. Si riscrive/rimuove in modo idempotente; fuori dal blocco non si tocca niente.

**Limiti noti.** Niente anteprima della formula (richiede un comando nuovo nel core: passo successivo); il codice vale nel testo normale; non si verifica la compilazione dei frammenti che usano pacchetti (rete).

## Strumento «Grafici» — piano (da realizzare, non iniziato) 2026-10-16

**Obiettivo.** Quinta voce del pannello Strumenti: creare grafici scientifici e diagrammi senza scrivere il codice dei pacchetti. Va realizzato quando c'è più sessione libera (consumo settimanale alto il 2026-10-16).

**Pacchetti candidati** (indice di Typst Universe del 2026-10-16): `lilaq 0.6.0` (grafici scientifici: linee, punti, barre, istogrammi, assi, legende; compilatore ≥ 0.13) — scelta principale per i grafici di dati; `cetz 0.5.2` + `cetz-plot 0.1.4` (disegno e grafici di funzioni; alternativa più libera); `fletcher 0.5.8` (diagrammi a nodi e frecce, diagrammi di flusso); `diagraph 0.3.7` (grafi con Graphviz, via WASM). Da scartare: `plotst` (poco mantenuto), `nulite` (Vega-Lite).

**Funzioni previste, in ordine:** (1) grafico di dati da tabella incollata o CSV (`lilaq`: linee, punti, barre; titoli, etichette degli assi, legenda, griglia); (2) grafico di una funzione `y = f(x)` con intervallo; (3) diagramma di flusso (`fletcher`) da un elenco di nodi e collegamenti; (4) esporta/inserisci come `#figure` con didascalia ed etichetta (riuso di Inserisci).

**Dati.** Tabella incollata (separatori tab/virgola/punto e virgola) → controllo della tipologia delle colonne → `#let dati = (…)` oppure `csv("file.csv")` se il file è nella cartella del progetto. Validazione numerica nel frontend, tetto di righe.

**Passi di lavoro (stima).** A: contratto e logica pura (parser dei dati, generatori di codice `lilaq`) con check — solo TypeScript. B: vista a moduli. C: anteprima del grafico tramite un comando di core che compila un documento di prova (lo stesso comando serve all'anteprima delle equazioni: va fatto una volta sola). D: prove sull'exe di produzione. Costo atteso: simile a Modelli + Pagina insieme; fare C una volta sola per Equazioni e Grafici.

**Prerequisito.** Verificare come l'app si comporta offline con i pacchetti `@preview` e se mostrare un avviso prima del primo download.

### Grafici — v1 realizzata (2026-10-16)

Fatto: passo A+B del piano, senza anteprima. `src/pannelli/grafici-logica.ts` (analisi della tabella incollata: separatori tab/`;`/spazi/virgola, virgola decimale, intestazione riconosciuta, tetti 2000 righe e 6 serie; generatore `lilaq` per linee `lq.plot`, punti `lq.scatter`, barre `lq.bar`, con titolo, etichette degli assi, legenda, `#figure` con didascalia ed etichetta) e `grafici-vista.ts`; `grafici.check.ts`. Il pacchetto `lilaq 0.6.0` si importa (`as lq`) solo se manca, nella stessa transazione. **Verificato compilando davvero** (test Rust `#[ignore]` con rete, `cargo test -p typst-core --test inserisci_snippet -- --ignored`): lilaq e, nello stesso modo, physica 0.9.8, mitex 0.2.7 ed equate 0.3.3 compilano con i frammenti generati; il download automatico dei pacchetti `@preview` funziona nell'ambiente di prova.

Resta del piano: grafico di una funzione `y = f(x)` (cetz-plot o lilaq `plot` con un intervallo campionato), diagrammi di flusso (fletcher), grafi (diagraph), CSV da file del progetto, e l'**anteprima** (comando di core, condiviso con le Equazioni). Nota: con le barre e più serie lilaq le sovrappone; un raggruppamento è da fare.

## Stampa del documento — 2026-10-23

**Decisione.** Pulsante «Stampa» (barra in alto) e comando di palette «Stampa il documento…», **senza scorciatoia** (Ctrl+P è della palette; le combinazioni con `Alt` sono escluse perché AltGr = Ctrl+Alt rompe la digitazione su alcuni layout: bocciata `Ctrl+Alt+P` dalla revisione). Si stampano le **pagine SVG dell'anteprima** con `window.print()` di WebView2 (finestra di stampa di Windows, «Salva come PDF»), non il file PDF esportato: stesso layout di Typst, nessun programma esterno, nessun codice Rust.

**Come.** `anteprima.preparaStampa()` carica tutte le pagine (max 8 richieste in parallelo, nuovi tentativi, scadenza 30 s); `stampa.ts` genera una `@page` per ogni formato distinto (pagine con nome `f1`, `f2`, … per i documenti a formati misti) e l'altezza di ogni pagina in pt; `stili/stampa.css` (`@media print`) nasconde tutto tranne le pagine. Le regole `@page` stanno in un `CSSStyleSheet` costruito da codice. Con errori di compilazione non si stampa (l'anteprima mostrerebbe una versione vecchia); se il testo non è compilato si compila prima e si attende che la compilazione sia ferma, poi si ricontrolla prima di aprire la finestra.

**Verifica.** Edge headless `--print-to-pdf` sulla struttura reale di `index.html`: con il CSS 3 pagine A4 (595×842 pt) e le definizioni SVG condivise stampate su ognuna; senza il CSS 1 pagina Letter con l'interfaccia (la prova può fallire); formati misti → 595×842, 842×595, 595×842 pt. **Non provata**: la finestra di stampa vera dentro l'exe (spetta all'utente).

**Limiti noti.** Le pagine sono grafica vettoriale (il testo stampato non è selezionabile nel PDF salvato dalla finestra di stampa); per un PDF con testo selezionabile e PDF/A usare «Esporta». Se `afterprint` non arrivasse, il ritorno del fuoco alla finestra chiude lo stato di stampa.

## Scheda «Gantt» (pacchetto gantty) — 2026-10-23

**Decisione.** Sesta voce del pannello Strumenti, basata sul pacchetto `@preview/gantty:0.5.1` (LGPL-3.0-o-successiva: non incorporato, lo scarica l'utente alla prima compilazione, come gli altri). Solo frontend: `src/pannelli/gantt-logica.ts` (validazione e generatore, verificati da `gantt.check.ts`) e `gantt-vista.ts`; `gantty` è tra i pacchetti gestiti di `equazioni-logica.ts` (import aggiunto solo se manca).

**Formato** (letto dal pacchetto installato, `src/normalize.typ`, NON dal README): dizionario passato a `#gantt((...))` con `start`/`end` (testo ISO), `tasks` (`name` + `start`/`end`[/`done`], oppure `subtasks`), `id` e `dependencies: ((id: "a1"),)`, `milestones` (`name`, `date`, `show-date`). **Il README è obsoleto**: nella 0.5.1 le chiavi `headers` e `show-today` del file YAML non hanno più effetto (le intestazioni sono un parametro del «drawer»; la riga «oggi» la disegna il drawer di default quando cade nell'intervallo), quindi la scheda non le offre.

**Scelte dalla revisione.** `start`/`end` sono sempre espliciti (gantty li ricava solo dalle attività e lascerebbe fuori pietre miliari e completamenti in ritardo); l'intervallo deve coprire almeno un giorno (gantty divide per l'ampiezza); anni 1900-2200 (fuori, l'intestazione per mesi/settimane rallenta o va in overflow); nomi ≤ 80 caratteri; completamento non prima dell'inizio; dipendenze circolari rifiutate; `escapaContenuto` ora escapa anche `/` (una didascalia con `//` diventava un commento: vale anche per Grafici). Nella scheda il ridisegno avviene solo quando cambia il numero di righe (le etichette dei menu si aggiornano sul posto) e il fuoco torna sul campo giusto.

**Verifica.** Test Rust `#[ignore]` `gantt_gantty_compila` (rete o cache): diagramma semplice e dentro una figura citata compilano con il pacchetto vero. La stringa del test è una copia dell'output di `generaGantt` per `esempioGantt("2026-06-01")`: va rigenerata se cambia il generatore.

## Scheda «Gantt»: passa al pacchetto gantt-newgen incorporato — 2026-10-23

**Decisione.** Sostituisce la voce «Scheda Gantt (pacchetto gantty)» qui sopra. L'utente voleva inglobare nell'eseguibile il **proprio** pacchetto (gantt-newgen 0.1.0, LGPL-3.0-o-successiva, sviluppato a parte nella cartella `typst_gantty`, implementazione indipendente ispirata a gantty): la scheda lo usa al posto di `@preview/gantty`, senza scaricare nulla.

**Incorporamento.** Copia dei soli file distribuiti in `crates/typst-core/pacchetti/gantt-newgen/` (`tools/sincronizza-gantt-newgen.ps1`: rifiuta di copiare se trova dati personali o percorsi locali; scrive `ORIGINE.txt` con l'impronta SHA-256, che non viene incorporato). `build.rs` genera una tabella `include_bytes!` (solo std) e `world.rs` serve il pacchetto come ripiego per `@local/gantt-newgen:0.1.0`: se l'utente ne installa una versione propria in `@local`, questa ha la precedenza (è il modo per rispettare la LGPL: sorgente in chiaro nel repo e sostituibile).

**Formato.** Schema del pacchetto (chiavi sconosciute = errore): `locale: "it"`, `headers`, `show-today`, `tasks`/`subtasks` con `progress` (percentuale) e dipendenze con `type`, `milestones`. La linea «oggi» è **spenta di default**: accesa estende l'intervallo fino a oggi (provato). Limiti di sicurezza della scheda: anni 1900-2200 (anno 9999 manda in panico il crate `time`), intervallo ≤ 25 anni (E-HDR-06 oltre ~10000 segmenti), nomi ≤ 80 caratteri, niente dipendenze circolari, completamento non prima dell'inizio.

**Verifica.** Test Rust non ignorato `gantt_newgen_incorporato_compila` (offline) sui casi generati da `generaGantt`; `gantt.check.ts`. Licenza: NOTICE e `licenses/LGPL-3.0-gantt-newgen.txt` / `GPL-3.0-gantt-newgen.txt`.

## gantt-newgen incorporato nell'eseguibile — 2026-10-23

**Decisione.** Il pacchetto Typst `gantt-newgen` 0.1.0 (dell'utente, LGPL-3.0-o-successiva, nessuna dipendenza) è incorporato in `typst-core` e servito con `#import "@local/gantt-newgen:0.1.0": gantt` senza installazione e senza rete. Complementa la voce «Scheda Gantt: passa al pacchetto gantt-newgen incorporato» (lato scheda/frontend): qui il lato Rust.

**Namespace.** `@local/gantt-newgen:0.1.0` **come ripiego**: il pacchetto installato dall'utente in `@local` prevale sempre. Motivo: (1) portabilità, perché dopo «Pubblica come @local» lo stesso documento compila anche con la CLI di Typst, che usa lo stesso nome; (2) l'utente può sviluppare il pacchetto installandone una versione propria nella stessa cartella, senza ricompilare Typster. Non si è usato `@preview` (il nome non è pubblicato su Universe, e `@preview` fa partire il download).

**Meccanismo.** `crates/typst-core/build.rs` (solo std, nessuna dipendenza) scansiona `pacchetti/gantt-newgen/` e genera in `OUT_DIR` una tabella ordinata `&[(&str, &[u8])]` con `include_bytes!` su percorsi assoluti con `/` (il progetto sta in un percorso con spazi; `{:?}` per l'escape); esclude `ORIGINE.txt`; `rerun-if-changed` sulla cartella. Il modulo `pacchetti_incorporati` (`file`, `elenca`, `versione`) riconosce solo namespace `local`, nome `gantt-newgen` e la versione del `typst.toml` incorporato (letta con il crate `toml` già presente, nessuna costante da tenere allineata). In `world.rs`, `CaricatoreFile::carica_da_pacchetto` prova prima `SystemPackages::obtain`: se riesce il comportamento è invariato; se fallisce e l'incorporato risponde `Some` serve quello, altrimenti propaga l'errore originale (i messaggi per `@preview` non cambiano). Verificato nel sorgente di typst-kit 0.15.0: per `@local` inesistente `obtain` dà `NotFound` subito, il download parte solo per il namespace `preview`.

**Licenza.** LGPL-3.0-o-successiva: sorgente in chiaro nel repo (`crates/typst-core/pacchetti/gantt-newgen/`, con LICENSE e COPYING) e sostituibile dall'utente installando una versione propria in `@local` (la sostituzione non richiede di ricompilare l'app). Testi in `licenses/`, NOTICE.

**Aggiornamento.** Non modificare i file a mano: rilanciare `tools/sincronizza-gantt-newgen.ps1` (rigenera la cartella e `ORIGINE.txt` con l'impronta). Se cambia la versione nel `typst.toml`, la versione servita cambia da sola e i documenti con l'import della vecchia versione tornano a dare «package not found» (test dedicato su `9.9.9`).

**Verifica.** Test unitari in `pacchetti_incorporati` (lookup, cartelle, pacchetti diversi, `ORIGINE.txt` assente, coerenza byte per byte con il disco) e in `world.rs` (priorità dell'installato e ripiego, con una cartella temporanea come radice dei pacchetti: nessun accesso a `%APPDATA%`/`%LOCALAPPDATA%`); integrazione in `tests/pacchetto_incorporato.rs` (offline, due facce dello schema e versione non incorporata).

**Limiti.** (1) Una sola versione incorporata; (2) l'installato prevale **per intero**: un `@local/gantt-newgen:0.1.0` incompleto non viene completato con i file incorporati (errore di file mancante); (3) vale solo per la compilazione di `EditorWorld`: elenco e completamento dei pacchetti (`packages.rs`) vedono solo ciò che è su disco; (4) l'eseguibile cresce della dimensione dei sorgenti del pacchetto (23 file di testo; misura non fatta); (5) la prova dell'exe di produzione spetta all'utente.
