#!/bin/bash
# Vérifie si l'adresse des LEDs de pads dépend du mode de pad affiché sur le contrôleur.
# Guidé à la voix : l'utilisateur change de mode sur le deck gauche, puis on allume deux plages.
set -u
cd "$(dirname "$0")/.." || exit 1
bin=target/debug/midi-sniff
VOICE=${VOICE:-Thomas}
n=0

flash() { # first last
  local args=() i
  for ((i = 16#$1; i <= 16#$2; i++)); do args+=(--send "$(printf '96 %02X 7F' "$i")"); done
  "$bin" "${args[@]}" --seconds 0.05 >/dev/null 2>&1
  sleep 3
  args=()
  for ((i = 16#$1; i <= 16#$2; i++)); do args+=(--send "$(printf '96 %02X 00' "$i")"); done
  "$bin" "${args[@]}" --seconds 0.05 >/dev/null 2>&1
  sleep 1
}

step() { # label first last
  n=$((n + 1))
  printf '=== Étape %d — %s (96 %s–%s) ===\n' "$n" "$1" "$2" "$3"
  say -v "$VOICE" "Étape $n"
  flash "$2" "$3"
}

mode() { # spoken-name input-offset
  say -v "$VOICE" "Mets le deck gauche en mode $1"
  printf '\n--- Deck gauche en mode %s ---\n' "$1"
  sleep 6
  step "adresse d'entrée du mode $1" "$2" "$(printf '%02X' $((16#$2 + 3)))"
  step "adresse 30" 30 33
}

say -v "$VOICE" "Test des modes. Regarde les pads du deck gauche."
mode "Hot Cue" 00
mode "Sampler" 10
mode "F X, avec Shift" 20
mode "Loop, avec Shift" 30
say -v "$VOICE" "Terminé"
echo "Terminé."
