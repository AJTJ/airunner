#!/bin/sh
# `make trial` (air-4usc, air-fkb): run the live trial (docs/design.md §9.1) with no keystroke
# from the owner once it starts. Copies examples/minimal to $TRIAL_DIR/minimal, adopts the copy
# with this tree's release build, pins that build into it with `air install --write --pin`,
# copies in the fixed scenario request (scripts/trial/request.md), starts the trial fleet with
# that request as the coordinator's launch prompt, and answers the development-channels prompt
# in every pane. The `air` on PATH, which the fleet building Air runs, is not touched.
#
# Needs bd and claude on PATH, like `air init` itself. TRIAL_DIR is the copy's parent and
# defaults to $TMPDIR/air-trial, one fixed path: Claude Code keys folder trust on the path, so
# the owner runs `claude` once in $TRIAL_DIR/minimal after the first `make trial`, answers yes,
# and every later trial at that path starts without the dialog (docs/research/folder-trust.md,
# air-ekj). Nothing here answers that dialog: the launcher refuses, naming the fix, while the
# copy is not trusted, and `make trial` is run again after it.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
bin="$here/target/release"
[ -x "$bin/air" ] || { echo "trial: build first (cargo build --release -p air)"; exit 1; }

# tmux as the launcher sees it: AIR_TMUX_SOCKET names a private server (the tests' and a
# rehearsal's), else the owner's own.
tmux_() {
    if [ -n "${AIR_TMUX_SOCKET:-}" ]; then tmux -L "$AIR_TMUX_SOCKET" "$@"; else tmux "$@"; fi
}

# Stop a previous trial's sessions first, so `make trial` never collides with it: they are named
# `<project>-*`, which the new copy's would also want, and a fleet left running on a deleted
# copy took the new fleet's session names once (0.4.9 trial, 2026-09-26). Only a copy this
# script made is touched: a directory holding minimal/ with a pinned air. Fails when $1 holds
# no such copy.
stop_copy() {
    [ -x "$1/minimal/.air/bin/air" ] || return 1
    old_project=$(sed -n 's/^ *"project": *"\([^"]*\)".*/\1/p' "$1/minimal/.claude/air.json" 2>/dev/null | head -n 1)
    [ -n "$old_project" ] || return 0
    for s in $(tmux_ ls -F '#{session_name}' 2>/dev/null | grep "^$old_project-" || true); do
        tmux_ kill-session -t "=$s"
        echo "trial: stopped the previous trial's tmux session $s"
    done
}
# Copies made by the earlier default, a fresh mktemp directory per run.
for old in "${TMPDIR:-/tmp}"/air-trial.*; do
    stop_copy "$old" || continue
    rm -rf "$old"
    echo "trial: removed the previous trial copy $old"
done
parent=${TRIAL_DIR:-${TMPDIR:-/tmp}/air-trial}
repo="$parent/minimal"
if stop_copy "$parent"; then
    rm -rf "$repo"
    echo "trial: removed the previous trial copy $repo"
fi
[ ! -e "$repo" ] || { echo "trial: $repo exists and is not a copy this script made"; exit 1; }
mkdir -p "$parent"
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
# The fleet always starts a lane, and every scenario in §9.1 but the happy path assumes the
# with-lane sequence, which `"verify_lane": true` selects (air-h91g: the 0.4.0 trial ran
# without it, workers verified their own branches, and eight of ten scenarios never started).
#
# The scenarios' own config is set here too, not left to the trial's coordinator (air-e07q):
# the second trial of 2026-09-29 stalled because that session's Claude Code refused to edit
# `.claude/air.json`, and the first trial's, given the same request, did not. The precheck and
# the `serve` lease, and the CLAUDE.md line naming the precheck, are in the adopt commit.
cfg=.claude/air.json
[ "$(head -n 1 "$cfg")" = "{" ] || { echo "trial: $cfg does not start with {"; exit 1; }
awk 'NR == 1 {
    print
    print "  \"verify_lane\": true,"
    print "  \"precheck\": true,"
    print "  \"leases\": {\"serve\": [\"sh serve.sh\"]},"
    next
} { print }' "$cfg" >"$cfg.new"
mv "$cfg.new" "$cfg"
for key in '"verify_lane": true' '"precheck": true' '"serve": \["sh serve.sh"\]'; do
    grep -q "$key" "$cfg" || { echo "trial: $cfg lacks $key"; exit 1; }
done
awk '/^- \*\*Precheck\*\*/ {
    print "- **Precheck** is `sh -n greet.sh`, recorded as `air record precheck -- sh -n greet.sh`."
    skip = 1
    next
}
skip && /^  / { next }
{ skip = 0; print }' CLAUDE.md >CLAUDE.md.new
mv CLAUDE.md.new CLAUDE.md
grep -q 'sh -n greet.sh' CLAUDE.md || { echo "trial: CLAUDE.md does not name the precheck"; exit 1; }
git add -A
git commit -qm "Adopt Air, pinned to the candidate, with the verification lane and the trial's config"

# The scenario request is fixed, so nobody rewrites it per run. It is the coordinator's launch
# prompt (air-fkb): told to read it from a file, the trial coordinator of 2026-11-05 had its
# `cat` refused by the auto-mode classifier and waited on the owner for hours (air-fzl). The
# file stays as the record; .air is ignored by git. stdin is not a terminal here, so the
# launcher starts every session detached and returns, and the panes are answered below.
cp "$here/scripts/trial/request.md" .air/trial-request.md
request=$(cat "$here/scripts/trial/request.md")
project=$(sed -n 's/^ *"project": *"\([^"]*\)".*/\1/p' "$cfg" | head -n 1)
echo "trial copy: $repo"
echo "pinned:     $("$repo/.air/bin/air" --version)"
"$repo/.air/bin/air" coordinator --fleet -- "$request" </dev/null \
    || { echo "trial: the fleet did not start; do what the launcher said, then run make trial again"; exit 1; }

# Every session warns about loading development channels (the Air channel) and Claude Code
# documents no skip; scripts/trial/answer-prompts.sh sends each pane the recorded Enter once the
# prompt is on it, and names any pane that shows neither the prompt nor the auto-mode status
# line within three minutes.
sh "$here/scripts/trial/answer-prompts.sh" "$project" \
    || echo "trial: a pane may still be waiting; read it with tmux capture-pane -p -t <session>"
echo "attach:     tmux attach -t $project-coordinator"
echo "report:     $repo/.air/trial-report.md, once every bead is closed"
