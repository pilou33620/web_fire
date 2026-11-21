//! Les populations : évacuation par la route ou confinement (mise à l'abri sur place).
//!
//! - La fumée est calculée chaque minute sous le vent des flammes ; elle ralentit la circulation.
//! - Une route est coupée quand elle brûle ou longe un feu intense : les convois cherchent un autre chemin.
//! - Les convois (300 personnes, une centaine de voitures) partent toutes les 3 minutes du site évacué et roulent vers
//!   un point de regroupement sûr (une ville hors de danger, ou une sortie de la carte). Plusieurs convois sur le même
//!   tronçon : embouteillage.
//! - Un convoi bloqué près des flammes, ou des habitants confinés dans un site qui brûle presque entièrement,
//!   comptent comme « personnes exposées ». La doctrine : évacuer tôt, confiner quand il est trop tard pour partir.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use crate::carte::Carte;
use crate::feu::{BRULE, BRULE_FINI};
use crate::hexa::astar;
use crate::ia::Feux;
use crate::partie::Partie;
use crate::rond;

const TAILLE_CONVOI: i64 = 300;
/// minutes entre deux départs de convoi
const CADENCE: i64 = 3;

#[derive(Clone, Debug)]
pub struct Convoi {
    pub id: usize,
    pub site: usize,
    pub n: i64,
    pub cell: usize,
    pub x: f64,
    pub y: f64,
    pub chemin: Vec<usize>,
    pub avance: f64,
    /// route, bloque, arrive
    pub etat: &'static str,
    pub dest: Option<usize>,
    pub cap: f64,
    pub expose: bool,
    /// demi-tour vers son village : on s'y mettra à l'abri
    pub repli: bool,
    pub signale: bool,
}

#[derive(Clone, Debug)]
pub struct SiteEvac {
    /// normal, evacuation, evacue, confinement
    pub statut: &'static str,
    pub pop: i64,
    pub reste: i64,
    pub route: i64,
    pub arrives: i64,
    pub confines: i64,
    pub exposes: i64,
    pub t: i64,
    pub par: Option<&'static str>,
    pub dest: Option<usize>,
}

pub struct Bilan {
    pub evacues: i64,
    pub confines: i64,
    pub exposes: i64,
    pub route: i64,
}

#[derive(Default)]
pub struct Evacuation {
    pub sites: IndexMap<usize, SiteEvac>,
    pub convois: Vec<Convoi>,
    pub n: usize,
    pub fumee: IndexMap<usize, f64>,
    pub coupees: HashSet<usize>,
    /// cellule -> minutes avant l'arrivée prévue du feu (tous feux)
    pub menace: HashMap<usize, f64>,
    pub sorties: Vec<usize>,
}

impl Evacuation {
    pub fn new(c: &Carte) -> Evacuation {
        let mut sites = IndexMap::new();
        for s in &c.sites {
            if s.pop > 0 && s.type_ != "reserve" {
                sites.insert(
                    s.id,
                    SiteEvac { statut: "normal", pop: s.pop, reste: s.pop, route: 0, arrives: 0, confines: 0, exposes: 0, t: 0, par: None, dest: None },
                );
            }
        }
        let g = &c.g;
        // sorties de carte : les routes qui touchent un bord
        let mut sorties: Vec<usize> = (0..g.n)
            .filter(|&i| {
                let (col, row) = g.col_row(i);
                c.route[i] >= 2 && (col == 0 || col == g.cols - 1 || row == 0 || row == g.rows - 1)
            })
            .collect();
        if sorties.is_empty() {
            sorties.extend(c.aerodrome);
        }
        Evacuation { sites, sorties, ..Default::default() }
    }

