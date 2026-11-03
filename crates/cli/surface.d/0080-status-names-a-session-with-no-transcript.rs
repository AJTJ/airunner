SurfaceChange {
    id: "status-names-a-session-with-no-transcript",
    since: "2026-09-06 (air-3jv5)",
    headline: "A hook invocation from a demonstration or a fixture writes a session row \
               indistinguishable from a real session, and it fired channel conditions. Air \
               REPORTS rather than refuses: the row and its event line are still written, \
               `air status` names it NO TRANSCRIPT, and it is never announced as a worker \
               joining. Refusing was rejected deliberately — it needs Air to tell a real \
               session id from a fake one, and losing a live worker from `air status` the \
               day a harness omits the field is strictly worse than showing a synthetic \
               row.",
    silent_break: false,
    action: "Nothing to run. If `air status` names a session NO TRANSCRIPT, it is a row no \
             session is behind; the SQL to list them is in the line itself.",
}
