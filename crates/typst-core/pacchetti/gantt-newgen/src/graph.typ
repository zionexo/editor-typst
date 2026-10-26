// gantt-newgen — src/graph.typ
// Grafo delle dipendenze fra nodi (RF10): indice degli id (E-DEP-01),
// risoluzione di ogni dipendenza (id sconosciuto E-DEP-02 con
// suggerimento, antenato/discendente/se' stesso E-DEP-03, nodo senza
// date E-DEP-05), rilevazione dei cicli (E-DEP-04, visita ITERATIVA a
// tre colori: RF10) e ordine topologico (base del percorso critico,
// v0.2, ricavato dalla STESSA visita, terza revisione: vedi
// `_visita-grafo`). Core puro: lavora sull'albero GIA' validato da
// `validate.typ` (date ancora grezze, D-050; `type` delle dipendenze
// gia' canonicalizzato a fs/ss/ff/sf); non disegna nulla.
//
// Implementazione indipendente (D-032): gantty 0.5.1 risolve le
// dipendenze con `id-mappings.at(id)` (id sconosciuto = panic di
// dizionario "nudo", senza percorso ne' codice), sovrascrive id
// duplicati in silenzio (`ids.insert(task.id, id)` nell'ordine di
// visita, l'ultimo vince) e non rileva i cicli — letto come riferimento
// funzionale, non eseguito, non trascritto (D-032).
//
// "Calcola una volta" (RNF3, seconda revisione Fase 3): un SOLO
// dizionario `per-id` (id -> nodo-piatto) costruito in `valida()` e
// passato a tutte le funzioni sotto, invece di ricostruirlo in ognuna.
//
// Ricorsione residua (terza revisione, rilievo 5): la SOLA funzione
// ancora ricorsiva in questo modulo e' `_analizza` (sotto), che scorre
// l'albero gia' validato — la sua profondita' di chiamata e' quindi
// GIA' limitata da E-VAL-13 (`validate.typ`, max 10 livelli): nel caso
// peggiore ≤ 10 livelli × ~3 frame per livello (la chiamata a
// `_analizza` stessa, la chiusura passata a `.map()`, la chiusura
// passata a `.any()`) ≈ 30 frame, ben sotto il limite osservato di
// Typst (~80 frame, T19). Tutto il resto (`_visita-grafo`, cicli E
// ordine topologico insieme) e' ITERATIVO con pila esplicita: nessun
// limite di PROFONDITA' DI CHIAMATA legato al numero di id/dipendenze
// (verificato a 2000 id in catena, D-078/D-082) — resta invece un tetto
// di ITERAZIONI del `while` (T26, 5000 id in catena dalla testa,
// D-087): vedi la nota sopra `_visita-grafo`, due limiti distinti.

#import "errors.typ": fail
#import "schema.typ": figli, suggerisci

/// Vero se `antenato` e' un prefisso di percorso STRETTO di
/// `discendente` (es. "tasks[0]" e' antenato di
/// "tasks[0].subtasks[1]"): confronto di stringhe, corretto perche' ogni
/// percorso figlio aggiunge sempre ".qualcosa" dopo quello del genitore
/// (mai un'ambiguita' tipo "tasks[0]" contro "tasks[01]": gli indici
/// sono sempre in base 10 senza zero iniziale, D-036).
#let _e-antenato(antenato, discendente) = (
  discendente != antenato and discendente.starts-with(antenato + ".")
)

/// Visita ricorsiva BOTTOM-UP di un albero (gruppo o nodo, D-036;
/// `schema.figli` discrimina gruppo/nodo) che calcola in un'UNICA
/// passata (RNF3, "calcola una volta"):
/// - `ha-date`: vero se il nodo (o un suo discendente) ha una fonte di
///   date propria (RF10/D-039, E-DEP-05) — nessun aggregato di date, solo
///   "esiste almeno una data qui sotto?" (quello vero, con `date-mode`,
///   e' compito della timeline, D-050);
/// - `piatti`: l'elenco di `(path, id, name, dependencies, ha-date)` di
///   questo nodo e di TUTTI i suoi discendenti, in ordine di visita
///   (preordine: il nodo prima dei suoi figli).
/// Unica funzione ancora ricorsiva del modulo: profondita' limitata da
/// E-VAL-13 (≤ 10 livelli), vedi nota in testa al file.
#let _analizza(nodo) = {
  let figli-analizzati = figli(nodo).map(_analizza)
  let ha-date-propria = nodo.at("start", default: none) != none or nodo.at("intervals", default: ()).len() > 0
  // ATTENZIONE (controprova registrata, D-082): l'aggregazione
  // "or figli-analizzati.any(...)" e' quella che fa risultare "ha-date"
  // vero per un nodo SENZA date proprie ma con discendenti datati (es.
  // un gruppo 0.1 i cui task hanno "start"/"end" propri): senza questo
  // pezzo un `dependencies:` verso un simile gruppo darebbe un
  // E-DEP-05 SPURIO (il gruppo "sembrerebbe" senza date, anche se i
  // suoi task ne hanno). Caso positivo dedicato in `unit/graph.typ`.
  let ha-date = ha-date-propria or figli-analizzati.any(f => f.ha-date)
  let piatti = (
    (
      path: nodo.path, id: nodo.at("id", default: none), name: nodo.name,
      dependencies: nodo.at("dependencies", default: ()), "ha-date": ha-date,
    ),
  ) + figli-analizzati.map(f => f.piatti).flatten()
  (ha-date: ha-date, piatti: piatti)
}

