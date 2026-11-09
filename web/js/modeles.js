// Les modèles en volumes simples, style web_power / web_conquete : végétation de chaque combustible, maisons, sites,
// installations (caserne, vigie, hélistation, aérodrome) et moyens de lutte (CCF, bulldozer, HBE, Canadair, Dash 8).
// Chaque modèle se construit dans un « lot » (moteur.js) centré sur l'origine ; les véhicules regardent vers +x.
import { THREE, put, toit, boite, cyl, cone, sphere, rnd, entre, choix, lot, SQ3 } from './moteur.js';

export const NOIR = 0x2b2622, CENDRE = 0x5b554e, BRAISE = 0x7a3a1c;
const TRONC = 0x7a5233, TRONC_PIN = 0x8a5a3a;

// ------------------------------------------------------------------ végétation (etat : '.', 'f' en feu, 'b' brûlé, 'e'/'n' éteint)
// pos() : un point au hasard dans l'hexagone ; s : échelle (1 sur la carte)
export function pin(L, x, y, z, s, etat) {
  const mort = etat === 'b' || etat === 'e' || etat === 'n';
  put(L, cyl(0.22 * s, 0.32 * s, 3.4 * s, 5), mort ? NOIR : TRONC_PIN, x, y + 1.7 * s, z);
  if (mort) { if (etat !== 'b' || rnd() < 0.5) put(L, cyl(0.05 * s, 0.1 * s, 1.4 * s, 4), NOIR, x + 0.3 * s, y + 2.9 * s, z, rnd() * 3, 0.08); return; }
  const p = sphere(1.7 * s, 0); p.scale(1, 0.45, 1);
  put(L, p, etat === 'f' ? 0x6b4a22 : choix([0x4f7d3a, 0x5c8a40, 0x48763a]), x, y + 3.7 * s, z);
}
export function feuillu(L, x, y, z, s, etat) {
  const mort = etat === 'b' || etat === 'e' || etat === 'n';
  put(L, cyl(0.3 * s, 0.4 * s, 1.4 * s, 5), mort ? NOIR : TRONC, x, y + 0.7 * s, z);
  if (mort) { put(L, cyl(0.06 * s, 0.12 * s, 1.6 * s, 4), NOIR, x, y + 2 * s, z, 0, 0.08); return; }
  put(L, sphere(1.8 * s, 0), etat === 'f' ? 0x7a4f22 : choix([0x5fae3e, 0x74b83f, 0x4f9a37]), x, y + 2.8 * s, z);
}
export function sapinPin(L, x, y, z, s, etat) {     // pin d'Alep, plus touffu, en Méditerranée
  const mort = etat === 'b' || etat === 'e' || etat === 'n';
  put(L, cyl(0.25 * s, 0.35 * s, 2.2 * s, 5), mort ? NOIR : TRONC_PIN, x, y + 1.1 * s, z);
  if (mort) return;
  const c = etat === 'f' ? 0x6b4a22 : choix([0x5d8f3c, 0x6a9a44]);
  const p = sphere(1.5 * s, 0); p.scale(1.1, 0.7, 1); put(L, p, c, x, y + 2.9 * s, z);
  const q = sphere(1.0 * s, 0); q.scale(1, 0.7, 1); put(L, q, c, x + 0.6 * s, y + 3.6 * s, z - 0.3 * s);
}
export function buisson(L, x, y, z, s, etat, col) {
  const mort = etat === 'b' || etat === 'e' || etat === 'n';
  const b = sphere(entre(0.7, 1.1) * s, 0); b.scale(1.3, 0.6, 1);
  put(L, b, mort ? CENDRE : etat === 'f' ? 0x7a4f22 : col, x, y + 0.3 * s, z, rnd() * 3, 0.12);
}
export function olivier(L, x, y, z, s, etat) {
  const mort = etat === 'b' || etat === 'e' || etat === 'n';
  put(L, cyl(0.2 * s, 0.32 * s, 1 * s, 5), mort ? NOIR : 0x6b5a46, x, y + 0.5 * s, z);
  if (!mort) put(L, sphere(1.0 * s, 0), etat === 'f' ? 0x7a4f22 : 0x8fa269, x, y + 1.5 * s, z);
}

