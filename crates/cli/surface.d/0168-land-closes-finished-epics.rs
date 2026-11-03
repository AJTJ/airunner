SurfaceChange {
    id: "land-closes-finished-epics",
    since: "2026-09-28 (air-92o0)",
    headline: "`air land` closes each open epic that `bd epic status` reports with every \
               child closed and at least one child, after the landing. `air status` names an \
               epic to decompose only when it has no children (`epic ready to decompose: <id> \
               (no children yet)`), and the queue-empty notice says `epics with no children`. \
               The beads-without-initiative count prints only with `\"metis\": true`, and \
               `air batch cut` prints `(no bead)` rather than `()` for such a branch.",
    silent_break: true,
    action: "A script matching `epics with no open child` or `(0 open children, N closed)` \
             matches nothing now. An epic's \"Done when\" belongs in its last child's \
             acceptance, so the epic closes when that child lands.",
}
