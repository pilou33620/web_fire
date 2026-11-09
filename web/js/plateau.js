// La carte en plateau d'hexagones (vue d'ensemble) : tuiles en relief, végétation, villes et sites, routes et pistes
// DFCI, installations ; puis tout ce qui bouge : flammes, braises, fumée, eau et retardant, moyens de lutte.
// Le sol est découpé en blocs de 10 × 10 hexagones, reconstruits quand l'état d'un de leurs hexagones change.
import { THREE, SQ3, MAT, CONT, DEGRADE, put, lot, fige, jeter, cyl, boite, sphere, cone, ruban, semer, rnd, entre, hache, clamp, toon } from './moteur.js';
import * as M from './modeles.js';
import { dessinerVoies } from './voies.js';

const BLOC = 10;
export const R = 10;

const COUL_ETAT = { b: 0x3b3632, e: 0x4f4439, n: 0x464a4c, x: 0xb8955f };

export class Plateau {
  constructor(scene, C) {
    this.scene = scene; this.C = C;
    const Rm = C.hexM / SQ3;
    this.S = R / Rm;                                 // unités 3D par mètre
    this.OX = C.largeur * this.S / 2; this.OZ = C.hauteur * this.S / 2;
    this.n = C.cols * C.rows;
    this.k = 0.026;                                  // exagération du relief
    this.dens = clamp(3000 / this.n, 0.25, 1);         // grande carte : moins d'arbres par hexagone, mais plus gros (mémoire, tablette)
    this.X = new Float32Array(this.n); this.Z = new Float32Array(this.n); this.top = new Float32Array(this.n);
    this.Q = new Int32Array(this.n); this.Rr = new Int32Array(this.n);
    this.index = new Map();
    for (let row = 0; row < C.rows; row++) for (let col = 0; col < C.cols; col++) {
      const i = row * C.cols + col, q = col - ((row - (row & 1)) >> 1);
      this.Q[i] = q; this.Rr[i] = row; this.index.set(q + ',' + row, i);
      const [x, z] = this.vers3D(C.hexM * (col + 0.5 * (row & 1)) + C.hexM / 2, 1.5 * Rm * row + Rm);
      this.X[i] = x; this.Z[i] = z;
    }
    for (let i = 0; i < this.n; i++) {
      const f = C.fuel[i];
      this.top[i] = f === 'M' ? -3 : (f === 'L' || f === 'R') ? Math.max(-0.4, C.alt[i] * this.k - 0.9) : C.alt[i] * this.k - 0.2;
    }
    this.site = new Int32Array(this.n).fill(-1);
    for (const s of C.sites) for (const i of s.cells) this.site[i] = s.id;
    this.sig = new Array(this.n).fill('');
    this.etat = '.'.repeat(this.n);
    this.ret = new Set(); this.mouille = new Map(); this.prog = new Map();
    this.blocs = new Map();
    this.groupe = new THREE.Group(); scene.add(this.groupe);
    this._eau(); this._routes(); this._installations(); this._calques(); this._feux();
    for (let i = 0; i < this.n; i++) this.sig[i] = this._signe(i);
    for (const k of this._tousBlocs()) this._bloc(k);
  }

  vers3D(x, y) { return [x * this.S - this.OX, y * this.S - this.OZ]; }
  versMetres(x, z) { return [(x + this.OX) / this.S, (z + this.OZ) / this.S]; }
  // l'hexagone sous un point 3D
  cellule(x, z) {
    const C = this.C, Rm = C.hexM / SQ3;
    const [mx, my] = this.versMetres(x, z);
    const xx = mx - C.hexM / 2, yy = my - Rm;
    const q = (SQ3 / 3 * xx - yy / 3) / Rm, r = (2 / 3 * yy) / Rm, s = -q - r;
    let rq = Math.round(q), rr = Math.round(r); const rs = Math.round(s);
    const dq = Math.abs(rq - q), dr = Math.abs(rr - r), ds = Math.abs(rs - s);
    if (dq > dr && dq > ds) rq = -rr - rs; else if (dr > ds) rr = -rq - rs;
    const i = this.index.get(rq + ',' + rr);
    return i === undefined ? -1 : i;
  }
  voisins(i) {
    const out = [];
    for (const [a, b] of [[1, 0], [1, -1], [0, -1], [-1, 0], [-1, 1], [0, 1]]) { const j = this.index.get((this.Q[i] + a) + ',' + (this.Rr[i] + b)); out.push(j === undefined ? -1 : j); }
    return out;
  }
  blocDe(i) { const col = i % this.C.cols, row = (i / this.C.cols) | 0; return ((row / BLOC) | 0) * 100 + ((col / BLOC) | 0); }
  _tousBlocs() { const s = new Set(); for (let i = 0; i < this.n; i++) s.add(this.blocDe(i)); return [...s]; }
  _signe(i) { const e = this.etat[i]; return e + (this.ret.has(i) ? 'r' : '') + (e === 'x' ? '' : this.C.fuel[i]); }

