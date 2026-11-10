SurfaceChange {
    id: "coordinator-prompt",
    since: "2026-11-05 (air-fkb)",
    headline: "`air coordinator -- \"<text>\"` starts the coordinator on `<text>` as its first \
               prompt, placed first in claude's argv like a worker's task prompt. Before, a \
               positional after `--` landed among the `--disallowed-tools` values and was read \
               as one more deny rule, and the session started on an empty prompt. Pass-through \
               flags after the text are placed as before.",
    silent_break: false,
    action: "Nothing. If a script of yours passed a positional and relied on it being \
             ignored, drop it.",
}
