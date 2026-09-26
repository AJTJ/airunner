# Adopting Air in a repository

This covers installing Air into a repo, configuring it, and upgrading it. How the fleet works
(claiming, closing, the verification lane, landing) is in `.air/roles.md`, which Air writes
into the repo and gives every session at start. This file does not restate it.

## Install

You need git 2.38 or later, tmux, Claude Code, and bd 1.2.2 (`brew install beads && brew pin
beads`). Build Air from its checkout with `cargo install --path crates/cli` and check that
`which air` is that binary. Then run `air doctor`: it compares bd against the pinned version,
checks that `bd list --json` answers, and exits 2 when it does not. Fix bd before going on.

In a new repo, run `air init` to see what it would do and `air init --write` to do it. It runs
`git init` if needed and `bd init --skip-agents --skip-hooks`, adds `.air/` to `.gitignore`, and
writes `.claude/air.json` with deny patterns proposed from a scan of the repo. It also writes
the hooks, `.mcp.json`, `.air/roles.md`, the `air-*` skills, a `Makefile` with a `verify`
target, a `.worktreeinclude`, and a `CLAUDE.md` stub. It creates each only when absent and
never edits one, so re-running it is safe.

In a repo that already has its own setup, run `air install` to read what it would do and `air
install --write` to apply it. It merges `air hook` into every hook event in
`.claude/settings.json`, adds the `air` server to `.mcp.json`, and writes `.air/roles.md` and
the `air-*` skills. Add `.air/` to `.gitignore` yourself. Commit `.claude/settings.json`,
`.mcp.json` and the `air-*` skills on main: every session runs in a worktree, which gets only
committed files, and `air worker` and `air lane` refuse while they are uncommitted.

Air does not install bd's agent setup, because `bd prime` tells agents to run commands Air
denies. If `air install` prints `STALE HOOK` for a `bd prime --hook-json` entry, delete that
entry from `.claude/settings.json`.

Then record the first green and start the fleet:

    air record verify -- make verify
    air coordinator                                   # tmux session <project>-coordinator
    air lane --tmux                                   # the verification lane; only it lands
    air worker --tmux --task "<a complete task>"      # worker-1, worker-2, …

A session started before the install has no hooks and no channel. Restart it through these
launchers.

## What the repo provides

A verify command. Air records its exit code, and the close gate reads that record. The
`verify` target `air init` writes exits 1 until you replace it with the repo's real check, so
a placeholder can never record a green. Run the verify three times at one commit before you
rely on it: `air record` flags runs that disagree at one sha, and a flaky test will hold closes.

Claude Code's Bash tool stops a command after 10 minutes by default, and a verify killed that
way records nothing. Set `BASH_MAX_TIMEOUT_MS` above your longest verify, or have the lane run
its verify detached.

Optionally, the repo also provides:

- a precheck command, which workers run under a lane with `air record precheck -- <cmd>`;
- a test-state reset the lane runs after a cut;
- `"leases"` in `.claude/air.json` for commands that need a shared resource;
- a `.worktreeinclude` in gitignore syntax, naming untracked files (such as `.env`) that Air
  copies into each new worktree.

The repo's `CLAUDE.md` holds its domain rules, its verify and precheck commands, its worktree
setup and its shared resources. It does not restate the protocol in `.air/roles.md`.

## Configure: `.claude/air.json`

This file is tracked, read from the main checkout, and never written by `air install`. Every
key is optional.

