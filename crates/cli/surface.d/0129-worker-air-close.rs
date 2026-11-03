SurfaceChange {
    id: "worker-air-close",
    since: "2026-09-26 (owner ruling, 0.4.5 live trial)",
    headline: "Workers and the lane close with `air close <id> --reason-file <proof>`, which \
               runs the hand-over gate on each bead before it closes and closes nothing if \
               one fails, whether or not AIR_ENFORCE is set. It is no longer on the worker \
               deny list. The hook's check on a raw `bd close` stays as a backstop, because \
               it reads the command text and missed a close on its own line in the trial; \
               its refusal now names `air close`.",
    silent_break: false,
    action: "Replace `bd close` with `air close` in your repo's own prose, prompts and \
             scripts that tell a worker how to close a bead.",
}
