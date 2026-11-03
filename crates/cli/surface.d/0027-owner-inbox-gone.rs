SurfaceChange {
    id: "owner-inbox-gone",
    since: "2026-09-05 (air-uef)",
    headline: "The owner inbox is gone: `air capture --for owner` is refused, `air inbox \
               --owner` and the `owner-decision-waiting` condition no longer exist. The \
               owner's queue is beads labelled `owner`, counted on the `ready:` line of \
               `air status`.",
    silent_break: true,
    action: "A rule or script that runs `air capture --for owner` now gets a refusal \
             (exit 2) naming the replacement; one that runs `air inbox --owner` gets a \
             clap error. Captures an older binary wrote for the owner are still there: \
             `air inbox` lists every open capture, so triage them on the next pass. The \
             coordinator triages EVERY capture into a bead or drops it with a reason, and \
             labels the bead `owner` (with its recommendation in the description) when the \
             decision is the owner's. `air://owner-queue` is gone from `air mcp`.",
}
