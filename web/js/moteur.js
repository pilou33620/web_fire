// Le moteur du rendu en plateau d'hexagones, repris de web_conquete (web/js/hex/moteur.js, lui-même repris de web_power) :
// ombrage « cartoon » à trois tons, contours sombres (une copie gonflée de chaque pièce, faces arrière), lots de pièces
// fusionnés en un seul maillage, grille d'hexagones pointe en haut.
import * as THREE from '../vendor/three/three.module.js';

export { THREE };
export const SQ3 = Math.sqrt(3);
export const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);
export const lerp = (a, b, t) => a + (b - a) * t;

export function hache(x, y, s = 7) {
  let h = (Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(s | 0, 1442695041)) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}
let g = 1;
export function rnd() { g = (g + 0x6D2B79F5) | 0; let t = Math.imul(g ^ (g >>> 15), 1 | g); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }
export const semer = (a, b = 0) => { g = (Math.round(a * 7919) + Math.round(b * 104729) + 13) | 0; };
export const entre = (a, b) => a + rnd() * (b - a);
export const choix = l => l[(rnd() * l.length) | 0];

// ------------------------------------------------------------------ matériaux
const degrade = new THREE.DataTexture(new Uint8Array([105, 190, 255]), 3, 1, THREE.RedFormat);
degrade.minFilter = degrade.magFilter = THREE.NearestFilter; degrade.needsUpdate = true;
export const DEGRADE = degrade;
export const MAT = new THREE.MeshToonMaterial({ vertexColors: true, gradientMap: degrade });
export const CONT = new THREE.MeshBasicMaterial({ color: 0x24180f, side: THREE.BackSide });
export const toon = (col, o = {}) => new THREE.MeshToonMaterial({ color: col, gradientMap: degrade, ...o });

// ------------------------------------------------------------------ lots de pièces
export const lot = () => ({ g: [], o: [] });
const VA = new THREE.Vector3(), VB = new THREE.Vector3(), COUL = new THREE.Color();
// une pièce de couleur unie, tournée de ry, posée en (x, y, z) ; ep : épaisseur du contour (0 : sans contour)
export function put(L, geo, col, x = 0, y = 0, z = 0, ry = 0, ep = 0.26) {
  if (geo.index) geo = geo.toNonIndexed();
  for (const k of Object.keys(geo.attributes)) if (k !== 'position' && k !== 'normal') geo.deleteAttribute(k);
  if (!geo.attributes.normal) geo.computeVertexNormals();
  if (ry) geo.rotateY(ry);
  geo.translate(x, y, z);
  COUL.set(col);
  const n = geo.attributes.position.count, a = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) { a[i * 3] = COUL.r; a[i * 3 + 1] = COUL.g; a[i * 3 + 2] = COUL.b; }
  geo.setAttribute('color', new THREE.BufferAttribute(a, 3));
  L.g.push(geo);
  if (ep) {
    const o = geo.clone(); o.computeBoundingBox(); o.boundingBox.getCenter(VA); o.boundingBox.getSize(VB);
    o.translate(-VA.x, -VA.y, -VA.z);
    o.scale((VB.x + 2 * ep) / Math.max(VB.x, 1e-3), (VB.y + 2 * ep) / Math.max(VB.y, 1e-3), (VB.z + 2 * ep) / Math.max(VB.z, 1e-3));
    o.translate(VA.x, VA.y, VA.z); o.deleteAttribute('color'); L.o.push(o);
  }
  return geo;
}
// verse le lot src dans L, tourné de ry puis posé en (x, y, z), à l'échelle s
export function transfere(src, L, x, y, z, ry = 0, s = 1) {
  for (const k of ['g', 'o']) for (const geo of src[k]) { if (s !== 1) geo.scale(s, s, s); if (ry) geo.rotateY(ry); geo.translate(x, y, z); L[k].push(geo); }
}
export function fusion(geos) {
  if (!geos.length) return null;
  const noms = Object.keys(geos[0].attributes), out = new THREE.BufferGeometry();
  for (const k of noms) {
    const it = geos[0].attributes[k].itemSize;
    let n = 0; for (const geo of geos) n += geo.attributes[k].count;
    const arr = new Float32Array(n * it); let o = 0;
    for (const geo of geos) { arr.set(geo.attributes[k].array, o); o += geo.attributes[k].array.length; }
    out.setAttribute(k, new THREE.BufferAttribute(arr, it));
  }
  for (const geo of geos) geo.dispose();
  return out;
}
export function fige(L, parent, { mat = MAT, ombre = true, contour = true } = {}) {
  const grp = new THREE.Group();
  const geo = fusion(L.g);
  if (geo) { const m = new THREE.Mesh(geo, mat); m.castShadow = ombre; m.receiveShadow = true; grp.add(m); }
  const geoO = contour ? fusion(L.o) : (L.o.forEach(o => o.dispose()), null);
  if (geoO) grp.add(new THREE.Mesh(geoO, CONT));
  L.g = []; L.o = [];
  if (parent) parent.add(grp);
  return grp;
}
export function jeter(obj) {
  if (!obj) return;
  obj.parent?.remove(obj);
  obj.traverse(o => { o.geometry?.dispose(); if (o.material && o.material !== MAT && o.material !== CONT && !o.material.partage) o.material.dispose?.(); });
}

