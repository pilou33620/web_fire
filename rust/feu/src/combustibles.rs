//! Les types de sol et leur comportement au feu.
//!
//! ros  : vitesse de propagation de base, en hexagones par minute, sans vent ni pente, combustible sec
//! dur  : minutes de combustion à pleine intensité (ce que le feu met à consommer l'hexagone)
//! imax : intensité maximale (1 = pinède sèche en plein feu)
//! res  : résistance à l'eau (litres nécessaires, relatif) ; plus c'est haut, plus il faut d'eau
//! saute: aptitude à produire des brandons (sautes de feu)

use serde_json::{json, Map, Value};

pub struct Comb {
    pub code: u8,
    pub nom: &'static str,
    pub brule: bool,
    pub ros: f64,
    pub dur: f64,
    pub imax: f64,
    pub res: f64,
    pub saute: f64,
    pub col: &'static str,
}

const fn eau(code: u8, nom: &'static str, col: &'static str) -> Comb {
    Comb { code, nom, brule: false, ros: 0.0, dur: 1.0, imax: 0.0, res: 1.0, saute: 0.0, col }
}

#[allow(clippy::too_many_arguments)]
const fn vg(code: u8, nom: &'static str, ros: f64, dur: f64, imax: f64, res: f64, saute: f64, col: &'static str) -> Comb {
    Comb { code, nom, brule: true, ros, dur, imax, res, saute, col }
}

pub const COMB: [Comb; 22] = [
    eau(b'M', "Océan", "#1d5a86"),
    eau(b'L', "Étang, lac", "#3fa6dc"),
    eau(b'R', "Rivière", "#46a9dd"),
    eau(b'S', "Plage, sable", "#ead9a2"),
    eau(b'K', "Roche", "#9b958a"),
    eau(b'X', "Pare-feu (sol nu)", "#b8955f"),
    eau(b'A', "Aérodrome", "#a9a394"),
    vg(b'D', "Dune", 0.020, 6.0, 0.25, 0.6, 0.0, "#e3d39a"),
    vg(b'P', "Pinède", 0.030, 45.0, 1.00, 1.5, 1.0, "#4f8a3a"),
    vg(b'J', "Lande (ajoncs, fougères)", 0.055, 15.0, 0.80, 1.0, 0.5, "#9da35a"),
    vg(b'F', "Feuillus", 0.012, 70.0, 0.75, 1.3, 0.3, "#5d9b3a"),
    vg(b'G', "Garrigue", 0.050, 15.0, 0.75, 1.0, 0.5, "#a7a868"),
    vg(b'Q', "Maquis", 0.040, 30.0, 0.95, 1.3, 0.8, "#6f8f44"),
    vg(b'H', "Prairie, friche", 0.070, 6.0, 0.35, 0.6, 0.1, "#9cc65a"),
    vg(b'C', "Culture (maïs, céréales)", 0.045, 8.0, 0.40, 0.7, 0.1, "#d9c25a"),
    vg(b'V', "Vigne", 0.006, 10.0, 0.25, 0.6, 0.0, "#b4b660"),
    vg(b'O', "Oliveraie", 0.010, 20.0, 0.35, 0.8, 0.1, "#9aa76a"),
    vg(b'Y', "Forêt débroussaillée", 0.014, 35.0, 0.55, 1.2, 0.4, "#6f9e4c"),
    vg(b'T', "Camping sous les pins", 0.020, 30.0, 0.70, 1.4, 0.5, "#7aa24c"),
    vg(b'B', "Village, hameau", 0.008, 90.0, 0.60, 2.0, 0.2, "#d8cba8"),
    vg(b'U', "Ville", 0.004, 120.0, 0.60, 2.2, 0.2, "#d3c7a6"),
    vg(b'Z', "Zone d'activités", 0.003, 60.0, 0.50, 2.0, 0.1, "#bdb6a6"),
];

/// Le combustible d'un code ; un code inconnu compte comme de la roche (rien ne brûle).
pub fn comb(code: u8) -> &'static Comb {
    COMB.iter().find(|c| c.code == code).unwrap_or(&COMB[4])
}

pub fn brule(code: u8) -> bool {
    comb(code).brule
}

pub const EAU: &[u8] = b"MLR";
/// Les cellules où il y a des gens ou des biens.
pub const HABITE: &[u8] = b"UBTZ";
/// Une piste ou une route freine le feu (coupure de combustible).
pub const ROUTE_FREIN: [f64; 4] = [1.0, 0.75, 0.55, 0.45];

pub fn est_eau(code: u8) -> bool {
    EAU.contains(&code)
}

/// Bonus et malus de chaque type de voie (repris dans l'interface), par niveau 3, 2, 1.
pub fn voies() -> Value {
    json!({
        "3": {"nom": "Route principale",
              "bonus": ["Camions rapides (60 km/h)", "Large coupure : le feu passe mal (×0,45)", "Itinéraire des évacuations"],
              "malus": ["Bouchons : les convois d'évacuation ralentissent les camions", "Fumée : visibilité réduite, on roule moins vite",
                        "Plus de départs de feu (mégots, véhicules)", "Contourne le massif : loin du feu"]},
        "2": {"nom": "Route",
              "bonus": ["Camions rapides (50 km/h)", "Coupure : le feu passe mal (×0,55)", "Itinéraire des évacuations"],
              "malus": ["Bouchons : les convois d'évacuation ralentissent les camions", "Fumée : visibilité réduite, on roule moins vite",
                        "Plus de départs de feu (mégots, véhicules)"]},
        "1": {"nom": "Piste forestière DFCI",
              "bonus": ["Pénètre le massif : poste d'attaque au contact du feu", "Citernes DFCI aux croisements",
                        "Coupure : freine le feu (×0,75)", "Bulldozer plus rapide pour ouvrir un pare-feu"],
              "malus": ["Sable : camions lents (27 km/h)", "Étroite : à plus de 2 engins sur un tronçon, on n'avance plus qu'au pas",
                        "Piège : fermée aux camions quand elle longe un feu intense", "Interdite aux convois de civils"]},
    })
}

/// Les combustibles pour l'interface : {code: {nom, brule, col}}.
pub fn export() -> Value {
    let mut m = Map::new();
    for c in &COMB {
        m.insert((c.code as char).to_string(), json!({"nom": c.nom, "brule": c.brule, "col": c.col}));
    }
    Value::Object(m)
}
