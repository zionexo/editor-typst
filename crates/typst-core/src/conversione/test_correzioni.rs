//! Test di regressione per le correzioni del 2026-10-08 sulle esportazioni
//! (LaTeX non compilabile, id HTML dei titoli annidati, profondità, avvisi).
//!
//! Dove possibile la verifica è **indipendente** dal generatore: si
//! simulano i contatori di LaTeX, si ricontrolla la struttura dell'HTML
//! con una scansione propria, si confrontano fra loro i backend.

use super::*;
use std::collections::HashSet;
use std::time::{Duration, Instant};

fn txt(s: &str) -> Conversione {
    typst_a_testo(s, &OpzioniTesto::default())
}
fn html(s: &str) -> Conversione {
    typst_a_html(s, &OpzioniHtml::default())
}
fn tex(s: &str) -> Conversione {
    typst_a_latex(s, &OpzioniLatex::default())
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

/// Estrae il contenuto di tutte le formule `$…$` in linea di una riga LaTeX.
fn formule_in_linea(t: &str) -> Vec<String> {
    let mut v = Vec::new();
    for riga in t.lines() {
        // le formule delle sole righe di testo (non ambienti di display)
        if riga.starts_with('\\') && !riga.contains('$') {
            continue;
        }
        let pezzi: Vec<&str> = riga.split('$').collect();
        for (k, p) in pezzi.iter().enumerate() {
            if k % 2 == 1 {
                v.push((*p).to_string());
            }
        }
    }
    v
}

#[test]
fn latex_math_in_linea_senza_allineamenti() {
    let t = tex("Vale $a &= b \\ c$ qui, e $x & y$.").contenuto;
    let formule = formule_in_linea(&t);
    assert_eq!(formule.len(), 2, "{t}");
    for f in &formule {
        // `&` e `\\` sono ammessi solo dentro un ambiente di allineamento
        assert!(!f.contains('&'), "& in math in linea: {f:?}\n{t}");
        assert!(!f.contains("\\\\"), "\\\\ in math in linea: {f:?}\n{t}");
    }
    // il contenuto resta: a, b, c compaiono ancora
    assert!(formule[0].contains('a') && formule[0].contains('b') && formule[0].contains('c'));
    // la math a blocco continua a usare l'ambiente di allineamento
    let t = tex("$ a &= b \\ c &= d $").contenuto;
    assert!(t.contains("\\begin{aligned}") && t.contains('&'), "{t}");
}

#[test]
fn latex_bibliografia_usa_il_comando_della_classe() {
    let src = "#bibliography(\"r.bib\")";
    let article = tex(src).contenuto;
    assert!(article.contains("\\renewcommand{\\refname}"), "{article}");
    assert!(!article.contains("\\bibname"), "{article}");
    for classe in ["report", "book"] {
        let t = typst_a_latex(
            src,
            &OpzioniLatex {
                classe: classe.into(),
                ..Default::default()
            },
        )
        .contenuto;
        assert!(t.contains("\\renewcommand{\\bibname}"), "{classe}: {t}");
        assert!(!t.contains("\\refname"), "{classe}: {t}");
    }
    // anche con titolo esplicito
    let t = tex("#bibliography(\"r.bib\", title: [Fonti])").contenuto;
    assert!(t.contains("\\renewcommand{\\refname}{Fonti}"), "{t}");
    assert!(!t.contains("\\bibname"), "{t}");
}

#[test]
fn latex_figura_con_etichetta_senza_didascalia_avanza_il_contatore() {
    let t = tex("#figure(image(\"a.png\")) <f>\n\nVedi @f.").contenuto;
    // l'etichetta deve seguire un \caption o un \refstepcounter nello stesso ambiente
    let pos_label = t.find("\\label{et:f}").expect("label");
    let prima = &t[..pos_label];
    let inizio_amb = prima.rfind("\\begin{figure}").expect("begin figure");
    let tra = &prima[inizio_amb..];
    assert!(
        tra.contains("\\caption{") || tra.contains("\\refstepcounter{figure}"),
        "\\label senza \\caption né \\refstepcounter:\n{t}"
    );
    // senza etichetta il contatore deve comunque avanzare (Typst numera lo stesso)
    let t = tex("#figure(image(\"a.png\"))\n\n#figure(image(\"b.png\"), caption: [B])").contenuto;
    assert_eq!(t.matches("\\refstepcounter{figure}").count(), 1, "{t}");
}

/// Simula i contatori LaTeX: per ogni `\caption` ritorna (ambiente, numero, testo).
fn didascalie_latex(t: &str) -> Vec<(String, usize, String)> {
    let mut contatori: std::collections::HashMap<String, usize> = Default::default();
    let mut pila: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for riga in t.lines() {
        if let Some(r) = riga.strip_prefix("\\begin{") {
            pila.push(r.split('}').next().unwrap().to_string());
        } else if riga.starts_with("\\end{") {
            pila.pop();
        } else if let Some(r) = riga.strip_prefix("\\refstepcounter{") {
            let nome = r.split('}').next().unwrap().to_string();
            *contatori.entry(nome).or_default() += 1;
        } else if let Some(r) = riga.strip_prefix("\\caption{") {
            let env = pila.last().cloned().unwrap_or_default();
            let n = contatori.entry(env.clone()).or_default();
            *n += 1;
            out.push((env, *n, r.trim_end_matches('}').to_string()));
        }
    }
    out
}

#[test]
fn latex_listati_non_condividono_il_contatore_delle_figure() {
    let src = "#figure(image(\"a.png\"), caption: [Alfa])\n\n\
               #figure(```rust\nfn f() {}\n```, caption: [Beta])\n\n\
               #figure(image(\"b.png\"), caption: [Gamma])\n\n\
               #figure(table(columns: 1, [x]), caption: [Delta])\n";
    let c = tex(src);
    let t = &c.contenuto;
    assert!(t.contains("\\begin{listato}"), "{t}");
    assert!(t.contains("\\usepackage{float}"), "{t}");
    assert!(t.contains("\\newfloat{listato}"), "{t}");
    // verifica indipendente: numeri LaTeX simulati == numeri del testo semplice
    let caps = didascalie_latex(t);
    assert_eq!(caps.len(), 4, "{caps:?}\n{t}");
    let piano = txt(src).contenuto;
    for (env, n, testo) in &caps {
        let parola = match env.as_str() {
            "figure" => "Figure",
            "listato" => "Listing",
            "table" => "Table",
            altro => panic!("ambiente inatteso {altro}"),
        };
        assert!(
            piano.contains(&format!("{parola} {n}: {testo}")),
            "numerazione LaTeX ({env} {n} {testo}) diversa da Typst:\n{piano}"
        );
    }
}

#[test]
fn latex_item_non_legge_le_quadre_come_argomento_opzionale() {
    let t = tex("- [x] fatto\n- altro\n\n+ [y] primo\n+ secondo\n").contenuto;
    // dopo ogni \item (di itemize/enumerate) il primo carattere non spazio non è '['
    let mut resto = t.as_str();
    let mut n = 0;
    while let Some(p) = resto.find("\\item") {
        let dopo = resto[p + 5..].trim_start();
        assert!(!dopo.starts_with('['), "\\item seguito da '[':\n{t}");
        resto = &resto[p + 5..];
        n += 1;
    }
    assert_eq!(n, 4);
    assert!(t.contains("[x] fatto") && t.contains("[y] primo"), "{t}");
}

/// Argomento opzionale di `\item[...]` letto come farebbe LaTeX: fino alla
/// prima `]` a profondità di graffe zero.
fn argomento_opzionale(t: &str) -> Option<String> {
    let p = t.find("\\item[")? + "\\item[".len();
    let mut profondita = 0i32;
    let mut s = String::new();
    for c in t[p..].chars() {
        match c {
            '{' => profondita += 1,
            '}' => profondita -= 1,
            ']' if profondita == 0 => return Some(s),
            _ => {}
        }
        s.push(c);
    }
    None
}

#[test]
fn latex_termine_con_quadra_chiusa_non_rompe_item() {
    // `\]` è l'escape Typst di una `]` letterale nel termine
    let t = tex("/ a\\]b: descrizione\n").contenuto;
    assert_eq!(
        argomento_opzionale(&t).as_deref(),
        Some("{a]b}"),
        "il termine deve stare tutto nell'argomento opzionale:\n{t}"
    );
    assert!(t.contains("] descrizione"), "{t}");
}

#[test]
fn latex_a_capo_in_cella_non_termina_la_riga() {
    let t = tex("#table(columns: 2, [a \\ b], [c])").contenuto;
    let righe: Vec<&str> = t
        .lines()
        .filter(|r| r.contains("\\hline") && !r.starts_with("\\hline"))
        .collect();
    assert_eq!(righe.len(), 1, "{t}");
    // una sola terminazione di riga, con due celle
    assert_eq!(righe[0].matches("\\\\").count(), 1, "{}", righe[0]);
    assert_eq!(righe[0].matches(" & ").count(), 1, "{}", righe[0]);
    assert!(righe[0].starts_with("a b & c"), "{}", righe[0]);
    // fuori dalle celle l'a capo resta
    let t = tex("uno \\ due").contenuto;
    assert!(t.contains("uno\\\\ due"), "{t}");
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

/// Tutti i valori di un attributo (`id="…"`, `href="#…"`) in ordine.
fn valori(h: &str, prefisso: &str) -> Vec<String> {
    h.split(prefisso)
        .skip(1)
        .map(|p| p.split('"').next().unwrap_or("").to_string())
        .collect()
}

/// `id` del titolo il cui testo è `testo`: cerca `<hN id="…">…testo</hN>`.
fn id_del_titolo(h: &str, testo: &str) -> String {
    let fine = h
        .find(&format!("{testo}</h"))
        .unwrap_or_else(|| panic!("titolo {testo:?} assente:\n{h}"));
    let inizio_tag = h[..fine].rfind("<h").unwrap();
    let tag = &h[inizio_tag..fine];
    valori(tag, " id=\"").into_iter().next().unwrap()
}

#[test]
fn html_titoli_annidati_hanno_id_corretto_e_indice_coerente() {
    let h = html(
        "#quote(block: true)[\n= Interno\n]\n\n= Esterno <x>\n\n== Figlio\n\n- voce\n\n#outline()\n",
    )
    .contenuto;
    let id_int = id_del_titolo(&h, "Interno");
    let id_est = id_del_titolo(&h, "Esterno");
    let id_fig = id_del_titolo(&h, "Figlio");
    // l'etichetta <x> appartiene al titolo "Esterno", non a quello annidato
    assert_eq!(id_est, "et-x", "{h}");
    assert_ne!(id_int, "et-x");
    // id unici
    let ids = valori(&h, " id=\"");
    let unici: HashSet<&String> = ids.iter().collect();
    assert_eq!(unici.len(), ids.len(), "id duplicati: {ids:?}");
    // l'indice contiene tutti i titoli e ogni href punta a un id esistente
    let nav = &h[h.find("<nav").unwrap()..h.find("</nav>").unwrap()];
    for (testo, id) in [
        ("Interno", &id_int),
        ("Esterno", &id_est),
        ("Figlio", &id_fig),
    ] {
        assert!(
            nav.contains(&format!("<a href=\"#{id}\">{testo}</a>")),
            "voce d'indice {testo} -> #{id} assente o errata:\n{nav}"
        );
    }
    for href in valori(nav, "href=\"#") {
        assert!(ids.contains(&href), "href #{href} senza destinazione");
    }
}

/// Coppie (href, testo del link) dell'indice `<nav>`.
fn voci_indice(nav: &str) -> Vec<(String, String)> {
    nav.split("<a href=\"#")
        .skip(1)
        .map(|p| {
            let (href, resto) = p.split_once("\">").unwrap();
            let testo = resto.split("</a>").next().unwrap();
            (href.to_string(), testo.to_string())
        })
        .collect()
}

#[test]
fn html_indice_prima_dei_titoli_punta_ai_titoli_giusti() {
    // `#outline()` PRIMA dei titoli: l'indice non può dedurre gli id dall'ordine
    // di lettura del documento ma dall'identità dei blocchi
    let h = html(
        "#outline()\n\n#quote(block: true)[\n= Interno\n]\n\n= Esterno <x>\n\n== Figlio\n\n\
         #quote(block: true)[\n== Secondo interno\n]\n",
    )
    .contenuto;
    let nav = &h[h.find("<nav").unwrap()..h.find("</nav>").unwrap()];
    let voci = voci_indice(nav);
    assert_eq!(voci.len(), 4, "{nav}");
    let mut visti = HashSet::new();
    for (href, testo) in &voci {
        // ogni href ha il suo `<hN id="href">` e quel titolo ha il testo del link
        let tag = format!(" id=\"{href}\">");
        let pos = h
            .find(&tag)
            .unwrap_or_else(|| panic!("href #{href} senza destinazione:\n{h}"));
        let inizio = h[..pos].rfind('<').unwrap();
        assert!(
            h[inizio..].starts_with("<h")
                && h[inizio + 2..].starts_with(|c: char| c.is_ascii_digit()),
            "#{href} non punta a un titolo: {}",
            &h[inizio..pos + tag.len()]
        );
        let chiusura = h[pos..].find("</h").unwrap();
        assert!(
            h[pos + tag.len()..pos + chiusura].contains(testo.as_str()),
            "#{href}: il titolo non è «{testo}»:\n{h}"
        );
        assert!(visti.insert(href.clone()), "href duplicato #{href}");
        assert_eq!(h.matches(&tag).count(), 1, "id duplicato {href}");
    }
    assert_eq!(id_del_titolo(&h, "Esterno"), "et-x");
}

#[test]
fn html_indice_usa_testo_semplice_senza_note_ne_link_annidati() {
    let h = html(
        "= Titolo con nota#footnote[Testo nota]\n\n= Vai a #link(\"https://a.it\")[sito]\n\n#outline()\n",
    )
    .contenuto;
    // la nota compare una sola volta (nel titolo, non anche nell'indice)
    assert_eq!(h.matches("id=\"rif-1\"").count(), 1, "{h}");
    assert_eq!(h.matches("<li id=\"nota-").count(), 1, "{h}");
    // nessun <a> annidato dentro l'indice
    let nav = &h[h.find("<nav").unwrap()..h.find("</nav>").unwrap()];
    let mut aperti = 0i32;
    for (i, _) in nav.match_indices('<') {
        let r = &nav[i..];
        if r.starts_with("<a ") {
            aperti += 1;
            assert!(aperti <= 1, "<a> annidato nell'indice:\n{nav}");
        } else if r.starts_with("</a>") {
            aperti -= 1;
        }
    }
    assert!(nav.contains(">Vai a sito</a>"), "{nav}");
}

// ---------------------------------------------------------------------------
// Avvisi: dedup lineare
// ---------------------------------------------------------------------------

#[test]
fn avvisi_helper_conserva_ordine_e_scarta_duplicati() {
    let mut a = ir::Avvisi::default();
    assert!(a.aggiungi("uno"));
    assert!(a.aggiungi(String::from("due")));
    assert!(!a.aggiungi("uno"));
    assert!(a.aggiungi("tre"));
    let mut altri = ir::Avvisi::default();
    altri.aggiungi("tre");
    altri.aggiungi("quattro");
    a.estendi(altri);
    assert_eq!(a.into_vec(), ["uno", "due", "tre", "quattro"]);
}

/// Misura a tempo: dipende dal carico della macchina (altri processi, CI
/// condivise), quindi è fragile e non gira di default. Il comportamento
/// verificabile in modo deterministico (nessun duplicato, ordine conservato)
/// è in `avvisi_distinti_sono_tutti_conservati_in_ordine`; questa serve a
/// mano per cogliere una regressione di scala:
/// `cargo test -p typst-core -- --ignored avvisi_distinti_scalano`.
#[test]
#[ignore = "misura a tempo, fragile sotto carico: eseguire a mano con --ignored"]
fn avvisi_distinti_scalano_in_modo_lineare() {
    fn misura(n: usize) -> Duration {
        let src: String = (0..n).map(|i| format!("#fn{i:07}[x]\n")).collect();
        let t0 = Instant::now();
        let c = txt(&src);
        let dt = t0.elapsed();
        assert!(
            c.avvisi.len() >= n,
            "attesi {n} avvisi, trovati {}",
            c.avvisi.len()
        );
        let unici: HashSet<&String> = c.avvisi.iter().collect();
        assert_eq!(unici.len(), c.avvisi.len());
        dt
    }
    let piccolo = misura(8_000).max(Duration::from_millis(5));
    let grande = misura(32_000);
    // lineare: ~4x; quadratico: ~16x. Soglia a metà strada in scala logaritmica.
    assert!(
        grande.as_secs_f64() < piccolo.as_secs_f64() * 9.0 + 0.3,
        "crescita non lineare: {piccolo:?} -> {grande:?}"
    );
}

#[test]
fn avvisi_distinti_sono_tutti_conservati_in_ordine() {
    // versione deterministica (senza orologio) del controllo di scala
    let n = 8_000;
    let src: String = (0..n).map(|i| format!("#fn{i:07}[x]\n")).collect();
    let c = txt(&src);
    let unici: HashSet<&String> = c.avvisi.iter().collect();
    assert_eq!(unici.len(), c.avvisi.len(), "avvisi duplicati");
    assert!(c.avvisi.len() >= n, "attesi {n} avvisi, {}", c.avvisi.len());
    // ordine d'inserimento: fn0000000 prima di fn0000001, …
    let pos = |i: usize| {
        c.avvisi
            .iter()
            .position(|a| a.contains(&format!("fn{i:07}")))
            .unwrap_or_else(|| panic!("avviso per fn{i:07} assente"))
    };
    assert!(pos(0) < pos(1) && pos(1) < pos(n - 1));
}

// ---------------------------------------------------------------------------
// Ultimo giro 2026-10-08
// ---------------------------------------------------------------------------

/// Esegue `f` in un thread con pila ridotta (come nei test di profondità).
fn con_pila_ridotta(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn profondita_un_livello_per_ogni_citazione_annidata() {
    con_pila_ridotta(|| {
        // 24 citazioni annidate: sotto il tetto di 32, nessuna parte omessa
        // (con il doppio conteggio il tetto effettivo era 16)
        let n = 24;
        let src = format!("{}x{}", "#quote(block: true)[".repeat(n), "]".repeat(n));
        let c = txt(&src);
        assert!(c.contenuto.contains('x'), "{}", c.contenuto);
        assert!(
            !c.avvisi.iter().any(|a| a.contains("profondità")),
            "{:?}",
            c.avvisi
        );
        // idem per note e figure annidate
        let src = format!("{}x{}", "#footnote[".repeat(20), "]".repeat(20));
        let c = txt(&src);
        assert!(c.contenuto.contains('x'), "{}", c.contenuto);
        assert!(!c.avvisi.iter().any(|a| a.contains("profondità")));
        // oltre il tetto resta l'avviso (e nessun overflow)
        let n = 200;
        let src = format!("{}x{}", "#quote(block: true)[".repeat(n), "]".repeat(n));
        let c = txt(&src);
        assert!(c.avvisi.iter().any(|a| a.contains("profondità")));
    });
}

#[test]
fn colspan_oltre_le_colonne_e_avvisato() {
    let src = "#table(columns: 2, [a], [b], table.cell(colspan: 5)[c])";
    for (nome, c) in [("txt", txt(src)), ("html", html(src)), ("tex", tex(src))] {
        assert!(
            c.avvisi.iter().any(|a| a.contains("colspan 5")),
            "{nome}: {:?}",
            c.avvisi
        );
    }
    // entro le colonne: nessun avviso di riduzione
    let ok = html("#table(columns: 3, [a], table.cell(colspan: 2)[b])");
    assert!(
        !ok.avvisi.iter().any(|a| a.contains("colspan")),
        "{:?}",
        ok.avvisi
    );
}

// ---- LaTeX: etichette di titoli non numerati, figure dentro le liste ----

/// Chiavi citate da `\ref`, `\eqref`, `\hyperref[…]` senza il `\label` corrispondente.
fn riferimenti_senza_label(t: &str) -> Vec<String> {
    let mut mancanti = Vec::new();
    for (apertura, chiusura) in [("\\ref{", '}'), ("\\eqref{", '}'), ("\\hyperref[", ']')] {
        for p in t.split(apertura).skip(1) {
            let chiave = p.split(chiusura).next().unwrap_or("");
            if !t.contains(&format!("\\label{{{chiave}}}")) {
                mancanti.push(chiave.to_string());
            }
        }
    }
    mancanti
}

#[test]
fn latex_titolo_non_numerato_con_etichetta_ha_il_label() {
    let t = tex("= Intro <x>\n\nVedi @x.\n").contenuto;
    assert!(
        t.contains("\\section*{Intro}\\phantomsection\\label{et:x}"),
        "{t}"
    );
    assert!(riferimenti_senza_label(&t).is_empty(), "{t}");
    // senza numero non si cita `\ref` (darebbe la sezione precedente)
    assert!(!t.contains("\\ref{et:x}"), "{t}");
    assert!(t.contains("\\hyperref[et:x]{Section}"), "{t}");
    assert!(t.contains("\\usepackage{hyperref}"), "{t}");
    // con la numerazione resta `\ref` + `\label` subito dopo il titolo
    let n = tex("#set heading(numbering: \"1.\")\n= Intro <x>\n\nVedi @x.\n").contenuto;
    assert!(n.contains("\\section{Intro}\\label{et:x}"), "{n}");
    assert!(
        n.contains("\\ref{et:x}") && !n.contains("\\phantomsection"),
        "{n}"
    );
    assert!(riferimenti_senza_label(&n).is_empty(), "{n}");
}

/// Ambienti `\begin{…}`/`\end{…}` in ordine: (posizione, apertura?, nome).
fn ambienti_latex(t: &str) -> Vec<(usize, bool, String)> {
    let mut v = Vec::new();
    for (tag, apre) in [("\\begin{", true), ("\\end{", false)] {
        for (pos, _) in t.match_indices(tag) {
            let nome = t[pos + tag.len()..].split('}').next().unwrap_or("");
            v.push((pos, apre, nome.to_string()));
        }
    }
    v.sort();
    v
}

/// Descrive il primo ambiente flottante aperto dentro una lista, se c'è.
fn flottante_in_lista(t: &str) -> Option<String> {
    let mut pila: Vec<String> = Vec::new();
    for (_, apre, nome) in ambienti_latex(t) {
        if !apre {
            pila.pop();
            continue;
        }
        let flottante = matches!(nome.as_str(), "figure" | "table" | "listato");
        if flottante
            && pila
                .iter()
                .any(|e| matches!(e.as_str(), "itemize" | "enumerate" | "description"))
        {
            return Some(format!("{nome} dentro {pila:?}"));
        }
        pila.push(nome);
    }
    None
}

#[test]
fn latex_figura_in_una_lista_non_e_flottante() {
    let src = "- voce\n  #figure(image(\"a.png\"), caption: [Cap]) <f>\n\n\
               + altra\n  #figure(table(columns: 1, [x]), caption: [Tab])\n\n\
               - senza didascalia\n  #figure(image(\"b.png\")) <g>\n\n\
               Vedi @f e @g.\n\n\
               #figure(image(\"c.png\"), caption: [Fuori])\n";
    let t = tex(src).contenuto;
    assert_eq!(flottante_in_lista(&t), None, "{t}");
    assert!(t.contains("\\usepackage{caption}"), "{t}");
    assert!(t.contains("\\captionof{figure}{Cap}\n\\label{et:f}"), "{t}");
    assert!(t.contains("\\captionof{table}{Tab}"), "{t}");
    assert!(t.contains("\\refstepcounter{figure}\n\\label{et:g}"), "{t}");
    assert!(riferimenti_senza_label(&t).is_empty(), "{t}");
    // fuori dalla lista resta un vero flottante
    assert!(t.contains("\\begin{figure}[htbp]"), "{t}");
    assert!(t.contains("\\caption{Fuori}"), "{t}");
    // senza figure in liste, il pacchetto non è caricato
    let solo_fuori = tex("#figure(image(\"c.png\"), caption: [Fuori])\n").contenuto;
    assert!(!solo_fuori.contains("{caption}"), "{solo_fuori}");
    // il verificatore non è vacuo: riconosce il caso sbagliato
    let sbagliato = "\\begin{itemize}\n\\begin{figure}\n\\end{figure}\n\\end{itemize}";
    assert!(flottante_in_lista(sbagliato).is_some());
}

// ---- HTML: testo alternativo delle immagini ----

#[test]
fn html_alt_dalla_didascalia_o_dall_argomento() {
    let alt_di = |h: &str| -> Vec<String> { valori(h, " alt=\"") };
    // didascalia come ripiego (testo semplice, senza markup)
    let h = html("#figure(image(\"a.png\"), caption: [Un *grande* grafico])").contenuto;
    assert_eq!(alt_di(&h), ["Un grande grafico"], "{h}");
    // `alt:` dell'immagine ha la precedenza sulla didascalia
    let h = html("#figure(image(\"a.png\", alt: \"Barre blu\"), caption: [Didascalia])").contenuto;
    assert_eq!(alt_di(&h), ["Barre blu"], "{h}");
    // `alt:` della figura
    let h =
        html("#figure(image(\"a.png\"), alt: \"Descrizione\", caption: [Didascalia])").contenuto;
    assert_eq!(alt_di(&h), ["Descrizione"], "{h}");
    let h = html("#figure(image(\"a.png\"), alt: \"Solo alt\")").contenuto;
    assert_eq!(alt_di(&h), ["Solo alt"], "{h}");
    // immagine isolata senza alt né didascalia: resta vuoto (decorativa)
    let h = html("#image(\"a.png\")").contenuto;
    assert_eq!(alt_di(&h), [""], "{h}");
    // l'alt è scappato: nessuna virgoletta che chiuda l'attributo
    let h = html("#figure(image(\"a.png\"), caption: [Dice \"ciao\" e <b>])").contenuto;
    let alt = alt_di(&h);
    assert_eq!(alt.len(), 1, "{h}");
    assert!(alt[0].contains("ciao") && !alt[0].contains('<'), "{h}");
}