// la végétation d'un hexagone selon son combustible
export function vegetation(L, f, etat, x, y, z, R, s = 1, densite = 1, libre = null) {
  const pos = () => {
    for (let k = 0; k < 8; k++) { const a = rnd() * Math.PI * 2, l = Math.sqrt(rnd()) * R * 0.72, p = [x + Math.cos(a) * l, z + Math.sin(a) * l]; if (!libre || libre(p[0], p[1], 1.6 * s)) return p; }
    return [x + 1e5, z];          // pas de place : l'objet part hors de vue (il est éliminé plus bas)
  };
  const nb = k => Math.round(k * densite);
  const L0 = L; L = { g: [], o: [] };
  const brule = etat === 'b' || etat === 'e' || etat === 'n';
  if (f === 'P') for (let i = 0, n = nb(6 + rnd() * 3); i < n; i++) { const [a, b] = pos(); pin(L, a, y, b, entre(0.8, 1.15) * s, etat); }
  else if (f === 'F') for (let i = 0, n = nb(5 + rnd() * 3); i < n; i++) { const [a, b] = pos(); feuillu(L, a, y, b, entre(0.8, 1.2) * s, etat); }
  else if (f === 'J') {
    for (let i = 0, n = nb(2 + rnd() * 2); i < n; i++) { const [a, b] = pos(); pin(L, a, y, b, entre(0.6, 0.85) * s, etat); }
    for (let i = 0, n = nb(5); i < n; i++) { const [a, b] = pos(); buisson(L, a, y, b, s, etat, choix([0x9a8f4e, 0xb4a24a, 0x8a9a4a])); }
  } else if (f === 'G') for (let i = 0, n = nb(7); i < n; i++) { const [a, b] = pos(); buisson(L, a, y, b, 0.8 * s, etat, choix([0x9aa06a, 0xa8a878, 0x8c9a62])); }
  else if (f === 'Q') {
    for (let i = 0, n = nb(8); i < n; i++) { const [a, b] = pos(); buisson(L, a, y, b, 1.15 * s, etat, choix([0x5f8a3c, 0x6f8f44, 0x557a36])); }
    for (let i = 0, n = nb(2); i < n; i++) { const [a, b] = pos(); sapinPin(L, a, y, b, 0.8 * s, etat); }
  } else if (f === 'H') { if (!brule) for (let i = 0; i < nb(3); i++) { const [a, b] = pos(); buisson(L, a, y, b, 0.5 * s, etat, 0x8cbf4a); } }
  else if (f === 'C') { for (let i = 0; i < 3; i++) { const [a, b] = pos(); put(L, boite(entre(3, 5) * s, 0.4 * s, entre(2, 3) * s), brule ? CENDRE : etat === 'f' ? 0x8a5a2a : choix([0xe3c75b, 0xd6b24a, 0xc9b24a]), a, y + 0.2 * s, b, rnd() * 3, 0.12); } }
  else if (f === 'V') { const a = rnd() * Math.PI; for (let k = -3; k <= 3; k++) put(L, boite((11 - Math.abs(k) * 1.6) * s, 0.6 * s, 0.5 * s), brule ? NOIR : 0x557a2e, x + Math.cos(a + Math.PI / 2) * k * 1.6 * s, y + 0.3 * s, z - Math.sin(a + Math.PI / 2) * k * 1.6 * s, a, 0.1); }
  else if (f === 'O') for (let i = 0, n = nb(6); i < n; i++) { const [a, b] = pos(); olivier(L, a, y, b, s, etat); }
  else if (f === 'D') { if (!brule) for (let i = 0; i < nb(3); i++) { const [a, b] = pos(), d = sphere(entre(2, 3) * s, 1); d.scale(1.4, 0.3, 0.8); put(L, d, 0xe8d8a0, a, y, b, rnd() * 3, 0.12); } }
  garder(L, L0, x);
}
// verse dans L0 les pièces de L qui ne sont pas parties « hors de vue »
function garder(L, L0, x) {
  for (const k of ['g', 'o']) for (const geo of L[k]) {
    geo.computeBoundingBox();
    if (geo.boundingBox.min.x > x + 5e4) geo.dispose(); else L0[k].push(geo);
  }
}