| Key | What it does | Default |
|---|---|---|
| `worker_deny` | Deny patterns (`"Bash(make deploy*)"`) added to every worker's and the lane's deny list. Use patterns, not lists of targets, so a new target is covered the day it exists. | none; `air init` proposes some |
| `coordinator_deny` | Deny patterns added to the coordinator's list. | none |
| `project` | The `<project>` in tmux session and Claude session names. Set it when two fleets on one machine would collide. | the beads prefix |
| `verify_lane` | `true`: workers run no verify of their own and close on the lane's batch green (the with-lane sequence in `.air/roles.md`). It does not name the lane; the lane is the session `air lane` started. An older string value counts as `true`. | absent: workers verify their own branch |
| `precheck` | `true`: a branch is batch-ready only with a green `air record precheck` at its head. A precheck green never counts as a verify green. | `false` |
| `verify_key` | `"tree"`: a green at one commit also counts at another commit with the identical tree. Set it only if your verify reads the tree and not git history (`git log`, `rev-list`, commit messages). | `"commit"` |
| `leases` | `{"<resource>": ["<pattern>", …]}` in deny-rule syntax. Air refuses a matching command to a worker or the lane that does not hold the lease, and advises the coordinator. `air lease needs "<cmd>"` shows the match. | none |
| `digests` | `true`: the close gate needs a digest naming the bead in the main checkout's `.air/digests/`. | `false` |
| `digest_dir` | A repo path: the close gate needs a digest naming the bead there, tracked by git. Wins over `digests`. | none |
| `journal_dir` | A repo path for session journals, tracked in the repo instead of `.air/journal/`. | none |
| `metis` | `true`: `air coordinator` attaches Metis's MCP server. Workers never get it. If `metis` is not on `PATH` the launch prints one line and goes on. | `false`; `air init` writes `true` |
| `metis_plugin_dir` | The `plugins/metis` directory of a Metis checkout, attached as a plugin as well. | none |
| `adopters` | `true`: `air adopter-check` refuses when `private/adopters.md` is missing instead of skipping. | `false` |

A repo with its own lease guard moves its patterns into `leases` and retires the guard. Two
lease stores that disagree refuse commands while reporting success, so drain the old store
(release every held lock) before switching, and move its directory aside rather than deleting
it so a reader you missed fails loudly.

To add environment variables to a worker, pass inline JSON: `air worker <name> -- --settings
'{"env":{…}}'`. Air merges it into its own settings and its own four values win. A `--settings
<file>` there is refused. `AIR_BD_TIMEOUT_MS` replaces Air's whole bd budget with one flat
figure; leave it unset unless a bd call times out.

## Upgrading an existing installation

Air changes under a repo that already has it, so each release carries notices in
`install::SURFACE`, and `air install` shows the ones this repo has not seen. Releases are cut
once per round, so notices arrive in batches.

1. In the Air checkout, `cargo install --path crates/cli`, then check `which air`.
2. `air selftest`, then `air doctor`.
3. In the repo's main checkout, run `air install`. It writes nothing and prints the surface
   diff against `.air/installed.json`, each change with what to do. Changes marked `!!` alter
   behaviour without an error; do those first.
4. `air install --write`. It merges into `.claude/settings.json` and `.mcp.json`, and
   overwrites `.air/roles.md`, `.claude/skills/air-*/SKILL.md` and `.air/installed.json`. It
   removes each `air-*` skill directory Air once installed and no longer ships, naming it. It
   never touches `.claude/air.json`, `.gitignore`, the ledger, `.beads/`, a skill whose name
   does not start with `air-`, or any other file. It refuses to write when the `air` on `PATH`
   is not the binary being run.
5. Commit `.claude/settings.json`, `.mcp.json` and `.claude/skills/air-*` on main. A new
   worktree gets only committed files, so `air worker` and `air lane` refuse, naming the files,
   while any of them is uncommitted in the main checkout.
6. Restart every session. A running session keeps the old roles text and hooks.
7. Grep the repo's Makefile, scripts and `CLAUDE.md` for `air ` and check each use against the
   notices. Air cannot see what the repo built on top of it.

Between upgrades, `air doctor` and `air status` print one line when `.air/installed.json` lags
the binary the hooks run.

After a Claude Code upgrade, `air worker <name> --print` shows the exact `claude` command line,
which is the thing to check if a flag is rejected. After a bd upgrade, run `air doctor`.

A repo that used the label `human` to hold beads for the owner must move to `owner`, the only
label `air claim` refuses to workers. Exclude both labels from the repo's ready target, relabel
each `human` bead that is really an owner decision (`bd update <id> … --add-label owner`), and
drop `human` from the exclusion only when `bd list --label human` is empty. Read each bead
rather than relabelling in bulk, since `human` was often used for ordinary work.
