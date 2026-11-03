SurfaceChange {
    id: "bd-budgets-generous-and-scaled",
    since: "2026-09-25 (air-8lj8)",
    headline: "bd calls that refuse a command on timeout are generous now: 60 s per bd process \
               (was 10 s), plus 5 s per id for every call naming several ids, from one \
               shared function. `air close <ids...>` gets the per-id allowance for the first \
               time (it was a flat 10 s), and `air land`'s acceptance read goes from 10 s + \
               2 s per id to 60 s + 5 s per id. The probe after a timed-out claim, and the \
               id check in a triage pass, go from 5 s to 30 s. `air status` (8 s cap, under \
               the 20 s MCP tool limit) and the Stop nudge (3 s, under the 5 s hook cap) are \
               unchanged. A timeout now names the id count, the budget and the override.",
    silent_break: false,
    action: "If you exported `AIR_BD_TIMEOUT_MS` to get past the old 10 s, you can drop it: \
             while set it replaces the WHOLE budget with one flat figure, per-id allowance \
             included, so a large `air close` can hit it where the default would not.",
}
