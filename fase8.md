# fase8.md — Catalogo di riuso per le Fasi 5 e 7 (estratti COPIA con provenienza e licenza)

> **Cos'è questo file.** Catalogo del codice di terze parti riusabile nelle
> task della **Fase 5** (F5-xxx) e della **Fase 7** (F7-xxx) di
> [PIANO.md](PIANO.md). Il nome `fase8.md` è stato scelto dall'utente
> (2026-09-28) e **non** indica una "Fase 8" del piano. Prima si trovava in
> `sorgenti/riuso-fase5-7.md` (cartella ignorata da git); ora è in radice e
> versionato. Le proposte di modifica al contratto derivate da questo
> catalogo sono state accettate con DECISIONI.md **D-016**; gli obblighi di
> licenza di §0 sono attuati dalla task **F5-024** (NOTICE + `licenses/` +
> commento di derivazione su ogni blocco COPIA). Le task di PIANO.md citano
> le sezioni di questo file (es. "fase8.md §7").
>
> Materiale di **sola consultazione** preparato dall'analista (2026-09-28) per
> le task F5-xxx / F7-xxx di [PIANO.md](PIANO.md). Nulla qui è codice del
> progetto: la copia vera nei sorgenti avviene in fase develop, task per task,
> rispettando gli obblighi di licenza riportati in §0. Verdetti:
> **COPIA** = codice riusabile quasi 1:1 (stesso linguaggio/stack, licenza
> compatibile, versione verificata su Typst 0.15.0) · **ADATTA** = idea/
> algoritmo sì, codice no · **NIENTE** = nessuna soluzione utile.
> Le API dei crate Typst citate sono state verificate sul sorgente **0.15.0**
> presente nel registry locale di Cargo (`typst-svg`, `typst-pdf`, `typst-ide`,
> `typst-kit`, `typst-library`, `typst-layout`, `typst-syntax`), non su
> documentazione.

## 0. Obblighi di licenza (da rispettare al momento della copia)

| Fonte | Licenza | Obbligo quando si copia codice |
|-------|---------|--------------------------------|
| Typst (typst-cli, typst-kit, typst-pdf, typst-ide, typst-svg) — tag `v0.15.0`, commit `3ae52774b48987fc78a72ff483068cacc28e46c2` | Apache-2.0 (`LICENSE` + `NOTICE` nel repo) | Conservare l'avviso di copyright ("Copyright Typst contributors" / Apache-2.0), includere una copia della licenza Apache-2.0 nel progetto, segnalare nei file modificati che derivano da Typst (commento in testa al file o al blocco), riportare in un `NOTICE` di Typster la riga "Contiene codice derivato da Typst (https://github.com/typst/typst), Apache-2.0". Il `NOTICE` di Typst elenca terze parti (colors.css, rustup, rustc, rust-analyzer): **nessuna** riguarda gli estratti qui sotto, quindi non va ricopiato. |
| Tinymist — commit `f3b00eb553d6b7a35b5d59cce529c8d0d774cfce` (main, 2026-09-28; usa typst 0.15.1) | Apache-2.0 (`LICENSE`, nessun `NOTICE`) | Stessi obblighi di Apache-2.0: avviso "Copyright Myriad-Dreamin e contributori Tinymist", copia della licenza, nota di derivazione nei file. Verificare che l'API usata esista identica in 0.15.0 (fatto per gli estratti sotto). |
| ripgrep, crate `grep` 0.4.1 — commit `3fce3b5bb0236da2df6d99672afb8a719642eca7` | Unlicense **OR** MIT | Scegliendo MIT: conservare "Copyright (c) 2015 Andrew Gallant" + testo MIT accanto al codice copiato. Scegliendo Unlicense: nessun obbligo (pubblico dominio); citare comunque la fonte per cortesia. |
| deathau/cm-typewriter-scroll-obsidian — commit `625b42f188060d64a6ca3b5f4ace77e3adb1761a` | MIT (`package.json`) | Conservare l'avviso di copyright MIT (deathau) accanto al codice copiato. |
| davisriedel/obsidian-typewriter-mode — commit `3bed45b3ac665e083504d31fcbb6b0229633c752` | MIT | Solo idee (ADATTA): nessun obbligo. |
| MarkText — commit `34b5c2832380da3c0c24b50a4a0a203cfcde46bf` (develop) | MIT | Se si copia il tipo `CommandDescriptor`: avviso MIT (MarkText contributors). Il resto è ADATTA. |
| Muya (in `sorgenti/make_doc/muya/`) | MIT | Come da `sorgenti/README.md`. |
| TyX — commit `871dc2a7d270ec5ef038f46bde28d4db15732eb2` | MIT | **Nulla da copiare** (React/Lexical/Mantine, i18next; export con `PdfOptions::default()`): solo confronto. |
| Typstify — commit `df2790e8050aa603312cb41401773a725c16db1d` | Apache-2.0 | **Nulla da copiare** (Go + Gio; usa `typst` e `tinymist` esterni): solo idee (ADATTA), nessun obbligo. |
| Typora | closed source | **Nulla da copiare**: solo comportamento/UX osservato (focus/macchina da scrivere, F5-023). |

Regola pratica: un unico file `NOTICE` (o sezione del README) di Typster con
l'elenco delle derivazioni (Typst, Tinymist, ripgrep/grep, deathau) e una
cartella `licenses/` con Apache-2.0 e MIT; ogni blocco copiato porta in testa
un commento `// Derivato da <progetto> <file> (<commit>), licenza <X>`.

## 1. Tabella riassuntiva

