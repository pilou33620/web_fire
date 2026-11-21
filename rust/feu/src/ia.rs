//! Le commandant des opérations de secours (COS) : il lit les feux, prévoit leur avancée et engage les moyens.
//!
//! Doctrine (inspirée de la doctrine française de lutte contre les feux de forêts) :
//! 1. attaque massive des feux naissants : tout feu détecté reçoit vite des moyens importants, terrestres et aériens ;
//! 2. sur un feu installé, priorité aux vies et aux biens : mise en défense des points sensibles menacés (villes, campings,
//!    usines) avant l'arrivée du front ;
//! 3. on ne combat pas la tête d'un feu violent au sol : les camions attaquent par les flancs, de l'arrière vers la tête,
//!    en s'appuyant sur le noir (zone déjà brûlée) ;
//! 4. les avions cassent la tête et traitent les sautes de feu ; le Dash pose des lignes de retardant devant la tête ;
//!    les bulldozers ouvrent un pare-feu là où le feu arrivera dans une heure ou plus ;
//! 5. quand le feu est fixé : surveillance et noyage des lisières pour empêcher les reprises ;
//! 6. par risque très sévère : pré-positionnement des camions dans les massifs et guet aérien armé (GAAr).

use std::collections::{BTreeSet, HashMap, HashSet};

use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use crate::carte::Carte;
use crate::combustibles::{brule, comb, ROUTE_FREIN};
use crate::feu::{Feu, BRULE, BRULE_FINI, ETEINT, INTACT, NOYE};
use crate::hexa::dijkstra;
use crate::moyens::{vitesse_sol, Mission, Moyen};
use crate::partie::Partie;
use crate::{argmax, argmin, arrondi, majuscule, rond, trier};

/// La fiche d'un feu, tenue par le commandement.
#[derive(Clone, Debug, Default)]
pub struct Fiche {
    pub id: usize,
    pub nom: String,
    pub t0: i64,
    pub cause: String,
    /// naissant, en progression, fixé, maîtrisé, éteint
    pub statut: &'static str,
    pub strategie: String,
    /// site -> minutes avant l'arrivée prévue du feu
    pub menaces: IndexMap<usize, i64>,
    pub menace_log: IndexSet<usize>,
    pub hist: Vec<(i64, f64)>,
    pub prio: f64,
    pub detecte: bool,
    pub t_detect: Option<i64>,
    pub max_cells: usize,
    pub ha: f64,
    pub sites_touches: BTreeSet<usize>,
    pub cell0: usize,
    pub cells: IndexSet<usize>,
    pub eteints: IndexSet<usize>,
    pub sautes: IndexSet<usize>,
    pub front: Vec<usize>,
    pub tete: Option<usize>,
    pub flancs: Vec<usize>,
    /// cellule -> minutes avant l'arrivée prévue de ce feu
    pub prev: IndexMap<usize, f64>,
    pub vitesse: f64,
    pub pc: bool,
    pub fusion: Vec<String>,
    pub absorbe: Option<usize>,
    pub t_ret: i64,
}

pub type Feux = IndexMap<usize, Fiche>;

/// Le foyer d'origine d'un feu, en suivant les feux qui se sont rejoints.
pub fn racine(alias: &HashMap<usize, usize>, mut f: usize) -> usize {
    while let Some(&a) = alias.get(&f) {
        f = a;
    }
    f
}

fn racine_de(alias: &HashMap<usize, usize>, m: &Option<Mission>) -> Option<usize> {
    m.as_ref().and_then(|m| m.feu).map(|f| racine(alias, f))
}

#[derive(Default)]
pub struct Commandement {
    /// equilibree, offensive (tout sur la tête), defensive (tout sur les biens)
    pub doctrine: String,
    /// racine du foyer -> fiche du feu
    pub feux: Feux,
    /// foyer -> racine (feux qui se sont rejoints)
    pub alias: HashMap<usize, usize>,
    pub renforts: usize,
    pub gaar: bool,
    pub prepos: bool,
    /// auto : le COS est l'IA ; manuel : le COS, c'est vous (l'IA conseille seulement)
    pub mode: &'static str,
    interdit_jour: Option<i64>,
}

fn accessible(p: &Partie, i: usize) -> bool {
    vitesse_sol("ccf", &p.carte, &p.feu, i) > 0.0 && p.feu.etat[i] != BRULE
}

/// Temps d'arrivée prévu du feu (minutes) sur chaque hexagone, au vent actuel.
fn prevision(p: &Partie, cells: &IndexSet<usize>, horizon: f64) -> IndexMap<usize, f64> {
    let (c, feu, m, g) = (&p.carte, &p.feu, &p.meteo, &p.carte.g);
    let (vx, vy) = m.vers();
    let v = m.vent;
    let fsech = m.f_secheresse();
    let cout = |i: usize, j: usize| {
        let cj = comb(c.fuel[j]);
        if !cj.brule || matches!(feu.etat[j], BRULE | BRULE_FINI) {
            return None;
        }
        let (ux, uy) = ((g.cx[j] - g.cx[i]) / g.hex_m, (g.cy[j] - g.cy[i]) / g.hex_m);
        let fw = (0.045 * v * (ux * vx + uy * vy)).exp();
        let fs = (3.0 * ((c.alt[j] - c.alt[i]) / g.hex_m).clamp(-0.3, 0.45)).exp();
        let mut r = cj.ros * fw * fs * m.f_humidite(feu.mouille[j]) * fsech * ROUTE_FREIN[c.route[j] as usize];
        r *= if feu.ret[j] > 0 { 0.12 } else { 1.0 };
        r *= 0.3f64.powi(feu.defense[j].min(3) as i32);
        r *= 0.85 * g.ech; // intensité de référence
        Some(1.0 / r.max(1e-4))
    };
    let sources: Vec<(usize, f64)> = cells.iter().map(|&i| (i, 0.0)).collect();
    dijkstra(g, &sources, cout, horizon)
}

impl Commandement {
    pub fn new(doctrine: &str) -> Commandement {
        Commandement { doctrine: doctrine.to_string(), mode: "auto", ..Default::default() }
    }

    pub fn racine(&self, f: usize) -> usize {
        racine(&self.alias, f)
    }

