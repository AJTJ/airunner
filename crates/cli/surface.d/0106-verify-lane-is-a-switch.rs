SurfaceChange {
    id: "verify-lane-is-a-switch",
    since: "2026-09-25 (air-rr98)",
    headline: "`verify_lane` in `.claude/air.json` is now `true` or absent. `true` puts the \
               with-lane closing sequence in force; it no longer names the lane, which is \
               the session `air lane` started. A string value still counts as `true`, and \
               no code reads the key: `air batch cut`'s refusal in the main checkout names \
               `air lane`'s worktree instead of the key's value.",
    silent_break: false,
    action: "Optional: change `\"verify_lane\": \"<name>\"` to `\"verify_lane\": true`. A \
             lane whose worktree is not `lane` is started with `air lane <name>`.",
}