| Task | Programma / fonte | File | Verdetto | Motivazione |
|------|-------------------|------|----------|-------------|
| F5-006 palette su registro | MarkText | `packages/desktop/src/renderer/src/commands/index.ts`, `components/commandPalette/index.vue` | ADATTA (tipo `CommandDescriptor` copiabile, §9) | Forma dati `{id, description, shortcut[], subcommands, execute, executeSubcommand, run/unload/search}` è quella richiesta da F5-006/F7-001; la UI è Vue. |
| F5-006 | TyX | `src/commands.ts`, `src/shortcuts.ts` | NIENTE | Comandi Lexical + Mousetrap `[scorciatoia, "comando parametro"]`: framework diverso, nessuna palette. |
| F5-006 | Typstify | `ui/palette/palette.go` | NIENTE | È la palette **colori** dei temi, non una palette comandi. |
| F5-008 render sola pagina visibile | typst-cli 0.15.0 | `crates/typst-cli/src/compile.rs` (`ExportCache`) | COPIA (§4) | Hash `typst::utils::hash128(page)` per pagina: salta la ri-serializzazione delle pagine invariate fra due compilazioni. |
| F5-008 | Tinymist/typst.ts | `crates/typst-preview/`, reflexo-vec2svg | ADATTA | Rendering SVG incrementale con diff dei frammenti: dipendenza pesante (reflexo), fuori scala per Typster. |
| F5-008 | Typstify, TyX | `ui/viewer/image_list.go`; typst.ts nel browser | NIENTE | Nessuna logica "solo pagine visibili". |
| F5-009 dedup glifi SVG | typst-svg 0.15.0 (già dipendenza) | `src/lib.rs` (`Deduplicator`, `DedupId(char, u128)`), `src/text.rs` | COPIA (fatto verificato, §4) | Gli id dei glifi sono `g<hash128(font, glyph_id, scala)>`: **stabili fra pagine e fra compilazioni** → dedup a valle banale, `svg_merged` non serve. |
| F5-011 export PNG/SVG/PDF-A | typst-cli 0.15.0 | `compile.rs` (`export_image`, `export_image_page`, `output_template`, `png_options`, `pdf_options`, `From<PdfStandard>`), `args.rs` (`Pages`, `parse_page_number`) | COPIA (§3) | Stesso linguaggio, stessa versione, stesse API (`typst_render::render`, `Pixmap::encode_png`, `typst_svg::svg`, `PdfOptions`). Gotcha `tagged` sotto. |
| F5-011 | Tinymist | `crates/tinymist-task/src/compute/pdf.rs` | conferma | Stesso mapping standard e stessa regola `tagged = !no_pdf_tags && pages.is_none()`. |
| F5-011 | Typstify | `typst/pdf.go` | ADATTA | Tabella "standard PDF/A ↔ versione PDF minima" e descrizioni umane per la UI (§3.4). |
| F5-011 | TyX | `src-tauri/src/pdf.rs` | NIENTE | `PdfOptions::default()`, nessun intervallo né standard. |
| F5-012 aggiornamenti pacchetti | typst-kit 0.15.0 (già dipendenza) | `src/packages.rs` (`FsPackages::latest_version`, `UniversePackages::latest_version`) | COPIA (chiamate API, §5) | Versione installata più alta = `max` delle sottocartelle parse→`PackageVersion` (che è `Ord`); indice → `max` per nome. |
| F5-012 (import del documento) | Tinymist | `crates/tinymist-query/src/package.rs` (`parse_package_import`, `find_package_and_latest`) | COPIA (§5) | 15 righe di solo `typst-syntax`: da nodo `Str` dentro `ModuleImport` a `PackageSpec`. |
| F5-012 | Typstify, TyX | `typst/pkg/*.go` | NIENTE | Typstify usa un indice proprietario (tpix); TyX non gestisce pacchetti. |
| F5-013 cronologia versioni | tutti | Typstify `ui/navpanel/history.go` (= progetti recenti), `editor/autosaver.go` (timer + contatore) | NIENTE | Nessun programma ha snapshot locali ripristinabili; l'autosaver Go è banale. |
| F5-020 indice dal compilato | typst-pdf 0.15.0 (già dipendenza) | `src/outline.rs` (`build_outline`) + `typst_library::model::OutlineNode::build_tree` | COPIA (§6) | Esattamente "titoli di tutto il documento dal documento compilato" (D-015): query `HeadingElem`, `resolve_level`, `outlined`, albero già costruito dalla libreria. |
| F5-020 | Tinymist | `crates/typst-preview/src/outline.rs` | COPIA (§6, parte "titolo + span") | `plain_text().trim()` del corpo, `body.span()` se lo span dell'elemento è detached, `introspector.position(loc)` → pagina/punto. API identica in 0.15.0 (verificata). |
| F5-020 | Typstify; Tinymist `document_symbol.rs` | LSP `documentSymbol` (sintattico) | NIENTE | Contratto vuole l'indice dal **compilato** (include/heading generati). |
| F5-022 file watching + anti-loop | typst-kit 0.15.0 (già dipendenza, feature `watcher`) | `src/watcher.rs` | COPIA / riuso diretto (§7) | Batching 100 ms / tetto 500 ms, poll 300 ms per file mancanti, workaround inotify su remove/rename, esclusione dell'output via `same_file`. |
| F5-022 | typst-cli 0.15.0 | `src/watch.rs` (ciclo `update(dependencies) → wait → reset → compile → evict`) | COPIA (§7) | `FileStore::dependencies()`/`reset()` sono **già** nel core (`typst_kit::files::FileStore`). |
| F5-022 | Tinymist | `crates/tinymist-project/src/watch.rs` (`WatchState`, `notify_entry_update`, `recheck_notify_event`) | ADATTA (§7.3) | Anti-loop per **contenuto** (`prev == next` → ignora) + stato transitorio `EmptyOrRemoval` con ricontrollo ≥ 50 ms: è il modello giusto per D-014; il codice dipende da reflexo vfs. |
| F5-022 | MarkText | `packages/desktop/src/main/filesystem/watcher.ts` (`ignoreChangedEvent`, `_shouldIgnoreEvent`) | ADATTA | Anti-loop a **finestra temporale** + ripiego su `stat().mtime` per i cloud drive; chokidar `awaitWriteFinish`. Meno robusto dell'impronta di contenuto. |
| F5-022 | Typstify | `service/filewatcher.go` | ADATTA | Osserva la **cartella padre** e filtra per file (sopravvive a cancella+ricrea); nessun anti-loop. |
| F5-023 focus / macchina da scrivere | deathau/cm-typewriter-scroll-obsidian | `extension.ts` (88 righe, CM6 puro) | COPIA (§8) | Facet offset, plugin di padding, scroll solo su `userEvent` consentiti, guardia anti-rientro, `scrollIntoView(head, {y:"start", yMargin})` in rAF. |
| F5-023 | davisriedel/obsidian-typewriter-mode | `src/cm6/plugin.ts` | ADATTA | Dimming per paragrafo/frase con decorazioni, ma 785 righe legate a Obsidian. |
| F5-023 | Muya | `lib/index.js` `setFocusMode`, `assets/styles/index.css` `.ag-focus-mode` | ADATTA | Focus = classe sul contenitore + opacità sui blocchi non attivi (in CM6: decorazioni di riga). |
| F5-023 | Typora | — | NIENTE (solo UX) | Focus mode e Typewriter mode come riferimento di comportamento. |
| F7-001 sottomenu | MarkText | `commandPalette/index.vue` (`executeCommand`) | ADATTA | Discesa: comando senza `execute` ma con `subcommands` diventa il livello corrente, query azzerata; manca il "Backspace per risalire" (da scrivere). |
| F7-003 ricerca nel progetto | ripgrep crate `grep` 0.4.1 | `crates/grep/examples/simplegrep.rs` | COPIA (§10) | `RegexMatcher::new_line_matcher`, `SearcherBuilder.binary_detection(quit(b'\0'))`, `search_path(matcher, path, sink)`; walk con `ignore::WalkBuilder`. |
| F7-003 | MarkText | `main/ipc/ripgrep.ts`, `renderer/src/node/ripgrepSearcher.ts` | ADATTA | Protocollo di streaming con `searchId`, eventi match/progress/done/error/cancelled, cancellazione, pulizia se il mittente muore: stesso disegno con `Channel` Tauri. |
| F7-003 | Typstify | `editor/search.go` | NIENTE | Ricerca nel buffer aperto, non nel progetto. |
| F7-004 i18n | TyX; Typstify | `src/translations/index.ts` (i18next); `i18n/localizer.go` (x/text) | NIENTE | Librerie/framework diversi dal `t()` fatto in casa deciso. |
| F7-004 | MarkText | `commandPalette/index.vue` (`handleLanguageChanged`) | ADATTA | Evento globale `language-changed` → le viste aperte riemettono i testi: è il "cambio immediato senza riavvio" di D-015. |
| F7-005 scheda parametri | typst-ide 0.15.0 (già dipendenza) | `src/complete.rs` (`complete_params`, `param_completions`), `src/analyze.rs` (`analyze_expr_with_fallback`, **pubblica**) | COPIA (§11) | Individua callee/args/set dal leaf, risolve il callee in `Func`, itera `func.params()` → `ParamInfo` (nome, docs, tipi, default, posizionale/nominale/variadico/settable). |
| F7-005 | Tinymist | `crates/tinymist-analysis/src/syntax/matcher.rs` (`arg_context`, `ArgClass`) | COPIA (§11) | Parametro attivo: nominale se il cursore è in `Named`/dopo `:`; altrimenti conta gli `ast::Arg::Pos` prima del cursore. Solo `typst-syntax`. |
| F7-005 | Tinymist | `crates/tinymist-query/src/signature_help.rs` | ADATTA | Costruzione dell'etichetta `nome: tipo`, `active_parameter` limitato a `pos_size-1` per i variadici, filtro `settable` nei `set`: dipende dal type system di tinymist. |
| F7-005 | TyX | `src/functions.ts`, `src/components/plugins/FunctionCallPlugin.tsx` | NIENTE | Catalogo di firme scritto a mano + modale React: concetto diverso (non deriva dal compilatore). |
| F7-006 menu `/` | Muya | `lib/ui/quickInsert/` (già in SPEC-UNIFICATA) | ADATTA | Nessuna novità da TyX/Typstify (nessuno slash menu). |
| F7-007 `sys.inputs` | typst-cli 0.15.0 | `src/world.rs` (Dict → `Library::builder().with_inputs`), `src/args.rs` (`parse_sys_input_pair`) | COPIA (§12) | Stessa API `with_inputs` in typst-library 0.15.0 (verificata). |
| F7-007 | Typstify | `typst/inputs.go` | ADATTA | File `sys-inputs.json` nella cartella progetto, `{nome: valore}` solo stringhe, creato solo su richiesta, JSON invalido → mappa vuota senza errore: conferma la forma decisa in D-014. |
| F5-010 debounce | Typstify | `utils/debouncer.go` | ADATTA | Debounce con `MaxInterval`: durante una raffica esegue comunque almeno una volta per periodo (evita la coda infinita). |

