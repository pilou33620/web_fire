"""La propagation du feu, minute par minute, d'hexagone en hexagone.

Modèle simplifié inspiré de Rothermel : pour un hexagone qui brûle (intensité I) et un voisin combustible,
  vitesse = ros(combustible) × vent × pente × humidité × sécheresse × coupure × (1 − mouillé) × retardant × défense
  probabilité d'allumage dans la minute = 1 − exp(−vitesse × I)
Vent : exp(0,045 × V × cos θ) (θ : angle entre le vent et la direction de propagation) ; pente : exp(3 × dénivelé / 300 m).
Les sautes de feu (brandons) partent des hexagones intenses quand le vent dépasse 30 km/h.
L'eau abaisse l'intensité et mouille ; le retardant freine fortement l'allumage pendant des heures.
"""
import math

from .combustibles import COMB, ROUTE_FREIN

INTACT, BRULE, BRULE_FINI, ETEINT, NOYE = 0, 1, 2, 3, 4


class Feu:
    def __init__(self, carte, meteo, rnd):
        self.c, self.m, self.rnd = carte, meteo, rnd
        n = carte.g.n
        self.etat = [INTACT] * n
        self.I = [0.0] * n
        self.prog = [0.0] * n
        self.mouille = [0.0] * n
        self.ret = [0.0] * n
        self.dfrom = [-1] * n            # direction (0..5) d'où le feu est arrivé, -1 : départ ou saute
        self.foyer = [-1] * n            # le départ de feu à l'origine
        self.t_ign = [0] * n
        self.defense = [0] * n           # camions en défense sur l'hexagone (remis à zéro à chaque minute)
        self.brulant = set()
        self.eteints = set()
        self.departs = []
        self.nouvelles_sautes = []
        self.litres = 0.0
        self.litres_ret = 0.0
        self.ha_hex = carte.g.hex_m ** 2 * math.sqrt(3) / 2 / 10000

    # ------------------------------------------------------------------ allumer
    def combustible(self, i):
        f = self.c.fuel[i]
        return COMB[f]['brule']

    def peut_bruler(self, i):
        return self.combustible(i) and self.etat[i] in (INTACT, ETEINT, NOYE) and self.prog[i] < 0.92

    def allumer(self, i, d=-1, foyer=None, cause='inconnue', t=0):
        if not self.peut_bruler(i):
            return False
        if foyer is None:
            foyer = len(self.departs)
            self.departs.append(dict(id=foyer, cell=i, t=t, cause=cause, detecte=False, t_detect=None))
        self.etat[i] = BRULE
        self.I[i] = 0.15
        self.dfrom[i] = d
        self.foyer[i] = foyer
        self.t_ign[i] = t
        self.brulant.add(i)
        self.eteints.discard(i)
        return True

    # ------------------------------------------------------------------ un pas d'une minute
    def pas(self, t):
        c, m, g = self.c, self.m, self.c.g
        vx, vy = m.vers()
        V = m.vent
        fsech = m.f_secheresse()
        allumes = []
        for i in self.brulant:
            Ii = self.I[i]
            if Ii <= 0.02:
                continue
            for j, d, ux, uy in g.vois[i]:
                if not self.peut_bruler(j):
                    continue
                cj = COMB[c.fuel[j]]
                a = ux * vx + uy * vy
                fw = math.exp(0.045 * V * a)
                pente = max(-0.3, min(0.45, (c.alt[j] - c.alt[i]) / g.hex_m))
                fs = math.exp(3.0 * pente)
                fm = m.f_humidite(self.mouille[j])
                frein = ROUTE_FREIN[c.route[j]]
                fr = 0.12 if self.ret[j] > 0 else 1.0
                fd = 0.3 ** min(3, self.defense[j])
                reste = 1.0 - self.prog[j]
                r = cj['ros'] * fw * fs * fm * fsech * frein * fr * fd * (1 - self.mouille[j]) * (0.4 + 0.6 * reste)
                p = 1 - math.exp(-r * Ii)
                if p > 0 and self.rnd.random() < p:
                    allumes.append((j, (d + 3) % 6, self.foyer[i]))
            # sautes de feu
            if V > 30 and Ii > 0.6:
                taux = 0.0016 * (V - 30) / 10 * Ii * COMB[c.fuel[i]]['saute']
                if self.rnd.random() < taux:
                    dist = 2 + self.rnd.expovariate(1 / max(1.0, V / 14))
                    ang = math.atan2(vy, vx) + self.rnd.gauss(0, 0.3)
                    x = g.cx[i] + math.cos(ang) * dist * g.hex_m
                    y = g.cy[i] + math.sin(ang) * dist * g.hex_m
                    k = g.sous(x, y)
                    if k is not None and self.peut_bruler(k) and self.mouille[k] < 0.3 and self.ret[k] <= 0:
                        if self.rnd.random() < m.f_humidite(self.mouille[k]):
                            allumes.append((k, -1, self.foyer[i]))
                            self.nouvelles_sautes.append(k)
        # intensité, consommation, extinction naturelle
        fini = []
        for i in self.brulant:
            ci = COMB[c.fuel[i]]
            fm = m.f_humidite(self.mouille[i])
            imax = ci['imax'] * (0.45 + 0.55 * fm) * (0.8 + 0.2 * fsech) * (1 - 0.85 * self.mouille[i])
            self.I[i] += (imax - self.I[i]) * 0.25
            self.prog[i] += (0.35 + 0.65 * self.I[i]) / ci['dur']
            if self.prog[i] >= 1.0:
                fini.append((i, BRULE_FINI))
            elif self.I[i] < 0.05:
                fini.append((i, ETEINT))
        for i, e in fini:
            self.brulant.discard(i)
            self.etat[i] = e
            self.I[i] = 0.0
            if e == BRULE_FINI:
                self.prog[i] = 1.0
            else:
                self.eteints.add(i)
        for j, d, f in allumes:
            if self.etat[j] != BRULE:
                if self.etat[j] in (ETEINT, NOYE):
                    self.eteints.discard(j)
                self.allumer(j, d, f, t=t)
        # reprises : un hexagone éteint mais pas noyé peut se rallumer (braises)
        for i in list(self.eteints):
            if self.etat[i] == ETEINT and self.mouille[i] < 0.08 and self.rnd.random() < 0.0012 * (1 + V / 30) * fsech:
                self.allumer(i, -1, self.foyer[i], t=t)
        # séchage : l'eau s'évapore, le retardant tient des heures
        evap = 0.985 if not m.nuit else 0.995
        if m.pluie > 0.2:
            evap = 1.0
        for i in range(g.n):
            if self.mouille[i] > 0.001:
                self.mouille[i] *= evap
            if m.pluie > 0:
                self.mouille[i] = min(1.0, self.mouille[i] + 0.02 * m.pluie)
            if self.ret[i] > 0:
                self.ret[i] -= 1
            self.defense[i] = 0

    # ------------------------------------------------------------------ lutte
    def arroser(self, i, litres):
        """Eau sur un hexagone : baisse l'intensité, mouille. Renvoie les litres utilisés."""
        if not COMB[self.c.fuel[i]]['brule']:
            return 0
        res = COMB[self.c.fuel[i]]['res']
        k = litres / (6000.0 * res)
        self.mouille[i] = min(1.0, self.mouille[i] + k * 0.6)
        if self.etat[i] == BRULE:
            # un feu très intense « boit » l'eau : le largage est moins efficace sur une tête violente
            self.I[i] = max(0.0, self.I[i] - k * (1.25 - 0.6 * self.I[i]))
        self.litres += litres
        return litres

    def larguer_retardant(self, cells, litres):
        par = litres / max(1, len(cells))
        for i in cells:
            if COMB[self.c.fuel[i]]['brule']:
                self.ret[i] = max(self.ret[i], 240)
                if self.etat[i] == BRULE:
                    self.I[i] = max(0.0, self.I[i] - par / 9000)
        self.litres_ret += litres

    def noyer(self, i):
        if self.etat[i] == ETEINT:
            self.etat[i] = NOYE
            self.eteints.discard(i)
            self.mouille[i] = max(self.mouille[i], 0.5)
            return True
        return False

    def defricher(self, i):
        """Un bulldozer met le sol à nu : plus rien à brûler."""
        if self.etat[i] == BRULE:
            return False
        self.c.fuel[i] = 'X'
        return True

    # ------------------------------------------------------------------ bilan
    def surface_ha(self, foyer=None):
        n = 0
        for i in range(self.c.g.n):
            if self.etat[i] != INTACT and (foyer is None or self.foyer[i] == foyer):
                n += 1 if self.etat[i] in (BRULE, BRULE_FINI) else self.prog[i]
        return n * self.ha_hex
