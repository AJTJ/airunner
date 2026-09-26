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
`git init` if needed and `bd init --skip-agents --skip-hooks`, which names the bead prefix after
the directory unless you pass `--prefix`. It adds `.air/` to `.gitignore` and writes
`.claude/air.json` with the directory name as `project`, `metis` set to whether Metis is
installed, and deny patterns proposed from a scan of the repo. It also writes the hooks,
`.mcp.json`, `.air/roles.md`, the `air-*` skills, a `.worktreeinclude`, and a `CLAUDE.md` stub.
It creates each only when absent and never edits one, so re-running it is safe.

`air init` prints the verify command it proposes. It uses the first of these it finds: a
Makefile `verify` target, a Makefile `test` target, `cargo test` for a `Cargo.toml` at the root,
and `npm test` when `package.json` has a real test script. When it finds none, it writes a
`Makefile` whose `verify` target fails until you replace it with your check. The `CLAUDE.md`
stub names the command, and so does the last thing `air init --write` prints:

    next:
      git add -A && git commit -m "Adopt Air"
      air record verify -- <the proposed command>   # the first green
      air coordinator                               # its own worktree and tmux session

In a repo that already has its own setup, run `air install` to read what it would do and `air
install --write` to apply it. It merges `air hook` into every hook event in
`.claude/settings.json`, adds the `air` server to `.mcp.json`, and writes `.air/roles.md` and
the `air-*` skills. Add `.air/` to `.gitignore` yourself. Commit `.mcp.json`: the coordinator
runs in a worktree and needs the channel server there.

Air does not install bd's agent setup, because `bd prime` tells agents to run commands Air
denies. If `air install` prints `STALE HOOK` for a `bd prime --hook-json` entry, delete that
entry from `.claude/settings.json`.

Then commit, record the first green and start the fleet:

    air record verify -- <your verify command>
    air coordinator                                   # tmux session <project>-coordinator
    air lane --tmux                                   # the verification lane; only it lands
    air worker --tmux --task "<a complete task>"      # worker-1, worker-2, …

A session started before the install has no hooks and no channel. Restart it through these
launchers.

## What the repo provides

A verify command. Air records its exit code, and the close gate reads that record. When
`air init` writes a `verify` target, it exits 1 until you replace it with the repo's real check,
so a placeholder can never record a green. `air record` flags a green as `suspicious` when it
printed nothing, or when it ran in under a fifth of the time the same command's last green took.
Run the verify three times at one commit before you
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
| `project` | The `<project>` in tmux session and Claude session names. Set it when two fleets on one machine would collide. | the beads prefix; `air init` writes the directory name |
| `verify_lane` | Names the lane. It puts the with-lane closing sequence in force, and when `air batch cut` is refused in the main checkout, the refusal names the lane's worktree from it. | absent: no lane sequence |
| `precheck` | `true`: a branch is batch-ready only with a green `air record precheck` at its head. A precheck green never counts as a verify green. | `false` |
| `verify_key` | `"tree"`: a green at one commit also counts at another commit with the identical tree. Set it only if your verify reads the tree and not git history (`git log`, `rev-list`, commit messages). | `"commit"` |
| `leases` | `{"<resource>": ["<pattern>", …]}` in deny-rule syntax. Air refuses a matching command to a worker or the lane that does not hold the lease, and advises the coordinator. `air lease needs "<cmd>"` shows the match. | none |
| `digests` | `true`: the close gate needs a digest naming the bead in the main checkout's `.air/digests/`. | `false` |
| `digest_dir` | A repo path: the close gate needs a digest naming the bead there, tracked by git. Wins over `digests`. | none |
| `journal_dir` | A repo path for session journals, tracked in the repo instead of `.air/journal/`. | none |
| `metis` | `true`: `air coordinator` attaches Metis's MCP server. Workers never get it. If `metis` is not on `PATH` the launch prints one line and goes on. | `false`; `air init` writes `true` when Metis is installed |
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
   never touches `.claude/air.json`, `.gitignore`, the ledger, `.beads/` or any other file,
   and it does not delete a skill Air no longer ships. It refuses to write when the `air` on
   `PATH` is not the binary being run.
5. Restart every session. A running session keeps the old roles text and hooks.
6. Grep the repo's Makefile, scripts and `CLAUDE.md` for `air ` and check each use against the
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
