# Ripresa — per la prossima sessione

> File di scambio, non di contratto: dice **dove siamo e cosa fare dopo**, non
> *perché*. Il perché sta in `DECISIONI.md` (registro append-only: cerca per
> data, non leggerlo tutto). Si sovrascrive a ogni aggiornamento. Leggilo prima
> di `PROGETTO.md`.

**Ultimo aggiornamento:** 2026-10-16 (sera). **Ciclo di vita:** il progetto non ha
il campo `Ciclo di vita`; di fatto è in **develop** dal 2026-09-29. Lo stato per fase è nelle note «Stato:» di `PIANO.md`.

## Leggi in questo ordine, se riprendi da zero

1. Questo file (2-3 minuti).
2. `DECISIONI.md`, voci del **2026-10-15 e 2026-10-16** (le ultime del file): motore
   lezer, Strumenti e Modelli (contratto, scelte del core, correzioni dopo
   revisione), Pagina, Inserisci, Equazioni, Grafici (piano e v1). Per il resto
   cerca per data.
3. La nota di build `build-test-env` (come compilare qui: toolchain GNU, target dir senza spazi).

## Stato in una riga

Fasi 0-4 e 6 completate, Fase 5 in chiusura, Fase 7 pianificata. Dal 2026-10-15:
motore Typst predefinito **lezer**, **help in linea** dell'autocompletamento e il
pannello **Strumenti** nella barra laterale (icona ✦) con cinque voci: **Modelli**,
**Pagina**, **Inserisci**, **Equazioni**, **Grafici** v1. Tutto **committato e
pubblicato** su GitHub (`origin/main` a `48a832d`, 2026-10-17). L'utente ha
provato sull'exe di produzione: help in linea, Modelli, Pagina («funziona»);
**Inserisci, Equazioni e Grafici non ancora provati dall'utente**.

## Cosa esiste (2026-10-15/16)

- **Core** `crates/typst-core/src/modelli.rs` + `crates/typst-core/modelli/*.typ`:
  modelli con intestazione `// @modello/@campo`, sostituzione `{{nome}}` con
  escaping, tetti, percorsi sicuri; modelli utente in `config_dir()/typster/modelli/`.
  Comandi Tauri: `elenca_modelli`, `compila_modello`, `salva_come_modello`
  (con `sovrascrivi`), `elimina_modello`. È l'**unico** strumento che passa dal core:
  Pagina, Inserisci, Equazioni e Grafici sono solo TypeScript.
- **Frontend** `src/pannelli/`: `strumenti.ts` (lista e viste), `strumenti-logica.ts`,
  `pagina-*`, `inserisci-*`, `equazioni-*`, `grafici-*`, `dom.ts`; stili in
  `src/stili/pannello-strumenti.css`; `src/firma.ts` (suggerimento dei parametri).
  Ogni logica pura ha il suo `*.check.ts` (elencati in `npm run check`).
- **Editor**: `selezione()`, `sostituisciIntervalli()`; `GestoreFile.nuovoDa()`;
  comando di palette «Nuovo da modello…». Equazioni e Grafici condividono
  `pianoInserimento` (import dei pacchetti solo se mancano).
- **Mock approvati** in `output/anteprime-menu/` (`indice.html`, `strumenti.html`;
  `output/` è in `.gitignore`).

## Verifica rapida che tutto sia ancora come descritto

Misurata il **2026-10-16** (PowerShell; prima `. ...\code_tools\rust_bundle\activate.ps1`
e `$env:CARGO_TARGET_DIR="C:\Users\<utente>\et_target"`):

```
cargo test -p typst-core -p typster                                   # misurato: 528 passed (2 ignored) + 17 + 7 (2 ignored) + 6, 0 failed
cargo test -p typst-core --test inserisci_snippet -- --ignored        # misurato: 2 passed (richiede la RETE: scarica lilaq, physica, mitex, equate)
cargo clippy -p typst-core -p typster --all-targets -- -D warnings    # misurato: exit 0
npm.cmd run typecheck                                                 # misurato: exit 0
npm.cmd run check                                                     # misurato: exit 0, 24 righe «OK»
npm.cmd run tauri build -- --no-bundle                                # misurato: exit 0 in ~6 min
```

