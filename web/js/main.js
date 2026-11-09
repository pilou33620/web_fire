// Feux de forêt — l'interface : le plateau, la boucle de rendu, l'interrogation du moteur, les gestes et le panneau.
import { THREE, Camera, clamp } from './moteur.js';
import { Plateau, Flotte, Convois } from './plateau.js';
import { Intervention } from './intervention.js';
import { choisirLiaison } from './liaison.js';
import * as UI from './ui.js';

const $ = s => document.querySelector(s);
const zone = $('#zone'), toile = $('#scene'), ETI = $('#etiquettes'), bulle = $('#bulle');
const progres = t => { $('#ch-txt').textContent = t; };

// ------------------------------------------------------------------ rendu
const renderer = new THREE.WebGLRenderer({ canvas: toile, antialias: true, powerPreference: 'high-performance' });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.shadowMap.enabled = true; renderer.shadowMap.type = THREE.PCFSoftShadowMap;
const scene = new THREE.Scene(); scene.background = new THREE.Color(0x1b211a);
const cam = new THREE.PerspectiveCamera(30, 1, 5, 8000);
const ciel = new THREE.HemisphereLight(0xeef7ff, 0x7d8466, 1.5); scene.add(ciel);
const soleil = new THREE.DirectionalLight(0xfff0d4, 2.6); soleil.castShadow = true;
Object.assign(soleil.shadow.camera, { left: -260, right: 260, top: 220, bottom: -220, near: 10, far: 1200 });
soleil.shadow.mapSize.set(2048, 2048); soleil.shadow.bias = -0.0006; soleil.shadow.normalBias = 0.4;
scene.add(soleil, soleil.target);
const camera = new Camera(toile, cam, { zmin: 60, zmax: 1500 });

let CV = null;
let L = null, C = null, P = null, F = null, INFOS = null, E = null, inter = null;
let seq = 0, partieId = null, journal = [], nonLus = 0;
const sel = { feu: null, cell: null, moyen: null, groupe: null };
let onglet = 'situation', viser = null, voirNoms = true, voirMissions = true;
let dureeInterp = 1000, tChange = performance.now(), dernierT = -1;

// ------------------------------------------------------------------ lumière du jour et de la nuit
const COUL = { jour: [0xeef7ff, 0x7d8466, 1.5, 0xfff0d4, 2.6, 0x1b211a], aube: [0xffd9b0, 0x6d6450, 1.1, 0xffa060, 1.6, 0x2a2420], nuit: [0x5a6a9a, 0x1a1e2a, 0.45, 0x8fa8ff, 0.35, 0x0c0f16] };
const cA = new THREE.Color(), cB = new THREE.Color();
function lumiere(minute) {
  const h = minute / 60;
  let a, b, k;
  if (h >= 8 && h < 18.5) { a = b = COUL.jour; k = 0; }
  else if (h >= 6.5 && h < 8) { a = COUL.aube; b = COUL.jour; k = (h - 6.5) / 1.5; }
  else if (h >= 18.5 && h < 20.5) { a = COUL.jour; b = COUL.aube; k = (h - 18.5) / 2; }
  else if (h >= 20.5 && h < 21.5) { a = COUL.aube; b = COUL.nuit; k = h - 20.5; }
  else if (h >= 5.5 && h < 6.5) { a = COUL.nuit; b = COUL.aube; k = h - 5.5; }
  else { a = b = COUL.nuit; k = 0; }
  const mix = (i) => cA.set(a[i]).lerp(cB.set(b[i]), k).getHex();
  ciel.color.setHex(mix(0)); ciel.groundColor.setHex(mix(1)); ciel.intensity = a[2] + (b[2] - a[2]) * k;
  soleil.color.setHex(mix(3)); soleil.intensity = a[4] + (b[4] - a[4]) * k;
  scene.background.setHex(mix(5));
  const ang = (h - 6) / 12 * Math.PI;
  soleil.userData.dec = new THREE.Vector3(-Math.cos(ang) * 220, 160 + Math.sin(Math.max(0, ang)) * 200, 140);
  const nuit = h < 6 || h >= 21;
  for (const m of [P?.lueur]) if (m) m.material.opacity = nuit ? 0.6 : 0.3;
  inter?.lumiere(ciel, soleil, scene.background, nuit);
}

