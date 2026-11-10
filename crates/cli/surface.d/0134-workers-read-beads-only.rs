SurfaceChange {
    id: "workers-read-beads-only",
    since: "2026-09-26 (owner ruling)",
    headline: "Workers and the lane read beads and add comments (`bd comment`, `bd comments add`); \
               every bd command that changes a bead or the store, `bd close` and \
               `bd update` among them, is on their deny list. Claims, closes and captures go \
               through `air claim`, `air close` and `air capture`.",
    silent_break: false,
    action: "Remove any instruction in your repo's own prose or prompts that tells a worker or the \
             lane to run a bd write command.",
}
