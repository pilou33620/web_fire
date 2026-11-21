//! Tests du moteur (ceux de l'ancien tests/test_moteur.py, un pour un) : cargo test -p feu

use feu::argmin;
use feu::carte::Carte;
use feu::feu::{BRULE, BRULE_FINI};
use feu::moyens::{dans_cone, vitesse_sol, Mission, CONE, RESERVE};
use feu::partie::{Config, Partie};
use serde_json::json;

fn sans_moyens() -> Config {
    ["ccf", "fpt", "bull", "debr", "vpc", "ravit", "vsav", "helico", "canadair", "dash", "dragon"].iter().map(|k| (k.to_string(), 0)).collect()
}

/// Une partie sans départ de feu automatique.
fn partie_calme(moyens: Option<Config>) -> Partie {
    let mut p = Partie::new(5, "gironde", "chaud", &moyens.unwrap_or_default(), "equilibree", false);
    p.prochain_depart = 1_000_000_000;
    p.meteo.sc.bascule = None;
    p
}

fn partie(graine: i64, scenario: &str, moyens: Option<Config>) -> Partie {
    Partie::new(graine, "gironde", scenario, &moyens.unwrap_or_default(), "equilibree", true)
}

/// Au cœur du massif : le centre des hexagones de ce combustible.
fn cellule_centrale(p: &Partie, fuel: u8) -> usize {
    let g = &p.carte.g;
    let cells: Vec<usize> = (0..g.n).filter(|&i| p.carte.fuel[i] == fuel).collect();
    let mx = cells.iter().map(|&i| g.cx[i]).sum::<f64>() / cells.len() as f64;
    let my = cells.iter().map(|&i| g.cy[i]).sum::<f64>() / cells.len() as f64;
    let mid = g.sous(mx, my).expect("milieu");
    argmin((0..g.n).filter(|&i| p.carte.fuel[i] == fuel && p.carte.site_de[i] < 0), |i| g.distance(i, mid)).expect("cellule")
}

fn pins(p: &Partie) -> usize {
    cellule_centrale(p, b'P')
}

fn allumer(p: &mut Partie, i: usize) -> bool {
    let t = p.t;
    p.feu.allumer(&p.carte, i, -1, None, "test", t)
}

fn pas(p: &mut Partie, n: usize) {
    for _ in 0..n {
        p.pas();
    }
}

fn journal_contient(p: &Partie, mot: &str) -> bool {
    p.reg.journal.iter().any(|j| j.txt.contains(mot))
}

// ---------------------------------------------------------------------- carte

#[test]
fn carte_reproductible() {
    let (a, b) = (Carte::new(11, "gironde"), Carte::new(11, "gironde"));
    assert_eq!(a.fuel, b.fuel);
    assert_eq!(a.route, b.route);
}

#[test]
fn carte_themes() {
    let c = Carte::new(3, "gironde");
    assert!(c.sites.len() >= 8);
    assert!(c.casernes.len() >= 2);
    assert!(c.aerodrome.is_some());
    assert!((0..c.g.n).any(|i| c.ecopable(i)));
    assert!(c.route.iter().any(|&r| r == 1)); // pistes DFCI
}

#[test]
fn grille_inverse() {
    let c = Carte::new(2, "gironde");
    let g = &c.g;
    for i in (0..g.n).step_by(37) {
        assert_eq!(g.sous(g.cx[i], g.cy[i]), Some(i));
    }
}

// ---------------------------------------------------------------------- feu

#[test]
fn le_vent_pousse_le_feu() {
    let mut p = partie_calme(Some(sans_moyens()));
    p.action(&json!({"type": "vent", "de": 90, "force": 40})).unwrap(); // vent d'est : le feu part vers l'ouest
    let i0 = pins(&p);
    allumer(&mut p, i0);
    let n = (90.0 / p.carte.g.ech).round() as usize; // 90 min sur des hexagones de 300 m
    pas(&mut p, n);
    let g = &p.carte.g;
    let touches: Vec<usize> = (0..g.n).filter(|&i| matches!(p.feu.etat[i], BRULE | BRULE_FINI)).collect();
    assert!(touches.len() > 8, "{} hexagones touchés", touches.len());
    let ouest = touches.iter().filter(|&&i| g.cx[i] < g.cx[i0] - g.hex_m).count();
    let est = touches.iter().filter(|&&i| g.cx[i] > g.cx[i0] + g.hex_m).count();
    assert!(ouest > 2 * est, "ouest {ouest}, est {est}");
}