## 2. Estratti — convenzioni

Ogni estratto riporta: URL del file al commit indicato, righe, licenza. Il
codice è ridotto all'essenziale (import e commenti originali omessi salvo dove
servono); gli identificatori restano quelli originali per facilitare il
confronto. Da tradurre in italiano (commenti/nomi) al momento della copia,
come da convenzioni del progetto.

## 3. F5-011 — Export PNG/SVG/PDF-A (typst-cli v0.15.0, Apache-2.0)

Fonte: <https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/compile.rs>
e <https://github.com/typst/typst/blob/v0.15.0/crates/typst-cli/src/args.rs>.

### 3.1 Intervallo di pagine (`args.rs` righe 730-768)

```rust
/// Implements parsing of page ranges (`1-3`, `4`, `5-`, `-2`).
#[derive(Debug, Clone)]
pub struct Pages(pub RangeInclusive<Option<NonZeroUsize>>);

impl FromStr for Pages {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.split('-').map(str::trim).collect::<Vec<_>>().as_slice() {
            [] | [""] => Err("page export range must not be empty"),
            [single_page] => {
                let page_number = parse_page_number(single_page)?;
                Ok(Pages(Some(page_number)..=Some(page_number)))
            }
            ["", ""] => Err("page export range must have start or end"),
            [start, ""] => Ok(Pages(Some(parse_page_number(start)?)..=None)),
            ["", end] => Ok(Pages(None..=Some(parse_page_number(end)?))),
            [start, end] => {
                let start = parse_page_number(start)?;
                let end = parse_page_number(end)?;
                if start > end {
                    Err("page export range must end at a page after the start")
                } else {
                    Ok(Pages(Some(start)..=Some(end)))
                }
            }
            [_, _, _, ..] => Err("page export range must have a single hyphen"),
        }
    }
}

fn parse_page_number(value: &str) -> Result<NonZeroUsize, &'static str> {
    if value == "0" {
        Err("page numbers start at one")
    } else {
        NonZeroUsize::from_str(value).map_err(|_| "not a valid page number")
    }
}
```

Conversione al tipo del compilatore (`compile.rs` righe 144-146): la lista
`Vec<Pages>` (separatore `,`) diventa
`typst::layout::PageRanges::new(ranges.iter().map(|r| r.0.clone()).collect())`;
`PageRanges::includes_page_index(i)` filtra le pagine.

### 3.2 Regola `tagged` e standard (`compile.rs` righe 144-181, 600-625, 735-757)

```rust
let tagged = !args.no_pdf_tags && pages.is_none();
// "using --pages implies --no-pdf-tags": con un intervallo di pagine i tag
// di accessibilità vengono disattivati (la struttura logica sarebbe rotta).

let pdf_standards = PdfStandards::new(
    &args.pdf_standard.iter().copied().map(Into::into).collect::<Vec<_>>(),
)?; // errore leggibile se la combinazione è incompatibile

fn pdf_options(config: &CompileConfig) -> PdfOptions {
    // timestamp: locale con offset in minuti, altrimenti UTC se imposto da CLI
    PdfOptions {
        ident: Smart::Auto,
        creator: Smart::Auto,
        timestamp,
        page_ranges: config.pages.clone(),
        standards: config.pdf_standards.clone(),
        tagged: config.tagged,
        pretty: config.pretty,
    }
}

impl From<PdfStandard> for typst_pdf::PdfStandard {
    fn from(standard: PdfStandard) -> Self {
        match standard {
            PdfStandard::V_1_4 => typst_pdf::PdfStandard::V_1_4,
            PdfStandard::V_1_5 => typst_pdf::PdfStandard::V_1_5,
            PdfStandard::V_1_6 => typst_pdf::PdfStandard::V_1_6,
            PdfStandard::V_1_7 => typst_pdf::PdfStandard::V_1_7,
            PdfStandard::V_2_0 => typst_pdf::PdfStandard::V_2_0,
            PdfStandard::A_1b => typst_pdf::PdfStandard::A_1b,
            PdfStandard::A_1a => typst_pdf::PdfStandard::A_1a,
            PdfStandard::A_2b => typst_pdf::PdfStandard::A_2b,
            PdfStandard::A_2u => typst_pdf::PdfStandard::A_2u,
            PdfStandard::A_2a => typst_pdf::PdfStandard::A_2a,
            PdfStandard::A_3b => typst_pdf::PdfStandard::A_3b,
            PdfStandard::A_3u => typst_pdf::PdfStandard::A_3u,
            PdfStandard::A_3a => typst_pdf::PdfStandard::A_3a,
            PdfStandard::A_4 => typst_pdf::PdfStandard::A_4,
            PdfStandard::A_4f => typst_pdf::PdfStandard::A_4f,
            PdfStandard::A_4e => typst_pdf::PdfStandard::A_4e,
            PdfStandard::UA_1 => typst_pdf::PdfStandard::Ua_1,
        }
    }
}
```

