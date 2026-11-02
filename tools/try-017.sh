#!/bin/bash
# Essai guidé à la voix de l'itération 017 : écoute des voicings en WAV puis accords Shift, latch, voicings et gammes au deck B.
# Journal dans target/017-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-017.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-017.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/017-test.txt

steps=(
  "Deck droit en mode Hot Cue. Shift plus les pads joue deux, trois, sept, et un à l'octave. Mélange-les avec les accords sans Shift"
  "Passe le deck droit en mode Loop. Pad 1 : latch, l'accord tient après le relâchement, jusqu'au suivant. Essaie-le avec les pads Hot Cue"
  "Toujours en Loop : pad 2 renverse l'accord vers le haut, Shift vers le bas. Pad 3 change d'octave, Shift descend. L'accord tenu suit tout de suite"
  "Joue un accord en latch, puis tiens Shift et appuie trois fois sur le pad Loop 2, puis trois fois sur le pad Loop 3 : l'accord doit descendre"
  "Deck droit en mode Loop. Tiens Shift enfoncé, et sans le lâcher, appuie trois fois sur le pad 2. Puis lâche Shift"
  "Pad Loop 4 : extension suivante, septième, sus4, add9. Shift plus pad 4 revient à la triade"
  "Tiens Shift et tourne le jog droit d'un demi-tour : la gamme change à la mesure, mineur, majeur, dorien, phrygien, mixolydien, penta. La basse et les accords suivent"
  "Joue librement avec les deux decks"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des voicings : triades, renversements, septièmes, puis add9 en dorien."
  afplay target/renders/voicings.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 17. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
