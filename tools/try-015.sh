#!/bin/bash
# Essai guidé à la voix de l'itération 015 : écoute synthé contre samples en WAV puis bascule au Shift + PFL A.
# Journal dans target/015-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-015.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-015.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/015-test.txt

steps=(
  "Les samples sont maintenant 3 décibels plus forts. Shift plus PFL gauche bascule synthé et samples à la mesure : compare les niveaux"
  "Volume gauche règle la batterie seule. Tiens Shift et bouge le même fader : il règle la basse, une fois que le fader a rejoint son niveau actuel. Lâche Shift : le fader doit rejoindre le niveau de la batterie avant d'agir"
  "Équilibre batterie et basse à ton goût, en changeant de style au Shift plus jog"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute comparée : pour chaque style, deux mesures au synthé puis deux mesures sur les samples 909."
  afplay target/renders/samples.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 --quiet --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 15. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
