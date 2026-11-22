#!/bin/sh
# Simulateur de feux de forêt (Linux, macOS, Raspberry Pi) : obtient l'exécutable puis le lance ;
# garde le terminal ouvert en cas d'erreur.
#   ./demarrer_WEB_FIRE.sh [--port 8797] [--lan | --local] [--no-browser] [--sans-maj]
cd "$(dirname "$0")" || exit 1
# Un seul fichier à cliquer : le lanceur Windows (.cmd) est masqué du gestionnaire de fichiers.
ls *.cmd > .hidden 2>/dev/null
. ./scripts/binaires.sh
# Code 75 : « mis à jour, relancez-moi » : on reprend l'exécutable de la nouvelle version, puis on le relance.
export WEB_FIRE_RELANCEUR=1
while :; do
  obtenir_binaires && bin/web_fire "$@"
  code=$?
  [ "$code" -eq 75 ] || break
  export WEB_FIRE_DEJA_MAJ=1
done
# pause seulement devant un vrai terminal (pas sous systemd ni dans le terminal de web_launcher, TERM=dumb)
if [ "$code" -ne 0 ] && [ -t 0 ] && [ "$TERM" != dumb ]; then
  echo
  echo "[X] Le démarrage a échoué. La raison est écrite au-dessus."
  echo
  printf "Appuyez sur Entrée pour fermer..."; read -r _
fi
exit "$code"
