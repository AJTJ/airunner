SurfaceChange {
    id: "epic-count-drops-the-instruction",
    since: "2026-09-07 (air-3vkg)",
    headline: "The `ready:` line's epic count no longer says \"to decompose\": it reads \
               `N epic(s), not claimable`. That count is every epic in the ready set, \
               decomposed or not, so the old phrase told a coordinator to decompose epics \
               that were already at their frontier — one adopter audited all six of theirs \
               on the strength of it and none needed anything. The instruction now sits \
               only on the line that can tell, `epic ready to decompose: <id> (0 open \
               children, N closed)`, which is computed from the children and is the \
               number the phrase always meant. This SUPERSEDES the wording quoted in the \
               `ready-split-epics` notice above; that notice is left as written, since it \
               was true when it shipped.",
    silent_break: true,
    action: "Anything of yours that greps `air status` for `epic(s) to decompose` finds \
             nothing now and will not error. The count is unchanged in value and in \
             source; only the words moved. Read the per-epic line for the action, and \
             note it is absent on a cached status — the same line ends `(cached; bd not \
             called this tick)` there, because the children calls it needs were not made.",
}
