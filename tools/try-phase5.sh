#!/bin/bash
# Essai guidé à la voix de la fin de phase 5 : tous les widgets de la fenêtre (021–023), puis la revue sur scène D7.
# Journal dans target/phase5-test.txt (repères « ── # étape n » écrits par l'app elle-même).
#   try-phase5.sh          toutes les étapes
#   try-phase5.sh 2 3      seulement certaines étapes
# Une étape « commande|texte » envoie d'abord la commande à l'app (ex. « t 90 » change le tempo).
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/phase5-test.txt

steps=(
  "Deck gauche en mode Hot Cue. Appuie sur le pad B : dans la fenêtre, B apparaît en pointillé et clignote à la croche, avec le compte à rebours, puis devient le pattern en cours à la mesure"
  "Deck gauche en mode FX. Coupe les hats au pad 3 : le pad s'éteint comme sa LED, et les lignes des hats sont barrées dans la grille. Rallume-les"
  "Deck gauche en mode Sampler, pad 1 : le fill est annoncé, puis joué. Puis Cue A coupe la basse : la colonne Basse s'assombrit. Rappuie"
  "Deck droit en Hot Cue : joue des accords. Le badge et la rangée des degrés s'allument. En mode Loop, change renversement et octave : les valeurs suivent"
  "Deck droit en Sampler : lance une phrase, elle attend la mesure en pointillé. Puis en FX, lance l'arpège : son mode s'affiche en haut à droite du deck"
  "Tourne les potards : Bass du deck gauche, Volume du deck droit, Headphone. Une étiquette blanche montre la valeur une seconde, dans la zone du potard"
  "Bouge le fader Tempo du deck gauche loin du tempo actuel : l'étiquette passe en contour et indique la valeur à rejoindre"
  "Regarde les vumètres en bas à droite. Monte Master et Volume : le témoin Limiteur s'allume quand la sortie est limitée"
  "Revue sur scène : baisse la luminosité de l'écran à trente pour cent, recule à un mètre, et joue librement. Note ce qui se lit mal"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 120 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
exec 3> "$fifo" # keeps the app's stdin open until we send "q"
sleep 3
say -v "$VOICE" "Essai de la phase 5. ${#selected[@]} étapes, une toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  if [[ $s == *"|"* ]]; then
    echo "${s%%|*}" >&3
    s=${s#*|}
  fi
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  # The stage review (last step) lasts longer: REVIEW seconds, 15 min by default.
  if [ "$n" = "${#steps[@]}" ]; then sleep "${REVIEW:-900}"; else sleep "$STEP"; fi
done
echo "q" >&3
exec 3>&-
wait "$app"
rm -f "$fifo"
say -v "$VOICE" "Terminé"
echo "Journal : $log"
