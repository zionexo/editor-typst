# Spec unificata — il meglio di MarkText/Muya per editor-typst

> Specifica **additiva** derivata dall'analisi del motore Muya di MarkText
> (materiale in [`sorgenti/`](sorgenti/README.md)). Non riprogetta nulla: la
> fondazione architetturale resta in [PROGETTO.md](PROGETTO.md),
> [PIANO.md](PIANO.md) e [DECISIONI.md](DECISIONI.md). Qui si raccolgono i
> pattern di UX/UI riusabili di MarkText/Muya e li si mappa sulle fasi
> esistenti. Le decisioni vivono in DECISIONI.md.
>
> **Stato (2026-09-30): tutte le voci sono decise.** Palette gerarchica,
> floating UI, slash menu, sidebar (file-tree, outline, ricerca), file
> watching con anti-loop e i18n sono state accettate con **D-014** (task
> F5-019…F5-023 e F7-001…F7-007 di PIANO.md). Le restanti —
> reveal-markup-on-focus, auto-pair contestuale + Tab, toggle della toolbar,
> input rules, fence speciale, cursore `{key, offset}` — sono accettate
> nella direzione con **D-025** e stanno nel backlog "Dopo la Fase 7"
> (B-OP-04…B-OP-09), non pianificate. Il testo sotto è conservato come
> origine delle decisioni.

---

## 1. Scopo e relazione con gli altri documenti

Questo documento consolida due filoni nella nuova casa del progetto,
**`editor-typst/`** (prima `editor_typst/`):

- **editor-typst** — l'editor Typst già maturo: Tauri 2 + crate `typst-core`
  (Rust puro) + CodeMirror 6 + Tinymist come sidecar LSP. Fasi 0, 1, 3, 4, 6
  **completate**; Fase 2 (assistenza) **in corso**; Fase 5 (rifinitura) da
  fare. Tutto documentato in PROGETTO/PIANO/DECISIONI.

  > *Aggiornamento 2026-09-30:* Tinymist non è un sidecar ma un extra
  > caricabile (D-010, X-001); la Fase 2 è completata (2026-06-28). Testo
  > originale conservato per traccia.
- **make_doc** — l'analisi in profondità dell'editor **MarkText** e del suo
  motore WYSIWYG Markdown **Muya** (~25.700 LOC leggibili), ora archiviata in
  `sorgenti/make_doc/` come materiale di riferimento (vedi
  [`sorgenti/README.md`](sorgenti/README.md)).

Rapporto tra i documenti:

| Documento | Ruolo |
|-----------|-------|
| PROGETTO.md | Visione, architettura, roadmap a fasi. **Invariato.** |
| PIANO.md | Piano operativo, stack bloccato, DoD per fase. **Invariato.** |
| DECISIONI.md | Log delle decisioni prese (D-xxx, B-xxx, F*-xxx). **Unica fonte delle decisioni.** |
| **SPEC-UNIFICATA.md** (questo) | Raccolto UX/UI da MarkText/Muya, mappato sulle fasi. **Proposte.** |
| sorgenti/README.md | Provenienza e licenza del materiale di riferimento. |

Quando una proposta di questa spec viene accettata e implementata, la
decisione va registrata in DECISIONI.md come per ogni altro incremento; questa
spec non va tenuta "sincronizzata" a mano — è il punto di partenza, non il log.

---

## 2. Perché MarkText conferma la rotta

MarkText reimplementò un intero parser Markdown (fork di `marked` +
tokenizzatore inline custom) perché ne aveva bisogno e poteva permetterselo:

| Ragione valida per Markdown | Vale per Typst? |
|-----------------------------|-----------------|
| Grammatica piccola e sostanzialmente stabile (CommonMark) | **No** — linguaggio grande, con scripting integrato, in evoluzione attiva (pin `=0.15.0`, D-002). |
| Serviva un source-mapping a livello di token per il WYSIWYG | **No** — `typst-ide` fornisce già `jump_from_cursor`/`jump_from_click` (F4-003). |
| Rendering per-riga senza stato globale | **No** — Typst ha stato globale (`set`/`show`, contatori, riferimenti): un frammento non si rende in isolamento. |

Nessuna delle ragioni che giustificavano il parser custom di Muya vale per
Typst: questo **conferma per contrasto** la scelta fondativa di editor-typst
(D-001, D-003) di delegare tutto al compilatore ufficiale embeddato e di
tenere il sorgente testuale in CodeMirror 6 invece di un modello WYSIWYG.

**In una riga: da Muya si prende la UX di editing e il guscio, non il motore
documentale.**

