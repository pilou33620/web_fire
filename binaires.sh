#!/bin/sh
# Met l'exécutable du simulateur dans bin/ et le moteur WebAssembly dans web/wasm/
# (à inclure par « . ./binaires.sh » depuis la racine).
# Avec cargo sur le poste : compilés ici, toujours à jour avec les sources (le WASM seulement si la cible
# wasm32-unknown-unknown et wasm-bindgen sont installés).
# Sinon : téléchargés depuis GitHub (publication « binaires », compilée par la CI à chaque envoi sur main),
# une fois par version : bin/commit et web/wasm/commit disent de quelle version ils viennent.
DEPOT_BINAIRES="https://github.com/pilou33620/web_fire/releases/download/binaires"

# L'archive téléchargée doit avoir l'empreinte SHA-256 publiée avec elle : tronquée ou altérée en
# route, elle est refusée. (L'empreinte vient du même dépôt : elle ne protège pas d'un compte volé.)
empreinte_juste() {
  if command -v sha256sum >/dev/null 2>&1; then recu=$(sha256sum bin/.archive); else recu=$(shasum -a 256 bin/.archive); fi
  [ -n "$recu" ] && [ "${recu%% *}" = "$(cat bin/.empreinte)" ] && return 0
  echo "  [!] Empreinte de l'archive incorrecte : archive refusée."
  return 1
}

# telecharger ARCHIVE DOSSIER : télécharge, vérifie et extrait l'archive dans le dossier
telecharger() {
  if curl -fsSL -o bin/.archive "$DEPOT_BINAIRES/$1" \
     && curl -fsSL -o bin/.empreinte "$DEPOT_BINAIRES/$1.sha256" \
     && empreinte_juste && tar -xzf bin/.archive -C "$2"; then
    rm -f bin/.archive bin/.empreinte
    return 0
  fi
  rm -f bin/.archive bin/.empreinte
  return 1
}

# Le moteur dans le navigateur : facultatif (sans lui, la page passe par le serveur).
obtenir_wasm() {
  ici=$(git rev-parse HEAD 2>/dev/null)
  mkdir -p web/wasm
  if command -v cargo >/dev/null 2>&1 && command -v wasm-bindgen >/dev/null 2>&1 \
     && rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    cargo build --release -p feu_wasm --target wasm32-unknown-unknown \
      && wasm-bindgen --target web --no-typescript --out-dir web/wasm target/wasm32-unknown-unknown/release/feu_wasm.wasm \
      && return 0
  fi
  if [ -f web/wasm/feu_wasm_bg.wasm ] && [ -n "$ici" ] && [ "$(cat web/wasm/commit 2>/dev/null)" = "$ici" ]; then
    return 0
  fi
  echo "  Téléchargement du moteur pour le navigateur (WebAssembly)..."
  telecharger feu-wasm.tar.gz web/wasm && return 0
  echo "  [!] Moteur WebAssembly non téléchargé : la page passera par le serveur."
  return 0
}

obtenir_binaires() {
  mkdir -p bin
  if command -v cargo >/dev/null 2>&1; then
    cargo build --release -p web_fire --bins || return 1
    for f in target/release/*; do
      if [ -f "$f" ] && [ -x "$f" ]; then
        # copie puis renommage : un exécutable en cours d'usage ne se réécrit pas
        cp "$f" "bin/.neuf" && mv -f "bin/.neuf" "bin/$(basename "$f")"
      fi
    done
    obtenir_wasm
    return 0
  fi
  obtenir_wasm
  ici=$(git rev-parse HEAD 2>/dev/null)
  if [ -x bin/web_fire ] && [ -n "$ici" ] && [ "$(cat bin/commit 2>/dev/null)" = "$ici" ]; then
    return 0
  fi
  case "$(uname -s)" in Linux) os=linux ;; Darwin) os=macos ;; *) os=inconnu ;; esac
  case "$(uname -m)" in x86_64|amd64) cpu=x86_64 ;; aarch64|arm64) cpu=aarch64 ;; *) cpu=inconnu ;; esac
  echo "  Téléchargement du simulateur ($os-$cpu)..."
  telecharger "web_fire-$os-$cpu.tar.gz" bin && return 0
  if [ -x bin/web_fire ]; then
    echo "  [!] Téléchargement impossible : on garde l'exécutable déjà présent."
    return 0
  fi
  echo "[X] Pas d'exécutable pour ce poste ($os-$cpu) et téléchargement impossible."
  echo "    Installer Rust (https://rustup.rs) puis relancer : il sera compilé ici."
  return 1
}
