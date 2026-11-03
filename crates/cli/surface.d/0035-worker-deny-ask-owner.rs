SurfaceChange {
    id: "worker-deny-ask-owner",
    since: "2026-09-05 (air-bm3; owner ruling 2026-08-30)",
    headline: "Workers are denied `AskUserQuestion`, and the PreToolUse hook matcher \
               carries it so an attempt is an event line.",
    silent_break: false,
    action: "A worker that needs the owner runs `air capture \"<question>\"`; the \
             coordinator files it as a bead labelled `owner`, which `air status` counts \
             and `air claim` refuses to workers. Re-run `air install --write` to pick up \
             the matcher; workers launched by this binary carry the deny already. If a \
             rule of yours told workers to ask the owner directly, point it at the \
             capture.",
}
