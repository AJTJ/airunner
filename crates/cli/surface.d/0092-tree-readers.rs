SurfaceChange {
    id: "tree-readers",
    since: "2026-09-25 (owner)",
    headline: "`air status` names every process that is not a Claude Code session with its \
               working directory in a fleet tree (`readers: <tree>: N (<exe> <age>, ...)`, \
               `--json` `tree_readers`), `idle-without-claim` no longer fires for a worker \
               whose tree has one, and `air land` warns, without refusing, about any in the \
               main checkout before moving main.",
    silent_break: false,
    action: "A repo-side tree-readers check (processes by cwd before landing or before \
             calling a worker idle) can be retired in favour of `air status`. Nothing is \
             required.",
}
