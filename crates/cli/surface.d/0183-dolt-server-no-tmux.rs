SurfaceChange {
    id: "dolt-server-no-tmux",
    since: "2026-09-28 (air-8iu7)",
    headline: "Air starts the project's Dolt server as a background process, pid in \
               `.air/dolt/server.pid`, output in `.air/dolt/server.log`, and no longer in a \
               tmux session `<project>-dolt`; tmux is no longer needed for it. `air bd-server \
               status` names the pid. A server already running in the old tmux session keeps \
               answering and is left alone.",
    silent_break: false,
    action: "Nothing required. To move a running server off tmux, stop the `<project>-dolt` \
             session and run `air bd-server up`.",
}
