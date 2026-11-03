SurfaceChange {
    id: "ancestor-deadlock-named",
    since: "2026-09-06 (air-btz)",
    headline: "`air status` names a bead blocked by one of its own ancestors, with the \
               edge and the `bd dep remove` that clears it. Such a bead can never become \
               ready — the ancestor cannot finish until its descendants do — and bd shows \
               it as \"not ready yet\" like any queued bead. bd 1.2.2 refuses the edge on \
               nine routes but NOT from `bd create --graph` or `bd create --parent X \
               --deps <ancestor>`, because its guard is a parent-child row on the pair \
               plus a dotted-id prefix test, not an ancestor walk.",
    silent_break: false,
    action: "Nothing to run, and silent unless a repo has the shape. It costs one \
             `bd list --status …` per tick, plus one `bd dep list` only when a bead that \
             has a parent also has an edge. If you file waves with `bd create --graph`, \
             read `bd dep tree <epic> --json` after each one for an edge from a child to \
             ANY ancestor: `bd dep cycles` does not report this shape, because the \
             hierarchy is definitional rather than an edge.",
}
