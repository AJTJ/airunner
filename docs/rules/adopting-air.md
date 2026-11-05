# Adopting Air in a repository

This covers installing Air into a repo, configuring it, and upgrading it. How the fleet works
(claiming, closing, the verification lane, landing) is in `.air/roles.md`, which Air writes
into the repo and gives every session at start. This file does not restate it.

## Install

You need git 2.38 or later, tmux, Claude Code, bd 1.3.0, and, for a new repo, Dolt 2.3.5. Homebrew only has the
latest of each, so install these versions from their releases: for bd, the archive for your
platform from https://github.com/gastownhall/beads/releases/tag/v1.3.0, with `bd` put on your
PATH; for Dolt, `curl -L https://github.com/dolthub/dolt/releases/download/v2.3.5/install.sh |
sudo bash`. Build Air from its checkout with `cargo install --path crates/cli` and check that
`which air` is that binary. Then run `air doctor`: it compares bd, and Dolt when bd runs in server mode, against the pinned versions,
checks that `bd list --json` answers, and exits 2 when it does not. Fix bd before going on.

In a new repo, run `air init` to see what it would do and `air init --write` to do it. It runs
`git init` if needed, starts a Dolt server for bd on a free port in 3400..3900 (data in
`.air/dolt/`, a background process whose pid is in `.air/dolt/server.pid`), and runs
`bd init --server --skip-agents
--skip-hooks` against it, which names the bead prefix after the directory unless you pass
`--prefix`. The port goes in `.beads/dolt-server.port`, which is not committed. From then on
`air fleet up` and the launchers start the server when it does not answer, `air bd-server up`
does the same by hand, and the coordinator's channel restarts it if it stops. A repo that
already has `.beads/` keeps whatever mode it is in; `air doctor` and `air status` print it. It adds `.air/` to `.gitignore` and writes
`.claude/air.json` with the directory name as `project`, `metis` set to whether Metis is
installed, and deny patterns proposed from a scan of the repo. It also writes the hooks,
`.mcp.json`, `.air/roles.md`, `.air/docs/`, the `air-*` skills, a `.worktreeinclude`, and a
`CLAUDE.md` stub.
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
`.claude/settings.json`, adds the `air` server to `.mcp.json`, and writes `.air/roles.md`, the
`air-*` skills, and `.air/docs/` (Air's design doc, its register of solved issues, and this
guide, as the binary was built with them; `air doctor` names a copy that differs). Add `.air/` to `.gitignore` yourself. Commit `.claude/settings.json`,
`.mcp.json` and the `air-*` skills on main: every session runs in a worktree, which gets only
committed files, and `air worker` and `air lane` refuse while they are uncommitted.

Air does not install bd's agent setup, because `bd prime` tells agents to run commands Air
denies. If `air install` prints `STALE HOOK` for a `bd prime --hook-json` entry, run
`air install --write --remove-stale` to remove it.

Then commit, record the first green and start the fleet:

    air record verify -- <your verify command>
    air coordinator      # asks whether to start the fleet, then opens <project>-coordinator

A yes starts the verification lane and the workers, each in its own worktree and tmux session,
the same as `air fleet up`. `air lane` and `air worker` start one session by hand.

Before it starts a session, a launcher reads whether Claude Code has been trusted in the main
checkout (`projects["<main checkout>"].hasTrustDialogAccepted` in `~/.claude.json`) and refuses,
naming the key and the fix, when it has not. The fix is to run `claude` once in the main
checkout and answer yes to "Is this a project you created or one you trust?"; one yes covers
every worktree, and Air never writes that key. Each launch then pre-answers the prompts Claude
Code documents an answer for: `--add-dir <main checkout>`, `--permission-mode auto`, and
`skipAutoPermissionPrompt` and `enabledMcpjsonServers` on its `--settings`, so the "New MCP
server found in this project: air" question does not appear.

One prompt remains, "Loading development channels", on every session, because each loads the
Air channel, a local server, and Claude Code documents no skip for it. Choose "I am using this
for local development", or let the coordinator do it: after a launch it reads each pane, asks
you once whether to accept on your behalf, and on a yes answers with `tmux send-keys`, which its
launcher allows. The lane's whole loop (`air batch cut`, `air record verify`, `air land`) is on
its allow list, run bare, so auto mode's classifier does not refuse it.

A session started before the install has no hooks and no channel. Restart it through these
launchers.

## What the repo provides

