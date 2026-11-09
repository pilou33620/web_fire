// Le geste de commandement : appuyer à l'endroit voulu, glisser pour orienter (lances, axe de largage) ou tracer
// (ligne d'arrêt, pare-feu), relâcher pour donner l'ordre. Marche au doigt, à la souris et au stylet (au stylet, le
// survol montre déjà le poste et le cône). Pendant le geste, un aperçu : trajet prévu, cône des lances, hexagones
// arrosés (bleu) ou trop chauds (rouge), eau restante ; pour les avions, le point d'écopage et l'empreinte du largage.
//
// Une « vue » (la carte ou la vue Intervention) fournit : cellule(cx, cy), sol(cx, cy) -> [x, z], pos(i) -> [x, y, z]
// ou null, posM(mx, my) -> [x, y, z], R (rayon d'un hexagone de la carte, en unités 3D), groupe (THREE.Group),
// ecran(x, y, z) -> [px, py] ou null.
import { THREE } from './moteur.js';
import * as UI from './ui.js';

const CONE = 50;
const MAT = {
  cone: new THREE.MeshBasicMaterial({ color: 0x4aa8ff, transparent: true, opacity: 0.28, side: THREE.DoubleSide, depthWrite: false }),
  bord: new THREE.LineBasicMaterial({ color: 0x9fd2ff, transparent: true, opacity: 0.9, depthTest: false }),
  bleu: new THREE.MeshBasicMaterial({ color: 0x3b8fe8, transparent: true, opacity: 0.45, depthWrite: false }),
  rouge: new THREE.MeshBasicMaterial({ color: 0xff3b1f, transparent: true, opacity: 0.55, depthWrite: false }),
  ret: new THREE.MeshBasicMaterial({ color: 0xe0403a, transparent: true, opacity: 0.5, depthWrite: false }),
  poste: new THREE.MeshBasicMaterial({ color: 0xffc94d, depthTest: false }),
  trajet: new THREE.LineDashedMaterial({ color: 0xffe08a, dashSize: 2.2, gapSize: 1.4, depthTest: false }),
  vol: new THREE.LineDashedMaterial({ color: 0xbfe6ff, dashSize: 3, gapSize: 2, depthTest: false }),
  axe: new THREE.MeshBasicMaterial({ color: 0xffffff, transparent: true, opacity: 0.8, depthTest: false }),
};

export class Commande {
  constructor({ L, getE, C, toast, zone, surOrdre }) {
    Object.assign(this, { L, getE, C, toast, zone, surOrdre });
    this.arme = null;            // { ordre, ids, kinds }
    this.geste = null;           // { vue, i0, i2, dir, ap }
    this.eti = document.createElement('div'); this.eti.className = 'eti strat'; this.eti.hidden = true;
    this.eti.style.cssText = 'position:absolute;left:0;top:0;z-index:4;pointer-events:none';
    zone.appendChild(this.eti);
    this.vues = new Set();
  }
  armer(ordre, ids, kinds) { this.arme = { ordre, ids, kinds }; }
  desarmer() { this.arme = null; this.annuler(); }
  annuler() { this.geste = null; for (const v of this.vues) this._vider(v); this.eti.hidden = true; }

  // ------------------------------------------------------------------ le geste
  press(vue, cx, cy) {
    if (!this.arme) return false;
    const [, , cibles] = UI.ORDRES[this.arme.ordre];
    if (!cibles) return false;
    const i = vue.cellule(cx, cy);
    if (i < 0) return false;
    this.vues.add(vue);
    this.geste = { vue, i0: i, i2: null, dir: null, ap: null, n: 0 };
    this._demander();
    this._dessiner();
    return true;
  }
  drag(vue, cx, cy) {
    const g = this.geste; if (!g || g.vue !== vue) return;
    const p0 = vue.pos(g.i0), s = vue.sol(cx, cy);
    if (!p0 || !s) return;
    const dx = s[0] - p0[0], dz = s[2] !== undefined ? s[2] - p0[2] : s[1] - p0[2];
    if (Math.hypot(dx, dz) > vue.R * 0.45) g.dir = Math.atan2(dz, dx) * 180 / Math.PI;
    const o = this.arme.ordre;
    if (o === 'ligne' || o === 'pare_feu') { const i2 = vue.cellule(cx, cy); if (i2 >= 0 && i2 !== g.i2) { g.i2 = i2; this._demander(); } }
    else if (o === 'arroser' || o === 'flanc' || o === 'larguer' || o === 'retardant') this._demander(true);
    this._dessiner();
  }
  async release(vue, cx, cy, annule) {
    const g = this.geste; this.geste = null;
    this._vider(vue); this.eti.hidden = true;
    if (!g || annule || !this.arme) return;
    const a = this.arme, o = a.ordre;
    const r = await this.L.action({ type: 'ordre', moyens: a.ids, ordre: o, cell: g.i0, cell2: g.i2 ?? undefined, dir: g.dir ?? undefined });
    if (!r.ok) this.toast(r.erreur || 'Ordre impossible.', 'alerte');
    else if (r.refus?.length) this.toast(`Impossible pour : ${r.refus.join(', ')}.`, 'info');
    this.surOrdre?.(r);
  }
  // au stylet ou à la souris : le survol montre déjà le poste
  survol(vue, cx, cy) {
    if (!this.arme || this.geste) return false;
    const i = vue.cellule(cx, cy);
    if (i < 0) { this._vider(vue); return true; }
    this.vues.add(vue);
    if (this._survol?.i === i && this._survol.vue === vue) return true;
    this._survol = { i, vue };
    this._vider(vue);
    const p = vue.pos(i); if (!p) return true;
    this._anneau(vue, p, 0xffc94d);
    return true;
  }

