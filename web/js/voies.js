// Le dessin des voies, commun à la carte et à la vue Intervention.
// Routes : enrobé gris, bas-côtés, ligne blanche discontinue au milieu, panneaux d'entrée d'agglomération, garde-corps
// sur les ponts. Pistes forestières (DFCI) : sable clair, deux ornières, barrière rouge et blanche là où la piste
// quitte la route, poteau indicateur en bois au bout de la piste.
import { THREE, put, boite, cyl, ruban } from './moteur.js';

// Chaikin : adoucit les angles
export function lisser(p, it = 2) {
  if (p.length < 3) return p;
  for (let k = 0; k < it; k++) {
    const o = [p[0]];
    for (let i = 0; i < p.length - 1; i++) {
      const a = p[i], b = p[i + 1];
      o.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25], [a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
    }
    o.push(p[p.length - 1]); p = o;
  }
  return p;
}
// des points tous les `pas`, posés sur le sol
export function densifier(p, pas, sol, dy) {
  const o = [];
  for (let i = 0; i < p.length - 1; i++) {
    const a = p[i], b = p[i + 1], n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / pas));
    for (let k = 0; k < n; k++) o.push([a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n]);
  }
  o.push(p[p.length - 1]);
  return o.map(([x, z]) => [x, sol(x, z) + dy, z]);
}
function decaler(pts, d) {
  return pts.map((p, i) => {
    const a = pts[Math.max(0, i - 1)], b = pts[Math.min(pts.length - 1, i + 1)];
    const dx = b[0] - a[0], dz = b[2] - a[2], l = Math.hypot(dx, dz) || 1;
    return [p[0] - dz / l * d, p[1] + 0.005, p[2] + dx / l * d];
  });
}
// des objets posés le long de la ligne, tous les `pas`, tournés dans son sens
function jalonner(pts, pas, poser, depart = pas / 2) {
  let reste = depart;
  for (let i = 0; i < pts.length - 1; i++) {
    const a = pts[i], b = pts[i + 1], l = Math.hypot(b[0] - a[0], b[2] - a[2]);
    let t = reste;
    while (t < l) { const k = t / l; poser(a[0] + (b[0] - a[0]) * k, Math.max(a[1], b[1]), a[2] + (b[2] - a[2]) * k, Math.atan2(b[2] - a[2], b[0] - a[0])); t += pas; }
    reste = t - l;
  }
}

const COUL = { 3: [0x6f6c67, 0x58472f], 2: [0x88837a, 0x6b5636] };

