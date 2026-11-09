"""Les ordres du joueur, quand il prend la place du commandant des opérations de secours (COS).

Un moyen qui reçoit un ordre passe « sous vos ordres » : l'IA ne le réaffecte plus tant que l'ordre n'est pas
terminé ou que vous ne le lui rendez pas (« Rendre à l'IA »).

Les camions se commandent par groupe (GIFF : 4 CCF et le VLHR du chef, qui suit). Quatre modes d'action, chacun avec
un cône d'arrosage orienté (direction en degrés, x vers l'est, y vers le sud, comme les caps des moyens) :
  arroser   — le groupe se poste en ligne, face à la direction donnée, et n'arrose que dans son cône ;
  ligne     — ligne d'arrêt : les camions s'étalent sur la ligne tracée, arrosent le front ou mouillent devant eux ;
  proteger  — protection à 360° d'un site (maisons, camping) ;
  flanc     — attaque de flanc : le groupe remonte seul le flanc, de l'arrière vers la tête, en gardant son orientation.
Les avions : larguer (point visé et axe du largage), retardant (ligne du Dash selon l'axe).
Ordres simples : poster, attaquer, defendre, surveiller, pare_feu (bulldozer), retour, liberer.
"""
import math

from .combustibles import COMB, EAU
from .feu import BRULE
from .moyens import CONE, dans_cone, vitesse_sol

ORDRES_PAR_TYPE = {
    'ccf': {'arroser', 'ligne', 'proteger', 'flanc', 'poster', 'attaquer', 'defendre', 'surveiller', 'retour', 'liberer'},
    'bull': {'poster', 'pare_feu', 'retour', 'liberer'},
    'helico': {'larguer', 'retour', 'liberer'},
    'canadair': {'larguer', 'retour', 'liberer'},
    'dash': {'retardant', 'retour', 'liberer'},
    'vlhr': set(),
}
NOMS = {'poster': 'Se poster', 'attaquer': 'Attaque', 'defendre': 'Défense', 'surveiller': 'Surveillance', 'larguer': 'Largages',
        'retardant': 'Ligne de retardant', 'pare_feu': 'Pare-feu', 'retour': 'Retour à la base', 'arroser': 'Arrosage orienté',
        'ligne': "Ligne d'arrêt", 'proteger': 'Protection 360°', 'flanc': 'Attaque de flanc'}


def _feu_proche(p, cell):
    """Le feu dont une flamme est la plus proche de la cellule (sa racine), ou None."""
    g, feu = p.carte.g, p.feu
    best = None
    for i in feu.brulant:
        d = g.distance(i, cell)
        if d <= 8 and (best is None or d < best[0]):
            best = (d, i)
    return p.ia.racine(feu.foyer[best[1]]) if best else None


def _accessible(p, j, kind='ccf'):
    return vitesse_sol(kind, p.carte, p.feu, j) > 0 and p.feu.etat[j] != BRULE


def _poste_sur(p, cell, kind):
    """La cellule accessible la plus proche (pas en feu) autour de la cellule visée."""
    g = p.carte.g
    for r in range(0, 4):
        cands = [j for j in g.anneau(cell, r) if _accessible(p, j, kind)]
        if cands:
            return min(cands, key=lambda j: (g.distance(j, cell), -p.carte.route[j]))
    return None


def _angle(g, i, j):
    return math.degrees(math.atan2(g.cy[j] - g.cy[i], g.cx[j] - g.cx[i]))


def _decale(g, i, angle, n=1):
    a = math.radians(angle)
    return g.sous(g.cx[i] + math.cos(a) * g.hex_m * n, g.cy[i] + math.sin(a) * g.hex_m * n)


def direction_defaut(p, cell):
    """Vers la flamme la plus proche ; sans feu, face au vent (c'est de là que le feu viendrait)."""
    g, feu = p.carte.g, p.feu
    proches = [i for i in feu.brulant if g.distance(i, cell) <= 10]
    if proches:
        i = min(proches, key=lambda i: (g.distance(i, cell), -feu.I[i]))
        if i != cell:
            return round(_angle(g, cell, i))
    vx, vy = p.meteo.vers()
    return round(math.degrees(math.atan2(-vy, -vx)))


def _postes_en_ligne(p, poste, d, n):
    """n postes face à la direction d : le poste, puis ses voisins de part et d'autre (une ligne de front)."""
    g = p.carte.g
    cotes = []
    for k in (1, -1, 2, -2):
        j = _decale(g, poste, d + 90 * (1 if k > 0 else -1), abs(k))
        if j is not None and _accessible(p, j):
            cotes.append(j)
    ordre = [poste] + cotes
    return [ordre[k % len(ordre)] for k in range(n)]


