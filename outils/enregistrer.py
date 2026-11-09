#!/usr/bin/env python3
"""Enregistre une journée jouée par le vrai moteur, pour la démo rejouée quand Python ne peut pas tourner dans la page.

    python outils/enregistrer.py --sortie web/demo/rejeu.json --graine 7 --scenario canicule --minutes 600 --pas 3
"""
import argparse
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, ROOT)
from jeu.meteo import SCENARIOS  # noqa: E402
from jeu.partie import LIMITES, MOYENS_DEFAUT, Partie  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--sortie', default=os.path.join(ROOT, 'web', 'demo', 'rejeu.json'))
    ap.add_argument('--graine', type=int, default=7)
    ap.add_argument('--theme', default='landes')
    ap.add_argument('--scenario', default='canicule')
    ap.add_argument('--minutes', type=int, default=600)
    ap.add_argument('--pas', type=int, default=3)
    a = ap.parse_args()
    p = Partie(a.graine, a.theme, a.scenario)
    carte = p.export_carte()                  # la carte du début (les bulldozers la modifient ensuite)
    images, journal, evts, seq = [], [], [], 0
    for m in range(a.minutes + 1):
        if m % a.pas == 0:
            e = p.export_etat(seq)
            journal += e.pop('journal')
            evts += [v for v in e.pop('evts') if v['type'] == 'largage']
            seq = e['seq']
            for f in e['feux']:                       # on allège : le front sert au calque de prévision
                f['front'] = f['front'][:30]
                f['flancs'] = []
            for u in e['moyens']:
                u.pop('v', None)
            images.append(e)
        p.pas()
    infos = dict(scenarios={k: v['nom'] for k, v in SCENARIOS.items()}, themes={'landes': 'Landes de Gascogne', 'mediterranee': 'Massif méditerranéen'},
                 limites=LIMITES, defaut=MOYENS_DEFAUT, vitesses=[0.25, 0.5, 1, 2, 4, 8, 16])
    d = dict(pas=a.pas, carte=carte, infos=infos, images=images, journal=journal, evts=evts,
             reglages=dict(graine=a.graine, theme=a.theme, scenario=a.scenario, moyens=dict(MOYENS_DEFAUT), doctrine='equilibree'))
    os.makedirs(os.path.dirname(a.sortie), exist_ok=True)
    with open(a.sortie, 'w', encoding='utf-8') as fh:
        json.dump(d, fh, ensure_ascii=False, separators=(',', ':'))
    _, st = p.bilan()
    print(f'Démo enregistrée : {a.sortie} ({len(images)} images, {os.path.getsize(a.sortie) // 1024} Ko, {st["ha"]:.0f} ha brûlés)')


if __name__ == '__main__':
    main()
