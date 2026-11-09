"""Tests du moteur : python -m unittest discover -s tests"""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from jeu.carte import Carte  # noqa: E402
from jeu.feu import BRULE, BRULE_FINI, INTACT  # noqa: E402
from jeu.partie import Partie  # noqa: E402

SANS_MOYENS = dict(ccf=0, fpt=0, bull=0, debr=0, vpc=0, ravit=0, vsav=0, helico=0, canadair=0, dash=0, dragon=0)


def partie_calme(moyens=None, graine=5, theme='gironde'):
    """Une partie sans départ de feu automatique."""
    p = Partie(graine, theme, 'chaud', moyens=moyens, premier_feu=False)
    p.prochain_depart = 10 ** 9
    p.meteo.sc.pop('bascule', None)
    return p


def cellule_centrale(p, fuel='P'):
    """Au cœur du massif : le centre des hexagones de ce combustible."""
    g = p.carte.g
    cells = [i for i in range(g.n) if p.carte.fuel[i] == fuel]
    mid = g.sous(sum(g.cx[i] for i in cells) / len(cells), sum(g.cy[i] for i in cells) / len(cells))
    return min((i for i in range(g.n) if p.carte.fuel[i] == fuel and p.carte.site_de[i] < 0), key=lambda i: g.distance(i, mid))


class TestCarte(unittest.TestCase):
    def test_reproductible(self):
        a, b = Carte(11, 'gironde'), Carte(11, 'gironde')
        self.assertEqual(a.fuel, b.fuel)
        self.assertEqual(a.route, b.route)

    def test_themes(self):
        for th in ('gironde',):
            c = Carte(3, th)
            self.assertGreaterEqual(len(c.sites), 8, th)
            self.assertGreaterEqual(len(c.casernes), 2, th)
            self.assertIsNotNone(c.aerodrome, th)
            self.assertTrue(any(c.ecopable(i) for i in range(c.g.n)), th)
            self.assertTrue(any(r == 1 for r in c.route), th)          # pistes DFCI

    def test_grille_inverse(self):
        g = Carte(2).g
        for i in range(0, g.n, 37):
            self.assertEqual(g.sous(g.cx[i], g.cy[i]), i)


class TestFeu(unittest.TestCase):
    def test_le_vent_pousse_le_feu(self):
        p = partie_calme(SANS_MOYENS)
        p.action(dict(type='vent', de=90, force=40))         # vent d'est : le feu part vers l'ouest
        i0 = cellule_centrale(p)
        p.feu.allumer(i0, t=0)
        g = p.carte.g
        for _ in range(round(90 / g.ech)):        # 90 min sur des hexagones de 300 m
            p.pas()
        touches = [i for i in range(g.n) if p.feu.etat[i] in (BRULE, BRULE_FINI)]
        self.assertGreater(len(touches), 8)
        ouest = sum(1 for i in touches if g.cx[i] < g.cx[i0] - g.hex_m)
        est = sum(1 for i in touches if g.cx[i] > g.cx[i0] + g.hex_m)
        self.assertGreater(ouest, 2 * est)

    def test_l_eau_abat_le_feu(self):
        p = partie_calme(SANS_MOYENS)
        i0 = cellule_centrale(p)
        p.feu.allumer(i0, t=0)
        for _ in range(8):
            p.pas()
        avant = p.feu.I[i0]
        p.feu.arroser(i0, 6000)
        self.assertLess(p.feu.I[i0], avant)
        self.assertGreater(p.feu.mouille[i0], 0.3)

    def test_rien_ne_brule_dans_l_eau(self):
        p = partie_calme(SANS_MOYENS)
        lac = next(i for i in range(p.carte.g.n) if p.carte.fuel[i] == 'L')
        self.assertFalse(p.feu.allumer(lac))


