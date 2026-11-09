// Le panneau de droite : situation (météo, bilan, feux), moyens, journal du commandement, réglages de la partie.
export const esc = s => String(s ?? '').replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
export const nf = n => Math.round(n).toLocaleString('fr-FR');
export const ICONE = { ccf: '🚒', vlhr: '🚙', fpt: '🚒', vpc: '📡', bull: '🚜', debr: '🌿', ravit: '⛽', vsav: '🚑', helico: '🚁', canadair: '🛩', dash: '✈️', dragon: '🚁' };
export const ORDRE_TYPES = ['ccf', 'vlhr', 'fpt', 'vpc', 'bull', 'debr', 'ravit', 'vsav', 'helico', 'canadair', 'dash', 'dragon'];
export const NOM_TYPE = { ccf: 'Camions-citernes feux de forêt', vlhr: 'Chefs de groupe (VLHR)', fpt: 'Fourgons pompe-tonnes (maisons)', vpc: 'Postes de commandement',
  debr: 'Débroussailleuses', ravit: 'Ravitailleurs en gazole', vsav: 'Ambulances (VSAV)', dragon: 'Hélicoptères de secours (Dragon)', bull: 'Bulldozers', helico: 'Hélicoptères bombardiers d\'eau', canadair: 'Canadair', dash: 'Dash 8 (retardant)' };
const RISQUE_COUL = { Faible: '#7be08a', 'Léger': '#b9e07b', 'Modéré': '#ffd34d', 'Sévère': '#ff9a3c', 'Très sévère': '#ff6a3a', Exceptionnel: '#e0403a' };
export const risqueCouleur = n => RISQUE_COUL[n] || '#ffc94d';
const STATUT = { 'non détecté': ['#d7c9b4', 'Non détecté'], naissant: ['#ffb02e', 'Naissant'], 'en progression': ['#ff6a3a', 'En progression'], 'fixé': ['#ffd34d', 'Fixé'], 'maîtrisé': ['#7be08a', 'Maîtrisé'], 'éteint': ['#9aa39a', 'Éteint'] };
export const statut = s => STATUT[s] || ['#ccc', s];
const SITE_ETAT = { sauf: ['ok', 'Sauf'], 'menacé': ['alerte', 'Menacé'], 'touché': ['chaud', 'Touché'], 'détruit': ['chaud', 'Détruit'] };
// [icône, nom, cible (0 : aucune, 1 : un point, 2 : un point et une orientation ou une fin), aide du geste]
export const ORDRES = {
  arroser: ['🎯', 'Arroser vers…', 2, 'Appuyez à l\'endroit du poste, glissez vers le feu pour orienter les lances, relâchez.'],
  ligne: ['➖', 'Ligne d\'arrêt', 2, 'Appuyez au début de la ligne, glissez jusqu\'à sa fin, relâchez : les camions s\'y étalent face au feu.'],
  proteger: ['🛡', 'Protéger à 360°', 1, 'Touchez la maison, le village ou le camping à protéger.'],
  flanc: ['↗', 'Attaque de flanc', 2, 'Appuyez au point de départ, glissez vers le feu, relâchez : le groupe remontera le flanc vers la tête.'],
  poster: ['📍', 'Se poster ici', 1, 'Touchez l\'endroit où attendre.'],
  surveiller: ['👁', 'Surveiller ici', 1, 'Touchez la lisière à surveiller et noyer.'],
  larguer: ['💧', 'Larguer (axe)', 2, 'Appuyez sur le point de largage, glissez pour orienter l\'axe de la passe, relâchez.'],
  retardant: ['🟥', 'Ligne de retardant', 2, 'Appuyez au milieu de la ligne, glissez pour l\'orienter, relâchez.'],
  debroussailler: ['🌿', 'Débroussailler', 2, 'Appuyez au début de la bande à débroussailler, glissez jusqu\'à sa fin, relâchez.'],
  pare_feu: ['🚧', 'Pare-feu', 2, 'Appuyez au début du pare-feu, glissez jusqu\'à sa fin, relâchez.'],
  retour: ['↩', 'Retour à la base', 0, ''], liberer: ['🤖', 'Rendre à l\'IA', 0, ''],
};
export const ORDRES_TYPE = { ccf: ['arroser', 'ligne', 'proteger', 'flanc', 'poster', 'surveiller'], vlhr: [], fpt: ['proteger', 'arroser', 'poster'],
  vpc: ['poster'], bull: ['poster', 'pare_feu'], debr: ['debroussailler', 'poster'], ravit: ['poster'], vsav: ['poster'], helico: ['larguer'], canadair: ['larguer'], dash: ['retardant'], dragon: [] };
