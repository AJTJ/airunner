SurfaceChange {
    id: "selftest-prove",
    since: "2026-08-29 (air-682)",
    headline: "`air selftest --prove` runs each probe's declared mutation. It EDITS \
               TRACKED FILES and rebuilds, and refuses to start on a dirty tree.",
    silent_break: false,
    action: "Do not put it in CI beside `air selftest`, and do not run it over \
             uncommitted work. Plain `air selftest` is unchanged and still what `make \
             verify` should call.",
}
