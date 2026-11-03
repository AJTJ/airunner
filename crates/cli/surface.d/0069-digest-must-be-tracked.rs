SurfaceChange {
    id: "digest-must-be-tracked",
    since: "2026-09-06 (air-ahlf)",
    headline: "The hand-over gate REFUSES a digest git does not track, under its own check \
               name. It used to read the directory, so a digest you wrote and never added \
               satisfied it — a file only your worktree has, offered as proof to somebody \
               who cannot see it. `digest_for_bead` now answers Missing, Untracked or \
               Tracked, and the two get different fixes because \"write one\" and \"add the \
               one you wrote\" are different repairs.",
    silent_break: false,
    action: "This is a NEW REFUSAL you can hit: commit the digest, not just write it. A \
             tracked digest beside an untracked stray still passes. If your flow writes \
             digests late, move the commit before the close.",
}
