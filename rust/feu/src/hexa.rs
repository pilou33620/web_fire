//! La grille d'hexagones (pointe en haut, lignes impaires décalées, comme web_conquete et web_power).
//!
//! Repère : x vers l'est, y vers le sud, en mètres. Un hexagone fait `hex_m` mètres d'un bord plat à l'autre.
//! Cellule i = row * cols + col ; coordonnées axiales q = col - (row - (row & 1)) / 2, r = row.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use indexmap::IndexMap;

pub const SQ3: f64 = 1.732_050_807_568_877_2;
/// Axiales : E, NE, NO, O, SO, SE.
pub const DIRS: [(i64, i64); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// Un voisin j de la cellule i : direction d (0..5) de i vers j, vecteur unitaire (ux, uy).
#[derive(Clone, Copy, Debug)]
pub struct Voisin {
    pub j: usize,
    pub d: usize,
    pub ux: f64,
    pub uy: f64,
}

pub struct Grille {
    pub cols: usize,
    pub rows: usize,
    pub hex_m: f64,
    /// rayon (centre -> coin)
    pub r: f64,
    /// les vitesses du jeu sont réglées pour des hexagones de 300 m
    pub ech: f64,
    pub n: usize,
    pub cx: Vec<f64>,
    pub cy: Vec<f64>,
    pub q: Vec<i64>,
    pub rr: Vec<i64>,
    pub largeur: f64,
    pub hauteur: f64,
    pub vois: Vec<Vec<Voisin>>,
}

impl Grille {
    pub fn new(cols: usize, rows: usize, hex_m: f64) -> Grille {
        let r = hex_m / SQ3;
        let n = cols * rows;
        let (mut cx, mut cy, mut q, mut rr) = (vec![0.0; n], vec![0.0; n], vec![0i64; n], vec![0i64; n]);
        for row in 0..rows {
            for col in 0..cols {
                let i = row * cols + col;
                let ro = row as i64;
                q[i] = col as i64 - (ro - (ro & 1)) / 2;
                rr[i] = ro;
                cx[i] = hex_m * (col as f64 + 0.5 * (row & 1) as f64) + hex_m / 2.0;
                cy[i] = 1.5 * r * row as f64 + r;
            }
        }
        let mut g = Grille {
            cols,
            rows,
            hex_m,
            r,
            ech: 300.0 / hex_m,
            n,
            cx,
            cy,
            q,
            rr,
            largeur: hex_m * (cols as f64 + 0.5),
            hauteur: 1.5 * r * (rows as f64 - 1.0) + 2.0 * r,
            vois: Vec::new(),
        };
        let mut vois = Vec::with_capacity(n);
        for i in 0..n {
            let mut l = Vec::with_capacity(6);
            for (d, &(a, b)) in DIRS.iter().enumerate() {
                if let Some(j) = g.index(g.q[i] + a, g.rr[i] + b) {
                    let (dx, dy) = (g.cx[j] - g.cx[i], g.cy[j] - g.cy[i]);
                    let lg = dx.hypot(dy);
                    l.push(Voisin { j, d, ux: dx / lg, uy: dy / lg });
                }
            }
            vois.push(l);
        }
        g.vois = vois;
        g
    }

    /// La cellule de coordonnées axiales (q, r), si elle est sur la carte.
    pub fn index(&self, q: i64, r: i64) -> Option<usize> {
        if r < 0 || r >= self.rows as i64 {
            return None;
        }
        let col = q + (r - (r & 1)) / 2;
        if col < 0 || col >= self.cols as i64 {
            return None;
        }
        Some(r as usize * self.cols + col as usize)
    }

    pub fn col_row(&self, i: usize) -> (usize, usize) {
        (i % self.cols, i / self.cols)
    }

    pub fn voisins(&self, i: usize) -> Vec<usize> {
        self.vois[i].iter().map(|v| v.j).collect()
    }

    /// Distance en hexagones.
    pub fn distance(&self, i: usize, j: usize) -> i64 {
        let (dq, dr) = (self.q[i] - self.q[j], self.rr[i] - self.rr[j]);
        (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
    }

    pub fn dist_m(&self, i: usize, j: usize) -> f64 {
        (self.cx[i] - self.cx[j]).hypot(self.cy[i] - self.cy[j])
    }

    /// La cellule sous un point (mètres), ou None hors de la carte.
    pub fn sous(&self, x: f64, y: f64) -> Option<usize> {
        let x = x - self.hex_m / 2.0;
        let y = y - self.r;
        let q = (SQ3 / 3.0 * x - y / 3.0) / self.r;
        let r = (2.0 / 3.0 * y) / self.r;
        let s = -q - r;
        let (mut rq, mut rr, rs) = (q.round_ties_even(), r.round_ties_even(), s.round_ties_even());
        let (dq, dr, ds) = ((rq - q).abs(), (rr - r).abs(), (rs - s).abs());
        if dq > dr && dq > ds {
            rq = -rr - rs;
        } else if dr > ds {
            rr = -rq - rs;
        }
        if !rq.is_finite() || !rr.is_finite() {
            return None;
        }
        // les coordonnées axiales de la grille sont décalées d'une demi-colonne sur les lignes impaires
        self.index(rq as i64, rr as i64)
    }

    /// Toutes les cellules à au plus `rayon` hexagones de i.
    pub fn anneau(&self, i: usize, rayon: i64) -> Vec<usize> {
        let mut out = Vec::new();
        let (q0, r0) = (self.q[i], self.rr[i]);
        for dq in -rayon..=rayon {
            for dr in (-rayon).max(-dq - rayon)..=rayon.min(-dq + rayon) {
                if let Some(j) = self.index(q0 + dq, r0 + dr) {
                    out.push(j);
                }
            }
        }
        out
    }

    /// Les cellules traversées par le segment de i à j.
    pub fn ligne(&self, i: usize, j: usize) -> Vec<usize> {
        let n = self.distance(i, j).max(1);
        let mut out: Vec<usize> = Vec::new();
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let c = self.sous(self.cx[i] + (self.cx[j] - self.cx[i]) * t, self.cy[i] + (self.cy[j] - self.cy[i]) * t);
            if let Some(c) = c {
                if out.last() != Some(&c) {
                    out.push(c);
                }
            }
        }
        out
    }
}

/// Un élément du tas, rangé du plus petit au plus grand (heapq de Python).
struct Min<T>(T);

impl<T: PartialOrd> PartialEq for Min<T> {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl<T: PartialOrd> Eq for Min<T> {}
impl<T: PartialOrd> PartialOrd for Min<T> {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl<T: PartialOrd> Ord for Min<T> {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}

/// Plus court chemin : cout(i, j) = minutes pour passer de i à j (None = infranchissable).
/// heur_min : minutes par hexagone au mieux (pour l'heuristique). Rend (cellules, minutes), ou None.
pub fn astar(
    g: &Grille,
    depart: usize,
    arrivee: usize,
    mut cout: impl FnMut(usize, usize) -> Option<f64>,
    heur_min: f64,
) -> Option<(Vec<usize>, f64)> {
    if depart == arrivee {
        return Some((vec![depart], 0.0));
    }
    let mut ouvert = BinaryHeap::new();
    ouvert.push(Min((0.0, 0.0, depart)));
    let mut venu: HashMap<usize, usize> = HashMap::new();
    let mut gs: HashMap<usize, f64> = HashMap::new();
    gs.insert(depart, 0.0);
    while let Some(Min((_, gc, i))) = ouvert.pop() {
        if i == arrivee {
            let mut chemin = vec![i];
            let mut k = i;
            while let Some(&p) = venu.get(&k) {
                chemin.push(p);
                k = p;
            }
            chemin.reverse();
            return Some((chemin, gc));
        }
        if gc > *gs.get(&i).unwrap_or(&1e18) {
            continue;
        }
        for v in &g.vois[i] {
            let Some(c) = cout(i, v.j) else { continue };
            let ng = gc + c;
            if ng < *gs.get(&v.j).unwrap_or(&1e18) {
                gs.insert(v.j, ng);
                venu.insert(v.j, i);
                ouvert.push(Min((ng + g.distance(v.j, arrivee) as f64 * heur_min, ng, v.j)));
            }
        }
    }
    None
}

/// Temps d'arrivée depuis plusieurs sources (cellule, temps0). cout(i, j) -> minutes ou None.
pub fn dijkstra(
    g: &Grille,
    sources: &[(usize, f64)],
    mut cout: impl FnMut(usize, usize) -> Option<f64>,
    horizon: f64,
) -> IndexMap<usize, f64> {
    let mut t: IndexMap<usize, f64> = IndexMap::new();
    let mut tas = BinaryHeap::new();
    for &(k, v) in sources {
        t.insert(k, v);
        tas.push(Min((v, k)));
    }
    while let Some(Min((tc, i))) = tas.pop() {
        if tc > *t.get(&i).unwrap_or(&1e18) || tc > horizon {
            continue;
        }
        for v in &g.vois[i] {
            let Some(c) = cout(i, v.j) else { continue };
            let nt = tc + c;
            if nt < *t.get(&v.j).unwrap_or(&1e18) && nt <= horizon {
                t.insert(v.j, nt);
                tas.push(Min((nt, v.j)));
            }
        }
    }
    t
}

/// Comme astar, mais un changement de direction coûte : les routes générées tournent peu et franchement
/// (pas d'escaliers en zigzag). État = (cellule, direction d'arrivée).
pub fn astar_virages(
    g: &Grille,
    depart: usize,
    arrivee: usize,
    mut cout: impl FnMut(usize, usize) -> Option<f64>,
    heur_min: f64,
) -> Option<(Vec<usize>, f64)> {
    let (pen60, pen120) = (0.4, 2.5);
    if depart == arrivee {
        return Some((vec![depart], 0.0));
    }
    let mut ouvert = BinaryHeap::new();
    ouvert.push(Min((0.0, 0.0, depart, -1i64)));
    let mut venu: HashMap<(usize, i64), (usize, i64)> = HashMap::new();
    let mut gs: HashMap<(usize, i64), f64> = HashMap::new();
    gs.insert((depart, -1), 0.0);
    while let Some(Min((_, gc, i, d0))) = ouvert.pop() {
        if i == arrivee {
            let mut chemin = vec![i];
            let mut k = (i, d0);
            while let Some(&p) = venu.get(&k) {
                chemin.push(p.0);
                k = p;
            }
            chemin.reverse();
            return Some((chemin, gc));
        }
        if gc > *gs.get(&(i, d0)).unwrap_or(&1e18) {
            continue;
        }
        for v in &g.vois[i] {
            let Some(mut c) = cout(i, v.j) else { continue };
            let d = v.d as i64;
            if d0 >= 0 && d != d0 {
                let ecart = (d - d0).rem_euclid(6).min((d0 - d).rem_euclid(6));
                c += if ecart == 1 { pen60 } else { pen120 };
            }
            let ng = gc + c;
            if ng < *gs.get(&(v.j, d)).unwrap_or(&1e18) {
                gs.insert((v.j, d), ng);
                venu.insert((v.j, d), (i, d0));
                ouvert.push(Min((ng + g.distance(v.j, arrivee) as f64 * heur_min, ng, v.j, d)));
            }
        }
    }
    None
}

/// Ramer-Douglas-Peucker : retire les points qui s'écartent de moins de `tol` mètres de la ligne droite,
/// tant que le raccourci reste valide (valide(a, b), par exemple : ne traverse pas l'eau).
pub fn simplifier(pts: &[(f64, f64)], tol: f64, valide: &dyn Fn((f64, f64), (f64, f64)) -> bool) -> Vec<(f64, f64)> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = {
        let h = dx.hypot(dy);
        if h == 0.0 {
            1e-9
        } else {
            h
        }
    };
    let (mut k, mut dmax) = (0, -1.0);
    for (idx, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / l;
        if d > dmax {
            k = idx;
            dmax = d;
        }
    }
    if dmax <= tol && valide(a, b) {
        return vec![a, b];
    }
    let mut gauche = simplifier(&pts[..=k], tol, valide);
    let droite = simplifier(&pts[k..], tol, valide);
    gauche.pop();
    gauche.extend(droite);
    gauche
}
