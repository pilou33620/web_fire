"""Génération de la carte : relief, végétation, eaux, villes, sites à protéger, routes, pistes DFCI, moyens fixes.

Deux thèmes :
- 'landes'        : massif des Landes de Gascogne (océan à l'ouest, dunes, grands étangs, pinède, pistes DFCI) ;
- 'mediterranee'  : massif méditerranéen (mer au sud, relief, garrigue et maquis, villages perchés).
"""
import math
import random

from .bruit import fbm, hache
from .combustibles import COMB, EAU
from .hexa import Grille, astar

NOMS = {
    'landes': {
        'ville': ['Saint-Aurans', 'Mios-la-Lande', 'Belin-sur-Leyre'],
        'village': ['Hostens', 'Louchats', 'Saugnac', 'Le Tuzan', 'Sore', 'Lugos', 'Cazalis', 'Origne', 'Balizac', 'Lucmau',
                    'Escource', 'Moustey', 'Pissos', 'Liposthey', 'Commensacq'],
        'camping': ['Camping Les Pins', 'Camping du Lac', 'Camping La Dune', "Camping L'Océan", 'Camping Les Arbousiers'],
        'usine': ['Papeterie de la Leyre', 'Scierie Dubourg', 'Dépôt pétrolier (Seveso)'],
        'reserve': ['Réserve naturelle du Courant', 'Forêt domaniale des Bruyères'],
        'patrimoine': ['Chapelle de Notre-Dame-des-Lanes', 'Airial de Marquèze'],
        'lac': ['Étang de Carcans', 'Étang de Cazaux', 'Lac de Lacanau'],
        'riviere': 'La Leyre',
        'aerodrome': 'Aérodrome de la Lande',
    },
    'mediterranee': {
        'ville': ['Sainte-Maxime-des-Pins', 'Port-Grimaud-le-Vieux', 'Fréjus-sur-Mer'],
        'village': ['Collobrières', 'Le Plan-de-la-Tour', 'Vidauban', 'La Garde-Freinet', 'Gonfaron', 'Bormes', 'Le Cannet',
                    'Pignans', 'Cogolin', 'Ramatuelle', 'Gassin', 'Les Mayons'],
        'camping': ['Camping de la Plage', 'Camping Les Oliviers', 'Camping Le Maquis', 'Camping des Calanques'],
        'usine': ['Distillerie de lavande', 'Dépôt de gaz (Seveso)'],
        'reserve': ['Réserve naturelle de la plaine des Maures', 'Forêt des chênes-lièges'],
        'patrimoine': ['Chartreuse de la Verne', 'Abbaye du Thoronet'],
        'lac': ['Lac de Carcès', 'Lac de Saint-Cassien'],
        'riviere': "L'Argens",
        'aerodrome': 'Aérodrome du Luc',
    },
}

VALEUR = {'ville': 100, 'village': 50, 'camping': 70, 'usine': 60, 'reserve': 25, 'patrimoine': 30}