  // ------------------------------------------------------------------ sol, végétation, bâti d'un bloc
  _bloc(k) {
    const C = this.C, L = lot(), D = lot();
    jeter(this.blocs.get(k));
    const grp = new THREE.Group();
    for (let i = 0; i < this.n; i++) {
      if (this.blocDe(i) !== k) continue;
      semer(i, 3);
      const f = C.fuel[i], e = this.etat[i], y = this.top[i], bas = -6, h = y - bas;
      let col = e === '.' || e === 'f' ? C.comb[f].col : COUL_ETAT[e] || C.comb[f].col;
      const c = new THREE.Color(col);
      if (f === 'M' || f === 'L' || f === 'R') c.set(f === 'M' ? 0x1d5a86 : 0x3fa6dc);
      if (e === 'f') c.lerp(new THREE.Color(0x5a2a12), 0.55);
      if (this.ret.has(i) && e !== 'b') c.lerp(new THREE.Color(0xd2453c), 0.55);
      if (this.site[i] >= 0 && (e === '.' || e === 'f') && 'UBZ'.includes(f)) c.set(f === 'U' ? 0xcfc3a2 : 0xd8cba8);
      c.offsetHSL((rnd() - 0.5) * 0.015, 0, (rnd() - 0.5) * 0.05);
      const geo = put(L, cyl(R * 0.97, R, h, 6), 0xffffff, this.X[i], bas + h / 2, this.Z[i], 0, 0);
      const nr = geo.attributes.normal, co = geo.attributes.color;
      for (let v = 0; v < co.count; v++) { const kk = nr.getY(v) > 0.5 ? 1 : 0.62; co.setXYZ(v, c.r * kk, c.g * kk, c.b * kk); }
      if (f === 'M' || f === 'L' || f === 'R' || f === 'S' || f === 'A') continue;
      if (e === 'b') {                 // cendres et souches
        for (let t = 0; t < 3; t++) { const a = rnd() * 6.28, l = rnd() * R * 0.6, b = sphere(entre(0.6, 1.2), 0); b.scale(1.4, 0.25, 1); put(D, b, rnd() < 0.5 ? 0x6a645c : 0x2a2522, this.X[i] + Math.cos(a) * l, y + 0.05, this.Z[i] + Math.sin(a) * l, 0, 0); }
      }
      const s = this.site[i];
      if (s >= 0) this._bati(D, i, C.sites[s], e);
      else if (f === 'X') { /* sol nu */ }
      else if (e !== 'b' || 'PFJQ'.includes(f)) M.vegetation(D, f === 'B' || f === 'U' || f === 'Z' || f === 'T' ? 'H' : f, e, this.X[i], y, this.Z[i], R, 1 / Math.sqrt(this.dens) ** 0.5, (e === 'b' ? 0.6 : 1) * this.dens, (a, b, r) => this.libre(a, b, r));
    }
    const sol = fige(L, grp, { contour: false });
    sol.children[0].castShadow = false;
    fige(D, grp);
    this.groupe.add(grp); this.blocs.set(k, grp);
  }

  _bati(D, i, s, e) {
    const C = this.C, x = this.X[i], z = this.Z[i], y = this.top[i];
    const pos = (r = 0.62) => {
      for (let k = 0; k < 10; k++) { const a = rnd() * 6.28, l = Math.sqrt(rnd()) * R * r, p = [x + Math.cos(a) * l, z + Math.sin(a) * l]; if (this.libre(p[0], p[1], 1.9)) return p; }
      return null;
    };
    const etat = e === 'b' ? 'b' : '.';
    const D0 = D; D = lot();
    const landes = C.theme === 'gironde';
    if (s.type === 'ville') {
      if (i === s.centre) { const c = pos(0.5); if (c) M.eglise(D, c[0], y, c[1], 0.3, 1.1); }
      const nb = i === s.centre ? 5 : 7;
      for (let k = 0; k < nb; k++) { const q = pos(0.7); if (!q) continue; const [a, b] = q; (rnd() < 0.5 ? M.immeuble : M.maison)(D, a, y, b, rnd() * 3, 0.95, etat); }
      for (let k = 0; k < 2; k++) { const q = pos(0.75); if (!q) continue; const [a, b] = q; M.feuillu(D, a, y, b, 0.6, e === 'b' ? 'b' : '.'); }
    } else if (s.type === 'village' || s.type === 'patrimoine') {
      if (s.type === 'patrimoine') { const c = pos(0.45) || [x, z]; M.chapelle(D, c[0], y, c[1], 1); for (let k = 0; k < 3; k++) { const q = pos(); if (!q) continue; const [a, b] = q; M.feuillu(D, a, y, b, 0.8, etat); } for (const k of ['g', 'o']) for (const geo of D[k]) D0[k].push(geo); return; }
      if (i === s.centre) { const c = pos(0.5); if (c) M.eglise(D, c[0], y, c[1], rnd() * 3, 0.8); }
      for (let k = 0; k < 4; k++) { const q = pos(0.7); if (!q) continue; const [a, b] = q; (landes ? M.maisonLandaise : M.maison)(D, a, y, b, rnd() * 3, 0.9, etat); }
      for (let k = 0; k < 3; k++) { const q = pos(0.75); if (!q) continue; const [a, b] = q; (landes ? M.feuillu : M.olivier)(D, a, y, b, 0.7, etat); }
    } else if (s.type === 'camping') {
      for (let k = 0; k < 4; k++) { const q = pos(0.75); if (!q) continue; const [a, b] = q; (landes ? M.pin : M.sapinPin)(D, a, y, b, 0.85, e); }
      for (let k = 0; k < 5; k++) { const q = pos(0.6); if (!q) continue; const [a, b] = q; M.tente(D, a, y, b, rnd() * 3, 1, etat); }
      for (let k = 0; k < 3; k++) { const q = pos(0.6); if (!q) continue; const [a, b] = q; M.mobilhome(D, a, y, b, rnd() * 3, 1, etat); }
    } else if (s.type === 'usine') {
      if (i === s.centre) { const c = pos(0.3) || [x, z]; M.usine(D, c[0], y, c[1], 1, etat); }
      else for (let k = 0; k < 2; k++) { const q = pos(0.5); if (!q) continue; const [a, b] = q; M.maison(D, a, y, b, rnd() * 3, 1.2, etat); }
    } else if (s.type === 'reserve') {
      M.vegetation(D, 'F', e, x, y, z, R, 1, 1.2, (a, b, r) => this.libre(a, b, r));
    }
    for (const k of ['g', 'o']) for (const geo of D[k]) D0[k].push(geo);
  }

