#!/usr/bin/env python3
"""Fabrique la version sans serveur (dossier dist/) : l'interface web + le moteur Python, exécuté dans le navigateur
par Pyodide. Le dossier peut être ouvert par n'importe quel hébergement de fichiers statiques (GitHub Pages, un
simple `python -m http.server`, une page publiée…).

    python outils/statique.py                 # Pyodide chargé depuis le CDN jsDelivr au lancement
    python outils/statique.py --pyodide DOSSIER   # copie Pyodide (pyodide.js, .wasm, stdlib) pour un usage hors ligne
"""
import argparse
import json
import os
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FICHIERS_PYODIDE = ['pyodide.js', 'pyodide.asm.js', 'pyodide.asm.wasm', 'python_stdlib.zip', 'pyodide-lock.json']


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--sortie', default=os.path.join(ROOT, 'dist'))
    ap.add_argument('--pyodide', help='dossier contenant pyodide.js, pyodide.asm.wasm… (paquet npm « pyodide »)')
    ap.add_argument('--sans-demo', action='store_true', help='ne pas enregistrer la démo rejouée (secours si Python ne charge pas)')
    ap.add_argument('--cdn', action='store_true', help='Three.js chargé depuis jsDelivr au lieu du dossier vendor/ (page publiée)')
    a = ap.parse_args()
    out = a.sortie
    if os.path.isdir(out):
        shutil.rmtree(out)
    shutil.copytree(os.path.join(ROOT, 'web'), out)
    dst = os.path.join(out, 'py')
    os.makedirs(dst)
    fichiers = []
    # les sources sont servies en .txt (certains hébergeurs refusent l'extension .py)
    for f in sorted(os.listdir(os.path.join(ROOT, 'jeu'))):
        if f.endswith('.py'):
            shutil.copy(os.path.join(ROOT, 'jeu', f), os.path.join(dst, f[:-3] + '.txt'))
            fichiers.append(dict(f='jeu/' + f, u=f[:-3] + '.txt'))
    with open(os.path.join(out, 'py', 'manifest.json'), 'w', encoding='utf-8') as fh:
        json.dump(dict(fichiers=fichiers), fh)
    if not a.sans_demo:
        import subprocess
        import sys
        subprocess.run([sys.executable, os.path.join(ROOT, 'outils', 'enregistrer.py'), '--sortie', os.path.join(out, 'demo', 'rejeu.json')], check=True)
    if a.cdn:
        shutil.rmtree(os.path.join(out, 'vendor'))
        m = os.path.join(out, 'js', 'moteur.js')
        with open(m, encoding='utf-8') as fh:
            txt = fh.read()
        with open(m, 'w', encoding='utf-8') as fh:
            fh.write(txt.replace("'../vendor/three/three.module.js'", "'https://cdn.jsdelivr.net/npm/three@0.186.1/build/three.module.js'"))
    if a.pyodide:
        vp = os.path.join(out, 'vendor', 'pyodide')
        os.makedirs(vp, exist_ok=True)
        for f in FICHIERS_PYODIDE:
            shutil.copy(os.path.join(a.pyodide, f), vp)
    print(f'Version statique prête : {out} ({len(fichiers)} fichiers Python)')


if __name__ == '__main__':
    main()
