SurfaceChange {
    id: "target-holds-only-commits",
    since: "2026-11-06 (air-32u)",
    headline: "Before a command runs through a target, `air run` and `air record` now run \
               `git clean -qfd && git checkout -q -f --detach <sha>` there, so the target \
               checkout holds only the commit being run: an untracked file a previous command \
               wrote there is deleted and a tracked file it changed is overwritten. Ignored \
               files stay (no `-x`). Before, a file a command wrote and the session then \
               committed made the next checkout refuse (\"untracked working tree files would \
               be overwritten\") and the precheck or verify recorded nothing.",
    silent_break: true,
    action: "If a target checkout holds untracked files you want to keep across runs, add \
             them to that checkout's `.git/info/exclude` or `.gitignore`; anything untracked \
             and not ignored goes on the next run.",
}
