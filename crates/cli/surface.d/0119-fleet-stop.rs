SurfaceChange {
    id: "fleet-stop",
    since: "2026-09-26 (air-1vri.1)",
    headline: "`air fleet stop [--reason <why>]` and `air fleet resume` (coordinator and \
               owner only; a worker or the lane is refused) stop and resume all work with \
               one command. Every other session is told through its channel. While \
               stopped, `air claim`, `air batch cut` and `air land` refuse naming the stop \
               and who set it, the ready fan-out and the Stop nudge are silent, and `air \
               status` leads with `FLEET STOPPED`. Sessions are not killed, and a verify \
               already running finishes and is recorded. New ledger table `fleet_stop`. \
               `.air/roles.md` says what a stop means for each role.",
    silent_break: false,
    action: "",
}
