SurfaceChange {
    id: "version-says-the-build",
    since: "2026-09-06 (air-dwq5)",
    headline: "`air --version` now prints the commit the binary was built from and its \
               surface version, `air --version --json` emits JSON instead of the same \
               plain string, and `air doctor` and `air status --json` carry the same \
               object. Air embedded the build, stored it in `installed.json` and read it \
               for the install-lag check, and told nobody — so one crate version covered a \
               whole round of behaviour changes and two binaries with different behaviour \
               reported the same string.",
    silent_break: true,
    action: "If anything of yours parses `air --version --json`, it was getting `air \
             0.2.19` and now gets `{\"name\":\"air\",\"version\":…,\"built_from\":…,\
             \"surface_version\":…}`; the plain `air --version` gained a suffix on the \
             same line. `air status --json` gains a third top-level key, `air`, beside \
             `snapshot` and `attention`. Attribute a log line or a session to a build with \
             `built_from`, not with the version: lanes cut no release rows mid-round, so \
             the version does not move when behaviour does.",
}
