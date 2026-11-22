// La liaison avec le moteur (Rust), de trois façons qui offrent la même interface :
// 1. par le serveur (bin/web_fire, sur le Pi ou le PC) ;
// 2. sans serveur, par WebAssembly : le même moteur tourne dans la page (web/wasm/) ;
// 3. si le moteur ne peut pas se charger (web/wasm/ absent, navigateur trop ancien), une démo enregistrée du vrai
//    moteur est rejouée (web/demo/rejeu.json, fabriqué par « cargo run -p feu --bin enregistrer »).

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

const BASE = new URL('../', import.meta.url).href;

// le moteur compilé en WebAssembly (web/wasm/, mis là par scripts/binaires.sh) : même interface que l'ancienne liaison Pyodide
export class LiaisonWasm {
  constructor() { this.mode = 'navigateur'; this.vitesse = 1; this.pause = false; this.reste = 0; }
  async init(progres) {
    progres('Chargement du moteur (WebAssembly)…');
    let mod;
    try { mod = await import(BASE + 'wasm/feu_wasm.js'); await mod.default(); }
    catch (e) { throw new Error('moteur WebAssembly absent (dossier web/wasm/ : lancez scripts/binaires.sh)'); }
    progres('Génération de la carte…');
    await new Promise(r => setTimeout(r, 30));
    this.sim = new mod.Sim();
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
    return { ok: false, erreur: "Démo enregistrée : ici on ne peut que regarder. Lancez demarrer_WEB_FIRE (ou ouvrez la page avec web/wasm/) pour jouer." };
  }
}

// le serveur répond-il ? sinon WebAssembly ; sinon la démo enregistrée
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
    try { const L = new LiaisonWasm(); await L.init(progres); return L; } catch (e) { raison = e.message || String(e); console.warn('WebAssembly indisponible :', e); }
  }
  const L = new LiaisonRejeu();
  await L.init(progres);
  L.raison = raison;
  return L;
}
