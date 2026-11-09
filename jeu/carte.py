"""La carte : toute la Gironde, d'après la carte de web_conquete (jeu/gironde_data.py, vraie géographie en cases de 1 km),
en hexagones de 1 km : combustibles, relief, eaux, villes et villages, sites à protéger, routes, pistes DFCI, moyens fixes.
"""
import random

from . import gironde_data as GD
from .combustibles import COMB, EAU, VOIES
import math as _m

from .hexa import Grille, astar_virages, simplifier

VALEUR = {'ville': 100, 'village': 50, 'camping': 70, 'usine': 60, 'reserve': 25, 'patrimoine': 30}


class Carte:
    def __init__(self, graine=1, theme='gironde', hex_m=1000):
        self.graine, self.theme = graine, theme
        R = hex_m / _m.sqrt(3)
        self.g = Grille(round(GD.W * 1000 / hex_m), int((GD.H * 1000 - 2 * R) / (1.5 * R)) + 1, hex_m)
        self.rnd = random.Random(graine * 7919)
        n = self.g.n
        self.fuel = ['P'] * n
        self.alt = [40.0] * n
        self.route = [0] * n
        self.site_de = [-1] * n          # cellule -> site
        self.corps = [-1] * n            # cellule d'eau -> plan d'eau
        self.corps_taille = []
        self.corps_nom = []
        self.sites, self.casernes, self.vigies, self.points_eau = [], [], [], []
        self.helistation = self.aerodrome = None
        self.routes_poly = []
        self._gironde()
        self._plans_eau()
        self._finaliser_routes()

    # ------------------------------------------------------------------ outils
    def x(self, i):
        return self.g.cx[i]

    def y(self, i):
        return self.g.cy[i]

    def libre(self, i):
        return self.fuel[i] not in EAU and self.fuel[i] not in 'SA' and self.site_de[i] < 0

    def _poser_site(self, typ, nom, centre, rayon, fuel, part=1.0):
        sid = len(self.sites)
        cells = []
        for j in self.g.anneau(centre, rayon):
            if self.fuel[j] in EAU or self.site_de[j] >= 0:
                continue
            if j != centre and self.rnd.random() > part:
                continue
            if fuel:
                self.fuel[j] = fuel
            self.site_de[j] = sid
            cells.append(j)
        pop = {'ville': 6000, 'village': 600, 'camping': 1500, 'usine': 80, 'reserve': 0, 'patrimoine': 10}[typ]
        self.sites.append(dict(id=sid, type=typ, nom=nom, centre=centre, cells=cells, valeur=VALEUR[typ],
                               pop=int(pop * self.rnd.uniform(0.6, 1.5))))
        return sid

    def _cell(self, xy):
        """La cellule de terre la plus proche d'un point (km de gironde_data)."""
        c = self.g.sous(xy[0] * 1000, xy[1] * 1000)
        if c is None:
            return None
        terre = [j for j in self.g.anneau(c, 3) if self.fuel[j] not in EAU]
        return min(terre, key=lambda j: self.g.distance(c, j)) if terre and self.fuel[c] in EAU else c

    # ------------------------------------------------------------------ la Gironde
    def _gironde(self):
        g = self.g
        for i in range(g.n):
            x, y = min(GD.W - 1, int(g.cx[i] / 1000)), min(GD.H - 1, int(g.cy[i] / 1000))
            self.fuel[i] = GD.FUEL[y][x]
            self.alt[i] = (ord(GD.ALT[y][x]) - 48) * 4.0
        # villes et villages (la préfecture d'abord : sa caserne est le CSP, son hôpital reçoit les blessés)
        for l in sorted(GD.LIEUX, key=lambda l: l['type'] != 'capitale'):
            c = self._cell(l['xy'])
            if c is None or self.site_de[c] >= 0:
                continue
            if l['type'] == 'capitale':
                sid = self._poser_site('ville', l['nom'], c, 3, 'U', 0.9)
                self.sites[sid]['pop'] *= 5
            elif l['type'] == 'ville':
                sid = self._poser_site('ville', l['nom'], c, 1, 'U', 0.8)
            else:
                sid = self._poser_site('village' if l['type'] == 'village' else 'patrimoine', l['nom'], c, 0, 'B')
            if l['d33'] and l['type'] in ('capitale', 'ville'):
                self.casernes.append(dict(id=len(self.casernes), nom=('CSP ' if l['type'] == 'capitale' else 'CIS ') + l['nom'], cell=c))
        for s in GD.SITES:
            c = self._cell(s['xy'])
            if c is None or self.site_de[c] >= 0:
                continue
            fuel = dict(camping='T', usine='Z', patrimoine='B').get(s['type'])
            self._poser_site(s['type'], s['nom'], c, 1 if s['type'] == 'reserve' else 0,
                             None if s['type'] == 'reserve' or self.fuel[c] in 'DS' else fuel, 0.9)
        # les routes de web_conquete, posées sur les hexagones (coupées aux plans d'eau : pas de bacs)
        for r in GD.ROUTES:
            ch = []
            for (x0, y0), (x1, y1) in zip(r['p'], r['p'][1:]):
                n = max(1, int(_m.hypot(x1 - x0, y1 - y0) / 0.3))
                for k in range(n + 1):
                    c = g.sous((x0 + (x1 - x0) * k / n) * 1000, (y0 + (y1 - y0) * k / n) * 1000)
                    if c is not None and (not ch or ch[-1] != c):
                        if ch and c not in g.voisins(ch[-1]):
                            ch.extend(g.ligne(ch[-1], c)[1:-1])
                        ch.append(c)
            run = []
            for c in ch + [None]:
                if c is not None and self.fuel[c] not in 'ML':
                    run.append(c)
                    continue
                if len(run) > 1:
                    for i in run:
                        self.route[i] = max(self.route[i], r['n'])
                    self.routes_poly.append(dict(n=r['n'], c=run))
                run = []
        self._pistes_dfci()
        # une petite route vers chaque site qui n'en a pas
        cout = lambda i, j: None if self.fuel[j] in 'ML' else (0.3 if self.route[j] else 1.0)
        for s in self.sites:
            if not any(self.route[i] for i in s['cells']):
                cible = [i for i in g.anneau(s['centre'], 12) if self.route[i]]
                if cible:
                    ch, _ = astar_virages(g, s['centre'], min(cible, key=lambda i: g.distance(i, s['centre'])), cout, 0.3)
                    if ch:
                        for i in ch:
                            self.route[i] = max(self.route[i], 2)
                        self.routes_poly.append(dict(n=2, c=ch))
        self._moyens_fixes(GD.AERODROME)
        self.riviere, self.nom_riviere = [], ''

    def _pistes_dfci(self):
        """Pistes DFCI : un quadrillage dans les massifs, qui sert de coupure et d'accès aux camions."""
        g = self.g
        foret = set('PJFQG')
        pas_c, pas_r = 6, 5
        ok = lambda i: self.fuel[i] in foret and self.site_de[i] < 0
        for r0 in range(3, g.rows - 1, pas_r):
            run = []
            for col in range(g.cols):
                i = r0 * g.cols + col
                if ok(i) or (run and self.route[i]):
                    run.append(i)
                else:
                    if len(run) > 3:
                        self._poser_piste(run)
                    run = []
            if len(run) > 3:
                self._poser_piste(run)
        for c0 in range(4, g.cols - 1, pas_c):
            run = []
            for row in range(g.rows):
                col = c0
                i = row * g.cols + col
                if ok(i) or (run and self.route[i]):
                    run.append(i)
                else:
                    if len(run) > 3:
                        self._poser_piste(run)
                    run = []
            if len(run) > 3:
                self._poser_piste(run)

    def _poser_piste(self, run):
        for i in run:
            if self.route[i] == 0:
                self.route[i] = 1
        self.routes_poly.append(dict(n=1, c=run))

    # ------------------------------------------------------------------ casernes, hélistation, aérodrome, vigies, points d'eau
    def _moyens_fixes(self, aero):
        g = self.g
        villes = [s for s in self.sites if s['type'] == 'ville']
        # aérodrome : à sa vraie place, relié à la préfecture
        nom_aero, a = aero[0], self._cell(aero[1])
        if a is not None and self.site_de[a] >= 0:
            a = min((j for j in g.anneau(a, 3) if self.site_de[j] < 0 and self.fuel[j] not in EAU), key=lambda j: g.distance(a, j), default=a)
        if a is not None:
            ref = villes[0]['centre'] if villes else a
            self.aerodrome = a
            for j in g.anneau(a, 1):
                if self.site_de[j] < 0 and self.fuel[j] not in EAU:
                    self.fuel[j] = 'A'
            ch, _ = astar_virages(g, a, ref, lambda i, j: None if self.fuel[j] in 'ML' else (0.3 if self.route[j] >= 2 else 1.0), 0.3)
            if ch:
                for i in ch:
                    self.route[i] = max(self.route[i], 2)
                self.routes_poly.append(dict(n=2, c=ch))
            self.nom_aerodrome = nom_aero
        # hélistation DFCI : sur une piste, au centre du massif forestier
        c = [i for i in range(g.n) if self.route[i] >= 1 and self.site_de[i] < 0 and self.fuel[i] not in EAU]
        pins = [i for i in range(g.n) if self.fuel[i] == 'P']
        if c and pins:
            mid = g.sous(sum(g.cx[i] for i in pins) / len(pins), sum(g.cy[i] for i in pins) / len(pins))
            self.helistation = min(c, key=lambda i: g.distance(i, mid))
            self.fuel[self.helistation] = 'X'
        # vigies : sur des points hauts, bien réparties
        foret = [i for i in range(g.n) if self.fuel[i] in 'PJFQG']
        for _ in range(max(4, len(foret) // 700)):
            best = None
            for i in self.rnd.sample(foret, min(300, len(foret))):
                d = min([g.distance(i, v) for v in self.vigies] + [99])
                sc = self.alt[i] / 50 + min(d, 25)
                if d >= 12 and (best is None or sc > best[0]):
                    best = (sc, i)
            if best:
                self.vigies.append(best[1])
        # points d'eau : bornes dans les lieux habités, citernes DFCI aux croisements de pistes
        for s in self.sites:
            if s['type'] in ('ville', 'village', 'camping', 'usine'):
                self.points_eau.append(s['centre'])
        croix = [i for i in range(g.n) if self.route[i] == 1 and sum(1 for j in g.voisins(i) if self.route[j]) >= 3]
        self.rnd.shuffle(croix)
        for i in croix:
            if all(g.distance(i, p) >= max(2, round(6 * g.ech)) for p in self.points_eau):
                self.points_eau.append(i)
        if self.helistation is not None:
            self.points_eau.append(self.helistation)

    def _finaliser_routes(self):
        """Nettoie le réseau pour le dessin : chaque tronçon n'est tracé qu'une fois (au niveau le plus élevé), puis
        les escaliers de cellules deviennent des lignes droites et des courbes (sans couper par l'eau)."""
        g = self.g
        faits = set()
        propres = []
        for r in sorted(self.routes_poly, key=lambda r: -r['n']):
            run = []
            for a, b in zip(r['c'], r['c'][1:]):
                e = (min(a, b), max(a, b))
                if e in faits or (r['n'] == 1 and self.route[a] >= 2 and self.route[b] >= 2):
                    if len(run) > 1:
                        propres.append(dict(n=r['n'], c=run))
                    run = []
                    continue
                faits.add(e)
                if not run:
                    run = [a]
                run.append(b)
            if len(run) > 1:
                propres.append(dict(n=r['n'], c=run))

        def valide(a, b):
            n = max(2, int(_m.hypot(b[0] - a[0], b[1] - a[1]) / (g.hex_m / 3)))
            for k in range(1, n):
                i = g.sous(a[0] + (b[0] - a[0]) * k / n, a[1] + (b[1] - a[1]) * k / n)
                if i is None or self.fuel[i] in 'ML' or (self.fuel[i] == 'R' and not self.route[i]):
                    return False
            return True
        for r in propres:
            pts = [(g.cx[i], g.cy[i]) for i in r['c']]
            tol = g.hex_m * (0.42 if r['n'] == 1 else 0.36)
            r['p'] = [[round(x), round(y)] for x, y in simplifier(pts, tol, valide)]
        self.routes_poly = propres

    def _plans_eau(self):
        """Repère les plans d'eau (pour l'écopage des Canadair : il en faut un grand)."""
        g = self.g
        for i in range(g.n):
            if self.fuel[i] in 'ML' and self.corps[i] < 0:
                cid = len(self.corps_taille)
                pile, n = [i], 0
                self.corps[i] = cid
                while pile:
                    k = pile.pop()
                    n += 1
                    for j in g.voisins(k):
                        if self.fuel[j] == self.fuel[i] and self.corps[j] < 0:
                            self.corps[j] = cid
                            pile.append(j)
                self.corps_taille.append(n)
                self.corps_nom.append('Océan' if self.fuel[i] == 'M' else None)
        for e in GD.EAUX:      # les noms : le plan d'eau le plus proche du centre de chaque étang de web_conquete
            c = g.sous(e['xy'][0] * 1000, e['xy'][1] * 1000)
            eau = [j for j in g.anneau(c, 6) if self.fuel[j] == 'L'] if c is not None else []
            if eau:
                cid = self.corps[min(eau, key=lambda j: g.distance(c, j))]
                if self.corps_nom[cid] is None:
                    self.corps_nom[cid] = e['nom']
        for cid, n in enumerate(self.corps_taille):
            if self.corps_nom[cid] is None:
                self.corps_nom[cid] = 'Étang' if n >= 12 else 'Mare'

    def ecopable(self, i):
        """Un Canadair peut écoper ici : la mer, ou un plan d'eau d'au moins 12 hexagones."""
        return self.fuel[i] in 'ML' and self.corps[i] >= 0 and self.corps_taille[self.corps[i]] >= 12

    def pompable(self, i):
        """Un hélicoptère peut puiser ici : n'importe quelle eau."""
        return self.fuel[i] in EAU

    # ------------------------------------------------------------------ export
    def export(self):
        g = self.g
        return dict(
            graine=self.graine, theme=self.theme, cols=g.cols, rows=g.rows, hexM=g.hex_m,
            largeur=round(g.largeur), hauteur=round(g.hauteur),
            fuel=''.join(self.fuel), alt=[round(a) for a in self.alt], route=''.join(str(r) for r in self.route),
            routes=self.routes_poly, riviere=self.riviere, nomRiviere=self.nom_riviere,
            sites=[dict(id=s['id'], type=s['type'], nom=s['nom'], centre=s['centre'], cells=s['cells'], valeur=s['valeur'], pop=s['pop'])
                   for s in self.sites],
            casernes=self.casernes, vigies=self.vigies, pointsEau=self.points_eau,
            helistation=self.helistation, aerodrome=self.aerodrome, nomAerodrome=getattr(self, 'nom_aerodrome', ''),
            lacs=[dict(id=cid, nom=self.corps_nom[cid], taille=n) for cid, n in enumerate(self.corps_taille)],
            corps=self.corps,
            comb={k: dict(nom=v['nom'], brule=v['brule'], col=v['col']) for k, v in COMB.items()},
            voies={str(k): v for k, v in VOIES.items()},
        )
