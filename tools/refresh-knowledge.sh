#!/bin/bash
# Rafraîchit les sources de connaissance de LiveDeck : mapping Mixxx du DJControl Mix, changelogs des
# crates clés, et versions courantes des crates sur crates.io.
#   refresh-knowledge.sh       tout mettre à jour et lister ce qui a changé
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
R="$ROOT/research"
mkdir -p "$R/mixxx" "$R/upstream"
df -h / | tail -1 | awk '{print "Disque libre : " $4}'

changed=""
fetch() { # url dest
  tmp="$2.new"
  if curl -sfL -o "$tmp" "$1"; then
    if [ -f "$2" ] && cmp -s "$tmp" "$2"; then rm -f "$tmp"; else mv "$tmp" "$2"; changed="$changed\n  $2"; fi
  else
    rm -f "$tmp"; echo "ÉCHEC : $1"
  fi
}

MX="https://raw.githubusercontent.com/mixxxdj/mixxx/main/res/controllers"
fetch "$MX/Hercules%20DJControl%20MIX.midi.xml" "$R/mixxx/Hercules DJControl MIX.midi.xml"
fetch "$MX/Hercules-DJControl-MIX-scripts.js" "$R/mixxx/Hercules-DJControl-MIX-scripts.js"
fetch "https://raw.githubusercontent.com/RustAudio/cpal/master/CHANGELOG.md" "$R/upstream/cpal-CHANGELOG.md"
fetch "https://raw.githubusercontent.com/Boddlnagg/midir/master/CHANGELOG.md" "$R/upstream/midir-CHANGELOG.md"
fetch "https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md" "$R/upstream/egui-CHANGELOG.md"

out="$R/crates-versions.txt"
{
  echo "# relevé du $(date +%Y-%m-%d) — crate version_stable date_maj"
  for c in cpal midir wmidi rtrb triple_buffer eframe egui symphonia rubato serde toml thiserror anyhow \
           tracing assert_no_alloc criterion hound no_denormals objc2-foundation rusty_link fundsp; do
    curl -s -A "livedeck-refresh" "https://crates.io/api/v1/crates/$c" \
      | python3 -I -c 'import sys,json
try:
  d=json.load(sys.stdin)["crate"]; print(d["name"], d["max_stable_version"], d["updated_at"][:10])
except Exception: print(sys.argv[1], "?")' "$c"
  done
} > "$out.new"
if [ -f "$out" ] && diff -q <(tail -n +2 "$out") <(tail -n +2 "$out.new") >/dev/null 2>&1; then
  rm -f "$out.new"
else
  [ -f "$out" ] && diff <(tail -n +2 "$out") <(tail -n +2 "$out.new") | grep '^[<>]'
  mv "$out.new" "$out"; changed="$changed\n  $out"
fi

if [ -n "$changed" ]; then printf "Modifiés :%b\n" "$changed"; else echo "Rien de neuf."; fi
