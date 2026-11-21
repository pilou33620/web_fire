//! Les moyens de lutte : camions-citernes feux de forêt (CCF), bulldozers, hélicoptères bombardiers d'eau (HBE),
//! Canadair (indicatif « Pélican ») et Dash 8 de retardant (indicatif « Milan »), et les engins de soutien.
//!
//! Unités : vitesses en hexagones de 300 m par minute (1 hex/min = 18 km/h), ramenées à la taille des hexagones de la carte
//! (Grille::ech) ; eau en litres.

use serde_json::{json, Map, Value};

use crate::carte::Carte;
use crate::combustibles::{brule, est_eau};
use crate::feu::{Feu, BRULE, BRULE_FINI, ETEINT, NOYE};
use crate::hexa::{astar, Grille};
use crate::partie::Partie;
use crate::{argmax, argmin, rond};

pub struct TypeMoyen {
    pub nom: &'static str,
    pub cap: f64,
    pub debit: f64,
    pub vitesse: f64,
    pub endurance: i64,
    pub terre: bool,
}

pub fn type_moyen(kind: &str) -> TypeMoyen {
    let t = |nom, cap, debit| TypeMoyen { nom, cap, debit, vitesse: 0.0, endurance: 0, terre: true };
    let a = |nom, cap, vitesse, endurance| TypeMoyen { nom, cap, debit: 400.0, vitesse, endurance, terre: false };
    match kind {
        "ccf" => t("CCF", 4000.0, 400.0),
        "bull" => t("Bulldozer", 0.0, 400.0),
        "vlhr" => t("VLHR", 0.0, 400.0), // véhicule léger hors route du chef de groupe
        "fpt" => t("FPT", 3000.0, 500.0), // fourgon pompe-tonne : défense des maisons, routes seulement
        "vpc" => t("VPC", 0.0, 400.0),   // véhicule poste de commandement
        "debr" => t("Débroussailleuse", 0.0, 400.0),
        "ravit" => t("Ravitailleur", 0.0, 400.0), // camion-citerne de gazole
        "vsav" => t("VSAV", 0.0, 400.0),          // véhicule de secours et d'assistance aux victimes
        "dragon" => a("Dragon", 0.0, 12.0, 240),  // hélicoptère de secours
        "helico" => a("HBE", 1200.0, 10.0, 150),
        "canadair" => a("Canadair", 6000.0, 16.0, 180),
        _ => a("Dash 8", 10000.0, 24.0, 240),
    }
}

/// Minutes entre l'alerte et le départ.
fn delai(kind: &str) -> i64 {
    match kind {
        "ccf" | "vlhr" => 3,
        "bull" | "debr" | "dragon" => 6,
        "fpt" | "vsav" => 2,
        "vpc" => 5,
        "ravit" => 4,
        "helico" => 7,
        "canadair" => 14,
        _ => 18, // dash
    }
}

/// Gazole : minutes de travail d'un plein (rouler ou pomper consomme 1 par minute, attendre 0,2).
fn carburant(kind: &str) -> Option<f64> {
    match kind {
        "ccf" | "fpt" => Some(600.0),
        "bull" | "debr" => Some(420.0),
        "vlhr" | "vpc" | "ravit" | "vsav" => Some(900.0),
        _ => None,
    }
}

fn equipage(kind: &str) -> i64 {
    match kind {
        "ccf" => 4,
        "fpt" => 6,
        "vpc" | "vsav" => 3,
        "bull" | "debr" | "ravit" => 1,
        _ => 2,
    }
}

/// Demi-angle du cône des lances (degrés).
pub const CONE: f64 = 50.0;
/// Litres gardés pour l'autoprotection : les lances n'y touchent jamais.
pub const RESERVE: f64 = 500.0;
/// Litres par minute des rampes d'aspersion quand le camion est encerclé.
pub const DEBIT_AUTO: f64 = 120.0;

pub fn ecart_angle(a: f64, b: f64) -> f64 {
    ((a - b + 180.0).rem_euclid(360.0) - 180.0).abs()
}

/// La cellule j est-elle dans le cône (direction, demi-angle) vu depuis i ? Sa propre cellule l'est toujours.
pub fn dans_cone(g: &Grille, i: usize, j: usize, cone: Option<(f64, f64)>) -> bool {
    let Some((dir, demi)) = cone else { return true };
    if j == i || demi >= 180.0 {
        return true;
    }
    let a = (g.cy[j] - g.cy[i]).atan2(g.cx[j] - g.cx[i]).to_degrees();
    ecart_angle(a, dir) <= demi
}

/// Hexagones (de la carte) par minute pour un engin terrestre sur la cellule i.
pub fn vitesse_sol(kind: &str, c: &Carte, feu: &Feu, i: usize) -> f64 {
    vitesse_300(kind, c, feu, i) * c.g.ech
}

fn vitesse_300(kind: &str, c: &Carte, feu: &Feu, i: usize) -> f64 {
    let r = c.route[i] as usize;
    let f = c.fuel[i];
    let eau = est_eau(f);
    if kind == "bull" || kind == "debr" {
        return if eau { 0.0 } else { [0.35, 0.8, 1.2, 1.2][r] };
    }
    if kind == "fpt" {
        // pas tout-terrain : routes et lieux habités seulement
        if r >= 2 {
            return if r == 3 { 3.5 } else { 3.0 };
        }
        return if c.site_de[i] >= 0 && !eau { 1.0 } else { 0.0 };
    }
    if matches!(kind, "vsav" | "ravit" | "vpc") && r == 0 {
        return if eau { 0.0 } else { 0.3 };
    }
    if r >= 3 {
        3.3
    } else if r == 2 {
        2.8
    } else if r == 1 {
        1.5
    } else if eau {
        0.0
    } else if matches!(feu.etat[i], BRULE_FINI | ETEINT | NOYE) {
        0.8 // « travailler depuis le noir »
    } else if b"PFQJG".contains(&f) {
        0.45
    } else {
        0.9
    }
}