export const ORDRE_DEFAUT = { ccf: 'arroser', vlhr: 'arroser', fpt: 'proteger', vpc: 'poster', bull: 'pare_feu', debr: 'debroussailler', helico: 'larguer', canadair: 'larguer', dash: 'retardant' };
const LOGISTIQUE = { ravit: 'Logistique automatique : il va faire le plein des engins à sec. Posté par vous, il ravitaille ceux qui passent à côté.',
  vsav: 'Logistique automatique : il prend en charge les blessés accessibles par la route et les conduit à l\'hôpital.',
  dragon: 'Logistique automatique : hélitreuillage des équipages en danger et des blessés graves, de jour comme de nuit.',
  vpc: 'Posé près d\'un feu (5 min) : prévision du feu affichée, commandement plus réactif, lances +20 % sur ce feu.' };
const EVAC = { normal: '', evacuation: 'évacuation en cours', confinement: 'à l\'abri sur place', evacue: 'évacué' };
export function boutonsOrdres(kinds, ids, arme) {
  const l = [...new Set(kinds.flatMap(k => ORDRES_TYPE[k]))].concat(['retour', 'liberer']);
  return `<div class="ordres" data-ids="${ids.join(',')}">` + l.map(o => `<button type="button" class="btn ${arme === o ? 'prim' : ''}" data-ordre="${o}" title="${esc(ORDRES[o][3] || ORDRES[o][1])}">${ORDRES[o][0]} ${esc(ORDRES[o][1])}</button>`).join('') + '</div>';
}
function boutonsSite(E, sid) {
  const ev = E.evac?.sites[sid]; if (!ev) return '';
  if (ev.statut === 'evacue') return '<small class="ok">évacué</small>';
  const b = [];
  if (ev.statut !== 'evacuation') b.push(`<button type="button" class="mini" data-evac="${sid}" title="Évacuer par la route">🚗 Évacuer</button>`);
  if (ev.statut !== 'confinement') b.push(`<button type="button" class="mini" data-conf="${sid}" title="Mise à l'abri sur place">🏠 Confiner</button>`);
  return (ev.statut !== 'normal' ? `<small>${EVAC[ev.statut]}</small>` : '') + b.join('');
}
const DIRS = ['N', 'NNE', 'NE', 'ENE', 'E', 'ESE', 'SE', 'SSE', 'S', 'SSO', 'SO', 'OSO', 'O', 'ONO', 'NO', 'NNO'];
export const rose = de => DIRS[Math.round(((de % 360) + 360) % 360 / 22.5) % 16];

export function etatMoyen(d) {
  const a = d.a?.k;
  const A = { arrose: 'arrose le feu', noie: 'noie les lisières', protege: 'protège les maisons', remplit: 'fait le plein d\'eau', va_remplir: 'va remplir sa citerne',
    ecope: d.k === 'canadair' ? 'écope' : 'puise de l\'eau', largue: 'largue !', vol: 'en vol', au_sol: 'au sol', defriche: 'ouvre le pare-feu', attend: 'en position',
    surveille: 'surveille', patrouille: 'patrouille', mouille: 'mouille devant la ligne', commande: 'commande le groupe',
    autoprotection: '🛡 autoprotection : équipage à l\'abri', hs: '⚠ hors service (perdu)',
    deploie: 'déploie le poste de commandement', pc: '📡 poste de commandement en place', ravitaille: 'fait le plein de gazole d\'un engin',
    soigne: 'soigne les blessés', treuille: 'hélitreuillage', debroussaille: 'débroussaille', panne: '⛽ panne sèche', alerte: d.k === 'ccf' || d.k === 'bull' ? 'départ de la caserne' : 'décollage' };
  if (a && A[a]) return A[a];
  if (d.hs) return '⚠ hors service (perdu)';
  return { dispo: 'disponible', route: 'en route', action: 'en action', ravitaillement: 'ravitaillement', au_sol: 'au sol', retour: 'retour à la base', alerte: 'alerte' }[d.e] || d.e;
}