// ------------------------------------------------------------------ démarrage
async function demarrer() {
  L = await choisirLiaison(progres);
  $('#t-mode').textContent = { serveur: 'SERVEUR', navigateur: 'NAVIGATEUR', demo: 'DÉMO' }[L.mode];
  progres('Lecture de la carte…');
  INFOS = await L.infos();
  await chargerCarte();
  boutonsTemps();
  $('#chargement').hidden = true;
  interroger();
  requestAnimationFrame(boucle);
}

async function chargerCarte() {
  C = await L.carte();
  if (P) { scene.remove(P.groupe); scene.remove(F.grp); scene.remove(CV.grp); P.groupe.traverse(o => o.geometry?.dispose()); }
  progres('Plantation des pins…');
  await new Promise(r => setTimeout(r, 20));
  P = new Plateau(scene, C);
  F = new Flotte(P, 1.9);
  CV = new Convois(P, 1.6);
  camera.borne = [-P.OX, P.OX, -P.OZ, P.OZ];
  camera.cible.set(0, 0, 0); camera.dist = 1250;
  redim();
  etiquettesSites();
  seq = 0; journal = []; E = null; sel.feu = sel.cell = sel.moyen = sel.groupe = null;
  inter?.quitter(); inter = new Intervention(renderer, C, P, zone);
}

// ------------------------------------------------------------------ interrogation du moteur
const dormir = ms => new Promise(r => setTimeout(r, ms));
async function interroger() {
  for (;;) {
    try {
      const e = await L.etat(seq);
      if (partieId !== null && e.partie !== partieId) { partieId = e.partie; await chargerCarte(); continue; }
      partieId = e.partie;
      appliquer(e);
    } catch (err) { console.warn(err); await dormir(1500); }
    await dormir(E && E.vitesse >= 4 ? 200 : 300);
  }
}

function appliquer(e) {
  const now = performance.now();
  if (e.t !== dernierT) {
    if (dernierT >= 0) dureeInterp = clamp((now - tChange) * 1.05, 120, 2500);
    tChange = now; dernierT = e.t;
  }
  E = e;
  P.majEtat(e);
  F.maj(e.moyens, now);
  CV.maj(e.evac?.convois || [], now);
  P.evac = e.evac;
  inter?.majEtat(e, now);
  for (const v of e.evts) {
    if (v.type === 'largage') { P.largage(v.cells, v.produit, v.kind); inter?.largage(v); }
  }
  for (const j of e.journal) {
    journal.push(j);
    const recent = e.meteo.t - j.t < 15;            // pas d'avalanche de bulles au chargement
    if (recent && (j.k === 'alerte' || j.k === 'sauve' || (j.k === 'cos' && /renfort|Évacuation|Guet|Confinement|abri/.test(j.txt)))) toast(j.txt, j.k);
    if (onglet !== 'journal') nonLus++;
  }
  journal = journal.slice(-200);
  seq = e.seq;
  if (sel.feu == null || !e.feux.find(f => f.id === sel.feu && f.statut !== 'éteint')) {
    const f = e.feux.filter(f => f.statut !== 'éteint').sort((a, b) => b.prio - a.prio)[0];
    sel.feu = f ? f.id : null;
  }
  const fp = e.feux.find(f => f.id === sel.feu);
  P.majCalques(fp?.prev ? fp.prev : prevision(fp));
  barre(e);
  panneauPeriodique();
}
// prévision locale grossière (le serveur n'envoie que le front) : distance au front, pondérée par le vent
function prevision(f) {
  if (!f || !P.lentilles.prevision || !E) return null;
  const out = [], vx = -Math.sin(E.meteo.de * Math.PI / 180), vy = Math.cos(E.meteo.de * Math.PI / 180);
  const front = f.front.length ? f.front : [f.cell0];
  const v = Math.max(0.3, f.vitesse) * 1000 / 60;   // m/min
  for (let i = 0; i < P.n; i++) {
    if (E.etat[i] !== '.' || !C.comb[C.fuel[i]].brule) continue;
    let best = 1e9;
    for (const j of front) {
      const dx = P.X[i] - P.X[j], dz = P.Z[i] - P.Z[j], d = Math.hypot(dx, dz) / P.S;
      if (d > 4000) continue;
      const a = d > 1 ? (dx * vx + dz * vy) / Math.hypot(dx, dz) : 1;
      const t = d / (v * Math.max(0.12, Math.exp(1.4 * (a - 1)) ));
      if (t < best) best = t;
    }
    if (best < 180) out.push([i, best]);
  }
  return out;
}