  // ------------------------------------------------------------------ aperçu : demandé au moteur, limité à 4 par seconde
  _demander(rapide) {
    const g = this.geste; if (!g) return;
    const maintenant = performance.now();
    if (rapide && this._dernier && maintenant - this._dernier < 250) { clearTimeout(this._tm); this._tm = setTimeout(() => this._demander(), 260); return; }
    this._dernier = maintenant;
    const n = ++g.n, a = this.arme;
    this.L.action({ type: 'apercu', moyens: a.ids, ordre: a.ordre, cell: g.i0, cell2: g.i2 ?? undefined, dir: g.dir ?? undefined })
      .then(r => { if (this.geste === g && n === g.n) { g.ap = r; this._dessiner(); } }).catch(() => {});
  }

  _vider(vue) { const G = vue.groupe; while (G.children.length) { const o = G.children.pop(); o.geometry?.dispose(); } }
  _anneau(vue, p, col) {
    const m = new THREE.Mesh(new THREE.RingGeometry(vue.R * 0.55, vue.R * 0.75, 6).rotateX(-Math.PI / 2).rotateY(Math.PI / 6), new THREE.MeshBasicMaterial({ color: col, depthTest: false, side: THREE.DoubleSide }));
    m.position.set(p[0], p[1] + 0.6, p[2]); m.renderOrder = 12; vue.groupe.add(m);
  }
  _case(vue, i, mat) {
    const p = vue.pos(i); if (!p) return;
    const m = new THREE.Mesh(new THREE.CylinderGeometry(vue.R * 0.9, vue.R * 0.9, 0.12, 6), mat);
    m.position.set(p[0], p[1] + 0.35, p[2]); m.renderOrder = 10; vue.groupe.add(m);
  }
  _ligne(vue, pts, mat) {
    if (pts.length < 2) return;
    const geo = new THREE.BufferGeometry().setFromPoints(pts.map(p => new THREE.Vector3(p[0], p[1] + 1.2, p[2])));
    const l = new THREE.Line(geo, mat); l.computeLineDistances(); l.renderOrder = 11; vue.groupe.add(l);
  }
  _cone(vue, p, dir, demi) {
    const r = vue.R * 2.4;
    const geo = new THREE.CircleGeometry(r, 28, (dir - demi) * Math.PI / 180, demi * 2 * Math.PI / 180);
    geo.rotateX(Math.PI / 2);                 // le cercle passe dans le plan du sol, l'angle compté vers +z (le sud)
    const m = new THREE.Mesh(geo, MAT.cone); m.position.set(p[0], p[1] + 0.9, p[2]); m.renderOrder = 9; vue.groupe.add(m);
    if (demi < 180) {
      const a1 = (dir - demi) * Math.PI / 180, a2 = (dir + demi) * Math.PI / 180;
      const pts = [[p[0] + Math.cos(a1) * r, p[1], p[2] + Math.sin(a1) * r], p, [p[0] + Math.cos(a2) * r, p[1], p[2] + Math.sin(a2) * r]];
      this._ligne(vue, pts.map(q => [q[0], q[1] - 0.3, q[2]]), MAT.bord);
    }
  }