function carteFeu(f, C, sel, E) {
  const [col, lib] = statut(f.statut);
  const menaces = f.menaces.slice(0, 4).map(m => {
    const s = C.sites[m.site];
    return `<li class="${m.eta < 60 ? 'urgent' : ''}"><span>${esc(s.nom)}</span><span class="droite">${m.eta < 1 ? 'atteint' : 'dans ' + m.eta + ' min'}</span><span class="act">${boutonsSite(E, m.site)}</span></li>`;
  }).join('');
  const moy = Object.entries(f.moyens).map(([k, n]) => `${ICONE[k]} ${n}`).join('  ');
  return `<div class="carte-feu ${sel ? 'choisi' : ''}" data-feu="${f.id}">
    <div class="tete"><b>${esc(f.nom)}</b><span class="ha">${nf(f.ha)} ha</span></div>
    <div class="chips"><span class="chip"><i style="background:${col}"></i>${lib}</span>${f.vitesse ? `<span class="chip">tête ${String(f.vitesse).replace('.', ',')} km/h</span>` : ''}${f.sautes ? `<span class="chip chaud">${f.sautes} saute${f.sautes > 1 ? 's' : ''}</span>` : ''}${f.pc ? '<span class="chip ok">📡 PC posé</span>' : ''}${moy ? `<span class="chip">${moy}</span>` : ''}</div>
    ${f.strategie ? `<div class="strat">🧭 ${esc(f.strategie)}</div>` : ''}
    ${menaces ? `<ul>${menaces}</ul>` : ''}
    <div class="bas"><button type="button" class="btn prim" data-inter="${f.id}">🔍 Voir l'intervention</button><button type="button" class="btn" data-centrer="${f.id}">Centrer</button></div>
  </div>`;
}

