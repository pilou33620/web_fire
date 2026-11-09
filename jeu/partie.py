"""Une partie : la carte, la météo, le feu, les moyens et le commandement, avancés minute par minute.

C'est la seule porte d'entrée pour le serveur (server.py) comme pour la version navigateur (Pyodide) :
  Partie(...).pas()  ·  .export_carte()  ·  .export_etat()  ·  .action(dict)
"""
import math
import random

from . import ordres
from .carte import Carte
from .combustibles import COMB, EAU, HABITE
from .feu import BRULE, BRULE_FINI, ETEINT, NOYE, INTACT, Feu
from .ia import Commandement
from .evacuation import Evacuation
from .meteo import Meteo
from .moyens import Moyen

MOYENS_DEFAUT = dict(ccf=16, bull=2, helico=3, canadair=4, dash=1)
LIMITES = dict(ccf=(0, 60), bull=(0, 8), helico=(0, 12), canadair=(0, 16), dash=(0, 4))


class Partie:
    def __init__(self, graine=7, theme='landes', scenario='canicule', moyens=None, doctrine='equilibree',
                 heure0=11 * 60, premier_feu=True):
        self.graine, self.theme, self.scenario = graine, theme, scenario
        self.rnd = random.Random(graine * 1009 + 3)
        self.carte = Carte(graine, theme)
        self.meteo = Meteo(scenario, graine, heure0)
        self.feu = Feu(self.carte, self.meteo, self.rnd)
        self.t = 0
        self.journal = []
        self.evts = []
        self.seq = 0
        self.evacues = set()
        self.stats = dict(largages=0, defriche=0, departs=0, renforts=0)
        self.config_moyens = {k: max(LIMITES[k][0], min(LIMITES[k][1], int(v)))
                              for k, v in {**MOYENS_DEFAUT, **(moyens or {})}.items() if k in LIMITES}
        self.moyens = []
        self._creer_moyens()
        self.ia = Commandement(self, doctrine)
        self.evac = Evacuation(self)
        self.prochain_depart = 25 if premier_feu else self._delai_depart()
        self.renforts_attente = []
        self.journal_ajout('info', f"{self.meteo.heure()} — {self.meteo.sc['nom']}. Risque {self.meteo.niveau.lower()}.")
        self._pe_cache = {}
        self.danger = set()
        self.trafic_engins, self.trafic_convois = {}, {}

    # ------------------------------------------------------------------ moyens
    def _creer_moyens(self):
        c, cm = self.carte, self.config_moyens
        cas = c.casernes or [dict(id=0, nom='Caserne', cell=c.g.n // 2)]
        for k in range(cm['ccf']):
            ca = cas[k % len(cas)]
            giff = k // 4 + 1
            u = Moyen(len(self.moyens), 'ccf', f'CCF {giff}.{k % 4 + 1}', ca['cell'], c)
            u.groupe = f'GIFF {giff}'
            self.moyens.append(u)
        for k in range(cm['bull']):
            ca = cas[(k + 1) % len(cas)]
            self.moyens.append(Moyen(len(self.moyens), 'bull', f'Bull {k + 1}', ca['cell'], c))
        heli = c.helistation if c.helistation is not None else cas[0]['cell']
        for k in range(cm['helico']):
            self.moyens.append(Moyen(len(self.moyens), 'helico', f'HBE {k + 1}', heli, c))
        aero = c.aerodrome if c.aerodrome is not None else cas[0]['cell']
        for k in range(cm['canadair']):
            self.moyens.append(Moyen(len(self.moyens), 'canadair', f'Pélican {31 + k}', aero, c))
        for k in range(cm['dash']):
            self.moyens.append(Moyen(len(self.moyens), 'dash', f'Milan {71 + k}', aero, c))

    def demander_renfort(self, nb):
        self.renforts_attente.append((self.t + 45, nb))
        self.journal_ajout('cos', f"Demande de renforts : une colonne de {nb} CCF arrivera dans 45 min.")

    def _arrivee_renforts(self):
        c, g = self.carte, self.carte.g
        for item in list(self.renforts_attente):
            t, nb = item
            if self.t < t:
                continue
            self.renforts_attente.remove(item)
            entrees = [i for i in range(g.n) if g.col_row(i)[0] == g.cols - 1 and c.route[i] >= 2] or \
                      [i for i in range(g.n) if g.col_row(i)[0] == g.cols - 1 and c.fuel[i] not in EAU]
            e = entrees[0] if entrees else (c.casernes[0]['cell'] if c.casernes else 0)
            self.stats['renforts'] += 1
            for k in range(nb):
                u = Moyen(len(self.moyens), 'ccf', f'CCF R{self.stats["renforts"]}.{k + 1}', e, c)
                u.renfort = True
                u.groupe = f'Colonne {self.stats["renforts"]}'
                self.moyens.append(u)
            self.journal_ajout('cos', f"La colonne de renfort n° {self.stats['renforts']} arrive ({nb} CCF).")

    def point_eau_proche(self, cell):
        """Point d'eau terrestre le plus proche (borne, citerne DFCI, caserne)."""
        g = self.carte.g
        pts = self.carte.points_eau + [ca['cell'] for ca in self.carte.casernes]
        if not pts:
            return None
        return min(pts, key=lambda q: g.distance(q, cell))

    def eau_aerienne(self, u):
        """Où un aéronef va chercher son eau : un plan d'eau écopable (Canadair) ou n'importe quelle eau (hélico)."""
        c, g = self.carte, self.carte.g
        key = (u.kind, u.cell // 3)
        if u.kind == 'canadair':
            cand = [i for i in range(g.n) if c.ecopable(i)]
        else:
            cand = [i for i in range(g.n) if c.pompable(i)]
            if c.helistation is not None:
                cand.append(c.helistation)
        if not cand:
            return None
        # au plus près de la cible si on la connaît, sinon de l'avion
        ref = u.cell
        if u.mission and 'feu' in u.mission:
            f = self.ia.feux.get(self.ia.racine(u.mission['feu']))
            if f and f.get('tete') is not None:
                ref = f['tete']
        return min(cand, key=lambda i: g.distance(i, ref) * 2 + g.distance(i, u.cell))

    # ------------------------------------------------------------------ journal et événements
    def journal_ajout(self, kind, txt):
        self.seq += 1
        self.journal.append(dict(id=self.seq, t=self.meteo.t, h=self.meteo.heure(), k=kind, txt=txt))
        self.journal = self.journal[-150:]

    def evenement(self, type_, **kw):
        self.seq += 1
        self.evts.append(dict(id=self.seq, t=self.meteo.t, type=type_, **kw))
        self.evts = self.evts[-80:]

    # ------------------------------------------------------------------ départs de feu et détection
    def _delai_depart(self):
        taux = self.meteo.sc.get('departs', 1.0) * (0.3 + self.meteo.danger)
        return self.t + int(self.rnd.expovariate(taux / 140)) + 10

    def _departs(self):
        c, g, met = self.carte, self.carte.g, self.meteo
        # foudre
        if met.orage and self.rnd.random() < 0.035:
            cand = [i for i in self.rnd.sample(range(g.n), 60) if c.fuel[i] in 'PJFQG']
            if cand:
                i = cand[0]
                if self.feu.allumer(i, cause='foudre', t=self.t):
                    self.stats['departs'] += 1
                    self.evenement('foudre', cell=i)
        if met.nuit or self.t < self.prochain_depart:
            return
        self.prochain_depart = self._delai_depart()
        # causes humaines : bords de routes, abords des villages, travaux en forêt
        for _ in range(80):
            i = self.rnd.randrange(g.n)
            if not COMB[c.fuel[i]]['brule'] or c.fuel[i] in HABITE or self.feu.etat[i] != INTACT:
                continue
            pres = c.route[i] >= 2 or any(c.site_de[j] >= 0 or c.route[j] >= 2 for j in g.voisins(i))   # malus des routes : mégots, véhicules
            if pres or self.rnd.random() < 0.15:
                cause = self.rnd.choice(['mégot jeté d\'une voiture', 'travaux agricoles', 'barbecue', 'ligne électrique',
                                         'engin forestier', 'origine inconnue', 'malveillance présumée'])
                if self.feu.allumer(i, cause=cause, t=self.t):
                    self.stats['departs'] += 1
                break

    def _detection(self):
        feu, c, g, met = self.feu, self.carte, self.carte.g, self.meteo
        avions = [u for u in self.moyens if not u.terre and u.mission and u.mission.get('type') == 'gaar']
        for d in feu.departs:
            if d['detecte']:
                continue
            cells = [i for i in feu.brulant if feu.foyer[i] == d['id']]
            if not cells:
                cells = [d['cell']]
            n = len(cells)
            i0 = d['cell']
            p, qui = 0.0, None
            for v in c.vigies:
                if g.distance(v, i0) <= 22:
                    p, qui = max(p, 0.10 + 0.02 * n), 'la vigie'
            if any(c.site_de[j] >= 0 for j in g.anneau(i0, 4)):
                if 0.08 + 0.02 * n > p:
                    p, qui = 0.08 + 0.02 * n, 'des habitants'
            for u in avions:
                if g.distance(u.cell, i0) <= 9:
                    p, qui = 0.7, u.nom
            p = max(p, 0.02 + 0.01 * n)
            if met.nuit:
                p *= 0.5
            if self.rnd.random() < p:
                d['detecte'], d['t_detect'] = True, self.t
                qui = qui or 'un promeneur'
                lieu = self.ia._nommer(i0).replace('Feu ', '').replace('près de ', 'près de ')
                self.journal_ajout('alerte', f"🔥 Départ de feu signalé {lieu} par {qui} ({d['cause']}).")
                self.evenement('alerte', cell=i0)

    # ------------------------------------------------------------------ pas d'une minute
    def pas(self):
        self.t += 1
        self.meteo.pas()
        self._departs()
        self._arrivee_renforts()
        for u in self.moyens:
            if u.terre and u.mission and u.mission.get('type') == 'defense' and u.cell == u.mission.get('poste'):
                self.feu.defense[u.cell] += 1
        self.feu.pas(self.t)
        for k in self.feu.nouvelles_sautes:
            self.evenement('saute', cell=k)
        self.feu.nouvelles_sautes = []
        self._detection()
        g, feu = self.carte.g, self.feu
        self.danger = set()
        self.trafic_engins, self.trafic_convois = {}, {}
        for u in self.moyens:
            if u.terre:
                self.trafic_engins[u.cell] = self.trafic_engins.get(u.cell, 0) + 1
        for cv in self.evac.convois:
            if cv.etat != 'arrive':
                self.trafic_convois[cv.cell] = self.trafic_convois.get(cv.cell, 0) + 1
        for i in feu.brulant:
            if feu.I[i] > 0.55:
                self.danger.update(g.voisins(i))
        self.ia.pas()
        for u in self.moyens:
            u.pas(self)
        self.evac.pas()

    # ------------------------------------------------------------------ actions du joueur
    def action(self, a):
        typ = a.get('type')
        if typ == 'allumer':
            i = int(a['cell'])
            if self.feu.allumer(i, cause='départ provoqué (joueur)', t=self.t):
                self.stats['departs'] += 1
                if a.get('detecte', True):
                    d = self.feu.departs[self.feu.foyer[i]]
                    d['detecte'], d['t_detect'] = True, self.t
                    self.journal_ajout('alerte', f"🔥 Départ de feu provoqué {self.ia._nommer(i).replace('Feu ', '')}.")
                return dict(ok=True)
            return dict(ok=False, erreur='Rien à brûler ici.')
        if typ == 'orage':
            h = self.meteo.minute_du_jour()
            self.meteo.sc['orage'] = (h, h + 120)
            self.journal_ajout('info', 'Un orage sec éclate : la foudre frappe la forêt.')
            return dict(ok=True)
        if typ == 'vent':
            if 'de' in a:
                self.meteo.de_base = float(a['de']) % 360
                self.meteo.sc['de'] = self.meteo.de_base
                self.meteo.sc.pop('bascule', None)
            if 'force' in a:
                self.meteo.sc['vent'] = max(0, min(110, float(a['force'])))
            self.meteo.maj()
            self.journal_ajout('info', f"Le vent tourne : du {self.meteo.de}° à {round(self.meteo.vent)} km/h.")
            return dict(ok=True)
        if typ == 'ordre':
            return ordres.donner(self, [int(x) for x in a.get('moyens', [])], a.get('ordre'),
                                 None if a.get('cell') is None else int(a['cell']),
                                 None if a.get('cell2') is None else int(a['cell2']))
        if typ == 'commandement':
            v = 'manuel' if a.get('valeur') == 'manuel' else 'auto'
            if v != self.ia.mode:
                self.ia.mode = v
                if v == 'manuel':
                    self.journal_ajout('joueur', "Vous prenez le commandement des opérations. Les moyens gardent leur mission en cours ; à vous de donner les ordres.")
                    for u in self.moyens:
                        if u.mission and u.mission.get('type') in ('preposition', 'gaar'):
                            u.mission = None
                else:
                    self.journal_ajout('cos', "L'IA reprend le commandement des opérations (vos ordres en cours sont respectés).")
            return dict(ok=True)
        if typ in ('evacuer', 'confiner'):
            sid = int(a['site'])
            ok = (self.evac.evacuer if typ == 'evacuer' else self.evac.confiner)(sid, par='joueur')
            return dict(ok=ok, erreur=None if ok else ('Aucun itinéraire sûr pour évacuer.' if typ == 'evacuer' else 'Déjà fait.'))
        if typ == 'renfort':
            if sum(1 for _ in self.renforts_attente) >= 2:
                return dict(ok=False, erreur='Deux colonnes sont déjà en route.')
            self.demander_renfort(4)
            return dict(ok=True)
        if typ == 'doctrine':
            self.ia.doctrine = a.get('valeur', 'equilibree')
            self.journal_ajout('cos', f"Doctrine du commandement : {self.ia.doctrine}.")
            return dict(ok=True)
        return dict(ok=False, erreur='action inconnue')

    # ------------------------------------------------------------------ export
    def export_carte(self):
        d = self.carte.export()
        d['moyensConfig'] = self.config_moyens
        return d

    def bilan(self):
        feu, c = self.feu, self.carte
        sites = []
        for s in c.sites:
            brule = sum(1 for i in s['cells'] if feu.etat[i] in (BRULE, BRULE_FINI))
            part = brule / max(1, len(s['cells']))
            menace = min([f['menaces'].get(s['id'], 9999) for f in self.ia.feux.values() if f['statut'] != 'éteint'] + [9999])
            etat = 'détruit' if part > 0.6 else 'touché' if brule else 'menacé' if menace < 150 else 'sauf'
            sites.append(dict(id=s['id'], etat=etat, part=round(part, 2), eta=menace if menace < 9999 else None,
                              evacue=s['id'] in self.evacues, evac=self.evac.sites.get(s['id'], {}).get('statut'),
                              defenseurs=sum(1 for u in self.moyens if u.mission and u.mission.get('site') == s['id'])))
        ha = feu.surface_ha()
        perdus = sum(s['part'] * c.sites[s['id']]['valeur'] for s in sites)
        pers = self.evac.bilan()
        score = max(0, round(100 - 12 * math.log10(1 + ha) - perdus * 0.6 - pers['exposes'] / 40))
        return sites, dict(ha=round(ha, 1), litres=round(feu.litres), litresRetardant=round(feu.litres_ret),
                           largages=self.stats['largages'], departs=self.stats['departs'], renforts=self.stats['renforts'],
                           defriche=self.stats['defriche'], score=score,
                           personnes=pers,
                           feuxActifs=sum(1 for f in self.ia.feux.values() if f['statut'] not in ('éteint',)),
                           maisons=round(sum(s['part'] * c.sites[s['id']]['pop'] / 3 for s in sites
                                             if c.sites[s['id']]['type'] in ('ville', 'village'))))

    def export_etat(self, depuis=0):
        feu, n = self.feu, self.carte.g.n
        code = {INTACT: '.', BRULE: 'f', BRULE_FINI: 'b', ETEINT: 'e', NOYE: 'n'}
        etat = ''.join(code[feu.etat[i]] if not (feu.etat[i] == INTACT and self.carte.fuel[i] == 'X' and
                                                 i != self.carte.helistation) else 'x' for i in range(n))
        sites, stats = self.bilan()
        return dict(
            t=self.t, meteo=self.meteo.export(), etat=etat,
            feu=[[i, round(feu.I[i] * 100), round(feu.prog[i] * 100), feu.dfrom[i]] for i in feu.brulant],
            prog=[[i, round(feu.prog[i] * 100)] for i in range(n) if feu.etat[i] in (ETEINT, NOYE)],
            mouille=[[i, round(feu.mouille[i] * 100)] for i in range(n) if feu.mouille[i] > 0.08],
            ret=[i for i in range(n) if feu.ret[i] > 0],
            moyens=[u.export() for u in self.moyens], feux=self.ia.export(), sites=sites, stats=stats,
            journal=[j for j in self.journal if j['id'] > depuis], evts=[e for e in self.evts if e['id'] > depuis],
            seq=self.seq, doctrine=self.ia.doctrine, commandement=self.ia.mode, evac=self.evac.export(),
            departs=[dict(cell=d['cell'], detecte=d['detecte']) for d in feu.departs],
        )
