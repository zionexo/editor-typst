# Progetto: Typster — editor di testi basato su Typst

> Documento di visione e progettazione. Descrive obiettivi, architettura e
> roadmap di un editor leggero e facile da usare, costruito attorno al
> compilatore ufficiale **Typst** per sfruttarne l'intero ecosistema di
> pacchetti e template disponibili in rete.

---

## 1. Visione

Realizzare un editor di testi che renda **Typst accessibile a chiunque**, senza
nascondere la sua potenza. L'utente scrive in un'interfaccia pulita con
**anteprima dal vivo**, mentre l'editor resta un guscio sottile attorno al
compilatore ufficiale: tutta la resa, il layout e la tipografia sono delegati a
Typst, così da ereditare *gratis* ogni evoluzione del linguaggio e l'intera
libreria di pacchetti e template della comunità.

Due forze guidano ogni decisione:

- **Facilità d'uso** — chi non conosce Typst deve poter produrre un documento
  curato in pochi minuti, partendo da un template.
- **Leggerezza del codice** — non si reimplementa nulla di ciò che Typst già fa.
  L'editor orchestra, non duplica.

---

## 2. Principi guida

| Principio | Conseguenza pratica |
|-----------|---------------------|
| **Non reinventare Typst** | Il rendering è sempre delegato al compilatore ufficiale. Niente parser o motore tipografico custom. |
| **Sottile per scelta** | Ogni funzione che può vivere come pacchetto/template Typst *non* sta nel codice dell'editor. |
| **Ecosistema di prima classe** | Pacchetti e template di [Typst Universe](https://typst.app/universe) sono navigabili e installabili dall'interno dell'app. |
| **Separazione core / presentazione** | Logica documento (file, compilazione, progetto) separata dall'interfaccia. |
| **Configurazione esterna e versionata** | Impostazioni, template preferiti e snippet sono file leggibili e versionabili. |
| **Iterazione documentata** | Bug noti e decisioni di design tracciati nel repository. |

---

## 3. Cos'è Typst (in breve)

Typst è un sistema di *typesetting* moderno (alternativa a LaTeX) basato su un
linguaggio di markup leggibile con un linguaggio di scripting integrato.
Caratteristiche che ne fanno la base ideale:

- **Compilazione velocissima** e incrementale (motore scritto in Rust) →
  abilita l'anteprima dal vivo a ogni battitura.
- **Sintassi a markup** semplice (`= Titolo`, `*grassetto*`, `_corsivo_`),
  affiancata da funzioni componibili per casi avanzati.
- **Ecosistema di pacchetti** (Typst Universe): bibliografie, grafici, diagrammi
  (CeTZ, Fletcher), tabelle avanzate, loghi, fino a interi template di tesi,
  CV, lettere, presentazioni.
- **Output deterministico** in PDF, PNG e SVG.

L'editor punta a esporre questa potenza dietro un'interfaccia amichevole.

---

## 4. Funzionalità principali

### 4.1 Scrittura ed editing
- Editor di codice con **evidenziazione della sintassi** Typst.
- **Anteprima dal vivo** affiancata, aggiornata mentre si scrive (compilazione
  incrementale).
- **Sincronizzazione sorgente ↔ anteprima** (SyncTeX-like): clic sull'anteprima
  → cursore nel sorgente e viceversa.
- Numerazione righe, rientro automatico, chiusura di parentesi e delimitatori.
- **Autocompletamento** di funzioni, parametri e simboli sfruttando il
  *language server* ufficiale (Tinymist).
  *Nota (2026-09-28, DECISIONI.md D-010):* l'assistenza integrata è
  `typst-ide` embeddato; Tinymist è un extra opzionale caricabile, non
  incluso nell'app.
- Diagnostica inline: errori e avvisi del compilatore mostrati sulla riga.

### 4.2 Esperienza per principianti
- **Barra strumenti formattazione** (grassetto, corsivo, titoli, liste,
  tabelle, immagini) che inserisce il markup corretto: chi non conosce la
  sintassi non deve impararla per iniziare.
- **Galleria template** all'avvio: si parte da un documento pronto (relazione,
  tesi, CV, lettera, slide…).
- **Snippet** riutilizzabili e personalizzabili.
- Modalità "documento singolo" senza concetti di progetto per i casi più
  semplici.

### 4.3 Integrazione ecosistema (cuore del progetto)
- **Browser dei pacchetti** di Typst Universe: ricerca, anteprima del README,
  installazione con un clic, inserimento automatico dell'`import`.
- **Browser dei template**: crea un nuovo documento da un template della rete.
- Gestione della **cache locale dei pacchetti** (la stessa usata dal compilatore
  ufficiale, percorso standard `@preview`).
