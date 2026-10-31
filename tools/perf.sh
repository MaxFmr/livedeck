#!/bin/sh
# Lance un binaire et rapporte RSS max, temps CPU et durée (macOS /usr/bin/time -l).
#   perf.sh <binaire> [args…]      (arrêter l'app normalement pour obtenir le rapport)
set -u
[ $# -ge 1 ] || { echo "usage: $0 <binaire> [args…]"; exit 2; }
export LC_ALL=C
log="$(mktemp -t livedeck-perf)"
/usr/bin/time -l "$@" 2> "$log"
# Program stderr is mixed with time's report: show it first, without the report lines.
sed -n '/^ *[0-9.]* real /,$!p' "$log"
awk '
  /real/ && /user/ { printf "Durée %.1f s | CPU user %.1f s | sys %.1f s\n", $1, $3, $5 }
  /maximum resident set size/ { printf "RSS max %.1f Mo\n", $1/1048576 }
  /peak memory footprint/ { printf "Empreinte mémoire max %.1f Mo\n", $1/1048576 }
' "$log"
rm -f "$log"