    // ------------------------------------------------------------------ foyers et feux
    /// Regroupe les hexagones en feu par foyer d'origine ; fusionne deux foyers dont les flammes se touchent.
    #[allow(clippy::type_complexity)]
    fn regrouper(&mut self, p: &mut Partie) -> (IndexMap<usize, IndexSet<usize>>, IndexMap<usize, IndexSet<usize>>) {
        let mut par: IndexMap<usize, IndexSet<usize>> = IndexMap::new();
        let brulant: Vec<usize> = p.feu.brulant.iter().copied().collect();
        for i in brulant {
            let mut r = racine(&self.alias, p.feu.foyer[i] as usize);
            for v in &p.carte.g.vois[i] {
                let j = v.j;
                if p.feu.etat[j] != BRULE {
                    continue;
                }
                let r2 = racine(&self.alias, p.feu.foyer[j] as usize);
                if r2 != r {
                    let (a, b) = (r.min(r2), r.max(r2));
                    self.alias.insert(b, a);
                    if self.feux.contains_key(&b) && self.feux.contains_key(&a) {
                        let nom_b = self.feux[&b].nom.clone();
                        let fa = self.feux.get_mut(&a).expect("feu");
                        fa.fusion.push(nom_b.clone());
                        let txt = format!("{nom_b} rejoint {}.", fa.nom);
                        p.reg.ajout(&p.meteo, "feu", txt);
                        let fb = self.feux.get_mut(&b).expect("feu");
                        fb.statut = "éteint";
                        fb.absorbe = Some(a);
                    }
                    r = a;
                }
            }
            par.entry(r).or_default().insert(i);
        }
        // les hexagones éteints non noyés restent dans leur feu (reprises possibles)
        let mut eteints: IndexMap<usize, IndexSet<usize>> = IndexMap::new();
        for &i in &p.feu.eteints {
            eteints.entry(racine(&self.alias, p.feu.foyer[i] as usize)).or_default().insert(i);
        }
        (par, eteints)
    }

    fn detecte(&self, feu: &Feu, r: usize) -> bool {
        feu.departs.iter().any(|d| d.detecte && racine(&self.alias, d.id) == r)
    }

    // ------------------------------------------------------------------ boucle
    /// ×1,2 sur l'efficacité des lances si le feu de cette cellule a un poste de commandement posé.
    pub fn bonus_pc(&self, feu: &Feu, cell: usize) -> f64 {
        if feu.foyer[cell] < 0 {
            return 1.0;
        }
        match self.feux.get(&racine(&self.alias, feu.foyer[cell] as usize)) {
            Some(f) if f.pc => 1.2,
            _ => 1.0,
        }
    }

    pub fn pas(&mut self, p: &mut Partie) {
        // un poste de commandement posé : le commandement suit les feux chaque minute au lieu de toutes les deux
        if p.t % 2 != 0 && !self.feux.values().any(|f| f.pc && f.statut != "éteint") {
            return;
        }
        let (par, eteints) = self.regrouper(p);
        self.fiches(p, par, eteints);
        self.liberer(p);
        let mut actifs: Vec<usize> = self.feux.values().filter(|f| f.statut != "éteint" && f.detecte).map(|f| f.id).collect();
        trier(&mut actifs, |id| -self.feux[id].prio);
        if self.mode == "manuel" {
            for id in actifs {
                self.alertes(p, id);
            }
            return;
        }
        for &id in &actifs {
            self.engager(p, id);
        }
        self.renforts_demande(p);
        self.prevention(p, !actifs.is_empty());
    }

    /// En mode manuel, l'IA ne commande plus : elle signale seulement les sites menacés.
    fn alertes(&mut self, p: &mut Partie, fid: usize) {
        let f = self.feux.get_mut(&fid).expect("feu");
        let menaces: Vec<(usize, i64)> = f.menaces.iter().map(|(&s, &e)| (s, e)).collect();
        for (sid, eta) in menaces {
            if eta <= 150 && f.menace_log.insert(sid) {
                p.reg.ajout(&p.meteo, "alerte", format!("{} menacé : arrivée du feu estimée dans {eta} min.", p.carte.sites[sid].nom));
            }
        }
        let st = match f.statut {
            "naissant" => "Feu naissant : à vous d'engager les moyens".to_string(),
            "en progression" => "Feu en progression : à vous de jouer".to_string(),
            s => majuscule(s),
        };
        self.strategie(p, fid, format!("{st} (COS : vous)"));
    }

