#!/bin/bash
# Makes the benchmark matches (docs/performance.md) in DIR: for each, the Gecko codes that boot
# into it (melee-bench codes), the match recorded by ssbm-rs with the CPU AI playing both
# controllers (NAME.slp), then two checks that the recording is a match anyone can play from its
# controllers: ssbm-rs's replay check plays it back through Slippi's playback codes with the
# console's floating point (NAME.check.log), and ssbm-rs plays its recorded controllers again
# and compares (NAME.pads.log). Both must find no divergence. Last, ssbm-rs's state trace of the
# match's first frames (NAME.start.trace) keeps the fighters' heap addresses.
#
#   SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... bash tools/bench/make-matches.sh DIR [NAME...]
#
# SSBM_RUN is ssbm-rs's runner with BENCH_CODES (ssbm-rs main 3f9ac3a and later). Each match
# takes ssbm-rs ten to twenty minutes per run on one core; JOBS (default 1) runs that many
# matches at once.
set -u
dir=$1
shift
names=${*:-fox-fox-fd fox-marth-bf fox-marth-fd fox-marth-ps ics-peach-ps}
here=$(cd "$(dirname "$0")" && pwd)
bench=${MELEE_BENCH:-$here/../../target/release/melee-bench}
run=${SSBM_RUN:?SSBM_RUN=path to ssbm-run with BENCH_CODES}
fields=${FIELDS:-31000}
"$bench" codes "$dir" > /dev/null || exit 1

make() {
    local name=$1 out=$dir/$1
    BENCH_CODES=$out.ini BENCH_AI=0,1 BENCH_SLP=$out.slp "$run" --fields "$fields" > "$out.make.log" 2>&1
    "$run" --replay "$out.slp" --fp hardware --fields "$fields" > "$out.check.log" 2>&1
    local check=$?
    BENCH_CODES=$out.ini BENCH_PADS=$out.slp BENCH_SLP=$out.pads.slp "$run" --fields "$fields" > "$out.pads.log" 2>&1
    local pads=$?
    # The fighters' heap addresses at the match's start (ssbm-rs's state trace), which a match
    # with a fighter running CPU code needs in Dolphin (melee-bench's DI patch).
    STATE_TRACE=$out.start.trace BENCH_CODES=$out.ini BENCH_PADS=$out.slp "$run" --port sysdolphin/baselib/perf --fields 300 > /dev/null 2>&1
    echo "$name: $(grep -h '^replay check' "$out.check.log") (exit $check); $(grep -h '^bench check' "$out.pads.log") (exit $pads)"
}

jobs=${JOBS:-1}
for name in $names; do
    while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do
        wait -n
    done
    make "$name" &
done
wait