/// Costruisce, in UN SOLO giro su `radici`, sia l'elenco appiattito di
/// TUTTO il bosco (in ordine di visita, preordine) sia il dizionario
/// `id -> nodo-piatto` (solo i nodi con un id proprio): usato da tutte
/// le funzioni sotto, cosi' nessuna ricostruisce la propria versione.
#let _prepara(radici) = {
  let analisi = radici.map(_analizza)
  let piatti = analisi.map(a => a.piatti).flatten()
  let per-id = (:)
  for n in piatti {
    if n.id != none { per-id.insert(n.id, n) }
  }
  (piatti: piatti, per-id: per-id)
}

/// Indice `id -> path`, derivato da `per-id` (nessuna ricostruzione):
/// id duplicato -> E-DEP-01 con il percorso della SECONDA occorrenza
/// (in ordine di visita) e, come `other-path`, quello della prima
/// (D-034). Qui si verifica la duplicazione VERA (lo stesso id in due
/// nodi distinti): `per-id` da solo non la vedrebbe (l'ultimo
/// `.insert()` vincerebbe in silenzio, lo stesso bug di gantty 0.5.1),
/// quindi questa funzione riscorre `piatti` in ordine, non `per-id`.
#let _verifica-duplicati(piatti, locale) = {
  let visti = (:)
  for n in piatti {
    if n.id != none {
      if n.id in visti {
        fail(
          "E-DEP-01", n.path + ".id", locale,
          argomenti: (id: n.id, other-path: visti.at(n.id)),
        )
      }
      visti.insert(n.id, n.path)
    }
  }
}

/// Etichetta sicura per un segnaposto di testo (C3, seconda revisione
/// Fase 3): `locale.errors`/`interpola` fanno SEMPRE `str.replace` su
/// stringhe — un `name` `content` (RF2 lo ammette: "name: content")
/// passato direttamente come valore di un segnaposto farebbe un panic
/// "nudo" di Typst ("expected string, found content"), non un errore
/// del catalogo (contrario a D-030). Se il nome e' una stringa si usa
/// quella; altrimenti il PERCORSO del nodo (sempre una stringa valida,
/// univoco, utile quanto un nome per individuare il nodo nel messaggio).
#let _etichetta(nodo-piatto) = if type(nodo-piatto.name) == str { nodo-piatto.name } else { nodo-piatto.path }

/// Verifica ogni dipendenza di ogni nodo, nell'ordine di visita (D-027,
/// determinismo): id sconosciuto (E-DEP-02, con suggerimento per
/// prefisso sugli id noti), dipendenza da se' stesso o da un
/// antenato/discendente (E-DEP-03), nodo senza date a UNO dei due capi
/// (E-DEP-05, D-039, verificato separatamente sulla SORGENTE e sul
/// BERSAGLIO: due occorrenze distinte, non la stessa guardia ripetuta).
/// Non tocca cicli ne' ordine (dopo, in `valida`): un problema di
/// risoluzione e' sempre PIU' specifico di un ciclo.
#let _verifica-dipendenze(piatti, per-id, locale) = {
  for n in piatti {
    for (k, dep) in n.dependencies.enumerate() {
      let percorso-dep = n.path + ".dependencies[" + str(k) + "]"
      if dep.id not in per-id {
        fail(
          "E-DEP-02", percorso-dep, locale,
          received: repr(dep.id), suggest: suggerisci(dep.id, per-id.keys()),
        )
      }
      let bersaglio = per-id.at(dep.id)
      if bersaglio.path == n.path or _e-antenato(bersaglio.path, n.path) or _e-antenato(n.path, bersaglio.path) {
        fail("E-DEP-03", percorso-dep, locale, argomenti: (id: dep.id))
      }
      // SORGENTE (il nodo che dichiara la dipendenza) senza date:
      if not n.ha-date {
        fail("E-DEP-05", percorso-dep, locale, argomenti: (name: _etichetta(n)))
      }
      // BERSAGLIO (il nodo referenziato dall'id) senza date: guardia
      // SEPARATA dalla precedente (controprova dedicata, D-078+).
      if not bersaglio.ha-date {
        fail("E-DEP-05", percorso-dep, locale, argomenti: (name: _etichetta(bersaglio)))
      }
    }
  }
}

