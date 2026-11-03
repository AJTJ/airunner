SurfaceChange {
    id: "budgets-side-by-side",
    since: "2026-09-28 (air-onqi)",
    headline: "`air audit` prints each budget's recorded p50 and max beside the budget and \
               `max x 10`, with the measurement or ceiling the number came from; a budget \
               reached reads `over budget N time(s)` instead of `N hit(s)`.",
    silent_break: true,
    action: "A script that matched `hit(s)` in `air audit` output matches nothing now; read \
             `over budget` or the `hits` field of `--json`.",
}
