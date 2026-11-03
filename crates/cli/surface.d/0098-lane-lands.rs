SurfaceChange {
    id: "lane-lands",
    since: "2026-09-25 (owner ruling 2026-09-14, air-jc2p.2)",
    headline: "New `air lane [<name>]` starts the verification lane (worktree `lane` by \
               default): a worker session with AIR_ROLE=lane and the worker deny list minus \
               `air land`. `air land` is now refused to AIR_ROLE=coordinator as well as to \
               workers; the lane and the owner's own shell may land. `air close` is refused \
               to the lane as to a worker. Workers are denied `air lane`.",
    silent_break: true,
    action: "Start the lane with `air lane` (`air lane w4` keeps an existing worktree). A \
             lane still running as `air worker <name>` is refused `air land` and is offered \
             ready beads again until relaunched. A coordinator that landed must hand that to \
             the lane; the owner's shell (no AIR_ROLE) can still land by hand.",
}
