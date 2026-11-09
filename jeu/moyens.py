"""Les moyens de lutte : camions-citernes feux de forêt (CCF), bulldozers, hélicoptères bombardiers d'eau (HBE),
Canadair (indicatif « Pélican ») et Dash 8 de retardant (indicatif « Milan »).

Unités : vitesses en hexagones par minute (1 hexagone = 300 m : 1 hex/min = 18 km/h), eau en litres.
"""
import math

from .combustibles import COMB, EAU
from .feu import BRULE, BRULE_FINI, ETEINT, NOYE
from .hexa import astar

TYPES = {
    'ccf': dict(nom='CCF', cap=4000, debit=400, terre=True),
    'bull': dict(nom='Bulldozer', cap=0, terre=True),
    'vlhr': dict(nom='VLHR', cap=0, terre=True),     # véhicule léger hors route du chef de groupe
    'helico': dict(nom='HBE', cap=1200, vitesse=10.0, endurance=150, terre=False),
    'canadair': dict(nom='Canadair', cap=6000, vitesse=16.0, endurance=180, terre=False),
    'dash': dict(nom='Dash 8', cap=10000, vitesse=24.0, endurance=240, terre=False),
}


DELAI = dict(ccf=3, bull=6, vlhr=3, helico=7, canadair=14, dash=18)
CONE = 50          # demi-angle du cône des lances (degrés)
RESERVE = 500      # litres gardés pour l'autoprotection : les lances n'y touchent jamais
DEBIT_AUTO = 120   # litres par minute des rampes d'aspersion quand le camion est encerclé


def ecart_angle(a, b):
    return abs((a - b + 180) % 360 - 180)


def dans_cone(g, i, j, cone):
    """La cellule j est-elle dans le cône (direction, demi-angle) vu depuis i ? Sa propre cellule l'est toujours."""
    if j == i or cone is None or cone[1] >= 180:
        return True
    a = math.degrees(math.atan2(g.cy[j] - g.cy[i], g.cx[j] - g.cx[i]))
    return ecart_angle(a, cone[0]) <= cone[1]   # minutes entre l'alerte et le départ


def vitesse_sol(kind, carte, feu, i):
    """Hexagones par minute pour un engin terrestre sur la cellule i."""
    r = carte.route[i]
    f = carte.fuel[i]
    if kind == 'bull':
        return [0.35, 0.8, 1.2, 1.2][r] if f not in EAU else 0.0
    if r >= 3:
        return 3.3
    if r == 2:
        return 2.8
    if r == 1:
        return 1.5
    if f in EAU:
        return 0.0
    if feu.etat[i] in (BRULE_FINI, ETEINT, NOYE):
        return 0.8                        # « travailler depuis le noir »
    if f in 'PFQJG':
        return 0.45
    return 0.9


