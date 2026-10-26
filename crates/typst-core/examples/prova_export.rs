//! Prova di esportazione headless: compila i documenti di una cartella-progetto
//! ed esporta in tutti i formati del core, con tempi, dimensioni e memoria.
//!
//! Uso: `cargo run --release -p typst-core --example prova_export -- <cartella> [ripetizioni] [doc1.typ doc2.typ ...]`
//!
//! Percorsi di codice identici a quelli dell'app (guscio Tauri): `apri_progetto`,
//! `compila`, `esporta_pdf_con`, `pagine_da_esportare` + `png`/`svg`,
//! `typst_a_md` / `md_a_typst`, più le esportazioni pure `typst_a_testo`,
//! `typst_a_html` (anche con immagini data-URI) e `typst_a_latex`. Output in `<cartella>/export/<doc>/`.
//! Stampa righe `chiave = valore` (tempi in ms, mediana su N ripetizioni).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use typst_core::esporta::percorsi_pagine;
use typst_core::{
    data_uri, md_a_typst, typst_a_html, typst_a_latex, typst_a_md, typst_a_md_con_avvisi,
    typst_a_testo, EditorWorld, OpzioniHtml, OpzioniLatex, OpzioniPdf, OpzioniTesto, StandardPdf,
};

/// Picco del working set in byte (solo Windows; altrove 0).
#[cfg(windows)]
fn picco_memoria() -> usize {
    #[repr(C)]
    struct Contatori {
        cb: u32,
        page_fault: u32,
        picco_ws: usize,
        ws: usize,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        pagefile: usize,
        picco_pagefile: usize,
    }
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(p: isize, c: *mut Contatori, cb: u32) -> i32;
    }
    let mut c = Contatori {
        cb: std::mem::size_of::<Contatori>() as u32,
        page_fault: 0,
        picco_ws: 0,
        ws: 0,
        a: 0,
        b: 0,
        c: 0,
        d: 0,
        pagefile: 0,
        picco_pagefile: 0,
    };
    // SAFETY: `c` è una struttura #[repr(C)] valida con `cb` corretto; la
    // pseudo-handle di GetCurrentProcess è sempre valida nel processo corrente.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok != 0 {
        c.picco_ws
    } else {
        0
    }
}

#[cfg(not(windows))]
fn picco_memoria() -> usize {
    0
}

fn mediana(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn ms<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let r = f();
    (r, t.elapsed().as_secs_f64() * 1000.0)
}

fn mb(b: usize) -> f64 {
    b as f64 / 1_048_576.0
}

/// Ripete `f` `n` volte; ritorna (mediana ms, ultimo risultato).
fn ripeti<T>(n: usize, mut f: impl FnMut() -> T) -> (f64, T) {
    let mut tempi = Vec::new();
    let mut ultimo = None;
    for _ in 0..n {
        let (r, t) = ms(&mut f);
        tempi.push(t);
        ultimo = Some(r);
    }
    (mediana(tempi), ultimo.expect("n>0"))
}

fn scrivi(p: &Path, dati: &[u8]) {
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(p, dati) {
        eprintln!("scrittura {} fallita: {e}", p.display());
    }
}