    // ------------------------------------------------------------------ décisions
    pub fn evacuer(&mut self, p: &mut Partie, feux: &Feux, sid: usize, par: &'static str) -> bool {
        match self.sites.get(&sid) {
            Some(s) if s.statut != "evacuation" && s.statut != "evacue" => {}
            _ => return false,
        }
        let Some(dest) = self.destination(p, feux, sid) else { return false };
        let s = self.sites.get_mut(&sid).expect("site");
        // des habitants déjà confinés peuvent partir
        s.reste += s.confines;
        s.confines = 0;
        s.statut = "evacuation";
        s.t = p.t;
        s.par = Some(par);
        s.dest = Some(dest);
        let (reste, nom) = (s.reste, p.carte.sites[sid].nom.clone());
        let suite = if par == "joueur" { " — ordre du COS (vous)." } else { "." };
        let txt = format!("Évacuation de {nom} ({reste} personnes) vers {}{suite}", nom_dest(&p.carte, dest));
        p.journal_ajout("cos", txt);
        p.evacues.insert(sid);
        true
    }

    pub fn confiner(&mut self, p: &mut Partie, sid: usize, par: &'static str) -> bool {
        let Some(s) = self.sites.get_mut(&sid) else { return false };
        if s.statut == "confinement" || s.statut == "evacue" {
            return false;
        }
        s.confines += s.reste;
        s.reste = 0;
        s.statut = "confinement";
        s.t = p.t;
        s.par = Some(par);
        let suite = if par == "joueur" { " — ordre du COS (vous)." } else { " (trop tard pour partir par la route)." };
        let txt = format!("Confinement à {} : les {} personnes restent à l'abri dans les bâtiments{suite}", p.carte.sites[sid].nom, s.confines);
        p.journal_ajout("cos", txt);
        true
    }

    /// Minutes pour vider le site : départs échelonnés + trajet vers le point de regroupement.
    pub fn temps_evacuation(&mut self, p: &Partie, feux: &Feux, sid: usize) -> f64 {
        let Some(reste) = self.sites.get(&sid).map(|s| s.reste) else { return 1e9 };
        let Some(dest) = self.destination(p, feux, sid) else { return 1e9 };
        let g = &p.carte.g;
        let t = astar(g, p.carte.sites[sid].centre, dest, self.cout(&p.carte), 0.6 / g.ech).map_or(f64::INFINITY, |(_, t)| t);
        ((reste as f64 / TAILLE_CONVOI as f64).ceil() - 1.0) * CADENCE as f64 + t.min(1e9) * 0.5
    }

    // ------------------------------------------------------------------ routes
    fn vitesse(&self, c: &Carte, i: usize) -> f64 {
        let v = [0.35, 0.6, 1.0, 1.5][c.route[i] as usize];
        let f = self.fumee.get(&i).copied().unwrap_or(0.0);
        v * (1.0 - f).max(0.25) * c.g.ech
    }

    fn cout<'a>(&'a self, c: &'a Carte) -> impl Fn(usize, usize) -> Option<f64> + 'a {
        move |_i, j| {
            if self.coupees.contains(&j) {
                return None;
            }
            let t = self.menace.get(&j).copied().unwrap_or(1e9);
            if t < 40.0 {
                return None; // on n'envoie pas un convoi là où le feu sera dans moins de 40 min
            }
            let pen = if t < 120.0 { 4.0 } else { 0.0 };
            if c.route[j] < 2 && c.site_de[j] < 0 {
                return None; // pas de convoi de civils sur les pistes forestières
            }
            if b"ML".contains(&c.fuel[j]) || (c.fuel[j] == b'R' && c.route[j] == 0) {
                return None;
            }
            Some(1.0 / self.vitesse(c, j) + pen)
        }
    }

    /// Un point de regroupement sûr : une ville hors de la prévision du feu, sinon une sortie de la carte.
    fn destination(&mut self, p: &Partie, feux: &Feux, sid: usize) -> Option<usize> {
        let (c, g) = (&p.carte, &p.carte.g);
        let centre = c.sites[sid].centre;
        self.calcul_menace(feux);
        let menace = |i: &usize| self.menace.get(i).copied().unwrap_or(1e9);
        let mut cands: Vec<usize> = c
            .sites
            .iter()
            .filter(|s| s.type_ == "ville" && s.id != sid && s.cells.iter().map(menace).fold(1e9, f64::min) > 240.0)
            .map(|s| s.centre)
            .collect();
        cands.extend(self.sorties.iter().copied().filter(|i| menace(i) > 240.0));
        cands.sort_by_key(|&i| g.distance(i, centre));
        cands.into_iter().take(4).find(|&d| astar(g, centre, d, self.cout(c), 0.6 / g.ech).is_some())
    }

