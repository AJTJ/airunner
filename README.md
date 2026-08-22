# Air

Air is a small tool for running a few AI coding agents on one repository at the same time
without them tripping over each other or over you. One Rust binary, `air`. It sits beside
[beads](https://github.com/steveyegge/beads) (the issue tracker) and Claude Code (the agents),
and it keeps the *facts* that a fleet otherwise carries in chat and in people's heads.

## The ethos, in five lines

1. **Do less.** The point is to let capable models work, not to tell them how. Every rule Air
   holds must name a real failure it prevents and the condition under which it is removed.
   Guardrails that nobody re-examines become the throttles a better model does not need.
2. **Facts, not procedures.** Air supplies what an agent cannot know on its own (who is in
   this file, whether this commit verified green, who holds the port). It does not supply
   judgement (what to work on, how to split it, when to ask).
3. **One refusal.** Air refuses exactly one thing: handing work over without evidence that it
   verified green at the commit being handed over. Everything else is a fact or a count.
4. **A human is always in the loop.** Every agent is a terminal you can watch and type into.
   Air launches interactive sessions; it never runs anything headless.
5. **Your project comes first.** Air exists so your projects get built. Building Air is
   secondary, and rounds of real work are where its features are earned or removed.

## What Air does

| | |
|---|---|
| **Records** | verify results against the exact commit (`air record verify -- <cmd>`), who is editing which file (hooks, zero tokens), claims and their history (`air claim`, `air release`), leases on things two agents cannot share (`air lease`), captures from workers (`air capture`), every decision it made with its reason (`.air/events/*.ndjson`). |
| **Answers** | `air status` (every worker: session, claim, green at HEAD, files, leases, review waits, queue depth), `air holdings` (who is in which file), `air handover` (what is missing and the command that fixes it). |
| **Refuses** | moving a bead to review or closing it without a recorded green at HEAD that contains `main` (advisory for a first round; `AIR_ENFORCE=1` makes it real). Also: workers claiming a `human` bead, reopening a closed bead. |
| **Informs** | the coordinator session, through a Claude Code channel, only when a condition holds: a worker stuck on a prompt, idle or gone with a claim, a hand-over not green, a lease held by a dead session, a decision waiting for the owner, a session joining or leaving. Silence means all is well. |
| **Launches** | `air coordinator` (main checkout, channel attached) and `air worker <name> [--tmux --task "..."]` (a worktree, the roles text, a deny list that holds in every permission mode, actor and role set by flag instead of files that drift). |
| **Sets up** | `air init` on a new or existing repo: checks `bd` and `claude` are present, then git, beads, `.gitignore`, `.claude/air.json` with deny patterns scanned from your publish targets, hooks, the MCP server, roles, and the coordinator's skills. |

## What is Air's, and what is your project's

This is the line that matters. Air owns the **loop**; your project owns the **craft**.

| Your project decides | Air provides |
|---|---|
| What "verify" runs (`make verify`, `cargo test`, …) and whether it is complete | That the result is recorded against the commit, how long it took, whether it was suspicious, flaky, or run on a dirty tree |
| Domain rules, code conventions, architecture, what to build next | Nothing. Those live in your `CLAUDE.md`; Air's roles text never mentions your domain |
| Which commands publish, deploy, or destroy | The deny list is applied at launch, in every permission mode, from patterns you put in `.claude/air.json` (`air init` proposes them) |
| What a bead must contain (acceptance, estimate, files named) and who triages captures | `air capture` → `air inbox` → `air triage`; beads' own `--validate` refuses a bead without acceptance |
| Which beads need the owner | The `human` label; Air refuses it to workers and pushes `owner-decision-waiting` |
| How to cut work so two agents do not touch one file | `air holdings` and a once-per-session warning when a peer is in the file you open |
| Whether a digest is required at hand-over, and what it says | The check that one exists newer than the claim, if you configure `digest_dir` |
| Fleet size, who works on what, when a round starts and stops | `air status`, the channel, and the launchers; no scheduler, no queues, no caps |
| Which resources are exclusive (a port, the simulator, the browser) and what they are called | `air lease take <name>`; a dead holder is detected from the process, not a timer |
| Landing to `main`, with your own gates | Your `make land` (or equivalent) until `air land` exists; Air records the green it needs |

**What Air is not useful for:** choosing work, planning features, reviewing code, writing
acceptance criteria, enforcing code style, or replacing beads. It will not make one agent
smarter. It makes three agents and one person cost fewer messages and fewer false greens.

## Quick start

```bash
cargo install --path crates/cli            # `air` on PATH must be this binary
cd ~/projects/<repo>
air init --prefix <p>                      # dry run: the gate and what it will create
air init --prefix <p> --write              # everything; never overwrites a file you own
air record verify -- <your verify command> # the first proof
air coordinator                            # your terminal, channel attached
air worker <name> --tmux --task "<a complete task>"   # or plain `air worker <name>`
```

`air init` cannot install `bd` or `claude`; it checks both first and prints the install command
if one is missing. Existing repos: it skips what is already there. For the migration path
(retiring an existing hand-rolled process) see `docs/rules/adopting-air.md`.

## Day to day

**Worker:** `bd ready --type task` → `air claim <id> --files a,b` → work, small commits →
write the digest and commit it → `git merge main` → `air record verify -- <cmd>` →
`air handover` → `bd update <id> -s awaiting_review` → next. Found something outside the bead?
`air capture "<one line>"`. Blocked? `air capture --for owner "<question>"` or
`air release <id> --reason <why>`.

**Coordinator:** `air status`; act on channel events; `air inbox` → `bd create --validate
--estimate <min>` → `air triage <id> --bead <new>`; keep the ready list full of claimable
tasks; launch workers; land with your own command.

**You:** `air status --attention` in any terminal; `jq` over `.air/events/*.ndjson` after a
round; `air inbox --owner` for the decisions waiting on you.

## Status and scope

Built and running on two repositories (the author's fleet project and Air itself). First
external use is planned; open-sourcing after that. The record of every decision, with the
incident behind it, is `docs/decisions.md`; what was learned from the first real round is
`docs/notes/rounds/`. Roadmap: `docs/plans/0005-roadmap.md`.

## Repository map

`crates/ledger` (SQLite + events), `crates/hooks` (hook I/O, the gate), `crates/bd` (the
beads boundary), `crates/cli` (`air`). `docs/rules/roles.md` is what agents read;
`docs/rules/adopting-air.md` is the integration guide; `docs/research/` is the evidence, every
claim with a source.
