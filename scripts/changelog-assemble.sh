#!/bin/sh
# Fold changelog.d/<bead>.md fragments into CHANGELOG.md's [Unreleased] section (air-be6d).
#
# Every bead used to add its line under the same CHANGELOG.md heading, so every landing
# conflicted with every other branch that had one: nine drops by the lane on 2026-09-28, each a
# merge, a resolve and a re-verify. A fragment is a file no other bead touches.
#
# A fragment's first word is its heading (Added, Changed, Removed or Fixed; a trailing colon is
# allowed) and the rest is the entry, wrapped by its author with two-space continuation lines:
#
#     Fixed `air close` named the wrong bead when ...
#       and a second line.
#
# Usage: changelog-assemble.sh [--dry-run] [DIR] [CHANGELOG]
#   --dry-run prints the assembled changelog and changes nothing. Otherwise CHANGELOG is
#   rewritten and the fragments are deleted. `make changelog` runs it; `make release` refuses
#   while fragments are left.
set -eu

dry=0
if [ "${1:-}" = "--dry-run" ]; then
    dry=1
    shift
fi
dir=${1:-changelog.d}
log=${2:-CHANGELOG.md}

frags=$(find "$dir" -maxdepth 1 -name '*.md' ! -name README.md 2>/dev/null | sort)
if [ -z "$frags" ]; then
    [ "$dry" -eq 1 ] && cat "$log"
    echo "changelog: no fragments in $dir" >&2
    exit 0
fi

tmp=$(mktemp)
list=$(mktemp)
trap 'rm -f "$tmp" "$list"' EXIT
echo "$frags" > "$list"
# Every fragment names a heading this changelog has, before anything is written.
for f in $frags; do
    head=$(sed -n '1s/^\([A-Za-z]*\):\{0,1\}[[:space:]].*/\1/p' "$f")
    case "$head" in
        Added | Changed | Removed | Fixed) ;;
        *)
            echo "changelog: $f does not start with Added, Changed, Removed or Fixed" >&2
            exit 1
            ;;
    esac
done

awk -v list="$list" '
    BEGIN {
        while ((getline f < list) > 0) {
            first = 1
            while ((getline line < f) > 0) {
                if (first) {
                    h = line
                    sub(/[:[:space:]].*/, "", h)
                    sub(/^[A-Za-z]+:?[[:space:]]+/, "", line)
                    entry = "- " line
                    first = 0
                } else if (line != "") {
                    entry = entry "\n" line
                }
            }
            close(f)
            add[h] = (h in add) ? add[h] "\n" entry : entry
        }
    }
    /^## / { unreleased = ($0 ~ /^## \[Unreleased\]/) }
    {
        print
        if (unreleased && $0 ~ /^### / ) {
            h = substr($0, 5)
            if (h in add) { pending = h }
        } else if (pending != "" && $0 == "") {
            print add[pending]
            done[pending] = 1
            pending = ""
        }
    }
    END {
        for (h in add) {
            if (!(h in done)) {
                print "changelog: no ### " h " heading under [Unreleased]" > "/dev/stderr"
                exit 1
            }
        }
    }
' "$log" > "$tmp"

if [ "$dry" -eq 1 ]; then
    cat "$tmp"
else
    cp "$tmp" "$log"
    for f in $frags; do rm "$f"; done
    echo "changelog: folded $(echo "$frags" | wc -l | tr -d ' ') fragment(s) into $log" >&2
fi