    fn fiches(&mut self, p: &mut Partie, par: IndexMap<usize, IndexSet<usize>>, eteints: IndexMap<usize, IndexSet<usize>>) {
        let (vx, vy) = p.meteo.vers();
        let mut vus: HashSet<usize> = HashSet::new();
        let mut liste: Vec<(usize, IndexSet<usize>)> = Vec::new();
        for &r in eteints.keys() {
            if !par.contains_key(&r) {
                liste.push((r, IndexSet::new()));
            }
        }
        let mut tous: Vec<(usize, IndexSet<usize>)> = par.into_iter().collect();
        tous.extend(liste);
        for (r, cells) in tous {
            vus.insert(r);
            if !self.feux.contains_key(&r) {
                let dep = p.feu.departs[r].clone();
                let nom = self.nommer(&p.carte, dep.cell);
                self.feux.insert(
                    r,
                    Fiche { id: r, nom, t0: dep.t, cause: dep.cause, statut: "naissant", cell0: dep.cell, t_ret: -999, ..Default::default() },
                );
            }
            let detecte = self.detecte(&p.feu, r);
            let prev = if cells.is_empty() { IndexMap::new() } else { prevision(p, &cells, 360.0) };
            let pc = p.moyens.iter().any(|u| {
                u.kind == "vpc"
                    && u.mission.as_ref().is_some_and(|m| m.type_ == "pc" && m.pose >= 5)
                    && racine_de(&self.alias, &u.mission) == Some(r)
            });
            let ha = {
                let feu = &p.feu;
                let n = (0..p.carte.g.n).filter(|&i| feu.etat[i] != INTACT && racine(&self.alias, feu.foyer[i] as usize) == r).count();
                arrondi(n as f64 * feu.ha_hex, 1)
            };
            let doctrine_defensive = self.doctrine == "defensive";
            let f = self.feux.get_mut(&r).expect("feu");
            let (g, feu, c) = (&p.carte.g, &p.feu, &p.carte);
            if !f.detecte && detecte {
                f.detecte = true;
                f.t_detect = Some(p.t);
            }
            f.eteints = eteints.get(&r).cloned().unwrap_or_default();
            f.max_cells = f.max_cells.max(cells.len());
            f.ha = ha;
            // amas : le principal et les sautes
            let mut reste = cells.clone();
            let mut amas: Vec<IndexSet<usize>> = Vec::new();
            while let Some(a) = reste.pop() {
                let mut pile = vec![a];
                let mut grp = IndexSet::new();
                grp.insert(a);
                while let Some(k) = pile.pop() {
                    for v in &g.vois[k] {
                        if reste.shift_remove(&v.j) {
                            grp.insert(v.j);
                            pile.push(v.j);
                        }
                    }
                }
                amas.push(grp);
            }
            amas.sort_by(|a, b| b.len().cmp(&a.len()));
            f.sautes = amas.iter().skip(1).flatten().copied().collect();
            // front actif, tête, flancs
            let front: Vec<usize> =
                cells.iter().copied().filter(|&i| g.vois[i].iter().any(|v| feu.etat[v.j] == INTACT && brule(c.fuel[v.j]))).collect();
            let proj = |i: usize| g.cx[i] * vx + g.cy[i] * vy;
            if front.is_empty() {
                f.tete = None;
                f.flancs = Vec::new();
            } else {
                f.tete = argmax(front.iter().copied(), proj);
                let pmin = front.iter().map(|&i| proj(i)).fold(f64::INFINITY, f64::min);
                let pmax = front.iter().map(|&i| proj(i)).fold(f64::NEG_INFINITY, f64::max);
                f.flancs = front.iter().copied().filter(|&i| proj(i) < pmax - 0.35 * (pmax - pmin) - 1.0).collect();
            }
            // prévision et menaces
            let mut menaces = IndexMap::new();
            for s in &c.sites {
                if s.type_ == "reserve" {
                    continue;
                }
                let eta = s.cells.iter().map(|i| prev.get(i).copied().unwrap_or(1e9)).fold(1e9, f64::min);
                if eta < 360.0 {
                    menaces.insert(s.id, rond(eta));
                }
            }
            // vitesse de la tête (km/h), d'après la prévision autour de la tête
            let mut vit = 0.0;
            if let Some(h) = f.tete {
                let cand = g.vois[h].iter().filter_map(|v| prev.get(&v.j).copied()).filter(|&t| t > 0.0).fold(f64::INFINITY, f64::min);
                if cand.is_finite() {
                    vit = g.hex_m / cand * 60.0 / 1000.0;
                }
            }
            f.vitesse = arrondi(vit.min(12.0), 1);
            f.pc = pc;
            let dern = cells.iter().map(|&i| feu.t_ign[i]).max().unwrap_or(-999).max(-999);
            // statut
            let st = if cells.is_empty() && f.eteints.is_empty() {
                "éteint"
            } else if cells.is_empty() {
                "maîtrisé"
            } else if f.max_cells <= 4 && f.ha < 5.2 * feu.ha_hex {
                "naissant"
            } else if (p.t - dern) as f64 > 25.0 / g.ech || (f.vitesse < 0.25 && front.len() <= 3) {
                "fixé"
            } else {
                "en progression"
            };
            if st != f.statut {
                if f.detecte && matches!(st, "fixé" | "maîtrisé" | "éteint") {
                    p.reg.ajout(&p.meteo, "feu", format!("{} : {st} à {} ({:.0} ha).", f.nom, p.meteo.heure(), f.ha));
                }
                if st == "éteint" {
                    for sid in f.menace_log.iter() {
                        if !f.sites_touches.contains(sid) {
                            p.reg.ajout(&p.meteo, "sauve", format!("{} a été protégé.", c.sites[*sid].nom));
                        }
                    }
                }
                f.statut = st;
            }
            // sites touchés
            for s in &c.sites {
                let touche = s.cells.iter().any(|&i| matches!(feu.etat[i], BRULE | BRULE_FINI) && racine(&self.alias, feu.foyer[i] as usize) == r);
                if touche && f.sites_touches.insert(s.id) {
                    p.reg.ajout(&p.meteo, "alerte", format!("⚠ Le feu atteint {} !", s.nom));
                }
            }
            // priorité
            let mut pr = 0.0;
            if st == "naissant" {
                pr += if doctrine_defensive { 400.0 } else { 900.0 };
            }
            for (&sid, &eta) in &menaces {
                pr += c.sites[sid].valeur as f64 * 60.0 / (eta as f64 + 30.0);
            }
            pr += f.ha * 0.4 + f.sautes.len() as f64 * 30.0;
            f.prio = pr;
            f.menaces = menaces;
            f.front = front;
            f.prev = prev;
            f.cells = cells;
            f.hist.push((p.t, f.ha));
            if f.hist.len() > 90 {
                let n = f.hist.len() - 90;
                f.hist.drain(..n);
            }
        }
        for (r, f) in self.feux.iter_mut() {
            if vus.contains(r) || f.statut == "éteint" {
                continue;
            }
            f.cells.clear();
            f.eteints.clear();
            f.front.clear();
            f.flancs.clear();
            f.tete = None;
            f.statut = "éteint";
            if f.detecte {
                p.reg.ajout(&p.meteo, "feu", format!("{} : éteint à {} ({:.0} ha).", f.nom, p.meteo.heure(), f.ha));
                for sid in f.menace_log.iter() {
                    if !f.sites_touches.contains(sid) {
                        p.reg.ajout(&p.meteo, "sauve", format!("{} a été protégé.", p.carte.sites[*sid].nom));
                    }
                }
            }
        }
    }

    pub fn nommer(&self, c: &Carte, i: usize) -> String {
        let Some(s) = argmin(c.sites.iter(), |s| c.g.distance(i, s.centre)) else { return "Feu".into() };
        let base = if s.type_ == "ville" || s.type_ == "village" {
            let de = if s.nom.starts_with(['A', 'E', 'I', 'O', 'U', 'H', 'É']) { "d'" } else { "de " };
            format!("Feu {de}{}", s.nom)
        } else {
            format!("Feu près de {}", s.nom)
        };
        let n = self.feux.values().filter(|f| f.nom.starts_with(&base)).count();
        if n > 0 {
            format!("{base} ({})", n + 1)
        } else {
            base
        }
    }

