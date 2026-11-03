SurfaceChange {
    id: "owner-label",
    since: "2026-08-22 (air-5hw)",
    headline: "The authority label is `owner`. `human` is presence only and gates nothing.",
    silent_break: true,
    action: "See the migration in docs/rules/adopting-air.md §5a. A repo that used \
             `human` as its gate has its owner queue unfenced the moment it upgrades: \
             exclude BOTH labels until its beads are relabelled.",
}
