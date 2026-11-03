#!/bin/bash
# Essai guidé à la voix de l'itération 018 : écoute de arp.wav puis arpège, phrases et transposition au deck B.
# Journal dans target/018-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-018.sh          écoute du WAV puis toutes les étapes
#   SKIP_WAV=1 try-018.sh 2 3   sans le WAV, seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/018-test.txt

steps=(
  "La lecture tourne déjà. Deck droit en mode Loop, avec Shift plus le bouton Sampler. Pad 1 : latch. Puis repasse en mode Hot Cue et joue l'accord un : il tient"
  "Passe le deck droit en mode FX, avec Shift plus le bouton Hot Cue. Pad 2 : l'accord s'arpège en montant. Pad 3 : aller-retour. Pad 4 : aléatoire. Pad 1 coupe l'arpège"
  "Toujours en FX, remets l'arpège montant au pad 2. Shift plus les pads 1 à 4 : l'arpège couvre une à quatre octaves"
  "Tourne le bord du jog droit, sans toucher le dessus. Un quart de tour par vitesse : noire, croche, triolet, double, triolet, triple croche"
  "Repasse en mode Hot Cue et change d'accord : l'arpège suit le nouvel accord latché"
  "Mode FX, pad 1 : arpège coupé. Puis mode Sampler. Pad 1 lance la phrase un à la mesure : la LED clignote, puis reste allumée. Rappuie : elle s'arrête à la mesure. Essaie les pads 2 à 4"
  "Shift plus un pad Sampler : la variation de la phrase, sa seconde moitié monte d'une tierce"
  "Pose la main sur le dessus du jog droit et tourne d'un quart de tour : les accords et la phrase montent d'un degré dans la gamme, tout de suite. Tourne dans l'autre sens pour redescendre"
  "Change de style avec Shift plus le jog gauche : les phrases changent avec le style"
  "Joue librement avec les deux decks : arpège, phrases et transposition"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

if [ "${SKIP_WAV:-0}" != 1 ]; then
  say -v "$VOICE" "D'abord, écoute des arpèges : montant, aller-retour sur deux octaves, aléatoire en triolets, puis les quatre phrases, une variation et une transposition."
  afplay target/renders/arp.wav
fi

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 2
say -v "$VOICE" "Essai de l'itération 18. ${#selected[@]} étapes, une toutes les $STEP secondes."
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