class Moyen:
    def __init__(self, uid, kind, nom, base, carte):
        self.id, self.kind, self.nom, self.base = uid, kind, nom, base
        t = TYPES[kind]
        self.cap = t['cap']
        self.eau = self.cap
        self.terre = t['terre']
        self.cell = base
        self.x, self.y = carte.g.cx[base], carte.g.cy[base]
        self.cap_deg = 0.0
        self.etat = 'dispo'               # dispo, route, action, ravitaillement, au_sol, retour
        self.mission = None               # dict(type=..., feu=..., ...)
        self.chemin = []
        self.avance = 0.0
        self.attente = 0                  # minutes d'immobilisation (remplissage, écopage, défrichage)
        self.phase = None                 # cycle aérien : charge, ecope, vers_feu
        self.cible = None                 # (x, y) en vol
        self.vise = None                  # cellules visées par le prochain largage
        self.vol = 0                      # minutes de vol depuis le dernier plein
        self.act = None                   # ce que fait le moyen, pour l'affichage
        self.poste = None
        self.reprise = None               # mission suspendue pendant un ravitaillement
        self.litres = 0
        self.largages = 0
        self.renfort = False
        self.joueur = False               # sous les ordres du joueur : l'IA n'y touche pas
        self.hs = False                   # hors service (engin perdu dans le feu)
        self.groupe = None
        self.auto = False                 # autoprotection en cours

    # ------------------------------------------------------------------ déplacement au sol
    def aller(self, p, dest):
        """Calcule un chemin vers dest. Renvoie False si inaccessible."""
        if dest == self.cell:
            self.chemin = []
            return True
        ch = self.chemin_vers(p, dest)
        if not ch:
            return False
        self.chemin = ch[1:]
        self.avance = 0.0
        return True

    def chemin_vers(self, p, dest, depart=None):
        """Le chemin (liste de cellules) jusqu'à dest, sans rien modifier (sert aussi à l'aperçu des ordres)."""
        c, feu = p.carte, p.feu
        depart = self.cell if depart is None else depart
        if dest == depart:
            return [depart]

        danger = p.danger
        etat = feu.etat
        kind = self.kind

        def cout(i, j):
            v = vitesse_sol(kind, c, feu, j)
            if v <= 0:
                if c.fuel[j] == 'R' and c.route[j]:
                    v = 2.0                 # un pont
                else:
                    return None
            if etat[j] == BRULE and j != dest:
                return None
            if j in danger and j != dest:
                if c.route[j] == 1:
                    return None             # malus des pistes : on ne s'engage pas sur une piste qui longe un feu intense
                return 1.0 / v + 2.5
            return 1.0 / v

        ch, _ = astar(c.g, depart, dest, cout, 0.42)
        return ch

    def facteur_voie(self, p, j):
        """Les malus de la voie empruntée : bouchons et fumée sur les routes, pistes trop étroites pour se croiser."""
        r = p.carte.route[j]
        if r >= 2:
            f = 1.0 / (1 + 0.6 * p.trafic_convois.get(j, 0))
            return f * max(0.45, 1 - 0.6 * p.evac.fumee.get(j, 0))
        if r == 1:
            autres = p.trafic_engins.get(j, 0) - (1 if self.cell == j else 0)
            return 0.35 if autres >= 2 else 1.0
        return 1.0

    def avancer_sol(self, p):
        c, feu, g = p.carte, p.feu, p.carte.g
        budget = 1.0
        while budget > 0 and self.chemin:
            j = self.chemin[0]
            if feu.etat[j] == BRULE:
                dest = self.chemin[-1]
                if not self.aller(p, dest):
                    self.chemin = []
                return
            v = (vitesse_sol(self.kind, c, feu, j) or 2.0) * self.facteur_voie(p, j)
            reste = (1 - self.avance) / v
            if reste <= budget:
                budget -= reste
                self.cell = j
                self.chemin.pop(0)
                self.avance = 0.0
            else:
                self.avance += budget * v
                budget = 0
        # position affichée : entre la cellule courante et la suivante
        if self.chemin:
            j = self.chemin[0]
            x0, y0, x1, y1 = g.cx[self.cell], g.cy[self.cell], g.cx[j], g.cy[j]
            self.x, self.y = x0 + (x1 - x0) * self.avance, y0 + (y1 - y0) * self.avance
            self.cap_deg = math.degrees(math.atan2(y1 - y0, x1 - x0))
        else:
            self.x, self.y = g.cx[self.cell], g.cy[self.cell]

    # ------------------------------------------------------------------ vol
    def voler(self, p, x, y):
        """Vole vers (x, y). Renvoie True à l'arrivée."""
        g = p.carte.g
        v = TYPES[self.kind]['vitesse'] * g.hex_m
        dx, dy = x - self.x, y - self.y
        d = math.hypot(dx, dy)
        self.vol += 1
        if d > 1:
            self.cap_deg = math.degrees(math.atan2(dy, dx))
        if d <= v:
            self.x, self.y = x, y
            self.cell = g.sous(x, y) or self.cell
            return True
        self.x += dx / d * v
        self.y += dy / d * v
        self.cell = g.sous(self.x, self.y) or self.cell
        return False

    # ------------------------------------------------------------------ un pas
    def pas(self, p):
        self.act = None
        # délai d'alerte : partir de la caserne ou décoller prend du temps
        m = self.mission
        if m is not None and m.get('_vu') is None:
            m['_vu'] = True
            if self.etat in ('dispo', 'au_sol') and self.attente == 0 and self.cell in (self.base, p.carte.aerodrome, p.carte.helistation):
                self.attente = DELAI[self.kind]
                self.etat = 'alerte'
        if self.attente > 0:
            self.attente -= 1
            if self.etat == 'ravitaillement':
                self.act = dict(k='remplit')
            elif self.etat == 'action' and self.kind == 'bull':
                self.act = dict(k='defriche', c=self.cell)
            elif self.phase == 'ecope':
                self.act = dict(k='ecope', c=self.cell)
            elif self.etat == 'au_sol':
                self.act = dict(k='au_sol')
            elif self.etat == 'alerte':
                self.act = dict(k='alerte')
            if self.attente == 0:
                self._fin_attente(p)
            return
        if self.terre:
            self._pas_sol(p)
        else:
            self._pas_air(p)

    def _fin_attente(self, p):
        if self.etat == 'ravitaillement':
            self.eau = self.cap
            self.etat = 'route'
            if self.poste is not None:
                self.aller(p, self.poste)
        elif self.kind == 'bull' and self.etat == 'action':
            if self.mission and self.mission.get('cells'):
                i = self.mission['cells'][0]
                if p.feu.defricher(i):
                    p.stats['defriche'] += 1
                self.mission['cells'].pop(0)
            self.etat = 'route'
        elif self.phase == 'ecope':
            self.eau = self.cap
            self.phase = 'vers_feu'
            self.cible = None
        elif self.etat == 'alerte':
            self.etat = 'route' if self.terre else 'action'
        elif self.etat == 'au_sol':
            self.vol = 0
            self.eau = self.cap
            self.etat = 'dispo'

    # ---------------------------------------------------------------- engins terrestres
    def _encercle(self, p, i=None):
        """Le feu est-il sur le camion (sa cellule brûle, ou au moins deux voisines brûlent fort) ?"""
        feu, g = p.feu, p.carte.g
        i = self.cell if i is None else i
        # seul le front compte : un hexagone allumé depuis peu (ensuite, sur la route ou la piste, le combustible
        # autour du camion est déjà consumé ; il ne reste que des braises)
        front = lambda j, imin: feu.etat[j] == BRULE and feu.I[j] > imin and p.t - feu.t_ign[j] <= 8
        if front(i, 0.3):
            return True
        return sum(1 for j in g.voisins(i) if front(j, 0.7)) >= 2

    def _securite(self, p):
        """Repli si c'est possible ; sinon autoprotection (CCF) ; réserve épuisée : l'engin est perdu.
        Renvoie True si le moyen a passé la minute à se protéger."""
        c, feu, g = p.carte, p.feu, p.carte.g
        if not self._encercle(p):
            if self.auto:
                self.auto = False
                self.etat = 'route'
                p.journal_ajout('cos', f"{self.nom} : le front est passé, fin de l'autoprotection ; l'équipage repart faire le plein.")
            return False
        if not self.auto:
            sur = [j for j in g.voisins(self.cell) if feu.etat[j] != BRULE and vitesse_sol(self.kind, c, feu, j) > 0
                   and not self._encercle(p, j)]
            if sur:
                sur.sort(key=lambda j: (feu.etat[j] not in (BRULE_FINI, ETEINT, NOYE), -c.route[j]))
                self.cell = sur[0]
                self.chemin = []
                self.x, self.y = g.cx[self.cell], g.cy[self.cell]
                if p.t - getattr(self, '_repli_t', -99) > 15:          # une seule annonce par quart d'heure
                    p.journal_ajout('repli', f'{self.nom} se replie : le feu arrive sur sa position.')
                self._repli_t = p.t
                return False
        # plus d'issue : on reste, l'équipage s'abrite dans la cabine et les rampes d'aspersion arrosent le camion
        if self.kind == 'ccf' and self.eau > 0:
            if not self.auto:
                self.auto = True
                p.stats['autoprotections'] = p.stats.get('autoprotections', 0) + 1
                p.journal_ajout('alerte', f"🛡 {self.nom} encerclé par le feu : autoprotection déclenchée, l'équipage est à l'abri dans la cabine.")
            l = min(self.eau, DEBIT_AUTO)
            self.eau -= l
            self.litres += l
            feu.mouille[self.cell] = min(1.0, feu.mouille[self.cell] + 0.15)
            self.etat = 'autoprotection'
            self.chemin = []
            self.act = dict(k='autoprotection', c=self.cell)
            return True
        if self.kind == 'ccf' or feu.etat[self.cell] == BRULE and feu.I[self.cell] > 0.5:
            self.hs = True
            self.auto = False
            self.mission, self.joueur = None, False
            self.etat = 'hs'
            p.stats['perdus'] = p.stats.get('perdus', 0) + 1
            quoi = "réserve d'autoprotection épuisée" if self.kind == 'ccf' else 'pris par les flammes'
            p.journal_ajout('alerte', f"⚠ {self.nom} perdu ({quoi}) : l'équipage est évacué de justesse, l'engin est hors service.")
            return True
        return False

    def _pas_sol(self, p):
        c, feu, g = p.carte, p.feu, p.carte.g
        if self.hs:
            self.etat, self.act = 'hs', dict(k='hs')
            return
        if self._securite(p):
            return
        m = self.mission
        if m is None:
            # pas de mission : retour à la caserne
            if self.cell != self.base:
                if not self.chemin:
                    self.aller(p, self.base)
                self.etat = 'retour'
                self.avancer_sol(p)
            else:
                self.etat = 'dispo'
                if self.eau < self.cap:
                    self.eau = min(self.cap, self.eau + 500)
                self.avancer_sol(p)
            return
        if self.kind == 'bull':
            return self._pas_bull(p)
        if self.kind == 'vlhr':
            return self._pas_chef(p)
        # ravitaillement en eau
        if self.eau < RESERVE + 300 and self.etat != 'ravitaillement':
            pe = p.point_eau_proche(self.cell)
            if pe is not None:
                if self.cell == pe:
                    self.etat = 'ravitaillement'
                    self.attente = 8
                    self.act = dict(k='remplit')
                    return
                if not self.chemin or self.chemin[-1] != pe:
                    self.aller(p, pe)
                self.etat = 'route'
                self.act = dict(k='va_remplir')
                self.avancer_sol(p)
                return
        poste = m.get('poste')
        self.poste = poste
        if poste is not None and self.cell != poste:
            if not self.chemin or self.chemin[-1] != poste:
                if not self.aller(p, poste):
                    m['bloque'] = m.get('bloque', 0) + 1
            self.etat = 'route'
            self.avancer_sol(p)
            # sur le trajet, on arrose ce qui est à portée et peu intense
            if self.chemin and self.eau > 600 and m.get('dir') is None:     # orienté par le joueur : on garde l'eau pour son cône
                self._arroser_voisins(p, 0.5, debit=200)
            return
        self.etat = 'action'
        self.x, self.y = g.cx[self.cell], g.cy[self.cell]
        t = m['type']
        if t == 'defense':
            proche = any(feu.etat[j] == BRULE for j in g.anneau(self.cell, 2))
            if proche:
                if not self._arroser_voisins(p, 1.01):
                    if self.eau > RESERVE + 120:
                        feu.mouille[self.cell] = min(1.0, feu.mouille[self.cell] + 0.04)
                        self.eau -= 120
                    self.act = dict(k='protege', c=self.cell)
            else:
                self.act = dict(k='attend')
        elif t == 'attaque':
            if not self._arroser_voisins(p, 0.8):
                if not self._noyer_voisins(p):
                    m['inactif'] = m.get('inactif', 0) + 1
                    self.act = dict(k='attend')
        elif t == 'surveillance':
            if not self._noyer_voisins(p):
                if not self._arroser_voisins(p, 0.8):
                    m['inactif'] = m.get('inactif', 0) + 1
                    self.act = dict(k='surveille')
        elif t == 'preposition':
            self.act = dict(k='attend')
        elif t in ('arroser', 'flanc'):
            cone = (m['dir'], CONE)
            if not self._arroser_voisins(p, 0.8, cone=cone):
                if not self._noyer_voisins(p, cone):
                    m['inactif'] = m.get('inactif', 0) + 1
                    self.act = dict(k='attend', d=m['dir'])
                    if t == 'flanc' and m['inactif'] >= 2:
                        from .ordres import suivre_flanc
                        suivre_flanc(p, self)
        elif t == 'ligne':
            cone = (m['dir'], CONE)
            if not self._arroser_voisins(p, 0.85, cone=cone):
                self._mouiller(p, cone)

    def _mouiller(self, p, cone):
        """Ligne d'arrêt sans flammes à portée : on mouille la végétation devant soi (ligne d'eau)."""
        feu, g, c = p.feu, p.carte.g, p.carte
        cands = [j for j in g.voisins(self.cell) if dans_cone(g, self.cell, j, cone) and COMB[c.fuel[j]]['brule']
                 and feu.etat[j] != BRULE and feu.mouille[j] < 0.6]
        if not cands or self.eau < RESERVE + 150:
            self.act = dict(k='attend', d=cone[0])
            return
        j = min(cands, key=lambda k: feu.mouille[k])
        feu.mouille[j] = min(0.6, feu.mouille[j] + 0.05)
        self.eau -= 120
        self.litres += 120
        self.act = dict(k='mouille', c=j, d=cone[0])

    def _pas_chef(self, p):
        """Le VLHR du chef de groupe suit son groupe et se tient un peu en arrière."""
        g = p.carte.g
        poste = self.mission.get('poste')
        if poste is not None and self.cell != poste:
            if not self.chemin or self.chemin[-1] != poste:
                self.aller(p, poste)
            self.etat = 'route'
            self.avancer_sol(p)
            return
        self.etat = 'action'
        self.x, self.y = g.cx[self.cell], g.cy[self.cell]
        self.act = dict(k='commande')

    def _arroser_voisins(self, p, imax, debit=None, cone=None):
        feu, g = p.feu, p.carte.g
        cands = [j for j in [self.cell] + g.voisins(self.cell) if feu.etat[j] == BRULE and feu.I[j] <= imax
                 and dans_cone(g, self.cell, j, cone)]
        if not cands or self.eau <= RESERVE:
            return False
        j = max(cands, key=lambda k: feu.I[k])
        l = min(self.eau - RESERVE, debit or TYPES['ccf']['debit'])
        feu.arroser(j, l)
        self.eau -= l
        self.litres += l
        self.act = dict(k='arrose', c=j, d=cone[0] if cone else None)
        if j != self.cell:
            self.cap_deg = math.degrees(math.atan2(g.cy[j] - g.cy[self.cell], g.cx[j] - g.cx[self.cell]))
        return True

    def _noyer_voisins(self, p, cone=None):
        feu, g = p.feu, p.carte.g
        for j in [self.cell] + g.voisins(self.cell):
            if feu.etat[j] == ETEINT and dans_cone(g, self.cell, j, cone) and self.eau > RESERVE + 150:
                if feu.noyer(j):
                    self.eau -= 150
                    self.act = dict(k='noie', c=j)
                    return True
        return False

    def _pas_bull(self, p):
        feu, g = p.feu, p.carte.g
        m = self.mission
        cells = m.get('cells') or []
        while cells and (not COMB[p.carte.fuel[cells[0]]]['brule'] or feu.etat[cells[0]] != 0):
            cells.pop(0)
        if not cells:
            m['fini'] = True
            self.act = dict(k='attend')
            return
        i = cells[0]
        if any(feu.etat[k] == BRULE and feu.I[k] > 0.5 for k in [i] + g.voisins(i)):
            cells.pop(0)                  # trop près du feu : on saute cette cellule
            return
        if self.cell != i:
            if not self.chemin or self.chemin[-1] != i:
                if not self.aller(p, i):
                    cells.pop(0)
                    return
            self.etat = 'route'
            self.avancer_sol(p)
            return
        self.etat = 'action'
        self.attente = 5
        self.act = dict(k='defriche', c=i)

    # ---------------------------------------------------------------- aéronefs
    def _pas_air(self, p):
        c, g, met = p.carte, p.carte.g, p.meteo
        base = c.aerodrome if self.kind in ('canadair', 'dash') else (c.helistation if c.helistation is not None else self.base)
        if base is None:
            base = self.base
        bx, by = g.cx[base], g.cy[base]
        m = self.mission
        cloue = not met.vol_possible(self.kind)
        trop_long = self.vol > TYPES[self.kind]['endurance']
        if m is None or cloue or trop_long:
            if (self.x, self.y) != (bx, by):
                self.etat = 'retour'
                self.phase = None
                if self.voler(p, bx, by):
                    self.etat = 'au_sol'
                    self.attente = 20 if trop_long else 2
                    self.vol = 0
                return
            self.etat = 'au_sol' if cloue else 'dispo'
            self.act = dict(k='au_sol')
            self.vol = 0
            if self.kind != 'canadair':
                self.eau = self.cap
            return
        self.etat = 'action'
        if m['type'] == 'gaar':
            return self._gaar(p)
        if self.kind == 'dash':
            return self._cycle_dash(p, bx, by)
        if self.eau < self.cap * 0.5 and self.phase != 'vers_feu':
            self.phase = 'charge'
        if self.phase in (None, 'charge'):
            if self.eau >= self.cap * 0.5:
                self.phase = 'vers_feu'
            else:
                self.phase = 'charge'
                if self.cible is None:
                    pt = p.eau_aerienne(self)
                    if pt is None:
                        self.mission = None
                        return
                    self.cible = (g.cx[pt], g.cy[pt])
                if self.voler(p, *self.cible):
                    self.phase = 'ecope'
                    self.attente = 1 if self.kind == 'canadair' else 2
                    self.act = dict(k='ecope', c=self.cell)
                    self.cible = None
                else:
                    self.act = dict(k='vol')
                return
        if self.phase == 'vers_feu':
            if self.vise is None or self.cible is None:
                v = p.ia.cible_largage(self)
                if v is None:
                    self.act = dict(k='vol')
                    self.voler(p, bx, by)
                    return
                self.vise = v
                cx = sum(g.cx[i] for i in v) / len(v)
                cy = sum(g.cy[i] for i in v) / len(v)
                self.cible = (cx, cy)
            if self.voler(p, *self.cible):
                cells = [i for i in self.vise if COMB[c.fuel[i]]['brule']]
                if cells:
                    par = self.eau / len(cells)
                    for i in cells:
                        p.feu.arroser(i, par)
                    p.evenement('largage', moyen=self.id, kind=self.kind, cells=cells, litres=round(self.eau), produit='eau',
                                cap=round(self.cap_deg))
                    self.litres += self.eau
                    self.largages += 1
                    p.stats['largages'] += 1
                self.eau = 0
                self.vise = None
                self.cible = None
                self.phase = 'charge'
                self.act = dict(k='largue')
            else:
                self.act = dict(k='vol')

    def _gaar(self, p):
        """Guet aérien armé : chargé d'eau, l'avion tourne au-dessus des massifs (et repère les départs de feu)."""
        g = p.carte.g
        if self.eau < self.cap:
            if self.cible is None or self.phase != 'charge':
                pt = p.eau_aerienne(self)
                if pt is None:
                    return
                self.phase = 'charge'
                self.cible = (g.cx[pt], g.cy[pt])
            if self.voler(p, *self.cible):
                self.phase = 'ecope'
                self.attente = 1
                self.cible = None
            self.act = dict(k='vol')
            return
        pts = self.mission['pts']
        k = self.mission.get('k', 0)
        self.phase = 'gaar'
        if self.voler(p, *pts[k % len(pts)]):
            self.mission['k'] = k + 1
        self.act = dict(k='patrouille')

    def _cycle_dash(self, p, bx, by):
        g = p.carte.g
        if self.eau <= 0:
            if self.voler(p, bx, by):
                self.etat = 'action'
                self.phase = 'ecope'
                self.attente = 12
                self.act = dict(k='remplit')
            else:
                self.act = dict(k='vol')
            return
        if self.vise is None:
            v = p.ia.ligne_retardant(self)
            if not v:
                self.mission = None
                return
            self.vise = v
            self.cible = (g.cx[v[0]], g.cy[v[0]])
            self.passe = 0
        if self.voler(p, *self.cible):
            p.feu.larguer_retardant(self.vise, self.eau)
            p.evenement('largage', moyen=self.id, kind='dash', cells=self.vise, litres=round(self.eau), produit='retardant',
                        cap=round(self.cap_deg))
            p.stats['largages'] += 1
            self.litres += self.eau
            self.largages += 1
            self.eau = 0
            self.vise = None
            self.act = dict(k='largue')
            self.mission = None           # une ligne par mission : le commandement en redemande s'il faut
            self.joueur = False
        else:
            self.act = dict(k='vol')

    # ------------------------------------------------------------------ export
    def export(self):
        m = self.mission
        return dict(id=self.id, k=self.kind, n=self.nom, x=round(self.x), y=round(self.y), cap=round(self.cap_deg),
                    e=self.etat, eau=round(100 * self.eau / self.cap) if self.cap else None,
                    m=m.get('label') if m else None, mt=m.get('type') if m else None, f=m.get('feu') if m else None,
                    p=m.get('poste') if m else None, a=self.act, c=self.cell, r=self.renfort, ph=self.phase,
                    v=self.vise if self.vise else None, l=round(self.litres), j=self.joueur, hs=self.hs, au=self.auto,
                    dir=m.get('dir') if m else None, ax=m.get('axe') if m else None, ci=m.get('cible') if m else None,
                    g=getattr(self, 'groupe', None))
