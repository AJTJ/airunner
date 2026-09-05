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
   `air hook` under every event in `hook_entries()` (`crates/cli/src/cmd/install.rs`; nine as of
   2026-08-22), the `air` server in `.mcp.json`, `.air/roles.md`, and the `air-*` skills. Add
   `.air/` to `.gitignore`.
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

   A fourth field, `"verify_key": "tree"`, is **opt-in and off by default** (air-7wf). A
   recorded green is looked up by commit, by whichever worker ran it. `air land` builds the
   landing commit from the branch's tree, so main is a new sha over a verified tree and reads
   `not green` until somebody re-verifies it; with the tree key that green is found again and
   no re-verify runs. **Declare it only if your verify is a function of the tree alone.** A
   suite that reads history verifies differently at two commits with one tree: adopter's
   `make verify` runs `git log main..HEAD` to choose which beads to check
   (`scripts/lib/bead_citations.py:140`), so a tree-keyed gate there would pass beads it never
   checked. The ten-minute check is to grep the verify for `git log`, `rev-list`, `describe`
   and anything reading commit messages; ai_runner's own passes (its only real-repo git calls
   in `make verify` are `git status --porcelain` and `git checkout --`), which is why this
   repo's `.claude/air.json` declares it. Either way `air status` names a tree green honestly:
   `green (same tree as <sha> verified by <worker>)` when it counts, and `not green (this
   exact tree is green at <sha> by <worker>, but this repo keys green by commit)` when it
   does not.

## 2. Coexistence, not retirement (default adoption model)

**[adopter, owner reframing 2026-08-21]** Do not retire the repo's make targets and
scripts; map the boundary. bd stays truth for ownership; `.air` becomes truth for evidence
(verify-at-sha, sessions, claims history, leases); make targets read both and write neither.
A script that depended on something bd no longer provides (`bd events`) should say "moved to
`air status`" rather than error. Retire a target only when its Air replacement has a passed
check beside it in the adoption log.

### The repo owns its work flow; Air does not

`.air/roles.md` is generated from Air's `ROLES_MD` and overwritten on every install, so a
target repo cannot edit it and should not try. What it says is therefore deliberately limited
to **what Air records and what Air refuses**:

| Air says | The repo says |
|---|---|
| `air claim` is the claim path; a claim is a commitment to finish now | Whether a finished bead is handed over for review, closed with proof, or something else |
| The recorded green must be at the commit you hand on, and that commit must contain `main` | Which command counts as verify |
| A digest must be newer than the claim, where `digest_dir` is set | Where digests live and what goes in one |
| The one refusal: the `bd` write that ends work on a bead is denied without a green at HEAD | Which `bd` status that write sets |
| `air handover` names what is missing | When in the loop to run it |
| Landing is the coordinator's, not a worker's; a landing needs a recorded green at a head containing `main`; Air records the landings it performs | **Which command lands, and everything it does on the way** |

**[adopter, 2026-08-22, air-8zu]** roles.md used to prescribe
`bd update <id> -s awaiting_review` as the closing step. adopter's owner had ruled that step
out of existence — a worker there closes its own bead with proof — so its workers were told one
flow by Air at session start and another by their own CLAUDE.md, and could not fix the file.
The fix was for Air to stop saying it, not to add per-repo configuration: config is a
mechanism, and the smaller version is Air not saying what it has no business saying.

Put the repo's own sequence in its CLAUDE.md (this repo keeps ai_runner's under "This repo's
work flow"). The one refusal still covers both shapes: it matches `bd close` as well as
`bd update -s closed` / `-s awaiting_review`, so a close-with-proof repo is gated exactly as a
hand-over repo is.

**[owner ruling 2026-08-29, air-97z] The same applies to landing, and it did not used to.**
roles.md said hand-over was the repo's in as many words and then prescribed the landing command
three lines later. **A repo with its own lander keeps it.** Air does not ask for it to be
replaced, and nothing `air init` or `air install` writes names a landing command.

The worked case is adopter's `make land`: 685 lines, hardened over four separate blockers, and
carrying repo knowledge Air does not have and has no business acquiring — which generated files
are safe to discard on a rewind, that their verify must run `SCOPE=full`, digest tiers, a bead
index, a friction query, and a refusal when a branch adds no `docs/log.d/` entry. It does not
call `air` at all, and their Makefile already calls `air land` "the other path". Replacing it
with `air land` would trade four blockers' worth of hardening for uniformity nobody asked for.

