// Le mode Intervention : la vue rapprochée d'un feu, comme le mode Bataille de web_conquete.
// Chaque hexagone de la carte (300 m) y est découpé en 25 petits hexagones (60 m). Le front de flammes avance à
// l'intérieur des grands hexagones d'après l'état du moteur (côté d'où vient le feu, combustible consumé) ; les camions
// se postent, les pompiers déroulent leurs lances, les hélicoptères et les Canadair font leurs passes de largage,
// le Dash pose sa ligne rouge, le bulldozer ouvre le pare-feu. Des étiquettes montrent la stratégie du commandement.
import { THREE, SQ3, MAT, CONT, DEGRADE, Camera, put, lot, fige, jeter, cyl, boite, sphere, cone, semer, rnd, entre, hache, clamp, lerp } from './moteur.js';
import * as M from './modeles.js';
import { Convois, trajet, suivre } from './plateau.js';
import { dessinerVoies } from './voies.js';
import { eventail } from './commande.js';
import * as UI from './ui.js';

const RAY = 4;            // rayon de la zone, en hexagones de la carte
const DIV = 5;            // un hexagone de la carte = 5 petits hexagones d'un bord à l'autre
const RM = 3;             // rayon d'un petit hexagone, en unités 3D
const DIRV = [[1, 0], [1, -1], [0, -1], [-1, 0], [-1, 1], [0, 1]];

export class Intervention {
  constructor(renderer, C, P, zone) {
    this.r = renderer; this.C = C; this.P = P; this.zone = zone; this.actif = false;
    this.scene = new THREE.Scene(); this.scene.background = new THREE.Color(0x1b211a);
    this.cam = new THREE.PerspectiveCamera(32, 1, 2, 4000);
    this.ciel = new THREE.HemisphereLight(0xeef7ff, 0x7d8466, 1.5); this.scene.add(this.ciel);
    this.soleil = new THREE.DirectionalLight(0xfff0d4, 2.6); this.soleil.castShadow = true;
    Object.assign(this.soleil.shadow.camera, { left: -160, right: 160, top: 160, bottom: -160, near: 10, far: 900 });
    this.soleil.shadow.mapSize.set(2048, 2048); this.soleil.shadow.bias = -0.0006; this.soleil.shadow.normalBias = 0.3;
    this.soleil.position.set(-120, 220, 110); this.scene.add(this.soleil, this.soleil.target);
    this.camera = new Camera(renderer.domElement, this.cam, { zmin: 35, zmax: 520 });
    this.camera.off = true;
    this.eti = document.createElement('div'); this.eti.className = 'inter-eti'; this.eti.style.cssText = 'position:absolute;inset:0;z-index:2;pointer-events:none;overflow:hidden';
    this.eti.hidden = true; zone.appendChild(this.eti);
    this.hud = zone.querySelector('#inter-hud');
    this.hud.addEventListener('click', e => { if (e.target.closest('[data-recentrer]')) this.recentrer(); });
    this.mainCam = null;
    this.apercu = new THREE.Group(); this.scene.add(this.apercu);
    this.ray = new THREE.Raycaster(); this.plan = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  }

  // ------------------------------------------------------------------ la vue, pour le geste de commandement (commande.js)
  _sol(cx, cy) {
    const r = this.zone.getBoundingClientRect(), v = new THREE.Vector2((cx - r.left) / r.width * 2 - 1, -((cy - r.top) / r.height) * 2 + 1);
    this.ray.setFromCamera(v, this.cam);
    let y = 0, pt = new THREE.Vector3(), i = -1;
    for (let k = 0; k < 3; k++) {
      this.plan.constant = -y; if (!this.ray.ray.intersectPlane(this.plan, pt)) return null;
      i = this.macroDe(pt.x, pt.z); if (i < 0) return null; y = this.haut.get(i) ?? 0;
    }
    return { i, x: pt.x, z: pt.z };
  }
  vue() {
    const P = this.P, self = this;
    return {
      cellule: (cx, cy) => { const s = self._sol(cx, cy); return s && self.macro.has(s.i) ? s.i : -1; },
      sol: (cx, cy) => { const s = self._sol(cx, cy); return s ? [s.x, s.z] : null; },
      pos: i => { if (i == null || i < 0 || !self.macro?.has(i)) return null; const [x, z] = self.vers3(...P.versMetres(P.X[i], P.Z[i])); return [x, self.haut.get(i) ?? 0, z]; },
      posM: (mx, my) => { const [x, z] = self.vers3(mx, my); return [x, self.hauteurSous(x, z), z]; },
      voisins: i => P.voisins(i),
      get R() { return RM * DIV; }, groupe: self.apercu,
      ecran: (x, y, z) => { const v = new THREE.Vector3(x, y, z).project(self.cam); if (v.z > 1) return null; return [(v.x + 1) / 2 * self.W, (1 - v.y) / 2 * self.H]; },
    };
  }

  lumiere(ciel, soleil, fond, nuit) {
    this.ciel.color.copy(ciel.color); this.ciel.groundColor.copy(ciel.groundColor); this.ciel.intensity = ciel.intensity;
    this.soleil.color.copy(soleil.color); this.soleil.intensity = soleil.intensity; this.scene.background.copy(fond);
    this.nuit = nuit;
    if (this.lueur) this.lueur.material.opacity = nuit ? 0.65 : 0.32;
  }
  taille(W, H) { this.W = W; this.H = H; this.camera.taille(W, H); }

  // ------------------------------------------------------------------ entrer, sortir
  entrer(f, E) {
    this.feuId = f.id; this.E = E;
    this.centre = f.tete ?? f.cell0;
    this.construire();
    this.actif = true; this.camera.off = false;
    for (const c of window.__feux ? [window.__feux.camera] : []) c.off = true;
    this.eti.hidden = false; this.hud.hidden = false;
    const r = this.zone.getBoundingClientRect(); this.taille(r.width, r.height);
    this.camera.cible.set(0, 0, 0); this.camera.dist = 260; this.camera.maj();
    this.majEtat(E, performance.now(), true);
  }
  quitter() {
    if (!this.actif) return;
    this.actif = false; this.camera.off = true;
    if (window.__feux) window.__feux.camera.off = false;
    this.eti.hidden = true; this.hud.hidden = true;
    this.vider();
  }
  recentrer() { const f = this.E?.feux.find(x => x.id === this.feuId); if (f) { this.centre = f.tete ?? f.cell0; this.construire(); this.majEtat(this.E, performance.now(), true); } }
  vider() {
    if (this.monde) { jeter(this.monde); this.monde = null; }
    this.eti.innerHTML = '';
  }

