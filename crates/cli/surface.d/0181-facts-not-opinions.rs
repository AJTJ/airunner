SurfaceChange {
    id: "facts-not-opinions",
    since: "2026-09-28 (air-ch9p)",
    headline: "Messages state facts and the fixing command. `air status` prints `in batch: \
               <worker> at <head> is in batch <sha>, being verified now` for a head inside a \
               running batch, which is no longer batch-ready (`not_batch_ready` check \
               `in-batch`). `landable` is not pushed while a lane session is live. The \
               no-trailer close refusal names `Bead: <id>`. Gate fixes print as `fix: ...`, \
               never \"run `...`\", and none suggests `git commit --amend`. A batch of one has \
               its own green and red text; \"close them now\" names only beads the member \
               still holds; `batch-ready` prints `(no bead)` for a branch carrying none.",
    silent_break: true,
    action: "A script that matched the old wording (\"run `\", \"prompt them\", \"most \
             likely\", \"the lane splits\" for a one-member batch, \"worker(s) idle\" in the \
             queue-empty notice) matches nothing now; read the new lines.",
}