class TestCommandement(unittest.TestCase):
    def test_les_moyens_limitent_les_degats(self):
        """Sur plusieurs journées de canicule, le commandement brûle bien moins que le feu livré à lui-même."""
        avec = sans = 0.0
        for graine in (3, 7, 11):
            for moyens, cumul in ((None, 'avec'), (SANS_MOYENS, 'sans')):
                p = Partie(graine, 'gironde', 'canicule', moyens=moyens)
                for _ in range(300):
                    p.pas()
                if cumul == 'avec':
                    avec += p.feu.surface_ha()
                else:
                    sans += p.feu.surface_ha()
        self.assertLess(avec, sans * 0.6)

    def test_feu_naissant_attaque(self):
        p = partie_calme()
        i0 = cellule_centrale(p)
        p.action(dict(type='allumer', cell=i0))
        for _ in range(10):
            p.pas()
        engages = [u for u in p.moyens if u.mission and u.mission.get('type') in ('attaque', 'largage')]
        self.assertGreaterEqual(len(engages), 4)
        self.assertTrue(any('naissant' in j['txt'] for j in p.journal))

    def test_trace_du_largage(self):
        """L'interface fait tomber l'eau quand l'avion arrive : le trajet exporté passe par le point de largage, à sa minute."""
        p = Partie(7, 'landes', 'canicule')
        g, vus = p.carte.g, 0
        for _ in range(120):
            p.pas()
            for e in [e for e in p.evts if e['type'] == 'largage' and e['id'] > vus and e['produit'] == 'eau']:
                u = next(x for x in p.moyens if x.id == e['moyen'])
                cx = sum(g.cx[i] for i in e['cells']) / len(e['cells'])
                cy = sum(g.cy[i] for i in e['cells']) / len(e['cells'])
                self.assertTrue(any(t == e['t'] and abs(x - cx) < 2 and abs(y - cy) < 2 for t, x, y in u.export()['tr']))
                return
            vus = p.seq
        self.fail('aucun largage')


class TestOrdres(unittest.TestCase):
    def test_l_ia_respecte_vos_ordres(self):
        p = partie_calme()
        i0 = cellule_centrale(p)
        p.action(dict(type='allumer', cell=i0))
        ccf = [u for u in p.moyens if u.kind == 'ccf'][:2]
        loin = p.carte.casernes[0]['cell']
        r = p.action(dict(type='ordre', moyens=[u.id for u in ccf], ordre='poster', cell=loin))
        self.assertTrue(r['ok'])
        for _ in range(12):
            p.pas()
        for u in ccf:
            self.assertTrue(u.joueur)
            self.assertEqual(u.mission['type'], 'preposition')
        p.action(dict(type='ordre', moyens=[ccf[0].id], ordre='liberer'))
        self.assertFalse(ccf[0].joueur)

    def test_mode_manuel(self):
        p = partie_calme()
        p.action(dict(type='commandement', valeur='manuel'))
        p.action(dict(type='allumer', cell=cellule_centrale(p)))
        for _ in range(10):
            p.pas()
        self.assertFalse([u for u in p.moyens if u.mission], "en mode manuel, l'IA n'engage rien")
        canadair = next(u for u in p.moyens if u.kind == 'canadair')
        self.assertTrue(p.action(dict(type='ordre', moyens=[canadair.id], ordre='larguer', cell=cellule_centrale(p)))['ok'])
        self.assertFalse(p.action(dict(type='ordre', moyens=[canadair.id], ordre='attaquer', cell=cellule_centrale(p)))['ok'])


