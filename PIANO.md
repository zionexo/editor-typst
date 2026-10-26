# Piano di sviluppo — Typster (editor di testi basato su Typst)

Ciclo di vita: develop — dal 2026-09-29, su dichiarazione esplicita dell'utente (avvio implementazione Fase 5 da F5-001 rinomina e F5-003 impostazioni).

> Piano operativo derivato da [PROGETTO.md](PROGETTO.md). Traduce la visione e la
> roadmap a fasi in scelte tecniche concrete, struttura del codice, dipendenze
> bloccate e criteri di completamento ("Definition of Done") per ciascuna fase.
> Questo documento traccia le **decisioni di design**; va aggiornato a ogni fase.

---

## 0. Decisione tecnica (registrata)

Come da §6 di [PROGETTO.md](PROGETTO.md), la scelta dello stack è presa **prima
della Fase 1** ed è qui documentata.

| Voce | Decisione |
|------|-----------|
| **Stack** | **Opzione A — Tauri 2 + compilatore Typst embeddato come crate** |
| **Compilazione / anteprima / sync** | Crate Typst ufficiali embeddati nel backend Rust (di cui siamo padroni: compilazione incrementale, render anteprima, jump source↔preview). |
| **Assistenza alla scrittura** | **`typst-ide` embeddato nel core** (autocompletamento, hover; F2-002): è l'assistenza di base, sempre presente. **Tinymist NON fa parte dello stack di base né del bundle/installer** (DECISIONI.md D-010, supera D-001 per questa parte): è un **componente aggiuntivo opzionale** che l'utente installa/indica e che l'app, se presente, carica come LSP su processo separato (JSON-RPC su stdio, mai linkato come crate) — voce **X-001**, vedi §4 "Estensioni opzionali" e §7. |
| **Editor** | CodeMirror 6 nel webview. |
| **Confine core/presentazione** | Crate `typst-core` puro (nessuna dipendenza Tauri/UI), testabile headless; il guscio Tauri e il frontend dipendono da esso, mai il contrario. |

**Motivazione dell'ibrido.** I crate Typst embeddati danno il controllo totale
sul ciclo anteprima (fluidità, render della sola pagina visibile, sincronizzazione
SyncTeX-like via `typst-ide`) senza dipendere da un server esterno. Tinymist resta
la fonte dell'assistenza ricca (completamento, hover, diagnostica avanzata,
formattazione, rename) che sarebbe costoso reimplementare. Le due metà non si
sovrappongono: noi rendiamo, Tinymist assiste.

> **Aggiornamento (2026-09-28, D-010).** Il paragrafo sopra descrive l'ibrido
> come pianificato in Fase 0. Nei fatti l'assistenza di base è `typst-ide`
> embeddato (F2-002) e Tinymist diventa un **extra caricabile** a scelta
> dell'utente (X-001): l'app completa non lo richiede e non lo distribuisce.

---

## 1. Architettura concreta

Mappa il diagramma §5 di [PROGETTO.md](PROGETTO.md) sui componenti reali.

```
┌──────────────────────── FRONTEND (webview, TypeScript) ───────────────────────┐
│  CodeMirror 6 (grammatica Typst, tema)  ·  Pannello anteprima (SVG inline)     │
│  Toolbar formattazione  ·  Palette comandi  ·  Browser pacchetti/template      │
│  Assistenza typst-ide (via comandi) · [Client LSP opzionale — X-001, D-010]    │
└───────────────┬─────────────────────────────────────────────┬────────────────┘
       invoke()  │  comandi                       Channel<T>   │  frame SVG, diagnostica
┌───────────────▼─────────────────────────────────────────────▼────────────────┐
│                       GUSCIO TAURI  (src-tauri, Rust)                          │
│  #[tauri::command] (apri, salva, compila, esporta, cerca/installa pacchetto…) │
│  State condiviso (Mutex)  ·  [proxy LSP verso Tinymist opzionale — X-001]      │
└───────────────┬───────────────────────────────────────────────────────────────┘
       chiama    │  (nessuna dipendenza inversa)
┌───────────────▼───────────────────────────────────────────────────────────────┐
│                       CORE  (crate `typst-core`, Rust puro)                     │
│  • World/IdeWorld (typst, typst-kit: fonts + file + packages)                  │
│  • Orchestratore compilazione (debounce, comemo::evict, incrementale)          │
│  • Render anteprima (typst-svg) · Export (typst-pdf, typst-render)             │
│  • IDE (typst-ide: autocomplete, tooltip, jump_from_click/cursor)              │
│  • Gestore pacchetti/template (indice Universe, install @preview, @local)      │
│  • Configurazione (impostazioni, snippet, preferiti — file esterni)            │
└───────────────┬───────────────────────────────────────────────────────────────┘
        rete     │  (solo per scaricare pacchetti/template)
        ┌────────▼─────────┐
        │  Typst Universe   │  packages.typst.org/preview/index.json + tar.gz
        └───────────────────┘
```

**Tinymist opzionale** (X-001, D-010): **non** è parte fissa dell'architettura
né del bundle. Se l'utente lo installa/indica, il guscio Tauri avvia il suo
eseguibile come processo separato e fa da proxy puro JSON-RPC tra un client LSP
nel webview e lo stdio di Tinymist; se assente, l'assistenza resta quella di
`typst-ide` nel core. Le voci tra `[…]` nello schema esistono solo se X-001
viene attuata.

---

## 2. Struttura del repository (workspace Cargo + frontend)

```
editor-typst/
  PROGETTO.md
  PIANO.md
  DECISIONI.md            # log decisioni di design e bug noti
  SPEC-UNIFICATA.md       # raccolto UX da MarkText/Muya (D-014, D-025)
  fase8.md                # catalogo di riuso codice di terze parti (D-016)
  NOTICE                  # avvisi di licenza del codice derivato (F5-024)
  licenses/               # testi Apache-2.0 e MIT (F5-024)
  rust-toolchain.toml     # versione toolchain Rust bloccata
  Cargo.toml              # [workspace] members = ["crates/typst-core", "src-tauri"]; [profile.release] (F5-015)
  package.json            # frontend (Vite + TypeScript + CodeMirror)
  index.html
  vite.config.ts
  crates/
    typst-core/           # ⟵ CORE puro, nessuna dipendenza Tauri
      Cargo.toml
      examples/misura.rs  # misure di prestazione (F5-007…F5-009)
      src/
        lib.rs            # API pubblica, invalida_cache (comemo), tipi condivisi
        world.rs          # EditorWorld (World + IdeWorld): font, file, compila, svg per pagina, sync, invalida_risorse
        packages.rs       # indice Universe, installazione, template, @local, aggiornamenti (F5-012)
        progetto.rs       # cartella-progetto, rilevamento entrypoint (F4-001)
        config.rs         # impostazioni, snippet, scrittura atomica (F5-003)
        markdown.rs       # convertitori md↔typst (Fase 6)
        esporta.rs        # export PDF/PDF-A, PNG, SVG (F5-011)
        cronologia.rs     # cronologia versioni con chiave SHA-256 (F5-013, D-012)
        indice.rs         # indice del documento da OutlineNode (F5-020)
        albero.rs         # albero dei file confinato alla radice (F5-021)
        esterni.rs        # decisione ricarica/chiedi/sparito, verifica_integrita (F5-022, D-024)
        sorveglianza.rs   # watcher notify unico + Batcher + anti-loop (F5-022, D-017)
  src-tauri/              # ⟵ PRESENTAZIONE (guscio nativo)
    Cargo.toml
    build.rs
    tauri.conf.json       # bundle: resources NOTICE/licenses (F5-024)
    capabilities/default.json
    icons/                # segnaposto fino all'icona definitiva (D-020)
    src/
      main.rs
      lib.rs              # Builder, manage(State), comandi esposti al frontend
      osservatore.rs      # comandi file esterni, cronologia, invalida_risorse (F5-013/014/022)
      # lsp_proxy.rs      # NON presente: solo se si attua X-001 (opzionale)
  src/                    # ⟵ PRESENTAZIONE (UI web)
    main.ts               # avvio, ciclo di compilazione
    editor.ts             # CodeMirror 6 setup
    anteprima.ts          # render SVG per pagina (F5-008)
    svgdedup.ts           # dedup dei <defs> SVG (F5-009)
    debounce.ts           # debounce adattivo (F5-010)
    sincronizzazione.ts   # sync sorgente↔anteprima (F4-003)
    assistenza.ts         # completamento/hover typst-ide (F2-002)
    formattazione.ts      # toolbar (F2-001)
    paragrafo.ts, modalita.ts   # focus e macchina da scrivere (F5-023)
    comandi.ts, palette.ts      # registro comandi e palette (F5-006)
    impostazioni.ts, aspetto.ts # impostazioni, tema e font (F5-003…F5-005)
    fileops.ts            # apri/salva/nuovo, autosave (F5-013)
    cronologia.ts         # UI cronologia (F5-013)
    modificheesterne.ts   # reazione ai file modificati da fuori (F5-022)
    esporta.ts, opzioniesporta.ts # dialog di export (F5-011)
    pacchetti.ts, local.ts        # browser pacchetti, @local (F3-001, F4-004)
    barra-laterale.ts     # barra laterale a registro di pannelli (F5-019)
    pannelli/             # file.ts, indice.ts (+ *-logica.ts), tastiera.ts (F5-020/F5-021)
    *.check.ts            # test del frontend (node --experimental-strip-types), fixtures/
    # lsp/                # NON presente: solo se si attua X-001 (opzionale)
  tools/
    installer.ps1         # installer Windows Inno Setup portabile (F5-016, D-019)
    misura.ps1, misura_dedup.mjs  # misure di prestazione (F5-007…F5-009)
```

Test del core (`crates/typst-core`) eseguibili **senza** avviare la UI: compila
una stringa `.typ` di esempio e verifica PDF/SVG/diagnostica. Questo materializza
il confine core/presentazione richiesto dal documento.

---

## 3. Dipendenze bloccate

**Versioni esatte** (operatore `=` in Cargo per il sottoinsieme Typst: versioning
sincronizzato, niente patch automatiche — vedi §7 per l'allineamento con Tinymist,
rilevante solo per l'estensione opzionale X-001).

### Backend Rust — `typst-core`
```toml
# Già in uso dalla Fase 0:
typst        = "=0.15.0"
typst-layout = "=0.15.0"   # fornisce PagedDocument (NON riesportato da `typst`)
typst-kit    = { version = "=0.15.0", features = [
    "embedded-fonts", "scan-fonts", "system-packages",
    "universe-packages", "system-downloader", "datetime",
] }
typst-svg    = "=0.15.0"
# comemo     = "0.4"        # RIMOSSO (2026-09-30): pin diretto inutilizzato, gli evict
#                           # passano da `typst::comemo` (0.5.1) — vedi nota
# Infrastruttura (già in uso, non bloccate con `=`):
serde        = { version = "1", features = ["derive"] }  # tipi verso il frontend
serde_json   = "1"          # snippets.json, impostazioni.json, meta.json
dirs         = "6"          # cartelle config/dati per OS (typster/)
ureq         = "2"          # HTTP dell'indice Universe (API diretta, non via typst-kit)
toml         = "0.8"        # lettura di typst.toml (template, @local)
# Introdotti nelle fasi successive:
typst-pdf    = "=0.15.0"    # export PDF (Fase 1)
typst-render = "=0.15.0"    # export PNG / raster (Fase 1/5)
typst-ide    = "=0.15.0"    # autocomplete, hover, jump sync (Fase 2/4)
# Fase 6 (bloccate all'implementazione, 2026-07-16):
typst-syntax = "=0.15.0"    # AST per typst→md — ora dipendenza DIRETTA (prima transitiva)
pulldown-cmark = { version = "=0.13.4", default-features = false }  # md→typst (solo parser)
# Fase 5 (bloccate all'implementazione, 2026-09-29):
# typst-render = "=0.15.0" (sopra) usato davvero da F5-011: export PNG; porta pixglyph
chrono = { version = "=0.4.45", default-features = false, features = ["clock", "std"] }
                            # timestamp UTC dell'export, richiesto da PDF/A (F5-011);
                            # già transitiva via typst-kit
sha2   = "=0.10.9"          # chiave file→cronologia, SHA-256 del percorso (F5-013, D-012);
                            # anche registro SHA-256 delle scritture proprie (F5-022);
                            # già transitiva
notify = "=8.2.0"           # watcher unico ricorsivo sulla radice, NEL CORE
                            # (sorveglianza.rs; F5-022, D-017). Porta notify-types;
                            # inotify/kqueue/fsevent solo su non-Windows.
                            # NIENTE feature "watcher" di typst-kit (superata da D-017)
# Fase 7 (scelte con D-016, fase8.md §10; bloccare con `=` all'implementazione):
# grep   = "=0.4.1"           # ricerca nel progetto (F7-003); in alternativa i due
#                             # sotto-crate: grep-regex = "=0.1.14" + grep-searcher = "=0.1.17"
# ignore = "=0.4.33"          # visita della cartella (esclusioni, .gitignore, max_filesize)
```
> Nota (D-016, 2026-09-28): `typst-render = "=0.15.0"` basta per l'export PNG
> di F5-011 — `Pixmap::encode_png()` è un metodo del tipo restituito da
> `typst_render::render`, quindi **nessuna** dipendenza diretta da `tiny-skia`
> (fase8.md §3.3). Nessun crate `semver` per F5-012: `PackageVersion` di Typst
> è già `Ord` (fase8.md §5.1).
> Nota (verificato in ricostruzione core, DECISIONI.md D-008): la cache
> incrementale che conta è quella di `typst::comemo` (**comemo 0.5.1**
> ri-esportato da `typst` 0.15, che vi traccia il proprio World). Tutti gli
> `evict` del core passano dal ri-export; il pin diretto `comemo = "0.4"`
> risolve a un'istanza separata e resta solo come retaggio di D-002.
> **Rimosso il 2026-09-30** (audit completo): non era usato.
> Nota (verificato in Fase 0): i nomi reali delle feature `typst-kit` 0.15 sono
> `embedded-fonts`, `scan-fonts`, `system-packages`, `universe-packages`,
> `system-downloader`, `datetime`. Il download/estrazione tar.gz è fornito da
> `universe-packages` + `system-downloader` (ureq), non da crate separati.

> Nota (Fase 6, aggiornata 2026-07-16): dipendenze **bloccate** all'atto
> dell'implementazione — `pulldown-cmark = "=0.13.4"` con
> `default-features = false` (md→typst, solo parser: il renderer HTML non
> serve); `typst-syntax = "=0.15.0"` resa dipendenza **DIRETTA** del core
> (prima solo transitiva via lo stack Typst) per usare `typst_syntax::{parse,
> ast}` nel verso typst→md; `@codemirror/lang-markdown` ^6 (6.5.1) lato
> frontend. Vedi DECISIONI.md F6-001/F6-003.

### Guscio — `src-tauri`
```toml
tauri               = "2"
tauri-plugin-dialog = "2"   # dialog apri/salva (Fase 1)
serde               = { version = "1", features = ["derive"] }
# tauri-plugin-shell = "2"  # NON nello stack di base (D-010): Tinymist non è un
#                           # sidecar in bundle; per X-001 si valuta l'avvio del
#                           # processo dal guscio Rust, vedi §4 "Estensioni opzionali"
typst-core          = { path = "../crates/typst-core" }
# stato condiviso: std::sync::Mutex/RwLock; async: tokio (debounce con AbortHandle)
# Fase 5: NESSUN notify nel guscio (D-017, 2026-09-29) — il watcher unico vive nel
#         core (typst-core, notify = "=8.2.0"); il guscio ne inoltra solo gli eventi.
#         NESSUN crate debouncer: batching nel core (Batcher 100/500 ms).
```

### Frontend (npm)
```jsonc
// dependencies (package.json reale, 2026-09-30):
"@tauri-apps/api": "^2",
"@tauri-apps/plugin-dialog": "^2.7.1", // dialog apri/salva, lato frontend di tauri-plugin-dialog
"codemirror": "^6.0.2",               // basicSetup (porta language/commands in modo transitivo)
"@codemirror/state": "^6.7.0",
"@codemirror/view": "^6",             // usato direttamente ma non dichiarato: aggiunto il 2026-09-30 (audit)
"@codemirror/autocomplete": "^6.20.3",
"@codemirror/lint": "^6.9.7",         // diagnostica inline (F1-007)
"@codemirror/theme-one-dark": "^6.1.3", // tema scuro
"codemirror-lang-typst": "^0.4.0",    // in uso con import dinamico + fallback — vedi Rischi §7
"@codemirror/lang-markdown": "^6.5.1", // evidenziazione .md — ufficiale CodeMirror (Fase 6, bloccata 2026-07-16)
// devDependencies:
"@tauri-apps/cli": "^2", "vite": "^5", "typescript": "^5",
"vite-plugin-wasm": "^3.6.0", "vite-plugin-top-level-await": "^1.6.0", // WASM di codemirror-lang-typst
// Solo se attuata X-001 (Tinymist opzionale, D-010), oggi non installato:
// "@codemirror/lsp-client": "^6"
// Fase 7: "@floating-ui/dom" — base comune menu/popup (F7-002, D-014); versione da bloccare all'implementazione
```

### Tinymist — extra opzionale, fuori dallo stack di base (D-010)
~~Binario precompilato dalle GitHub Releases, posto in `src-tauri/binaries/` con
suffisso target-triple.~~ **Superato (2026-09-28, D-010):** Tinymist **non** si
distribuisce con l'app né entra nel bundle/installer, e non è una dipendenza
bloccata del progetto. È un componente che l'utente installa per conto proprio
e indica all'app (X-001). Resta valido il vincolo di compatibilità: la versione
usata deve essere **allineata** al livello di linguaggio dei crate embeddati
(`=0.15.0`, §7); l'app la verifica al caricamento, non la impone.

---

## 4. Roadmap a fasi con Definition of Done

Le fasi seguono §10 di [PROGETTO.md](PROGETTO.md), arricchite di dettaglio
implementativo. Ogni fase si chiude solo quando la DoD è verificata.

### Fase 0 — Decisione tecnica e walking skeleton
**Obiettivo:** apri `.typ` → compila → mostra anteprima. Provare l'intera catena.
- Scaffold workspace: `cargo create-tauri-app` (template Vanilla TS + Vite),
  riorganizzato nello workspace con il crate `typst-core` separato.
- `rust-toolchain.toml` con toolchain bloccata; `Cargo.lock` versionato.
- `typst-core`: `World` minimale via `typst-kit` (font `embedded()` + `system()`,
  `SystemPackages` per la cache standard), `compila(sorgente) -> PagedDocument`,
  `pagina_svg(doc, i) -> String` via `typst-svg`.
- Comando Tauri `compila(sorgente: String) -> Vec<String>` (un SVG per pagina).
- Frontend: `<textarea>` (CodeMirror arriva in Fase 1) + render degli SVG.
- `DECISIONI.md` creato con la scelta di stack e le versioni bloccate.

**DoD:** apertura di un `.typ` reale che usa un pacchetto `@preview` → anteprima
mostrata; build che gira su Windows (piattaforma di sviluppo).

### Fase 1 — Editor base usabile
**Obiettivo:** scrittura con anteprima dal vivo, salvataggio, export PDF.
- CodeMirror 6 con `codemirror-lang-typst` (evidenziazione), line numbers,
  auto-indent, chiusura parentesi, tema chiaro/scuro.
- Anteprima dal vivo: debounce nel backend (`tokio` + `AbortHandle`, 150–300 ms),
  `comemo::evict(30)` dopo ogni compilazione, streaming SVG via `Channel<String>`,
  render della **sola pagina visibile**.
- `EditorWorld` **persistente** come `tauri::State<Mutex<…>>`: tra una battitura e
  l'altra aggiornare solo il sorgente (`aggiorna_sorgente`, già predisposta in
  Fase 0) senza riscansionare i font né ricreare il `World` — altrimenti `comemo`
  non dà alcun beneficio incrementale. *(Carry-over dalla revisione Fase 0.)*
