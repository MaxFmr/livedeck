#!/bin/bash
# Essai guidé à la voix de l'itération 016 : écoute des patchs en WAV puis accords au deck B.
# Journal dans target/016-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-016.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-016.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/016-test.txt

steps=(
  "Passe le deck droit en mode Hot Cue. Les quatre pads jouent les accords un, quatre, cinq et six mineur, tant qu'ils sont tenus. Joue-les en rythme, par-dessus le groove"
  "Fader Tempo droit : la brillance du synthé. Volume droit : le niveau de la mélodie, et Shift plus Volume droit, le métronome"
  "Change de style au Shift plus jog gauche : chaque style amène son patch, skank dub, orgue afro, stab garage, keys lo-fi, stab house. Joue les accords sur chacun"
  "Change la tonalité au Cue droit, Shift pour monter : les accords suivent la basse, à la mesure"
  "Joue librement avec les deux decks"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des accords : pour chaque style, son patch sur un, six, quatre, cinq."
  afplay target/renders/chords.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 16. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
