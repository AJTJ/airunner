SurfaceChange {
    id: "land-worker",
    since: "2026-09-05 (air-09b)",
    headline: "`air land --worker <name>` names the branch to land. `air land <bead>` is \
               refused when more than one branch carries the bead, naming each carrier; \
               `air status` and `air inbox --owner` now offer the `--worker` form.",
    silent_break: false,
    action: "Land by branch: `air land --worker <name>`, which merges that branch with \
             every bead its range names. Naming a bead still works while exactly one \
             branch carries it. A script that greps the offered command for `air land \
             <bead>` reads `air land --worker <name>` now.",
}
