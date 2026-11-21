#!/bin/sh
# Simulateur de feux de forêt (Linux, macOS, Raspberry Pi) : obtient l'exécutable (binaires.sh) puis le lance.
#   ./lancer.sh [--port 8797] [--lan | --local] [--no-browser]
cd "$(dirname "$0")" || exit 1
. ./binaires.sh
# Code 75 : « mis à jour, relancez-moi » : on reprend l'exécutable de la nouvelle version, puis on le relance.
export WEB_FIRE_RELANCEUR=1
while :; do
  obtenir_binaires && bin/web_fire "$@"
  code=$?
  [ "$code" -eq 75 ] || break
done
exit "$code"
