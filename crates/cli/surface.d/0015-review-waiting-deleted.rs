SurfaceChange {
    id: "review-waiting-deleted",
    since: "2026-08-29 (air-okc)",
    headline: "`review-waiting` and its whole support chain are deleted: the condition, \
               `air status`'s `review: N waiting` line, and the `bd list --status \
               awaiting_review` call that fed them.",
    silent_break: true,
    action: "It last fired here on 2026-08-22 because close-with-proof replaced hand-over \
             (air-7o3) and the state stopped occurring. **If your repo still uses \
             `awaiting_review`, you are the case this was deleted without**: a surface you \
             may be reading is simply gone, and a script grepping `air status` for \
             \"review:\" now finds nothing rather than 0. Say so and it can come back - it \
             was removed on this repo's evidence, not yours.",
}
