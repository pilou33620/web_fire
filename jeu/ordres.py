"""Les ordres du joueur, quand il prend la place du commandant des opérations de secours (COS).

Un moyen qui reçoit un ordre passe « sous vos ordres » : l'IA ne le réaffecte plus tant que l'ordre n'est pas
terminé ou que vous ne le lui rendez pas (« Rendre à l'IA »).

Ordres : poster (aller et attendre), attaquer (se mettre au contact et arroser), defendre (un site), surveiller
(noyer les lisières), larguer (eau, au point visé), retardant (ligne du Dash au point visé), pare_feu (bulldozer,
d'un point à un autre), retour (à la base), liberer (rendre à l'IA).
"""
from .combustibles import COMB, EAU
from .feu import BRULE
from .moyens import vitesse_sol

ORDRES_PAR_TYPE = {
    'ccf': {'poster', 'attaquer', 'defendre', 'surveiller', 'retour', 'liberer'},
    'bull': {'poster', 'pare_feu', 'retour', 'liberer'},
    'helico': {'larguer', 'retour', 'liberer'},
    'canadair': {'larguer', 'retour', 'liberer'},
    'dash': {'retardant', 'retour', 'liberer'},
}
NOMS = {'poster': 'Se poster', 'attaquer': 'Attaque', 'defendre': 'Défense', 'surveiller': 'Surveillance', 'larguer': 'Largages',
        'retardant': 'Ligne de retardant', 'pare_feu': 'Pare-feu', 'retour': 'Retour à la base'}


def _feu_proche(p, cell):
    """Le feu dont une flamme est la plus proche de la cellule (sa racine), ou None."""
    g, feu = p.carte.g, p.feu
    best = None
    for i in feu.brulant:
        d = g.distance(i, cell)
        if d <= 8 and (best is None or d < best[0]):
            best = (d, i)
    return p.ia.racine(feu.foyer[best[1]]) if best else None


def _poste_sur(p, cell, kind):
    """La cellule accessible la plus proche (pas en feu) autour de la cellule visée."""
    g, feu = p.carte.g, p.feu
    for r in range(0, 4):
        cands = [j for j in g.anneau(cell, r) if vitesse_sol(kind, p.carte, feu, j) > 0 and feu.etat[j] != BRULE]
        if cands:
            return min(cands, key=lambda j: (g.distance(j, cell), -p.carte.route[j]))
    return None


def donner(p, ids, ordre, cell=None, cell2=None):
    c, g = p.carte, p.carte.g
    ok, refus = [], []
    for uid in ids:
        u = next((m for m in p.moyens if m.id == uid), None)
        if u is None:
            continue
        if ordre not in ORDRES_PAR_TYPE[u.kind]:
            refus.append(u.nom)
            continue
        if ordre == 'liberer':
            u.joueur = False
            u.mission = None
            ok.append(u)
            continue
        if ordre == 'retour':
            u.mission = None
            u.joueur = True
            u.phase = u.vise = u.cible = None
            ok.append(u)
            continue
        if cell is None:
            refus.append(u.nom)
            continue
        f = _feu_proche(p, cell)
        m = None
        if ordre in ('poster', 'attaquer', 'surveiller', 'defendre'):
            poste = _poste_sur(p, cell, u.kind)
            if poste is None:
                refus.append(u.nom)
                continue
            if ordre == 'defendre':
                sid = c.site_de[cell]
                if sid < 0:
                    proches = [s for s in c.sites if s['type'] != 'reserve']
                    s = min(proches, key=lambda s: g.distance(s['centre'], cell))
                    sid = s['id']
                s = c.sites[sid]
                poste = _poste_sur(p, cell if c.site_de[cell] == sid else s['centre'], u.kind)
                m = dict(type='defense', site=sid, poste=poste, label=f"Défense de {s['nom']} (vos ordres)")
            else:
                typ = {'poster': 'preposition', 'attaquer': 'attaque', 'surveiller': 'surveillance'}[ordre]
                m = dict(type=typ, poste=poste, label=NOMS[ordre] + ' (vos ordres)')
        elif ordre == 'larguer':
            if not COMB[c.fuel[cell]]['brule'] and c.fuel[cell] not in EAU:
                refus.append(u.nom)
                continue
            m = dict(type='largage', cible=cell, label='Largages au point visé (vos ordres)')
            u.phase = u.vise = u.cible = None
        elif ordre == 'retardant':
            m = dict(type='retardant', cible=cell, label='Ligne de retardant (vos ordres)')
            u.vise = None
        elif ordre == 'pare_feu':
            fin = cell2 if cell2 is not None else cell
            cells = [i for i in g.ligne(cell, fin) if COMB[c.fuel[i]]['brule'] and c.site_de[i] < 0][:14]
            if not cells:
                refus.append(u.nom)
                continue
            m = dict(type='pare_feu', cells=cells, label=f'Pare-feu de {len(cells) * 300} m (vos ordres)')
        if f is not None:
            m['feu'] = f
        u.mission = m
        u.joueur = True
        ok.append(u)
    if ok:
        noms = ', '.join(u.nom for u in ok[:4]) + (f' et {len(ok) - 4} autres' if len(ok) > 4 else '')
        txt = {'liberer': 'rendu(s) à l\'IA', 'retour': 'rentre(nt) à la base'}.get(ordre, NOMS.get(ordre, ordre).lower())
        p.journal_ajout('joueur', f"Vos ordres — {noms} : {txt}.")
    return dict(ok=bool(ok), n=len(ok), refus=refus,
                erreur=None if ok else "Cet ordre ne convient pas à ces moyens, ou l'endroit est inaccessible.")
