SurfaceChange {
    id: "close-text-match-removed",
    since: "2026-09-26 (owner ruling)",
    headline: "The hook no longer reads closes from shell commands: the hand-over check on \
               `bd close` and `bd update -s closed|awaiting_review`, and the claim release \
               after a `bd close`, are gone. Workers and the lane are denied bd's write \
               commands, and `air close` runs the hand-over gate and releases the claims it \
               closes.",
    silent_break: true,
    action: "A session Air did not launch (no worker deny list) that closes with raw \
             `bd close` is no longer checked and its claim is released only when `air \
             status` reconciles with bd. Close with `air close` instead.",
}