#[test]
fn l_eau_abat_le_feu() {
    let mut p = partie_calme(Some(sans_moyens()));
    let i0 = pins(&p);
    allumer(&mut p, i0);
    pas(&mut p, 8);
    let avant = p.feu.i[i0];
    p.feu.arroser(&p.carte, i0, 6000.0);
    assert!(p.feu.i[i0] < avant);
    assert!(p.feu.mouille[i0] > 0.3);
}

#[test]
fn rien_ne_brule_dans_l_eau() {
    let mut p = partie_calme(Some(sans_moyens()));
    let lac = (0..p.carte.g.n).find(|&i| p.carte.fuel[i] == b'L').expect("lac");
    assert!(!allumer(&mut p, lac));
}

// ---------------------------------------------------------------------- commandement

#[test]
fn les_moyens_limitent_les_degats() {
    // sur plusieurs journées de canicule, le commandement brûle bien moins que le feu livré à lui-même
    let (mut avec, mut sans) = (0.0, 0.0);
    for graine in [3, 7, 11] {
        let mut p = partie(graine, "canicule", None);
        pas(&mut p, 300);
        avec += p.feu.surface_ha(None);
        let mut p = partie(graine, "canicule", Some(sans_moyens()));
        pas(&mut p, 300);
        sans += p.feu.surface_ha(None);
    }
    assert!(avec < sans * 0.6, "avec {avec} ha, sans {sans} ha");
}

#[test]
fn feu_naissant_attaque() {
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    p.action(&json!({"type": "allumer", "cell": i0})).unwrap();
    pas(&mut p, 10);
    let engages = p.moyens.iter().filter(|u| u.mission.as_ref().is_some_and(|m| m.type_ == "attaque" || m.type_ == "largage")).count();
    assert!(engages >= 4, "{engages} moyens engagés");
    assert!(journal_contient(&p, "naissant"));
}

#[test]
fn trace_du_largage() {
    // l'interface fait tomber l'eau quand l'avion arrive : le trajet exporté passe par le point de largage, à sa minute
    let mut p = Partie::new(7, "landes", "canicule", &Config::new(), "equilibree", true);
    let mut vus = 0;
    for _ in 0..120 {
        p.pas();
        let g = &p.carte.g;
        for e in p.reg.evts.iter().filter(|e| e["type"] == "largage" && e["id"].as_i64().unwrap() > vus && e["produit"] == "eau") {
            let u = &p.moyens[e["moyen"].as_u64().unwrap() as usize];
            let cells: Vec<usize> = e["cells"].as_array().unwrap().iter().map(|c| c.as_u64().unwrap() as usize).collect();
            let cx = cells.iter().map(|&i| g.cx[i]).sum::<f64>() / cells.len() as f64;
            let cy = cells.iter().map(|&i| g.cy[i]).sum::<f64>() / cells.len() as f64;
            let tr = u.export()["tr"].clone();
            let ok = tr.as_array().unwrap().iter().any(|t| {
                t[0] == e["t"] && (t[1].as_f64().unwrap() - cx).abs() < 2.0 && (t[2].as_f64().unwrap() - cy).abs() < 2.0
            });
            assert!(ok, "le trajet ne passe pas par le largage");
            return;
        }
        vus = p.reg.seq;
    }
    panic!("aucun largage");
}

// ---------------------------------------------------------------------- ordres

#[test]
fn l_ia_respecte_vos_ordres() {
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    p.action(&json!({"type": "allumer", "cell": i0})).unwrap();
    let ccf: Vec<usize> = p.moyens.iter().filter(|u| u.kind == "ccf").take(2).map(|u| u.id).collect();
    let loin = p.carte.casernes[0].cell;
    let r = p.action(&json!({"type": "ordre", "moyens": ccf, "ordre": "poster", "cell": loin})).unwrap();
    assert_eq!(r["ok"], true);
    pas(&mut p, 12);
    for &id in &ccf {
        assert!(p.moyens[id].joueur);
        assert_eq!(p.moyens[id].mission.as_ref().unwrap().type_, "preposition");
    }
    p.action(&json!({"type": "ordre", "moyens": [ccf[0]], "ordre": "liberer"})).unwrap();
    assert!(!p.moyens[ccf[0]].joueur);
}

