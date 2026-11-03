#!/bin/bash
# Essai guidé à la voix de l'itération 019 : écoute de fx.wav puis delay et réverb en jeu.
# Journal dans target/019-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-019.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-019.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/019-test.txt

steps=(
  "La lecture tourne déjà. Deck droit : latch au pad Loop 1, un accord en Hot Cue, puis en mode Sampler, pad 1 lance une phrase"
  "Tourne doucement le potard Bass du deck droit vers la droite : la mélodie se répète en écho, à la croche pointée. Le rythme reste sec"
  "Tiens Shift et tourne le même potard Bass : il prend la main en passant par le milieu. Monte à fond : les répétitions s'allongent, comme un dub. Redescends vers le milieu"
  "Shift plus le bouton casque du deck droit, plusieurs fois : le temps de l'écho change, croche pointée, noire, croche, noire pointée, double, sans claquement"
  "Tourne le potard Headphone, au centre du mixer : la réverbération arrive sur tout le mix, large en stéréo. La basse reste nette"
  "Ramène le potard Bass du deck droit à gauche, et arrête la phrase avec le pad Sampler 1 : les échos et la réverb s'éteignent doucement"
  "Change de style avec Shift plus le jog gauche : chaque style amène son temps d'écho et sa longueur de réverb. Le dub a la plus longue"
  "Joue librement un morceau complet avec les deux decks et les effets"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des effets sur le style dub : sec, puis l'écho, des répétitions longues, la réverb ajoutée, chaque temps d'écho, et les queues qui s'éteignent."
  afplay target/renders/fx.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 19. ${#selected[@]} étapes, une toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  if [[ $s == *"|"* ]]; then
    echo "${s%%|*}" >&3
    s=${s#*|}
  fi
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  # The last step (free play) gets more time.
  if [ "$n" = "${#steps[@]}" ]; then sleep $((STEP * 3)); else sleep "$STEP"; fi
done
echo "q" >&3
exec 3>&-
wait "$app"
rm -f "$fifo"
say -v "$VOICE" "Terminé"
echo "Journal : $log"
