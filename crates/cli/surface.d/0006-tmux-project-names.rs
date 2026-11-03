SurfaceChange {
    id: "tmux-project-names",
    since: "2026-08-22 (air-5lg)",
    headline: "tmux sessions are `<project>-<worker>`, not `<worker>`.",
    silent_break: false,
    action: "Update any `tmux attach -t <worker>` in the repo's docs or scripts.",
}