/// Visita in profondita' a tre colori ("bianco"/"grigio"/"nero") sul
/// grafo "A dipende da B" (arco A -> B), ristretto ai nodi con un ID
/// PROPRIO (solo questi possono essere bersaglio di una dipendenza,
/// quindi solo questi possono chiudere un ciclo: un nodo senza id non
/// e' mai raggiungibile da un arco, D-034). **ITERATIVA con una PILA
/// ESPLICITA** (RF10: "visita iterativa a tre colori"), non ricorsiva:
/// Typst non permette di mutare una variabile catturata da una chiusura
/// ("variables from outside are read-only"), quindi l'accumulatore non
/// puo' vivere in una chiusura; una pila esplicita evita anche la
/// profondita' di CHIAMATA (type `function`) illimitata di una DFS
/// ricorsiva, che su una catena lineare molto lunga di id avvicinerebbe
/// il limite di Typst (~80 frame, T19) — con la pila la profondita' e'
/// quella dello STACK DI DATI (array), non della pila di chiamata di
/// Typst: nessun limite di FRAME legato al numero di id, verificato con
/// catene fino a 2000 nodi (D-082). **Resta comunque un tetto di
/// ITERAZIONI** (non di frame): il `while` sotto si ferma a 10 000
/// iterazioni (T26) — su una catena lineare visitata dalla testa (k id,
/// k-1 archi, 2k-1 iterazioni) questo e' un tetto di **5000 id**: 5000
/// compilano, 5001 danno «loop seems to be infinite» (misurato,
/// `project-init`, D-087), fuori dalla scala d'uso dichiarata (RNF3-dip).
///
/// **Terza revisione (suggerimento 4): l'ORDINE TOPOLOGICO si ricava
/// dalla STESSA visita, come POST-ORDINE** (invece di un secondo
/// algoritmo O(n²) separato, la prima stesura di questa fase, D-074):
/// quando un id diventa "nero" (tutti i suoi vicini sono completamente
/// esplorati, cioe' tutte le sue dipendenze sono gia' finite), viene
/// AGGIUNTO in coda a `ordine`. Questo e' topologicamente valido perche'
/// un id diventa nero solo DOPO che l'intera sotto-visita di ogni sua
/// dipendenza e' terminata (e quindi quella dipendenza e' GIA' in
/// `ordine`): "post-ordine di una DFS sul grafo delle dipendenze" è
/// esattamente "predecessori prima dei successori", senza bisogno di
/// invertire la lista alla fine. Costo O(n + e) (n id, e dipendenze fra
/// id del grafo), non piu' O(n²) in linea di principio — misurato A
/// FREDDO (un processo nuovo per misura, T32/D-087: le misure a caldo
/// ripetute nello stesso processo misurano la cache di memoizzazione di
/// Typst, non il calcolo — le "frazioni di secondo" riportate da una
/// voce precedente di questo commento erano proprio quell'artefatto):
/// 2000 id in catena lineare passano da ~7,5 s (D-078, algoritmo
/// precedente O(n²·d)) a ~2,0 s (D-087); il costo resta comunque
/// SUPERLINEARE anche dopo la fusione (causa non individuata, solo
/// misurata: D-087). 200 nodi/100 dipendenze (scala del benchmark RNF3)
/// restano comunque entro soglia con l'uno o l'altro algoritmo, la
/// differenza si vede solo a grande scala (criterio RNF3-dip, Fase 8).
/// Lo SPAREGGIO fra piu' id "pronti" (gia' nella prima stesura, D-074)
/// resta l'ordine di VISITA delle radici (preordine del documento,
/// l'ordine di `per-id.keys()`): non cambia nulla qui, la visita delle
/// radici segue lo stesso `for id-iniziale in per-id.keys()` di prima.
///
/// Nota sulla trappola di mutazione (verificata con TRE sonde dirette,
/// `.probe`, prima di scrivere questa funzione — correzione di una
/// descrizione troppo generale di una revisione precedente, D-082):
/// **non e' vero che ".at()" non muti mai in Typst.** La forma
/// INCATENATA diretta MUTA davvero (`pila.at(i).campo = valore` E
/// `pila.at(i).campo += valore` mutano entrambe l'array originale,
/// verificato): `.at()`/`.first()`/`.last()` sono normali accessori
/// mutabili ("place expressions") quando restano parte della STESSA
/// catena di accesso dell'assegnazione. La trappola vera e' un'altra:
/// se si LEGA PRIMA il risultato a una variabile (`let cima =
/// pila.at(i)`), quella variabile e' una COPIA indipendente — mutare
/// `cima` (anche con `cima.campo += valore`) NON si riflette piu'
/// sull'array originale. Qui si usa comunque l'assegnazione completa
/// (`pila.at(ultimo) = (id: ..., i: ...)`, non incatenata su `cima`)
/// per chiarezza, non perche' la forma incatenata non funzioni.
#let _visita-grafo(per-id) = {
  let colore = (:)
  for id in per-id.keys() { colore.insert(id, "bianco") }
  let ciclo = none
  let ordine = ()

  for id-iniziale in per-id.keys() {
    if ciclo != none { break }
    if colore.at(id-iniziale) != "bianco" { continue }

    // Un frame per id sulla pila: `id` e `i` (indice del prossimo
    // vicino, fra le sole dipendenze che puntano a un id del grafo, da
    // visitare). `cammino` e' la pila dei soli ID (per ricostruire il
    // ciclo quando si incontra un vicino "grigio").
    let pila = ((id: id-iniziale, i: 0),)
    let cammino = (id-iniziale,)
    colore.insert(id-iniziale, "grigio")

    while pila.len() > 0 and ciclo == none {
      let ultimo = pila.len() - 1
      let cima = pila.at(ultimo)
      let vicini = per-id.at(cima.id).dependencies.filter(d => d.id in per-id)
      if cima.i < vicini.len() {
        let prossimo = vicini.at(cima.i).id
        pila.at(ultimo) = (id: cima.id, i: cima.i + 1)
        let stato = colore.at(prossimo)
        if stato == "grigio" {
          let indice-inizio = cammino.position(x => x == prossimo)
          ciclo = cammino.slice(indice-inizio) + (prossimo,)
        } else if stato == "bianco" {
          colore.insert(prossimo, "grigio")
          pila.push((id: prossimo, i: 0))
          cammino.push(prossimo)
        }
        // "nero": vicino gia' completamente esplorato, nessun ciclo da
        // quella parte — si passa al prossimo vicino (i e' gia' avanzato).
      } else {
        colore.insert(cima.id, "nero")
        // POST-ORDINE (suggerimento 4): "cima.id" ha GIA' tutte le sue
        // dipendenze finite (nere) a questo punto — e' topologicamente
        // sicuro aggiungerlo ora.
        ordine.push(cima.id)
        // Bug reale trovato con una sonda diretta (nuova insidia di
        // Typst, T1-T23 di specification.md non toccata, riportata nel
        // rapporto): `array.pop()` RESTITUISCE l'elemento rimosso
        // (`-> any`), non `none` come `.push()`/`.insert()`. Il valore
        // di un'istruzione "nuda" (non legata a `let`) si propaga come
        // valore implicito del blocco che la contiene, e quello del
        // blocco si propaga a sua volta a `while`, poi a `for`, poi al
        // corpo della funzione — fino a tentare di UNIRSI con `ciclo`
        // (il valore finale della funzione): con un SOLO `.pop()` qui
        // (non scartato), la funzione proverebbe a unire il FRAME
        // rimosso (un dizionario) con `none`/una stringa del ciclo, e
        // con DUE `.pop()` consecutivi (pila e cammino, dizionario e
        // stringa) il tentativo di unirli FRA LORO fallisce ancora
        // prima: "cannot join dictionary with string". Scartare con
        // `let _ = ...` non e' cosmetico: senza, la funzione non
        // compila.
        let _ = pila.pop()
        let _ = cammino.pop()
      }
    }
  }
  (ciclo: ciclo, ordine: ordine)
}

/// Valida il grafo delle dipendenze sull'intero bosco `radici` (array di
/// gruppi e/o nodi radice, gia' validati da `validate.typ`) e restituisce
/// l'ordine topologico (RF10). Ordine dei controlli: duplicati
/// (E-DEP-01) -> risoluzione di ogni dipendenza (E-DEP-02/03/05) -> cicli
/// (E-DEP-04) -> ordine (ricavato dalla stessa visita dei cicli, sopra).
/// `locale` e' quello gia' risolto da `validate()`.
#let valida(radici, locale) = {
  let preparato = _prepara(radici)
  _verifica-duplicati(preparato.piatti, locale)
  _verifica-dipendenze(preparato.piatti, preparato.per-id, locale)
  let visita = _visita-grafo(preparato.per-id)
  if visita.ciclo != none {
    fail("E-DEP-04", "dependencies", locale, argomenti: (cycle: visita.ciclo.join(" → ")))
  }
  visita.ordine
}
