//! Les ordres du joueur, quand il prend la place du commandant des opérations de secours (COS).
//!
//! Un moyen qui reçoit un ordre passe « sous vos ordres » : l'IA ne le réaffecte plus tant que l'ordre n'est pas
//! terminé ou que vous ne le lui rendez pas (« Rendre à l'IA »).
//!
//! Les camions se commandent par groupe (GIFF : 4 CCF et le VLHR du chef, qui suit). Quatre modes d'action, chacun avec
//! un cône d'arrosage orienté (direction en degrés, x vers l'est, y vers le sud, comme les caps des moyens) :
//!   arroser   — le groupe se poste en ligne, face à la direction donnée, et n'arrose que dans son cône ;
//!   ligne     — ligne d'arrêt : les camions s'étalent sur la ligne tracée, arrosent le front ou mouillent devant eux ;
//!   proteger  — protection à 360° d'un site (maisons, camping) ;
//!   flanc     — attaque de flanc : le groupe remonte seul le flanc, de l'arrière vers la tête, en gardant son orientation.
//! Les avions : larguer (point visé et axe du largage), retardant (ligne du Dash selon l'axe).
//! Ordres simples : poster, attaquer, defendre, surveiller, pare_feu (bulldozer), retour, liberer.

use serde_json::{json, Map, Value};

use crate::combustibles::{brule, est_eau};
use crate::feu::BRULE;
use crate::hexa::Grille;
use crate::ia::racine;
use crate::moyens::{dans_cone, vitesse_sol, Mission, Moyen, CONE};
use crate::partie::Partie;
use crate::{argmax, argmin, rond, trier};

fn ordres_permis(kind: &str) -> &'static [&'static str] {
    match kind {
        "ccf" => &["arroser", "ligne", "proteger", "flanc", "poster", "attaquer", "defendre", "surveiller", "retour", "liberer"],
        "bull" => &["poster", "pare_feu", "retour", "liberer"],
        "helico" | "canadair" => &["larguer", "retour", "liberer"],
        "dash" => &["retardant", "retour", "liberer"],
        "fpt" => &["proteger", "arroser", "poster", "retour", "liberer"],
        "vpc" | "ravit" | "vsav" => &["poster", "retour", "liberer"],
        "debr" => &["debroussailler", "poster", "retour", "liberer"],
        "dragon" => &["retour", "liberer"],
        _ => &[], // vlhr
    }
}

fn permis(kind: &str, ordre: &str) -> bool {
    ordres_permis(kind).iter().any(|&o| o == ordre)
}

fn nom_ordre(o: &str) -> Option<&'static str> {
    Some(match o {
        "poster" => "Se poster",
        "attaquer" => "Attaque",
        "defendre" => "Défense",
        "surveiller" => "Surveillance",
        "larguer" => "Largages",
        "retardant" => "Ligne de retardant",
        "pare_feu" => "Pare-feu",
        "retour" => "Retour à la base",
        "arroser" => "Arrosage orienté",
        "ligne" => "Ligne d'arrêt",
        "proteger" => "Protection 360°",
        "flanc" => "Attaque de flanc",
        "debroussailler" => "Débroussaillement",
        _ => return None,
    })
}

fn vos_ordres(o: &str) -> String {
    format!("{} (vos ordres)", nom_ordre(o).unwrap_or(o))
}

/// Le feu dont une flamme est la plus proche de la cellule (sa racine), ou None.
fn feu_proche(p: &Partie, cell: usize) -> Option<usize> {
    let g = &p.carte.g;
    let mut best: Option<(i64, usize)> = None;
    for &i in &p.feu.brulant {
        let d = g.distance(i, cell);
        if d <= 8 && best.map_or(true, |b| d < b.0) {
            best = Some((d, i));
        }
    }
    best.map(|(_, i)| racine(&p.ia.alias, p.feu.foyer[i] as usize))
}

fn accessible(p: &Partie, j: usize, kind: &str) -> bool {
    vitesse_sol(kind, &p.carte, &p.feu, j) > 0.0 && p.feu.etat[j] != BRULE
}

