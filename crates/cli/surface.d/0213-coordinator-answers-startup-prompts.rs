SurfaceChange {
    id: "coordinator-answers-startup-prompts",
    since: "2026-11-03 (air-d73)",
    headline: "`.air/roles.md`: after a launch the coordinator reads each new pane with \
               `tmux capture-pane`; if a start-up prompt is waiting it asks the owner once \
               and, on a yes, answers it with `tmux send-keys`. The only exception to \
               \"do not type into panes\"; never the folder-trust dialog.",
    silent_break: false,
    action: "",
}
