// ---- 2026-08-29 ----------------------------------------------------------------
// Derived from the day's 50 landings on main, not from a list written afterwards
// (air-njb). The filter is the acceptance's: a change that alters what a target repo
// SEES, DOES, or MAY DO. Seventeen of the fifty touch `install.rs`, `launch.rs`,
// `schema.rs`, `main.rs` or `roles.md`; the rest are internal and are not here.
SurfaceChange {
    // First, because it is about the installer itself and it changes what every other
    // notice in this list is worth.
    id: "install-refreshes-matcher",
    since: "2026-08-29 (air-q07)",
    headline: "`air install --write` now UPDATES a stale hook matcher. Until today it \
               skipped any event that already had an `air hook` entry, so a re-run never \
               changed a matcher and your repo still has the one it was FIRST installed \
               with.",
    silent_break: true,
    action: "Check, do not assume: `grep -o '\"matcher\": \"[^\"]*\"' .claude/settings.json`. \
             If a matcher there is narrower than the one this version installs, it has \
             been narrower since your first install and every widening since was a \
             no-op that reported success. Re-run `air install --write` and diff the file. \
             The trap underneath - that the installer is only as new as the binary, so an \
             old `air` cannot write a new matcher however many times you run it - is now \
             CHECKED rather than left to you: since air-w9d `air install` refuses to write \
             when this repo was last installed by a newer air, and names both builds. You \
             no longer have to compare versions by eye.",
}
