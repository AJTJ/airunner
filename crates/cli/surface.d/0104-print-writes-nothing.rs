SurfaceChange {
    id: "print-writes-nothing",
    since: "2026-09-25 (an adopter's re-audit, air-rr98)",
    headline: "`--print` on `air worker`, `air lane` and `air coordinator` writes nothing. \
               It used to rewrite `.air/roles.md` (and `.air/coordinator.md`), and with \
               `--task` write `.air/tasks/<name>.md`; the printed line now names a task \
               file that does not exist yet.",
    silent_break: false,
    action: "Refresh `.air/roles.md` with `air install --write`, not with a printed launch.",
}