/// Une mission (ce que Python rangeait dans un dictionnaire libre).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mission {
    pub type_: String,
    pub feu: Option<usize>,
    pub poste: Option<usize>,
    pub label: String,
    pub site: Option<usize>,
    /// blessé (secours), moyen (ravitailler) ou cellule visée (largage, retardant)
    pub cible: Option<usize>,
    pub cells: Vec<usize>,
    pub dir: Option<i64>,
    pub axe: Option<i64>,
    pub pts: Vec<(f64, f64)>,
    pub k: usize,
    pub vu: bool,
    pub bloque: i64,
    pub inactif: i64,
    pub travail: i64,
    pub pose: i64,
    pub fini: bool,
}

/// Ce que fait le moyen, pour l'affichage : {k, c, d}.
#[derive(Clone, Debug, PartialEq)]
pub struct Act {
    pub k: &'static str,
    pub c: Option<usize>,
    pub d: Option<i64>,
}

fn act(k: &'static str) -> Option<Act> {
    Some(Act { k, c: None, d: None })
}

fn act_c(k: &'static str, c: usize) -> Option<Act> {
    Some(Act { k, c: Some(c), d: None })
}

/// Des blessés à bord (VSAV, Dragon).
#[derive(Clone, Debug)]
pub struct Charge {
    pub n: i64,
    pub g: String,
    pub t: String,
}

#[derive(Clone, Debug)]
pub struct Moyen {
    pub id: usize,
    pub kind: &'static str,
    pub nom: String,
    pub base: usize,
    pub cap: f64,
    pub eau: f64,
    pub carb_max: Option<f64>,
    pub carb: f64,
    pub reserve: f64,
    pub charge: Vec<Charge>,
    pub terre: bool,
    pub cell: usize,
    pub x: f64,
    pub y: f64,
    pub cap_deg: f64,
    /// dispo, alerte, route, action, ravitaillement, au_sol, retour, autoprotection, panne, hs
    pub etat: &'static str,
    pub mission: Option<Mission>,
    pub chemin: Vec<usize>,
    pub avance: f64,
    /// minutes d'immobilisation (remplissage, écopage, défrichage)
    pub attente: i64,
    /// cycle aérien : charge, ecope, vers_feu, gaar
    pub phase: Option<&'static str>,
    /// (x, y) en vol
    pub cible: Option<(f64, f64)>,
    /// cellules visées par le prochain largage
    pub vise: Option<Vec<usize>>,
    /// minutes de vol depuis le dernier plein
    pub vol: i64,
    pub act: Option<Act>,
    pub poste: Option<usize>,
    pub litres: f64,
    pub largages: i64,
    pub renfort: bool,
    /// sous les ordres du joueur : l'IA n'y touche pas
    pub joueur: bool,
    /// hors service (engin perdu dans le feu)
    pub hs: bool,
    pub groupe: Option<String>,
    /// autoprotection en cours
    pub auto: bool,
    /// [minute (heure de jeu), x, y] des derniers points de passage, pour que l'interface suive le trajet
    pub trace: Vec<[i64; 3]>,
    repli_t: i64,
}

impl Moyen {
    pub fn new(id: usize, kind: &'static str, nom: String, base: usize, c: &Carte) -> Moyen {
        let t = type_moyen(kind);
        let carb_max = carburant(kind);
        Moyen {
            id,
            kind,
            nom,
            base,
            cap: t.cap,
            eau: t.cap,
            carb_max,
            carb: carb_max.unwrap_or(0.0),
            reserve: match kind {
                "ccf" => 500.0,
                "fpt" => 300.0,
                _ => 0.0,
            },
            charge: Vec::new(),
            terre: t.terre,
            cell: base,
            x: c.g.cx[base],
            y: c.g.cy[base],
            cap_deg: 0.0,
            etat: "dispo",
            mission: None,
            chemin: Vec::new(),
            avance: 0.0,
            attente: 0,
            phase: None,
            cible: None,
            vise: None,
            vol: 0,
            act: None,
            poste: None,
            litres: 0.0,
            largages: 0,
            renfort: false,
            joueur: false,
            hs: false,
            groupe: None,
            auto: false,
            trace: Vec::new(),
            repli_t: -99,
        }
    }

    fn m(&mut self) -> &mut Mission {
        self.mission.as_mut().expect("mission en cours")
    }

    // ------------------------------------------------------------------ déplacement au sol
    /// Calcule un chemin vers dest. Rend false si inaccessible.
    pub fn aller(&mut self, p: &Partie, dest: usize) -> bool {
        if dest == self.cell {
            self.chemin.clear();
            return true;
        }
        match self.chemin_vers(p, dest, None) {
            Some(ch) => {
                self.chemin = ch[1..].to_vec();
                self.avance = 0.0;
                true
            }
            None => false,
        }
    }

    /// Le chemin (liste de cellules) jusqu'à dest, sans rien modifier (sert aussi à l'aperçu des ordres).
    pub fn chemin_vers(&self, p: &Partie, dest: usize, depart: Option<usize>) -> Option<Vec<usize>> {
        let (c, feu) = (&p.carte, &p.feu);
        let depart = depart.unwrap_or(self.cell);
        if dest == depart {
            return Some(vec![depart]);
        }
        let kind = self.kind;
        let cout = |_i: usize, j: usize| -> Option<f64> {
            let mut v = vitesse_sol(kind, c, feu, j);
            if v <= 0.0 {
                if c.fuel[j] == b'R' && c.route[j] > 0 {
                    v = 2.0 * c.g.ech; // un pont
                } else {
                    return None;
                }
            }
            if feu.etat[j] == BRULE && j != dest {
                return None;
            }
            if p.danger.contains(&j) && j != dest {
                if c.route[j] == 1 {
                    return None; // malus des pistes : on ne s'engage pas sur une piste qui longe un feu intense
                }
                return Some(1.0 / v + 2.5);
            }
            Some(1.0 / v)
        };
        // destination encerclée (par le feu) : inutile de fouiller toute la carte pour le découvrir
        if !c.g.vois[depart].iter().any(|v| v.j == dest) && c.g.vois[dest].iter().all(|v| cout(dest, v.j).is_none()) {
            return None;
        }
        astar(&c.g, depart, dest, cout, 0.42 / c.g.ech).map(|(ch, _)| ch)
    }

