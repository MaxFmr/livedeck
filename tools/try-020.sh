#!/bin/bash
# Essai guidé à la voix de l'itération 020 : la fenêtre (coquille, barre du temps) au contrôleur.
# Journal dans target/020-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-020.sh          toutes les étapes
#   try-020.sh 2 3      seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-25}
log=target/020-test.txt

steps=(
  "Regarde la fenêtre. En haut : lecture, le tempo, quatre carrés de temps dont un allumé qui avance, la mesure, la gamme et le style. Les trois panneaux des decks sont encore vides"
  "Appuie sur Play : le triangle devient un carré, plus rien ne bouge. Rappuie pour relancer"
  "Bouge le fader Tempo du deck gauche, ou le jog par le dessus : le tempo change dans la fenêtre, les chiffres ne bougent pas de place"
  "Change la gamme avec Shift plus le jog droit, et la tonique avec Cue B : la fenêtre annonce le changement, puis l'applique à la mesure"
  "Change de style avec Shift plus le jog gauche : le style et le swing changent en haut"
  "Débranche le câble USB du contrôleur quelques secondes : un bandeau magenta Contrôleur absent apparaît en haut à droite. Rebranche : il disparaît"
  "Clique dans la fenêtre puis appuie sur la touche F : plein écran. Encore F pour revenir. Regarde aussi de loin, écran baissé"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 3
say -v "$VOICE" "Essai de l'itération 20. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
