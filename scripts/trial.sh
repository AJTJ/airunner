#!/bin/sh
# `make trial` (air-4usc): prepare the live trial (docs/design.md §9.1). Copies examples/minimal
# to a scratch directory outside this repo, adopts it with this tree's release build, and pins
# that build into the copy with `air install --write --pin`. The `air` on PATH, which the fleet
# building Air runs, is not touched. Starts no session; it prints how to start one.
#
# Needs bd and claude on PATH, like `air init` itself. TRIAL_DIR picks the parent directory.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
bin="$here/target/release"
[ -x "$bin/air" ] || { echo "trial: build first (cargo build --release -p air)"; exit 1; }

parent=${TRIAL_DIR:-$(mktemp -d "${TMPDIR:-/tmp}/air-trial.XXXXXX")}
repo="$parent/minimal"
[ ! -e "$repo" ] || { echo "trial: $repo already exists"; exit 1; }
unset AIR_PROJECT AIR_ROLE AIR_ENFORCE CLAUDE_PID BEADS_ACTOR 2>/dev/null || true

# The project as it was before Air: examples/minimal minus every file `air init` writes.
cp -R "$here/examples/minimal" "$repo"
cd "$repo"
rm -rf CLAUDE.md .claude .mcp.json .worktreeinclude .gitignore
git init -q -b main
git add -A
git commit -qm "the project before Air"

# `air init --write` runs `air install --write`, which wants the binary it runs on PATH; the
# candidate is on PATH for this one command only.
PATH="$bin:$PATH" "$bin/air" init --write >/dev/null
"$bin/air" install --write --pin >/dev/null
git add -A
git commit -qm "Adopt Air, pinned to the candidate"

echo "trial copy: $repo"
echo "pinned:     $("$repo/.air/bin/air" --version)"
echo "start it:   cd $repo && .air/bin/air coordinator"
echo "Start with the pinned copy: an older air on PATH does not know about the pin and would"
echo "start sessions without it. Every session Air starts there runs the pin."