export function situation(E, C, sel, mode, raison) {
  const m = E.meteo, st = E.stats;
  const actifs = E.feux.filter(f => f.statut !== 'éteint');
  const finis = E.feux.filter(f => f.statut === 'éteint');
  let h = '';
  if (mode === 'demo') h += `<div class="cos vous"><b>▶ Démo enregistrée</b><span>Le moteur Python n'a pas pu se charger sur cette page${raison ? ' (' + esc(raison) + ')' : ''}. Vous regardez une journée jouée par le vrai moteur : vitesse et pause marchent, mais pas les ordres. Pour jouer : <code>python server.py</code> sur le Pi ou le PC.</span></div>`;
  if (sel.cell != null) h += ficheCellule(E, C, sel.cell);
  if (sel.groupe?.length) h += ficheGroupe(E, sel.groupe, sel.arme);
  else if (sel.moyen != null) { const d = E.moyens.find(u => u.id === sel.moyen); if (d) h += ficheMoyen(d, C, E); }
  const manuel = E.commandement === 'manuel';
  h += `<div class="cos ${manuel ? 'vous' : ''}"><b>${manuel ? '🧑‍🚒 Le COS, c\'est vous' : '🤖 Le COS est l\'IA'}</b><span>${manuel ? 'L\'IA ne fait que signaler les menaces. Choisissez un moyen (carte ou onglet Moyens) et donnez-lui un ordre.' : 'Vous pouvez quand même donner des ordres : le moyen passe sous vos ordres jusqu\'à ce que vous le rendiez.'}</span>
    <div class="bas"><button type="button" class="btn ${manuel ? '' : 'prim'}" data-cos="${manuel ? 'auto' : 'manuel'}">${manuel ? '🤖 Rendre le commandement à l\'IA' : '🧑‍🚒 Prendre le commandement'}</button>${manuel ? '<button type="button" class="btn" data-renfort>🚒 Demander des renforts</button>' : ''}</div></div>`;
  h += `<h2>${actifs.length ? actifs.length + ' feu' + (actifs.length > 1 ? 'x' : '') + ' en cours' : 'Aucun feu en cours'}</h2>
    <div class="sous">${esc(m.nomScenario)}</div>
    <div class="meteo">
      <div>Température<b>${String(m.temp).replace('.', ',')} °C</b></div><div>Humidité<b>${m.hr} %</b></div>
      <div>Vent du ${rose(m.de)}<b>${m.vent} km/h</b>rafales ${m.rafales}</div>
      <div>Danger<b style="color:${risqueCouleur(m.niveau)}">${esc(m.niveau)}</b><div class="jauge"><span style="width:${Math.round(m.danger * 100)}%"></span></div></div>
    </div>
    <div class="chips">${m.nuit ? '<span class="chip">🌙 Nuit : avions au sol</span>' : ''}${!m.nuit && !m.volCanadair ? '<span class="chip chaud">Vent trop fort : Canadair au sol</span>' : ''}${m.orage ? '<span class="chip chaud">⚡ Orage</span>' : ''}${m.pluie > 0.1 ? '<span class="chip ok">🌧 Pluie</span>' : ''}<span class="chip">Combustible ${String(m.hcomb).replace('.', ',')} % d'eau</span></div>`;
  h += actifs.map(f => carteFeu(f, C, f.id === sel.feu, E)).join('');
  if (!actifs.length) h += `<p class="note">Le commandement surveille le massif. Par risque sévère, il pré-positionne des camions et fait patrouiller des Canadair chargés d'eau (guet aérien armé). Vous pouvez provoquer un départ avec « 🔥 Allumer un feu ».</p>`;
  h += `<h3>Bilan de la journée</h3><dl class="lignes">
    <dt>Surface brûlée</dt><dd>${nf(st.ha)} ha</dd><dt>Départs de feu</dt><dd>${st.departs}</dd>
    <dt>Eau larguée et projetée</dt><dd>${nf(st.litres / 1000)} m³</dd><dt>Retardant</dt><dd>${nf(st.litresRetardant / 1000)} m³</dd>
    <dt>Largages aériens</dt><dd>${st.largages}</dd><dt>Colonnes de renfort</dt><dd>${st.renforts}</dd>
    <dt>Personnes évacuées</dt><dd>${nf(st.personnes.evacues)}</dd><dt>À l'abri sur place</dt><dd>${nf(st.personnes.confines)}</dd>
    <dt>Sur les routes</dt><dd>${nf(st.personnes.route)}</dd><dt>Personnes exposées au feu</dt><dd style="color:${st.personnes.exposes ? 'var(--rouge)' : 'inherit'}">${nf(st.personnes.exposes)}</dd>
    <dt>Habitations détruites (est.)</dt><dd>${nf(st.maisons)}</dd>
    <dt>Blessés à l'hôpital</dt><dd>${st.blesses?.hopital || 0}</dd><dt>Blessés en attente de secours</dt><dd style="color:${st.blesses?.attente ? 'var(--rouge)' : 'inherit'}">${st.blesses?.attente || 0}</dd>
    ${st.blesses?.aggraves ? `<dt>États aggravés (secours trop tardifs)</dt><dd style="color:var(--rouge)">${st.blesses.aggraves}</dd>` : ''}
    <dt>Hexagones débroussaillés</dt><dd>${st.debroussaille || 0}</dd>${st.pannes ? `<dt>Engins en panne sèche</dt><dd style="color:var(--rouge)">${st.pannes}</dd>` : ''}
    <dt>Autoprotections déclenchées</dt><dd>${st.autoprotections || 0}</dd><dt>Engins perdus</dt><dd style="color:${st.perdus ? 'var(--rouge)' : 'inherit'}">${st.perdus || 0}</dd><dt>Note du commandement</dt><dd style="color:var(--or)">${st.score} / 100</dd></dl>`;
  const touches = E.sites.filter(s => s.etat !== 'sauf');
  h += `<h3>Points sensibles</h3><div class="chips">` + C.sites.filter(s => s.type !== 'reserve').map(s => {
    const e = E.sites[s.id]; const [cls, lib] = SITE_ETAT[e.etat];
    const ev = E.evac?.sites[s.id];
    return `<span class="chip ${cls}" title="${esc(lib)}">${esc(s.nom)}${ev && ev.statut !== 'normal' ? ' · ' + EVAC[ev.statut] : ''}${e.defenseurs ? ' · 🚒' + e.defenseurs : ''}</span>`;
  }).join('') + `</div>`;
  if (finis.length) h += `<h3>Feux éteints</h3>` + finis.map(f => `<div class="chips"><span class="chip">${esc(f.nom)} · ${nf(f.ha)} ha</span></div>`).join('');
  if (!touches.length && !actifs.length) h += '';
  return h;
}

