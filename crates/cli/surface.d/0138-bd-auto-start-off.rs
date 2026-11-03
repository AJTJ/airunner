SurfaceChange {
    id: "bd-auto-start-off",
    since: "2026-09-26 (owner ruling)",
    headline: "`air init --write` turns off bd's own Dolt server in a new server-mode project \
               (`dolt.auto-start: false` in `.beads/config.yaml`).",
    silent_break: false,
    action: "In an existing server-mode project, add `dolt.auto-start: false` to \
             `.beads/config.yaml`. Otherwise, when Air's server is down, bd starts its own \
             server with an empty database on the same port.",
}
