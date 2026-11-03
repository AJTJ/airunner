SurfaceChange {
    id: "close-reason-file",
    since: "2026-09-07 (air-lyjr)",
    headline: "`air close <id>... --reason-file <path>` records the file as the reason, \
               untruncated, on every id named — the same one reason `--reason` already \
               applied to all of them. `--reason` stays; exactly one of the two, and both \
               or neither is refused naming both. A path that cannot be read is an error, \
               never an empty reason.",
    silent_break: false,
    action: "Nothing to re-run. This is air-45pw one command over, and it bites harder \
             here: if your repo asks a close to carry proof — a command and its output — \
             then the command demanding the longest argument was the one refusing it. The \
             harness classifies a long `--reason \"...\"` by the shape of the command line \
             and refuses it, and the obvious next move is to shorten the proof. Point your \
             rules at `--reason-file` for anything past a line or two. Observed on `bd \
             close` at ~2,500 characters, where bd's own `--reason-file` took the identical \
             text on the next attempt.",
}