    /// Les malus de la voie empruntée : bouchons et fumée sur les routes, pistes trop étroites pour se croiser.
    pub fn facteur_voie(&self, p: &Partie, j: usize) -> f64 {
        let r = p.carte.route[j];
        if r >= 2 {
            let f = 1.0 / (1.0 + 0.6 * *p.trafic_convois.get(&j).unwrap_or(&0) as f64);
            return f * (1.0 - 0.6 * p.evac.fumee.get(&j).copied().unwrap_or(0.0)).max(0.45);
        }
        if r == 1 {
            let autres = p.trafic_engins.get(&j).copied().unwrap_or(0) - i64::from(self.cell == j);
            return if autres >= 2 { 0.35 } else { 1.0 };
        }
        1.0
    }

    pub fn avancer_sol(&mut self, p: &Partie) {
        let (c, feu, g) = (&p.carte, &p.feu, &p.carte.g);
        let mut budget = 1.0;
        while budget > 0.0 && !self.chemin.is_empty() {
            let j = self.chemin[0];
            if feu.etat[j] == BRULE {
                let dest = *self.chemin.last().expect("chemin");
                if !self.aller(p, dest) {
                    self.chemin.clear();
                }
                return;
            }
            let vs = vitesse_sol(self.kind, c, feu, j);
            let v = (if vs != 0.0 { vs } else { 2.0 * g.ech }) * self.facteur_voie(p, j);
            let reste = (1.0 - self.avance) / v;
            if reste <= budget {
                budget -= reste;
                self.cell = j;
                self.trace.push([p.meteo.t, rond(g.cx[j]), rond(g.cy[j])]);
                self.chemin.remove(0);
                self.avance = 0.0;
            } else {
                self.avance += budget * v;
                budget = 0.0;
            }
        }
        // position affichée : entre la cellule courante et la suivante
        if let Some(&j) = self.chemin.first() {
            let (x0, y0, x1, y1) = (g.cx[self.cell], g.cy[self.cell], g.cx[j], g.cy[j]);
            self.x = x0 + (x1 - x0) * self.avance;
            self.y = y0 + (y1 - y0) * self.avance;
            self.cap_deg = (y1 - y0).atan2(x1 - x0).to_degrees();
        } else {
            self.x = g.cx[self.cell];
            self.y = g.cy[self.cell];
        }
    }

    // ------------------------------------------------------------------ vol
    /// Vole vers (x, y). Rend true à l'arrivée.
    fn voler(&mut self, g: &Grille, x: f64, y: f64) -> bool {
        let v = type_moyen(self.kind).vitesse * 300.0;
        let (dx, dy) = (x - self.x, y - self.y);
        let d = dx.hypot(dy);
        self.vol += 1;
        if d > 1.0 {
            self.cap_deg = dy.atan2(dx).to_degrees();
        }
        if d <= v {
            self.x = x;
            self.y = y;
            self.cell = g.sous(x, y).unwrap_or(self.cell);
            return true;
        }
        self.x += dx / d * v;
        self.y += dy / d * v;
        self.cell = g.sous(self.x, self.y).unwrap_or(self.cell);
        false
    }

    // ------------------------------------------------------------------ un pas
    pub fn pas(&mut self, p: &mut Partie) {
        self.act = None;
        // délai d'alerte : partir de la caserne ou décoller prend du temps
        let au_depart = self.cell == self.base || Some(self.cell) == p.carte.aerodrome || Some(self.cell) == p.carte.helistation;
        if let Some(m) = self.mission.as_mut() {
            if !m.vu {
                m.vu = true;
                if (self.etat == "dispo" || self.etat == "au_sol") && self.attente == 0 && au_depart {
                    self.attente = delai(self.kind);
                    self.etat = "alerte";
                }
            }
        }
        if self.attente > 0 {
            self.attente -= 1;
            if self.etat == "ravitaillement" {
                self.act = act("remplit");
            } else if self.etat == "action" && self.kind == "bull" {
                self.act = act_c("defriche", self.cell);
            } else if self.phase == Some("ecope") {
                self.act = act_c("ecope", self.cell);
            } else if self.etat == "au_sol" {
                self.act = act("au_sol");
            } else if self.etat == "alerte" {
                self.act = act("alerte");
            }
            if self.attente == 0 {
                self.fin_attente(p);
            }
            return;
        }
        if self.terre {
            self.pas_sol(p);
        } else {
            self.pas_air(p);
        }
        let pos = [rond(self.x), rond(self.y)];
        if self.trace.last().map_or(true, |t| t[1..] != pos) {
            self.trace.push([p.meteo.t, pos[0], pos[1]]);
        }
        if self.trace.len() > 24 {
            let n = self.trace.len() - 24;
            self.trace.drain(..n);
        }
    }

    fn fin_attente(&mut self, p: &mut Partie) {
        if self.etat == "ravitaillement" {
            self.eau = self.cap;
            self.etat = "route";
            if let Some(poste) = self.poste {
                self.aller(p, poste);
            }
        } else if self.kind == "bull" && self.etat == "action" {
            if let Some(m) = self.mission.as_mut() {
                if !m.cells.is_empty() {
                    let i = m.cells.remove(0);
                    if p.feu.defricher(&mut p.carte, i) {
                        p.stats.defriche += 1;
                    }
                }
            }
            self.etat = "route";
        } else if self.phase == Some("ecope") {
            self.eau = self.cap;
            self.phase = Some("vers_feu");
            self.cible = None;
        } else if self.etat == "alerte" {
            self.etat = if self.terre { "route" } else { "action" };
        } else if self.etat == "au_sol" {
            self.vol = 0;
            self.eau = self.cap;
            self.etat = "dispo";
        }
    }

