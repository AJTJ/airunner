Fixed The `air ui` fleet card showed no current tool: the transcript's last 64 KiB often holds
  only Claude Code's non-message records. The tail is now read at 64 KiB, 256 KiB and 1 MiB in
  turn until it holds an assistant record, with one stat and at most three reads per session.
