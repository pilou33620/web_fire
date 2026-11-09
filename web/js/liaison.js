// La liaison avec le moteur Python, de trois façons qui offrent la même interface :
// 1. par le serveur (server.py, sur le Pi ou le PC) ;
// 2. sans serveur, par Pyodide : le même code Python tourne dans la page ;
// 3. si Python ne peut pas se charger (page publiée trop restreinte, pas de réseau), une démo enregistrée du vrai
//    moteur est rejouée (outils/enregistrer.py).

export class LiaisonHTTP {
  constructor() { this.mode = 'serveur'; }
  async carte() { const r = await fetch('api/carte', { cache: 'no-store' }); if (!r.ok) throw new Error('carte'); return r.json(); }
  async etat(depuis) { const r = await fetch('api/etat?depuis=' + depuis, { cache: 'no-store' }); if (!r.ok) throw new Error('état'); return r.json(); }
  async infos() { const r = await fetch('api/infos', { cache: 'no-store' }); return r.json(); }
  async action(a) {
    const r = await fetch('api/action', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(a) });
    return r.json();
  }
}

const GLUE = `
import json, sys, time
sys.path.insert(0, '/home/pyodide')
from jeu.partie import Partie, MOYENS_DEFAUT, LIMITES
from jeu.meteo import SCENARIOS

class Sim:
    def __init__(self):
        self.reglages = dict(graine=7, theme='landes', scenario='canicule', moyens=dict(MOYENS_DEFAUT), doctrine='equilibree')
        self.nouvelle({})
    def nouvelle(self, r):
        r = {**self.reglages, **(r or {})}
        r['graine'] = int(r.get('graine') or 1)
        self.partie = Partie(r['graine'], r['theme'], r['scenario'], r.get('moyens'), r.get('doctrine', 'equilibree'))
        self.reglages = r
        self.id = int(time.time() * 1000) % 10 ** 9
    def carte(self):
        return json.dumps(self.partie.export_carte(), ensure_ascii=False)
    def etat(self, depuis, vitesse, pause):
        e = self.partie.export_etat(int(depuis))
        e.update(vitesse=vitesse, pause=pause, partie=self.id, reglages=self.reglages)
        return json.dumps(e, ensure_ascii=False, separators=(',', ':'))
    def pas(self, n):
        for _ in range(int(n)):
            self.partie.pas()
    def action(self, txt):
        a = json.loads(txt)
        if a.get('type') == 'nouvelle':
            self.nouvelle(a.get('reglages'))
            return json.dumps(dict(ok=True, partie=self.id))
        if a.get('type') == 'avancer':
            self.pas(max(1, min(600, int(a.get('minutes', 10)))))
            return json.dumps(dict(ok=True))
        return json.dumps(self.partie.action(a), ensure_ascii=False)
    def infos(self):
        return json.dumps(dict(scenarios={k: v['nom'] for k, v in SCENARIOS.items()},
                               themes={'landes': 'Landes de Gascogne', 'mediterranee': 'Massif méditerranéen'},
                               limites=LIMITES, defaut=MOYENS_DEFAUT, vitesses=[0.25, 0.5, 1, 2, 4, 8, 16]), ensure_ascii=False)

sim = Sim()
`;

const BASE = new URL('../', import.meta.url).href;
async function existe(url) { try { const r = await fetch(url, { method: 'HEAD', cache: 'no-store' }); return r.ok; } catch (e) { return false; } }
function chargerScript(src) {
  return new Promise((ok, ko) => { const s = document.createElement('script'); s.src = src; s.onload = ok; s.onerror = () => ko(new Error('script refusé : ' + src)); document.head.appendChild(s); });
}
const delai = (p, ms, quoi) => Promise.race([p, new Promise((_, ko) => setTimeout(() => ko(new Error(quoi + ' : délai dépassé')), ms))]);