// ------------------------------------------------------------------ barre du haut
function barre(e) {
  const m = e.meteo;
  $('#t-heure').textContent = `Jour ${m.jour} · ${m.heure}${e.pause ? ' · pause' : ''}`;
  $('#t-scenario').textContent = m.nomScenario;
  $('#p-temp').textContent = `${Math.round(m.temp)} °C`;
  $('#p-hr').textContent = `${m.hr} %`;
  $('#p-vent').textContent = `${UI.rose(m.de)} ${m.vent} km/h`;
  $('#p-fleche').style.transform = `rotate(${m.de + 90}deg)`;
  const r = $('#p-risque'); r.textContent = m.niveau; r.style.background = UI.risqueCouleur(m.niveau);
  $('#p-avions').textContent = m.nuit ? '✈ au sol (nuit)' : m.volCanadair ? '✈ en vol possible' : '✈ au sol (vent)';
  $('#t-ha').textContent = UI.nf(e.stats.ha) + ' ha';
  $('#aiguille').style.transform = `rotate(${m.de + 180}deg)`;
  $('#b-vent').textContent = `${m.vent} km/h`;
  lumiere(m.t % 1440);
  for (const b of document.querySelectorAll('#temps button')) b.classList.toggle('actif', e.pause ? b.dataset.v === 'pause' : +b.dataset.v === e.vitesse);
  const cos = $('#b-cos'); cos.textContent = e.commandement === 'manuel' ? '🧑‍🚒 COS : vous' : '🤖 COS : IA'; cos.setAttribute('aria-pressed', e.commandement === 'manuel');
  const pst = $('#pastille'); pst.hidden = !nonLus; pst.textContent = nonLus > 99 ? '99+' : nonLus;
}
function boutonsTemps() {
  const el = $('#temps');
  el.innerHTML = `<button type="button" data-v="pause" title="Pause">⏸</button>` + INFOS.vitesses.map(v => `<button type="button" data-v="${v}" title="${v} min de jeu par seconde">×${String(v).replace('.', ',')}</button>`).join('');
  el.onclick = async ev => {
    const b = ev.target.closest('button'); if (!b) return;
    if (b.dataset.v === 'pause') await L.action({ type: 'pause', valeur: !E?.pause });
    else await L.action({ type: 'vitesse', valeur: +b.dataset.v });
  };
}

