SurfaceChange {
    id: "batch-ready-behind-main",
    since: "2026-09-25 (owner ruling, from an adopter's fleet protocol)",
    headline: "A branch behind `main` is now batch-ready: `air status` no longer drops it \
               with `behind-main`. The lane merges main forward at the cut, so a landing no \
               longer takes every waiting branch out of the queue until its worker re-merges.",
    silent_break: false,
    action: "Workers under a lane stop merging main just to re-enter the queue. A lane that \
             cuts from the listed shas already merges main first; one that assumed every \
             member contained main must now drop, and name, a member that conflicts with it.",
}
