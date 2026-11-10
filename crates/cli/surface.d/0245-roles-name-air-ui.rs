SurfaceChange {
    id: "roles-name-air-ui",
    since: "2026-11-10 (air-o6x.5)",
    headline: "The coordinator's section of `.air/roles.md` says that `air ui` is a read-only \
               page the owner may have open, reading the ledger, the transcripts and bd and \
               writing nothing, so it changes nothing the coordinator does. `.air/docs/` \
               (the design doc and the adopting guide) describe the command.",
    silent_break: false,
    action: "`air install --write` refreshes `.air/roles.md` and `.air/docs/`; restart the \
             coordinator to read it.",
}