// ------------------------------------------------------------------ panneau
let dernierPanneau = 0;
function panneauPeriodique(force) {
  const now = performance.now();
  if (!force && now - dernierPanneau < 900) return;
  if (!force && document.activeElement && $('#contenu').contains(document.activeElement) && document.activeElement.tagName !== 'BUTTON') return;
  dernierPanneau = now;
  panneau();
}
function panneau() {
  if (!E) return;
  const el = $('#contenu'), haut = el.scrollTop;
  if (onglet === 'situation') el.innerHTML = UI.situation(E, C, sel, L.mode, L.raison);
  else if (onglet === 'moyens') el.innerHTML = UI.moyens(E, sel.moyen);
  else if (onglet === 'journal') { el.innerHTML = UI.journal(journal); nonLus = 0; }
  else if (onglet === 'partie') { if (!el.querySelector('#f-partie')) { el.innerHTML = UI.partie(INFOS, E.reglages, E.doctrine, L.mode); initPartie(); } return; }
  el.scrollTop = haut;
}
$('#onglets').onclick = e => {
  const b = e.target.closest('button'); if (!b) return;
  onglet = b.dataset.o;
  for (const x of document.querySelectorAll('#onglets button')) x.setAttribute('aria-selected', x === b);
  $('#contenu').innerHTML = ''; $('#contenu').scrollTop = 0; panneauPeriodique(true);
};
$('#contenu').addEventListener('click', async e => {
  const o = e.target.closest('[data-ordre],[data-evac],[data-conf],[data-cos],[data-renfort],[data-groupe],[data-tous],[data-deselect]');
  if (o) {
    e.stopPropagation();
    if (o.dataset.ordre) { ordre(o.dataset.ordre, o.closest('.ordres').dataset.ids.split(',').map(Number)); return; }
    if (o.dataset.evac) { const r = await L.action({ type: 'evacuer', site: +o.dataset.evac }); if (!r.ok) toast(r.erreur, 'alerte'); return; }
    if (o.dataset.conf) { await L.action({ type: 'confiner', site: +o.dataset.conf }); return; }
    if (o.dataset.cos) { await L.action({ type: 'commandement', valeur: o.dataset.cos }); return; }
    if (o.dataset.renfort !== undefined) { const r = await L.action({ type: 'renfort' }); toast(r.ok ? 'Colonne de renfort demandée : arrivée dans 45 min.' : r.erreur, 'cos'); return; }
    if (o.dataset.groupe) { sel.groupe = E.moyens.filter(d => d.g === o.dataset.groupe).map(d => d.id); sel.moyen = null; montrerSituation(); return; }
    if (o.dataset.tous) { sel.groupe = E.moyens.filter(d => d.k === o.dataset.tous).map(d => d.id); sel.moyen = null; montrerSituation(); return; }
    if (o.dataset.deselect !== undefined) { sel.groupe = null; panneau(); return; }
  }
  const t = e.target.closest('[data-inter],[data-centrer],[data-feu],[data-moyen]'); if (!t) return;
  if (t.dataset.inter) { entrerIntervention(+t.dataset.inter); return; }
  if (t.dataset.centrer) { centrerFeu(+t.dataset.centrer); return; }
  if (t.dataset.feu) { sel.feu = +t.dataset.feu; centrerFeu(sel.feu); panneau(); return; }
  if (t.dataset.moyen) { sel.moyen = +t.dataset.moyen; sel.cell = null; sel.groupe = null; const o = F.u.get(sel.moyen); if (o?.pos) camera.aller(o.pos[0], o.pos[2], Math.min(camera.dist, 320)); onglet = 'situation'; document.querySelector('[data-o=situation]').click(); }
});
function montrerSituation() { if (onglet !== 'situation') document.querySelector('[data-o=situation]').click(); else panneau(); }
// ------------------------------------------------------------------ ordres du joueur
function ordre(o, ids) {
  const [ic, nom, cibles] = UI.ORDRES[o];
  if (!cibles) { L.action({ type: 'ordre', moyens: ids, ordre: o }).then(r => { if (!r.ok) toast(r.erreur, 'alerte'); }); return; }
  viser = { type: 'ordre', ordre: o, ids, cells: [], n: cibles };
  visee(`${ic} ${nom} : touchez ${cibles === 2 ? 'le point de départ du pare-feu' : o === 'defendre' ? 'le site à défendre' : 'l\'endroit visé'} sur la carte.`);
}
function visee(txt) {
  const v = $('#visee');
  if (!txt) { v.hidden = true; toile.classList.remove('viser'); return; }
  v.hidden = false; $('#visee-txt').textContent = txt; toile.classList.add('viser');
  if (inter?.actif) quitterIntervention();
}
$('#visee-annuler').onclick = () => { viser = null; visee(null); $('#b-allumer').setAttribute('aria-pressed', 'false'); };
$('#b-cos').onclick = () => L.action({ type: 'commandement', valeur: E?.commandement === 'manuel' ? 'auto' : 'manuel' });
function initPartie() {
  const f = $('#f-partie');
  f.querySelectorAll('input[type=range]').forEach(r => r.oninput = () => { r.nextElementSibling.textContent = r.value; });
  f.onsubmit = async ev => {
    ev.preventDefault();
    const d = new FormData(f), moyens = {};
    f.querySelectorAll('[data-m]').forEach(r => { moyens[r.dataset.m] = +r.value; });
    $('#chargement').hidden = false; progres('Nouvelle carte…');
    await L.action({ type: 'nouvelle', reglages: { theme: d.get('theme'), scenario: d.get('scenario'), graine: +d.get('graine'), moyens, doctrine: d.get('doctrine') } });
    partieId = -1;
    setTimeout(() => { $('#chargement').hidden = true; }, 400);
  };
  $('#doctrine').onchange = e => L.action({ type: 'doctrine', valeur: e.target.value });
  const vd = $('#vent-de'), vf = $('#vent-f');
  vd.value = Math.round((E?.meteo.de ?? 90) / 15) * 15; vf.value = E?.meteo.vent ?? 30;
  const maj = () => { $('#vent-de-o').textContent = UI.rose(+vd.value); $('#vent-f-o').textContent = vf.value; }; maj();
  vd.oninput = vf.oninput = maj;
  $('#b-vent').onclick = () => L.action({ type: 'vent', de: +vd.value, force: +vf.value });
  $('#b-orage').onclick = () => L.action({ type: 'orage' });
  $('#b-avancer').onclick = () => L.action({ type: 'avancer', minutes: 30 });
}

