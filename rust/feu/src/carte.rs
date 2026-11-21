//! La carte : toute la Gironde, d'après la carte de web_conquete (gironde.json, vraie géographie en cases de 1 km),
//! en hexagones de 1 km : combustibles, relief, eaux, villes et villages, sites à protéger, routes, pistes DFCI, moyens fixes.

use std::collections::HashSet;

use serde_json::{json, Value};

use crate::combustibles::{self, est_eau};
use crate::gironde::gironde;
use crate::hasard::Hasard;
use crate::hexa::{astar_virages, simplifier, Grille, SQ3};
use crate::{argmin, rond};

#[derive(Clone, Debug)]
pub struct Site {
    pub id: usize,
    pub type_: String,
    pub nom: String,
    pub centre: usize,
    pub cells: Vec<usize>,
    pub valeur: i64,
    pub pop: i64,
}

#[derive(Clone, Debug)]
pub struct Caserne {
    pub id: usize,
    pub nom: String,
    pub cell: usize,
}

/// Un tronçon de route (n = 3, 2) ou de piste (n = 1) : ses cellules, puis son tracé simplifié (mètres).
#[derive(Clone, Debug)]
pub struct Voie {
    pub n: u8,
    pub c: Vec<usize>,
    pub p: Vec<[i64; 2]>,
}

fn valeur(typ: &str) -> i64 {
    match typ {
        "ville" => 100,
        "village" => 50,
        "camping" => 70,
        "usine" => 60,
        "reserve" => 25,
        _ => 30, // patrimoine
    }
}

pub struct Carte {
    pub graine: i64,
    pub theme: String,
    pub g: Grille,
    rnd: Hasard,
    pub fuel: Vec<u8>,
    pub alt: Vec<f64>,
    pub route: Vec<u8>,
    /// cellule -> site (-1 : aucun)
    pub site_de: Vec<i64>,
    /// cellule d'eau -> plan d'eau (-1 : aucun)
    pub corps: Vec<i64>,
    pub corps_taille: Vec<usize>,
    pub corps_nom: Vec<String>,
    pub sites: Vec<Site>,
    pub casernes: Vec<Caserne>,
    pub vigies: Vec<usize>,
    pub points_eau: Vec<usize>,
    pub helistation: Option<usize>,
    pub aerodrome: Option<usize>,
    pub nom_aerodrome: String,
    pub routes_poly: Vec<Voie>,
}

impl Carte {
    pub fn new(graine: i64, theme: &str) -> Carte {
        let hex_m = 1000.0;
        let gd = gironde();
        let r = hex_m / SQ3;
        let cols = (gd.w as f64 * 1000.0 / hex_m).round() as usize;
        let rows = ((gd.h as f64 * 1000.0 - 2.0 * r) / (1.5 * r)) as usize + 1;
        let g = Grille::new(cols, rows, hex_m);
        let n = g.n;
        let mut c = Carte {
            graine,
            theme: theme.to_string(),
            g,
            rnd: Hasard::new(graine.wrapping_mul(7919)),
            fuel: vec![b'P'; n],
            alt: vec![40.0; n],
            route: vec![0; n],
            site_de: vec![-1; n],
            corps: vec![-1; n],
            corps_taille: Vec::new(),
            corps_nom: Vec::new(),
            sites: Vec::new(),
            casernes: Vec::new(),
            vigies: Vec::new(),
            points_eau: Vec::new(),
            helistation: None,
            aerodrome: None,
            nom_aerodrome: String::new(),
            routes_poly: Vec::new(),
        };
        c.gironde();
        c.plans_eau();
        c.finaliser_routes();
        c
    }

    // ------------------------------------------------------------------ outils
    pub fn libre(&self, i: usize) -> bool {
        !est_eau(self.fuel[i]) && !b"SA".contains(&self.fuel[i]) && self.site_de[i] < 0
    }

    fn poser_site(&mut self, typ: &str, nom: &str, centre: usize, rayon: i64, fuel: Option<u8>, part: f64) -> usize {
        let sid = self.sites.len();
        let mut cells = Vec::new();
        for j in self.g.anneau(centre, rayon) {
            if est_eau(self.fuel[j]) || self.site_de[j] >= 0 {
                continue;
            }
            if j != centre && self.rnd.random() > part {
                continue;
            }
            if let Some(f) = fuel {
                self.fuel[j] = f;
            }
            self.site_de[j] = sid as i64;
            cells.push(j);
        }
        let pop = match typ {
            "ville" => 6000.0,
            "village" => 600.0,
            "camping" => 1500.0,
            "usine" => 80.0,
            "reserve" => 0.0,
            _ => 10.0,
        };
        let pop = (pop * self.rnd.uniform(0.6, 1.5)) as i64;
        self.sites.push(Site { id: sid, type_: typ.to_string(), nom: nom.to_string(), centre, cells, valeur: valeur(typ), pop });
        sid
    }

