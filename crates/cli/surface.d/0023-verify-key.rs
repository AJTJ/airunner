SurfaceChange {
    id: "verify-key",
    since: "2026-09-05 (air-7wf)",
    headline: "A recorded green is keyed by COMMIT, by whichever worker ran it; the worker \
               is no longer part of the gate's key. Opt in to keying by TREE with \
               `\"verify_key\": \"tree\"` in .claude/air.json, and `air status` names a \
               tree green either way instead of reading `not green` after every landing.",
    silent_break: true,
    action: "Two things changed under you. (1) Any worker's green at a sha now satisfies \
             every worker's gate at that sha: a batching lane's one verify stands for the \
             workers that fast-forward onto it. Who ran it is still on the row. (2) The \
             tree key is NOT on by default and must not be turned on by reflex: a green \
             transfers to an identical tree only if your verify is a function of the tree \
             alone. The adopter's is not - `make verify` runs `git log main..HEAD` to pick \
             the beads it checks (scripts/lib/bead_citations.py:140), so two commits over \
             one tree verify differently there and a tree-keyed gate would pass beads it \
             never checked. Ten-minute check before declaring it: grep your verify for \
             `git log`, `rev-list`, `describe` and anything reading commit messages. Without \
             the declaration nothing about the gate's strictness changed; the price is one \
             re-verify per landed bead, which `air status` now names as such.",
}