// voies : [{ n, p: [[x, z], …] }] en coordonnées 3D ; o : { sol(x,z), s (échelle), site(x,z) -> id ou -1, eau(x,z) -> 'R'… , pont(x,z) }
export function dessinerVoies(voies, o, Lsol, Ldeco) {
  const s = o.s, sd = s * (o.deco ?? 1);
  const barrieres = [], poteaux = [];
  for (const v of voies) {
    const brut = lisser(v.p, v.n === 1 ? 1 : 2);
    const pts = densifier(brut, 2.2 * s, o.sol, 0.14 * s);
    if (pts.length < 2) continue;
    if (o.marquer) for (const q of pts) o.marquer(q[0], q[2], (v.n >= 2 ? (v.n === 3 ? 1.05 : 0.8) + 0.5 : 0.6) * s);
    if (v.n >= 2) {
      const w = (v.n === 3 ? 1.05 : 0.8) * s, [enrobe, bord] = COUL[v.n];
      put(Lsol, ruban(pts.map(p => [p[0], p[1] - 0.02 * s, p[2]]), w + 0.32 * s), bord, 0, 0, 0, 0, 0);
      put(Lsol, ruban(pts, w), enrobe, 0, 0, 0, 0, 0);
      // ligne blanche discontinue (plus serrée sur la route principale), lignes de rive sur la principale
      const pas = (v.n === 3 ? 2.6 : 3.4) * s;
      jalonner(pts, pas, (x, y, z, a) => put(Lsol, boite(pas * 0.45, 0.04 * s, 0.13 * s), 0xf4f1ea, x, y + 0.03 * s, z, -a, 0));
      if (v.n === 3) for (const d of [w * 0.82, -w * 0.82]) put(Lsol, ruban(decaler(pts, d).map(p => [p[0], p[1] + 0.02 * s, p[2]]), 0.05 * s), 0xe8e4da, 0, 0, 0, 0, 0);
      // panneaux d'entrée d'agglomération et garde-corps des ponts
      let avant = -1;
      for (let i = 0; i < pts.length; i++) {
        const [x, y, z] = pts[i], sid = o.site(x, z);
        if (i > 0 && sid !== avant && (sid >= 0 || avant >= 0)) {
          const [px, py, pz] = sid >= 0 ? pts[i - 1] : pts[i];
          const b = pts[Math.min(pts.length - 1, i + 1)], a = pts[Math.max(0, i - 1)], ang = Math.atan2(b[2] - a[2], b[0] - a[0]);
          const ox = -Math.sin(ang) * (w + 0.7 * s), oz = Math.cos(ang) * (w + 0.7 * s);
          panneauAgglo(Ldeco, px + ox, py, pz + oz, -ang + Math.PI / 2, sd);
        }
        avant = sid;
        if (o.pont(x, z) && i % 2 === 0) {
          const b = pts[Math.min(pts.length - 1, i + 1)], a = pts[Math.max(0, i - 1)], ang = Math.atan2(b[2] - a[2], b[0] - a[0]);
          for (const d of [1, -1]) put(Ldeco, boite(2.4 * s, 0.35 * s, 0.12 * s), 0xb9b6ae, x - Math.sin(ang) * (w + 0.15 * s) * d, y + 0.3 * s, z + Math.cos(ang) * (w + 0.15 * s) * d, -ang, 0.06 * s);
        }
      }
    } else {
      // piste forestière : sable, deux ornières, herbe au milieu
      put(Lsol, ruban(pts, 0.62 * s), 0xe2cd96, 0, 0, 0, 0, 0);
      for (const d of [0.27 * s, -0.27 * s]) put(Lsol, ruban(decaler(pts, d), 0.08 * s), 0xb08f58, 0, 0, 0, 0, 0);
      const a0 = pts[0], a1 = pts[1], b0 = pts[pts.length - 1], b1 = pts[pts.length - 2];
      for (const [p, q] of [[a0, a1], [b0, b1]]) {
        const ang = Math.atan2(q[2] - p[2], q[0] - p[0]);
        if (o.route(p[0], p[2])) barrieres.push([p[0] + Math.cos(ang) * 1.6 * s, p[1], p[2] + Math.sin(ang) * 1.6 * s, ang]);
        else poteaux.push([p[0] - Math.sin(ang) * 0.9 * s, p[1], p[2] + Math.cos(ang) * 0.9 * s, ang]);
      }
    }
  }
  for (const [x, y, z, a] of barrieres) barriereDFCI(Ldeco, x, y, z, a, sd);
  for (const [x, y, z, a] of poteaux) poteauDFCI(Ldeco, x, y, z, a, sd);
}

// panneau blanc à bord rouge, sur un poteau
function panneauAgglo(L, x, y, z, ry, s) {
  put(L, cyl(0.07 * s, 0.07 * s, 1.5 * s, 5), 0x9a9a9a, x, y + 0.75 * s, z, 0, 0.04 * s);
  put(L, boite(1.15 * s, 0.62 * s, 0.1 * s), 0xd8232a, x, y + 1.55 * s, z, ry, 0.05 * s);
  put(L, boite(0.95 * s, 0.44 * s, 0.12 * s), 0xffffff, x, y + 1.55 * s, z, ry, 0);
}
// barrière DFCI : deux poteaux et une lisse rouge et blanche en travers de la piste
function barriereDFCI(L, x, y, z, a, s) {
  const px = -Math.sin(a), pz = Math.cos(a), ry = -a + Math.PI / 2;
  for (const d of [-0.85, 0.85]) put(L, cyl(0.08 * s, 0.09 * s, 1.0 * s, 5), 0x7a5a3a, x + px * d * s, y + 0.5 * s, z + pz * d * s, 0, 0.04 * s);
  for (let k = 0; k < 4; k++) {
    const d = (-0.6 + k * 0.4) * s;
    put(L, boite(0.4 * s, 0.13 * s, 0.13 * s), k % 2 ? 0xffffff : 0xd8232a, x + px * d, y + 0.85 * s, z + pz * d, ry, k === 0 ? 0.04 * s : 0);
  }
}
// poteau indicateur DFCI : poteau de bois, plaque verte
function poteauDFCI(L, x, y, z, a, s) {
  put(L, cyl(0.07 * s, 0.08 * s, 1.3 * s, 5), 0x8a6a48, x, y + 0.65 * s, z, 0, 0.04 * s);
  put(L, boite(0.62 * s, 0.32 * s, 0.08 * s), 0x2f7d4a, x, y + 1.25 * s, z, -a, 0.04 * s);
}