    /// La cellule de terre la plus proche d'un point (km de gironde.json).
    fn cellule(&self, xy: (f64, f64)) -> Option<usize> {
        let c = self.g.sous(xy.0 * 1000.0, xy.1 * 1000.0)?;
        let terre: Vec<usize> = self.g.anneau(c, 3).into_iter().filter(|&j| !est_eau(self.fuel[j])).collect();
        if !terre.is_empty() && est_eau(self.fuel[c]) {
            return argmin(terre, |j| self.g.distance(c, j));
        }
        Some(c)
    }

    fn ajouter_route(&mut self, ch: Vec<usize>) {
        for &i in &ch {
            self.route[i] = self.route[i].max(2);
        }
        self.routes_poly.push(Voie { n: 2, c: ch, p: Vec::new() });
    }

    // ------------------------------------------------------------------ la Gironde
    fn gironde(&mut self) {
        let gd = gironde();
        for i in 0..self.g.n {
            let x = ((self.g.cx[i] / 1000.0) as usize).min(gd.w - 1);
            let y = ((self.g.cy[i] / 1000.0) as usize).min(gd.h - 1);
            self.fuel[i] = gd.fuel[y].as_bytes()[x];
            self.alt[i] = (gd.alt[y].as_bytes()[x] as f64 - 48.0) * 4.0;
        }
        // villes et villages (la préfecture d'abord : sa caserne est le CSP, son hôpital reçoit les blessés)
        let mut lieux: Vec<_> = gd.lieux.iter().collect();
        lieux.sort_by_key(|l| l.type_ != "capitale");
        for l in lieux {
            let Some(c) = self.cellule(l.xy) else { continue };
            if self.site_de[c] >= 0 {
                continue;
            }
            match l.type_.as_str() {
                "capitale" => {
                    let sid = self.poser_site("ville", &l.nom, c, 3, Some(b'U'), 0.9);
                    self.sites[sid].pop *= 5;
                }
                "ville" => {
                    self.poser_site("ville", &l.nom, c, 1, Some(b'U'), 0.8);
                }
                t => {
                    self.poser_site(if t == "village" { "village" } else { "patrimoine" }, &l.nom, c, 0, Some(b'B'), 1.0);
                }
            }
            if l.d33 && (l.type_ == "capitale" || l.type_ == "ville") {
                let pre = if l.type_ == "capitale" { "CSP " } else { "CIS " };
                self.casernes.push(Caserne { id: self.casernes.len(), nom: format!("{pre}{}", l.nom), cell: c });
            }
        }
        for s in &gd.sites {
            let Some(c) = self.cellule(s.xy) else { continue };
            if self.site_de[c] >= 0 {
                continue;
            }
            let fuel = match s.type_.as_str() {
                "camping" => Some(b'T'),
                "usine" => Some(b'Z'),
                "patrimoine" => Some(b'B'),
                _ => None,
            };
            let reserve = s.type_ == "reserve";
            let fuel = if reserve || b"DS".contains(&self.fuel[c]) { None } else { fuel };
            self.poser_site(&s.type_, &s.nom, c, if reserve { 1 } else { 0 }, fuel, 0.9);
        }
        // les routes de web_conquete, posées sur les hexagones (coupées aux plans d'eau : pas de bacs)
        for r in &gd.routes {
            let mut ch: Vec<usize> = Vec::new();
            for w in r.p.windows(2) {
                let ((x0, y0), (x1, y1)) = (w[0], w[1]);
                let n = (((x1 - x0).hypot(y1 - y0) / 0.3) as usize).max(1);
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let Some(c) = self.g.sous((x0 + (x1 - x0) * t) * 1000.0, (y0 + (y1 - y0) * t) * 1000.0) else { continue };
                    if ch.last() == Some(&c) {
                        continue;
                    }
                    if let Some(&der) = ch.last() {
                        if !self.g.voisins(der).contains(&c) {
                            let li = self.g.ligne(der, c);
                            if li.len() > 2 {
                                ch.extend_from_slice(&li[1..li.len() - 1]);
                            }
                        }
                    }
                    ch.push(c);
                }
            }
            let mut run: Vec<usize> = Vec::new();
            for c in ch.iter().copied().map(Some).chain(std::iter::once(None)) {
                if let Some(c) = c {
                    if !b"ML".contains(&self.fuel[c]) {
                        run.push(c);
                        continue;
                    }
                }
                if run.len() > 1 {
                    for &i in &run {
                        self.route[i] = self.route[i].max(r.n);
                    }
                    self.routes_poly.push(Voie { n: r.n, c: std::mem::take(&mut run), p: Vec::new() });
                }
                run.clear();
            }
        }
        self.pistes_dfci();
        // une petite route vers chaque site qui n'en a pas
        for k in 0..self.sites.len() {
            if self.sites[k].cells.iter().any(|&i| self.route[i] > 0) {
                continue;
            }
            let centre = self.sites[k].centre;
            let cible: Vec<usize> = self.g.anneau(centre, 12).into_iter().filter(|&i| self.route[i] > 0).collect();
            let Some(but) = argmin(cible, |i| self.g.distance(i, centre)) else { continue };
            let (fuel, route) = (&self.fuel, &self.route);
            let cout = |_i: usize, j: usize| if b"ML".contains(&fuel[j]) { None } else { Some(if route[j] > 0 { 0.3 } else { 1.0 }) };
            if let Some((ch, _)) = astar_virages(&self.g, centre, but, cout, 0.3) {
                self.ajouter_route(ch);
            }
        }
        let aero = &gd.aerodrome;
        self.moyens_fixes(&aero.0, aero.1);
    }

    /// Pistes DFCI : un quadrillage dans les massifs, qui sert de coupure et d'accès aux camions.
    fn pistes_dfci(&mut self) {
        let (cols, rows) = (self.g.cols, self.g.rows);
        let (pas_c, pas_r) = (6, 5);
        let ok = |c: &Carte, i: usize| b"PJFQG".contains(&c.fuel[i]) && c.site_de[i] < 0;
        let mut lignes: Vec<Vec<usize>> = Vec::new();
        for r0 in (3..rows.saturating_sub(1)).step_by(pas_r) {
            lignes.push((0..cols).map(|col| r0 * cols + col).collect());
        }
        for c0 in (4..cols.saturating_sub(1)).step_by(pas_c) {
            lignes.push((0..rows).map(|row| row * cols + c0).collect());
        }
        for ligne in lignes {
            let mut run: Vec<usize> = Vec::new();
            for i in ligne {
                if ok(self, i) || (!run.is_empty() && self.route[i] > 0) {
                    run.push(i);
                } else {
                    if run.len() > 3 {
                        self.poser_piste(std::mem::take(&mut run));
                    }
                    run.clear();
                }
            }
            if run.len() > 3 {
                self.poser_piste(run);
            }
        }
    }

    fn poser_piste(&mut self, run: Vec<usize>) {
        for &i in &run {
            if self.route[i] == 0 {
                self.route[i] = 1;
            }
        }
        self.routes_poly.push(Voie { n: 1, c: run, p: Vec::new() });
    }

    // ------------------------------------------------------------------ casernes, hélistation, aérodrome, vigies, points d'eau
    fn moyens_fixes(&mut self, nom_aero: &str, xy: (f64, f64)) {
        let villes: Vec<usize> = self.sites.iter().filter(|s| s.type_ == "ville").map(|s| s.centre).collect();
        // aérodrome : à sa vraie place, relié à la préfecture
        let mut a = self.cellule(xy);
        if let Some(a0) = a {
            if self.site_de[a0] >= 0 {
                let cand: Vec<usize> = self.g.anneau(a0, 3).into_iter().filter(|&j| self.site_de[j] < 0 && !est_eau(self.fuel[j])).collect();
                a = Some(argmin(cand, |j| self.g.distance(a0, j)).unwrap_or(a0));
            }
        }
        if let Some(a) = a {
            let reference = villes.first().copied().unwrap_or(a);
            self.aerodrome = Some(a);
            for j in self.g.anneau(a, 1) {
                if self.site_de[j] < 0 && !est_eau(self.fuel[j]) {
                    self.fuel[j] = b'A';
                }
            }
            let (fuel, route) = (&self.fuel, &self.route);
            let cout = |_i: usize, j: usize| if b"ML".contains(&fuel[j]) { None } else { Some(if route[j] >= 2 { 0.3 } else { 1.0 }) };
            if let Some((ch, _)) = astar_virages(&self.g, a, reference, cout, 0.3) {
                self.ajouter_route(ch);
            }
            self.nom_aerodrome = nom_aero.to_string();
        }
        // hélistation DFCI : sur une piste, au centre du massif forestier
        let g = &self.g;
        let c: Vec<usize> = (0..g.n).filter(|&i| self.route[i] >= 1 && self.site_de[i] < 0 && !est_eau(self.fuel[i])).collect();
        let pins: Vec<usize> = (0..g.n).filter(|&i| self.fuel[i] == b'P').collect();
        if !c.is_empty() && !pins.is_empty() {
            let mx = pins.iter().map(|&i| g.cx[i]).sum::<f64>() / pins.len() as f64;
            let my = pins.iter().map(|&i| g.cy[i]).sum::<f64>() / pins.len() as f64;
            if let Some(mid) = g.sous(mx, my) {
                self.helistation = argmin(c, |i| g.distance(i, mid));
            }
        }
        if let Some(h) = self.helistation {
            self.fuel[h] = b'X';
        }
        // vigies : sur des points hauts, bien réparties
        let foret: Vec<usize> = (0..self.g.n).filter(|&i| b"PJFQG".contains(&self.fuel[i])).collect();
        for _ in 0..(foret.len() / 700).max(4) {
            let mut best: Option<(f64, usize)> = None;
            for i in self.rnd.sample(&foret, foret.len().min(300)) {
                let d = self.vigies.iter().map(|&v| self.g.distance(i, v)).min().unwrap_or(99).min(99);
                let sc = self.alt[i] / 50.0 + d.min(25) as f64;
                if d >= 12 && best.map_or(true, |b| sc > b.0) {
                    best = Some((sc, i));
                }
            }
            if let Some((_, i)) = best {
                self.vigies.push(i);
            }
        }
        // points d'eau : bornes dans les lieux habités, citernes DFCI aux croisements de pistes
        for s in &self.sites {
            if matches!(s.type_.as_str(), "ville" | "village" | "camping" | "usine") {
                self.points_eau.push(s.centre);
            }
        }
        let mut croix: Vec<usize> =
            (0..self.g.n).filter(|&i| self.route[i] == 1 && self.g.vois[i].iter().filter(|v| self.route[v.j] > 0).count() >= 3).collect();
        self.rnd.shuffle(&mut croix);
        let seuil = 2.max(rond(6.0 * self.g.ech));
        for i in croix {
            if self.points_eau.iter().all(|&p| self.g.distance(i, p) >= seuil) {
                self.points_eau.push(i);
            }
        }
        if let Some(h) = self.helistation {
            self.points_eau.push(h);
        }
    }

    /// Nettoie le réseau pour le dessin : chaque tronçon n'est tracé qu'une fois (au niveau le plus élevé), puis
    /// les escaliers de cellules deviennent des lignes droites et des courbes (sans couper par l'eau).
    fn finaliser_routes(&mut self) {
        let mut faits: HashSet<(usize, usize)> = HashSet::new();
        let mut propres: Vec<Voie> = Vec::new();
        let mut tries: Vec<&Voie> = self.routes_poly.iter().collect();
        tries.sort_by_key(|r| std::cmp::Reverse(r.n));
        for r in tries {
            let mut run: Vec<usize> = Vec::new();
            for w in r.c.windows(2) {
                let (a, b) = (w[0], w[1]);
                let e = (a.min(b), a.max(b));
                if faits.contains(&e) || (r.n == 1 && self.route[a] >= 2 && self.route[b] >= 2) {
                    if run.len() > 1 {
                        propres.push(Voie { n: r.n, c: std::mem::take(&mut run), p: Vec::new() });
                    }
                    run.clear();
                    continue;
                }
                faits.insert(e);
                if run.is_empty() {
                    run.push(a);
                }
                run.push(b);
            }
            if run.len() > 1 {
                propres.push(Voie { n: r.n, c: run, p: Vec::new() });
            }
        }
        let g = &self.g;
        let (fuel, route) = (&self.fuel, &self.route);
        let valide = |a: (f64, f64), b: (f64, f64)| {
            let n = (((b.0 - a.0).hypot(b.1 - a.1) / (g.hex_m / 3.0)) as usize).max(2);
            for k in 1..n {
                let t = k as f64 / n as f64;
                match g.sous(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) {
                    None => return false,
                    Some(i) if b"ML".contains(&fuel[i]) || (fuel[i] == b'R' && route[i] == 0) => return false,
                    _ => {}
                }
            }
            true
        };
        for r in &mut propres {
            let pts: Vec<(f64, f64)> = r.c.iter().map(|&i| (g.cx[i], g.cy[i])).collect();
            let tol = g.hex_m * if r.n == 1 { 0.42 } else { 0.36 };
            r.p = simplifier(&pts, tol, &valide).into_iter().map(|(x, y)| [rond(x), rond(y)]).collect();
        }
        self.routes_poly = propres;
    }

    /// Repère les plans d'eau (pour l'écopage des Canadair : il en faut un grand).
    fn plans_eau(&mut self) {
        let mut noms: Vec<Option<String>> = Vec::new();
        for i in 0..self.g.n {
            if b"ML".contains(&self.fuel[i]) && self.corps[i] < 0 {
                let cid = self.corps_taille.len() as i64;
                let mut pile = vec![i];
                let mut n = 0;
                self.corps[i] = cid;
                while let Some(k) = pile.pop() {
                    n += 1;
                    for v in &self.g.vois[k] {
                        if self.fuel[v.j] == self.fuel[i] && self.corps[v.j] < 0 {
                            self.corps[v.j] = cid;
                            pile.push(v.j);
                        }
                    }
                }
                self.corps_taille.push(n);
                noms.push(if self.fuel[i] == b'M' { Some("Océan".to_string()) } else { None });
            }
        }
        // les noms : le plan d'eau le plus proche du centre de chaque étang de web_conquete
        for e in &gironde().eaux {
            let Some(c) = self.g.sous(e.xy.0 * 1000.0, e.xy.1 * 1000.0) else { continue };
            let eau: Vec<usize> = self.g.anneau(c, 6).into_iter().filter(|&j| self.fuel[j] == b'L').collect();
            if let Some(j) = argmin(eau, |j| self.g.distance(c, j)) {
                let cid = self.corps[j] as usize;
                if noms[cid].is_none() {
                    noms[cid] = Some(e.nom.clone());
                }
            }
        }
        self.corps_nom = noms
            .into_iter()
            .zip(&self.corps_taille)
            .map(|(nom, &n)| nom.unwrap_or_else(|| if n >= 12 { "Étang" } else { "Mare" }.to_string()))
            .collect();
    }

    /// Un Canadair peut écoper ici : la mer, ou un plan d'eau d'au moins 12 hexagones.
    pub fn ecopable(&self, i: usize) -> bool {
        b"ML".contains(&self.fuel[i]) && self.corps[i] >= 0 && self.corps_taille[self.corps[i] as usize] >= 12
    }

    /// Un hélicoptère peut puiser ici : n'importe quelle eau.
    pub fn pompable(&self, i: usize) -> bool {
        est_eau(self.fuel[i])
    }

    // ------------------------------------------------------------------ export
    pub fn export(&self) -> Value {
        let g = &self.g;
        let routes: Vec<Value> = self.routes_poly.iter().map(|r| json!({"n": r.n, "c": r.c, "p": r.p})).collect();
        let sites: Vec<Value> = self
            .sites
            .iter()
            .map(|s| json!({"id": s.id, "type": s.type_, "nom": s.nom, "centre": s.centre, "cells": s.cells, "valeur": s.valeur, "pop": s.pop}))
            .collect();
        let casernes: Vec<Value> = self.casernes.iter().map(|c| json!({"id": c.id, "nom": c.nom, "cell": c.cell})).collect();
        let lacs: Vec<Value> =
            self.corps_taille.iter().enumerate().map(|(cid, &n)| json!({"id": cid, "nom": self.corps_nom[cid], "taille": n})).collect();
        json!({
            "graine": self.graine, "theme": self.theme, "cols": g.cols, "rows": g.rows, "hexM": rond(g.hex_m),
            "largeur": rond(g.largeur), "hauteur": rond(g.hauteur),
            "fuel": String::from_utf8_lossy(&self.fuel), "alt": self.alt.iter().map(|&a| rond(a)).collect::<Vec<_>>(),
            "route": self.route.iter().map(|r| char::from(b'0' + r)).collect::<String>(),
            "routes": routes, "riviere": [], "nomRiviere": "",
            "sites": sites, "casernes": casernes, "vigies": self.vigies, "pointsEau": self.points_eau,
            "helistation": self.helistation, "aerodrome": self.aerodrome, "nomAerodrome": self.nom_aerodrome,
            "lacs": lacs, "corps": self.corps,
            "comb": combustibles::export(), "voies": combustibles::voies(),
        })
    }
}
