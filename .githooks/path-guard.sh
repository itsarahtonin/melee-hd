#!/usr/bin/env bash
# Refuses personal paths and names, build output and oversized files, by the rules in
# path-guard.conf beside it (the same rules tools/path-guard applies in CI and `cargo test`).
#
#   path-guard.sh commit|merge   the index: everything the next commit records (pre-commit and
#                                pre-merge-commit; git points GIT_INDEX_FILE at the right one)
#   path-guard.sh push REMOTE    every blob new to REMOTE in the refs pushed (pre-push's stdin),
#                                in every commit of the range, so one added and later deleted
#                                counts too
set -u

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
conf="$here/path-guard.conf"
mode=${1:-commit}
remote=${2:-}

deny=()
allow=()
forbid=()
large=()
max_size=
while IFS= read -r line || [ -n "$line" ]; do
    line=${line%$'\r'}
    case $line in '' | '#'*) continue ;; esac
    read -r key value <<<"$line"
    case $key in
        deny) deny+=(-e "$value") ;;
        allow) allow+=("$value") ;;
        forbid) forbid+=("$value") ;;
        max-size) max_size=$value ;;
        large) large+=("$value") ;;
        *)
            echo "path-guard: $conf: unknown line: $line" >&2
            exit 2
            ;;
    esac
done <"$conf"
if [ ${#deny[@]} -eq 0 ] || [ -z "$max_size" ]; then
    echo "path-guard: $conf names no deny patterns or no max-size" >&2
    exit 2
fi

names=()
while IFS= read -r n; do
    [ -n "$n" ] && names+=(-e "$n")
done < <(git config --get-all path-guard.name)
for n in ${PATH_GUARD_NAMES:-}; do
    names+=(-e "$n")
done

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
found=0

report() {
    found=1
    printf '  %s\n' "$1" >&2
}

# How a path in the index scanned reads in a report: in push mode, entries are BLOB/PATH.
label() {
    if [ "$mode" = push ]; then
        printf '%s (blob %s)' "${1#*/}" "${1%%/*}"
    else
        printf '%s' "$1"
    fi
}

allowed() {
    local last=${1##*[\\/]} a
    while [[ $last == *. ]]; do last=${last%.}; done
    for a in ${allow[@]+"${allow[@]}"}; do
        [ "$last" = "$a" ] && return 0
    done
    return 1
}

# Matches shown per file; a binary can hold thousands.
shown=5

# git grep over the index in use (GIT_INDEX_FILE), every file as text. $1: deny or name.
scan_content() {
    local kind=$1 st p ln col m last= n=0
    shift
    git grep --cached --no-color -z -a -n --column -o "$@" >"$tmp/grep"
    st=$?
    if [ $st -gt 1 ]; then
        echo "path-guard: git grep failed ($st)" >&2
        exit 2
    fi
    # git grep reports a file's matches together.
    while IFS= read -r -d '' p && IFS= read -r -d '' ln && IFS= read -r -d '' col && IFS= read -r m; do
        if [ "$kind" = deny ] && allowed "$m"; then continue; fi
        if [ "$p" != "$last" ]; then
            [ $n -gt $shown ] && report "$(label "$last"): $((n - shown)) more"
            last=$p
            n=0
        fi
        n=$((n + 1))
        [ $n -le $shown ] && report "$(label "$p"):$ln:$col: \`$m\`"
    done <"$tmp/grep"
    [ $n -gt $shown ] && report "$(label "$last"): $((n - shown)) more"
}

check_path() {
    local d
    for d in ${forbid[@]+"${forbid[@]}"}; do
        [[ $1 == "$d"* ]] && report "$1: under $d, which is never committed"
    done
}

# Reads "SIZE PATH" lines.
check_sizes() {
    local size path l ok
    while read -r size path; do
        case $size in '' | *[!0-9]*) continue ;; esac
        [ "$size" -le "$max_size" ] && continue
        ok=
        for l in ${large[@]+"${large[@]}"}; do
            [ "$path" = "$l" ] && ok=1
        done
        [ -z "$ok" ] && report "$path: $size bytes, over the $max_size-byte limit"
    done
}

scan_index() {
    scan_content deny -E "${deny[@]}"
    if [ ${#names[@]} -gt 0 ]; then
        scan_content name -F -i ${names[@]+"${names[@]}"}
    fi
}

case $mode in
    commit | merge)
        what="the $mode"
        git -c core.quotepath=off ls-files -s >"$tmp/ls"
        while IFS=$'\t' read -r meta path; do
            check_path "$path"
        done <"$tmp/ls"
        # Mode, blob and stage, then the path; submodules (160000) have no blob here.
        while IFS=$'\t' read -r meta path; do
            set -- $meta
            [ "$1" = 160000 ] || printf '%s %s\n' "$2" "$path"
        done <"$tmp/ls" | git cat-file --batch-check='%(objectsize) %(rest)' >"$tmp/sizes"
        check_sizes <"$tmp/sizes"
        scan_index
        ;;
    push)
        what="the push to $remote"
        export GIT_INDEX_FILE="$tmp/index"
        zero='^0+$'
        while read -r _ lsha _ rsha; do
            [[ $lsha =~ $zero ]] && continue
            excl=(--not --remotes="$remote")
            if ! [[ $rsha =~ $zero ]] && git cat-file -e "$rsha^{commit}" 2>/dev/null; then
                excl+=("$rsha")
            fi
            while IFS= read -r path; do
                [ -n "$path" ] && check_path "$path"
            done < <(git -c core.quotepath=off log -m --no-renames --diff-filter=A --name-only --format= "$lsha" "${excl[@]}" | sort -u)
            git rev-list --objects "$lsha" "${excl[@]}" |
                git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' |
                while read -r type sha size path; do
                    [ "$type" = blob ] && printf '%s %s %s\n' "$sha" "$size" "$path"
                done >>"$tmp/blobs"
        done
        if [ -s "$tmp/blobs" ]; then
            while read -r sha size path; do printf '%s %s\n' "$size" "$path"; done <"$tmp/blobs" >"$tmp/sizes"
            check_sizes <"$tmp/sizes"
            while read -r sha size path; do printf '100644 %s\t%s/%s\n' "$sha" "$sha" "$path"; done <"$tmp/blobs" |
                git update-index --add --index-info
            scan_index
        fi
        ;;
    *)
        echo "usage: path-guard.sh commit|merge|push REMOTE" >&2
        exit 2
        ;;
esac

if [ $found -ne 0 ]; then
    cat >&2 <<EOF
path-guard: refusing $what: the files above hold personal paths or names, build output
(dist/, target/) or more than $max_size bytes (rules: .githooks/path-guard.conf).
EOF
    if [ "$mode" = push ]; then
        echo "Rewrite the commits that added them before pushing; git log --all --find-object=BLOB finds them." >&2
    fi
    exit 1
fi
exit 0
