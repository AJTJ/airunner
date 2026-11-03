SurfaceChange {
    id: "stopped-sessions",
    since: "2026-09-06 (air-1n3)",
    headline: "`air install --write` now adds `air hook` on `Notification` and \
               `StopFailure`, and a session the harness stops is recorded on its row \
               (`stopped_at`, `stopped_kind`, `stopped_text`, schema v20) and printed by \
               `air status` as `STOPPED at <t>` with the reason. Nothing is refused, \
               woken or relaunched. The SessionStart hook also says one sentence, once \
               per session, telling it to create a recovery wake.",
    silent_break: false,
    action: "Run `air install --write` to get the two new hook entries; without them \
             nothing records a stop. Then READ THE KIND before acting on one: \
             `quota_auto_resume_fired` means the harness is bringing that session back \
             and typing at it CANCELS the recovery, while `quota_auto_resume_stale`, \
             `quota_auto_resume_disabled` and `stop_failure` mean nothing is coming. If \
             your fleet has a rule that nudges a silent worker, narrow it to the kind: on \
             2026-09-06 five of seven sessions on one machine were recovering on their \
             own while two were not, and silence read identically for both. Requires a \
             harness that sends these events; on one that does not, the columns stay \
             NULL and nothing changes.",
}
