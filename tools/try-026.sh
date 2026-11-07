#!/bin/bash
# Essai guidé à la voix de l'itération 026 : notes au pad et boucles de mélodie enregistrées à la volée.
# Journal dans target/026-test.txt.
#   try-026.sh          toutes les étapes
#   try-026.sh 2 3      seulement certaines étapes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/026-test.txt

steps=(
  "Appuie sur Sync du deck droit : sa LED reste allumée et les pads Hot Cue du deck droit jouent des notes. Les pads un à quatre, puis avec Shift cinq à huit. Tout est dans la gamme"
  "Pad Sampler 1 du deck droit : la case B1 attend la mesure, puis passe en rose. Joue une petite mélodie pendant deux mesures. Ensuite elle tourne toute seule"
  "Joue par-dessus ta boucle. Puis tourne le jog droit par le dessus : la boucle se transpose dans la gamme"
  "Tiens Shift et tourne le jog droit par le dessus : choisis une mesure. Puis pad Sampler 2 : enregistre une boucle courte"
  "Alterne les pads Sampler 1 et 2 : on passe d'une boucle à l'autre à la mesure. Rappuie sur celle qui tourne pour l'arrêter"
  "Shift et pad Sampler 2 : la boucle 2 est effacée. Puis Sync B : retour aux accords"
  "Jeu libre : improvise avec accords, notes et boucles. Note ce qui gêne"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 100 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo"
sleep 3
say -v "$VOICE" "Essai des boucles de mélodie. ${#selected[@]} étapes, une toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  if [ "$n" = "${#steps[@]}" ]; then sleep "${FREE:-180}"; else sleep "$STEP"; fi
done
echo "q" >&3
exec 3>&-
wait "$app"
rm -f "$fifo"
say -v "$VOICE" "Terminé"
echo "Journal : $log"