class Carte:
    def __init__(self, graine=1, theme='landes', cols=60, rows=44, hex_m=300):
        self.graine, self.theme = graine, theme
        self.g = Grille(cols, rows, hex_m)
        self.rnd = random.Random(graine * 7919 + (0 if theme == 'landes' else 17))
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
        if theme == 'mediterranee':
            self._mediterranee()
        else:
            self._landes()
        self._plans_eau()

    # ------------------------------------------------------------------ outils
    def x(self, i):
        return self.g.cx[i]

    def y(self, i):
        return self.g.cy[i]

    def nx(self, i):
        return self.g.cx[i] / self.g.largeur

    def ny(self, i):
        return self.g.cy[i] / self.g.hauteur

    def libre(self, i):
        return self.fuel[i] not in EAU and self.fuel[i] not in 'SA' and self.site_de[i] < 0

    def _cellule_vers(self, fx, fy):
        return self.g.sous(fx * self.g.largeur, fy * self.g.hauteur)

    def _poser_site(self, typ, nom, centre, rayon, fuel, part=1.0):
        sid = len(self.sites)
        cells = []
        for j in self.g.anneau(centre, rayon):
            if self.fuel[j] in EAU or self.site_de[j] >= 0:
                continue
            if j != centre and self.rnd.random() > part:
                continue
            self.fuel[j] = fuel
            self.site_de[j] = sid
            cells.append(j)
        pop = {'ville': 6000, 'village': 600, 'camping': 1500, 'usine': 80, 'reserve': 0, 'patrimoine': 10}[typ]
        self.sites.append(dict(id=sid, type=typ, nom=nom, centre=centre, cells=cells, valeur=VALEUR[typ],
                               pop=int(pop * self.rnd.uniform(0.6, 1.5))))
        return sid

    def _loin_des_sites(self, i, dmin):
        return all(self.g.distance(i, s['centre']) >= dmin for s in self.sites)

    def _choisir(self, filtre, dmin, essais=400):
        for _ in range(essais):
            i = self.rnd.randrange(self.g.n)
            if filtre(i) and self._loin_des_sites(i, dmin):
                return i
        return None

    # ------------------------------------------------------------------ Landes de Gascogne
    def _landes(self):
        g, s, N = self.g, self.graine, NOMS['landes']
        W, H = g.largeur, g.hauteur
        for i in range(g.n):
            x, y = g.cx[i], g.cy[i]
            cote = 900 + 500 * fbm(y / 3000, 0.3, s)
            if x < cote:
                self.fuel[i], self.alt[i] = 'M', 0
            elif x < cote + 260:
                self.fuel[i], self.alt[i] = 'S', 3
            elif x < cote + 900:
                self.fuel[i] = 'D'
                self.alt[i] = 15 + 70 * fbm(x / 500, y / 700, s + 3) * (1 - (x - cote - 260) / 640) ** 0.5
            else:
                self.alt[i] = 35 + 30 * fbm(x / 6000, y / 6000, s + 5)
                v1, v2, v3 = fbm(x / 1800, y / 1800, s + 11), fbm(x / 1400, y / 1400, s + 23), fbm(x / 2500, y / 2500, s + 37)
                if v1 > 0.63:
                    self.fuel[i] = 'J'
                elif v3 > 0.66:
                    self.fuel[i] = 'F'
                elif v2 > 0.72:
                    self.fuel[i] = 'H'
                else:
                    self.fuel[i] = 'P'
        # les étangs, en arrière des dunes
        for k, (fy, ry) in enumerate([(0.22, 0.15), (0.72, 0.11)]):
            cy = fy * H
            cxl = 900 + 500 * fbm(cy / 3000, 0.3, s) + 1900 + 500 * k
            rx, ryy = 950 - 200 * k, ry * H
            for i in range(g.n):
                dx, dy = (g.cx[i] - cxl) / rx, (g.cy[i] - cy) / ryy
                d = dx * dx + dy * dy + 0.35 * (fbm(g.cx[i] / 800, g.cy[i] / 800, s + 50 + k) - 0.5)
                if d < 1:
                    self.fuel[i], self.alt[i] = 'L', 12
        # la rivière : de l'est vers l'étang du sud, en méandres
        y = 0.62 * H + (hache(s, 2, 3) - 0.5) * 0.2 * H
        x = W
        cible = (900 + 500 * fbm(0.70 * H / 3000, 0.3, s) + 2400 + 900, 0.70 * H)
        riv = []
        while x > cible[0]:
            i = g.sous(min(x, W - 1), y)
            if i is not None:
                if self.fuel[i] == 'L':
                    break
                riv.append(i)
            x -= g.hex_m * 0.5
            y += (cible[1] - y) * 0.03 + (fbm(x / 1500, 0.7, s + 61) - 0.5) * 260
        for i in riv:
            self.fuel[i] = 'R'
            self.alt[i] -= 8
            for j in g.voisins(i):
                if self.fuel[j] not in EAU and self.rnd.random() < 0.75:
                    self.fuel[j] = 'F'          # ripisylve
                    self.alt[j] -= 4
        self.riviere = riv
        terre = lambda i: self.fuel[i] in 'PJFH' and self.nx(i) > 0.22
        # villes et villages
        noms_v = self.rnd.sample(N['ville'], 1)
        v = self._choisir(lambda i: terre(i) and 0.62 < self.nx(i) < 0.88 and 0.25 < self.ny(i) < 0.75, 0)
        self._poser_site('ville', noms_v[0], v, 2, 'U', 0.75)
        for j in g.anneau(v, 5):
            if self.site_de[j] < 0 and self.fuel[j] in 'PJFH':
                self.fuel[j] = 'V' if self.nx(j) > 0.8 and self.rnd.random() < 0.7 else ('C' if self.rnd.random() < 0.65 else self.fuel[j])
        for nom in self.rnd.sample(N['village'], 6):
            c = self._choisir(lambda i: terre(i) and 0.06 < self.ny(i) < 0.94 and self.nx(i) < 0.95, 9)
            if c is None:
                continue
            self._poser_site('village', nom, c, 1, 'B', 0.7)
            for j in g.anneau(c, 3):
                if self.site_de[j] < 0 and self.fuel[j] in 'PJF' and self.rnd.random() < 0.55:
                    self.fuel[j] = 'C' if self.rnd.random() < 0.6 else 'H'      # l'airial : champs et prairies
        # campings : au bord des étangs et derrière la dune
        for nom in self.rnd.sample(N['camping'], 3):
            c = self._choisir(lambda i: self.fuel[i] in 'PDJ' and any(self.fuel[j] in 'LS' for j in g.anneau(i, 2)), 5)
            if c is not None:
                self._poser_site('camping', nom, c, 1, 'T', 0.6)
        for typ in ('usine', 'reserve', 'patrimoine'):
            nom = self.rnd.choice(N[typ])
            if typ == 'reserve':
                c = self._choisir(lambda i: self.fuel[i] in 'PF' and 0.3 < self.nx(i) < 0.75, 7)
                if c is not None:
                    self._poser_site('reserve', nom, c, 2, 'F', 0.9)
            else:
                c = self._choisir(lambda i: terre(i), 6)
                if c is not None:
                    self._poser_site(typ, nom, c, 0 if typ == 'patrimoine' else 1, 'Z' if typ == 'usine' else 'B', 0.6)
        self._reseau(pistes=True)
        self._moyens_fixes(N['aerodrome'])
        self.nom_lacs = N['lac']
        self.nom_riviere = N['riviere']

    # ------------------------------------------------------------------ massif méditerranéen
    def _mediterranee(self):
        g, s, N = self.g, self.graine, NOMS['mediterranee']
        W, H = g.largeur, g.hauteur
        for i in range(g.n):
            x, y = g.cx[i], g.cy[i]
            cote = H - 1100 - 700 * fbm(x / 3500, 0.2, s)
            relief = fbm(x / 4500, y / 4500, s + 3, 5)
            crete = 1 - abs(2 * fbm(x / 3000, y / 3000, s + 9) - 1)
            h = max(0.0, (relief - 0.35) * 900 + crete * 260) * min(1, (cote - y) / 2500) if y < cote else 0
            self.alt[i] = h
            if y >= cote:
                self.fuel[i] = 'M'
            elif y >= cote - 220:
                self.fuel[i] = 'S'
            else:
                v1, v2 = fbm(x / 1500, y / 1500, s + 11), fbm(x / 1200, y / 1200, s + 23)
                if h > 470 and v1 > 0.55:
                    self.fuel[i] = 'K'
                elif h > 330:
                    self.fuel[i] = 'G' if v1 > 0.45 else 'Q'
                elif h > 140:
                    self.fuel[i] = 'P' if v2 > 0.5 else ('Q' if v1 > 0.4 else 'F')
                elif h > 40:
                    self.fuel[i] = 'Q' if v1 > 0.55 else ('P' if v2 > 0.45 else 'G')
                else:
                    self.fuel[i] = 'V' if v2 > 0.55 else ('O' if v1 > 0.6 else ('C' if v1 > 0.42 else 'H'))
        # une retenue d'eau dans les collines, et le fleuve côtier
        c = self._choisir(lambda i: 120 < self.alt[i] < 300 and 0.3 < self.nx(i) < 0.7 and self.ny(i) < 0.5, 0)
        if c is not None:
            for j in g.anneau(c, 3):
                if self.rnd.random() < 0.85 or g.distance(c, j) < 2:
                    self.fuel[j] = 'L'
        x, y = 0.2 * W + hache(s, 4, 5) * 0.5 * W, 0
        riv = []
        while y < H:
            i = g.sous(x, y)
            if i is not None:
                if self.fuel[i] == 'M':
                    break
                riv.append(i)
            y += g.hex_m * 0.5
            x += (fbm(y / 1300, 0.5, s + 61) - 0.5) * 300
        for i in riv:
            if self.fuel[i] != 'L':
                self.fuel[i] = 'R'
            for j in g.voisins(i):
                if self.fuel[j] not in EAU and self.alt[j] < 200 and self.rnd.random() < 0.6:
                    self.fuel[j] = 'F'
        self.riviere = riv
        terre = lambda i: self.fuel[i] not in EAU and self.fuel[i] not in 'SKA'
        v = self._choisir(lambda i: terre(i) and self.alt[i] < 40 and any(self.fuel[j] == 'S' for j in g.anneau(i, 3)) and 0.25 < self.nx(i) < 0.8, 0)
        self._poser_site('ville', self.rnd.choice(N['ville']), v, 2, 'U', 0.75)
        for nom in self.rnd.sample(N['village'], 6):
            c = self._choisir(lambda i: terre(i) and 60 < self.alt[i] < 420 and 0.05 < self.nx(i) < 0.95 and self.ny(i) > 0.06, 9)
            if c is None:
                continue
            self._poser_site('village', nom, c, 1, 'B', 0.7)
            for j in g.anneau(c, 2):
                if self.site_de[j] < 0 and terre(j) and self.rnd.random() < 0.5:
                    self.fuel[j] = 'O' if self.rnd.random() < 0.5 else 'V'
        for nom in self.rnd.sample(N['camping'], 3):
            c = self._choisir(lambda i: terre(i) and any(self.fuel[j] == 'S' for j in g.anneau(i, 2)), 5)
            if c is not None:
                self._poser_site('camping', nom, c, 1, 'T', 0.6)
        for typ in ('usine', 'reserve', 'patrimoine'):
            nom = self.rnd.choice(N[typ])
            if typ == 'reserve':
                c = self._choisir(lambda i: self.fuel[i] in 'PFQ' and 100 < self.alt[i] < 350, 7)
                if c is not None:
                    self._poser_site('reserve', nom, c, 2, 'F', 0.9)
            else:
                c = self._choisir(lambda i: terre(i) and self.alt[i] < 250, 6)
                if c is not None:
                    self._poser_site(typ, nom, c, 0 if typ == 'patrimoine' else 1, 'Z' if typ == 'usine' else 'B', 0.6)
        self._reseau(pistes=True)
        self._moyens_fixes(N['aerodrome'])
        self.nom_lacs = N['lac']
        self.nom_riviere = N['riviere']

    # ------------------------------------------------------------------ routes et pistes
    def _reseau(self, pistes=True):
        g = self.g
        lieux = [s for s in self.sites if s['type'] in ('ville', 'village', 'camping', 'usine')]

        def cout(i, j):
            f = self.fuel[j]
            if f in 'ML':
                return None
            c = 1.0
            if f == 'R':
                c = 6.0               # un pont coûte cher : on n'en fait pas partout
            elif f in 'KD':
                c = 3.0
            elif f in 'UBZTCVHO':
                c = 0.8
            if self.route[j] >= 2:
                c = 0.3
            c += abs(self.alt[j] - self.alt[i]) / 40.0
            return c

        # arbre couvrant (Prim) entre les lieux, puis quelques liaisons en plus
        if not lieux:
            return
        dans, aretes = {lieux[0]['id']}, []
        while len(dans) < len(lieux):
            best = None
            for a in lieux:
                if a['id'] not in dans:
                    continue
                for b in lieux:
                    if b['id'] in dans:
                        continue
                    d = g.distance(a['centre'], b['centre'])
                    if best is None or d < best[0]:
                        best = (d, a, b)
            dans.add(best[2]['id'])
            aretes.append((best[1], best[2]))
        for a in lieux:
            for b in lieux:
                if a['id'] < b['id'] and (a, b) not in aretes and 10 < g.distance(a['centre'], b['centre']) < 20 and self.rnd.random() < 0.3:
                    aretes.append((a, b))
        for a, b in aretes:
            ch, _ = astar(g, a['centre'], b['centre'], cout, 0.3)
            if not ch:
                continue
            niveau = 3 if 'ville' in (a['type'], b['type']) else 2
            for i in ch:
                self.route[i] = max(self.route[i], niveau)
            self.routes_poly.append(dict(n=niveau, c=ch))
        # sorties de carte : la ville est reliée au bord est (et à l'aérodrome plus tard)
        if pistes:
            self._pistes_dfci()

    def _pistes_dfci(self):
        """Pistes DFCI : un quadrillage dans les massifs, qui sert de coupure et d'accès aux camions."""
        g = self.g
        foret = set('PJFQG')
        pas_c, pas_r = 9, 8
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
    def _moyens_fixes(self, nom_aero):
        g = self.g
        villes = [s for s in self.sites if s['type'] == 'ville']
        villages = [s for s in self.sites if s['type'] == 'village']
        if villes:
            self.casernes.append(dict(id=0, nom='CSP ' + villes[0]['nom'], cell=villes[0]['centre']))
        for k, v in enumerate(sorted(villages, key=lambda s: -g.distance(s['centre'], villes[0]['centre']) if villes else 0)[:3]):
            self.casernes.append(dict(id=len(self.casernes), nom='CIS ' + v['nom'], cell=v['centre']))
        # aérodrome : sur le bord est, au plus près de la ville
        cand = [i for i in range(g.n) if g.col_row(i)[0] >= g.cols - 3 and self.fuel[i] not in EAU and self.site_de[i] < 0]
        if cand:
            ref = villes[0]['centre'] if villes else cand[0]
            a = min(cand, key=lambda i: g.distance(i, ref) + 3 * abs(self.alt[i] - 40) / 100)
            self.aerodrome = a
            for j in g.anneau(a, 1):
                if self.site_de[j] < 0 and self.fuel[j] not in EAU:
                    self.fuel[j] = 'A'
            ch, _ = astar(g, a, ref, lambda i, j: None if self.fuel[j] in 'ML' else (0.3 if self.route[j] >= 2 else 1.0), 0.3)
            if ch:
                for i in ch:
                    self.route[i] = max(self.route[i], 2)
                self.routes_poly.append(dict(n=2, c=ch))
            self.nom_aerodrome = nom_aero
        # hélistation DFCI : sur une piste, au centre du massif
        c = [i for i in range(g.n) if self.route[i] >= 1 and self.site_de[i] < 0 and self.fuel[i] not in EAU]
        if c:
            mid = g.sous(g.largeur * 0.5, g.hauteur * 0.5)
            self.helistation = min(c, key=lambda i: g.distance(i, mid))
            self.fuel[self.helistation] = 'X'
        # vigies : sur des points hauts, bien réparties
        foret = [i for i in range(g.n) if self.fuel[i] in 'PJFQG']
        for _ in range(4):
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
            if all(g.distance(i, p) >= 6 for p in self.points_eau):
                self.points_eau.append(i)
        if self.helistation is not None:
            self.points_eau.append(self.helistation)

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
        k = 0
        for cid, n in enumerate(self.corps_taille):
            if self.corps_nom[cid] is None:
                self.corps_nom[cid] = self.nom_lacs[k % len(self.nom_lacs)] if n >= 12 else 'Mare'
                if n >= 12:
                    k += 1

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
        )
