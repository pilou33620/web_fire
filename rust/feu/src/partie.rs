//! Une partie : la carte, la météo, le feu, les moyens et le commandement, avancés minute par minute.
//!
//! C'est la seule porte d'entrée pour le serveur comme pour la version navigateur (WebAssembly) :
//!   Partie::new(...).pas()  ·  .export_carte()  ·  .export_etat()  ·  .action(&json)

use std::collections::{HashMap, HashSet};

use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use crate::carte::Carte;
use crate::combustibles::{brule, est_eau, HABITE};
use crate::evacuation::Evacuation;
use crate::feu::{Feu, BRULE, BRULE_FINI, ETEINT, INTACT, NOYE};
use crate::hasard::Hasard;
use crate::ia::{racine, Commandement};
use crate::meteo::Meteo;
use crate::moyens::{Mission, Moyen};
use crate::{arrondi, argmin, entier, nombre, ordres, repr, rond, trier, vrai};

/// Les moyens demandés par type (ccf, fpt…) ; ceux qui manquent prennent leur valeur par défaut.
pub type Config = IndexMap<String, i64>;

pub const MOYENS_DEFAUT: &[(&str, i64)] = &[
    ("ccf", 16), ("fpt", 4), ("bull", 2), ("debr", 1), ("vpc", 1), ("ravit", 1), ("vsav", 3), ("helico", 3), ("canadair", 4), ("dash", 1), ("dragon", 1),
];
pub const LIMITES: &[(&str, i64, i64)] = &[
    ("ccf", 0, 60), ("fpt", 0, 20), ("bull", 0, 8), ("debr", 0, 4), ("vpc", 0, 3), ("ravit", 0, 4), ("vsav", 0, 8),
    ("helico", 0, 12), ("canadair", 0, 16), ("dash", 0, 4), ("dragon", 0, 3),
];

#[derive(Clone, Debug)]
pub struct Ligne {
    pub id: i64,
    pub t: i64,
    pub h: String,
    pub k: String,
    pub txt: String,
}

/// Des personnes à secourir.
#[derive(Clone, Debug)]
pub struct Blesse {
    pub id: usize,
    pub cell: usize,
    pub n: i64,
    /// type : equipage, pompier, civil
    pub t: String,
    /// gravité : danger, grave, leger
    pub g: String,
    /// attente, pris
    pub etat: &'static str,
    pub quoi: String,
    pub t0: i64,
    pub aggrave: bool,
}

/// Le journal, les événements et les blessés : à part du reste de la partie, pour qu'un moyen ou le
/// commandement y écrive tout en lisant la carte et le feu.
#[derive(Default)]
pub struct Registre {
    pub seq: i64,
    pub journal: Vec<Ligne>,
    pub evts: Vec<Value>,
    pub blesses: Vec<Blesse>,
    pub n_blesses: usize,
}

impl Registre {
    pub fn ajout(&mut self, m: &Meteo, kind: &str, txt: String) {
        self.seq += 1;
        self.journal.push(Ligne { id: self.seq, t: m.t, h: m.heure(), k: kind.to_string(), txt });
        if self.journal.len() > 150 {
            let n = self.journal.len() - 150;
            self.journal.drain(..n);
        }
    }

