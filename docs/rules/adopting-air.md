# Adopting Air in a target repository

> **Read when:** you are the agent (or person) integrating Air into a repo that runs a fleet.
> This is the whole package: what to install, what to change in the repo's own rules and
> scripts, what Air now does that the repo's prose or scripts used to do, and how the repo keeps
> itself current afterwards. The README says what Air *is*; this says what *you* do.
> adopter-specific items are marked **[adopter]** and come from its coordinator's report of
> 2026-08-21 (`../decisions.md`, same date).

## 1. Install (owner runs; Air never writes into the target repo on its own)

1. `cargo install --path crates/cli` in the Air checkout. `which air` must be that binary.
2. `brew upgrade beads && brew pin beads`; `bd --version` reads 1.2.2. **[adopter: 1.2.1 today]**
3. In the target's main checkout: `air install` (read it), then `air install --write`. It adds
   `air hook` under seven events in `.claude/settings.json`, the `air` server in `.mcp.json`,
   and `.air/roles.md`. Add `.air/` to `.gitignore`.
4. Create `.claude/air.json` (tracked):
   ```json
   {
     "digest_dir": "docs/log.d",
     "worker_deny": ["Bash(make deploy*)", "Bash(eas *)", "Bash(expo publish*)"],
     "coordinator_deny": []
   }
   ```
   `digest_dir` turns on the fourth hand-over check. Deny entries are *patterns* so a new
   publish target is covered the day it exists (**[adopter]** `make deploy-site` shipped
   outside an enumerated list).

## 2. Rules to change in the repo (prose that Air replaces or that is wrong)

| Today | Change to | Why |
|---|---|---|
| `bd update <id> --claim` in worker rules | `air claim <id> [--files a,b]`; release with `air release <id> --reason …` | The launcher denies raw `--claim`; Air keeps the claim history bd does not |
| `bd human <id>` (**[adopter]** CLAUDE.md tells agents to run it 4×) | Delete. It does not exist in bd 1.2.x; it prints help and no-ops | Replace with `air capture --for owner "<question>"`; the owner walks `air inbox --owner` |
| `make note` / a tracked intake file (**[adopter]** `intake.jsonl` dirtied main and blocked `make land`) | `air capture "<one line>"` | `.air/` is gitignored; capture never touches a tracked file |
| Workers file beads | Workers never run `bd create` (denied). Coordinator: `air inbox` → `bd create --validate --estimate <min>` → `air triage <id> --bead <new>` | bd `--validate` already refuses a task/feature/bug without `## Acceptance Criteria` |
| Per-worktree `settings.local.json` env (`BEADS_ACTOR`, `CARGO_TARGET_DIR`) | Set nothing in files; `air worker <name>` passes `BEADS_ACTOR=<name>` and `AIR_ROLE` by flag. Add repo env the same way via `air worker <name> -- --settings '{"env":{…}}'` or put it in `.claude/air.json` (`worker_env`, when built) | **[adopter]** renamed worktrees kept old values: beads misattributed, `make test` built into another worktree's target dir, `make land` refused |
| `scripts/lease.sh` / `make lease-*` | `air lease take|release|status|break [<resource>]`; resources: `runtime` (ports, device, Docker), `:8080`, `simulator`, `chrome`, … | Same semantics (worktree identity, pid liveness, stale heartbeat), plus dead-holder attention pushed to the coordinator. Keep the make targets as aliases for one round |
| Heartbeat cron that wakes the coordinator | Delete it. `air coordinator` attaches the channel; conditions arrive when they hold | `air status --attention` is the same list on demand |
| `make fleet` (live agents, overlap) | Alias to `air status` / `air holdings` | One source; no drift between scripts |
| `make verify` run bare | `air record verify -- make verify` (also `fitness`, `docs-check`) | The gate needs the fact; Air flags suspicious (under 2 s, silent), changed-command, dirty-tree, and refuses backgrounded runs |
| Generated-files exclusion list duplicated in `land.sh` and `fleet.sh` **[adopter]** | One file sourced by both until `air land` owns it | Drift |
| "Verify is complete" assumed **[adopter]** (jest silently skipped; a deleted generated `router.d.ts` silenced tsc) | Add a fitness check: verify invokes every test runner the repo has; land regenerates generated inputs before verify | Air records the exit honestly; completeness is the repo's |

## 3. What Air now does that the repo's rules used to say

| Prose rule | Machinery |
|---|---|
| "Say which checkout you are in" | Hook records `role` (main = coordinator, worktree = worker) on every session and event |
| "Announce before touching a shared file" | `PreToolUse(Edit\|Write)` warns with the peer's name from the edit journal |
| "Do not set awaiting_review without green" | Hand-over gate: green at HEAD, main merged, claim held, digest present (advisory; `AIR_ENFORCE=1` refuses) |
| "Workers do not land, push, create beads, or leave the worktree" | Launcher deny list, held in every permission mode |
| "Coordinator does not commit on main" | Coordinator launcher denies `git commit`/`git push` |
| "Check on the fleet every N minutes" | Channel push: stuck, idle/silent/gone with a claim, hand-over not green, inbox waiting, owner decision waiting, lease held by a dead or stale session |

Delete the prose once the machinery is installed (CLAUDE.md rule: machinery over Markdown).

## 4. Keeping the integration current

- **Air version**: `air selftest` after every `cargo install`; every check proves it fires.
  `air doctor` shows the ledger version (schema migrates forward automatically).
- **Repo changes**: a new publish or destructive target goes into `.claude/air.json` deny
  patterns; a new exclusive resource is just a new lease name; a new digest location is
  `digest_dir`.
- **Claude Code upgrades**: `air worker <name> --print` shows the exact `claude` invocation;
  if a flag is rejected, that line is the bug report.
- **bd upgrades**: Air uses only `update --claim --actor`, `update -s`, `list/show/ready
  --json`, `comment`, `close`. Anything else bd adds is not assumed.
- **Round review**: `jq` over `.air/events/*.ndjson` and `air status --json`; the measurement
  spec (`../research/verification/ticks/2026-08-18-0430-measurement-spec.md`) says what each
  number means. What still had to be relayed by hand is the next thing Air builds.

## 5. Day one, in order

`air coordinator` in the main terminal. `air worker <name>` per worktree terminal (re-enters an
existing worktree). Workers: `bd ready` → `air claim` → work → `git merge main` →
`air record verify -- <cmd>` → `air handover` → `bd update -s awaiting_review`. Coordinator:
reads `air status`, acts on channel events, triages `air inbox`, walks `air inbox --owner`
with the owner, lands with the repo's own `make land` this round.