    // ---------------------------------------------------------------- engins terrestres
    /// Le feu est-il sur le camion (sa cellule brûle, ou au moins deux voisines brûlent fort) ?
    fn encercle(&self, p: &Partie, i: usize) -> bool {
        let feu = &p.feu;
        // seul le front compte : un hexagone allumé depuis peu (ensuite, sur la route ou la piste, le combustible
        // autour du camion est déjà consumé ; il ne reste que des braises)
        let front = |j: usize, imin: f64| feu.etat[j] == BRULE && feu.i[j] > imin && p.t - feu.t_ign[j] <= 8;
        if front(i, 0.3) {
            return true;
        }
        p.carte.g.vois[i].iter().filter(|v| front(v.j, 0.7)).count() >= 2
    }

    /// Repli si c'est possible ; sinon autoprotection (CCF) ; réserve épuisée : l'engin est perdu.
    /// Rend true si le moyen a passé la minute à se protéger.
    fn securite(&mut self, p: &mut Partie) -> bool {
        if !self.encercle(p, self.cell) {
            if self.auto {
                self.auto = false;
                self.etat = "route";
                p.journal_ajout("cos", format!("{} : le front est passé, fin de l'autoprotection ; l'équipage repart faire le plein.", self.nom));
                if p.rnd.random() < 0.3 {
                    p.ajouter_blesses(self.cell, 1, "pompier", "leger", &format!("pompier du {} (coup de chaleur)", self.nom));
                }
            }
            return false;
        }
        if !self.auto {
            let (c, feu) = (&p.carte, &p.feu);
            let mut sur: Vec<usize> = c
                .g
                .voisins(self.cell)
                .into_iter()
                .filter(|&j| feu.etat[j] != BRULE && vitesse_sol(self.kind, c, feu, j) > 0.0 && !self.encercle(p, j))
                .collect();
            if !sur.is_empty() {
                crate::trier(&mut sur, |&j| (!matches!(feu.etat[j], BRULE_FINI | ETEINT | NOYE), -(c.route[j] as i64)));
                self.cell = sur[0];
                self.chemin.clear();
                self.x = c.g.cx[self.cell];
                self.y = c.g.cy[self.cell];
                if p.t - self.repli_t > 15 {
                    // une seule annonce par quart d'heure
                    p.journal_ajout("repli", format!("{} se replie : le feu arrive sur sa position.", self.nom));
                }
                self.repli_t = p.t;
                return false;
            }
        }
        // plus d'issue : on reste, l'équipage s'abrite dans la cabine et les rampes d'aspersion arrosent le camion
        let lances = self.kind == "ccf" || self.kind == "fpt";
        if lances && self.eau > 0.0 {
            if !self.auto {
                self.auto = true;
                p.stats.autoprotections += 1;
                p.journal_ajout("alerte", format!("🛡 {} encerclé par le feu : autoprotection déclenchée, l'équipage est à l'abri dans la cabine.", self.nom));
            }
            let l = self.eau.min(DEBIT_AUTO);
            self.eau -= l;
            self.litres += l;
            p.feu.mouille[self.cell] = (p.feu.mouille[self.cell] + 0.15).min(1.0);
            self.etat = "autoprotection";
            self.chemin.clear();
            self.act = act_c("autoprotection", self.cell);
            return true;
        }
        if lances || (p.feu.etat[self.cell] == BRULE && p.feu.i[self.cell] > 0.5) {
            self.hs = true;
            p.ajouter_blesses(self.cell, equipage(self.kind), "equipage", "danger", &format!("équipage du {}", self.nom));
            self.auto = false;
            self.mission = None;
            self.joueur = false;
            self.etat = "hs";
            p.stats.perdus += 1;
            let quoi = if lances { "réserve d'autoprotection épuisée" } else { "pris par les flammes" };
            p.journal_ajout("alerte", format!("⚠ {} perdu ({quoi}) : l'équipage s'est mis à l'abri, il faut l'évacuer (Dragon ou VSAV).", self.nom));
            return true;
        }
        false
    }

    /// Consommation de gazole ; panne sèche à zéro (jusqu'à l'arrivée d'un ravitailleur). Rend true en panne.
    fn gazole(&mut self, p: &mut Partie) -> bool {
        let Some(cm) = self.carb_max else { return false };
        if self.cell == self.base && (self.etat == "dispo" || self.etat == "retour") {
            self.carb = cm;
        }
        let k = self.act.as_ref().map(|a| a.k).unwrap_or("");
        let actif = matches!(self.etat, "route" | "retour" | "ravitaillement")
            || matches!(k, "arrose" | "protege" | "mouille" | "noie" | "defriche" | "debroussaille" | "autoprotection");
        self.carb = (self.carb - if actif { 1.0 } else { 0.2 }).max(0.0);
        if self.carb <= 0.0 {
            if self.etat != "panne" {
                let suite = if p.nb_kind("ravit") > 0 { " : un ravitailleur est demandé." } else { " : plus de gazole, il attendra." };
                p.journal_ajout("alerte", format!("⛽ {} en panne sèche{suite}", self.nom));
            }
            self.etat = "panne";
            self.act = act("panne");
            self.chemin.clear();
            return true;
        }
        if self.etat == "panne" {
            self.etat = "route";
        }
        false
    }

    /// Va vers `poste` (recalcule le chemin si besoin) ; compte les échecs dans la mission.
    fn rejoindre(&mut self, p: &Partie, poste: usize) {
        if self.chemin.last() != Some(&poste) && !self.aller(p, poste) {
            if let Some(m) = self.mission.as_mut() {
                m.bloque += 1;
            }
        }
        self.etat = "route";
        self.avancer_sol(p);
    }