// ------------------------------------------------------------------ formes
export const boite = (w, h, d) => new THREE.BoxGeometry(w, h, d);
export const cyl = (rt, rb, h, s = 10) => new THREE.CylinderGeometry(rt, rb, h, s);
export const cone = (r, h, s = 8) => new THREE.ConeGeometry(r, h, s);
export const sphere = (r, d = 1) => new THREE.IcosahedronGeometry(r, d);
export function toit(L, w, d, h, col, x, y, z, ry = 0, ep = 0.2) {
  const p = cyl(1, 1, d, 3); p.rotateX(Math.PI / 2); p.rotateZ(Math.PI); p.scale(w / SQ3, h / 1.5, 1); p.translate(0, h / 3, 0);
  put(L, p, col, x, y, z, ry, ep);
}
export function ruban(pts, larg) {
  const pos = [];
  const perp = i => { const a = pts[Math.max(0, i - 1)], b = pts[Math.min(pts.length - 1, i + 1)], dx = b[0] - a[0], dz = b[2] - a[2], l = Math.hypot(dx, dz) || 1; return [-dz / l, dx / l]; };
  for (let i = 0; i < pts.length - 1; i++) {
    const A = pts[i], B = pts[i + 1], pa = perp(i), pb = perp(i + 1);
    pos.push(A[0] + pa[0] * larg, A[1], A[2] + pa[1] * larg, B[0] + pb[0] * larg, B[1], B[2] + pb[1] * larg, A[0] - pa[0] * larg, A[1], A[2] - pa[1] * larg,
      A[0] - pa[0] * larg, A[1], A[2] - pa[1] * larg, B[0] + pb[0] * larg, B[1], B[2] + pb[1] * larg, B[0] - pb[0] * larg, B[1], B[2] - pb[1] * larg);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  geo.setAttribute('normal', new THREE.Float32BufferAttribute(pos.map((_, i) => (i % 3 === 1 ? 1 : 0)), 3));
  return geo;
}
// un hexagone plat (pointe en haut) de rayon r, pour les calques
export function hexPlat(r) {
  const g = new THREE.CylinderGeometry(r, r, 0.08, 6);
  return g;
}

// ------------------------------------------------------------------ caméra inclinée (web_power/js/15-camera.js), gestes
export class Camera {
  constructor(el, cam, { zmin = 60, zmax = 1600, borne = null } = {}) {
    this.el = el; this.cam = cam; this.zmin = zmin; this.zmax = zmax; this.borne = borne;
    this.cible = new THREE.Vector3(); this.dist = 400; this.AXE = new THREE.Vector3(0, 0.83, 0.56).normalize();
    this.W = 1; this.H = 1; this.sale = true; this.onTap = null; this.onHover = null; this.onMove = null;
    const ptrs = new Map(); let glisse = false, dep0 = null, pinch0 = 0, dist0 = 0, vit = { x: 0, z: 0 }, tAv = 0;
    this.ptrs = ptrs;
    el.addEventListener('pointerdown', e => {
      if (this.off) return;
      try { el.setPointerCapture(e.pointerId); } catch (err) { /* pointeur déjà relâché */ }
      // geste capturé (ordre en cours : appuyer, glisser pour orienter, relâcher) : la carte ne bouge pas
      if (ptrs.size === 0 && this.saisie == null && this.onPress?.(e.clientX, e.clientY, e) === true) { this.saisie = e.pointerId; this.elan = null; return; }
      ptrs.set(e.pointerId, { x: e.clientX, y: e.clientY }); dep0 = { x: e.clientX, y: e.clientY }; glisse = false; vit = { x: 0, z: 0 };
      if (ptrs.size === 2) { const [a, b] = [...ptrs.values()]; pinch0 = Math.hypot(a.x - b.x, a.y - b.y); dist0 = this.dist; }
    });
    el.addEventListener('pointermove', e => {
      if (this.off) return;
      if (this.saisie === e.pointerId) { this.onDrag?.(e.clientX, e.clientY, e); return; }
      const p = ptrs.get(e.pointerId);
      if (p) {
        if (ptrs.size === 2) { p.x = e.clientX; p.y = e.clientY; const [a, b] = [...ptrs.values()]; this.dist = clamp(dist0 * pinch0 / Math.max(20, Math.hypot(a.x - b.x, a.y - b.y)), this.zmin, this.zmax); this.maj(); glisse = true; return; }
        const dx = e.clientX - p.x, dy = e.clientY - p.y; p.x = e.clientX; p.y = e.clientY;
        if (!glisse && Math.hypot(e.clientX - dep0.x, e.clientY - dep0.y) > 7) { glisse = true; el.classList.add('glisse'); this.onHover?.(null); }
        if (glisse) {
          const upp = this.upp(); const ddx = -dx * upp, ddz = -dy * upp / this.AXE.y;
          this.cible.x += ddx; this.cible.z += ddz; vit = { x: ddx, z: ddz }; tAv = performance.now(); this.suivi = null; this.maj();
        }
        return;
      }
      if (e.pointerType === 'mouse' || e.pointerType === 'pen') this.onHover?.(e.clientX, e.clientY, e);
    });
    const fin = e => {
      if (this.off) { ptrs.delete(e.pointerId); return; }
      if (this.saisie === e.pointerId) { this.saisie = null; this.onRelease?.(e.clientX, e.clientY, e.type === 'pointercancel'); return; }
      const etait = ptrs.has(e.pointerId); ptrs.delete(e.pointerId); el.classList.remove('glisse');
      if (etait && !glisse && e.type === 'pointerup' && ptrs.size === 0) this.onTap?.(e.clientX, e.clientY);
      if (etait && glisse && ptrs.size === 0 && performance.now() - tAv < 60) this.elan = vit;
    };
    el.addEventListener('pointerup', fin); el.addEventListener('pointercancel', fin);
    el.addEventListener('pointerleave', () => this.onHover?.(null));
    el.addEventListener('wheel', e => { e.preventDefault(); if (this.off) return; this.dist = clamp(this.dist * Math.exp(e.deltaY * 0.0012), this.zmin, this.zmax); this.maj(); }, { passive: false });
  }
  upp() { return 2 * this.dist * Math.tan(this.cam.fov * Math.PI / 360) / this.H; }
  maj() {
    if (this.borne) { this.cible.x = clamp(this.cible.x, this.borne[0], this.borne[1]); this.cible.z = clamp(this.cible.z, this.borne[2], this.borne[3]); }
    this.cam.position.copy(this.cible).addScaledVector(this.AXE, this.dist); this.cam.lookAt(this.cible);
    this.sale = true; this.onMove?.();
  }
  taille(W, H) { this.W = W; this.H = H; this.cam.aspect = W / H; this.cam.updateProjectionMatrix(); this.maj(); }
  anime(dt) {
    if (this.elan) { this.cible.x += this.elan.x; this.cible.z += this.elan.z; this.elan.x *= 0.9; this.elan.z *= 0.9; if (Math.abs(this.elan.x) + Math.abs(this.elan.z) < 0.02) this.elan = null; this.maj(); }
    if (this.vers) {
      const k = 1 - Math.pow(0.002, dt);
      this.cible.lerp(this.vers.c, k); this.dist += (this.vers.d - this.dist) * k;
      if (this.cible.distanceTo(this.vers.c) < 0.3 && Math.abs(this.dist - this.vers.d) < 1) this.vers = null;
      this.maj();
    }
  }
  aller(x, z, d) { this.vers = { c: new THREE.Vector3(x, 0, z), d: d ?? this.dist }; this.elan = null; }
}
