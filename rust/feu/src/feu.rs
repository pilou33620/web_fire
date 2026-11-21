//! La propagation du feu, minute par minute, d'hexagone en hexagone.
//!
//! Modèle simplifié inspiré de Rothermel : pour un hexagone qui brûle (intensité I) et un voisin combustible,
//!   vitesse = ros(combustible) × vent × pente × humidité × sécheresse × coupure × (1 − mouillé) × retardant × défense
//!   probabilité d'allumage dans la minute = 1 − exp(−vitesse × I)
//! Vent : exp(0,045 × V × cos θ) (θ : angle entre le vent et la direction de propagation) ; pente : exp(3 × dénivelé / 300 m).
//! Les sautes de feu (brandons) partent des hexagones intenses quand le vent dépasse 30 km/h.
//! L'eau abaisse l'intensité et mouille ; le retardant freine fortement l'allumage pendant des heures.

use indexmap::IndexSet;

use crate::carte::Carte;
use crate::combustibles::{brule, comb, ROUTE_FREIN};
use crate::hasard::Hasard;
use crate::meteo::Meteo;

pub const INTACT: u8 = 0;
pub const BRULE: u8 = 1;
pub const BRULE_FINI: u8 = 2;
pub const ETEINT: u8 = 3;
pub const NOYE: u8 = 4;

#[derive(Clone, Debug)]
pub struct Depart {
    pub id: usize,
    pub cell: usize,
    pub t: i64,
    pub cause: String,
    pub detecte: bool,
    pub t_detect: Option<i64>,
}

pub struct Feu {
    pub etat: Vec<u8>,
    pub i: Vec<f64>,
    pub prog: Vec<f64>,
    pub mouille: Vec<f64>,
    /// minutes de retardant restantes
    pub ret: Vec<i64>,
    /// direction (0..5) d'où le feu est arrivé, -1 : départ ou saute
    pub dfrom: Vec<i64>,
    /// le départ de feu à l'origine (-1 : jamais brûlé)
    pub foyer: Vec<i64>,
    pub t_ign: Vec<i64>,
    /// camions en défense sur l'hexagone (remis à zéro à chaque minute)
    pub defense: Vec<i64>,
    pub brulant: IndexSet<usize>,
    pub eteints: IndexSet<usize>,
    pub departs: Vec<Depart>,
    pub nouvelles_sautes: Vec<usize>,
    pub litres: f64,
    pub litres_ret: f64,
    pub ha_hex: f64,
}

impl Feu {
    pub fn new(c: &Carte) -> Feu {
        let n = c.g.n;
        Feu {
            etat: vec![INTACT; n],
            i: vec![0.0; n],
            prog: vec![0.0; n],
            mouille: vec![0.0; n],
            ret: vec![0; n],
            dfrom: vec![-1; n],
            foyer: vec![-1; n],
            t_ign: vec![0; n],
            defense: vec![0; n],
            brulant: IndexSet::new(),
            eteints: IndexSet::new(),
            departs: Vec::new(),
            nouvelles_sautes: Vec::new(),
            litres: 0.0,
            litres_ret: 0.0,
            ha_hex: c.g.hex_m * c.g.hex_m * 3f64.sqrt() / 2.0 / 10000.0,
        }
    }

    // ------------------------------------------------------------------ allumer
    pub fn peut_bruler(&self, c: &Carte, i: usize) -> bool {
        brule(c.fuel[i]) && matches!(self.etat[i], INTACT | ETEINT | NOYE) && self.prog[i] < 0.92
    }