// ------------------------------------------------------------------ bâti
export function maison(L, x, y, z, ry, s = 1, etat = '.') {
  const brule = etat === 'b';
  const h = 1.6 * s;
  put(L, boite(2.4 * s, h, 1.9 * s), brule ? 0x4a4440 : choix([0xf6eedb, 0xf1e2c2, 0xfaf6ee, 0xe9dcc0]), x, y + h / 2, z, ry);
  if (brule) { put(L, boite(2.5 * s, 0.25 * s, 2 * s), NOIR, x, y + h + 0.1, z, ry, 0.1); return; }
  toit(L, 2.7 * s, 2.1 * s, 1.1 * s, choix([0xd9573b, 0xc4493a, 0xe07a3a, 0xb8503a]), x, y + h, z, ry);
}
// maison landaise : basse, à colombages, grand toit à faible pente
export function maisonLandaise(L, x, y, z, ry, s = 1, etat = '.') {
  const brule = etat === 'b';
  put(L, boite(3.2 * s, 1.4 * s, 2.2 * s), brule ? 0x4a4440 : 0xf3ead6, x, y + 0.7 * s, z, ry);
  if (brule) return;
  for (const k of [-1, 0, 1]) {
    const dx = Math.cos(ry) * k * 1.05 * s, dz = -Math.sin(ry) * k * 1.05 * s;
    put(L, boite(0.18 * s, 1.42 * s, 2.24 * s), 0x6b4a2f, x + dx, y + 0.71 * s, z + dz, ry, 0);
  }
  toit(L, 3.6 * s, 2.6 * s, 0.9 * s, 0xc8603e, x, y + 1.4 * s, z, ry + Math.PI / 2);
}
export function immeuble(L, x, y, z, ry, s = 1, etat = '.') {
  const h = entre(2.6, 3.8) * s, brule = etat === 'b';
  put(L, boite(2.6 * s, h, 2.6 * s), brule ? 0x4a4440 : choix([0xefe6d2, 0xe6d8bc, 0xf4eee2]), x, y + h / 2, z, ry);
  if (!brule) toit(L, 2.9 * s, 2.9 * s, 1.0 * s, choix([0xc4493a, 0xd9573b]), x, y + h, z, ry);
}
export function eglise(L, x, y, z, ry, s = 1) {
  put(L, boite(2 * s, 1.9 * s, 3.4 * s), 0xe8dcc0, x, y + 0.95 * s, z, ry); toit(L, 2.3 * s, 3.6 * s, 1.4 * s, 0x6d6a75, x, y + 1.9 * s, z, ry);
  const dx = Math.sin(ry) * 1.9 * s, dz = Math.cos(ry) * 1.9 * s;
  put(L, boite(1.1 * s, 3.6 * s, 1.1 * s), 0xe8dcc0, x + dx, y + 1.8 * s, z + dz, ry); put(L, cone(0.95 * s, 2.2 * s, 4), 0x5b5866, x + dx, y + 4.7 * s, z + dz, ry + Math.PI / 4);
}
export function tente(L, x, y, z, ry, s = 1, etat = '.') {
  if (etat === 'b') { put(L, boite(1.4 * s, 0.1, 1.2 * s), NOIR, x, y + 0.05, z, ry, 0); return; }
  const p = cyl(1, 1, 1.4 * s, 3); p.rotateZ(Math.PI / 2); p.rotateY(Math.PI / 2); p.scale(0.8 * s, 0.6 * s, 1);
  put(L, p, choix([0x3a7bd5, 0xe04e39, 0xf2b134, 0x2fa36b, 0xffffff]), x, y + 0.45 * s, z, ry, 0.12);
}
export function mobilhome(L, x, y, z, ry, s = 1, etat = '.') {
  put(L, boite(2.8 * s, 1.0 * s, 1.3 * s), etat === 'b' ? 0x4a4440 : choix([0xf2efe6, 0xdfe9e2, 0xe9e2d0]), x, y + 0.6 * s, z, ry);
  if (etat !== 'b') put(L, boite(2.9 * s, 0.12 * s, 1.5 * s), 0x8a8f96, x, y + 1.15 * s, z, ry, 0.1);
}
export function usine(L, x, y, z, s = 1, etat = '.') {
  const brule = etat === 'b';
  put(L, boite(6 * s, 2.4 * s, 3.6 * s), brule ? 0x4a4440 : 0xc9cfd4, x, y + 1.2 * s, z);
  if (!brule) { toit(L, 6.2 * s, 3.8 * s, 0.9 * s, 0x7d8a96, x, y + 2.4 * s, z, Math.PI / 2); }
  put(L, cyl(0.45 * s, 0.6 * s, 6 * s, 8), 0xb9b0a4, x + 2.6 * s, y + 3 * s, z + 1.2 * s);
  put(L, cyl(1.2 * s, 1.2 * s, 2.2 * s, 10), 0xe8e8e8, x - 3.4 * s, y + 1.1 * s, z - 2.3 * s);
  put(L, cyl(1.2 * s, 1.2 * s, 2.2 * s, 10), 0xe8e8e8, x - 0.8 * s, y + 1.1 * s, z - 2.6 * s);
}
export function chapelle(L, x, y, z, s = 1) { eglise(L, x, y, z, 0.4, 0.9 * s); }
export function caserne(L, x, y, z, s = 1) {
  put(L, boite(4.6 * s, 1.9 * s, 2.6 * s), 0xf1ece0, x, y + 0.95 * s, z);
  for (const k of [-1.5, 0, 1.5]) put(L, boite(1.1 * s, 1.3 * s, 0.1), 0xc8262b, x + k * s, y + 0.65 * s, z + 1.32 * s, 0, 0);
  toit(L, 4.9 * s, 2.9 * s, 0.8 * s, 0xb03a2e, x, y + 1.9 * s, z, Math.PI / 2);
  put(L, boite(0.9 * s, 3.4 * s, 0.9 * s), 0xe9dcc0, x + 2.7 * s, y + 1.7 * s, z - 0.5 * s);   // tour de séchage
}
export function vigie(L, x, y, z, s = 1) {
  for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) put(L, cyl(0.12 * s, 0.18 * s, 9 * s, 5), 0x8a6a48, x + a * 0.9 * s, y + 4.5 * s, z + b * 0.9 * s, 0, 0.08);
  put(L, boite(2.6 * s, 1.5 * s, 2.6 * s), 0xe9dcc0, x, y + 9.7 * s, z);
  put(L, cone(2.2 * s, 1.3 * s, 4), 0xb03a2e, x, y + 11.1 * s, z, Math.PI / 4);
}
export function helistation(L, x, y, z, s = 1) {
  put(L, cyl(4.2 * s, 4.2 * s, 0.25, 16), 0x6c6f72, x, y + 0.12, z, 0, 0.15);
  put(L, boite(0.5 * s, 0.06, 3 * s), 0xffffff, x - 1 * s, y + 0.28, z, 0, 0); put(L, boite(0.5 * s, 0.06, 3 * s), 0xffffff, x + 1 * s, y + 0.28, z, 0, 0);
  put(L, boite(2 * s, 0.06, 0.5 * s), 0xffffff, x, y + 0.28, z, 0, 0);
  put(L, cyl(1.2 * s, 1.2 * s, 1.8 * s, 10), 0x3f7fbf, x + 5 * s, y + 0.9 * s, z + 2 * s);  // réserve d'eau
}
export function aerodrome(L, x, y, z, s = 1) {
  put(L, boite(30 * s, 0.2, 4 * s), 0x55595e, x, y + 0.1, z, 0.25, 0.12);
  for (let k = -12; k <= 12; k += 4) put(L, boite(2 * s, 0.05, 0.35 * s), 0xffffff, x + Math.cos(0.25) * k * s, y + 0.24, z - Math.sin(0.25) * k * s, 0.25, 0);
  put(L, boite(7 * s, 3 * s, 5 * s), 0xd6d2c8, x + 3 * s, y + 1.5 * s, z + 7 * s);
  const t = new THREE.CylinderGeometry(3.5 * s, 3.5 * s, 7 * s, 12, 1, false, 0, Math.PI); t.rotateZ(Math.PI / 2); put(L, t, 0x9aa3ad, x + 3 * s, y + 3 * s, z + 7 * s);
  put(L, boite(1.2 * s, 4.5 * s, 1.2 * s), 0xe9dcc0, x - 6 * s, y + 2.25 * s, z + 6 * s); put(L, boite(2 * s, 1.2 * s, 2 * s), 0x8fc3e0, x - 6 * s, y + 5 * s, z + 6 * s);
}