What Air keeps saying about landing names no command: it is the coordinator's and not a
worker's (the worker deny list and the role check enforce that), a landing needs a recorded green
at a head containing `main`, and Air records the landings it performs. `air land` is offered to a
repo that has no lander, and offered is the whole of it.

### Find the repo's existing lease store, and collapse to one

**[adopter, 2026-08-23, air-uae]** Coexistence has one exception, and it is the only place
where leaving the repo's own machinery running is worse than retiring it: **two lease stores that
disagree deny work while reporting success.**

adopter ran both. `make lease-take` called `air lease take`, which writes the ledger, and
reported success; their PreToolUse guard read `$(git --git-common-dir)/ad-leases/runtime/`. So
`make api` was refused with *"take it first: `make lease-take`"* — naming the command that had
just succeeded. Every worker hit it, twice recorded (ad-3wnp, ad-gpj0, capture
`01M0NM3YSE05GTNPNGVDQB24FW`).

This is worse than an ordinary overlap because of its failure direction. A lease store that fails
to record leaves two agents in one file, which someone notices; a second store that disagrees
denies a command and tells the agent to run the command it just ran, which reads as a bug in the
agent. Nobody suspects the lock.

### Migrating a repo's own lease store to `air lease`

A repo keeping its own store needs none of this: `air lease` simply goes unused there and Air
records nothing about leases. **A fine outcome, not a loss.** What follows is for a repo that has
decided to switch, and it is written to be followed on cutover day without asking Air anything.

**Which store wins is not a preference. It is whichever one the guard reads**, because a store
nothing enforces is a record and the store that refuses commands is the lock. So the migration is
not "start calling `air lease take`" — that is what adopter already did, and it is precisely how
they ended up with two.

#### 1. Find every reader, not just the guard

Grep for **the lock path**, not for the guard, and not for the make target. The path is the one
thing every reader must name:

    $ git grep -n 'ad-leases'            # substitute the repo's lock directory
    $ git grep -rn 'git --git-common-dir'  # where lock paths are usually built

Expect more than one kind of hit, and treat a single hit as a sign you grepped the wrong string:

| pattern | adopter's instance |
|---|---|
| the guard that refuses | PreToolUse guard — but it *calls out* rather than reading the path itself |
| the script the guard calls | `scripts/lease.sh check`, which is the actual reader |
| the documented take/release commands | `make lease-take`, `make lease-status` |
| **targets that refuse on their own, inside the recipe** | `make reseed`, `make seed-demo` — they write over HTTP to a fixed port and would otherwise split a seed across two databases |

That last row is the one that makes this a procedure rather than a line. the adopter corrected an
earlier draft of this section that said "change the guard to read `air lease status --json`":
**it is not one call site**, and sizing it as one is how a cutover half-lands and leaves exactly
the two-store state it was meant to end.

#### 2. Drain the old store before switching, not after

Air's `leases` table starts empty. A lock held in the old store at the moment of cutover is
**invisible to Air**, so the resource it protects can be taken by a second agent immediately —
the two-store failure inverted, and worse, because now nothing refuses at all.

