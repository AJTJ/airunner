SurfaceChange {
    id: "install-removes-retired-skills",
    since: "2026-09-25 (an adopter's re-audit, air-rr98)",
    headline: "`air install --write` removes each `air-*` skill directory Air once installed \
               and no longer ships (today `air-phase-transitions`) and names each one. A \
               skill whose name does not start with `air-` is never touched. The installed \
               skills and `.air/roles.md` no longer name a skill Air does not install: \
               roles.md now says `air-decomposition`.",
    silent_break: false,
    action: "Nothing to run beyond `air install --write`, then commit the removal.",
}