// ------------------------------------------------------------------ toasts
function toast(txt, k) {
  const el = document.createElement('div'); el.className = 'toast ' + k; el.textContent = txt;
  const box = $('#toasts'); box.prepend(el);
  while (box.children.length > 3) box.lastChild.remove();
  setTimeout(() => el.remove(), 6000);
}

// ------------------------------------------------------------------ étiquettes
const SYMB = { ville: '🏘', village: '⌂', camping: '⛺', usine: '🏭', reserve: '🌳', patrimoine: '⛪' };
const COUL_SITE = { sauf: '#fff7e6', 'menacé': '#ffc94d', 'touché': '#ff8a5a', 'détruit': '#a59d92' };
let etisSites = [], etisFeux = new Map(), etisMoyen = null;
function etiquettesSites() {
  ETI.innerHTML = ''; etisSites = []; etisFeux = new Map();
  for (const s of C.sites) {
    const el = document.createElement('div');
    el.className = 'ban' + (s.type === 'ville' ? '' : ' petit');
    el.innerHTML = `<span class="pop">${SYMB[s.type]}</span><span class="nom">${UI.esc(s.nom)}</span>`;
    el.style.setProperty('--c', '#fff7e6');
    ETI.appendChild(el); etisSites.push({ s, el, etat: 'sauf' });
  }
  etisMoyen = document.createElement('div'); etisMoyen.className = 'eti moyen'; etisMoyen.hidden = true; ETI.appendChild(etisMoyen);
}
const V3 = new THREE.Vector3();
function ecran(x, y, z) { V3.set(x, y, z).project(cam); if (V3.z > 1 || Math.abs(V3.x) > 1.15 || Math.abs(V3.y) > 1.15) return null; return [(V3.x + 1) / 2 * camera.W, (1 - V3.y) / 2 * camera.H]; }
function majEtiquettes() {
  if (!P) return;
  const loin = camera.dist;
  for (const o of etisSites) {
    const s = o.s, st = E?.sites[s.id]?.etat || 'sauf';
    if (st !== o.etat) { o.etat = st; o.el.style.setProperty('--c', COUL_SITE[st]); }
    const seuil = s.type === 'ville' ? 9e9 : s.type === 'village' ? 1300 : 750;
    if (!voirNoms || loin > seuil) { o.el.hidden = true; continue; }
    const i = s.centre, p = ecran(P.X[i], P.top[i] + 9, P.Z[i]);
    if (!p) { o.el.hidden = true; continue; }
    o.el.hidden = false; o.el.style.transform = `translate(${Math.round(p[0] - 15)}px, ${Math.round(p[1] - 15)}px)`;
  }
  // feux
  const vus = new Set();
  for (const f of E?.feux || []) {
    if (f.statut === 'éteint') continue;
    vus.add(f.id);
    let el = etisFeux.get(f.id);
    if (!el) { el = document.createElement('div'); el.className = 'ban feu'; ETI.appendChild(el); etisFeux.set(f.id, el); el.style.pointerEvents = 'auto'; el.style.cursor = 'pointer'; el.onclick = () => { sel.feu = f.id; onglet = 'situation'; document.querySelector('[data-o=situation]').click(); }; }
    const nd = !f.detecte;
    el.classList.toggle('nd', nd);
    const txt = `<span class="pop">🔥</span><span class="nom">${UI.esc(nd ? 'Feu non détecté' : f.nom)}<small>${UI.nf(f.ha)} ha · ${UI.statut(f.statut)[1]}</small></span>`;
    if (el._t !== txt) { el.innerHTML = txt; el._t = txt; }
    const i = f.tete ?? f.cell0, p = ecran(P.X[i], P.top[i] + 16, P.Z[i]);
    if (!p || !voirNoms) { el.hidden = true; continue; }
    el.hidden = false; el.style.transform = `translate(${Math.round(p[0] - 15)}px, ${Math.round(p[1] - 15)}px)`;
  }
  for (const [id, el] of etisFeux) if (!vus.has(id)) { el.remove(); etisFeux.delete(id); }
  // le moyen choisi
  const o = sel.moyen != null ? F.u.get(sel.moyen) : null;
  if (o?.pos) {
    const p = ecran(o.pos[0], o.pos[1] + 4, o.pos[2]);
    if (p) { etisMoyen.hidden = false; etisMoyen.textContent = `${UI.ICONE[o.d.k]} ${o.d.n} · ${UI.etatMoyen(o.d)}`; etisMoyen.style.transform = `translate(${Math.round(p[0] - etisMoyen.offsetWidth / 2)}px, ${Math.round(p[1] - 28)}px)`; }
    else etisMoyen.hidden = true;
  } else etisMoyen.hidden = true;
}
function majEchelle() {
  const mPx = camera.upp() / P.S;
  const m = [100, 200, 500, 1000, 2000, 5000].find(k => k / mPx >= 70) || 5000;
  $('#ech-txt').textContent = m >= 1000 ? m / 1000 + ' km' : m + ' m'; $('#ech-regle').style.width = Math.round(m / mPx) + 'px';
}