    /// Un événement pour l'interface : {id, t, type, ...kw}.
    pub fn evenement(&mut self, m: &Meteo, type_: &str, kw: Value) {
        self.seq += 1;
        let mut e = Map::new();
        e.insert("id".into(), json!(self.seq));
        e.insert("t".into(), json!(m.t));
        e.insert("type".into(), json!(type_));
        if let Value::Object(o) = kw {
            e.extend(o);
        }
        self.evts.push(Value::Object(e));
        if self.evts.len() > 80 {
            let n = self.evts.len() - 80;
            self.evts.drain(..n);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn blesses_ajout(&mut self, m: &Meteo, t: i64, cell: usize, n: i64, typ: &str, gravite: &str, quoi: String) {
        if n <= 0 {
            return;
        }
        self.n_blesses += 1;
        let txt = format!("🚑 {} blessé{} à secourir ({}).", n, if n > 1 { "s" } else { "" }, quoi);
        self.blesses.push(Blesse { id: self.n_blesses, cell, n, t: typ.into(), g: gravite.into(), etat: "attente", quoi, t0: t, aggrave: false });
        self.ajout(m, "alerte", txt);
    }

    pub fn blesse(&mut self, bid: usize) -> Option<&mut Blesse> {
        self.blesses.iter_mut().find(|b| b.id == bid)
    }
}

#[derive(Default, Debug)]
pub struct Stats {
    pub largages: i64,
    pub defriche: i64,
    pub departs: i64,
    pub renforts: i64,
    pub perdus: i64,
    pub autoprotections: i64,
    pub debroussaille: i64,
}

#[derive(Default, Debug)]
pub struct Secourus {
    pub hopital: i64,
    pub graves: i64,
    pub aggraves: i64,
}

pub struct Partie {
    pub graine: i64,
    pub theme: String,
    pub scenario: String,
    pub rnd: Hasard,
    pub carte: Carte,
    pub meteo: Meteo,
    pub feu: Feu,
    pub t: i64,
    pub reg: Registre,
    pub evacues: HashSet<usize>,
    pub stats: Stats,
    pub secourus: Secourus,
    /// cellules débroussaillées (le combustible a changé)
    pub carte_modifiee: IndexSet<usize>,
    pub config_moyens: IndexMap<&'static str, i64>,
    pub moyens: Vec<Moyen>,
    pub ia: Commandement,
    pub evac: Evacuation,
    pub prochain_depart: i64,
    pub renforts_attente: Vec<(i64, i64)>,
    pub danger: HashSet<usize>,
    pub trafic_engins: HashMap<usize, i64>,
    pub trafic_convois: HashMap<usize, i64>,
    pub hopital: usize,
}

impl Partie {
    pub fn new(graine: i64, theme: &str, scenario: &str, moyens: &Config, doctrine: &str, premier_feu: bool) -> Partie {
        let rnd = Hasard::new(graine.wrapping_mul(1009).wrapping_add(3));
        let carte = Carte::new(graine, theme);
        let meteo = Meteo::new(scenario, graine, 11 * 60);
        let feu = Feu::new(&carte);
        let mut config_moyens = IndexMap::new();
        for &(k, a, b) in LIMITES {
            let defaut = MOYENS_DEFAUT.iter().find(|d| d.0 == k).map_or(0, |d| d.1);
            config_moyens.insert(k, moyens.get(k).copied().unwrap_or(defaut).clamp(a, b));
        }
        let evac = Evacuation::new(&carte);
        let mut p = Partie {
            graine,
            theme: theme.to_string(),
            scenario: scenario.to_string(),
            rnd,
            carte,
            meteo,
            feu,
            t: 0,
            reg: Registre::default(),
            evacues: HashSet::new(),
            stats: Stats::default(),
            secourus: Secourus::default(),
            carte_modifiee: IndexSet::new(),
            config_moyens,
            moyens: Vec::new(),
            ia: Commandement::new(doctrine),
            evac,
            prochain_depart: 0,
            renforts_attente: Vec::new(),
            danger: HashSet::new(),
            trafic_engins: HashMap::new(),
            trafic_convois: HashMap::new(),
            hopital: 0,
        };
        p.creer_moyens();
        p.prochain_depart = if premier_feu { 25 } else { p.delai_depart() };
        let txt = format!("{} — {}. Risque {}.", p.meteo.heure(), p.meteo.sc.nom, p.meteo.niveau.to_lowercase());
        p.journal_ajout("info", txt);
        p
    }

    // ------------------------------------------------------------------ moyens
    fn creer_moyens(&mut self) {
        let c = &self.carte;
        let cm = |k: &str| self.config_moyens.get(k).copied().unwrap_or(0);
        let cas: Vec<usize> = if c.casernes.is_empty() { vec![c.g.n / 2] } else { c.casernes.iter().map(|k| k.cell).collect() };
        let mut l: Vec<Moyen> = Vec::new();
        let ajouter = |l: &mut Vec<Moyen>, kind: &'static str, nom: String, base: usize, groupe: Option<String>| {
            let mut u = Moyen::new(l.len(), kind, nom, base, c);
            u.groupe = groupe;
            l.push(u);
        };
        // les camions vont par groupes (GIFF) : 4 CCF et le VLHR du chef de groupe, partis de la même caserne
        for k in 0..cm("ccf") as usize {
            let giff = k / 4 + 1;
            let ca = cas[(giff - 1) % cas.len()];
            if k % 4 == 0 {
                ajouter(&mut l, "vlhr", format!("Chef GIFF {giff}"), ca, Some(format!("GIFF {giff}")));
            }
            ajouter(&mut l, "ccf", format!("CCF {giff}.{}", k % 4 + 1), ca, Some(format!("GIFF {giff}")));
        }
        for k in 0..cm("bull") as usize {
            ajouter(&mut l, "bull", format!("Bull {}", k + 1), cas[(k + 1) % cas.len()], None);
        }
        let heli = c.helistation.unwrap_or(cas[0]);
        for k in 0..cm("helico") as usize {
            ajouter(&mut l, "helico", format!("HBE {}", k + 1), heli, None);
        }
        let aero = c.aerodrome.unwrap_or(cas[0]);
        for k in 0..cm("canadair") as usize {
            // les Canadair volent par deux
            ajouter(&mut l, "canadair", format!("Pélican {}", 31 + k), aero, Some(format!("Patrouille {}", k / 2 + 1)));
        }
        for k in 0..cm("dash") as usize {
            ajouter(&mut l, "dash", format!("Milan {}", 71 + k), aero, None);
        }
        let csp = cas[0];
        for k in 0..cm("fpt") as usize {
            ajouter(&mut l, "fpt", format!("FPT {}", k + 1), cas[k % cas.len().min(2)], None);
        }
        for k in 0..cm("vpc") as usize {
            ajouter(&mut l, "vpc", format!("PC {}", k + 1), csp, None);
        }
        for k in 0..cm("debr") as usize {
            ajouter(&mut l, "debr", format!("Débroussailleuse {}", k + 1), cas[(k + 1) % cas.len()], None);
        }
        for k in 0..cm("ravit") as usize {
            ajouter(&mut l, "ravit", format!("Ravitailleur {}", k + 1), csp, None);
        }
        for k in 0..cm("vsav") as usize {
            ajouter(&mut l, "vsav", format!("VSAV {}", k + 1), cas[k % cas.len()], None);
        }
        for k in 0..cm("dragon") as usize {
            ajouter(&mut l, "dragon", format!("Dragon {}", 33 + k), heli, None);
        }
        self.hopital = c.sites.iter().find(|s| s.type_ == "ville").map_or(csp, |s| s.centre);
        self.moyens = l;
    }

    pub fn nb_kind(&self, kind: &str) -> usize {
        self.moyens.iter().filter(|u| u.kind == kind && !u.hs).count()
    }

    // ------------------------------------------------------------------ blessés et secours
    pub fn ajouter_blesses(&mut self, cell: usize, n: i64, typ: &str, gravite: &str, quoi: &str) {
        self.reg.blesses_ajout(&self.meteo, self.t, cell, n, typ, gravite, quoi.to_string());
    }

    pub fn deposer(&mut self, u: &mut Moyen) {
        let n: i64 = u.charge.iter().map(|x| x.n).sum();
        if n > 0 {
            self.secourus.hopital += n;
            self.secourus.graves += u.charge.iter().filter(|x| x.g != "leger").map(|x| x.n).sum::<i64>();
            self.reg.ajout(&self.meteo, "sauve", format!("🏥 {} dépose {} blessé{} à l'hôpital.", u.nom, n, if n > 1 { "s" } else { "" }));
        }
        u.charge.clear();
    }

    /// Secours aux blessés (Dragon d'abord pour les graves et les équipages, VSAV sinon) et ravitaillement en gazole.
    /// Ces moyens obéissent à la logistique automatique, sauf s'ils sont sous vos ordres.
    fn logistique(&mut self) {
        let t = self.t;
        let mut messages = Vec::new();
        for b in self.reg.blesses.iter_mut() {
            if b.etat != "attente" {
                continue;
            }
            let attente = t - b.t0;
            if b.g == "danger" && attente > 20 {
                b.g = "grave".into();
                let s = if b.n > 1 { "s" } else { "" };
                messages.push(format!("⚠ {} : personne n'est venu, {} blessé{s} grave{s}.", b.quoi, b.n));
            }
            if b.g == "grave" && attente > 80 && !b.aggrave {
                b.aggrave = true;
                self.secourus.aggraves += b.n;
            }
        }
        for m in messages {
            self.reg.ajout(&self.meteo, "alerte", m);
        }
        self.reg.blesses.retain(|b| b.etat == "attente" || t - b.t0 < 30);
        let mut pris: HashSet<usize> =
            self.moyens.iter().filter_map(|u| u.mission.as_ref().filter(|m| m.type_ == "secours").and_then(|m| m.cible)).collect();
        let mut attente: Vec<Blesse> = self.reg.blesses.iter().filter(|b| b.etat == "attente").cloned().collect();
        trier(&mut attente, |b| (b.g == "leger", b.t0));
        let g = &self.carte.g;
        for b in attente {
            if pris.contains(&b.id) {
                continue;
            }
            let urgent = b.g == "danger" || b.g == "grave" || b.t == "equipage";
            let mut choix = None;
            if urgent || self.nb_kind("vsav") == 0 {
                let libres = (0..self.moyens.len()).filter(|&k| {
                    let u = &self.moyens[k];
                    u.kind == "dragon" && u.mission.is_none() && !u.joueur && !u.hs
                });
                choix = argmin(libres, |k| g.distance(self.moyens[k].cell, b.cell));
            }
            if choix.is_none() && !(b.t == "equipage" && self.feu.etat[b.cell] == BRULE) {
                let libres: Vec<usize> = (0..self.moyens.len())
                    .filter(|&k| {
                        let u = &self.moyens[k];
                        u.kind == "vsav" && u.mission.is_none() && !u.joueur && !u.hs && u.charge.is_empty() && u.chemin_vers(self, b.cell, None).is_some()
                    })
                    .collect();
                choix = argmin(libres, |k| g.distance(self.moyens[k].cell, b.cell));
            }
            if let Some(k) = choix {
                self.moyens[k].mission = Some(Mission {
                    type_: "secours".into(),
                    cible: Some(b.id),
                    poste: Some(b.cell),
                    label: format!("Secours : {}", b.quoi),
                    ..Default::default()
                });
                pris.insert(b.id);
            }
        }
        // gazole : le ravitailleur va au camion engagé le plus à sec
        let mut vises: HashSet<usize> =
            self.moyens.iter().filter_map(|u| u.mission.as_ref().filter(|m| m.type_ == "ravitailler").and_then(|m| m.cible)).collect();
        let ravits: Vec<usize> =
            (0..self.moyens.len()).filter(|&k| self.moyens[k].kind == "ravit" && self.moyens[k].mission.is_none() && !self.moyens[k].joueur && !self.moyens[k].hs).collect();
        for r in ravits {
            let rcell = self.moyens[r].cell;
            let besoin = self.moyens.iter().filter(|u| {
                u.carb_max.is_some_and(|cm| u.kind != "ravit" && !u.hs && !vises.contains(&u.id) && u.carb < 0.35 * cm && u.cell != u.base)
            });
            let Some(u) = argmin(besoin, |u| (u.carb / u.carb_max.unwrap_or(1.0), g.distance(u.cell, rcell))) else { break };
            let (uid, ucell, unom) = (u.id, u.cell, u.nom.clone());
            self.moyens[r].mission = Some(Mission {
                type_: "ravitailler".into(),
                cible: Some(uid),
                poste: Some(ucell),
                label: format!("Gazole pour {unom}"),
                ..Default::default()
            });
            vises.insert(uid);
        }
        // le ravitailleur suit sa cible si elle bouge
        let mut suivis = Vec::new();
        for (k, r) in self.moyens.iter().enumerate() {
            let Some(m) = r.mission.as_ref() else { continue };
            if r.kind != "ravit" || m.type_ != "ravitailler" {
                continue;
            }
            if let Some(u) = m.cible.and_then(|id| self.moyens.iter().find(|x| x.id == id)) {
                if m.poste != Some(u.cell) && u.etat != "route" {
                    suivis.push((k, u.cell));
                }
            }
        }
        for (k, cell) in suivis {
            if let Some(m) = self.moyens[k].mission.as_mut() {
                m.poste = Some(cell);
            }
        }
    }

    pub fn demander_renfort(&mut self, nb: i64) {
        self.renforts_attente.push((self.t + 45, nb));
        self.journal_ajout("cos", format!("Demande de renforts : une colonne de {nb} CCF arrivera dans 45 min."));
    }

    fn arrivee_renforts(&mut self) {
        for item in self.renforts_attente.clone() {
            let (t, nb) = item;
            if self.t < t {
                continue;
            }
            if let Some(pos) = self.renforts_attente.iter().position(|x| *x == item) {
                self.renforts_attente.remove(pos);
            }
            let c = &self.carte;
            let g = &c.g;
            let bord = |i: usize| g.col_row(i).0 == g.cols - 1;
            let e = (0..g.n)
                .find(|&i| bord(i) && c.route[i] >= 2)
                .or_else(|| (0..g.n).find(|&i| bord(i) && !est_eau(c.fuel[i])))
                .unwrap_or_else(|| c.casernes.first().map_or(0, |k| k.cell));
            self.stats.renforts += 1;
            let r = self.stats.renforts;
            let mut chef = Moyen::new(self.moyens.len(), "vlhr", format!("Chef colonne {r}"), e, c);
            chef.renfort = true;
            chef.groupe = Some(format!("Colonne {r}"));
            self.moyens.push(chef);
            for k in 0..nb {
                let mut u = Moyen::new(self.moyens.len(), "ccf", format!("CCF R{r}.{}", k + 1), e, &self.carte);
                u.renfort = true;
                u.groupe = Some(format!("Colonne {r}"));
                self.moyens.push(u);
            }
            self.journal_ajout("cos", format!("La colonne de renfort n° {r} arrive ({nb} CCF)."));
        }
    }

    /// Chaque VLHR suit son groupe : il se poste en retrait du poste le plus tenu par ses camions.
    fn chefs(&mut self) {
        let g = &self.carte.g;
        let mut groupes: IndexMap<String, Vec<usize>> = IndexMap::new();
        for (k, u) in self.moyens.iter().enumerate() {
            if u.kind == "ccf" && u.mission.as_ref().is_some_and(|m| m.poste.is_some()) {
                if let Some(gr) = &u.groupe {
                    groupes.entry(gr.clone()).or_default().push(k);
                }
            }
        }
        for c in 0..self.moyens.len() {
            if self.moyens[c].kind != "vlhr" {
                continue;
            }
            let l = self.moyens[c].groupe.as_ref().and_then(|gr| groupes.get(gr)).cloned().unwrap_or_default();
            if l.is_empty() {
                self.moyens[c].mission = None;
                self.moyens[c].joueur = false;
                continue;
            }
            let postes: Vec<usize> = l.iter().filter_map(|&k| self.moyens[k].mission.as_ref().and_then(|m| m.poste)).collect();
            let mut uniques: Vec<usize> = Vec::new();
            for &q in &postes {
                if !uniques.contains(&q) {
                    uniques.push(q);
                }
            }
            let poste = crate::argmax(uniques, |q| postes.iter().filter(|&&x| x == q).count()).unwrap_or(postes[0]);
            let premier = self.moyens[l[0]].mission.as_ref().expect("mission du groupe");
            let (d, lab) = (premier.dir, premier.label.clone());
            let mut retrait = poste;
            if let Some(d) = d {
                let a = ((d + 180) as f64).to_radians();
                // en retrait, hors de portée des flammes
                for n in 1..=3 {
                    let n = n as f64;
                    if let Some(j) = g.sous(g.cx[poste] + a.cos() * g.hex_m * n, g.cy[poste] + a.sin() * g.hex_m * n) {
                        if self.feu.etat[j] != BRULE && !est_eau(self.carte.fuel[j]) && !self.danger.contains(&j) {
                            retrait = j;
                            break;
                        }
                    }
                }
            }
            let joueur = l.iter().any(|&k| self.moyens[k].joueur);
            let chef = &mut self.moyens[c];
            if chef.mission.as_ref().map_or(true, |m| m.poste != Some(retrait)) {
                chef.mission = Some(Mission { type_: "escorte".into(), poste: Some(retrait), label: format!("Commande : {lab}"), ..Default::default() });
            }
            chef.joueur = joueur;
        }
    }

    /// Point d'eau terrestre le plus proche (borne, citerne DFCI, caserne).
    pub fn point_eau_proche(&self, cell: usize) -> Option<usize> {
        let g = &self.carte.g;
        let pts = self.carte.points_eau.iter().copied().chain(self.carte.casernes.iter().map(|c| c.cell));
        argmin(pts, |q| g.distance(q, cell))
    }

    /// Où un aéronef va chercher son eau : un plan d'eau écopable (Canadair) ou n'importe quelle eau (hélico).
    pub fn eau_aerienne(&self, u: &Moyen) -> Option<usize> {
        let c = &self.carte;
        let g = &c.g;
        let mut cand: Vec<usize> =
            if u.kind == "canadair" { (0..g.n).filter(|&i| c.ecopable(i)).collect() } else { (0..g.n).filter(|&i| c.pompable(i)).collect() };
        if u.kind != "canadair" {
            if let Some(h) = c.helistation {
                cand.push(h);
            }
        }
        // au plus près de la cible si on la connaît, sinon de l'avion
        let mut reference = u.cell;
        if let Some(fe) = u.mission.as_ref().and_then(|m| m.feu) {
            if let Some(t) = self.ia.feux.get(&racine(&self.ia.alias, fe)).and_then(|f| f.tete) {
                reference = t;
            }
        }
        argmin(cand, |i| g.distance(i, reference) * 2 + g.distance(i, u.cell))
    }

    // ------------------------------------------------------------------ journal et événements
    pub fn journal_ajout(&mut self, kind: &str, txt: String) {
        self.reg.ajout(&self.meteo, kind, txt);
    }

    pub fn evenement(&mut self, type_: &str, kw: Value) {
        self.reg.evenement(&self.meteo, type_, kw);
    }

    // ------------------------------------------------------------------ départs de feu et détection
    fn delai_depart(&mut self) -> i64 {
        let taux = self.meteo.sc.departs * (0.3 + self.meteo.danger);
        self.t + self.rnd.expovariate(taux / 140.0) as i64 + 10
    }

    fn departs(&mut self) {
        let n = self.carte.g.n;
        // foudre
        if self.meteo.orage && self.rnd.random() < 0.035 {
            let tous: Vec<usize> = (0..n).collect();
            let cand = self.rnd.sample(&tous, 60).into_iter().find(|&i| b"PJFQG".contains(&self.carte.fuel[i]));
            if let Some(i) = cand {
                if self.feu.allumer(&self.carte, i, -1, None, "foudre", self.t) {
                    self.stats.departs += 1;
                    self.evenement("foudre", json!({"cell": i}));
                }
            }
        }
        if self.meteo.nuit || self.t < self.prochain_depart {
            return;
        }
        self.prochain_depart = self.delai_depart();
        // causes humaines : bords de routes, abords des villages, travaux en forêt
        for _ in 0..80 {
            let i = self.rnd.randrange(n);
            let c = &self.carte;
            if !brule(c.fuel[i]) || HABITE.contains(&c.fuel[i]) || self.feu.etat[i] != INTACT {
                continue;
            }
            // malus des routes : mégots, véhicules
            let pres = c.route[i] >= 2 || c.g.vois[i].iter().any(|v| c.site_de[v.j] >= 0 || c.route[v.j] >= 2);
            if pres || self.rnd.random() < 0.15 {
                let causes = [
                    "mégot jeté d'une voiture", "travaux agricoles", "barbecue", "ligne électrique", "engin forestier", "origine inconnue", "malveillance présumée",
                ];
                let cause = *self.rnd.choice(&causes);
                if self.feu.allumer(&self.carte, i, -1, None, cause, self.t) {
                    self.stats.departs += 1;
                }
                break;
            }
        }
    }

    fn detection(&mut self) {
        let avions: Vec<(usize, String)> = self
            .moyens
            .iter()
            .filter(|u| !u.terre && u.mission.as_ref().is_some_and(|m| m.type_ == "gaar"))
            .map(|u| (u.cell, u.nom.clone()))
            .collect();
        for k in 0..self.feu.departs.len() {
            if self.feu.departs[k].detecte {
                continue;
            }
            let (id, i0, cause) = {
                let d = &self.feu.departs[k];
                (d.id as i64, d.cell, d.cause.clone())
            };
            let g = &self.carte.g;
            let n = self.feu.brulant.iter().filter(|&&i| self.feu.foyer[i] == id).count().max(1) as f64;
            let (mut p, mut qui): (f64, Option<String>) = (0.0, None);
            for &v in &self.carte.vigies {
                if g.distance(v, i0) <= 22 {
                    p = p.max(0.10 + 0.02 * n);
                    qui = Some("la vigie".into());
                }
            }
            if g.anneau(i0, 4).iter().any(|&j| self.carte.site_de[j] >= 0) && 0.08 + 0.02 * n > p {
                p = 0.08 + 0.02 * n;
                qui = Some("des habitants".into());
            }
            for (cell, nom) in &avions {
                if g.distance(*cell, i0) <= 9 {
                    p = 0.7;
                    qui = Some(nom.clone());
                }
            }
            p = p.max(0.02 + 0.01 * n);
            if self.meteo.nuit {
                p *= 0.5;
            }
            if self.rnd.random() < p {
                let d = &mut self.feu.departs[k];
                d.detecte = true;
                d.t_detect = Some(self.t);
                let qui = qui.unwrap_or_else(|| "un promeneur".into());
                let lieu = self.ia.nommer(&self.carte, i0).replace("Feu ", "");
                self.journal_ajout("alerte", format!("🔥 Départ de feu signalé {lieu} par {qui} ({cause})."));
                self.evenement("alerte", json!({"cell": i0}));
            }
        }
    }

    // ------------------------------------------------------------------ pas d'une minute
    pub fn pas(&mut self) {
        self.t += 1;
        self.meteo.pas();
        self.departs();
        self.arrivee_renforts();
        for u in &self.moyens {
            if u.terre && u.mission.as_ref().is_some_and(|m| m.type_ == "defense" && m.poste == Some(u.cell)) {
                self.feu.defense[u.cell] += 1;
            }
        }
        self.feu.pas(&self.carte, &self.meteo, &mut self.rnd, self.t);
        for k in std::mem::take(&mut self.feu.nouvelles_sautes) {
            self.evenement("saute", json!({"cell": k}));
        }
        self.detection();
        self.danger = HashSet::new();
        self.trafic_engins = HashMap::new();
        self.trafic_convois = HashMap::new();
        for u in &self.moyens {
            if u.terre {
                *self.trafic_engins.entry(u.cell).or_insert(0) += 1;
            }
        }
        for cv in &self.evac.convois {
            if cv.etat != "arrive" {
                *self.trafic_convois.entry(cv.cell).or_insert(0) += 1;
            }
        }
        for &i in &self.feu.brulant {
            if self.feu.i[i] > 0.55 {
                self.danger.extend(self.carte.g.voisins(i));
            }
        }
        let mut ia = std::mem::take(&mut self.ia);
        ia.pas(self);
        self.ia = ia;
        self.chefs();
        self.logistique();
        for k in 0..self.moyens.len() {
            // le moyen sort de la liste le temps de son pas : il lit et modifie la partie (et les autres moyens)
            let mut u = self.moyens[k].clone();
            u.pas(self);
            self.moyens[k] = u;
        }
        let mut ev = std::mem::take(&mut self.evac);
        ev.pas(self);
        self.evac = ev;
    }

    // ------------------------------------------------------------------ actions du joueur
    fn cellule(&self, v: Option<&Value>, cle: &str) -> Result<usize, String> {
        let i = entier(v.ok_or_else(|| format!("'{cle}'"))?)?;
        if i < 0 || i as usize >= self.carte.g.n {
            return Err(format!("cellule hors de la carte : {i}"));
        }
        Ok(i as usize)
    }

    fn cellule_ou_rien(&self, a: &Value, cle: &str) -> Result<Option<usize>, String> {
        match a.get(cle) {
            None | Some(Value::Null) => Ok(None),
            v => self.cellule(v, cle).map(Some),
        }
    }

    /// Une action de la page ; Err : requête mal formée (le serveur répond 500, comme l'ancienne version).
    pub fn action(&mut self, a: &Value) -> Result<Value, String> {
        let typ = a.get("type").and_then(Value::as_str).unwrap_or("");
        match typ {
            "allumer" => {
                let i = self.cellule(a.get("cell"), "cell")?;
                if self.feu.allumer(&self.carte, i, -1, None, "départ provoqué (joueur)", self.t) {
                    self.stats.departs += 1;
                    if a.get("detecte").map_or(true, vrai) {
                        let f = self.feu.foyer[i] as usize;
                        self.feu.departs[f].detecte = true;
                        self.feu.departs[f].t_detect = Some(self.t);
                        let lieu = self.ia.nommer(&self.carte, i).replace("Feu ", "");
                        self.journal_ajout("alerte", format!("🔥 Départ de feu provoqué {lieu}."));
                    }
                    return Ok(json!({"ok": true}));
                }
                Ok(json!({"ok": false, "erreur": "Rien à brûler ici."}))
            }
            "orage" => {
                let h = self.meteo.minute_du_jour();
                self.meteo.sc.orage = Some((h, h + 120));
                self.journal_ajout("info", "Un orage sec éclate : la foudre frappe la forêt.".into());
                Ok(json!({"ok": true}))
            }
            "vent" => {
                if let Some(de) = a.get("de") {
                    self.meteo.de_base = nombre(de)?.rem_euclid(360.0);
                    self.meteo.sc.de = self.meteo.de_base;
                    self.meteo.sc.bascule = None;
                }
                if let Some(f) = a.get("force") {
                    self.meteo.sc.vent = nombre(f)?.clamp(0.0, 110.0);
                }
                self.meteo.maj();
                let txt = format!("Le vent tourne : du {}° à {} km/h.", repr(self.meteo.de), rond(self.meteo.vent));
                self.journal_ajout("info", txt);
                Ok(json!({"ok": true}))
            }
            "ordre" | "apercu" => {
                let mut ids = Vec::new();
                if let Some(l) = a.get("moyens").and_then(Value::as_array) {
                    for x in l {
                        ids.push(entier(x)?);
                    }
                }
                let ids: Vec<usize> = ids.into_iter().filter(|&x| x >= 0).map(|x| x as usize).collect();
                let ordre = a.get("ordre").and_then(Value::as_str).unwrap_or("");
                let cell = self.cellule_ou_rien(a, "cell")?;
                let cell2 = self.cellule_ou_rien(a, "cell2")?;
                let d = match a.get("dir") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(nombre(v)?),
                };
                Ok(if typ == "ordre" { ordres::donner(self, &ids, ordre, cell, cell2, d) } else { ordres::apercu(self, &ids, ordre, cell, cell2, d) })
            }
            "commandement" => {
                let v = if a.get("valeur").and_then(Value::as_str) == Some("manuel") { "manuel" } else { "auto" };
                if v != self.ia.mode {
                    self.ia.mode = v;
                    if v == "manuel" {
                        self.journal_ajout(
                            "joueur",
                            "Vous prenez le commandement des opérations. Les moyens gardent leur mission en cours ; à vous de donner les ordres.".into(),
                        );
                        for u in &mut self.moyens {
                            if u.mission.as_ref().is_some_and(|m| m.type_ == "preposition" || m.type_ == "gaar") {
                                u.mission = None;
                            }
                        }
                    } else {
                        self.journal_ajout("cos", "L'IA reprend le commandement des opérations (vos ordres en cours sont respectés).".into());
                    }
                }
                Ok(json!({"ok": true}))
            }
            "evacuer" | "confiner" => {
                let sid = entier(a.get("site").ok_or("'site'")?)?;
                let ok = sid >= 0 && {
                    let ia = std::mem::take(&mut self.ia);
                    let mut ev = std::mem::take(&mut self.evac);
                    let ok = if typ == "evacuer" { ev.evacuer(self, &ia.feux, sid as usize, "joueur") } else { ev.confiner(self, sid as usize, "joueur") };
                    self.evac = ev;
                    self.ia = ia;
                    ok
                };
                let erreur = if ok {
                    Value::Null
                } else if typ == "evacuer" {
                    json!("Aucun itinéraire sûr pour évacuer.")
                } else {
                    json!("Déjà fait.")
                };
                Ok(json!({"ok": ok, "erreur": erreur}))
            }
            "renfort" => {
                if self.renforts_attente.len() >= 2 {
                    return Ok(json!({"ok": false, "erreur": "Deux colonnes sont déjà en route."}));
                }
                self.demander_renfort(4);
                Ok(json!({"ok": true}))
            }
            "doctrine" => {
                self.ia.doctrine = a.get("valeur").and_then(Value::as_str).unwrap_or("equilibree").to_string();
                let txt = format!("Doctrine du commandement : {}.", self.ia.doctrine);
                self.journal_ajout("cos", txt);
                Ok(json!({"ok": true}))
            }
            _ => Ok(json!({"ok": false, "erreur": "action inconnue"})),
        }
    }

