SurfaceChange {
    id: "exec-targets",
    since: "2026-11-04 (air-tfi)",
    headline: "A target in `\"targets\"` is an argv prefix: `\"exec\": [\"kubectl\", \"exec\", \
               \"-i\", \"<pod>\", \"-c\", \"<c>\", \"-n\", \"<ns>\", \"--\"]` with `\"path\"`. \
               Every step `air run` and `air record` take on the machine is the prefix then \
               `sh -c '<line>'`, and the push runs the prefix as git's `ext::` remote with \
               `-c protocol.ext.allow=always` on that one command. `\"kind\": \"ssh\"` with \
               `host`, `user` and `path` is unchanged and is the prefix `ssh user@host`.",
    silent_break: false,
    action: "Nothing for an ssh target. A machine reached by any other command gets an `exec` \
             list and a `path` in its session's entry; `air run --help` has the shape.",
}