/// La cellule accessible la plus proche (pas en feu) autour de la cellule visée.
fn poste_sur(p: &Partie, cell: usize, kind: &str) -> Option<usize> {
    let g = &p.carte.g;
    for r in 0..4 {
        let cands: Vec<usize> = g.anneau(cell, r).into_iter().filter(|&j| accessible(p, j, kind)).collect();
        if !cands.is_empty() {
            return argmin(cands, |j| (g.distance(j, cell), -(p.carte.route[j] as i64)));
        }
    }
    None
}

fn angle(g: &Grille, i: usize, j: usize) -> f64 {
    (g.cy[j] - g.cy[i]).atan2(g.cx[j] - g.cx[i]).to_degrees()
}

fn decale(g: &Grille, i: usize, angle: f64, n: i64) -> Option<usize> {
    let a = angle.to_radians();
    g.sous(g.cx[i] + a.cos() * g.hex_m * n as f64, g.cy[i] + a.sin() * g.hex_m * n as f64)
}

/// Vers la flamme la plus proche ; sans feu, face au vent (c'est de là que le feu viendrait).
pub fn direction_defaut(p: &Partie, cell: usize) -> i64 {
    let (g, feu) = (&p.carte.g, &p.feu);
    let proches = feu.brulant.iter().copied().filter(|&i| g.distance(i, cell) <= 10);
    if let Some(i) = argmin(proches, |i| (g.distance(i, cell), -feu.i[i])) {
        if i != cell {
            return rond(angle(g, cell, i));
        }
    }
    let (vx, vy) = p.meteo.vers();
    rond((-vy).atan2(-vx).to_degrees())
}

/// n postes face à la direction d : le poste, puis ses voisins de part et d'autre (une ligne de front).
fn postes_en_ligne(p: &Partie, poste: usize, d: f64, n: usize, kind: &str) -> Vec<usize> {
    let g = &p.carte.g;
    let mut ordre = vec![poste];
    for k in [1i64, -1, 2, -2] {
        if let Some(j) = decale(g, poste, d + 90.0 * k.signum() as f64, k.abs()) {
            if accessible(p, j, kind) {
                ordre.push(j);
            }
        }
    }
    (0..n).map(|k| ordre[k % ordre.len()]).collect()
}

fn mission(type_: &str, poste: Option<usize>, label: String) -> Mission {
    Mission { type_: type_.to_string(), poste, label, ..Default::default() }
}