    fn pas_sol(&mut self, p: &mut Partie) {
        if self.hs {
            self.etat = "hs";
            self.act = act("hs");
            return;
        }
        if self.securite(p) || self.gazole(p) {
            return;
        }
        // gazole presque vide et pas de ravitailleur : on rentre faire le plein à la caserne
        if let Some(cm) = self.carb_max {
            if self.carb < 0.12 * cm && p.nb_kind("ravit") == 0 && self.mission.is_some() && !self.joueur {
                self.mission = None;
            }
        }
        if self.mission.is_none() {
            // pas de mission : retour à la caserne
            if self.cell != self.base {
                if self.chemin.is_empty() {
                    let base = self.base;
                    self.aller(p, base);
                }
                self.etat = "retour";
            } else {
                self.etat = "dispo";
                if self.eau < self.cap {
                    self.eau = (self.eau + 500.0).min(self.cap);
                }
            }
            self.avancer_sol(p);
            return;
        }
        match self.kind {
            "bull" => return self.pas_bull(p),
            "vlhr" => return self.pas_chef(p),
            "debr" => return self.pas_debr(p),
            "vpc" | "ravit" | "vsav" => return self.pas_logistique(p),
            _ => {}
        }
        // ravitaillement en eau
        if self.eau < self.reserve + 300.0 && self.etat != "ravitaillement" {
            if let Some(pe) = p.point_eau_proche(self.cell) {
                if self.cell == pe {
                    self.etat = "ravitaillement";
                    self.attente = 8;
                    self.act = act("remplit");
                    return;
                }
                if self.chemin.last() != Some(&pe) {
                    self.aller(p, pe);
                }
                self.etat = "route";
                self.act = act("va_remplir");
                self.avancer_sol(p);
                return;
            }
        }
        let (poste, dir, t) = {
            let m = self.m();
            (m.poste, m.dir, m.type_.clone())
        };
        self.poste = poste;
        if let Some(poste) = poste.filter(|&q| q != self.cell) {
            self.rejoindre(p, poste);
            // sur le trajet, on arrose ce qui est à portée et peu intense
            if !self.chemin.is_empty() && self.eau > 600.0 && dir.is_none() {
                // orienté par le joueur : on garde l'eau pour son cône
                self.arroser_voisins(p, 0.5, Some(200.0), None);
            }
            return;
        }
        self.etat = "action";
        self.x = p.carte.g.cx[self.cell];
        self.y = p.carte.g.cy[self.cell];
        let cone = Some((dir.unwrap_or(0) as f64, CONE));
        match t.as_str() {
            "defense" => {
                let proche = p.carte.g.anneau(self.cell, 2).iter().any(|&j| p.feu.etat[j] == BRULE);
                if proche {
                    if !self.arroser_voisins(p, 1.01, None, None) {
                        if self.eau > self.reserve + 120.0 {
                            p.feu.mouille[self.cell] = (p.feu.mouille[self.cell] + 0.04).min(1.0);
                            self.eau -= 120.0;
                        }
                        self.act = act_c("protege", self.cell);
                    }
                } else {
                    self.act = act("attend");
                }
            }
            "attaque" => {
                if !self.arroser_voisins(p, 0.8, None, None) && !self.noyer_voisins(p, None) {
                    self.m().inactif += 1;
                    self.act = act("attend");
                }
            }
            "surveillance" => {
                if !self.noyer_voisins(p, None) && !self.arroser_voisins(p, 0.8, None, None) {
                    self.m().inactif += 1;
                    self.act = act("surveille");
                }
            }
            "preposition" => self.act = act("attend"),
            "arroser" | "flanc" => {
                if !self.arroser_voisins(p, 0.8, None, cone) && !self.noyer_voisins(p, cone) {
                    let m = self.m();
                    m.inactif += 1;
                    let inactif = m.inactif;
                    self.act = Some(Act { k: "attend", c: None, d: dir });
                    if t == "flanc" && inactif >= 2 {
                        crate::ordres::suivre_flanc(p, self);
                    }
                }
            }
            "ligne" => {
                if !self.arroser_voisins(p, 0.85, None, cone) {
                    self.mouiller(p, cone.expect("cône"));
                }
            }
            _ => {}
        }
    }

    /// Ligne d'arrêt sans flammes à portée : on mouille la végétation devant soi (ligne d'eau).
    fn mouiller(&mut self, p: &mut Partie, cone: (f64, f64)) {
        let (g, c) = (&p.carte.g, &p.carte);
        let cands: Vec<usize> = g
            .voisins(self.cell)
            .into_iter()
            .filter(|&j| dans_cone(g, self.cell, j, Some(cone)) && brule(c.fuel[j]) && p.feu.etat[j] != BRULE && p.feu.mouille[j] < 0.6)
            .collect();
        let d = Some(cone.0 as i64);
        if cands.is_empty() || self.eau < self.reserve + 150.0 {
            self.act = Some(Act { k: "attend", c: None, d });
            return;
        }
        let j = argmin(cands, |k| p.feu.mouille[k]).expect("candidat");
        p.feu.mouille[j] = (p.feu.mouille[j] + 0.05).min(0.6);
        self.eau -= 120.0;
        self.litres += 120.0;
        self.act = Some(Act { k: "mouille", c: Some(j), d });
    }

