Added `air ui`'s `/api/fleet` and `/api/feed` carry real data. Each session card has its
  state, model, claims, worktree head, whether its process is alive, the harness's busy, idle
  or waiting from Claude Code's session file, the tool call in flight with its command or file
  from the transcript tail, the last text, the context size, and the last lines of its tmux
  pane. The feed is Air's decisions from the event stream with the quiet lines left out, plus
  the notices and messages in the same span, from a byte offset the page passes back.