  // un point est-il libre (loin des routes et pistes) ?
  libre(x, z, r = 1.2) {
    const i = this.cellule(x, z); if (i < 0) return true;
    for (const j of [i, ...this.voisins(i)]) {
      const l = j >= 0 && this.bord?.get(j); if (!l) continue;
      for (let k = 0; k < l.length; k += 3) { const d = Math.hypot(l[k] - x, l[k + 1] - z); if (d < l[k + 2] + r) return false; }
    }
    return true;
  }

  // ------------------------------------------------------------------ eau, routes, installations
  _eau() {
    const C = this.C;
    const eau = new THREE.Mesh(new THREE.PlaneGeometry(C.largeur * this.S + 900, C.hauteur * this.S + 900),
      new THREE.MeshToonMaterial({ color: 0x3fa6dc, gradientMap: DEGRADE, transparent: true, opacity: 0.72 }));
    eau.rotation.x = -Math.PI / 2; eau.position.y = -1.0; eau.receiveShadow = true; this.groupe.add(eau);
    const fond = new THREE.Mesh(new THREE.PlaneGeometry(C.largeur * this.S + 2400, C.hauteur * this.S + 2400), new THREE.MeshBasicMaterial({ color: 0x1d5a86 }));
    fond.rotation.x = -Math.PI / 2; fond.position.y = -6.2; this.groupe.add(fond);
  }
  _lisser(pts) {
    if (pts.length < 3) return pts;
    let p = pts;
    for (let it = 0; it < 2; it++) {
      const o = [p[0]];
      for (let k = 0; k < p.length - 1; k++) { const a = p[k], b = p[k + 1]; o.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25, a[2] * 0.75 + b[2] * 0.25], [a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75, a[2] * 0.25 + b[2] * 0.75]); }
      o.push(p[p.length - 1]); p = o;
    }
    return p;
  }
  _routes() {
    const C = this.C, L = lot(), D = lot();
    const voies = C.routes.map(r => ({ n: r.n, p: (r.p || r.c.map(i => this.versMetres(this.X[i], this.Z[i]))).map(([x, y]) => this.vers3D(x, y)) }));
    const haut = (x, z) => { const i = this.cellule(x, z); if (i < 0) return -0.6; const f = C.fuel[i]; return f === 'R' ? this.top[i] + 0.9 : (f === 'L' || f === 'M') ? -0.3 : this.top[i]; };
    const sol = (x, z) => Math.max(haut(x, z), haut(x + 1.6, z), haut(x - 1.6, z), haut(x, z + 1.6), haut(x, z - 1.6));
    const site = (x, z) => { const i = this.cellule(x, z); const k = i >= 0 ? this.site[i] : -1; return k >= 0 && ['ville', 'village', 'camping'].includes(C.sites[k].type) ? k : -1; };
    const pont = (x, z) => { const i = this.cellule(x, z); return i >= 0 && C.fuel[i] === 'R'; };
    const route = (x, z) => { const i = this.cellule(x, z); return i >= 0 && [i, ...this.voisins(i)].some(j => j >= 0 && +C.route[j] >= 2); };
    this.bord = new Map();          // cellule -> points des voies (pour ne rien construire dessus)
    const marquer = (x, z, w) => { const i = this.cellule(x, z); if (i < 0) return; let l = this.bord.get(i); if (!l) this.bord.set(i, l = []); l.push(x, z, w); };
    dessinerVoies(voies, { s: 1, deco: 1.6, sol, site, pont, route, marquer }, L, D);
    this.routes = fige(L, this.groupe, { ombre: false, contour: false }); this.panneaux = fige(D, this.groupe);
    // la rivière : un ruban d'eau vive
    if (C.riviere?.length > 1) {
      const pts = this._lisser(C.riviere.map(i => [this.X[i], this.top[i] + 0.12, this.Z[i]]));
      const E = lot(); put(E, ruban(pts, 3.2), 0x46a9dd, 0, 0, 0, 0, 0); fige(E, this.groupe, { ombre: false, contour: false });
    }
  }
  _installations() {
    const C = this.C, L = lot();
    for (const ca of C.casernes) { const i = ca.cell; M.caserne(L, this.X[i] + 5, this.top[i], this.Z[i] + 4, 1); }
    for (const i of C.vigies) M.vigie(L, this.X[i], this.top[i], this.Z[i], 1);
    if (C.helistation != null) M.helistation(L, this.X[C.helistation], this.top[C.helistation], this.Z[C.helistation], 1);
    if (C.aerodrome != null) M.aerodrome(L, this.X[C.aerodrome], this.top[C.aerodrome], this.Z[C.aerodrome], 0.6);
    for (const i of C.pointsEau) if (this.site[i] < 0 && i !== C.helistation) put(L, cyl(1.3, 1.3, 1.2, 10), 0x3f7fbf, this.X[i] + 3, this.top[i] + 0.6, this.Z[i] - 3, 0, 0.12);
    fige(L, this.groupe);
  }

  // ------------------------------------------------------------------ calques (retardant, eau, prévision, sélection)
  _calques() {
    const geo = new THREE.CylinderGeometry(R * 0.93, R * 0.93, 0.1, 6);
    const mat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.38, depthWrite: false });
    this.calque = new THREE.InstancedMesh(geo, mat, this.n); this.calque.count = 0; this.calque.renderOrder = 2;
    this.calque.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(this.n * 3), 3);
    this.calque.frustumCulled = false; this.groupe.add(this.calque);
    const anneau = col => {
      const p = []; for (let k = 0; k < 6; k++) { const a = Math.PI / 180 * (60 * k - 30), b = Math.PI / 180 * (60 * k + 30); p.push(Math.cos(a) * R * 0.95, 0, Math.sin(a) * R * 0.95, Math.cos(b) * R * 0.95, 0, Math.sin(b) * R * 0.95); }
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(p, 3));
      const m = new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: col, depthTest: false })); m.renderOrder = 9; m.visible = false; this.groupe.add(m); return m;
    };
    this.survol = anneau(0xfff7e6); this.choix = anneau(0xffc94d);
    this.lentilles = { prevision: false, eau: true, evac: true };
  }
  majCalques(prev) {
    const m = this.calque, c = new THREE.Color(), mat = new THREE.Matrix4();
    let n = 0;
    const poser = (i, col) => { mat.makeTranslation(this.X[i], this.top[i] + 0.25, this.Z[i]); m.setMatrixAt(n, mat); c.set(col); m.setColorAt(n, c); n++; };
    if (this.lentilles.eau) {
      for (const i of this.ret) if (this.etat[i] !== 'f') poser(i, 0xe0403a);
      for (const [i, w] of this.mouille) if (!this.ret.has(i) && w > 25 && this.etat[i] !== 'b') poser(i, w > 55 ? 0x2f7fd8 : 0x6fb6ee);
    }
    if (this.lentilles.evac && this.evac) {
      const coup = new Set(this.evac.coupees);
      for (const [i, d] of this.evac.fumee) if (!coup.has(i) && this.etat[i] !== 'f') poser(i, d > 60 ? 0x5d5a57 : 0x8a8682);
      for (const i of coup) if (this.etat[i] !== 'f') poser(i, 0x9b1c1c);
    }
    if (this.lentilles.prevision && prev) {
      for (const [i, t] of prev) { if (this.etat[i] !== '.') continue; poser(i, t < 30 ? 0xff3b1f : t < 60 ? 0xff7a1a : t < 120 ? 0xffb02e : 0xffe07a); }
    }
    m.count = n; m.instanceMatrix.needsUpdate = true; if (m.instanceColor) m.instanceColor.needsUpdate = true;
  }

  // ------------------------------------------------------------------ flammes, lueur, fumée, gouttes
  _feux() {
    const MAXF = 5000;
    const cone1 = new THREE.ConeGeometry(1, 1, 6); cone1.translate(0, 0.5, 0);
    this.fl1 = new THREE.InstancedMesh(cone1, new THREE.MeshBasicMaterial({ color: 0xff6a1a }), MAXF);
    this.fl2 = new THREE.InstancedMesh(cone1, new THREE.MeshBasicMaterial({ color: 0xffd34d }), MAXF);
    for (const m of [this.fl1, this.fl2]) { m.count = 0; m.frustumCulled = false; this.groupe.add(m); }
    const disque = new THREE.CylinderGeometry(R * 1.05, R * 1.05, 0.1, 6);
    this.lueur = new THREE.InstancedMesh(disque, new THREE.MeshBasicMaterial({ color: 0xff5a10, transparent: true, opacity: 0.35, blending: THREE.AdditiveBlending, depthWrite: false }), this.n);
    this.lueur.count = 0; this.lueur.frustumCulled = false; this.lueur.renderOrder = 3; this.groupe.add(this.lueur);
    // fumée
    this.MAXS = 900;
    this.fumee = new THREE.InstancedMesh(new THREE.IcosahedronGeometry(1, 1), new THREE.MeshToonMaterial({ color: 0xffffff, gradientMap: DEGRADE, transparent: true, opacity: 0.5, depthWrite: false }), this.MAXS);
    this.fumee.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(this.MAXS * 3), 3);
    this.fumee.count = 0; this.fumee.frustumCulled = false; this.fumee.renderOrder = 4; this.groupe.add(this.fumee);
    this.parts = [];
    // gouttes (eau, retardant, jets des lances)
    this.MAXG = 2500;
    this.gouttes = new THREE.InstancedMesh(new THREE.IcosahedronGeometry(0.35, 0), new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.85 }), this.MAXG);
    this.gouttes.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(this.MAXG * 3), 3);
    this.gouttes.count = 0; this.gouttes.frustumCulled = false; this.groupe.add(this.gouttes);
    this.gt = [];
    this.flammes = [];
  }

  // ------------------------------------------------------------------ nouvel état du serveur
  majEtat(E) {
    this.etat = E.etat;
    this.ret = new Set(E.ret);
    this.mouille = new Map(E.mouille);
    this.prog = new Map(E.prog);
    this.brulant = new Map(E.feu.map(f => [f[0], f]));
    const sales = new Set();
    for (let i = 0; i < this.n; i++) { const s = this._signe(i); if (s !== this.sig[i]) { this.sig[i] = s; sales.add(this.blocDe(i)); } }
    for (const k of sales) this.aFaire.add(k);
    // les flammes de chaque hexagone en feu
    const fl = [];
    for (const [i, I, p] of E.feu) {
      const nb = 2 + Math.round(I / 16);
      for (let k = 0; k < nb; k++) {
        const a = hache(i, k, 3) * 6.28, l = Math.sqrt(hache(i, k, 5)) * R * 0.62;
        fl.push({ x: this.X[i] + Math.cos(a) * l, z: this.Z[i] + Math.sin(a) * l, y: this.top[i], h: (2.2 + 9 * I / 100) * (0.6 + 0.6 * hache(i, k, 9)), ph: hache(i, k, 11) * 6.28, I: I / 100 });
      }
    }
    this.flammes = fl;
    const m = this.lueur, mat = new THREE.Matrix4(); let n = 0;
    for (const [i, I] of E.feu) { const s = 0.5 + I / 120; mat.makeScale(s, 1, s); mat.setPosition(this.X[i], this.top[i] + 0.3, this.Z[i]); m.setMatrixAt(n++, mat); }
    m.count = n; m.instanceMatrix.needsUpdate = true;
  }
  aFaire = new Set();
  reconstruire(max = 3) {
    let k = 0;
    for (const b of this.aFaire) { this._bloc(b); this.aFaire.delete(b); if (++k >= max) break; }
    return k > 0;
  }

  // ------------------------------------------------------------------ largages et jets (effets)
  largage(cells, produit, kind) {
    const col = produit === 'retardant' ? 0xe0403a : 0xdff2ff;
    const haut = kind === 'helico' ? 9 : 14;
    for (const i of cells) {
      for (let k = 0; k < (kind === 'helico' ? 70 : 160); k++) {
        if (this.gt.length >= this.MAXG) break;
        const a = Math.random() * 6.28, l = Math.random() * R * (kind === 'helico' ? 0.5 : 0.9);
        this.gt.push({ x: this.X[i] + Math.cos(a) * l, y: this.top[i] + haut + Math.random() * 4, z: this.Z[i] + Math.sin(a) * l, vx: 0, vy: -2 - Math.random() * 4, vz: 0, t: 0, vie: 2.2, col, s: 0.8 + Math.random() * 0.9 });
      }
    }
  }
  dome(x, y, z, r) {
    for (let k = 0; k < 3 && this.gt.length < this.MAXG; k++) {
      const a = Math.random() * 6.28, v = 2 + Math.random() * 2;
      this.gt.push({ x, y: y + 1.6, z, vx: Math.cos(a) * r * 1.4, vy: v, vz: Math.sin(a) * r * 1.4, t: 0, vie: 0.7, col: 0xcfeaff, s: 0.55, g: 9 });
    }
  }
  jet(x0, y0, z0, i) {
    if (this.gt.length >= this.MAXG) return;
    const x1 = this.X[i] + (Math.random() - 0.5) * 6, z1 = this.Z[i] + (Math.random() - 0.5) * 6, y1 = this.top[i] + 0.5;
    const T = 0.7;
    this.gt.push({ x: x0, y: y0, z: z0, vx: (x1 - x0) / T, vy: (y1 - y0) / T + 4.9 * T, vz: (z1 - z0) / T, t: 0, vie: T, col: 0xbfe6ff, s: 0.45, g: 9.8 });
  }

  // ------------------------------------------------------------------ animation de chaque image
  anime(dt, t, vent) {
    // flammes
    const mat = new THREE.Matrix4(), q = new THREE.Quaternion(), e = new THREE.Euler(), v = new THREE.Vector3(), sc = new THREE.Vector3();
    const pen = Math.min(0.5, vent.v / 90), ax = vent.x, az = vent.z;
    let n = 0;
    for (const f of this.flammes) {
      if (n >= 4990) break;
      const k = 0.75 + 0.25 * Math.sin(t * 9 + f.ph) + 0.1 * Math.sin(t * 23 + f.ph * 2);
      e.set(az * pen, 0, -ax * pen); q.setFromEuler(e);
      sc.set(1.2 + 1.3 * f.I, f.h * k, 1.2 + 1.3 * f.I); v.set(f.x, f.y, f.z);
      mat.compose(v, q, sc); this.fl1.setMatrixAt(n, mat);
      sc.set(0.65 + f.I * 0.7, f.h * k * 0.62, 0.65 + f.I * 0.7); mat.compose(v, q, sc); this.fl2.setMatrixAt(n, mat);
      n++;
    }
    this.fl1.count = this.fl2.count = n; this.fl1.instanceMatrix.needsUpdate = this.fl2.instanceMatrix.needsUpdate = true;
    // fumée : naît au-dessus des flammes, monte et part sous le vent
    const nbNaitre = Math.min(this.flammes.length, 4 + this.flammes.length * dt * 0.8);
    for (let k = 0; k < nbNaitre && this.parts.length < this.MAXS; k++) {
      const f = this.flammes[(Math.random() * this.flammes.length) | 0];
      if (!f || Math.random() > 0.35 + f.I) continue;
      this.parts.push({ x: f.x, y: f.y + f.h, z: f.z, t: 0, vie: 7 + Math.random() * 6, s0: 1.4 + f.I * 2, d: f.I });
    }
    const vv = 3 + vent.v / 6, cc = new THREE.Color();
    let m2 = 0;
    for (let k = this.parts.length - 1; k >= 0; k--) {
      const p = this.parts[k]; p.t += dt;
      if (p.t > p.vie) { this.parts.splice(k, 1); continue; }
      const u = p.t / p.vie;
      p.x += ax * vv * dt; p.z += az * vv * dt; p.y += (3.2 - u * 1.5) * dt;
      const s = p.s0 * (1 + u * 4) * (u > 0.8 ? (1 - u) * 5 : 1);
      mat.makeScale(s, s * 0.8, s); mat.setPosition(p.x, p.y, p.z);
      this.fumee.setMatrixAt(m2, mat);
      cc.setRGB(0.32 + u * 0.5 - p.d * 0.1, 0.3 + u * 0.48 - p.d * 0.1, 0.29 + u * 0.46 - p.d * 0.1); this.fumee.setColorAt(m2, cc);
      m2++;
    }
    this.fumee.count = m2; this.fumee.instanceMatrix.needsUpdate = true; this.fumee.instanceColor.needsUpdate = true;
    // gouttes
    let m3 = 0;
    for (let k = this.gt.length - 1; k >= 0; k--) {
      const p = this.gt[k]; p.t += dt;
      if (p.t > p.vie) { this.gt.splice(k, 1); continue; }
      p.vy -= (p.g ?? 6) * dt; p.x += p.vx * dt; p.y += p.vy * dt; p.z += p.vz * dt;
      mat.makeScale(p.s, p.s * 1.6, p.s); mat.setPosition(p.x, p.y, p.z);
      this.gouttes.setMatrixAt(m3, mat); cc.set(p.col); this.gouttes.setColorAt(m3, cc); m3++;
    }
    this.gouttes.count = m3; this.gouttes.instanceMatrix.needsUpdate = true; if (m3) this.gouttes.instanceColor.needsUpdate = true;
  }
}

