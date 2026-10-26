# Manuale di Typster

Versione 0.1.1 — per Windows.

Typster è un editor per [Typst](https://typst.app): scrivi a sinistra, vedi il documento
impaginato a destra. Questo manuale spiega come usarlo, dalle prime righe fino agli
strumenti per creare pagine. Se non conosci Typst, parti da un **modello** (capitolo 6.1):
non serve conoscere la sintassi per ottenere un documento curato.

## Indice

1. [Requisiti e primo avvio](#1-requisiti-e-primo-avvio)
2. [L'interfaccia](#2-linterfaccia)
3. [Scrivere](#3-scrivere)
4. [File, progetti e versioni](#4-file-progetti-e-versioni)
5. [La barra laterale](#5-la-barra-laterale)
6. [Gli Strumenti](#6-gli-strumenti)
7. [Esportare](#7-esportare)
8. [Aspetto e Opzioni](#8-aspetto-e-opzioni)
9. [Modalità focus e macchina da scrivere](#9-modalità-focus-e-macchina-da-scrivere)
10. [Scorciatoie da tastiera](#10-scorciatoie-da-tastiera)
11. [Dove sono i tuoi dati](#11-dove-sono-i-tuoi-dati)
12. [Problemi frequenti](#12-problemi-frequenti)

---

## 1. Requisiti e primo avvio

- Windows 10 o 11 a 64 bit, con il runtime *Microsoft Edge WebView2* (di norma già presente).
- Estrai **tutto** l'archivio in una cartella e avvia `typster.exe`. L'eseguibile e
  `WebView2Loader.dll` devono restare nella stessa cartella.
- L'eseguibile non è firmato: alla prima esecuzione Windows può mostrare «Windows ha
  protetto il PC». Scegli *Ulteriori informazioni → Esegui comunque*.
- I **pacchetti** di Typst Universe si scaricano alla prima compilazione che li usa: serve
  Internet solo quella volta.

All'avvio trovi un documento di esempio. Modificalo e guarda l'anteprima cambiare.

## 2. L'interfaccia

- **Barra in alto**: *Nuovo*, *Apri*, *Apri cartella*, *Salva*, *Esporta*, *Compila*,
  *Pacchetti*, *@local*, i menu *Aspetto* e *Opzioni* e il pulsante *→ Markdown*.
  Passando il mouse sui pulsanti compare la scorciatoia.
- **Barra laterale** (a sinistra): una colonna di icone — *Indice*, *File*, *Pacchetti*,
  *Strumenti* — e il pannello della sezione scelta. Si nasconde con `Ctrl+Maiusc+B`; il
  divisore ne regola la larghezza (anche con le frecce quando ha il fuoco).
- **Editor** (al centro): il testo del documento, con una barra di formattazione.
- **Anteprima** (a destra): il documento impaginato, pagina per pagina.
- **Riquadro errori** (sotto l'editor): gli errori e gli avvisi del compilatore, ognuno con
  la sua posizione. Un clic porta il cursore sul punto.
- **Pallino di salvataggio**: verde = salvato, ambra = modifiche non salvate, rosso =
  errore di scrittura (le tre condizioni hanno anche forme diverse, per chi non distingue i
  colori).
- **Palette comandi** (`Ctrl+P`): scrivi per cercare qualsiasi comando, incluse le
  esportazioni e i cambi di tema.

## 3. Scrivere

### 3.1 Typst e Markdown

Typster apre documenti **Typst** (`.typ`) e **Markdown** (`.md`). Un documento Markdown è
convertito in Typst per l'anteprima; il pulsante **→ Markdown** converte un documento Typst
in Markdown. La conversione conserva ciò che Markdown non può rappresentare in blocchi
speciali, così il ritorno non perde informazioni.

### 3.2 Aiuti alla scrittura

- **Completamento automatico**: mentre scrivi compaiono funzioni, parametri, tipi e
  simboli. Con l'*help* attivo, accanto a ogni voce c'è la **documentazione** e, per i
  parametri, il **tipo** accettato.
- **Firma della funzione**: dentro le parentesi di una chiamata (per esempio
  `#figure(`) compare sopra il cursore la firma con il **parametro che stai scrivendo** in
  grassetto.
- **Hover**: passando il mouse su un elemento si vede la sua descrizione.
- **Snippet**: scrivendo il prefisso di uno snippet si inserisce il modello di codice con i
  punti da compilare.
- **Barra di formattazione**: grassetto, corsivo, codice, titolo, liste, collegamento,
  immagine, tabella.
- Le due funzioni di assistenza (documentazione e firma) si attivano e disattivano in
  **Opzioni → Assistenza alla scrittura**.

### 3.3 Errori

Un errore di Typst compare sottolineato nell'editor e nel riquadro errori, con riga e
colonna. Gli errori che nascono in un pacchetto o in un file collegato puntano alla
chiamata nel documento principale, e le note con suggerimenti aiutano a correggere.

## 4. File, progetti e versioni

- **Nuovo** (`Ctrl+N`), **Apri** (`Ctrl+O`), **Salva** (`Ctrl+S`). Prima di sostituire un
  documento con modifiche non salvate l'app chiede se salvarle.
- **Apri cartella** (`Ctrl+Maiusc+O`): apre un **progetto** a più file (documenti, immagini,
  dati). Gli `#include`, le immagini e i file di dati si risolvono rispetto alla cartella.
  *Ricarica risorse del progetto* (dalla palette) rilegge i file dopo averli modificati
  fuori dall'app.
- **Salvataggio automatico**: si attiva e disattiva dalla palette (*Salvataggio automatico:
  attiva*); salva a riposo dopo l'ultima modifica, per i documenti che hanno già un file.
- **Cronologia versioni** (dalla palette): l'app conserva copie del documento e permette di
  ripristinarne una.
- **Modifiche esterne**: se un altro programma modifica o cancella il file aperto, l'app lo
  segnala e ti lascia scegliere quale versione tenere, senza sovrascrivere in silenzio.

## 5. La barra laterale

- **Indice** (`Ctrl+Maiusc+B` per aprire la barra): i titoli di **tutto** il documento
  compilato, anche quelli dei file inclusi. Scegliere una voce porta il cursore al titolo
  (o apre il file che lo contiene); il titolo sotto il cursore è evidenziato.
- **File** (`Ctrl+Maiusc+E`): i file della cartella-progetto; un clic apre il file.
- **Pacchetti**: l'elenco dei pacchetti di **Typst Universe**, con ricerca. Su ogni scheda,
  **Installa e importa** aggiunge il pacchetto al documento e, per i template, **Nuovo da
  template** crea un documento da quel template. L'app segnala i pacchetti usati dal
  documento che non sono aggiornati. *Pubblica come pacchetto @local* rende il documento
  corrente utilizzabile come pacchetto locale.
- **Strumenti**: vedi il capitolo successivo.

## 6. Gli Strumenti

Si aprono dall'icona ✦ in fondo alla colonna della barra laterale. La lista mostra gli
strumenti; scegline uno, e per tornare indietro usa «‹ Strumenti» in cima al pannello.
Gli strumenti **scrivono codice Typst nel documento** alla posizione del cursore (o al
posto della selezione) e ogni modifica si annulla con `Ctrl+Z` in un solo passo.

### 6.1 Modelli

Per i documenti che ripeti spesso (lettere, schede, relazioni).

1. *Strumenti → Modelli* (o, dalla palette, **Nuovo da modello…**).
2. Scegli un modello dall'elenco (c'è la ricerca). Sono forniti **Lettera**, **Scheda
   prodotto** e **Relazione breve**.
3. Compila il **modulo**: ogni campo corrisponde a una parte che cambia (destinatario,
   data, firma…). I campi *data* hanno il calendario, gli *elenchi* una voce per riga.
4. **Crea documento**: si apre un documento nuovo, *non salvato*, con i valori inseriti.

I testi che inserisci sono sempre trattati come **testo semplice**: caratteri come `#`, `*`
o `$` non vengono interpretati come codice.

**Salvare un tuo modello.** Scrivi il documento una volta, poi: seleziona nell'editor una
parte che cambia ogni volta → *Strumenti → Modelli → Salva il documento aperto come
modello…* → dai un nome al campo → **Rendi la selezione un campo** (se la stessa frase
compare più volte, puoi sostituirle tutte insieme). Ripeti per ogni parte variabile, poi
**Salva tra i modelli**. Se esiste già un tuo modello con quel nome, l'app chiede conferma
prima di sostituirlo. I tuoi modelli compaiono nell'elenco con «· tuo» e si possono
eliminare; quelli forniti con l'app no.

### 6.2 Pagina

Imposta formato carta (A3, A4, A5, Letter, Legal), orientamento, **colonne**, **margini**
(alto/basso e sinistra/destra, per esempio «2,5 cm»), **intestazione** e **numero di
pagina**. Un foglio in scala mostra i margini mentre li cambi. **Applica al documento**
riscrive solo la regola `#set page(...)` e lascia intatto ogni altro argomento che hai
scritto a mano (per esempio lo sfondo o il piè di pagina). Se il documento non ha la
regola, viene aggiunta in cima. Le impostazioni in forma avanzata (margini `top`/`left`,
intestazioni con codice) non si modificano da qui e il pannello lo segnala.

### 6.3 Inserisci

Produce il codice di:

- **Tabella**: righe, colonne e riga di intestazione.
- **Figura**: percorso dell'immagine, didascalia ed etichetta per citarla.
- **Equazione**: in linea o su una riga propria.
- **Riferimento a un'etichetta**: sceglie fra le etichette `<nome>` già presenti e inserisce
  `@nome`.
- **Nota a piè di pagina**.

*Attenzione:* in Typst un **titolo** o un'**equazione** si possono citare solo se sono
**numerati** (`#set heading(numbering: "1.")`, `#set math.equation(numbering: "(1)")`); lo
strumento Equazioni imposta la numerazione per te.

### 6.4 Equazioni

Per comporre formule senza ricordare la sintassi.

1. Scegli una **struttura** — frazione, radice, sommatoria o produttoria, integrale,
   limite, matrice, sistema di equazioni, vettore, coefficiente binomiale, accento — e
   compila i campi; **Aggiungi alla formula**. Puoi accumulare più strutture e anche
   modificare la formula a mano.
2. Scegli se la formula sta **su una riga propria**, dai un'**etichetta** (per esempio
   `eq:energia`) e **Inserisci nel documento**.
3. **Fisica**: derivata, derivata parziale, differenziale, gradiente, ket, bra e prodotto
   scalare usano il pacchetto *physica*; l'app aggiunge `#import` in cima al documento solo
   se manca.
4. **Incolla da LaTeX**: scrivi una formula LaTeX e viene inserita con il pacchetto *mitex*.
5. **Numerazione**: «Numera le equazioni (1)» e, a scelta, *equate* per numerare per riga e
   spezzare le formule lunghe fra più pagine. Si applicano con un blocco gestito
   dall'app in cima al documento (tra i commenti `// typster:equazioni`), che si riscrive
   o si toglie senza duplicarsi.

### 6.5 Grafici

1. Incolla una **tabella**: la prima colonna è X, le altre sono le serie. Separatori
   accettati: tabulazione, punto e virgola, spazi, o virgole (CSV). La **virgola decimale**
   all'italiana (`1,5`) è riconosciuta; l'intestazione si riconosce da sola.
   Sotto la tabella vedi subito «N righe, M serie» oppure l'errore con riga e colonna.
2. Scegli **linee**, **punti** o **barre**; aggiungi titolo, etichette degli assi, legenda
   con i nomi delle serie, didascalia ed etichetta.
3. **Inserisci nel documento**: il grafico usa il pacchetto *lilaq*.

Limiti della versione attuale: al massimo 2000 righe e 6 serie; con le barre e più serie, il
pacchetto le sovrappone (meglio linee o punti). Grafici di una funzione e diagrammi di
flusso non sono ancora disponibili.

### 6.6 Gantt

Per i diagrammi di Gantt (cronoprogrammi) con date reali, con il pacchetto *gantt-newgen* (incorporato in Typster).

1. **Attività:** per ognuna indica il **nome**, l'**inizio** e la **fine**; facoltativi il
   **gruppo**, la data di **completamento** e l'attività da cui **dipende** (il diagramma disegna
   una freccia). Le attività con lo stesso gruppo diventano le sotto-attività di quel gruppo, che
   compare come riga riassuntiva. Una nuova attività parte il giorno dopo la fine dell'ultima e
   dura una settimana; **+ Attività** ne aggiunge, **Rimuovi** ne toglie una (le dipendenze si
   riallineano).
2. **Pietre miliari:** nome, data e, se vuoi, la data visibile.
3. **Diagramma:** puoi fissare l'intervallo mostrato (altrimenti si ricava dalle date) e
   aggiungere didascalia ed etichetta (crea una figura citabile). Scegli le **intestazioni**
   dell'asse (mese e settimana, mese e settimana ISO, anno e trimestre, anno e mese, mese e giorno, settimana e giorno).
   La **linea «oggi»** è spenta di default: accesa, estende l'intervallo fino a oggi.
4. **Inserisci nel documento.** **Carica un esempio** riempie la scheda con un piccolo progetto per
   vedere come si compila.

Ogni dipendenza ha un tipo (fine-inizio, inizio-inizio, fine-fine, inizio-fine) e ogni attività
un avanzamento in percentuale. Il diagramma è in italiano e occupa la larghezza della pagina.

Il pacchetto *gantt-newgen* è incorporato nell'eseguibile (`@local/gantt-newgen:0.1.0`): non
serve la rete né l'installazione. È distribuito con licenza LGPL-3.0; il sorgente è nel repository
(`crates/typst-core/pacchetti/gantt-newgen`). Se installi una tua versione nella cartella dei
pacchetti `@local` di Typst, ha la precedenza su quella incorporata.

## 7. Esportare

**Esporta** (`Ctrl+E`) produce un **PDF**. Dalla palette comandi trovi anche:

| Formato | Note |
|---|---|
| PDF | Con le opzioni di esportazione: pagine da includere e standard (*Nessuno*, *PDF/A-2b*, *PDF/A-3b*, *PDF/A-4*). |
| PNG | Una immagine per pagina; si sceglie la risoluzione (predefinita 144 PPI, massimo 1200). |
| SVG | Una immagine vettoriale per pagina. |
| Testo semplice | Solo il testo del documento. |
| HTML | Pagina autonoma; le immagini del progetto vengono incorporate. |
| LaTeX | Conversione del documento; ciò che LaTeX non rappresenta genera un avviso. |
| Markdown | Conversione fedele; il ritorno da Markdown a Typst non perde informazioni. |

Le conversioni (testo, HTML, LaTeX, Markdown) segnalano in un elenco di **avvisi** le parti
che non hanno un equivalente esatto.

### 7.1 Stampare

Il pulsante **Stampa** (o *Stampa il documento…* dalla palette comandi) stampa il
documento **formattato**, pagina per pagina, senza passare dall'esportazione in PDF:

1. Typster compila la versione attuale se ci sono modifiche non ancora compilate. **Se il
   documento ha errori non stampa** (l'anteprima mostrerebbe una versione vecchia): correggili
   e riprova.
2. Carica tutte le pagine (anche quelle fuori vista) e imposta il formato carta del documento.
3. Si apre la finestra di stampa di Windows, dove scegli la stampante, le copie e l'intervallo
   di pagine; con **«Salva come PDF»** ottieni anche un PDF.

Una pagina del documento esce su un foglio, senza margini aggiunti (i margini sono quelli del
documento). Se il documento mescola pagine di formati diversi (per esempio una pagina orizzontale), ogni
pagina esce sul suo formato. Per un PDF con le opzioni di standard (PDF/A, intervalli) usa **Esporta**.

## 8. Aspetto e Opzioni

**Aspetto**: scegli fra tre **stili** — *Classico*, *Composizione* (carta, per chi scrive
testi) e *Strumento* (denso, da console di lavoro) — e il **tema** (sistema, chiaro,
scuro). La dimensione del font dell'editor si cambia con `Ctrl++`, `Ctrl+-` e `Ctrl+0`.

**Opzioni**: il **motore di evidenziazione Typst** (*Lezer*, predefinito, oppure *WASM*;
cambia solo l'evidenziazione e la modifica, non la compilazione) e l'**assistenza alla
scrittura** (documentazione accanto ai suggerimenti e firma delle funzioni).

Le scelte si salvano e si applicano subito, senza riavviare.

## 9. Modalità focus e macchina da scrivere

- **Modalità focus** (`Ctrl+Maiusc+D`): schiarisce tutto tranne il paragrafo in cui stai
  scrivendo.
- **Macchina da scrivere** (`Ctrl+Maiusc+T` o `F9`): la riga del cursore resta a metà
  altezza della finestra.

## 10. Scorciatoie da tastiera

| Azione | Scorciatoia |
|---|---|
| Palette comandi | `Ctrl+P` (o `Ctrl+Maiusc+P`) |
| Nuovo documento | `Ctrl+N` |
| Apri file | `Ctrl+O` |
| Apri cartella-progetto | `Ctrl+Maiusc+O` |
| Salva | `Ctrl+S` |
| Esporta in PDF | `Ctrl+E` |
| Mostra/nascondi barra laterale | `Ctrl+Maiusc+B` |
| Pannello File | `Ctrl+Maiusc+E` |
| Modalità focus | `Ctrl+Maiusc+D` |
| Macchina da scrivere | `Ctrl+Maiusc+T` o `F9` |
| Aumenta / riduci / ripristina il font | `Ctrl++` / `Ctrl+-` / `Ctrl+0` |
| Annulla un'inserzione degli Strumenti | `Ctrl+Z` |

Altri comandi (cronologia, esportazioni, Pacchetti, tema) sono nella palette.

## 11. Dove sono i tuoi dati

| Cosa | Dove (Windows) |
|---|---|
| Impostazioni, snippet | `%APPDATA%\typster\` |
| **I tuoi modelli** | `%APPDATA%\typster\modelli\` (file `.typ`, copiabili e condivisibili) |
| Cronologia delle versioni | `%APPDATA%\typster\cronologia\` |
| Indice dei pacchetti (cache) | `%LOCALAPPDATA%\typster\` |
| Pacchetti scaricati | nella cache standard di Typst |

Un modello è un normale file Typst: le righe iniziali che cominciano con `// @` descrivono
il nome e i campi (`// @campo: destinatario | testo | valore predefinito`), e nel testo
`{{destinatario}}` indica dove va il valore. Puoi scrivere o correggere un modello anche a
mano.

## 12. Problemi frequenti

- **L'app non parte o resta bianca**: controlla che `WebView2Loader.dll` sia nella stessa
  cartella di `typster.exe` e che il runtime *WebView2* sia installato.
- **«Windows ha protetto il PC»**: l'eseguibile non è firmato; vedi il capitolo 1.
- **Un pacchetto non si carica / errore alla prima compilazione**: i pacchetti si scaricano
  alla prima volta; verifica la connessione a Internet e riprova con *Compila*.
- **«cannot reference heading/equation without numbering»**: un titolo o un'equazione si
  citano solo se numerati; vedi 6.3.
- **Il salvataggio fallisce (pallino rosso)**: controlla i permessi della cartella e che il
  file non sia aperto in un altro programma.
- **Segnalare un problema**: apri una segnalazione sulla pagina del repository con i passi
  per riprodurlo e, se possibile, il documento che lo causa.

## Licenza

Typster è distribuito con licenza **MIT**: puoi usarlo, copiarlo, modificarlo e
ridistribuirlo, anche per scopi commerciali, a condizione di mantenere la nota di copyright
(«Copyright (c) 2026 Nexodux») e la licenza, cioè di citare l'autore. Il testo completo è nel
file `LICENSE`; i crediti del software di terze parti sono in `NOTICE` e nella cartella
`licenses`.
