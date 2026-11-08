// La liaison avec le moteur Python : par le serveur (server.py, sur le Pi ou le PC), ou, sans serveur, par Pyodide
// (le même code Python tourne dans le navigateur, dans un Web Worker). Les deux offrent la même interface.

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

export class LiaisonPyodide {
  constructor(surProgres) {
    this.mode = 'navigateur'; this.n = 0; this.attente = new Map();
    this.w = new Worker(new URL('./pyworker.js', import.meta.url));
    this.w.onmessage = e => {
      const m = e.data;
      if (m.type === 'progres') { surProgres?.(m.txt); return; }
      const cb = this.attente.get(m.id); if (!cb) return;
      this.attente.delete(m.id);
      m.erreur ? cb[1](new Error(m.erreur)) : cb[0](m.res);
    };
    this.pret = this.appel('init', { base: new URL('../', import.meta.url).href });
  }
  appel(type, arg) { return new Promise((ok, ko) => { const id = ++this.n; this.attente.set(id, [ok, ko]); this.w.postMessage({ id, type, arg }); }); }
  async carte() { await this.pret; return this.appel('carte'); }
  async etat(depuis) { await this.pret; return this.appel('etat', depuis); }
  async infos() { await this.pret; return this.appel('infos'); }
  async action(a) { await this.pret; return this.appel('action', a); }
}

// le serveur répond-il ? sinon, Pyodide
export async function choisirLiaison(surProgres) {
  if (!location.hash.includes('navigateur')) {
    try {
      const ctl = new AbortController(); const t = setTimeout(() => ctl.abort(), 2500);
      const r = await fetch('api/infos', { cache: 'no-store', signal: ctl.signal }); clearTimeout(t);
      if (r.ok && (r.headers.get('content-type') || '').includes('json')) return new LiaisonHTTP();
    } catch (e) { /* pas de serveur */ }
  }
  return new LiaisonPyodide(surProgres);
}
