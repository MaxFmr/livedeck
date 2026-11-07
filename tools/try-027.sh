#!/bin/bash
# Essai guidé à la voix de R1 (027) : 3 kits dub techno et deck A selon la carte D8.
# La voix se tait pendant le jeu libre. Journal dans target/027-test.txt.
#   try-027.sh          toutes les étapes
#   try-027.sh 2 3      seulement certaines étapes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/027-test.txt

steps=(
  "Kit 808. Pads Hot Cue du deck gauche : les patterns un à quatre, le kick reste sur chaque temps"
  "Tourne le jog gauche par la tranche : la densité monte et descend. Par le dessus : le swing"
  "Tiens Shift et appuie sur les pads Sampler du deck gauche : un, deux, trois. Les kits 808, 909 et Dub changent à la mesure"
  "Potard Filter du deck gauche : la coupure de la basse. Puis les pads FX : les mutes"
  "Tiens Shift et tourne le jog gauche : le tempo. Par le dessus, plus finement"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 124 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
# Ctrl-C (or any exit) also stops the app.
trap 'kill "$app" 2>/dev/null; rm -f "$fifo"' EXIT
exec 3> "$fifo"
sleep 3
say -v "$VOICE" "Essai du rythme dub techno. ${#selected[@]} étapes, puis jeu libre en silence."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  sleep "$STEP"
done
say -v "$VOICE" "Jeu libre. Je me tais. Contrôle C pour finir."
echo "# jeu libre" >&3
wait "$app"
