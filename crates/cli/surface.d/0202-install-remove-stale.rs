SurfaceChange {
    id: "install-remove-stale",
    since: "2026-09-29 (air-e07q)",
    headline: "`air install --write --remove-stale` removes the hooks `air install` reports \
               as STALE HOOK (a `bd prime` entry) from `.claude/settings.json`; without the \
               flag nothing another tool wrote is removed, as before. Air's docs no longer \
               ask anyone to edit a config file by hand for something Air requires.",
    silent_break: false,
    action: "If `air install` reports a STALE HOOK, run `air install --write --remove-stale`.",
}
