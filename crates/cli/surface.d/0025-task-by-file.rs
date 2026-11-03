SurfaceChange {
    id: "task-by-file",
    since: "2026-09-05 (air-er0)",
    headline: "`air worker --task` writes the task to `<main>/.air/tasks/<name>.md`; the \
               process's command line carries a fixed sentence naming that path, never \
               the task text.",
    silent_break: false,
    action: "Nothing to change in how workers are launched. What changes is what `ps` \
             shows: a worker's argv no longer contains its prompt, so a `pkill -f` over \
             ordinary command text (`air record verify`, `git status`) stops matching \
             every peer. The adopter lost seven workers to that on 2026-08-30. Anything \
             that reads a worker's task out of `ps` or the tmux command line reads the \
             file instead; anything that cleans `.air/` leaves `tasks/` alone.",
}
