SurfaceChange {
    id: "verification-lane",
    since: "2026-09-05 (air-80x.1, air-80x.6)",
    headline: "A verification lane. The close gate now accepts a green at a verified \
               commit that contains `main` and every commit carrying the bead's trailer, \
               recorded by any worker (air-80x.1): a worker closes on a lane's batch green \
               with no verify run of its own. `.air/roles.md` gains a Verification lane \
               section under Worker stating what Air records and refuses for it (air-80x.6).",
    silent_break: false,
    action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. If your repo \
             runs a lane, its flow (who the lane is, its cadence, what workers do instead \
             of verifying) goes in your CLAUDE.md, not in roles.md. Without a lane nothing \
             changes: a green at HEAD containing `main` still closes exactly as before.",
}