    // ------------------------------------------------------------------ missions
    fn liberer(&mut self, p: &mut Partie) {
        for u in p.moyens.iter_mut() {
            if u.joueur {
                if let Some(m) = &u.mission {
                    if m.fini || m.bloque > 3 {
                        let pourquoi = if m.bloque > 3 { " (position inaccessible)" } else { "" };
                        p.reg.ajout(&p.meteo, "cos", format!("{} : ordre terminé{pourquoi}, retour à l'IA.", u.nom));
                        u.mission = None;
                        u.joueur = false;
                    }
                }
                continue;
            }
            let Some(m) = u.mission.as_mut() else { continue };
            let f = m.feu.and_then(|fe| self.feux.get(&racine(&self.alias, fe)));
            if m.feu.is_some() && f.map_or(true, |f| f.statut == "éteint") {
                u.mission = None;
                continue;
            }
            if let Some(f) = f {
                if m.feu != Some(f.id) {
                    m.feu = Some(f.id);
                }
            }
            let effacer = if m.fini || m.bloque > 3 || m.inactif > 4 {
                true
            } else if m.type_ == "surveillance" && f.is_some_and(|f| f.statut != "maîtrisé" && f.statut != "fixé") {
                true
            } else if m.type_ == "defense" && f.is_some_and(|f| m.site.map_or(true, |s| !f.menaces.contains_key(&s))) {
                true
            } else {
                (m.type_ == "preposition" || m.type_ == "gaar") && self.feux.values().any(|x| x.statut != "éteint" && x.detecte)
            };
            if effacer {
                u.mission = None;
            }
        }
    }

    /// Les moyens disponibles d'un type, du plus proche au plus loin ; un moyen engagé sur un feu bien moins
    /// prioritaire peut être réaffecté.
    fn libres(&self, p: &Partie, kind: &str, prio: f64, cell: usize) -> Vec<usize> {
        let g = &p.carte.g;
        let mut out: Vec<(i64, usize)> = Vec::new();
        for (k, u) in p.moyens.iter().enumerate() {
            if u.kind != kind || u.joueur || u.hs {
                continue;
            }
            if u.etat == "au_sol" && !p.meteo.vol_possible(kind) {
                continue;
            }
            match &u.mission {
                None => out.push((g.distance(u.cell, cell), k)),
                Some(m) if m.type_ == "preposition" || m.type_ == "gaar" => out.push((g.distance(u.cell, cell), k)),
                Some(m) => {
                    if let Some(f) = m.feu.and_then(|fe| self.feux.get(&racine(&self.alias, fe))) {
                        if f.prio * 3.0 < prio && m.type_ != "defense" {
                            out.push((g.distance(u.cell, cell) + 30, k));
                        }
                    }
                }
            }
        }
        out.sort_by_key(|t| t.0);
        out.into_iter().map(|(_, k)| k).collect()
    }

    fn engages(&self, p: &Partie, fid: usize, kind: &str, typ: Option<&str>) -> Vec<usize> {
        (0..p.moyens.len())
            .filter(|&k| {
                let u = &p.moyens[k];
                u.kind == kind
                    && !u.joueur
                    && racine_de(&self.alias, &u.mission) == Some(fid)
                    && typ.map_or(true, |t| u.mission.as_ref().is_some_and(|m| m.type_ == t))
            })
            .collect()
    }

    fn strategie(&mut self, p: &mut Partie, fid: usize, texte: String) {
        let f = self.feux.get_mut(&fid).expect("feu");
        if texte != f.strategie {
            p.reg.ajout(&p.meteo, "cos", format!("{} — {texte}", f.nom));
            f.strategie = texte;
        }
    }