function ficheCellule(E, C, i) {
  const f = C.fuel[i], e = E.etat[i];
  const ETAT = { '.': 'Intact', f: 'En feu', b: 'Brûlé', e: 'Éteint (braises possibles)', n: 'Noyé', x: 'Pare-feu (sol nu)' };
  const feu = E.feu.find(x => x[0] === i), ret = E.ret.includes(i), mou = E.mouille.find(x => x[0] === i);
  const s = C.sites.find(s => s.cells.includes(i));
  return `<div class="carte-feu choisi"><div class="tete"><b>${esc(s ? s.nom : C.comb[f].nom)}</b><span class="sous">${C.alt[i]} m</span></div>
    <div class="chips"><span class="chip">${esc(C.comb[f].nom)}</span><span class="chip ${e === 'f' ? 'chaud' : ''}">${ETAT[e]}</span>${ret ? '<span class="chip chaud">Retardant</span>' : ''}${mou ? `<span class="chip">Mouillé ${mou[1]} %</span>` : ''}${C.route[i] !== '0' ? `<span class="chip">${['', 'Piste DFCI', 'Route', 'Route principale'][+C.route[i]]}</span>` : ''}</div>
    ${feu ? `<dl class="lignes"><dt>Intensité</dt><dd>${feu[1]} %</dd><dt>Combustible consumé</dt><dd>${feu[2]} %</dd></dl>` : ''}
    ${C.route[i] !== '0' && C.voies ? bonusMalus(C.voies[C.route[i]]) : ''}</div>`;
}

export function ficheMoyen(d, C, E) {
  const f = d.f != null ? E.feux.find(x => x.id === d.f) : null;
  return `<div class="carte-feu choisi fiche"><div class="tete"><b>${ICONE[d.k]} ${esc(d.n)}</b><span class="sous">${d.j ? '🧑‍🚒 vos ordres' : '🤖 IA'}${d.r ? ' · renfort' : ''}</span></div>
    <div class="chips"><span class="chip ${d.hs || d.au ? 'chaud' : ''}">${esc(etatMoyen(d))}</span>${d.eau != null ? `<span class="chip">💧 ${d.eau} %</span>` : ''}${d.m ? `<span class="chip">${esc(d.m)}</span>` : ''}</div>
    ${d.ca != null || d.ch ? `<div class="chips">${d.ca != null ? `<span class="chip ${d.ca < 15 ? 'chaud' : ''}">⛽ gazole ${d.ca} %</span>` : ''}${d.ch ? `<span class="chip">🚑 ${d.ch} blessé${d.ch > 1 ? 's' : ''} à bord</span>` : ''}</div>` : ''}
    ${LOGISTIQUE[d.k] ? `<p class="note">${esc(LOGISTIQUE[d.k])}</p>` : ''}
    <dl class="lignes">${f ? `<dt>Feu</dt><dd>${esc(f.nom)}</dd>` : ''}${d.k === 'ccf' || d.k === 'fpt' ? `<dt>Réserve d'autoprotection</dt><dd>${d.au ? 'en cours d\'utilisation' : (d.k === 'fpt' ? '300' : '500') + ' L (intouchable par les lances)'}</dd>` : ''}${d.g ? `<dt>Groupe</dt><dd>${esc(d.g)} <button type="button" class="mini" data-groupe="${esc(d.g)}">choisir le groupe</button></dd>` : ''}<dt>Eau projetée</dt><dd>${nf(d.l / 1000)} m³</dd></dl>
    ${d.k === 'vlhr' ? '<p class="note">Le chef suit son groupe : donnez vos ordres au groupe.</p>' : boutonsOrdres([d.k], [d.id], null)}</div>`;
}
export function ficheGroupe(E, ids, arme) {
  const l = E.moyens.filter(d => ids.includes(d.id));
  if (!l.length) return '';
  const eau = l.filter(d => d.eau != null), moy = eau.length ? Math.round(eau.reduce((s, d) => s + d.eau, 0) / eau.length) : null;
  const auto = l.filter(d => d.k === 'ccf').reduce((s, d) => s + d.eau * 40, 0) / Math.max(1, l.filter(d => d.k === 'ccf').length) / 400;
  return `<div class="carte-feu choisi fiche"><div class="tete"><b>${esc(l[0].g || 'Groupe')} · ${l.length} engins</b><button type="button" class="mini" data-deselect>✕</button></div>
    ${moy != null ? `<div class="chips"><span class="chip">💧 ${moy} % en moyenne</span>${l[0].k === 'ccf' || l[0].k === 'vlhr' ? `<span class="chip">≈ ${Math.round(auto)} min de lance</span>` : ''}</div>` : ''}
    <ul>${l.map(d => `<li><span>${ICONE[d.k]} ${esc(d.n)}</span><span>${d.eau != null ? d.eau + ' % · ' : ''}${esc(etatMoyen(d))}${d.j ? ' · 🧑‍🚒' : ''}</span></li>`).join('')}</ul>
    ${boutonsOrdres([...new Set(l.map(d => d.k))], l.filter(d => d.k !== 'vlhr').map(d => d.id), arme)}</div>`;
}

