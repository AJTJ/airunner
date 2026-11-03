SurfaceChange {
    id: "no-hidden-limits",
    since: "2026-09-28 (air-06k7)",
    headline: "Seven limits that changed an answer silently are removed. The close gate \
               checks every green recorded after the bead's oldest commit (it read the \
               newest 12, and a refusal said \"12\"); `bd ready` and `bd list --status` \
               pass `-n 0` (bd's defaults capped them at 100 and 50); the model column reads \
               the transcript to its first model line (launched sessions showed `[?]`); the \
               loop times, `air init`'s tree walk and prose bead ids are uncapped. Red run \
               logs in `.air/logs/` are kept, never pruned to 20, and `air gc` does not \
               collect them: each red run adds up to 64 KiB.",
    silent_break: false,
    action: "",
}