    /// Débroussailleuse : broie le sous-bois et les broussailles, cellule après cellule (3 minutes chacune).
    fn pas_debr(&mut self, p: &mut Partie) {
        {
            let (c, feu) = (&p.carte, &p.feu);
            let m = self.m();
            while let Some(&i) = m.cells.first() {
                if b"PFTJGQ".contains(&c.fuel[i]) && feu.etat[i] == 0 {
                    break;
                }
                m.cells.remove(0);
            }
            if m.cells.is_empty() {
                m.fini = true;
                self.act = act("attend");
                return;
            }
        }
        let i = self.m().cells[0];
        if std::iter::once(i).chain(p.carte.g.voisins(i)).any(|k| p.feu.etat[k] == BRULE) {
            self.m().cells.remove(0);
            return;
        }
        if self.cell != i {
            if self.chemin.last() != Some(&i) && !self.aller(p, i) {
                self.m().cells.remove(0);
                return;
            }
            self.etat = "route";
            self.avancer_sol(p);
            return;
        }
        self.etat = "action";
        let m = self.m();
        m.travail += 1;
        let fait = m.travail >= 3;
        self.act = act_c("debroussaille", i);
        if fait {
            let m = self.m();
            m.travail = 0;
            m.cells.remove(0);
            p.carte.fuel[i] = if b"PFT".contains(&p.carte.fuel[i]) { b'Y' } else { b'H' };
            p.stats.debroussaille += 1;
            p.carte_modifiee.insert(i);
        }
    }

    /// PC, ravitailleur, VSAV : aller au poste, puis faire leur travail (les missions viennent de la logistique).
    fn pas_logistique(&mut self, p: &mut Partie) {
        let (poste, t, cible) = {
            let m = self.m();
            (m.poste, m.type_.clone(), m.cible)
        };
        if let Some(poste) = poste.filter(|&q| q != self.cell) {
            self.rejoindre(p, poste);
            return;
        }
        self.etat = "action";
        self.x = p.carte.g.cx[self.cell];
        self.y = p.carte.g.cy[self.cell];
        match t.as_str() {
            "pc" => {
                let m = self.m();
                m.pose += 1;
                let pose = m.pose;
                self.act = act(if pose >= 5 { "pc" } else { "deploie" });
                if pose == 5 {
                    p.journal_ajout("cos", format!("📡 {} posé : prévision du feu affichée, commandement plus réactif, lances mieux coordonnées (+20 %).", self.nom));
                }
            }
            "ravitailler" => {
                let k = cible.and_then(|id| p.moyens.iter().position(|x| x.id == id));
                let Some(k) = k.filter(|&k| p.moyens[k].carb_max.is_some() && p.carte.g.distance(p.moyens[k].cell, self.cell) <= 1) else {
                    self.m().fini = true;
                    return;
                };
                let m = self.m();
                m.travail += 1;
                let fait = m.travail >= 6;
                self.act = act_c("ravitaille", p.moyens[k].cell);
                if fait {
                    let u = &mut p.moyens[k];
                    u.carb = u.carb_max.unwrap_or(u.carb);
                    if u.etat == "panne" {
                        u.etat = "route";
                    }
                    let txt = format!("⛽ {} a fait le plein de {}.", self.nom, u.nom);
                    p.journal_ajout("info", txt);
                    self.m().fini = true;
                }
            }
            "secours" => self.secourir(p, cible, 5, 3, "info"),
            "hopital" => {
                p.deposer(self);
                self.mission = None;
            }
            _ => self.act = act("attend"),
        }
    }

    /// Prise en charge de blessés (VSAV : 5 minutes, 3 à la fois ; Dragon : 3 minutes, 4 à la fois), puis l'hôpital.
    fn secourir(&mut self, p: &mut Partie, cible: Option<usize>, duree: i64, places: i64, genre: &str) {
        let pret = match cible.and_then(|id| p.reg.blesse(id)) {
            Some(b) if b.etat == "attente" => true,
            _ => false,
        };
        if !pret {
            if self.kind == "dragon" {
                self.mission = None;
            } else {
                self.m().fini = true;
            }
            return;
        }
        let m = self.m();
        m.travail += 1;
        let fait = m.travail >= duree;
        let poste = m.poste.unwrap_or(self.cell);
        self.act = act_c(if self.kind == "dragon" { "treuille" } else { "soigne" }, if self.kind == "dragon" { poste } else { self.cell });
        if !fait {
            return;
        }
        let b = p.reg.blesse(cible.expect("blessé")).expect("blessé");
        let n = places.min(b.n);
        b.n -= n;
        self.charge.push(Charge { n, g: b.g.clone(), t: b.t.clone() });
        if b.n <= 0 {
            b.etat = "pris";
        }
        let quoi = b.quoi.clone();
        let txt = if self.kind == "dragon" {
            format!("🚁 {} hélitreuille {} personne{} ({}).", self.nom, n, if n > 1 { "s" } else { "" }, quoi)
        } else {
            format!("🚑 {} prend en charge {} blessé{} ({}).", self.nom, n, if n > 1 { "s" } else { "" }, quoi)
        };
        p.journal_ajout(genre, txt);
        let label = if self.kind == "dragon" { "Vers l'hôpital" } else { "Transport vers l'hôpital" };
        self.mission = Some(Mission { type_: "hopital".into(), poste: Some(p.hopital), label: label.into(), ..Default::default() });
    }

    /// Le VLHR du chef de groupe suit son groupe et se tient un peu en arrière.
    fn pas_chef(&mut self, p: &mut Partie) {
        let poste = self.m().poste;
        if let Some(poste) = poste.filter(|&q| q != self.cell) {
            if self.chemin.last() != Some(&poste) {
                self.aller(p, poste);
            }
            self.etat = "route";
            self.avancer_sol(p);
            return;
        }
        self.etat = "action";
        self.x = p.carte.g.cx[self.cell];
        self.y = p.carte.g.cy[self.cell];
        self.act = act("commande");
    }