  // ------------------------------------------------------------------ construction de la zone
  construire() {
    this.vider();
    const C = this.C, P = this.P;
    this.monde = new THREE.Group(); this.scene.add(this.monde);
    const c0 = this.centre;
    const [mx0, my0] = P.versMetres(P.X[c0], P.Z[c0]);
    this.mx0 = mx0; this.my0 = my0;
    this.S2 = RM / (C.hexM / DIV / SQ3);
    this.vex = C.theme === 'gironde' ? 1.4 : 0.45;
    // la zone : les hexagones de la carte à au plus RAY du centre
    const dist = (a, b) => { const dq = P.Q[a] - P.Q[b], dr = P.Rr[a] - P.Rr[b]; return (Math.abs(dq) + Math.abs(dr) + Math.abs(dq + dr)) / 2; };
    this.macro = new Set(); for (let i = 0; i < P.n; i++) if (dist(i, c0) <= RAY) this.macro.add(i);
    // hauteur de chaque hexagone de la carte dans la vue
    this.haut = new Map();
    for (const i of this.macro) {
      const f = C.fuel[i];
      let h = (C.alt[i] - C.alt[c0]) * this.S2 * this.vex;
      if ('MLR'.includes(f)) h = Math.min(h, ...P.voisins(i).filter(j => j >= 0 && !'MLR'.includes(C.fuel[j])).map(j => (C.alt[j] - C.alt[c0]) * this.S2 * this.vex), h) - 1.6;
      this.haut.set(i, h);
    }
    // les petits hexagones
    const rm = C.hexM / DIV / SQ3, N = Math.ceil((RAY + 0.8) * DIV * 1.05);
    const T = [];
    for (let r = -N; r <= N; r++) for (let q = -N; q <= N; q++) {
      if (Math.abs(q) + Math.abs(r) + Math.abs(q + r) > 2 * N) continue;
      const x = mx0 + rm * SQ3 * (q + r / 2), y = my0 + rm * 1.5 * r;
      const [X, Z] = P.vers3D(x, y), i = P.cellule(X, Z);
      if (i < 0 || !this.macro.has(i)) continue;
      T.push({ q, r, x, y, X: (x - mx0) * this.S2, Z: (y - my0) * this.S2, M: i, n: hache(q + 500, r + 500, 17), sig: '' });
    }
    this.T = T;
    // tuiles
    const geo = new THREE.CylinderGeometry(RM * 0.985, RM, 1, 6); geo.translate(0, -0.5, 0);
    this.tuiles = new THREE.InstancedMesh(geo, MAT.clone(), T.length);
    this.tuiles.material.vertexColors = false;
    const mtx = new THREE.Matrix4(), col = new THREE.Color();
    T.forEach((t, k) => {
      t.top = this.haut.get(t.M) + (t.n - 0.5) * 0.25;
      mtx.makeScale(1, t.top + 8, 1); mtx.setPosition(t.X, t.top, t.Z); this.tuiles.setMatrixAt(k, mtx);
      this.tuiles.setColorAt(k, col.set(0x777777));
    });
    this.tuiles.receiveShadow = true; this.monde.add(this.tuiles);
    this.parQR = new Map(T.map(t => [t.q + ',' + t.r, t]));
    // les voies d'abord : on ne plante ni ne construit sur leur emprise
    this._routes();
    // végétation : troncs, houppiers, buissons (instanciés, pour brûler arbre par arbre)
    this._vegetation();
    this._bati();
    this._feux();
    this.unites = new Map();
    this.passes = [];
    this.convois = new Convois({ scene: this.monde }, 2.2, { vers: (a, b) => this.vers3(a, b), sol: (x, z) => this.hauteurSous(x, z) });
    this.lances = new THREE.Group(); this.monde.add(this.lances);
    this.etis = new Map();
  }

