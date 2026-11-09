"""Le commandant des opérations de secours (COS) : il lit les feux, prévoit leur avancée et engage les moyens.

Doctrine (inspirée de la doctrine française de lutte contre les feux de forêts) :
1. attaque massive des feux naissants : tout feu détecté reçoit vite des moyens importants, terrestres et aériens ;
2. sur un feu installé, priorité aux vies et aux biens : mise en défense des points sensibles menacés (villes, campings,
   usines) avant l'arrivée du front ;
3. on ne combat pas la tête d'un feu violent au sol : les camions attaquent par les flancs, de l'arrière vers la tête,
   en s'appuyant sur le noir (zone déjà brûlée) ;
4. les avions cassent la tête et traitent les sautes de feu ; le Dash pose des lignes de retardant devant la tête ;
   les bulldozers ouvrent un pare-feu là où le feu arrivera dans une heure ou plus ;
5. quand le feu est fixé : surveillance et noyage des lisières pour empêcher les reprises ;
6. par risque très sévère : pré-positionnement des camions dans les massifs et guet aérien armé (GAAr).
"""
import math

from .combustibles import COMB, ROUTE_FREIN
from .feu import BRULE, BRULE_FINI, ETEINT, NOYE, INTACT
from .hexa import dijkstra
from .moyens import vitesse_sol

STATUTS = ['naissant', 'en progression', 'fixé', 'maîtrisé', 'éteint']


