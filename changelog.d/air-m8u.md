Changed The coordinator's branch is batch-ready only at a head it offered with `air ready` (new,
  coordinator only), unless `.claude/air.json` sets `coordinator_autobatch` true; `air status
  --json` says `not-offered` until then. Every coordinator commit used to cost the lane a cut.
