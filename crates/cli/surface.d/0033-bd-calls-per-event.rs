SurfaceChange {
    id: "bd-calls-per-event",
    since: "2026-09-05 (air-bp0)",
    headline: "`bd_calls`/`bd_ms` on an event line are that event's own cost. Lines from \
               `air mcp` (`status.attention`, `channel.push`) used to carry the server's \
               running lifetime total, restamped on every tick. `air status` looks up \
               every reconciled claim in one `bd show`, and SubagentStop no longer runs \
               the Stop nudge (no bd call, no idle mark).",
    silent_break: true,
    action: "Any number derived by summing `bd_calls` over the event log is wrong for \
             every day before this: the adopter's 2026-08-30 summed to 570,989 while the \
             largest total any process reached was 1,661, and one-shot commands cost \
             1 to 4. Re-derive from lines written by this version; for older days, take \
             the max `bd_calls` per command as that process's lifetime total. The bead's \
             own 14.8-per-command figure was this artefact.",
}
