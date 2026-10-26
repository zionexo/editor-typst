//! Generatore di testo semplice (.txt) dal documento intermedio.
//!
//! Layout: titoli sottolineati (`=` livello 1, `-` livello 2, `~` livello 3)
//! con eventuale numerazione, paragrafi a capo a larghezza fissa, liste con
//! rientro sospeso, citazioni e codice rientrati di 4 spazi, tabelle ASCII,
//! formule in forma lineare Unicode, figure come `[Figura N: didascalia]`,
//! note numerate `[n]` raccolte in coda.

use super::ir::{
    lessico, testo_semplice, Allineamento, Blocco, Documento, Inline, Tabella, TipoFigura,
};
use super::matematica::{a_apice, a_pedice, lineare};

pub(crate) struct Rend<'d> {
    doc: &'d Documento,
    larghezza: usize,
    note: Vec<Vec<Blocco>>,
}

fn n_car(s: &str) -> usize {
    s.chars().count()
}

/// A capo "greedy" a `larghezza` colonne; i `\n` presenti sono a capo forzati.
pub(crate) fn avvolgi(testo: &str, larghezza: usize) -> String {
    let larghezza = larghezza.max(1);
    let mut righe: Vec<String> = Vec::new();
    for paragrafo in testo.split('\n') {
        let mut riga = String::new();
        let mut len = 0usize;
        for parola in paragrafo.split(' ').filter(|p| !p.is_empty()) {
            let lp = n_car(parola);
            if len == 0 {
                riga.push_str(parola);
                len = lp;
            } else if len + 1 + lp <= larghezza {
                riga.push(' ');
                riga.push_str(parola);
                len += 1 + lp;
            } else {
                righe.push(std::mem::take(&mut riga));
                riga.push_str(parola);
                len = lp;
            }
        }
        righe.push(riga);
    }
    righe.join("\n")
}