// ------------------------------------------------------------------ moyens de lutte (regardent vers +x)
const ROUGE = 0xc8262b, BLANC = 0xf4f1ea, JAUNE = 0xf2c230, PNEU = 0x2a2a2a;
export function ccf(s = 1) {
  const L = lot();
  put(L, boite(1.2 * s, 1.15 * s, 1.35 * s), ROUGE, 1.15 * s, 0.85 * s, 0);                  // cabine
  put(L, boite(0.08, 0.5 * s, 1.2 * s), 0x8fc3e0, 1.76 * s, 1.05 * s, 0, 0, 0);               // pare-brise
  put(L, boite(2.1 * s, 1.15 * s, 1.4 * s), ROUGE, -0.55 * s, 0.9 * s, 0);                   // citerne
  put(L, boite(2.12 * s, 0.18 * s, 1.42 * s), BLANC, -0.55 * s, 0.95 * s, 0, 0, 0);          // bande blanche
  put(L, boite(0.4 * s, 0.14 * s, 0.9 * s), 0x2f6fd6, 1.15 * s, 1.5 * s, 0, 0, 0.06);         // gyrophares
  put(L, boite(1.9 * s, 0.1 * s, 0.25 * s), 0x8d8d8d, -0.6 * s, 1.55 * s, 0.35 * s, 0, 0.05);   // dévidoir
  for (const xx of [1.15, -0.2, -1.1]) for (const zz of [-0.62, 0.62]) { const r = cyl(0.36 * s, 0.36 * s, 0.26 * s, 10); r.rotateX(Math.PI / 2); put(L, r, PNEU, xx * s, 0.36 * s, zz * s, 0, 0.06); }
  return L;
}
// VLHR du chef de groupe : un petit 4×4 blanc à bande rouge
export function vlhr(s = 1) {
  const L = lot();
  put(L, boite(2.0 * s, 0.75 * s, 1.15 * s), BLANC, 0, 0.75 * s, 0);
  put(L, boite(1.1 * s, 0.55 * s, 1.05 * s), BLANC, -0.25 * s, 1.35 * s, 0);
  put(L, boite(0.08, 0.4 * s, 0.95 * s), 0x8fc3e0, 0.32 * s, 1.35 * s, 0, 0, 0);
  put(L, boite(2.02 * s, 0.16 * s, 1.17 * s), ROUGE, 0, 0.85 * s, 0, 0, 0);
  put(L, boite(0.35 * s, 0.12 * s, 0.7 * s), 0x2f6fd6, -0.25 * s, 1.68 * s, 0, 0, 0.05);
  for (const xx of [0.65, -0.65]) for (const zz of [-0.55, 0.55]) { const r = cyl(0.3 * s, 0.3 * s, 0.22 * s, 10); r.rotateX(Math.PI / 2); put(L, r, PNEU, xx * s, 0.3 * s, zz * s, 0, 0.05); }
  return L;
}
export function bull(s = 1) {
  const L = lot();
  put(L, boite(1.9 * s, 0.9 * s, 1.3 * s), JAUNE, -0.1 * s, 0.85 * s, 0);
  put(L, boite(0.9 * s, 0.8 * s, 1.0 * s), JAUNE, -0.4 * s, 1.7 * s, 0); put(L, boite(0.7 * s, 0.5 * s, 0.9 * s), 0x8fc3e0, -0.4 * s, 1.75 * s, 0, 0, 0);
  for (const zz of [-0.62, 0.62]) put(L, boite(2.2 * s, 0.55 * s, 0.38 * s), PNEU, -0.1 * s, 0.3 * s, zz * s, 0, 0.06);
  put(L, boite(0.18 * s, 0.75 * s, 1.9 * s), 0x5b5b5b, 1.25 * s, 0.45 * s, 0, 0, 0.08);   // lame
  return L;
}
export function helico(s = 1, coul = ROUGE, bande = BLANC) {
  const L = lot(), R = lot();
  const c = sphere(0.95 * s, 1); c.scale(1.5, 0.95, 0.9); put(L, c, coul, 0.2 * s, 0, 0);
  put(L, boite(0.9 * s, 0.25 * s, 0.95 * s), bande, 0.2 * s, -0.15 * s, 0, 0, 0.06);
  const v = sphere(0.55 * s, 1); v.scale(1, 0.8, 1.2); put(L, v, 0x8fc3e0, 1.25 * s, 0.15 * s, 0, 0, 0.06);
  put(L, boite(2.4 * s, 0.25 * s, 0.25 * s), coul, -1.7 * s, 0.25 * s, 0, 0, 0.06);
  put(L, boite(0.4 * s, 0.9 * s, 0.1 * s), JAUNE, -2.85 * s, 0.6 * s, 0, 0, 0.05);
  for (const zz of [-0.7, 0.7]) put(L, boite(2.2 * s, 0.1 * s, 0.1 * s), 0x3a3a3a, 0.2 * s, -0.95 * s, zz * s, 0, 0.04);
  put(L, cyl(0.12 * s, 0.12 * s, 0.4 * s, 6), 0x3a3a3a, 0.2 * s, 0.95 * s, 0, 0, 0.04);
  put(R, boite(5.6 * s, 0.06 * s, 0.32 * s), 0x2e2e2e, 0, 0, 0, 0, 0.04); put(R, boite(0.32 * s, 0.06 * s, 5.6 * s), 0x2e2e2e, 0, 0, 0, 0, 0.04);
  return { L, R, rotorY: 1.18 * s };
}
// le seau (bambi bucket) sous l'hélico
export function seau(s = 1) {
  const L = lot();
  put(L, cyl(0.03 * s, 0.03 * s, 2.4 * s, 4), 0x333333, 0, -1.2 * s, 0, 0, 0);
  put(L, cyl(0.5 * s, 0.35 * s, 0.7 * s, 8), 0xe85d1a, 0, -2.6 * s, 0, 0, 0.06);
  return L;
}
export function canadair(s = 1) {
  const L = lot();
  const f = cyl(0.55 * s, 0.45 * s, 5.2 * s, 10); f.rotateZ(Math.PI / 2); put(L, f, JAUNE, 0, 0, 0);
  const n = sphere(0.55 * s, 1); n.scale(1.2, 1, 1); put(L, n, JAUNE, 2.6 * s, 0, 0);
  put(L, boite(5.4 * s, 0.4 * s, 0.6 * s), ROUGE, 0, -0.45 * s, 0, 0, 0.06);                 // coque
  put(L, boite(0.9 * s, 0.2 * s, 7.6 * s), JAUNE, 0.3 * s, 0.7 * s, 0, 0, 0.08);              // aile haute
  for (const zz of [-1.4, 1.4]) { const m = cyl(0.3 * s, 0.3 * s, 0.9 * s, 8); m.rotateZ(Math.PI / 2); put(L, m, ROUGE, 0.75 * s, 0.6 * s, zz * s, 0, 0.06); }
  for (const zz of [-3.3, 3.3]) put(L, boite(0.6 * s, 0.25 * s, 0.25 * s), ROUGE, 0.3 * s, 0.15 * s, zz * s, 0, 0.05);   // flotteurs
  put(L, boite(0.7 * s, 1.4 * s, 0.12 * s), ROUGE, -2.3 * s, 0.75 * s, 0, 0, 0.06);
  put(L, boite(0.6 * s, 0.12 * s, 2.6 * s), JAUNE, -2.3 * s, 1.4 * s, 0, 0, 0.06);
  put(L, boite(0.5 * s, 0.3 * s, 0.7 * s), 0x8fc3e0, 2.4 * s, 0.35 * s, 0, 0, 0);
  return L;
}
export function dash(s = 1) {
  const L = lot();
  const f = cyl(0.62 * s, 0.5 * s, 6.6 * s, 10); f.rotateZ(Math.PI / 2); put(L, f, BLANC, 0, 0, 0);
  const n = sphere(0.62 * s, 1); n.scale(1.3, 1, 1); put(L, n, BLANC, 3.3 * s, 0, 0);
  put(L, boite(6.6 * s, 0.35 * s, 1.27 * s), JAUNE, 0, -0.35 * s, 0, 0, 0.06);
  put(L, boite(1.0 * s, 0.18 * s, 8.6 * s), BLANC, 0.4 * s, 0.6 * s, 0, 0, 0.08);
  for (const zz of [-1.8, 1.8]) { const m = cyl(0.32 * s, 0.32 * s, 1.4 * s, 8); m.rotateZ(Math.PI / 2); put(L, m, ROUGE, 0.9 * s, 0.4 * s, zz * s, 0, 0.06); }
  put(L, boite(0.8 * s, 1.7 * s, 0.12 * s), ROUGE, -3 * s, 0.9 * s, 0, 0, 0.06);
  put(L, boite(0.6 * s, 0.12 * s, 3 * s), BLANC, -3.1 * s, 1.75 * s, 0, 0, 0.06);
  put(L, boite(1.6 * s, 0.25 * s, 0.9 * s), 0xb02a2a, 0.4 * s, -0.62 * s, 0, 0, 0.05);   // réservoir de retardant
  return L;
}
// un pompier (vue rapprochée) : tenue feux de forêt beige, casque jaune
export function pompier(L, x, y, z, ry, s = 1) {
  put(L, cyl(0.18 * s, 0.2 * s, 0.75 * s, 6), 0x3a3f46, x, y + 0.38 * s, z, ry, 0.05);
  put(L, cyl(0.22 * s, 0.2 * s, 0.6 * s, 6), 0xd8c08a, x, y + 1.0 * s, z, ry, 0.05);
  put(L, sphere(0.16 * s, 0), 0xe8b896, x, y + 1.42 * s, z, 0, 0.04);
  put(L, sphere(0.19 * s, 0), 0xf2c230, x, y + 1.52 * s, z, 0, 0.04);
}

