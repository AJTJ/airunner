Fixed `air land` could not close a bead its worker held: bd 1.3.0 refuses a close by another
  actor without `--force`, and the lane was refused at every landing of the 2026-11-09 trial.
  The landing's close passes `--force` (the landing's only; `air close` and `air reclaim` pass
  none), a close bd refuses even so is reported as `done: bd refused to close` and in the
  `main moved` notice as `done but refused`, apart from a bead still waiting for its head, and
  `air handover` names a missing precheck whatever the green, as `air done` does.
