SurfaceChange {
    id: "record-labels-removed",
    since: "2026-09-28 (air-qlx3)",
    headline: "`air record` no longer labels a run `suspicious` or `flaky-at-head`; it prints \
               `at this commit: N green / M red` when a commit has more than one run. The \
               close refusal for such a head states the same counts and no longer says the \
               test is flaky.",
    silent_break: true,
    action: "A script that matched `suspicious` or `flaky` in `air record` output matches \
             nothing now; read the exit and the counts.",
}