Gotcha verificati su `typst-pdf` 0.15.0 `src/lib.rs`: `PdfOptions::default()`
ha `tagged: true`; `PdfStandards::new` rifiuta più di un PDF/A ("choose at most
one PDF/A standard") e più di un PDF/UA; le varianti `A_*a` e `Ua_1` impostano
validatori di accessibilità che **richiedono i tag** → con un intervallo di
pagine (tag spenti) vanno rifiutate a monte. La UI decisa (nessuno/2b/3b/4)
è compatibile con gli intervalli.

### 3.3 PNG e SVG per pagina (`compile.rs` righe 462-593, 628-640)

```rust
enum ImageExportFormat { Png, Svg }

fn export_image(document: &PagedDocument, config: &CompileConfig, fmt: ImageExportFormat)
    -> StrResult<Vec<Output>>
{
    let can_handle_multiple = /* il percorso contiene {p}, {0p} o {n} */;
    let exported_pages = document.pages().iter().enumerate()
        .filter(|(i, _)| config.pages.as_ref()
            .is_none_or(|ranges| ranges.includes_page_index(*i)))
        .collect::<Vec<_>>();
    if !can_handle_multiple && exported_pages.len() > 1 {
        bail!("cannot export multiple images without a page number template ({p}, {0p}) in the output path");
    }
    exported_pages.par_iter().map(|(i, page)| {
        let path = output_template::format(template, i + 1, document.pages().len());
        // In modalità watch: salta se ExportCache::is_cached(i, page) && path.exists()
        export_image_page(config, page, &Output::Path(path), fmt)?;
        Ok(...)
    }).collect()
}

mod output_template {
    const INDEXABLE: [&str; 3] = ["{p}", "{0p}", "{n}"];
    pub fn has_indexable_template(output: &str) -> bool {
        INDEXABLE.iter().any(|template| output.contains(template))
    }
    pub fn format(output: &str, this_page: usize, total_pages: usize) -> String {
        fn width(i: usize) -> usize { 1 + i.checked_ilog10().unwrap_or(0) as usize }
        let other_templates = ["{t}"];
        INDEXABLE.iter().chain(other_templates.iter()).fold(output.to_string(), |out, template| {
            let replacement = match *template {
                "{p}" => format!("{this_page}"),
                "{0p}" | "{n}" => format!("{:01$}", this_page, width(total_pages)),
                "{t}" => format!("{total_pages}"),
                _ => unreachable!("unhandled template placeholder {template}"),
            };
            out.replace(template, replacement.as_str())
        })
    }
}

fn export_image_page(config: &CompileConfig, page: &Page, output: &Output, fmt: ImageExportFormat)
    -> StrResult<()>
{
    match fmt {
        ImageExportFormat::Png => {
            let options = png_options(config);
            let pixmap = typst_render::render(page, &options);
            let buf = pixmap.encode_png()
                .map_err(|err| eco_format!("failed to encode PNG file ({err})"))?;
            output.write(&buf).map_err(|err| eco_format!("failed to write PNG file ({err})"))?;
        }
        ImageExportFormat::Svg => {
            let options = svg_options(config);
            let svg = typst_svg::svg(page, &options);
            output.write(svg.as_bytes()).map_err(|err| eco_format!("failed to write SVG file ({err})"))?;
        }
    }
    Ok(())
}

fn svg_options(config: &CompileConfig) -> SvgOptions {
    SvgOptions { render_bleed: false, pretty: config.pretty }
}

fn png_options(config: &CompileConfig) -> RenderOptions {
    RenderOptions { pixel_per_pt: Scalar::new(config.ppi / 72.0), render_bleed: false }
}
```

Note: `typst-render` esiste su crates.io in **0.15.0** (e 0.15.1); pinnare
`=0.15.0`. `encode_png()` è un metodo di `tiny_skia::Pixmap` restituito da
`typst_render::render`: non serve nominare `tiny-skia` come dipendenza diretta.
`ppi` di default della CLI: 144.

### 3.4 Tabella di compatibilità per la UI (Typstify `typst/pdf.go`, ADATTA)

Da <https://github.com/typstify/typstify/blob/df2790e8050aa603312cb41401773a725c16db1d/typst/pdf.go>
(Apache-2.0, Go — solo il contenuto informativo): PDF/A-1 ⇔ PDF 1.4;
PDF/A-2 e PDF/A-3 ⇔ PDF 1.7; PDF/UA-1 ⇔ PDF 1.7; PDF/A-4 ⇔ PDF 2.0. Utile per
il testo di aiuto della modale ("PDF/A-3b: come 2b, ma permette di allegare
file sorgente"); la validazione vera resta `PdfStandards::new`.

## 4. F5-008 / F5-009 — Pagina visibile e dedup glifi

### 4.1 Cache per pagina (`compile.rs` righe 647-672, Apache-2.0)

```rust
/// Caches exported files so that we can avoid re-exporting them if they haven't changed.
pub struct ExportCache {
    /// The hashes of last compilation's frames.
    pub cache: RwLock<Vec<u128>>,
}

impl ExportCache {
    pub fn new() -> Self { Self { cache: RwLock::new(Vec::with_capacity(32)) } }

    /// Returns true if the entry is cached and appends the new hash to the
    /// cache (for the next compilation).
    pub fn is_cached(&self, i: usize, page: &Page) -> bool {
        let hash = typst::utils::hash128(page);
        let mut cache = self.cache.upgradable_read();
        if i >= cache.len() {
            cache.with_upgraded(|cache| cache.push(hash));
            return false;
        }
        cache.with_upgraded(|cache| std::mem::replace(&mut cache[i], hash) == hash)
    }
}
```

Uso per F5-008: dopo `compila`, il core restituisce **metadati** (numero
pagine, dimensioni, hash per pagina) e l'SVG solo delle pagine richieste; il
frontend ri-chiede l'SVG di una pagina solo se il suo hash è cambiato o se
entra nel viewport (`IntersectionObserver`). `parking_lot::RwLock` è già
nell'albero; un `Mutex<Vec<u128>>` di std basta.

### 4.2 Dedup glifi: fatto verificato su `typst-svg` 0.15.0 (`src/lib.rs` 477-530, `src/text.rs` 55-95, `src/write.rs` 257-262)

- Ogni renderer (una chiamata a `typst_svg::svg(page, ..)`) ha un
  `Deduplicator<T>` per glifi (`'g'`), clip path (`'c'`), gradienti, tiling.
- L'id emesso è `DedupId(kind, hash128(key))` e viene scritto come
  `#g<hash>`; per i glifi vettoriali `key = (&font, glyph_id, scale)`, per
  quelli a immagine `key = (&font, glyph_id)`.
- Quindi **lo stesso glifo ha lo stesso id in ogni pagina e in ogni
  compilazione**: quando due pagine sono inlineate nello stesso DOM, il
  browser risolve `href="#g…"` sul primo `<symbol>` con quell'id, che è il
  glifo giusto. Oggi funziona per questo motivo (nessun bug latente).
- Dedup a valle (F5-009): tenere nel DOM un contenitore condiviso
  `<svg style="display:none"><defs>…</defs></svg>` e, all'inserimento di una
  pagina, spostare/scartare le `<symbol>`/`<clipPath>`/gradienti il cui id è
  già presente (un `Set<string>` degli id). Nessuna modifica al core, nessun
  `svg_merged` (che produce un unico SVG verticale e ricostruirebbe tutto a
  ogni pagina: incompatibile con F5-008). In alternativa il core può fare lo
  stesso filtro sul testo SVG prima di inviarlo (le `<defs>` sono in testa,
  gli id sono attributi `id="g…"` regolari).

## 5. F5-012 — Aggiornamenti pacchetti (typst-kit 0.15.0 + Tinymist)

### 5.1 API già disponibili (typst-kit 0.15.0 `src/packages.rs`, nessuna copia)

- `FsPackages::system_cache() -> Option<FsPackages>` (cache `@preview`),
  `FsPackages::system_data()` (dati `@local`).
- `FsPackages::latest_version(&VersionlessPackageSpec) -> Option<PackageVersion>`:
  `read_dir(<root>/<namespace>/<name>)`, parse dei nomi cartella in
  `PackageVersion`, `max()` (righe 199-215).
- `UniversePackages::latest_version(&VersionlessPackageSpec)`: scarica/usa
  `index.json` e prende `max` di `version` per `name` (righe 385-412); il
  progetto ha già la propria cache dell'indice (`packages.rs::elenca`,
  offline-first): usare quella per il confronto e non il downloader.
- `PackageVersion` è `Ord` (major, minor, patch): nessun crate `semver`.

Algoritmo F5-012: per ogni `<cache>/preview/<nome>/` → `installata =
latest_version(nome)`; `ultima = max versione di <nome> nell'indice in cache`;
segnala se `ultima > installata`.

### 5.2 Import del documento (Tinymist `crates/tinymist-query/src/package.rs` righe 55-74, Apache-2.0)

<https://github.com/Myriad-Dreamin/tinymist/blob/f3b00eb553d6b7a35b5d59cce529c8d0d774cfce/crates/tinymist-query/src/package.rs>

```rust
/// Parses a package import from a string literal node in an import statement.
pub fn parse_package_import(node: &LinkedNode) -> Option<PackageSpec> {
    if !matches!(node.kind(), SyntaxKind::Str) {
        return None;
    }
    let import_node = node.parent()?.cast::<ast::ModuleImport>()?;
    let ast::Expr::Str(str_node) = import_node.source() else {
        return None;
    };
    let import_str = str_node.get();
    if import_str.starts_with('@') {
        import_str.parse().ok()
    } else {
        None
    }
}
```

Per l'intero documento: visitare `Source::root()` e, per ogni nodo
`SyntaxKind::Str`, applicare la funzione; confronto con
`.max_by_key(|entry| entry.package.version)` sull'indice (stessa idea di
`find_package_and_latest`, righe 77-113).

