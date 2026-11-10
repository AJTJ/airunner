SurfaceChange {
    id: "worker-close-is-done",
    since: "2026-11-09 (air-4xj.2)",
    headline: "A worker's or the lane's `air close` is refused and names `air done <id> \
               --reason-file <proof>`: the worker declares its bead done as soon as the work \
               is committed, and the landing that puts that commit on the base branch closes \
               the bead. The coordinator's and the owner's `air close` are unchanged. The \
               `batch green` notice to members is gone, since a green asks nothing of them; \
               the red and dropped notices stay, and a member's `main moved` copy names the \
               beads of its own the landing closed and any done bead still waiting for its \
               head. `air status`'s loops line measures done to landed in place of batch green \
               to close.",
    silent_break: true,
    action: "Workers and the lane run `air done <id> --reason-file <proof>` where they ran \
             `air close`; `air handover --bead <id>` still names what the declaration needs. \
             Nothing to close on a green: the landing closes.",
}
