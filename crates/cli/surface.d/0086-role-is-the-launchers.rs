SurfaceChange {
    id: "role-is-the-launchers",
    since: "2026-09-14 (owner ruling)",
    headline: "Who may run `air land`, `air close`, `air release --worker`, and claim an \
               `owner` bead now comes from `AIR_ROLE`, never from the directory the command \
               runs in. The hook's fence and Stop advice follow the same rule, and `air land` \
               always moves main in the main checkout wherever it is run from.",
    silent_break: true,
    action: "Sessions started with `air worker` and `air coordinator` need nothing. A worker \
             started any other way must set AIR_ROLE=worker, or it is treated as the owner \
             and may land. A shell with no AIR_ROLE is the owner.",
}
