SurfaceChange {
    id: "env-on-the-process",
    since: "2026-09-05 (air-9dg)",
    headline: "AIR_ENFORCE, AIR_ROLE, AIR_PROJECT and BEADS_ACTOR are set on the spawned \
               process (and by `tmux new-session -e` on the detached path); a pass-through \
               `--settings` is MERGED into Air's, and `air status` marks a worker whose \
               hooks do not see AIR_ENFORCE=1 as UNENFORCED (ledger v15).",
    silent_break: true,
    action: "Check every launch of yours for a second `--settings`: `air worker … -- \
             --settings '{…}'` used to REPLACE Air's env block and switch the one refusal \
             off with no message (the adopter ran five hours unenforced after adding one to \
             disable Remote Control, and found out from a close that should have been \
             refused). Inline JSON now merges; a `--settings <file>` in pass-through is \
             refused with the four names. Relaunch every worker through `air worker` and \
             read `air status`: an UNENFORCED line is a session whose hooks still run \
             without the env. The detached path needs tmux >= 3.2 for `-e`.",
}