class TestSections(unittest.TestCase):
    def test_giff_quatre_camions_et_un_chef(self):
        p = partie_calme()
        chefs = [u for u in p.moyens if u.kind == 'vlhr']
        self.assertEqual(len(chefs), 4)
        for ch in chefs:
            self.assertEqual(sum(1 for u in p.moyens if u.kind == 'ccf' and u.groupe == ch.groupe), 4)

    def _feu_et_section(self):
        p = partie_calme()
        p.meteo.sc['vent'] = 5
        p.meteo.maj()
        i0 = cellule_centrale(p)
        p.feu.allumer(i0, t=0)
        for _ in range(6):
            p.pas()
        return p, i0, [u.id for u in p.moyens if u.kind == 'ccf' and u.groupe == 'GIFF 1']

    def test_le_cone_limite_l_arrosage(self):
        from jeu.moyens import dans_cone, CONE
        p, i0, ids = self._feu_et_section()
        g = p.carte.g
        r = p.action(dict(type='ordre', moyens=ids, ordre='arroser', cell=i0, dir=0))
        self.assertTrue(r['ok'])
        for _ in range(40):
            p.pas()
            for u in p.moyens:
                if u.id in ids and u.act and u.act.get('k') == 'arrose':
                    self.assertTrue(dans_cone(g, u.cell, u.act['c'], (0, CONE)))

    def test_ligne_d_arret_etalee(self):
        p, i0, ids = self._feu_et_section()
        g = p.carte.g
        a = g.sous(g.cx[i0] - 3 * g.hex_m, g.cy[i0] + 2 * g.hex_m)
        b = g.sous(g.cx[i0] + 3 * g.hex_m, g.cy[i0] + 2 * g.hex_m)
        p.action(dict(type='ordre', moyens=ids, ordre='ligne', cell=a, cell2=b))
        postes = {u.mission['poste'] for u in p.moyens if u.id in ids}
        self.assertGreaterEqual(len(postes), 3)

    def test_apercu_sans_effet(self):
        p, i0, ids = self._feu_et_section()
        avant = [(u.mission, u.joueur) for u in p.moyens]
        r = p.action(dict(type='apercu', moyens=ids, ordre='arroser', cell=i0, dir=90))
        self.assertTrue(r['ok'])
        self.assertEqual(len(r['moyens']), 4)
        self.assertTrue(all(e['chemin'] and e['portee'] for e in r['moyens']))
        self.assertEqual(avant, [(u.mission, u.joueur) for u in p.moyens])

    def test_axe_du_canadair(self):
        p, i0, _ = self._feu_et_section()
        g = p.carte.g
        can = next(u for u in p.moyens if u.kind == 'canadair')
        r = p.action(dict(type='apercu', moyens=[can.id], ordre='larguer', cell=i0, dir=0))
        cells = r['moyens'][0]['largage']
        self.assertEqual(len(cells), 2)
        self.assertLess(abs(g.cy[cells[0]] - g.cy[cells[1]]), 1)      # axe est-ouest : même ligne
        self.assertIsNotNone(r['moyens'][0]['ecopage'])


class TestAutoprotection(unittest.TestCase):
    def _encercler(self, eau):
        from jeu.moyens import RESERVE
        p = partie_calme()
        g = p.carte.g
        i0 = cellule_centrale(p)
        u = next(m for m in p.moyens if m.kind == 'ccf')
        u.cell, u.x, u.y, u.eau = i0, g.cx[i0], g.cy[i0], eau
        u.mission, u.joueur = dict(type='preposition', poste=i0, label='test'), True
        for j in [i0] + g.voisins(i0):
            if p.feu.allumer(j, t=0):
                p.feu.I[j] = 0.95
        return p, u

    def test_l_autoprotection_sauve_le_camion(self):
        from jeu.moyens import RESERVE
        p, u = self._encercler(RESERVE)
        u.pas(p)
        self.assertTrue(u.auto)
        self.assertFalse(u.hs)
        self.assertEqual(u.eau, RESERVE - 120)
        self.assertTrue(any('autoprotection' in j['txt'] for j in p.journal))

    def test_sans_reserve_le_camion_est_perdu(self):
        p, u = self._encercler(0)
        u.pas(p)
        self.assertTrue(u.hs)
        self.assertEqual(p.stats.get('perdus'), 1)
        self.assertFalse(p.action(dict(type='ordre', moyens=[u.id], ordre='poster', cell=u.base))['ok'])

    def test_les_lances_ne_touchent_pas_la_reserve(self):
        from jeu.moyens import RESERVE
        p = partie_calme()
        g = p.carte.g
        i0 = cellule_centrale(p)
        u = next(m for m in p.moyens if m.kind == 'ccf')
        voisin = g.voisins(i0)[0]
        u.cell, u.eau = i0, RESERVE + 250
        p.feu.allumer(voisin, t=0)
        p.feu.I[voisin] = 0.5
        for _ in range(3):
            u._arroser_voisins(p, 0.8)
        self.assertGreaterEqual(u.eau, RESERVE)


