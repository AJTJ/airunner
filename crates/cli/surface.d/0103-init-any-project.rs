SurfaceChange {
    id: "init-any-project",
    since: "2026-09-26 (owner, air-gn5o)",
    headline: "`air record` no longer flags a green `suspicious` for being under 2 s. It \
               flags a green that printed nothing, or that ran in under a fifth of the time \
               this worker's last green of the same command took. `air init` in a new repo \
               proposes the verify command it finds (a Makefile's `verify` or `test`, \
               `cargo test`, `npm test`) and writes the failing `make verify` placeholder \
               only when it finds none; lets bd name the bead prefix after the directory; \
               writes `\"project\"` as the directory name and `\"metis\"` as whether Metis \
               is installed; and no longer writes `\"adopters\"` or runs `bd config set \
               status.custom awaiting_review`.",
    silent_break: false,
    action: "Nothing required. A repo that read `suspicious` as \"under 2 s\" now sees it \
             only on a drop against its own history.",
}