    pub fn arroser_voisins(&mut self, p: &mut Partie, imax: f64, debit: Option<f64>, cone: Option<(f64, f64)>) -> bool {
        let g = &p.carte.g;
        let cands: Vec<usize> = std::iter::once(self.cell)
            .chain(g.voisins(self.cell))
            .filter(|&j| p.feu.etat[j] == BRULE && p.feu.i[j] <= imax && dans_cone(g, self.cell, j, cone))
            .collect();
        if cands.is_empty() || self.eau <= self.reserve {
            return false;
        }
        let j = argmax(cands, |k| p.feu.i[k]).expect("candidat");
        let l = (self.eau - self.reserve).min(debit.unwrap_or(type_moyen(self.kind).debit));
        // sous un poste de commandement, les lances sont mieux placées
        let bonus = p.ia.bonus_pc(&p.feu, j);
        p.feu.arroser(&p.carte, j, l * bonus);
        self.eau -= l;
        self.litres += l;
        self.act = Some(Act { k: "arrose", c: Some(j), d: cone.map(|c| c.0 as i64) });
        if j != self.cell {
            let g = &p.carte.g;
            self.cap_deg = (g.cy[j] - g.cy[self.cell]).atan2(g.cx[j] - g.cx[self.cell]).to_degrees();
        }
        true
    }

    fn noyer_voisins(&mut self, p: &mut Partie, cone: Option<(f64, f64)>) -> bool {
        let cells: Vec<usize> = std::iter::once(self.cell).chain(p.carte.g.voisins(self.cell)).collect();
        for j in cells {
            if p.feu.etat[j] == ETEINT && dans_cone(&p.carte.g, self.cell, j, cone) && self.eau > self.reserve + 150.0 && p.feu.noyer(j) {
                self.eau -= 150.0;
                self.act = act_c("noie", j);
                return true;
            }
        }
        false
    }

    fn pas_bull(&mut self, p: &mut Partie) {
        {
            let (c, feu) = (&p.carte, &p.feu);
            let m = self.m();
            while let Some(&i) = m.cells.first() {
                if brule(c.fuel[i]) && feu.etat[i] == 0 {
                    break;
                }
                m.cells.remove(0);
            }
            if m.cells.is_empty() {
                m.fini = true;
                self.act = act("attend");
                return;
            }
        }
        let i = self.m().cells[0];
        if std::iter::once(i).chain(p.carte.g.voisins(i)).any(|k| p.feu.etat[k] == BRULE && p.feu.i[k] > 0.5) {
            self.m().cells.remove(0); // trop près du feu : on saute cette cellule
            return;
        }
        if self.cell != i {
            if self.chemin.last() != Some(&i) && !self.aller(p, i) {
                self.m().cells.remove(0);
                return;
            }
            self.etat = "route";
            self.avancer_sol(p);
            return;
        }
        self.etat = "action";
        self.attente = 5;
        self.act = act_c("defriche", i);
    }

    // ---------------------------------------------------------------- aéronefs
    /// Dragon : hélicoptère de secours. Il vole aussi de nuit ; hélitreuillage des équipages et des blessés graves.
    fn pas_dragon(&mut self, p: &mut Partie) {
        let base = p.carte.helistation.unwrap_or(self.base);
        let Some((dest, t, cible)) = self.mission.as_ref().map(|m| (m.poste.unwrap_or(base), m.type_.clone(), m.cible)) else {
            let (bx, by) = (p.carte.g.cx[base], p.carte.g.cy[base]);
            if (self.x, self.y) != (bx, by) {
                self.etat = "retour";
                if self.voler(&p.carte.g, bx, by) {
                    self.etat = "dispo";
                }
            } else {
                self.etat = "dispo";
                self.vol = 0;
            }
            self.act = act(if self.etat == "dispo" { "au_sol" } else { "vol" });
            return;
        };
        self.etat = "action";
        let (x, y) = (p.carte.g.cx[dest], p.carte.g.cy[dest]);
        if !self.voler(&p.carte.g, x, y) {
            self.act = act("vol");
            return;
        }
        if t == "hopital" {
            p.deposer(self);
            self.mission = None;
            return;
        }
        self.secourir(p, cible, 3, 4, "sauve");
    }

    fn pas_air(&mut self, p: &mut Partie) {
        if self.kind == "dragon" {
            return self.pas_dragon(p);
        }
        let base = if self.kind == "canadair" || self.kind == "dash" { p.carte.aerodrome } else { Some(p.carte.helistation.unwrap_or(self.base)) };
        let base = base.unwrap_or(self.base);
        let (bx, by) = (p.carte.g.cx[base], p.carte.g.cy[base]);
        let cloue = !p.meteo.vol_possible(self.kind);
        let trop_long = self.vol > type_moyen(self.kind).endurance;
        if self.mission.is_none() || cloue || trop_long {
            if (self.x, self.y) != (bx, by) {
                self.etat = "retour";
                self.phase = None;
                if self.voler(&p.carte.g, bx, by) {
                    self.etat = "au_sol";
                    self.attente = if trop_long { 20 } else { 2 };
                    self.vol = 0;
                }
                return;
            }
            self.etat = if cloue { "au_sol" } else { "dispo" };
            self.act = act("au_sol");
            self.vol = 0;
            if self.kind != "canadair" {
                self.eau = self.cap;
            }
            return;
        }
        self.etat = "action";
        if self.m().type_ == "gaar" {
            return self.gaar(p);
        }
        if self.kind == "dash" {
            return self.cycle_dash(p, bx, by);
        }
        if self.eau < self.cap * 0.5 && self.phase != Some("vers_feu") {
            self.phase = Some("charge");
        }
        if self.phase.is_none() || self.phase == Some("charge") {
            if self.eau >= self.cap * 0.5 {
                self.phase = Some("vers_feu");
            } else {
                self.phase = Some("charge");
                if self.cible.is_none() {
                    let Some(pt) = p.eau_aerienne(self) else {
                        self.mission = None;
                        return;
                    };
                    self.cible = Some((p.carte.g.cx[pt], p.carte.g.cy[pt]));
                }
                let (x, y) = self.cible.expect("cible");
                if self.voler(&p.carte.g, x, y) {
                    self.phase = Some("ecope");
                    self.attente = if self.kind == "canadair" { 1 } else { 2 };
                    self.act = act_c("ecope", self.cell);
                    self.cible = None;
                } else {
                    self.act = act("vol");
                }
                return;
            }
        }
        if self.phase == Some("vers_feu") {
            if self.vise.is_none() || self.cible.is_none() {
                let pr: &Partie = p;
                let Some(v) = pr.ia.cible_largage(pr, self) else {
                    self.act = act("vol");
                    self.voler(&p.carte.g, bx, by);
                    return;
                };
                let g = &p.carte.g;
                let cx = v.iter().map(|&i| g.cx[i]).sum::<f64>() / v.len() as f64;
                let cy = v.iter().map(|&i| g.cy[i]).sum::<f64>() / v.len() as f64;
                self.vise = Some(v);
                self.cible = Some((cx, cy));
            }
            let (x, y) = self.cible.expect("cible");
            if self.voler(&p.carte.g, x, y) {
                let cells: Vec<usize> = self.vise.take().unwrap_or_default().into_iter().filter(|&i| brule(p.carte.fuel[i])).collect();
                if !cells.is_empty() {
                    let par = self.eau / cells.len() as f64;
                    for &i in &cells {
                        p.feu.arroser(&p.carte, i, par);
                    }
                    p.evenement(
                        "largage",
                        json!({"moyen": self.id, "kind": self.kind, "cells": cells, "litres": rond(self.eau), "produit": "eau", "cap": rond(self.cap_deg)}),
                    );
                    self.litres += self.eau;
                    self.largages += 1;
                    p.stats.largages += 1;
                }
                self.eau = 0.0;
                self.cible = None;
                self.phase = Some("charge");
                self.act = act("largue");
            } else {
                self.act = act("vol");
            }
        }
    }