#[test]
fn mode_manuel() {
    let mut p = partie_calme(None);
    p.action(&json!({"type": "commandement", "valeur": "manuel"})).unwrap();
    let i0 = pins(&p);
    p.action(&json!({"type": "allumer", "cell": i0})).unwrap();
    pas(&mut p, 10);
    assert!(p.moyens.iter().all(|u| u.mission.is_none()), "en mode manuel, l'IA n'engage rien");
    let can = p.moyens.iter().find(|u| u.kind == "canadair").unwrap().id;
    assert_eq!(p.action(&json!({"type": "ordre", "moyens": [can], "ordre": "larguer", "cell": i0})).unwrap()["ok"], true);
    assert_eq!(p.action(&json!({"type": "ordre", "moyens": [can], "ordre": "attaquer", "cell": i0})).unwrap()["ok"], false);
}

// ---------------------------------------------------------------------- sections

#[test]
fn giff_quatre_camions_et_un_chef() {
    let p = partie_calme(None);
    let chefs: Vec<_> = p.moyens.iter().filter(|u| u.kind == "vlhr").collect();
    assert_eq!(chefs.len(), 4);
    for ch in chefs {
        assert_eq!(p.moyens.iter().filter(|u| u.kind == "ccf" && u.groupe == ch.groupe).count(), 4);
    }
}

fn feu_et_section() -> (Partie, usize, Vec<usize>) {
    let mut p = partie_calme(None);
    p.meteo.sc.vent = 5.0;
    p.meteo.maj();
    let i0 = pins(&p);
    allumer(&mut p, i0);
    pas(&mut p, 6);
    let ids = p.moyens.iter().filter(|u| u.kind == "ccf" && u.groupe.as_deref() == Some("GIFF 1")).map(|u| u.id).collect();
    (p, i0, ids)
}

#[test]
fn le_cone_limite_l_arrosage() {
    let (mut p, i0, ids) = feu_et_section();
    let r = p.action(&json!({"type": "ordre", "moyens": ids, "ordre": "arroser", "cell": i0, "dir": 0})).unwrap();
    assert_eq!(r["ok"], true);
    for _ in 0..40 {
        p.pas();
        for u in p.moyens.iter().filter(|u| ids.contains(&u.id)) {
            if let Some(a) = u.act.as_ref().filter(|a| a.k == "arrose") {
                assert!(dans_cone(&p.carte.g, u.cell, a.c.unwrap(), Some((0.0, CONE))));
            }
        }
    }
}

#[test]
fn ligne_d_arret_etalee() {
    let (mut p, i0, ids) = feu_et_section();
    let g = &p.carte.g;
    let a = g.sous(g.cx[i0] - 3.0 * g.hex_m, g.cy[i0] + 2.0 * g.hex_m).unwrap();
    let b = g.sous(g.cx[i0] + 3.0 * g.hex_m, g.cy[i0] + 2.0 * g.hex_m).unwrap();
    p.action(&json!({"type": "ordre", "moyens": ids, "ordre": "ligne", "cell": a, "cell2": b})).unwrap();
    let mut postes: Vec<usize> = p.moyens.iter().filter(|u| ids.contains(&u.id)).map(|u| u.mission.as_ref().unwrap().poste.unwrap()).collect();
    postes.sort_unstable();
    postes.dedup();
    assert!(postes.len() >= 3);
}

#[test]
fn apercu_sans_effet() {
    let (mut p, i0, ids) = feu_et_section();
    let avant: Vec<(Option<Mission>, bool)> = p.moyens.iter().map(|u| (u.mission.clone(), u.joueur)).collect();
    let r = p.action(&json!({"type": "apercu", "moyens": ids, "ordre": "arroser", "cell": i0, "dir": 90})).unwrap();
    assert_eq!(r["ok"], true);
    let m = r["moyens"].as_array().unwrap();
    assert_eq!(m.len(), 4);
    assert!(m.iter().all(|e| !e["chemin"].as_array().unwrap().is_empty() && !e["portee"].as_array().unwrap().is_empty()));
    let apres: Vec<(Option<Mission>, bool)> = p.moyens.iter().map(|u| (u.mission.clone(), u.joueur)).collect();
    assert_eq!(avant, apres);
}

