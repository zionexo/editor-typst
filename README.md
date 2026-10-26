# Typster

**Un editor leggero per [Typst](https://typst.app) con anteprima dal vivo.**
Scrivi in un'interfaccia pulita, vedi subito il documento impaginato, parti da un
modello e inserisci tabelle, equazioni e grafici senza ricordare la sintassi.

Typster è un guscio sottile attorno al compilatore ufficiale di Typst: la resa, il
layout e la tipografia sono quelle di Typst, e l'intero ecosistema di pacchetti di
[Typst Universe](https://typst.app/universe) è raggiungibile dall'interno dell'app.

> Stato: **versione 0.1.1**, per Windows. L'app è in sviluppo attivo; vedi
> [RIPRESA.md](RIPRESA.md) per lo stato dei lavori.

## Cosa fa

- **Anteprima dal vivo** che si aggiorna mentre scrivi, con sincronizzazione fra il
  cursore nell'editor e la posizione nell'anteprima, ed errori mostrati nel punto in cui
  si trovano.
- **Editor** per documenti Typst e Markdown, con completamento automatico, suggerimento
  dei parametri e documentazione accanto alle voci, snippet e barra di formattazione.
- **Progetti a più file**, salvataggio automatico, cronologia delle versioni e rilevamento
  delle modifiche fatte da altri programmi.
- **Barra laterale** con Indice del documento, File del progetto, Pacchetti di Typst
  Universe e **Strumenti**.
- **Strumenti per creare pagine**: *Modelli* (documenti ripetitivi da un modulo, anche
  salvando i propri), *Pagina* (formato, margini, intestazione, numerazione), *Inserisci*
  (tabella, figura, equazione, riferimento, nota), *Equazioni* (strutture matematiche,
  pacchetti physica, mitex ed equate), *Grafici* (tabella incollata → grafico con lilaq) e
  *Gantt* (cronoprogrammi con attività, dipendenze e pietre miliari, con il pacchetto gantt-newgen, incorporato: funziona anche senza rete).
- **Esportazione** in PDF (anche PDF/A), PNG, SVG, testo semplice, HTML, LaTeX e Markdown.
- **Stampa** diretta del documento formattato (pulsante *Stampa*), con la
  finestra di stampa di Windows e «Salva come PDF».
- **Tre stili grafici** (Classico, Composizione, Strumento), tema chiaro/scuro/di sistema,
  modalità focus e macchina da scrivere.

Il manuale completo è in **[docs/manuale.md](docs/manuale.md)**.

## Installazione (Windows)

1. Scarica l'archivio `Typster-0.1.1-windows-x64.zip` dalla pagina
   **[Releases](../../releases)** del repository.
2. Estrai **tutto** il contenuto in una cartella a tua scelta (l'eseguibile e
   `WebView2Loader.dll` devono stare nella stessa cartella).
3. Avvia `typster.exe`.

**Requisiti:** Windows 10 o 11 a 64 bit e il runtime *Microsoft Edge WebView2* (già
presente su Windows 11 e sulla maggior parte dei Windows 10 aggiornati).

**Avviso di Windows SmartScreen:** l'eseguibile non è firmato digitalmente, quindi alla
prima esecuzione Windows può mostrare «Windows ha protetto il PC». Scegli *Ulteriori
informazioni → Esegui comunque*. Puoi verificare l'integrità del file confrontando lo
SHA-256 con quello riportato nel file `SHA256SUMS.txt` della release.

I pacchetti di Typst Universe (e quindi alcuni strumenti: Equazioni con physica, mitex ed
equate; Grafici con lilaq) si scaricano **alla prima compilazione** che li usa: serve la
connessione a Internet solo quella volta.

## Compilare dai sorgenti

Servono Rust, Node.js (con npm) e, su Windows, una toolchain C per Tauri.

```
npm ci
npm run check                           # verifiche della logica del frontend
npm run typecheck
cargo test -p typst-core -p typster     # test del core e del guscio
npm run tauri build -- --no-bundle      # eseguibile in target/release/typster.exe
```

Note: il progetto è predisposto per la toolchain `x86_64-pc-windows-msvc`
(`rust-toolchain.toml`); in sviluppo è stato compilato anche con una toolchain GNU
portabile, e in quel caso conviene impostare `CARGO_TARGET_DIR` su un percorso **senza
spazi**. Con la toolchain GNU l'eseguibile richiede `WebView2Loader.dll` accanto a sé.
Per sviluppare con ricarica a caldo: `npm run tauri dev`.

## Struttura del repository

```
crates/typst-core/   core in Rust (compilazione, esportazioni, conversioni, modelli, cronologia)
src-tauri/           guscio Tauri (comandi, finestra)
src/                 frontend TypeScript (editor CodeMirror 6, anteprima, pannelli, strumenti)
docs/                manuale dell'applicazione
DECISIONI.md         registro delle decisioni di progetto e dei difetti noti
PIANO.md, PROGETTO.md, SPEC-UNIFICATA.md   visione, specifiche e roadmap
RIPRESA.md           stato corrente e prossimi passi
NOTICE, licenses/    note e testi delle licenze di terze parti
```

## Licenza e crediti

**Typster è distribuito con licenza [MIT](LICENSE).** Puoi usare, copiare, modificare e
ridistribuire il codice, anche per scopi commerciali, **a condizione di citare l'autore**:
la nota di copyright («Copyright (c) 2026 Nexodux») e il testo della licenza vanno mantenuti
in tutte le copie e nelle parti sostanziali del codice. Il software è fornito «così com'è»,
senza garanzie.

Typster contiene codice derivato da Typst e da Tinymist (Apache-2.0), da MarkText e da
cm-typewriter-scroll-obsidian (MIT). Le note di derivazione e i testi delle licenze di terze
parti sono in [NOTICE](NOTICE) e nella cartella [licenses/](licenses/).
