SurfaceChange {
    id: "adopters-declared",
    since: "2026-09-06 (air-jsz)",
    headline: "`air adopter-check` no longer skips silently when it has no names. \
               `.claude/air.json` gains `\"adopters\"`: declared true with no \
               `private/adopters.md` is now a REFUSAL naming the file to write; \
               undeclared with no list still skips, which is the clone-with-no-adopter \
               case. The list is read from the MAIN checkout, beside the declaration, so \
               a worktree's copy cannot disagree with it. `air init` writes \
               `\"adopters\": false`.",
    silent_break: true,
    action: "If your repo quotes an adopter, set `\"adopters\": true` AND write \
             `private/adopters.md` in the main checkout BEFORE upgrading, or your next \
             verify goes red. If it quotes nobody, do nothing: the default is false and \
             the behaviour is unchanged. Why this changed: the check ran for a whole \
             round here having never once been given a list — the file was absent \
             everywhere and every green verify printed `Skipped`, so the one mechanism \
             guarding the no-adopter-content rule would have passed over any leak. A \
             count of zero firings meant nothing, because the input never arrived. Check \
             your own verify output for that line before assuming yours has ever run.",
}