#[test]
fn axe_du_canadair() {
    let (mut p, i0, _) = feu_et_section();
    let can = p.moyens.iter().find(|u| u.kind == "canadair").unwrap().id;
    let r = p.action(&json!({"type": "apercu", "moyens": [can], "ordre": "larguer", "cell": i0, "dir": 0})).unwrap();
    let cells: Vec<usize> = r["moyens"][0]["largage"].as_array().unwrap().iter().map(|c| c.as_u64().unwrap() as usize).collect();
    assert_eq!(cells.len(), 2);
    let g = &p.carte.g;
    assert!((g.cy[cells[0]] - g.cy[cells[1]]).abs() < 1.0); // axe est-ouest : même ligne
    assert!(!r["moyens"][0]["ecopage"].is_null());
}

// ---------------------------------------------------------------------- autoprotection

/// Le premier CCF, posté au milieu d'un feu violent. Rend la partie et l'indice du camion.
fn encercler(eau: f64) -> (Partie, usize) {
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    let k = p.moyens.iter().position(|m| m.kind == "ccf").unwrap();
    let g = &p.carte.g;
    let u = &mut p.moyens[k];
    u.cell = i0;
    u.x = g.cx[i0];
    u.y = g.cy[i0];
    u.eau = eau;
    u.mission = Some(Mission { type_: "preposition".into(), poste: Some(i0), label: "test".into(), ..Default::default() });
    u.joueur = true;
    let cells: Vec<usize> = std::iter::once(i0).chain(p.carte.g.voisins(i0)).collect();
    for j in cells {
        if allumer(&mut p, j) {
            p.feu.i[j] = 0.95;
        }
    }
    (p, k)
}

fn pas_du_moyen(p: &mut Partie, k: usize) {
    let mut u = p.moyens[k].clone();
    u.pas(p);
    p.moyens[k] = u;
}

#[test]
fn l_autoprotection_sauve_le_camion() {
    let (mut p, k) = encercler(RESERVE);
    pas_du_moyen(&mut p, k);
    let u = &p.moyens[k];
    assert!(u.auto);
    assert!(!u.hs);
    assert_eq!(u.eau, RESERVE - 120.0);
    assert!(journal_contient(&p, "autoprotection"));
}

#[test]
fn sans_reserve_le_camion_est_perdu() {
    let (mut p, k) = encercler(0.0);
    pas_du_moyen(&mut p, k);
    assert!(p.moyens[k].hs);
    assert_eq!(p.stats.perdus, 1);
    let (id, base) = (p.moyens[k].id, p.moyens[k].base);
    assert_eq!(p.action(&json!({"type": "ordre", "moyens": [id], "ordre": "poster", "cell": base})).unwrap()["ok"], false);
}

#[test]
fn les_lances_ne_touchent_pas_la_reserve() {
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    let k = p.moyens.iter().position(|m| m.kind == "ccf").unwrap();
    let voisin = p.carte.g.voisins(i0)[0];
    allumer(&mut p, voisin);
    p.feu.i[voisin] = 0.5;
    let mut u = p.moyens[k].clone();
    u.cell = i0;
    u.eau = RESERVE + 250.0;
    for _ in 0..3 {
        u.arroser_voisins(&mut p, 0.8, None, None);
    }
    assert!(u.eau >= RESERVE);
}

// ---------------------------------------------------------------------- engins de soutien

#[test]
fn dragon_et_vsav_secourent() {
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    let g = &p.carte.g;
    let route = argmin((0..g.n).filter(|&i| p.carte.route[i] >= 2 && p.carte.site_de[i] < 0), |i| g.distance(i, i0)).unwrap();
    p.ajouter_blesses(i0, 4, "equipage", "danger", "test équipage");
    p.ajouter_blesses(route, 2, "pompier", "leger", "test léger");
    pas(&mut p, 3);
    let secours = |kind: &str| p.moyens.iter().any(|u| u.kind == kind && u.mission.as_ref().is_some_and(|m| m.type_ == "secours"));
    assert!(secours("dragon"));
    assert!(secours("vsav"));
    pas(&mut p, 150);
    assert_eq!(p.secourus.hopital, 6);
}

