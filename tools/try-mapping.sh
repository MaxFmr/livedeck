#!/bin/bash
# Essai guidé à la voix du mapping : lance l'app, annonce chaque geste, journalise la session dans
# target/mapping-test.txt (repères « ── # geste n » écrits par l'app elle-même, dans l'ordre).
#   try-mapping.sh            les 8 gestes
#   try-mapping.sh 7 8        seulement certains gestes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-12}
log=target/mapping-test.txt

steps=(
  "Tourne le jog gauche par le bord, dans un sens puis dans l'autre"
  "Pose la main sur le jog gauche et tourne"
  "Tape quatre fois sur Sync gauche, en rythme"
  "Bouge le fader Tempo gauche lentement de bas en haut"
  "Tourne le potard Master vers le bas puis remonte le"
  "Baisse puis remonte le volume du deck gauche"
  "Appuie deux fois sur le bouton casque gauche"
  "Garde Shift et appuie sur Play gauche"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai du mapping. ${#selected[@]} gestes, un toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  echo "# geste $n : $s" >&3
  say -v "$VOICE" "Geste $n. $s"
  sleep "$STEP"
done
echo "q" >&3
exec 3>&-
wait "$app"
rm -f "$fifo"
say -v "$VOICE" "Terminé"
echo "Journal : $log"