/// Les missions que recevraient ces moyens (sans les donner) : (indice du moyen, mission) et les refus.
pub fn planifier(p: &Partie, ids: &[usize], ordre: &str, cell: Option<usize>, cell2: Option<usize>, d: Option<f64>) -> (Vec<(usize, Option<Mission>)>, Vec<String>) {
    let (c, g) = (&p.carte, &p.carte.g);
    let nom = |k: usize| p.moyens[k].nom.clone();
    let kind = |k: usize| p.moyens[k].kind;
    let mut moyens: Vec<usize> = (0..p.moyens.len()).filter(|&k| ids.contains(&p.moyens[k].id) && !p.moyens[k].hs).collect();
    let mut refus: Vec<String> = moyens.iter().filter(|&&k| !permis(kind(k), ordre) && kind(k) != "vlhr").map(|&k| nom(k)).collect();
    moyens.retain(|&k| permis(kind(k), ordre));
    let mut plans: Vec<(usize, Option<Mission>)> = Vec::new();
    if ordre == "liberer" || ordre == "retour" {
        return (moyens.into_iter().map(|k| (k, None)).collect(), refus);
    }
    let Some(cell) = cell.filter(|_| !moyens.is_empty()) else {
        refus.extend(moyens.iter().map(|&k| nom(k)));
        return (Vec::new(), refus);
    };
    let f = feu_proche(p, cell);
    // les engins à lances (CCF, FPT) se commandent ensemble ; chacun cherche un poste qu'il peut atteindre
    let (mut ccf, autres): (Vec<usize>, Vec<usize>) = moyens.iter().partition(|&&k| kind(k) == "ccf" || kind(k) == "fpt");
    if !ccf.is_empty() && matches!(ordre, "ligne" | "flanc" | "surveiller" | "attaquer") {
        refus.extend(ccf.iter().filter(|&&k| kind(k) == "fpt").map(|&k| nom(k)));
        ccf.retain(|&k| kind(k) == "ccf");
    }
    // --- les groupes de camions
    if !ccf.is_empty() && (ordre == "arroser" || ordre == "flanc") {
        for kd in ["ccf", "fpt"] {
            let sub: Vec<usize> = ccf.iter().copied().filter(|&k| kind(k) == kd).collect();
            if sub.is_empty() {
                continue;
            }
            let Some(poste) = poste_sur(p, cell, kd) else {
                refus.extend(sub.iter().map(|&k| nom(k)));
                continue;
            };
            let dd = d.unwrap_or_else(|| direction_defaut(p, poste) as f64);
            for (&u, q) in sub.iter().zip(postes_en_ligne(p, poste, dd, sub.len(), kd)) {
                plans.push((u, Some(Mission { dir: Some(rond(dd)), ..mission(ordre, Some(q), vos_ordres(ordre)) })));
            }
        }
    } else if !ccf.is_empty() && ordre == "ligne" {
        let fin = cell2.unwrap_or(cell);
        let mut cases: Vec<usize> = Vec::new();
        for i in g.ligne(cell, fin) {
            if let Some(q) = poste_sur(p, i, "ccf") {
                if !cases.contains(&q) {
                    cases.push(q);
                }
            }
        }
        if cases.is_empty() {
            refus.extend(ccf.iter().map(|&k| nom(k)));
        } else {
            // face au feu : perpendiculaire à la ligne, du côté de la flamme la plus proche (sinon face au vent)
            let axe = if fin != cell { angle(g, cell, fin) } else { 0.0 };
            let vers = d.unwrap_or_else(|| direction_defaut(p, cases[cases.len() / 2]) as f64);
            let mut dd = if (axe + 90.0 - vers).to_radians().cos() >= 0.0 { axe + 90.0 } else { axe - 90.0 };
            if fin == cell {
                dd = vers;
            }
            let n = ccf.len();
            for (k, &u) in ccf.iter().enumerate() {
                let q = if n > 1 { cases[rond(k as f64 * (cases.len() - 1) as f64 / (n - 1).max(1) as f64) as usize] } else { cases[cases.len() / 2] };
                plans.push((u, Some(Mission { dir: Some(rond(dd).rem_euclid(360)), ..mission("ligne", Some(q), vos_ordres("ligne")) })));
            }
        }
    } else if !ccf.is_empty() && (ordre == "proteger" || ordre == "defendre") {
        let sid = if c.site_de[cell] >= 0 {
            Some(c.site_de[cell] as usize)
        } else {
            argmin(c.sites.iter().filter(|s| s.type_ != "reserve"), |s| g.distance(s.centre, cell)).map(|s| s.id)
        };
        match sid {
            None => refus.extend(ccf.iter().map(|&k| nom(k))),
            Some(sid) => {
                let s = &c.sites[sid];
                let menace = &p.evac.menace;
                let mut cases: Vec<usize> = s.cells.iter().copied().filter(|&i| accessible(p, i, "fpt")).collect();
                if cases.is_empty() {
                    cases.push(s.centre);
                }
                trier(&mut cases, |&i| (menace.get(&i).copied().unwrap_or(1e9), g.distance(i, cell)));
                cases.truncate(3);
                for (k, &u) in ccf.iter().enumerate() {
                    let m = Mission { site: Some(sid), ..mission("defense", Some(cases[k % cases.len()]), format!("Protection 360° de {} (vos ordres)", s.nom)) };
                    plans.push((u, Some(m)));
                }
            }
        }
    } else if !ccf.is_empty() && matches!(ordre, "poster" | "attaquer" | "surveiller") {
        let typ = match ordre {
            "poster" => "preposition",
            "attaquer" => "attaque",
            _ => "surveillance",
        };
        for &u in &ccf {
            match poste_sur(p, cell, kind(u)) {
                None => refus.push(nom(u)),
                Some(poste) => plans.push((u, Some(mission(typ, Some(poste), vos_ordres(ordre))))),
            }
        }
    }
    // --- bulldozers et avions
    for &u in &autres {
        let fin = cell2.unwrap_or(cell);
        let m = match ordre {
            "poster" => poste_sur(p, cell, kind(u)).map(|poste| {
                if kind(u) == "vpc" {
                    mission("pc", Some(poste), "Poste de commandement (vos ordres)".into())
                } else {
                    mission("preposition", Some(poste), vos_ordres(ordre))
                }
            }),
            "debroussailler" | "pare_feu" => {
                let cells: Vec<usize> = g
                    .ligne(cell, fin)
                    .into_iter()
                    .filter(|&i| c.site_de[i] < 0 && if ordre == "pare_feu" { brule(c.fuel[i]) } else { b"PFTJGQ".contains(&c.fuel[i]) })
                    .take(14)
                    .collect();
                (!cells.is_empty()).then(|| {
                    let quoi = if ordre == "pare_feu" { "Pare-feu" } else { "Débroussaillement" };
                    let label = format!("{quoi} de {} m (vos ordres)", cells.len() * 300);
                    Mission { cells, ..mission(ordre, None, label) }
                })
            }
            "larguer" => (brule(c.fuel[cell]) || est_eau(c.fuel[cell]))
                .then(|| Mission { cible: Some(cell), axe: d.map(rond), ..mission("largage", None, "Largages au point visé (vos ordres)".into()) }),
            "retardant" => Some(Mission { cible: Some(cell), axe: d.map(rond), ..mission("retardant", None, "Ligne de retardant (vos ordres)".into()) }),
            _ => None,
        };
        match m {
            None => refus.push(nom(u)),
            Some(m) => plans.push((u, Some(m))),
        }
    }
    if let Some(f) = f {
        for (_, m) in plans.iter_mut() {
            if let Some(m) = m {
                m.feu = Some(f);
            }
        }
    }
    (plans, refus)
}