## 6. F5-020 — Indice dal documento compilato (typst-pdf 0.15.0 + Tinymist, Apache-2.0)

### 6.1 Costruzione dell'albero (typst-pdf 0.15.0 `src/outline.rs` righe 11-39)

```rust
use typst_library::foundations::{NativeElement, Packed, StyleChain};
use typst_library::introspection::Introspector;
use typst_library::model::{HeadingElem, OutlineNode};

let elems = document.introspector().query(&HeadingElem::ELEM.select());

let flat = elems.iter().map(|elem| {
    let heading = elem.to_packed::<HeadingElem>().unwrap();
    let level = heading.resolve_level(StyleChain::default());
    // typst-pdf usa `bookmarked` (auto → outlined); per F5-020 usare
    // direttamente `outlined`, come fa `#outline` (D-015).
    let include = heading.outlined.get(StyleChain::default());
    (heading, level, include)
}).collect::<Vec<_>>();

// `OutlineNode { entry, level, children }` e `build_tree` sono pubblici in
// typst_library::model (0.15.0, righe 323-340): gestiscono livelli saltati e
// antenati esclusi.
let tree = OutlineNode::build_tree(flat);
```

### 6.2 Titolo, span, pagina (Tinymist `crates/typst-preview/src/outline.rs` righe 100-126, 158-183)

<https://github.com/Myriad-Dreamin/tinymist/blob/f3b00eb553d6b7a35b5d59cce529c8d0d774cfce/crates/typst-preview/src/outline.rs>

```rust
// posizione nel documento (pagina 1-based + punto in pt)
let loc = element.location().unwrap();
let pos = introspector.position(loc)
    .map(|pos| pos.as_paged_or_default())
    .unwrap_or(PagedPosition::ORIGIN);
// pos.page: NonZeroUsize, pos.point.x/y: Abs

// titolo in chiaro
let title = element.body.plain_text().trim().to_owned();

