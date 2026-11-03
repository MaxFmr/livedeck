#!/bin/bash
# Capture la fenêtre de l'app dans des états joués par script (développement, sans contrôleur).
#   printf "%s\n" "midi 96017F 960100" "wait 0.3" "shot" | tools/shoot.sh target/shots/a target/debug/app
# Lignes : « wait <s> », « shot » (→ <préfixe>-<n>.png), sinon une commande de l'app (p, midi …, t 90).
cd "$(dirname "$0")/.." || exit 1
out=$1; shift; bin=$1; shift
fifo=$(mktemp -u -t ld-shot); mkfifo "$fifo"
"$bin" --quiet --no-metronome "$@" < "$fifo" > "$out.log" 2>&1 &
pid=$!
exec 3> "$fifo"
sleep 3
n=0
while IFS= read -r line; do
  case "$line" in
    wait*) sleep "${line#wait }" ;;
    shot) n=$((n+1)); echo "shot $out-$n.bmp" >&3; sleep 1; sips -s format png "$out-$n.bmp" --out "$out-$n.png" >/dev/null 2>&1; rm -f "$out-$n.bmp" ;;
    *) echo "$line" >&3 ;;
  esac
done
echo q >&3; exec 3>&-; wait $pid; rm -f "$fifo"