pub fn donner(p: &mut Partie, ids: &[usize], ordre: &str, cell: Option<usize>, cell2: Option<usize>, d: Option<f64>) -> Value {
    let (plans, refus) = planifier(p, ids, ordre, cell, cell2, d);
    let n = plans.len();
    let mut noms: Vec<String> = Vec::new();
    for (k, m) in plans {
        let u = &mut p.moyens[k];
        noms.push(u.nom.clone());
        if ordre == "liberer" {
            u.joueur = false;
            u.mission = None;
            continue;
        }
        u.mission = m;
        u.joueur = true;
        if !u.terre {
            u.phase = None;
            u.vise = None;
            u.cible = None;
        }
    }
    if n > 0 {
        let mut liste = noms.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
        if n > 4 {
            liste += &format!(" et {} autres", n - 4);
        }
        let txt = match ordre {
            "liberer" => "rendu(s) à l'IA".to_string(),
            "retour" => "rentre(nt) à la base".to_string(),
            o => nom_ordre(o).unwrap_or(o).to_lowercase(),
        };
        p.journal_ajout("joueur", format!("Vos ordres — {liste} : {txt}."));
    }
    let erreur = if n > 0 { Value::Null } else { json!("Cet ordre ne convient pas à ces moyens, ou l'endroit est inaccessible.") };
    json!({"ok": n > 0, "n": n, "refus": refus, "erreur": erreur})
}

