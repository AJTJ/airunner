SurfaceChange {
    id: "audit-splits-pushes",
    since: "2026-08-29 (air-5uz)",
    headline: "`air audit` no longer reports \"fires\". A condition now has separate \
               `evaluated` and `pushed` counts, because the poll re-evaluates and calling \
               that a firing read 10,722 log lines as 45 real pushes.",
    silent_break: true,
    action: "A script reading the old field gets nothing and reports zero rather than \
             erroring. Also read the counts differently than before: any `pushed` figure \
             from a day before 2026-08-29 is uninformative, because nothing recorded a \
             push until this landed. A zero there is a fact about what was countable.",
}