    // ------------------------------------------------------------------ export
    pub fn export_carte(&self) -> Value {
        let mut d = self.carte.export();
        let mut cm = Map::new();
        for (k, v) in &self.config_moyens {
            cm.insert(k.to_string(), json!(v));
        }
        d["moyensConfig"] = Value::Object(cm);
        d
    }

    pub fn bilan(&self) -> (Vec<Value>, Value) {
        let (feu, c) = (&self.feu, &self.carte);
        let mut sites = Vec::new();
        let mut parts = Vec::new();
        for s in &c.sites {
            let brule = s.cells.iter().filter(|&&i| matches!(feu.etat[i], BRULE | BRULE_FINI)).count();
            let part = arrondi(brule as f64 / s.cells.len().max(1) as f64, 2);
            let menace = self
                .ia
                .feux
                .values()
                .filter(|f| f.statut != "éteint")
                .map(|f| f.menaces.get(&s.id).copied().unwrap_or(9999))
                .min()
                .unwrap_or(9999)
                .min(9999);
            let etat = if part > 0.6 {
                "détruit"
            } else if brule > 0 {
                "touché"
            } else if menace < 150 {
                "menacé"
            } else {
                "sauf"
            };
            parts.push(part);
            sites.push(json!({
                "id": s.id, "etat": etat, "part": part, "eta": if menace < 9999 { json!(menace) } else { Value::Null },
                "evacue": self.evacues.contains(&s.id), "evac": self.evac.sites.get(&s.id).map(|e| e.statut),
                "defenseurs": self.moyens.iter().filter(|u| u.mission.as_ref().is_some_and(|m| m.site == Some(s.id))).count(),
            }));
        }
        let ha = feu.surface_ha(None);
        let perdus: f64 = c.sites.iter().zip(&parts).map(|(s, p)| p * s.valeur as f64).sum();
        let pers = self.evac.bilan();
        let attente: i64 = self.reg.blesses.iter().filter(|b| b.etat == "attente").map(|b| b.n).sum();
        let score = (100.0 - 12.0 * (1.0 + ha).log10() - perdus * 0.6 - pers.exposes as f64 / 40.0 - 4.0 * self.stats.perdus as f64
            - 2.0 * self.secourus.aggraves as f64
            - 0.5 * attente as f64)
            .round_ties_even()
            .max(0.0) as i64;
        let maisons: f64 = c
            .sites
            .iter()
            .zip(&parts)
            .filter(|(s, _)| s.type_ == "ville" || s.type_ == "village")
            .map(|(s, p)| p * s.pop as f64 / 3.0)
            .sum();
        let stats = json!({
            "ha": arrondi(ha, 1), "litres": rond(feu.litres), "litresRetardant": rond(feu.litres_ret),
            "largages": self.stats.largages, "departs": self.stats.departs, "renforts": self.stats.renforts,
            "defriche": self.stats.defriche, "score": score,
            "blesses": {"attente": attente, "hopital": self.secourus.hopital, "graves": self.secourus.graves, "aggraves": self.secourus.aggraves},
            "debroussaille": self.stats.debroussaille,
            "pannes": self.moyens.iter().filter(|u| u.etat == "panne").count(),
            "personnes": {"evacues": pers.evacues, "confines": pers.confines, "exposes": pers.exposes, "route": pers.route},
            "perdus": self.stats.perdus, "autoprotections": self.stats.autoprotections,
            "feuxActifs": self.ia.feux.values().filter(|f| f.statut != "éteint").count(),
            "maisons": rond(maisons),
        });
        (sites, stats)
    }