    /// Allume i ; `foyer` None : c'est un nouveau départ de feu.
    pub fn allumer(&mut self, c: &Carte, i: usize, d: i64, foyer: Option<i64>, cause: &str, t: i64) -> bool {
        if !self.peut_bruler(c, i) {
            return false;
        }
        let foyer = match foyer {
            Some(f) => f,
            None => {
                let id = self.departs.len();
                self.departs.push(Depart { id, cell: i, t, cause: cause.to_string(), detecte: false, t_detect: None });
                id as i64
            }
        };
        self.etat[i] = BRULE;
        self.i[i] = 0.15;
        self.dfrom[i] = d;
        self.foyer[i] = foyer;
        self.t_ign[i] = t;
        self.brulant.insert(i);
        self.eteints.shift_remove(&i);
        true
    }

    // ------------------------------------------------------------------ un pas d'une minute
    pub fn pas(&mut self, c: &Carte, m: &Meteo, rnd: &mut Hasard, t: i64) {
        let g = &c.g;
        let (vx, vy) = m.vers();
        let v = m.vent;
        let fsech = m.f_secheresse();
        let mut allumes: Vec<(usize, i64, i64)> = Vec::new();
        let brulant: Vec<usize> = self.brulant.iter().copied().collect();
        for &i in &brulant {
            let ii = self.i[i];
            if ii <= 0.02 {
                continue;
            }
            for vo in &g.vois[i] {
                let j = vo.j;
                if !self.peut_bruler(c, j) {
                    continue;
                }
                let cj = comb(c.fuel[j]);
                let a = vo.ux * vx + vo.uy * vy;
                let fw = (0.045 * v * a).exp();
                let pente = ((c.alt[j] - c.alt[i]) / g.hex_m).clamp(-0.3, 0.45);
                let fs = (3.0 * pente).exp();
                let fm = m.f_humidite(self.mouille[j]);
                let frein = ROUTE_FREIN[c.route[j] as usize];
                let fr = if self.ret[j] > 0 { 0.12 } else { 1.0 };
                let fd = 0.3f64.powi(self.defense[j].min(3) as i32);
                let reste = 1.0 - self.prog[j];
                let r = cj.ros * g.ech * fw * fs * fm * fsech * frein * fr * fd * (1.0 - self.mouille[j]) * (0.4 + 0.6 * reste);
                let p = 1.0 - (-r * ii).exp();
                if p > 0.0 && rnd.random() < p {
                    allumes.push((j, ((vo.d + 3) % 6) as i64, self.foyer[i]));
                }
            }
            // sautes de feu
            if v > 30.0 && ii > 0.6 {
                let taux = 0.0016 * (v - 30.0) / 10.0 * ii * comb(c.fuel[i]).saute;
                if rnd.random() < taux {
                    let dist = 2.0 + rnd.expovariate(1.0 / (v / 14.0).max(1.0));
                    let ang = vy.atan2(vx) + rnd.gauss(0.0, 0.3);
                    let x = g.cx[i] + ang.cos() * dist * 300.0;
                    let y = g.cy[i] + ang.sin() * dist * 300.0;
                    if let Some(k) = g.sous(x, y) {
                        if self.peut_bruler(c, k) && self.mouille[k] < 0.3 && self.ret[k] <= 0 && rnd.random() < m.f_humidite(self.mouille[k]) {
                            allumes.push((k, -1, self.foyer[i]));
                            self.nouvelles_sautes.push(k);
                        }
                    }
                }
            }
        }
        // intensité, consommation, extinction naturelle
        let mut fini = Vec::new();
        for &i in &brulant {
            let ci = comb(c.fuel[i]);
            let fm = m.f_humidite(self.mouille[i]);
            let imax = ci.imax * (0.45 + 0.55 * fm) * (0.8 + 0.2 * fsech) * (1.0 - 0.85 * self.mouille[i]);
            self.i[i] += (imax - self.i[i]) * 0.25;
            self.prog[i] += (0.35 + 0.65 * self.i[i]) * g.ech / ci.dur;
            if self.prog[i] >= 1.0 {
                fini.push((i, BRULE_FINI));
            } else if self.i[i] < 0.05 {
                fini.push((i, ETEINT));
            }
        }
        for (i, e) in fini {
            self.brulant.shift_remove(&i);
            self.etat[i] = e;
            self.i[i] = 0.0;
            if e == BRULE_FINI {
                self.prog[i] = 1.0;
            } else {
                self.eteints.insert(i);
            }
        }
        for (j, d, f) in allumes {
            if self.etat[j] != BRULE {
                if matches!(self.etat[j], ETEINT | NOYE) {
                    self.eteints.shift_remove(&j);
                }
                self.allumer(c, j, d, Some(f), "inconnue", t);
            }
        }
        // reprises : un hexagone éteint mais pas noyé peut se rallumer (braises)
        let eteints: Vec<usize> = self.eteints.iter().copied().collect();
        for i in eteints {
            if self.etat[i] == ETEINT && self.mouille[i] < 0.08 && rnd.random() < 0.0012 * (1.0 + v / 30.0) * fsech {
                let f = self.foyer[i];
                self.allumer(c, i, -1, Some(f), "inconnue", t);
            }
        }
        // séchage : l'eau s'évapore, le retardant tient des heures
        let mut evap = if !m.nuit { 0.985 } else { 0.995 };
        if m.pluie > 0.2 {
            evap = 1.0;
        }
        for i in 0..g.n {
            if self.mouille[i] > 0.001 {
                self.mouille[i] *= evap;
            }
            if m.pluie > 0.0 {
                self.mouille[i] = (self.mouille[i] + 0.02 * m.pluie).min(1.0);
            }
            if self.ret[i] > 0 {
                self.ret[i] -= 1;
            }
            self.defense[i] = 0;
        }
    }

