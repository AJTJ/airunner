SurfaceChange {
    id: "ledger-v12",
    since: "2026-08-29 (air-air, air-4cr)",
    headline: "Ledger schema 11 -> 12: `sessions.model`, and a `verify_inflight` table \
               behind `air status`'s in-flight verify reporting.",
    silent_break: false,
    action: "It migrates forward on the first write by the new binary and needs nothing \
             from you. It does NOT migrate backwards: if two `air` binaries of different \
             ages share one `.air/ledger.db`, upgrade them together. That is the case \
             worth checking before the first run, not after.",
}
