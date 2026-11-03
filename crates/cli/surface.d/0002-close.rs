SurfaceChange {
    id: "close",
    since: "2026-08-22 (air-869)",
    headline: "`air close <id>… --reason` closes a landing pass in ONE bd process.",
    silent_break: false,
    action: "Use it instead of a loop over `bd close`: bd costs ~1.4 s per process here \
             whatever it is asked, so N closes cost N × that. Denied to workers.",
}
