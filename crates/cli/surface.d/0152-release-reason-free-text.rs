SurfaceChange {
    id: "release-reason-free-text",
    since: "2026-09-26 (0.4.9 live trial)",
    headline: "`air release --reason` and `air reclaim --reason` take any text. The former \
               fixed words (landed, abandoned, reassigned, superseded, false-premise, \
               owner-gated, unknown) are the documented common values; `air audit` still \
               reads `owner-gated` by substring.",
    silent_break: false,
    action: "",
}
