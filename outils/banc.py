#!/usr/bin/env python3
"""Banc d'essai : compare les doctrines du commandement (et l'absence de moyens) sur les mêmes journées.

    python outils/banc.py --scenario canicule --graines 1-6 --minutes 600
"""
import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from jeu.partie import Partie  # noqa: E402

SANS = dict(ccf=0, bull=0, helico=0, canadair=0, dash=0)


def jouer(graine, theme, scenario, minutes, doctrine, moyens=None):
    p = Partie(graine, theme, scenario, moyens=moyens, doctrine=doctrine)
    for _ in range(minutes):
        p.pas()
    _, st = p.bilan()
    return st


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--theme', default='landes')
    ap.add_argument('--scenario', default='canicule')
    ap.add_argument('--graines', default='1-5')
    ap.add_argument('--minutes', type=int, default=540)
    a = ap.parse_args()
    d, f = (a.graines.split('-') + [None])[:2]
    graines = range(int(d), int(f or d) + 1)
    lignes = [('sans moyens', 'equilibree', SANS), ('équilibrée', 'equilibree', None), ('offensive', 'offensive', None), ('défensive', 'defensive', None)]
    print(f"{a.theme} · {a.scenario} · {a.minutes} min · graines {a.graines}\n")
    print(f"{'doctrine':<14}{'ha brûlés':>12}{'habitations':>13}{'note':>7}{'largages':>10}")
    for nom, doc, moy in lignes:
        t0 = time.time()
        res = [jouer(g, a.theme, a.scenario, a.minutes, doc, moy) for g in graines]
        n = len(res)
        print(f"{nom:<14}{sum(r['ha'] for r in res) / n:>12.0f}{sum(r['maisons'] for r in res) / n:>13.0f}"
              f"{sum(r['score'] for r in res) / n:>7.0f}{sum(r['largages'] for r in res) / n:>10.0f}   ({time.time() - t0:.0f} s)")


if __name__ == '__main__':
    main()
