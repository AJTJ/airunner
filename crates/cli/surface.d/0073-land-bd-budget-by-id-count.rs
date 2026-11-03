SurfaceChange {
    id: "land-bd-budget-by-id-count",
    since: "2026-09-06 (air-fzv)",
    headline: "`air land`'s bd budget is `10 s + 2 s per id` rather than a flat 10 s for \
               the whole set, and the refusal names the id count, the budget and the \
               override. Measured here: `bd show` with 14 ids took 21.27 s against 1.94 s \
               for one, so a landing carrying many beads timed out on a budget that was \
               never sized for it.",
    silent_break: false,
    action: "Nothing to run. `AIR_BD_TIMEOUT_MS` overrides the whole budget if your bd is \
             slower or faster than ours; the refusal names it when you hit it.",
}