// ------------------------------------------------------------------ les moyens de lutte sur la carte
export class Flotte {
  constructor(plateau, s = 1) {
    this.P = plateau; this.s = s; this.u = new Map(); this.grp = new THREE.Group(); plateau.scene.add(this.grp);
    // une géométrie par type de moyen, partagée
    this.geo = {};
    const fig = (L) => fige(L);
    this.geo.ccf = fig(M.ccf(s)); this.geo.bull = fig(M.bull(s)); this.geo.vlhr = fig(M.vlhr(s));
    for (const k of ['fpt', 'vpc', 'debr', 'ravit', 'vsav']) this.geo[k] = fig(M[k](s));
    const h = M.helico(s); this.geo.helico = fig(h.L); this.geo.rotor = fig(h.R); this.rotorY = h.rotorY;
    this.geo.dragon = fig(M.helico(s * 0.9, M.COULEURS_DRAGON[1], M.COULEURS_DRAGON[0]).L);
    this.geo.seau = fig(M.seau(s));
    this.geo.canadair = fig(M.canadair(s)); this.geo.dash = fig(M.dash(s));
    this.ombreMat = new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.22, depthWrite: false });
    this.ombreGeo = new THREE.CircleGeometry(1, 16); this.ombreGeo.rotateX(-Math.PI / 2);
    this.gyro = new THREE.MeshBasicMaterial({ color: 0x3b7bff }); this.gyroGeo = new THREE.SphereGeometry(0.28 * s, 6, 4);
    this.lignes = new THREE.Group(); this.grp.add(this.lignes);
  }
  _creer(d) {
    const o = { grp: new THREE.Group(), k: d.k };
    const cloner = g => { const c = new THREE.Group(); for (const m of g.children) { const mm = new THREE.Mesh(m.geometry, m.material); mm.castShadow = m.castShadow; c.add(mm); } return c; };
    o.corps = cloner(this.geo[d.k]); o.grp.add(o.corps);
    if (d.k === 'dragon') { o.rotor = cloner(this.geo.rotor); o.rotor.position.y = this.rotorY * 0.9; o.corps.add(o.rotor); }
    if (d.k === 'helico') { o.rotor = cloner(this.geo.rotor); o.rotor.position.y = this.rotorY; o.corps.add(o.rotor); o.seau = cloner(this.geo.seau); o.corps.add(o.seau); }
    if (d.k === 'ccf') { o.gyro = new THREE.Mesh(this.gyroGeo, this.gyro.clone()); o.gyro.position.set(1.15 * this.s, 1.65 * this.s, 0); o.corps.add(o.gyro); }
    if (!['ccf', 'bull', 'vlhr', 'fpt', 'vpc', 'debr', 'ravit', 'vsav'].includes(d.k)) { o.ombre = new THREE.Mesh(this.ombreGeo, this.ombreMat); o.ombre.scale.setScalar(d.k === 'helico' ? 2.2 * this.s : 3.4 * this.s); this.grp.add(o.ombre); }
    o.grp.scale.setScalar(1);
    this.grp.add(o.grp);
    return o;
  }
  // nouvel état : on garde la position d'avant pour glisser jusqu'à la nouvelle
  maj(liste, now) {
    const vus = new Set();
    for (const d of liste) {
      vus.add(d.id);
      let o = this.u.get(d.id);
      if (!o) { o = this._creer(d); this.u.set(d.id, o); o.x0 = o.x1 = d.x; o.y0 = o.y1 = d.y; o.cap0 = o.cap1 = d.cap; }
      else { o.x0 = o.xa ?? o.x1; o.y0 = o.ya ?? o.y1; o.cap0 = o.capa ?? o.cap1; o.x1 = d.x; o.y1 = d.y; o.cap1 = d.cap; }
      o.t0 = now; o.d = d;
    }
    for (const [id, o] of this.u) if (!vus.has(id)) { this.grp.remove(o.grp); o.ombre && this.grp.remove(o.ombre); o.bulle && this.grp.remove(o.bulle); this.u.delete(id); }
    // les engins garés au même endroit (caserne, aérodrome) se rangent côte à côte au lieu de se superposer
    const parc = new Map();
    for (const o of this.u.values()) {
      o.place = null;
      const d = o.d; if (!(d.e === 'dispo' || d.e === 'alerte' || d.e === 'au_sol')) continue;
      const k = d.c + (['helico', 'canadair', 'dash', 'dragon'].includes(d.k) ? 'a' : 's');
      const n = parc.get(k) || 0; parc.set(k, n + 1);
      const pas = ['canadair', 'dash'].includes(d.k) ? 9 : 3.6;
      o.place = [((n % 4) - 1.5) * pas, (Math.floor(n / 4) - 0.5) * pas];
    }
  }
  // position 3D d'un moyen à l'instant présent (interpolée)
  anime(dt, t, now, duree, plateau, jets) {
    const P = this.P;
    for (const o of this.u.values()) {
      const d = o.d, a = clamp((now - o.t0) / duree, 0, 1);
      const mx = o.x0 + (o.x1 - o.x0) * a, my = o.y0 + (o.y1 - o.y0) * a;
      let dc = ((o.cap1 - o.cap0 + 540) % 360) - 180; const cap = o.cap0 + dc * a;
      o.xa = mx; o.ya = my; o.capa = cap;
      let [x, z] = P.vers3D(mx, my);
      if (o.place) { x += o.place[0]; z += o.place[1]; }
      const i = P.cellule(x, z), sol = i >= 0 ? Math.max(P.top[i], -0.9) : 0;
      let y = sol, inc = 0;
      const act = d.a?.k;
      if (d.k === 'helico') y = sol + (d.e === 'au_sol' || d.e === 'dispo' || act === 'au_sol' ? 0.9 : act === 'ecope' ? 4.5 : 10);
      if (d.k === 'dragon') y = sol + (d.e === 'dispo' || act === 'au_sol' ? 0.9 : act === 'treuille' ? 5 : 12);
      if (d.k === 'canadair') { y = sol + (d.e === 'au_sol' || d.e === 'dispo' || act === 'au_sol' ? 0.9 : act === 'ecope' ? 0.4 : act === 'largue' ? 6 : 15); inc = act === 'ecope' ? 0 : 0; }
      if (d.k === 'dash') y = sol + (d.e === 'au_sol' || d.e === 'dispo' || act === 'au_sol' || act === 'remplit' ? 0.9 : 18);
      if (o.y3 === undefined) o.y3 = y;
      o.y3 += (y - o.y3) * Math.min(1, dt * 2.5);
      o.grp.position.set(x, o.y3, z);
      o.grp.rotation.y = -cap * Math.PI / 180;
      if (o.ombre) { o.ombre.position.set(x, sol + 0.2, z); o.ombre.visible = o.y3 > sol + 1.5; }
      if (o.rotor) { o.rotor.rotation.y += dt * (o.y3 > sol + 1.5 ? 28 : 4); if (o.seau) o.seau.visible = o.y3 > sol + 3 && !!d.m; }
      if (d.k === 'canadair' || d.k === 'dash') o.corps.rotation.z = o.y3 > sol + 2 ? 0.05 : 0;
      if (o.gyro) { o.gyro.visible = d.e === 'route' || d.e === 'action' || d.e === 'alerte'; o.gyro.material.color.setHex(Math.sin(t * 12 + d.id) > 0 ? 0x3b7bff : 0xbfd4ff); }
      // autoprotection : les rampes d'aspersion font un dôme d'eau autour du camion ; engin perdu : épave noire
      if (act === 'autoprotection' && jets && Math.random() < dt * 40) plateau.dome(x, o.y3, z, 2.6 * this.s);
      if (d.k === 'ccf') {
        if (!o.bulle) { o.bulle = new THREE.Mesh(new THREE.SphereGeometry(2.4 * this.s, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2), new THREE.MeshBasicMaterial({ color: 0x6cc0ff, transparent: true, opacity: 0.42, depthWrite: false })); o.bulle.renderOrder = 6; this.grp.add(o.bulle); }
        o.bulle.visible = act === 'autoprotection';
        if (o.bulle.visible) { o.bulle.position.set(x, o.y3, z); o.bulle.scale.setScalar(1 + 0.06 * Math.sin(t * 9)); }
      }
      if (d.hs && !o.epave) { o.epave = true; const m = new THREE.MeshToonMaterial({ color: 0x2b2622, gradientMap: DEGRADE }); o.corps.traverse(c => { if (c.isMesh && c.material !== CONT) c.material = m; }); if (o.gyro) o.gyro.visible = false; }
      // jets des lances
      if ((act === 'arrose' || act === 'noie') && d.a.c != null && jets && Math.random() < dt * 14) {
        plateau.jet(x + Math.cos(cap * Math.PI / 180) * 0.4, o.y3 + 1.6, z + Math.sin(cap * Math.PI / 180) * 0.4, d.a.c);
      }
      o.pos = [x, o.y3, z];
    }
  }
  // traits vers les postes (calque « moyens »)
  traits(voir) {
    while (this.lignes.children.length) { const l = this.lignes.children.pop(); l.geometry.dispose(); }
    if (!voir) return;
    const P = this.P, pos = [], cols = [];
    const C = { defense: [1, 0.79, 0.3], attaque: [1, 0.45, 0.3], surveillance: [0.5, 0.85, 0.5], pare_feu: [0.95, 0.75, 0.2], preposition: [0.6, 0.75, 1] };
    for (const o of this.u.values()) {
      const d = o.d; if (d.p == null || !o.pos || !C[d.mt]) continue;
      const c = C[d.mt];
      pos.push(o.pos[0], o.pos[1] + 1, o.pos[2], P.X[d.p], P.top[d.p] + 1, P.Z[d.p]); cols.push(...c, ...c);
    }
    if (!pos.length) return;
    const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3)); g.setAttribute('color', new THREE.Float32BufferAttribute(cols, 3));
    const l = new THREE.LineSegments(g, new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.85, depthTest: false })); l.renderOrder = 8; this.lignes.add(l);
  }
}

