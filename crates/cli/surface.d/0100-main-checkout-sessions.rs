SurfaceChange {
    id: "main-checkout-sessions",
    since: "2026-09-25 (air-jc2p.3)",
    headline: "`air status` prints `warning: <role> session <name> (pid N) runs in the main \
               checkout` for each launched session whose `claude` process has its working \
               directory there (`--json` `main_checkout_sessions`). The owner's own shell \
               (no AIR_ROLE) is exempt, and nothing is refused.",
    silent_break: false,
    action: "Relaunch such a session through `air coordinator`, `air lane` or `air worker`, \
             each of which runs in its own worktree.",
}
