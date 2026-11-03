SurfaceChange {
    id: "dated-fallbacks-deleted",
    since: "2026-09-28 (air-jsee)",
    headline: "The 2026-08-23 fallbacks are gone: a commit with no `Bead:` trailer is \
               attributed to nothing and a digest with no `bead:` front matter does not \
               count, whatever their date. `air doctor` has no dated rules left and \
               `air doctor --json` no longer has `dated_rules`.",
    silent_break: false,
    action: "A branch or digest from before 2026-08-23 that is still in play needs a \
             `Bead:` trailer or a `bead:` line; `air handover` names which.",
}
