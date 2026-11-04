#!/bin/bash
# CPU et RSS de l'app en lecture puis à l'arrêt (20 s chacun, sans geste).
#   tools/measure-cpu.sh target/release/app [--headless]
cd "$(dirname "$0")/.." || exit 1
fifo=$(mktemp -u -t ld-fifo); mkfifo "$fifo"
"$@" --quiet --no-metronome < "$fifo" > /dev/null 2>&1 &
pid=$!
exec 3> "$fifo"
cpu() { ps -o time= -p $pid | awk -F'[:.]' '{ if (NF==3) print $1*60+$2+$3/100; else print $1*3600+$2*60+$3+$4/100 }'; }
rss() { ps -o rss= -p $pid | awk '{printf "%.1f", $1/1024}'; }
sleep 5; c0=$(cpu); sleep 20; c1=$(cpu); r1=$(rss)
echo p >&3; sleep 5; c2=$(cpu); sleep 20; c3=$(cpu); r2=$(rss)
echo q >&3; exec 3>&-; wait $pid; rm -f "$fifo"
LC_ALL=C awk -v a=$c0 -v b=$c1 -v c=$c2 -v d=$c3 -v r1=$r1 -v r2=$r2 -v n="$*" 'BEGIN{printf "%s | lecture : CPU %.1f %%, RSS %s Mo | arrêt : CPU %.1f %%, RSS %s Mo\n", n, (b-a)/20*100, r1, (d-c)/20*100, r2}'
