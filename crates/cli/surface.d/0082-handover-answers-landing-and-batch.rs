SurfaceChange {
    id: "handover-answers-landing-and-batch",
    since: "2026-09-06 (air-33rn, air-hpp8)",
    headline: "`air handover` now answers two questions a worker previously had to ask \
               somebody. It always prints a `landing:` line saying what `air land` would \
               say about this branch, read from the same selection the command runs; and \
               when a standing RED batch has this branch among its members it prints a \
               `batch:` line naming the batch sha, the lane, the time and where the lane's \
               output is. Both are lookups over values Air already computes, not second \
               copies of a decision. Workers are denied `air land` and were told about a \
               red batch only by the lane remembering to message each member, which in one \
               fleet reached everyone but one member twice in a night.",
    silent_break: false,
    action: "Nothing to run, and nothing existing changed: `--json` gained a `landing` key \
             always and a `batch` key only when there is a batch to name, alongside the \
             unchanged `pass`, `block`, `message` and `missing`. Read the `batch:` line \
             as one-directional — its ABSENCE is not a statement that your branch was not \
             in a batch, because Air knows membership only from what the run recorded, and \
             a run that recorded none is indistinguishable from no batch at all.",
}
