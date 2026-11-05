#!/bin/bash
# Essai guidé à la voix de l'itération 025 : scènes (enregistrer, rappeler à la mesure, réenregistrer,
# relancer l'app). Journal dans target/025-test.txt ; session d'essai dans target/025-session.toml.
#   try-025.sh          toutes les étapes
#   try-025.sh 2 3      seulement certaines étapes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-30}
log=target/025-test.txt
session=target/025-session.toml

steps=(
  "Joue un groove : pattern, quelques mutes, un accord en latch. Puis, deck gauche en mode FX, tiens Shift et appuie sur le pad 1 : la scène 1 est enregistrée, la case 1 s'allume en bas de la fenêtre"
  "Change tout : autre pattern, autre style au jog, phrase ou arpège sur le deck droit. Puis Shift et pad 2 du mode FX : scène 2"
  "Shift et pad 1 : la case 1 clignote en pointillé, puis tout revient d'un coup au premier temps. Le tempo, les volumes et le crossfader ne bougent pas"
  "Alterne Shift pad 1 et Shift pad 2 plusieurs fois, en écoutant si le changement tombe pile sur la mesure"
  "Deck gauche en mode Loop, Shift et pad 1 : c'est la scène 5, enregistrée dans une case vide"
  "Appuie sur Sync du deck droit : il clignote vite et le pied affiche Enregistrer. Change un réglage puis Shift pad 1 du mode FX : la scène 1 est remplacée"
  "q|Je relance l'app : les scènes doivent revenir, lues dans le fichier de session"
  "relance|Rappelle la scène 1 puis la scène 5 : elles sont les mêmes qu'avant la relance"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

start_app() {
  fifo=$(mktemp -u -t livedeck-fifo)
  mkfifo "$fifo"
  target/release/app --bpm 120 ${QUIET---quiet} --no-metronome --session "$session" < "$fifo" >> "$log" 2>&1 &
  app=$!
  exec 3> "$fifo"
  sleep 3
}
stop_app() {
  echo "q" >&3
  exec 3>&-
  wait "$app"
  rm -f "$fifo"
}

: > "$log"
start_app
say -v "$VOICE" "Essai des scènes. ${#selected[@]} étapes, une toutes les $STEP secondes."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  if [[ $s == "q|"* ]]; then
    s=${s#q|}
    say -v "$VOICE" "Étape $n. $s"
    stop_app
    echo "# étape $n : relance" >> "$log"
    start_app
    continue
  fi
  s=${s#relance|}
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  sleep "$STEP"
done
stop_app
say -v "$VOICE" "Terminé"
echo "Journal : $log — session : $session"
