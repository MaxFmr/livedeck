#!/bin/sh
# Attend que le DJControl Mix apparaisse en MIDI, puis lance le sniffer (avec demande de dump) et
# garde une copie de la session dans target/sniff-AAAAMMJJ-HHMM.txt.
#   sniff-wait.sh [options de midi-sniff…]
set -u
cd "$(dirname "$0")/.." || exit 1
PATH="$HOME/.cargo/bin:$PATH"
cargo build -q -p midi-sniff || exit 1
bin=target/debug/midi-sniff
if ! "$bin" --list | grep -qi djcontrol; then
  echo "En attente du DJControl Mix… (branche-le en USB-C)"
  until "$bin" --list | grep -qi djcontrol; do sleep 1; done
fi
log="target/sniff-$(date +%Y%m%d-%H%M).txt"
echo "Session enregistrée dans $log"
"$bin" --dump "$@" 2>&1 | tee "$log"
