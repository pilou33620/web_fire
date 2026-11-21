//! La météo du feu : température, humidité, vent (direction, force, rafales, bascule), nuit, orages, pluie.
//!
//! Le vent est donné comme les météorologues : `de` = d'où il vient, en degrés (0 = nord, 90 = est).
//! Le feu, lui, est poussé vers `vers()` (vecteur unitaire en x est, y sud).

use serde_json::{json, Value};

use crate::arrondi;
use crate::hasard::Hasard;
use crate::rond;

/// Un scénario : température max, humidité min, vent moyen (km/h), direction d'où vient le vent,
/// sécheresse 0..1, fréquence des départs, bascule du vent (minute, nouvelle direction, durée), orage (de, à).
#[derive(Clone, Debug)]
pub struct Scenario {
    pub nom: &'static str,
    pub tmax: f64,
    pub hrmin: f64,
    pub vent: f64,
    pub de: f64,
    pub sech: f64,
    pub departs: f64,
    pub bascule: Option<(f64, f64, f64)>,
    pub orage: Option<(i64, i64)>,
}

pub const CODES: [&str; 5] = ["calme", "chaud", "canicule", "orage", "mistral"];
pub const NIVEAUX: [&str; 6] = ["Faible", "Léger", "Modéré", "Sévère", "Très sévère", "Exceptionnel"];

pub fn scenario(code: &str) -> Option<Scenario> {
    let s = |nom, tmax, hrmin, vent, de, sech, departs| Scenario { nom, tmax, hrmin, vent, de, sech, departs, bascule: None, orage: None };
    Some(match code {
        "calme" => s("Journée d'été ordinaire", 28.0, 40.0, 12.0, 290.0, 0.35, 0.5),
        "chaud" => s("Forte chaleur, vent modéré", 34.0, 28.0, 22.0, 300.0, 0.6, 1.0),
        "canicule" => Scenario {
            bascule: Some((17.0 * 60.0, 250.0, 30.0)),
            ..s("Canicule et vent d'est (type Landiras 2022)", 40.0, 18.0, 35.0, 95.0, 0.95, 1.6)
        },
        "orage" => Scenario { orage: Some((15 * 60, 19 * 60)), ..s("Orages secs : la foudre frappe la forêt", 35.0, 25.0, 20.0, 220.0, 0.75, 0.7) },
        "mistral" => s("Mistral violent", 33.0, 20.0, 55.0, 330.0, 0.85, 1.3),
        _ => return None,
    })
}

pub struct Meteo {
    pub sc: Scenario,
    pub code: &'static str,
    rnd: Hasard,
    /// minutes depuis minuit du jour 1
    pub t: i64,
    pub de: f64,
    pub de_base: f64,
    pub derive: f64,
    pub rafale: f64,
    pub pluie: f64,
    pub orage: bool,
    pub sech: f64,
    pub temp: f64,
    pub hr: f64,
    pub vent: f64,
    pub rafales: f64,
    pub nuit: bool,
    pub hcomb: f64,
    pub danger: f64,
    pub niveau: &'static str,
}

impl Meteo {
    pub fn new(code: &str, graine: i64, heure0: i64) -> Meteo {
        let code: &'static str = CODES.iter().find(|c| **c == code).copied().unwrap_or("chaud");
        let sc = scenario(code).expect("scénario connu");
        let mut m = Meteo {
            de: sc.de,
            de_base: sc.de,
            sech: sc.sech,
            sc,
            code,
            rnd: Hasard::new(graine.wrapping_mul(31).wrapping_add(7)),
            t: heure0,
            derive: 0.0,
            rafale: 0.0,
            pluie: 0.0,
            orage: false,
            temp: 0.0,
            hr: 0.0,
            vent: 0.0,
            rafales: 0.0,
            nuit: false,
            hcomb: 0.0,
            danger: 0.0,
            niveau: NIVEAUX[0],
        };
        m.maj();
        m
    }

