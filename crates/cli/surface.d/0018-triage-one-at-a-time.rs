SurfaceChange {
    id: "triage-one-at-a-time",
    since: "2026-08-29 (air-zlq)",
    headline: "`air triage` takes exactly ONE capture id. Batch mode is deleted.",
    silent_break: false,
    action: "A caller passing several ids now fails loudly instead of silently handling \
             about two of them under one budget. Loop instead.",
}
