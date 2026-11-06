Changed the ready notice: `<n> bead(s) are claimable; run `bd ready`` instead of a list of ids
  that was stale by the time a worker read it; the per-recipient rotation of that list is gone
  with it.