// ------------------------------------------------------------------ les convois d'évacuation (quelques voitures à la file)
export class Convois {
  constructor(plateau, s = 1, opts = {}) {
    this.P = plateau; this.s = s; this.u = new Map(); this.grp = new THREE.Group(); plateau.scene.add(this.grp);
    this.vers = opts.vers || ((x, y) => plateau.vers3D(x, y));
    this.sol = opts.sol || ((x, z) => { const i = plateau.cellule(x, z); return i >= 0 ? Math.max(plateau.top[i], -0.9) : 0; });
    this.voitures = [0xf2f2f2, 0x3a7bd5, 0xd9573b, 0x2fa36b, 0xf2c230, 0x8a8f96].map(col => {
      const L = lot();
      put(L, boite(1.6 * s, 0.6 * s, 0.9 * s), col, 0, 0.45 * s, 0);
      put(L, boite(0.9 * s, 0.45 * s, 0.82 * s), 0x8fc3e0, -0.1 * s, 0.95 * s, 0, 0, 0.06);
      return fige(L);
    });
    this.feux = new THREE.MeshBasicMaterial({ color: 0xffa020 });
  }
  maj(liste, now) {
    const vus = new Set();
    for (const d of liste) {
      vus.add(d.id);
      let o = this.u.get(d.id);
      if (!o) {
        o = { grp: new THREE.Group(), cars: [] };
        const nb = Math.max(2, Math.min(5, Math.round(d.n / 70)));
        for (let k = 0; k < nb; k++) {
          const src = this.voitures[(d.id * 3 + k) % this.voitures.length], c = new THREE.Group();
          for (const m of src.children) c.add(new THREE.Mesh(m.geometry, m.material));
          c.position.x = -k * 2.3 * this.s; o.grp.add(c); o.cars.push(c);
        }
        o.alerte = new THREE.Mesh(new THREE.SphereGeometry(0.5 * this.s, 6, 4), this.feux); o.alerte.position.y = 2.2 * this.s; o.grp.add(o.alerte);
        this.grp.add(o.grp); this.u.set(d.id, o); o.x0 = o.x1 = d.x; o.y0 = o.y1 = d.y; o.cap0 = o.cap1 = d.cap;
      } else { o.x0 = o.xa ?? o.x1; o.y0 = o.ya ?? o.y1; o.cap0 = o.capa ?? o.cap1; o.x1 = d.x; o.y1 = d.y; o.cap1 = d.cap; }
      o.t0 = now; o.d = d;
    }
    for (const [id, o] of this.u) if (!vus.has(id)) { this.grp.remove(o.grp); this.u.delete(id); }
  }
  anime(dt, t, now, duree) {
    const P = this.P;
    for (const o of this.u.values()) {
      const a = clamp((now - o.t0) / duree, 0, 1);
      const mx = o.x0 + (o.x1 - o.x0) * a, my = o.y0 + (o.y1 - o.y0) * a;
      const dc = ((o.cap1 - o.cap0 + 540) % 360) - 180, cap = o.cap0 + dc * a;
      o.xa = mx; o.ya = my; o.capa = cap;
      const [x, z] = this.vers(mx, my);
      o.grp.position.set(x, this.sol(x, z) + 0.15, z);
      o.grp.rotation.y = -cap * Math.PI / 180;
      o.alerte.visible = o.d.e === 'bloque' && Math.sin(t * 8) > 0;
      o.pos = [x, o.grp.position.y, z];
    }
  }
}