So, with the fleet stopped:

    $ ls -la "$(git --git-common-dir)"/<lock-dir>/*/     # every held resource, and its age

Release each one through the repo's own release path while it still works. Expect debris rather
than a clean list: adopter's directory held a `log` file last written days earlier with no lock
beside it, and nothing documented that this was normal. A file that is not a lock is not a held
lease; a lock whose holder is gone is released, not preserved. Then **move the directory aside**
(`mv <lock-dir> <lock-dir>.pre-air`) rather than deleting it, so a reader you missed in step 1
fails loudly instead of silently reading an empty store and permitting everything.

#### 3. Verify with a command whose output settles it

Two checks. The first is that Air's store is the only one anything names:

    $ air lease status
    no leases held
    lease store: /path/to/repo/.air/ledger.db (leases table)

    $ git grep -n '<old-lock-dir>'
    (no output outside documentation and history)

The second is the incident itself, reproduced and passing. This is the check that matters, because
the failure being prevented is a *gated command refusing after a successful take*:

    $ air lease take runtime --reason "cutover check"
    $ make <the-target-that-used-to-refuse>     # must RUN, not refuse
    $ air lease release runtime

If the gated command still refuses while `air lease status` shows the lease held, step 1 missed a
reader. That is the whole diagnostic, and it is the one adopter did not have: their disagreement
had to be inferred from a contradiction, because neither side ever said where it was looking.

**adopter is switching to `air lease`**, effective the next time Air is built there (owner,
2026-08-29). Their `make land` stays theirs — see the landing note above; this is the lease store
only. Note the ordering trap on their side and anyone's: `air lease status` only names its store
in a build that has that change, so a repo checking with an older binary sees nothing new and
concludes wrongly. A tool is only true where it is installed.

`air lease` itself stays. The 2026-08-24 audit proposed deleting it on zero rows in this repo's
ledger, and adopter's round contradicted that: a worker read `air lease status`, saw `runtime`
held by a peer, and took different work rather than routing around it (owner ruling 2026-08-29;
`decisions.md`). A verdict from an absence in one repo is not a verdict about a mechanism.

**adopter's repo is theirs to change.** Air's part is this procedure and saying where its own
store is; the collapse is their call, and the finding was sent to them rather than committed to
their tree.

## 3. Rules to change in the repo (prose that Air replaces or that is wrong)

| Today | Change to | Why |
|---|---|---|
| `bd update <id> --claim` in worker rules | `air claim <id> [--files a,b]`; release with `air release <id> --reason …` | The launcher denies raw `--claim`; Air keeps the claim history bd does not |
| `bd human <id>` (**[adopter]** CLAUDE.md tells agents to run it 4×) | Delete. It does not exist in bd 1.2.x; it prints help and no-ops | Replace with `air capture "<question>"`; the coordinator files a bead labelled `owner` with its recommendation, and those beads are the owner's queue (air-uef) |
| `make note` / a tracked intake file (**[adopter]** `intake.jsonl` dirtied main and blocked `make land`) | `air capture "<one line>"` | `.air/` is gitignored; capture never touches a tracked file |
| Workers file beads | Workers never run `bd create` (denied). Coordinator: `air inbox` → `bd create --validate --estimate <min>` → `air triage <id> --bead <new>` | bd `--validate` refuses a task/feature without `## Acceptance Criteria`, a bug without that and `## Steps to Reproduce`, an epic without `## Success Criteria` (roles.md has the sourced list) |
| Per-worktree `settings.local.json` env (`BEADS_ACTOR`, `CARGO_TARGET_DIR`) | Set nothing in files; `air worker <name>` passes `BEADS_ACTOR=<name>` and `AIR_ROLE` by flag. Add repo env the same way via `air worker <name> -- --settings '{"env":{…}}'` or put it in `.claude/air.json` (`worker_env`, when built) | **[adopter]** renamed worktrees kept old values: beads misattributed, `make test` built into another worktree's target dir, `make land` refused |
| `scripts/lease.sh` / `make lease-*` | `air lease take|release|status|break [<resource>]`; resources: `runtime` (ports, device, Docker), `:8080`, `simulator`, `chrome`, … | Same semantics (worktree identity, pid liveness, stale heartbeat), plus dead-holder attention pushed to the coordinator. Keep the make targets as aliases for one round |
| Heartbeat cron that wakes the coordinator | Delete it. `air coordinator` attaches the channel; conditions arrive when they hold | `air status --attention` is the same list on demand |
| `make fleet` (live agents, overlap) | Alias to `air status` / `air holdings` | One source; no drift between scripts |
| `make verify` run bare | `air record verify -- make verify` (also `fitness`, `docs-check`) | The gate needs the fact; Air flags suspicious (under 2 s, silent), changed-command, dirty-tree, and refuses backgrounded runs |
| Generated-files exclusion list duplicated in `land.sh` and `fleet.sh` **[adopter]** | One file sourced by both | Drift |
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
| "The owner merges every green branch at the end of the round" | Landing is the coordinator's, and a branch is landable when it carries a recorded green at a head containing `main`. **Which command does it stays the repo's** (air-97z): a repo with its own lander keeps it, and `air land <bead>` / `air land --all` is there for one that has none (air-3pz) |
| "Close the landed beads one by one" | `air close <id>… --reason "<why>"`: one `bd` process for the whole pass, and the matching claims released in one ledger transaction. `bd` costs ~1.4 s per process here whatever it is asked, so the count of processes IS the cost (air-869) |
| "Do not set awaiting_review without green" (advisory) | Refused, not advised: worker launches set `AIR_ENFORCE=1` and the hook denies the `bd` write, naming the fixing command (air-i59) |
| "Say which fleet a pane belongs to" | tmux sessions are `<project>-<worker>`: `tmux ls` is machine-wide, and with two fleets running it said nothing about which project a pane was (air-5lg) |
| "Say why the tracker feels slow" | Event lines carry `bd_ms`/`bd_calls` when the command shelled out to bd, and `air status` prints the median cost of one bd process (air-869) |
| "A bead awaiting the owner is labelled `human`" | The label is `owner`; `human` is presence and gates nothing. See §5b before upgrading a repo that used `human` (air-5hw) |

Delete the prose once the machinery is installed (CLAUDE.md rule: machinery over Markdown).

## 5. Keeping the integration current

- **Air version**: `air selftest` after every `cargo install`; every check proves it fires.
  `air doctor` shows the ledger version (schema migrates forward automatically). When Air's own
  surface has moved — new commands, changed `--json` shapes, a default that became a refusal —
  §5c is the checklist to run, §5a the detail behind it, and `air install` prints the diff.
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
- **bd's agent setup is not installed.** `air init` runs `bd init --skip-agents --skip-hooks`:
  no AGENTS.md, no `bd prime` SessionStart hook. `bd prime` injects a command reference that
  tells agents to `bd update --claim` and `bd create`, which Air denies; Air's roles text is
  the only agent-facing instruction. **[adopter]** remove the `bd prime --hook-json` hook
  from `.claude/settings.json`.
- **Air records friction it did not cause.** `PermissionDenied` and `PostToolUseFailure` hooks
  log, per worker, the tool, the command, and who or what refused, so the repo's own guards
  and declined prompts land in the same event stream as Air's.
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

## 5a. Upgrading an existing installation (Air's own surface moved)

§5 keeps the integration current against *bd* and *Claude Code* upgrades. This is the other
direction: **Air changed under a repo that already has it installed.** That happened for the
first time on 2026-08-22, when one round added two commands, changed a JSON shape, turned a
warning into a refusal, and redefined a label — under adopter, which had Air installed and
was told none of it (air-6g1).

**Running an upgrade rather than reading about one? §5c is the checklist, top to bottom.**
This section and §5b are the detail behind its steps.

### The one command

    air install            # dry run: prints the SURFACE DIFF, writes nothing, exits 0
    air install --write    # applies, and records that this repo has been told

`air install` was always a dry run. It now also answers "what moved since this repo last
installed Air": a list of surface changes, each with what to do about it, and `!!` against the
ones that **change behaviour without erroring** — the ones a repo discovers by getting a wrong
answer rather than a stack trace.

The baseline lives in `.air/installed.json`, written by `--write`:

```json
{ "air_version": "0.0.1", "installed_at": "…", "surface": ["land", "close", "…"] }
```

The diff is computed against those recorded **ids**, not against `air_version`. A version
string does not move on its own, so a comparison keyed to one reports nothing the first time
somebody forgets to bump it, which is the same silence this section exists to end. Adding an
entry to `SURFACE` in `crates/cli/src/cmd/install.rs` is the whole job: every repo installed
before it then sees it, and `air selftest` proves an older recorded surface produces a
non-empty diff and a current one produces nothing.

A repo with no `.air/installed.json` and no Air wiring is a **first install**, not an upgrade,
and gets no diff: nothing has changed under a repo that never had Air.

### What `air install --write` re-runs safely

| | |
|---|---|
| `.claude/settings.json` | Merges the `air hook` entries (`hook_entries()`; nine as of 2026-08-22). Idempotent, and other hooks, permissions and settings are preserved — proved by `merge_hooks_is_idempotent_and_preserves_others` |
| `.mcp.json` | Adds the `air` server; leaves other servers alone |
| `.air/roles.md` | **Overwritten** with the version embedded in the binary. Never hand-edit it; it is `include_str!` of `docs/rules/roles.md` and a test asserts the two are identical |
| `.claude/skills/air-*/SKILL.md` | **Overwritten**, same reason: coordinator procedures are versioned with `air` |
| `.air/installed.json` | Rewritten with the current surface |

### What it will never touch

`.claude/air.json` (yours: `digest_dir`, deny patterns), `.gitignore` (it advises, you edit),
the ledger and its events, anything under `.beads/`, and every other file in the repo. It also
refuses to write at all when the `air` on `PATH` is not the binary being run, so a stale
install cannot quietly wire a repo to a different Air.

### Order of operations for an upgrade

1. `cargo install --path crates/cli` in the Air checkout; `which air` must be that binary.
2. `air selftest` — every check proves it fires — then `air doctor`.
3. In the target's main checkout, `air install` and **read the surface diff**. Do the `!!`
   items first: those are the ones that are already wrong and not saying so.
4. `air install --write`.
5. Restart every agent session. Sessions started before the upgrade hold the old roles text
   and the old hooks; they do not pick it up.

### Anything the diff cannot know

The surface diff reports what Air changed. It cannot know what the *repo* built on top —
scripts parsing `air … --json`, make targets wrapping `air` commands, prose in CLAUDE.md
naming a flag. Grep for `air ` in the repo's Makefile, `scripts/`, and CLAUDE.md after every
upgrade; that is a judgement call Air does not have the standing to make.

## 5b. Migration: `human` → `owner` (a repo that used `human` as its gate)

Air's authority label is `owner` (2026-08-22, air-5hw). Two words, two meanings: `human` is
about **presence** (a person is in the loop and can watch and type into every session);
`owner` is about **authority** (a worker may not decide or finish this). `air claim` refuses a
bead labelled `owner`, and `human` now gates nothing.

**This is the dangerous one.** A repo that used `human` as its gate does not get an error when
it upgrades. Its owner queue simply stops being fenced: beads that were held back become
claimable, and workers start finishing decisions that were the owner's. the adopter is exactly
that repo — its `make ready` is `bd ready --exclude-label owner,runtime,human`
(`docs/research/adopter-as-built.md:91`, citing its `Makefile:395-411`), and its beads and
CLAUDE.md read `human` as the gate.

### The transition, in order

**Exclude both labels for the whole migration.** Nothing unfences mid-flight, and the order
below stops mattering:

    bd ready --exclude-label owner,runtime,human      # keep `human` here until step 4

1. **Makefile `ready` target** — already excludes both if it looks like adopter's. Leave it
   alone until the end. A repo excluding only `human` adds `owner` *first*, before anything
   else.
2. **CLAUDE.md label list** — document both: `owner` is the gate, `human` is being retired.
3. **Existing beads** — relabel. `bd list --label human --json` finds them; each one is either
   a real owner gate or was never a gate at all, which is the common case and the reason the
   word rotted. Several ids in one process:

       bd update <id> <id> … --add-label owner

   `--add-label` is repeatable and `--remove-label` is its companion (verified against bd
   1.2.2; `bd update [id...] [flags]`). `bd label add <id> <id> … owner` is equally valid and
   is *not* used here only because its argument order reads correctly either way to a skimmer.
   `-l` is **not** valid on `bd update` and is silently dropped — it belongs to `bd create`.
   One process rather than one per bead is the difference between a second and most of a
   minute on a 27-bead queue (air-869). adopter's own triage note has a category C
   for beads that "carry `human` but need no owner ruling — ordinary agent work"
   (`human-queue-triage.md`), and 27 of 152 beads in one 11-hour round carried
   `human`/`owner` (`adopter-as-built.md:204`).
4. **Only when `bd list --label human` is empty**: drop `human` from the exclude list and from
   CLAUDE.md.

### Do not skip step 3 by relabelling in bulk

`human` was applied to two different things. Copying every `human` onto `owner` moves the rot
across rather than clearing it, and makes the owner queue longer than it ever needed to be.
Read each one.

## 5c. The transition checklist (run this top to bottom)

For a repo already running an older Air. **This section is the order**; §5a explains what
`air install` does and §5b explains the label migration, but neither has to be read first.
Everything below is run by the owner, in the target repo, except where it says otherwise.

Written for adopter as the first customer (air-5tu). Every adopter-specific fact here is
cited from this repo's `docs/research/adopter-as-built.md`; nothing in this repo reads or
writes that fleet.

### Before anything: five checks, and what skipping each costs

These come first because `air install --write` cannot answer them and will not warn you.

Every command below has been run against bd 1.2.2 and its **argument order** checked, not only
its flag names. Those are two different claims, and a checklist is read under time pressure by
someone who will not notice that `bd label add owner <ids>` parses `owner` as the first issue
id — its real usage is `bd label add [issue-id...] [label]`, label **last**. Where a form reads
correctly either way to a skimmer, this section uses the one that names its argument
(`bd update <ids…> --add-label owner`) even when the other is also valid.
**[adopter, 2026-08-22]** that exact inversion was sent and caught before it ran; it would
have applied a bead id as a label to eight real beads.

**1. Does anything parse `air inbox --json` as a bare array?**

It now returns `{"captures": [...], "landings": [...]}` — with **or without** `--owner`. A
caller that indexes the top level as a list does not error. It reads zero captures and reports
an empty queue.

```sh
# in the target repo
grep -rn "air inbox" --include=Makefile --include=*.sh --include=*.py --include=*.js .
```

For each hit, look at what consumes the JSON: `[0]`, `.[]`, `len(...)`, `for x in ...`, or a
jq filter starting `.[]`. Each becomes `.captures[]` or `["captures"]`.

*Skipping it:* the owner queue silently reads empty. Decisions that were waiting stop being
reported, and nothing anywhere says so. **This is the only check here whose failure produces no
signal at all**, which is why it is first: 2 shows up as a bead being offered and then refused,
4 as a target going red, 3 as a visibly wrong `bd list`, 5 as agents told to run denied
commands. This one produces a shorter list and no error.

**2. Does the repo's ready target exclude `owner`?**

```sh
grep -n "exclude-label" Makefile
```

Check for the **new** label, not just that the old one is still there. A repo that never had
`owner` in its filter is the dangerous case, because its owner-fence rests entirely on a label
that nothing gates on after the upgrade.

**[adopter, 2026-08-22]** this is not hypothetical. `adopter-as-built.md:91` recorded
`--exclude-label owner,runtime,human` from its `Makefile:395-411`, but by the time of the
migration its `make ready` (`Makefile:464`) filtered `human,runtime,research` and excluded
`owner` **not at all**. Read the Makefile as it is now; a recorded reading from a previous
round is not the current state.

*Skipping it:* the moment Air's gate becomes `owner`, beads held back for the owner's decision
become claimable, and workers start finishing decisions that were never theirs.

*What actually degrades, measured 2026-08-22, and it is three surfaces rather than one:*
`air claim` refuses an `owner` bead as soon as the label is applied, so that fence works
immediately. Raw `bd ready` lists the unfenced beads, because the exclusion lives in the
repo's own target and not in bd. And the Stop hook's offer list **does not degrade at all** if
`.air/ready.json` predates the relabel: the hook reads that cache rather than bd, and a stale
cache is still offered, annotated `(ready list may be stale)` — verified in
`crates/hooks/src/gate.rs`, `stop_nudge`. So a worker can be handed an `owner` bead by the
Stop hook and refused by `air claim` in the same minute, which reads as Air contradicting
itself and is really one stale file.

**3. Which `human` beads are real owner gates?**

```sh
bd list --label human --json
```

Read each one. adopter's own triage note records a whole category that "carries `human` but
needs no owner ruling — ordinary agent work", and 27 of 152 beads in one 11-hour round carried
`human`/`owner` (`adopter-as-built.md:204`).

*Skipping it:* relabelling in bulk moves the rot onto `owner` instead of clearing it, and the
owner queue stays permanently longer than it needs to be.

**4. Does the repo's own label vocabulary include `owner`?**

Wherever the repo documents its labels — an intake guide, CONTRIBUTING, CLAUDE.md, a lint
fixture:

```sh
grep -rn "runtime" --include=*.md docs/ CLAUDE.md 2>/dev/null   # find the list, whatever it is called
```

Add `owner` to it **before** relabelling any bead.

*The symptom, so it is recognisable when it happens:* a fitness, lint or docs-check target
starts failing with **"undocumented label"** on several beads at once, immediately after a
relabel that itself looks fine. It is a green-to-red with nothing to do with the labels'
meaning.

**[adopter, 2026-08-22]** exactly this. It had *retired* `owner` from its documented list on
2026-08-21 when it consolidated on `human`, so adding `owner` to nine beads broke
`make fitness` and produced four undocumented-label failures at once. Fixed by putting `owner`
back in its `docs/guides/intake.md`.

**5. Is the stale `bd prime --hook-json` hook still in `.claude/settings.json`?**

```sh
grep -n "bd prime" .claude/settings.json
```

adopter had exactly one hook, `SessionStart → bd prime --hook-json`
(`adopter-as-built.md:50`). `air install --write` **merges**, so it adds Air's hooks
alongside that one and leaves it in place — it will not remove it and does not report it.
Delete the `bd prime` entry by hand.

*Skipping it:* `bd prime` injects a command reference telling agents to run `bd update --claim`
and `bd create`, both of which Air denies. Agents get instructions that contradict their deny
list, and the failure looks like the agent being wrong.

### The run, in order

1. **In the Air checkout**, not the target: `cargo install --path crates/cli`, then
   `which air` — it must be that binary. `air install --write` refuses if it is not.
2. `air selftest` — every check proves it fires — then `air doctor`.
3. Do the five checks above. Fix 2, 4 and 5 now; 1 can be fixed now or immediately after; 3
   runs across the migration in step 7.
4. **In the target repo**, `air install`. It writes nothing. Read the SURFACE DIFF: it lists
   what moved since this repo last installed Air, with `!!` against changes that alter
   behaviour without erroring. Do those first.
5. `air install --write`. This records the baseline, so the next `air install` is quiet.
6. **Restart every agent session** through `air coordinator` / `air worker <name>`. Sessions
   started before the upgrade hold the old roles text and the old hooks and do not pick the
   new ones up.
7. Run the `human` → `owner` migration (§5b). Keep **both** labels excluded from `make ready`
   for the whole migration so nothing unfences mid-flight; drop `human` only when
   `bd list --label human` is empty.

### Verify afterwards

```sh
bd list --label owner --json          # the gate's beads
air status                            # sessions, claims, review waits, owner queue
air audit                             # every mechanism, and what it costs
```

Three things to confirm:

- **The owner queue is still fenced.** `air claim <an owner-labelled bead>` as a worker is
  refused, naming the label. If it succeeds, step 7 is incomplete.
- **The ready set is unchanged** except for beads you deliberately relabelled. Compare
  `make ready` against what you noted in check 2.
- **The owner queue is not empty by accident.** The owner-labelled count on the `ready:` line
  of `air status` and, if any script reads it, that script's output too. This is check 1
  coming back to be confirmed rather than assumed.

### If it goes wrong

`air install --write` writes exactly five things and nothing else: it merges into
`.claude/settings.json` and `.mcp.json`, and overwrites `.air/roles.md`,
`.claude/skills/air-*/SKILL.md`, and `.air/installed.json` (verified against
`crates/cli/src/cmd/install.rs`, 2026-08-22; §5a has the table). It never touches
`.claude/air.json`, the ledger, `.beads/`, or any other file.

Of those, the two JSON files and the skills are tracked, so `git diff` shows precisely what
changed and `git checkout --` reverts it; `.air/` is gitignored and holds nothing you would
want back. Nothing here needs an uninstall path.

## 6. Day one, in order

`air coordinator` in the main terminal. `air worker <name>` per worktree terminal (re-enters an
existing worktree). Workers: `bd ready` → `air claim` → work → `git merge main` →
`air record verify -- <cmd>` → `air handover` → `bd update -s awaiting_review`. Coordinator:
reads `air status`, acts on channel events, triages every `air inbox` capture into a bead
(labelled `owner` when the decision is the owner's), and lands by whatever path this repo lands by — its own `make land`, or
`air land <bead>` / `air land --all` where there is none (air-3pz, air-97z). Upgrading a repo
that already has Air: §5a.
