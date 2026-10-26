//! Conversione bidirezionale Markdown ↔ Typst (Fase 6, D-006/D-007 opzione B).
//!
//! Conversione **nativa**, senza binari esterni (niente Pandoc): `md_a_typst`
//! percorre lo stream di [`pulldown_cmark::Event`] ed emette markup Typst;
//! `typst_a_md` percorre l'AST tipato di `typst-syntax` (già nell'albero delle
//! dipendenze) ed emette CommonMark. Entrambe le funzioni sono pure
//! (`&str -> String`, nessun I/O) e puntano al **sottoinsieme di markup
//! comune** fra i due formati, non alla fedeltà totale.
//!
//! # Limiti noti (lossy, per design — vedi DECISIONI.md D-007)
//! - **md → typst**: HTML grezzo (blocco o inline, tranne i commenti `<!-- -->`,
//!   che diventano commenti Typst), liste di definizione, apice/pedice,
//!   blocchi di metadati e math sono omessi. Le note a piè di pagina
//!   (`[^n]`) diventano `#footnote[…]`; un'immagine **sola** nel paragrafo con
//!   testo alternativo diventa `#figure(image(..), caption: [alt])`, in mezzo
//!   al testo `#image(.., alt: "…")`. L'annidamento delle liste è reso con
//!   indentazione a 2 spazi per livello. Le sequenze di trattini (`--`, `---`,
//!   `-?` prima di una cifra) e i tre punti (`...`) **non** vengono scappate
//!   (richiederebbe guardare i caratteri adiacenti, non solo quello
//!   corrente): un testo Markdown che le contiene alla lettera può finire
//!   renderizzato come lineetta/ellissi Unicode invece che come
//!   trattini/punti letterali.
//! - **typst → md**: i costrutti senza equivalente CommonMark (funzioni
//!   arbitrarie, `set`/`show`/`let`, math, cicli/condizionali, riferimenti
//!   `@label`, etichette `<label>`, voci di termini, figure complesse) sono
//!   **preservati come Typst grezzo** — il sorgente esatto, **con** il `#` —
//!   in blocchi ```` ```{=typst} ```` o span `` `…`{=typst} `` (attributo «raw»
//!   di Pandoc); `md_a_typst` li riconosce e li riemette invariati, quindi
//!   `typ → md → typ` non li perde. Sono mappati su Markdown nativo, quando la
//!   forma lo consente: `link`, `image`, `strike`/`strong`/`emph`, `quote`
//!   (blockquote; l'`attribution:` diventa una riga `— …` **con avviso**),
//!   `footnote` (`[^n]`), `figure(image, caption)` (`![didascalia](..)`),
//!   commenti (`<!-- -->`) e liste annidate. Le perdite sono elencate dagli
//!   avvisi di [`typst_a_md_con_avvisi`].
//! - **Round-trip**: `md → typst → md` (o viceversa) **non è garantito**
//!   bit-per-bit: è garantito che i costrutti del sottoinsieme comune restino
//!   riconoscibili nella struttura (titoli, enfasi, liste, link, codice, note,
//!   figure) e che per `typ → md → typ` nessun comando Typst vada perso.
//!
//! # Sicurezza: `{=typst}` e Markdown non fidato
//! I blocchi ```` ```{=typst} ```` e gli span `` `…`{=typst} `` sono riemessi
//! **invariati** da [`md_a_typst`] (è ciò che rende possibile il round-trip
//! `typ → md → typ`). Ne segue che un `.md` **non fidato** può contenere
//! codice Typst arbitrario, ad esempio `#read("…")` o `#include "…"`, che al
//! momento della compilazione legge file dal progetto. Per i Markdown di
//! provenienza non affidabile usare [`md_a_typst_con_opzioni`] con
//! [`OpzioniMd::consenti_typst_grezzo`] a `false`: blocchi e span `{=typst}`
//! diventano allora semplice codice letterale (nessun codice eseguibile
//! nell'output). Il default di [`md_a_typst`] resta permissivo per non
//! rompere il round-trip dei documenti propri dell'utente.
//!
//! # Escaping
//! In entrambe le direzioni, il testo letterale (non markup) subisce
//! l'escape dei metacaratteri del formato di **destinazione**, per evitare
//! che testo innocuo generi markup involontario — in particolare, senza
//! l'escape di `` ` ``/`[`/`]` sul lato Typst, un `]` in un testo di link
//! chiuderebbe in anticipo il content block `#link(..)[…]` generato,
//! iniettando markup spurio nel resto del documento; senza l'escape di `/`
//! e `=`, un `//` letterale aprirebbe un commento di riga Typst (troncando
//! il resto della riga) e un paragrafo che inizia per `=` diventerebbe un
//! titolo. Il contenuto già "grezzo" (blocchi/segmenti di codice, testo alt
//! scartato, fallback lossy) non viene toccato: è per definizione già testo
//! letterale nel formato di partenza. Le destinazioni di `link`/`image`
//! verso Markdown che contengono spazi o `<>()` sono racchiuse fra
//! parentesi angolari CommonMark (`<…>`, con `<`/`>` interni
//! percent-encodati) invece di essere emesse grezze — vedi
//! [`formatta_destinazione_md`]. Il codice, sia inline sia a blocco (in
//! entrambe le direzioni), usa un numero di backtick delimitatori pari alla
//! run interna più lunga più uno (minimo 3 per i blocchi, dove comunque lo
//! richiede la sintassi del fence), con padding di uno spazio se il
//! contenuto **inline** inizia o finisce con un backtick (regola comune a
//! CommonMark e Typst, che altrimenti fonderebbero delimitatore e contenuto
//! in un'unica run ambigua) — **eccetto** che verso Typst il conteggio
//! inline non può mai risultare esattamente 2: il lexer di Typst tratta due
//! backtick consecutivi come raw span sempre vuoto, qualunque cosa segua
//! (vedi [`racchiudi_in_backtick_typst`]).

use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use typst_syntax::ast::{self, AstNode};
use typst_syntax::{SyntaxKind, SyntaxNode};

use crate::conversione::{Avvisi, Conversione};

/// Attributo «raw» (sintassi di Pandoc) che marca Typst grezzo dentro il
/// Markdown: blocco ```` ```{=typst} ```` o span `` `…`{=typst} ``.
const ATTR_TYPST: &str = "{=typst}";

// =======================================================================
// md → typst
// =======================================================================

/// Converte un documento Markdown (CommonMark + tabelle + barrato + note) in
/// markup Typst equivalente, sul sottoinsieme comune fra i due formati.
///
/// Pura (nessun I/O): il chiamante (guscio Tauri) decide se e come usare il
/// risultato (es. compilarlo per l'anteprima di un documento `.md`).
///
/// Riconosce anche i blocchi/span **Typst grezzo** prodotti da
/// [`typst_a_md`] (```` ```{=typst} ```` e `` `…`{=typst} ``) e li riemette
/// invariati, così il round-trip `typ → md → typ` non perde `#set`, `#show`,
/// math, riferimenti, ecc.
pub fn md_a_typst(md: &str) -> String {
    md_a_typst_con_opzioni(md, &OpzioniMd::default())
}

/// Opzioni di [`md_a_typst_con_opzioni`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpzioniMd {
    /// Se `true` (default) i blocchi ```` ```{=typst} ```` e gli span
    /// `` `…`{=typst} `` sono riemessi come codice Typst (necessario al
    /// round-trip). Se `false` diventano codice **letterale**: serve per
    /// Markdown non fidato, che altrimenti potrebbe iniettare `#read`,
    /// `#include` e simili.
    pub consenti_typst_grezzo: bool,
}

impl Default for OpzioniMd {
    fn default() -> Self {
        Self {
            consenti_typst_grezzo: true,
        }
    }
}

/// Come [`md_a_typst`], con opzioni esplicite (vedi [`OpzioniMd`] e la
/// sezione «Sicurezza» della documentazione del modulo).
pub fn md_a_typst_con_opzioni(md: &str, opz: &OpzioniMd) -> String {
    let opzioni =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    let eventi: Vec<Event> = Parser::new_ext(md, opzioni).collect();
    let note = raccogli_note(&eventi, opz);
    eventi_a_typst(&eventi, &note, opz)
}

