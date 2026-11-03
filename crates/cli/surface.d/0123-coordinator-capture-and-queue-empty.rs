SurfaceChange {
    id: "coordinator-capture-and-queue-empty",
    since: "2026-09-26 (air-1vri.5)",
    headline: "The coordinator hears \"capture from <worker>: <first line>\" once when a \
               worker runs `air capture`, and \"the ready queue is empty: <n> worker(s) \
               idle; epics with no open child: <ids or none>\" once per emptying while a \
               worker holds no claim (event lines `fanout / capture` and `fanout / \
               queue-empty`). `.air/roles.md` says what to do with each.",
    silent_break: false,
    action: "If the repo's own prose tells the coordinator to poll `air inbox` or the ready \
             queue on a timer for these, it can rely on the notices instead.",
}