    fn calcul_menace(&mut self, feux: &Feux) {
        let mut menace: HashMap<usize, f64> = HashMap::new();
        for f in feux.values() {
            if f.statut == "éteint" {
                continue;
            }
            for (&i, &t) in &f.prev {
                if t < menace.get(&i).copied().unwrap_or(1e9) {
                    menace.insert(i, t);
                }
            }
        }
        self.menace = menace;
    }

    // ------------------------------------------------------------------ chaque minute
    pub fn pas(&mut self, p: &mut Partie) {
        self.calcul_fumee(p);
        if p.t % 2 == 0 {
            self.calcul_menace(&p.ia.feux);
        }
        // routes coupées : en feu, ou le long d'un feu intense
        self.coupees.clear();
        {
            let (c, feu) = (&p.carte, &p.feu);
            for &i in &feu.brulant {
                if c.route[i] > 0 || c.site_de[i] >= 0 {
                    self.coupees.insert(i);
                }
                if feu.i[i] > 0.55 {
                    for v in &c.g.vois[i] {
                        if c.route[v.j] > 0 {
                            self.coupees.insert(v.j);
                        }
                    }
                }
            }
        }
        // départs des convois
        let sids: Vec<usize> = self.sites.keys().copied().collect();
        for sid in sids {
            let (centre, nom, ncells) = {
                let s = &p.carte.sites[sid];
                (s.centre, s.nom.clone(), s.cells.len())
            };
            let menace_centre = self.menace.get(&centre).copied().unwrap_or(1e9);
            let s = self.sites.get_mut(&sid).expect("site");
            if s.statut == "evacuation" && s.reste > 0 && (p.t - s.t).rem_euclid(CADENCE) == 0 {
                let n = TAILLE_CONVOI.min(s.reste);
                s.reste -= n;
                s.route += n;
                let g = &p.carte.g;
                self.convois.push(Convoi {
                    id: self.n,
                    site: sid,
                    n,
                    cell: centre,
                    x: g.cx[centre],
                    y: g.cy[centre],
                    chemin: Vec::new(),
                    avance: 0.0,
                    etat: "route",
                    dest: s.dest,
                    cap: 0.0,
                    expose: false,
                    repli: false,
                    signale: false,
                });
                self.n += 1;
            }
            if s.statut == "evacuation" && s.reste == 0 && s.route == 0 {
                if s.confines > 0 {
                    s.statut = "confinement";
                    p.journal_ajout("cos", format!("{nom} : {} personnes évacuées, {} revenues se mettre à l'abri.", s.arrives, s.confines));
                } else {
                    s.statut = "evacue";
                    if s.arrives > 0 {
                        p.journal_ajout("sauve", format!("{nom} est entièrement évacué ({} personnes en sécurité).", s.arrives));
                    }
                }
            }
            // évacuation en cours mais le feu arrive sur le village : ceux qui restent se mettent à l'abri
            if s.statut == "evacuation" && s.reste > 0 && menace_centre < 25.0 {
                s.confines += s.reste;
                p.journal_ajout("cos", format!("{nom} : le feu est trop près, les {} personnes restantes se mettent à l'abri sur place.", s.reste));
                s.reste = 0;
            }
            // confinés dans un site qui brûle presque entièrement : exposés
            if s.confines > 0 && s.statut == "confinement" {
                let brule = p.carte.sites[sid].cells.iter().filter(|&&i| matches!(p.feu.etat[i], BRULE | BRULE_FINI)).count();
                if brule as f64 / ncells.max(1) as f64 > 0.6 {
                    s.exposes += s.confines;
                    let confines = s.confines;
                    s.confines = 0;
                    p.ajouter_blesses(centre, 1.max(rond(confines as f64 * 0.03)), "civil", "grave", &format!("habitants de {nom}"));
                    p.journal_ajout("alerte", format!("⚠ {confines} personnes confinées à {nom} sont exposées : le site brûle."));
                }
            }
        }
        // circulation
        let mut par_cell: HashMap<usize, i64> = HashMap::new();
        for cv in &self.convois {
            if cv.etat == "route" {
                *par_cell.entry(cv.cell).or_insert(0) += 1;
            }
        }
        let mut convois = std::mem::take(&mut self.convois);
        for cv in convois.iter_mut() {
            if cv.etat == "arrive" {
                continue;
            }
            self.rouler(p, cv, &par_cell);
        }
        convois.retain(|cv| cv.etat != "arrive");
        self.convois = convois;
    }