Ultimo build di produzione 2026-10-26 11:59, **versione 0.1.1** (Stampa + scheda Gantt su gantt-newgen incorporato,
risincronizzato con la scorciatoia `week-iso` nelle intestazioni, D-150 del pacchetto; la scheda non la offre ancora):
`typster.exe` 41.762.304 byte, SHA-256 `425A8BAF87197F39CF32C7C29C210F69099006E308A5A8688B90620FF3789BC5`,
con `WebView2Loader.dll`. È l'**unico exe rimasto** nel progetto, in **`dist\`** (le altre copie in `output\`
sono state eliminate il 2026-10-26; restano archivio `output\release\*.zip` 0.1.0 e i `.msix` 0.1.0.0,
entrambi vecchi e da rigenerare). Prova di avvio: vivo a 14 s, chiusura pulita.
Versione 0.1.1 aggiornata in package.json, Cargo (core e guscio), tauri.conf.json, Package.appxmanifest
(0.1.1.0), README, manuale, scheda Store.
**Gantt** (2026-10-23): scheda del pannello Strumenti sul pacchetto dell'utente **gantt-newgen 0.1.0**
(LGPL-3.0), **incorporato nell'exe** come `@local/gantt-newgen:0.1.0` (offline; un pacchetto installato in
`@local` ha la precedenza). Sorgente in `crates/typst-core/pacchetti/gantt-newgen/` (aggiornare con
`tools\sincronizza-gantt-newgen.ps1`), `build.rs` + `pacchetti_incorporati.rs` + aggancio in `world.rs`.
Frontend: `gantt-logica.ts` + `gantt-vista.ts` (intestazioni, linea «oggi» spenta di default, tipi di
dipendenza, avanzamento). Sostituisce la versione gantty (DECISIONI, voce in coda). Test: 537+17+8+4+6
passano, clippy/fmt puliti, `npm run check`/`typecheck` ok; code-reviewer: nessun critico, corretti
ampiezza con «oggi», messaggio «Mostra fino a», anni 0-99, build.rs che fallisce se manca la cartella.
**Non** ancora provata dall'utente nell'exe.
Da sapere: il test di compilazione su questo PC può usare un gantt-newgen installato in `@local` invece
dell'incorporato (la priorità è provata a livello di caricatore in `world.rs`); la versione `0.1.0` è
duplicata a mano in `equazioni-logica.ts`.
**Attenzione:** `dist\` è la cartella di Vite: il prossimo `npm run build`/`tauri build` la
svuota (e fallisce con EPERM se `dist\typster.exe` è in esecuzione): chiudere l'app prima.
L'archivio di rilascio `output\release\` (0.1.0) contiene ancora l'exe PRECEDENTE, senza Stampa:
va rigenerato se si pubblica.
**Stampa** (2026-10-23): `src/stampa.ts` + `src/stili/stampa.css` + `anteprima.preparaStampa`;
usa `window.print()` di WebView2 sulle pagine SVG dell'anteprima; nessuna scorciatoia (Ctrl+P è
della palette, le combinazioni con Alt sono escluse); verificata con Edge headless su PDF di
prova (3 pagine A4 e formati misti), **non** ancora provata dall'utente nell'exe.
Se qualcosa non torna con questi numeri, **qualcosa è cambiato da allora**.

## Lezioni da non riscoprire

- **Escape nella shell**: i backslash nei comandi Bash/heredoc/Python vengono
  mangiati (`\[` → `[`, `\n` → vero a capo dentro una regex). Per testi con escape
  usare lo strumento Edit/Write o stringhe grezze (`r#"…"#` in Rust); verificare
  sempre il risultato con Grep.
- **Scrivere un file dopo averlo modificato**: Write fallisce se il file è cambiato
  dopo la lettura. Un commit lanciato nello stesso comando parte comunque e può
  dichiarare un lavoro non fatto (successo con RIPRESA): controllare l'esito prima.
- **Typst**: un titolo e un'equazione si possono citare (`@x`) **solo se numerati**.
  I commenti `/** … */` non devono contenere `*/` (rompe node `--experimental-strip-types`).
- **Asset dell'exe compressi**: cercare una stringa nell'exe non dimostra nulla;
  controllare `dist/assets/*.js` (stessa build) e l'ora del file.
- **Parser WASM desincronizzato**: il predefinito è `lezer` per questo.
- **`dist/`** non è un posto per l'exe e non va usata come cartella di lavoro; se
  l'exe è aperto la copia in `output\Typster-prova\` fallisce (chiuderlo o avviare
  da `et_target\release`). Senza `activate.ps1` `cargo` non è nel PATH.
- **Prove GUI con Edge headless + CDP sono fragili**; il test decisivo è l'exe di
  produzione (B-005, CSP).
- Una prova che non può fallire non prova nulla (un `|| true` in un check ne ha
  nascosto uno): ogni correzione è stata verificata facendo fallire il test prima.

## Prossimo passo (dopo il reset della quota)

1. **Prova dell'utente sull'exe** di Inserisci, Equazioni e Grafici. Elenco di cosa
   provare: nella conversazione del 2026-10-16 (strutture native, pacchetti con la
   rete, blocco di numerazione, «Incolla da LaTeX», grafico da tabella con didascalia).
2. **Comando di core per l'anteprima** (compila un documento di prova in un World
   separato e restituisce l'immagine): serve alle Equazioni (anteprima della formula)
   e ai Grafici; va fatto **una volta sola**, con `c-rust-dev`. Costo alto: da fare
   a quota piena.
3. **Grafici**, resto del piano: funzione `y = f(x)`, diagrammi di flusso (fletcher),
   grafi (diagraph), CSV da file del progetto, barre raggruppate per più serie.
4. **Strumento «Scienza»** (unify/zero: numeri con unità), richiesto dall'utente.
5. **Freccia accanto a «Nuovo»** con «Nuovo da modello…» (oggi solo da palette).
6. Più avanti: frontespizio e indici, controlli sul documento (etichette senza
   riferimento, immagini mancanti), modelli di pacchetto Typst Universe.

## Cose da decidere, non da fare da soli

- **Pubblicazione della versione 0.1.0**: `README.md`, `docs/manuale.md` e `LICENSE` sono
  committati in locale; il pacchetto di rilascio è pronto in `output/release/`
  (`Typster-0.1.0-windows-x64.zip` 20 MB con `LICENSE`, `SHA256SUMS.txt`,
  `NOTE-RILASCIO.md` con i passi per pubblicarlo da GitHub). **Licenza decisa
  (2026-10-18): MIT, «Copyright (c) 2026 Nexodux»** (uso libero con obbligo di citare
  l'autore). Da decidere: **(a)** il repository è **privato** (API anonima → 404):
  rendere pubblico?; **(b)** pubblicare la release (nessuna `gh` installata: si fa dal
  sito); **(c)** **firma di Windows**: nessun certificato né `signtool` su questa
  macchina. **Strada scelta il 2026-10-18: Microsoft Store con pacchetto MSIX** (Microsoft
  ri-firma il pacchetto dopo la certificazione, ≤3 giorni lavorativi; nessun certificato da
  comprare; account sviluppatore gratuito, **individuale** per «Nexodux», email
  dell'utente non scritta nel repo). Verificato sulle pagine Microsoft: Azure Artifact
  Signing non è disponibile ai privati in Italia (solo USA/Canada); SignPath richiede
  repo pubblico, build su CI e reputazione; l'EV non salta più SmartScreen dal 2024.
  **Stato**: `packaging/msix/` (manifesto con Name/Publisher **segnaposto**, icone,
  `build-msix.ps1`) produce `output/msix/Nexodux.Typster_0.1.0.0_x64.msix` (20,7 MB, non
  firmato, contenuto verificato). **Il PC di sviluppo non può attivare la modalità
  sviluppatore (criteri): la prova si fa su un altro PC**
  (scheda Store in `docs/store-scheda.md`). `winapp` CLI 0.7.1 installata con winget
  (`winget uninstall Microsoft.WinAppCli` per toglierla). **Non provato in esecuzione**:
  `winapp run` si ferma perché la **modalità sviluppatore di Windows è spenta** (serve
  l'utente: Impostazioni → Privacy e sicurezza → Per sviluppatori; può essere bloccata da
  criteri aziendali). Da verificare quando si avvia: avvio dell'app impacchettata, **dove
  finiscono le scritture in `%APPDATA%`** (impostazioni, cronologia, modelli: gli MSIX
  possono reindirizzarle), cartella dei pacchetti Typst, `runFullTrust`.
  **Per l'utente**: aprire l'account su `storedeveloper.microsoft.com` (verifica con
  documento e selfie), riservare il nome «Typster» (verificare che sia libero), poi
  sostituire Name/Publisher/PublisherDisplayName nel manifesto con i valori di Partner
  Center; servono anche schermate dell'app, informativa privacy, questionario età.
  Il build per lo Store andrebbe fatto con MSVC (oggi exe GNU + `WebView2Loader.dll`);
  **(d)** push dei commit locali.
  L'exe in `output/release/` è quello con SHA-256 `d82a5282…`, codice a `1457475`.
- **Gara nella riapertura dello stesso file** — se si riapre il file già aperto
  mentre compila e un altro programma lo scrive, il salvataggio può sovrascrivere
  la versione esterna (fix piccolo proposto: riapertura via `ricaricaDaDisco`).
- **LaTeX non compilato** (nessun compilatore sulla macchina) e **PDF/A-UA solo
  con marcatori** (nessun veraPDF): installare strumenti di verifica è una scelta
  dell'utente.
- **Comando di sola lettura nel guscio** per i percorsi non canonici (cambia
  l'IPC): rimandato.
- **Aggiornamento di Impeccable** (v4.6.0 disponibile): `npx impeccable update` NON si applica («nessuna
  cartella di skill trovata»; non lanciare `install`, farebbe una copia doppia). Va
  aggiornato dal gestore dei plugin dell'app (`/plugin` in una sessione interattiva).
