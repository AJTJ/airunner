SurfaceChange {
    id: "fresh-session-idle",
    since: "2026-09-26 (air-ludo)",
    headline: "A session is recorded idle at its start, not working: a worker launched with \
               no task takes no turn, so `air status` called it `working since <launch>` for \
               good. The first tool call makes it `running`; a start caused by compaction \
               keeps its state. \"beads are ready\" now goes to every worker with a live \
               session holding no claim, whatever its state, not only an idle one; the \
               2026-09-26 trial's fleet reached nobody and stalled at `ready: 3`. \
               `idle-without-claim` can now name a worker that never took a turn.",
    silent_break: false,
    action: "",
}