    /// Une minute de route d'un convoi.
    fn rouler(&mut self, p: &mut Partie, cv: &mut Convoi, par_cell: &HashMap<usize, i64>) {
        let nom = p.carte.sites[cv.site].nom.clone();
        // danger : le feu sur le convoi
        let danger = {
            let feu = &p.feu;
            feu.etat[cv.cell] == BRULE || p.carte.g.vois[cv.cell].iter().any(|v| feu.etat[v.j] == BRULE && feu.i[v.j] > 0.6)
        };
        if danger && !cv.expose {
            cv.expose = true;
            if let Some(s) = self.sites.get_mut(&cv.site) {
                s.exposes += cv.n;
            }
            let quoi = format!("convoi de {nom}");
            p.ajouter_blesses(cv.cell, 1.max(rond(cv.n as f64 * 0.02)), "civil", "grave", &quoi);
            p.ajouter_blesses(cv.cell, rond(cv.n as f64 * 0.05), "civil", "leger", &quoi);
            p.journal_ajout("alerte", format!("⚠ Un convoi de {} personnes parti de {nom} est pris par le feu !", cv.n));
        }
        if cv.chemin.is_empty() || self.coupees.contains(&cv.chemin[0]) || (p.t + cv.id as i64) % 6 == 0 {
            if !cv.repli && cv.dest.map_or(true, |d| self.coupees.contains(&d) || self.menace.get(&d).copied().unwrap_or(1e9) < 60.0) {
                let feux = &p.ia.feux;
                cv.dest = self.destination(p, feux, cv.site).or(cv.dest);
            }
            let g = &p.carte.g;
            let mut ch = cv.dest.and_then(|d| astar(g, cv.cell, d, self.cout(&p.carte), 0.6 / g.ech)).map(|(ch, _)| ch);
            if ch.is_none() && !cv.repli {
                // plus d'itinéraire sûr : demi-tour vers le village, où l'on se mettra à l'abri
                let c = &p.carte;
                let origine = c.sites[cv.site].centre;
                let coupees = &self.coupees;
                let cout = |_i: usize, j: usize| if coupees.contains(&j) || (c.route[j] < 2 && c.site_de[j] < 0) { None } else { Some(1.0) };
                ch = astar(g, cv.cell, origine, cout, 0.6).map(|(ch, _)| ch);
                if ch.is_some() {
                    cv.repli = true;
                    cv.dest = Some(origine);
                    p.journal_ajout("alerte", format!("Convoi de {nom} : route coupée, demi-tour et mise à l'abri au village."));
                }
            }
            let Some(ch) = ch else {
                if cv.etat != "bloque" {
                    cv.etat = "bloque";
                    if !cv.signale {
                        cv.signale = true;
                        p.journal_ajout("alerte", format!("Convoi de {nom} bloqué : route coupée."));
                    }
                }
                return;
            };
            cv.chemin = ch[1..].to_vec();
            cv.avance = 0.0;
            cv.etat = "route";
        }
        if cv.chemin.is_empty() {
            self.arrivee(cv);
            return;
        }
        let g = &p.carte.g;
        let mut budget = 1.0;
        while budget > 0.0 && !cv.chemin.is_empty() {
            let j = cv.chemin[0];
            let v = self.vitesse(&p.carte, j) / (0.7 * par_cell.get(&j).copied().unwrap_or(0) as f64).max(1.0);
            let reste = (1.0 - cv.avance) / v;
            if reste <= budget {
                budget -= reste;
                cv.cell = j;
                cv.chemin.remove(0);
                cv.avance = 0.0;
            } else {
                cv.avance += budget * v;
                budget = 0.0;
            }
        }
        if let Some(&j) = cv.chemin.first() {
            let (x0, y0, x1, y1) = (g.cx[cv.cell], g.cy[cv.cell], g.cx[j], g.cy[j]);
            cv.x = x0 + (x1 - x0) * cv.avance;
            cv.y = y0 + (y1 - y0) * cv.avance;
            cv.cap = (y1 - y0).atan2(x1 - x0).to_degrees();
        } else {
            cv.x = g.cx[cv.cell];
            cv.y = g.cy[cv.cell];
            self.arrivee(cv);
        }
    }

