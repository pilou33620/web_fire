"""Les types de sol et leur comportement au feu.

ros  : vitesse de propagation de base, en hexagones par minute, sans vent ni pente, combustible sec
dur  : minutes de combustion à pleine intensité (ce que le feu met à consommer l'hexagone)
imax : intensité maximale (1 = pinède sèche en plein feu)
res  : résistance à l'eau (litres nécessaires, relatif) ; plus c'est haut, plus il faut d'eau
saute: aptitude à produire des brandons (sautes de feu)
"""

COMB = {
    # code: nom, ros, dur, imax, res, saute, couleur (rendu)
    'M': dict(nom='Océan', brule=False, col='#1d5a86'),
    'L': dict(nom='Étang, lac', brule=False, col='#3fa6dc'),
    'R': dict(nom='Rivière', brule=False, col='#46a9dd'),
    'S': dict(nom='Plage, sable', brule=False, col='#ead9a2'),
    'K': dict(nom='Roche', brule=False, col='#9b958a'),
    'X': dict(nom='Pare-feu (sol nu)', brule=False, col='#b8955f'),
    'A': dict(nom='Aérodrome', brule=False, col='#a9a394'),
    'D': dict(nom='Dune', brule=True, ros=0.020, dur=6, imax=0.25, res=0.6, saute=0.0, col='#e3d39a'),
    'P': dict(nom='Pinède', brule=True, ros=0.030, dur=45, imax=1.00, res=1.5, saute=1.0, col='#4f8a3a'),
    'J': dict(nom='Lande (ajoncs, fougères)', brule=True, ros=0.055, dur=15, imax=0.80, res=1.0, saute=0.5, col='#9da35a'),
    'F': dict(nom='Feuillus', brule=True, ros=0.012, dur=70, imax=0.75, res=1.3, saute=0.3, col='#5d9b3a'),
    'G': dict(nom='Garrigue', brule=True, ros=0.050, dur=15, imax=0.75, res=1.0, saute=0.5, col='#a7a868'),
    'Q': dict(nom='Maquis', brule=True, ros=0.040, dur=30, imax=0.95, res=1.3, saute=0.8, col='#6f8f44'),
    'H': dict(nom='Prairie, friche', brule=True, ros=0.070, dur=6, imax=0.35, res=0.6, saute=0.1, col='#9cc65a'),
    'C': dict(nom='Culture (maïs, céréales)', brule=True, ros=0.045, dur=8, imax=0.40, res=0.7, saute=0.1, col='#d9c25a'),
    'V': dict(nom='Vigne', brule=True, ros=0.006, dur=10, imax=0.25, res=0.6, saute=0.0, col='#b4b660'),
    'O': dict(nom='Oliveraie', brule=True, ros=0.010, dur=20, imax=0.35, res=0.8, saute=0.1, col='#9aa76a'),
    'Y': dict(nom='Forêt débroussaillée', brule=True, ros=0.014, dur=35, imax=0.55, res=1.2, saute=0.4, col='#6f9e4c'),
    'T': dict(nom='Camping sous les pins', brule=True, ros=0.020, dur=30, imax=0.70, res=1.4, saute=0.5, col='#7aa24c'),
    'B': dict(nom='Village, hameau', brule=True, ros=0.008, dur=90, imax=0.60, res=2.0, saute=0.2, col='#d8cba8'),
    'U': dict(nom='Ville', brule=True, ros=0.004, dur=120, imax=0.60, res=2.2, saute=0.2, col='#d3c7a6'),
    'Z': dict(nom="Zone d'activités", brule=True, ros=0.003, dur=60, imax=0.50, res=2.0, saute=0.1, col='#bdb6a6'),
}

EAU = set('MLR')
HABITE = set('UBTZ')          # les cellules où il y a des gens ou des biens
ROUTE_FREIN = {0: 1.0, 1: 0.75, 2: 0.55, 3: 0.45}   # une piste ou une route freine le feu (coupure de combustible)

# Bonus et malus de chaque type de voie (repris dans l'interface)
VOIES = {
    3: dict(nom='Route principale',
            bonus=['Camions rapides (60 km/h)', 'Large coupure : le feu passe mal (×0,45)', 'Itinéraire des évacuations'],
            malus=['Bouchons : les convois d\'évacuation ralentissent les camions', 'Fumée : visibilité réduite, on roule moins vite',
                   'Plus de départs de feu (mégots, véhicules)', 'Contourne le massif : loin du feu']),
    2: dict(nom='Route',
            bonus=['Camions rapides (50 km/h)', 'Coupure : le feu passe mal (×0,55)', 'Itinéraire des évacuations'],
            malus=['Bouchons : les convois d\'évacuation ralentissent les camions', 'Fumée : visibilité réduite, on roule moins vite',
                   'Plus de départs de feu (mégots, véhicules)']),
    1: dict(nom='Piste forestière DFCI',
            bonus=['Pénètre le massif : poste d\'attaque au contact du feu', 'Citernes DFCI aux croisements',
                   'Coupure : freine le feu (×0,75)', 'Bulldozer plus rapide pour ouvrir un pare-feu'],
            malus=['Sable : camions lents (27 km/h)', 'Étroite : à plus de 2 engins sur un tronçon, on n\'avance plus qu\'au pas',
                   'Piège : fermée aux camions quand elle longe un feu intense', 'Interdite aux convois de civils']),
}


def brule(c):
    return COMB[c]['brule']
