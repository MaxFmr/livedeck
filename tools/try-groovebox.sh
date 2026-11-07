#!/bin/bash
# Essai guidé intégral de la groovebox (R1 → R5) : docs/tests/plan-essai-groovebox.md.
# La voix annonce chaque étape puis se tait pendant le jeu libre. Ctrl-C arrête tout, app comprise.
#   try-groovebox.sh          toutes les étapes
#   try-groovebox.sh 7 8 9    seulement certaines étapes
set -u
cd "$(dirname "$0")/.." || exit 1
VOICE=${VOICE:-Thomas}
STEP=${STEP:-35}
FREE=${FREE:-600}
log=target/groovebox-test.txt

steps=(
  "Rythme. Pads Hot Cue du deck gauche : les patterns un à quatre, à la mesure. Le kick reste sur chaque temps"
  "Tourne le jog gauche, comme tu veux : la densité de la batterie"
  "Tiens Shift et appuie sur les pads Sampler un, deux, trois du deck gauche : les kits 808, 909 et Dub"
  "Pads FX du deck gauche : les mutes. Puis Shift et les pads Loop : le swing"
  "Basse. Potards Filter et Bass du deck gauche, puis le fader Tempo, avec et sans Shift : coupure, résonance, enveloppe, decays"
  "Tiens Shift et appuie sur le pad Sampler quatre du deck gauche : une ligne de basse au hasard, à la mesure"
  "Phrase. Deck droit en mode Sampler, pad un : Générer. La phrase arrive à la mesure et varie toute seule"
  "Fader Tempo du deck droit : la densité. Puis Générer à nouveau"
  "Pads Sampler deux, trois, quatre du deck droit : muter, figer, effacer"
  "Deck droit : tiens le pad Loop un et joue des accords en mode Hot Cue. Ils s'ajoutent à la phrase"
  "Effets. Deck droit en mode FX : tiens le pad un, throw delay, puis le pad deux, throw réverb"
  "Pad FX trois : temps du delay. Pad quatre : arpège. Potard Bass du deck droit et Headphone : les envois"
  "Crossfader : le fondu entre le rythme et la mélodie"
  "Interface. Baisse la luminosité à trente pour cent et recule d'un mètre. Vérifie que les onglets et les pads suivent le contrôleur, et que Shift montre la seconde couche"
)
selected=("$@")
[ ${#selected[@]} -eq 0 ] && selected=($(seq 1 ${#steps[@]}))

fifo=$(mktemp -u -t livedeck-fifo)
mkfifo "$fifo"
target/release/app --bpm 124 ${QUIET---quiet} --no-metronome < "$fifo" > "$log" 2>&1 &
app=$!
trap 'kill "$app" 2>/dev/null; rm -f "$fifo"' EXIT
exec 3> "$fifo"
sleep 3
say -v "$VOICE" "Essai intégral de la groovebox. ${#selected[@]} étapes, puis jeu libre en silence."
for n in "${selected[@]}"; do
  s=${steps[$((n - 1))]}
  echo "# étape $n : $s" >&3
  say -v "$VOICE" "Étape $n. $s"
  sleep "$STEP"
done
say -v "$VOICE" "Jeu libre, dix minutes. Je me tais."
echo "# jeu libre" >&3
sleep "$FREE"
say -v "$VOICE" "Terminé"
echo "q" >&3
wait "$app"
echo "Journal : $log"
