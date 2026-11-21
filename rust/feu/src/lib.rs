//! Moteur du simulateur de feux de forêt : la Gironde en hexagones de 1 km, la météo, la propagation
//! du feu, les moyens de lutte, le commandement automatique et les évacuations. Sans réseau : il sert
//! au serveur (web_fire) comme au navigateur (feu_wasm).
//!
//! Porte d'entrée : `Partie::new(...)`, `.pas()`, `.export_carte()`, `.export_etat()`, `.action(&json)` ;
//! `Sim` y ajoute les réglages et les actions « nouvelle » et « avancer », communes au serveur et à la page.

pub mod carte;
pub mod combustibles;
pub mod evacuation;
pub mod feu;
pub mod gironde;
pub mod hasard;
pub mod hexa;
pub mod ia;
pub mod meteo;
pub mod moyens;
pub mod ordres;
pub mod partie;

use serde_json::{json, Map, Value};

pub use partie::{Partie, LIMITES, MOYENS_DEFAUT};

// ---------------------------------------------------------------------- petits outils

/// round() de Python : à l'entier pair le plus proche en cas d'égalité.
pub fn rond(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// round(x, n) de Python, pour l'affichage.
pub fn arrondi(x: f64, n: i32) -> f64 {
    let k = 10f64.powi(n);
    (x * k).round_ties_even() / k
}

/// Le premier élément de plus petite clé (min de Python).
pub fn argmin<T: Copy, K: PartialOrd>(it: impl IntoIterator<Item = T>, mut f: impl FnMut(T) -> K) -> Option<T> {
    let mut best: Option<(K, T)> = None;
    for x in it {
        let k = f(x);
        if best.as_ref().map_or(true, |(bk, _)| k < *bk) {
            best = Some((k, x));
        }
    }
    best.map(|(_, x)| x)
}

/// Le premier élément de plus grande clé (max de Python).
pub fn argmax<T: Copy, K: PartialOrd>(it: impl IntoIterator<Item = T>, mut f: impl FnMut(T) -> K) -> Option<T> {
    let mut best: Option<(K, T)> = None;
    for x in it {
        let k = f(x);
        if best.as_ref().map_or(true, |(bk, _)| k > *bk) {
            best = Some((k, x));
        }
    }
    best.map(|(_, x)| x)
}

/// Tri stable par clé (sort de Python), clés flottantes permises.
pub fn trier<T, K: PartialOrd>(v: &mut [T], mut f: impl FnMut(&T) -> K) {
    v.sort_by(|a, b| f(a).partial_cmp(&f(b)).unwrap_or(std::cmp::Ordering::Equal));
}

/// str.capitalize() de Python.
pub fn majuscule(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Un flottant écrit comme Python l'écrit (95.0, 12.5).
pub fn repr(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e16 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

/// int() de Python sur une valeur JSON : nombre (tronqué), texte d'entier, booléen.
pub fn entier(v: &Value) -> Result<i64, String> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f.trunc() as i64)).ok_or_else(|| format!("entier invalide : {n}")),
        Value::String(s) => s.trim().parse().map_err(|_| format!("invalid literal for int(): '{s}'")),
        Value::Bool(b) => Ok(*b as i64),
        _ => Err(format!("entier attendu : {v}")),
    }
}

/// float() de Python sur une valeur JSON.
pub fn nombre(v: &Value) -> Result<f64, String> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| format!("nombre invalide : {n}")),
        Value::String(s) => s.trim().parse().map_err(|_| format!("could not convert string to float: '{s}'")),
        Value::Bool(b) => Ok(*b as i64 as f64),
        _ => Err(format!("nombre attendu : {v}")),
    }
}