  // ------------------------------------------------------------------ dessin de l'aperçu
  _dessiner() {
    const g = this.geste; if (!g) return;
    const vue = g.vue, E = this.getE(), o = this.arme.ordre;
    this._vider(vue);
    const p0 = vue.pos(g.i0);
    if (p0) this._anneau(vue, p0, 0xffc94d);
    const ap = g.ap;
    const feux = new Map((E?.feu || []).map(f => [f[0], f[1]]));
    let txt = `${UI.ORDRES[o][0]} ${UI.ORDRES[o][1]}`;
    if (!ap || !ap.moyens?.length) {
      // pas encore de réponse du moteur : au moins le cône à l'endroit visé
      if (p0 && g.dir != null && (o === 'arroser' || o === 'flanc')) this._cone(vue, p0, g.dir, CONE);
      this._etiquette(vue, p0, txt + ' · calcul…');
      return;
    }
    const sol = ap.moyens.filter(e => e.k === 'ccf' || e.k === 'bull');
    const air = ap.moyens.filter(e => e.k !== 'ccf' && e.k !== 'bull');
    const postes = new Map();
    for (const e of sol) {
      // trajet prévu
      if (e.chemin?.length > 1) this._ligne(vue, e.chemin.map(i => vue.pos(i)).filter(Boolean), MAT.trajet);
      if (e.poste == null) continue;
      postes.set(e.poste, (postes.get(e.poste) || 0) + 1);
      const dir = g.dir ?? e.dir;
      const pp = vue.pos(e.poste); if (!pp) continue;
      if (e.k === 'ccf') this._cone(vue, pp, o === 'proteger' ? 0 : dir ?? 0, o === 'proteger' ? 180 : CONE);
      // hexagones atteints / trop chauds (recalculés ici si l'orientation a bougé depuis la réponse)
      const portee = g.dir != null && o !== 'proteger' && e.dir != null && Math.abs(((g.dir - e.dir + 540) % 360) - 180) > 4 ? this._portee(vue, e.poste, g.dir) : e.portee || [];
      for (const i of portee) {
        const I = feux.get(i);
        this._case(vue, i, I != null && I > (o === 'proteger' ? 101 : 80) ? MAT.rouge : MAT.bleu);
      }
    }
    for (const [i, n] of postes) { const pp = vue.pos(i); if (pp) this._anneau(vue, pp, n > 1 ? 0xffb02e : 0xffc94d); }
    for (const e of air) {
      const prod = e.k === 'dash' ? MAT.ret : MAT.bleu;
      for (const i of e.largage || []) this._case(vue, i, prod);
      const pa = vue.posM(e.x, e.y), pe = e.ecopage != null ? vue.pos(e.ecopage) : null, pc = vue.pos(e.largage?.[0] ?? g.i0);
      const pts = [pa, pe, pc].filter(Boolean).map(q => [q[0], q[1] + 6, q[2]]);
      this._ligne(vue, pts, MAT.vol);
      if (pc && (g.dir ?? e.axe) != null) {
        const a = (g.dir ?? e.axe) * Math.PI / 180, l = vue.R * 3.2;
        const m = new THREE.Mesh(new THREE.BoxGeometry(l, 0.15, vue.R * 0.08), MAT.axe);
        m.position.set(pc[0], pc[1] + 1.4, pc[2]); m.rotation.y = -a; m.renderOrder = 12; vue.groupe.add(m);
      }
    }
    // l'étiquette : eau, autonomie, durée du trajet
    const ccf = sol.filter(e => e.k === 'ccf');
    if (ccf.length) {
      const eau = Math.round(ccf.reduce((s, e) => s + (e.eau || 0), 0) / ccf.length);
      const auto = Math.round(ccf.reduce((s, e) => s + (e.autonomie || 0), 0) / ccf.length);
      const tr = Math.max(...ccf.map(e => e.minutes || 0));
      const chauds = ccf.reduce((s, e) => s + (e.chaud?.length || 0), 0);
      txt += ` · 💧 ${eau} % (≈ ${auto} min de lance) · trajet ${tr} min${chauds ? ' · ⚠ zone trop chaude' : ''}`;
    } else if (air.length) {
      const e = air[0];
      txt += ` · 💧 ${e.eau ?? 0} %` + (e.ecopage != null ? ` · écopage : ${this._nomEau(e.ecopage)}` : '');
    }
    if (ap.refus?.length) txt += ` · impossible pour ${ap.refus.length}`;
    this._etiquette(vue, p0, txt);
  }
  _portee(vue, poste, dir) {
    const C = this.C, out = [poste];
    const pp = vue.pos(poste); if (!pp) return out;
    for (const j of vue.voisins(poste)) {
      const pj = vue.pos(j); if (j < 0 || !pj) continue;
      const a = Math.atan2(pj[2] - pp[2], pj[0] - pp[0]) * 180 / Math.PI;
      if (Math.abs(((a - dir + 540) % 360) - 180) <= CONE) out.push(j);
    }
    return out;
  }
  _nomEau(i) {
    const C = this.C, f = C.fuel[i];
    if (f === 'M') return 'la mer';
    const l = C.lacs?.find(x => x.id === C.corps?.[i]);
    return l && l.taille >= 12 ? l.nom : f === 'R' ? 'la rivière' : 'un point d\'eau';
  }
  _etiquette(vue, p, txt) {
    this.eti.textContent = txt; this.eti.hidden = false; this._etiPos = { vue, p };
    this.placer();
  }
  placer() {
    if (this.eti.hidden || !this._etiPos?.p) return;
    const { vue, p } = this._etiPos, e = vue.ecran(p[0], p[1] + 12, p[2]);
    if (!e) return;
    this.eti.style.transform = `translate(${Math.round(e[0] - this.eti.offsetWidth / 2)}px, ${Math.round(e[1] - 40)}px)`;
  }
}

// un éventail (cône des lances) posé au sol, pour montrer en permanence l'orientation des groupes
export function eventail(r, dir, demi, opacite = 0.18) {
  const geo = new THREE.CircleGeometry(r, 20, (dir - demi) * Math.PI / 180, demi * 2 * Math.PI / 180);
  geo.rotateX(Math.PI / 2);
  const m = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color: 0x4aa8ff, transparent: true, opacity: opacite, side: THREE.DoubleSide, depthWrite: false }));
  m.renderOrder = 8;
  return m;
}