// span: prima quello dell'elemento, se detached quello del corpo
let span = element.span();
let span = if span.is_detached() { element.body.span() } else { span };
```

Da span a file/riga/colonna con il `World` del core: `span.id()` → `FileId`
(`VirtualRoot::Project` + `vpath`), `world.source(id)?.range(span)` → byte
range → `Source::byte_to_line/column` (già usato dalla sync F4-003 in
`world.rs::intervallo_utf16`). API verificate in typst-library 0.15.0:
`HeadingElem { level: Smart<NonZeroUsize>, depth, offset, outlined: bool,
bookmarked: Smart<bool>, body }`, `resolve_level(StyleChain)`,
`Introspector::{query, page, position}`, `Content::plain_text`.

## 7. F5-022 — Controllo file modificati (typst-kit 0.15.0, typst-cli, Tinymist)

### 7.1 Riuso diretto: `typst_kit::watcher::Watcher` (feature `watcher`)

`typst-kit` 0.15.0 `src/watcher.rs` (Apache-2.0) è già una dipendenza del
core: basta aggiungere `"watcher"` alle feature (porta `notify = "8"` e
`same-file = "1"`, `same-file` 1.0.6 è già nel `Cargo.lock`). API:
`Watcher::new(output: Option<PathBuf>)`, `update(paths)` (mark-and-sweep dei
path osservati, file inesistenti in `missing` con polling), `wait()` (blocca
fino a un evento rilevante). Costanti: `BATCH_TIMEOUT` 100 ms,
`STARVE_TIMEOUT` 500 ms, `POLL_INTERVAL` 300 ms. Limiti: osserva solo file
(non cartelle), non riporta *quale* file né il tipo di evento, un solo
`output` escluso. Per D-014 (rinomina, albero file, decisione per file) serve
un watcher proprio nel guscio: copiare da qui le parti sotto.

### 7.2 Parti da copiare (typst-kit `src/watcher.rs` righe 120-211)

```rust
/// Whether a kind of watch event is relevant for compilation.
fn is_relevant_event_kind(kind: &notify::EventKind) -> bool {
    match kind {
        notify::EventKind::Any => true,
        notify::EventKind::Access(_) => false,
        notify::EventKind::Create(_) => true,
        notify::EventKind::Modify(kind) => match kind {
            notify::event::ModifyKind::Any => true,
            notify::event::ModifyKind::Data(_) => true,
            notify::event::ModifyKind::Metadata(_) => false,
            notify::event::ModifyKind::Name(_) => true,
            notify::event::ModifyKind::Other => false,
        },
        notify::EventKind::Remove(_) => true,
        notify::EventKind::Other => false,
    }
}

// Batching degli eventi (wait): il primo evento, poi tutti quelli che
// arrivano entro BATCH_TIMEOUT l'uno dall'altro, senza superare STARVE_TIMEOUT
// totali ("helps against some editors' remove & move behavior").
let batch_start = Instant::now();
for event in first.into_iter()
    .chain(iter::from_fn(|| self.rx.recv_timeout(Self::BATCH_TIMEOUT).ok()))
    .take_while(|_| batch_start.elapsed() <= Self::STARVE_TIMEOUT)
{
    // Workaround per l'unwatch implicito di notify (inotify) su remove/rename:
    if matches!(event.kind,
        notify::EventKind::Remove(notify::event::RemoveKind::File)
        | notify::EventKind::Modify(notify::event::ModifyKind::Name(
            notify::event::RenameMode::From)))
    {
        for path in &event.paths { self.watcher.unwatch(path).ok(); self.watched.remove(path); }
    }
    // Ignora gli eventi che riguardano solo il file di output:
    if let Some(output) = &self.output
        && event.paths.iter().all(|path| is_same_file(path, output).unwrap_or(false))
    { continue; }
    relevant = true;
}
```

Configurazione: `notify::Config::default().with_poll_interval(300 ms)`,
`RecommendedWatcher::new(tx, config)`, `watch(path, RecursiveMode::NonRecursive)`.
Per l'albero dei file (F5-021) la cartella di progetto va osservata con
`RecursiveMode::Recursive` (Typstify osserva la cartella padre anche per i
singoli file: sopravvive a cancella+ricrea, `service/filewatcher.go` 39-56).

### 7.3 Ciclo di ricompilazione (typst-cli `src/watch.rs` righe 67-84)

```rust
loop {
    watcher.update(world.dependencies())?;   // tutti i file letti dall'ultima compilazione
    watcher.wait()?;
    world.reset();                          // FileStore::reset() — già nel core
    compile_once(world, &mut config)?;
    comemo::evict(10);
}
```

`world.dependencies()` = `FileStore::dependencies()` filtrato in path reali
(`loader.resolve(id)`): il core Typster usa già `typst_kit::files::FileStore`
(`world.rs` riga 26), quindi l'elenco dei file fratelli da osservare è gratis
(perimetro esatto di F5-014).

### 7.4 Anti-loop per contenuto e stato transitorio (Tinymist `crates/tinymist-project/src/watch.rs`, ADATTA)

<https://github.com/Myriad-Dreamin/tinymist/blob/f3b00eb553d6b7a35b5d59cce529c8d0d774cfce/crates/tinymist-project/src/watch.rs>
(righe 132-171 stati, 475-520 evento, 522-625 decisione, 628-668 ricontrollo).
Modello da riprodurre in Rust puro nel core (senza reflexo):

- per ogni file osservato: `prev: Option<Result<Bytes, FileError>>`, `state:
  Stable | EmptyOrRemoval { recheck_at, payload }`;
- a ogni evento rilevante si **rilegge il contenuto** e si confronta con
  `prev`: uguale → nessuna notifica (copre da solo i salvataggi propri,
  perché ciò che l'app ha scritto è ciò che rilegge);
- contenuto vuoto o file sparito da uno stato `Stable` → si entra in
  `EmptyOrRemoval`, si programma un ricontrollo (≥ 50 ms) e **non** si
  notifica subito: gli editor che troncano/rimuovono e poi riscrivono
  (salvataggio atomico con rinomina) producono un secondo evento che riporta
  a `Stable`; se al ricontrollo nessun evento successivo è arrivato
  (`recheck_at == tick dell'evento`), allora si notifica davvero la
  cancellazione/troncamento;
- errore uguale a quello precedente → ignorato.

Per D-014 questo si combina col registro delle scritture proprie (impronta
SHA-256 del contenuto scritto, F5-013/F5-022): evento su un percorso con
contenuto uguale all'impronta registrata → ignora; diverso → decisione
`ricarica | chiedi | invalida` in base allo stato del documento.

MarkText (`watcher.ts` 408-462, MIT) mostra il ripiego temporale: finestra di
`stabilityThreshold + 2·pollInterval` dopo `ignoreChangedEvent` e, se scaduta,
confronto di `stat().mtime` con l'istante del salvataggio (cloud drive che
riemettono l'evento in ritardo). Da tenere come **seconda** difesa, non come
prima.

## 8. F5-023 — Macchina da scrivere (deathau, MIT) e focus

### 8.1 `extension.ts` completo (88 righe, CM6 puro)

<https://github.com/deathau/cm-typewriter-scroll-obsidian/blob/625b42f188060d64a6ca3b5f4ace77e3adb1761a/extension.ts>
Copyright (c) deathau — MIT.

