SurfaceChange {
    id: "reclaim-replaces-release-worker",
    since: "2026-09-26 (owner ruling: no --force)",
    headline: "The bd 1.3.0 fix is done. `air release` passes the worker's own actor to \
               bd, so a worker can give back its bead on 1.3.0. `air release --worker` is \
               gone: the coordinator takes back a gone worker's bead with `air reclaim \
               <id> --worker <name> --reason <r>`, which runs `bd reclaim --id` and never \
               `--force`. bd lets go once the claim's lease has run out, five minutes \
               after the claim; before that `air reclaim` names the expiry and exits 1.",
    silent_break: false,
    action: "Replace any `air release <id> --worker <name>` in your own scripts or prose \
             with `air reclaim <id> --worker <name> --reason <r>`.",
}
