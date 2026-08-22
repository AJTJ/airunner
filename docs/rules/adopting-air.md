# Adopting Air in a target repository

> **Read when:** you are the agent (or person) integrating Air into a repo that runs a fleet.
> This is the whole package: what to install, what to change in the repo's own rules and
> scripts, what Air now does that the repo's prose or scripts used to do, and how the repo keeps
> itself current afterwards. The README says what Air *is*; this says what *you* do.
> adopter-specific items are marked **[adopter]** and come from its coordinator's report of
> 2026-08-21 (`../decisions.md`, same date).

## 0. The bd gate comes first

A repo whose gates read bd inherits every breaking change in bd. **[adopter, 2026-08-21]**
bd 1.2.1 had silently corrupted the Dolt schema; 1.2.2 refused it and `bd list` returned 4 of
144 beads, breaking every bd-reading make target (recovered via bd's `RECOVERY-1.2.1.md`).
1.2.2 also removed `bd events`, which an adopter script depended on. So, before anything
else: `air doctor`. It reports the installed bd version against the pin (1.2.2) and whether
`bd list --json` actually answers, and exits 2 when it does not. Fix bd first; then install.
`air record verify` is what made the corruption visible (a recorded red at HEAD), which is why
it is the first proof below.

## 0b. No WIP cap

**[owner, 2026-08-21]** "There is no cap; we set our goals and finish them." Air measures the
`awaiting_review` count and the review wait per bead in `air status`; it never raises a
condition on them and roles.md carries no two-units-then-stop rule. A cap condition was built
and removed the same day; do not reintroduce it.

## 0c. Lessons from the first round (adopter, 2026-08-21), one line each

- Cheap checks first: doc rules moved ahead of cargo in `make verify` cut a red from ~10 min to
  4 s. A gate that reports ten minutes after the edit is a throttle; at the edit it is a fact.
- Write the digest and commit it with the work; run `air record verify` last. Otherwise the
  digest commit moves HEAD past the green and every hand-over costs two verifies.
- Deny the verb, not the tool (`Bash(eas build *)`, not `Bash(eas *)`), and match command
  tokens, never substrings: a prose guard denied an edit because a note contained the word.
- Do not set `assignee` on an open bead: in bd 1.2.x it blocks every other worker's claim.
- One label for "awaiting the owner": `human`. `air claim` refuses it to workers. Drop `owner`.
- Workers request beads (including friction beads) with `air capture`; they never `bd create`.
  The round's captures deduplicated 11 → 5 beads.
- Cut beads so no shared doc is touched by two at once; that was the only real overlap shape.
- Three workers plus a coordinator plus verify on one machine saturates it (load 22-25); bd
  writes time out. Size the fleet to the machine.
- Read before decomposing: three epics were already done on main.
- A label can be a schema when a fitness check parses it; check the documented set first.
- Closed is closed: never reopen a bead; unfinished work is a new bead referencing the old.

## 0a. Flakiness becomes load-bearing

**[adopter, 2026-08-21, ad-jklh]** Once a machine gates on "green at HEAD", a flaky test is no
longer a nuisance: a real green can record red and hold a hand-over, and a flake can mask a
real red. Air does not retry (a retry hides real reds); it makes the disagreement visible:
`air record` flags `flaky-at-head: N green / M red` when runs at one sha disagree, and
`air handover` names it with the fix "fix or quarantine the flaky test, then re-run". Before
adopting, run the repo's verify three times at one commit; every disagreement is a bug to file
first. Whether the gate should require N-of-M agreement is an owner policy, not built.

## 1. Install (owner runs; Air never writes into the target repo on its own)

1. `cargo install --path crates/cli` in the Air checkout. `which air` must be that binary.
2. `brew upgrade beads && brew pin beads`; `bd --version` reads 1.2.2; `air doctor` exits 0.
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

## 2. Coexistence, not retirement (default adoption model)

**[adopter, owner reframing 2026-08-21]** Do not retire the repo's make targets and
scripts; map the boundary. bd stays truth for ownership; `.air` becomes truth for evidence
(verify-at-sha, sessions, claims history, leases); make targets read both and write neither.
A script that depended on something bd no longer provides (`bd events`) should say "moved to
`air status`" rather than error. Retire a target only when its Air replacement has a passed
check beside it in the adoption log.

## 3. Rules to change in the repo (prose that Air replaces or that is wrong)

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

## 4. What Air now does that the repo's rules used to say

| Prose rule | Machinery |
|---|---|
| "Say which checkout you are in" | Hook records `role` (main = coordinator, worktree = worker) on every session and event |
| "Announce before touching a shared file" | `PreToolUse(Edit\|Write)` warns with the peer's name from the edit journal |
| "Do not set awaiting_review without green" | Hand-over gate: green at HEAD, main merged, claim held, digest present (advisory; `AIR_ENFORCE=1` refuses) |
| "Workers do not land, push, create beads, or leave the worktree" | Launcher deny list, held in every permission mode |
| "Coordinator does not commit on main" | Coordinator launcher denies `git commit`/`git push` |
| "Check on the fleet every N minutes" | Channel push: stuck, idle/silent/gone with a claim, hand-over not green, inbox waiting, owner decision waiting, lease held by a dead or stale session |

Delete the prose once the machinery is installed (CLAUDE.md rule: machinery over Markdown).

## 5. Keeping the integration current

- **Air version**: `air selftest` after every `cargo install`; every check proves it fires.
  `air doctor` shows the ledger version (schema migrates forward automatically).
- **Repo changes**: a new publish or destructive target goes into `.claude/air.json` deny
  patterns; a new exclusive resource is just a new lease name; a new digest location is
  `digest_dir`.
- **Claude Code upgrades**: `air worker <name> --print` shows the exact `claude` invocation;
  if a flag is rejected, that line is the bug report.
- **bd upgrades**: Air uses only `update --claim --actor`, `update -s`, `list/show/ready
  --json`, `comment`, `close`. Anything else bd adds is not assumed. Run `air doctor` after
  every bd upgrade; a version or a schema it refuses is reported before any gate sees it.
- **Air owns the loop; the repo owns the craft.** A worker reads both `.air/roles.md` (claim,
  verify, hand over, capture) and the repo's CLAUDE.md (domain rules). Keep domain rules out of
  roles.md and loop mechanics out of CLAUDE.md.
- **A role carries its drive, not just its commands.** **[adopter §9]** A worker that knows
  `air claim` and `air handover` but is not told "work it to completion now" claims and waits.
  roles.md opens the Worker section with the run-to-completion loop; the launch prompt should
  also be a complete task, not a bead id.
- **Hooks are quiet unless actionable, and quiet unless changed.** A human reads every line
  a Stop hook prints. Air's hooks say nothing on the ok path (the event line records it), speak
  once when a gap appears, and again only when something moved (HEAD, the set of missing
  checks, a new verify run; for peer warnings, the set of peers on that path). A blocked
  worker is not nagged every turn about a blocker it cannot clear. **[adopter, 2026-08-21]**
  The channel applies the same rule (new or escalated conditions only). **[adopter §9]** "handover ok" on every turn was noise in a happier
  costume; fixed 2026-08-21.
- **Sessions started before install** have the CLI but no channel and no hooks; restart them
  through `air coordinator` / `air worker`.
- **Round review**: `jq` over `.air/events/*.ndjson` and `air status --json`; the measurement
  spec (`../research/verification/ticks/2026-08-18-0430-measurement-spec.md`) says what each
  number means. What still had to be relayed by hand is the next thing Air builds.

## 6. Day one, in order

`air coordinator` in the main terminal. `air worker <name>` per worktree terminal (re-enters an
existing worktree). Workers: `bd ready` → `air claim` → work → `git merge main` →
`air record verify -- <cmd>` → `air handover` → `bd update -s awaiting_review`. Coordinator:
reads `air status`, acts on channel events, triages `air inbox`, walks `air inbox --owner`
with the owner, lands with the repo's own `make land` this round.
