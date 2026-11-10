SurfaceChange {
    id: "repo-allow-keys",
    since: "2026-11-05 (air-xji)",
    headline: "`.claude/air.json` reads `worker_allow` and `coordinator_allow`, arrays of \
               permission patterns the launcher puts on that role's `--settings` allow list \
               (the lane takes `worker_allow`), the way `worker_deny` and `coordinator_deny` \
               already reach its deny list. They are for the adopter's own machine commands, \
               a pod's `kubectl` or a VM's `ssh`; Air names no provider. The deny list wins \
               where both name a command.",
    silent_break: false,
    action: "Set the keys for the commands the classifier refuses, then relaunch the sessions \
             that need them. A rule matches the bare line only.",
}