```ts
import { Extension, Transaction, Facet } from "@codemirror/state"
import { ViewPlugin, ViewUpdate, EditorView } from "@codemirror/view"

const allowedUserEvents = /^(select|input|delete|undo|redo)(\..+)?$/
const disallowedUserEvents = /^(select.pointer)$/

const typewriterOffset = Facet.define<number, number>({
  combine: values => values.length ? Math.min(...values) : 0.5
})

const typewriterScrollPaddingPlugin = ViewPlugin.fromClass(class {
  private topPadding: string = null;
  constructor(private view: EditorView) { }
  update(update: ViewUpdate) {
    const offset = (update.view.dom.clientHeight * update.view.state.facet(typewriterOffset))
      - (update.view.defaultLineHeight / 2)
    this.topPadding = offset + "px"
    if (this.topPadding != this.view.contentDOM.style.paddingTop) {
      this.view.contentDOM.style.paddingTop = this.topPadding
      this.view.contentDOM.style.paddingBottom = (update.view.dom.clientHeight - offset) + "px";
    }
  }
})

const typewriterScrollPlugin = ViewPlugin.fromClass(class {
  private myUpdate = false;
  constructor(private view: EditorView) { }
  update(update: ViewUpdate) {
    if (this.myUpdate) this.myUpdate = false;
    else {
      const userEvents = update.transactions.map(tr => tr.annotation(Transaction.userEvent))
      const isAllowed = userEvents.reduce<boolean>(
        (result, event) => result && allowedUserEvents.test(event) && !disallowedUserEvents.test(event),
        userEvents.length > 0
      );
      if (isAllowed) {
        this.myUpdate = true;
        this.centerOnHead(update)
      }
    }
  }
  async centerOnHead(update: ViewUpdate) {
    // can't update inside an update, so request the next animation frame
    window.requestAnimationFrame(() => {
      if (update.state.selection.ranges.length == 1) {
        const head = update.state.selection.main.head;
        const prevHead = update.startState.selection.main.head;
        if (prevHead != head) {
          let offset = (update.view.dom.clientHeight * update.view.state.facet(typewriterOffset))
            - (update.view.defaultLineHeight / 2);
          const effect = EditorView.scrollIntoView(head, { y: "start", yMargin: offset });
          const transaction = this.view.state.update({ effects: effect });
          this.view.dispatch(transaction)
        }
      }
    })
  }
})

export function typewriterScroll(options: {typewriterOffset?: number} = {}): Extension {
  return [
    options.typewriterOffset == null ? [] : typewriterOffset.of(options.typewriterOffset),
    typewriterScrollPaddingPlugin,
    typewriterScrollPlugin
  ]
}
```

Adattamenti previsti: il padding via `style` inline sul `contentDOM` è
compatibile con la CSP di produzione (B-005) perché è `element.style`, non un
`<style>` iniettato; togliere il padding quando la modalità si spegne
(`resetTypewriterSrcoll` nel repo fa questo); in Typster il tutto vive in un
`Compartment` (PIANO F5-023).

### 8.2 Focus (da scrivere, ~40 righe; riferimenti ADATTA)

Paragrafo = blocco fra righe vuote (PIANO F5-023). Un `ViewPlugin` con
`Decoration.line({class: "cm-sfocato"})` su ogni riga di `visibleRanges`
fuori dal paragrafo del cursore, ricalcolato su `docChanged | selectionSet |
viewportChanged`; stile nel tema CM6 (`opacity` + `transition`). Riferimenti:
davisriedel `src/cm6/plugin.ts` (decorazioni per frase,
`getActiveSentenceDecos`), Muya `.ag-focus-mode` (opacità sui blocchi non
`.ag-active`). Typora: F8 focus, F9 typewriter — solo comportamento.

## 9. F5-006 / F7-001 — Registro comandi (MarkText, MIT, ADATTA)

<https://github.com/marktext/marktext/blob/34b5c2832380da3c0c24b50a4a0a203cfcde46bf/packages/desktop/src/renderer/src/commands/index.ts>
(righe 18-54) — tipo copiabile:

```ts
export interface CommandSubcommand {
  id: string
  description?: string
  value?: unknown
  execute?: () => void | Promise<void>
}

export interface CommandDescriptor {
  id: string
  description?: string
  shortcut?: string[]
  subcommands?: CommandSubcommand[]
  execute?: () => void | Promise<void>
  executeSubcommand?: (commandId: string, value?: unknown) => void | Promise<void>
}
```

La palette (`commandPalette/index.vue`, righe 171-260) aggiunge ai comandi
`run(): Promise` (caricamento asincrono dei sottocomandi prima di mostrarli),
`unload()`, `search(q): Promise<items>` (ricerca delegata al comando, es.
quick-open sui file), `placeholder`; `executeCommand`: se il comando
selezionato non ha `execute` né `run` ma ha `subcommands`, diventa il livello
corrente (query azzerata, indice -1) — è la discesa di F7-001; l'evento
`language-changed` fa rieseguire `run()` e ricalcolare la lista (F7-004).
Per Typster: id gerarchici (`esporta.pdf`), titolo come chiave `t()`,
sottocomandi anche asincroni, "Backspace su campo vuoto = risali" da
aggiungere (MarkText non lo ha).

## 10. F7-003 — Ricerca nel progetto (crate `grep` 0.4.1, Unlicense OR MIT)

<https://github.com/BurntSushi/ripgrep/blob/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/grep/examples/simplegrep.rs>

```rust
use grep::{regex::RegexMatcher, searcher::{BinaryDetection, SearcherBuilder}};

let matcher = RegexMatcher::new_line_matcher(&pattern)?;      // regex per riga
let mut searcher = SearcherBuilder::new()
    .binary_detection(BinaryDetection::quit(b'\x00'))          // salta i binari
    .line_number(true)
    .build();
for dent in WalkDir::new(root) {                               // → ignore::WalkBuilder
    if !dent.file_type().is_file() { continue; }
    searcher.search_path(&matcher, dent.path(), sink)?;        // sink = Sink personalizzato
}
```

Per Typster: `grep-regex` (`RegexMatcherBuilder` con `case_insensitive`,
`word(true)` per "parola intera", `fixed_strings` per il letterale),
`grep-searcher` (`Sink::matched` riceve `SinkMatch { line_number, bytes }`;
restituire `Ok(false)` interrompe il file → annullamento con un `AtomicBool`
controllato nel sink e nel ciclo di visita), `ignore::WalkBuilder` (esclude
nascosti/`.gitignore`, `overrides` per le esclusioni di F5-021, `max_filesize`).
Versioni al 2026-09-28: `grep` 0.4.1, `grep-searcher` 0.1.17, `grep-regex`
0.1.14, `ignore` 0.4.33. I file con modifiche non salvate si cercano nel
buffer (D-015): `searcher.search_slice(&matcher, bytes, sink)`.

## 11. F7-005 — Scheda parametri (typst-ide 0.15.0 + Tinymist, Apache-2.0)

