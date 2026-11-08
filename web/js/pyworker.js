// Le moteur Python dans le navigateur (Pyodide), dans un Web Worker : mêmes réponses que server.py.
/* global loadPyodide, importScripts */
let py = null, sim = null, base = '';
let vitesse = 1, pause = false, reste = 0, dernier = 0;

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

async function init(arg) {
  base = arg.base;
  postMessage({ type: 'progres', txt: 'Chargement de Python (Pyodide)…' });
  let url = base + 'vendor/pyodide/';
  try { const r = await fetch(url + 'pyodide.js', { method: 'HEAD' }); if (!r.ok) throw 0; } catch (e) { url = 'https://cdn.jsdelivr.net/pyodide/v0.27.7/full/'; }
  importScripts(url + 'pyodide.js');
  py = await loadPyodide({ indexURL: url });
  postMessage({ type: 'progres', txt: 'Installation du moteur…' });
  const man = await (await fetch(base + 'py/manifest.json')).json();
  py.FS.mkdirTree('/home/pyodide/jeu');
  for (const e of man.fichiers) {
    const f = typeof e === 'string' ? e : e.f, u = typeof e === 'string' ? e : e.u;
    const txt = await (await fetch(base + 'py/' + u)).text();
    py.FS.writeFile('/home/pyodide/' + f, txt);
  }
  postMessage({ type: 'progres', txt: 'Génération de la carte…' });
  py.runPython(GLUE);
  sim = py.globals.get('sim');
  dernier = performance.now();
  setInterval(boucle, 100);
  return true;
}

function boucle() {
  const now = performance.now(), dt = (now - dernier) / 1000; dernier = now;
  if (pause || !sim) return;
  reste += dt * vitesse;
  const n = Math.min(12, Math.floor(reste));
  if (n > 0) { reste -= n; try { sim.pas(n); } catch (e) { pause = true; console.error(e); } }
  if (reste > 5) reste = 0;
}

onmessage = async e => {
  const { id, type, arg } = e.data;
  try {
    let res;
    if (type === 'init') res = await init(arg);
    else if (type === 'carte') res = JSON.parse(sim.carte());
    else if (type === 'infos') res = JSON.parse(sim.infos());
    else if (type === 'etat') res = JSON.parse(sim.etat(arg || 0, vitesse, pause));
    else if (type === 'action') {
      if (arg.type === 'vitesse') { vitesse = +arg.valeur || 1; pause = false; res = { ok: true }; }
      else if (arg.type === 'pause') { pause = arg.valeur === undefined ? !pause : !!arg.valeur; res = { ok: true }; }
      else res = JSON.parse(sim.action(JSON.stringify(arg)));
    }
    postMessage({ id, res });
  } catch (err) { postMessage({ id, erreur: String(err && err.message || err) }); }
};
