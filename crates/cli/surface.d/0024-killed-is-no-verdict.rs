SurfaceChange {
    id: "killed-is-no-verdict",
    since: "2026-09-05 (air-ppm)",
    headline: "A verify that exits 143 or 137 is recorded as KILLED, not red: it is no \
               verdict for that sha. A child that died by signal is recorded as 128+signal \
               rather than -1.",
    silent_break: true,
    action: "A run your harness killed at its timeout no longer reads as a red at HEAD, \
             no longer makes a green/kill pair read as flaky, and no longer tells the \
             worker \"the repo's test is the bug\". The exit code is still recorded and \
             `air record` still mirrors it, so a script gated on its exit sees what it \
             saw. Only 137 and 143 are read this way: when `make`'s CHILD is the process \
             signalled, make exits 2 and Air records a red, because 2 is a real failure's \
             code too and Air does not parse make's \"Terminated\" line to tell them \
             apart. A wrapper that knows a stage was signalled should exit 143 to say so \
             (the adopter's run-logged.sh does).",
}