- Supporto a **pacchetti locali** dell'utente (`@local`) per riuso tra progetti.

### 4.4 Gestione documenti e progetti
- Apertura di file singoli o cartelle-progetto multi-file (`include`/`import`).
- Gestione **risorse** (immagini, font, dati `.csv`/`.json`/`.yaml`,
  bibliografie `.bib`/`.yaml`).
- **Esportazione** in PDF, PNG, SVG con scelta di pagine, DPI e profilo.
- Salvataggio automatico opzionale e cronologia delle versioni locale.

### 4.5 Modalità Markdown e interoperabilità md↔typst
- **Consapevolezza del tipo di documento:** l'editor distingue `.typ` (nativo)
  da `.md` (Markdown). Il tipo di documento guida (a) la modalità di
  evidenziazione dell'editor, (b) se convertire prima di compilare l'anteprima,
  (c) il formato di salvataggio. È un concetto a cavallo tra dominio (core) e
  presentazione.
- **Editor Markdown:** apertura e scrittura di file `.md` con evidenziazione
  dedicata.
- **Anteprima dal vivo anche per il Markdown**, senza introdurre un secondo
  motore di rendering: il documento viene convertito md→typst e compilato con
  la pipeline Typst già esistente (principio "orchestrare, non duplicare").
- **Conversione bidirezionale:** comandi "converti Markdown → Typst" e
  "converti Typst → Markdown". La conversione punta al **sottoinsieme di
  markup comune** ed è dichiaratamente **lossy** sui costrutti avanzati di
  Typst (funzioni arbitrarie, math complessa, layout): i costrutti non mappati
  vengono segnalati all'utente (vedi §11).

---

## 5. Architettura

```
┌─────────────────────────────────────────────────────────────┐
│                        INTERFACCIA (UI)                       │
│  Editor di codice · Anteprima · Toolbar · Browser pacchetti   │
└───────────────┬───────────────────────────┬─────────────────┘
                │                            │
        comandi/eventi                 frame renderizzati
                │                            │
┌───────────────▼───────────────────────────▼─────────────────┐
│                          CORE (logica)                        │
│  • Gestore documento/progetto (file, risorse, stato)          │
│  • Orchestratore di compilazione (debounce, incrementale)     │
│  • Gestore pacchetti/template (ricerca, install, cache)       │
│  • Configurazione (impostazioni, snippet, preferiti)          │
└───────────────┬───────────────────────────┬─────────────────┘
                │                            │
       compila / esporta             autocompletamento / diagnostica
                │                            │
┌───────────────▼──────────┐    ┌────────────▼────────────────┐
│   Compilatore Typst       │    │  Language Server (Tinymist) │
│   (ufficiale, embedded     │    │  completamento, hover,      │
│    o via CLI/IPC)          │    │  diagnostica, formattazione │
└───────────────────────────┘    └─────────────────────────────┘
                │
        ┌───────▼────────┐
        │  Typst Universe │  ← pacchetti e template della rete
        └────────────────┘
```

> *Nota (2026-09-28, DECISIONI.md D-010):* il riquadro "Language Server
> (Tinymist)" non è parte fissa dell'architettura: l'assistenza di base viene
> da `typst-ide` embeddato nel core; Tinymist è un componente aggiuntivo
> opzionale che l'app carica solo se l'utente lo installa/indica (PIANO.md
> X-001).

**Confine netto core / presentazione.** Il core non conosce widget né eventi
grafici: espone un'API a comandi (apri, compila, esporta, cerca pacchetto…) e
emette risultati (frame, diagnostica, elenco pacchetti). La UI è sostituibile
senza toccare la logica.

**Il compilatore non si reimplementa.** Si integra in una delle due modalità
(vedi §6), entrambe basate sul codice ufficiale.

---

## 6. Stack tecnologico — opzioni e raccomandazione

Il vincolo "leggero" e l'obiettivo "tutta la potenza di Typst" spingono verso
soluzioni che **incorporano il compilatore ufficiale** anziché dialogare con un
processo esterno generico.