    pub fn export_etat(&self, depuis: i64) -> Value {
        let (feu, c) = (&self.feu, &self.carte);
        let n = c.g.n;
        let etat: String = (0..n)
            .map(|i| {
                if feu.etat[i] == INTACT && c.fuel[i] == b'X' && Some(i) != c.helistation {
                    'x'
                } else {
                    ['.', 'f', 'b', 'e', 'n'][feu.etat[i] as usize]
                }
            })
            .collect();
        let (sites, stats) = self.bilan();
        let mut fuel_mod = Map::new();
        for &i in &self.carte_modifiee {
            fuel_mod.insert(i.to_string(), json!((c.fuel[i] as char).to_string()));
        }
        let journal: Vec<Value> = self
            .reg
            .journal
            .iter()
            .filter(|j| j.id > depuis)
            .map(|j| json!({"id": j.id, "t": j.t, "h": j.h, "k": j.k, "txt": j.txt}))
            .collect();
        json!({
            "t": self.t, "meteo": self.meteo.export(), "etat": etat,
            "feu": feu.brulant.iter().map(|&i| json!([i, rond(feu.i[i] * 100.0), rond(feu.prog[i] * 100.0), feu.dfrom[i]])).collect::<Vec<_>>(),
            "prog": (0..n).filter(|&i| matches!(feu.etat[i], ETEINT | NOYE)).map(|i| json!([i, rond(feu.prog[i] * 100.0)])).collect::<Vec<_>>(),
            "mouille": (0..n).filter(|&i| feu.mouille[i] > 0.08).map(|i| json!([i, rond(feu.mouille[i] * 100.0)])).collect::<Vec<_>>(),
            "ret": (0..n).filter(|&i| feu.ret[i] > 0).collect::<Vec<_>>(),
            "moyens": self.moyens.iter().map(|u| u.export()).collect::<Vec<_>>(), "feux": self.ia.export(self), "sites": sites, "stats": stats,
            "journal": journal, "evts": self.reg.evts.iter().filter(|e| e["id"].as_i64().unwrap_or(0) > depuis).collect::<Vec<_>>(),
            "seq": self.reg.seq, "doctrine": self.ia.doctrine, "commandement": self.ia.mode, "evac": self.evac.export(c),
            "departs": feu.departs.iter().map(|d| json!({"cell": d.cell, "detecte": d.detecte})).collect::<Vec<_>>(),
            "blesses": self.reg.blesses.iter().filter(|b| b.etat == "attente")
                .map(|b| json!({"id": b.id, "cell": b.cell, "n": b.n, "t": b.t, "g": b.g, "quoi": b.quoi})).collect::<Vec<_>>(),
            "fuelMod": fuel_mod, "hopital": self.hopital,
        })
    }
}
