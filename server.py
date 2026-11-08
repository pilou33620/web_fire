#!/usr/bin/env python3
"""Feux de forêt — serveur local du simulateur.

Lancement :  python server.py            (puis ouvrir http://127.0.0.1:8780)
Options   :  --port 8780  --lan (ouvert au réseau local, pour l'iPad)  --no-browser
Aucune dépendance externe : Python 3.8+ suffit (Windows, Linux, macOS, Raspberry Pi, Termux).

La simulation tourne dans un fil à part ; l'interface interroge /api/etat quelques fois par seconde.
"""
import argparse
import gzip
import json
import mimetypes
import os
import socket
import sys
import threading
import time
import traceback
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

ROOT = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(ROOT, 'web')
sys.path.insert(0, ROOT)

from jeu.meteo import SCENARIOS  # noqa: E402
from jeu.partie import LIMITES, MOYENS_DEFAUT, Partie  # noqa: E402

VITESSES = [0.25, 0.5, 1, 2, 4, 8, 16]      # minutes de jeu par seconde réelle


class Simulation:
    def __init__(self):
        self.lock = threading.Lock()
        self.vitesse = 1.0
        self.pause = False
        self.reglages = dict(graine=7, theme='landes', scenario='canicule', moyens=dict(MOYENS_DEFAUT), doctrine='equilibree')
        self.nouvelle(self.reglages)
        threading.Thread(target=self._boucle, daemon=True).start()

    def nouvelle(self, r):
        r = {**self.reglages, **(r or {})}
        r['graine'] = int(r.get('graine') or 1)
        with self.lock:
            self.partie = Partie(r['graine'], r['theme'], r['scenario'], r.get('moyens'), r.get('doctrine', 'equilibree'))
            self.carte_json = json.dumps(self.partie.export_carte(), ensure_ascii=False).encode('utf-8')
            self.carte_gz = gzip.compress(self.carte_json, 6)
            self.reglages = r
            self.id = int(time.time() * 1000) % 10 ** 9

    def _boucle(self):
        reste = 0.0
        dernier = time.perf_counter()
        while True:
            time.sleep(0.05)
            now = time.perf_counter()
            dt, dernier = now - dernier, now
            if self.pause:
                continue
            reste += dt * self.vitesse
            n = 0
            while reste >= 1 and n < 20:
                reste -= 1
                n += 1
                try:
                    with self.lock:
                        self.partie.pas()
                except Exception:
                    traceback.print_exc()
                    self.pause = True
                    break
            if reste > 5:
                reste = 0          # la machine ne suit pas : on ralentit plutôt que d'accumuler du retard

    def etat(self, depuis):
        with self.lock:
            e = self.partie.export_etat(depuis)
        e.update(vitesse=self.vitesse, pause=self.pause, partie=self.id, reglages=self.reglages)
        return e

    def action(self, a):
        typ = a.get('type')
        if typ == 'vitesse':
            self.vitesse = float(a.get('valeur', 1))
            self.pause = False
            return dict(ok=True)
        if typ == 'pause':
            self.pause = bool(a.get('valeur', not self.pause))
            return dict(ok=True)
        if typ == 'nouvelle':
            self.nouvelle(a.get('reglages'))
            return dict(ok=True, partie=self.id)
        if typ == 'avancer':
            with self.lock:
                for _ in range(max(1, min(600, int(a.get('minutes', 10))))):
                    self.partie.pas()
            return dict(ok=True)
        with self.lock:
            return self.partie.action(a)


SIM = None


def infos():
    return dict(scenarios={k: v['nom'] for k, v in SCENARIOS.items()}, themes={'landes': 'Landes de Gascogne', 'mediterranee': 'Massif méditerranéen'},
                limites=LIMITES, defaut=MOYENS_DEFAUT, vitesses=VITESSES)


class Handler(BaseHTTPRequestHandler):
    server_version = 'FeuxDeForet/1.0'

    def log_message(self, fmt, *args):
        pass

    def _envoyer(self, code, corps, typ='application/json; charset=utf-8', gz=None):
        accepte = 'gzip' in (self.headers.get('Accept-Encoding') or '')
        if accepte and gz is None and len(corps) > 1400 and (typ.startswith('application/json') or typ.startswith('text')):
            gz = gzip.compress(corps, 5)
        self.send_response(code)
        self.send_header('Content-Type', typ)
        self.send_header('Cache-Control', 'no-store' if 'json' in typ else 'max-age=60')
        if accepte and gz is not None:
            self.send_header('Content-Encoding', 'gzip')
            corps = gz
        self.send_header('Content-Length', str(len(corps)))
        self.end_headers()
        self.wfile.write(corps)

    def _json(self, obj, code=200):
        self._envoyer(code, json.dumps(obj, ensure_ascii=False, separators=(',', ':')).encode('utf-8'))

    def do_GET(self):
        u = urlparse(self.path)
        q = parse_qs(u.query)
        try:
            if u.path == '/api/carte':
                return self._envoyer(200, SIM.carte_json, gz=SIM.carte_gz)
            if u.path == '/api/etat':
                return self._json(SIM.etat(int(q.get('depuis', ['0'])[0] or 0)))
            if u.path == '/api/infos':
                return self._json(infos())
            chemin = u.path.lstrip('/') or 'index.html'
            f = os.path.normpath(os.path.join(WEB, chemin))
            if not f.startswith(WEB) or not os.path.isfile(f):
                return self._envoyer(404, b'introuvable', 'text/plain; charset=utf-8')
            typ = mimetypes.guess_type(f)[0] or 'application/octet-stream'
            if f.endswith('.js') or f.endswith('.mjs'):
                typ = 'text/javascript'
            if typ.startswith('text/') or typ.endswith('json'):
                typ += '; charset=utf-8'
            with open(f, 'rb') as fh:
                self._envoyer(200, fh.read(), typ)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_POST(self):
        u = urlparse(self.path)
        n = int(self.headers.get('Content-Length') or 0)
        try:
            a = json.loads(self.rfile.read(n) or b'{}')
        except ValueError:
            return self._json(dict(ok=False, erreur='JSON invalide'), 400)
        if u.path == '/api/action':
            try:
                return self._json(SIM.action(a))
            except Exception as e:
                traceback.print_exc()
                return self._json(dict(ok=False, erreur=str(e)), 500)
        self._json(dict(ok=False, erreur='inconnu'), 404)


def adresse_locale():
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(('10.255.255.255', 1))
        ip = s.getsockname()[0]
        s.close()
        return ip
    except OSError:
        return '127.0.0.1'


def main():
    global SIM
    ap = argparse.ArgumentParser(description='Simulateur de feux de forêt')
    ap.add_argument('--port', type=int, default=8780)
    ap.add_argument('--lan', action='store_true', help='ouvrir au réseau local (iPad, téléphone)')
    ap.add_argument('--no-browser', action='store_true')
    args = ap.parse_args()
    print('Préparation de la carte…', flush=True)
    SIM = Simulation()
    hote = '0.0.0.0' if args.lan else '127.0.0.1'
    srv = ThreadingHTTPServer((hote, args.port), Handler)
    url = f'http://127.0.0.1:{args.port}'
    print(f'Simulateur prêt : {url}', flush=True)
    if args.lan:
        print(f'Sur le réseau local : http://{adresse_locale()}:{args.port}', flush=True)
    if not args.no_browser and not os.environ.get('TERMUX_VERSION') and (os.environ.get('DISPLAY') or sys.platform in ('win32', 'darwin')):
        threading.Timer(0.8, lambda: webbrowser.open(url)).start()
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print('\nArrêt.')


if __name__ == '__main__':
    main()