export function bonusMalus(v) {
  if (!v) return '';
  return `<div class="bm"><b>${esc(v.nom)}</b><ul>${v.bonus.map(t => `<li class="plus">＋ ${esc(t)}</li>`).join('')}${v.malus.map(t => `<li class="moins">－ ${esc(t)}</li>`).join('')}</ul></div>`;
}

export function moyens(E, sel) {
  const par = {};
  for (const d of E.moyens) (par[d.k] ||= []).push(d);
  let h = `<h2>Les moyens</h2><div class="sous">${E.moyens.length} engins · touchez-en un pour le suivre</div>`;
  for (const k of ORDRE_TYPES) {
    const l = par[k]; if (!l) continue;
    const eng = l.filter(d => d.m).length;
    const grp = [...new Set(l.map(d => d.g).filter(Boolean))];
    h += `<div class="groupe"><h3>${ICONE[k]} ${NOM_TYPE[k]} · ${eng}/${l.length} engagés</h3>` + (grp.length ? `<div class="chips">${grp.map(g => `<button type="button" class="mini" data-groupe="${esc(g)}">Choisir ${esc(g)}</button>`).join('')}<button type="button" class="mini" data-tous="${k}">Tous</button></div>` : `<div class="chips"><button type="button" class="mini" data-tous="${k}">Choisir tous</button></div>`) + l.map(d => `<div class="moyen ${d.id === sel ? 'choisi' : ''}" data-moyen="${d.id}">
      <span class="ic">${ICONE[k]}</span><b>${esc(d.n)}${d.j ? ' 🧑‍🚒' : ''}</b>${d.eau != null ? `<span class="eau" title="Eau ${d.eau} % (le trait marque la réserve d'autoprotection)"><span style="width:${d.eau}%"></span>${k === 'ccf' ? '<i class="res"></i>' : ''}</span>` : '<span></span>'}
      <small>${esc(etatMoyen(d))}${d.m ? ' · ' + esc(d.m) : ''}</small></div>`).join('') + '</div>';
  }
  return h;
}

export function journal(liste) {
  return `<h2>Journal du commandement</h2><div class="sous">Les décisions du COS et les alertes, les plus récentes en haut.</div>
    <ul class="journal">${liste.slice().reverse().map(j => `<li class="${esc(j.k)}"><time>${esc(j.h)}</time><span>${esc(j.txt)}</span></li>`).join('')}</ul>`;
}

