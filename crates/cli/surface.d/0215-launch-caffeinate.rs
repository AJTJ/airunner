SurfaceChange {
    id: "launch-caffeinate",
    since: "2026-11-03 (air-1ye)",
    headline: "On a Mac, `air worker`, `air lane`, `air coordinator` and `air fleet up` run the \
               session as the child of `caffeinate -i`, so the machine does not idle-sleep \
               while a session works and may once the fleet is idle; the assertion ends with \
               the session. `air status` prints `idle sleep: <n> min` when `pmset -g` reports \
               under ten minutes. Elsewhere nothing changes.",
    silent_break: false,
    action: "None. `AIR_CAFFEINATE_BIN=` (empty) launches without the wrapper.",
}
