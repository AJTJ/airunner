SurfaceChange {
    id: "lane-gate-no-merge-main",
    since: "2026-09-26 (0.4.8 live trial)",
    headline: "Under a lane (`\"verify_lane\": true`, or a session started by `air lane`), \
               `air close` and `air handover` name `main-merged` only when a green at the \
               head exists and lacks main, and the Stop hook says nothing when every missing \
               check is one the lane's batch will satisfy. Without a lane, unchanged.",
    silent_break: false,
    action: "",
}