### 11.1 Individuare chiamata, callee e funzione (typst-ide 0.15.0 `src/complete.rs` righe 428-449, 495-515)

```rust
// Ensure that we are in a function call or set rule's argument list.
let (callee, set, args, args_linked) = if let Some(parent) = ctx.leaf.parent()
    && let Some(parent) = match parent.kind() {
        SyntaxKind::Named => parent.parent(),
        _ => Some(parent),
    }
    && let Some(args) = parent.get().cast::<ast::Args>()
    && let Some(grand) = parent.parent()
    && let Some(expr) = grand.get().cast::<ast::Expr>()
    && let set = matches!(expr, ast::Expr::SetRule(_))
    && let Some(callee) = match expr {
        ast::Expr::FuncCall(call) => Some(call.callee()),
        ast::Expr::SetRule(set) => Some(set.target()),
        _ => None,
    }
    && let Some(callee) = grand.find(callee.span())
{
    (callee, set, args, parent)
} else {
    return false;
};

// Risoluzione del callee in una funzione (analyze_expr_with_fallback è pub):
let Some(value) = analyze_expr_with_fallback(ctx.world, callee) else { return };
let Ok(func) = value.cast::<Func>() else { return };

// Argomenti già presenti (posizionali prima del cursore, nominali per nome):
let mut existing_positional = 0;
let mut existing_named = FxHashSet::default();
for arg in args.items() {
    match arg {
        ast::Arg::Pos(_) => {
            let Some(node) = args_linked.find(arg.span()) else { continue };
            if node.range().end < ctx.cursor { existing_positional += 1; }
        }
        ast::Arg::Named(named) => { existing_named.insert(named.name().as_str()); }
        _ => {}
    }
}
for param in func.params() {            // ParamInfo: name(), docs(), settable(), positional(), named(), variadic(), required()
    if set && !param.settable() { continue; }
    // ...
}
```

`ParamInfo` (typst-library 0.15.0 `foundations/func.rs`): `Native(&NativeParamInfo
{ name, docs, input: CastInfo, default: Option<fn() -> Value>, positional,
named, variadic, required, settable })` oppure `Closure(Spanned<ClosureParamInfo>)`
con `ClosureParamInfo::{Pos { name }, Sink { name }, Named { name, default }}`:
per le funzioni definite con `let` si hanno **nomi, tipo di parametro e default
dei nominali**, niente documentazione né tipi (perimetro di F7-005).
`CastInfo::describe()`/`Display` dà il testo del tipo accettato.
Il leaf al cursore: `LinkedNode::new(source.root()).leaf_at(cursor, Side::Before)`
(come già in `world.rs` per completamento e jump).

### 11.2 Parametro attivo (Tinymist `crates/tinymist-analysis/src/syntax/matcher.rs` righe 1442-1520, solo typst-syntax)

<https://github.com/Myriad-Dreamin/tinymist/blob/f3b00eb553d6b7a35b5d59cce529c8d0d774cfce/crates/tinymist-analysis/src/syntax/matcher.rs>

```rust
pub enum ArgClass<'a> {
    Positional { spreads: EcoVec<LinkedNode<'a>>, positional: usize, is_spread: bool },
    Named(LinkedNode<'a>),
}

fn arg_context<'a>(args_node: LinkedNode<'a>, mut node: LinkedNode<'a>) -> Option<ArgClass<'a>> {
    if node.kind() == SyntaxKind::RightParen {
        node = node.prev_sibling()?;
    }
    match node.kind() {
        SyntaxKind::Named => {
            let param_ident = node.cast::<ast::Named>()?.name();
            Some(ArgClass::Named(args_node.find(param_ident.span())?))
        }
        SyntaxKind::Colon => {
            let prev = node.prev_leaf()?;
            let param_ident = prev.cast::<ast::Ident>()?;
            Some(ArgClass::Named(args_node.find(param_ident.span())?))
        }
        _ => {
            // cursore dentro il valore di un argomento nominale → nominale
            if let Some(parent) = node.parent() && parent.kind() == SyntaxKind::Named {
                let param_ident = parent.cast::<ast::Named>()?;
                let init = parent.find(param_ident.expr().span())?;
                if init.range().contains(&node.offset()) {
                    return Some(ArgClass::Named(args_node.find(param_ident.name().span())?));
                }
            }
            // altrimenti: indice = numero di argomenti posizionali prima del cursore
            let mut spreads = EcoVec::new();
            let mut positional = 0;
            let is_spread = node.kind() == SyntaxKind::Spread;
            let args_before = args_node.children().take_while(|arg| arg.range().end <= node.offset());
            for ch in args_before {
                match ch.cast::<ast::Arg>() {
                    Some(ast::Arg::Pos(..)) => positional += 1,
                    Some(ast::Arg::Spread(..)) => spreads.push(ch),
                    Some(ast::Arg::Named(..)) | None => {}
                }
            }
            Some(ArgClass::Positional { spreads, positional, is_spread })
        }
    }
}
```

Da `signature_help.rs` (righe 55-108, ADATTA): etichetta `nome: tipo` per
parametro, `active_parameter` = indice del parametro (saltando i non
`settable` nei `set`), e per i posizionali `min(indice, numero_posizionali-1)`
così un variadico resta evidenziato oltre l'ultimo.

## 12. F7-007 — `sys.inputs` (typst-cli v0.15.0, Apache-2.0)

`src/world.rs` righe 56-68 e `src/args.rs` righe 800-810:

```rust
// Convert the input pairs to a dictionary.
let inputs: Dict = world_args.inputs.iter()
    .map(|(k, v)| (k.as_str().into(), v.as_str().into_value()))
    .collect();
let library = Library::builder().with_inputs(inputs).build();   // with_features(..) opzionale
// nel World: library: LazyHash::new(library)

/// Parses key/value pairs split by the first equal sign.
fn parse_sys_input_pair(raw: &str) -> Result<(String, String), String> {
    let (key, val) = raw.split_once('=')
        .ok_or("input must be a key and a value separated by an equal sign")?;
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err("the key was missing or empty".to_owned());
    }
    let val = val.trim().to_owned();
    Ok((key, val))
}
```

Verificato in typst-library 0.15.0 `src/lib.rs` riga 207:
`LibraryBuilder::with_inputs(mut self, inputs: Dict) -> Self`, `build()` riga
221. Nel core Typster: metodo che ricostruisce `libreria` con i nuovi input
(`LazyHash::new(...)`) e invalida `ultimo_documento`; stessa libreria per
anteprima ed export. Typstify (`typst/inputs.go`, ADATTA): file JSON piatto
`{ "nome": "valore" }` nella cartella progetto, creato solo alla prima
variabile, parsing tollerante (JSON invalido → nessuna variabile, nessun
errore bloccante) — coincide con D-014 (`typster-inputs.json`).
