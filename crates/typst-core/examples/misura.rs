//! Misure di prestazione del core (F5-007/008/009): `cargo run --release -p
//! typst-core --example misura` (wrapper: `tools/misura.ps1`).
//!
//! Stampa tempi e byte in formato `chiave = valore`, per confrontare prima/dopo.

use std::time::Instant;

use typst_core::EditorWorld;

/// Documento di prova: `pagine` sezioni, ciascuna su una pagina propria.
fn documento(pagine: usize) -> String {
    let mut s = String::new();
    for i in 1..=pagine {
        s.push_str(&format!("= Sezione {i}\n#lorem(250)\n"));
        if i < pagine {
            s.push_str("#pagebreak()\n");
        }
    }
    s
}

/// Contenuto "ricco" di una pagina: testo, gradienti (lineare su testo, radiale),
/// box con clip, tiling e un'immagine SVG (il conico è a parte). Stessa sorgente
/// del test `forma_reale_dell_svg` in `world.rs`.
const BLOCCO_RICCO: &str = r##"#text(fill: gradient.linear(red, blue))[Testo con gradiente lineare]
#box(clip: true, width: 3cm, height: 0.8cm, fill: gradient.radial(red, blue))[Testo molto lungo che viene tagliato dal box]
#rect(width: 3cm, height: 1cm, fill: tiling(size: (10pt, 10pt))[#square(size: 5pt)])
#image(bytes("<svg xmlns='http://www.w3.org/2000/svg' width='40' height='20'><rect width='40' height='20' fill='green'/><circle cx='20' cy='10' r='8' fill='yellow'/></svg>"), format: "svg", width: 2cm)
"##;

/// Gradiente conico: genera molti sotto-gradienti (id `s…`), circa 180 KB a pagina.
const BLOCCO_CONICO: &str =
    "#rect(width: 4cm, height: 1.5cm, fill: gradient.conic(red, blue, green))\n";

/// Come `documento`, ma ogni pagina contiene anche [`BLOCCO_RICCO`] (e, se
/// `conico`, [`BLOCCO_CONICO`]).
fn documento_ricco(pagine: usize, conico: bool) -> String {
    let mut s = String::new();
    let extra = if conico { BLOCCO_CONICO } else { "" };
    for i in 1..=pagine {
        s.push_str(&format!(
            "= Sezione {i}\n#lorem(120)\n{BLOCCO_RICCO}{extra}"
        ));
        if i < pagine {
            s.push_str("#pagebreak()\n");
        }
    }
    s
}

/// Salva gli SVG di ogni pagina in `cartella` (per `tools/misura_dedup.mjs`).
fn salva_pagine(mondo: &mut EditorWorld, testo: &str, cartella: &str) {
    mondo.aggiorna_sorgente(testo);
    let r = mondo.compila();
    let _ = std::fs::create_dir_all(cartella);
    for i in 0..r.pagine.len() {
        if let Some(p) = mondo.svg_pagina(i) {
            let _ = std::fs::write(format!("{cartella}/p{i:02}.svg"), p.svg);
        }
    }
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn main() {
    let t = Instant::now();
    let mut mondo = EditorWorld::nuovo("");
    println!("nuovo_ms = {:.1}", t.elapsed().as_secs_f64() * 1000.0);

    for pagine in [1usize, 60] {
        let base = documento(pagine);
        mondo.aggiorna_sorgente(&base);
        let t = Instant::now();
        let r = mondo.compila();
        println!(
            "p{pagine}.prima_compilazione_ms = {:.1}",
            t.elapsed().as_secs_f64() * 1000.0
        );
        println!("p{pagine}.pagine = {}", r.pagine.len());
        // Tutte le pagine serializzate (confronto col vecchio "render di tutto").
        let byte: usize = (0..r.pagine.len())
            .filter_map(|i| mondo.svg_pagina(i))
            .map(|p| p.svg.len())
            .sum();
        println!("p{pagine}.svg_byte_totali_tutte_le_pagine = {byte}");

        // Battitura: si aggiunge un carattere a metà documento.
        // (in coda al titolo della sezione centrale: sintassi sempre valida).
        let cerca = format!(
            "= Sezione {}
",
            pagine.div_ceil(2)
        );
        let meta = base.find(&cerca).expect("titolo presente") + cerca.len() - 1;
        let mut tempi = Vec::new();
        for k in 0..10 {
            let mut testo = base.clone();
            testo.insert_str(meta, &"x".repeat(k + 1));
            mondo.aggiorna_sorgente(&testo);
            let t = Instant::now();
            let r = mondo.compila();
            // Come il frontend: SVG solo delle pagine in vista (3 centrali).
            let centro = r.pagine.len() / 2;
            for i in centro.saturating_sub(1)..(centro + 2).min(r.pagine.len()) {
                let _ = mondo.svg_pagina(i);
            }
            tempi.push(t.elapsed().as_secs_f64() * 1000.0);
            assert!(!r.pagine.is_empty(), "il documento di prova deve compilare");
        }
        println!(
            "p{pagine}.battitura_compila_e_3_pagine_mediana_ms = {:.1}",
            mediana(tempi)
        );
    }

    // Con MISURA_DUMP=<cartella> salva gli SVG: `lorem/` (60 pagine di solo
    // testo), `ricco/` (60 pagine con gradienti/clip/tiling/immagine) e
    // `fixture/` (1 pagina ricca senza conico, per src/fixtures/pagina-ricca.svg).
    if let Ok(cartella) = std::env::var("MISURA_DUMP") {
        salva_pagine(&mut mondo, &documento(60), &format!("{cartella}/lorem"));
        salva_pagine(
            &mut mondo,
            &documento_ricco(60, true),
            &format!("{cartella}/ricco"),
        );
        salva_pagine(
            &mut mondo,
            &documento_ricco(1, false).replace("#lorem(120)", "#lorem(8)"),
            &format!("{cartella}/fixture"),
        );
    }
}
