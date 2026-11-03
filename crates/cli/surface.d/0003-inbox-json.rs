SurfaceChange {
    id: "inbox-json",
    since: "2026-08-22 (air-6p5)",
    headline: "`air inbox --json` returns {captures, landings}, not a bare array.",
    silent_break: true,
    action: "Fix any script that indexes the top level as a list. It will not error: it \
             will read zero captures and report an empty queue.",
}
