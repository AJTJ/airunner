SurfaceChange {
    id: "pin-delegation",
    since: "2026-09-25 (air-qyrm)",
    headline: "In a pinned repo, an `air` that is not the pin hands every command to the pin \
               (`<main checkout>/.air/bin/air`) before running it, with the same arguments, \
               so a bare `air` runs the pin whatever order a shell put PATH in; \
               `air --version` there prints the pin's version. `air install` is the one \
               command that runs where it was found, so re-pinning with a newer build works. \
               Each hand-over writes one `pin / delegated` event line.",
    silent_break: false,
    action: "Only if the `air` on PATH is older than this release: install this one, or \
             bare `air` in a pinned repo still runs the PATH binary.",
}
