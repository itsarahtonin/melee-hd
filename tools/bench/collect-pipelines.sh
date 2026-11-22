#!/bin/bash
# Makes the shipped pipeline list, data/pipelines (docs/fast-renderer.md, F5's fifth round): the
# uber and specialized pipelines the fast renderer asks for over a corpus, as the player's own
# saved lists keep them: the benchmark matches (data/bench), the console from power-on under both
# rulesets (idle, then each SESSION.inputs given), then each REPLAY.slp given, each played
# headless (--bench) at internal 1x. The lists hold pipeline keys only: what each pipeline is
# made for (blending, writes, depth, culling) and its TEV configuration's register bits, nothing
# of the game's. Make it again when the player's `the_shipped_list_is_one_this_build_reads` test
# says the list is another build's.
#
#   SSBM_DISC=... bash tools/bench/collect-pipelines.sh [REPLAY.slp | SESSION.inputs ...]
#
# MELEE_HD is the player (default target/release/melee-hd, built with
# `cargo build --locked --release -p melee-player`); TICKS caps each run's ticks (default
# 30000), POWER_ON_TICKS the idle power-on's (default 3600); JOBS players play the replays at
# once (default 1), each from the list the matches and menus made, and their lists are merged
# in turn (each entry once, in the order first listed; PYTHON, default python3, merges them).
# Make the list where the device's draws test depth early (Vulkan: WGPU_BACKEND=vulkan on
# Windows), so it holds those pipelines; a renderer whose draws can't takes each without early z.
set -u
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
player=${MELEE_HD:-$root/target/release/melee-hd}
[ -x "$player" ] || player=$player.exe
disc=${SSBM_DISC:?SSBM_DISC=the disc image}
ticks=${TICKS:-30000}
jobs=${JOBS:-1}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
# The lists gather in cache folders of their own, without the shipped list's.
export MELEE_SHIPPED_PIPELINES=0
mkdir -p "$work/cache" "$work/saves"

run() { # CACHE NAME ARGS...
    local cache=$1 name=$2
    shift 2
    MELEE_HD_CACHE=$cache "$player" --disc "$disc" --renderer fast --size 640x480 --scale 1 "$@" \
        >"$cache.log" 2>&1
    local rc=$?
    echo "$name: exit $rc; $(grep -h '^specialized shaders' "$cache.log" | cut -c1-80)"
}

for f in "$root"/data/bench/*.slp; do
    run "$work/cache" "$(basename "$f" .slp)" --bench "$ticks" --replay "$f"
done
for ruleset in vanilla slippi; do
    run "$work/cache" "power-on, $ruleset" --bench "${POWER_ON_TICKS:-3600}" --power-on \
        --ruleset "$ruleset" --saves "$work/saves"
    for f in "$@"; do
        case $f in
        *.inputs)
            run "$work/cache" "$(basename "$f"), $ruleset" --bench "$ticks" --inputs "$f" \
                --ruleset "$ruleset" --saves "$work/saves"
            ;;
        esac
    done
done

# The replays, JOBS at once, each player's list starting from the one so far.
replays=()
for f in "$@"; do
    case $f in *.slp) replays+=("$f") ;; esac
done
for ((j = 0; j < jobs; j++)); do
    cp -r "$work/cache" "$work/cache-$j"
    (
        for ((i = j; i < ${#replays[@]}; i += jobs)); do
            f=${replays[$i]}
            run "$work/cache-$j" "$(basename "$f")" --bench "$ticks" --replay "$f"
        done
    ) &
done
wait

# Each player's lists in turn: each entry once, where first listed.
lists=()
for ((j = 0; j < jobs; j++)); do lists+=("$work/cache-$j"); done
"${PYTHON:-python3}" -I "$here/merge-pipelines.py" "$root/data/pipelines" "${lists[@]}" || exit 1
