//! La Gironde, extraite de la carte de web_conquete par outils/gironde (gironde.json : ne pas modifier à la main).
//!
//! Cases de 1 km, x vers l'est, y vers le sud. FUEL : combustibles (combustibles.rs) ; ALT : altitude, chr(48 + m / 4).

use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Lieu {
    pub nom: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub xy: (f64, f64),
    pub d33: bool,
}

#[derive(Deserialize)]
pub struct Route {
    pub n: u8,
    pub p: Vec<(f64, f64)>,
}

#[derive(Deserialize)]
pub struct Site {
    #[serde(rename = "type")]
    pub type_: String,
    pub nom: String,
    pub xy: (f64, f64),
}

#[derive(Deserialize)]
pub struct Eau {
    pub nom: String,
    pub xy: (f64, f64),
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Gironde {
    pub w: usize,
    pub h: usize,
    pub fuel: Vec<String>,
    pub alt: Vec<String>,
    pub lieux: Vec<Lieu>,
    pub routes: Vec<Route>,
    pub sites: Vec<Site>,
    pub eaux: Vec<Eau>,
    pub aerodrome: (String, (f64, f64)),
}

pub fn gironde() -> &'static Gironde {
    static G: OnceLock<Gironde> = OnceLock::new();
    G.get_or_init(|| serde_json::from_str(include_str!("gironde.json")).expect("gironde.json illisible"))
}
