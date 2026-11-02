#!/bin/bash
# Importe le kit TR-909 de la bibliothèque Battery 4 dans assets/kits/tr909/ (itération 015) :
# un fichier par voix (kick, snare, closed_hat, open_hat, clap, perc), mono, 48 kHz, 24 bits,
# converti par afconvert (rééchantillonnage de macOS, qualité max). À relancer pour changer de samples :
#   SRC=... tools/import-kit-909.sh
set -eu
cd "$(dirname "$0")/.."
SRC=${SRC:-"/Users/Shared/Battery 4 Factory Library/Samples/Drums"}
DST=assets/kits/tr909
mkdir -p "$DST"
pairs=(
  "kick|Kick/Kick 909 1.wav"
  "snare|Snare/Snare 909 1.wav"
  "closed_hat|HiHat/ClosedHH 909 1.wav"
  "open_hat|HiHat/OpenHH 909 1.wav"
  "clap|Clap/Clap 909.wav"
  "perc|Snare/RimShot 909.wav"
)
for pair in "${pairs[@]}"; do
  voice=${pair%%|*}
  file=${pair#*|}
  afconvert -f WAVE -d LEI24@48000 -c 1 -r 127 "$SRC/$file" "$DST/$voice.wav"
  printf '%-11s ← %s\n' "$voice" "$file"
done