    /// Une minute.
    pub fn pas(&mut self) {
        self.t += 1;
        // direction : marche au hasard lente autour de la direction du jour, plus une éventuelle bascule
        self.derive += self.rnd.gauss(0.0, 0.6) - self.derive * 0.01;
        if let Some((tb, de2, duree)) = self.sc.bascule {
            let h = self.minute_du_jour() as f64;
            if tb <= h && h <= tb + duree {
                let k = (h - tb) / duree;
                self.de_base = self.sc.de + ((de2 - self.sc.de + 540.0).rem_euclid(360.0) - 180.0) * k;
            }
        }
        self.de = (self.de_base + self.derive).rem_euclid(360.0);
        // rafales
        self.rafale += self.rnd.gauss(0.0, 0.25) - self.rafale * 0.15;
        // orage : de telle heure à telle heure, avec de la pluie à la fin
        self.orage = false;
        if let Some((a, b)) = self.sc.orage {
            let h = self.minute_du_jour();
            if a <= h && h <= b {
                self.orage = true;
                if h > b - 50 {
                    self.pluie = (self.pluie + 0.03).min(1.0);
                }
            }
        }
        self.pluie = (self.pluie - 0.006).max(0.0);
        self.maj();
    }

    pub fn maj(&mut self) {
        let sc = &self.sc;
        let h = self.minute_du_jour() as f64 / 60.0;
        let jour = ((h - 9.5) / 24.0 * 2.0 * std::f64::consts::PI).sin(); // max vers 15 h 30
        self.temp = sc.tmax - 9.0 + 9.0 * jour - 6.0 * self.pluie;
        self.hr = (sc.hrmin + 32.0 * (1.0 - jour) / 2.0 * 1.6 + 45.0 * self.pluie).min(100.0);
        let f_jour = 0.75 + 0.35 * jour.max(0.0);
        self.vent = (sc.vent * f_jour * (1.0 + 0.12 * self.rafale) + if self.orage { 10.0 } else { 0.0 }).max(0.0);
        self.rafales = self.vent * (1.35 + 0.1 * self.rafale.abs());
        self.nuit = !(6.5..21.5).contains(&h);
        // humidité du combustible mort (%), indice de danger 0..1, niveau
        self.hcomb = (3.0 + 0.22 * self.hr - 0.12 * (self.temp - 20.0) + 12.0 * self.pluie).max(3.0);
        let danger = (1.25 - self.hcomb / 25.0) * (0.55 + 0.45 * self.sech) * (1.0 + self.vent / 60.0);
        self.danger = (danger / 1.6).clamp(0.0, 1.0);
        self.niveau = NIVEAUX[((self.danger * 6.0) as usize).min(5)];
    }

    pub fn minute_du_jour(&self) -> i64 {
        self.t.rem_euclid(1440)
    }

    pub fn heure(&self) -> String {
        let m = self.minute_du_jour();
        format!("{:02}:{:02}", m / 60, m % 60)
    }

    pub fn jour(&self) -> i64 {
        self.t.div_euclid(1440) + 1
    }

    pub fn vers(&self) -> (f64, f64) {
        let a = self.de.to_radians();
        (-a.sin(), a.cos())
    }

    /// Facteur de propagation dû à l'humidité du combustible (0 : trop humide pour brûler).
    pub fn f_humidite(&self, mouille: f64) -> f64 {
        let m = self.hcomb + 25.0 * mouille;
        (1.25 - m / 25.0).clamp(0.0, 1.0)
    }

    pub fn f_secheresse(&self) -> f64 {
        0.6 + 0.6 * self.sech
    }

    /// Les avions ne volent pas de nuit ; au-delà d'un certain vent, ils restent au sol.
    pub fn vol_possible(&self, kind: &str) -> bool {
        if self.nuit {
            return false;
        }
        let lim = match kind {
            "canadair" => 75.0,
            "dash" => 70.0,
            "helico" => 85.0,
            _ => 80.0,
        };
        self.rafales < lim
    }

    pub fn export(&self) -> Value {
        json!({
            "heure": self.heure(), "jour": self.jour(), "t": self.t, "temp": arrondi(self.temp, 1), "hr": rond(self.hr),
            "vent": rond(self.vent), "rafales": rond(self.rafales), "de": rond(self.de), "nuit": self.nuit,
            "pluie": arrondi(self.pluie, 2), "orage": self.orage, "sech": arrondi(self.sech, 2), "hcomb": arrondi(self.hcomb, 1),
            "danger": arrondi(self.danger, 2), "niveau": self.niveau, "scenario": self.code, "nomScenario": self.sc.nom,
            "volCanadair": self.vol_possible("canadair"), "volHelico": self.vol_possible("helico"),
        })
    }
}