    fn arrivee(&mut self, cv: &mut Convoi) {
        cv.etat = "arrive";
        let Some(s) = self.sites.get_mut(&cv.site) else { return };
        s.route -= cv.n;
        if cv.repli && !cv.expose {
            s.confines += cv.n;
        } else if !cv.expose {
            s.arrives += cv.n;
        }
    }

    /// Panache de fumée sous le vent de chaque hexagone en feu.
    fn calcul_fumee(&mut self, p: &Partie) {
        let (g, feu) = (&p.carte.g, &p.feu);
        let (vx, vy) = p.meteo.vers();
        let portee = 2.0 + p.meteo.vent / 12.0;
        let mut fum: IndexMap<usize, f64> = IndexMap::new();
        for &i in &feu.brulant {
            let ii = feu.i[i];
            if ii < 0.2 {
                continue;
            }
            for k in 1..=(portee as i64) {
                let kf = k as f64;
                for lat in [-1.0f64, 0.0, 1.0] {
                    if lat != 0.0 && k < 2 {
                        continue;
                    }
                    let x = g.cx[i] + (vx * kf - vy * lat * kf * 0.35) * g.hex_m;
                    let y = g.cy[i] + (vy * kf + vx * lat * kf * 0.35) * g.hex_m;
                    if let Some(j) = g.sous(x, y) {
                        let e = fum.entry(j).or_insert(0.0);
                        *e = (*e + ii * (1.0 - kf / (portee + 1.0)) * if lat != 0.0 { 0.6 } else { 1.0 }).min(1.0);
                    }
                }
            }
            fum.insert(i, 1.0);
        }
        self.fumee = fum;
    }

    // ------------------------------------------------------------------ bilan
    pub fn bilan(&self) -> Bilan {
        let mut b = Bilan { evacues: 0, confines: 0, exposes: 0, route: 0 };
        for s in self.sites.values() {
            b.evacues += s.arrives;
            b.confines += s.confines;
            b.exposes += s.exposes;
            b.route += s.route;
        }
        b
    }

    pub fn export(&self, c: &Carte) -> Value {
        let mut sites = Map::new();
        for (sid, s) in &self.sites {
            sites.insert(
                sid.to_string(),
                json!({"statut": s.statut, "pop": s.pop, "reste": s.reste, "route": s.route, "arrives": s.arrives,
                       "confines": s.confines, "exposes": s.exposes, "par": s.par}),
            );
        }
        let mut coupees: Vec<usize> = self.coupees.iter().copied().filter(|&i| c.route[i] > 0).collect();
        coupees.sort_unstable();
        json!({
            "convois": self.convois.iter().map(|cv| json!({"id": cv.id, "x": rond(cv.x), "y": rond(cv.y), "cap": rond(cv.cap), "n": cv.n, "e": cv.etat, "s": cv.site})).collect::<Vec<_>>(),
            "sites": sites,
            "coupees": coupees,
            "fumee": self.fumee.iter().filter(|e| *e.1 > 0.25 && c.route[*e.0] > 0).map(|(i, d)| json!([i, rond(d * 100.0)])).collect::<Vec<_>>(),
        })
    }
}

fn nom_dest(c: &Carte, d: usize) -> String {
    if c.site_de[d] >= 0 {
        return c.sites[c.site_de[d] as usize].nom.clone();
    }
    "la sortie du massif".into()
}
