"""La météo du feu : température, humidité, vent (direction, force, rafales, bascule), nuit, orages, pluie.

Le vent est donné comme les météorologues : `vent_de` = d'où il vient, en degrés (0 = nord, 90 = est).
Le feu, lui, est poussé vers `vers()` (vecteur unitaire en x est, y sud).
"""
import math
import random

# scénarios : (température max, humidité min, vent moyen km/h, direction d'où vient le vent, sécheresse 0..1, extras)
SCENARIOS = {
    'calme': dict(nom='Journée d\'été ordinaire', tmax=28, hrmin=40, vent=12, de=290, sech=0.35, departs=0.5),
    'chaud': dict(nom='Forte chaleur, vent modéré', tmax=34, hrmin=28, vent=22, de=300, sech=0.6, departs=1.0),
    'canicule': dict(nom='Canicule et vent d\'est (type Landiras 2022)', tmax=40, hrmin=18, vent=35, de=95, sech=0.95,
                     departs=1.6, bascule=(17 * 60, 250, 30)),
    'orage': dict(nom='Orages secs : la foudre frappe la forêt', tmax=35, hrmin=25, vent=20, de=220, sech=0.75,
                  departs=0.7, orage=(15 * 60, 19 * 60)),
    'mistral': dict(nom='Mistral violent', tmax=33, hrmin=20, vent=55, de=330, sech=0.85, departs=1.3),
}
NIVEAUX = ['Faible', 'Léger', 'Modéré', 'Sévère', 'Très sévère', 'Exceptionnel']


class Meteo:
    def __init__(self, scenario='chaud', graine=1, heure0=10 * 60):
        self.sc = dict(SCENARIOS.get(scenario, SCENARIOS['chaud']))
        self.code = scenario if scenario in SCENARIOS else 'chaud'
        self.rnd = random.Random(graine * 31 + 7)
        self.t = heure0                       # minutes depuis minuit du jour 1
        self.de = float(self.sc['de'])
        self.de_base = self.de
        self.derive = 0.0
        self.rafale = 0.0
        self.pluie = 0.0
        self.orage = False
        self.foudre = []                      # impacts de foudre de la minute (cellules choisies par la partie)
        self.sech = self.sc['sech']
        self.maj()

    # ------------------------------------------------------------------ pas d'une minute
    def pas(self):
        self.t += 1
        sc = self.sc
        # direction : marche au hasard lente autour de la direction du jour, plus une éventuelle bascule
        self.derive += self.rnd.gauss(0, 0.6) - self.derive * 0.01
        if 'bascule' in sc:
            tb, de2, duree = sc['bascule']
            h = self.minute_du_jour()
            if tb <= h <= tb + duree:
                k = (h - tb) / duree
                self.de_base = sc['de'] + ((de2 - sc['de'] + 540) % 360 - 180) * k
        self.de = (self.de_base + self.derive) % 360
        # rafales
        self.rafale += self.rnd.gauss(0, 0.25) - self.rafale * 0.15
        # orage : de telle heure à telle heure, avec de la pluie à la fin
        self.orage = False
        if 'orage' in sc:
            a, b = sc['orage']
            h = self.minute_du_jour()
            if a <= h <= b:
                self.orage = True
                if h > b - 50:
                    self.pluie = min(1.0, self.pluie + 0.03)
        self.pluie = max(0.0, self.pluie - 0.006)
        self.maj()

    def maj(self):
        sc = self.sc
        h = self.minute_du_jour() / 60
        jour = math.sin((h - 9.5) / 24 * 2 * math.pi)                    # max vers 15 h 30
        self.temp = sc['tmax'] - 9 + 9 * jour - 6 * self.pluie
        self.hr = min(100, sc['hrmin'] + 32 * (1 - jour) / 2 * 1.6 + 45 * self.pluie)
        f_jour = 0.75 + 0.35 * max(0, jour)
        self.vent = max(0.0, sc['vent'] * f_jour * (1 + 0.12 * self.rafale) + (10 if self.orage else 0))
        self.rafales = self.vent * (1.35 + 0.1 * abs(self.rafale))
        self.nuit = not (6.5 <= h < 21.5)
        # humidité du combustible mort (%), indice de danger 0..1, niveau
        self.hcomb = max(3.0, 3 + 0.22 * self.hr - 0.12 * (self.temp - 20) + 12 * self.pluie)
        danger = (1.25 - self.hcomb / 25) * (0.55 + 0.45 * self.sech) * (1 + self.vent / 60)
        self.danger = max(0.0, min(1.0, danger / 1.6))
        self.niveau = NIVEAUX[min(5, int(self.danger * 6))]

    def minute_du_jour(self):
        return self.t % 1440

    def heure(self):
        m = self.minute_du_jour()
        return f'{m // 60:02d}:{m % 60:02d}'

    def jour(self):
        return self.t // 1440 + 1

    def vers(self):
        a = math.radians(self.de)
        return -math.sin(a), math.cos(a)

    def f_humidite(self, mouille=0.0):
        """Facteur de propagation dû à l'humidité du combustible (0 : trop humide pour brûler)."""
        m = self.hcomb + 25 * mouille
        return max(0.0, min(1.0, 1.25 - m / 25))

    def f_secheresse(self):
        return 0.6 + 0.6 * self.sech

    def vol_possible(self, kind):
        """Les avions ne volent pas de nuit ; au-delà d'un certain vent, ils restent au sol."""
        if self.nuit:
            return False
        lim = {'canadair': 75, 'dash': 70, 'helico': 85}.get(kind, 80)
        return self.rafales < lim

    def export(self):
        return dict(heure=self.heure(), jour=self.jour(), t=self.t, temp=round(self.temp, 1), hr=round(self.hr),
                    vent=round(self.vent), rafales=round(self.rafales), de=round(self.de), nuit=self.nuit,
                    pluie=round(self.pluie, 2), orage=self.orage, sech=round(self.sech, 2), hcomb=round(self.hcomb, 1),
                    danger=round(self.danger, 2), niveau=self.niveau, scenario=self.code, nomScenario=self.sc['nom'],
                    volCanadair=self.vol_possible('canadair'), volHelico=self.vol_possible('helico'))
