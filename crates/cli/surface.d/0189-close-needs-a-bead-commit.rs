SurfaceChange {
    id: "close-needs-a-bead-commit",
    since: "2026-09-28 (air-gcjp)",
    headline: "`air close` and `air handover` refuse a bead with no commit carrying its \
               `Bead:` trailer on the branch or on main (check `bead-commit`), whatever the \
               head's green. A green head no longer closes a bead nothing names.",
    silent_break: false,
    action: "A bead that produces no code still needs a commit: record the result in a \
             document and commit it with the trailer.",
}
