SurfaceChange {
    id: "poll-runs-no-lsof",
    since: "2026-09-28 (air-czbg)",
    headline: "The channel poll no longer runs `lsof`. Only `air status` run by a person and \
               `air land` read processes, and the poll uses their last reading while it is \
               under 30 s old. Otherwise a tree's readers are unknown, which answers \"not \
               busy\": `idle-with-claim` and `idle-without-claim` can then fire for a worker \
               running an unrecorded check, as they did before readers existed. \
               `AIR_TREE_READERS=off` still turns the reading off entirely.",
    silent_break: false,
    action: "",
}
