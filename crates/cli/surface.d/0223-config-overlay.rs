SurfaceChange {
    id: "config-overlay",
    since: "2026-11-05 (air-aei)",
    headline: "Air reads `.claude/air.local.json` beside `.claude/air.json`, both from the main \
               checkout: the overlay's top-level keys replace the committed file's keys of the \
               same name, and a key it does not name comes from the committed file. It is for \
               a machine's own values, `targets` and the allow lists, which then stay out of \
               git and take effect when written rather than when a landing carries them. \
               `air status` names a key set there that the binary does not read with the \
               file, says once when the overlay is not valid JSON (the committed file is then \
               read alone), and warns once if git tracks it.",
    silent_break: false,
    action: "Nothing unless you want it. To use it, add `.claude/air.local.json` to \
             `.gitignore` yourself (`air init` never writes it), move each machine's \
             `targets` there, and leave the committed file with what Air does.",
}
