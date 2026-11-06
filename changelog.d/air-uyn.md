Added `docs/research/pull-over-exec.md`: how `kubectl cp`, the tar-over-ssh idiom and rsync
  carry files back over an exec transport and judge success. The 141 `air run --pull` sees is
  the local `kubectl` killed by SIGPIPE after the receiving tar stopped at the end-of-archive
  marker; the idiom's verdict is the receiver's, and a sender's non-zero exit code stays a
  failure. air-b50 follows it; nothing else is built.