// ------------------------------------------------------------------ engins de soutien
const roues = (L, s, xs, zz = 0.62, r = 0.34) => { for (const xx of xs) for (const z of [-zz, zz]) { const g = cyl(r * s, r * s, 0.24 * s, 10); g.rotateX(Math.PI / 2); put(L, g, PNEU, xx * s, r * s, z * s, 0, 0.05); } };
// fourgon pompe-tonne : plus court, plus haut, échelle sur le toit
export function fpt(s = 1) {
  const L = lot();
  put(L, boite(1.0 * s, 1.2 * s, 1.35 * s), ROUGE, 1.05 * s, 0.9 * s, 0);
  put(L, boite(0.08, 0.5 * s, 1.2 * s), 0x8fc3e0, 1.56 * s, 1.1 * s, 0, 0, 0);
  put(L, boite(1.9 * s, 1.35 * s, 1.4 * s), ROUGE, -0.45 * s, 0.98 * s, 0);
  put(L, boite(1.92 * s, 0.14 * s, 1.42 * s), 0xf2c230, -0.45 * s, 0.75 * s, 0, 0, 0);
  for (const z of [-0.4, 0.4]) put(L, boite(1.9 * s, 0.08 * s, 0.1 * s), 0xcfcfcf, -0.4 * s, 1.75 * s, z * s, 0, 0.04);
  put(L, boite(0.35 * s, 0.14 * s, 0.9 * s), 0x2f6fd6, 1.05 * s, 1.58 * s, 0, 0, 0.06);
  roues(L, s, [1.0, -0.9]);
  return L;
}
// poste de commandement : grand fourgon blanc, mât et antennes
export function vpc(s = 1) {
  const L = lot();
  put(L, boite(3.0 * s, 1.5 * s, 1.45 * s), BLANC, 0, 1.05 * s, 0);
  put(L, boite(3.02 * s, 0.22 * s, 1.47 * s), ROUGE, 0, 0.75 * s, 0, 0, 0);
  put(L, boite(0.08, 0.5 * s, 1.25 * s), 0x8fc3e0, 1.52 * s, 1.35 * s, 0, 0, 0);
  put(L, cyl(0.06 * s, 0.08 * s, 2.6 * s, 6), 0x9a9a9a, -0.8 * s, 3.1 * s, 0, 0, 0.04);
  put(L, sphere(0.35 * s, 0), 0xe8e8e8, -0.8 * s, 4.4 * s, 0, 0, 0.05);
  put(L, cyl(0.45 * s, 0.45 * s, 0.08 * s, 10), 0xdedede, 0.6 * s, 1.9 * s, 0, 0, 0.04);
  put(L, boite(0.4 * s, 0.12 * s, 0.8 * s), 0x2f6fd6, 1.0 * s, 1.86 * s, 0, 0, 0.05);
  roues(L, s, [1.0, -1.0]);
  return L;
}
// débroussailleuse : tracteur vert et son broyeur à l'avant
export function debr(s = 1) {
  const L = lot();
  put(L, boite(1.4 * s, 0.8 * s, 1.1 * s), 0x3f8f3a, -0.1 * s, 0.9 * s, 0);
  put(L, boite(0.8 * s, 0.75 * s, 0.9 * s), 0x3f8f3a, -0.35 * s, 1.65 * s, 0); put(L, boite(0.6 * s, 0.5 * s, 0.85 * s), 0x8fc3e0, -0.35 * s, 1.68 * s, 0, 0, 0);
  for (const z of [-0.55, 0.55]) { const g = cyl(0.55 * s, 0.55 * s, 0.35 * s, 10); g.rotateX(Math.PI / 2); put(L, g, PNEU, -0.6 * s, 0.55 * s, z * s, 0, 0.05); }
  for (const z of [-0.5, 0.5]) { const g = cyl(0.32 * s, 0.32 * s, 0.25 * s, 10); g.rotateX(Math.PI / 2); put(L, g, PNEU, 0.55 * s, 0.32 * s, z * s, 0, 0.05); }
  const b = cyl(0.3 * s, 0.3 * s, 1.6 * s, 10); b.rotateX(Math.PI / 2); put(L, b, 0xe08a1a, 1.25 * s, 0.35 * s, 0, 0, 0.06);
  put(L, boite(0.5 * s, 0.12 * s, 0.2 * s), 0x5b5b5b, 0.9 * s, 0.55 * s, 0, 0, 0.04);
  return L;
}
// ravitailleur : citerne de gazole blanche, bande jaune
export function ravit(s = 1) {
  const L = lot();
  put(L, boite(1.0 * s, 1.1 * s, 1.3 * s), ROUGE, 1.2 * s, 0.85 * s, 0);
  const c = cyl(0.62 * s, 0.62 * s, 2.3 * s, 12); c.rotateZ(Math.PI / 2); put(L, c, BLANC, -0.6 * s, 1.05 * s, 0);
  const b = cyl(0.64 * s, 0.64 * s, 0.25 * s, 12); b.rotateZ(Math.PI / 2); put(L, b, JAUNE, -0.6 * s, 1.05 * s, 0, 0, 0);
  roues(L, s, [1.2, -0.3, -1.2]);
  return L;
}
// VSAV : ambulance rouge à bande blanche, croix
export function vsav(s = 1) {
  const L = lot();
  put(L, boite(2.6 * s, 1.35 * s, 1.3 * s), ROUGE, -0.1 * s, 0.95 * s, 0);
  put(L, boite(2.62 * s, 0.2 * s, 1.32 * s), BLANC, -0.1 * s, 0.85 * s, 0, 0, 0);
  put(L, boite(0.08, 0.45 * s, 1.15 * s), 0x8fc3e0, 1.22 * s, 1.25 * s, 0, 0, 0);
  put(L, boite(0.7 * s, 0.06 * s, 0.22 * s), BLANC, -0.4 * s, 1.66 * s, 0, 0, 0); put(L, boite(0.22 * s, 0.06 * s, 0.7 * s), BLANC, -0.4 * s, 1.66 * s, 0, 0, 0);
  put(L, boite(0.35 * s, 0.12 * s, 0.8 * s), 0x2f6fd6, 0.9 * s, 1.7 * s, 0, 0, 0.05);
  roues(L, s, [0.85, -0.9], 0.58, 0.3);
  return L;
}
export const COULEURS_DRAGON = [0xd8232a, 0xf2c230];
