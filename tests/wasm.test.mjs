// Le moteur WebAssembly (web/wasm/, fait par wasm-bindgen --target web) se charge et joue depuis JavaScript,
// par LiaisonWasm elle-même, comme dans la page. Sans navigateur : node tests/wasm.test.mjs → « OK » ou la première erreur.
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const WASM = new URL('../web/wasm/', import.meta.url);

try {
  // Node ne sait pas fetch() une URL file: : on initialise le module d'avance avec les octets du .wasm ;
  // LiaisonWasm importe la même URL, donc le même module, et son mod.default() le trouve déjà prêt.
  const mod = await import(new URL('feu_wasm.js', WASM).href);
  await mod.default({ module_or_path: readFileSync(new URL('feu_wasm_bg.wasm', WASM)) });
  const { LiaisonWasm } = await import('../web/js/liaison.js');
  const L = new LiaisonWasm();
  await L.init(() => {});

  const infos = await L.infos();
  assert.ok(Array.isArray(infos.vitesses) && infos.vitesses.length > 0, 'infos.vitesses');
  assert.ok(Object.keys(infos.scenarios).length > 0, 'infos.scenarios');

  const c = await L.carte();
  assert.ok(c.cols > 0 && c.rows > 0, 'carte : dimensions');
  assert.equal(c.alt.length, c.cols * c.rows, 'carte : une altitude par case');
  assert.equal(typeof c.fuel, 'string', 'carte.fuel');

  let e = await L.etat(0);
  assert.equal(e.t, 0, 'état initial : t');
  assert.ok(e.moyens.length > 0, 'état : moyens');
  assert.equal(typeof e.partie, 'number', 'état : partie');

  // vitesse et pause restent dans la page
  assert.equal((await L.action({ type: 'vitesse', valeur: 4 })).ok, true, 'action vitesse');
  e = await L.etat(0);
  assert.ok(e.vitesse === 4 && e.pause === false, 'état après vitesse');
  await L.action({ type: 'pause', valeur: true });
  assert.equal((await L.etat(0)).pause, true, 'état après pause');

  // la boucle de la page : 2 s à vitesse 1 → 2 minutes de jeu
  Object.assign(L, { pause: false, vitesse: 1, reste: 0, dernier: performance.now() - 2000 });
  L.boucle();
  L.pause = true;
  assert.equal((await L.etat(0)).t, 2, 'boucle : 2 minutes jouées');

  assert.equal((await L.action({ type: 'avancer', minutes: 30 })).ok, true, 'action avancer');
  assert.equal((await L.etat(0)).t, 32, 'avancer : 30 minutes de plus');

  // un départ de feu sur la première case qui brûle (le scénario allume aussi des feux tout seul : on compare)
  const avant = (await L.etat(0)).stats.departs;
  let i = 0;
  while (i < c.alt.length && !(await L.action({ type: 'allumer', cell: i })).ok) i += 97;
  assert.ok(i < c.alt.length, 'allumer : aucune case ne brûle');
  assert.equal((await L.etat(0)).stats.departs, avant + 1, 'allumer : un départ compté');

  assert.equal((await L.action({ type: 'inconnue' })).ok, false, 'action inconnue refusée');

  const r = await L.action({ type: 'nouvelle', reglages: { graine: 9 } });
  assert.ok(r.ok && r.partie !== e.partie, 'nouvelle partie');
  e = await L.etat(0);
  assert.ok(e.t === 0 && e.reglages.graine === 9 && e.partie === r.partie, 'état de la nouvelle partie');

  console.log('OK');
  process.exit(0);   // la boucle de LiaisonWasm (setInterval) garderait Node en vie
} catch (err) {
  console.log('ÉCHEC :', err.message);
  process.exit(1);
}
