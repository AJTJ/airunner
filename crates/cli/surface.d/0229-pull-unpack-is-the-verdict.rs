SurfaceChange {
    id: "pull-unpack-is-the-verdict",
    since: "2026-11-06 (air-b50)",
    headline: "`air run --pull <glob>` judges the pull by the local unpack, as `kubectl cp` \
               does: a sender that dies of SIGPIPE (signal 13, or exit 141 from a shell) after \
               a good unpack is the normal end, and any other non-zero sender exit is reported \
               as the target's tar failing. Before, every pull through `kubectl exec` printed \
               \"the tar stream failed (target failed, unpack exit 0)\" and recorded nothing \
               pulled although the files had arrived, because the local tar closes the pipe at \
               the end-of-archive marker.",
    silent_break: false,
    action: "Nothing. A run row's `pulled` list is now filled through such a prefix.",
}
