SurfaceChange {
    id: "idle-without-claim-respects-a-running-verify",
    since: "2026-09-06 (air-t6ap)",
    headline: "`idle-without-claim` no longer fires for a worker whose own verify is in \
               flight. It offered an adopter's verify lane 58 claimable beads 945 seconds \
               into a batch verify, with `air status` printing `verify in flight` three \
               lines above — and a lane that claims a bead mid-batch cannot cut the batch, \
               so the remedy was the failure it exists to prevent.",
    silent_break: false,
    action: "Nothing to run. The condition still fires for a genuinely idle worker with \
             claimable work, and somebody else's verify does not silence it. If you added \
             a rule telling your coordinator to ignore this condition for a lane, you can \
             drop it.",
}