export class LiaisonPyodide {
  constructor() { this.mode = 'navigateur'; this.vitesse = 1; this.pause = false; this.reste = 0; }
  async init(progres) {
    let url = BASE + 'vendor/pyodide/';
    if (!await existe(url + 'pyodide.js')) url = 'https://cdn.jsdelivr.net/pyodide/v0.27.7/full/';
    progres('Téléchargement de Python (environ 10 Mo)…');
    if (!window.loadPyodide) await delai(chargerScript(url + 'pyodide.js'), 20000, 'pyodide.js');
    const py = await delai(window.loadPyodide({ indexURL: url }), 45000, 'Pyodide');
    progres('Installation du moteur…');
    const man = await (await fetch(BASE + 'py/manifest.json')).json();
    py.FS.mkdirTree('/home/pyodide/jeu');
    for (const e of man.fichiers) {
      const f = typeof e === 'string' ? e : e.f, u = typeof e === 'string' ? e : e.u;
      py.FS.writeFile('/home/pyodide/' + f, await (await fetch(BASE + 'py/' + u)).text());
    }
    progres('Génération de la carte…');
    await new Promise(r => setTimeout(r, 30));
    py.runPython(GLUE);
    this.sim = py.globals.get('sim');
    this.dernier = performance.now();
    setInterval(() => this.boucle(), 100);
  }
  boucle() {
    const now = performance.now(), dt = (now - this.dernier) / 1000; this.dernier = now;
    if (this.pause) return;
    this.reste += dt * this.vitesse;
    const n = Math.min(6, Math.floor(this.reste));      // la page reste fluide : 6 minutes de jeu au plus par passage
    if (n > 0) { this.reste -= n; try { this.sim.pas(n); } catch (e) { this.pause = true; console.error(e); } }
    if (this.reste > 4) this.reste = 0;
  }
  async carte() { return JSON.parse(this.sim.carte()); }
  async infos() { return JSON.parse(this.sim.infos()); }
  async etat(depuis) { return JSON.parse(this.sim.etat(depuis || 0, this.vitesse, this.pause)); }
  async action(a) {
    if (a.type === 'vitesse') { this.vitesse = +a.valeur || 1; this.pause = false; return { ok: true }; }
    if (a.type === 'pause') { this.pause = a.valeur === undefined ? !this.pause : !!a.valeur; return { ok: true }; }
    return JSON.parse(this.sim.action(JSON.stringify(a)));
  }
}

// la démo enregistrée : des images de l'état du moteur, toutes les quelques minutes de jeu, rejouées
export class LiaisonRejeu {
  constructor() { this.mode = 'demo'; this.vitesse = 1; this.pause = false; this.tJeu = 0; }
  async init(progres) {
    progres('Chargement de la démo enregistrée…');
    const r = await fetch(BASE + 'demo/rejeu.json');
    if (!r.ok) throw new Error('pas de démo');
    this.D = await r.json();
    this.dernier = performance.now();
  }
  _avancer() {
    const now = performance.now(), dt = (now - this.dernier) / 1000; this.dernier = now;
    if (!this.pause) this.tJeu = Math.min(this.tJeu + dt * this.vitesse, (this.D.images.length - 1) * this.D.pas);
  }
  async carte() { return this.D.carte; }
  async infos() { return this.D.infos; }
  async etat(depuis) {
    this._avancer();
    const k = Math.floor(this.tJeu / this.D.pas), e = { ...this.D.images[k] };
    e.journal = this.D.journal.filter(j => j.id > depuis && j.id <= e.seq);
    e.evts = this.D.evts.filter(j => j.id > depuis && j.id <= e.seq);
    e.vitesse = this.vitesse; e.pause = this.pause || k >= this.D.images.length - 1; e.partie = 1; e.reglages = this.D.reglages;
    return e;
  }
  async action(a) {
    if (a.type === 'vitesse') { this.vitesse = +a.valeur || 1; this.pause = false; return { ok: true }; }
    if (a.type === 'pause') { this.pause = a.valeur === undefined ? !this.pause : !!a.valeur; return { ok: true }; }
    return { ok: false, erreur: "Démo enregistrée : ici on ne peut que regarder. Lancez server.py (ou la version dist/) pour jouer." };
  }
}

// le serveur répond-il ? sinon Pyodide ; sinon la démo enregistrée
export async function choisirLiaison(progres) {
  if (!location.hash.includes('navigateur') && !location.hash.includes('demo')) {
    try {
      const ctl = new AbortController(); const t = setTimeout(() => ctl.abort(), 2500);
      const r = await fetch('api/infos', { cache: 'no-store', signal: ctl.signal }); clearTimeout(t);
      if (r.ok && (r.headers.get('content-type') || '').includes('json')) return new LiaisonHTTP();
    } catch (e) { /* pas de serveur */ }
  }
  let raison = '';
  if (!location.hash.includes('demo')) {
    try { const L = new LiaisonPyodide(); await L.init(progres); return L; } catch (e) { raison = e.message || String(e); console.warn('Pyodide indisponible :', e); }
  }
  const L = new LiaisonRejeu();
  await L.init(progres);
  L.raison = raison;
  return L;
}
