SurfaceChange {
    id: "selftest-git-identity",
    since: "2026-11-04 (air-3sh)",
    headline: "`air selftest` passes its base_branch probe on a host with no global git config: \
               every `air` a probe spawns carries the git identity the probe's fixtures set. \
               Before, the probe's `air land` ran `git commit-tree` with no author and was red \
               on such a host (exit 128), the first Linux verify.",
    silent_break: false,
    action: "Nothing.",
}
