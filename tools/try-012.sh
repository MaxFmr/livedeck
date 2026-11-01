#!/bin/bash
# Essai guidé à la voix de l'itération 012 : écoute des gestes en WAV puis mutes, fills, beat-repeat et densité au contrôleur.
# Journal dans target/012-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-012.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-012.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-15}
log=target/012-test.txt

steps=(
  "Lecture en cours, pattern A, à la densité où se trouve le fader Tempo gauche : il règle maintenant la densité. Choisis le groove B avec le pad Hot Cue 2, puis descends le fader lentement jusqu'en bas, puis remonte"
  "Passe le deck gauche en mode FX. Les pads coupent kick, snare et clap, hats, perc. Coupe et rétablis chaque groupe : la LED s'éteint quand le groupe est coupé"
  "Passe en mode Sampler. Chaque pad lance un fill de la prochaine noire jusqu'à la fin de la mesure. Essaie les quatre, un par mesure"
  "Passe en mode Loop. Maintiens un pad : le beat-repeat boucle la batterie. Essaie les quatre, puis relâche : le groove reprend à sa place"
  "Maintiens le pad Loop 1, puis ajoute le pad 3 sans lâcher le premier, puis relâche le 3 : on revient au 1"
  "Shift plus pads Sampler : kick, snare et hats joués à la main par-dessus le groove"
  "Tempo au dessus du jog gauche, swing par le bord. Joue librement avec tout le deck rythme"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des gestes : densité, fills, puis beat-repeat."
  afplay target/renders/gestures.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 12. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