A verify command. Air records its exit code, and the close gate reads that record. When
`air init` writes a `verify` target, it exits 1 until you replace it with the repo's real check,
so a placeholder can never record a green. `air record` prints how long the run took, how much
it printed, and the green and red counts at the commit when it has more than one run.
Run the verify three times at one commit before you rely on it: a test that fails some of the
time will hold closes.

Claude Code's Bash tool stops a command after 10 minutes by default, and a verify killed that
way records nothing. Set `BASH_MAX_TIMEOUT_MS` above your longest verify, or have the lane run
its verify detached.

Optionally, the repo also provides:

- a precheck command, which workers run under a lane with `air record precheck -- <cmd>`;
- a test-state reset the lane runs after a cut;
- `"leases"` in `.claude/air.json` for commands that need a shared resource;
- `"targets"` in `.claude/air.json`, a machine per session that `air run -- <command>` pushes
  the session's branch to and runs the command on, for work the local machine cannot do;
  `air record` runs the check there the same way. The machine is reached by whatever argv
  prefix runs a command on it, `ssh` or `kubectl exec` alike;
- a `.worktreeinclude` in gitignore syntax, naming untracked files (such as `.env`) that Air
  copies into each new worktree.
- bead sections bd must see before it files a bead, under `lint: sections:` in
  `.beads/config.yaml`. The installed `air-decomposition` skill's "Bead shape" section lists
  every part of a bead, who reads it, and the setting.

The repo's `CLAUDE.md` holds its domain rules, its verify and precheck commands, its worktree
setup and its shared resources. It does not restate the protocol in `.air/roles.md`.

## A file every branch appends to

Air cannot know which of a repo's files every bead touches. A running log that each bead and
coordinator note prepends to is the usual one: every branch changes the same spot, so the lane
drops a batch on a conflict that is only two entries side by side. At an adopter two batches in
ten minutes were dropped on one such file (2026-10-30).

For a file that is append-only and nothing else, add one line to `.gitattributes`:

    log.md merge=union

Git then keeps the lines from both sides instead of writing conflict markers. The cost is
order: entries from one batch can land in either order, and git's own documentation says to
verify the result. Do not use it for a file whose lines edit each other, a list or a count,
where both sides changing the same item merges cleanly and is wrong.

Not chosen: a stitch script that rebuilds the file after each merge, or per-session journals
that never share a file. Both cost more than one line and the second changes what the file is.
Air adds nothing for this; it is the repo's `.gitattributes`.

Source: https://git-scm.com/docs/gitattributes, "Built-in merge drivers", `union` (accessed
2026-11-02).

## Configure: `.claude/air.json`

This file is tracked, read from the main checkout, and never written by `air install`. Every
key is optional. `air status` names a top-level key the installed binary does not read, with
that binary's version, so a key landed ahead of an upgrade, or a typo, is seen there and not in
a silent default.