    /// Guet aérien armé : chargé d'eau, l'avion tourne au-dessus des massifs (et repère les départs de feu).
    fn gaar(&mut self, p: &mut Partie) {
        if self.eau < self.cap {
            if self.cible.is_none() || self.phase != Some("charge") {
                let Some(pt) = p.eau_aerienne(self) else { return };
                self.phase = Some("charge");
                self.cible = Some((p.carte.g.cx[pt], p.carte.g.cy[pt]));
            }
            let (x, y) = self.cible.expect("cible");
            if self.voler(&p.carte.g, x, y) {
                self.phase = Some("ecope");
                self.attente = 1;
                self.cible = None;
            }
            self.act = act("vol");
            return;
        }
        let (pts, k) = {
            let m = self.m();
            (m.pts.clone(), m.k)
        };
        if pts.is_empty() {
            return;
        }
        self.phase = Some("gaar");
        let (x, y) = pts[k % pts.len()];
        if self.voler(&p.carte.g, x, y) {
            self.m().k = k + 1;
        }
        self.act = act("patrouille");
    }

    fn cycle_dash(&mut self, p: &mut Partie, bx: f64, by: f64) {
        if self.eau <= 0.0 {
            if self.voler(&p.carte.g, bx, by) {
                self.etat = "action";
                self.phase = Some("ecope");
                self.attente = 12;
                self.act = act("remplit");
            } else {
                self.act = act("vol");
            }
            return;
        }
        if self.vise.is_none() {
            let pr: &Partie = p;
            let v = pr.ia.ligne_retardant(pr, self);
            if v.is_empty() {
                self.mission = None;
                return;
            }
            self.cible = Some((p.carte.g.cx[v[0]], p.carte.g.cy[v[0]]));
            self.vise = Some(v);
        }
        let (x, y) = self.cible.unwrap_or((bx, by));
        if self.voler(&p.carte.g, x, y) {
            let vise = self.vise.take().unwrap_or_default();
            p.feu.larguer_retardant(&p.carte, &vise, self.eau);
            p.evenement(
                "largage",
                json!({"moyen": self.id, "kind": "dash", "cells": vise, "litres": rond(self.eau), "produit": "retardant", "cap": rond(self.cap_deg)}),
            );
            p.stats.largages += 1;
            self.litres += self.eau;
            self.largages += 1;
            self.eau = 0.0;
            self.act = act("largue");
            self.mission = None; // une ligne par mission : le commandement en redemande s'il faut
            self.joueur = false;
        } else {
            self.act = act("vol");
        }
    }

    // ------------------------------------------------------------------ export
    pub fn export(&self) -> Value {
        let m = self.mission.as_ref();
        let a = self.act.as_ref().map(|a| {
            let mut o = Map::new();
            o.insert("k".into(), json!(a.k));
            if let Some(c) = a.c {
                o.insert("c".into(), json!(c));
            }
            if let Some(d) = a.d {
                o.insert("d".into(), json!(d));
            }
            Value::Object(o)
        });
        let debut = self.trace.len().saturating_sub(16);
        json!({
            "id": self.id, "k": self.kind, "n": self.nom, "x": rond(self.x), "y": rond(self.y), "cap": rond(self.cap_deg),
            "e": self.etat, "eau": if self.cap > 0.0 { json!(rond(100.0 * self.eau / self.cap)) } else { Value::Null },
            "m": m.map(|m| m.label.clone()), "mt": m.map(|m| m.type_.clone()), "f": m.and_then(|m| m.feu),
            "p": m.and_then(|m| m.poste), "a": a, "c": self.cell, "r": self.renfort, "ph": self.phase,
            "v": self.vise.as_ref().filter(|v| !v.is_empty()), "l": rond(self.litres), "j": self.joueur, "hs": self.hs, "au": self.auto,
            "ca": self.carb_max.map(|cm| rond(100.0 * self.carb / cm)),
            "ch": self.charge.iter().map(|x| x.n).sum::<i64>(),
            "dir": m.and_then(|m| m.dir), "ax": m.and_then(|m| m.axe), "ci": m.and_then(|m| m.cible),
            "g": self.groupe, "tr": self.trace[debut..],
        })
    }
}