/// Ce que donnerait l'ordre, pour l'afficher avant de le valider : postes, trajets, cellules arrosées ou trop
/// chaudes, eau restante ; pour les avions, le point d'écopage et les cellules du largage.
pub fn apercu(p: &Partie, ids: &[usize], ordre: &str, cell: Option<usize>, cell2: Option<usize>, d: Option<f64>) -> Value {
    let (g, feu) = (&p.carte.g, &p.feu);
    let (plans, refus) = planifier(p, ids, ordre, cell, cell2, d);
    let mut sortie = Vec::new();
    for (k, m) in &plans {
        let Some(m) = m else { continue };
        let u = &p.moyens[*k];
        let mut e = Map::new();
        e.insert("id".into(), json!(u.id));
        e.insert("k".into(), json!(u.kind));
        e.insert("eau".into(), if u.cap > 0.0 { json!(rond(100.0 * u.eau / u.cap)) } else { Value::Null });
        e.insert("poste".into(), json!(m.poste));
        e.insert("dir".into(), json!(m.dir));
        e.insert("axe".into(), json!(m.axe));
        if let (true, Some(q)) = (u.terre, m.poste) {
            let ch = u.chemin_vers(p, q, None).unwrap_or_default();
            e.insert("chemin".into(), json!(ch[..ch.len().min(80)]));
            let minutes: f64 = ch.iter().skip(1).map(|&j| 1.0 / (0.2 * g.ech).max(vitesse_sol(u.kind, &p.carte, feu, j))).sum();
            e.insert("minutes".into(), json!(rond(minutes)));
            let cone = m.dir.map(|d| (d as f64, CONE));
            let portee: Vec<usize> = std::iter::once(q).chain(g.voisins(q)).filter(|&j| dans_cone(g, q, j, cone)).collect();
            let seuil = if m.type_ == "defense" { 1.01 } else { 0.8 };
            let chaud: Vec<usize> = portee.iter().copied().filter(|&j| feu.etat[j] == BRULE && feu.i[j] > seuil).collect();
            e.insert("portee".into(), json!(portee));
            e.insert("chaud".into(), json!(chaud));
            e.insert("autonomie".into(), if u.kind == "ccf" { json!(rond(u.eau / 400.0)) } else { Value::Null });
        } else if !u.terre {
            let mut essai: Moyen = u.clone();
            essai.mission = Some(m.clone());
            let largage = if u.kind == "dash" { p.ia.ligne_retardant(p, &essai) } else { p.ia.cible_largage(p, &essai).unwrap_or_default() };
            e.insert("largage".into(), json!(largage));
            e.insert("ecopage".into(), if u.kind == "dash" { Value::Null } else { json!(p.eau_aerienne(&essai)) });
            e.insert("x".into(), json!(rond(u.x)));
            e.insert("y".into(), json!(rond(u.y)));
        }
        sortie.push(Value::Object(e));
    }
    json!({"ok": !plans.is_empty(), "refus": refus, "moyens": sortie})
}

/// Attaque de flanc : le groupe avance vers la tête, d'un poste au contact à l'autre, en gardant le feu dans son cône.
pub fn suivre_flanc(p: &Partie, u: &mut Moyen) {
    let (g, feu) = (&p.carte.g, &p.feu);
    let (vx, vy) = p.meteo.vers();
    let proj = |i: usize| g.cx[i] * vx + g.cy[i] * vy;
    let ici = proj(u.cell);
    let mut best: Option<(f64, usize, usize)> = None;
    for j in g.anneau(u.cell, 2) {
        if j == u.cell || !accessible(p, j, "ccf") {
            continue;
        }
        let flammes: Vec<usize> = g.voisins(j).into_iter().filter(|&k| feu.etat[k] == BRULE && feu.i[k] <= 0.85).collect();
        if flammes.is_empty() || g.vois[j].iter().any(|v| feu.etat[v.j] == BRULE && feu.i[v.j] > 0.92) {
            continue;
        }
        let sc = proj(j) - ici - g.distance(j, u.cell) as f64 * 40.0;
        if proj(j) > ici - 10.0 && best.map_or(true, |b| sc > b.0) {
            let k = argmax(flammes, |k| feu.i[k]).expect("flamme");
            best = Some((sc, j, k));
        }
    }
    let Some(m) = u.mission.as_mut() else { return };
    m.inactif = 0;
    if let Some((_, j, k)) = best {
        m.poste = Some(j);
        m.dir = Some(rond(angle(g, j, k)));
    }
}