class TestEngins(unittest.TestCase):
    def test_dragon_et_vsav_secourent(self):
        p = partie_calme()
        g = p.carte.g
        i0 = cellule_centrale(p)
        route = min((i for i in range(g.n) if p.carte.route[i] >= 2 and p.carte.site_de[i] < 0), key=lambda i: g.distance(i, i0))
        p.ajouter_blesses(i0, 4, 'equipage', 'danger', 'test équipage')
        p.ajouter_blesses(route, 2, 'pompier', 'leger', 'test léger')
        for _ in range(3):
            p.pas()
        missions = {u.kind: u.mission['type'] for u in p.moyens if u.mission and u.mission.get('type') == 'secours'}
        self.assertEqual(missions.get('dragon'), 'secours')
        self.assertEqual(missions.get('vsav'), 'secours')
        for _ in range(150):
            p.pas()
        self.assertEqual(p.secourus['hopital'], 6)

    def test_ravitailleur(self):
        p = partie_calme()
        g = p.carte.g
        u = next(m for m in p.moyens if m.kind == 'ccf')
        i0 = cellule_centrale(p)
        p.action(dict(type='ordre', moyens=[u.id], ordre='poster', cell=i0))
        for _ in range(30):
            p.pas()
        u.carb = 0.2 * u.carb_max
        for _ in range(round(60 / g.ech)):
            p.pas()
        self.assertGreater(u.carb, 0.9 * u.carb_max)

    def test_fpt_reste_sur_les_routes(self):
        from jeu.moyens import vitesse_sol
        p = partie_calme()
        c = p.carte
        piste = next(i for i in range(c.g.n) if c.route[i] == 1)
        foret = next(i for i in range(c.g.n) if c.route[i] == 0 and c.fuel[i] == 'P' and c.site_de[i] < 0)
        route = next(i for i in range(c.g.n) if c.route[i] >= 2)
        self.assertEqual(vitesse_sol('fpt', c, p.feu, piste), 0)
        self.assertEqual(vitesse_sol('fpt', c, p.feu, foret), 0)
        self.assertGreater(vitesse_sol('fpt', c, p.feu, route), 2.5 * c.g.ech)

    def test_debroussaillage_hors_risque_severe(self):
        p = Partie(3, 'gironde', 'calme', premier_feu=False)
        p.prochain_depart = 10 ** 9
        for _ in range(240):
            p.pas()
        self.assertGreater(p.stats.get('debroussaille', 0), 0)
        self.assertIn('Y', p.carte.fuel + ['?'] if isinstance(p.carte.fuel, list) else p.carte.fuel)

    def test_poste_de_commandement(self):
        p = partie_calme(dict(SANS_MOYENS, fpt=0, debr=0, ravit=0, vsav=0, dragon=0, vpc=1))
        p.action(dict(type='vent', de=90, force=35))
        i0 = cellule_centrale(p)
        p.action(dict(type='allumer', cell=i0))
        for _ in range(150):
            p.pas()
        self.assertTrue(any(f.get('pc') for f in p.ia.feux.values()) or any('📡' in j['txt'] for j in p.journal))


