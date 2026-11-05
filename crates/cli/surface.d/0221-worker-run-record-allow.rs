SurfaceChange {
    id: "worker-run-record-allow",
    since: "2026-11-05 (air-nyi)",
    headline: "`air worker` passes two allow rules on its `--settings`, `Bash(air run *)` and \
               `Bash(air record *)` (and the pinned binary's forms when the repo is pinned), \
               so a worker's `air run` against a target resolves before auto mode's \
               classifier, which refused it as a remote write. The roles text tells workers \
               to run both bare. The coordinator's list is unchanged.",
    silent_break: false,
    action: "Relaunch workers to get the rules. A piped `air run` line still meets the \
             classifier.",
}
