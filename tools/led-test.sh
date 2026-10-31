#!/bin/sh
# Balaye des valeurs de LED sur quelques cibles du DJControl Mix, en annonçant chaque étape.
# À lancer avec le deck gauche en mode Hot Cue. Ctrl+C pour arrêter.
set -u
cd "$(dirname "$0")/.." || exit 1
bin=target/debug/midi-sniff
echo "Mets le deck gauche en mode Hot Cue et regarde le contrôleur. Début dans 10 s…"
sleep 10
step() { # label "status note"
  printf '\n=== %s (%s) ===\n' "$1" "$2"
  "$bin" --leds "$2" --seconds 0.1 2>&1 | grep '^LED'
}
step "Pad 1 gauche, mode Hot Cue" "96 00"
step "Pad 1 gauche, adresse du mode Sampler (mode Hot Cue affiché)" "96 10"
step "Play gauche" "91 07"
step "Casque (PFL) gauche" "91 0C"
step "Vinyl" "91 03"
printf '\n=== Toutes les LEDs éteintes (B0 7F 00) ===\n'
"$bin" --send "B0 7F 00" --seconds 0.1 >/dev/null 2>&1
echo "Terminé."
