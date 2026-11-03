SurfaceChange {
    id: "every-decision-measured",
    since: "2026-09-25 (owner, air-hqj8)",
    headline: "`air audit` has a row for every decision Air writes and every timing budget \
               it can hit, each with a `counts:` line naming what it counts; rows with no \
               recorded removal condition say so as a defect. New event lines: \
               `batch-cut / no-precheck` per branch the precheck kept out of a cut, \
               `status / main-checkout-session` when `air status` prints that warning, and \
               `mcp.tool / timeout` when an MCP tool's subprocess is killed. Renamed: \
               `batch-cut / dropped` is now `batch-cut / drop`, and `air land`'s role \
               refusal is `land / refuse-role` rather than `land / refuse`.",
    silent_break: true,
    action: "Only if something of the repo's reads `.air/events/`: match the two renamed \
             decisions.",
}
