#!/bin/bash
# Essai guidé à la voix de l'itération 013 : écoute de la basse en WAV puis lignes, mute, filtre et tonalité au contrôleur.
# Journal dans target/013-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-013.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-013.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-20}
log=target/013-test.txt

steps=(
  "Lecture en cours, pattern A avec sa ligne de basse, en la mineur. Écoute la basse. Passe le deck gauche en mode Hot Cue et essaie les quatre patterns : chacun amène sa ligne"
  "Shift plus pads Hot Cue : la ligne de basse change seule, à la mesure, sous la même batterie. Essaie les quatre"
  "Tourne le potard Bass gauche : il ouvre et ferme le filtre de la basse. Va jusqu'aux deux extrêmes"
  "Bouton Cue gauche : coupe et rétablit la basse. La LED est allumée quand la basse joue"
  "Bouton Cue droit : la tonalité descend d'un demi-ton, Shift plus Cue droit la monte. Le changement tombe à la mesure"
  "Joue librement avec tout le deck rythme : patterns, lignes, filtre, mutes, fills, beat-repeat"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute de la basse : les quatre lignes seules, puis avec leur batterie, le filtre qui s'ouvre, et un passage en mi mineur."
  afplay target/renders/bass.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 13. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
