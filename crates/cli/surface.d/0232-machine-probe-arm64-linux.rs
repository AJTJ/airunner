SurfaceChange {
    id: "machine-probe-arm64-linux",
    since: "2026-11-06 (air-c14)",
    headline: "The machine probe behind `air run` and `air record` names the CPU on arm64 \
               Linux: `model name` from `/proc/cpuinfo`, then `Model name` from `lscpu`, then \
               the CPU implementer and part joined, then macOS's `sysctl`. Before, an arm64 \
               Linux target stored an empty `cpu` column, since its cpuinfo has no `model name` \
               line.",
    silent_break: false,
    action: "Nothing.",
}
