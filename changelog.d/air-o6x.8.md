Fixed The `air ui` beads board's ready column held epics and owner-labelled beads that bd
  lists as ready and no worker may claim. `/api/beads` puts them in `not_claimable`, by the same
  rule `air claim` and the `ready:` line use, and the page shows them as muted chips above the
  board.
