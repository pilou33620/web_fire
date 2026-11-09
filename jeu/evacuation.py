"""Les populations : évacuation par la route ou confinement (mise à l'abri sur place).

- La fumée est calculée chaque minute sous le vent des flammes ; elle ralentit la circulation.
- Une route est coupée quand elle brûle ou longe un feu intense : les convois cherchent un autre chemin.
- Les convois (300 personnes, une centaine de voitures) partent toutes les 3 minutes du site évacué et roulent vers
  un point de regroupement sûr (une ville hors de danger, ou une sortie de la carte). Plusieurs convois sur le même
  tronçon : embouteillage.
- Un convoi bloqué près des flammes, ou des habitants confinés dans un site qui brûle presque entièrement,
  comptent comme « personnes exposées ». La doctrine : évacuer tôt, confiner quand il est trop tard pour partir.
"""
import math

from .feu import BRULE
from .hexa import astar

TAILLE_CONVOI = 300
CADENCE = 3            # minutes entre deux départs de convoi


class Convoi:
    def __init__(self, cid, site, n, cell, carte):
        self.id, self.site, self.n, self.cell = cid, site, n, cell
        self.x, self.y = carte.g.cx[cell], carte.g.cy[cell]
        self.chemin, self.avance, self.etat, self.dest = [], 0.0, 'route', None
        self.cap = 0.0
        self.expose = False
        self.attente = 0
        self.repli = False               # demi-tour vers son village : on s'y mettra à l'abri
        self.signale = False


