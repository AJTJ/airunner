Added `model` and `lane_model` in `.claude/air.json`: every launcher passes `--model` from them
  when the command line gives none, `air fleet up --model <m>` overrides for one launch, and
  `air status` marks a session whose transcript names another model.
