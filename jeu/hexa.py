"""La grille d'hexagones (pointe en haut, lignes impaires décalées, comme web_conquete et web_power).

Repère : x vers l'est, y vers le sud, en mètres. Un hexagone fait HEX_M mètres d'un bord plat à l'autre.
Cellule i = row * cols + col ; coordonnées axiales q = col - (row - (row & 1)) // 2, r = row.
"""
import heapq
import math

SQ3 = math.sqrt(3)
DIRS = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)]   # axiales : E, NE, NO, O, SO, SE


class Grille:
    def __init__(self, cols, rows, hex_m):
        self.cols, self.rows, self.hex_m = cols, rows, hex_m
        self.R = hex_m / SQ3                     # rayon (centre -> coin)
        self.n = cols * rows
        self.cx = [0.0] * self.n
        self.cy = [0.0] * self.n
        self.q = [0] * self.n
        self.r = [0] * self.n
        self.index = {}
        for row in range(rows):
            for col in range(cols):
                i = row * cols + col
                q = col - (row - (row & 1)) // 2
                self.q[i], self.r[i] = q, row
                self.cx[i] = hex_m * (col + 0.5 * (row & 1)) + hex_m / 2
                self.cy[i] = 1.5 * self.R * row + self.R
                self.index[(q, row)] = i
        self.largeur = hex_m * (cols + 0.5)
        self.hauteur = 1.5 * self.R * (rows - 1) + 2 * self.R
        # voisins : (j, direction 0..5, ux, uy) ; la direction d est celle de i vers j
        self.vois = []
        for i in range(self.n):
            lst = []
            for d, (a, b) in enumerate(DIRS):
                j = self.index.get((self.q[i] + a, self.r[i] + b))
                if j is not None:
                    dx, dy = self.cx[j] - self.cx[i], self.cy[j] - self.cy[i]
                    l = math.hypot(dx, dy)
                    lst.append((j, d, dx / l, dy / l))
            self.vois.append(lst)

    def col_row(self, i):
        return i % self.cols, i // self.cols

    def voisins(self, i):
        return [v[0] for v in self.vois[i]]

    def distance(self, i, j):
        """Distance en hexagones."""
        dq, dr = self.q[i] - self.q[j], self.r[i] - self.r[j]
        return (abs(dq) + abs(dr) + abs(dq + dr)) // 2

    def dist_m(self, i, j):
        return math.hypot(self.cx[i] - self.cx[j], self.cy[i] - self.cy[j])

    def sous(self, x, y):
        """La cellule sous un point (mètres), ou None hors de la carte."""
        x -= self.hex_m / 2
        y -= self.R
        q = (SQ3 / 3 * x - y / 3) / self.R
        r = (2 / 3 * y) / self.R
        s = -q - r
        rq, rr, rs = round(q), round(r), round(s)
        dq, dr, ds = abs(rq - q), abs(rr - r), abs(rs - s)
        if dq > dr and dq > ds:
            rq = -rr - rs
        elif dr > ds:
            rr = -rq - rs
        # les coordonnées axiales de la grille sont décalées d'une demi-colonne sur les lignes impaires
        return self.index.get((int(rq), int(rr)))

    def anneau(self, i, rayon):
        """Toutes les cellules à au plus `rayon` hexagones de i."""
        out = []
        q0, r0 = self.q[i], self.r[i]
        for dq in range(-rayon, rayon + 1):
            for dr in range(max(-rayon, -dq - rayon), min(rayon, -dq + rayon) + 1):
                j = self.index.get((q0 + dq, r0 + dr))
                if j is not None:
                    out.append(j)
        return out

    def ligne(self, i, j):
        """Les cellules traversées par le segment de i à j."""
        n = max(1, self.distance(i, j))
        out = []
        for k in range(n + 1):
            t = k / n
            c = self.sous(self.cx[i] + (self.cx[j] - self.cx[i]) * t, self.cy[i] + (self.cy[j] - self.cy[i]) * t)
            if c is not None and (not out or out[-1] != c):
                out.append(c)
        return out


def astar(grille, depart, arrivee, cout, heur_min=0.2):
    """Plus court chemin : cout(i, j) = minutes pour passer de i à j (None = infranchissable).
    heur_min : minutes par hexagone au mieux (pour l'heuristique). Renvoie (liste de cellules, minutes) ou (None, inf)."""
    if depart == arrivee:
        return [depart], 0.0
    ouvert = [(0.0, 0.0, depart)]
    venu = {depart: None}
    g = {depart: 0.0}
    while ouvert:
        _, gc, i = heapq.heappop(ouvert)
        if i == arrivee:
            chemin = []
            while i is not None:
                chemin.append(i)
                i = venu[i]
            return chemin[::-1], gc
        if gc > g.get(i, 1e18):
            continue
        for j, _d, _ux, _uy in grille.vois[i]:
            c = cout(i, j)
            if c is None:
                continue
            ng = gc + c
            if ng < g.get(j, 1e18):
                g[j] = ng
                venu[j] = i
                heapq.heappush(ouvert, (ng + grille.distance(j, arrivee) * heur_min, ng, j))
    return None, float('inf')


def dijkstra(grille, sources, cout, horizon=1e18):
    """Temps d'arrivée depuis plusieurs sources {cellule: temps0}. cout(i, j) -> minutes ou None."""
    t = dict(sources)
    tas = [(v, k) for k, v in sources.items()]
    heapq.heapify(tas)
    while tas:
        tc, i = heapq.heappop(tas)
        if tc > t.get(i, 1e18) or tc > horizon:
            continue
        for j, _d, _ux, _uy in grille.vois[i]:
            c = cout(i, j)
            if c is None:
                continue
            nt = tc + c
            if nt < t.get(j, 1e18) and nt <= horizon:
                t[j] = nt
                heapq.heappush(tas, (nt, j))
    return t
