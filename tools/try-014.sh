#!/bin/bash
# Essai guidé à la voix de l'itération 014 : écoute des styles en WAV puis défilement au Shift + jog gauche.
# Journal dans target/014-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-014.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-014.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/014-test.txt

steps=(
  "t 140|Lecture en cours, style hybride. Tiens Shift et tourne le jog gauche d'un demi-tour vers la droite : le style suivant arrive à la mesure. Écoute le dub steppa, et essaie ses quatre patterns aux pads Hot Cue"
  "t 108|Shift plus jog, style suivant : afrobeat reggae. Écoute, et essaie ses patterns et ses fills"
  "t 132|Style suivant : UK garage. Écoute le shuffle et la basse courte"
  "t 86|Style suivant : lo-fi hip-hop"
  "t 124|Style suivant : house techno"
  "Fais défiler les styles dans les deux sens, en jouant au tempo que tu veux. Le tempo et la tonalité restent les tiens"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des six styles, chacun à son tempo conseillé, patterns A à D."
  afplay target/renders/styles.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 14. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