// ------------------------------------------------------------------ gestes : survol, choix, allumer
const ray = new THREE.Raycaster(), PL = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), PT = new THREE.Vector3(), NDC = new THREE.Vector2();
function celluleSous(cx, cy) {
  const r = zone.getBoundingClientRect(); NDC.set((cx - r.left) / r.width * 2 - 1, -((cy - r.top) / r.height) * 2 + 1); ray.setFromCamera(NDC, cam);
  let i = -1, y = 0;
  for (let k = 0; k < 3; k++) { PL.constant = -y; if (!ray.ray.intersectPlane(PL, PT)) return -1; i = P.cellule(PT.x, PT.z); if (i < 0) return -1; y = Math.max(P.top[i], -1); }
  return i;
}
function moyenSous(cx, cy) {
  const r = zone.getBoundingClientRect(); let best = null, bd = 26;
  for (const o of F.u.values()) { if (!o.pos) continue; const p = ecran(...o.pos); if (!p) continue; const d = Math.hypot(p[0] + r.left - cx, p[1] + r.top - cy); if (d < bd) { bd = d; best = o; } }
  return best;
}
camera.onHover = (cx, cy) => {
  if (inter?.actif) return;
  if (cx == null || !P) { bulle.hidden = true; P && (P.survol.visible = false); return; }
  const o = moyenSous(cx, cy);
  if (o) { bulle.hidden = false; bulle.innerHTML = `<b>${UI.ICONE[o.d.k]} ${UI.esc(o.d.n)}</b><br>${UI.esc(UI.etatMoyen(o.d))}${o.d.m ? ' · ' + UI.esc(o.d.m) : ''}`; }
  const i = celluleSous(cx, cy);
  if (i < 0) { bulle.hidden = !o; P.survol.visible = false; return; }
  P.survol.visible = true; P.survol.position.set(P.X[i], P.top[i] + 0.4, P.Z[i]);
  if (!o) {
    const s = C.sites.find(s => s.cells.includes(i)), e = E?.etat[i];
    bulle.hidden = false;
    const v = C.voies?.[C.route[i]];
    bulle.innerHTML = `<b>${UI.esc(s ? s.nom : C.comb[C.fuel[i]].nom)}</b><br>${e === 'f' ? '🔥 en feu' : e === 'b' ? 'brûlé' : e === 'e' || e === 'n' ? 'éteint' : C.comb[C.fuel[i]].nom}${v ? `<br>${C.route[i] === '1' ? '🟫' : '🛣'} ${UI.esc(v.nom)} : ＋ ${UI.esc(v.bonus[0].toLowerCase())} · － ${UI.esc(v.malus[0].toLowerCase())}` : ''}`;
  }
  bulle.style.left = cx + 14 + 'px'; bulle.style.top = cy + 14 + 'px';
  camera.sale = true;
};
camera.onTap = async (cx, cy) => {
  if (inter?.actif) { inter.tap(cx, cy); return; }
  const i = celluleSous(cx, cy);
  if (viser?.type === 'allumer') {
    viser = null; $('#b-allumer').setAttribute('aria-pressed', 'false'); visee(null);
    if (i >= 0) { const r = await L.action({ type: 'allumer', cell: i }); if (!r.ok) toast(r.erreur || 'Impossible ici', 'info'); }
    return;
  }
  if (viser?.type === 'ordre') {
    if (i < 0) return;
    viser.cells.push(i);
    if (viser.cells.length < viser.n) { visee('🚧 Pare-feu : touchez maintenant le point d\'arrivée.'); P.choix.visible = true; P.choix.position.set(P.X[i], P.top[i] + 0.45, P.Z[i]); return; }
    const v = viser; viser = null; visee(null);
    const r = await L.action({ type: 'ordre', moyens: v.ids, ordre: v.ordre, cell: v.cells[0], cell2: v.cells[1] });
    if (!r.ok) toast(r.erreur, 'alerte'); else if (r.refus?.length) toast(`Ordre impossible pour : ${r.refus.join(', ')}.`, 'info');
    P.choix.visible = true; P.choix.position.set(P.X[i], P.top[i] + 0.45, P.Z[i]);
    return;
  }
  const o = moyenSous(cx, cy);
  if (o) { sel.moyen = o.d.id; sel.cell = null; sel.groupe = null; }
  else if (i >= 0) {
    sel.moyen = null; sel.cell = i;
    P.choix.visible = true; P.choix.position.set(P.X[i], P.top[i] + 0.45, P.Z[i]);
    const f = E?.feux.find(f => f.front.includes(i) || f.cell0 === i);
    if (f) sel.feu = f.id;
  }
  if (onglet !== 'situation') document.querySelector('[data-o=situation]').click(); else panneau();
};
$('#b-allumer').onclick = () => {
  if (viser?.type === 'allumer') { viser = null; visee(null); $('#b-allumer').setAttribute('aria-pressed', 'false'); return; }
  viser = { type: 'allumer' }; $('#b-allumer').setAttribute('aria-pressed', 'true'); visee('🔥 Touchez un hexagone de végétation pour y allumer un feu.');
};
$('#b-inter').onclick = () => { if (inter?.actif) { quitterIntervention(); return; } const f = sel.feu ?? E?.feux.find(f => f.statut !== 'éteint')?.id; if (f == null) { toast('Aucun feu en cours : allumez-en un, ou attendez un départ.', 'info'); return; } entrerIntervention(f); };
$('#lentilles').onclick = e => {
  const b = e.target.closest('button'); if (!b) return;
  const on = b.getAttribute('aria-pressed') !== 'true'; b.setAttribute('aria-pressed', on);
  const l = b.dataset.l;
  if (l === 'noms') voirNoms = on; else if (l === 'moyens') voirMissions = on; else if (l === 'evac') { P.lentilles.evac = on; P.majCalques(prevision(E?.feux.find(f => f.id === sel.feu))); } else { P.lentilles[l] = on; const fp = E?.feux.find(f => f.id === sel.feu); P.majCalques(prevision(fp)); }
  camera.sale = true;
};
function centrerFeu(id) {
  const f = E?.feux.find(f => f.id === id); if (!f) return;
  const i = f.tete ?? f.cell0; camera.aller(P.X[i], P.Z[i], Math.min(camera.dist, 420));
}