class Commandement:
    def __init__(self, p, doctrine='equilibree'):
        self.p = p
        self.doctrine = doctrine            # 'equilibree', 'offensive' (tout sur la tête), 'defensive' (tout sur les biens)
        self.feux = {}                      # racine du foyer -> fiche du feu
        self.alias = {}                     # foyer -> racine (feux qui se sont rejoints)
        self.renforts = 0
        self.gaar = False
        self.prepos = False
        self.mode = 'auto'                  # 'auto' : le COS est l'IA ; 'manuel' : le COS, c'est vous (l'IA conseille seulement)

    # ------------------------------------------------------------------ foyers et feux
    def racine(self, f):
        while f in self.alias:
            f = self.alias[f]
        return f

    def _regrouper(self):
        """Regroupe les hexagones en feu par foyer d'origine ; fusionne deux foyers dont les flammes se touchent."""
        feu, g = self.p.feu, self.p.carte.g
        par = {}
        for i in feu.brulant:
            r = self.racine(feu.foyer[i])
            for j in g.voisins(i):
                if feu.etat[j] == BRULE:
                    r2 = self.racine(feu.foyer[j])
                    if r2 != r:
                        a, b = min(r, r2), max(r, r2)
                        self.alias[b] = a
                        if b in self.feux and a in self.feux:
                            self.feux[a]['fusion'] = self.feux[a].get('fusion', []) + [self.feux[b]['nom']]
                            self.p.journal_ajout('feu', f"{self.feux[b]['nom']} rejoint {self.feux[a]['nom']}.")
                            self.feux[b]['statut'] = 'éteint'
                            self.feux[b]['absorbe'] = a
                        r = a
            par.setdefault(r, set()).add(i)
        # les hexagones éteints non noyés restent dans leur feu (reprises possibles)
        eteints = {}
        for i in feu.eteints:
            eteints.setdefault(self.racine(feu.foyer[i]), set()).add(i)
        return par, eteints

    def _detecte(self, r):
        d = self.p.feu.departs
        return any(d[f]['detecte'] for f in range(len(d)) if self.racine(f) == r)

    # ------------------------------------------------------------------ prévision
    def _prevision(self, cells, horizon=360):
        """Temps d'arrivée prévu du feu (minutes) sur chaque hexagone, au vent actuel."""
        p = self.p
        c, feu, m, g = p.carte, p.feu, p.meteo, p.carte.g
        vx, vy = m.vers()
        V = m.vent
        fsech = m.f_secheresse()
        dirs = {}

        def cout(i, j):
            if not COMB[c.fuel[j]]['brule'] or feu.etat[j] in (BRULE, BRULE_FINI):
                return None
            cj = COMB[c.fuel[j]]
            ux, uy = (g.cx[j] - g.cx[i]) / g.hex_m, (g.cy[j] - g.cy[i]) / g.hex_m
            fw = math.exp(0.045 * V * (ux * vx + uy * vy))
            fs = math.exp(3.0 * max(-0.3, min(0.45, (c.alt[j] - c.alt[i]) / g.hex_m)))
            r = cj['ros'] * fw * fs * m.f_humidite(feu.mouille[j]) * fsech * ROUTE_FREIN[c.route[j]]
            r *= 0.12 if feu.ret[j] > 0 else 1.0
            r *= 0.3 ** min(3, feu.defense[j])
            r *= 0.85 * g.ech                          # intensité de référence
            return 1.0 / max(r, 1e-4)

        return dijkstra(g, {i: 0.0 for i in cells}, cout, horizon)

    # ------------------------------------------------------------------ boucle
    def bonus_pc(self, cell):
        """×1,2 sur l'efficacité des lances si le feu de cette cellule a un poste de commandement posé."""
        f = self.feux.get(self.racine(self.p.feu.foyer[cell])) if self.p.feu.foyer[cell] >= 0 else None
        return 1.2 if f and f.get('pc') else 1.0

    def pas(self):
        p = self.p
        # un poste de commandement posé : le commandement suit les feux chaque minute au lieu de toutes les deux
        if p.t % 2 and not any(f.get('pc') for f in self.feux.values() if f['statut'] != 'éteint'):
            return
        par, eteints = self._regrouper()
        self._fiches(par, eteints)
        self._liberer()
        actifs = sorted([f for f in self.feux.values() if f['statut'] != 'éteint' and f['detecte']],
                        key=lambda f: -f['prio'])
        if self.mode == 'manuel':
            for f in actifs:
                self._alertes(f)
            return
        for f in actifs:
            self._engager(f)
        self._renforts()
        self._prevention(bool(actifs))

    def _alertes(self, f):
        """En mode manuel, l'IA ne commande plus : elle signale seulement les sites menacés."""
        for sid, eta in f['menaces'].items():
            if eta <= 150 and sid not in f['menace_log']:
                f['menace_log'].add(sid)
                self.p.journal_ajout('alerte', f"{self.p.carte.sites[sid]['nom']} menacé : arrivée du feu estimée dans {eta} min.")
        st = {'naissant': 'Feu naissant : à vous d\'engager les moyens', 'en progression': 'Feu en progression : à vous de jouer'}.get(f['statut'], f['statut'].capitalize())
        self._strategie(f, st + ' (COS : vous)')

    def _fiches(self, par, eteints):
        p, feu, g = self.p, self.p.feu, self.p.carte.g
        vx, vy = p.meteo.vers()
        vus = set()
        for r, cells in list(par.items()) + [(r, set()) for r in eteints if r not in par]:
            vus.add(r)
            f = self.feux.get(r)
            if f is None:
                dep = feu.departs[r]
                f = dict(id=r, nom=self._nommer(dep['cell']), t0=dep['t'], cause=dep['cause'], statut='naissant',
                         strategie='', menaces={}, menace_log=set(), touche_log=set(), hist=[], prio=0,
                         detecte=False, t_detect=None, max_cells=0, ha=0, sites_touches=set(), cell0=dep['cell'])
                self.feux[r] = f
            if not f['detecte'] and self._detecte(r):
                f['detecte'] = True
                f['t_detect'] = p.t
            f['cells'] = cells
            f['eteints'] = eteints.get(r, set())
            f['max_cells'] = max(f['max_cells'], len(cells))
            f['ha'] = round(sum(1 for i in range(g.n) if feu.etat[i] != INTACT and self.racine(feu.foyer[i]) == r) * feu.ha_hex, 1)
            # amas : le principal et les sautes
            amas, reste = [], set(cells)
            while reste:
                a = reste.pop()
                pile, grp = [a], {a}
                while pile:
                    k = pile.pop()
                    for j in g.voisins(k):
                        if j in reste:
                            reste.discard(j)
                            grp.add(j)
                            pile.append(j)
                amas.append(grp)
            amas.sort(key=len, reverse=True)
            f['principal'] = amas[0] if amas else set()
            f['sautes'] = set().union(*amas[1:]) if len(amas) > 1 else set()
            # front actif, tête, flancs
            front = []
            for i in cells:
                for j, d, ux, uy in g.vois[i]:
                    if feu.etat[j] == INTACT and COMB[p.carte.fuel[j]]['brule']:
                        front.append(i)
                        break
            f['front'] = front
            if front:
                proj = lambda i: g.cx[i] * vx + g.cy[i] * vy
                f['tete'] = max(front, key=proj)
                pmin, pmax = min(proj(i) for i in front), max(proj(i) for i in front)
                f['flancs'] = [i for i in front if proj(i) < pmax - 0.35 * (pmax - pmin) - 1]
            else:
                f['tete'] = None
                f['flancs'] = []
            # prévision et menaces
            prev = self._prevision(cells) if cells else {}
            f['prev'] = prev
            menaces = {}
            for s in p.carte.sites:
                if s['type'] == 'reserve':
                    continue
                eta = min([prev.get(i, 1e9) for i in s['cells']] + [1e9])
                if eta < 360:
                    menaces[s['id']] = round(eta)
            f['menaces'] = menaces
            # vitesse de la tête (km/h), d'après la prévision autour de la tête
            v = 0.0
            if f['tete'] is not None:
                h = f['tete']
                cand = [prev[j] for j in g.voisins(h) if j in prev and prev[j] > 0]
                if cand:
                    v = g.hex_m / min(cand) * 60 / 1000
            f['vitesse'] = round(min(v, 12), 1)
            f['pc'] = any(u.kind == 'vpc' and u.mission and u.mission.get('type') == 'pc' and u.mission.get('pose', 0) >= 5
                          and self.racine(u.mission.get('feu', -1)) == r for u in p.moyens)
            dern = max([feu.t_ign[i] for i in cells] + [-999])
            # statut
            if not cells and not f['eteints']:
                st = 'éteint'
            elif not cells:
                st = 'maîtrisé'
            elif f['max_cells'] <= 4 and f['ha'] < 5.2 * feu.ha_hex:
                st = 'naissant'
            elif p.t - dern > 25 / g.ech or (f['vitesse'] < 0.25 and len(front) <= 3):
                st = 'fixé'
            else:
                st = 'en progression'
            if st != f['statut']:
                if f['detecte'] and st in ('fixé', 'maîtrisé', 'éteint'):
                    p.journal_ajout('feu', f"{f['nom']} : {st} à {p.meteo.heure()} ({f['ha']:.0f} ha).")
                if st == 'éteint':
                    for sid in list(f['menace_log']):
                        if sid not in f['sites_touches']:
                            p.journal_ajout('sauve', f"{p.carte.sites[sid]['nom']} a été protégé.")
                f['statut'] = st
            # sites touchés
            for s in p.carte.sites:
                if any(feu.etat[i] in (BRULE, BRULE_FINI) and self.racine(feu.foyer[i]) == r for i in s['cells']):
                    if s['id'] not in f['sites_touches']:
                        f['sites_touches'].add(s['id'])
                        p.journal_ajout('alerte', f"⚠ Le feu atteint {s['nom']} !")
            # priorité
            pr = 0.0
            if st == 'naissant':
                pr += 900 if self.doctrine != 'defensive' else 400
            for sid, eta in menaces.items():
                pr += p.carte.sites[sid]['valeur'] * 60 / (eta + 30)
            pr += f['ha'] * 0.4 + len(f['sautes']) * 30
            f['prio'] = pr
            f['hist'].append((p.t, f['ha']))
            f['hist'] = f['hist'][-90:]
        for r, f in self.feux.items():
            if r not in vus and f['statut'] != 'éteint':
                f['cells'], f['eteints'], f['front'], f['flancs'], f['tete'] = set(), set(), [], [], None
                f['statut'] = 'éteint'
                if f['detecte']:
                    p.journal_ajout('feu', f"{f['nom']} : éteint à {p.meteo.heure()} ({f['ha']:.0f} ha).")
                    for sid in list(f['menace_log']):
                        if sid not in f['sites_touches']:
                            p.journal_ajout('sauve', f"{p.carte.sites[sid]['nom']} a été protégé.")

    def _nommer(self, i):
        c = self.p.carte
        s = min(c.sites, key=lambda s: c.g.distance(i, s['centre']))
        nom = s['nom']
        if s['type'] in ('ville', 'village'):
            de = "d'" if nom[0] in 'AEIOUHÉ' else 'de '
            base = f'Feu {de}{nom}'
        else:
            base = f'Feu près de {nom}'
        n = sum(1 for f in self.feux.values() if f['nom'].startswith(base))
        return base + (f' ({n + 1})' if n else '')

    # ------------------------------------------------------------------ missions
    def _liberer(self):
        for u in self.p.moyens:
            m = u.mission
            if u.joueur:
                if m and (m.get('fini') or m.get('bloque', 0) > 3):
                    self.p.journal_ajout('cos', f"{u.nom} : ordre terminé" + (' (position inaccessible)' if m.get('bloque', 0) > 3 else '') + ', retour à l\'IA.')
                    u.mission = None
                    u.joueur = False
                continue
            if not m:
                continue
            f = self.feux.get(self.racine(m['feu'])) if 'feu' in m else None
            if 'feu' in m and (f is None or f['statut'] == 'éteint'):
                u.mission = None
                continue
            if f is not None and m['feu'] != f['id']:
                m['feu'] = f['id']
            if m.get('fini') or m.get('bloque', 0) > 3 or m.get('inactif', 0) > 4:
                u.mission = None
            elif m['type'] == 'surveillance' and f and f['statut'] not in ('maîtrisé', 'fixé'):
                u.mission = None
            elif m['type'] == 'defense' and f and m['site'] not in f['menaces']:
                u.mission = None
            elif m['type'] in ('preposition', 'gaar') and any(x['statut'] != 'éteint' and x['detecte'] for x in self.feux.values()):
                u.mission = None

    def _libres(self, kind, prio, cell):
        """Les moyens disponibles d'un type, du plus proche au plus loin ; un moyen engagé sur un feu bien moins
        prioritaire peut être réaffecté."""
        g = self.p.carte.g
        out = []
        for u in self.p.moyens:
            if u.kind != kind or u.joueur or u.hs:
                continue
            if u.etat == 'au_sol' and not self.p.meteo.vol_possible(kind):
                continue
            m = u.mission
            if m is None or m['type'] in ('preposition', 'gaar'):
                out.append((g.distance(u.cell, cell), u))
            elif 'feu' in m:
                f = self.feux.get(self.racine(m['feu']))
                if f and f['prio'] * 3 < prio and m['type'] != 'defense':
                    out.append((g.distance(u.cell, cell) + 30, u))
        out.sort(key=lambda t: t[0])
        return [u for _, u in out]

    def _engages(self, f, kind, typ=None):
        return [u for u in self.p.moyens if u.kind == kind and not u.joueur and u.mission and self.racine(u.mission.get('feu', -1)) == f['id']
                and (typ is None or u.mission['type'] == typ)]

    def _strategie(self, f, texte):
        if texte != f['strategie']:
            f['strategie'] = texte
            self.p.journal_ajout('cos', f"{f['nom']} — {texte}")

    def _engager(self, f):
        p, feu, g, met = self.p, self.p.feu, self.p.carte.g, self.p.meteo
        c = p.carte
        centre = f['tete'] if f['tete'] is not None else (next(iter(f['cells'])) if f['cells'] else f['cell0'])
        st = f['statut']
        vol = met.vol_possible('canadair')
        parts = []
        # --- surveillance des lisières
        if st in ('maîtrisé',) or (st == 'fixé' and len(f['cells']) <= 2):
            self._strategie(f, 'Surveillance et noyage des lisières (prévenir les reprises)')
            deja = self._engages(f, 'ccf')
            for u in deja:
                if u.mission['type'] != 'surveillance':
                    u.mission = None
            zone = sorted(f['eteints'] | f['cells'], key=lambda i: g.distance(i, centre))
            postes = self._postes_bord(zone, 2)
            for u, poste in zip(self._libres('ccf', f['prio'], centre)[:max(0, 2 - len(self._engages(f, 'ccf', 'surveillance')))], postes):
                u.mission = dict(type='surveillance', feu=f['id'], poste=poste, label='Surveillance des lisières')
            for kind in ('helico', 'canadair', 'dash', 'bull'):
                for u in self._engages(f, kind):
                    u.mission = None
            return
        # --- défense des points sensibles
        n_def = 0
        menaces = sorted(f['menaces'].items(), key=lambda t: t[1])
        for sid, eta in menaces:
            if eta > 150:
                continue
            s = c.sites[sid]
            if sid not in f['menace_log']:
                f['menace_log'].add(sid)
                p.journal_ajout('alerte', f"{s['nom']} menacé : arrivée du feu estimée dans {eta} min.")
            self._population(s, eta, f)
            besoin = {'ville': 4, 'village': 2, 'camping': 3, 'usine': 3, 'patrimoine': 1}.get(s['type'], 2)
            if eta < 60:
                besoin += 1
            if self.doctrine == 'offensive':
                besoin = max(1, besoin - 1)
            elif self.doctrine == 'defensive':
                besoin += 1
            postes = self._postes_defense(f, s)
            # les fourgons pompe-tonnes d'abord (spécialistes des maisons, un FPT compte pour 1,5), puis les CCF
            for kind, poids in (('fpt', 1.5), ('ccf', 1.0)):
                en_place = [u for u in self._engages(f, kind, 'defense') if u.mission.get('site') == sid]
                for k, u in enumerate(en_place):
                    if u.mission['poste'] not in postes and postes:
                        u.mission['poste'] = postes[k % len(postes)]
                besoin -= poids * len(en_place)
                if besoin <= 0 or not postes:
                    continue
                n = int(-(-besoin // poids))
                cand = self._libres(kind, f['prio'] + 200, s['centre'])
                if kind == 'fpt':
                    cand = [u for u in cand if u.chemin_vers(p, postes[0])]
                for k, u in enumerate(cand[:n]):
                    u.mission = dict(type='defense', feu=f['id'], site=sid, poste=postes[(len(en_place) + k) % len(postes)],
                                     label=f"Défense de {s['nom']}")
                    besoin -= poids
            n_def += besoin
        if n_def:
            parts.append('défense des points sensibles')
        # --- poste de commandement : posé en retrait sur une route, dès que le feu dépasse 20 ha
        naissant = st == 'naissant'
        if not naissant and f['ha'] > 20 and not self._engages(f, 'vpc'):
            poste = self._poste_pc(f)
            if poste is not None:
                for u in self._libres('vpc', f['prio'], centre)[:1]:
                    u.mission = dict(type='pc', feu=f['id'], poste=poste, label=f"Poste de commandement de {f['nom']}")
                    p.journal_ajout('cos', f"{u.nom} part installer le poste de commandement de {f['nom']}.")
        # --- attaque au sol : par les flancs, de l'arrière vers la tête (feu naissant : directement)
        nb_att = 4 if naissant else min(10, 2 + len(f['front']) // 2)
        if self.doctrine == 'defensive' and not naissant:
            nb_att = max(2, nb_att // 2)
        att = self._engages(f, 'ccf', 'attaque')
        postes = self._postes_attaque(f, naissant)
        occupes = set()
        for u in att:
            ok = u.mission['poste'] in postes and u.mission['poste'] not in occupes
            if not ok and postes:
                libres = [q for q in postes if q not in occupes]
                if libres:
                    u.mission['poste'] = min(libres, key=lambda q: g.distance(q, u.cell))
                    u.mission['inactif'] = 0
            occupes.add(u.mission['poste'])
        manque = nb_att - len(att)
        if manque > 0:
            libres_postes = [q for q in postes if q not in occupes]
            for u in self._libres('ccf', f['prio'], centre)[:manque]:
                if not libres_postes:
                    break
                q = min(libres_postes, key=lambda q: g.distance(q, u.cell))
                libres_postes.remove(q)
                u.mission = dict(type='attaque', feu=f['id'], poste=q,
                                 label='Attaque du feu naissant' if naissant else 'Attaque de flanc')
        parts.append('attaque des feux naissants' if naissant else 'attaque par les flancs')
        # --- moyens aériens
        if vol:
            nb_can = 2 if naissant else min(8, 2 + int(f['ha'] / 40) + len(menaces))
            nb_hel = 2 if naissant else min(6, 1 + int(f['ha'] / 60) + len(f['sautes']))
            if self.doctrine == 'defensive':
                nb_can = max(1, nb_can - 1)
            for kind, nb in (('canadair', nb_can), ('helico', nb_hel)):
                deja = self._engages(f, kind)
                for u in self._libres(kind, f['prio'], centre)[:max(0, nb - len(deja))]:
                    u.mission = dict(type='largage', feu=f['id'], label='Largages')
                    u.phase = None
                    u.vise = u.cible = None
            if not naissant:
                parts.append('largages sur la tête' if not f['sautes'] else 'largages sur la tête et les sautes')
            # Dash : ligne de retardant devant une tête rapide
            if not naissant and f['vitesse'] >= 0.6 and met.vent >= 18 and f['tete'] is not None:
                if not self._engages(f, 'dash') and p.t - f.get('t_ret', -999) > 25:
                    for u in self._libres('dash', f['prio'], centre)[:1]:
                        u.mission = dict(type='retardant', feu=f['id'], label='Ligne de retardant')
                        u.vise = None
                        f['t_ret'] = p.t
                        p.journal_ajout('cos', f"{u.nom} décolle pour poser une ligne de retardant devant {f['nom']}.")
                parts.append('ligne de retardant')
        # --- bulldozers : pare-feu là où le feu arrivera dans 60 à 150 minutes
        if not naissant and f['tete'] is not None and f['ha'] > 20:
            if not self._engages(f, 'bull'):
                ligne = self._ligne_devant(f, 60, 150, 7)
                if len(ligne) >= 3:
                    for u in self._libres('bull', f['prio'], ligne[0])[:1]:
                        u.mission = dict(type='pare_feu', feu=f['id'], cells=ligne, label='Ouverture d\'un pare-feu')
                        p.journal_ajout('cos', f"{u.nom} ouvre un pare-feu de {len(ligne) * 300} m devant {f['nom']}.")
                    parts.append('pare-feu au bulldozer')
        if naissant:
            txt = 'Attaque massive du feu naissant'
        else:
            txt = ' + '.join(dict.fromkeys(parts)).capitalize()
        if not vol:
            txt += ' (moyens aériens au sol : ' + ('nuit' if met.nuit else 'vent trop fort') + ')'
        self._strategie(f, txt)

    def _population(self, s, eta, f):
        """Évacuer tant qu'il est temps de partir par la route ; sinon, confiner (mise à l'abri sur place).
        On n'évacue pas pour un feu naissant (l'attaque massive doit suffire) ; une ville ne s'évacue qu'en dernier recours."""
        ev = self.p.evac.sites.get(s['id'])
        if not ev or ev['statut'] != 'normal' or s['type'] not in ('ville', 'village', 'camping', 'usine'):
            return
        if f['statut'] == 'naissant' or eta > (100 if s['type'] == 'ville' else 150):
            return
        if self.p.evac.temps_evacuation(s['id']) + 15 < eta:
            self.p.evac.evacuer(s['id'])
        elif eta < 75:
            self.p.evac.confiner(s['id'])

    def _poste_pc(self, f):
        """Une route à 3-7 hexagones des flammes, hors de la prévision du feu pour les 2 h 30 à venir."""
        p, g, c = self.p, self.p.carte.g, self.p.carte
        cells = f['cells']
        if not cells:
            return None
        prev = f.get('prev', {})
        ref = f['cell0']
        best = None
        for i in range(g.n):
            if c.route[i] < 2 or prev.get(i, 1e9) < 150 or p.feu.etat[i] != INTACT and p.feu.etat[i] != BRULE_FINI:
                continue
            d = g.distance(i, ref)
            if d > 12:
                continue
            dmin = min(g.distance(i, k) for k in cells)
            if 3 <= dmin <= 7 and (best is None or dmin < best[0]):
                best = (dmin, i)
        return best[1] if best else None

    # ------------------------------------------------------------------ choix des postes
    def _accessible(self, i):
        p = self.p
        return vitesse_sol('ccf', p.carte, p.feu, i) > 0 and p.feu.etat[i] != BRULE

    def _postes_bord(self, zone, nb):
        g, out = self.p.carte.g, []
        for i in zone:
            for j in [i] + g.voisins(i):
                if self._accessible(j) and j not in out:
                    out.append(j)
                    break
            if len(out) >= nb:
                break
        return out

    def _postes_defense(self, f, s):
        """Les hexagones du site les plus tôt atteints, côté feu, pas en feu."""
        prev, feu = f['prev'], self.p.feu
        cells = [i for i in s['cells'] if feu.etat[i] != BRULE and self._accessible(i)]
        cells.sort(key=lambda i: prev.get(i, 1e9))
        return cells[:3] or [s['centre']]

    def _postes_attaque(self, f, naissant):
        """Postes au contact : à côté d'hexagones en feu d'intensité tenable, sur les flancs et l'arrière ;
        on préfère le noir (déjà brûlé) et les pistes."""
        p, feu, g = self.p, self.p.feu, self.p.carte.g
        vx, vy = p.meteo.vers()
        sources = f['cells'] if naissant else (set(f['flancs']) | f['sautes']) or set(f['front'])
        cand = {}
        for i in sources:
            if feu.I[i] > 0.85 and not naissant:
                continue
            for j in g.voisins(i):
                if not self._accessible(j):
                    continue
                if any(feu.etat[k] == BRULE and feu.I[k] > 0.9 for k in g.voisins(j)):
                    continue
                sc = g.cx[j] * vx + g.cy[j] * vy                 # de l'arrière vers la tête
                if feu.etat[j] in (BRULE_FINI, ETEINT, NOYE):
                    sc -= 400
                if p.carte.route[j]:
                    sc -= 200
                cand[j] = min(cand.get(j, 1e18), sc)
        return [j for j, _ in sorted(cand.items(), key=lambda t: t[1])][:14]

    def _ligne_devant(self, f, tmin, tmax, longueur):
        p, g, c = self.p, self.p.carte.g, self.p.carte
        prev, h = f['prev'], f['tete']
        vx, vy = p.meteo.vers()
        best = None
        for i, t in prev.items():
            if tmin <= t <= tmax:
                d = (g.cx[i] - g.cx[h]) * vx + (g.cy[i] - g.cy[h]) * vy
                lat = abs((g.cx[i] - g.cx[h]) * -vy + (g.cy[i] - g.cy[h]) * vx)
                sc = d - 2 * lat
                if best is None or sc > best[0]:
                    best = (sc, i)
        if best is None:
            return []
        i0 = best[1]
        out = []
        for k in range(-(longueur // 2), longueur // 2 + 1):
            j = g.sous(g.cx[i0] - vy * k * g.hex_m, g.cy[i0] + vx * k * g.hex_m)
            if j is not None and COMB[c.fuel[j]]['brule'] and p.feu.etat[j] == INTACT and c.site_de[j] < 0 and j not in out:
                out.append(j)
        out.sort(key=lambda j: -(g.cx[j] * -vy + g.cy[j] * vx))
        return out

    # ------------------------------------------------------------------ cibles aériennes (appelées par les avions)
    def _ligne_en(self, i0, longueur, axe=None):
        """Une ligne de cellules combustibles centrée sur i0 : le long de l'axe donné (degrés), sinon perpendiculaire au vent."""
        p, g, c = self.p, self.p.carte.g, self.p.carte
        if axe is None:
            vx, vy = p.meteo.vers()
            ux, uy = -vy, vx
        else:
            ux, uy = math.cos(math.radians(axe)), math.sin(math.radians(axe))
        out = []
        for k in range(-(longueur // 2), longueur // 2 + 1):
            j = g.sous(g.cx[i0] + ux * k * g.hex_m, g.cy[i0] + uy * k * g.hex_m)
            if j is not None and COMB[c.fuel[j]]['brule'] and p.feu.etat[j] != BRULE and j not in out:
                out.append(j)
        return out

    def cible_largage(self, u):
        p, feu, g = self.p, self.p.feu, self.p.carte.g
        m = u.mission
        if m and m.get('cible') is not None:
            # largage demandé par le joueur : la flamme la plus proche du point visé, sinon le point lui-même (mouiller)
            ci = m['cible']
            proches = [j for j in g.anneau(ci, 2) if feu.etat[j] == BRULE]
            cible = min(proches, key=lambda j: g.distance(j, ci)) if proches else ci
            if not COMB[p.carte.fuel[cible]]['brule']:
                return None
            if u.kind == 'canadair':
                l = self._ligne_en(cible, 3, m.get('axe'))
                return sorted(l, key=lambda j: g.distance(j, cible))[:2] or [cible]
            return [cible]
        if not m or 'feu' not in m:
            return None
        f = self.feux.get(self.racine(m['feu']))
        if not f or not f['cells']:
            return None
        cells = [i for i in f['cells'] if feu.etat[i] == BRULE]
        if not cells:
            return None
        vx, vy = p.meteo.vers()
        # déjà visés par d'autres avions : on répartit
        vises = set()
        for o in p.moyens:
            if o is not u and o.vise:
                vises.update(o.vise)
        sautes = [i for i in f['sautes'] if feu.etat[i] == BRULE]
        prev = f.get('prev', {})
        # le point le plus menaçant : la flamme la plus proche (en temps) d'un site menacé
        menace = None
        if f['menaces']:
            sid, eta = min(f['menaces'].items(), key=lambda t: t[1])
            if eta < 120:
                s = p.carte.sites[sid]
                menace = min(cells, key=lambda i: min(g.distance(i, k) for k in s['cells']))
        if sautes and (u.kind == 'helico' or len(sautes) <= 3):
            cible = min(sautes, key=lambda i: g.distance(i, u.cell))
        elif menace is not None and (u.kind == 'canadair' or self.doctrine == 'defensive'):
            cible = menace
        else:
            front = [i for i in f['front'] if feu.etat[i] == BRULE] or cells
            proj = lambda i: g.cx[i] * vx + g.cy[i] * vy
            front.sort(key=lambda i: (i in vises, -proj(i) - feu.I[i] * 200))
            cible = front[0] if u.kind == 'canadair' or self.doctrine == 'offensive' else \
                max(front[:6], key=lambda i: feu.I[i] if i not in vises else -1)
        out = [cible]
        if u.kind == 'canadair':
            # un Canadair largue en ligne, le long du front (perpendiculaire au vent)
            for s in (1, -1):
                j = g.sous(g.cx[cible] - vy * s * g.hex_m, g.cy[cible] + vx * s * g.hex_m)
                if j is not None and j != cible and COMB[p.carte.fuel[j]]['brule'] and feu.etat[j] in (BRULE, INTACT):
                    out.append(j)
                    break
        return out

    def ligne_retardant(self, u):
        m = u.mission
        if m and m.get('cible') is not None:
            return self._ligne_en(m['cible'], 5, m.get('axe'))
        f = self.feux.get(self.racine(m['feu'])) if m else None
        if not f or f['tete'] is None:
            return []
        return self._ligne_devant(f, 20, 70, 5)

    # ------------------------------------------------------------------ renforts et prévention
    def _renforts(self):
        p = self.p
        total = sum(f['ha'] for f in self.feux.values() if f['statut'] != 'éteint' and f['detecte'])
        seuils = [120, 450, 1200]
        if self.renforts < len(seuils) and total > seuils[self.renforts]:
            self.renforts += 1
            p.demander_renfort(4)

    def _debroussaillage(self, feux_actifs):
        """Obligations légales de débroussaillement : hors risque sévère, les débroussailleuses dégagent les abords
        des villages et des campings. Par risque sévère, les travaux mécaniques en forêt sont interdits (étincelles)."""
        p, met, c, g = self.p, self.p.meteo, self.p.carte, self.p.carte.g
        engins = [u for u in p.moyens if u.kind == 'debr' and not u.joueur and not u.hs]
        if not engins:
            return
        interdit = met.danger >= 0.55 or met.nuit or feux_actifs
        if interdit:
            for u in engins:
                if u.mission and u.mission.get('type') == 'debroussailler':
                    u.mission = None
            if met.danger >= 0.55 and not met.nuit and getattr(self, '_interdit_jour', None) != met.jour():
                self._interdit_jour = met.jour()
                p.journal_ajout('info', 'Risque sévère : travaux mécaniques interdits en forêt, les débroussailleuses restent au dépôt.')
            return
        faits = {i for u in engins if u.mission for i in u.mission.get('cells', [])}
        for u in engins:
            if u.mission:
                continue
            cand = []
            for s in c.sites:
                if s['type'] not in ('ville', 'village', 'camping', 'usine'):
                    continue
                for i in g.anneau(s['centre'], 3):
                    if c.fuel[i] in 'PTJGQ' and c.site_de[i] < 0 and i not in faits and p.feu.etat[i] == INTACT:
                        cand.append(i)
            if not cand:
                return
            cand.sort(key=lambda i: g.distance(i, u.cell))
            lot_ = cand[:5]
            faits.update(lot_)
            u.mission = dict(type='debroussailler', cells=lot_, label='Débroussaillement autour des habitations')

    def _prevention(self, feux_actifs):
        p, met, c, g = self.p, self.p.meteo, self.p.carte, self.p.carte.g
        self._debroussaillage(feux_actifs)
        if feux_actifs:
            self.prepos = self.gaar = False
            return
        haut = met.danger >= 0.55 and not met.nuit
        if haut and not self.prepos:
            self.prepos = True
            croix = [i for i in range(g.n) if c.route[i] == 1 and c.fuel[i] in 'PJQGF']
            ccf = [u for u in p.moyens if u.kind == 'ccf' and u.mission is None]
            nb = len(ccf) // 3
            if croix and nb:
                pts = []
                for _ in range(nb):
                    best = max(croix, key=lambda i: min([g.distance(i, q) for q in pts] +
                                                         [g.distance(i, k['cell']) for k in c.casernes] + [99]))
                    pts.append(best)
                for u, q in zip(ccf, pts):
                    u.mission = dict(type='preposition', poste=q, label='Patrouille préventive')
                p.journal_ajout('cos', f"Risque {met.niveau.lower()} : {nb} CCF pré-positionnés dans le massif.")
        if met.danger >= 0.7 and met.vol_possible('canadair') and not self.gaar:
            self.gaar = True
            libres = [u for u in p.moyens if u.kind == 'canadair' and u.mission is None][:2]
            for u in libres:
                u.mission = dict(type='gaar', label='Guet aérien armé', pts=self._circuit())
                u.phase = None
            if libres:
                p.journal_ajout('cos', f"Guet aérien armé : {len(libres)} Canadair patrouillent, chargés d'eau.")
        if not haut:
            self.prepos = False
            for u in p.moyens:
                if u.mission and u.mission['type'] in ('preposition', 'gaar'):
                    u.mission = None

    def _circuit(self):
        g = self.p.carte.g
        W, H = g.largeur, g.hauteur
        return [(W * 0.35, H * 0.3), (W * 0.7, H * 0.35), (W * 0.65, H * 0.7), (W * 0.3, H * 0.65)]

    # ------------------------------------------------------------------ export
    def export(self):
        out = []
        p = self.p
        for f in self.feux.values():
            if f.get('absorbe') is not None:
                continue
            if f['statut'] == 'éteint' and p.t - f['hist'][-1][0] > 120 if f['hist'] else False:
                continue
            g = p.carte.g
            moy = {}
            for u in p.moyens:
                if u.mission and self.racine(u.mission.get('feu', -1)) == f['id']:
                    moy[u.kind] = moy.get(u.kind, 0) + 1
            t = f['tete']
            out.append(dict(id=f['id'], nom=f['nom'], statut=f['statut'] if f['detecte'] else 'non détecté', ha=f['ha'],
                            n=len(f['cells']), tete=t, vitesse=f['vitesse'], strategie=f['strategie'], prio=round(f['prio']),
                            menaces=[dict(site=k, eta=v) for k, v in sorted(f['menaces'].items(), key=lambda kv: kv[1])],
                            moyens=moy, detecte=f['detecte'], t0=f['t0'], tDetect=f['t_detect'], cause=f['cause'],
                            sautes=len(f['sautes']), front=f['front'][:80], flancs=f['flancs'][:60],
                            touches=sorted(f['sites_touches']), cell0=f['cell0'], pc=bool(f.get('pc')),
                            prev=[[i, round(t)] for i, t in f.get('prev', {}).items() if t <= 180][:2500] if f.get('pc') else None))
        return out
