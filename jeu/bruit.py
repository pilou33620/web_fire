"""Bruit de valeur reproductible (même graine, même carte)."""
import math


def hache(x, y, s):
    h = (x * 374761393 + y * 668265263 + s * 1442695041) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h ^ (h >> 16)) & 0xFFFFFFFF) / 4294967296.0


def bruit(x, y, s=7):
    xi, yi = math.floor(x), math.floor(y)
    fx, fy = x - xi, y - yi
    u, v = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a, b = hache(xi, yi, s), hache(xi + 1, yi, s)
    c, d = hache(xi, yi + 1, s), hache(xi + 1, yi + 1, s)
    return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v


def fbm(x, y, s=7, octaves=4):
    tot, amp, f, norm = 0.0, 1.0, 1.0, 0.0
    for k in range(octaves):
        tot += bruit(x * f, y * f, s + k * 101) * amp
        norm += amp
        amp *= 0.5
        f *= 2.0
    return tot / norm
