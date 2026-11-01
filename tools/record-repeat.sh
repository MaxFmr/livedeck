#!/bin/bash
# Enregistre 40 s de jeu au beat-repeat (sortie réelle de l'app) pour chercher des clics.
# Voie gauche = son, voie droite = 0,5 quand le beat-repeat est engagé.
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
say -v "$VOICE" "Enregistrement de 40 secondes. Passe en mode Loop et joue le beat-repeat, en appuyant et relâchant souvent, là où tu entends le clic."
target/release/app --bpm 120 --quiet --no-metronome --seconds 40 --record target/012-repeat.wav > target/012-rec.txt 2>&1
say -v "$VOICE" "Terminé"