def planifier(p, ids, ordre, cell=None, cell2=None, d=None):
    """Les missions que recevraient ces moyens (sans les donner). Renvoie (liste de (moyen, mission), refus)."""
    c, g = p.carte, p.carte.g
    moyens = [m for m in p.moyens if m.id in ids and not m.hs]
    refus = [u.nom for u in moyens if ordre not in ORDRES_PAR_TYPE[u.kind] and u.kind != 'vlhr']
    moyens = [u for u in moyens if ordre in ORDRES_PAR_TYPE[u.kind]]
    plans = []
    if ordre in ('liberer', 'retour'):
        return [(u, None) for u in moyens], refus
    if cell is None or not moyens:
        return [], refus + [u.nom for u in moyens]
    f = _feu_proche(p, cell)
    ccf = [u for u in moyens if u.kind == 'ccf']
    autres = [u for u in moyens if u.kind != 'ccf']
    # --- les groupes de camions
    if ccf and ordre in ('arroser', 'flanc'):
        poste = _poste_sur(p, cell, 'ccf')
        if poste is None:
            refus += [u.nom for u in ccf]
        else:
            dd = direction_defaut(p, poste) if d is None else d
            for u, q in zip(ccf, _postes_en_ligne(p, poste, dd, len(ccf))):
                plans.append((u, dict(type=ordre, poste=q, dir=round(dd), label=NOMS[ordre] + ' (vos ordres)')))
    elif ccf and ordre == 'ligne':
        fin = cell2 if cell2 is not None else cell
        cases = []
        for i in g.ligne(cell, fin):
            q = _poste_sur(p, i, 'ccf')
            if q is not None and q not in cases:
                cases.append(q)
        if not cases:
            refus += [u.nom for u in ccf]
        else:
            # face au feu : perpendiculaire à la ligne, du côté de la flamme la plus proche (sinon face au vent)
            axe = _angle(g, cell, fin) if fin != cell else 0.0
            vers = direction_defaut(p, cases[len(cases) // 2]) if d is None else d
            dd = axe + 90 if math.cos(math.radians(axe + 90 - vers)) >= 0 else axe - 90
            if fin == cell:
                dd = vers
            n = len(ccf)
            for k, u in enumerate(ccf):
                q = cases[round(k * (len(cases) - 1) / max(1, n - 1))] if n > 1 else cases[len(cases) // 2]
                plans.append((u, dict(type='ligne', poste=q, dir=round(dd) % 360, label=NOMS['ligne'] + ' (vos ordres)')))
    elif ccf and ordre in ('proteger', 'defendre'):
        sid = c.site_de[cell]
        if sid < 0:
            proches = [s for s in c.sites if s['type'] != 'reserve']
            sid = min(proches, key=lambda s: g.distance(s['centre'], cell))['id']
        s = c.sites[sid]
        menace = p.evac.menace if hasattr(p, 'evac') else {}
        cases = [i for i in s['cells'] if _accessible(p, i)] or [s['centre']]
        cases.sort(key=lambda i: (menace.get(i, 1e9), g.distance(i, cell)))
        cases = cases[:3]
        for k, u in enumerate(ccf):
            plans.append((u, dict(type='defense', site=sid, poste=cases[k % len(cases)], label=f"Protection 360° de {s['nom']} (vos ordres)")))
    elif ccf and ordre in ('poster', 'attaquer', 'surveiller'):
        poste = _poste_sur(p, cell, 'ccf')
        if poste is None:
            refus += [u.nom for u in ccf]
        else:
            typ = {'poster': 'preposition', 'attaquer': 'attaque', 'surveiller': 'surveillance'}[ordre]
            for u in ccf:
                plans.append((u, dict(type=typ, poste=poste, label=NOMS[ordre] + ' (vos ordres)')))
    # --- bulldozers et avions
    for u in autres:
        m = None
        if ordre == 'poster':
            poste = _poste_sur(p, cell, u.kind)
            if poste is not None:
                m = dict(type='preposition', poste=poste, label=NOMS[ordre] + ' (vos ordres)')
        elif ordre == 'pare_feu':
            fin = cell2 if cell2 is not None else cell
            cells = [i for i in g.ligne(cell, fin) if COMB[c.fuel[i]]['brule'] and c.site_de[i] < 0][:14]
            if cells:
                m = dict(type='pare_feu', cells=cells, label=f'Pare-feu de {len(cells) * 300} m (vos ordres)')
        elif ordre == 'larguer':
            if COMB[c.fuel[cell]]['brule'] or c.fuel[cell] in EAU:
                m = dict(type='largage', cible=cell, label='Largages au point visé (vos ordres)')
                if d is not None:
                    m['axe'] = round(d)
        elif ordre == 'retardant':
            m = dict(type='retardant', cible=cell, label='Ligne de retardant (vos ordres)')
            if d is not None:
                m['axe'] = round(d)
        if m is None:
            refus.append(u.nom)
        else:
            plans.append((u, m))
    if f is not None:
        for _, m in plans:
            if m is not None:
                m['feu'] = f
    return plans, refus


def donner(p, ids, ordre, cell=None, cell2=None, d=None):
    plans, refus = planifier(p, ids, ordre, cell, cell2, d)
    for u, m in plans:
        if ordre == 'liberer':
            u.joueur, u.mission = False, None
            continue
        u.mission = m
        u.joueur = True
        if not u.terre:
            u.phase = u.vise = u.cible = None
    if plans:
        noms = ', '.join(u.nom for u, _ in plans[:4]) + (f' et {len(plans) - 4} autres' if len(plans) > 4 else '')
        txt = {'liberer': "rendu(s) à l'IA", 'retour': 'rentre(nt) à la base'}.get(ordre, NOMS.get(ordre, ordre).lower())
        p.journal_ajout('joueur', f"Vos ordres — {noms} : {txt}.")
    return dict(ok=bool(plans), n=len(plans), refus=refus,
                erreur=None if plans else "Cet ordre ne convient pas à ces moyens, ou l'endroit est inaccessible.")


def apercu(p, ids, ordre, cell=None, cell2=None, d=None):
    """Ce que donnerait l'ordre, pour l'afficher avant de le valider : postes, trajets, cellules arrosées ou trop
    chaudes, eau restante ; pour les avions, le point d'écopage et les cellules du largage."""
    g, feu = p.carte.g, p.feu
    plans, refus = planifier(p, ids, ordre, cell, cell2, d)
    out = dict(ok=bool(plans), refus=refus, moyens=[])
    for u, m in plans:
        if m is None:
            continue
        e = dict(id=u.id, k=u.kind, eau=round(100 * u.eau / u.cap) if u.cap else None, poste=m.get('poste'),
                 dir=m.get('dir'), axe=m.get('axe'))
        if u.terre and m.get('poste') is not None:
            ch = u.chemin_vers(p, m['poste']) or []
            e['chemin'] = ch[:80]
            e['minutes'] = round(sum(1 / max(0.2, vitesse_sol(u.kind, p.carte, feu, j)) for j in ch[1:]))
            cone = (m['dir'], CONE) if m.get('dir') is not None else None
            q = m['poste']
            portee = [j for j in [q] + g.voisins(q) if dans_cone(g, q, j, cone)]
            e['portee'] = portee
            e['chaud'] = [j for j in portee if feu.etat[j] == BRULE and feu.I[j] > (1.01 if m['type'] == 'defense' else 0.8)]
            e['autonomie'] = round(u.eau / 400) if u.kind == 'ccf' else None
        elif not u.terre:
            ancienne = u.mission
            u.mission = m
            try:
                e['largage'] = (p.ia.ligne_retardant(u) if u.kind == 'dash' else p.ia.cible_largage(u)) or []
                e['ecopage'] = None if u.kind == 'dash' else p.eau_aerienne(u)
            finally:
                u.mission = ancienne
            e['x'], e['y'] = round(u.x), round(u.y)
        out['moyens'].append(e)
    return out


def suivre_flanc(p, u):
    """Attaque de flanc : le groupe avance vers la tête, d'un poste au contact à l'autre, en gardant le feu dans son cône."""
    g, feu = p.carte.g, p.feu
    m = u.mission
    vx, vy = p.meteo.vers()
    proj = lambda i: g.cx[i] * vx + g.cy[i] * vy
    ici = proj(u.cell)
    best = None
    for j in g.anneau(u.cell, 2):
        if j == u.cell or not _accessible(p, j):
            continue
        flammes = [k for k in g.voisins(j) if feu.etat[k] == BRULE and feu.I[k] <= 0.85]
        if not flammes or any(feu.etat[k] == BRULE and feu.I[k] > 0.92 for k in g.voisins(j)):
            continue
        sc = proj(j) - ici - g.distance(j, u.cell) * 40
        if proj(j) > ici - 10 and (best is None or sc > best[0]):
            best = (sc, j, max(flammes, key=lambda k: feu.I[k]))
    m['inactif'] = 0
    if best:
        m['poste'] = best[1]
        m['dir'] = round(_angle(g, best[1], best[2]))
