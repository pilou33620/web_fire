//! Extrait la Gironde de la carte de web_conquete (vraie géographie, cases de 1 km) vers rust/feu/src/gironde.json.
//!
//!     cargo run --release --manifest-path outils/gironde/Cargo.toml
//!
//! À relancer seulement si la carte de web_conquete change (sa version est fixée dans Cargo.toml). Le jeu n'a pas
//! besoin de web_conquete : il lit rust/feu/src/gironde.json (compilé dans le moteur).
//! Le terrain de web_conquete (une carte d'époque : landes, marais) est traduit en combustibles d'aujourd'hui :
//! la lande de Gascogne est devenue la pinède des Landes.
use conquete::geo::{self, fbm, GeoMap};
use conquete::py::{self, json, Val};
use conquete::{dict, tup};
use std::collections::HashSet;

const MARGE: i64 = 3; // km autour de la Gironde
const GEOGRAPHIE: i64 = 3; // la géographie 3 de web_conquete (toute la France), celle d'où gironde.json a été tiré

// Sites que web_conquete n'a pas : (type, nom, lat, lon)
const SITES: [(&str, &str, f64, f64); 14] = [
    ("camping", "Camping de Lacanau-Océan", 45.000, -1.195), ("camping", "Camping de Carcans-Plage", 45.080, -1.185),
    ("camping", "Camping d'Hourtin-Plage", 45.220, -1.160), ("camping", "Camping de Montalivet", 45.375, -1.145),
    ("camping", "Campings du Pyla", 44.585, -1.205), ("camping", "Camping de Soulac", 45.495, -1.120),
    ("usine", "Papeterie de Facture", 44.640, -0.980), ("usine", "Dépôt pétrolier d'Ambès (Seveso)", 44.930, -0.530),
    ("usine", "Poudrerie de Saint-Médard (Seveso)", 44.880, -0.760),
    ("reserve", "Réserve naturelle de l'étang de Cousseau", 45.060, -1.165), ("reserve", "Prés salés d'Arès", 44.770, -1.120),
    ("patrimoine", "Dune du Pilat", 44.589, -1.213), ("patrimoine", "Château de Roquetaillade", 44.490, -0.250),
    ("patrimoine", "Abbaye de La Sauve-Majeure", 44.770, -0.315),
];
const AERODROME: (&str, f64, f64) = ("Aéroport de Bordeaux-Mérignac", 44.830, -0.700);