    fn engager(&mut self, p: &mut Partie, fid: usize) {
        let (tete, st, prio, ha, nfront, nsautes, vitesse, nom, ncells, menaces, centre) = {
            let f = &self.feux[&fid];
            let centre = f.tete.or_else(|| f.cells.first().copied()).unwrap_or(f.cell0);
            let mut menaces: Vec<(usize, i64)> = f.menaces.iter().map(|(&s, &e)| (s, e)).collect();
            menaces.sort_by_key(|t| t.1);
            (f.tete, f.statut, f.prio, f.ha, f.front.len(), f.sautes.len(), f.vitesse, f.nom.clone(), f.cells.len(), menaces, centre)
        };
        let vol = p.meteo.vol_possible("canadair");
        let mut parts: Vec<&str> = Vec::new();
        let nouvelle = |type_: &str, poste: Option<usize>, label: String| Mission { type_: type_.into(), feu: Some(fid), poste, label, ..Default::default() };
        // --- surveillance des lisières
        if st == "maîtrisé" || (st == "fixé" && ncells <= 2) {
            self.strategie(p, fid, "Surveillance et noyage des lisières (prévenir les reprises)".into());
            for k in self.engages(p, fid, "ccf", None) {
                if p.moyens[k].mission.as_ref().is_some_and(|m| m.type_ != "surveillance") {
                    p.moyens[k].mission = None;
                }
            }
            let mut zone: Vec<usize> = {
                let f = &self.feux[&fid];
                f.eteints.iter().copied().chain(f.cells.iter().copied().filter(|i| !f.eteints.contains(i))).collect()
            };
            let g = &p.carte.g;
            trier(&mut zone, |&i| g.distance(i, centre));
            let postes = postes_bord(p, &zone, 2);
            let deja = self.engages(p, fid, "ccf", Some("surveillance")).len();
            let libres = self.libres(p, "ccf", prio, centre);
            for (k, poste) in libres.into_iter().take(2usize.saturating_sub(deja)).zip(postes) {
                p.moyens[k].mission = Some(nouvelle("surveillance", Some(poste), "Surveillance des lisières".into()));
            }
            for kind in ["helico", "canadair", "dash", "bull"] {
                for k in self.engages(p, fid, kind, None) {
                    p.moyens[k].mission = None;
                }
            }
            return;
        }
        // --- défense des points sensibles
        let mut n_def = 0.0;
        for &(sid, eta) in &menaces {
            if eta > 150 {
                continue;
            }
            let (snom, stype, scentre) = {
                let s = &p.carte.sites[sid];
                (s.nom.clone(), s.type_.clone(), s.centre)
            };
            if self.feux.get_mut(&fid).expect("feu").menace_log.insert(sid) {
                p.journal_ajout("alerte", format!("{snom} menacé : arrivée du feu estimée dans {eta} min."));
            }
            self.population(p, sid, eta, fid);
            let mut besoin: f64 = match stype.as_str() {
                "ville" => 4.0,
                "village" => 2.0,
                "camping" | "usine" => 3.0,
                "patrimoine" => 1.0,
                _ => 2.0,
            };
            if eta < 60 {
                besoin += 1.0;
            }
            if self.doctrine == "offensive" {
                besoin = (besoin - 1.0).max(1.0);
            } else if self.doctrine == "defensive" {
                besoin += 1.0;
            }
            let postes = self.postes_defense(p, &self.feux[&fid], sid);
            // les fourgons pompe-tonnes d'abord (spécialistes des maisons, un FPT compte pour 1,5), puis les CCF
            for (kind, poids) in [("fpt", 1.5), ("ccf", 1.0)] {
                let en_place: Vec<usize> =
                    self.engages(p, fid, kind, Some("defense")).into_iter().filter(|&k| p.moyens[k].mission.as_ref().is_some_and(|m| m.site == Some(sid))).collect();
                for (n, &k) in en_place.iter().enumerate() {
                    let m = p.moyens[k].mission.as_mut().expect("mission");
                    if !postes.is_empty() && !m.poste.is_some_and(|q| postes.contains(&q)) {
                        m.poste = Some(postes[n % postes.len()]);
                    }
                }
                besoin -= poids * en_place.len() as f64;
                if besoin <= 0.0 || postes.is_empty() {
                    continue;
                }
                let n = (besoin / poids).ceil() as usize;
                let mut cand = self.libres(p, kind, prio + 200.0, scentre);
                if kind == "fpt" {
                    cand.retain(|&k| p.moyens[k].chemin_vers(p, postes[0], None).is_some());
                }
                for (j, &k) in cand.iter().take(n).enumerate() {
                    let m = Mission { site: Some(sid), ..nouvelle("defense", Some(postes[(en_place.len() + j) % postes.len()]), format!("Défense de {snom}")) };
                    p.moyens[k].mission = Some(m);
                    besoin -= poids;
                }
            }
            n_def += besoin;
        }
        if n_def != 0.0 {
            parts.push("défense des points sensibles");
        }
        // --- poste de commandement : posé en retrait sur une route, dès que le feu dépasse 20 ha
        let naissant = st == "naissant";
        if !naissant && ha > 20.0 && self.engages(p, fid, "vpc", None).is_empty() {
            if let Some(poste) = poste_pc(p, &self.feux[&fid]) {
                if let Some(&k) = self.libres(p, "vpc", prio, centre).first() {
                    p.moyens[k].mission = Some(nouvelle("pc", Some(poste), format!("Poste de commandement de {nom}")));
                    let txt = format!("{} part installer le poste de commandement de {nom}.", p.moyens[k].nom);
                    p.journal_ajout("cos", txt);
                }
            }
        }
        // --- attaque au sol : par les flancs, de l'arrière vers la tête (feu naissant : directement)
        let mut nb_att = if naissant { 4 } else { (2 + nfront / 2).min(10) };
        if self.doctrine == "defensive" && !naissant {
            nb_att = (nb_att / 2).max(2);
        }
        let att = self.engages(p, fid, "ccf", Some("attaque"));
        let postes = postes_attaque(p, &self.feux[&fid], naissant);
        let mut occupes: HashSet<usize> = HashSet::new();
        for &k in &att {
            let cell = p.moyens[k].cell;
            let m = p.moyens[k].mission.as_mut().expect("mission");
            let ok = m.poste.is_some_and(|q| postes.contains(&q) && !occupes.contains(&q));
            if !ok && !postes.is_empty() {
                let libres = postes.iter().copied().filter(|q| !occupes.contains(q));
                if let Some(q) = argmin(libres, |q| p.carte.g.distance(q, cell)) {
                    m.poste = Some(q);
                    m.inactif = 0;
                }
            }
            occupes.extend(m.poste);
        }
        if nb_att > att.len() {
            let manque = nb_att - att.len();
            let mut libres_postes: Vec<usize> = postes.iter().copied().filter(|q| !occupes.contains(q)).collect();
            for k in self.libres(p, "ccf", prio, centre).into_iter().take(manque) {
                let cell = p.moyens[k].cell;
                let Some(q) = argmin(libres_postes.iter().copied(), |q| p.carte.g.distance(q, cell)) else { break };
                libres_postes.retain(|&x| x != q);
                let label = if naissant { "Attaque du feu naissant" } else { "Attaque de flanc" };
                p.moyens[k].mission = Some(nouvelle("attaque", Some(q), label.into()));
            }
        }
        parts.push(if naissant { "attaque des feux naissants" } else { "attaque par les flancs" });
        // --- moyens aériens
        if vol {
            let mut nb_can = if naissant { 2 } else { (2 + (ha / 40.0) as usize + menaces.len()).min(8) };
            let nb_hel = if naissant { 2 } else { (1 + (ha / 60.0) as usize + nsautes).min(6) };
            if self.doctrine == "defensive" {
                nb_can = nb_can.saturating_sub(1).max(1);
            }
            for (kind, nb) in [("canadair", nb_can), ("helico", nb_hel)] {
                let deja = self.engages(p, fid, kind, None).len();
                for k in self.libres(p, kind, prio, centre).into_iter().take(nb.saturating_sub(deja)) {
                    let u = &mut p.moyens[k];
                    u.mission = Some(nouvelle("largage", None, "Largages".into()));
                    u.phase = None;
                    u.vise = None;
                    u.cible = None;
                }
            }
            if !naissant {
                parts.push(if nsautes == 0 { "largages sur la tête" } else { "largages sur la tête et les sautes" });
            }
            // Dash : ligne de retardant devant une tête rapide
            if !naissant && vitesse >= 0.6 && p.meteo.vent >= 18.0 && tete.is_some() {
                if self.engages(p, fid, "dash", None).is_empty() && p.t - self.feux[&fid].t_ret > 25 {
                    if let Some(&k) = self.libres(p, "dash", prio, centre).first() {
                        p.moyens[k].mission = Some(nouvelle("retardant", None, "Ligne de retardant".into()));
                        p.moyens[k].vise = None;
                        self.feux.get_mut(&fid).expect("feu").t_ret = p.t;
                        let txt = format!("{} décolle pour poser une ligne de retardant devant {nom}.", p.moyens[k].nom);
                        p.journal_ajout("cos", txt);
                    }
                }
                parts.push("ligne de retardant");
            }
        }
        // --- bulldozers : pare-feu là où le feu arrivera dans 60 à 150 minutes
        if !naissant && tete.is_some() && ha > 20.0 && self.engages(p, fid, "bull", None).is_empty() {
            let ligne = ligne_devant(p, &self.feux[&fid], 60.0, 150.0, 7);
            if ligne.len() >= 3 {
                if let Some(&k) = self.libres(p, "bull", prio, ligne[0]).first() {
                    let txt = format!("{} ouvre un pare-feu de {} m devant {nom}.", p.moyens[k].nom, ligne.len() * 300);
                    p.moyens[k].mission = Some(Mission { cells: ligne, ..nouvelle("pare_feu", None, "Ouverture d'un pare-feu".into()) });
                    p.journal_ajout("cos", txt);
                }
                parts.push("pare-feu au bulldozer");
            }
        }
        let mut txt = if naissant {
            "Attaque massive du feu naissant".to_string()
        } else {
            let mut uniques: Vec<&str> = Vec::new();
            for x in parts {
                if !uniques.contains(&x) {
                    uniques.push(x);
                }
            }
            majuscule(&uniques.join(" + "))
        };
        if !vol {
            txt += &format!(" (moyens aériens au sol : {})", if p.meteo.nuit { "nuit" } else { "vent trop fort" });
        }
        self.strategie(p, fid, txt);
    }

