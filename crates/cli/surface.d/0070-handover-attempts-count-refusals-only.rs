SurfaceChange {
    id: "handover-attempts-count-refusals-only",
    since: "2026-09-06 (air-zqmi)",
    headline: "`handover-not-green` counted SUCCESSES. The gate stamped an attempt on \
               every hand-over command it saw, and that condition reads the counter — so a \
               worker whose closes all passed was reported to the whole fleet as having \
               failed. Only a hand-over that did NOT go through is an attempt now, and a \
               pass CLEARS the counter, because otherwise one early refusal keeps firing \
               after a clean close. `air handover`, the diagnostic, never stamps at all \
               (air-eiv): running the query used to raise the alarm it was meant to \
               diagnose.",
    silent_break: true,
    action: "If you discounted this condition, or told your workers to, STOP discounting \
             it — its firings mean what they say now. On an older binary it fires for \
             refusals that never happened, and there are TWO tells: a worker whose closes \
             all passed carrying `handovers N` on `air status`, and a worker that runs \
             `air handover` BY HAND, which stamped once per run before air-eiv — an \
             adopter found the second at 77 runs in one worktree. To tell a diagnostic \
             stamp from a genuinely refused close, join the claim's `last_handover_at` \
             against your events stream for a `bd close` at that time: a real attempt has \
             one beside it and a diagnostic stamp does not. Nothing to run; the counter \
             clears on the next passing hand-over of that bead.",
}
