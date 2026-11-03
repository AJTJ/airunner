SurfaceChange {
    id: "sendmessage-measured",
    since: "2026-08-29 (air-q07)",
    headline: "The PreToolUse matcher includes `SendMessage`: agent-to-agent messages are \
               counted (sender, recipient, byte count -- never content).",
    silent_break: true,
    action: "Re-run `air install --write`. Until you do, `air audit`'s traffic block reads \
             zero for this repo, and a zero there is indistinguishable from silence -- \
             which is the bug it replaced: one worker sent ~46,900 characters in a day and \
             the event log contained none of it. Nothing is gated; there is no threshold \
             and no refusal.",
}