    /// Évacuer tant qu'il est temps de partir par la route ; sinon, confiner (mise à l'abri sur place).
    /// On n'évacue pas pour un feu naissant (l'attaque massive doit suffire) ; une ville ne s'évacue qu'en dernier recours.
    fn population(&mut self, p: &mut Partie, sid: usize, eta: i64, fid: usize) {
        let stype = p.carte.sites[sid].type_.clone();
        if p.evac.sites.get(&sid).map_or(true, |e| e.statut != "normal") || !matches!(stype.as_str(), "ville" | "village" | "camping" | "usine") {
            return;
        }
        if self.feux[&fid].statut == "naissant" || eta > if stype == "ville" { 100 } else { 150 } {
            return;
        }
        let mut ev = std::mem::take(&mut p.evac);
        if ev.temps_evacuation(p, &self.feux, sid) + 15.0 < eta as f64 {
            ev.evacuer(p, &self.feux, sid, "COS");
        } else if eta < 75 {
            ev.confiner(p, sid, "COS");
        }
        p.evac = ev;
    }

    // ------------------------------------------------------------------ choix des postes
    /// Les hexagones du site les plus tôt atteints, côté feu, pas en feu.
    fn postes_defense(&self, p: &Partie, f: &Fiche, sid: usize) -> Vec<usize> {
        let s = &p.carte.sites[sid];
        let mut cells: Vec<usize> = s.cells.iter().copied().filter(|&i| p.feu.etat[i] != BRULE && accessible(p, i)).collect();
        trier(&mut cells, |i| f.prev.get(i).copied().unwrap_or(1e9));
        cells.truncate(3);
        if cells.is_empty() {
            cells.push(s.centre);
        }
        cells
    }

    // ------------------------------------------------------------------ cibles aériennes (appelées par les avions)
    /// Une ligne de cellules combustibles centrée sur i0 : le long de l'axe donné (degrés), sinon perpendiculaire au vent.
    fn ligne_en(&self, p: &Partie, i0: usize, longueur: i64, axe: Option<i64>) -> Vec<usize> {
        let g = &p.carte.g;
        let (ux, uy) = match axe {
            None => {
                let (vx, vy) = p.meteo.vers();
                (-vy, vx)
            }
            Some(a) => {
                let r = (a as f64).to_radians();
                (r.cos(), r.sin())
            }
        };
        let mut out = Vec::new();
        for k in -(longueur / 2)..=longueur / 2 {
            let kf = k as f64;
            if let Some(j) = g.sous(g.cx[i0] + ux * kf * g.hex_m, g.cy[i0] + uy * kf * g.hex_m) {
                if brule(p.carte.fuel[j]) && p.feu.etat[j] != BRULE && !out.contains(&j) {
                    out.push(j);
                }
            }
        }
        out
    }

    pub fn cible_largage(&self, p: &Partie, u: &Moyen) -> Option<Vec<usize>> {
        let (feu, g) = (&p.feu, &p.carte.g);
        let m = u.mission.as_ref()?;
        if let Some(ci) = m.cible {
            // largage demandé par le joueur : la flamme la plus proche du point visé, sinon le point lui-même (mouiller)
            let proches = g.anneau(ci, 2).into_iter().filter(|&j| feu.etat[j] == BRULE);
            let cible = argmin(proches, |j| g.distance(j, ci)).unwrap_or(ci);
            if !brule(p.carte.fuel[cible]) {
                return None;
            }
            if u.kind == "canadair" {
                let mut l = self.ligne_en(p, cible, 3, m.axe);
                trier(&mut l, |&j| g.distance(j, cible));
                l.truncate(2);
                return Some(if l.is_empty() { vec![cible] } else { l });
            }
            return Some(vec![cible]);
        }
        let f = self.feux.get(&racine(&self.alias, m.feu?))?;
        if f.cells.is_empty() {
            return None;
        }
        let cells: Vec<usize> = f.cells.iter().copied().filter(|&i| feu.etat[i] == BRULE).collect();
        if cells.is_empty() {
            return None;
        }
        let (vx, vy) = p.meteo.vers();
        // déjà visés par d'autres avions : on répartit
        let vises: HashSet<usize> = p.moyens.iter().filter(|o| o.id != u.id).filter_map(|o| o.vise.as_ref()).flatten().copied().collect();
        let sautes: Vec<usize> = f.sautes.iter().copied().filter(|&i| feu.etat[i] == BRULE).collect();
        // le point le plus menaçant : la flamme la plus proche (en temps) d'un site menacé
        let mut menace = None;
        if let Some((&sid, &eta)) = argmin(f.menaces.iter(), |(_, &e)| e) {
            if eta < 120 {
                let s = &p.carte.sites[sid];
                menace = argmin(cells.iter().copied(), |i| s.cells.iter().map(|&k| g.distance(i, k)).min().unwrap_or(i64::MAX));
            }
        }
        let cible = if !sautes.is_empty() && (u.kind == "helico" || sautes.len() <= 3) {
            argmin(sautes.iter().copied(), |i| g.distance(i, u.cell)).expect("saute")
        } else if let (Some(mc), true) = (menace, u.kind == "canadair" || self.doctrine == "defensive") {
            mc
        } else {
            let mut front: Vec<usize> = f.front.iter().copied().filter(|&i| feu.etat[i] == BRULE).collect();
            if front.is_empty() {
                front = cells.clone();
            }
            let proj = |i: usize| g.cx[i] * vx + g.cy[i] * vy;
            trier(&mut front, |&i| (vises.contains(&i), -proj(i) - feu.i[i] * 200.0));
            if u.kind == "canadair" || self.doctrine == "offensive" {
                front[0]
            } else {
                argmax(front.iter().copied().take(6), |i| if vises.contains(&i) { -1.0 } else { feu.i[i] }).expect("front")
            }
        };
        let mut out = vec![cible];
        if u.kind == "canadair" {
            // un Canadair largue en ligne, le long du front (perpendiculaire au vent)
            for s in [1.0, -1.0] {
                if let Some(j) = g.sous(g.cx[cible] - vy * s * g.hex_m, g.cy[cible] + vx * s * g.hex_m) {
                    if j != cible && brule(p.carte.fuel[j]) && matches!(feu.etat[j], BRULE | INTACT) {
                        out.push(j);
                        break;
                    }
                }
            }
        }
        Some(out)
    }

