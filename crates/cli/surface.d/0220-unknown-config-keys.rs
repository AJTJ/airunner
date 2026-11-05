SurfaceChange {
    id: "unknown-config-keys",
    since: "2026-11-05 (air-xdc)",
    headline: "`air status` prints one line per top-level key in `.claude/air.json` that the \
               running binary does not read, naming the binary's version and build commit, and \
               `air status --json` carries them as `unknown_config_keys`. A fleet ran a build \
               thirteen days behind main against a key it had never heard of, and nothing \
               said so. Nothing is refused.",
    silent_break: false,
    action: "If the line names a key you set on purpose, upgrade the installed `air`; if it \
             names a typo, fix the key.",
}
