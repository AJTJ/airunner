SurfaceChange {
    id: "batch-landing-attributes-every-bead",
    since: "2026-09-05 (air-80x.2)",
    headline: "`air land --worker <lane>` lands a batch branch as ONE landing and \
               attributes every bead the merged branches carry by `Bead:` trailer, with \
               the member heads recorded on the landing row. Without this a lane's batch \
               landed as one branch and the beads its members carried were attributed to \
               nothing.",
    silent_break: false,
    action: "Nothing to run, and nothing changes for a repo with no lane. If you run one, \
             put a `Bead:` trailer on every commit that does a bead's work — that trailer \
             is the whole attribution, and a commit without one is attributed to nothing.",
}