    pub fn ligne_retardant(&self, p: &Partie, u: &Moyen) -> Vec<usize> {
        let Some(m) = u.mission.as_ref() else { return Vec::new() };
        if let Some(ci) = m.cible {
            return self.ligne_en(p, ci, 5, m.axe);
        }
        match m.feu.and_then(|fe| self.feux.get(&racine(&self.alias, fe))) {
            Some(f) if f.tete.is_some() => ligne_devant(p, f, 20.0, 70.0, 5),
            _ => Vec::new(),
        }
    }

    // ------------------------------------------------------------------ renforts et prévention
    fn renforts_demande(&mut self, p: &mut Partie) {
        let total: f64 = self.feux.values().filter(|f| f.statut != "éteint" && f.detecte).map(|f| f.ha).sum();
        let seuils = [120.0, 450.0, 1200.0];
        if self.renforts < seuils.len() && total > seuils[self.renforts] {
            self.renforts += 1;
            p.demander_renfort(4);
        }
    }

    /// Obligations légales de débroussaillement : hors risque sévère, les débroussailleuses dégagent les abords
    /// des villages et des campings. Par risque sévère, les travaux mécaniques en forêt sont interdits (étincelles).
    fn debroussaillage(&mut self, p: &mut Partie, feux_actifs: bool) {
        let engins: Vec<usize> = (0..p.moyens.len()).filter(|&k| p.moyens[k].kind == "debr" && !p.moyens[k].joueur && !p.moyens[k].hs).collect();
        if engins.is_empty() {
            return;
        }
        let met = &p.meteo;
        if met.danger >= 0.55 || met.nuit || feux_actifs {
            for &k in &engins {
                if p.moyens[k].mission.as_ref().is_some_and(|m| m.type_ == "debroussailler") {
                    p.moyens[k].mission = None;
                }
            }
            if p.meteo.danger >= 0.55 && !p.meteo.nuit && self.interdit_jour != Some(p.meteo.jour()) {
                self.interdit_jour = Some(p.meteo.jour());
                p.journal_ajout("info", "Risque sévère : travaux mécaniques interdits en forêt, les débroussailleuses restent au dépôt.".into());
            }
            return;
        }
        let mut faits: HashSet<usize> = engins.iter().filter_map(|&k| p.moyens[k].mission.as_ref()).flat_map(|m| m.cells.iter().copied()).collect();
        for &k in &engins {
            if p.moyens[k].mission.is_some() {
                continue;
            }
            let (c, g) = (&p.carte, &p.carte.g);
            let mut cand = Vec::new();
            for s in &c.sites {
                if !matches!(s.type_.as_str(), "ville" | "village" | "camping" | "usine") {
                    continue;
                }
                for i in g.anneau(s.centre, 3) {
                    if b"PTJGQ".contains(&c.fuel[i]) && c.site_de[i] < 0 && !faits.contains(&i) && p.feu.etat[i] == INTACT {
                        cand.push(i);
                    }
                }
            }
            if cand.is_empty() {
                return;
            }
            let ucell = p.moyens[k].cell;
            cand.sort_by_key(|&i| g.distance(i, ucell));
            cand.truncate(5);
            faits.extend(cand.iter().copied());
            p.moyens[k].mission =
                Some(Mission { type_: "debroussailler".into(), cells: cand, label: "Débroussaillement autour des habitations".into(), ..Default::default() });
        }
    }

    fn prevention(&mut self, p: &mut Partie, feux_actifs: bool) {
        self.debroussaillage(p, feux_actifs);
        if feux_actifs {
            self.prepos = false;
            self.gaar = false;
            return;
        }
        let haut = p.meteo.danger >= 0.55 && !p.meteo.nuit;
        if haut && !self.prepos {
            self.prepos = true;
            let (c, g) = (&p.carte, &p.carte.g);
            let croix: Vec<usize> = (0..g.n).filter(|&i| c.route[i] == 1 && b"PJQGF".contains(&c.fuel[i])).collect();
            let ccf: Vec<usize> = (0..p.moyens.len()).filter(|&k| p.moyens[k].kind == "ccf" && p.moyens[k].mission.is_none()).collect();
            let nb = ccf.len() / 3;
            if !croix.is_empty() && nb > 0 {
                let mut pts: Vec<usize> = Vec::new();
                for _ in 0..nb {
                    let best = argmax(croix.iter().copied(), |i| {
                        pts.iter().map(|&q| g.distance(i, q)).chain(c.casernes.iter().map(|k| g.distance(i, k.cell))).min().unwrap_or(99).min(99)
                    })
                    .expect("croisement");
                    pts.push(best);
                }
                for (&k, &q) in ccf.iter().zip(&pts) {
                    p.moyens[k].mission = Some(Mission { type_: "preposition".into(), poste: Some(q), label: "Patrouille préventive".into(), ..Default::default() });
                }
                let txt = format!("Risque {} : {nb} CCF pré-positionnés dans le massif.", p.meteo.niveau.to_lowercase());
                p.journal_ajout("cos", txt);
            }
        }
        if p.meteo.danger >= 0.7 && p.meteo.vol_possible("canadair") && !self.gaar {
            self.gaar = true;
            let libres: Vec<usize> = (0..p.moyens.len()).filter(|&k| p.moyens[k].kind == "canadair" && p.moyens[k].mission.is_none()).take(2).collect();
            let pts = circuit(p);
            for &k in &libres {
                p.moyens[k].mission = Some(Mission { type_: "gaar".into(), label: "Guet aérien armé".into(), pts: pts.clone(), ..Default::default() });
                p.moyens[k].phase = None;
            }
            if !libres.is_empty() {
                p.journal_ajout("cos", format!("Guet aérien armé : {} Canadair patrouillent, chargés d'eau.", libres.len()));
            }
        }
        if !haut {
            self.prepos = false;
            for u in p.moyens.iter_mut() {
                if u.mission.as_ref().is_some_and(|m| m.type_ == "preposition" || m.type_ == "gaar") {
                    u.mission = None;
                }
            }
        }
    }