| Key | What it does | Default |
|---|---|---|
| `worker_deny` | Deny patterns (`"Bash(make deploy*)"`) added to every worker's and the lane's deny list. Use patterns, not lists of targets, so a new target is covered the day it exists. | none; `air init` proposes some |
| `coordinator_deny` | Deny patterns added to the coordinator's list. | none |
| `project` | The `<project>` in tmux session and Claude session names. Set it when two fleets on one machine would collide. | the beads prefix; `air init` writes the directory name |
| `workers` | How many workers `air fleet up` starts beside the lane, named `worker-1` onward. `0` starts only the lane. | `3` |
| `model` | The model every launcher (`air worker`, `air lane`, `air coordinator`, `air fleet up`) passes as `--model` when the command line gives none. `air status` marks a session whose transcript names another model. | absent: the session inherits the harness's model |
| `lane_model` | The lane's model, over `model`. | absent: the lane takes `model` |
| `heartbeat_minutes` | `<n>`: the coordinator creates a heartbeat every n minutes (run `air status`, continue owed work, else say nothing), which is what brings it back after an account limit. `air status` prints the interval. | absent: no heartbeat; `air status` still prints `STOPPED at <t>` |
| `base_branch` | The branch the fleet lands onto. Every gate, cut, landing, notice and refusal reads it where it used to say `main` (`trunk moved to`, `contains trunk`, `git merge trunk`); the main checkout must have it checked out for `air land`. The session named `main` (the primary checkout, the owner's) keeps its name. Landing the base onto the real `main`, when the base is a feature branch, is the owner's merge and stays outside Air; `main` is a long-lived branch several agents merge into, which trunk-based development warns against. | `main` |
| `coordinator_autobatch` | `true`: every coordinator commit main lacks is batch-ready, so the lane cuts after each one. Absent or `false`, the coordinator's branch is batch-ready only at a head it offered with `air ready`. | `false` |
| `verify_lane` | `true`: workers run no verify of their own and close on the lane's batch green (the with-lane sequence in `.air/roles.md`). It does not name the lane; the lane is the session `air lane` started. A string value counts as absent; a lane session having run also counts as a lane. | absent: workers verify their own branch |
| `precheck` | `true`: a branch is batch-ready only with a green `air record precheck` at its head. A precheck green never counts as a verify green. | `false` |
| `verify_key` | `"tree"`: a green at one commit also counts at another commit with the identical tree. Set it only if your verify reads the tree and not git history (`git log`, `rev-list`, commit messages). | `"commit"` |
| `leases` | `{"<resource>": ["<pattern>", …]}` in deny-rule syntax. Air refuses a matching command to a worker or the lane that does not hold the lease, and advises the coordinator. `air lease needs "<cmd>"` shows the match. | none |
| `targets` | `{"<session>": {"exec": ["kubectl", "exec", "-i", "<pod>", "-c", "<container>", "-n", "<ns>", "--"], "path": "~/air/<project>"}}`, one per session (`worker-1`, `lane`, …): `exec` is the argv prefix that runs a command on the machine with stdin and stdout attached, and `path` the repository directory there. `{"kind": "ssh", "host": "<h>", "user": "<u>", "path": …}` is the short form for the prefix `ssh <u>@<h>`; it is a kind of its own because ssh hands its arguments to a remote shell as one line, where `kubectl exec` passes them as argv, and Air quotes for that shell only in the short form. `air run -- <cmd>` from that session pushes HEAD to `refs/air/<session>` in a repository it initialises at `path` (the push runs the prefix as git's `ext::` remote, turned on for that one command), checks it out, and runs the command there as `<prefix> sh -c '<line>'`; the exit code and output come back. A session with no entry runs the command locally. The push never goes to the project's remote, and `.git/config` is not written. `air record` through a target runs the check there on the pushed HEAD and records the result at the local HEAD; a dirty tree is refused, since the target holds only commits, and `--local` runs it here. `air run --pull <glob>` brings the matching result paths back into the worktree. Every run, local or not, is one row in the ledger's `runs` table with the machine it ran on, and `--json` prints it (air-opq.3, air-opq.4, air-opq.5). | none: every session runs locally |
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
'{"env":{…}}'`. Air merges it into its own settings: objects merge key by key, lists such as
`permissions.allow` are combined, and Air's four env values win. A `--settings
<file>` there is refused. `AIR_BD_TIMEOUT_MS` replaces Air's whole bd budget with one flat
figure; leave it unset unless a bd call times out.

On a Mac each launcher runs the session as the child of `caffeinate -i`, so the machine does
not idle-sleep under an hour-long command and may once the session ends. `AIR_CAFFEINATE_BIN`
names another binary, and an empty value turns the wrapper off. `air status` prints the
idle-sleep setting when `pmset -g` reports under ten minutes (air-1ye).

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
   overwrites `.air/roles.md`, `.air/docs/`, `.claude/skills/air-*/SKILL.md` and
   `.air/installed.json`. It removes each `air-*` skill directory Air once installed and no
   longer ships, naming it. It
   never touches `.claude/air.json`, `.gitignore`, the ledger, `.beads/`, a skill whose name
   does not start with `air-`, or any other file. It refuses to write when the `air` on `PATH`
   is not the binary being run, because the hooks call `air` through `PATH`.

   To run a different build in one repo without replacing the `air` on `PATH`, use
   `air install --write --pin`. It copies the binary being run to `.air/bin/air` and points the
   hooks and the channel at that copy by absolute path. Air's launchers put `.air/bin` first on
   `PATH` in every session they start, and `air status` and `air doctor` name the pin. Because a
   shell may re-order that `PATH`, any other `air` run in the repo hands the command to the pin
   before running it, so a bare `air` runs the pin as long as the one found is new enough to do
   that; `air install` is the exception, so re-pinning runs the new build. `air
   install --write --unpin` goes back to the `air` on `PATH`. A pinned repo refuses a plain
   `--write` until one of the two flags says which binary it should run. The hook commands
   hold an absolute path, so a pin suits a scratch copy or one machine rather than a repo
   others clone.
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