### Opzione A — Desktop nativo con compilatore embedded *(raccomandata)*
- **Guscio:** [Tauri](https://tauri.app) (binario piccolo, UI web, backend Rust).
- **Compilatore:** Typst come *crate* Rust embedded → anteprima dal vivo
  velocissima e senza dipendenze esterne da installare.
- **Editor di testo:** [CodeMirror 6](https://codemirror.net) (leggero) con
  grammatica Typst.
- **Language server:** Tinymist via LSP per completamento e diagnostica.
  *Nota (2026-09-28, D-010):* nell'attuazione l'assistenza di base è
  `typst-ide` embeddato; Tinymist resta un extra opzionale, fuori dal
  bundle (il "solo binario" non lo include).
- **Pro:** leggero, veloce, un solo binario, accesso pieno alle API Typst.
- **Contro:** richiede competenze Rust per il backend.

### Opzione B — Web/PWA con Typst in WebAssembly
- **Compilatore:** [`typst.ts`](https://github.com/Myriad-Dreamin/typst.ts) (WASM).
- **Pro:** gira nel browser, zero installazione, ottimo per condivisione.
- **Contro:** accesso a font/file system più vincolato; pacchetti dalla rete
  da gestire con cura.

### Opzione C — Wrapper Python attorno alla CLI ufficiale
- **Compilatore:** `typst-cli` invocato come sottoprocesso (`typst watch`).
- **UI:** PySide6/Qt o interfaccia web locale.
- **Pro:** coerente con lo stack Python del team; rapido da prototipare.
- **Contro:** anteprima leggermente meno fluida (IPC vs embedded); doppia
  dipendenza (app + binario Typst).

> **Raccomandazione.** Per gli obiettivi dichiarati (massima fluidità + un solo
> artefatto leggero) l'**Opzione A (Tauri + Typst embedded)** è la più
> coerente. L'**Opzione C** è il percorso più rapido per un **prototipo
> funzionante** sfruttando competenze Python già presenti, da promuovere a B/A
> se servono distribuzione web o performance massime. La decisione va presa
> prima della Fase 1 e documentata.

---

## 7. Integrazione con l'ecosistema Typst

Questo è il valore distintivo dell'editor: rendere l'enorme libreria della
comunità **scopribile e usabile senza frizioni**.

- **Indice pacchetti/template:** lettura dell'indice pubblico di Typst Universe
  (metadati: nome, versione, descrizione, categorie, README).
- **Ricerca e filtri** per categoria (bibliografia, grafici, tabelle, CV,
  tesi, slide…).
- **Installazione trasparente:** scaricamento nella cache standard `@preview`,
  così i documenti restano compatibili con il compilatore ufficiale fuori
  dall'editor.
- **"Nuovo da template":** clona la struttura del template (file, risorse,
  esempi) in una cartella di lavoro.
- **Riuso personale:** pubblicazione di pacchetti `@local` per i propri stili
  ricorrenti (intestazioni aziendali, temi, macro).
- **Aggiornamenti:** notifica quando esiste una versione più recente di un
  pacchetto usato.

Principio: l'editor **non duplica** i contenuti dell'ecosistema, li **indicizza
e li collega**. Ogni nuovo pacchetto pubblicato in rete è immediatamente
disponibile senza aggiornare l'app.

---

## 8. Interfaccia ed esperienza d'uso

- **Layout a due riquadri** ridimensionabili: sorgente a sinistra, anteprima a
  destra (commutabile a tab su schermi piccoli).
- **Toolbar contestuale** per la formattazione comune.
- **Palette comandi** (`Ctrl/Cmd+P`) per ogni azione, ricercabile.
- **Pannello pacchetti/template** apribile a lato.
- **Tema chiaro/scuro** e dimensione font regolabile.
- **Zero configurazione obbligatoria:** si apre, si scrive, si vede il risultato.
- **Scorciatoie familiari** (salva, esporta, cerca, commenta riga).

Obiettivo di usabilità: *time-to-first-PDF* per un nuovo utente inferiore a
**due minuti** partendo da un template.

---

## 9. Requisiti non funzionali

- **Leggerezza:** binario/installazione contenuti; avvio rapido; basso consumo
  di memoria a riposo.
- **Reattività:** anteprima aggiornata entro poche centinaia di millisecondi su
  documenti tipici (compilazione incrementale + debounce).
- **Affidabilità:** un errore di compilazione non blocca l'editor; l'ultima
  anteprima valida resta visibile.
- **Portabilità:** Windows, macOS, Linux.
- **Offline-first:** scrittura e compilazione locali; la rete serve solo per
  scaricare pacchetti/template.
- **Trasparenza dei dati:** i progetti sono semplici file `.typ` + risorse,
  apribili e compilabili anche senza l'editor.

---

## 10. Roadmap a fasi

### Fase 0 — Decisione tecnica e walking skeleton
- Scelta dello stack (§6) e prototipo minimo: apri `.typ` → compila → mostra PDF.

### Fase 1 — Editor base usabile
- Evidenziazione sintassi, anteprima dal vivo, salvataggio, esportazione PDF.

### Fase 2 — Assistenza alla scrittura
- Integrazione Tinymist: autocompletamento, diagnostica inline, hover.
  *Nota (D-010):* realizzata con `typst-ide` embeddato; Tinymist è diventato
  un extra opzionale fuori dalle fasi.
- Toolbar di formattazione e snippet.

### Fase 3 — Ecosistema integrato
- Browser e installazione di pacchetti e template da Typst Universe.
- "Nuovo da template".

### Fase 4 — Progetti e produttività
- Multi-file, gestione risorse, sincronizzazione sorgente↔anteprima,
  cronologia versioni, pacchetti `@local`.

### Fase 5 — Rifinitura
- Temi, palette comandi, prestazioni, pacchettizzazione e distribuzione.
- Barra laterale (indice del documento, albero dei file, browser pacchetti),
  controllo dei file modificati da fuori, modalità focus e macchina da
  scrivere (accettate il 2026-09-28, DECISIONI.md D-014).

### Fase 6 — Editor Markdown e interoperabilità md↔typst
> **Stato: COMPLETATA (2026-07-16)** — conversione nativa nel core (D-007,
> Opzione B), comandi Tauri e frontend consapevole del tipo di documento.
> Dettaglio in [PIANO.md](PIANO.md) §4 e DECISIONI.md F6-001…F6-003; smoke
> test interattivo a runtime rinviato all'utente (il guscio si compila su
> toolchain GNU senza MSVC, vedi DECISIONI.md B-004).
- Tipo di documento (`.typ` | `.md`) come concetto di dominio; apertura e
  scrittura di `.md` con evidenziazione Markdown dedicata.
- Anteprima dal vivo del Markdown via conversione md→typst e riuso della
  pipeline di compilazione esistente (nessun secondo motore di rendering).
- Comandi di conversione bidirezionale md↔typst (lossy sul sottoinsieme
  comune, con avviso sui costrutti non mappati). Vedi §4.5 e
  [PIANO.md](PIANO.md) §4/§6.

### Fase 7 — Estensioni
> **Stato: PIANIFICATA (2026-09-28)** — istituita con DECISIONI.md D-014;
> task F7-001…F7-007 in [PIANO.md](PIANO.md) §4. Si avvia a Fase 5 completata.
- Palette comandi a sottomenu e menu `/` di inserimento rapido, su una base
  comune per menu e popup.
- Ricerca nel testo di tutto il progetto, nella barra laterale.
- Interfaccia multilingua (italiano + inglese).
- Scheda parametri delle funzioni durante la scrittura.
- Variabili `sys.inputs` definite dall'interfaccia e salvate nel progetto
  (documento singolo: nella cartella dati dell'app, D-016).

---

## 11. Rischi e mitigazioni

| Rischio | Mitigazione |
|---------|-------------|
| Cambi all'indice/API di Typst Universe | Astrarre l'accesso dietro un modulo dedicato; degradare con grazia se l'indice non è raggiungibile. |
| Evoluzione del linguaggio Typst | Bloccare/segnalare la versione del compilatore usata; aggiornamenti controllati. |
| Anteprima lenta su documenti grandi | Compilazione incrementale, debounce, render della sola pagina visibile. |
| Competenze Rust (Opzione A) | Iniziare con prototipo Opzione C in Python, migrare se necessario. |
| Sicurezza pacchetti di terzi | Usare la cache standard ufficiale; informare l'utente su cosa viene scaricato. |
| Conversione md↔typst lossy: mismatch di espressività tra i due linguaggi (Fase 6) | Puntare al sottoinsieme di markup comune, documentato; avviso all'utente sui costrutti non mappati; round-trip non garantito. |
| Pandoc come dipendenza pesante, se scelto per la conversione (Fase 6) | Raccomandata la conversione nativa nel core (vedi [PIANO.md](PIANO.md) §6); Pandoc rivalutabile solo come sidecar opzionale. |

---

## 12. Possibili estensioni future

- Collaborazione in tempo reale (multi-cursore).
- Esportazione verso formati aggiuntivi via pacchetti della comunità.
- Controllo ortografico/grammaticale integrato.
- Galleria di esempi e tutorial in-app per imparare Typst.
- Sincronizzazione cloud opzionale dei progetti.

---

## 13. In sintesi

Un **guscio sottile e amichevole** attorno al **compilatore Typst ufficiale**:
facile per chi inizia (template, toolbar, anteprima immediata), potente per chi
sa (accesso pieno al linguaggio e a **tutti** i pacchetti della rete), e
**leggero** per scelta architetturale — perché tutto ciò che Typst e la sua
comunità già fanno, l'editor non lo reimplementa: lo orchestra.