class TestEvacuation(unittest.TestCase):
    def test_evacuer_puis_arriver(self):
        p = partie_calme(SANS_MOYENS)
        s = next(s for s in p.carte.sites if s['type'] == 'village')
        self.assertTrue(p.action(dict(type='evacuer', site=s['id']))['ok'])
        for _ in range(240):
            p.pas()
        e = p.evac.sites[s['id']]
        self.assertEqual(e['reste'] + e['route'], 0)
        self.assertEqual(e['arrives'], s['pop'])

    def test_confiner(self):
        p = partie_calme(SANS_MOYENS)
        s = next(s for s in p.carte.sites if s['type'] == 'camping')
        p.action(dict(type='confiner', site=s['id']))
        self.assertEqual(p.evac.sites[s['id']]['confines'], s['pop'])

    def test_la_fumee_suit_le_vent(self):
        p = partie_calme(SANS_MOYENS)
        p.action(dict(type='vent', de=90, force=40))
        i0 = cellule_centrale(p)
        p.feu.allumer(i0, t=0)
        for _ in range(15):
            p.pas()
        g = p.carte.g
        ouest = sum(1 for i in p.evac.fumee if g.cx[i] < g.cx[i0] - g.hex_m)
        est = sum(1 for i in p.evac.fumee if g.cx[i] > g.cx[i0] + g.hex_m)
        self.assertGreater(ouest, est)


class TestVoies(unittest.TestCase):
    def test_reseau_propre(self):
        for th in ('gironde',):
            c = Carte(4, th)
            aretes = set()
            for r in c.routes_poly:
                self.assertGreaterEqual(len(r['p']), 2)
                for a, b in zip(r['c'], r['c'][1:]):
                    e = (min(a, b), max(a, b))
                    self.assertNotIn(e, aretes, 'tronçon dessiné deux fois')
                    aretes.add(e)
                    self.assertEqual(c.g.distance(a, b), 1)
                for i in r['c']:
                    self.assertNotIn(c.fuel[i], 'ML', 'une voie dans l\'eau')

    def test_bonus_malus(self):
        p = partie_calme()
        c, g = p.carte, p.carte.g
        piste = next(i for i in range(g.n) if c.route[i] == 1)
        route = next(i for i in range(g.n) if c.route[i] >= 2 and c.site_de[i] < 0)
        u = next(m for m in p.moyens if m.kind == 'ccf')
        # piste étroite : à trois engins on n'avance plus qu'au pas
        p.trafic_engins = {piste: 3}
        self.assertLess(u.facteur_voie(p, piste), 0.5)
        # route : les convois font des bouchons, la fumée ralentit
        p.trafic_convois = {route: 2}
        self.assertLess(u.facteur_voie(p, route), 0.5)
        # piste qui longe un feu intense : fermée aux camions
        p.danger = {piste}
        u.cell = next(j for j in g.voisins(piste) if j != piste)
        dest = next(j for j in g.anneau(piste, 3) if g.distance(j, piste) == 3 and c.fuel[j] not in 'MLR')
        u.aller(p, dest)
        self.assertNotIn(piste, u.chemin)

    def test_pas_de_convoi_sur_les_pistes(self):
        p = partie_calme(SANS_MOYENS)
        for s in p.carte.sites:
            if s['type'] in ('village', 'camping'):
                p.action(dict(type='evacuer', site=s['id']))
        for _ in range(60):
            p.pas()
            for cv in p.evac.convois:
                self.assertTrue(p.carte.route[cv.cell] >= 2 or p.carte.site_de[cv.cell] >= 0)


class TestExport(unittest.TestCase):
    def test_json(self):
        p = Partie(4, 'gironde', 'mistral')
        for _ in range(60):
            p.pas()
        json.dumps(p.export_carte())
        e = p.export_etat()
        txt = json.dumps(e)
        self.assertEqual(len(e['etat']), p.carte.g.n)
        self.assertLess(len(txt), 400000)


if __name__ == '__main__':
    unittest.main()
