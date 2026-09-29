Fixed: every notice to workers and the lane (beads ready, batch-ready, the queue empty) was
  produced only by the coordinator's channel poll, so nobody was told anything while that
  session was stopped. `air record`, `air claim`, `air release` and `air close` now queue them
  after their write; the poll still runs as a sweeper, and each notice is sent once.