fn nuovo_mondo(cartella: &Path, doc: &str, testo: &str) -> EditorWorld {
    let mut m = EditorWorld::nuovo("");
    m.apri_progetto(cartella.to_path_buf(), doc, testo)
        .expect("apri_progetto");
    m
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cartella = PathBuf::from(args.first().expect("serve la cartella"));
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let docs: Vec<String> = if args.len() > 2 {
        args[2..].to_vec()
    } else {
        vec!["scientifico.typ".into(), "letteratura.typ".into()]
    };

    for doc in docs {
        let nome = doc.trim_end_matches(".typ").to_string();
        let out = cartella.join("export").join(&nome);
        let _ = std::fs::create_dir_all(&out);
        let testo = std::fs::read_to_string(cartella.join(&doc)).expect("lettura sorgente");
        println!(
            "== {doc}: {} caratteri, {} byte",
            testo.chars().count(),
            testo.len()
        );

        // --- compilazione a freddo (prima nel processo per il primo doc: font+cache vuoti)
        let t0 = Instant::now();
        let mut m = nuovo_mondo(&cartella, &doc, &testo);
        let nuovo_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let (r, freddo) = ms(|| m.compila());
        println!("{nome}.nuovo_world_ms = {nuovo_ms:.1}");
        println!("{nome}.compila_prima_nel_processo_ms = {freddo:.1}");
        for d in &r.diagnostici {
            println!("{nome}.diagnostico[{}] = {}", d.gravita, d.messaggio);
        }
        let pagine = r.pagine.len();
        println!("{nome}.pagine = {pagine}");
        if pagine == 0 {
            println!("{nome}: compilazione fallita, salto");
            continue;
        }

        // --- a caldo: stesso sorgente (cache comemo) e sorgente modificato (1 char)
        let (caldo_uguale, _) = ripeti(n, || m.compila());
        println!("{nome}.compila_caldo_invariato_ms = {caldo_uguale:.1}");
        let mut k = 0;
        let (caldo_mod, _) = ripeti(n, || {
            k += 1;
            m.aggiorna_sorgente(&format!("{testo}\n// {}", "x".repeat(k)));
            m.compila()
        });
        println!("{nome}.compila_caldo_commento_in_coda_ms = {caldo_mod:.1}");
        // Battitura realistica: un carattere nuovo in una frase a circa metà documento.
        let meta = testo
            .char_indices()
            .filter(|(i, _)| *i > testo.len() / 2)
            .find(|(i, _)| testo[*i..].starts_with(" che "))
            .map(|(i, _)| i)
            .unwrap_or(testo.len() / 2);
        let mut k = 0;
        let (caldo_tipo, _) = ripeti(n, || {
            k += 1;
            let mut t = testo.clone();
            t.insert_str(meta, &"x".repeat(k));
            m.aggiorna_sorgente(&t);
            m.compila()
        });
        println!("{nome}.compila_caldo_battitura_meta_doc_ms = {caldo_tipo:.1}");
        // Export PDF subito dopo una modifica (compilazione non memoizzata inclusa).
        let mut k = 100;
        let (pdf_dopo_modifica, _) = ripeti(n, || {
            k += 1;
            let mut t = testo.clone();
            t.insert_str(meta, &"x".repeat(k));
            m.aggiorna_sorgente(&t);
            m.esporta_pdf()
        });
        println!("{nome}.pdf_dopo_modifica_compila_e_export_ms = {pdf_dopo_modifica:.1}");
        m.aggiorna_sorgente(&testo);
        let _ = m.compila();
        m.aggiorna_sorgente(&testo);
        let _ = m.compila();

        // --- compilazione a freddo ripetuta con World nuovo (cache comemo ancora calda ma
        // font e file ricaricati): stima del "riapri documento"
        let (riapri, _) = ripeti(n, || {
            let mut w = nuovo_mondo(&cartella, &doc, &testo);
            w.compila()
        });
        println!("{nome}.apri_e_compila_world_nuovo_ms = {riapri:.1}");

        // --- SVG anteprima (percorso della vista)
        let (svg_anteprima, _) = ripeti(n, || {
            (0..pagine)
                .filter_map(|i| m.svg_pagina(i))
                .map(|p| p.svg.len())
                .sum::<usize>()
        });
        println!("{nome}.svg_anteprima_tutte_le_pagine_ms = {svg_anteprima:.1}");

        // --- PDF: nessuno standard + tutti gli standard
        let tutti: Vec<(&str, Vec<StandardPdf>)> = vec![
            ("pdf", vec![]),
            ("pdf-1.4", vec![StandardPdf::V1_4]),
            ("pdf-1.7", vec![StandardPdf::V1_7]),
            ("pdf-2.0", vec![StandardPdf::V2_0]),
            ("pdf-a-1b", vec![StandardPdf::A1b]),
            ("pdf-a-1a", vec![StandardPdf::A1a]),
            ("pdf-a-2b", vec![StandardPdf::A2b]),
            ("pdf-a-2u", vec![StandardPdf::A2u]),
            ("pdf-a-2a", vec![StandardPdf::A2a]),
            ("pdf-a-3b", vec![StandardPdf::A3b]),
            ("pdf-a-3u", vec![StandardPdf::A3u]),
            ("pdf-a-3a", vec![StandardPdf::A3a]),
            ("pdf-a-4", vec![StandardPdf::A4]),
            ("pdf-a-4f", vec![StandardPdf::A4f]),
            ("pdf-a-4e", vec![StandardPdf::A4e]),
            ("pdf-ua-1", vec![StandardPdf::Ua1]),
        ];
        for (etichetta, standard) in tutti {
            let opz = OpzioniPdf {
                standard,
                pagine: None,
            };
            let mut ultimo: Result<Vec<u8>, Vec<String>> = Err(vec![]);
            let (t, _) = ripeti(n, || {
                ultimo = m.esporta_pdf_con(&opz);
            });
            match ultimo {
                Ok(byte) => {
                    println!(
                        "{nome}.{etichetta}.ms = {t:.1}  byte = {}  (compila+export)",
                        byte.len()
                    );
                    scrivi(&out.join(format!("{nome}.{etichetta}.pdf")), &byte);
                }
                Err(e) => println!("{nome}.{etichetta}.ERRORE = {}", e.join(" | ")),
            }
        }

        // --- PNG per pagina (144 e 300 PPI) e SVG per pagina
        for ppi in [72.0f32, 144.0, 300.0] {
            let mut prep = Vec::new();
            let mut rend = Vec::new();
            let mut ultimo: Vec<Vec<u8>> = Vec::new();
            for _ in 0..n {
                let (sel, tp) = ms(|| m.pagine_da_esportare(None));
                let sel = sel.expect("pagine");
                let (png, tr) = ms(|| sel.png(ppi));
                prep.push(tp);
                rend.push(tr);
                ultimo = png.expect("png");
            }
            let dest = out.join(format!("{nome}-ppi{ppi:.0}.png"));
            let perc =
                percorsi_pagine(&dest, &(1..=ultimo.len()).collect::<Vec<_>>(), ultimo.len());
            let totale: usize = ultimo.iter().map(Vec::len).sum();
            for (p, b) in perc.iter().zip(&ultimo) {
                scrivi(p, b);
            }
            println!(
                "{nome}.png-{ppi:.0}ppi.compila_ms = {:.1}  render_encode_ms = {:.1}  byte_totali = {totale}  pagine = {}",
                mediana(prep),
                mediana(rend),
                ultimo.len()
            );
        }
        {
            let mut prep = Vec::new();
            let mut rend = Vec::new();
            let mut ultimo: Vec<String> = Vec::new();
            for _ in 0..n {
                let (sel, tp) = ms(|| m.pagine_da_esportare(None));
                let sel = sel.expect("pagine");
                let (svg, tr) = ms(|| sel.svg());
                prep.push(tp);
                rend.push(tr);
                ultimo = svg;
            }
            let dest = out.join(format!("{nome}.svg"));
            let perc =
                percorsi_pagine(&dest, &(1..=ultimo.len()).collect::<Vec<_>>(), ultimo.len());
            let totale: usize = ultimo.iter().map(String::len).sum();
            for (p, s) in perc.iter().zip(&ultimo) {
                scrivi(p, s.as_bytes());
            }
            println!(
                "{nome}.svg.compila_ms = {:.1}  serializza_ms = {:.1}  byte_totali = {totale}  pagine = {}",
                mediana(prep),
                mediana(rend),
                ultimo.len()
            );
        }

        // --- Testo: typ -> md -> typ
        let (t_md, md) = ripeti(n, || typst_a_md(&testo));
        println!("{nome}.typ_a_md_ms = {t_md:.2}  byte = {}", md.len());
        scrivi(&out.join(format!("{nome}.md")), md.as_bytes());
        let (t_ty, typ2) = ripeti(n, || md_a_typst(&md));
        println!("{nome}.md_a_typ_ms = {t_ty:.2}  byte = {}", typ2.len());
        scrivi(&out.join(format!("{nome}.roundtrip.typ")), typ2.as_bytes());
        let mut m2 = nuovo_mondo(
            &cartella,
            &format!("{nome}.roundtrip.typ"), // nella radice: i percorsi relativi (refs.bib, img/) restano validi
            &typ2,
        );
        let r2 = m2.compila();
        let errori: Vec<_> = r2
            .diagnostici
            .iter()
            .filter(|d| d.gravita == "errore")
            .map(|d| d.messaggio.clone())
            .collect();
        println!(
            "{nome}.roundtrip_compila: pagine = {} errori = {:?}",
            r2.pagine.len(),
            errori
        );
        // PDF del round-trip per confronto
        if let Ok(b) = m2.esporta_pdf() {
            scrivi(&out.join(format!("{nome}.roundtrip.pdf")), &b);
            println!("{nome}.roundtrip.pdf byte = {}", b.len());
        }
        println!("{nome}.picco_working_set_MB = {:.1}", mb(picco_memoria()));

        // --- Esportazioni pure da sorgente: md (con avvisi), testo, HTML, LaTeX
        let (t_mda, mda) = ripeti(n, || typst_a_md_con_avvisi(&testo));
        println!(
            "{nome}.typ_a_md_con_avvisi_ms = {t_mda:.2}  byte = {}  avvisi = {}",
            mda.contenuto.len(),
            mda.avvisi.len()
        );
        let (t_txt, c_txt) = ripeti(n, || typst_a_testo(&testo, &OpzioniTesto::default()));
        println!(
            "{nome}.typ_a_testo_ms = {t_txt:.2}  byte = {}  avvisi = {}",
            c_txt.contenuto.len(),
            c_txt.avvisi.len()
        );
        scrivi(&out.join(format!("{nome}.txt")), c_txt.contenuto.as_bytes());

        let (t_html, c_html) = ripeti(n, || typst_a_html(&testo, &OpzioniHtml::default()));
        println!(
            "{nome}.typ_a_html_ms = {t_html:.2}  byte = {}  avvisi = {}",
            c_html.contenuto.len(),
            c_html.avvisi.len()
        );
        scrivi(
            &out.join(format!("{nome}.html")),
            c_html.contenuto.as_bytes(),
        );

        // HTML con immagini incorporate (data-URI): il chiamante legge i file.
        let radice = cartella.clone();
        let opz_incorpora = OpzioniHtml {
            risolvi_immagine: Some(Arc::new(move |p: &str| {
                let byte = std::fs::read(radice.join(p)).ok()?;
                let mime = match Path::new(p).extension()?.to_str()? {
                    "png" => "image/png",
                    "svg" => "image/svg+xml",
                    "jpg" | "jpeg" => "image/jpeg",
                    _ => return None,
                };
                Some(data_uri(mime, &byte))
            })),
            ..Default::default()
        };
        let (t_htmlb, c_htmlb) = ripeti(n, || typst_a_html(&testo, &opz_incorpora));
        println!(
            "{nome}.typ_a_html_data_uri_ms = {t_htmlb:.2}  byte = {}",
            c_htmlb.contenuto.len()
        );
        scrivi(
            &out.join(format!("{nome}.autonomo.html")),
            c_htmlb.contenuto.as_bytes(),
        );

        let (t_tex, c_tex) = ripeti(n, || typst_a_latex(&testo, &OpzioniLatex::default()));
        println!(
            "{nome}.typ_a_latex_ms = {t_tex:.2}  byte = {}  avvisi = {}",
            c_tex.contenuto.len(),
            c_tex.avvisi.len()
        );
        scrivi(&out.join(format!("{nome}.tex")), c_tex.contenuto.as_bytes());
        for a in c_tex.avvisi.iter().chain(&c_html.avvisi).chain(&mda.avvisi) {
            println!("{nome}.avviso = {a}");
        }

        // Compilazione LaTeX, solo se un compilatore è nel PATH.
        let compilatore = ["pdflatex", "xelatex", "tectonic"].into_iter().find(|c| {
            std::process::Command::new(c)
                .arg("--version")
                .output()
                .is_ok()
        });
        match compilatore {
            Some(c) => {
                let (stato, t) = ms(|| {
                    std::process::Command::new(c)
                        .args(if c == "tectonic" {
                            vec![]
                        } else {
                            vec!["-interaction=nonstopmode", "-halt-on-error"]
                        })
                        .arg(format!("{nome}.tex"))
                        .current_dir(&out)
                        .output()
                });
                match stato {
                    Ok(o) => println!(
                        "{nome}.latex_compila_{c} = {} ({t:.0} ms)",
                        if o.status.success() { "OK" } else { "ERRORE" }
                    ),
                    Err(e) => println!("{nome}.latex_compila_{c} = impossibile avviare: {e}"),
                }
            }
            None => println!(
                "{nome}.latex_compila = SALTATO (nessun pdflatex/xelatex/tectonic nel PATH)"
            ),
        }
    }
    println!("picco_working_set_processo_MB = {:.1}", mb(picco_memoria()));
}
