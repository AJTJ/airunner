SurfaceChange {
    id: "gc-retention",
    since: "2026-08-29 (air-i7s)",
    headline: "`air gc [--keep-days N] [--apply]` exists and DELETES event files under \
               `.air/events/`. It is manual: nothing runs it for you.",
    silent_break: false,
    action: "If you archive or ship `.air/events/` anywhere, know that a command now \
             removes files from it. Dry-run first - without `--apply` it only reports.",
}
