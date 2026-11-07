#!/bin/bash
# Essai guidé à la voix de R1 + R2 (027-028) : kits dub techno, deck A, son de la basse.
# La voix se tait pendant le jeu libre. Journal dans target/028-test.txt.
#   try-028.sh          toutes les étapes
#   try-028.sh 2 3      seulement certaines étapes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/028-test.txt

steps=(
  "Kit 808. Pads Hot Cue du deck gauche : les patterns un à quatre, le kick reste sur chaque temps"
  "Tourne le jog gauche par la tranche : la densité monte et descend. Par le dessus : le swing"
  "Tiens Shift et appuie sur les pads Sampler du deck gauche : un, deux, trois. Les kits 808, 909 et Dub changent à la mesure"
  "Potard Filter du deck gauche : la coupure de la basse. Puis les pads FX : les mutes"
  "Tiens Shift et tourne le jog gauche : le tempo. Par le dessus, plus finement"
  "Basse. Potard Bass du deck gauche : la résonance. Avec Shift : la quantité d'enveloppe. Le potard attend de rejoindre sa valeur quand tu changes de couche"
  "Fader Tempo du deck gauche : le decay du filtre de la basse. Avec Shift : le decay de la note, tout en haut elle est tenue"
  "Tiens Shift et appuie sur le pad Sampler 4 du deck gauche : une ligne de basse au hasard arrive à la mesure. Recommence pour en tirer une autre"
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
say -v "$VOICE" "Essai du rythme et de la basse. ${#selected[@]} étapes, puis jeu libre en silence."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  sleep "$STEP"
done
say -v "$VOICE" "Jeu libre. Je me tais. Contrôle C pour finir."
echo "# jeu libre" >&3
wait "$app"
