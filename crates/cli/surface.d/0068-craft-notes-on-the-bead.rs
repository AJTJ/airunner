SurfaceChange {
    id: "craft-notes-on-the-bead",
    since: "2026-09-07 (air-u3l7)",
    headline: "`roles.md`'s Coordinator section now names the route: put craft notes on the \
               bead with `bd comment <id> --file <notes>`, not in the message that names \
               the bead at a worker. The rule that naming a bead reserves nothing is \
               unchanged; what is added is what to do instead.",
    silent_break: false,
    action: "Re-run `air install --write` to refresh `.air/roles.md`. Worth reading if your \
             coordinator dispatches by message: the old line stated only the CONSEQUENCE \
             (your reservation will not hold), and between 2026-09-05 and 2026-09-07 it \
             was read, agreed with and worked around five times — two beads lost at one \
             repo, two dispatched twice at another, and once the Stop hook itself told an \
             idle worker to claim a bead already spoken for, from correct inputs. No new \
             state, nothing refuses, and the hook is unchanged: a ready bead is available \
             again once nothing is reserved outside the bead.",
}
