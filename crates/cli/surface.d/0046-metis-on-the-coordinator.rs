SurfaceChange {
    id: "metis-on-the-coordinator",
    since: "2026-09-06 (air-g5o)",
    headline: "`air coordinator` attaches Metis when `.claude/air.json` says \
               `\"metis\": true`, which `air init` now writes by default: `--mcp-config` \
               with metis's own server declaration, and `--plugin-dir` when \
               `metis_plugin_dir` names a directory that exists. Workers never get it. \
               The coordinator's appended prose gains a paragraph stating that Metis \
               holds vision and initiatives while tasks stay beads and decisions stay in \
               `docs/`, and `air status` prints how many beads declare no \
               `initiative: <CODE>` line.",
    silent_break: false,
    action: "Nothing is refused: the count is a count, and a missing `metis` binary is one \
             printed line, not a failed launch. Set `\"metis\": false` in \
             `.claude/air.json` if your repo plans elsewhere. To get the plugin as well as \
             the tools, set `metis_plugin_dir` to the `plugins/metis` directory of a metis \
             checkout; Air will not guess that path, and a `--plugin-dir` pointing at \
             nothing loads nothing silently.",
}
