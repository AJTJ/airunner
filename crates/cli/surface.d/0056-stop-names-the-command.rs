SurfaceChange {
    id: "stop-names-the-command",
    since: "2026-09-06 (air-avj)",
    headline: "The Stop hook no longer prints the flow-dependent repairs. It stated `git \
               merge main` and `air record verify -- make verify`, which under a verify \
               lane are the two things the lane exists to prevent — merging moves the head \
               off the sha the lane cut at, and recording a green is the lane's job. It \
               now states the same FACTS and names `air handover`, which reads your repo's \
               flow and prints the repair it calls for. Every other fix, and every CLI \
               surface, is unchanged.",
    silent_break: false,
    action: "Nothing to run. If anything of yours greps the Stop hook's \
             `additionalContext` for `git merge main`, it will not find it; the check \
             names and details are unchanged, and `air handover --json` still carries \
             every `fix` verbatim, plus a new `flow_dependent` flag per check. Air does \
             NOT read `verify_lane` for this: that key stays yours, and a hook branching \
             on it would be a second copy of a decision `air handover` already makes.",
}