class Evacuation:
    def __init__(self, p):
        self.p = p
        c = p.carte
        self.sites = {}
        for s in c.sites:
            if s['pop'] > 0 and s['type'] != 'reserve':
                self.sites[s['id']] = dict(statut='normal', pop=s['pop'], reste=s['pop'], route=0, arrives=0, confines=0,
                                           exposes=0, t=None, par=None, dest=None)
        self.convois = []
        self.n = 0
        self.fumee = {}
        self.coupees = set()
        self.menace = {}                 # cellule -> minutes avant l'arrivée prévue du feu (tous feux)
        g = c.g
        # sorties de carte : les routes qui touchent un bord
        self.sorties = [i for i in range(g.n) if c.route[i] >= 2 and (g.col_row(i)[0] in (0, g.cols - 1) or g.col_row(i)[1] in (0, g.rows - 1))]
        if c.aerodrome is not None and not self.sorties:
            self.sorties = [c.aerodrome]

    # ------------------------------------------------------------------ décisions
    def evacuer(self, sid, par='COS'):
        s = self.sites.get(sid)
        if not s or s['statut'] in ('evacuation', 'evacue'):
            return False
        dest = self._destination(sid)
        if dest is None:
            return False
        # des habitants déjà confinés peuvent partir
        s['reste'] += s['confines']
        s['confines'] = 0
        s['statut'], s['t'], s['par'], s['dest'] = 'evacuation', self.p.t, par, dest
        nom = self.p.carte.sites[sid]['nom']
        self.p.journal_ajout('cos', f"Évacuation de {nom} ({s['reste']} personnes) vers {self._nom_dest(dest)}" + (' — ordre du COS (vous).' if par == 'joueur' else '.'))
        self.p.evacues.add(sid)
        return True

    def confiner(self, sid, par='COS'):
        s = self.sites.get(sid)
        if not s or s['statut'] in ('confinement', 'evacue'):
            return False
        s['confines'] += s['reste']
        s['reste'] = 0
        s['statut'], s['t'], s['par'] = 'confinement', self.p.t, par
        nom = self.p.carte.sites[sid]['nom']
        self.p.journal_ajout('cos', f"Confinement à {nom} : les {s['confines']} personnes restent à l'abri dans les bâtiments" +
                             (' — ordre du COS (vous).' if par == 'joueur' else ' (trop tard pour partir par la route).'))
        return True

    def temps_evacuation(self, sid):
        """Minutes pour vider le site : départs échelonnés + trajet vers le point de regroupement."""
        s = self.sites.get(sid)
        if not s:
            return 1e9
        dest = self._destination(sid)
        if dest is None:
            return 1e9
        site = self.p.carte.sites[sid]
        _, t = astar(self.p.carte.g, site['centre'], dest, self._cout(), 0.6 / self.p.carte.g.ech)
        return (math.ceil(s['reste'] / TAILLE_CONVOI) - 1) * CADENCE + min(t, 1e9) * 0.5

    # ------------------------------------------------------------------ routes
    def _vitesse(self, i):
        r = self.p.carte.route[i]
        v = [0.35, 0.6, 1.0, 1.5][r]
        f = self.fumee.get(i, 0)
        return v * max(0.25, 1 - f) * self.p.carte.g.ech

    def _cout(self):
        c = self.p.carte
        menace = self.menace

        def cout(i, j):
            if j in self.coupees:
                return None
            t = menace.get(j, 1e9)
            if t < 40:
                return None                # on n'envoie pas un convoi là où le feu sera dans moins de 40 min
            pen = 4.0 if t < 120 else 0.0
            if c.route[j] < 2 and c.site_de[j] < 0:
                return None                # pas de convoi de civils sur les pistes forestières
            if c.fuel[j] in 'ML' or (c.fuel[j] == 'R' and not c.route[j]):
                return None
            return 1.0 / self._vitesse(j) + pen
        return cout

    def _destination(self, sid):
        """Un point de regroupement sûr : une ville hors de la prévision du feu, sinon une sortie de la carte."""
        p, c, g = self.p, self.p.carte, self.p.carte.g
        site = c.sites[sid]
        self._calcul_menace()
        menace = self.menace
        cands = []
        for s in c.sites:
            if s['type'] == 'ville' and s['id'] != sid and min(menace.get(i, 1e9) for i in s['cells']) > 240:
                cands.append(s['centre'])
        cands += [i for i in self.sorties if menace.get(i, 1e9) > 240]
        if not cands:
            return None
        cands.sort(key=lambda i: g.distance(i, site['centre']))
        cout = self._cout()
        for d in cands[:4]:
            ch, t = astar(g, site['centre'], d, cout, 0.6 / g.ech)
            if ch:
                return d
        return None

    def _calcul_menace(self):
        menace = {}
        for f in self.p.ia.feux.values():
            if f['statut'] == 'éteint':
                continue
            for i, t in f.get('prev', {}).items():
                if t < menace.get(i, 1e9):
                    menace[i] = t
        self.menace = menace

    def _nom_dest(self, d):
        c = self.p.carte
        if c.site_de[d] >= 0:
            return c.sites[c.site_de[d]]['nom']
        return 'la sortie du massif'

    # ------------------------------------------------------------------ chaque minute
    def pas(self):
        p, c, g, feu = self.p, self.p.carte, self.p.carte.g, self.p.feu
        self._fumee()
        if p.t % 2 == 0:
            self._calcul_menace()
        # routes coupées : en feu, ou le long d'un feu intense
        self.coupees = set()
        for i in feu.brulant:
            if c.route[i] or c.site_de[i] >= 0:
                self.coupees.add(i)
            if feu.I[i] > 0.55:
                for j in g.voisins(i):
                    if c.route[j]:
                        self.coupees.add(j)
        # départs des convois
        for sid, s in self.sites.items():
            if s['statut'] == 'evacuation' and s['reste'] > 0 and (p.t - s['t']) % CADENCE == 0:
                n = min(TAILLE_CONVOI, s['reste'])
                s['reste'] -= n
                s['route'] += n
                cv = Convoi(self.n, sid, n, c.sites[sid]['centre'], c)
                self.n += 1
                cv.dest = s['dest']
                self.convois.append(cv)
            if s['statut'] == 'evacuation' and s['reste'] == 0 and s['route'] == 0:
                if s['confines']:
                    s['statut'] = 'confinement'
                    p.journal_ajout('cos', f"{c.sites[sid]['nom']} : {s['arrives']} personnes évacuées, {s['confines']} revenues se mettre à l'abri.")
                else:
                    s['statut'] = 'evacue'
                    if s['arrives']:
                        p.journal_ajout('sauve', f"{c.sites[sid]['nom']} est entièrement évacué ({s['arrives']} personnes en sécurité).")
            # évacuation en cours mais le feu arrive sur le village : ceux qui restent se mettent à l'abri
            if s['statut'] == 'evacuation' and s['reste'] and self.menace.get(c.sites[sid]['centre'], 1e9) < 25:
                s['confines'] += s['reste']
                p.journal_ajout('cos', f"{c.sites[sid]['nom']} : le feu est trop près, les {s['reste']} personnes restantes se mettent à l'abri sur place.")
                s['reste'] = 0
            # confinés dans un site qui brûle presque entièrement : exposés
            if s['confines'] and s['statut'] == 'confinement':
                cells = c.sites[sid]['cells']
                brule = sum(1 for i in cells if feu.etat[i] in (BRULE, 2)) / max(1, len(cells))
                if brule > 0.6:
                    s['exposes'] += s['confines']
                    p.ajouter_blesses(c.sites[sid]['centre'], max(1, round(s['confines'] * 0.03)), 'civil', 'grave', f"habitants de {c.sites[sid]['nom']}")
                    p.journal_ajout('alerte', f"⚠ {s['confines']} personnes confinées à {c.sites[sid]['nom']} sont exposées : le site brûle.")
                    s['confines'] = 0
        # circulation
        par_cell = {}
        for cv in self.convois:
            if cv.etat == 'route':
                par_cell[cv.cell] = par_cell.get(cv.cell, 0) + 1
        cout = self._cout()
        for cv in list(self.convois):
            if cv.etat == 'arrive':
                continue
            s = self.sites[cv.site]
            # danger : le feu sur le convoi
            danger = feu.etat[cv.cell] == BRULE or any(feu.etat[j] == BRULE and feu.I[j] > 0.6 for j in g.voisins(cv.cell))
            if danger and not cv.expose:
                cv.expose = True
                s['exposes'] += cv.n
                p.ajouter_blesses(cv.cell, max(1, round(cv.n * 0.02)), 'civil', 'grave', f"convoi de {c.sites[cv.site]['nom']}")
                p.ajouter_blesses(cv.cell, round(cv.n * 0.05), 'civil', 'leger', f"convoi de {c.sites[cv.site]['nom']}")
                p.journal_ajout('alerte', f"⚠ Un convoi de {cv.n} personnes parti de {c.sites[cv.site]['nom']} est pris par le feu !")
            if not cv.chemin or cv.chemin[0] in self.coupees or (p.t + cv.id) % 6 == 0:
                if not cv.repli and (cv.dest is None or cv.dest in self.coupees or self.menace.get(cv.dest, 1e9) < 60):
                    cv.dest = self._destination(cv.site) or cv.dest
                ch, _ = astar(g, cv.cell, cv.dest, cout, 0.6 / g.ech) if cv.dest is not None else (None, 0)
                if not ch and not cv.repli:
                    # plus d'itinéraire sûr : demi-tour vers le village, où l'on se mettra à l'abri
                    origine = c.sites[cv.site]['centre']
                    ch, _ = astar(g, cv.cell, origine, lambda i, j: None if j in self.coupees or (c.route[j] < 2 and c.site_de[j] < 0) else 1.0, 0.6)
                    if ch:
                        cv.repli, cv.dest = True, origine
                        p.journal_ajout('alerte', f"Convoi de {c.sites[cv.site]['nom']} : route coupée, demi-tour et mise à l'abri au village.")
                if not ch:
                    if cv.etat != 'bloque':
                        cv.etat = 'bloque'
                        if not cv.signale:
                            cv.signale = True
                            p.journal_ajout('alerte', f"Convoi de {c.sites[cv.site]['nom']} bloqué : route coupée.")
                    continue
                cv.chemin, cv.avance, cv.etat = ch[1:], 0.0, 'route'
            if not cv.chemin:
                self._arrivee(cv)
                continue
            budget = 1.0
            while budget > 0 and cv.chemin:
                j = cv.chemin[0]
                v = self._vitesse(j) / max(1.0, 0.7 * par_cell.get(j, 0))
                reste = (1 - cv.avance) / v
                if reste <= budget:
                    budget -= reste
                    cv.cell = j
                    cv.chemin.pop(0)
                    cv.avance = 0.0
                else:
                    cv.avance += budget * v
                    budget = 0
            if cv.chemin:
                j = cv.chemin[0]
                x0, y0, x1, y1 = g.cx[cv.cell], g.cy[cv.cell], g.cx[j], g.cy[j]
                cv.x, cv.y = x0 + (x1 - x0) * cv.avance, y0 + (y1 - y0) * cv.avance
                cv.cap = math.degrees(math.atan2(y1 - y0, x1 - x0))
            else:
                cv.x, cv.y = g.cx[cv.cell], g.cy[cv.cell]
                self._arrivee(cv)

    def _arrivee(self, cv):
        s = self.sites[cv.site]
        cv.etat = 'arrive'
        s['route'] -= cv.n
        if cv.repli and not cv.expose:
            s['confines'] += cv.n
        elif not cv.expose:
            s['arrives'] += cv.n
        self.convois.remove(cv)

    def _fumee(self):
        """Panache de fumée sous le vent de chaque hexagone en feu."""
        p, g, feu = self.p, self.p.carte.g, self.p.feu
        vx, vy = p.meteo.vers()
        V = p.meteo.vent
        portee = 2 + V / 12
        fum = {}
        for i in feu.brulant:
            I = feu.I[i]
            if I < 0.2:
                continue
            for k in range(1, int(portee) + 1):
                for lat in (-1, 0, 1):
                    if lat and k < 2:
                        continue
                    x = g.cx[i] + (vx * k - vy * lat * k * 0.35) * g.hex_m
                    y = g.cy[i] + (vy * k + vx * lat * k * 0.35) * g.hex_m
                    j = g.sous(x, y)
                    if j is not None:
                        fum[j] = min(1.0, fum.get(j, 0) + I * (1 - k / (portee + 1)) * (0.6 if lat else 1))
            fum[i] = 1.0
        self.fumee = fum

    # ------------------------------------------------------------------ bilan
    def bilan(self):
        t = dict(evacues=0, confines=0, exposes=0, route=0)
        for s in self.sites.values():
            t['evacues'] += s['arrives']
            t['confines'] += s['confines']
            t['exposes'] += s['exposes']
            t['route'] += s['route']
        return t

    def export(self):
        c = self.p.carte
        return dict(
            convois=[dict(id=cv.id, x=round(cv.x), y=round(cv.y), cap=round(cv.cap), n=cv.n, e=cv.etat, s=cv.site) for cv in self.convois],
            sites={sid: dict(statut=s['statut'], pop=s['pop'], reste=s['reste'], route=s['route'], arrives=s['arrives'],
                             confines=s['confines'], exposes=s['exposes'], par=s['par']) for sid, s in self.sites.items()},
            coupees=sorted(i for i in self.coupees if c.route[i]),
            fumee=[[i, round(d * 100)] for i, d in self.fumee.items() if d > 0.25 and c.route[i]],
        )
