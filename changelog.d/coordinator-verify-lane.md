Changed this repo sets `verify_lane: true`: the lane verifies once per batch and workers run no
  verify of their own (four parallel Rust verifies had put the owner's machine at load 40).
