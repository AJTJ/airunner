SurfaceChange {
    id: "bd-server-checks-its-database-first",
    since: "2026-09-28 (air-df53)",
    headline: "`air bd-server up` checks for Air's database before asking whether anything \
               answers on the port. With the database missing and another process on the \
               port, it now fails naming the missing database instead of reporting the server \
               up.",
    silent_break: false,
    action: "",
}
