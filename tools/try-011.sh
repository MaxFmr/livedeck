#!/bin/bash
# Essai guidé à la voix de l'itération 011 : écoute des 4 patterns en WAV puis séquenceur joué au contrôleur.
# Journal dans target/011-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-011.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-011.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-15}
log=target/011-test.txt

steps=(
  "Lecture en cours, pattern A. Mets le deck gauche en mode Hot Cue : le pad 1 est allumé"
  "Appuie sur le pad 2. Il clignote jusqu'à la mesure suivante, puis le groove B démarre et le pad 2 s'allume"
  "Essaie les pads 3 puis 4, en écoutant le changement tomber pile sur la mesure"
  "Tourne le jog gauche par le bord, lentement : le swing change. Pousse vers la droite, puis reviens"
  "Pose la main sur le dessus du jog et tourne : c'est le tempo, le swing ne doit pas bouger"
  "Passe le deck gauche en mode Sampler et joue kick, snare et hats par-dessus le groove"
  "Appuie sur Play pour arrêter, repasse en Hot Cue, choisis un pattern à l'arrêt, puis relance avec Play"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des quatre patterns, deux mesures chacun."
  afplay target/renders/patterns.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 11. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
