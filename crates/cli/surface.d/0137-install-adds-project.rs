SurfaceChange {
    id: "install-adds-project",
    since: "2026-09-26 (owner ruling)",
    headline: "`air install --write` adds `\"project\"` to a `.claude/air.json` that has none, set to \
               the main checkout's directory name (what `air init` writes), and says so. It \
               never changes a value that is there. Without it, session names, \
               `<project>-dolt` included, used the bead prefix.",
    silent_break: false,
    action: "If your sessions were named after the bead prefix, their names change to the directory \
             name after this install; set `\"project\"` yourself first to keep another name.",
}
