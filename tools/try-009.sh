#!/bin/bash
# Essai guidé à la voix de l'itération 009 : états des LEDs et latence geste → son des pads.
# Journal dans target/009-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-009.sh          toutes les étapes
#   try-009.sh 4 5      seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-12}
log=target/009-test.txt

steps=(
  "Regarde le deck gauche. Lecture en cours : Play reste allumé, Sync flashe au premier temps, Cue aux autres temps"
  "Appuie sur Play pour arrêter. Play doit clignoter au tempo, Sync et Cue s'éteignent"
  "Appuie sur Play pour relancer"
  "Mets le deck gauche en mode Hot Cue, puis frappe les quatre pads un par un. Chaque pad fait un son grave, différent du métronome, et s'allume seulement tant que tu le tiens"
  "Même chose sur le deck droit, en mode Hot Cue"
  "Frappe un pad gauche dix fois, en rythme"
  "t 90|Le tempo vient de passer à 90. Si Sync gauche clignote vite, bouge le fader Tempo gauche jusqu'à ce que le clignotement s'arrête"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 9. ${#selected[@]} étapes, une toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  if [[ $s == *"|"* ]]; then
    echo "${s%%|*}" >&3
    s=${s#*|}
  fi
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  sleep "$STEP"
done
echo "q" >&3
exec 3>&-
wait "$app"
rm -f "$fifo"
say -v "$VOICE" "Terminé"
echo "Journal : $log"
