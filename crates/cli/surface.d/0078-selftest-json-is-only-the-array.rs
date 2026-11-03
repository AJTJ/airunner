SurfaceChange {
    id: "selftest-json-is-only-the-array",
    since: "2026-09-06 (air-e21v)",
    headline: "`air selftest --json` emitted a stray line before the array, so it did not \
               parse — and `air selftest --prove`, which reads it, reported EVERY declared \
               mutation as broken. The evidence tool was dead by construction: 82 broken \
               before, 80 proven and 3 vacuous and 1 genuinely broken after. Cause was a \
               tee whose sinks were this process's stdout and stderr, right for `air record` \
               and wrong inside a probe whose parent emits JSON.",
    silent_break: true,
    action: "Re-run `air selftest --prove`: on an older binary its verdict was \
             uninformative rather than bad news, and anything of yours that skipped past a \
             leading non-JSON line can stop. The first byte is `[` again.",
}
