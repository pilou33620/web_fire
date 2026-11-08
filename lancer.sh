#!/bin/sh
# Lance le simulateur de feux de forêt (Linux, macOS, Raspberry Pi, Termux)
cd "$(dirname "$0")"
exec python3 server.py "$@"
