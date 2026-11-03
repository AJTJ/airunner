SurfaceChange {
    id: "enforce-default",
    since: "2026-08-22 (air-i59)",
    headline: "Worker launches set AIR_ENFORCE=1: the hand-over gate refuses, it no \
               longer advises.",
    silent_break: false,
    action: "A worker without a recorded green at HEAD is now denied the `bd` write \
             instead of warned. Make sure the repo's verify command is the one workers \
             actually run: `air record verify -- <cmd>`.",
}
