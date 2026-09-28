Added `crates/cli/src/cmd/lifecycle.rs`, a model test that drives the batch-ready rule, the cut's
  refusal, the batch scan and the close gate through every order of commit, cut, verify, land,
  close, merge and a move of main for two workers and a lane, and fails if any reachable state
  can no longer land and close every bead. `docs/design.md` §6.8 has the lifecycle table it
  checks.
