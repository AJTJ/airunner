SurfaceChange {
    id: "handover-names-the-bead",
    since: "2026-09-05 (air-xbl)",
    headline: "`air handover` from a worktree holding no claim (and naming no bead) no \
               longer fails the digest check unconditionally: with nothing to declare, \
               the check is skipped. A refusal names the bead the worker holds instead \
               of a literal `<bead>`.",
    silent_break: false,
    action: "Nothing to run. A batching lane that merges other workers' green work and \
             holds no claim can now hand over; a worker that holds a claim or names a \
             bead must still declare it in the digest's `bead:` front matter, unchanged. \
             If a rule of yours told workers to expect `<bead>` in the message, delete it.",
}
