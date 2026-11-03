SurfaceChange {
    id: "bd-server-checks-its-data",
    since: "2026-09-26 (owner ruling)",
    headline: "`air bd-server up` and `air bd-server status` count the server as up only when the \
               process on the port serves this project's `.air/dolt/data`. If another process \
               answers, they name it and start nothing.",
    silent_break: false,
    action: "If told the port is held by another process, stop that process and run \
             `air bd-server up` again.",
}
