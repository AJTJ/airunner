SurfaceChange {
    id: "empty-cut-exits-1",
    since: "2026-09-28 (air-k81s)",
    headline: "`air batch cut` with no members prints `held: <why>` for each branch it left \
               out and exits 1, so a chained `air record verify` does not run on bare main. \
               A dry run still exits 0.",
    silent_break: true,
    action: "A lane script that treats any non-zero exit from `air batch cut` as an error \
             now sees one when nothing is batch-ready; read it as \"nothing to verify\".",
}
