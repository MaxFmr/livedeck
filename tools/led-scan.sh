#!/bin/bash
# Cherche où sont les LEDs du DJControl Mix : pour chaque canal (ou une plage de notes d'un canal),
# allume toutes les notes pendant quelques secondes puis les éteint, en annonçant l'étape.
#   led-scan.sh                     balaye les 16 canaux Note On (90..9F), toutes les notes
#   led-scan.sh <st> <de> <à> […]   une ou plusieurs plages (triplets), ex. led-scan.sh 96 00 3F 96 40 7F
#                                   (st = 9x pour des notes, Bx pour des CC ; B0 7F toujours exclu)
set -u
cd "$(dirname "$0")/.." || exit 1
bin=target/debug/midi-sniff
HOLD=${HOLD:-3}
# Annonce vocale de chaque étape : on regarde le contrôleur, pas l'écran.
VOICE=${VOICE:-"Thomas"}
say -v "$VOICE" "" 2>/dev/null || VOICE="Flo (Français (France))"

burst() { # status first last value seconds-to-keep-the-port-open
  local args=() n
  for ((n = 16#$2; n <= 16#$3; n++)); do
    [ "$1" = "B0" ] && [ "$n" -eq 127 ] && continue # B0 7F = dump / LEDs off
    args+=(--send "$(printf '%s %02X %s' "$1" "$n" "$4")")
  done
  "$bin" "${args[@]}" --seconds "$5" >/dev/null 2>&1
}

n_step=0
step() { # status first last
  n_step=$((n_step + 1))
  printf '=== Étape %d — %s, %s à %s : allumé %ss ===\n' "$n_step" "$1" "$2" "$3" "$HOLD"
  say -v "$VOICE" "Étape $n_step" 2>/dev/null
  # The port stays open while lit: some LEDs may be reset when the host disconnects.
  burst "$1" "$2" "$3" 7F "$HOLD"
  burst "$1" "$2" "$3" 00 0.05
  sleep 1
}

echo "Regarde tout le contrôleur. Début dans 5 s…"
say -v "$VOICE" "Regarde le contrôleur. Chaque étape est annoncée." 2>/dev/null
sleep 2
if [ $# -eq 0 ]; then
  for ch in 90 91 92 93 94 95 96 97 98 99 9A 9B 9C 9D 9E 9F; do step "$ch" 00 7F; done
else
  while [ $# -ge 3 ]; do step "$1" "$2" "$3"; shift 3; done
fi
echo "Terminé."
say -v "$VOICE" "Terminé" 2>/dev/null