#[test]
fn ravitailleur() {
    // le camion attend à son poste, au cœur du massif : un camion à sec y reçoit la visite du ravitailleur
    // (en Python, le camion roulait encore et le ravitailleur n'arrivait qu'à la dernière minute : trop fragile)
    let mut p = partie_calme(None);
    let i0 = pins(&p);
    let k = p.moyens.iter().position(|m| m.kind == "ccf").unwrap();
    let id = p.moyens[k].id;
    let (x, y) = (p.carte.g.cx[i0], p.carte.g.cy[i0]);
    let u = &mut p.moyens[k];
    u.cell = i0;
    u.x = x;
    u.y = y;
    p.action(&json!({"type": "ordre", "moyens": [id], "ordre": "poster", "cell": i0})).unwrap();
    pas(&mut p, 30);
    let max = p.moyens[k].carb_max.unwrap();
    p.moyens[k].carb = 0.2 * max;
    for _ in 0..300 {
        p.pas();
        if p.moyens[k].carb > 0.9 * max {
            return;
        }
    }
    panic!("gazole : {}", p.moyens[k].carb);
}

#[test]
fn fpt_reste_sur_les_routes() {
    let p = partie_calme(None);
    let c = &p.carte;
    let piste = (0..c.g.n).find(|&i| c.route[i] == 1).unwrap();
    let foret = (0..c.g.n).find(|&i| c.route[i] == 0 && c.fuel[i] == b'P' && c.site_de[i] < 0).unwrap();
    let route = (0..c.g.n).find(|&i| c.route[i] >= 2).unwrap();
    assert_eq!(vitesse_sol("fpt", c, &p.feu, piste), 0.0);
    assert_eq!(vitesse_sol("fpt", c, &p.feu, foret), 0.0);
    assert!(vitesse_sol("fpt", c, &p.feu, route) > 2.5 * c.g.ech);
}

#[test]
fn debroussaillage_hors_risque_severe() {
    let mut p = Partie::new(3, "gironde", "calme", &Config::new(), "equilibree", false);
    p.prochain_depart = 1_000_000_000;
    pas(&mut p, 240);
    assert!(p.stats.debroussaille > 0);
    assert!(p.carte.fuel.contains(&b'Y'));
}

#[test]
fn poste_de_commandement() {
    let mut moyens = sans_moyens();
    moyens.insert("vpc".into(), 1);
    let mut p = partie_calme(Some(moyens));
    p.action(&json!({"type": "vent", "de": 90, "force": 35})).unwrap();
    let i0 = pins(&p);
    p.action(&json!({"type": "allumer", "cell": i0})).unwrap();
    pas(&mut p, 150);
    assert!(p.ia.feux.values().any(|f| f.pc) || journal_contient(&p, "📡"));
}

// ---------------------------------------------------------------------- évacuation

#[test]
fn evacuer_puis_arriver() {
    let mut p = partie_calme(Some(sans_moyens()));
    let (sid, pop) = p.carte.sites.iter().find(|s| s.type_ == "village").map(|s| (s.id, s.pop)).unwrap();
    assert_eq!(p.action(&json!({"type": "evacuer", "site": sid})).unwrap()["ok"], true);
    pas(&mut p, 240);
    let e = &p.evac.sites[&sid];
    assert_eq!(e.reste + e.route, 0);
    assert_eq!(e.arrives, pop);
}

#[test]
fn confiner() {
    let mut p = partie_calme(Some(sans_moyens()));
    let (sid, pop) = p.carte.sites.iter().find(|s| s.type_ == "camping").map(|s| (s.id, s.pop)).unwrap();
    p.action(&json!({"type": "confiner", "site": sid})).unwrap();
    assert_eq!(p.evac.sites[&sid].confines, pop);
}

#[test]
fn la_fumee_suit_le_vent() {
    let mut p = partie_calme(Some(sans_moyens()));
    p.action(&json!({"type": "vent", "de": 90, "force": 40})).unwrap();
    let i0 = pins(&p);
    allumer(&mut p, i0);
    pas(&mut p, 15);
    let g = &p.carte.g;
    let ouest = p.evac.fumee.keys().filter(|&&i| g.cx[i] < g.cx[i0] - g.hex_m).count();
    let est = p.evac.fumee.keys().filter(|&&i| g.cx[i] > g.cx[i0] + g.hex_m).count();
    assert!(ouest > est, "ouest {ouest}, est {est}");
}

