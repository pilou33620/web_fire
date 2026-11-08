"""Tests du moteur : python -m unittest discover -s tests"""
import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from jeu.carte import Carte  # noqa: E402
from jeu.feu import BRULE, BRULE_FINI, INTACT  # noqa: E402
from jeu.partie import Partie  # noqa: E402

SANS_MOYENS = dict(ccf=0, bull=0, helico=0, canadair=0, dash=0)


def partie_calme(moyens=None, graine=5, theme='landes'):
    """Une partie sans départ de feu automatique."""
    p = Partie(graine, theme, 'chaud', moyens=moyens, premier_feu=False)
    p.prochain_depart = 10 ** 9
    p.meteo.sc.pop('bascule', None)
    return p


def cellule_centrale(p, fuel='P'):
    g = p.carte.g
    mid = g.sous(g.largeur / 2, g.hauteur / 2)
    return min((i for i in range(g.n) if p.carte.fuel[i] == fuel and p.carte.site_de[i] < 0), key=lambda i: g.distance(i, mid))


class TestCarte(unittest.TestCase):
    def test_reproductible(self):
        a, b = Carte(11, 'landes'), Carte(11, 'landes')
        self.assertEqual(a.fuel, b.fuel)
        self.assertEqual(a.route, b.route)

    def test_themes(self):
        for th in ('landes', 'mediterranee'):
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
        for _ in range(90):
            p.pas()
        g = p.carte.g
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
                p = Partie(graine, 'landes', 'canicule', moyens=moyens)
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


class TestExport(unittest.TestCase):
    def test_json(self):
        p = Partie(4, 'mediterranee', 'mistral')
        for _ in range(60):
            p.pas()
        json.dumps(p.export_carte())
        e = p.export_etat()
        txt = json.dumps(e)
        self.assertEqual(len(e['etat']), p.carte.g.n)
        self.assertLess(len(txt), 400000)


if __name__ == '__main__':
    unittest.main()
