Removed The `closed-not-landed` attention condition: its input, a worker's close before the
  landing, no longer arrives once a worker declares done and the landing closes. Recorded
  firings still attribute in `air audit`. `handover-not-green` is renamed `done-refused`: a
  worker whose `air done` was refused and has not passed since, with the attempt count per
  bead and the last attempt's time, whatever the green at its head.