/// Valeur de vérité de Python (0, "", null, [] et {} sont faux).
pub fn vrai(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---------------------------------------------------------------------- simulation

/// Minutes de jeu par seconde réelle.
pub const VITESSES: [f64; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];

/// Ce que l'interface affiche dans ses réglages : scénarios, thèmes, bornes et valeurs par défaut des moyens.
pub fn infos() -> Value {
    let mut sc = Map::new();
    for code in meteo::CODES {
        sc.insert(code.to_string(), json!(meteo::scenario(code).map(|s| s.nom).unwrap_or("")));
    }
    let mut lim = Map::new();
    let mut def = Map::new();
    for &(k, a, b) in LIMITES {
        lim.insert(k.to_string(), json!([a, b]));
    }
    for &(k, v) in MOYENS_DEFAUT {
        def.insert(k.to_string(), json!(v));
    }
    json!({"scenarios": sc, "themes": {"gironde": "Gironde"}, "limites": lim, "defaut": def,
           "vitesses": [0.25, 0.5, 1, 2, 4, 8, 16]})
}

fn reglages_defaut() -> Value {
    let mut moy = Map::new();
    for &(k, v) in MOYENS_DEFAUT {
        moy.insert(k.to_string(), json!(v));
    }
    json!({"graine": 7, "theme": "gironde", "scenario": "canicule", "moyens": moy, "doctrine": "equilibree"})
}

/// La partie en cours et ses réglages ; `id` change à chaque nouvelle partie (la page recharge la carte).
pub struct Sim {
    pub partie: Partie,
    pub reglages: Value,
    pub id: u64,
}

impl Sim {
    pub fn new(id: u64) -> Sim {
        let reglages = reglages_defaut();
        let partie = Sim::creer(&reglages).expect("réglages par défaut");
        Sim { partie, reglages, id }
    }

    fn creer(r: &Value) -> Result<Partie, String> {
        let graine = r.get("graine").map(entier).transpose()?.unwrap_or(1);
        let mut moyens = partie::Config::new();
        if let Some(m) = r.get("moyens").and_then(Value::as_object) {
            for (k, v) in m {
                moyens.insert(k.clone(), entier(v)?);
            }
        }
        let texte = |cle: &str, defaut: &'static str| r.get(cle).and_then(Value::as_str).unwrap_or(defaut).to_string();
        Ok(Partie::new(graine, &texte("theme", "gironde"), &texte("scenario", "canicule"), &moyens, &texte("doctrine", "equilibree"), true))
    }

    /// Nouvelle partie : les réglages donnés remplacent les anciens, clé par clé.
    pub fn nouvelle(&mut self, r: &Value, id: u64) -> Result<(), String> {
        let mut regl = self.reglages.as_object().cloned().unwrap_or_default();
        match r {
            Value::Object(o) => regl.extend(o.clone()),
            Value::Null => {}
            _ => return Err("réglages invalides".into()),
        }
        let graine = match regl.get("graine") {
            Some(v) if vrai(v) => entier(v)?,
            _ => 1,
        };
        regl.insert("graine".into(), json!(graine));
        let regl = Value::Object(regl);
        self.partie = Sim::creer(&regl)?;
        self.reglages = regl;
        self.id = id;
        Ok(())
    }

    pub fn etat(&self, depuis: i64, vitesse: f64, pause: bool) -> Value {
        let mut e = self.partie.export_etat(depuis);
        e["vitesse"] = json!(vitesse);
        e["pause"] = json!(pause);
        e["partie"] = json!(self.id);
        e["reglages"] = self.reglages.clone();
        e
    }

    /// Les actions de la page sauf vitesse et pause (que gèrent le serveur ou la page).
    pub fn action(&mut self, a: &Value, id: u64) -> Result<Value, String> {
        match a.get("type").and_then(Value::as_str) {
            Some("nouvelle") => {
                self.nouvelle(a.get("reglages").unwrap_or(&Value::Null), id)?;
                Ok(json!({"ok": true, "partie": self.id}))
            }
            Some("avancer") => {
                let n = a.get("minutes").map(entier).transpose()?.unwrap_or(10).clamp(1, 600);
                for _ in 0..n {
                    self.partie.pas();
                }
                Ok(json!({"ok": true}))
            }
            _ => self.partie.action(a),
        }
    }
}