fn main() {
    let g = GeoMap::new(GEOGRAPHIE);
    let d33 = g.dept_codes.iter().position(|c| c == "33").expect("pas de Gironde dans la carte") as u8 + 1;
    let cs: Vec<i64> = (0..g.W * g.H).filter(|&c| g.dept[c as usize] == d33).collect();
    let x0 = cs.iter().map(|c| c % g.W).min().unwrap() - MARGE;
    let x1 = cs.iter().map(|c| c % g.W).max().unwrap() + MARGE + 1;
    let y0 = cs.iter().map(|c| c / g.W).min().unwrap() - MARGE;
    let y1 = cs.iter().map(|c| c / g.W).max().unwrap() + MARGE + 1;
    let (larg, haut) = (x1 - x0, y1 - y0);

    let mut landes = vec![0u8; (g.W * g.H) as usize];
    let anneau: Vec<geo::Pt> = geo::LANDES_POLY.iter().map(|&(la, lo)| g.xy(la, lo)).collect();
    g._raster_ring(&anneau, &mut |c: usize| landes[c] = 1);
    let (mut fuel, mut alt) = (Vec::<Val>::new(), Vec::<Val>::new());
    for y in y0..y1 {
        let (mut lf, mut la) = (String::new(), String::new());
        for x in x0..x1 {
            let c = g.cell(x as f64, y as f64);
            assert!(c >= 0, "case ({x}, {y}) hors de la carte de web_conquete");
            let c = c as usize;
            let (t, riv) = (g.terrain[c] as i64, g.river[c]);
            let (lat, lon) = (g.lat_of(y as f64 + 0.5), g.lon_of(x as f64 + 0.5));
            let n = fbm(x as f64 / 3.0, y as f64 / 3.0, 41, 3); // le mélange fin des combustibles
            let e = GeoMap::_elev3(lat, lon) + (fbm(x as f64 / 22.0, y as f64 / 22.0, 11, 4) - 0.5) * 0.42;
            let e = if e > 0.0 { e } else { 0.0 };
            let mut a = 8.0 + 240.0 * e;
            let mut f = if t == geo::T_OCEAN {
                a = 0.0;
                'M'
            } else if t == geo::T_EAU {
                a = 1.0;
                'L'
            } else if riv != 0 && riv <= 2 {
                a -= 6.0;
                'R'
            } else if t == geo::T_DUNES {
                a += 35.0;
                'D'
            } else if t == geo::T_FORET || t == geo::T_LANDE {
                let pin = landes[c] != 0 || lon < -0.6;
                if pin {
                    if n > 0.72 { 'J' } else if n < 0.18 { 'C' } else { 'P' }
                } else if n > 0.3 { 'F' } else { 'P' }
            } else if t == geo::T_MARAIS {
                if landes[c] != 0 { 'J' } else { 'H' }
            } else if t == geo::T_VIGNE {
                'V'
            } else if t == geo::T_COLLINES {
                if n > 0.5 { 'F' } else { 'C' }
            } else if n > 0.68 { 'F' } else if n < 0.3 { 'H' } else { 'C' };
            if riv == 3 && "CHV".contains(f) {
                f = 'F'; // ripisylve des petites rivières
            }
            lf.push(f);
            la.push(char::from(48 + py::round(a / 4.0).clamp(0, 74) as u8)); // 4 m par cran
        }
        fuel.push(lf.into());
        alt.push(la.into());
    }

    let dedans = |x: f64, y: f64| x0 as f64 <= x && x < x1 as f64 && y0 as f64 <= y && y < y1 as f64;
    let km = |x: f64, y: f64| tup![py::round_n(x - x0 as f64, 2), py::round_n(y - y0 as f64, 2)];
    let (mut lieux, mut ids) = (Vec::<Val>::new(), HashSet::new());
    for n in g.nodes.iter() {
        let (x, y) = (n.at("x").f(), n.at("y").f());
        if dedans(x, y) {
            let genre = if n.at("type") == "fort" { Val::str("patrimoine") } else { n.at("type") };
            lieux.push(dict! {"nom" => n.at("name"), "type" => genre, "xy" => km(x, y), "d33" => n.at("dept") == "33"});
            ids.insert(n.at("id").i());
        }
    }
    // str(v) == 'True' du Python : le booléen vrai, ou le texte « True » d'un ancien cache
    let vrai = |v: &Val| matches!(v, Val::Bool(true)) || *v == "True";
    let mut routes = Vec::<Val>::new();
    for e in g.edges.iter() {
        if ids.contains(&e.at("a").i()) && ids.contains(&e.at("b").i()) && !vrai(&e.get("ferry")) {
            let p: Val = e.at("pts").iter().map(|q| { let (x, y) = q.xy(); km(x, y) }).collect();
            routes.push(dict! {"n" => if vrai(&e.at("royal")) { 3 } else { 2 }, "p" => p});
        }
    }
    let sites: Vec<Val> = SITES.iter().map(|&(t, nom, la, lo)| { let (x, y) = g.xy(la, lo); dict! {"type" => t, "nom" => nom, "xy" => km(x, y)} }).collect();
    let mut eaux = Vec::<Val>::new();
    for w in g.src.at("waters").iter() {
        if w.at("name").truthy() {
            let pts: Vec<geo::Pt> = w.at("ring").iter().map(|p| { let (lo, la) = p.xy(); g.xy(la, lo) }).collect();
            let k = pts.len() as f64;
            let (cx, cy) = (py::sum(pts.iter().map(|p| p.0)) / k, py::sum(pts.iter().map(|p| p.1)) / k);
            if dedans(cx, cy) {
                eaux.push(dict! {"nom" => w.at("name"), "xy" => km(cx, cy)});
            }
        }
    }
    let (ax, ay) = g.xy(AERODROME.1, AERODROME.2);
    let (nl, nr, ne) = (lieux.len(), routes.len(), eaux.len());
    let d = dict! {"W" => larg, "H" => haut, "FUEL" => fuel, "ALT" => alt, "LIEUX" => lieux, "ROUTES" => routes,
                   "SITES" => sites, "EAUX" => eaux, "AERODROME" => tup![AERODROME.0, km(ax, ay)]};
    let chemin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rust/feu/src/gironde.json");
    std::fs::write(&chemin, json::dumps_compact(&d) + "\n").unwrap_or_else(|e| panic!("{} : {e}", chemin.display()));
    println!("{larg} × {haut} km, {nl} lieux, {nr} routes, {ne} plans d'eau");
}