export function partie(infos, R, doctrine, mode) {
  if (!infos) return '<p class="sous">Chargement…</p>';
  const opt = (o, v) => Object.entries(o).map(([k, n]) => `<option value="${k}" ${k === v ? 'selected' : ''}>${esc(n)}</option>`).join('');
  const cur = k => { const [a, b] = infos.limites[k]; const v = R.moyens?.[k] ?? infos.defaut[k]; return `<label>${ICONE[k]} ${NOM_TYPE[k]}<span class="curseur"><input type="range" min="${a}" max="${b}" value="${v}" data-m="${k}"><output>${v}</output></span></label>`; };
  return `<h2>Nouvelle partie</h2><div class="sous">Moteur Python ${mode === 'serveur' ? 'sur le serveur' : mode === 'demo' ? 'indisponible ici : démo enregistrée (lecture seule)' : 'dans le navigateur (Pyodide)'}</div>
    <form class="form" id="f-partie">
      <label>Massif<select name="theme">${opt(infos.themes, R.theme)}</select></label>
      <label>Météo du jour<select name="scenario">${opt(infos.scenarios, R.scenario)}</select></label>
      <label>Graine de la carte<input type="number" name="graine" value="${R.graine}" min="1" max="99999"></label>
      ${['ccf', 'fpt', 'helico', 'canadair', 'dash', 'dragon', 'bull', 'debr', 'vpc', 'ravit', 'vsav'].filter(k => infos.limites[k]).map(cur).join('')}
      <label>Doctrine du commandement<select name="doctrine">${opt({ equilibree: 'Équilibrée (doctrine française)', offensive: 'Offensive : tout sur la tête du feu', defensive: 'Défensive : priorité absolue aux biens' }, R.doctrine || doctrine)}</select></label>
      <button type="submit" class="btn prim">Lancer la nouvelle partie</button>
    </form>
    <h3>Pendant la partie</h3>
    <div class="form">
      <label>Doctrine en cours<select id="doctrine">${opt({ equilibree: 'Équilibrée', offensive: 'Offensive', defensive: 'Défensive' }, doctrine)}</select></label>
      <label>Le vent vient du… <span class="curseur"><input type="range" min="0" max="345" step="15" id="vent-de"><output id="vent-de-o">–</output></span></label>
      <label>Force du vent (km/h) <span class="curseur"><input type="range" min="0" max="100" step="5" id="vent-f"><output id="vent-f-o">–</output></span></label>
      <div style="display:flex;gap:6px;flex-wrap:wrap"><button type="button" class="btn" id="b-vent">Appliquer le vent</button><button type="button" class="btn feu" id="b-orage">⚡ Orage sec</button><button type="button" class="btn" id="b-avancer">⏩ +30 min</button></div>
    </div>
    <h3>Routes et pistes : bonus et malus</h3>${['3', '2', '1'].map(k => bonusMalus(window.__feux?.C?.voies?.[k])).join('')}
    <h3>Légende</h3><div class="legende">
      <span><i style="background:#4f8a3a"></i>Pinède</span><span><i style="background:#9da35a"></i>Lande</span><span><i style="background:#5d9b3a"></i>Feuillus</span><span><i style="background:#d9c25a"></i>Cultures</span>
      <span><i style="background:#3b3632"></i>Brûlé</span><span><i style="background:#4f4439"></i>Éteint</span><span><i style="background:#e0403a"></i>Retardant</span><span><i style="background:#2f7fd8"></i>Mouillé</span>
      <span><i style="background:#6f9e4c"></i>Forêt débroussaillée</span><span><i style="background:#e0cc98"></i>Piste DFCI</span><span><i style="background:#b8955f"></i>Pare-feu</span></div>
    <p class="note">Le simulateur se gère seul : le commandement (COS) suit la doctrine française — attaque massive des feux naissants, défense des points sensibles, attaque par les flancs, avions sur la tête et les sautes, retardant et pare-feu devant le front, puis surveillance des lisières. Les avions ne volent ni de nuit ni par vent trop fort.</p>`;
}
