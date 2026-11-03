SurfaceChange {
    id: "channel-every-session",
    since: "2026-09-26 (air-1vri)",
    headline: "The lane and every worker now load the Air channel, as the coordinator \
               already did, and each session's `air mcp` pushes into it the messages Air \
               addressed to that session (ledger table `deliveries`, one event line \
               `channel.deliver / delivered` per message). So every session Air starts \
               shows the development-channels warning; answer \"I am using this for local \
               development\". Only the coordinator's and the owner's `air mcp` run the \
               attention poll now; a worker's used to run it into a session with no \
               channel.",
    silent_break: false,
    action: "Restart running sessions through `air fleet up`, `air worker` or `air lane` \
             to attach the channel.",
}
