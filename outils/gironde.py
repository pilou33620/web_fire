#!/usr/bin/env python3
"""Extrait la Gironde de la carte de web_conquete (vraie géographie, cases de 1 km) vers rust/feu/src/gironde.json.

    python outils/gironde.py ../web_conquete

À relancer seulement si la carte de web_conquete change. Le jeu n'a pas besoin de web_conquete : il lit rust/feu/src/gironde.json (compilé dans le moteur).
Le terrain de web_conquete (une carte d'époque : landes, marais) est traduit en combustibles d'aujourd'hui :
la lande de Gascogne est devenue la pinède des Landes.
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MARGE = 3   # km autour de la Gironde

# Sites que web_conquete n'a pas : (type, nom, lat, lon)
SITES = [
    ('camping', 'Camping de Lacanau-Océan', 45.000, -1.195), ('camping', 'Camping de Carcans-Plage', 45.080, -1.185),
    ('camping', "Camping d'Hourtin-Plage", 45.220, -1.160), ('camping', 'Camping de Montalivet', 45.375, -1.145),
    ('camping', 'Campings du Pyla', 44.585, -1.205), ('camping', 'Camping de Soulac', 45.495, -1.120),
    ('usine', 'Papeterie de Facture', 44.640, -0.980), ('usine', "Dépôt pétrolier d'Ambès (Seveso)", 44.930, -0.530),
    ('usine', 'Poudrerie de Saint-Médard (Seveso)', 44.880, -0.760),
    ('reserve', "Réserve naturelle de l'étang de Cousseau", 45.060, -1.165), ('reserve', "Prés salés d'Arès", 44.770, -1.120),
    ('patrimoine', 'Dune du Pilat', 44.589, -1.213), ('patrimoine', 'Château de Roquetaillade', 44.490, -0.250),
    ('patrimoine', 'Abbaye de La Sauve-Majeure', 44.770, -0.315),
]
AERODROME = ('Aéroport de Bordeaux-Mérignac', 44.830, -0.700)


def main():
    sys.path.insert(0, os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, '..', 'web_conquete')))
    from game import geo
    from game.geo import GeoMap, fbm
    g = GeoMap()
    d33 = g.dept_codes.index('33') + 1
    cs = [c for c in range(g.W * g.H) if g.dept[c] == d33]
    x0, x1 = min(c % g.W for c in cs) - MARGE, max(c % g.W for c in cs) + MARGE + 1
    y0, y1 = min(c // g.W for c in cs) - MARGE, max(c // g.W for c in cs) + MARGE + 1
    W, H = x1 - x0, y1 - y0

    landes = bytearray(g.W * g.H)
    g._raster_ring([g.xy(la, lo) for la, lo in geo.LANDES_POLY], lambda c: landes.__setitem__(c, 1))
    T = geo
    fuel, alt = [], []
    for y in range(y0, y1):
        lf, la = [], []
        for x in range(x0, x1):
            c = g.cell(x, y)
            t, riv = g.terrain[c], g.river[c]
            lat, lon = g.lat_of(y + 0.5), g.lon_of(x + 0.5)
            n = fbm(x / 3, y / 3, 41)     # le mélange fin des combustibles
            e = max(0.0, g._elev3(lat, lon) + (fbm(x / 22, y / 22, 11, 4) - 0.5) * 0.42)
            a = 8 + 240 * e
            if t == T.T_OCEAN:
                f, a = 'M', 0
            elif t == T.T_EAU:
                f, a = 'L', 1
            elif riv and riv <= 2:
                f, a = 'R', a - 6
            elif t == T.T_DUNES:
                f, a = 'D', a + 35
            elif t in (T.T_FORET, T.T_LANDE):
                pin = landes[c] or lon < -0.6
                f = ('J' if n > 0.72 else 'C' if n < 0.18 else 'P') if pin else ('F' if n > 0.3 else 'P')
            elif t == T.T_MARAIS:
                f = 'J' if landes[c] else 'H'
            elif t == T.T_VIGNE:
                f = 'V'
            elif t == T.T_COLLINES:
                f = 'F' if n > 0.5 else 'C'
            else:
                f = 'F' if n > 0.68 else 'H' if n < 0.3 else 'C'
            if riv == 3 and f in 'CHV':
                f = 'F'                   # ripisylve des petites rivières
            lf.append(f)
            la.append(chr(48 + max(0, min(74, round(a / 4)))))   # 4 m par cran
        fuel.append(''.join(lf)); alt.append(''.join(la))

    dedans = lambda x, y: x0 <= x < x1 and y0 <= y < y1
    km = lambda x, y: (round(x - x0, 2), round(y - y0, 2))
    lieux = [dict(nom=n['name'], type='patrimoine' if n['type'] == 'fort' else n['type'], xy=km(n['x'], n['y']), d33=n['dept'] == '33')
             for n in g.nodes if dedans(n['x'], n['y'])]
    ids = {n['id'] for n in g.nodes if dedans(n['x'], n['y'])}
    routes = []
    for e in g.edges:
        if int(e['a']) in ids and int(e['b']) in ids and str(e.get('ferry')) != 'True':
            pts = e['pts'] if isinstance(e['pts'], list) else eval(e['pts'])
            routes.append(dict(n=3 if str(e['royal']) == 'True' else 2, p=[km(x, y) for x, y in pts]))
    sites = [dict(type=t, nom=nom, xy=km(*g.xy(la, lo))) for t, nom, la, lo in SITES]
    eaux = []
    for w in g.src['waters']:
        if w['name']:
            pts = [g.xy(la, lo) for lo, la in w['ring']]
            cx, cy = sum(p[0] for p in pts) / len(pts), sum(p[1] for p in pts) / len(pts)
            if dedans(cx, cy):
                eaux.append(dict(nom=w['name'], xy=km(cx, cy)))
    d = dict(W=W, H=H, FUEL=list(fuel), ALT=list(alt), LIEUX=lieux, ROUTES=routes, SITES=sites, EAUX=eaux,
             AERODROME=(AERODROME[0], km(*g.xy(AERODROME[1], AERODROME[2]))))
    with open(os.path.join(ROOT, 'rust', 'feu', 'src', 'gironde.json'), 'w', encoding='utf-8') as fh:
        fh.write(json.dumps(d, ensure_ascii=False, separators=(',', ':')) + '\n')
    print(f'{W} × {H} km, {len(lieux)} lieux, {len(routes)} routes, {len(eaux)} plans d\'eau')


if __name__ == '__main__':
    main()