---

## 3. Raccolto da MarkText/Muya, mappato sulle fasi

Verdetti: **RIUSA** = idea (e spesso struttura dell'implementazione)
trasferibile quasi 1:1 · **ADATTA** = l'idea sì, il codice no · **EVITA** =
non trasferire (vedi §4). Riferimenti relativi a `sorgenti/make_doc/`.

### 3.1 RIUSA

| Pattern | Fase | Nota per Typst | Riferimento |
|---------|------|----------------|-------------|
| **Reveal-markup-on-focus** — il modello tiene la sintassi completa; a ogni render `conflict(token.range, cursore)` decide se i delimitatori sono nascosti o resi in grigio in base a dove sta il cursore. | 1/2 | Marcatori `*…*`, `_…_`, `` `…` ``, `$…$` visibili solo quando li si edita. In CodeMirror 6 si realizza con decorazioni (replace/mark) pilotate dalla posizione del cursore, senza toccare il documento. | `muya/lib/parser/render/index.js:55-101` (checkConflicted/getClassName), `muya/lib/utils/index.js:36-38` (conflict) |
| **Auto-pair contestuale + "esci dal formato" (Tab)** — coppie `{}[]()$"~` con condizioni fini (niente pairing dentro code/math, non dopo alfanumerico); Tab salta oltre i delimitatori di chiusura del formato corrente. | 2 | Ancora più utile in Typst, dove `[]{}()$` sono strutturali. CodeMirror 6 ha già `closeBrackets` (attivo da F1-005): da qui si prendono le **condizioni contestuali** e il Tab-per-uscire. | `muya/lib/contentState/inputCtrl.js:182-273`, `muya/lib/contentState/tabCtrl.js:281-382` (checkCursorAtEndFormat) |
| **Slash menu / quick-insert** — menu data-driven per inserire costrutti, fuzzy match sul titolo, config `{label, title, subTitle, shortCut, icon}`. | 2/5 | Trigger `/` a inizio riga (in Typst `/` è libero nel markup); catalogo: `#figure`, `#table`, `$ … $`, `= heading`, liste, `@label`, `#image`. Inserisce snippet con tabstop — il supporto snippet CodeMirror c'è già (F2-003). | `muya/lib/ui/quickInsert/` (config.js + index.js) |
| **Command palette gerarchica** — `RootCommand` con subcommands e `search()` async; scorciatoie mostrate come `<kbd>`; indipendente dal contenuto: si cambia solo il catalogo. | 5 | Già prevista da PROGETTO.md §8 e PIANO.md Fase 5 (`Ctrl/Cmd+P`). Il pattern MarkText (app renderer, non Muya) dà la struttura dati: comandi componibili + ricerca asincrona. | app renderer MarkText (riferimento concettuale; non incluso in `sorgenti/`) |
| **Sistema di floating UI** — classi base `BaseFloat`/`BaseScrollFloat`: reference virtuale (oggetto con solo `getBoundingClientRect`) per ancorare al caret; dismiss unificato (Esc + click-fuori + scroll > 50 px); navigazione tastiera nelle liste. Micro-accorgimenti: `setTimeout(hide)` dopo la selezione (evita il doppio-Enter), hideTimer per i popover su hover. | 2/5 | È la **base comune** per slash menu, symbol picker e popup vari. Nel porting: Popper v2 (usato da Muya) → **Floating UI** (successore mantenuto). Nota: l'autocomplete LSP resta quello di `@codemirror/autocomplete` (F2-002), che ha già il suo popup. | `muya/lib/ui/baseFloat/index.js`, `muya/lib/ui/baseScrollFloat/index.js` |
| **Toolbar di formattazione che inserisce il markup** — formatPicker sulla selezione. | 2 | **Già fatta** in F2-001 (`src/formattazione.ts`). Da Muya si può prendere il refinement "toggle" (rimuovere il formato se già presente), oggi rimandato. | `muya/lib/ui/formatPicker/` |
| **File watching con anti-loop di auto-save** — chokidar con `awaitWriteFinish` + flag `ignoreChangedEvent` dopo il salvataggio, per non reagire al change generato dal salvataggio stesso. | 4 | Rilevante perché editor e compilatore toccano gli stessi file (progetti multi-file F4-001, edit live dei fratelli = refinement aperto). In Tauri: crate **`notify`** lato Rust; l'anti-loop è il pezzo che di solito manca. | pattern dell'app MarkText (main process); concetto, non codice in `sorgenti/` |
| **Ricerca full-text in streaming (ripgrep)** — `rg --json` in streaming, glob include/exclude, cancellazione della ricerca in corso, modalità file-picker. | 4/5 | "Find in project" per i progetti multi-file. In Tauri: sidecar `rg` o crate `grep` di ripgrep lato Rust. | pattern dell'app MarkText; concetto, non codice in `sorgenti/` |
| **Sidebar stile VS Code** — activity bar + pannelli (file-tree, ricerca, outline/TOC), larghezza resizable persistita. | 4/5 | Il pannello pacchetti drawer (F3-001) è il primo inquilino naturale. TOC costruibile dai heading `=` (via `typst-syntax`, già dipendenza diretta) o dall'outline del documento compilato. | pattern dell'app MarkText; `muya/lib/ui/fileIcons/` per le icone file |
| **Fence speciale → anteprima in container con cache/async/error-boundary** — placeholder di loading nel render sincrono, contenuto pesante iniettato dopo in async; cache per stringa-sorgente; try/catch → messaggio d'errore per singolo frammento. | 1/2 | È esattamente il modello dell'anteprima Typst: compilazione lazy con debounce, ultima anteprima valida conservata su errore. **editor-typst lo fa già** (F1-001, F1-007): il pattern Muya conferma la bontà dell'approccio, nessun lavoro nuovo. | `muya/lib/parser/render/index.js:179-248` (render/partialRender + cache mermaid/diagram) |

> *Nota di correzione (2026-09-28, DECISIONI.md D-015):* nella riga "Slash menu / quick-insert" l'affermazione "in Typst `/` è libero nel markup" è **inesatta**: `/ Termine: descrizione` a inizio riga è la **lista di termini** di Typst. Regola adottata in PIANO.md F7-006: `/` apre il menu, ma `/` seguito da spazio lo chiude e la riga resta una riga di lista di termini. Il testo originale della tabella resta invariato per tracciabilità.

### 3.2 ADATTA

| Pattern | Fase | Nota per Typst | Riferimento |
|---------|------|----------------|-------------|
| **Type-to-format (input rules)** — una regex composita riconosce `#␣`→H1, `- ␣`→lista, `> `→quote ecc. mentre si digita. | 2 (opzionale) | Il **meccanismo** (input rules su pattern a inizio riga) è riusabile; la **tabella va riscritta** per Typst: heading è `=` (non `#`, che introduce codice), liste `-`/`+` sono già markup nativo. Cautela: la sintassi Typst è ricorsiva, non regex-friendly — limitarsi a regole di inizio riga, mai provare a "capire" l'inline con regex. | `muya/lib/contentState/updateCtrl.js:4-16` (INLINE_UPDATE_FRAGMENTS) |
| **Cursore in coordinate stabili `{key, offset}` + marker-sentinella per il round-trip** — sentinelle (`CURSOR_ANCHOR_DNA`/`CURSOR_FOCUS_DNA`) iniettate nel testo prima di una ritrasformazione, poi ritrovate per riposizionare il cursore. | 4 (già coperto meglio) | editor-typst ha già `typst-ide` (`jump_from_cursor`/`jump_from_click`, F4-003), **superiore** perché lavora sul documento compilato. La sentinella resta un fallback concettuale per trasformazioni testuali dove non c'è un mapping (es. riposizionare il cursore dopo `converti md↔typst`, F6). | `muya/lib/selection/index.js`, `muya/lib/utils/importMarkdown.js:460-621` (getMuyaIndexCursor) |
| **i18n con vocabolario condiviso app↔editor, caricamento lazy per-locale** — funzione `t()` passata al motore, fallback alla chiave. | 5 (eventuale) | Buon pattern se si vorrà il multilingua; oggi l'app è in italiano e non è un requisito. Da valutare solo in Fase 5, senza framework pesanti. | `muya/lib/parser/render/index.js:181` (opzione `t` con fallback) |

### 3.3 EVITA

Vedi §4: per ognuna editor-typst ha già la scelta divergente giusta.

---

## 4. Cosa NON prendere, e perché editor-typst già diverge bene

| Pattern Muya da evitare | Perché collassa su Typst | Cosa fa già editor-typst |
|-------------------------|--------------------------|--------------------------|
| **Motore documentale a `span` markdown**, ri-tokenizzato per riga | La sintassi Typst è ricorsiva e con stato globale: la tokenizzazione per-riga non può essere corretta. | Sorgente testuale in **CodeMirror 6**; la verità sul documento è del compilatore (D-001, D-003). |
| **Undo a snapshot full-document + deepCopy** della history | Non scala: O(dimensione documento) a ogni passo. | CodeMirror 6 dà undo/redo incrementale via **ChangeSet** (in `basicSetup` da F1-005). |
| **Parser markdown/Typst fatto in casa** (fork di `marked` + inline lexer) | Rincorrere il linguaggio Typst a mano è il lavoro di una vita; §2 spiega perché per Markdown era sostenibile e qui no. | Il compilatore è la fonte di verità: crate **`typst-core`** su Typst 0.15 pinnato (D-002); persino md↔typst usa `pulldown-cmark`/`typst-syntax`, non parser propri (D-007). |
| **Prism per l'highlighting** dei blocchi codice | Prism non ha una grammatica Typst. | **`codemirror-lang-typst`** (WASM su `typst-syntax`, con fallback graceful — F1-005) + `@codemirror/lang-markdown` per i `.md` (F6-003). |
| **Export PDF via `printToPDF` di Chromium** | Rende l'HTML dell'anteprima, non il documento: perde la tipografia. | Export **dal compilatore**: `typst-pdf` (F1-006), `typst-render` per i raster. |

---

## 5. Backlog integrato per fase

Riassunto operativo: cosa aggiungerebbe questa spec alle fasi esistenti.

> **Esito (2026-09-30):** tutte le voci sono decise. Collocazione reale
> (le colonne "Fase" qui sotto riflettono lo stato del 2026-07 e non sono
> più aggiornate):
>
> | Voce | Decisione | Dove |
> |------|-----------|------|
> | Command palette gerarchica | D-014 | F5-006 + F7-001 |
> | Floating UI unificato | D-014 | F7-002 |
> | Slash menu | D-014, D-015 | F7-006 |
> | Sidebar (activity bar, file-tree, TOC) | D-014, D-015 | F5-019…F5-021 |
> | File watching `notify` + anti-loop | D-014, D-017 | F5-022 |
> | Ricerca nel progetto | D-014, D-016 | F7-003 (crate `grep`, niente sidecar `rg`) |
> | i18n con `t()` | D-014, D-015 | F7-004 |
> | Reveal-markup-on-focus | D-025 | backlog B-OP-04 |
> | Auto-pair contestuale + Tab | D-025 | backlog B-OP-05 |
> | Toggle sulla toolbar | D-025 | backlog B-OP-06 |
> | Input rules a inizio riga | D-025 | backlog B-OP-07 |
> | Fence speciale con anteprima | D-025 | backlog B-OP-08 |
> | Cursore `{key, offset}` + sentinelle | D-025 | backlog B-OP-09 |

Testo originale (proposte, 2026-07):

| Fase (stato) | Voci esistenti (PIANO.md) | Aggiunte proposte da MarkText/Muya |
|--------------|---------------------------|-------------------------------------|
| 1 — Editor base (**completata**) | — | Nessuna: il pattern anteprima lazy/cache/error-boundary (§3.1) è già in linea con F1-001/F1-007. Reveal-markup-on-focus collocabile qui concettualmente, ma da attuare in Fase 2. |
| 2 — Assistenza (**in corso**) | Sidecar Tinymist (enhancement), toolbar ✓, snippet ✓, completamento ✓ | **Reveal-markup-on-focus** (decorazioni CM6); **auto-pair contestuale + Tab esci-dal-formato**; **slash menu** data-driven su base floating UI; toggle sulla toolbar (refinement F2-001); *(opzionale, con cautela)* input rules a inizio riga. |
| 4 — Progetti (**completata**; refinement aperti) | Auto-save + cronologia (refinement), edit live file fratelli (refinement) | **File watching `notify` con anti-loop di auto-save** (abilita i due refinement aperti); **ricerca nel progetto via ripgrep**; **sidebar file-tree** per la cartella-progetto. |
| 5 — Rifinitura (**da fare**) | Palette comandi, temi, prestazioni, distribuzione | **Command palette gerarchica** (struttura `RootCommand` + search async); **sistema di floating UI unificato** (Floating UI) come base di palette/menu; **sidebar completa** (activity bar + pannelli + TOC); *(eventuale)* i18n con `t()` lazy. |

Fasi 0, 3 e 6: nessuna aggiunta — il raccolto MarkText non le tocca.

Ordine suggerito dentro la Fase 2 (coerente con lo stile a piccoli incrementi
D-004/D-005): auto-pair contestuale (rischio minimo, frontend puro) → slash
menu (riusa gli snippet F2-003) → reveal-markup-on-focus (il più delicato:
decorazioni + interazione con l'evidenziazione WASM).
