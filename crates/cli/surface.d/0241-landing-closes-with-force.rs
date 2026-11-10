SurfaceChange {
    id: "landing-closes-with-force",
    since: "2026-11-09 (air-52u)",
    headline: "`air land` closes a done bead with bd's `--force`: bd 1.3.0 refuses to close a \
               bead whose assignee is not the actor, and the landing closes as the lane what \
               a worker holds, so on bd 1.3.0 no landing closed a bead and every worker ran \
               `air done` twice. A close bd refuses even so is `done: bd refused to close`, \
               and the `main moved` notice says `done but refused`, apart from `done but \
               waiting`. `air handover` names a missing precheck whatever the green, as \
               `air done` does.",
    silent_break: false,
    action: "none; a bd-pinned bead now closes at the landing too, since `--force` overrides \
             bd's pin and gates.",
}
