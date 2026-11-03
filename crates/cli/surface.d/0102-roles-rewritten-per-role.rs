SurfaceChange {
    id: "roles-rewritten-per-role",
    since: "2026-09-25 (owner, air-igcz)",
    headline: "`.air/roles.md` is rewritten as facts and refusals per role (every role, \
               worker, verification lane, coordinator), at about 60% of its old length. \
               Incident histories, dates, bead ids and removal conditions moved out \
               (removal conditions are in `air audit`). Corrected: a landed bead whose \
               acceptance clause names a file the merge did not change is a lookup that did \
               not answer, not a wrong close; the lease gate refuses the lane as well as \
               workers; journals and opt-in digests live under the main checkout's `.air/`. \
               `docs/rules/adopting-air.md` is cut to install, the `.claude/air.json` keys \
               and upgrading.",
    silent_break: false,
    action: "Nothing to run; `air install --write` refreshes `.air/roles.md`. Restart \
             sessions to give them the new text. If the repo's CLAUDE.md quotes a roles.md \
             sentence, check it still appears.",
}
