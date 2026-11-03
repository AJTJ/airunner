SurfaceChange {
    id: "dated-cutoffs",
    since: "2026-08-29 (air-24e)",
    headline: "`air doctor` reports every rule with a date in it and whether that date has \
               passed. Both of Air's expired on 2026-08-23 and made this repo's test \
               suite red for six days with nothing failing at the moment it broke.",
    silent_break: true,
    action: "Run `air doctor` and read the `dated rule` lines. If yours say EXPIRED, the \
             fallback is dead: a commit now needs a `Bead:` trailer to be attributed and a \
             digest needs a `bead:` front-matter line to satisfy the gate. Fixtures in \
             your own tests that write a commit or a digest \"now\" were inside the window \
             when they were written and are outside it now - that is what went red here, \
             and the same clock is in your copy.",
}