    // ------------------------------------------------------------------ lutte
    /// Eau sur un hexagone : baisse l'intensité, mouille. Rend les litres utilisés.
    pub fn arroser(&mut self, c: &Carte, i: usize, litres: f64) -> f64 {
        let ci = comb(c.fuel[i]);
        if !ci.brule {
            return 0.0;
        }
        // shortcut: pas ramené à la taille de l'hexagone (le jeu resterait injouable avec 16 CCF)
        let k = litres / (6000.0 * ci.res);
        self.mouille[i] = (self.mouille[i] + k * 0.6).min(1.0);
        if self.etat[i] == BRULE {
            // un feu très intense « boit » l'eau : le largage est moins efficace sur une tête violente
            self.i[i] = (self.i[i] - k * (1.25 - 0.6 * self.i[i])).max(0.0);
        }
        self.litres += litres;
        litres
    }

    pub fn larguer_retardant(&mut self, c: &Carte, cells: &[usize], litres: f64) {
        let par = litres / cells.len().max(1) as f64;
        for &i in cells {
            if brule(c.fuel[i]) {
                self.ret[i] = self.ret[i].max(240);
                if self.etat[i] == BRULE {
                    self.i[i] = (self.i[i] - par / 9000.0).max(0.0);
                }
            }
        }
        self.litres_ret += litres;
    }

    pub fn noyer(&mut self, i: usize) -> bool {
        if self.etat[i] == ETEINT {
            self.etat[i] = NOYE;
            self.eteints.shift_remove(&i);
            self.mouille[i] = self.mouille[i].max(0.5);
            return true;
        }
        false
    }

    /// Un bulldozer met le sol à nu : plus rien à brûler.
    pub fn defricher(&mut self, c: &mut Carte, i: usize) -> bool {
        if self.etat[i] == BRULE {
            return false;
        }
        c.fuel[i] = b'X';
        true
    }

    // ------------------------------------------------------------------ bilan
    pub fn surface_ha(&self, foyer: Option<i64>) -> f64 {
        let mut n = 0.0;
        for i in 0..self.etat.len() {
            if self.etat[i] != INTACT && foyer.map_or(true, |f| self.foyer[i] == f) {
                n += if matches!(self.etat[i], BRULE | BRULE_FINI) { 1.0 } else { self.prog[i] };
            }
        }
        n * self.ha_hex
    }
}
