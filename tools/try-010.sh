#!/bin/bash
# Essai guidé à la voix de l'itération 010 : écoute du kit en WAV puis batterie jouée aux pads.
# Journal dans target/010-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-010.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-010.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-15}
log=target/010-test.txt

steps=(
  "Mets les deux decks en mode Hot Cue. Deck gauche : pad 1 kick, pad 2 snare, pad 3 hat fermé, pad 4 hat ouvert. Frappe chacun plusieurs fois"
  "Frappe le hat ouvert, puis tout de suite le hat fermé : le fermé doit couper le ouvert"
  "Deck droit : pad 1 clap, pad 2 percussion. Frappe chacun plusieurs fois"
  "Baisse puis remonte le volume du deck gauche : c'est le niveau de la batterie"
  "Joue un rythme libre sur les pads, avec le métronome"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute du kit : chaque son seul, puis quatre mesures de rythme."
  afplay target/renders/kit.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 10. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
