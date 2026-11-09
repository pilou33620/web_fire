# web_fire — simulateur de feux de forêt en hexagones

Un massif forestier en plateau d'hexagones (même rendu que **web_conquete** : Three.js « cartoon », tuiles en relief,
contours sombres), des villes, des campings et des sites à protéger, des départs de feu… et un commandement
automatique qui engage camions, bulldozers, hélicoptères, Canadair et Dash 8 pour sauver ce qui peut l'être.
La météo (vent, chaleur, humidité, sécheresse, nuit, orages) pilote l'avancée du feu.

Moteur en **Python** (aucune dépendance), interface **web** (HTML/CSS/JS, Three.js inclus dans `web/vendor`).

## Lancer

```bash
python server.py            # puis ouvrir http://127.0.0.1:8780
python server.py --lan      # ouvert au réseau local (iPad, téléphone) — pratique sur le Raspberry Pi
```

`lancer.sh` (Linux, macOS, Termux) et `lancer.bat` (Windows) font la même chose.

**Sans serveur** : `python outils/statique.py` fabrique `dist/`, une version où le même moteur Python tourne dans le
navigateur grâce à Pyodide (à héberger n'importe où, ou à servir avec `python -m http.server`). Si Python ne peut
pas se charger, la page rejoue une journée enregistrée du vrai moteur (lecture seule).

## Ce qu'on voit

- **La carte** : 60 × 44 hexagones de 300 m (18 × 11 km, 7,8 ha par hexagone). Deux massifs générés :
  *Landes de Gascogne* (océan, dunes, grands étangs, pinède, quadrillage de pistes DFCI) et *massif méditerranéen*
  (mer, relief, garrigue, maquis, villages perchés). Une graine = une carte.
- **Le feu** : flammes, braises, fumée qui part sous le vent, sautes de feu, zones brûlées, retardant rouge, zones mouillées.
- **Les moyens** : CCF (groupes GIFF de 4), bulldozers, hélicoptères bombardiers d'eau (HBE), Canadair « Pélican »,
  Dash 8 « Milan » ; colonnes de renfort quand le feu grandit.
- **Le panneau** : météo et danger, chaque feu (statut, vitesse de la tête, stratégie, sites menacés avec l'heure
  d'arrivée estimée), les moyens, le journal du commandement, les réglages.
- **Le mode Intervention** (🔍) : la vue rapprochée d'un feu, comme le mode Bataille de web_conquete. Chaque hexagone
  est découpé en 25 petits hexagones : le front avance dans la forêt arbre par arbre, les camions se postent, les
  pompiers déroulent les lances, les hélicos et les Canadair font leurs passes de largage, le Dash pose sa ligne rouge,
  le bulldozer ouvre le pare-feu ; des étiquettes montrent la tête du feu, les points défendus et les missions.

## Vous pouvez prendre le commandement

Le bouton **🤖 COS : IA / 🧑‍🚒 COS : vous** décide qui commande.
- **IA** : tout se gère seul. Vous pouvez quand même donner un ordre à un moyen : il passe « sous vos ordres »
  (🧑‍🚒) et l'IA n'y touche plus tant que vous ne le lui rendez pas.
- **Vous** : l'IA ne fait plus que signaler les menaces. Vous engagez les moyens, demandez les renforts, décidez
  des évacuations.

Choisissez un moyen (sur la carte ou dans l'onglet Moyens), ou un groupe entier (un GIFF de 4 CCF, tous les
Canadair…), puis un ordre, puis l'endroit sur la carte :
📍 se poster · 🔥 attaquer · 🛡 défendre un site · 👁 surveiller les lisières · 💧 larguer · 🟥 ligne de retardant ·
🚧 pare-feu (deux points) · ↩ retour à la base · 🤖 rendre à l'IA.

## Routes et pistes : bonus et malus

| | Bonus | Malus |
|---|---|---|
| **Routes** (enrobé, ligne blanche, panneaux d'entrée de village, garde-corps sur les ponts) | camions rapides (50-60 km/h) ; large coupure, le feu passe mal ; seules voies des convois d'évacuation | bouchons quand les convois roulent ; la fumée ralentit ; plus de départs de feu le long des routes ; elles contournent le massif |
| **Pistes DFCI** (sable, deux ornières, barrière rouge et blanche à la jonction avec la route, poteau indicateur au bout) | pénètrent le massif jusqu'au feu, postes d'attaque privilégiés ; citernes aux croisements ; coupure ; bulldozer plus rapide | sable : camions lents (27 km/h) ; étroites : au-delà de 2 engins on n'avance plus qu'au pas ; fermées quand elles longent un feu intense ; interdites aux convois de civils |

Dans les Landes, les pistes forment le quadrillage DFCI ; en Méditerranée, elles suivent les crêtes et descendent
vers les routes. Le tracé est nettoyé à la génération : routes qui tournent peu, sans escaliers ni doublons, rien de
construit ni de planté sur l'emprise des voies.

## Commander au geste : sections et cônes

Les camions vont par **groupe (GIFF)** : 4 CCF et le VLHR du chef de groupe, qui suit et se tient en retrait.
En mode COS, toucher un camion choisit tout son groupe, et l'ordre « Arroser vers… » est prêt :
**appuyez** à l'endroit du poste, **glissez** vers le feu pour orienter les lances, **relâchez** pour donner l'ordre.
Pendant le geste, l'aperçu montre le trajet prévu (pointillés), les postes (anneaux), le **cône des lances** (bleu),
les hexagones arrosés (bleu) ou trop chauds pour être tenus (rouge), l'eau du groupe et son autonomie.
Ça marche au doigt, à la souris et **au stylet** (au survol du stylet, le poste s'affiche déjà), sur la carte comme
dans la vue Intervention.

Les quatre modes d'un groupe :
- 🎯 **Arroser vers…** : le groupe se poste en ligne face à la direction choisie et n'arrose que dans son cône ;
- ➖ **Ligne d'arrêt** : on trace la ligne, les camions s'y étalent face au feu ; sans flammes à portée, ils mouillent la végétation devant eux ;
- 🛡 **Protéger à 360°** : autour d'un village ou d'un camping, ils arrosent tout ce qui approche ;
- ↗ **Attaque de flanc** : le groupe remonte seul le flanc vers la tête, de poste en poste, en gardant le feu dans son cône.

Les avions se commandent pareil : appuyer sur le point de largage, glisser pour orienter l'**axe de la passe**
(Canadair : deux hexagones le long de l'axe ; Dash : une ligne de retardant de cinq hexagones), relâcher. L'aperçu
montre l'empreinte du largage, le point d'écopage et la route de vol. Les Canadair volent par patrouilles de deux.

## Évacuations

Chaque site menacé peut être **évacué** (🚗 convois de 300 personnes qui partent toutes les 3 minutes vers une ville
sûre ou la sortie du massif) ou **confiné** (🏠 mise à l'abri sur place). La fumée ralentit les convois, les routes
qui brûlent ou longent un feu intense sont coupées, plusieurs convois sur la même route font un embouteillage.
Un convoi sans itinéraire sûr fait demi-tour et se met à l'abri au village ; un convoi pris par le feu, ou des
confinés dans un site qui brûle presque entièrement, comptent comme personnes exposées. L'IA évacue tant que le
trajet tient avant l'arrivée du feu, et confine quand il est trop tard. Le calque « Routes » montre les routes
coupées (rouge) et enfumées (gris).

## Le modèle

**Propagation** (`jeu/feu.py`, inspirée de Rothermel), minute par minute, d'hexagone à hexagone :
vitesse = combustible × vent `exp(0,045·V·cos θ)` × pente `exp(3·dénivelé/300 m)` × humidité du combustible ×
sécheresse × coupure (route, piste) × eau × retardant × défense. Chaque combustible a sa vitesse, sa durée de
combustion, son intensité, sa résistance à l'eau et son aptitude aux sautes de feu (brandons par vent > 30 km/h).
Un hexagone éteint mais pas noyé peut se rallumer (reprise).

**Météo** (`jeu/meteo.py`) : cycle du jour (chaleur à 15 h 30, humidité la nuit), vent qui dérive, rafales,
bascule de vent possible, orages secs (foudre) puis pluie. Les avions ne volent ni la nuit ni par vent trop fort.
Scénarios : journée ordinaire, forte chaleur, canicule et vent d'est (type Landiras 2022), orages secs, mistral.

**Commandement** (`jeu/ia.py`), d'après la doctrine française :
1. attaque massive des feux naissants ;
2. mise en défense des points sensibles menacés (évacuation préventive des campings) ;
3. attaque au sol par les flancs, de l'arrière vers la tête, en s'appuyant sur le noir ;
4. avions sur la tête et les sautes, ligne de retardant devant une tête rapide, pare-feu au bulldozer là où le feu
   arrivera dans une à deux heures ;
5. surveillance et noyage des lisières quand le feu est fixé ;
6. par risque très sévère : patrouilles préventives et guet aérien armé.

Trois doctrines au choix (équilibrée, offensive, défensive). `python outils/banc.py` les compare sur les mêmes journées.

## Fichiers

| Fichier | Rôle |
|---|---|
| `server.py` | serveur local (simulation en continu, API `/api/carte`, `/api/etat`, `/api/action`) |
| `jeu/partie.py` | une partie : carte, météo, feu, moyens, commandement ; départs de feu et détection |
| `jeu/carte.py` | génération des massifs, villes, sites, routes, pistes DFCI, casernes, vigies, aérodrome |
| `jeu/feu.py`, `jeu/meteo.py`, `jeu/combustibles.py` | le feu, la météo, les combustibles |
| `jeu/moyens.py`, `jeu/ia.py` | les moyens de lutte et le commandement |
| `jeu/ordres.py`, `jeu/evacuation.py` | vos ordres de COS ; évacuations, fumée, routes coupées |
| `web/js/plateau.js`, `web/js/modeles.js`, `web/js/moteur.js` | le plateau 3D, les modèles, le moteur de rendu (repris de web_conquete) |
| `web/js/intervention.js` | le mode Intervention |
| `web/js/voies.js` | le dessin des routes et des pistes (commun aux deux vues) |
| `web/js/liaison.js` | la liaison avec le serveur, ou Pyodide dans la page, ou la démo enregistrée |
| `outils/enregistrer.py`, `outils/statique.py` | la démo rejouée ; la version sans serveur (`dist/`) |
| `tests/` | `python -m unittest discover -s tests` |