- Apertura/salvataggio file singolo (modalità "documento singolo").
- Esportazione PDF via `typst-pdf` (`pdf(&doc, &PdfOptions)`).
- Diagnostica di compilazione: errori/avvisi del compilatore mostrati sulla riga.
- Affidabilità: su errore di compilazione l'**ultima anteprima valida resta
  visibile** (l'editor non si blocca).

**DoD:** *time-to-first-PDF* < 2 min partendo da sorgente vuoto; anteprima
aggiornata < 300 ms su documento tipico; export PDF identico al compilatore CLI.

**Stato: COMPLETATA (2026-06-28)** — verificata a build + test + clippy + tsc/Vite
(conferma a vista in sessione interattiva pendente). Dettaglio in DECISIONI.md
F1-001…F1-007. *Scostamenti dal piano (consapevoli):* debounce nel **frontend**
(non backend), render di **tutte** le pagine (lo streaming `Channel` e la sola
pagina visibile restano refinement F1-R2), evidenziazione Typst come enhancement
**opzionale** (WASM sperimentale con fallback). Refinement non bloccanti aperti:
F1-R1 (scan font all'avvio), F1-R2 (streaming `Channel`).

### Fase 2 — Assistenza alla scrittura
> **Stato: COMPLETATA (2026-06-28)** — DoD raggiunta con `typst-ide` embeddato
> (DECISIONI.md "Fase 2 — stato DoD": F2-001 toolbar, F2-002 completamento +
> hover, F2-003 snippet). Il **sidecar Tinymist**, qui sotto ancora elencato come
> voce pendente, è **tolto dalla fase e dallo stack di base** (2026-09-28,
> DECISIONI.md D-010): diventa l'estensione opzionale **X-001** (§4 "Estensioni
> opzionali"). Le voci barrate sotto restano come traccia storica.

**Obiettivo:** completamento, diagnostica ricca, toolbar, snippet.
- ~~Sidecar Tinymist (`tauri-plugin-shell`, `externalBin`, capability
  `shell:allow-execute` con `sidecar:true`).~~ → superato da D-010: niente
  sidecar in bundle; caricamento opzionale in X-001.
- ~~`lsp_proxy.rs`: proxy puro JSON-RPC stdio (preserva il framing
  `Content-Length`); frontend→Tinymist via `invoke`, Tinymist→frontend via
  `Channel<String>`.~~ → spostato in X-001 (stesso schema di proxy).
- ~~Client LSP in CodeMirror (`@codemirror/lsp-client`) con `Transport` custom su
  Tauri: completion, hover, diagnostics, formatting, inlay hints, rename.~~ →
  spostato in X-001.
- Toolbar formattazione (grassetto, corsivo, titoli, liste, tabelle, immagini)
  che inserisce il markup corretto.
- Snippet riutilizzabili/personalizzabili (file esterni versionati, da `config.rs`).
- ~~*Fallback*~~ **Via ufficiale** (D-010): `typst-ide::autocomplete`/`tooltip`
  embeddati nel core (F2-002); nessun sidecar richiesto.

**DoD:** completamento di funzioni/parametri e hover funzionanti nell'editor;
toolbar che produce markup compilabile; uno snippet personalizzato inseribile.

### Fase 3 — Ecosistema integrato (cuore del progetto)
**Obiettivo:** scoprire e installare pacchetti/template di Typst Universe.
- Modulo `packages.rs`: fetch di `https://packages.typst.org/preview/index.json`
  (cache con TTL 1–24 h), parsing dell'array, ricerca e filtri per `categories`.
- Dettaglio pacchetto: README recuperato dal `repository` o dal tar.gz.
- Installazione trasparente: download
  `https://packages.typst.org/preview/{name}-{version}.tar.gz`, estrazione nella
  **cache standard `@preview`** (compatibile col compilatore esterno), inserimento
  automatico dell'`#import "@preview/{name}:{version}"`.
- "Nuovo da template": `typst init`-like — scarica il tar, legge `typst.toml`,
  copia `[template].path` nella cartella di lavoro, apre l'`[template].entrypoint`.

**DoD:** cerca un pacchetto, installalo, l'import compare e compila; crea un
progetto nuovo da un template della rete e compilalo.

**Stato: COMPLETATA (2026-06-29)** — confermata a vista con rete reale (F3-001
browser+install, F3-002 "Nuovo da template" con risoluzione file di progetto via
`FsRoot`). Dettaglio in DECISIONI.md.

### Fase 4 — Progetti e produttività
> **Stato: COMPLETATA (2026-06-29)** — DoD confermata a vista: apertura cartella-progetto
> + risorse/font (F4-001/F4-002), sync sorgente↔anteprima (F4-003), pacchetti `@local`
> (F4-004). Refinement aperti (non bloccanti): salvataggio automatico + cronologia,
> uscita modalità progetto ("Nuovo documento"). Dettaglio in DECISIONI.md.
**Obiettivo:** multi-file, risorse, sync, cronologia, `@local`.
- Apertura cartella-progetto multi-file (`include`/`import`); gestione risorse
  (immagini, dati `.csv/.json/.yaml`, bibliografie `.bib/.yaml`).
- Sincronizzazione sorgente↔anteprima via `typst-ide`: `jump_from_cursor`
  (editor→pagina) e `jump_from_click` (clic sull'anteprima→cursore).
- Salvataggio automatico opzionale e cronologia versioni locale.
- Pacchetti `@local`: pubblicazione di stili ricorrenti nella data dir standard,
  con `typst.toml` generato.

**DoD:** progetto multi-file compila; clic sull'anteprima sposta il cursore e
viceversa; un pacchetto `@local` creato dall'app è importabile.

### Fase 5 — Rifinitura
> **Stato: IN CHIUSURA (2026-09-30; pianificata 2026-09-28)** — task F5-001…F5-024 qui sotto; nome del
> prodotto **Typster** (DECISIONI.md D-009, punti in sospeso decisi il
> 2026-09-28: identifier `it.nexodux.typster`, cartelle dati `…/typster/`,
> migrazione di `snippets.json` per copia). F5-002 fatta (D-010: Tinymist
> extra caricabile, X-001). Decisi inoltre PDF/A in F5-011 (D-011: in UI
> A-2b, A-3b, A-4), cronologia nella cartella dati dell'app in F5-013
> (D-012: chiave `sha2`), `panic = "abort"` in F5-015 (D-013). Proposte
> P5-A…P5-I **accettate** (D-014): P5-C, P5-D, P5-G entrano in Fase 5
> (F5-019…F5-023), le altre nella nuova **Fase 7 — Estensioni**. Vedi
> DECISIONI.md "Fase 5 — pianificazione". *Aggiornamento D-016
> (2026-09-28):* accettate le 10 proposte dell'analista sul catalogo di
> riuso [`fase8.md`](fase8.md) (riflesse in F5-006, F5-008, F5-009, F5-011,
> F5-012, F5-020, F5-022, F5-023 e nelle task F7 collegate); nuova task
> **F5-024** (licenze del codice derivato).
> Avanzamento 2026-09-30: implementate tutte le task F5-001…F5-024 (F5-002
> documentale); verifiche automatiche verdi; aperti per la chiusura: smoke
> dell'utente sull'exe di produzione, veraPDF (F5-011), smoke macOS/Linux
> (F5-017). **Stato: IN CHIUSURA.** Misure e stato DoD in DECISIONI.md
> F5-018; decisioni del 2026-09-30: D-018…D-025; audit completo del
> 2026-09-30 corretto.
>
> **Codice di terze parti:** prima di implementare una task, consultare la
> sezione di [`fase8.md`](fase8.md) citata nella task. Ogni blocco copiato
> porta il commento di derivazione e rientra nel `NOTICE` (F5-024).

**Obiettivo:** temi, palette, prestazioni, distribuzione; barra laterale
(indice, file, pacchetti), controllo dei file modificati da fuori, modalità
focus/macchina da scrivere.
- Palette comandi (`Ctrl/Cmd+P`) ricercabile per ogni azione; temi chiaro/scuro;
  dimensione font regolabile.
- Prestazioni: render sola pagina visibile, dedup delle definizioni glifo SVG,
  debounce affinato.
- Notifica aggiornamenti pacchetti (confronto SemVer con l'indice Universe).
- Pacchettizzazione Tauri per Windows/macOS/Linux; ottimizzazioni Cargo
  (`lto=true`, `codegen-units=1`, `opt-level="s"`, `strip=true`).
- Barra laterale con indice del documento, albero dei file del progetto e
  browser pacchetti; controllo dei file modificati da fuori (`notify`);
  modalità focus e macchina da scrivere (D-014).

#### Task della Fase 5

Formato: obiettivo · dove si tocca · dipendenze nuove · verifica. Ogni task è un
incremento verificabile a sé (stile D-004/D-005); la chiusura di ciascuna si
registra in DECISIONI.md come `F5-xxx`, con build + test + clippy + `tsc`/Vite
e, per le task con effetti visibili, **exe di produzione ricostruito + smoke
test interattivo** (dev ≠ prod su CSP/asset, cfr. B-005).

**Blocco A — Identità e contratto**

- **F5-001 — Rinomina del prodotto in Typster.** *(D-009, punti 1 e 2 decisi
  il 2026-09-28; origine: richiesta utente 2026-09-28)*
  - *Obiettivo:* nome visibile, metadati, identifier e cartelle dati del
    prodotto = "Typster" / `typster`.
  - *Dove — nome e metadati:* `package.json` (`"name": "editor-typst"` →
    `"typster"`; `package-lock.json` si allinea con `npm install`);
    `src-tauri/tauri.conf.json` (`productName` "Editor Typst" → "Typster";
    titolo finestra `app.windows[0].title`; **`identifier`
    `it.nexodux.editor-typst` → `it.nexodux.typster`**, D-009 punto 1);
    `src-tauri/Cargo.toml` (package/bin `editor-tauri` → `typster`, lib
    `editor_tauri_lib` → `typster_lib`) + `src-tauri/src/main.rs`
    (`editor_tauri_lib::run()`); `Cargo.toml` radice (commento
    `# Workspace Cargo — Editor Typst`); `index.html` (`<title>` e `<h1>`);
    `src/main.ts` (`document.title = \`Editor Typst — …\``);
    `src-tauri/capabilities/default.json` (descrizione; `gen/schemas` si
    rigenera da sé); `crates/typst-core/src/lib.rs` (`AGENTE_UTENTE`
    `"editor-typst/0.1"` → `"typster/0.1"`: è lo User-Agent HTTP verso Typst
    Universe, quindi identità del prodotto — rientra nella rinomina).
  - *Dove — cartelle dati del core (D-009 punto 2):* `…/editor-typst/` →
    `…/typster/` in tre punti verificati: `crates/typst-core/src/config.rs`
    (`carica_snippet()`, `base.join("editor-typst")` su `dirs::config_dir()`,
    più i commenti di modulo e di `NOME_FILE`) e
    `crates/typst-core/src/packages.rs` (`percorso_cache_indice()`,
    `base.join("editor-typst")` su `dirs::cache_dir()`). *Proposta:* un'unica
    costante del core per il nome cartella (sul modello di `AGENTE_UTENTE`,
    centralizzato per non avere valori incoerenti), riusata anche dalle
    impostazioni di F5-003 e dalla cronologia di F5-013.
  - *Migrazione una tantum — **decisa** (2026-09-28, D-009 punto 2):* per non
    perdere dati dell'utente, all'avvio, **prima** che `carica_snippet()` crei
    i predefiniti nella cartella nuova (altrimenti la condizione "nuova
    assente" non si verificherebbe più): se
    `config_dir()/typster/snippets.json` manca e
    `config_dir()/editor-typst/snippets.json` esiste, lo si **copia**. La
    cartella vecchia `editor-typst` resta **intatta** (nessuno spostamento,
    nessuna cancellazione, nessuna pulizia) e **nessun avviso** in UI: l'esito
    è solo loggato. La cache dell'indice
    (`cache_dir()/editor-typst/universe-index.json`) **non** si migra: si
    rigenera al primo accesso a Universe (la vecchia resta orfana, non
    toccata). Logica pura nel core, degrado silenzioso su errore di I/O (si
    parte dai predefiniti, come oggi).
  - *Identifier (D-009 punto 1):* cambia la cartella dati del webview
    (profilo WebView2 sotto `%LOCALAPPDATA%\<identifier>` su Windows) e il
    bundle id macOS. Costo oggi ~nullo (nessun `localStorage`/IndexedDB,
    nessun path API di Tauri): nessuna migrazione prevista per questa parte.
  - Il crate `typst-core` NON si rinomina (nome tecnico, non di prodotto); la
    cartella del repo resta `editor-typst/`.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* `grep -ri "editor typst\|editor-typst\|editor-tauri\|editor_tauri"`
    sui sorgenti (esclusi `target/`, `dist/`, `node_modules/`, documenti
    storici; l'unica occorrenza ammessa di `editor-typst` è il percorso
    *vecchio* usato dalla migrazione) vuoto; build + `tsc`/Vite verdi; test
    core della migrazione su cartelle temporanee (vecchia presente/nuova
    assente → copiato; nuova già presente → non toccata; vecchia assente →
    predefiniti; errore di I/O → predefiniti senza panic); exe di produzione
    con titolo finestra e nome "Typster"; smoke: lo `snippets.json`
    personalizzato dell'utente compare in `%APPDATA%\typster\` e i suoi
    snippet sono offerti nell'editor.
  - *Nota ambiente:* dopo la rinomina l'exe diventa `typster.exe` (aggiornare
    la ricetta build in B-004 / memoria di sviluppo).

- **F5-002 — Riallineamento Tinymist sì/no (documentale + decisione).**
  *(lacuna accertata: D-001 vs codice)* — **FATTA (2026-09-28, solo
  documenti).** *Esito deciso dall'utente:* **Tinymist fuori dal programma,
  come extra caricabile** (DECISIONI.md **D-010**, che supera D-001 per la
  parte Tinymist; chiusura registrata come F5-002 in DECISIONI.md). Né la (a)
  né la (c) pure: l'assistenza integrata resta `typst-ide` embeddato; Tinymist
  esce da stack di base, bundle e installer e diventa l'estensione opzionale
  **X-001** (§4 "Estensioni opzionali"), che non condiziona la DoD della
  Fase 5. Riallineamento eseguito in: DECISIONI.md (D-010, nota in D-001),
  PIANO.md §0, §1, §2, §3, Fase 2, §5, §6, §7, DoD Fase 5; PROGETTO.md §4.1,
  §5, §6 (Opzione A), §10 Fase 2 (note brevi). Il codice non cambia:
  `src/assistenza.ts` ("nessun sidecar") è già coerente. *Testo originale
  della task, per traccia:*
  - *Obiettivo:* rendere coerenti contratto e codice. D-001 e §0/§1/§3 di
    questo piano dicono che l'assistenza passa da **Tinymist sidecar**; il
    codice (`src/assistenza.ts`, intestazione: "usano il crate `typst-ide`
    embeddato (nessun sidecar)"; F2-002) usa solo `typst-ide`, e
    `src-tauri/Cargo.toml` non ha `tauri-plugin-shell` né esiste
    `src-tauri/binaries/`.
  - *Dove:* solo documenti — DECISIONI.md (nuova D-xxx che aggiorna D-001),
    PIANO.md §0, §1 (diagramma), §2, §3 (`tauri-plugin-shell`,
    `@codemirror/lsp-client`, sezione Tinymist), §7 (rischi Tinymist),
    DoD Fase 5 ("+ sidecar"); PROGETTO.md §10 Fase 2.
  - *Opzioni (da decidere all'utente):* (a) **abbandonare** Tinymist: `typst-ide`
    resta la via ufficiale, si rimuovono dal contratto sidecar e dipendenze
    relative (coerente con leggerezza §9); (b) **mantenerlo** come enhancement
    futuro esplicitamente fuori dalla Fase 5; (c) **integrarlo** in Fase 5
    (costo alto: allineamento versioni §7, transport LSP non collaudato;
    passare prima da `research-scout`).
  - *Dipendenze nuove:* nessuna (solo in caso (c): `tauri-plugin-shell`,
    `@codemirror/lsp-client`, già elencate in §3).
  - *Verifica:* nessuna contraddizione residua fra DECISIONI.md, PIANO.md,
    PROGETTO.md e l'intestazione di `src/assistenza.ts`.

**Blocco B — Impostazioni, aspetto, palette**

- **F5-003 — Impostazioni utente persistenti (file esterno).**
  - *Obiettivo:* un file `impostazioni.json` (tema, dimensione font, debounce,
    auto-save, …) accanto a `snippets.json`, letto all'avvio, scritto alle
    modifiche. Base per F5-004/005/010/013/019/023 e per la lingua di F7-004.
    **Proposta:** file nella config dir
    via `config.rs` (principio "config esterni") e **non** `localStorage`
    (legato all'`identifier`, non ispezionabile — oggi `src/` e `index.html` non
    usano `localStorage`/`sessionStorage`/IndexedDB).
  - *Dove:* core `crates/typst-core/src/config.rs` (struct `Impostazioni` con
    default, parsing tollerante e scrittura atomica sul modello di
    `carica_snippet`); guscio `src-tauri/src/lib.rs` (comandi
    `leggi_impostazioni`/`salva_impostazioni`); frontend nuovo
    `src/impostazioni.ts`.
  - *Dipendenze nuove:* nessuna (`serde_json`, `dirs` già presenti).
  - *Verifica:* test core (default su file mancante/JSON invalido, voce
    sconosciuta ignorata, round-trip); valore modificato sopravvive al riavvio.
  - *Prerequisito:* D-009 punto 2 — **deciso** (2026-09-28): il file nasce
    direttamente in `config_dir()/typster/`; eseguire dopo F5-001 così non
    serve migrarlo.

- **F5-004 — Tema chiaro/scuro.**
  - *Obiettivo:* commutazione tema dell'editor e della UI, persistita; opzione
    "segui il sistema" (`prefers-color-scheme`).
  - *Dove:* `src/editor.ts` (oggi `oneDark` fisso nelle estensioni, riga ~117 →
    `Compartment` del tema, sul modello di quello del linguaggio);
    `index.html` (stili della UI → variabili CSS per i due temi); le pagine
    SVG dell'anteprima restano su sfondo carta.
  - *Dipendenze nuove:* nessuna (tema chiaro = default di CodeMirror; oneDark
    già presente).
  - *Verifica:* commutazione a caldo senza perdere contenuto/cursore; smoke di
    **produzione** (gli stili CodeMirror iniettati a runtime dipendono da
    `style-src`, B-005).

- **F5-005 — Dimensione font regolabile.**
  - *Obiettivo:* font dell'editor regolabile (Ctrl/Cmd `+`/`-`/`0`), persistito;
    eventuale zoom dell'anteprima come voce separata.
  - *Dove:* `index.html` (`.cm-editor { font-size: 0.9rem }` → variabile CSS),
    `src/editor.ts` (`requestMeasure` dopo il cambio), `src/main.ts`
    (scorciatoie), `src/impostazioni.ts`.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* cambio immediato, gutter allineato, valore ripristinato al
    riavvio.

- **F5-006 — Palette comandi (`Ctrl/Cmd+P`) su registro di comandi.**
  - *Obiettivo:* ogni azione dell'app eseguibile da una palette con ricerca
    fuzzy, scorciatoia mostrata accanto. Architettura a **registry**: un
    registro unico `{id, titolo, scorciatoia, esegui, abilitato?}` alimenta
    palette, scorciatoie da tastiera e (in prospettiva) i pulsanti.
  - *Dove:* frontend nuovi `src/comandi.ts` (registro) e `src/palette.ts` (UI,
    modale accessibile, stesso stile di `src/local.ts`); `src/main.ts` (oggi le
    scorciatoie Ctrl+N/O/S/E e Ctrl+Shift+O sono in uno `switch` su `keydown`
    → migrano sul registro); `index.html` (markup modale). Gestire **`metaKey`
    su macOS** (oggi si guarda solo `ctrlKey`) e il `preventDefault` di Ctrl+P
    (stampa della webview).
  - *Compatibilità con la Fase 7 (D-014) — da rispettare già qui:* il
    registro va progettato perché vi si innestino senza rifattorizzazione
    (a) i **sottomenu** di F7-001 (es. "Esporta ▸ PDF/PNG/SVG", "Tema ▸
    chiaro/scuro"): identificatori gerarchici (`esporta.pdf`) e un campo
    opzionale per i figli/sottocomandi, anche calcolati in modo asincrono;
    (b) i **titoli tradotti** di F7-004: il titolo di un comando è una chiave
    (o una funzione valutata al momento della visualizzazione), non una
    stringa fissata alla registrazione; (c) i comandi registrati dalle task
    successive (barra laterale F5-019, focus/macchina da scrivere F5-023).
    In Fase 5 la palette resta **piatta**.
  - *Forma del registro — **decisa** (D-016, fase8.md §9):* modellata sul
    tipo `CommandDescriptor`/`CommandSubcommand` di MarkText (MIT, commit
    `34b5c28`; copiabile il solo tipo, con avviso MIT — F5-024):
    `{id, titolo (chiave t()), scorciatoia[], sottocomandi?, esegui?,
    eseguiSottocomando?}` più i metodi **asincroni** opzionali `run()`
    (carica i sottocomandi prima di mostrarli), `unload()` e `search(q)`
    (ricerca delegata al comando). Id gerarchici (`esporta.pdf`). La UI di
    MarkText (Vue) non si copia.
  - *Dipendenze nuove:* nessuna (fuzzy match minimale scritto in casa).
  - *Verifica:* tutte le azioni dell'header/toolbar raggiungibili dalla
    palette; navigazione solo tastiera; Esc chiude e restituisce il focus.

**Blocco C — Prestazioni**

- **F5-007 — Scan font fuori dall'avvio.** *(origine: F1-R1, DECISIONI.md)*
  - *Obiettivo:* la finestra compare senza attendere la scansione dei font di
    sistema.
  - *Dove:* guscio `src-tauri/src/lib.rs` (`run()`: oggi
    `.manage(Arc::new(Mutex::new(EditorWorld::nuovo(""))))` prima di mostrare
    la finestra → costruzione in `setup`/thread o init lazy); core
    `crates/typst-core/src/world.rs` se serve un costruttore a due tempi.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* tempo "avvio → finestra visibile" misurato prima/dopo sull'exe
    di produzione; la prima compilazione attende correttamente i font.

- **F5-008 — Render della sola pagina visibile + streaming.** *(origine:
  F1-R2 e scostamento Fase 1, "render di tutte le pagine")*
  - *Obiettivo:* su documenti lunghi si rasterizza/serializza in SVG solo ciò
    che è in vista (± 1 pagina); le altre sono segnaposto delle dimensioni
    corrette.
  - *Dove:* core `world.rs` (`compila` oggi genera un SVG per **ogni** pagina
    via `typst_svg::svg` → separare "compila + metadati pagine" da "SVG della
    pagina i" sull'`ultimo_documento` già in cache); guscio `lib.rs` (nuovo
    comando `svg_pagina` o `Channel<String>`; il contratto di
    `RisultatoCompilazione` cambia → aggiornare `src/main.ts`); frontend
    `src/main.ts` (IntersectionObserver) e `src/sincronizzazione.ts`
    (`.pagina-svg[data-pagina]`: i marcatori devono funzionare anche su pagine
    caricate in ritardo).
  - *Contratto core → frontend — **deciso** (D-016, fase8.md §4.1):*
    `compila` restituisce **metadati per pagina** (numero pagine, dimensioni,
    `hash128` di `typst::utils::hash128(page)`) e **nessun SVG**; l'SVG di
    una pagina si ottiene **solo a richiesta**. Il frontend ri-chiede l'SVG
    di una pagina solo se il suo hash è cambiato o se entra nel viewport.
    Da copiare: `ExportCache` di typst-cli v0.15.0 `compile.rs` 647-672
    (Apache-2.0, F5-024); `Mutex<Vec<u128>>` di std basta.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* documento di prova ≥ 50 pagine: tempo per battitura misurato
    prima/dopo; sync click↔cursore (F4-003) ancora corretta; ultima anteprima
    valida conservata su errore.

- **F5-009 — Dedup delle definizioni glifo SVG.**
  - *Obiettivo:* ridurre peso e tempo di parsing del DOM dell'anteprima evitando
    di ripetere le `<defs>` dei glifi pagina per pagina.
  - *Valutazione — **chiusa** (D-016, fase8.md §4.2):* **dedup a valle**
    per id glifo stabili; `svg_merged` **scartato** (un unico SVG verticale
    da ricostruire a ogni pagina, incompatibile con F5-008). Fatto verificato
    su `typst-svg` 0.15.0: gli id dei glifi sono `g<hash128(font, glyph_id,
    scala)>`, **stabili fra pagine e fra compilazioni**. Dedup: contenitore
    condiviso `<svg style="display:none"><defs>…</defs></svg>` nel DOM e, a
    ogni pagina inserita, scarto di `<symbol>`/`<clipPath>`/gradienti con id
    già presente (`Set<string>`); in alternativa lo stesso filtro nel core
    sul testo SVG prima dell'invio. Ricognizione `research-scout` non più
    necessaria.
  - *Dove:* frontend (inserimento pagine, `src/main.ts`) oppure core
    `world.rs` (filtro sul testo SVG) — **interagisce con F5-008**.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* byte SVG totali e tempo di aggiornamento misurati prima/dopo su
    un documento reale; resa visiva invariata.
  - *Ordine:* dopo F5-008.

- **F5-010 — Debounce affinato.**
  - *Obiettivo:* sostituire il debounce fisso di 300 ms (`src/main.ts`,
    `timerDebounce`) con uno adattivo sulla durata dell'ultima compilazione
    (es. limiti 150–600 ms), mantenendo il coalescing `inCorso`/
    `richiestaPendente`; valore base configurabile (F5-003).
  - *Dove:* frontend `src/main.ts`.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* DoD Fase 1 ancora rispettata (< 300 ms su documento tipico);
    su documento pesante nessuna coda di compilazioni.

**Blocco D — Funzionalità mancanti rispetto al contratto**

- **F5-011 — Export PNG e SVG con scelta pagine/DPI + PDF/A.** *(lacuna
  accertata: PROGETTO.md §4.4 promette "PDF, PNG, SVG con scelta di pagine, DPI
  e profilo"; il guscio espone solo `esporta_pdf` in `src-tauri/src/lib.rs`;
  profilo PDF/A **deciso** dall'utente il 2026-09-28 → DECISIONI.md D-011)*
  - *Obiettivo:* export PNG (per pagina, DPI scelto) e SVG (per pagina), con
    selezione dell'intervallo di pagine; **export PDF con scelta dello
    standard PDF/A** (e dell'intervallo di pagine) tramite le opzioni di
    `typst-pdf`.
  - *PDF/A — cosa offre `typst-pdf` `=0.15.0` (verificato sul sorgente del
    crate, `src/lib.rs`, 2026-09-28):* `PdfOptions { standards: PdfStandards,
    page_ranges: Option<PageRanges>, tagged, … }`;
    `PdfStandards::new(&[PdfStandard])` valida la combinazione e restituisce
    un errore con suggerimenti se è incompatibile. Varianti PDF/A di
    `PdfStandard` (enum `#[non_exhaustive]`): `A_1b`, `A_1a`, `A_2b`, `A_2u`,
    `A_2a`, `A_3b`, `A_3u`, `A_3a`, `A_4`, `A_4f`, `A_4e` (serde: `a-1b` …
    `a-4e`). **Al massimo un** PDF/A per export ("choose at most one PDF/A
    standard"); una versione PDF esplicita (`V_1_4`…`V_2_0`) si combina solo
    se compatibile. Esistono anche `Ua_1` (PDF/UA-1) e le versioni pure:
    **fuori da questa task** salvo richiesta. Oggi il core esporta con
    `PdfOptions::default()` (`crates/typst-core/src/world.rs`, riga ~327) →
    nessuno standard imposto.
  - *UI — **decisa** (2026-09-28, D-011):* nella modale di export PDF una
    scelta "Standard: nessuno (predefinito) | PDF/A-2b | PDF/A-3b | PDF/A-4";
    ultimo valore ricordato in `impostazioni.json` (F5-003). Le **altre**
    varianti supportate da `typst-pdf` non si mostrano, ma il tipo di opzioni
    del core resta aperto a tutte (mappatura completa verso `PdfStandard`):
    esporle in futuro è una modifica della sola UI, senza toccare il core.
  - *Errori di conformità:* se il documento viola i requisiti dello standard,
    `typst_pdf::pdf` restituisce diagnostica di errore → mostrarla all'utente
    come per gli errori di compilazione (nessun file scritto, nessun panic).
  - *Dove:* core `world.rs` (`esporta_png(pagine, dpi)` via
    `typst_render::render` + codifica PNG; `esporta_svg(pagine)` via
    `typst-svg`; `esporta_pdf` esteso con standard e intervallo pagine, con un
    tipo di opzioni del core che non espone al guscio i tipi `typst-pdf` oltre
    il necessario); guscio `lib.rs` (comandi con dialog cartella/file,
    scrittura con `scrivi_atomico`); frontend `src/fileops.ts` + piccola
    modale opzioni (pagine, DPI, standard PDF).
  - *Regola tag / intervallo — **decisa** (D-016, fase8.md §3.2):*
    `tagged = page_ranges.is_none()` (come typst-cli e Tinymist: con un
    intervallo di pagine la struttura logica sarebbe rotta, quindi niente tag
    di accessibilità). Poiché le varianti `A_*a` (1a/2a/3a) e `Ua_1`
    richiedono i tag, con un intervallo di pagine vanno **rifiutate a monte**
    nel core con un errore leggibile, prima di chiamare `typst_pdf::pdf`.
    Le varianti offerte in UI (nessuno/2b/3b/4) sono compatibili con gli
    intervalli. Nota: `PdfOptions::default()` ha `tagged: true`.
  - *Codice da copiare (D-016, fase8.md §3.1 e §3.3; typst-cli v0.15.0,
    Apache-2.0, F5-024):* `Pages` + `parse_page_number` (`args.rs`
    730-768, intervalli `1-3`, `4`, `5-`, `-2` → `PageRanges`);
    `output_template` (`{p}`, `{0p}`, `{n}`, `{t}`) e `png_options`
    (`pixel_per_pt = ppi / 72`, `ppi` predefinito 144) da `compile.rs`;
    mappatura `From<PdfStandard>`. Tabella informativa "standard ↔ versione
    PDF" per i testi d'aiuto della modale da Typstify (fase8.md §3.4, solo
    contenuto).
  - *Dipendenze nuove:* `typst-render = "=0.15.0"` (già prevista e bloccata in
    §3, "Fase 1/5"; oggi assente da `crates/typst-core/Cargo.toml`). Encoder
    PNG — **verificato** (D-016): `Pixmap::encode_png()` sul valore restituito
    da `typst_render::render`; **nessuna** dipendenza diretta da `tiny-skia`.
  - *Verifica:* test core (intestazione PNG valida, dimensioni coerenti col
    DPI, intervallo pagine rispettato); confronto visivo con `typst compile
    --format png` della CLI 0.15. **PDF/A:** test core che, per ogni variante
    offerta dalla UI, esporta un documento d'esempio senza errori e verifica
    nei metadati XMP del PDF l'identificazione dello standard
    (`pdfaid:part`/`pdfaid:conformance` coerenti con la scelta); test che due
    PDF/A insieme (o PDF/A + versione incompatibile) producono un errore
    leggibile, non un panic; export senza standard invariato rispetto a oggi.
    Gate manuale: validazione di almeno un PDF/A-2b prodotto dall'app con un
    validatore esterno (es. veraPDF) e confronto con `typst compile
    --pdf-standard a-2b` della CLI 0.15; esito registrato in DECISIONI.md.

- **F5-012 — Notifica aggiornamenti pacchetti (SemVer).**
  - *Obiettivo:* segnalare i pacchetti `@preview` installati nella cache
    standard per cui l'indice Universe ha una versione più recente, e (opz.)
    gli `#import "@preview/x:ver"` del documento corrente non aggiornati.
  - *Dove:* core `crates/typst-core/src/packages.rs` (elenco versioni installate
    nella cache `@preview` + confronto con `PackageVersion`, già usato in
    `valida_versione`/`elenca`); guscio `lib.rs` (comando dedicato); frontend
    `src/pacchetti.ts` (badge "aggiornamento disponibile").
  - *Algoritmo — **deciso** (D-016, fase8.md §5):* per ogni
    `<cache>/preview/<nome>/`, versione installata =
    `FsPackages::system_cache()` → `latest_version(&VersionlessPackageSpec)`
    (API di typst-kit 0.15.0, nessuna copia); ultima versione = `max` per
    nome sull'**indice già in cache** del core (`packages.rs::elenca`,
    offline-first — non il downloader di `UniversePackages`); segnala se
    `ultima > installata`. Per gli `#import` del documento: visita di
    `Source::root()` applicando `parse_package_import` di Tinymist
    (`tinymist-query/src/package.rs` 55-74, commit `f3b00eb`, Apache-2.0 —
    F5-024) a ogni nodo `Str`. **Niente crate `semver`**: `PackageVersion` è
    già `Ord`.
  - *Dipendenze nuove:* nessuna (confronto con `PackageVersion` di Typst).
  - *Verifica:* test core offline con cache e indice finti; nessuna rete
    richiesta oltre all'indice già in cache (offline-first).

- **F5-013 — Salvataggio automatico + cronologia versioni locale.**
  *(origine: refinement aperto Fase 4 — DECISIONI.md "Fase 4 — stato DoD";
  PROGETTO.md §4.4)*
  - *Obiettivo:* auto-save opzionale (intervallo/su blur) e snapshot locali
    ripristinabili del documento.
  - *Posizione della cronologia — **decisa** (2026-09-28, DECISIONI.md
    D-012):* nella **cartella dati dell'app**, sotto la cartella `typster`
    rinominata con D-009 punto 2 — proposta
    `dirs::data_dir()/typster/cronologia/` (Windows
    `%APPDATA%\typster\cronologia\`, cioè accanto a `snippets.json`; Linux
    `~/.local/share/typster/`; macOS `~/Library/Application Support/typster/`).
    **Non** in `.typster/` accanto al progetto: le cartelle dei progetti
    restano pulite (solo `.typ` + risorse, §9 "trasparenza dei dati").
    Percorso via `dirs` nel core, non via i path API di Tauri (il core non
    dipende da `tauri`, D-003).
  - *Radice — **confermata** (2026-09-28):* `dirs::data_dir()/typster/cronologia/`.
  - *Associazione file → cronologia — **decisa** (2026-09-28, D-012):*
    chiave = **SHA-256 (crate `sha2`)**, in esadecimale, del percorso
    assoluto canonicalizzato del file (su Windows normalizzato in minuscolo,
    perché il filesystem non distingue le maiuscole) → sottocartella
    `cronologia/<chiave>/` con gli snapshot (`AAAAMMGG-HHMMSS.typ`/`.md`,
    file di testo leggibili) e un `meta.json` con il percorso originale
    (leggibilità, diagnostica e controllo di coerenza: se il percorso in
    `meta.json` non coincide, si usa la chiave successiva). **Non**
    `DefaultHasher` di std (non stabile fra versioni di Rust); scartato
    l'FNV-1a scritto in casa. `sha2` si aggiunge alle dipendenze bloccate
    (§3), versione esatta fissata all'implementazione con `=`.
  - *Rinomina seguita da F5-022 (D-014):* quando Typster segue una rinomina
    fatta da fuori (o una propria "Salva con nome", se si vorrà), la
    cronologia va associata al **nuovo** percorso. *Dettaglio
    implementativo proposto:* **ri-chiavatura** — la sottocartella
    `cronologia/<chiave-vecchia>/` si rinomina in `<chiave-nuova>/` e
    `meta.json` aggiorna il percorso, conservando i precedenti in un campo
    `percorsi_precedenti` (tracciabilità); se `<chiave-nuova>/` esiste già,
    si fondono gli snapshot. Alternativa scartata per semplicità di lettura:
    alias in `meta.json` della chiave vecchia (costringerebbe a cercare gli
    alias a ogni apertura). Conseguenze accettate:
    file rinominato/spostato **senza** che Typster lo segua (app chiusa,
    evento non abbinabile) = nuova cronologia (la vecchia resta finché non
    ruotata); documento mai salvato = nessuna cronologia finché non ha un
    percorso; in modalità progetto ogni file ha la propria cronologia.
    Numero massimo di snapshot per file e rotazione configurabili in
    `impostazioni.json` (F5-003).
  - *Cambio di documento con modifiche non salvate — **D-018**
    (2026-09-30):* domanda "Salva / Non salvare / Annulla" (Salva fallito o
    in conflitto → operazione annullata; Non salvare → copia in cronologia e
    prosegue; documento mai salvato: Salva = Salva con nome) per apri, apri
    cartella, nuovo, apertura da albero/indice, conversione md↔typst e
    chiusura della finestra. **Sostituisce** il "copia in cronologia e
    prosegui" implementato inizialmente.
  - *Dove:* frontend `src/fileops.ts` (stato `modificato`, trigger); guscio
    `lib.rs` (riuso `salva_file`/`scrivi_atomico`); core (nuovo modulo puro per
    la cronologia: chiave, snapshot, rotazione, elenco/ripristino).
  - *Prerequisiti:* F5-001 (cartella `typster`), F5-003 (impostazioni).
  - *Legame con F5-022:* i salvataggi dell'auto-save passano dallo stesso
    registro "scritture proprie" dell'anti-loop di F5-022, così il controllo
    dei file modificati da fuori non li scambia per modifiche esterne.
  - *Dipendenze nuove:* `sha2` (core; versione esatta con `=` da fissare
    all'implementazione, §3).
  - *Verifica:* test core di rotazione/ripristino e della chiave (stesso
    percorso → stessa chiave fra esecuzioni; su Windows maiuscole/minuscole
    → stessa chiave); auto-save non segna conflitti né genera ricompilazioni
    superflue.

- **F5-014 — Edit live dei file fratelli (invalidazione).** *(origine: limite
  noto F3-002, refinement "Fase 4 — stato DoD")*
  - *Obiettivo:* modifiche su disco a file inclusi/importati o risorse di un
    progetto (fatte da un altro programma) riflesse nell'anteprima senza
    riaprire il progetto. Incremento minimo: invalidazione al focus della
    finestra / comando "Ricarica risorse" (comando del guscio
    `invalida_risorse` + ricompila); il
    controllo automatico con `notify` è la task **F5-022**, che riusa questo
    percorso di invalidazione (il comando manuale resta come ripiego).
  - *Dove:* core `world.rs` (`source`/`file` per `VirtualRoot::Project`,
    eventuale cache per mtime; implementato come
    `EditorWorld::invalida_risorse`, che azzera il `FileStore`), guscio
    (comando `invalida_risorse`, `osservatore.rs`); frontend `src/main.ts`.
    `invalida_cache` (`lib.rs`, D-008) resta la sola eviction `comemo`.
  - *Dipendenze nuove:* nessuna per l'incremento minimo.
  - *Verifica:* modifico `capitolo.typ` fuori dall'app → al ritorno
    l'anteprima si aggiorna.

**Blocco E — Distribuzione**

- **F5-015 — Profilo Cargo di release.**
  - *Obiettivo:* binario leggero.
  - *Dove:* `Cargo.toml` radice: scommentare il `[profile.release]` già
    predisposto (`lto = true`, `codegen-units = 1`, `opt-level = "s"`,
    `strip = true`, **`panic = "abort"` — approvato dall'utente il
    2026-09-28, DECISIONI.md D-013**).
  - *Conseguenza annotata di `panic = "abort"`:* in **release** un panic
    termina subito il processo, quindi nessun `Mutex` resta mai "avvelenato"
    in modo osservabile: il **recupero da lock avvelenato di F1-003**
    (`into_inner`) diventa **irraggiungibile** in release. Resta valido e
    attivo in **debug/test** (profili con `unwind`), dove va mantenuto e non
    rimosso. Idem per un eventuale `catch_unwind`: inefficace in release. Un
    panic interno a Typst durante la compilazione chiuderebbe l'app invece
    di lasciarla degradata: rischio accettato (gli errori di compilazione
    ordinari sono `Result`, non panic, e restano gestiti — §9 "affidabilità").
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* dimensione dell'exe prima/dopo registrata in DECISIONI.md;
    smoke test di produzione; tempo di compilazione annotato; `cargo test`
    (profilo di test, `unwind`) ancora verde, inclusi eventuali test del
    recupero da lock avvelenato.

- **F5-016 — Pacchettizzazione Windows/macOS/Linux.**
  - *Obiettivo:* installer per le tre piattaforme.
  - *Dove:* `src-tauri/tauri.conf.json` (`bundle`: oggi `targets: "all"` e solo
    `icons/icon.ico` → servono anche `.icns`/`.png` per macOS/Linux,
    generabili con `tauri icon`; strategia WebView2 su Windows); metadati
    (versione, publisher, copyright); eventuale pipeline CI (una build per OS:
    macOS non si compila da Windows).
  - *Dipendenze nuove:* nessuna crate/npm; strumenti di build per OS.
  - *Licenze (F5-024, D-016):* l'installer include il file `NOTICE` e la
    cartella `licenses/` (risorse del bundle Tauri).
  - *Verifica:* installer prodotto e installato/avviato su ciascuna
    piattaforma (almeno Windows verificato in locale; macOS/Linux su CI o
    macchina dedicata); `NOTICE` e `licenses/` presenti nell'installazione.
  - *Ordine:* dopo F5-001 (nome/identifier definitivi prima del primo
    installer), F5-015 e F5-024 (licenze).

- **F5-017 — Portabilità delle scorciatoie e dei percorsi.**
  - *Obiettivo:* comportamento corretto fuori da Windows: `Cmd` su macOS per
    tutte le scorciatoie (oggi solo `ctrlKey` in `src/main.ts`), percorsi e
    cartelle config/cache per OS (già via `dirs`), dialog e filtri file.
  - *Dove:* `src/main.ts` / registro comandi di F5-006; smoke sui tre OS.
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* checklist di smoke per OS registrata in DECISIONI.md.

- **F5-018 — Verifica requisiti non funzionali e chiusura DoD.**
  - *Obiettivo:* misurare e registrare §9: dimensione installer/binario, tempo
    di avvio, memoria a riposo, latenza anteprima, funzionamento offline.
  - *Dove:* nessun codice; eventuale script di misura in `tools/` (se creato).
  - *Verifica:* tabella di misure in DECISIONI.md "Fase 5 — stato DoD".

**Blocco F — Barra laterale, file esterni, scrittura concentrata** *(proposte
P5-C, P5-D, P5-G accettate il 2026-09-28, DECISIONI.md D-014)*

- **F5-019 — Barra laterale: struttura a pannelli + browser pacchetti.**
  *(P5-C, parte 1; SPEC-UNIFICATA §3.1 "sidebar stile VS Code")*
  - *Obiettivo:* una barra laterale a sinistra con una fila di icone
    (activity bar) che commuta i pannelli; mostra/nascondi, larghezza
    regolabile, pannello attivo e visibilità ricordati in `impostazioni.json`
    (F5-003). Il **browser pacchetti** di F3-001 (oggi pannello a scomparsa,
    `src/pacchetti.ts`) diventa il primo pannello della barra, senza perdere
    funzioni (ricerca, dettaglio, installazione con import).
  - *Architettura a registry:* i pannelli si registrano (`{id, icona, titolo,
    monta, smonta}`) invece di essere cablati nella barra: F5-020/F5-021
    aggiungono indice e file, la **ricerca nel progetto** della Fase 7
    (F7-003) si aggiunge come ulteriore pannello senza toccare la struttura.
    *Scelta motivata:* in Fase 5 **nessun pannello "Ricerca" vuoto o
    segnaposto** visibile (sarebbe interfaccia morta): lo "spazio" per la
    ricerca è il punto di registrazione, che F7-003 usa.
  - *Dove:* frontend nuovi `src/barra-laterale.ts` (contenitore + registro
    pannelli); `src/pacchetti.ts` (montaggio nel pannello invece che nel
    pannello a scomparsa); `index.html` (layout a tre colonne: barra ·
    editor · anteprima, variabili CSS dei temi di F5-004); comandi nel
    registro di F5-006 (mostra/nascondi barra, apri pannello X).
  - *Dipendenze nuove:* nessuna.
  - *Prerequisiti:* F5-003, F5-004 (variabili CSS), F5-006 (registro comandi).
  - *Verifica:* browser pacchetti funzionante dal pannello (cerca, installa,
    import inserito — DoD Fase 3 ancora valida); stato della barra
    ripristinato al riavvio; navigazione da tastiera fra i pannelli; smoke di
    produzione (stili sotto CSP, B-005).

- **F5-020 — Indice del documento (titoli) nella barra laterale.** *(P5-C,
  parte 2)*
  - *Obiettivo — **deciso** (2026-09-28, D-015):* pannello "Indice" con
    l'albero dei titoli di **tutto il documento**, ricavato dal **documento
    compilato**: comprende i titoli di tutti i file richiamati con
    `#include` e quelli generati da codice (`#heading(...)`); clic su una
    voce → apre il **file giusto** e porta il cursore **al punto giusto**
    (e, tramite la sync F4-003, l'anteprima sulla pagina del titolo); voce
    del titolo corrente evidenziata. Vale anche per i `.md` (compilati via
    md→typst: il punto nel sorgente `.md` si ottiene come per la sync del
    Markdown, da verificare nell'implementazione).
  - *Come si ottengono i titoli (verificato sul sorgente dei crate 0.15,
    2026-09-28):* il core conserva l'ultimo documento compilato con successo
    in `EditorWorld::ultimo_documento: Option<PagedDocument>`
    (`crates/typst-core/src/world.rs`, riga ~183, aggiornato alle righe
    ~304/~333, già usato da `jump_da_click`/`jump_da_cursore`).
    `PagedDocument::introspector()` (`typst-layout` 0.15, `document.rs`
    riga ~43) dà l'introspettore; il trait `Introspector` (`typst-library`
    0.15, `introspection/introspector.rs`) offre `query(&Selector)` →
    elementi `HeadingElem` nell'ordine del documento, e per ciascuno
    `page(location)`/`position(location)`. Da ogni elemento: livello
    (`resolve_level`/`depth`, `model/heading.rs`), testo
    (`Content::plain_text`), `Content::span()` → file e posizione nel
    sorgente tramite il `World` (stessa mappatura span→sorgente della sync
    F4-003). Titoli con `outlined: false` esclusi (come fa `#outline`).
    Nessuna dipendenza nuova.
  - *Costruzione — **decisa** (D-016, fase8.md §6):* albero con
    `typst_library::model::OutlineNode::build_tree` (pubblico in 0.15.0:
    gestisce livelli saltati e antenati esclusi), alimentato da
    `(heading, resolve_level, outlined)` — come `typst-pdf` 0.15.0
    `outline.rs` 11-39, ma con **`outlined`** al posto di `bookmarked`.
    Titolo = `body.plain_text().trim()`; **span con ripiego**: se lo span
    dell'elemento è *detached* si usa `body.span()` (Tinymist
    `typst-preview/src/outline.rs`, commit `f3b00eb`); pagina/punto da
    `introspector.position(loc)`. Codice Apache-2.0 → F5-024.
  - *Affidabilità:* se l'ultima compilazione è **fallita** si mostra
    l'**ultimo indice valido** (coerente con `ultimo_documento`, che su
    errore non si svuota); il pannello non si svuota durante la scrittura.
  - *Dove:* core, funzione `indice_documento(&self) -> Vec<VoceIndice {
    livello, testo, file, riga, colonna, pagina }>` su `EditorWorld`;
    guscio `lib.rs` (comando, o indice incluso nel risultato di
    compilazione); frontend nuovo `src/pannelli/indice.ts` (apertura del
    file con il percorso di apertura della modalità progetto), aggiornamento
    agganciato al ciclo di compilazione esistente (nessun debounce in più).
  - *Dipendenze nuove:* nessuna.
  - *Prerequisiti:* F5-019.
  - *Verifica:* test core su un progetto con `main.typ` che include due
    capitoli (titoli di tutti i file nell'ordine giusto, file e riga
    corretti, livelli annidati, titolo generato da `#heading` incluso,
    `outlined: false` escluso, **livello saltato** — es. `=` seguito
    direttamente da `===` — annidato correttamente senza perdere voci,
    compilazione fallita → ultimo indice
    valido); smoke: clic su un titolo di un capitolo incluso apre quel
    file al titolo e sposta l'anteprima.

- **F5-021 — Albero dei file del progetto nella barra laterale.** *(P5-C,
  parte 3)*
  - *Obiettivo:* pannello "File" con l'albero della cartella-progetto
    (F4-001): cartelle comprimibili, file `.typ`/`.md` apribili con un clic
    (stesso percorso di apertura della modalità progetto), risorse mostrate
    ma non aperte nell'editor; file principale evidenziato. In modalità
    documento singolo il pannello invita ad aprire una cartella.
  - *Dove:* core, funzione pura di elenco della cartella (ordinamento
    cartelle-prima, esclusione di cartelle nascoste e di artefatti noti —
    elenco esclusioni in un file/costante di config, non sparso nel codice);
    guscio `lib.rs` (comando, confinato alla radice del progetto come
    `FsRoot`); frontend nuovo `src/pannelli/file.ts`.
  - *Perimetro — **deciso** (2026-09-28, D-015):* in Fase 5 **solo
    consultazione** (navigazione e apertura). Le operazioni sui file
    (crea/rinomina/elimina/sposta, con aggiornamento degli `#include`) sono
    nel backlog "Dopo la Fase 7" (voce B-OP-01). L'aggiornamento
    automatico dell'albero su file creati/cancellati da fuori arriva con
    F5-022; fino ad allora pulsante "Aggiorna".
  - *Dipendenze nuove:* nessuna.
  - *Prerequisiti:* F5-019.
  - *Verifica:* test core su cartella temporanea (ordinamento, esclusioni,
    nessuna uscita dalla radice con link simbolici/`..`); smoke: apertura di
    un capitolo dall'albero, compilazione del progetto invariata.

- **F5-022 — Controllo dei file modificati da fuori (`notify`) con
  anti-loop.** *(P5-D; SPEC-UNIFICATA §3.1/§5 Fase 4; automatizza F5-014)*
  - *Obiettivo:* l'app si accorge da sola dei cambiamenti su disco fatti da
    altri programmi e reagisce senza mai perdere il lavoro dell'utente.
  - *Comportamento — **deciso** (2026-09-28, D-014):*
    - **file aperto nell'editor senza modifiche non salvate** → si
      **ricarica da solo** (cursore/scorrimento conservati per quanto
      possibile) e si ricompila;
    - **file aperto con modifiche non salvate** → si **chiede all'utente
      quale versione tenere** (quella nell'editor o quella su disco); fino
      alla risposta nulla viene sovrascritto;
    - **file fratelli** (inclusi/importati, risorse, bibliografie del
      progetto, cioè il perimetro di F5-014) → invalidazione delle risorse
      (`invalida_risorse`, F5-014) e ricompilazione, senza domande;
    - albero dei file (F5-021) aggiornato su creazioni/cancellazioni/
      rinomine.
    - **file aperto che sparisce dal disco** (cancellato, o rinominato/
      spostato) — **deciso dall'utente** (2026-09-28, D-014: "avviso, e il
      testo resta nell'editor come 'non salvato'. Verifica il file: se non è
      compromesso salvalo"). *Interpretazione registrata; criteri di
      integrità confermati come implementati (D-024, 2026-09-30,
      `verifica_integrita` in `crates/typst-core/src/esterni.rs`):*
      1. avviso all'utente; il testo resta nell'editor, segnato come **non
         salvato**;
      2. verifica di **integrità** del contenuto nell'editor. Criteri
         proposti: buffer **non vuoto** se il file salvato non lo era; testo
         **UTF-8 valido**; **nessun troncamento** rispetto all'ultima
         versione nota (confronto con l'ultima voce di cronologia di F5-013
         / impronta dell'ultimo salvataggio). **Non** si richiede che il
         documento compili: un documento con errori di sintassi non è
         "compromesso";
      3. verifica superata → prima si registra comunque una voce in
         **cronologia** (F5-013), così nulla si perde; poi Typster **salva
         automaticamente al percorso originale** (ricrea il file) e lo dice
         nell'avviso ("File eliminato dall'esterno: ripristinato da
         Typster"). Il salvataggio di ripristino passa dall'**anti-loop**
         (sotto): non deve generare un nuovo evento "modificato da fuori";
      4. verifica fallita → **nessun salvataggio**: il documento resta "non
         salvato" e l'avviso chiede all'utente **dove salvare**.
    - **rinomina/spostamento rilevato** — **deciso dall'utente**
      (2026-09-28, D-014): Typster **segue il nuovo nome**. Quando `notify`
      fornisce l'evento di rinomina con vecchio e nuovo percorso, l'editor
      aggiorna il percorso del documento aperto (titolo, scheda,
      radice/progetto se serve); **nessun** ripristino al vecchio nome,
      **nessun** duplicato; avviso informativo ("File rinominato
      dall'esterno in «nome»"). La cronologia (F5-013) si associa al nuovo
      percorso (vedi F5-013). Se l'evento arriva come cancellazione +
      creazione separate non abbinabili, oppure la nuova posizione non è
      rilevabile (es. spostato fuori dalla cartella osservata), si ricade
      nel caso "sparisce dal disco" qui sopra (verifica di integrità →
      ripristino al percorso originale).
  - *Anti-loop dei salvataggi propri:* ogni scrittura dell'app (`salva_file`,
    auto-save di F5-013, export dentro la cartella di progetto) registra
    percorso + impronta del contenuto scritto; un evento su quel percorso il
    cui contenuto coincide con l'impronta registrata è **ignorato**.
    Coalescing degli eventi a raffica (batching del watcher) prima di
    decidere.
  - *Architettura dei watcher — **decisa** (D-016, fase8.md §7; tolta la
    ricognizione `research-scout` "notify vs debouncer"):*
    > *Superato da D-017 (2026-09-29): un solo watcher `notify` ricorsivo nel
    > core; niente feature `"watcher"` di typst-kit né `notify` nel guscio.*
    > Testo originale conservato per traccia.
    1. **File fratelli** (perimetro di F5-014): `typst_kit::watcher::Watcher`
       con la feature **`"watcher"`** di `typst-kit` (porta `notify` 8 e
       `same-file`, batching integrato: 100 ms fra eventi, tetto 500 ms,
       polling 300 ms per i file mancanti). L'elenco dei file da osservare è
       `FileStore::dependencies()`, **già** disponibile nel core
       (`world.rs`, `typst_kit::files::FileStore`); ciclo come typst-cli
       `watch.rs` 67-84: `update(dependencies) → wait → reset/invalida →
       compila` (fase8.md §7.1, §7.3). Il `wait()` bloccante gira su un
       thread avviato dal guscio.
    2. **Secondo watcher nel guscio** sulla **cartella** (progetto, o
       cartella del documento singolo), `RecursiveMode::Recursive`, per
       rinomine, file aperti e albero F5-021 (il watcher di typst-kit
       osserva solo file e non dice quale file né il tipo di evento).
       **Copiati da typst-kit** `watcher.rs` 120-211 (Apache-2.0, F5-024) il
       filtro `is_relevant_event_kind` e il **batching** (niente crate
       debouncer) (fase8.md §7.2).
    3. **Anti-loop per contenuto** (prima difesa): macchina a stati di
       Tinymist (`tinymist-project/src/watch.rs`, ADATTA, fase8.md §7.4) —
       per file `prev` + stato `Stable | EmptyOrRemoval{recheck_at}`; a ogni
       evento si rilegge il contenuto e, se uguale a `prev`, nessuna
       notifica; file vuoto o sparito → stato transitorio con ricontrollo
       ≥ 50 ms prima di notificare (copre i salvataggi atomici tronca/rinomina
       degli altri editor). Si combina col **registro SHA-256 delle scritture
       proprie** (`sha2`, come F5-013): contenuto uguale all'impronta
       registrata → ignora.
    4. **Ripiego** (seconda difesa, solo se la prima non basta, es. cloud
       drive che riemettono l'evento in ritardo): finestra temporale dopo la
       scrittura propria + confronto `mtime` (modello MarkText `watcher.ts`).
    5. **Rinomine su Windows:** arrivano come eventi `Modify(Name(From))` e
       `Modify(Name(To))` **separati** → vanno **abbinati dentro lo stesso
       batch**; se l'abbinamento non riesce si ricade nel caso "sparito dal
       disco" (D-014).
  - *Dove:* **core** (watcher dei file fratelli su `typst-kit`; logica pura e
    testabile: macchina a stati, registro scritture proprie, abbinamento
    From/To, decisione "ignora | ricarica | chiedi | invalida" da stato del
    documento + evento); **guscio** (thread dei watcher, secondo watcher
    `notify` sulla cartella, eventi verso il frontend via `Channel`/evento
    Tauri); frontend `src/fileops.ts` (dialog di scelta, ricarica del
    buffer), `src/main.ts`.
  - *Dipendenze nuove:* feature `"watcher"` di `typst-kit` (core);
    `notify` major **8** nel guscio per il secondo watcher, allineato a
    quello portato da `typst-kit` (versione esatta con `=` in §3
    all'implementazione); `sha2` (già prevista da F5-013). **Nessun** crate
    debouncer. *Superato da D-017 (2026-09-29): un solo watcher `notify`
    ricorsivo nel core; niente feature `"watcher"` di typst-kit né `notify`
    nel guscio* (bloccati in §3: `notify = "=8.2.0"`, `sha2 = "=0.10.9"`).
  - *Prerequisiti:* F5-014 (percorso di invalidazione), F5-013 (auto-save da
    proteggere), F5-021 (albero da aggiornare).
  - *Verifica:* test core della decisione (tutte le combinazioni stato ×
    evento) e dell'anti-loop (scrittura propria ignorata, scrittura esterna
    con stesso percorso ma contenuto diverso rilevata); smoke: modifica di
    `capitolo.typ` da un altro editor → anteprima aggiornata senza azioni;
    modifica esterna del file aperto non modificato → ricaricato; con
    modifiche non salvate → domanda, nessuna perdita; salvataggio e auto-save
    dell'app → nessuna ricarica né domanda. File aperto **cancellato** da
    fuori con contenuto **integro** → voce in cronologia, file ripristinato
    al percorso originale, avviso "ripristinato da Typster", nessun evento
    di ritorno (anti-loop); cancellato con buffer **vuoto** (file salvato
    non vuoto) o **troncato** rispetto all'ultima versione nota → nessun
    salvataggio, documento "non salvato", richiesta del percorso; documento
    con errori di sintassi ma integro → ripristinato; **rinomina** con evento
    abbinato → l'editor segue il nuovo nome (titolo/scheda aggiornati,
    avviso informativo, nessun file ricreato al vecchio percorso, cronologia
    raggiungibile dal nuovo percorso); rinomina come eventi separati non
    abbinabili o spostamento fuori dalla cartella osservata → trattata come
    cancellazione. Test core dei
    criteri di integrità (vuoto, UTF-8 non valido, troncamento, errori di
    sintassi non bloccanti), della macchina a stati (troncamento seguito da
    riscrittura entro il ricontrollo → nessuna notifica; file sparito oltre
    il ricontrollo → "sparito") e dell'abbinamento From/To nel batch.

- **F5-023 — Modalità focus e macchina da scrivere.** *(P5-G; ispirazione
  Typora)*
  - *Obiettivo:* due modalità indipendenti, attivabili da palette/scorciatoia
    e ricordate in `impostazioni.json`: **focus** — tutto il testo è
    schiarito tranne il paragrafo del cursore (paragrafo = blocco fra righe
    vuote, per `.typ` e `.md`); **macchina da scrivere** — la riga del
    cursore resta a metà altezza dell'editor mentre si scrive.
  - *Dove:* frontend `src/editor.ts` (estensioni CodeMirror 6 in un
    `Compartment` ciascuna: `ViewPlugin` con decorazioni di riga/mark per il
    focus; scorrimento con `EditorView.scrollIntoView(..., { y: "center" })`
    agli aggiornamenti della selezione e margine in testa/coda perché anche
    prima e ultima riga possano centrarsi); comandi nel registro F5-006;
    stili tramite il tema CodeMirror (compatibili con F5-004).
  - *Riuso — **deciso** (D-016, fase8.md §8):* **macchina da scrivere** =
    `extension.ts` di deathau/cm-typewriter-scroll-obsidian (commit
    `625b42f`, MIT, 88 righe CM6 puro — avviso MIT, F5-024) nel
    `Compartment` della modalità: facet dell'offset, plugin di padding
    (via `element.style`, compatibile con la CSP di B-005), scorrimento solo
    su `userEvent` consentiti, guardia anti-rientro,
    `scrollIntoView(head, {y: "start", yMargin})` in `requestAnimationFrame`;
    il padding va tolto allo spegnimento. **Focus** = scritto in casa
    (~40 righe): `ViewPlugin` con `Decoration.line({class: "cm-sfocato"})`
    sulle righe di `visibleRanges` fuori dal paragrafo del cursore,
    ricalcolato su `docChanged | selectionSet | viewportChanged`; opacità
    nel tema CM6 (riferimenti solo ADATTA: davisriedel, Muya).
  - *Dipendenze nuove:* nessuna.
  - *Prerequisiti:* F5-003, F5-004, F5-006.
  - *Verifica:* commutazione a caldo senza perdere cursore/selezione;
    nessun effetto su anteprima e sync; documento lungo senza scatti
    percepibili; smoke di **produzione** (stili iniettati a runtime, B-005).

**Blocco G — Licenze del codice derivato** *(D-016, 2026-09-28)*

- **F5-024 — Avvisi di licenza per il codice copiato da terze parti.**
  *(obblighi da fase8.md §0)*
  - *Obiettivo:* rispettare le licenze del codice COPIA elencato in
    [`fase8.md`](fase8.md), per il sorgente e per l'installer.
  - *Cosa si produce:*
    - file **`NOTICE`** unico in radice: `Contiene codice derivato da Typst
      (https://github.com/typst/typst), Apache-2.0` e `Contiene codice
      derivato da Tinymist (https://github.com/Myriad-Dreamin/tinymist),
      Apache-2.0`, più le righe MIT/Unlicense dove serve (deathau
      cm-typewriter-scroll-obsidian, MIT; MarkText se si copia
      `CommandDescriptor`, MIT; crate `grep`/ripgrep, Unlicense OR MIT) —
      **solo** per le fonti effettivamente copiate;
    - cartella **`licenses/`** con i testi **Apache-2.0** e **MIT**;
    - **commento in testa a ogni blocco copiato**:
      `// Derivato da <progetto> <file> (<commit>), licenza <X>`.
  - *Obblighi per fonte (fase8.md §0):*
    - **Typst** (tag `v0.15.0`, commit `3ae5277`), Apache-2.0: conservare
      "Copyright Typst contributors", copia della licenza, nota di
      derivazione nei file, riga nel `NOTICE`. Il `NOTICE` di Typst elenca
      terze parti non coinvolte negli estratti: **non** va ricopiato.
    - **Tinymist** (commit `f3b00eb`), Apache-2.0: avviso "Copyright
      Myriad-Dreamin e contributori Tinymist", copia della licenza, nota di
      derivazione, riga nel `NOTICE`.
    - **ripgrep, crate `grep`** 0.4.1 (commit `3fce3b5`), Unlicense OR MIT:
      scegliendo MIT, "Copyright (c) 2015 Andrew Gallant" + testo MIT;
      scegliendo Unlicense nessun obbligo (citare comunque la fonte). La
      scelta si registra in DECISIONI.md alla chiusura. Vale solo per codice
      *copiato* dagli esempi: l'uso del crate come dipendenza non richiede
      avvisi nel sorgente.
    - **deathau** (commit `625b42f`), MIT: avviso di copyright MIT accanto al
      codice copiato (F5-023).
    - **MarkText** (commit `34b5c28`), MIT: avviso MIT solo se si copia il
      tipo `CommandDescriptor` (F5-006).
    - davisriedel, Typstify, TyX, Typora, Muya: solo idee (ADATTA/NIENTE),
      nessun obbligo (Muya: come da `sorgenti/README.md`).
  - *Quando:* il commento di derivazione si scrive **al momento della
    copia**, nella task che copia (F5-006/008/011/012/020/022/023, F7-003/
    005/007); F5-024 crea `NOTICE` e `licenses/` e verifica il tutto prima
    dell'installer.
  - *Dove:* radice del repo (`NOTICE`, `licenses/`); sorgenti con blocchi
    copiati; `src-tauri/tauri.conf.json` (risorse del bundle, collegato a
    F5-016).
  - *Dipendenze nuove:* nessuna.
  - *Verifica:* ogni blocco marcato **COPIA** in fase8.md §1 e
    effettivamente copiato nei sorgenti ha il suo commento di derivazione
    (controllo con `grep -rn "Derivato da"` confrontato con l'elenco); ogni
    fonte citata nei commenti compare nel `NOTICE`; `licenses/` contiene
    Apache-2.0 e MIT; `NOTICE` e `licenses/` inclusi nell'installer
    (verifica in F5-016). *Nota:* le task di Fase 7 che copiano codice
    (F7-003, F7-005, F7-007) aggiornano `NOTICE` alla propria chiusura.

**Ordine consigliato.** F5-002 (solo documenti, chiarisce il perimetro —
**fatta** 2026-09-28) → F5-001 (nome; D-009 decisa, sbloccata) → F5-003 → F5-004 → F5-005 →
F5-006 → F5-023 → F5-019 → F5-020 → F5-021 → F5-010 → F5-007 → F5-008 →
F5-009 → F5-011 → F5-012 → F5-014 → F5-013 → F5-022 → F5-015 → F5-017 →
F5-024 → F5-016 → F5-018. *Motivazione:* prima il contratto e l'identità (evitano
rilavorazioni su installer e cartelle dati), poi la base impostazioni da cui
dipendono aspetto e debounce; subito dopo la palette le parti di interfaccia
che ne usano registro e temi (focus/macchina da scrivere, barra laterale con
pacchetti, indice e file), così il layout è stabile prima delle prestazioni
(F5-009 dopo F5-008 perché condividono il render SVG); poi le lacune
funzionali, con F5-022 dopo F5-014 (percorso di invalidazione) e F5-013
(auto-save da proteggere con l'anti-loop); infine la distribuzione che
fotografa il risultato, con le licenze del codice derivato (F5-024) sistemate
prima del primo installer (F5-016).

#### Proposte P5-A…P5-I — confluite in task (D-014)

> Le proposte prima "opzionali (da confermare)" (SPEC-UNIFICATA.md §3/§5 e
> ispirazioni da altri editor) sono state **tutte accettate** dall'utente il
> 2026-09-28 (DECISIONI.md D-014). Tracciabilità:

| Proposta | Contenuto | Task |
|----------|-----------|------|
| P5-A | Palette comandi a sottomenu | F7-001 (innestata sul registro F5-006) |
| P5-B | Base comune menu/popup (`@floating-ui/dom`) | F7-002 |
| P5-C | Barra laterale: indice, albero file, pacchetti | F5-019, F5-020, F5-021 |
| P5-D | Controllo file modificati da fuori (`notify`) + anti-loop | F5-022 |
| P5-E | Ricerca nel testo di tutto il progetto (pannello della barra) | F7-003 |
| P5-F | Interfaccia multilingua (italiano + inglese) | F7-004 |
| P5-G | Modalità focus e macchina da scrivere | F5-023 |
| P5-H | Scheda parametri delle funzioni | F7-005 |
| P5-I | Pannello variabili `sys.inputs` salvate nel progetto | F7-007 |
| — | Menu `/` (SPEC-UNIFICATA §3.1 "Slash menu") | F7-006 |

**DoD:** installer per le tre piattaforme; avvio rapido e binario leggero
(target ~~pochi MB~~ rivisto con D-021: `typster.exe` 40,6 MB, setup Inno
15,8 MB misurati il 2026-09-29, valore definitivo con F5-018; nessun sidecar nel bundle — Tinymist è
un extra opzionale fuori installer, D-010); requisiti non funzionali §9 rispettati. *Ampliamento
(2026-09-28):* prodotto rinominato "Typster" in UI, finestra e installer
(F5-001); contratto coerente sull'assistenza (F5-002); palette, temi e
dimensione font persistiti su file di configurazione esterno (F5-003…F5-006);
export PNG/SVG con scelta pagine/DPI ed export PDF/A (F5-011, D-011); misure §9 registrate in
DECISIONI.md (F5-018). *Ampliamento (2026-09-28, D-014):* barra laterale con
browser pacchetti, indice del documento e albero dei file funzionanti
(F5-019…F5-021); file modificati da fuori ricaricati o segnalati secondo il
comportamento deciso, senza reazioni ai salvataggi propri (F5-022); modalità
focus e macchina da scrivere attivabili e persistite (F5-023). *Ampliamento
(2026-09-28, D-016):* obblighi di licenza del codice derivato rispettati —
`NOTICE` in radice, `licenses/` con Apache-2.0 e MIT, commento di
derivazione su ogni blocco copiato, `NOTICE` incluso nell'installer
(F5-024). Le voci della Fase 7 **non** fanno parte di questa DoD.

### Fase 6 — Editor Markdown e interoperabilità md↔typst
> **Stato: COMPLETATA (2026-07-16)** — sbloccata in giornata dalla
> **ricostruzione del core** (B-003 risolto: 5 moduli riscritti da zero,
> 41 test) e dalla **conferma della strategia di conversione** (D-007:
> Opzione B, nativa in Rust). DoD raggiunta: apri/salva `.md` con
> evidenziazione ✓; anteprima dal vivo via md→typst→compile ✓; `converti` nei
> due versi ✓; conversione documentata come lossy ✓. Verifica: build + clippy
> (`-D warnings`) + fmt + **74 test** core, `npm run build` (tsc+Vite) verdi.
> Dettaglio in DECISIONI.md F6-001…F6-003. *Nota:* lo smoke test
> **interattivo** a runtime dell'app completa resta in capo all'utente; il
> guscio si compila anche su questa macchina su toolchain **GNU** —
> `code_tools\rust_bundle` + `CARGO_TARGET_DIR` senza spazi, MSVC NON
> necessario; per l'esecuzione serve il runtime WebView2, di norma presente
> su Win11 (vedi DECISIONI.md B-004).

**Obiettivo:** aprire e scrivere `.md` con evidenziazione dedicata, anteprima
dal vivo riusando la pipeline Typst, conversione bidirezionale md↔typst
(§4.5 di [PROGETTO.md](PROGETTO.md)).
- **Tipo di documento** (`.typ` | `.md`) come concetto a cavallo dominio +
  presentazione: guida (a) la modalità di evidenziazione dell'editor, (b) se
  convertire prima di compilare l'anteprima, (c) il formato di salvataggio.
- **Evidenziazione Markdown** nel frontend via `@codemirror/lang-markdown`
  (ufficiale CodeMirror, leggero — §3), affiancata a quella Typst esistente e
  commutata in base al tipo di documento.
- **Anteprima dal vivo del `.md`**: NESSUN secondo motore di rendering — si
  converte md→typst e si riusa la pipeline esistente (`EditorWorld::compila`
  → SVG). Principio "orchestrare, non duplicare".
- **Convertitori nel core** (`typst-core`, puri e testabili headless; modulo
  `markdown.rs` in §2): `md→typst` via `pulldown-cmark` (titoli `=`,
  `*grassetto*`, `_corsivo_`, liste, `#link`, `#image`, blocchi codice raw,
  tabelle → `#table`); `typst→md` percorrendo l'AST di `typst-syntax` (già
  disponibile) per il sottoinsieme mappabile. *Strategia da confermare* (§6
  punto 5).
- **Comandi di conversione** "converti Markdown → Typst" e "converti Typst →
  Markdown" esposti dal guscio Tauri; conversione dichiaratamente **lossy**
  sui costrutti avanzati, con avviso all'utente sui costrutti non mappati.
- *Incremento di partenza consigliato* (stile D-004/D-005): tipo di documento
  + evidenziazione `.md` + anteprima via md→typst (riuso pipeline, valore
  immediato, rischio basso); `typst→md` come incremento successivo.

**DoD:** apertura e salvataggio di un `.md` con evidenziazione Markdown;
anteprima dal vivo di un `.md` (via md→typst→compile); comando `converti` nei
due versi che produce output compilabile (verso typst) / valido (verso md) su
un documento d'esempio; conversione documentata come lossy, con avviso
all'utente sui costrutti non mappati.

### Fase 7 — Estensioni
> **Stato: PIANIFICATA (2026-09-28)** — istituita con DECISIONI.md **D-014**
> (accettazione delle proposte P5-A…P5-I): raccoglie le proposte accettate
> che non entrano nella Fase 5. Task F7-001…F7-007 qui sotto. Si avvia a
> **Fase 5 completata**: poggia su registro comandi (F5-006), impostazioni
> (F5-003) e barra laterale (F5-019).

**Obiettivo:** estendere l'interfaccia e l'assistenza oltre la rifinitura —
palette a sottomenu e menu `/` su una base comune di popup, ricerca in tutto
il progetto, interfaccia multilingua, scheda parametri delle funzioni,
variabili `sys.inputs` per progetto.

#### Task della Fase 7

Stesso formato e stesse regole di chiusura della Fase 5 (obiettivo · dove ·
dipendenze nuove · verifica; registrazione in DECISIONI.md come `F7-xxx`;
build + test + clippy + `tsc`/Vite; per effetti visibili exe di produzione +
smoke interattivo, B-005).

- **F7-001 — Palette comandi a sottomenu.** *(P5-A; SPEC-UNIFICATA §3.1,
  MarkText `RootCommand`)*
  - *Obiettivo:* voci della palette che aprono un livello successivo, es.
    "Esporta ▸ PDF / PNG / SVG", "Tema ▸ chiaro / scuro / segui il sistema";
    percorso mostrato in testa (es. "Esporta ›"), Backspace su campo vuoto o
    Esc per risalire di un livello; sottocomandi anche calcolati al momento
    (es. elenco dei file recenti), con ricerca fuzzy dentro il livello.
  - *Dove:* frontend `src/comandi.ts` (usa il campo figli/sottocomandi
    previsto in F5-006: **nessuna** rifattorizzazione del registro),
    `src/palette.ts` (navigazione per livelli).
  - *Comportamento — **deciso** (D-016, fase8.md §9):* discesa come
    `executeCommand` di MarkText (ADATTA): un comando senza `esegui`/`run`
    ma con `sottocomandi` diventa il livello corrente, query azzerata;
    sottocomandi asincroni caricati con `run()`. **"Backspace su campo
    vuoto = risali di un livello"** si scrive in casa (MarkText non lo ha).
  - *Dipendenze nuove:* nessuna.
  - *Prerequisiti:* F5-006 (registro compatibile).
  - *Verifica:* "Esporta ▸ PDF" raggiungibile solo da tastiera ed eseguito;
    scorciatoie dirette invariate; Esc dal livello radice chiude e rende il
    focus.

- **F7-002 — Base comune per menu e popup (`@floating-ui/dom`).** *(P5-B;
  SPEC-UNIFICATA §3.1 "Sistema di floating UI")*
  - *Obiettivo:* un unico componente per i popup ancorati: posizionamento a
    un elemento o al **cursore** (riferimento virtuale con solo
    `getBoundingClientRect`), ribaltamento/spostamento ai bordi della
    finestra, chiusura unificata (Esc, clic fuori, scorrimento), navigazione
    da tastiera nelle liste, ritorno del focus. Lo usano il menu `/`
    (F7-006), la scheda parametri (F7-005) ed eventuali menu contestuali
    (es. albero file). L'autocompletamento di `@codemirror/autocomplete`
    (F2-002) mantiene il suo popup.
  - *Dove:* frontend nuovo `src/popup.ts` (o `src/ui/popup.ts`).
  - *Dipendenze nuove:* `@floating-ui/dom` (npm) — versione da **bloccare
    in §3 al momento dell'implementazione**.
  - *Verifica:* popup ancorato al cursore corretto a inizio/fine riga e
    vicino ai bordi della finestra; nessun doppio Enter dopo la selezione;
    smoke di produzione (posizionamento via `element.style`, compatibile
    con la CSP di B-005 — da confermare nello smoke).

- **F7-003 — Ricerca nel testo di tutto il progetto.** *(P5-E;
  SPEC-UNIFICATA §3.1)*
  - *Obiettivo:* pannello **"Cerca"** nella barra laterale (registrato sul
    registro pannelli di F5-019): testo o espressione regolare,
    maiuscole/minuscole, parola intera; risultati raggruppati per file con
    riga e anteprima, che arrivano **man mano** (streaming) e si possono
    **annullare**; clic su un risultato → apertura del file e cursore sulla
    corrispondenza. Solo in modalità progetto (in documento singolo il
    pannello invita ad aprire una cartella).
  - *Implementazione:* in **Rust**, nel core (funzione pura di ricerca su una
    radice, testabile headless) con il guscio che la esegue fuori dal thread
    UI e inoltra i risultati via `Channel`. **Niente sidecar `rg`**
    (coerente con D-010: nessun binario esterno nel bundle).
  - *Crate e disegno — **decisi** (D-016, fase8.md §10; ricognizione
    `research-scout` non più necessaria):* **`grep = "=0.4.1"`** (oppure i
    due sotto-crate `grep-regex = "=0.1.14"` + `grep-searcher = "=0.1.17"`)
    e **`ignore = "=0.4.33"`** per la visita (nascosti/`.gitignore`,
    `overrides` per le esclusioni di F5-021, `max_filesize`). Matcher con
    `RegexMatcherBuilder` (`case_insensitive`, `word(true)` per la parola
    intera, `fixed_strings` per il letterale); `SearcherBuilder` con
    `binary_detection(quit(b'\0'))` e numeri di riga (esempio
    `simplegrep.rs`, commit `3fce3b5`, Unlicense OR MIT — F5-024).
    **Annullamento** con un `AtomicBool` controllato nel `Sink` (restituire
    `Ok(false)` interrompe il file) e nel ciclo di visita. **Buffer non
    salvati** cercati con `search_slice(&matcher, bytes, sink)` sul testo
    dell'editor. **Protocollo** sul `Channel` Tauri modellato su MarkText
    (ADATTA): ogni ricerca ha un `searchId`; eventi match / progress /
    done / error / cancelled; una nuova ricerca annulla la precedente.
  - *Perimetro:* file di testo del progetto (esclusi binari, file oltre una
    soglia di dimensione, cartelle escluse come in F5-021); nei file con
    modifiche non salvate si cerca nel **testo dell'editor**, non su disco
    (**deciso** 2026-09-28, D-015). **"Sostituisci in tutto il progetto":
    fuori** dalla task, nel backlog "Dopo la Fase 7" (voce B-OP-02).
  - *Dipendenze nuove:* `grep = "=0.4.1"` (o `grep-regex = "=0.1.14"` +
    `grep-searcher = "=0.1.17"`) e `ignore = "=0.4.33"` nel core, già
    riportate in §3 (versioni al 2026-09-28: riconfermarle all'implementazione).
  - *Prerequisiti:* F5-019, F5-021 (apertura file dal pannello, esclusioni).
  - *Verifica:* test core (letterale, regex, maiuscole, parola intera, file
    binario saltato, annullamento che interrompe la visita); smoke su un
    progetto reale: primi risultati visibili prima della fine della
    ricerca, nuova ricerca che annulla la precedente.

- **F7-004 — Interfaccia multilingua (italiano + inglese).** *(P5-F;
  SPEC-UNIFICATA §3.2 "i18n con `t()` lazy")*
  - *Obiettivo:* tutte le stringhe dell'interfaccia passano da una funzione
    `t(chiave, parametri)`; lingue **italiano** (riferimento e ripiego per
    chiavi mancanti) e **inglese**; lingua scelta nelle impostazioni
    (`impostazioni.json`, F5-003, chiave `lingua`; predefinito: lingua del
    sistema se disponibile, altrimenti italiano). Altre lingue si aggiungono
    con **un file per lingua** (es. `src/i18n/it.json`, `src/i18n/en.json`,
    caricati a richiesta), senza toccare il codice.
  - *Dove:* frontend nuovo `src/i18n/` (funzione `t`, caricamento lazy dei
    file lingua); tutti i moduli di `src/` e `index.html` (estrazione delle
    stringhe); titoli dei comandi del registro F5-006 (già chiavi, vedi
    F5-006); pannello impostazioni (scelta lingua).
  - *Perimetro — **deciso** (2026-09-28, D-015):* in Fase 7 si traducono
    **solo i testi dell'interfaccia** (frontend). I **messaggi generati da
    core e guscio** Rust (errori di I/O, pacchetti, conversione md↔typst)
    **restano in italiano** per ora; la loro traduzione (codici + parametri
    restituiti dal core e tradotti nel frontend) è nel backlog "Dopo la
    Fase 7" (voce B-OP-03). La diagnostica del compilatore Typst resta
    nella lingua del compilatore.
  - *Cambio lingua — **deciso**:* **immediato, senza riavvio**: tutte le
    viste riemettono i testi alla scelta (pannelli, palette, barra
    laterale, modali aperte; titoli del registro F5-006 valutati al momento
    della visualizzazione). *Meccanismo — **deciso** (D-016, fase8.md §9):*
    un **evento unico "lingua cambiata"** (modello `language-changed` di
    MarkText, ADATTA) a cui si iscrivono le viste; la palette vi riesegue
    `run()` e ricalcola la lista.
  - *Dipendenze nuove:* nessuna (funzione `t` scritta in casa; nessuna
    libreria i18n prevista).
  - *Verifica:* test di completezza (ogni chiave di `it` presente in `en`,
    nessuna chiave usata nel codice assente da `it`); cambio lingua
    applicato **subito, senza riavvio** (nessun testo rimasto nella lingua
    precedente, con una modale aperta) e ricordato; smoke di produzione in
    entrambe le lingue.

- **F7-005 — Scheda parametri delle funzioni.** *(P5-H; ispirazione TyX)*
  - *Obiettivo:* mentre si scrivono gli argomenti di una chiamata
    (`#figure(`, `#table(` …), una scheda mostra la firma della funzione:
    nomi e **tipi** dei parametri, **documentazione** breve, valori
    predefiniti, con il **parametro corrente** evidenziato.
  - *Ricognizione — **fatta** (D-016, fase8.md §11; non serve più
    `research-scout`):* `typst-ide` 0.15 **non** espone una "signature
    help"; la scheda si costruisce nel core. Individuazione di
    chiamata/`set`, callee e argomenti come `complete_params` di
    `typst-ide` 0.15.0 `complete.rs` 428-449/495-515; risoluzione del callee
    con `analyze_expr_with_fallback` (pubblica) → `Func`; parametri da
    `func.params()` (`ParamInfo`); **parametro corrente** con `arg_context`
    di Tinymist (`tinymist-analysis/src/syntax/matcher.rs` 1442-1520,
    commit `f3b00eb`, solo `typst-syntax`); etichette e limite del
    variadico come `signature_help.rs` di Tinymist (ADATTA). Codice
    Apache-2.0 → F5-024.
  - *Perimetro — **deciso** (D-016):*
    - **funzioni native:** nomi, **tipi** (da `CastInfo`, testo via
      `describe()`/`Display`), **documentazione**, **valori predefiniti**,
      **parametro corrente** evidenziato (nei `set` solo parametri
      `settable`; per i posizionali indice limitato a `n_posizionali-1`, così
      un variadico resta evidenziato);
    - **funzioni definite con `let`:** **nomi** + tipo di parametro
      (posizionale / nominale / sink `..`) + **default dei nominali**; niente
      documentazione né tipi;
    - **senza una compilazione valida** (nessun `ultimo_documento` utile a
      risolvere il callee): **ripiego sui globali** della libreria
      standard (funzioni native risolte senza contesto del documento).
  - *Dove:* core `crates/typst-core/src/ide.rs` (funzione pura
    `scheda_parametri(sorgente, posizione)`); guscio `lib.rs` (comando);
    frontend `src/assistenza.ts` (popup su F7-002, senza sovrapporsi al
    popup di completamento).
  - *Dipendenze nuove:* nessuna prevista (`typst-ide`, `typst` già presenti).
  - *Prerequisiti:* F7-002.
  - *Verifica:* test core (funzione nativa con parametri posizionali e
    nominali, parametro corrente dopo virgole/argomenti nominali, chiamata
    annidata, `set` con soli parametri `settable`, funzione `let` con nomi +
    pos/named/sink + default, documento senza compilazione valida → scheda
    dai globali, posizione fuori da una chiamata → nessuna scheda); smoke:
    scheda visibile e aggiornata mentre si scrive, chiusa con Esc.

- **F7-006 — Menu `/` (inserimento rapido).** *(SPEC-UNIFICATA §3.1 "Slash
  menu / quick-insert", accettato dall'utente il 2026-09-28)*
  - *Obiettivo:* digitando `/` a inizio riga compare un menu con ricerca
    fuzzy dei costrutti da inserire (`= titolo`, liste, `#figure`, `#table`,
    `#image`, `$ … $`, `@etichetta`, …); la scelta inserisce uno **snippet
    con tabstop** (meccanismo di F2-003).
  - *Catalogo data-driven:* voci `{etichetta, titolo, sottotitolo,
    scorciatoia, snippet}` in un file di config esterno e versionato (sul
    modello di `snippets.json`), con gli snippet dell'utente di F2-003
    inclusi; catalogo distinto per `.typ` e `.md`.
  - *Attenzione — conflitto di sintassi:* in Typst `/ Termine: descrizione`
    è la **lista di termini**, quindi `/` a inizio riga **non** è libero
    (SPEC-UNIFICATA lo dava per libero: nota di correzione aggiunta in
    §3.1). Regola **confermata** (2026-09-28, D-015): il menu si apre su
    `/`, ma `/` **seguito da spazio** chiude il menu e la riga resta una
    riga di **lista di termini** (`/` e spazio intatti, nessuna
    interferenza); Esc chiude lasciando il `/`.
  - *Dove:* frontend nuovo `src/menu-slash.ts` (estensione CodeMirror che
    rileva il trigger + popup su F7-002); core `config.rs` (lettura del
    catalogo, come `carica_snippet`).
  - *Dipendenze nuove:* nessuna oltre a quella di F7-002.
  - *Prerequisiti:* F7-002, F2-003.
  - *Verifica:* test core del catalogo (default su file mancante/invalido);
    smoke: `/tab` + Invio inserisce una tabella con tabstop; `/ Termine:`
    scritto di seguito produce una lista di termini compilabile.

- **F7-007 — Pannello variabili `sys.inputs` salvate (progetto o documento
  singolo).** *(P5-I; ispirazione Typstify)*
  - *Obiettivo:* un pannello (nella barra laterale, F5-019) per definire
    coppie **nome = valore** lette dal documento con `sys.inputs` (es.
    `#sys.inputs.at("versione", default: "bozza")`); modifica → ricompilazione.
    Equivalente alle opzioni `--input nome=valore` della CLI.
  - *Persistenza — **decisa** (D-014):* le variabili si **salvano nel
    progetto**, in un file nella cartella-progetto, versionabile insieme ai
    sorgenti. Il file si crea solo quando si definisce la prima variabile:
    eccezione deliberata e visibile al principio "cartelle di progetto
    pulite" di D-012, perché qui il dato appartiene al progetto.
  - *Documento singolo — **deciso** (2026-09-28, D-016; chiude il punto
    aperto di D-015). **Decisione provvisoria, da rivedere dopo l'uso**
    ("b, per ora, poi vediamo come si comporta il sistema"):* anche per un
    documento singolo (fuori da un progetto) le variabili **restano salvate
    dopo la chiusura**, non valgono solo per la sessione. *Dettaglio
    implementativo proposto:* si salvano nella **cartella dati dell'app**
    (`…/typster/`, es. `dirs::data_dir()/typster/inputs/<chiave>.json`),
    associate al file con la **stessa chiave SHA-256 del percorso** usata
    dalla cronologia (D-012, F5-013: percorso canonicalizzato, minuscolo su
    Windows) — riuso della stessa funzione del core; quando F5-022 segue una
    rinomina (D-014) la voce si **ri-chiava** come la cronologia. La cartella
    del documento resta pulita.
  - *Progetto — file nel progetto (D-014, dettaglio D-016):*
    `typster-inputs.json` nella cartella-progetto, oggetto piatto
    `{ "nome": "valore" }` con soli valori stringa (come la CLI), **creato
    solo alla prima variabile** (come Typstify, che usa `sys-inputs.json`:
    Typster mantiene il proprio nome `typster-inputs.json`).
  - *Parsing — **deciso** (D-016, fase8.md §12):* l'inserimento
    `chiave=valore` si interpreta come `parse_sys_input_pair` di typst-cli
    v0.15.0 (`args.rs` 800-810, Apache-2.0 — F5-024): divisione al **primo**
    `=`, `trim` di chiave e valore, chiave vuota → errore leggibile. Lettura
    del file **tollerante**: file assente o JSON non valido → **nessuna
    variabile** e **avviso non bloccante**, mai panic; la compilazione
    prosegue.
  - *Punto di aggancio verificato (2026-09-28):*
    `crates/typst-core/src/world.rs` costruisce la libreria in
    `EditorWorld::nuovo` con `libreria: LazyHash::new(Library::default())`
    (riga ~193) e la espone da `fn library` (riga ~475). Si passa a
    `Library::builder().with_inputs(dizionario).build()` (`with_inputs`
    presente in `typst-library` 0.15) con un metodo del core che sostituisce
    la libreria quando le variabili cambiano; stessa libreria per anteprima
    ed export (risultati coerenti).
  - *Dove:* core `world.rs` (impostazione degli input) + lettura/scrittura
    del file in `config.rs` o modulo dedicato (parsing tollerante, scrittura
    atomica); guscio `lib.rs` (comandi; caricamento all'apertura del
    progetto o del documento singolo); frontend nuovo
    `src/pannelli/inputs.ts`.
  - *Dipendenze nuove:* nessuna (`sha2` già prevista da F5-013).
  - *Prerequisiti:* F5-019; F5-013 (chiave SHA-256 del percorso); F5-022
    (rinomina seguita).
  - *Verifica:* test core (documento che stampa `sys.inputs.at(...)`
    compilato con e senza variabili; file mancante/invalido → nessuna
    variabile e avviso, senza panic; round-trip del file; `a=b=c` → chiave
    `a`, valore `b=c`; spazi attorno rifilati; chiave vuota → errore; file
    di progetto creato solo alla prima variabile); confronto con `typst
    compile --input` della CLI 0.15; riapertura del progetto → variabili
    ripristinate; riapertura di un documento singolo dopo la chiusura →
    variabili ripristinate dalla cartella dati dell'app, nessun file creato
    accanto al documento.

**Ordine consigliato.** F7-004 → F7-002 → F7-001 → F7-006 → F7-005 →
F7-003 → F7-007. *Motivazione:* il multilingua per primo, così le interfacce
nuove della Fase 7 nascono già con `t()` invece di essere tradotte dopo; poi
la base dei popup, da cui dipendono palette a sottomenu, menu `/` e scheda
parametri (ricognizione già fatta, D-016); infine i due pannelli della barra
laterale (ricerca, crate già scelti con D-016, e variabili `sys.inputs`),
indipendenti fra loro.

**DoD:** palette con sottomenu navigabili da tastiera (F7-001); menu e popup
su una base comune (F7-002); ricerca nel progetto dal pannello della barra
laterale, in streaming e annullabile, senza binari esterni (F7-003);
interfaccia completa in italiano e inglese, lingua scelta nelle impostazioni
(F7-004); scheda parametri funzionante sul perimetro deciso dalla
ricognizione (F7-005); menu `/` che inserisce costrutti compilabili senza
ostacolare le liste di termini (F7-006); variabili `sys.inputs` salvate nel
progetto (o, per il documento singolo, nella cartella dati dell'app) e
applicate ad anteprima ed export (F7-007); avvisi di licenza aggiornati per il
codice copiato in Fase 7 (F5-024); dipendenze nuove
bloccate in §3; exe di produzione + smoke interattivo per ogni task visibile.

### Dopo la Fase 7 (backlog)
> Voci **accettate nella direzione** dall'utente (2026-09-28, DECISIONI.md
> D-015) ma rinviate a dopo la Fase 7: non sono pianificate in una fase,
> non entrano in alcuna DoD e non hanno ancora task. Diventano task solo con
> una nuova pianificazione registrata in DECISIONI.md.

- **B-OP-01 — Operazioni sui file dall'albero** (seguito di F5-021, che in
  Fase 5 è solo consultazione): crea, rinomina, elimina, sposta file e
  cartelle del progetto, con **aggiornamento degli `#include`/`#import`**
  che li richiamano. Da coordinare con F5-022 (le operazioni fatte dall'app
  passano dall'anti-loop) e con la cronologia F5-013 (rinomina → cronologia
  associata al nuovo percorso).
- **B-OP-02 — Sostituisci in tutto il progetto** (seguito di F7-003): con
  anteprima delle sostituzioni, conferma e annullamento; sui file con
  modifiche non salvate agisce sul testo dell'editor.
- **B-OP-03 — Traduzione dei messaggi di core e guscio** (seguito di
  F7-004, "poi vedremo"): i messaggi generati in Rust diventano codici +
  parametri tradotti nel frontend con `t()`.

> Voci **B-OP-04…B-OP-09**: proposte di SPEC-UNIFICATA.md §3/§5 mai decise,
> accettate nella direzione il 2026-09-30 (DECISIONI.md **D-025**); stesso
> stato delle precedenti (non pianificate).

- **B-OP-04 — Reveal-markup-on-focus** (SPEC-UNIFICATA §3.1): delimitatori
  di formato (`*…*`, `_…_`, `` `…` ``, `$…$`) attenuati o nascosti e resi
  visibili quando il cursore vi entra; decorazioni CodeMirror 6, documento
  intatto. Da coordinare con l'evidenziazione WASM.
- **B-OP-05 — Auto-pair contestuale + Tab "esci dal formato"**
  (SPEC-UNIFICATA §3.1): condizioni fini su `closeBrackets` (niente coppie
  in codice/math, non dopo alfanumerico); Tab salta oltre il delimitatore
  di chiusura.
- **B-OP-06 — Toggle sulla toolbar** (refinement F2-001): un formato già
  presente sulla selezione si rimuove invece di raddoppiarsi.
- **B-OP-07 — Input rules a inizio riga** (SPEC-UNIFICATA §3.2, con
  cautela): solo regole di inizio riga con tabella riscritta per Typst,
  mai regex sull'inline.
- **B-OP-08 — Fence speciale con anteprima in container**
  (SPEC-UNIFICATA §3.1): anteprima locale di un blocco con cache e errore
  per frammento; da valutare rispetto all'anteprima già esistente.
- **B-OP-09 — Cursore `{key, offset}` + sentinelle** (SPEC-UNIFICATA
  §3.2): riposizionamento del cursore dopo trasformazioni testuali senza
  mappatura (es. conversione md↔typst, F6).

### Estensioni opzionali (fuori dalle fasi)
> Voci **decise nella direzione** ma **non pianificate** in una fase: non
> entrano in nessuna DoD e non bloccano la chiusura di alcuna fase. Si
> attuano solo su richiesta esplicita dell'utente, con registrazione in
> DECISIONI.md.

- **X-001 — Tinymist come componente aggiuntivo caricabile.** *(DECISIONI.md
  D-010, 2026-09-28; esito di F5-002)*
  - *Obiettivo:* se l'utente ha installato Tinymist e lo indica all'app, questa
    lo carica come server LSP e ne sfrutta l'assistenza ricca (diagnostica
    avanzata, formattazione, rename, inlay hints, completamento/hover più
    ricchi). Se Tinymist è assente, disattivato o incompatibile, l'app
    funziona esattamente come oggi con `typst-ide` embeddato (F2-002):
    **nessuna** funzione di base dipende da Tinymist.
  - *Perimetro (D-010):* Tinymist **non** entra nel bundle/installer, **non**
    è in `src-tauri/binaries/` né in `externalBin`; l'app non lo scarica da
    sé (eventuale download assistito: proposta separata, da confermare).
  - *Come si indica:* percorso dell'eseguibile nelle impostazioni
    (`impostazioni.json`, F5-003 — es. chiave `tinymist.percorso`, vuota =
    disattivato) scelto dall'utente con un dialog; eventuale ricerca nel
    `PATH` come comodità, da confermare.
  - *Avvio e trasporto:* il **guscio** (non il frontend) avvia il processo
    dal percorso configurato e fa da proxy puro JSON-RPC su stdio
    (`lsp_proxy.rs`: preserva il framing `Content-Length`; frontend→Tinymist
    via `invoke`, Tinymist→frontend via `Channel<String>`), client
    `@codemirror/lsp-client` con `Transport` custom nel webview (`src/lsp/`).
    Poiché l'eseguibile è scelto dall'utente e non è un sidecar in bundle, lo
    schema `tauri-plugin-shell` + `externalBin` non si applica: *proposta*
    avvio con `std::process`/`tokio::process` dal Rust del guscio, senza
    esporre alcun permesso di esecuzione al webview — **da verificare con
    `research-scout`** insieme al transport.
  - *Compatibilità (rischio §7 "Allineamento versioni Typst↔Tinymist"):* al
    caricamento si interroga la versione dell'eseguibile (es. `--version`) e
    la si confronta con una tabella di versioni di Tinymist note come
    allineate al livello di linguaggio dei crate embeddati (`=0.15.0`),
    tabella da registrare in DECISIONI.md. Versione non riconosciuta →
    avviso chiaro all'utente e, *proposta*, caricamento solo su conferma
    esplicita; mai pinnare i crate embeddati per inseguire Tinymist (inverte
    D-002: è l'estensione ad adattarsi al core, non viceversa).
  - *Transport (rischio §7 "Transport LSP `invoke`-based su Tauri non
    collaudato"):* prototipo isolato **prima** dell'integrazione; fallback a
    WebSocket locale se il transport su `invoke`/`Channel` non regge.
  - *Convivenza con `typst-ide` (da definire nella task):* con Tinymist attivo
    le sorgenti di completamento/hover di `src/assistenza.ts` si
    disattivano (niente popup doppi); gli snippet di F2-003 restano; la
    diagnostica dell'anteprima resta quella del compilatore embeddato (unica
    fonte di verità per il rendering), quella di Tinymist si aggiunge senza
    duplicare le stesse righe. Crash/uscita di Tinymist → ritorno automatico a
    `typst-ide`, senza bloccare l'editor.
  - *Dipendenze nuove (solo se attuata):* `@codemirror/lsp-client` ^6 (da
    bloccare in §3 all'attuazione); nessuna crate nuova prevista
    (`std::process`/`tokio` già disponibili — da confermare).
  - *Prerequisiti:* F5-003 (impostazioni); `research-scout` su versioni
    Tinymist compatibili con Typst 0.15 e su transport LSP in Tauri 2.
  - *Verifica:* senza Tinymist configurato: nessuna differenza di
    comportamento né di dimensione del binario; con Tinymist compatibile:
    diagnostica/hover/formattazione funzionanti e nessun popup doppio;
    percorso inesistente, versione incompatibile o processo terminato a
    metà: messaggio chiaro e ritorno a `typst-ide` senza blocchi; smoke
    sull'exe di produzione (CSP: il transport non deve richiedere
    `connect-src` aggiuntivi, salvo fallback WebSocket da documentare).

---

## 5. Mappatura requisiti non funzionali (§9) → tecnica

| Requisito | Come lo otteniamo |
|-----------|-------------------|
| Leggerezza | WebView di sistema (no runtime Electron); ottimizzazioni Cargo; nessun sidecar nel bundle (Tinymist extra opzionale fuori installer, D-010/X-001). |
| Reattività | Compilazione incrementale `comemo` + debounce + render pagina visibile. |
| Affidabilità | Ultima anteprima valida conservata; errori isolati nel risultato di `compile`. |
| Portabilità | Tauri 2 multipiattaforma; percorsi cache risolti per OS via `typst-kit`. |
| Offline-first | Compilazione locale; rete solo per indice/pacchetti, con degrado se assente. |
| Trasparenza dati | Pacchetti nella cache standard ufficiale; progetti = `.typ` + risorse. |

---

## 6. Decisioni di design aperte (da confermare prima/durante la fase indicata)

1. **Motore anteprima (Fase 1).** *Raccomandato:* render nostro via `typst-svg`
   (controllo totale, nessuna dipendenza da Tinymist per il cuore). Alternativa:
   riusare il preview-server di Tinymist (WebSocket). → Si propone il render nostro.
   *Esito: **attuato*** — render nostro via `typst-svg` (F1-004, F5-008/F5-009).
2. **Framework frontend (Fase 0).** *Raccomandato:* Vanilla TS + Vite per
   leggerezza. Alternativa: Svelte/SolidJS se la UI dei pannelli diventa complessa.
   *Esito: **attuato*** — Vanilla TS + Vite (anche barra laterale e pannelli, F5-019).
3. **Completamento in Fase 1.** Usare subito `typst-ide` come base, oppure
   aspettare Tinymist in Fase 2. → Si propone `typst-ide` come base opzionale.
   *Esito (D-010, 2026-09-28):* `typst-ide` è l'assistenza di base, Tinymist
   un extra caricabile (X-001).
4. **Allineamento versioni (Fase 2).** Vedi §7: fissare la coppia
   compilatore-embeddato ↔ Tinymist compatibili sul medesimo livello di linguaggio.
   *Aggiornamento (D-010):* rilevante solo per X-001; si verifica al
   caricamento della versione indicata dall'utente.
5. **Strategia di conversione md↔typst (Fase 6) — CONFERMATA (2026-07-16):
   Opzione B, attuata in F6-001.**
   *Raccomandata (poi confermata dall'utente):* **Opzione B — conversione nativa in `typst-core`**
   (`pulldown-cmark` per md→typst; AST di `typst-syntax`, già disponibile, per
   typst→md): nessun binario esterno, convertitori nel core testabili headless,
   un solo artefatto leggero. Limite dichiarato: conversione lossy sul
   sottoinsieme di markup comune. Alternativa: Opzione A — Pandoc come sidecar
   (alta fedeltà md→typst, ma binario pesante e senza reader Typst maturo:
   typst→md resterebbe comunque da implementare a mano). Dettaglio, motivazioni
   e confronto in DECISIONI.md D-007.

---

## 7. Rischi tecnici emersi dalla ricerca e mitigazioni

| Rischio | Stato | Mitigazione |
|---------|-------|-------------|
| **Allineamento versioni Typst↔Tinymist.** I crate embeddati sono a 0.15; Tinymist viaggia su una propria cadenza. Disallineamento del linguaggio → comportamenti incoerenti tra anteprima e LSP. | Rilevante solo per X-001 (Tinymist extra opzionale, D-010) | Tabella di versioni di Tinymist allineate al livello di linguaggio dei crate embeddati, documentata in `DECISIONI.md`; controllo della versione al caricamento con avviso se non riconosciuta. ~~Se necessario, pinnare i crate alla versione che Tinymist supporta~~ → non più: con Tinymist opzionale è l'estensione ad adattarsi al core (D-010). |
| **`codemirror-lang-typst` sperimentale** (0.4.0, pochi star, basato su `typst-syntax` via WASM). | **Verificato — in uso** (0.4.0, `src/editor.ts`): import dinamico con fallback all'evidenziazione di default se il WASM non si carica; evidenziazione confermata nello smoke di produzione del 2026-07-18 (B-005) | Verificare l'allineamento a `typst-syntax`; fallback a grammatica TextMate/Lezer minimale o build WASM nostra di `typst-syntax`. |
| **Transport LSP `invoke`-based su Tauri non collaudato** pubblicamente. | Rilevante solo per X-001 (D-010); da prototipare prima dell'attuazione | Prototipo isolato prima dell'integrazione; fallback a WebSocket locale (plugin Tauri). |
| **`typst-as-lib` instabile** (wrapper non ufficiale). | Deciso | Non usarlo: implementare `World`/`IdeWorld` direttamente su `typst-kit` (riferimento: `SystemWorld` di typst-cli). |
| **Cambi all'indice/API di Universe.** | Continuo | Accesso isolato in `packages.rs`; degradare con grazia se l'indice è irraggiungibile (offline-first). |
| **Crescita cache `comemo`.** | Continuo | `comemo::evict(N)` dopo ogni compilazione. |
| **Conversione md↔typst lossy** (Fase 6): mismatch di espressività — funzioni Typst arbitrarie, math complessa e layout non hanno equivalente CommonMark. | Accettato per progetto | Puntare al sottoinsieme di markup comune, documentato; avviso all'utente sui costrutti non mappati; round-trip md→typst→md NON garantito. |
| **Pandoc pesante** (solo se in §6 punto 5 si scegliesse l'Opzione A): binario esterno grande, in tensione col vincolo di leggerezza; manca comunque un reader Typst maturo per il verso typst→md. | Evitato dalla raccomandazione | Preferire la conversione nativa nel core (Opzione B); rivalutare Pandoc solo per esigenze di fedeltà md→typst non coperte dal sottoinsieme. |

---

## 8. Convenzioni di sviluppo

- **Toolchain Rust bloccata** via `rust-toolchain.toml`; `Cargo.lock` versionato
  (equivalente Rust dell'ambiente isolato richiesto dalle convenzioni di progetto).
- Commenti e docstring **in italiano**.
- Codice Rust scritto/modificato tramite il subagent `c-rust-dev`; revisione con
  `code-reviewer` dopo ogni modifica significativa.
- Decisioni di design e bug noti tracciati in `DECISIONI.md`.
- Separazione netta: `typst-core` non importa mai `tauri`.

---

## 9. Primo passo operativo

Iniziare la **Fase 0** dallo scaffold del workspace e dal walking skeleton del
crate `typst-core` (World minimale + compile + un SVG di pagina), con un comando
Tauri `compila` e una UI di prova. Solo dopo aver verificato la catena
apri→compila→mostra si passa alla Fase 1.
```
1. cargo create-tauri-app  (Vanilla TS + Vite)
2. estrarre `typst-core` come crate del workspace
3. World minimale su typst-kit → compile<PagedDocument> → typst-svg
4. comando `compila` + render SVG nel webview
5. verifica con un .typ che importa un pacchetto @preview
```
