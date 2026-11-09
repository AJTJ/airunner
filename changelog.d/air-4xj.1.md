Added `air done <id> --reason-file <proof>` for a worker or the lane: the hand-over gate without
  its green checks (the bead's commit, your claim, a declared precheck), the proof as a comment
  on the bead and the judged head on the claim. `air land` closes a done bead once the base
  branch contains that head, in one bd process, and releases the claim as `landed`; a bead
  already on the base branch closes at once. A bead nobody declared done is never closed by a
  landing. `air close` still works for a worker in this release.