// ---------------------------------------------------------------------- voies

#[test]
fn reseau_propre() {
    let c = Carte::new(4, "gironde");
    let mut aretes = std::collections::HashSet::new();
    for r in &c.routes_poly {
        assert!(r.p.len() >= 2);
        for w in r.c.windows(2) {
            let e = (w[0].min(w[1]), w[0].max(w[1]));
            assert!(aretes.insert(e), "tronçon dessiné deux fois");
            assert_eq!(c.g.distance(w[0], w[1]), 1);
        }
        for &i in &r.c {
            assert!(!b"ML".contains(&c.fuel[i]), "une voie dans l'eau");
        }
    }
}

#[test]
fn bonus_malus() {
    let mut p = partie_calme(None);
    let g = &p.carte.g;
    let piste = (0..g.n).find(|&i| p.carte.route[i] == 1).unwrap();
    let route = (0..g.n).find(|&i| p.carte.route[i] >= 2 && p.carte.site_de[i] < 0).unwrap();
    let k = p.moyens.iter().position(|m| m.kind == "ccf").unwrap();
    let mut u = p.moyens[k].clone();
    // piste étroite : à trois engins on n'avance plus qu'au pas
    p.trafic_engins = [(piste, 3)].into_iter().collect();
    assert!(u.facteur_voie(&p, piste) < 0.5);
    // route : les convois font des bouchons, la fumée ralentit
    p.trafic_convois = [(route, 2)].into_iter().collect();
    assert!(u.facteur_voie(&p, route) < 0.5);
    // piste qui longe un feu intense : fermée aux camions
    p.danger = [piste].into_iter().collect();
    let g = &p.carte.g;
    u.cell = g.voisins(piste).into_iter().find(|&j| j != piste).unwrap();
    let dest = g.anneau(piste, 3).into_iter().find(|&j| g.distance(j, piste) == 3 && !b"MLR".contains(&p.carte.fuel[j])).unwrap();
    u.aller(&p, dest);
    assert!(!u.chemin.contains(&piste));
}

#[test]
fn pas_de_convoi_sur_les_pistes() {
    let mut p = partie_calme(Some(sans_moyens()));
    let sids: Vec<usize> = p.carte.sites.iter().filter(|s| s.type_ == "village" || s.type_ == "camping").map(|s| s.id).collect();
    for sid in sids {
        p.action(&json!({"type": "evacuer", "site": sid})).unwrap();
    }
    for _ in 0..60 {
        p.pas();
        for cv in &p.evac.convois {
            assert!(p.carte.route[cv.cell] >= 2 || p.carte.site_de[cv.cell] >= 0);
        }
    }
}

// ---------------------------------------------------------------------- export

#[test]
fn export_json() {
    let mut p = partie(4, "mistral", None);
    pas(&mut p, 60);
    let carte = p.export_carte();
    assert_eq!(carte["fuel"].as_str().unwrap().len(), p.carte.g.n);
    let e = p.export_etat(0);
    assert_eq!(e["etat"].as_str().unwrap().chars().count(), p.carte.g.n);
    assert!(e.to_string().len() < 400_000);
}

#[test]
fn simulation_reglages_et_actions() {
    let mut s = feu::Sim::new(1);
    assert_eq!(s.etat(0, 1.0, false)["partie"], 1);
    let r = s.action(&json!({"type": "nouvelle", "reglages": {"graine": 0, "scenario": "orage"}}), 2).unwrap();
    assert_eq!(r, json!({"ok": true, "partie": 2}));
    assert_eq!(s.reglages["graine"], 1); // graine nulle : 1, comme l'ancienne version
    assert_eq!(s.partie.meteo.code, "orage");
    let t = s.partie.t;
    s.action(&json!({"type": "avancer", "minutes": 5}), 3).unwrap();
    assert_eq!(s.partie.t, t + 5);
    assert_eq!(s.action(&json!({"type": "inconnue"}), 3).unwrap()["ok"], false);
    assert!(s.action(&json!({"type": "allumer"}), 3).is_err());
    assert!(s.action(&json!({"type": "allumer", "cell": 99_999_999}), 3).is_err());
    let infos = feu::infos();
    assert_eq!(infos["defaut"]["ccf"], 16);
    assert!(infos["scenarios"].as_object().unwrap().contains_key("canicule"));
}
