SurfaceChange {
    id: "lane-land-allow",
    since: "2026-09-25 (air-8cdh)",
    headline: "`air lane` (and `air fleet up`) put `permissions.allow: [\"Bash(air land *)\"]` \
               on the lane's own `--settings`, plus the pinned binary's absolute path when \
               the repo is pinned. In auto mode an allow rule resolves before the \
               classifier, which had denied the lane's `air land` as \"[Modify Shared \
               Resources]\". No other role gets it; a pass-through `--settings` allow list \
               is now combined with Air's rather than replaced.",
    silent_break: false,
    action: "Restart a running lane through `air lane` or `air fleet up` to pick it up.",
}
