SurfaceChange {
    id: "bd-server",
    since: "2026-09-26 (owner ruling)",
    headline: "`air init --write` on a new project sets bd up in server mode: it starts a \
               Dolt server on a free port in 3400..3900 (data in `.air/dolt/`, tmux \
               session `<project>-dolt`), runs `bd init --server`, and keeps the port in \
               `.beads/dolt-server.port`. For a project in server mode, `air bd-server up`, \
               `air fleet up` and the launchers start that server when it does not answer, \
               `air doctor` and `air status` print a `bd:` line with the mode, and the \
               coordinator's channel restarts a server that stops and tells the \
               coordinator once. A project whose bd is embedded is not changed.",
    silent_break: false,
    action: "",
}