/// Indice dell'`End` che chiude il `Start` in `eventi[inizio]`.
fn trova_fine(eventi: &[Event], inizio: usize) -> usize {
    let mut profondita = 0usize;
    for (k, e) in eventi.iter().enumerate().skip(inizio) {
        match e {
            Event::Start(_) => profondita += 1,
            Event::End(_) => {
                profondita = profondita.saturating_sub(1);
                if profondita == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    eventi.len().saturating_sub(1)
}

/// Corpo Typst (contenuto di `#footnote[…]`) di ogni definizione di nota.
fn raccogli_note(eventi: &[Event], opz: &OpzioniMd) -> HashMap<String, String> {
    let vuote = HashMap::new();
    let mut mappa = HashMap::new();
    let mut i = 0;
    while i < eventi.len() {
        if let Event::Start(Tag::FootnoteDefinition(etichetta)) = &eventi[i] {
            let fine = trova_fine(eventi, i);
            let corpo = eventi_a_typst(&eventi[i + 1..fine.max(i + 1)], &vuote, opz);
            mappa.insert(etichetta.to_string(), corpo.trim().to_string());
            i = fine + 1;
            continue;
        }
        i += 1;
    }
    mappa
}

/// Testo semplice (alt di un'immagine) contenuto negli eventi.
fn testo_di(eventi: &[Event]) -> String {
    let mut s = String::new();
    for e in eventi {
        match e {
            Event::Text(t) | Event::Code(t) => s.push_str(t),
            Event::SoftBreak | Event::HardBreak => s.push(' '),
            _ => {}
        }
    }
    s
}

/// Dopo un `Event::Code`, riconosce l'attributo `{=typst}` (anche se il
/// parser lo spezza in più `Text`): ritorna l'indice del primo evento non
/// consumato e il testo residuo che segue l'attributo.
fn consuma_attributo(eventi: &[Event], da: usize) -> Option<(usize, String)> {
    let attr = ATTR_TYPST.as_bytes();
    let mut acc = String::new();
    let mut k = da;
    while let Some(Event::Text(t)) = eventi.get(k) {
        acc.push_str(t);
        k += 1;
        let n = acc.len().min(attr.len());
        if acc.len() >= attr.len() || !attr.starts_with(&acc.as_bytes()[..n]) {
            break;
        }
    }
    acc.strip_prefix(ATTR_TYPST).map(|r| (k, r.to_string()))
}

/// Traduce un commento HTML (`<!-- … -->`) in commento a blocco Typst.
fn commento_html_a_typst(html: &str) -> Option<String> {
    let t = html.trim();
    let interno = t.strip_prefix("<!--")?.strip_suffix("-->")?;
    let interno = interno.replace("*/", "* /").replace("/*", "/ *");
    Some(format!("/*{interno}*/"))
}

/// Stato di una lista Markdown aperta durante la conversione.
///
/// Serve perché in pulldown-cmark `Tag::Item` non porta con sé il tipo di
/// lista: va dedotto dalla lista (eventualmente annidata) che lo contiene.
struct StatoLista {
    /// `true` = ordinata (`+`), `false` = puntata (`-`).
    ordinata: bool,
    /// Numero della prima voce se diverso da 1 (`3. a` → Typst `3. a`);
    /// le voci successive usano `+` e Typst continua la numerazione.
    inizio: Option<u64>,
    /// `true` dopo aver emesso la prima voce.
    primo_emesso: bool,
}

fn eventi_a_typst(eventi: &[Event], note: &HashMap<String, String>, opz: &OpzioniMd) -> String {
    let mut out = String::new();
    // Pila delle liste aperte (vedi `StatoLista`).
    let mut liste_ordinate: Vec<StatoLista> = Vec::new();
    // Pila di Strong/Emphasis aperti: `true` = resi con `#strong[…]`/`#emph[…]`
    // perché il delimitatore `*`/`_` cadrebbe a metà parola (dove Typst non
    // lo riconosce come markup), `false` = delimitatori compatti.
    let mut enfasi_funzione: Vec<bool> = Vec::new();
    // `true` fra `Start(CodeBlock)` e `End(CodeBlock)`: il testo del blocco
    // di codice va bufferizzato (non scappato: è già letterale) invece che
    // scritto subito in `out`, perché il fence di apertura/chiusura si
    // decide solo a chiusura blocco, quando se ne conosce l'intero
    // contenuto (stessa idea del codice inline, vedi
    // `racchiudi_in_backtick_typst`): altrimenti un blocco che contiene esso
    // stesso una riga di ``` chiuderebbe il fence in anticipo.
    let mut dentro_codice = false;
    let mut buffer_codice = String::new();
    let mut lingua_codice: Option<String> = None;
    // Blocco ```{=typst}: il contenuto è Typst grezzo, da riemettere invariato.
    let mut codice_typst = false;

    let mut i = 0;
    while i < eventi.len() {
        let evento = &eventi[i];
        match evento {
            Event::Start(Tag::Image { dest_url, .. }) => {
                let fine = trova_fine(eventi, i);
                let alt = testo_di(&eventi[i + 1..fine.max(i + 1)]);
                let sola = i > 0
                    && matches!(eventi[i - 1], Event::Start(Tag::Paragraph))
                    && matches!(eventi.get(fine + 1), Some(Event::End(TagEnd::Paragraph)));
                let percorso = escapa_stringa_typst(dest_url);
                if sola && !alt.trim().is_empty() {
                    // Immagine sola con testo alternativo: figura con didascalia.
                    // il testo sta in un content block `[…]`: inizio di markup
                    out.push_str(&format!(
                        "#figure(image(\"{percorso}\"), caption: [{}])",
                        escapa_typst_ctx(&alt, true, false)
                    ));
                } else if !alt.trim().is_empty() {
                    out.push_str(&format!(
                        "#image(\"{percorso}\", alt: \"{}\")",
                        escapa_stringa_typst(&alt)
                    ));
                } else {
                    out.push_str(&format!("#image(\"{percorso}\")"));
                }
                i = fine + 1;
                continue;
            }
            Event::Start(Tag::FootnoteDefinition(_)) => {
                i = trova_fine(eventi, i) + 1;
                continue;
            }
            Event::FootnoteReference(etichetta) => {
                if let Some(corpo) = note.get(etichetta.as_ref()) {
                    out.push_str("#footnote[");
                    out.push_str(corpo);
                    out.push(']');
                }
            }
            Event::Start(Tag::CodeBlock(tipo)) => {
                dentro_codice = true;
                buffer_codice.clear();
                codice_typst = opz.consenti_typst_grezzo
                    && matches!(tipo, CodeBlockKind::Fenced(info) if info.trim() == ATTR_TYPST);
                // L'`info string` di una fence può contenere più token (es.
                // ```js title="x"```): solo il primo, delimitato da spazi,
                // è la lingua — il resto non ha un posto dove andare in
                // Typst e viene scartato (lossy). Un token che non è un
                // identificatore semplice (es. residuo di una fence a tilde
                // con backtick nell'info string) viene scartato anziché
                // emesso com'è, per non iniettare markup nel tag lingua.
                lingua_codice = match tipo {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .filter(|l| lingua_fence_valida(l))
                        .map(str::to_string),
                    CodeBlockKind::Indented => None,
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                dentro_codice = false;
                if codice_typst {
                    // Typst grezzo: riemesso così com'è, su righe proprie.
                    codice_typst = false;
                    if !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(buffer_codice.trim_end_matches(['\n', '\r']));
                    out.push_str("\n\n");
                } else {
                    // Fence dinamico: lunghezza = run massima di backtick
                    // consecutivi nel contenuto + 1, minimo 3 (i blocchi, a
                    // differenza dell'inline, non hanno il caso speciale "mai
                    // esattamente 2": la sintassi di fence non accetta comunque
                    // meno di 3 backtick). Così un blocco che contiene esso
                    // stesso una riga di ``` (3 backtick) usa un fence a 4 e non
                    // si chiude in anticipo.
                    let n = (run_massima_backtick(&buffer_codice) + 1).max(3);
                    let delimitatore = "`".repeat(n);
                    out.push('\n');
                    out.push_str(&delimitatore);
                    if let Some(lingua) = &lingua_codice {
                        out.push_str(lingua);
                    }
                    out.push('\n');
                    out.push_str(&buffer_codice);
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&delimitatore);
                    out.push_str("\n\n");
                }
                lingua_codice = None;
            }
            Event::Start(tag @ (Tag::Strong | Tag::Emphasis)) => {
                let forte = matches!(tag, Tag::Strong);
                let fine = trova_fine(eventi, i);
                let usa_funzione = delimitatore_a_meta_parola(eventi, i, fine, &out);
                enfasi_funzione.push(usa_funzione);
                out.push_str(match (usa_funzione, forte) {
                    (true, true) => "#strong[",
                    (true, false) => "#emph[",
                    (false, true) => "*",
                    (false, false) => "_",
                });
            }
            Event::End(fine @ (TagEnd::Strong | TagEnd::Emphasis)) => {
                let usa_funzione = enfasi_funzione.pop().unwrap_or(false);
                out.push(match (usa_funzione, matches!(fine, TagEnd::Strong)) {
                    (true, _) => ']',
                    (false, true) => '*',
                    (false, false) => '_',
                });
            }
            Event::Start(tag) => avvia_tag_md(tag.clone(), &mut out, &mut liste_ordinate),
            Event::End(fine) => chiudi_tag_md(*fine, &mut out, &mut liste_ordinate),
            Event::Text(testo) => {
                if dentro_codice {
                    buffer_codice.push_str(testo);
                } else {
                    // Inizio riga / inizio di content block: `-`, `+`, `N.` sono
                    // marcatori di lista in Typst e vanno protetti.
                    let (inizio, dopo_cifre) = match testo.chars().next() {
                        Some('-' | '+' | '.' | '0'..='9') => contesto_inizio_markup(&out),
                        _ => (false, false),
                    };
                    out.push_str(&escapa_typst_ctx(testo, inizio, dopo_cifre));
                }
            }
            Event::Code(codice) => {
                // `` `…`{=typst} `` = span Typst grezzo (da `typst_a_md`);
                // se il Typst grezzo non è consentito (md non fidato) il codice è
                // letterale, ma l'attributo va comunque consumato: altrimenti
                // `{=typst}` resterebbe visibile nel testo.
                if let Some((k, resto)) = consuma_attributo(eventi, i + 1) {
                    if opz.consenti_typst_grezzo {
                        out.push_str(codice);
                    } else {
                        racchiudi_in_backtick_typst(codice, &mut out);
                    }
                    out.push_str(&escapa_typst(&resto));
                    i = k;
                    continue;
                }
                racchiudi_in_backtick_typst(codice, &mut out);
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("\\\n"),
            Event::Rule => out.push_str("\n#line(length: 100%)\n\n"),
            // I commenti HTML diventano commenti Typst; ogni altro HTML non ha
            // un equivalente diretto ed è omesso (vedi doc di modulo).
            Event::Html(h) | Event::InlineHtml(h) => {
                if let Some(c) = commento_html_a_typst(h) {
                    if matches!(evento, Event::Html(_)) && !out.is_empty() && !out.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str(&c);
                    if matches!(evento, Event::Html(_)) {
                        out.push('\n');
                    }
                }
            }
            // Nessun equivalente Typst diretto: omessi (vedi doc di modulo).
            Event::TaskListMarker(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {}
        }
        i += 1;
    }

    out
}

/// Verifica che il token di linguaggio di una fence Markdown sia un
/// identificatore semplice (`^[A-Za-z0-9_-]+$`): protegge dai fence a tilde
/// (`~~~`) con backtick nell'info string, che altrimenti finirebbero
/// letteralmente nel tag di linguaggio del fence Typst generato.
fn lingua_fence_valida(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Scrive il markup Typst di apertura per un tag a blocco/inline
/// direttamente nello stream d'uscita: non serve un buffer separato, il
/// contenuto fluisce naturalmente fra apertura e chiusura del tag.
fn avvia_tag_md(tag: Tag, out: &mut String, liste_ordinate: &mut Vec<StatoLista>) {
    match tag {
        Tag::Heading { level, .. } => {
            out.push('\n');
            for _ in 0..(level as usize) {
                out.push('=');
            }
            out.push(' ');
        }
        Tag::Strikethrough => out.push_str("#strike["),
        // `block: true` (non l'inline di default): una citazione Markdown è
        // sempre un elemento a blocco, mai in linea nel testo circostante.
        Tag::BlockQuote(_) => out.push_str("\n#quote(block: true)[\n"),
        Tag::List(numero) => liste_ordinate.push(StatoLista {
            ordinata: numero.is_some(),
            // `1.` è il default di Typst (`+`); qualunque altro numero iniziale
            // (anche 0) va scritto esplicitamente sulla prima voce.
            inizio: numero.filter(|n| *n != 1),
            primo_emesso: false,
        }),
        Tag::Item => {
            out.push('\n');
            // Indentazione a 2 spazi per livello di annidamento (lossy: non
            // è un adattamento semantico, solo una resa leggibile).
            for _ in 1..liste_ordinate.len() {
                out.push_str("  ");
            }
            match liste_ordinate.last_mut() {
                Some(l) if l.ordinata => {
                    match (l.inizio, l.primo_emesso) {
                        (Some(n), false) => out.push_str(&format!("{n}. ")),
                        _ => out.push_str("+ "),
                    }
                    l.primo_emesso = true;
                }
                _ => out.push_str("- "),
            }
        }
        Tag::Link { dest_url, .. } => {
            out.push_str("#link(\"");
            out.push_str(&escapa_stringa_typst(&dest_url));
            out.push_str("\")[");
        }
        Tag::Table(colonne) => {
            out.push_str("\n#table(\n  columns: ");
            out.push_str(&colonne.len().to_string());
            out.push_str(",\n");
        }
        Tag::TableCell => out.push_str("  ["),
        // Nessun markup Typst dedicato: il contenuto fluisce comunque
        // (best-effort, vedi doc di modulo). `Image` non arriva mai qui: è
        // intercettata prima, nel chiamante. `CodeBlock` è gestito a parte
        // nel chiamante (bufferizzazione per il fence dinamico). `Strong` ed
        // `Emphasis` sono gestiti nel chiamante (scelta fra `*`/`_` e
        // `#strong[`/`#emph[` a seconda delle lettere adiacenti).
        Tag::Strong
        | Tag::Emphasis
        | Tag::Paragraph
        | Tag::TableHead
        | Tag::TableRow
        | Tag::HtmlBlock
        | Tag::Image { .. }
        | Tag::CodeBlock(_)
        | Tag::FootnoteDefinition(_)
        | Tag::DefinitionList
        | Tag::DefinitionListTitle
        | Tag::DefinitionListDefinition
        | Tag::Subscript
        | Tag::Superscript
        | Tag::MetadataBlock(_) => {}
    }
}

/// Scrive il markup Typst di chiusura corrispondente a un `TagEnd`.
fn chiudi_tag_md(fine: TagEnd, out: &mut String, liste_ordinate: &mut Vec<StatoLista>) {
    match fine {
        TagEnd::Heading(_) => out.push_str("\n\n"),
        TagEnd::Paragraph => out.push_str("\n\n"),
        // `Strong`/`Emphasis` sono chiusi dal chiamante (vedi `avvia_tag_md`).
        TagEnd::Strikethrough => out.push(']'),
        TagEnd::BlockQuote(_) => out.push_str("\n]\n\n"),
        TagEnd::List(_) => {
            liste_ordinate.pop();
            // Doppio a capo di chiusura solo per la lista di livello 0 (pila
            // vuota dopo il pop): separa la lista dal paragrafo/blocco
            // seguente (un solo `\n` lo lascerebbe attaccato). Per una
            // sotto-lista annidata (pila ancora non vuota) NON si emette:
            // spezzerebbe con una riga vuota la lista genitrice che la
            // contiene, interrompendone la continuità.
            if liste_ordinate.is_empty() {
                out.push_str("\n\n");
            }
        }
        TagEnd::Link => out.push(']'),
        TagEnd::Table => out.push_str(")\n\n"),
        TagEnd::TableCell => out.push_str("],\n"),
        // `CodeBlock` è gestito a parte nel chiamante (bufferizzazione per
        // il fence dinamico, vedi `md_a_typst`).
        TagEnd::Item | TagEnd::TableRow | TagEnd::TableHead | TagEnd::CodeBlock => {}
        _ => {}
    }
}

/// Esegue l'escape dei caratteri con significato di markup in Typst
/// (`` \ * _ # $ < > @ ` [ ] ~ / = ``) all'interno di testo altrimenti
/// letterale.
///
/// `` ` ``, `[` e `]` sono cruciali da scappare: `md_a_typst` genera i propri
/// content block (`#link(..)[…]`, `#strike[…]`, `#quote(..)[…]`, celle di
/// `#table`) con parentesi quadre letterali, quindi un `]` non scappato nel
/// testo originale li chiuderebbe in anticipo, e un backtick aprirebbe un
/// raw span — in entrambi i casi iniettando markup nel resto del documento
/// invece di restare testo innocuo. `~` è la scorciatoia Typst per lo spazio
/// unificatore: senza escape un `~` letterale nel Markdown originale
/// renderizzerebbe come spazio unificatore invece che come tilde. `/` e `=`
/// sono altrettanto cruciali: `//` apre un commento di riga Typst (il resto
/// della riga sparirebbe silenziosamente) e `=` a inizio riga è sintassi di
/// titolo — testo Markdown innocuo come `http://x`, `and/or` o un paragrafo
/// che comincia per `=` verrebbe altrimenti interpretato come markup invece
/// che restare testo letterale.
///
/// L'escape è **context-free**, come per tutti gli altri caratteri qui: un
/// `/` o un `=` scappato in una posizione dove non avrebbe comunque un
/// significato speciale (es. `/` isolato senza un secondo `/` adiacente, o
/// `=` non a inizio riga) produce comunque il carattere letterale in output
/// — nessuna differenza visibile nel documento reso, solo un `\` di troppo
/// nel sorgente Typst generato (stessa scelta già adottata per `#`, `*`,
/// ecc.).
///
/// **Limite noto**: le run di trattini e i tre punti (scorciatoie per
/// lineette/ellissi) non sono coperte — vedi doc di modulo.
fn escapa_typst(testo: &str) -> String {
    let mut esito = String::with_capacity(testo.len());
    for c in testo.chars() {
        if matches!(
            c,
            '\\' | '*' | '_' | '#' | '$' | '<' | '>' | '@' | '`' | '[' | ']' | '~' | '/' | '='
        ) {
            esito.push('\\');
        }
        esito.push(c);
    }
    esito
}

/// Come [`escapa_typst`], ma protegge anche i marcatori di lista che il testo
/// formerebbe **a inizio riga** (o a inizio di un content block `[…]`):
/// `-` e `+` iniziali e il `.` di `N.`.
///
/// - `a_inizio`: il testo comincia dove Typst riconosce i marcatori di lista;
/// - `dopo_cifre`: la riga scritta finora è fatta di sole cifre (il testo,
///   se comincia per `.`, completerebbe un `N.`).
///
/// Senza questa protezione `\- x`, `\+ x` e `1\. x` di Markdown, che sono
/// testo letterale, diventerebbero liste Typst.
fn escapa_typst_ctx(testo: &str, a_inizio: bool, dopo_cifre: bool) -> String {
    let base = escapa_typst(testo);
    if dopo_cifre && testo.starts_with('.') {
        return format!("\\{base}");
    }
    if !a_inizio {
        return base;
    }
    if testo.starts_with(['-', '+']) {
        return format!("\\{base}");
    }
    let cifre = testo.bytes().take_while(u8::is_ascii_digit).count();
    if cifre > 0 && testo[cifre..].starts_with('.') {
        // le cifre non sono toccate da `escapa_typst`: prefisso identico
        return format!("{}\\{}", &testo[..cifre], &base[cifre..]);
    }
    base
}

/// Analizza la riga corrente di `out`: ritorna `(a_inizio, dopo_cifre)` per
/// [`escapa_typst_ctx`]. I marcatori di lista che `md_a_typst` stesso ha
/// appena scritto (`- `, `+ `, `N. `) contano come inizio, perché dopo di
/// essi Typst riconosce una voce annidata.
fn contesto_inizio_markup(out: &str) -> (bool, bool) {
    let riga = out.rsplit('\n').next().unwrap_or("");
    // Coda dopo l'ultimo `[` **non scappato** (apertura di un content block:
    // `#link(..)[`, `#strong[`, cella di tabella, …): lì Typst riconosce i
    // marcatori di lista come a inizio riga.
    let coda = ultima_quadra_aperta(riga).map_or(riga, |p| &riga[p + 1..]);
    let mut r = coda.trim_start();
    loop {
        if let Some(x) = r.strip_prefix("- ").or_else(|| r.strip_prefix("+ ")) {
            r = x.trim_start();
            continue;
        }
        let cifre = r.bytes().take_while(u8::is_ascii_digit).count();
        if cifre > 0 && r[cifre..].starts_with(". ") {
            r = r[cifre + 2..].trim_start();
            continue;
        }
        break;
    }
    let dopo_cifre = !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit());
    (r.is_empty(), dopo_cifre)
}

/// Posizione dell'ultimo `[` non preceduto da un numero dispari di `\`.
fn ultima_quadra_aperta(riga: &str) -> Option<usize> {
    let b = riga.as_bytes();
    (0..b.len()).rev().find(|&p| {
        b[p] == b'[' && b[..p].iter().rev().take_while(|&&c| c == b'\\').count() % 2 == 0
    })
}

/// `true` se un delimitatore `*`/`_` di enfasi aperto in `eventi[inizio]`
/// (chiuso in `eventi[fine]`) cadrebbe **a metà parola**, dove il lexer di
/// Typst lo tratta come testo: all'apertura quando il carattere prima e il
/// primo dentro sono alfanumerici, alla chiusura quando l'ultimo dentro e il
/// successivo lo sono. In quel caso si usa `#strong[…]`/`#emph[…]`.
///
/// I caratteri vengono letti solo dagli eventi `Text` adiacenti: ogni altro
/// evento (codice, link, fine di un tag, …) produce un carattere non
/// alfanumerico (`` ` ``, `#`, `]`, …).
fn delimitatore_a_meta_parola(eventi: &[Event], inizio: usize, fine: usize, out: &str) -> bool {
    let alnum = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let primo = |e: Option<&Event>| match e {
        Some(Event::Text(t)) => t.chars().next(),
        _ => None,
    };
    let ultimo = |e: Option<&Event>| match e {
        Some(Event::Text(t)) => t.chars().last(),
        _ => None,
    };
    let prima = out.chars().last();
    let apertura = alnum(prima) && alnum(primo(eventi.get(inizio + 1)));
    let chiusura =
        fine > 0 && alnum(ultimo(eventi.get(fine - 1))) && alnum(primo(eventi.get(fine + 1)));
    apertura || chiusura
}

/// Esegue l'escape di un valore da inserire in una stringa Typst fra
/// virgolette (`"…"`): qui contano solo `"` e `\`, non i metacaratteri di
/// markup (che in una stringa non hanno significato speciale).
fn escapa_stringa_typst(testo: &str) -> String {
    let mut esito = String::with_capacity(testo.len());
    for c in testo.chars() {
        if matches!(c, '"' | '\\') {
            esito.push('\\');
        }
        esito.push(c);
    }
    esito
}

/// Lunghezza della run di backtick consecutivi più lunga in `testo`.
fn run_massima_backtick(testo: &str) -> usize {
    let mut massima = 0usize;
    let mut corrente = 0usize;
    for c in testo.chars() {
        if c == '`' {
            corrente += 1;
            massima = massima.max(corrente);
        } else {
            corrente = 0;
        }
    }
    massima
}

/// Racchiude `contenuto` fra `n` backtick per lato: un numero di
/// delimitatori sufficiente a non essere ambiguo col contenuto (chi chiama
/// calcola `n`, tenendo conto delle regole — diverse! — dei due formati, vedi
/// [`racchiudi_in_backtick_md`]/[`racchiudi_in_backtick_typst`]). Se il
/// contenuto inizia o finisce esso stesso con un backtick, un delimitatore
/// adiacente si fonderebbe con esso in un'unica run più lunga (ambigua): un
/// padding di un solo spazio per lato rompe l'adiacenza.
///
/// `padding_forzato` richiede il padding anche se il contenuto non inizia o
/// finisce con un backtick (vedi [`racchiudi_in_backtick_typst`]).
fn scrivi_delimitato_da_backtick(
    contenuto: &str,
    n: usize,
    padding_forzato: bool,
    out: &mut String,
) {
    let delimitatore = "`".repeat(n);
    let padding = contenuto.starts_with('`') || contenuto.ends_with('`');

    out.push_str(&delimitatore);
    // `padding_forzato` protegge solo l'**inizio** (tag di lingua): Typst toglie
    // lo spazio iniziale, ma uno finale lo toglierebbe solo se il contenuto
    // finisse con un backtick, quindi qui resterebbe nel testo.
    if padding || padding_forzato {
        out.push(' ');
    }
    out.push_str(contenuto);
    if padding {
        out.push(' ');
    }
    out.push_str(&delimitatore);
}

/// Racchiude `contenuto` fra backtick per il codice inline **Markdown**:
/// CommonMark delimita contando run contigue di backtick, senza casi
/// speciali — "run interna più lunga + 1" per lato è sempre corretto.
fn racchiudi_in_backtick_md(contenuto: &str, out: &mut String) {
    scrivi_delimitato_da_backtick(contenuto, run_massima_backtick(contenuto) + 1, false, out);
}

/// Racchiude `contenuto` fra backtick per il raw span **Typst**: come sopra,
/// ma **mai con esattamente 2 backtick**. Il lexer di Typst tratta due
/// backtick consecutivi come caso speciale — un raw span sempre **vuoto**,
/// a prescindere da cosa segua (`` `` `` chiude immediatamente,
/// diversamente da CommonMark dove un delimitatore a 2 backtick è valido
/// come un altro): se il conteggio "run interna più lunga + 1" darebbe
/// esattamente 2, va portato a 3, altrimenti il raw span emesso risulta
/// vuoto e il resto del contenuto (backtick di chiusura compresi) finisce
/// letto come markup, non come testo del raw.
///
/// Con 3 o più backtick Typst legge inoltre un eventuale **identificatore**
/// subito dopo i delimitatori come tag di linguaggio: `` ```a`b``` `` è un
/// raw di lingua `a` con testo `` `b ``, non il codice `` a`b ``. Se il
/// contenuto inizia con una lettera o `_` si forza quindi il padding con uno
/// spazio (che Typst rimuove), così il contenuto non è mai scambiato per un
/// tag.
fn racchiudi_in_backtick_typst(contenuto: &str, out: &mut String) {
    let mut n = run_massima_backtick(contenuto) + 1;
    if n == 2 {
        n = 3;
    }
    let rischio_lingua = n >= 3
        && contenuto
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_');
    scrivi_delimitato_da_backtick(contenuto, n, rischio_lingua, out);
}

// =======================================================================
// typst → md
// =======================================================================

/// Contesto della conversione typst → md: avvisi sulle perdite, note a piè
/// di pagina da accodare e contatori.
#[derive(Default)]
struct Ctx {
    avvisi: Avvisi,
    /// Corpi (già in Markdown) delle note incontrate: `[^N]` = indice + 1.
    note: Vec<String>,
    /// Quanti costrutti sono stati preservati come Typst grezzo.
    grezzi: usize,
    /// Profondità di annidamento nei corpi di voci di lista.
    profondita_lista: usize,
    /// Profondità di annidamento del markup (limitata da [`PROFONDITA_MD_MAX`]).
    profondita: usize,
    /// Quanti contesti **inline** (testo di link, enfasi, titoli, …) sono
    /// aperti: lì un blocco ``` ```{=typst} ``` non può comparire e il Typst
    /// grezzo va sempre in uno span `` `…`{=typst} ``.
    dentro_inline: usize,
}

/// Annidamento massimo del markup Typst convertito (la conversione è
/// ricorsiva e ogni livello costa diversi KiB di stack): oltre, il
/// sotto-albero è conservato come Typst grezzo, con un avviso.
const PROFONDITA_MD_MAX: usize = 32;

impl Ctx {
    fn avvisa(&mut self, m: impl Into<String>) {
        // dedup in tempo costante e ordine d'inserimento: vedi `Avvisi`
        self.avvisi.aggiungi(m);
    }

    /// `true` se il Typst grezzo può occupare righe proprie (blocco).
    fn blocco_ammesso(&self) -> bool {
        self.dentro_inline == 0
    }
}

/// Converte con `ctx.dentro_inline` incrementato: il contenuto non può
/// contenere blocchi (testo di link, enfasi, titoli).
fn markup_inline_a_md(markup: ast::Markup, out: &mut String, ctx: &mut Ctx) {
    ctx.dentro_inline += 1;
    markup_a_md(markup, out, ctx);
    ctx.dentro_inline -= 1;
}

/// Converte un documento Typst in Markdown (CommonMark) equivalente, sul
/// sottoinsieme comune fra i due formati.
///
/// Percorre l'AST tipato di `typst-syntax` (radice [`ast::Markup`]); i
/// costrutti fuori dal sottoinsieme mappato vengono **preservati come Typst
/// grezzo** (vedi doc di modulo). Pura (nessun I/O). Per conoscere cosa si
/// perde usare [`typst_a_md_con_avvisi`].
pub fn typst_a_md(sorgente: &str) -> String {
    typst_a_md_con_avvisi(sorgente).contenuto
}

/// Come [`typst_a_md`], ma restituisce anche l'elenco degli avvisi sulle
/// perdite (es. attribuzione di una citazione trasformata in riga di testo,
/// numero di costrutti conservati come Typst grezzo).
pub fn typst_a_md_con_avvisi(sorgente: &str) -> Conversione {
    let radice = typst_syntax::parse(sorgente);
    let mut ctx = Ctx::default();
    let mut out = String::new();
    if radice.diagnosis().errors {
        ctx.avvisa("il sorgente Typst contiene errori di sintassi: conversione al meglio");
    }
    match radice.cast::<ast::Markup>() {
        Some(markup) => markup_a_md(markup, &mut out, &mut ctx),
        // Non dovrebbe mai accadere (`parse` produce sempre una radice
        // `Markup`), ma degrada comunque al testo grezzo anziché a un
        // documento vuoto.
        None => out.push_str(&radice.full_text()),
    }
    // Definizioni delle note a piè di pagina, in coda.
    if !ctx.note.is_empty() {
        inizia_blocco(&mut out);
        for (k, corpo) in ctx.note.iter().enumerate() {
            out.push_str(&format!("[^{}]: ", k + 1));
            out.push_str(&rientra_righe_successive(corpo.trim(), "    "));
            out.push('\n');
        }
    }
    if ctx.grezzi > 0 {
        let n = ctx.grezzi;
        ctx.avvisa(format!(
            "{n} costrutti Typst non rappresentabili in Markdown (set/show/let, math, funzioni, \
             riferimenti, …) sono conservati come Typst grezzo (blocchi `{ATTR_TYPST}`): Typster \
             li ripristina nella conversione inversa, altri visualizzatori Markdown no"
        ));
    }
    let fine = out.trim_end().len();
    out.truncate(fine);
    if !out.is_empty() {
        out.push('\n');
    }
    Conversione {
        contenuto: out,
        avvisi: ctx.avvisi.into_vec(),
    }
}

// ---- utilità di impaginazione --------------------------------------------

/// Porta `out` all'inizio di una riga (senza spazi finali né righe vuote).
fn a_inizio_riga(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

/// Assicura che il prossimo contenuto inizi dopo una riga vuota (blocco).
fn inizia_blocco(out: &mut String) {
    while out.ends_with(' ') {
        out.pop();
    }
    if out.is_empty() || out.ends_with("\n\n") {
        return;
    }
    if out.ends_with('\n') {
        out.push('\n');
    } else {
        out.push_str("\n\n");
    }
}

/// Indenta tutte le righe non vuote tranne la prima.
fn rientra_righe_successive(testo: &str, rientro: &str) -> String {
    let mut s = String::with_capacity(testo.len() + 8);
    for (k, riga) in testo.split('\n').enumerate() {
        if k > 0 {
            s.push('\n');
            if !riga.is_empty() {
                s.push_str(rientro);
            }
        }
        s.push_str(riga);
    }
    s
}

/// Protegge dal significato di marcatore di lista un testo letterale in
/// testa a una riga (`-`, `+`, `1.`, `1)`).
fn escapa_inizio_riga(t: &str) -> String {
    let cifre = t.chars().take_while(char::is_ascii_digit).count();
    if (t == "-" || t == "+") || (cifre > 0 && matches!(t[cifre..].chars().next(), Some('.' | ')')))
    {
        if cifre > 0 {
            let (a, b) = t.split_at(cifre);
            return format!("{a}\\{b}");
        }
        return format!("\\{t}");
    }
    t.to_string()
}

/// Accoda testo **già escapato** proteggendo l'inizio riga.
fn push_letterale_md(out: &mut String, s: &str) {
    if out.is_empty() || out.ends_with('\n') {
        out.push_str(&escapa_inizio_riga(s));
    } else {
        out.push_str(s);
    }
}

fn contiene_newline(n: &SyntaxNode) -> bool {
    n.full_text().contains('\n')
}

/// `true` se il nodo `da` è all'inizio di una riga (ignorando rientri).
fn inizio_riga(figli: &[&SyntaxNode], da: usize) -> bool {
    let mut k = da;
    while k > 0 {
        k -= 1;
        match figli[k].kind() {
            SyntaxKind::Parbreak => return true,
            SyntaxKind::Space if contiene_newline(figli[k]) => return true,
            SyntaxKind::Space => continue,
            _ => return false,
        }
    }
    true
}

/// `true` se dopo il nodo `a` la riga finisce (o finisce il documento).
fn fine_riga(figli: &[&SyntaxNode], a: usize) -> bool {
    let mut k = a + 1;
    while k < figli.len() {
        match figli[k].kind() {
            SyntaxKind::Parbreak => return true,
            SyntaxKind::Space if contiene_newline(figli[k]) => return true,
            SyntaxKind::Space => k += 1,
            _ => return false,
        }
    }
    true
}

fn su_riga_propria(figli: &[&SyntaxNode], da: usize, a: usize) -> bool {
    inizio_riga(figli, da) && fine_riga(figli, a)
}

/// Se subito dopo `fine` (anche dopo spazi senza a capo) c'è un'etichetta
/// `<nome>`, ne restituisce l'indice; altrimenti `fine`.
fn con_etichetta(figli: &[&SyntaxNode], fine: usize) -> usize {
    let mut k = fine + 1;
    while k < figli.len() && figli[k].kind() == SyntaxKind::Space && !contiene_newline(figli[k]) {
        k += 1;
    }
    if k < figli.len() && figli[k].kind() == SyntaxKind::Label {
        k
    } else {
        fine
    }
}

fn testo_nodi(figli: &[&SyntaxNode], da: usize, a: usize) -> String {
    let mut s = String::new();
    for n in &figli[da..=a] {
        s.push_str(&n.full_text());
    }
    s
}

// ---- conservazione del Typst grezzo --------------------------------------

/// Scrive `testo` come blocco Typst grezzo: fence ```` ```{=typst} ````
/// (sintassi «raw attribute» di Pandoc), con delimitatore più lungo di ogni
/// sequenza di backtick contenuta.
fn scrivi_raw_blocco(testo: &str, out: &mut String) {
    let testo = testo.trim_end_matches(['\n', '\r', ' ']);
    let n = (run_massima_backtick(testo) + 1).max(3);
    let delim = "`".repeat(n);
    inizia_blocco(out);
    out.push_str(&delim);
    out.push_str(ATTR_TYPST);
    out.push('\n');
    out.push_str(testo);
    out.push('\n');
    out.push_str(&delim);
    out.push_str("\n\n");
}

/// Scrive `testo` come span Typst grezzo in linea: `` `testo`{=typst} ``.
fn scrivi_raw_inline(testo: &str, out: &mut String) {
    let n = run_massima_backtick(testo) + 1;
    let delim = "`".repeat(n);
    let pad = testo.starts_with('`')
        || testo.ends_with('`')
        || (testo.starts_with(' ') && testo.ends_with(' ') && !testo.trim().is_empty());
    out.push_str(&delim);
    if pad {
        out.push(' ');
    }
    out.push_str(testo);
    if pad {
        out.push(' ');
    }
    out.push_str(&delim);
    out.push_str(ATTR_TYPST);
}

/// Emette i nodi `da..=a` come Typst grezzo. Se occupano righe proprie sono
/// un blocco (e si raggruppano con i blocchi grezzi consecutivi), altrimenti
/// uno span in linea. Ritorna l'indice del primo nodo non consumato.
fn emetti_raw(
    figli: &[&SyntaxNode],
    da: usize,
    a: usize,
    out: &mut String,
    ctx: &mut Ctx,
) -> usize {
    // `#x;testo`: il `;` che chiude l'espressione fa parte del sorgente (senza
    // di esso `#xtesto` sarebbe un altro identificatore).
    let a = if figli[da].kind() == SyntaxKind::Hash {
        con_punto_e_virgola(figli, a)
    } else {
        a
    };
    let a = con_etichetta(figli, a);
    ctx.grezzi += 1;
    // In un contesto inline (testo di link, enfasi, titolo) un blocco non può
    // comparire, anche se i nodi sono primo e ultimo del loro `Markup`.
    if ctx.blocco_ammesso() && su_riga_propria(figli, da, a) {
        let mut fine = a;
        // Raggruppa i blocchi grezzi successivi, separati da soli a capo.
        loop {
            let mut k = fine + 1;
            while k < figli.len()
                && matches!(figli[k].kind(), SyntaxKind::Space | SyntaxKind::Parbreak)
            {
                k += 1;
            }
            if k == fine + 1 || k >= figli.len() {
                break;
            }
            match candidato_blocco_raw(figli, k) {
                Some(f2) if su_riga_propria(figli, k, f2) => {
                    fine = f2;
                    ctx.grezzi += 1;
                }
                _ => break,
            }
        }
        scrivi_raw_blocco(&testo_nodi(figli, da, fine), out);
        return fine + 1;
    }
    let testo = testo_nodi(figli, da, a);
    if testo.contains('\n') {
        // Un a capo in uno span di codice CommonMark diventa spazio, e la
        // compattazione (trim + join) cancella rientri e righe vuote. È
        // innocua solo per codice "piano": con stringhe, raw, blocchi di
        // contenuto `[…]`, math, escape o commenti cambierebbe il
        // significato (una stringa multiriga perderebbe l'a capo, un commento
        // di riga inghiottirebbe il resto). In quei casi si emette un blocco.
        // `{`/`}`: i blocchi di codice separano le istruzioni con a capo, che
        // unendo le righe con uno spazio diventerebbero un errore di sintassi.
        let compattabile = !testo.contains(['`', '"', '[', ']', '$', '\\', '{', '}'])
            && !testo.contains("//")
            && !testo.contains("/*");
        if compattabile {
            let compatto = testo
                .split('\n')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ");
            scrivi_raw_inline(&compatto, out);
        } else if ctx.blocco_ammesso() {
            ctx.avvisa(
                "un'espressione Typst multiriga in mezzo al testo è stata emessa come blocco \
                 grezzo: il paragrafo risulta spezzato",
            );
            scrivi_raw_blocco(&testo, out);
        } else {
            // dentro un titolo, un link o un'enfasi il blocco è impossibile:
            // l'unica forma è lo span, con la formattazione compattata
            let (compatto, commenti, valido) = compatta_in_una_riga(&testo);
            let mut avviso = String::from(
                "un'espressione Typst multiriga dentro un titolo, un link o un'enfasi è stata \
                 compattata su una riga: rientri e a capo interni sono andati persi",
            );
            if commenti {
                avviso.push_str("; i commenti `//` sono diventati `/* */`");
            }
            if !valido {
                avviso.push_str(
                    "; il risultato potrebbe non essere Typst valido (controllare l'espressione)",
                );
            }
            ctx.avvisa(avviso);
            scrivi_raw_inline(&compatto, out);
        }
    } else {
        scrivi_raw_inline(&testo, out);
    }
    a + 1
}

/// Foglie (tipo, testo) del sorgente Typst `testo`, in ordine.
fn foglie_typst(n: &SyntaxNode, v: &mut Vec<(SyntaxKind, String)>) {
    if n.children().next().is_none() {
        v.push((n.kind(), n.full_text().to_string()));
    } else {
        for c in n.children() {
            foglie_typst(c, v);
        }
    }
}

/// Compatta su una riga un'espressione Typst multiriga che deve stare in uno
/// span (contesto inline). Ritorna `(testo, commenti_convertiti, valido)`.
///
/// - i commenti `// …` diventano `/* … */` (un commento di riga
///   inghiottirebbe il resto della riga compattata; `*/` interno è spezzato);
/// - gli a capo diventano spazi; se così il risultato non è Typst valido
///   (tipico di `{ let x = 1⏎ let y = 2 }`) si riprova separando le istruzioni
///   con `; `;
/// - `valido` dice se il risultato si legge senza errori di sintassi: se
///   nessuna delle due forme lo è, si usa quella con gli spazi.
fn compatta_in_una_riga(testo: &str) -> (String, bool, bool) {
    let mut foglie = Vec::new();
    foglie_typst(&typst_syntax::parse(testo), &mut foglie);
    let mut commenti = false;
    let mut tentativo = |istruzioni: bool| -> String {
        let mut s = String::with_capacity(testo.len());
        // ultimo token significativo (né spazio né commento) già emesso
        let mut prec = "";
        // `; ` già emesso dopo `prec`: un commento su riga propria fra due
        // istruzioni vedrebbe due a capo con lo stesso `prec` (→ `; /* c*/; `).
        let mut separato = false;
        for (k, (kind, t)) in foglie.iter().enumerate() {
            match kind {
                SyntaxKind::LineComment => {
                    commenti = true;
                    let corpo = t.strip_prefix("//").unwrap_or(t);
                    let corpo = corpo.replace("*/", "* /").replace("/*", "/ *");
                    s.push_str(&format!("/*{corpo}*/"));
                }
                SyntaxKind::Space if t.contains('\n') => {
                    let succ = foglie[k + 1..]
                        .iter()
                        .find(|(kk, _)| {
                            !matches!(
                                kk,
                                SyntaxKind::Space
                                    | SyntaxKind::LineComment
                                    | SyntaxKind::BlockComment
                            )
                        })
                        .map_or("", |(_, tt)| tt.as_str());
                    let fine_istruzione = prec
                        .chars()
                        .last()
                        .is_some_and(|c| c.is_alphanumeric() || "_)}]\"".contains(c));
                    let inizio_istruzione = succ
                        .chars()
                        .next()
                        .is_some_and(|c| !"})].,".contains(c) && !c.is_whitespace())
                        && !succ.starts_with("else");
                    if istruzioni && fine_istruzione && inizio_istruzione && !separato {
                        s.push_str("; ");
                        separato = true;
                    } else {
                        s.push(' ');
                    }
                }
                SyntaxKind::Space => s.push_str(t),
                _ => {
                    s.push_str(&t.split('\n').map(str::trim).collect::<Vec<_>>().join(" "));
                }
            }
            if !matches!(
                kind,
                SyntaxKind::Space | SyntaxKind::LineComment | SyntaxKind::BlockComment
            ) {
                prec = t;
                separato = false;
            }
        }
        s
    };
    let con_spazi = tentativo(false);
    if !typst_syntax::parse(&con_spazi).diagnosis().errors {
        return (con_spazi, commenti, true);
    }
    let con_istruzioni = tentativo(true);
    if !typst_syntax::parse(&con_istruzioni).diagnosis().errors {
        return (con_istruzioni, commenti, true);
    }
    (con_spazi, commenti, false)
}

/// Se subito dopo il nodo `a` c'è il `;` che chiude un'espressione `#…;`,
/// ne restituisce l'indice; altrimenti `a`.
fn con_punto_e_virgola(figli: &[&SyntaxNode], a: usize) -> usize {
    if figli.get(a + 1).map(|n| n.kind()) == Some(SyntaxKind::Semicolon) {
        a + 1
    } else {
        a
    }
}

/// Se il nodo `k` può aprire un blocco grezzo raggruppabile (`#espr` non
/// riconosciuta, equazione display, voce di termini) ritorna l'indice finale.
fn candidato_blocco_raw(figli: &[&SyntaxNode], k: usize) -> Option<usize> {
    match figli[k].kind() {
        SyntaxKind::Hash => {
            let espr = figli.get(k + 1)?.cast::<ast::Expr>()?;
            if let ast::Expr::FuncCall(c) = espr {
                let mut scarto = Ctx::default();
                let fine = con_etichetta(figli, k + 1);
                let da_sola = su_riga_propria(figli, k, fine);
                // `solo_forma`: basta sapere SE la chiamata è riconosciuta, non
                // serve convertirne il corpo (che sarà convertito dal
                // chiamante). Convertire anche qui raddoppiava il lavoro a ogni
                // livello di annidamento: costo 2^profondità.
                if funccall_a_md(c, da_sola, fine != k + 1, &mut scarto, true).is_some() {
                    return None;
                }
            }
            Some(con_etichetta(figli, con_punto_e_virgola(figli, k + 1)))
        }
        SyntaxKind::TermItem => Some(k),
        SyntaxKind::Equation => match figli[k].cast::<ast::Equation>() {
            Some(eq) if eq.block() => Some(con_etichetta(figli, k)),
            _ => None,
        },
        _ => None,
    }
}

// ---- conversione del markup ----------------------------------------------

/// Converte il contenuto di un nodo `Markup` (documento o corpo di un
/// elemento), accodando il Markdown risultante a `out`.
fn markup_a_md(markup: ast::Markup, out: &mut String, ctx: &mut Ctx) {
    // La conversione ricorre (markup → espressione → markup): oltre il limite
    // il sotto-albero è conservato come Typst grezzo, senza perdere contenuto.
    if ctx.profondita >= PROFONDITA_MD_MAX {
        ctx.avvisa(
            "contenuto annidato troppo in profondità: la parte più interna è conservata come \
             Typst grezzo",
        );
        let testo = markup.to_untyped().full_text();
        if !testo.trim().is_empty() {
            ctx.grezzi += 1;
            if ctx.blocco_ammesso() && testo.contains('\n') {
                scrivi_raw_blocco(&testo, out);
            } else {
                scrivi_raw_inline(&testo.replace("\n", " "), out);
            }
        }
        return;
    }
    ctx.profondita += 1;
    let figli: Vec<&SyntaxNode> = markup.to_untyped().children().collect();
    let mut i = 0;
    while i < figli.len() {
        i = nodo_a_md(&figli, i, out, ctx);
    }
    ctx.profondita -= 1;
}

/// Converte il nodo `figli[i]` (con il contesto dei fratelli) e ritorna
/// l'indice del prossimo da elaborare.
fn nodo_a_md(figli: &[&SyntaxNode], i: usize, out: &mut String, ctx: &mut Ctx) -> usize {
    let n = figli[i];
    match n.kind() {
        SyntaxKind::Hash => hash_a_md(figli, i, out, ctx),
        SyntaxKind::Space => {
            // niente spazi a inizio riga o a inizio documento, né doppi (lo
            // spazio può essere già stato scritto dall'enfasi che precede)
            if !(out.is_empty() || out.ends_with('\n') || out.ends_with(' ')) {
                out.push(' ');
            }
            i + 1
        }
        SyntaxKind::LineComment | SyntaxKind::BlockComment => {
            commento_a_md(figli, i, out, ctx);
            i + 1
        }
        SyntaxKind::ListItem | SyntaxKind::EnumItem => {
            if let Some(e) = n.cast::<ast::Expr>() {
                voce_lista_a_md(e, precede_voce(figli, i), out, ctx);
            }
            i + 1
        }
        SyntaxKind::TermItem => emetti_raw(figli, i, i, out, ctx),
        SyntaxKind::Equation => match n.cast::<ast::Equation>() {
            Some(eq) if eq.block() => emetti_raw(figli, i, i, out, ctx),
            _ => {
                ctx.grezzi += 1;
                scrivi_raw_inline(&n.full_text(), out);
                i + 1
            }
        },
        SyntaxKind::Heading => match n.cast::<ast::Heading>() {
            Some(h) => titolo_a_md(figli, i, h, out, ctx),
            None => i + 1,
        },
        _ => {
            if let Some(e) = n.cast::<ast::Expr>() {
                espressione_a_md(e, out, ctx);
            }
            i + 1
        }
    }
}

fn commento_a_md(figli: &[&SyntaxNode], i: usize, out: &mut String, ctx: &Ctx) {
    let n = figli[i];
    let testo = n.full_text();
    let interno = testo
        .strip_prefix("//")
        .or_else(|| testo.strip_prefix("/*").and_then(|t| t.strip_suffix("*/")))
        .unwrap_or(&testo);
    // «--» e «>» finali romperebbero il commento HTML
    let interno = interno.trim().replace("--", "- -").replace('\n', " ");
    let html = format!("<!-- {} -->", interno.trim_end_matches('-').trim_end());
    if ctx.blocco_ammesso() && su_riga_propria(figli, i, i) {
        inizia_blocco(out);
        out.push_str(&html);
        out.push_str("\n\n");
    } else {
        out.push_str(&html);
    }
}

fn titolo_a_md(
    figli: &[&SyntaxNode],
    i: usize,
    h: ast::Heading,
    out: &mut String,
    ctx: &mut Ctx,
) -> usize {
    inizia_blocco(out);
    for _ in 0..h.depth().get().min(6) {
        out.push('#');
    }
    out.push(' ');
    let mut corpo = String::new();
    // il titolo è una sola riga: niente blocchi al suo interno
    markup_inline_a_md(h.body(), &mut corpo, ctx);
    out.push_str(corpo.trim().replace('\n', " ").as_str());
    let j = con_etichetta(figli, i);
    if j != i {
        ctx.grezzi += 1;
        out.push(' ');
        scrivi_raw_inline(&figli[j].full_text(), out);
    }
    out.push_str("\n\n");
    j + 1
}

/// `true` se prima del nodo `i` (saltando spazi e interruzioni di paragrafo)
/// c'è un'altra voce di lista: la voce continua la lista, non la apre.
fn precede_voce(figli: &[&SyntaxNode], i: usize) -> bool {
    let mut k = i;
    while k > 0 {
        k -= 1;
        match figli[k].kind() {
            SyntaxKind::Space | SyntaxKind::Parbreak => continue,
            SyntaxKind::ListItem | SyntaxKind::EnumItem => return true,
            _ => return false,
        }
    }
    false
}

fn voce_lista_a_md(e: ast::Expr, continua: bool, out: &mut String, ctx: &mut Ctx) {
    let (marcatore, corpo) = match e {
        ast::Expr::ListItem(v) => ("- ".to_string(), v.body()),
        ast::Expr::EnumItem(v) => (
            match v.number() {
                Some(n) => format!("{n}. "),
                None => "1. ".to_string(),
            },
            v.body(),
        ),
        _ => return,
    };
    if ctx.profondita_lista == 0 && !continua {
        inizia_blocco(out);
    } else {
        a_inizio_riga(out);
    }
    ctx.profondita_lista += 1;
    let mut testo = String::new();
    markup_a_md(corpo, &mut testo, ctx);
    ctx.profondita_lista -= 1;
    let rientro = " ".repeat(marcatore.len());
    out.push_str(&marcatore);
    out.push_str(&rientra_righe_successive(testo.trim(), &rientro));
    out.push('\n');
}

/// Scrive `corpo` racchiuso fra `delim` (`**`, `*`, `~~`) spostando **fuori**
/// dai delimitatori gli spazi iniziali e finali: in CommonMark `** a **` non
/// è un'enfasi (il delimitatore deve toccare il testo), mentre in Typst
/// `* a *` lo è. Un corpo fatto di soli spazi non produce delimitatori.
fn scrivi_enfasi(out: &mut String, delim: &str, corpo: &str) {
    let f = formatta_enfasi(delim, corpo);
    // niente spazio iniziale doppio o a inizio riga
    let f = if out.is_empty() || out.ends_with([' ', '\n']) {
        f.trim_start_matches(' ')
    } else {
        f.as_str()
    };
    out.push_str(f);
}

/// Converte il corpo di un'enfasi in contesto inline, ripristinando gli spazi
/// ai bordi che `markup_a_md` scarta a inizio riga (il corpo parte da un
/// buffer vuoto), così [`formatta_enfasi`] li può spostare fuori.
fn corpo_enfasi_md(markup: ast::Markup, ctx: &mut Ctx) -> String {
    let sorgente = markup.to_untyped().full_text();
    let mut t = String::new();
    markup_inline_a_md(markup, &mut t, ctx);
    if sorgente.starts_with(char::is_whitespace) && !t.starts_with(' ') {
        t.insert(0, ' ');
    }
    if sorgente.ends_with(char::is_whitespace) && !t.ends_with(' ') {
        t.push(' ');
    }
    t
}

/// Forma pura di [`scrivi_enfasi`]: i bordi di spazio del corpo diventano
/// uno spazio singolo **fuori** dai delimitatori.
fn formatta_enfasi(delim: &str, corpo: &str) -> String {
    let utile = corpo.trim();
    let mut s = String::new();
    if corpo.len() != corpo.trim_start().len() {
        s.push(' ');
    }
    if !utile.is_empty() {
        s.push_str(delim);
        s.push_str(utile);
        s.push_str(delim);
    }
    if corpo.len() != corpo.trim_end().len() && !s.ends_with(' ') {
        s.push(' ');
    }
    s
}

/// Converte una singola espressione Typst (senza contesto di fratelli) nel
/// Markdown corrispondente, ricorrendo su `Markup` innestati.
fn espressione_a_md(espressione: ast::Expr, out: &mut String, ctx: &mut Ctx) {
    match espressione {
        ast::Expr::Text(t) => push_letterale_md(out, &escapa_md(t.get())),
        ast::Expr::Space(_) => {
            if !(out.is_empty() || out.ends_with('\n') || out.ends_with(' ')) {
                out.push(' ');
            }
        }
        ast::Expr::Linebreak(_) => out.push_str("  \n"),
        ast::Expr::Parbreak(_) => inizia_blocco(out),
        ast::Expr::Escape(e) => {
            let c = e.get();
            // `1\.` a inizio riga: il punto non deve diventare marcatore di lista
            let riga = out.rsplit('\n').next().unwrap_or("");
            if matches!(c, '.' | ')')
                && !riga.is_empty()
                && riga.chars().all(|x| x.is_ascii_digit())
            {
                out.push('\\');
            }
            let mut s = String::new();
            escapa_char_md(c, &mut s);
            push_letterale_md(out, &s);
        }
        ast::Expr::Shorthand(s) => {
            let mut t = String::new();
            escapa_char_md(s.get(), &mut t);
            push_letterale_md(out, &t);
        }
        ast::Expr::SmartQuote(q) => out.push(if q.double() { '"' } else { '\'' }),
        ast::Expr::Strong(s) => {
            let t = corpo_enfasi_md(s.body(), ctx);
            scrivi_enfasi(out, "**", &t);
        }
        ast::Expr::Emph(e) => {
            let t = corpo_enfasi_md(e.body(), ctx);
            scrivi_enfasi(out, "*", &t);
        }
        ast::Expr::Raw(r) => raw_a_md(r, out),
        ast::Expr::Link(l) => {
            out.push('<');
            out.push_str(l.get());
            out.push('>');
        }
        // Costrutti senza equivalente CommonMark diretto (math inline,
        // riferimenti, etichette, …): span Typst grezzo in linea.
        altro => {
            let testo = altro.to_untyped().full_text();
            if !testo.is_empty() {
                ctx.grezzi += 1;
                scrivi_raw_inline(&testo, out);
            }
        }
    }
}

/// Gestisce `#espressione`: riconosce le chiamate mappabili su Markdown,
/// altrimenti conserva il sorgente (con il `#`) come Typst grezzo.
fn hash_a_md(figli: &[&SyntaxNode], i: usize, out: &mut String, ctx: &mut Ctx) -> usize {
    let Some(espr) = figli.get(i + 1).and_then(|n| n.cast::<ast::Expr>()) else {
        return i + 1;
    };
    if let ast::Expr::FuncCall(c) = espr {
        let fine = con_etichetta(figli, i + 1);
        let da_sola = ctx.blocco_ammesso() && su_riga_propria(figli, i, fine);
        // Prova a secco (senza convertire i corpi né toccare `ctx`): un
        // elemento a blocco (citazione, figura) dentro un contesto inline non
        // ha forma Markdown valida e resta Typst grezzo.
        let forma = funccall_a_md(c, da_sola, fine != i + 1, &mut Ctx::default(), true);
        let ammessa = forma
            .as_ref()
            .is_some_and(|f| !f.blocco || ctx.blocco_ammesso());
        if ammessa {
            if let Some(resa) = funccall_a_md(c, da_sola, fine != i + 1, ctx, false) {
                if resa.blocco {
                    inizia_blocco(out);
                    out.push_str(&resa.testo);
                    out.push_str("\n\n");
                } else {
                    // lo spazio iniziale non va raddoppiato con quello già scritto
                    let t = if out.is_empty() || out.ends_with([' ', '\n']) {
                        resa.testo.trim_start_matches(' ')
                    } else {
                        resa.testo.as_str()
                    };
                    out.push_str(t);
                }
                return i + 2;
            }
        }
    }
    emetti_raw(figli, i, i + 1, out, ctx)
}

/// Esito della conversione di una chiamata riconosciuta.
struct Resa {
    testo: String,
    /// `true` per un elemento a blocco (preceduto/seguito da riga vuota).
    blocco: bool,
}

/// Plaintext (con escape Markdown) di un corpo Typst fatto solo di testo
/// semplice; `None` se contiene altro (enfasi, math, riferimenti, …).
fn testo_semplice_md(markup: ast::Markup) -> Option<String> {
    let mut s = String::new();
    for n in markup.to_untyped().children() {
        match n.cast::<ast::Expr>()? {
            ast::Expr::Text(t) => s.push_str(&escapa_md(t.get())),
            ast::Expr::Space(_) => s.push(' '),
            ast::Expr::Escape(e) => escapa_char_md(e.get(), &mut s),
            ast::Expr::Shorthand(e) => escapa_char_md(e.get(), &mut s),
            ast::Expr::SmartQuote(q) => s.push(if q.double() { '"' } else { '\'' }),
            _ => return None,
        }
    }
    Some(s)
}

/// Riconosce le chiamate di funzione Typst emesse da `md_a_typst` per i
/// costrutti Markdown nativi (`link`, `image`, `strike`, `quote`, `figure`,
/// `footnote`) più `strong`/`emph`, e le converte nel Markdown corrispondente.
/// Qualunque altra chiamata — o una di queste con argomenti in una forma che
/// il Markdown non può esprimere senza perdere informazione — ritorna `None`
/// e il chiamante la conserva come Typst grezzo: mai un panico su una
/// `FuncCall` arbitraria.
fn funccall_a_md(
    chiamata: ast::FuncCall,
    da_sola: bool,
    con_etichetta: bool,
    ctx: &mut Ctx,
    solo_forma: bool,
) -> Option<Resa> {
    let ast::Expr::Ident(nome) = chiamata.callee() else {
        return None;
    };
    // Con `solo_forma` si stabilisce soltanto SE la chiamata è riconosciuta
    // (il testo della `Resa` è vuoto, i corpi non sono convertiti e `ctx` non
    // cambia): serve a `candidato_blocco_raw` e a `hash_a_md`, che altrimenti
    // convertirebbero ogni corpo due volte per livello (costo 2^profondità).
    let converti = |corpo: ast::Markup, inline_ctx: bool, ctx: &mut Ctx| -> String {
        if solo_forma {
            return String::new();
        }
        let mut t = String::new();
        if inline_ctx {
            markup_inline_a_md(corpo, &mut t, ctx);
        } else {
            markup_a_md(corpo, &mut t, ctx);
        }
        t
    };
    let args = chiamata.args();
    let nominati: Vec<(String, ast::Expr)> = args
        .items()
        .filter_map(|a| match a {
            ast::Arg::Named(n) => Some((n.name().as_str().to_string(), n.expr())),
            _ => None,
        })
        .collect();
    let ha_spread = args.items().any(|a| matches!(a, ast::Arg::Spread(_)));
    if ha_spread {
        return None;
    }
    let solo_nominati =
        |ammessi: &[&str]| nominati.iter().all(|(n, _)| ammessi.contains(&n.as_str()));
    let inline = |testo: String| Resa {
        testo,
        blocco: false,
    };
    match nome.as_str() {
        // `#link("url")[testo]` → `[testo](url)`; senza corpo esplicito
        // (`#link("url")`, che in Typst userebbe l'URL come corpo) → `[url](url)`.
        "link" => {
            let (url, corpo) = argomenti_stringa_e_corpo(chiamata)?;
            let mut s = String::from("[");
            match corpo {
                Some(corpo) => {
                    let t = converti(corpo, true, ctx);
                    s.push_str(t.trim());
                }
                None => s.push_str(&escapa_md(&url)),
            }
            s.push_str("](");
            s.push_str(&formatta_destinazione_md(&url));
            s.push(')');
            Some(inline(s))
        }
        // `#image("percorso")` → `![](percorso)`; con `alt:` → `![alt](percorso)`.
        // Altri argomenti (larghezza, …) non hanno posto in Markdown: grezzo.
        "image" => {
            if !solo_nominati(&["alt"]) {
                return None;
            }
            let (percorso, corpo) = argomenti_stringa_e_corpo(chiamata)?;
            if corpo.is_some() {
                return None;
            }
            let alt = match nominati.iter().find(|(n, _)| n == "alt") {
                Some((_, ast::Expr::Str(s))) => escapa_md(s.get().as_str()),
                Some(_) => return None,
                None => String::new(),
            };
            Some(inline(format!(
                "![{alt}]({})",
                formatta_destinazione_md(&percorso)
            )))
        }
        // `#strike[testo]` → `~~testo~~`, `#strong`/`#emph` → `**`/`*`.
        "strike" | "strong" | "emph" => {
            if !nominati.is_empty() {
                return None;
            }
            let corpo = corpo_di(chiamata)?;
            let t = if solo_forma {
                String::new()
            } else {
                corpo_enfasi_md(corpo, ctx)
            };
            let m = match nome.as_str() {
                "strike" => "~~",
                "strong" => "**",
                _ => "*",
            };
            Some(inline(formatta_enfasi(m, &t)))
        }
        // `#footnote[testo]` → `[^N]` + definizione in coda.
        "footnote" => {
            if !nominati.is_empty() {
                return None;
            }
            let corpo = corpo_di(chiamata)?;
            if solo_forma {
                return Some(inline(String::new()));
            }
            let t = converti(corpo, false, ctx);
            ctx.note.push(t);
            Some(inline(format!("[^{}]", ctx.note.len())))
        }
        // `#quote(block: true)[testo]` (o `#quote[testo]` su riga propria) → `> testo`.
        "quote" => {
            if !solo_nominati(&["block", "attribution", "quotes"]) {
                return None;
            }
            if nominati.iter().any(|(n, _)| n == "quotes") {
                return None;
            }
            let blocco = nominati
                .iter()
                .any(|(n, e)| n == "block" && matches!(e, ast::Expr::Bool(b) if b.get()));
            if !blocco && !da_sola {
                return None;
            }
            let corpo = corpo_di(chiamata)?;
            if solo_forma {
                return Some(Resa {
                    testo: String::new(),
                    blocco: true,
                });
            }
            let t = converti(corpo, false, ctx);
            let mut testo = t.trim().to_string();
            if let Some((_, att)) = nominati.iter().find(|(n, _)| n == "attribution") {
                let mut a = String::new();
                match att {
                    ast::Expr::ContentBlock(cb) => markup_inline_a_md(cb.body(), &mut a, ctx),
                    ast::Expr::Str(s) => a.push_str(&escapa_md(s.get().as_str())),
                    altro => {
                        ctx.grezzi += 1;
                        scrivi_raw_inline(&altro.to_untyped().full_text(), &mut a);
                    }
                }
                ctx.avvisa(
                    "l'attribuzione di una citazione non esiste in CommonMark: convertita in una \
                     riga finale «— attribuzione» all'interno della citazione",
                );
                testo.push_str("\n\n— ");
                testo.push_str(a.trim());
            }
            let prefissato = testo
                .split('\n')
                .map(|r| {
                    if r.is_empty() {
                        ">".to_string()
                    } else {
                        format!("> {r}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(Resa {
                testo: prefissato,
                blocco: true,
            })
        }
        // `#figure(image("p"), caption: [testo])` → `![testo](p)`. Qualunque
        // altra forma (corpo diverso, etichetta, kind, …) resta grezza.
        "figure" => {
            if con_etichetta || !solo_nominati(&["caption"]) {
                return None;
            }
            let pos: Vec<ast::Expr> = args
                .items()
                .filter_map(|a| match a {
                    ast::Arg::Pos(e) => Some(e),
                    _ => None,
                })
                .collect();
            let [ast::Expr::FuncCall(img)] = pos.as_slice() else {
                return None;
            };
            if !matches!(img.callee(), ast::Expr::Ident(i) if i.as_str() == "image") {
                return None;
            }
            let img_args = img.args();
            if img_args.items().any(|a| !matches!(a, ast::Arg::Pos(_))) {
                return None;
            }
            let (percorso, corpo) = argomenti_stringa_e_corpo(*img)?;
            if corpo.is_some() {
                return None;
            }
            let didascalia = match nominati.iter().find(|(n, _)| n == "caption") {
                Some((_, ast::Expr::ContentBlock(cb))) => testo_semplice_md(cb.body())?,
                Some((_, ast::Expr::Str(s))) => escapa_md(s.get().as_str()),
                _ => return None,
            };
            Some(Resa {
                testo: format!("![{didascalia}]({})", formatta_destinazione_md(&percorso)),
                blocco: true,
            })
        }
        _ => None,
    }
}

/// Trova il primo argomento posizionale stringa (url/percorso) di una
/// `FuncCall`, e l'eventuale content block posizionale successivo (corpo).
/// Tollera argomenti nominati framezzo (es. `#link("u", alt: "…")[…]`) e un
/// ordine diverso: cerca semplicemente il primo `Str` e il primo
/// `ContentBlock` fra gli argomenti posizionali. `None` se non c'è nessun
/// argomento posizionale stringa.
fn argomenti_stringa_e_corpo(chiamata: ast::FuncCall) -> Option<(String, Option<ast::Markup>)> {
    let mut url = Option::None;
    let mut corpo = Option::None;
    for arg in chiamata.args().items() {
        if let ast::Arg::Pos(espressione) = arg {
            match espressione {
                ast::Expr::Str(s) if url.is_none() => url = Some(s.get().to_string()),
                ast::Expr::ContentBlock(cb) if corpo.is_none() => corpo = Some(cb.body()),
                _ => {}
            }
        }
    }
    url.map(|url| (url, corpo))
}

/// Trova il primo argomento posizionale content block (`[…]`) di una
/// `FuncCall`, indipendentemente da eventuali argomenti nominati framezzo
/// (es. `#quote(block: true)[…]`). `None` se non c'è.
fn corpo_di(chiamata: ast::FuncCall) -> Option<ast::Markup> {
    chiamata.args().items().find_map(|arg| match arg {
        ast::Arg::Pos(ast::Expr::ContentBlock(cb)) => Some(cb.body()),
        _ => Option::None,
    })
}

/// Formatta una destinazione di `link`/`image` per CommonMark.
///
/// La sintassi `[testo](dest)` senza parentesi angolari interpreta lo spazio
/// come fine della destinazione e tratta `(`/`)` come delimitatori
/// annidabili: una destinazione che contiene uno di questi caratteri (o
/// `<`/`>`, che aprirebbero la stessa ambiguità nella forma alternativa)
/// va quindi racchiusa fra parentesi angolari CommonMark esplicite
/// (`<dest>`), con `<`/`>` interni percent-encodati (altrimenti
/// romperebbero anche questa sintassi). Esempio: `#image("logo bianco.png")`
/// → `![](<logo bianco.png>)`, non `![](logo bianco.png)` (che CommonMark
/// leggerebbe come destinazione tronca a `logo`).
fn formatta_destinazione_md(dest: &str) -> String {
    let necessita_angolari = dest
        .chars()
        .any(|c| matches!(c, ' ' | '<' | '>' | '(' | ')'));
    if !necessita_angolari {
        return dest.to_string();
    }
    let mut esito = String::with_capacity(dest.len() + 2);
    esito.push('<');
    for c in dest.chars() {
        match c {
            '<' => esito.push_str("%3C"),
            '>' => esito.push_str("%3E"),
            _ => esito.push(c),
        }
    }
    esito.push('>');
    esito
}

/// Converte un nodo `Raw` (codice inline o a blocco) in Markdown.
fn raw_a_md(raw: ast::Raw, out: &mut String) {
    if raw.block() {
        let mut contenuto = String::new();
        let mut prima = true;
        for linea in raw.lines() {
            if !prima {
                contenuto.push('\n');
            }
            prima = false;
            contenuto.push_str(linea.get());
        }

        // Fence dinamico (stessa idea di `md_a_typst` verso Typst, vedi il
        // commento su `buffer_codice`): lunghezza = run massima di backtick
        // consecutivi nel contenuto + 1, minimo 3. Così un blocco Typst che
        // contiene esso stesso una riga di ``` non chiude il fence Markdown
        // in anticipo.
        let n = (run_massima_backtick(&contenuto) + 1).max(3);
        let delimitatore = "`".repeat(n);

        inizia_blocco(out);
        out.push_str(&delimitatore);
        if let Some(lingua) = raw.lang() {
            out.push_str(lingua.get());
        }
        out.push('\n');
        out.push_str(&contenuto);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&delimitatore);
        out.push_str("\n\n");
    } else {
        let mut contenuto = String::new();
        for linea in raw.lines() {
            contenuto.push_str(linea.get());
        }
        racchiudi_in_backtick_md(&contenuto, out);
    }
}

/// Esegue l'escape dei metacaratteri Markdown più comuni
/// (`` \ ` * _ # [ ] < > ``) all'interno di testo altrimenti letterale.
fn escapa_md(testo: &str) -> String {
    let mut esito = String::with_capacity(testo.len());
    let mut it = testo.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            // `&amp;` letterale in Typst sono cinque caratteri; in Markdown
            // sarebbe un'entità HTML (una `&`). Si scappa la `&` quando può
            // aprire un'entità (`&nome;`, `&#35;`), cioè se seguita da
            // alfanumerico o `#`, oppure se chiude il nodo (il successivo
            // potrebbe proseguire l'entità). Le `&` isolate restano leggibili.
            if it
                .peek()
                .is_none_or(|n| n.is_ascii_alphanumeric() || *n == '#')
            {
                esito.push('\\');
            }
            esito.push('&');
        } else {
            escapa_char_md(c, &mut esito);
        }
    }
    esito
}

/// Accoda un singolo carattere a `out`, con l'escape se è un metacarattere
/// Markdown (usata sia per il testo semplice sia per i caratteri decodificati
/// da `Escape`/`Shorthand` Typst, che potrebbero collidere con la sintassi
/// Markdown di destinazione).
fn escapa_char_md(c: char, out: &mut String) {
    // `&` sempre: un carattere decodificato da un escape non ha contesto, e
    // potrebbe formare un'entità con il testo vicino
    if matches!(
        c,
        '\\' | '`' | '*' | '_' | '#' | '[' | ']' | '<' | '>' | '&'
    ) {
        out.push('\\');
    }
    out.push(c);
}

#[cfg(test)]
mod test {
    use super::*;

    // ---- md → typst ----

    #[test]
    fn md_heading() {
        assert_eq!(md_a_typst("# Titolo").trim(), "= Titolo");
        assert_eq!(md_a_typst("### Sotto").trim(), "=== Sotto");
    }

    #[test]
    fn md_strong_ed_emph() {
        assert_eq!(md_a_typst("**bold**").trim(), "*bold*");
        assert_eq!(md_a_typst("*it*").trim(), "_it_");
    }

    #[test]
    fn md_strikethrough() {
        assert_eq!(md_a_typst("~~via~~").trim(), "#strike[via]");
    }

    #[test]
    fn md_lista_puntata_e_ordinata() {
        let puntata = md_a_typst("- a\n- b\n");
        assert!(puntata.contains("- a"));
        assert!(puntata.contains("- b"));

        let ordinata = md_a_typst("1. a\n2. b\n");
        assert!(ordinata.contains("+ a"));
        assert!(ordinata.contains("+ b"));
    }

    #[test]
    fn md_codice_inline_e_blocco() {
        assert_eq!(md_a_typst("`x`").trim(), "`x`");
        let blocco = md_a_typst("```rust\nfn f() {}\n```\n");
        assert!(blocco.contains("```rust"));
        assert!(blocco.contains("fn f() {}"));
    }

    #[test]
    fn md_codice_inline_con_backtick_interno() {
        // Contenuto del code span: "`x`" (inizia e finisce con un backtick).
        // Un solo backtick come delimitatore lo confonderebbe con la
        // chiusura; DUE backtick sarebbero ANCH'ESSI sbagliati verso Typst,
        // che tratta due backtick consecutivi come raw span sempre vuoto a
        // prescindere da cosa segue (vedi `racchiudi_in_backtick_typst`):
        // serve un delimitatore di almeno 3 backtick.
        let typst = md_a_typst("`` `x` ``");
        let aperti = typst.trim_start().chars().take_while(|&c| c == '`').count();
        assert_eq!(
            aperti, 3,
            "delimitatore atteso di 3 backtick (mai 2): {typst:?}"
        );
    }

    #[test]
    fn md_fence_info_string_con_piu_token_usa_solo_il_primo() {
        let out = md_a_typst("```js title=\"x\"\ncodice\n```\n");
        assert!(
            out.contains("```js\n"),
            "atteso solo 'js' come lingua: {out:?}"
        );
        assert!(!out.contains("title"));
    }

    #[test]
    fn md_fence_lingua_non_identificatore_viene_scartata() {
        // Fence a tilde con un backtick nell'info string (CommonMark lo
        // consente solo per le fence a tilde, non per quelle a backtick):
        // il token non è un identificatore semplice valido, va scartato
        // invece di finire letteralmente nel tag di linguaggio del fence
        // Typst generato.
        let out = md_a_typst("~~~js`sporco\ncodice\n~~~\n");
        assert!(
            !out.contains("js`sporco"),
            "il token lingua non valido non deve comparire nell'output: {out:?}"
        );
        assert!(
            out.contains("```\ncodice"),
            "senza lingua valida il fence deve aprirsi senza tag: {out:?}"
        );
    }

    #[test]
    fn md_blocco_codice_con_riga_di_tre_backtick_usa_fence_a_quattro() {
        // Il blocco Markdown contiene esso stesso una riga di 3 backtick:
        // il fence Typst generato deve usarne almeno 4 per non chiudersi in
        // anticipo su quella riga interna.
        let out = md_a_typst("````\ncontiene una riga:\n```\ninterna\n````\n");
        let aperti = out.trim_start().chars().take_while(|&c| c == '`').count();
        assert_eq!(
            aperti, 4,
            "fence atteso a 4 backtick (run interna massima 3 + 1): {out:?}"
        );
        assert!(out.contains("```\ninterna"));
    }

    #[test]
    fn md_link_e_immagine() {
        assert_eq!(
            md_a_typst("[testo](https://esempio.it)").trim(),
            "#link(\"https://esempio.it\")[testo]"
        );
        assert_eq!(md_a_typst("![](img.png)").trim(), "#image(\"img.png\")");
    }

    #[test]
    fn md_immagine_con_alt_sola_diventa_figura_con_didascalia() {
        assert_eq!(
            md_a_typst("![alt](img.png)").trim(),
            "#figure(image(\"img.png\"), caption: [alt])"
        );
    }

    #[test]
    fn md_immagine_con_alt_in_linea_usa_il_parametro_alt() {
        let out = md_a_typst("Vedi ![logo](l.png) qui.");
        assert!(out.contains("#image(\"l.png\", alt: \"logo\")"), "{out}");
        assert!(!out.contains("#figure"), "{out}");
    }

    #[test]
    fn md_link_con_parentesi_quadra_nel_testo_non_rompe_il_content_block() {
        // Il testo del link contiene una `]` letterale (via escape Markdown
        // `\]`): senza l'escape sul lato Typst chiuderebbe in anticipo il
        // content block `#link(..)[…]`, troncando il testo e lasciando
        // markup spurio nel resto del documento.
        let out = md_a_typst(r"[testo \] finale](https://esempio.it)");
        assert_eq!(
            out.trim(),
            "#link(\"https://esempio.it\")[testo \\] finale]"
        );
    }

    #[test]
    fn md_blockquote_e_linea_orizzontale() {
        assert!(md_a_typst("> citazione\n").contains("#quote(block: true)["));
        assert!(md_a_typst("---\n").contains("#line(length: 100%)"));
    }

    #[test]
    fn md_lista_seguita_da_doppio_a_capo() {
        let out = md_a_typst("- a\n- b\n\nparagrafo dopo\n");
        // Fra l'ultima voce della lista e il paragrafo successivo ci devono
        // essere (almeno) due newline consecutivi, non uno solo. (Un semplice
        // `contains("\n\n")` sarebbe sempre vero per l'a capo dopo ogni
        // titolo/paragrafo: si controlla il punto preciso di giunzione.)
        assert!(
            out.contains("- b\n\nparagrafo dopo"),
            "atteso un doppio a capo dopo la lista: {out:?}"
        );
        // verifica indipendente: per Typst la lista ha 2 voci e il paragrafo
        // seguente non ne fa parte
        let radice = typst_syntax::parse(&out);
        let voci = radice
            .children()
            .filter(|n| n.kind() == SyntaxKind::ListItem)
            .count();
        assert_eq!(voci, 2, "{out:?}");
    }

    #[test]
    fn md_lista_annidata_non_spezzata_da_riga_vuota() {
        // La sotto-lista (b1) è annidata dentro la voce "b" della lista di
        // livello 0: la sua chiusura non deve inserire una riga vuota fra
        // "b1" e "c" (livello 0), altrimenti in Typst la lista genitrice si
        // spezzerebbe in due liste separate.
        let out = md_a_typst("- a\n- b\n  - b1\n- c\n");
        assert!(
            !out.contains("b1\n\n- c"),
            "la chiusura della sotto-lista non deve spezzare la lista genitrice: {out:?}"
        );
        assert!(out.contains("- a"));
        assert!(out.contains("- b1"));
        assert!(out.contains("- c"));
    }

    #[test]
    fn md_tabella() {
        let out = md_a_typst("| a | b |\n|---|---|\n| 1 | 2 |\n");
        assert!(out.contains("#table("));
        assert!(out.contains("columns: 2"));
        // ogni cella è un content block chiuso, in ordine di riga
        let celle: Vec<&str> = out
            .lines()
            .map(str::trim)
            .filter(|r| r.starts_with('[') && r.ends_with("],"))
            .collect();
        assert_eq!(celle, ["[a],", "[b],", "[1],", "[2],"], "{out:?}");
        assert!(senza_errori_typst(&out), "{out:?}");
    }

    /// `true` se il Typst non ha errori di sintassi (parser indipendente).
    fn senza_errori_typst(typ: &str) -> bool {
        !typst_syntax::parse(typ).diagnosis().errors
    }

    #[test]
    fn md_escaping_caratteri_speciali_typst() {
        let out = md_a_typst("prezzo: 3$ e #tag @rif");
        assert!(out.contains("\\$"));
        assert!(out.contains("\\#"));
        assert!(out.contains("\\@"));
    }

    #[test]
    fn md_escaping_slash_e_uguale_non_aprono_commento_o_titolo() {
        // Senza l'escape di `/`, `//` aprirebbe un commento di riga Typst
        // (troncando silenziosamente il resto della riga); senza l'escape
        // di `=`, un testo che comincia per `=` diventerebbe un titolo.
        let out = md_a_typst("vedi http://esempio.it e and/or");
        assert!(
            out.contains("http:\\/\\/esempio.it"),
            "gli slash di un URL letterale vanno scappati: {out:?}"
        );
        assert!(
            out.contains("and\\/or"),
            "uno slash isolato va scappato comunque (context-free): {out:?}"
        );

        let uguale = md_a_typst("= non un titolo Typst");
        assert!(
            uguale.contains("\\= non un titolo Typst"),
            "un `=` letterale a inizio riga non deve diventare un titolo Typst: {uguale:?}"
        );
    }

    #[test]
    fn md_escaping_quadre_e_backtick_letterali() {
        // `\[`, `\]`, `` \` `` sono escape *Markdown*: producono i caratteri
        // letterali `[`, `]`, backtick come testo semplice (non link/code).
        let out = md_a_typst(r"testo con \[quadre\] e \`backtick\` letterali");
        assert!(out.contains("\\["));
        assert!(out.contains("\\]"));
        assert!(out.contains("\\`"));
    }

    // ---- typst → md ----

    #[test]
    fn typst_heading() {
        assert_eq!(typst_a_md("= Titolo").trim(), "# Titolo");
        assert_eq!(typst_a_md("=== Sotto").trim(), "### Sotto");
    }

    #[test]
    fn typst_strong_ed_emph() {
        assert_eq!(typst_a_md("*bold*").trim(), "**bold**");
        assert_eq!(typst_a_md("_it_").trim(), "*it*");
    }

    #[test]
    fn typst_raw_inline_e_blocco() {
        assert_eq!(typst_a_md("`x`").trim(), "`x`");
        let blocco = typst_a_md("```rust\nfn f() {}\n```");
        assert!(blocco.contains("```rust"));
        assert!(blocco.contains("fn f() {}"));
    }

    #[test]
    fn typst_raw_inline_con_backtick_interno() {
        // Sorgente Typst valido con un raw span a 3 backtick (2 sarebbe il
        // caso speciale "raw span vuoto" di Typst — vedi doc di modulo) il
        // cui contenuto, "1`2", ha un backtick interno non ai bordi. Il
        // prefisso numerico evita che "1" venga scambiato per un tag di
        // linguaggio (i tag Typst non possono iniziare con una cifra). Il
        // Markdown risultante usa 2 backtick (run interna massima 1 + 1),
        // senza bisogno di padding.
        assert_eq!(typst_a_md("```1`2```").trim(), "``1`2``");
    }

    #[test]
    fn typst_raw_blocco_con_riga_di_tre_backtick_usa_fence_a_quattro() {
        // Blocco Typst il cui contenuto contiene esso stesso una riga di 3
        // backtick (fence a 4 backtick per contenerlo, valido in Typst come
        // in Markdown: fence più lungo di qualunque run interna). Il
        // Markdown generato deve usare un fence di almeno 4 backtick, non 3
        // (che chiuderebbe il blocco in anticipo su quella riga interna).
        let out = typst_a_md("````\ncontiene una riga:\n```\ninterna\n````");
        let aperti = out.trim_start().chars().take_while(|&c| c == '`').count();
        assert_eq!(
            aperti, 4,
            "fence Markdown atteso a 4 backtick (run interna massima 3 + 1): {out:?}"
        );
        assert!(out.contains("```\ninterna"));
    }

    #[test]
    fn typst_link_funccall_esplicito() {
        assert_eq!(
            typst_a_md("#link(\"https://esempio.it\")[testo]").trim(),
            "[testo](https://esempio.it)"
        );
    }

    #[test]
    fn typst_link_funccall_senza_corpo_usa_url_come_testo() {
        assert_eq!(
            typst_a_md("#link(\"https://esempio.it\")").trim(),
            "[https://esempio.it](https://esempio.it)"
        );
    }

    #[test]
    fn typst_image_funccall() {
        assert_eq!(typst_a_md("#image(\"img.png\")").trim(), "![](img.png)");
    }

    #[test]
    fn typst_image_funccall_con_spazio_usa_angolari() {
        // Una destinazione con spazio non racchiusa fra `<...>` sarebbe
        // troncata dalla sintassi CommonMark `(dest)` al primo spazio.
        assert_eq!(
            typst_a_md("#image(\"logo bianco.png\")").trim(),
            "![](<logo bianco.png>)"
        );
    }

    #[test]
    fn typst_link_funccall_con_parentesi_usa_angolari_e_percent_encode() {
        let out = typst_a_md("#link(\"https://esempio.it/a(b)\")[testo]");
        assert_eq!(out.trim(), "[testo](<https://esempio.it/a(b)>)");

        // `<`/`>` interni vanno percent-encodati per non rompere anche la
        // sintassi `<dest>`.
        let out_angolari = typst_a_md("#link(\"https://esempio.it/<x>\")[testo]");
        assert_eq!(out_angolari.trim(), "[testo](<https://esempio.it/%3Cx%3E>)");
    }

    #[test]
    fn typst_strike_funccall() {
        assert_eq!(typst_a_md("#strike[via]").trim(), "~~via~~");
    }

    #[test]
    fn typst_quote_funccall() {
        assert!(typst_a_md("#quote(block: true)[citazione]").contains("> citazione"));
        assert!(typst_a_md("#quote[citazione]").contains("> citazione"));
    }

    #[test]
    fn typst_funccall_sconosciuta_ripiega_su_testo_grezzo() {
        // Non deve andare in panico né scomparire: ripiega sul testo grezzo.
        let out = typst_a_md("#foo(1, 2)");
        assert!(out.contains("foo(1, 2)"));
    }

    #[test]
    fn typst_link_funccall_con_argomento_inatteso_ripiega_su_testo_grezzo() {
        // Nessun argomento posizionale stringa: forma inattesa, niente
        // panico, ripiega sul testo grezzo della chiamata.
        let out = typst_a_md("#link(url: \"https://esempio.it\")[testo]");
        assert!(out.contains("link"));
    }

    #[test]
    fn typst_liste() {
        let puntata = typst_a_md("- a\n- b");
        assert!(puntata.contains("- a"));
        assert!(puntata.contains("- b"));

        let numerata = typst_a_md("+ a\n+ b");
        assert!(numerata.contains("1. a"));
        assert!(numerata.contains("1. b"));
    }

    #[test]
    fn typst_link_url_nuda() {
        assert_eq!(
            typst_a_md("https://esempio.it").trim(),
            "<https://esempio.it>"
        );
    }

    #[test]
    fn typst_linebreak_e_parbreak() {
        assert!(typst_a_md("riga\\\naltra").contains("  \n"));
        assert!(typst_a_md("uno\n\ndue").contains("\n\n"));
    }

    #[test]
    fn typst_escaping_metacaratteri_markdown() {
        // `\#`, `\[`, `\]` sono sequenze di escape *Typst*: producono i
        // caratteri letterali `#`, `[`, `]` come testo — non i delimitatori
        // di markup `*`/`_`, che in sorgente Typst hanno un altro
        // significato (Strong/Emph) e non arriverebbero mai come `Text`
        // letterale. Nel Markdown di destinazione `#`/`[`/`]` sono a loro
        // volta metacaratteri e vanno quindi scappati di nuovo.
        let out = typst_a_md(r"letterale \# e \[x\]");
        assert!(out.contains("\\#"));
        assert!(out.contains("\\["));
        assert!(out.contains("\\]"));
    }

    #[test]
    fn typst_costrutto_non_mappato_e_riprodotto_grezzo() {
        // `#let` è codice Typst, senza equivalente CommonMark: deve comparire
        // testualmente nell'output (best-effort), non essere perso.
        let out = typst_a_md("#let x = 1");
        // il `#` fa parte del sorgente conservato: dentro un blocco `{=typst}`
        assert!(out.contains("```{=typst}\n#let x = 1\n```"), "{out:?}");
    }

    // ---- round-trip (best-effort, non byte-per-byte) ----

    #[test]
    fn round_trip_md_typst_md_documento_semplice() {
        let originale = "# Titolo\n\nUn paragrafo con **grassetto** e *corsivo*.\n\n- uno\n- due\n\n[link](https://esempio.it) e `codice`.\n";

        let typst = md_a_typst(originale);
        let tornato = typst_a_md(&typst);

        assert!(tornato.contains("# Titolo"));
        assert!(tornato.contains("**grassetto**"));
        assert!(tornato.contains("*corsivo*"));
        assert!(tornato.contains("- uno"));
        assert!(tornato.contains("- due"));
        // Assunzione stretta: il `#link(..)[…]` generato da `md_a_typst` va
        // ora riconosciuto esplicitamente da `typst_a_md` (non solo "la
        // sottostringa 'esempio.it' compare da qualche parte", che
        // passerebbe anche col vecchio fallback grezzo che perdeva la
        // sintassi Markdown del link).
        assert!(tornato.contains("[link](https://esempio.it)"));
        assert!(tornato.contains("`codice`"));
    }

    #[test]
    fn round_trip_link_md_typst_md() {
        let originale = "[testo del link](https://esempio.it/pagina)";
        let typst = md_a_typst(originale);
        let tornato = typst_a_md(&typst);
        assert_eq!(
            tornato.trim(),
            "[testo del link](https://esempio.it/pagina)"
        );
    }

    #[test]
    fn round_trip_immagine_md_typst_md() {
        let originale = "![didascalia](img.png)";
        let typst = md_a_typst(originale);
        let tornato = typst_a_md(&typst);
        assert_eq!(tornato.trim(), "![didascalia](img.png)");
        let senza_alt = typst_a_md(&md_a_typst("![](img.png)"));
        assert_eq!(senza_alt.trim(), "![](img.png)");
    }

    // ---- Typst grezzo, note, figure, citazioni (correzione bug del `#`) ----

    /// Righe del Markdown fuori dai fence di codice.
    fn fuori_dai_fence(md: &str) -> String {
        let mut dentro = false;
        let mut out = String::new();
        for r in md.lines() {
            if r.trim_start().starts_with("```") {
                dentro = !dentro;
                continue;
            }
            if !dentro {
                out.push_str(r);
                out.push('\n');
            }
        }
        out
    }

    #[test]
    fn typst_set_show_let_non_diventano_testo_visibile() {
        // Bug originale: il `#` non faceva parte dell'`Expr`, quindi
        // `#set`/`#show`/`#let` uscivano come testo ("set text(…)").
        let src = "#set text(lang: \"it\")\n#show heading: it => it\n#let x = 1\n\nCiao\n";
        let c = typst_a_md_con_avvisi(src);
        assert!(
            c.contenuto.contains("```{=typst}\n#set text(lang: \"it\")"),
            "{}",
            c.contenuto
        );
        assert!(c.contenuto.contains("#show heading: it => it"));
        assert!(c.contenuto.contains("#let x = 1"));
        // un solo blocco grezzo per le tre righe consecutive
        assert_eq!(
            c.contenuto.matches("```{=typst}").count(),
            1,
            "{}",
            c.contenuto
        );
        // fuori dal fence resta solo il testo
        assert_eq!(fuori_dai_fence(&c.contenuto).trim(), "Ciao");
        assert!(
            c.avvisi.iter().any(|a| a.contains("Typst grezzo")),
            "{:?}",
            c.avvisi
        );
    }

    #[test]
    fn typst_funzioni_di_layout_restano_con_il_cancelletto() {
        for src in [
            "#align(center)[Testo]",
            "#v(8pt)",
            "#pagebreak()",
            "#h(1fr)",
        ] {
            let md = typst_a_md(src);
            assert!(md.contains(src), "{src} -> {md}");
            assert!(md.contains("```{=typst}"), "{src} -> {md}");
        }
    }

    #[test]
    fn typst_espressione_in_linea_diventa_span_grezzo() {
        let md = typst_a_md("prima #emoji.heart dopo #footnote(\"x\") fine");
        assert!(md.contains("`#emoji.heart`{=typst}"), "{md}");
        assert!(md.contains("`#footnote(\"x\")`{=typst}"), "{md}");
    }

    #[test]
    fn typst_math_riferimenti_ed_etichette_sono_preservati() {
        let md = typst_a_md("Vale $x^2$ come in @eq e <ancora>.\n\n$ a = b $ <eq>\n");
        assert!(md.contains("`$x^2$`{=typst}"), "{md}");
        assert!(md.contains("`@eq`{=typst}"), "{md}");
        assert!(md.contains("`<ancora>`{=typst}"), "{md}");
        assert!(md.contains("```{=typst}\n$ a = b $ <eq>\n```"), "{md}");
    }

    #[test]
    fn typst_etichetta_di_titolo_resta_sulla_riga_del_titolo() {
        let md = typst_a_md("= Titolo <lbl>\n\ntesto");
        assert!(md.starts_with("# Titolo `<lbl>`{=typst}\n"), "{md}");
    }

    #[test]
    fn typst_figure_con_didascalia() {
        let md = typst_a_md("#figure(image(\"img/a.png\"), caption: [Una figura])");
        assert_eq!(md.trim(), "![Una figura](img/a.png)");
        // con etichetta (o opzioni) non è esprimibile: resta Typst
        let md = typst_a_md("#figure(image(\"a.png\"), caption: [Una figura]) <fig>");
        assert!(
            md.contains("```{=typst}\n#figure(image(\"a.png\"), caption: [Una figura]) <fig>"),
            "{md}"
        );
        let md = typst_a_md("#figure(image(\"a.png\", width: 50%), caption: [Larga])");
        assert!(md.contains("```{=typst}"), "{md}");
        // didascalia con math: non esprimibile come testo alternativo
        let md = typst_a_md("#figure(image(\"a.png\"), caption: [Vale $x$])");
        assert!(md.contains("```{=typst}"), "{md}");
    }

    #[test]
    fn typst_note_a_piede_di_pagina() {
        let c = typst_a_md_con_avvisi("Testo#footnote[La nota.] e altro#footnote[Seconda.]");
        assert!(
            c.contenuto.contains("Testo[^1] e altro[^2]"),
            "{}",
            c.contenuto
        );
        assert!(c.contenuto.contains("[^1]: La nota."));
        assert!(c.contenuto.contains("[^2]: Seconda."));
        assert!(c.avvisi.is_empty(), "{:?}", c.avvisi);
    }

    #[test]
    fn typst_quote_block_true_e_attribuzione_non_sparisce_in_silenzio() {
        let c = typst_a_md_con_avvisi(
            "#quote(block: true, attribution: [Dante])[Nel mezzo del cammin.]",
        );
        assert!(
            c.contenuto.contains("> Nel mezzo del cammin."),
            "{}",
            c.contenuto
        );
        assert!(c.contenuto.contains("> — Dante"), "{}", c.contenuto);
        assert!(
            c.avvisi.iter().any(|a| a.contains("attribuzione")),
            "{:?}",
            c.avvisi
        );
        // blocco multi-paragrafo: ogni riga è prefissata
        let md = typst_a_md("#quote(block: true)[Uno.\n\nDue.]");
        assert!(md.contains("> Uno.\n>\n> Due."), "{md}");
    }

    #[test]
    fn typst_quote_in_linea_nel_testo_resta_grezza() {
        let md = typst_a_md("Disse #quote[ciao] a tutti.");
        assert!(md.contains("`#quote[ciao]`{=typst}"), "{md}");
        // anche con `block: true` ma in mezzo al testo resta una citazione a blocco
        let md = typst_a_md("#quote(block: true)[a]");
        assert!(md.contains("> a"));
    }

    #[test]
    fn typst_commenti_diventano_commenti_html() {
        let md = typst_a_md("Testo // nota privata\n\n/* blocco */\n\nFine");
        assert!(md.contains("<!-- nota privata -->"), "{md}");
        assert!(md.contains("<!-- blocco -->"), "{md}");
    }

    #[test]
    fn typst_liste_annidate_mantengono_il_rientro() {
        let md = typst_a_md("- uno\n  - interno\n  - altro\n- due\n");
        assert!(md.contains("- uno\n  - interno\n  - altro\n- due"), "{md}");
        let md = typst_a_md("+ a\n  + b\n+ c\n");
        assert!(md.contains("1. a\n   1. b\n1. c"), "{md}");
    }

    #[test]
    fn typst_voce_di_lista_con_codice_grezzo_resta_nella_voce() {
        let md = typst_a_md("- vedi #h(1fr) qui\n");
        assert!(md.contains("- vedi `#h(1fr)`{=typst} qui"), "{md}");
    }

    #[test]
    fn typst_testo_con_marcatore_di_lista_letterale_e_protetto() {
        let md = typst_a_md("\\- non una voce\n\n1\\. nemmeno");
        assert!(md.contains("\\- non una voce"), "{md}");
        assert!(md.contains("1\\. nemmeno"), "{md}");
    }

    #[test]
    fn typst_costrutto_grezzo_con_backtick_usa_fence_piu_lungo() {
        let md = typst_a_md("#let x = ```a```\n");
        assert!(md.contains("````{=typst}"), "{md}");
    }

    #[test]
    fn typst_con_errori_di_sintassi_non_va_in_panico() {
        let c = typst_a_md_con_avvisi("#let = = (\n= Titolo\n*aperto\n#foo(");
        assert!(!c.contenuto.is_empty());
        assert!(c.avvisi.iter().any(|a| a.contains("errori di sintassi")));
    }

    // ---- md → typ: Typst grezzo, note, commenti ----

    #[test]
    fn md_blocco_typst_grezzo_e_riemesso_invariato() {
        let md = "```{=typst}\n#set text(lang: \"it\")\n#show heading: it => it\n```\n\nCiao\n";
        let t = md_a_typst(md);
        assert!(
            t.starts_with("#set text(lang: \"it\")\n#show heading: it => it\n\n"),
            "{t:?}"
        );
        assert!(t.contains("Ciao"));
        assert!(!t.contains("{=typst}"));
    }

    #[test]
    fn md_span_typst_grezzo_e_riemesso_invariato() {
        let t = md_a_typst("Vale `$x^2$`{=typst} e `@eq`{=typst}, poi `#h(1fr)`{=typst}.");
        assert!(t.contains("Vale $x^2$ e @eq, poi #h(1fr)."), "{t:?}");
        // un code span normale resta codice
        let t = md_a_typst("`x`{=html}");
        assert!(t.contains("`x`"), "{t:?}");
    }

    #[test]
    fn md_note_a_piede_di_pagina() {
        let t = md_a_typst("Testo[^a] e fine.\n\n[^a]: Corpo della *nota*.\n");
        assert!(
            t.contains("Testo#footnote[Corpo della _nota_.] e fine."),
            "{t:?}"
        );
        assert!(!t.contains("[^a]"), "{t:?}");
    }

    #[test]
    fn md_commenti_html_diventano_commenti_typst() {
        let t = md_a_typst("<!-- privato -->\n\nTesto <!-- in linea --> fine");
        assert!(t.contains("/* privato */"), "{t:?}");
        assert!(t.contains("/* in linea */"), "{t:?}");
    }

    // ---- round-trip typ → md → typ ----

    #[test]
    fn round_trip_typ_md_typ_preserva_i_costrutti() {
        let originale = "\
#set text(lang: \"it\")\n\
#set heading(numbering: \"1.\")\n\
\n\
= Titolo <t>\n\
\n\
Testo con *forte*, _corsivo_ e math $x^2$ vedi @t.#footnote[Una nota.]\n\
\n\
#figure(image(\"a.png\"), caption: [Didascalia])\n\
\n\
#quote(block: true)[Citazione.]\n\
\n\
$ a = b $ <eq>\n\
\n\
- uno\n  - due\n";
        let md = typst_a_md(originale);
        let torna = md_a_typst(&md);
        for atteso in [
            "#set text(lang: \"it\")",
            "#set heading(numbering: \"1.\")",
            "= Titolo <t>",
            "*forte*",
            "_corsivo_",
            "$x^2$",
            "@t",
            "#footnote[Una nota.]",
            "#figure(image(\"a.png\"), caption: [Didascalia])",
            "#quote(block: true)[",
            "$ a = b $ <eq>",
            "- uno",
            "  - due",
        ] {
            assert!(
                torna.contains(atteso),
                "manca {atteso:?} in:\n{torna}\n--- md ---\n{md}"
            );
        }
    }

    #[test]
    fn typst_a_md_senza_avvisi_equivale_a_typst_a_md_con_avvisi() {
        let src = "= A\n\n#set text(lang: \"it\")\n\ntesto";
        assert_eq!(typst_a_md(src), typst_a_md_con_avvisi(src).contenuto);
    }
}

/// Test di regressione per le correzioni del 2026-10-08. Dove possibile la
/// verifica **non** si limita a confrontare la stringa attesa: rilegge
/// l'output con un parser indipendente (`typst_syntax` per il Typst,
/// `pulldown_cmark` per il Markdown) e controlla la struttura ottenuta.
#[cfg(test)]
mod test_correzioni {
    use super::*;
    use std::time::{Duration, Instant};

    // ---- ausili di verifica ----

    fn albero(typ: &str) -> SyntaxNode {
        typst_syntax::parse(typ)
    }

    fn ha_nodo(n: &SyntaxNode, k: SyntaxKind) -> bool {
        n.kind() == k || n.children().any(|c| ha_nodo(c, k))
    }

    fn conta_nodi(n: &SyntaxNode, k: SyntaxKind) -> usize {
        usize::from(n.kind() == k) + n.children().map(|c| conta_nodi(c, k)).sum::<usize>()
    }

    fn primo_nodo(n: &SyntaxNode, k: SyntaxKind) -> Option<&SyntaxNode> {
        if n.kind() == k {
            return Some(n);
        }
        n.children().find_map(|c| primo_nodo(c, k))
    }

    /// Prima `Raw` del sorgente Typst: (lingua, testo con righe unite da `\n`).
    fn primo_raw(typ: &str) -> Option<(Option<String>, String)> {
        let radice = albero(typ);
        let n = primo_nodo(&radice, SyntaxKind::Raw)?;
        let r = n.cast::<ast::Raw>()?;
        let testo = r
            .lines()
            .map(|l| l.get().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        Some((r.lang().map(|l| l.get().to_string()), testo))
    }

    fn senza_errori(typ: &str) -> bool {
        !albero(typ).diagnosis().errors
    }

    /// Testo letterale (concatenazione dei `Text`/`Code`) letto da pulldown-cmark.
    fn testo_pulldown(md: &str) -> String {
        Parser::new_ext(md, Options::all())
            .filter_map(|e| match e {
                Event::Text(t) | Event::Code(t) => Some(t.to_string()),
                _ => None,
            })
            .collect()
    }

    // ---- (3) backtick: lingua involontaria ----

    #[test]
    fn md_codice_inline_con_backtick_e_lettera_iniziale_non_diventa_lingua() {
        for contenuto in ["a`b", "_x`y", "é`z", "ab``c", "Rust`"] {
            let mut md = String::new();
            racchiudi_in_backtick_md(contenuto, &mut md);
            let typ = md_a_typst(&md);
            let (lingua, testo) =
                primo_raw(&typ).unwrap_or_else(|| panic!("nessun raw in {typ:?} da {md:?}"));
            assert_eq!(lingua, None, "{contenuto:?} -> {typ:?}");
            assert_eq!(testo, contenuto, "{contenuto:?} -> {typ:?}");
        }
    }

    #[test]
    fn md_codice_inline_semplice_e_con_cifra_iniziale_restano_invariati() {
        assert_eq!(md_a_typst("`x`").trim(), "`x`");
        // una cifra non può iniziare un tag di lingua: nessun padding necessario
        let typ = md_a_typst("`` 1`2 ``");
        let (lingua, testo) = primo_raw(&typ).unwrap();
        assert_eq!((lingua, testo.as_str()), (None, "1`2"), "{typ:?}");
    }

    // ---- (4) Typst grezzo dentro un contesto inline ----

    #[test]
    fn typ_a_md_grezzo_dentro_inline_resta_uno_span() {
        for (src, md_atteso) in [
            (
                "#link(\"https://x\")[#sym.arrow.r]",
                "[`#sym.arrow.r`{=typst}](https://x)",
            ),
            ("*#h(1fr)*", "**`#h(1fr)`{=typst}**"),
            ("_#h(1fr)_", "*`#h(1fr)`{=typst}*"),
            ("= #sym.arrow", "# `#sym.arrow`{=typst}"),
            ("#strike[#h(1fr)]", "~~`#h(1fr)`{=typst}~~"),
        ] {
            let md = typst_a_md(src);
            assert!(!md.contains("```"), "fence dentro un inline: {md:?}");
            assert_eq!(md.trim(), md_atteso, "{src}");
            // ritorno: Typst valido e con lo stesso costrutto
            let t = md_a_typst(&md);
            assert!(senza_errori(&t), "{src} -> {t:?}");
            assert!(t.contains(src), "{src} -> {t:?}");
        }
    }

    #[test]
    fn typ_a_md_grezzo_dentro_inline_e_riletto_da_pulldown_come_codice() {
        let md = typst_a_md("#link(\"https://x\")[#sym.arrow.r]");
        let eventi: Vec<Event> = Parser::new_ext(&md, Options::all()).collect();
        assert!(
            eventi
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::Link { .. }))),
            "{eventi:?}"
        );
        assert!(
            eventi
                .iter()
                .any(|e| matches!(e, Event::Code(c) if c.as_ref() == "#sym.arrow.r")),
            "{eventi:?}"
        );
        assert!(
            !eventi
                .iter()
                .any(|e| matches!(e, Event::Start(Tag::CodeBlock(_)))),
            "{eventi:?}"
        );
    }

    #[test]
    fn typ_a_md_grezzo_su_riga_propria_resta_un_blocco() {
        // il comportamento a blocco non cambia fuori dai contesti inline
        let md = typst_a_md("#set text(lang: \"it\")\n\nCiao");
        assert!(md.contains("```{=typst}\n#set text"), "{md}");
    }

    // ---- (5) marcatori di lista letterali ----

    #[test]
    fn md_testo_con_marcatore_di_lista_letterale_non_diventa_lista() {
        for md in [
            "\\- x",
            "\\+ x",
            "1\\. x",
            "12\\. x",
            "- \\- x",
            "> \\- x",
            "[\\- x](https://u)",
            "| a |\n|---|\n| \\+ x |",
            "![\\- didascalia](i.png)",
            // cifre dopo l'ultimo `[` aperto: link, enfasi come funzione, celle
            "[1\\. x](https://u)",
            "a**1\\. x**b",
            "a*1\\. x*b",
            "a~~1\\. x~~b",
            "[**1\\. x**](https://u)",
            "| a |\n|---|\n| 1\\. x |",
            "| a |\n|---|\n| \\- x |",
            "> [1\\. x](https://u)",
        ] {
            let t = md_a_typst(md);
            let radice = albero(&t);
            assert!(senza_errori(&t), "{md:?} -> {t:?}");
            let attese = usize::from(md.starts_with("- "));
            assert_eq!(
                conta_nodi(&radice, SyntaxKind::ListItem)
                    + conta_nodi(&radice, SyntaxKind::EnumItem),
                attese,
                "{md:?} -> {t:?}"
            );
        }
    }

    #[test]
    fn md_testo_con_marcatore_letterale_round_trip_typ_md_typ() {
        for src in ["\\- uno", "\\+ due", "1\\. tre"] {
            let md = typst_a_md(src);
            let t = md_a_typst(&md);
            assert!(
                !ha_nodo(&albero(&t), SyntaxKind::ListItem),
                "{src}: {md:?} -> {t:?}"
            );
            assert!(
                !ha_nodo(&albero(&t), SyntaxKind::EnumItem),
                "{src}: {md:?} -> {t:?}"
            );
        }
    }

    // ---- (6) enfasi a metà parola ----

    #[test]
    fn md_enfasi_a_meta_parola_sopravvive_al_ritorno() {
        for md in [
            "**a**b",
            "x**a**",
            "x**a**y",
            "*a*b",
            "c*a*",
            "co*stru*ire",
            "co**stru**ire",
        ] {
            let t = md_a_typst(md);
            let torna = typst_a_md(&t);
            assert_eq!(torna.trim(), md, "{md:?} -> {t:?}");
        }
        // fuori dalla parola resta la sintassi compatta
        assert_eq!(md_a_typst("a **b** c").trim(), "a *b* c");
        assert_eq!(md_a_typst("(**b**)").trim(), "(*b*)");
        assert_eq!(md_a_typst("a *b* c").trim(), "a _b_ c");
    }

    // ---- (13) punto e virgola dopo #espressione ----

    #[test]
    fn typ_a_md_conserva_il_punto_e_virgola() {
        let md = typst_a_md("#x;testo");
        let t = md_a_typst(&md);
        assert!(t.contains("#x;testo"), "{md:?} -> {t:?}");
        assert!(ha_nodo(&albero(&t), SyntaxKind::Semicolon), "{t:?}");
        // blocco su riga propria
        let md = typst_a_md("#x;\n\nCiao");
        assert!(md.contains("#x;"), "{md}");
    }

    // ---- (14) niente esplosione esponenziale ----

    #[test]
    fn typ_a_md_annidamento_non_esponenziale() {
        fn src(d: usize) -> String {
            if d == 0 {
                "x".to_string()
            } else {
                format!(
                    "#quote(block: true)[\n#set text(lang: \"it\")\n\n{}\n]",
                    src(d - 1)
                )
            }
        }
        let profondita = 20;
        let sorgente = src(profondita);
        let t0 = Instant::now();
        let md = typst_a_md(&sorgente);
        let dt = t0.elapsed();
        assert!(
            dt < Duration::from_secs(2),
            "conversione troppo lenta ({dt:?}): costo esponenziale?"
        );
        // verifica del contenuto: un blocco grezzo e una citazione per livello
        assert_eq!(md.matches("{=typst}").count(), profondita, "{md}");
        // l'ultima riga ha un prefisso `> ` per ciascun livello di citazione
        assert!(
            md.contains(&format!("{}x", "> ".repeat(profondita))),
            "{md}"
        );
    }

    // ---- (15) espressioni multiriga in mezzo al testo ----

    #[test]
    fn typ_a_md_non_comprime_stringhe_multiriga() {
        let src = "prima #f(\"riga1\n    riga2\") dopo";
        let md = typst_a_md(src);
        let t = md_a_typst(&md);
        assert!(
            t.contains("\"riga1\n    riga2\""),
            "stringa multiriga alterata: {md:?} -> {t:?}"
        );
        assert!(senza_errori(&t), "{t:?}");
    }

    #[test]
    fn typ_a_md_non_comprime_raw_multiriga_con_backtick() {
        let src = "prima #f(```\n  a\n  b\n```) dopo";
        let md = typst_a_md(src);
        let t = md_a_typst(&md);
        assert!(t.contains("```\n  a\n  b\n```"), "{md:?} -> {t:?}");
    }

    #[test]
    fn typ_a_md_compatta_ancora_il_codice_semplice_multiriga() {
        let src = "prima #f(\n  1,\n  2\n) dopo";
        let md = typst_a_md(src);
        assert!(!md.contains("```"), "{md}");
        let t = md_a_typst(&md);
        assert!(senza_errori(&t), "{t:?}");
        assert!(t.contains("#f(") && t.contains("2"), "{t:?}");
    }

    // ---- (16) & letterale ----

    #[test]
    fn typ_a_md_scappa_la_e_commerciale_che_formerebbe_un_entita() {
        let src = "AT&amp;T, &lt;x&gt; e R&D, 5 & 6";
        let md = typst_a_md(src);
        assert_eq!(testo_pulldown(&md), src, "{md:?}");
        // le `&` isolate restano leggibili
        assert!(md.contains("5 & 6"), "{md}");
    }

    // ---- (19) spazi ai bordi di enfasi ----

    #[test]
    fn typ_a_md_sposta_gli_spazi_fuori_dai_delimitatori() {
        for (src, atteso) in [
            ("x * a * y", "x **a** y"),
            ("x _ a _ y", "x *a* y"),
            ("x#strong[ a ]y", "x **a** y"),
            ("x#emph[ a ]y", "x *a* y"),
        ] {
            let md = typst_a_md(src);
            assert_eq!(md.trim(), atteso, "{src}");
            // lettura indipendente: pulldown vede davvero una enfasi su «a»
            let eventi: Vec<Event> = Parser::new_ext(&md, Options::all()).collect();
            assert!(
                eventi
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::Strong | Tag::Emphasis))),
                "{src}: {md:?} -> {eventi:?}"
            );
            assert!(!testo_pulldown(&md).contains('*'), "{src}: {md:?}");
        }
        // enfasi vuota: nessun delimitatore orfano
        assert!(!typst_a_md("x * * y").contains('*'));
    }

    // ---- (20) numero iniziale delle liste ordinate ----

    #[test]
    fn md_lista_ordinata_conserva_il_numero_iniziale() {
        let t = md_a_typst("3. a\n4. b\n");
        let radice = albero(&t);
        let primo = primo_nodo(&radice, SyntaxKind::EnumItem).expect("enum");
        assert_eq!(
            primo.cast::<ast::EnumItem>().unwrap().number(),
            Some(3),
            "{t:?}"
        );
        assert_eq!(conta_nodi(&radice, SyntaxKind::EnumItem), 2, "{t:?}");
        // e ritorna a Markdown con lo stesso inizio
        assert!(typst_a_md(&t).contains("3. a"), "{t:?}");
        // da 1 in poi resta la forma compatta
        assert!(md_a_typst("1. a\n2. b\n").contains("+ a"));
        // zero
        let t = md_a_typst("0. z\n");
        assert!(t.contains("0. z"), "{t:?}");
        // sotto-lista che parte da un numero diverso da 1 (la riga vuota è
        // richiesta da CommonMark: senza, «5. x» sarebbe testo di «a»)
        let t = md_a_typst("- a\n\n  5. x\n  6. y\n");
        assert!(t.contains("5. x") && t.contains("+ y"), "{t:?}");
    }

    // ---- (21) sicurezza di {=typst} ----

    #[test]
    fn md_typst_grezzo_disattivabile_per_md_non_fidati() {
        let md = "```{=typst}\n#read(\"/etc/passwd\")\n```\n\ne `#include \"x.typ\"`{=typst}\n";
        // default: consentito (round-trip typ → md → typ)
        let t = md_a_typst(md);
        assert!(ha_nodo(&albero(&t), SyntaxKind::Hash), "{t:?}");
        // opzione di sicurezza: nessun codice Typst eseguibile nell'output
        let sicuro = md_a_typst_con_opzioni(
            md,
            &OpzioniMd {
                consenti_typst_grezzo: false,
            },
        );
        let radice = albero(&sicuro);
        assert!(!ha_nodo(&radice, SyntaxKind::Hash), "{sicuro:?}");
        assert!(!ha_nodo(&radice, SyntaxKind::FuncCall), "{sicuro:?}");
        assert!(sicuro.contains("read"), "il testo non va perso: {sicuro:?}");
        assert!(senza_errori(&sicuro), "{sicuro:?}");
    }

    // ---- ultimo giro: blocchi di codice `{}` e commenti nello span compattato ----

    #[test]
    fn typ_a_md_blocco_codice_multiriga_in_mezzo_al_testo_torna_valido() {
        for src in [
            "a #{\n  let x = 1\n  let y = 2\n} b",
            // dentro un inline il blocco è impossibile: serve lo span
            "*a #{\n  let x = 1\n  let y = 2\n} b*",
            "#link(\"https://u\")[a #{\n  let x = 1\n  let y = 2\n} b]",
        ] {
            assert!(senza_errori(src), "precondizione: {src:?}");
            let md = typst_a_md(src);
            let t = md_a_typst(&md);
            assert!(
                senza_errori(&t),
                "{src:?} -> md {md:?} -> {t:?}: {:?}",
                albero(&t).diagnosis()
            );
            assert_eq!(
                conta_nodi(&albero(&t), SyntaxKind::LetBinding),
                2,
                "{src:?} -> {t:?}"
            );
        }
    }

    #[test]
    fn typ_a_md_commento_di_riga_nello_span_inline_non_inghiotte_il_resto() {
        for src in [
            "*a #f(\n  1, // nota\n  2\n) b*",
            "*a #f(\n  1, // fine */ commento\n  2\n) b*",
        ] {
            assert!(senza_errori(src), "precondizione: {src:?}");
            let c = typst_a_md_con_avvisi(src);
            let t = md_a_typst(&c.contenuto);
            let radice = albero(&t);
            assert!(senza_errori(&t), "{src:?} -> {:?} -> {t:?}", c.contenuto);
            // il «2» dopo il commento è ancora un argomento, non commento
            assert_eq!(conta_nodi(&radice, SyntaxKind::Int), 2, "{src:?} -> {t:?}");
            assert!(
                !c.avvisi.is_empty(),
                "la perdita di formattazione va avvisata"
            );
        }
    }

    #[test]
    fn compatta_commento_su_riga_propria_fra_due_istruzioni_non_raddoppia_il_separatore() {
        let src = "#{\n  let x = 1\n  // c\n  let y = 2\n}";
        assert!(senza_errori(src), "precondizione: {src:?}");
        let (compatto, commenti, valido) = compatta_in_una_riga(src);
        assert!(commenti);
        // un solo separatore, prima del commento
        assert!(compatto.contains("1; /* c*/ let y"), "{compatto:?}");
        assert_eq!(compatto.matches(';').count(), 1, "{compatto:?}");
        assert!(valido, "{compatto:?}");
        assert!(senza_errori(&compatto), "{compatto:?}");
        assert_eq!(
            conta_nodi(&albero(&compatto), SyntaxKind::LetBinding),
            2,
            "{compatto:?}"
        );
        // lo stesso, dentro un'enfasi, fino al ritorno in Typst
        let src = "*a #{\n  let x = 1\n  // c\n  let y = 2\n} b*";
        assert!(senza_errori(src), "precondizione: {src:?}");
        let md = typst_a_md(src);
        let t = md_a_typst(&md);
        assert!(senza_errori(&t), "{src:?} -> {md:?} -> {t:?}");
        assert_eq!(conta_nodi(&albero(&t), SyntaxKind::LetBinding), 2, "{t:?}");
    }

    #[test]
    fn compatta_stringhe_con_barre_non_sono_commenti() {
        for src in [
            "*a #f(\n  \"http://x\"\n) b*",
            "*a #f(\n  \"/* non è un commento */\",\n  2\n) b*",
            "*a #f(\n  \"http://x\", // vero commento\n  2\n) b*",
        ] {
            assert!(senza_errori(src), "precondizione: {src:?}");
            let c = typst_a_md_con_avvisi(src);
            let t = md_a_typst(&c.contenuto);
            assert!(senza_errori(&t), "{src:?} -> {:?} -> {t:?}", c.contenuto);
            // la stringa è intatta e il resto della chiamata non è inghiottito
            assert!(
                t.contains("\"http://x\"") || t.contains("\"/* non è un commento */\""),
                "{src:?} -> {t:?}"
            );
            assert!(
                conta_nodi(&albero(&t), SyntaxKind::Str) >= 1,
                "{src:?} -> {t:?}"
            );
        }
        // direttamente sul compattatore: nessun commento convertito per le sole stringhe
        let (compatto, commenti, valido) = compatta_in_una_riga("#f(\n  \"http://x\"\n)");
        assert!(!commenti && valido, "{compatto:?}");
        assert_eq!(compatto, "#f( \"http://x\" )");
        let (compatto, commenti, valido) = compatta_in_una_riga("#f(\n  \"a\", // c\n  \"b/*\"\n)");
        assert!(commenti && valido, "{compatto:?}");
        assert!(compatto.contains("\"b/*\""), "{compatto:?}");
        assert!(senza_errori(&compatto), "{compatto:?}");
    }

    #[test]
    fn md_typst_grezzo_vietato_consuma_anche_l_attributo() {
        let sicuro = md_a_typst_con_opzioni(
            "x `#f(1)`{=typst} dopo",
            &OpzioniMd {
                consenti_typst_grezzo: false,
            },
        );
        assert!(!sicuro.contains("typst}"), "attributo visibile: {sicuro:?}");
        assert!(sicuro.contains("dopo"), "{sicuro:?}");
        assert!(
            !ha_nodo(&albero(&sicuro), SyntaxKind::FuncCall),
            "{sicuro:?}"
        );
    }

    // ---- (17) profondità ----

    #[test]
    fn typ_a_md_oltre_il_limite_di_profondita_conserva_il_sorgente() {
        let n = 400;
        let src = format!("{}x{}", "#strong[".repeat(n), "]".repeat(n));
        let c = typst_a_md_con_avvisi(&src);
        assert!(
            c.avvisi.iter().any(|a| a.contains("profondità")),
            "{:?}",
            c.avvisi
        );
        // niente perdita di contenuto: la «x» è ancora nell'output
        assert!(c.contenuto.contains('x'));
    }
}