fn rientra(testo: &str, prefisso_prima: &str, prefisso_altre: &str) -> String {
    testo
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            let p = if i == 0 {
                prefisso_prima
            } else {
                prefisso_altre
            };
            if l.is_empty() {
                p.trim_end().to_string()
            } else {
                format!("{p}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl<'d> Rend<'d> {
    pub fn nuovo(doc: &'d Documento, larghezza: usize) -> Self {
        Self {
            doc,
            larghezza,
            note: Vec::new(),
        }
    }

    pub fn documento(&mut self) -> String {
        let doc = self.doc;
        let corpo = self.blocchi(&doc.blocchi, self.larghezza);
        let mut out = corpo;
        // Note in coda. Il ciclo copre anche note generate da altre note.
        let mut i = 0;
        let mut sezione: Vec<String> = Vec::new();
        while i < self.note.len() {
            let blocchi = self.note[i].clone();
            let marcatore = format!("[{}] ", i + 1);
            let w = self.larghezza.saturating_sub(n_car(&marcatore)).max(10);
            let testo = self.blocchi(&blocchi, w);
            let vuoto = " ".repeat(n_car(&marcatore));
            sezione.push(rientra(&testo, &marcatore, &vuoto));
            i += 1;
        }
        if !sezione.is_empty() {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&"-".repeat(self.larghezza.min(20)));
            out.push('\n');
            out.push_str(&sezione.join("\n"));
        }
        let out = out.replace('\u{00A0}', " ").replace('\u{00AD}', "");
        let mut out = out.trim_end().to_string();
        out.push('\n');
        out
    }

    fn blocchi(&mut self, bl: &[Blocco], w: usize) -> String {
        let mut parti: Vec<String> = Vec::new();
        for b in bl {
            let t = self.blocco(b, w);
            if !t.trim().is_empty() {
                parti.push(t);
            }
        }
        parti.join("\n\n")
    }

    fn titolo_sottolineato(&self, testo: &str, livello: usize, w: usize) -> String {
        let avv = avvolgi(testo, w);
        let massimo = avv.split('\n').map(n_car).max().unwrap_or(0);
        let c = match livello {
            1 => Some('='),
            2 => Some('-'),
            3 => Some('~'),
            _ => None,
        };
        match c {
            Some(c) => format!("{avv}\n{}", c.to_string().repeat(massimo)),
            None => avv,
        }
    }

    fn blocco(&mut self, b: &Blocco, w: usize) -> String {
        match b {
            Blocco::Titolo {
                livello,
                numero,
                contenuto,
                ..
            } => {
                let t = self.inline(contenuto);
                let t = match numero {
                    Some(n) if !n.is_empty() => format!("{n} {t}"),
                    _ => t,
                };
                self.titolo_sottolineato(&t, *livello, w)
            }
            Blocco::Paragrafo(v) => {
                let t = self.inline(v);
                avvolgi(&t, w)
            }
            Blocco::Lista {
                ordinata,
                inizio,
                voci,
            } => self.lista(*ordinata, *inizio, voci, w),
            Blocco::Termini(v) => {
                let mut parti = Vec::new();
                for (t, d) in v {
                    let titolo = avvolgi(&self.inline(t), w);
                    let desc = self.blocchi(d, w.saturating_sub(4).max(10));
                    parti.push(format!("{titolo}\n{}", rientra(&desc, "    ", "    ")));
                }
                parti.join("\n")
            }
            Blocco::Citazione {
                corpo,
                attribuzione,
            } => {
                let mut t = self.blocchi(corpo, w.saturating_sub(4).max(10));
                if let Some(a) = attribuzione {
                    let a = format!("— {}", self.inline(a));
                    t.push_str("\n\n");
                    t.push_str(&avvolgi(&a, w.saturating_sub(4).max(10)));
                }
                rientra(&t, "    ", "    ")
            }
            Blocco::Codice { testo, .. } => rientra(testo, "    ", "    "),
            Blocco::Figura {
                corpo,
                didascalia,
                tipo,
                numero,
                ..
            } => self.figura(corpo, didascalia.as_deref(), *tipo, numero, w),
            Blocco::Immagine(i) => {
                let nome = i.alt.clone().unwrap_or_else(|| i.percorso.clone());
                avvolgi(&format!("[Immagine: {nome}]"), w)
            }
            Blocco::Tabella(t) => self.tabella(t, w),
            Blocco::Equazione {
                formula, numero, ..
            } => {
                let l = lineare(&formula.m);
                let interno = w.saturating_sub(4).max(10);
                let testo = avvolgi(&l, interno);
                let mut righe: Vec<String> =
                    testo.split('\n').map(|r| format!("    {r}")).collect();
                if let Some(n) = numero {
                    let prima = righe[0].clone();
                    let spazio = w.saturating_sub(n_car(&prima) + n_car(n));
                    if spazio >= 2 {
                        righe[0] = format!("{prima}{}{n}", " ".repeat(spazio));
                    } else {
                        righe.push(format!("{}{n}", " ".repeat(w.saturating_sub(n_car(n)))));
                    }
                }
                righe.join("\n")
            }
            Blocco::Separatore => "-".repeat(w),
            Blocco::SaltoPagina => String::new(),
            Blocco::Segnaposto(t) => avvolgi(&format!("[{t}]"), w),
            Blocco::Bibliografia { file, titolo, .. } => {
                let lex = lessico(&self.doc.lingua);
                let mut parti = Vec::new();
                let t = match titolo {
                    Some(v) => testo_semplice(v),
                    None => lex.bibliografia.to_string(),
                };
                if !t.is_empty() {
                    parti.push(self.titolo_sottolineato(&t, 1, w));
                }
                parti.push(avvolgi(&format!("[Fonti bibliografiche: {file}]"), w));
                parti.join("\n\n")
            }
            Blocco::Indice => {
                let lex = lessico(&self.doc.lingua);
                let mut parti = vec![self.titolo_sottolineato(lex.indice, 1, w)];
                let mut righe = Vec::new();
                for (liv, num, cont, _) in self.doc.titoli() {
                    let t = testo_semplice(&cont);
                    let riga = match num {
                        Some(n) if !n.is_empty() => format!("{n} {t}"),
                        _ => t,
                    };
                    righe.push(format!("{}{riga}", "  ".repeat(liv.saturating_sub(1))));
                }
                parti.push(righe.join("\n"));
                parti.join("\n\n")
            }
        }
    }

    fn lista(&mut self, ordinata: bool, inizio: u64, voci: &[Vec<Blocco>], w: usize) -> String {
        let marcatori: Vec<String> = (0..voci.len())
            .map(|i| {
                if ordinata {
                    // saturante: `18446744073709551615. a` non deve traboccare
                    format!("{}. ", inizio.saturating_add(i as u64))
                } else {
                    "- ".to_string()
                }
            })
            .collect();
        let mw = marcatori.iter().map(|m| n_car(m)).max().unwrap_or(2);
        let interno = w.saturating_sub(mw).max(10);
        let mut righe = Vec::new();
        for (i, voce) in voci.iter().enumerate() {
            // paragrafo + sotto-lista: senza riga vuota in mezzo
            let mut testo = String::new();
            for (k, b) in voce.iter().enumerate() {
                let t = self.blocco(b, interno);
                if t.trim().is_empty() {
                    continue;
                }
                if k > 0 {
                    let stretta = matches!(b, Blocco::Lista { .. });
                    testo.push_str(if stretta { "\n" } else { "\n\n" });
                }
                testo.push_str(&t);
            }
            let mut m = marcatori[i].clone();
            while n_car(&m) < mw {
                m.insert(0, ' ');
            }
            righe.push(rientra(&testo, &m, &" ".repeat(mw)));
        }
        righe.join("\n")
    }

    fn figura(
        &mut self,
        corpo: &[Blocco],
        didascalia: Option<&[Inline]>,
        tipo: TipoFigura,
        numero: &str,
        w: usize,
    ) -> String {
        let lex = lessico(&self.doc.lingua);
        let parola = match tipo {
            TipoFigura::Figura => lex.figura,
            TipoFigura::Tabella => lex.tabella,
            TipoFigura::Listato => lex.listato,
        };
        let d = didascalia.map(|d| self.inline(d)).unwrap_or_default();
        let etichetta = if d.is_empty() {
            format!("{parola} {numero}")
        } else {
            format!("{parola} {numero}: {d}")
        };
        let ha_contenuto_testuale = corpo.iter().any(|b| {
            matches!(
                b,
                Blocco::Tabella(_) | Blocco::Codice { .. } | Blocco::Paragrafo(_)
            )
        });
        if ha_contenuto_testuale {
            let c = self.blocchi(corpo, w);
            format!("{c}\n\n{}", avvolgi(&etichetta, w))
        } else {
            avvolgi(&format!("[{etichetta}]"), w)
        }
    }

    fn tabella(&mut self, t: &Tabella, w: usize) -> String {
        let colonne = t.colonne.max(1);
        let mut righe: Vec<(Vec<(String, usize)>, bool)> = Vec::new(); // (celle, è_intestazione)
        for r in &t.intestazione {
            let celle = r
                .iter()
                .map(|c| {
                    (
                        self.inline(&c.contenuto).replace('\n', " "),
                        c.colspan.min(colonne),
                    )
                })
                .collect();
            righe.push((celle, true));
        }
        for r in &t.righe {
            let celle = r
                .iter()
                .map(|c| {
                    (
                        self.inline(&c.contenuto).replace('\n', " "),
                        c.colspan.min(colonne),
                    )
                })
                .collect();
            righe.push((celle, false));
        }
        // larghezze naturali (celle senza span)
        let mut larg = vec![3usize; colonne];
        for (celle, _) in &righe {
            let mut col = 0;
            for (testo, span) in celle {
                if *span == 1 && col < colonne {
                    larg[col] = larg[col].max(n_car(testo));
                }
                col += span;
            }
        }
        // le celle con span più larghe della somma richiedono spazio extra sull'ultima colonna
        for (celle, _) in &righe {
            let mut col = 0;
            for (testo, span) in celle {
                if *span > 1 && col + span <= colonne {
                    let attuale: usize =
                        larg[col..col + span].iter().sum::<usize>() + 3 * (span - 1);
                    let serve = n_car(testo);
                    if serve > attuale {
                        larg[col + span - 1] += serve - attuale;
                    }
                }
                col += span;
            }
        }
        let cornice = 3 * colonne + 1;
        let mut totale: usize = larg.iter().sum::<usize>() + cornice;
        while totale > w {
            let (i, m) = larg
                .iter()
                .enumerate()
                .max_by_key(|(_, v)| **v)
                .map(|(i, v)| (i, *v))
                .unwrap_or((0, 0));
            if m <= 6 {
                break;
            }
            larg[i] -= 1;
            totale -= 1;
        }
        let bordo = |c: char| -> String {
            let mut s = String::from("+");
            for l in &larg {
                s.push_str(&c.to_string().repeat(l + 2));
                s.push('+');
            }
            s
        };
        let mut out: Vec<String> = vec![bordo('-')];
        let n_righe = righe.len();
        for (idx, (celle, intest)) in righe.iter().enumerate() {
            // costruisci le celle avvolte
            let mut blocchi_cella: Vec<(Vec<String>, usize, usize)> = Vec::new(); // (righe, larghezza, colonna)
            let mut col = 0;
            for (testo, span) in celle {
                if col >= colonne {
                    break;
                }
                let span = (*span).min(colonne - col);
                let lw: usize = larg[col..col + span].iter().sum::<usize>() + 3 * (span - 1);
                let avv = avvolgi(testo, lw);
                blocchi_cella.push((avv.split('\n').map(str::to_string).collect(), lw, col));
                col += span;
            }
            let altezza = blocchi_cella
                .iter()
                .map(|c| c.0.len())
                .max()
                .unwrap_or(1)
                .max(1);
            for h in 0..altezza {
                let mut riga = String::from("|");
                for (righe_c, lw, c0) in &blocchi_cella {
                    let testo = righe_c.get(h).cloned().unwrap_or_default();
                    let al = t
                        .allineamenti
                        .get(*c0)
                        .copied()
                        .unwrap_or(Allineamento::Sinistra);
                    let pad = lw.saturating_sub(n_car(&testo));
                    let (a, d) = match (al, *intest) {
                        (Allineamento::Destra, _) => (pad, 0),
                        (Allineamento::Centro, _) | (_, true) => (pad / 2, pad - pad / 2),
                        _ => (0, pad),
                    };
                    riga.push(' ');
                    riga.push_str(&" ".repeat(a));
                    riga.push_str(&testo);
                    riga.push_str(&" ".repeat(d));
                    riga.push_str(" |");
                }
                out.push(riga);
            }
            let sep = if *intest && idx + 1 < n_righe && !righe[idx + 1].1 {
                '='
            } else {
                '-'
            };
            out.push(bordo(sep));
        }
        out.join("\n")
    }

    fn inline(&mut self, v: &[Inline]) -> String {
        let mut s = String::new();
        for i in v {
            match i {
                Inline::Testo(t) => s.push_str(t),
                Inline::Spazio => s.push(' '),
                Inline::ACapo => s.push('\n'),
                Inline::Enfasi(x)
                | Inline::Forte(x)
                | Inline::Barrato(x)
                | Inline::Sottolineato(x) => s.push_str(&self.inline(x)),
                Inline::Apice(x) => {
                    let t = self.inline(x);
                    match a_apice(&t) {
                        Some(c) => s.push_str(&c),
                        None => {
                            s.push_str("^(");
                            s.push_str(&t);
                            s.push(')');
                        }
                    }
                }
                Inline::Pedice(x) => {
                    let t = self.inline(x);
                    match a_pedice(&t) {
                        Some(c) => s.push_str(&c),
                        None => {
                            s.push_str("_(");
                            s.push_str(&t);
                            s.push(')');
                        }
                    }
                }
                Inline::Virgolette(x) => {
                    s.push('“');
                    s.push_str(&self.inline(x));
                    s.push('”');
                }
                Inline::Codice(t) => s.push_str(t),
                Inline::Link { url, contenuto } => {
                    let t = self.inline(contenuto);
                    if t == *url || t.is_empty() {
                        s.push_str(url);
                    } else {
                        s.push_str(&format!("{t} ({url})"));
                    }
                }
                Inline::Immagine(img) => {
                    let nome = img.alt.clone().unwrap_or_else(|| img.percorso.clone());
                    s.push_str(&format!("[Immagine: {nome}]"));
                }
                Inline::Math(f) => s.push_str(&lineare(&f.m).replace('\n', " ")),
                Inline::Nota(b) => {
                    self.note.push(b.clone());
                    s.push_str(&format!("[{}]", self.note.len()));
                }
                Inline::Rif {
                    bersaglio,
                    supplemento,
                } => {
                    let suppl = supplemento.as_ref().map(|v| testo_semplice(v));
                    match self.doc.risolvi(bersaglio, suppl.as_deref()) {
                        Some(t) => s.push_str(&t),
                        None => s.push_str(&format!("[{bersaglio}]")),
                    }
                }
                Inline::Ancora(_) => {}
            }
        }
        s
    }
}
