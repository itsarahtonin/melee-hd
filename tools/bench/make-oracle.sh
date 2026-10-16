#!/bin/bash
# Makes milestone 4's oracle matches (docs/oracle.md, "Oracle sets") in DIR and traces them into
# TRACES. For each match `melee-bench oracle-sets` picks (whole sets or single matches, every set
# if none are named): the Gecko codes that boot into it, the match recorded by ssbm-rs (NAME.slp;
# BENCH_AI plays the human players' controllers, the game's AI its computer players), ssbm-rs's
# replay check (NAME.check.log: Slippi's playback codes with the console's floating point), the
# recorded controllers played again when every player is human (NAME.pads.log), and ssbm-rs's
# state trace of the recording's playback, as the oracle's other traces are made: Show Player
# Names off, every port, main memory every tick (TRACES/oracle-NAME.trc and .log). NAME.times
# keeps each step's wall time and the trace's size.
#
#   SSBM_RUN=.../ssbm-run.exe SSBM_DISC=... bash tools/bench/make-oracle.sh DIR TRACES [SET|NAME...]
#
# SSBM_RUN is ssbm-rs's runner with BENCH_CODES (ssbm-rs main 3f9ac3a and later; the oracle's
# traces come from its ssbm-run-rf build, d61135b). JOBS (default 1) runs that many matches at
# once, each on one core at low priority. TRACE=0 skips the traces. RUN_TRACE=FIELDS also traces
# the recording run itself for its first FIELDS fields (DIR/NAME.run.trc, with memory; the
# interpreter as in the recording, but for the port the trace hooks: BENCH_AI hooks the middle
# of Fighter_procCpu, which a port would skip), whose heap layout `melee-oracle trace-heap`
# compares with the playback's.
set -u
dir=$1
traces=$2
shift 2
here=$(cd "$(dirname "$0")" && pwd)
bench=${MELEE_BENCH:-$here/../../target/release/melee-bench}
run=${SSBM_RUN:?SSBM_RUN=path to ssbm-run with BENCH_CODES}
mkdir -p "$dir" "$traces"
"$bench" oracle-sets "$dir" "$@" > "$dir/matches.tsv" || exit 1

make() {
    local name=$1 ai=$2 fields=$3 pads=$4 out=$dir/$1 trc=$traces/oracle-$1
    local controllers=()
    [ "$ai" != - ] && controllers=("BENCH_AI=$ai")
    local t=$SECONDS times=""
    env BENCH_CODES="$out.ini" "${controllers[@]}" BENCH_SLP="$out.slp" \
        nice -n 10 "$run" --fields "$fields" > "$out.make.log" 2>&1
    times+="make $((SECONDS - t)) s"
    t=$SECONDS
    nice -n 10 "$run" --replay "$out.slp" --fp hardware --fields "$fields" > "$out.check.log" 2>&1
    local check=$?
    times+=", check $((SECONDS - t)) s"
    local result="$name: $(grep -h '^replay check' "$out.check.log") (exit $check)"
    if [ "$pads" = pads ]; then
        t=$SECONDS
        BENCH_CODES="$out.ini" BENCH_PADS="$out.slp" BENCH_SLP="$out.pads.slp" \
            nice -n 10 "$run" --fields "$fields" > "$out.pads.log" 2>&1
        local padcheck=$?
        times+=", pads $((SECONDS - t)) s"
        result+="; $(grep -h '^bench check' "$out.pads.log") (exit $padcheck)"
    fi
    if [ "${RUN_TRACE:-}" ]; then
        t=$SECONDS
        env BENCH_CODES="$out.ini" "${controllers[@]}" STATE_TRACE="$out.run.trc" STATE_TRACE_MEMORY=1 \
            nice -n 10 "$run" --port sysdolphin/baselib/perf --fields "$RUN_TRACE" > "$out.run.log" 2>&1
        times+=", run trace $((SECONDS - t)) s"
    fi
    if [ "${TRACE:-1}" != 0 ]; then
        t=$SECONDS
        SLIPPI_PLAYER_NAMES=0 STATE_TRACE="$trc.trc" STATE_TRACE_MEMORY=1 \
            nice -n 10 "$run" --replay "$out.slp" --fp hardware --fields "$fields" --port all \
            > "$trc.log" 2>&1
        times+=", trace $((SECONDS - t)) s, $(du -m "$trc.trc" | cut -f1) MB"
    fi
    echo "$times" > "$out.times"
    echo "$result; $times"
}

jobs=${JOBS:-1}
while IFS=$'\t' read -r name ai fields pads; do
    while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do
        wait -n
    done
    make "$name" "$ai" "$fields" "$pads" &
done < "$dir/matches.tsv"
wait
