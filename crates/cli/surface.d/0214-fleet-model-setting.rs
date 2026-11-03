SurfaceChange {
    id: "fleet-model-setting",
    since: "2026-11-03 (air-ap1)",
    headline: "`model` and `lane_model` in `.claude/air.json` are the fleet's model: every \
               launcher passes `--model` from them when the command line gives none, `air \
               fleet up --model <m>` overrides for one launch, and `air status` marks a \
               session whose transcript names another model.",
    silent_break: false,
    action: "Optional: add `\"model\": \"<id>\"` to `.claude/air.json` so a relaunch \
             lands on the owner's model without `--model` each time.",
}
