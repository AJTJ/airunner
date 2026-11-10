SurfaceChange {
    id: "coordinator-may-commit",
    since: "2026-08-29 (air-iy1)",
    headline: "`Bash(git commit *)` is out of COORDINATOR_DENY. The boundary is the \
               remote, not main: `git push` is still denied.",
    silent_break: false,
    action: "Your coordinator GAINS a permission on its next launch - it can commit on \
             main by hand, which it could not before. Nothing forces it to; `air land` is \
             still how a worker's branch reaches main. If your repo relied on the deny to \
             keep main untouched by the coordinator, that is no longer what stops it.",
}
