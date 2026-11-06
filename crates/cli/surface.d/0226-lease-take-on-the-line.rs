SurfaceChange {
    id: "lease-take-on-the-line",
    since: "2026-11-06 (air-643)",
    headline: "The lease check counts `air lease take <resource> --reason \"...\" && <cmd>` as \
               holding `<resource>` for `<cmd>`, with any number of `&&`-joined commands \
               between: a refused take stops the chain before `<cmd>` runs. Before, the whole \
               line was refused and the advice said to take the lease in its own call, which \
               cost one retry per worker in three live trials. A `;`, `||`, `|` or newline \
               between the take and the command, or a take for another resource, is still \
               refused.",
    silent_break: false,
    action: "Nothing. The two-call form still works.",
}