  _vegetation() {
    const C = this.C, arbres = [];
    for (const t of this.T) {
      const f = C.fuel[t.M]; semer(t.q * 31 + t.r, 9);
      if ((this.P.site[t.M] >= 0 && 'UBZ'.includes(f)) || t.voie) continue;
      const pos = () => [t.X + (rnd() - 0.5) * RM * 1.1, t.Z + (rnd() - 0.5) * RM * 1.1];
      const add = (k, s) => { const [x, z] = pos(); arbres.push({ k, x, z, y: t.top, s: s * entre(0.8, 1.2), t, ry: rnd() * 6 }); };
      if (f === 'P' || f === 'T') { add('pin', 1); if (rnd() < 0.55) add('pin', 0.9); if (rnd() < 0.3) add('buisson', 0.6); }
      else if (f === 'F') { add('rond', 1); if (rnd() < 0.5) add('rond', 0.8); }
      else if (f === 'J') { add('buisson', 0.8); add('buisson', 0.7); if (rnd() < 0.4) add('pin', 0.8); }
      else if (f === 'G') { add('buisson', 0.6); add('buisson', 0.55); if (rnd() < 0.5) add('buisson', 0.5); }
      else if (f === 'Q') { add('buisson', 0.9); add('buisson', 0.8); if (rnd() < 0.4) add('pinA', 0.8); }
      else if (f === 'O') { if (rnd() < 0.7) add('olivier', 0.9); }
      else if (f === 'H' || f === 'D') { if (rnd() < 0.3) add('buisson', 0.4); }
    }
    this.arbres = arbres;
    const k = 0.9;   // échelle des arbres dans la vue rapprochée
    const geos = {
      tronc: (() => { const g = new THREE.CylinderGeometry(0.22 * k, 0.32 * k, 3.4 * k, 5); g.translate(0, 1.7 * k, 0); return g; })(),
      pin: (() => { const g = new THREE.IcosahedronGeometry(1.7 * k, 0); g.scale(1, 0.45, 1); g.translate(0, 3.7 * k, 0); return g; })(),
      pinA: (() => { const g = new THREE.IcosahedronGeometry(1.5 * k, 0); g.scale(1.1, 0.75, 1); g.translate(0, 3.0 * k, 0); return g; })(),
      rond: (() => { const g = new THREE.IcosahedronGeometry(1.8 * k, 0); g.translate(0, 2.8 * k, 0); return g; })(),
      buisson: (() => { const g = new THREE.IcosahedronGeometry(0.95 * k, 0); g.scale(1.3, 0.65, 1); g.translate(0, 0.45 * k, 0); return g; })(),
      olivier: (() => { const g = new THREE.IcosahedronGeometry(1.0 * k, 0); g.translate(0, 1.5 * k, 0); return g; })(),
    };
    const mk = (geo, n, ombre = true) => {
      const m = new THREE.InstancedMesh(geo, new THREE.MeshToonMaterial({ gradientMap: DEGRADE }), Math.max(1, n));
      m.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(Math.max(1, n) * 3), 3);
      m.castShadow = ombre; m.receiveShadow = true; this.monde.add(m);
      const o = new THREE.InstancedMesh(geo, CONT, Math.max(1, n)); this.monde.add(o); m.contour = o; return m;
    };
    const troncs = arbres.filter(a => a.k !== 'buisson');
    this.mTronc = mk(geos.tronc, troncs.length);
    this.mHoup = {};
    for (const kk of ['pin', 'pinA', 'rond', 'buisson', 'olivier']) this.mHoup[kk] = mk(geos[kk], arbres.filter(a => a.k === kk).length);
    // index de chaque arbre dans ses instanciations
    const n = {}; let nt = 0;
    for (const a of arbres) { a.ih = n[a.k] = (n[a.k] ?? -1) + 1; a.it = a.k !== 'buisson' ? nt++ : -1; a.sig = ''; }
    this.mTronc.count = nt; for (const kk in this.mHoup) this.mHoup[kk].count = this.mHoup[kk].contour.count = (n[kk] ?? -1) + 1;
    this.mTronc.contour.count = 0;
  }

  _bati() {
    const C = this.C, P = this.P;
    this.bati = new Map();
    for (const i of this.macro) if (P.site[i] >= 0) this._batiCellule(i, '.');
  }
  _batiCellule(i, e) {
    const C = this.C, P = this.P, s = C.sites[P.site[i]];
    jeter(this.bati.get(i));
    const L = lot(), tl = this.T.filter(t => t.M === i), landes = C.theme === 'gironde';
    semer(i, 5);
    const et = e === 'b' ? 'b' : '.';
    for (const t of tl) {
      if (rnd() < 0.25 || t.voie) continue;
      const y = t.top, ry = rnd() * 6;
      if (s.type === 'ville') (rnd() < 0.5 ? M.immeuble : M.maison)(L, t.X, y, t.Z, ry, 1.0, et);
      else if (s.type === 'village') (landes ? M.maisonLandaise : M.maison)(L, t.X, y, t.Z, ry, 0.95, et);
      else if (s.type === 'camping') { if (rnd() < 0.5) M.tente(L, t.X, y, t.Z, ry, 1.1, et); else M.mobilhome(L, t.X, y, t.Z, ry, 1, et); if (rnd() < 0.6) M.pin(L, t.X + 1.5, y, t.Z - 1.2, 0.8, e === 'b' ? 'b' : '.'); }
      else if (s.type === 'usine') { if (t === tl[0]) M.usine(L, t.X, y, t.Z, 0.9, et); else if (rnd() < 0.4) M.maison(L, t.X, y, t.Z, ry, 1.1, et); }
      else if (s.type === 'patrimoine' && t === tl[Math.floor(tl.length / 2)]) M.chapelle(L, t.X, y, t.Z, 1.1);
    }
    const g = fige(L, this.monde); this.bati.set(i, g); g.userData.e = e;
  }

  tuileDe(X, Z) {
    const rm = this.C.hexM / DIV / SQ3, x = X / this.S2, y = Z / this.S2;
    const q = (SQ3 / 3 * x - y / 3) / rm, r = (2 / 3 * y) / rm, s = -q - r;
    let rq = Math.round(q), rr = Math.round(r); const rs = Math.round(s);
    const dq = Math.abs(rq - q), dr = Math.abs(rr - r), ds = Math.abs(rs - s);
    if (dq > dr && dq > ds) rq = -rr - rs; else if (dr > ds) rr = -rq - rs;
    return this.parQR.get(rq + ',' + rr);
  }
  macroDe(X, Z) { const [a, b] = this.P.vers3D(X / this.S2 + this.mx0, Z / this.S2 + this.my0); return this.P.cellule(a, b); }
  _routes() {
    const C = this.C, P = this.P, L = lot(), D = lot();
    const voies = [];
    for (const r of C.routes) {
      const pm = r.p || r.c.map(i => P.versMetres(P.X[i], P.Z[i]));
      let run = [];
      const fin = () => { if (run.length > 1) voies.push({ n: r.n, p: run }); run = []; };
      for (let k = 0; k < pm.length - 1; k++) {
        const [ax, ay] = pm[k], [bx, by] = pm[k + 1], n = Math.max(1, Math.ceil(Math.hypot(bx - ax, by - ay) / 40));
        for (let j = 0; j <= n; j++) {
          if (j === n && k < pm.length - 2) continue;
          const x = ax + (bx - ax) * j / n, y = ay + (by - ay) * j / n, [X, Z] = this.vers3(x, y);
          if (this.macro.has(this.macroDe(X, Z))) run.push([X, Z]); else fin();
        }
      }
      fin();
    }
    const haut = (X, Z) => { const i = this.macroDe(X, Z); return (this.haut.get(i) ?? 0) + (C.fuel[i] === 'R' ? 1.6 : 0); };
    const sol = (X, Z) => Math.max(haut(X, Z), haut(X + 1.5, Z), haut(X - 1.5, Z), haut(X, Z + 1.5), haut(X, Z - 1.5)) + 0.25;
    const site = (X, Z) => { const i = this.macroDe(X, Z); const k = i >= 0 ? P.site[i] : -1; return k >= 0 && ['ville', 'village', 'camping'].includes(C.sites[k].type) ? k : -1; };
    const pont = (X, Z) => C.fuel[this.macroDe(X, Z)] === 'R';
    const route = (X, Z) => { const i = this.macroDe(X, Z); return i >= 0 && [i, ...P.voisins(i)].some(j => j >= 0 && +C.route[j] >= 2); };
    const marquer = (X, Z, w) => {
      for (const [dx, dz] of [[0, 0], [w, 0], [-w, 0], [0, w], [0, -w]]) { const t = this.tuileDe(X + dx, Z + dz); if (t) t.voie = true; }
    };
    dessinerVoies(voies, { s: 1.8, sol, site, pont, route, marquer, joint: C.hexM * this.S2 }, L, D);
    fige(L, this.monde, { ombre: false, contour: false }); fige(D, this.monde);
  }

  _feux() {
    const MAXF = 6000, cone1 = new THREE.ConeGeometry(1, 1, 6); cone1.translate(0, 0.5, 0);
    this.fl1 = new THREE.InstancedMesh(cone1, new THREE.MeshBasicMaterial({ color: 0xff6a1a }), MAXF);
    this.fl2 = new THREE.InstancedMesh(cone1, new THREE.MeshBasicMaterial({ color: 0xffd34d }), MAXF);
    for (const m of [this.fl1, this.fl2]) { m.count = 0; m.frustumCulled = false; this.monde.add(m); }
    const disque = new THREE.CylinderGeometry(RM * 1.05, RM * 1.05, 0.1, 6);
    this.lueur = new THREE.InstancedMesh(disque, new THREE.MeshBasicMaterial({ color: 0xff5a10, transparent: true, opacity: 0.35, blending: THREE.AdditiveBlending, depthWrite: false }), this.T.length);
    this.lueur.count = 0; this.lueur.frustumCulled = false; this.lueur.renderOrder = 3; this.monde.add(this.lueur);
    this.MAXS = 1100;
    this.fumee = new THREE.InstancedMesh(new THREE.IcosahedronGeometry(1, 1), new THREE.MeshToonMaterial({ color: 0xffffff, gradientMap: DEGRADE, transparent: true, opacity: 0.6, depthWrite: false }), this.MAXS);
    this.fumee.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(this.MAXS * 3), 3);
    this.fumee.count = 0; this.fumee.frustumCulled = false; this.fumee.renderOrder = 4; this.monde.add(this.fumee);
    this.parts = [];
    this.MAXG = 4000;
    this.gouttes = new THREE.InstancedMesh(new THREE.IcosahedronGeometry(0.3, 0), new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.85 }), this.MAXG);
    this.gouttes.instanceColor = new THREE.InstancedBufferAttribute(new Float32Array(this.MAXG * 3), 3);
    this.gouttes.count = 0; this.gouttes.frustumCulled = false; this.monde.add(this.gouttes);
    this.gt = [];
    this.flammes = [];
  }

  // ------------------------------------------------------------------ l'état du moteur, découpé en petits hexagones
  // s : position du petit hexagone dans le grand, le long de la marche du feu (−1 à l'arrivée du feu, +1 à l'opposé)
  _s(t, d) {
    const P = this.P, C = this.C, M = t.M;
    const [cx, cy] = P.versMetres(P.X[M], P.Z[M]);
    const dx = t.x - cx, dy = t.y - cy, h = C.hexM / 2;
    if (d < 0) return Math.hypot(dx, dy) / h * 2 - 1 + (t.n - 0.5) * 0.5;
    const v = P.voisins(M)[d];
    if (v < 0) return (t.n - 0.5);
    const [vx, vy] = P.versMetres(P.X[v], P.Z[v]); const l = Math.hypot(vx - cx, vy - cy);
    // le feu vient du voisin d : il avance dans la direction opposée
    return -((dx * (vx - cx) + dy * (vy - cy)) / l) / h + (t.n - 0.5) * 0.45;
  }
  majEtat(E, now, force) {
    if (!this.actif && !force) return;
    this.E = E;
    const C = this.C, P = this.P;
    const brul = new Map(E.feu.map(f => [f[0], f])), prog = new Map(E.prog), ret = new Set(E.ret), mou = new Map(E.mouille);
    const col = new THREE.Color(), cF = new THREE.Color(), fl = [];
    let tuilesSales = false;
    for (let k = 0; k < this.T.length; k++) {
      const t = this.T[k], M = t.M, f = C.fuel[M];
      let e = E.etat[M], I = 0, st = '.';
      if (e === 'f') {
        const [, Ii, p, d] = brul.get(M) || [0, 50, 0, -1];
        I = Ii / 100;
        const s = this._s(t, d), fp = -1.3 + 2.9 * clamp(p / 25, 0, 1);
        st = s < fp - 0.95 ? 'b' : s < fp ? 'f' : '.';
        if (st === 'b' && p < 60) st = 'c';                        // braises : vient de brûler
      } else if (e === 'e' || e === 'n') {
        const p = prog.get(M) ?? 50, s = this._s(t, -1), fp = -1.3 + 2.9 * clamp(p / 25, 0, 1);
        st = s < fp ? 'e' : '.';
      } else if (e === 'b') st = 'b';
      else if (e === 'x') st = 'x';
      const r = ret.has(M) && st !== 'b' && st !== 'f', w = (mou.get(M) || 0) / 100;
      t.st = st; t.I = I;
      const sig = st + (r ? 'r' : '') + Math.round(w * 4);
      if (sig !== t.sig) {
        t.sig = sig; tuilesSales = true;
        if ('MLR'.includes(f)) col.set(f === 'M' ? 0x1d5a86 : 0x3fa6dc);
        else if (st === 'b') col.set(0x34302c);
        else if (st === 'c') col.set(0x4a2a1a);
        else if (st === 'e') col.set(0x4a4038);
        else if (st === 'x') col.set(0xb8955f);
        else { col.set(C.comb[f].col); if (P.site[M] >= 0 && 'UBZ'.includes(f)) col.set(0xd8cba8); if (st === 'f') col.lerp(cF.set(0x5a2a12), 0.6); }
        if (r) col.lerp(cF.set(0xd2453c), 0.6);
        if (w > 0.1 && st !== 'b') col.lerp(cF.set(0x2f6fb8), Math.min(0.5, w * 0.6));
        col.offsetHSL(0, 0, (t.n - 0.5) * 0.06);
        this.tuiles.setColorAt(k, col);
      }
      if (st === 'f') {
        const nb = 1 + Math.round(I * 2.5) + ('PQT'.includes(f) ? 1 : 0);
        for (let j = 0; j < nb; j++) fl.push({ x: t.X + (hache(k, j, 3) - 0.5) * RM * 1.3, z: t.Z + (hache(k, j, 5) - 0.5) * RM * 1.3, y: t.top, h: (1.5 + 7 * I) * ('PQ'.includes(f) ? 1.25 : 'HCV'.includes(f) ? 0.45 : 0.8) * (0.6 + 0.6 * hache(k, j, 9)), ph: hache(k, j, 11) * 6.28, I });
      }
    }
    if (tuilesSales) this.tuiles.instanceColor.needsUpdate = true;
    this.flammes = fl;
    const m = this.lueur, mat = new THREE.Matrix4(); let n = 0;
    for (const t of this.T) if (t.st === 'f' || t.st === 'c') { const s = t.st === 'f' ? 0.7 + t.I * 0.4 : 0.6; mat.makeScale(s, 1, s); mat.setPosition(t.X, t.top + 0.15, t.Z); m.setMatrixAt(n++, mat); }
    m.count = n; m.instanceMatrix.needsUpdate = true;
    this._arbres();
    // le bâti brûle avec son hexagone
    for (const [i, g] of this.bati) { const e = E.etat[i] === 'b' ? 'b' : '.'; if (g.userData.e !== e) this._batiCellule(i, e); }
    this._unites(E, now);
    const lim = (RAY + 1) * C.hexM;
    this.convois.maj((E.evac?.convois || []).filter(c => Math.hypot(c.x - this.mx0, c.y - this.my0) < lim), now, E.meteo.t);
    this._hud(E);
  }
  _arbres() {
    const mat = new THREE.Matrix4(), q = new THREE.Quaternion(), v = new THREE.Vector3(), sc = new THREE.Vector3(), col = new THREE.Color();
    const COUL = { pin: [0x4f7d3a, 0x5c8a40], pinA: [0x5d8f3c, 0x6a9a44], rond: [0x5fae3e, 0x4f9a37], buisson: [0x8a9a4a, 0x6f8f44], olivier: [0x8fa269, 0x8fa269] };
    const sales = new Set();
    for (const a of this.arbres) {
      const st = a.t.st, sig = st;
      if (sig === a.sig) continue;
      a.sig = sig; sales.add(a.k);
      const mort = st === 'b' || st === 'c' || st === 'e';
      q.setFromAxisAngle(v.set(0, 1, 0), a.ry); v.set(a.x, a.y, a.z);
      if (a.it >= 0) {
        sc.setScalar(a.s); if (mort) sc.y = a.s * (st === 'b' ? 0.7 : 0.85);
        mat.compose(v, q, sc); this.mTronc.setMatrixAt(a.it, mat);
        this.mTronc.setColorAt(a.it, col.set(mort ? 0x231d19 : 0x7a5233));
      }
      const hm = this.mHoup[a.k];
      sc.setScalar(mort ? 0.0001 : a.s);
      mat.compose(v, q, sc); hm.setMatrixAt(a.ih, mat); hm.contour.setMatrixAt(a.ih, mat.clone().multiply(new THREE.Matrix4().makeScale(1.12, 1.12, 1.12)));
      hm.setColorAt(a.ih, col.set(st === 'f' ? 0x8a4a1a : COUL[a.k][(a.ih & 1)]));
    }
    if (sales.size) {
      this.mTronc.instanceMatrix.needsUpdate = true; if (this.mTronc.instanceColor) this.mTronc.instanceColor.needsUpdate = true;
      for (const k in this.mHoup) { const m = this.mHoup[k]; m.instanceMatrix.needsUpdate = true; m.contour.instanceMatrix.needsUpdate = true; if (m.instanceColor) m.instanceColor.needsUpdate = true; }
    }
  }

  // ------------------------------------------------------------------ les moyens dans la zone
  vers3(mx, my) { return [(mx - this.mx0) * this.S2, (my - this.my0) * this.S2]; }
  hauteurSous(X, Z) {
    const [x, z] = [X / this.S2 + this.mx0, Z / this.S2 + this.my0];
    const [a, b] = this.P.vers3D(x, z), i = this.P.cellule(a, b);
    return this.haut.get(i) ?? 0;
  }
  _modele(d) {
    const o = { grp: new THREE.Group(), k: d.k, d };
    const L = d.k === 'ccf' ? M.ccf(1.5) : d.k === 'bull' ? M.bull(1.5) : d.k === 'vlhr' ? M.vlhr(1.5) : ['fpt', 'vpc', 'debr', 'ravit', 'vsav'].includes(d.k) ? M[d.k](1.5) : d.k === 'canadair' ? M.canadair(1.6) : d.k === 'dash' ? M.dash(1.6) : null;
    if (L) fige(L, o.grp);
    if (d.k === 'dragon') { const h = M.helico(1.4, M.COULEURS_DRAGON[1], M.COULEURS_DRAGON[0]); fige(h.L, o.grp); o.rotor = fige(h.R, o.grp); o.rotor.position.y = h.rotorY; }
    if (d.k === 'helico') { const h = M.helico(1.5); fige(h.L, o.grp); o.rotor = fige(h.R, o.grp); o.rotor.position.y = h.rotorY; o.seau = fige(M.seau(1.6), o.grp); }
    if (d.k === 'ccf') { o.gyro = new THREE.Mesh(new THREE.SphereGeometry(0.4, 6, 4), new THREE.MeshBasicMaterial({ color: 0x3b7bff })); o.gyro.position.set(1.7, 2.45, 0); o.grp.add(o.gyro); }
    if (!['ccf', 'bull', 'vlhr', 'fpt', 'vpc', 'debr', 'ravit', 'vsav'].includes(d.k)) { o.ombre = new THREE.Mesh(new THREE.CircleGeometry(d.k === 'helico' ? 3.5 : 5.5, 16).rotateX(-Math.PI / 2), new THREE.MeshBasicMaterial({ color: 0, transparent: true, opacity: 0.22, depthWrite: false })); this.monde.add(o.ombre); }
    this.monde.add(o.grp);
    return o;
  }
  _unites(E, now) {
    const vus = new Set(), neuf = E.meteo.t !== this.tU, tPrec = this.tU ?? E.meteo.t;
    this.tU = E.meteo.t;
    const lim = (RAY + 1.5) * this.C.hexM;
    for (const d of E.moyens) {
      const dist = Math.hypot(d.x - this.mx0, d.y - this.my0);
      const garder = dist < lim || ['canadair', 'dash', 'helico', 'dragon'].includes(d.k) && dist < lim * 2.2;
      if (!garder) continue;
      vus.add(d.id);
      let o = this.unites.get(d.id);
      if (!o) { o = this._modele(d); this.unites.set(d.id, o); o.x0 = o.x1 = d.x; o.y0 = o.y1 = d.y; o.cap0 = o.cap1 = d.cap; o.t0 = now; }
      else if (neuf) { suivre(o, clamp((now - o.t0) / (this.dureeU || 1000), 0, 1)); trajet(o, d, tPrec, E.meteo.t); o.t0 = now; }
      o.d = d;
      // au poste, un camion se gare dans la partie sûre de son hexagone, côté feu, décalé selon son numéro
      o.off = [0, 0];
      if (['ccf', 'bull', 'vlhr', 'fpt', 'vpc', 'debr', 'ravit', 'vsav'].includes(d.k) && d.e !== 'route') {
        const cible = d.a?.c ?? d.c;
        const tl = this.T.filter(t => t.M === d.c && t.st === '.');
        if (tl.length) {
          const [tx, ty] = cible != null && cible !== d.c ? (() => { const [a, b] = this.P.versMetres(this.P.X[cible], this.P.Z[cible]); return [a, b]; })() : [d.x, d.y];
          tl.sort((a, b) => Math.hypot(a.x - tx, a.y - ty) - Math.hypot(b.x - tx, b.y - ty));
          const t = tl[Math.min(tl.length - 1, (d.id * 3) % Math.min(6, tl.length))];
          o.off = [t.x - d.x, t.y - d.y];
        }
      }
    }
    for (const [id, o] of this.unites) if (!vus.has(id)) { jeter(o.grp); if (o.ombre) jeter(o.ombre); this.unites.delete(id); }
  }

  // ------------------------------------------------------------------ largages : passes animées
  largage(v) {
    if (!this.actif) return;
    const cells = v.cells.filter(i => this.macro.has(i));
    if (!cells.length) return;
    const pts = cells.map(i => this.vers3(...this.P.versMetres(this.P.X[i], this.P.Z[i])));
    const cx = pts.reduce((s, p) => s + p[0], 0) / pts.length, cz = pts.reduce((s, p) => s + p[1], 0) / pts.length;
    const cap = (v.cap ?? 0) * Math.PI / 180;
    const ux = Math.cos(cap), uz = Math.sin(cap);
    // la passe : l'avion arrive de 120 unités, descend, largue au-dessus des cellules, remonte
    const o = this.unites.get(v.moyen);
    this.passes.push({ k: v.kind, produit: v.produit, cells, cx, cz, ux, uz, t: 0, T: v.kind === 'helico' ? 3.2 : 4.5, o, fait: false });
  }
  _rideau(cells, produit, kind) {
    const col = produit === 'retardant' ? 0xe0403a : 0xe4f4ff;
    for (const i of cells) {
      const tl = this.T.filter(t => t.M === i);
      for (const t of tl) for (let k = 0; k < (kind === 'helico' ? 5 : 9); k++) {
        if (this.gt.length >= this.MAXG) return;
        this.gt.push({ x: t.X + (Math.random() - 0.5) * RM * 1.6, y: t.top + (kind === 'helico' ? 20 : 12) + Math.random() * 5, z: t.Z + (Math.random() - 0.5) * RM * 1.6, vx: 0, vy: -4 - Math.random() * 5, vz: 0, t: 0, vie: 2.4, col, s: 0.7 + Math.random() * 0.8, g: 7 });
      }
    }
  }

  // ------------------------------------------------------------------ étiquettes de stratégie et panneau
  _hud(E) {
    const f = E.feux.find(x => x.id === this.feuId);
    if (!f) { this.hud.innerHTML = '<h3>Feu terminé</h3><p class="sous">Revenez à la carte.</p>'; return; }
    const [c, lib] = UI.statut(f.statut);
    const moy = Object.entries(f.moyens).map(([k, n]) => `${UI.ICONE[k]} ${n}`).join(' · ');
    const tete = f.tete != null && !this.macro.has(f.tete) ? '<button type="button" class="btn" data-recentrer>⌖ Suivre la tête</button>' : '<button type="button" class="btn" data-recentrer>⌖ Recentrer</button>';
    this.hud.innerHTML = `<div style="display:flex;justify-content:space-between;gap:8px;align-items:baseline"><h3>🔥 ${UI.esc(f.nom)}</h3><span class="ha" style="color:var(--or);font-family:var(--titre);font-weight:700">${UI.nf(f.ha)} ha</span></div>
      <div class="chips"><span class="chip"><i style="background:${c}"></i>${lib}</span>${f.vitesse ? `<span class="chip">tête ${String(f.vitesse).replace('.', ',')} km/h</span>` : ''}<span class="chip">vent ${UI.rose(E.meteo.de)} ${E.meteo.vent} km/h</span>${moy ? `<span class="chip">${moy}</span>` : ''}</div>
      <div class="sous" style="color:var(--doux)">🧭 ${UI.esc(f.strategie || 'Évaluation en cours')}</div>
      <div class="lg"><span><i style="background:#ff6a1a"></i>Flammes</span><span><i style="background:#4a2a1a"></i>Braises</span><span><i style="background:#34302c"></i>Brûlé</span><span><i style="background:#d2453c"></i>Retardant</span><span><i style="background:#2f6fb8"></i>Mouillé</span>${tete}</div>`;
  }
  _etiquette(cle, cls, html, X, Y, Z) {
    let el = this.etis.get(cle);
    if (!el) { el = document.createElement('div'); el.className = 'eti ' + cls; this.eti.appendChild(el); this.etis.set(cle, el); }
    el._vu = true;
    if (el._h !== html) { el.innerHTML = html; el._h = html; }
    const v = new THREE.Vector3(X, Y, Z).project(this.cam);
    if (v.z > 1 || Math.abs(v.x) > 1.1 || Math.abs(v.y) > 1.1) { el.hidden = true; return; }
    el.hidden = false;
    el.style.transform = `translate(${Math.round((v.x + 1) / 2 * this.W - el.offsetWidth / 2)}px, ${Math.round((1 - v.y) / 2 * this.H - el.offsetHeight)}px)`;
  }
  _etiquettes() {
    const E = this.E, P = this.P, C = this.C;
    for (const el of this.etis.values()) el._vu = false;
    const f = E.feux.find(x => x.id === this.feuId);
    if (f) {
      if (f.tete != null && this.macro.has(f.tete)) {
        const [x, z] = this.vers3(...P.versMetres(P.X[f.tete], P.Z[f.tete]));
        this._etiquette('tete', 'tete', `TÊTE DU FEU · ${String(f.vitesse).replace('.', ',')} km/h`, x, this.haut.get(f.tete) + 14, z);
      }
      // menaces
      for (const m of f.menaces.slice(0, 3)) {
        const s = C.sites[m.site]; if (!this.macro.has(s.centre)) continue;
        const [x, z] = this.vers3(...P.versMetres(P.X[s.centre], P.Z[s.centre]));
        this._etiquette('s' + s.id, 'strat', `🛡 ${UI.esc(s.nom)} · feu dans ${m.eta} min`, x, this.haut.get(s.centre) + 12, z);
      }
    }
    // les missions : un mot par poste
    const postes = new Map();
    for (const o of this.unites.values()) {
      const d = o.d; if (!d.m || d.p == null || !this.macro.has(d.p)) continue;
      const k = d.p + d.mt; const e = postes.get(k) || { p: d.p, m: d.m, n: 0, ic: UI.ICONE[d.k] }; e.n++; postes.set(k, e);
    }
    for (const [k, e] of postes) {
      const [x, z] = this.vers3(...P.versMetres(P.X[e.p], P.Z[e.p]));
      this._etiquette('p' + k, 'strat', `${e.ic} ${UI.esc(e.m)}${e.n > 1 ? ' ×' + e.n : ''}`, x, this.haut.get(e.p) + 8, z);
    }
    // blessés à secourir
    for (const b of E.blesses || []) {
      if (!this.macro.has(b.cell)) continue;
      const [x, z] = this.vers3(...P.versMetres(P.X[b.cell], P.Z[b.cell]));
      this._etiquette('b' + b.id, 'blesse', `🚑 ${b.n} blessé${b.n > 1 ? 's' : ''} · ${UI.esc(b.quoi)}`, x, this.haut.get(b.cell) + 7, z);
    }
    // ligne de retardant
    const ret = E.ret.filter(i => this.macro.has(i));
    if (ret.length) {
      const i = ret[Math.floor(ret.length / 2)], [x, z] = this.vers3(...P.versMetres(P.X[i], P.Z[i]));
      this._etiquette('ret', 'strat', '🟥 Ligne de retardant', x, this.haut.get(i) + 6, z);
    }
    // avions en passe
    for (const p of this.passes) if (p.o && p.pos) this._etiquette('pa' + p.o.d.id, 'moyen', `${UI.ICONE[p.k]} ${UI.esc(p.o.d.n)} · largage`, p.pos[0], p.pos[1] + 5, p.pos[2]);
    if (this.choisi) { const o = this.unites.get(this.choisi); if (o?.pos) this._etiquette('sel', 'moyen', `${UI.ICONE[o.d.k]} ${UI.esc(o.d.n)} · ${UI.esc(UI.etatMoyen(o.d))}`, o.pos[0], o.pos[1] + 6, o.pos[2]); }
    for (const [k, el] of this.etis) if (!el._vu) { el.remove(); this.etis.delete(k); }
  }
  tapMoyen(cx, cy) {
    const r = this.zone.getBoundingClientRect(); let best = null, bd = 30;
    for (const [id, o] of this.unites) { if (!o.pos) continue; const v = new THREE.Vector3(...o.pos).project(this.cam); const d = Math.hypot((v.x + 1) / 2 * this.W + r.left - cx, (1 - v.y) / 2 * this.H + r.top - cy); if (d < bd) { bd = d; best = id; } }
    return best;
  }
  tap(cx, cy) { this.choisi = this.tapMoyen(cx, cy); return this.choisi; }

  // ------------------------------------------------------------------ chaque image
  anime(dt, t, now, duree) {
    this.camera.anime(dt);
    const E = this.E; if (!E) return;
    const de = E.meteo.de * Math.PI / 180, ax = -Math.sin(de), az = Math.cos(de), V = E.meteo.vent;
    const mat = new THREE.Matrix4(), q = new THREE.Quaternion(), e = new THREE.Euler(), v = new THREE.Vector3(), sc = new THREE.Vector3(), cc = new THREE.Color();
    // flammes
    const pen = Math.min(0.55, V / 80); let n = 0;
    for (const f of this.flammes) {
      if (n >= 5990) break;
      const k = 0.72 + 0.28 * Math.sin(t * 10 + f.ph) + 0.1 * Math.sin(t * 27 + f.ph * 3);
      e.set(az * pen, 0, -ax * pen); q.setFromEuler(e); v.set(f.x, f.y, f.z);
      sc.set(0.8 + f.I * 0.9, f.h * k, 0.8 + f.I * 0.9); mat.compose(v, q, sc); this.fl1.setMatrixAt(n, mat);
      sc.set(0.45 + f.I * 0.45, f.h * k * 0.6, 0.45 + f.I * 0.45); mat.compose(v, q, sc); this.fl2.setMatrixAt(n, mat); n++;
    }
    this.fl1.count = this.fl2.count = n; this.fl1.instanceMatrix.needsUpdate = this.fl2.instanceMatrix.needsUpdate = true;
    // fumée
    const nb = Math.min(40, 4 + this.flammes.length * dt * 1.5);
    for (let k = 0; k < nb && this.parts.length < this.MAXS; k++) {
      const f = this.flammes[(Math.random() * this.flammes.length) | 0]; if (!f) break;
      this.parts.push({ x: f.x, y: f.y + f.h, z: f.z, t: 0, vie: 6 + Math.random() * 6, s0: 1.2 + f.I * 2.2, d: f.I });
    }
    const vv = 4 + V / 5; let m2 = 0;
    for (let k = this.parts.length - 1; k >= 0; k--) {
      const p = this.parts[k]; p.t += dt; if (p.t > p.vie) { this.parts.splice(k, 1); continue; }
      const u = p.t / p.vie; p.x += ax * vv * dt; p.z += az * vv * dt; p.y += (4 - u * 2) * dt;
      const s = p.s0 * (1 + u * 5) * (u > 0.8 ? (1 - u) * 5 : 1);
      mat.makeScale(s, s * 0.8, s); mat.setPosition(p.x, p.y, p.z); this.fumee.setMatrixAt(m2, mat);
      cc.setRGB(0.3 + u * 0.5 - p.d * 0.1, 0.29 + u * 0.48 - p.d * 0.1, 0.28 + u * 0.46 - p.d * 0.1); this.fumee.setColorAt(m2, cc); m2++;
    }
    this.fumee.count = m2; this.fumee.instanceMatrix.needsUpdate = true; this.fumee.instanceColor.needsUpdate = true;
    // moyens et convois
    this._animeUnites(dt, t, now, duree);
    this.convois.anime(dt, t, now, duree);
    // passes de largage
    for (let k = this.passes.length - 1; k >= 0; k--) {
      const p = this.passes[k]; p.t += dt; const u = p.t / p.T;
      if (u >= 1) { this.passes.splice(k, 1); continue; }
      const d = (u - 0.5) * (p.k === 'helico' ? 60 : 200);
      const sol = this.hauteurSous(p.cx, p.cz);
      const alt = p.k === 'helico' ? 22 : 10 + Math.abs(u - 0.5) * 2 * 40;
      const x = p.cx + p.ux * d, z = p.cz + p.uz * d;
      p.pos = [x, sol + alt, z];
      if (p.o) { p.o.passe = p; }
      if (!p.fait && u > 0.45) { p.fait = true; this._rideau(p.cells, p.produit, p.k); }
    }
    // gouttes
    let m3 = 0;
    for (let k = this.gt.length - 1; k >= 0; k--) {
      const p = this.gt[k]; p.t += dt; if (p.t > p.vie) { this.gt.splice(k, 1); continue; }
      p.vy -= (p.g ?? 7) * dt; p.x += p.vx * dt; p.y += p.vy * dt; p.z += p.vz * dt;
      mat.makeScale(p.s, p.s * 1.7, p.s); mat.setPosition(p.x, p.y, p.z); this.gouttes.setMatrixAt(m3, mat); cc.set(p.col); this.gouttes.setColorAt(m3, cc); m3++;
    }
    this.gouttes.count = m3; this.gouttes.instanceMatrix.needsUpdate = true; if (m3) this.gouttes.instanceColor.needsUpdate = true;
    this.soleil.position.copy(this.camera.cible).add(new THREE.Vector3(-120, 220, 110)); this.soleil.target.position.copy(this.camera.cible);
    this._etiquettes();
  }
  _animeUnites(dt, t, now, duree) {
    this.dureeU = duree;
    // pompiers et lances : refaits quelques fois par seconde
    const refaire = !this.tl || now - this.tl > 150;
    if (refaire) { this.tl = now; while (this.lances.children.length) jeter(this.lances.children[0]); }
    const L = lot();
    for (const o of this.unites.values()) {
      const d = o.d, a = clamp((now - o.t0) / duree, 0, 1);
      const [px, py, cap] = suivre(o, a);
      const mx = px + (o.off?.[0] || 0), my = py + (o.off?.[1] || 0);
      o.xa = px; o.ya = py; o.capa = cap;
      let [X, Z] = this.vers3(mx, my);
      const sol = this.hauteurSous(X, Z), act = d.a?.k;
      let y = sol;
      if (d.k === 'helico') y = sol + (d.e === 'au_sol' || d.e === 'dispo' ? 0.5 : act === 'ecope' ? 8 : 24);
      if (d.k === 'dragon') y = sol + (d.e === 'dispo' ? 0.5 : act === 'treuille' ? 9 : 26);
      if (d.k === 'canadair') y = sol + (d.e === 'au_sol' || d.e === 'dispo' ? 0.5 : act === 'ecope' ? 0.8 : 34);
      if (d.k === 'dash') y = sol + (d.e === 'au_sol' || d.e === 'dispo' ? 0.5 : 40);
      let ry = -cap * Math.PI / 180;
      if (o.passe && o.passe.pos && this.passes.includes(o.passe)) { [X, y, Z] = o.passe.pos; ry = -Math.atan2(o.passe.uz, o.passe.ux); }
      if (o.y3 === undefined) o.y3 = y;
      o.y3 += (y - o.y3) * Math.min(1, dt * 3);
      o.grp.position.set(X, o.y3, Z); o.grp.rotation.y = ry;
      if (o.ombre) { o.ombre.position.set(X, this.hauteurSous(X, Z) + 0.2, Z); o.ombre.visible = o.y3 > sol + 2; }
      if (o.rotor) { o.rotor.rotation.y += dt * 30; if (o.seau) o.seau.visible = o.y3 > sol + 4 && !!d.m; }
      if (o.gyro) { o.gyro.visible = d.e !== 'dispo' && !d.hs; o.gyro.material.color.setHex(Math.sin(t * 12 + d.id) > 0 ? 0x3b7bff : 0xbfd4ff); }
      o.pos = [X, o.y3, Z];
      if (d.a?.k === 'autoprotection' && this.gt.length < this.MAXG) for (let k = 0; k < 4; k++) {
        const a = Math.random() * 6.28;
        this.gt.push({ x: X, y: o.y3 + 2.4, z: Z, vx: Math.cos(a) * 5, vy: 3 + Math.random() * 2, vz: Math.sin(a) * 5, t: 0, vie: 0.8, col: 0xcfeaff, s: 0.3, g: 9.8 });
      }
      if (d.k === 'ccf') {
        if (!o.bulle) { o.bulle = new THREE.Mesh(new THREE.SphereGeometry(4.2, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2), new THREE.MeshBasicMaterial({ color: 0x6cc0ff, transparent: true, opacity: 0.42, depthWrite: false })); o.bulle.renderOrder = 6; o.grp.add(o.bulle); }
        o.bulle.visible = d.a?.k === 'autoprotection';
      }
      if (d.hs && !o.epave) { o.epave = true; const mat = new THREE.MeshToonMaterial({ color: 0x2b2622, gradientMap: DEGRADE }); o.grp.traverse(c => { if (c.isMesh && c.material !== CONT && c !== o.ombre && c !== o.bulle) c.material = mat; }); if (o.gyro) o.gyro.visible = false; }
      if (refaire && d.k === 'ccf' && d.dir != null && d.e === 'action') {
        const m = eventail(RM * DIV * 1.4, d.dir, 50, d.j ? 0.2 : 0.12); m.position.set(X, o.y3 + 0.6, Z); this.lances.add(m);
      }
      // pompiers et lances
      if (refaire && d.k === 'ccf' && (act === 'arrose' || act === 'protege' || act === 'noie' || act === 'attend' || act === 'surveille') && d.e === 'action') {
        let cible = null;
        if (act === 'arrose' || act === 'noie') {
          const tl = this.T.filter(tt => tt.M === d.a.c && (act === 'noie' ? tt.st === 'e' : tt.st === 'f'));
          if (tl.length) cible = tl.reduce((b, tt) => (Math.hypot(tt.X - X, tt.Z - Z) < Math.hypot(b.X - X, b.Z - Z) ? tt : b));
        }
        const ang = cible ? Math.atan2(cible.Z - Z, cible.X - X) : -ry + Math.PI / 2;
        const nbp = act === 'attend' || act === 'surveille' ? 2 : 3;
        for (let k = 0; k < nbp; k++) {
          const l = 3 + k * 1.6, lat = (k - 1) * 1.1;
          const px = X + Math.cos(ang) * l - Math.sin(ang) * lat, pz = Z + Math.sin(ang) * l + Math.cos(ang) * lat;
          M.pompier(L, px, this.hauteurSous(px, pz), pz, -ang, 1.3);
        }
        if (cible) {
          const lx = X + Math.cos(ang) * 6.5, lz = Z + Math.sin(ang) * 6.5, ly = this.hauteurSous(lx, lz) + 1.6;
          // le tuyau, au sol, du camion au porte-lance
          const h = new THREE.CylinderGeometry(0.12, 0.12, 6, 4); h.rotateZ(Math.PI / 2);
          put(L, h, 0xd96a2b, 0, 0, 0, 0, 0); const g = L.g[L.g.length - 1]; g.rotateY(-ang); g.translate(X + Math.cos(ang) * 3.2, this.hauteurSous(X, Z) + 0.15, Z + Math.sin(ang) * 3.2);
          if (Math.random() < dt * 30 && this.gt.length < this.MAXG) {
            const x1 = cible.X + (Math.random() - 0.5) * 3, z1 = cible.Z + (Math.random() - 0.5) * 3, y1 = cible.top + 1, T = 0.8;
            this.gt.push({ x: lx, y: ly, z: lz, vx: (x1 - lx) / T, vy: (y1 - ly) / T + 0.5 * 9.8 * T, vz: (z1 - lz) / T, t: 0, vie: T, col: act === 'noie' ? 0xd8eeff : 0xbfe6ff, s: 0.35, g: 9.8 });
          }
        }
      }
      if (d.k === 'bull' && act === 'defriche' && Math.random() < dt * 8 && this.parts.length < this.MAXS) this.parts.push({ x: X, y: sol + 1, z: Z, t: 0, vie: 3, s0: 0.8, d: -1.5 });
    }
    if (L.g.length) fige(L, this.lances, { ombre: false, contour: true });
  }
}

function ruban(pts, larg) {
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