    // ------------------------------------------------------------------ export
    pub fn export(&self, p: &Partie) -> Value {
        let mut out = Vec::new();
        for f in self.feux.values() {
            if f.absorbe.is_some() {
                continue;
            }
            if f.statut == "éteint" && f.hist.last().is_some_and(|h| p.t - h.0 > 120) {
                continue;
            }
            let mut moy = Map::new();
            for u in &p.moyens {
                if racine_de(&self.alias, &u.mission) == Some(f.id) {
                    let n = moy.get(u.kind).and_then(Value::as_i64).unwrap_or(0);
                    moy.insert(u.kind.to_string(), json!(n + 1));
                }
            }
            let mut menaces: Vec<(usize, i64)> = f.menaces.iter().map(|(&s, &e)| (s, e)).collect();
            menaces.sort_by_key(|t| t.1);
            let prev = if f.pc {
                json!(f.prev.iter().filter(|(_, &t)| t <= 180.0).take(2500).map(|(&i, &t)| json!([i, rond(t)])).collect::<Vec<_>>())
            } else {
                Value::Null
            };
            out.push(json!({
                "id": f.id, "nom": f.nom, "statut": if f.detecte { f.statut } else { "non détecté" }, "ha": f.ha,
                "n": f.cells.len(), "tete": f.tete, "vitesse": f.vitesse, "strategie": f.strategie, "prio": rond(f.prio),
                "menaces": menaces.iter().map(|&(s, e)| json!({"site": s, "eta": e})).collect::<Vec<_>>(),
                "moyens": moy, "detecte": f.detecte, "t0": f.t0, "tDetect": f.t_detect, "cause": f.cause,
                "sautes": f.sautes.len(), "front": f.front[..f.front.len().min(80)], "flancs": f.flancs[..f.flancs.len().min(60)],
                "touches": f.sites_touches, "cell0": f.cell0, "pc": f.pc, "prev": prev,
            }));
        }
        Value::Array(out)
    }
}

fn circuit(p: &Partie) -> Vec<(f64, f64)> {
    let g = &p.carte.g;
    let (w, h) = (g.largeur, g.hauteur);
    vec![(w * 0.35, h * 0.3), (w * 0.7, h * 0.35), (w * 0.65, h * 0.7), (w * 0.3, h * 0.65)]
}

fn postes_bord(p: &Partie, zone: &[usize], nb: usize) -> Vec<usize> {
    let g = &p.carte.g;
    let mut out: Vec<usize> = Vec::new();
    for &i in zone {
        if let Some(j) = std::iter::once(i).chain(g.voisins(i)).find(|&j| accessible(p, j) && !out.contains(&j)) {
            out.push(j);
        }
        if out.len() >= nb {
            break;
        }
    }
    out
}

/// Une route à 3-7 hexagones des flammes, hors de la prévision du feu pour les 2 h 30 à venir.
fn poste_pc(p: &Partie, f: &Fiche) -> Option<usize> {
    let (g, c) = (&p.carte.g, &p.carte);
    if f.cells.is_empty() {
        return None;
    }
    let mut best: Option<(i64, usize)> = None;
    for i in 0..g.n {
        let e = p.feu.etat[i];
        if c.route[i] < 2 || f.prev.get(&i).copied().unwrap_or(1e9) < 150.0 || (e != INTACT && e != BRULE_FINI) {
            continue;
        }
        if g.distance(i, f.cell0) > 12 {
            continue;
        }
        let dmin = f.cells.iter().map(|&k| g.distance(i, k)).min().unwrap_or(i64::MAX);
        if (3..=7).contains(&dmin) && best.map_or(true, |b| dmin < b.0) {
            best = Some((dmin, i));
        }
    }
    best.map(|(_, i)| i)
}

/// Postes au contact : à côté d'hexagones en feu d'intensité tenable, sur les flancs et l'arrière ;
/// on préfère le noir (déjà brûlé) et les pistes.
fn postes_attaque(p: &Partie, f: &Fiche, naissant: bool) -> Vec<usize> {
    let (feu, g) = (&p.feu, &p.carte.g);
    let (vx, vy) = p.meteo.vers();
    let sources: Vec<usize> = if naissant {
        f.cells.iter().copied().collect()
    } else {
        let mut s: Vec<usize> = f.flancs.clone();
        s.extend(f.sautes.iter().copied().filter(|i| !f.flancs.contains(i)));
        if s.is_empty() {
            s = f.front.clone();
        }
        s
    };
    let mut cand: IndexMap<usize, f64> = IndexMap::new();
    for i in sources {
        if feu.i[i] > 0.85 && !naissant {
            continue;
        }
        for v in &g.vois[i] {
            let j = v.j;
            if !accessible(p, j) || g.vois[j].iter().any(|w| feu.etat[w.j] == BRULE && feu.i[w.j] > 0.9) {
                continue;
            }
            let mut sc = g.cx[j] * vx + g.cy[j] * vy; // de l'arrière vers la tête
            if matches!(feu.etat[j], BRULE_FINI | ETEINT | NOYE) {
                sc -= 400.0;
            }
            if p.carte.route[j] > 0 {
                sc -= 200.0;
            }
            let e = cand.entry(j).or_insert(1e18);
            *e = e.min(sc);
        }
    }
    let mut l: Vec<(usize, f64)> = cand.into_iter().collect();
    trier(&mut l, |t| t.1);
    l.into_iter().take(14).map(|(j, _)| j).collect()
}

fn ligne_devant(p: &Partie, f: &Fiche, tmin: f64, tmax: f64, longueur: i64) -> Vec<usize> {
    let (g, c) = (&p.carte.g, &p.carte);
    let Some(h) = f.tete else { return Vec::new() };
    let (vx, vy) = p.meteo.vers();
    let mut best: Option<(f64, usize)> = None;
    for (&i, &t) in &f.prev {
        if tmin <= t && t <= tmax {
            let d = (g.cx[i] - g.cx[h]) * vx + (g.cy[i] - g.cy[h]) * vy;
            let lat = ((g.cx[i] - g.cx[h]) * -vy + (g.cy[i] - g.cy[h]) * vx).abs();
            let sc = d - 2.0 * lat;
            if best.map_or(true, |b| sc > b.0) {
                best = Some((sc, i));
            }
        }
    }
    let Some((_, i0)) = best else { return Vec::new() };
    let mut out = Vec::new();
    for k in -(longueur / 2)..=longueur / 2 {
        let kf = k as f64;
        if let Some(j) = g.sous(g.cx[i0] - vy * kf * g.hex_m, g.cy[i0] + vx * kf * g.hex_m) {
            if brule(c.fuel[j]) && p.feu.etat[j] == INTACT && c.site_de[j] < 0 && !out.contains(&j) {
                out.push(j);
            }
        }
    }
    trier(&mut out, |&j| -(g.cx[j] * -vy + g.cy[j] * vx));
    out
}
