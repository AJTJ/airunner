SurfaceChange {
    id: "adopter-check",
    since: "2026-09-06 (air-bpj)",
    headline: "`air adopter-check` refuses a tracked line naming an adopter. Names are \
               read from `private/adopters.md` (one `name: <x>` line each), never from \
               the binary, and the check SKIPS when that file is absent. Air's own tracked \
               text now says \"an adopter\": an incident keeps its date, its count and its \
               `air-` bead, and anything that quotes an adopter's files lives in an \
               ignored `private/`.",
    silent_break: false,
    action: "Nothing to run, and nothing changes for a repo that does not use it. If YOUR \
             repo is quoted in someone else's, the same shape works: `private/` in \
             `.gitignore`, the names in `private/adopters.md`, and `air adopter-check` in \
             your verify. Air will not tell you a name is missing — a check whose list is \
             public would publish what it exists to hide.",
}
