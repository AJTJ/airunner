# Surface notices

One file per notice, `NNNN-<id>.rs`, instead of an entry appended to one array in
`src/cmd/install.rs`. Every bead that changed the surface appended at the same end of that array,
so two beads in a round conflicted at the same line: four lane drops and one bad hand resolution
in one lane session on 2026-11-03 (air-38q). A fragment is a file no other bead touches; the
same fix as `changelog.d/` (air-be6d), run at build time because the surface is compiled in.

A fragment is one Rust expression, exactly the entry it replaces, with no trailing comma:

    SurfaceChange {
        id: "launch-no-startup-prompts",
        since: "2026-11-02 (air-z41)",
        headline: "…",
        silent_break: false,
        action: "…",
    }

`build.rs` folds every `*.rs` here, in filename order, into `install::SURFACE`, which keeps its
name and type; `air install`, `air release-check` and the version line read the array as before.
The ordinal is the position: take the number after the highest one present. Two beads in one
round that pick the same number do not conflict, since the files differ, and sort by id after it.
The name after the ordinal is the id, and a test holds the two equal. A missing directory or an
empty one fails the build naming the directory; a malformed fragment is a rustc error naming the
fragment, since each is included by its own path. Never edit an id, and never remove a fragment:
the release row's count (`install::RELEASES`) only goes up.
