SurfaceChange {
    id: "install-reports-bd-prime",
    since: "2026-09-05 (air-b5k)",
    headline: "`air install` reports a `bd prime` hook left in `.claude/settings.json` \
               (`STALE HOOK: SessionStart runs `bd prime --hook-json`...`) on every run \
               until it is gone; the adoption doc no longer asks for a hand edit nobody \
               re-checks.",
    silent_break: false,
    action: "Run `air install` and read any STALE HOOK line: delete the entry it names. \
             The merge never removes another tool's hook, so the report is the only thing \
             that will keep saying it is there.",
}