// ------------------------------------------------------------------ intervention (vue rapprochée)
function entrerIntervention(id) {
  const f = E?.feux.find(f => f.id === id); if (!f) return;
  sel.feu = id;
  inter.entrer(f, E);
  $('#b-inter').textContent = '◂ Retour à la carte';
  ETI.hidden = true; $('#lentilles').hidden = true; $('#b-allumer').hidden = true; bulle.hidden = true;
}
function quitterIntervention() {
  inter.quitter();
  $('#b-inter').textContent = '🔍 Intervention';
  ETI.hidden = false; $('#lentilles').hidden = false; $('#b-allumer').hidden = false;
  camera.sale = true;
}
window.addEventListener('keydown', e => { if (e.key === 'Escape' && inter?.actif) quitterIntervention(); });

// ------------------------------------------------------------------ boucle
function redim() {
  const r = zone.getBoundingClientRect(); const W = Math.max(1, r.width), H = Math.max(1, r.height);
  renderer.setSize(W, H, false); camera.taille(W, H); inter?.taille(W, H);
}
window.addEventListener('resize', redim);
let t0 = performance.now(), traitsT = 0;
function boucle() {
  const now = performance.now(), dt = Math.min(0.1, (now - t0) / 1000); t0 = now;
  const t = now / 1000;
  if (inter?.actif) {
    inter.anime(dt, t, now, dureeInterp);
    renderer.render(inter.scene, inter.cam);
  } else if (P) {
    camera.anime(dt);
    P.reconstruire(2);
    const de = (E?.meteo.de ?? 270) * Math.PI / 180;
    P.anime(dt, t, { x: -Math.sin(de), z: Math.cos(de), v: E?.meteo.vent ?? 10 });
    F.anime(dt, t, now, dureeInterp, P, true);
    CV.anime(dt, t, now, dureeInterp);
    if (now - traitsT > 400) { F.traits(voirMissions); traitsT = now; }
    const dec = soleil.userData.dec || new THREE.Vector3(-170, 300, 160);
    soleil.position.copy(camera.cible).add(dec); soleil.target.position.copy(camera.cible);
    const r = clamp(camera.dist * 0.45, 120, 520); Object.assign(soleil.shadow.camera, { left: -r, right: r, top: r, bottom: -r }); soleil.shadow.camera.updateProjectionMatrix();
    renderer.render(scene, cam);
    majEtiquettes(); majEchelle();
  }
  requestAnimationFrame(boucle);
}

demarrer().catch(err => { console.error(err); progres('Erreur : ' + err.message); });
window.__feux = { get E() { return E; }, get C() { return C; }, get P() { return P; }, camera, get L() { return L; }, entrerIntervention, quitterIntervention, get inter() { return inter; },
  choisirCellule(i) { sel.cell = i; sel.moyen = null; sel.groupe = null; montrerSituation(); } };
