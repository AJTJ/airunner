#!/bin/sh
# `make adoption-check` (air-livz): adopt examples/minimal the way a new user would, with this
# tree's `air` first on PATH, and fail on any non-zero exit, on output that says something went
# wrong, or on any difference between what `air init` wrote and what examples/minimal holds.
#
# Why: building examples/minimal by hand on 2026-09-25 found five `air init` defects in one run
# that no test caught (air-gn5o). This is that run, repeated. Removed when `air init` is gone
# or a test suite runs the same steps.
#
# Needs bd (pinned), dolt, tmux and claude on PATH, like `air init` itself. Uses a private tmux
# socket, so the Dolt server `air init --write` starts for the example runs there and is stopped
# on exit.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
bin="$here/target/debug"
[ -x "$bin/air" ] || { echo "adoption-check: build first (cargo build -p air)"; exit 1; }

tmp=$(mktemp -d)
trap 'tmux -L "$AIR_TMUX_SOCKET" kill-server 2>/dev/null; rm -rf "$tmp"' EXIT
# A `metis` that is not there, so the result does not depend on whether this machine has it.
mkdir "$tmp/bin"
printf '#!/bin/sh\nexit 127\n' > "$tmp/bin/metis"
chmod +x "$tmp/bin/metis"
export PATH="$bin:$tmp/bin:$PATH"
unset AIR_PROJECT AIR_ROLE AIR_ENFORCE CLAUDE_PID BEADS_ACTOR 2>/dev/null || true
export AIR_TMUX_SOCKET="air-adoption-check-$$"
export GIT_AUTHOR_NAME=air GIT_AUTHOR_EMAIL=air@localhost
export GIT_COMMITTER_NAME=air GIT_COMMITTER_EMAIL=air@localhost

# The project as it was before Air: examples/minimal minus every file `air init` writes. The
# directory is named `minimal` because init writes the directory name as "project".
repo="$tmp/minimal"
cp -R "$here/examples/minimal" "$repo"
(cd "$repo" && rm -rf CLAUDE.md .claude .mcp.json .worktreeinclude .gitignore)
cd "$repo"
git init -q -b main
git add -A
git commit -qm "the project before Air"

fail() { echo "adoption-check: $*"; exit 1; }
# step <name> <must-contain> <must-not-contain regex> -- cmd...
step() {
    name=$1 want=$2 refuse=$3
    shift 4
    out=$("$@" 2>&1) || { echo "$out"; fail "$name exited non-zero"; }
    [ -z "$want" ] || printf '%s' "$out" | grep -qF -- "$want" || { echo "$out"; fail "$name: expected \"$want\""; }
    ! printf '%s' "$out" | grep -qiE -- "$refuse" || { echo "$out"; fail "$name: unexpected output matching /$refuse/"; }
    echo "ok  $name"
}
bad='warning|error|refus|suspicious|stale|NOT FOUND|mismatch'

step "air init" 'verify:   `make verify` (found in this repo)' "$bad" -- air init
step "air init --write" "air record verify -- make verify" "$bad" -- air init --write
git add -A
git commit -qm "Adopt Air"
step "air install" "already wired" "$bad|!!" -- air install
step "air record verify" "recorded green verify" "$bad" -- air record verify -- make verify
step "air status" "green" "$bad" -- air status
step "air coordinator --print" "claude" "$bad" -- air coordinator --print
step "air lane --print" "claude" "$bad" -- air lane --print
step "air worker --print" "claude" "$bad" -- air worker --print

# What init wrote must be what the example shows. Generated per machine, and not in the
# example: the ledger, the task store, the skills `air install` writes, launcher worktrees.
diff -r -x .git -x .air -x .beads -x .beads.gate.lock -x skills -x worktrees "$here/examples/minimal" "$repo" \
    || fail "air init writes something examples/minimal does not show; regenerate the example"
echo "ok  examples/minimal matches what air init writes"
