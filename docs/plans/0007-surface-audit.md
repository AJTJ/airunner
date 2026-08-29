# 0007 — Air surface inventory

**What this is**: the complete list of everything Air ships, with, for each item, what it does,
**why it exists**, what the Claude Code harness now offers instead, what prior art exists, and a
verdict. It is a living document: re-read it before adding anything, update it when anything
lands, and re-run the counts on every refresh.

**Why it exists**: on 2026-08-24 a first pass over this same question missed the beads integration
almost entirely, missed the skill installer, missed the surface-change notices, and reported an
attention-condition count that turned out to mean something different from what was claimed
(§11). The owner asked for a durable inventory reviewed several times rather than a summary
written once.

**How to regenerate every number here** (about ten minutes):

    air --help                      # command surface
    air audit --since <first-day>   # mechanism firings over the whole ledger
    air doctor                      # ledger location, tables, row counts, bd pin
    air selftest                    # probe list and count
    find crates -name '*.rs' | xargs wc -l | sort -rn      # module sizes
    grep -rhoE 'Command::new\([^)]*\)' crates/*/src crates/*/src/cmd | sort | uniq -c
    grep -rhoE '(AIR|BEADS|CLAUDE)[A-Z_]*' crates/ | sort | uniq -c | sort -rn
    du -sh .air/events                                     # event-stream growth

**Counts as of 2026-08-25** (ledger from 2026-08-15): 21 CLI commands, 4 crates, 12,434 lines in
the CLI crate, 11 ledger tables at schema v10, 9 hook events, 13 MCP tools, 6 MCP resources,
**0 MCP prompts despite a code comment claiming three** (§11), 25 environment variables, 14
registered mechanisms, 32 selftest probes (all green), 34 skills in `.claude/skills/`. Ledger rows: 70 verify runs, 112 edit-journal, 45 claims, 37 captures,
6 landings, 3 sessions, **0 leases**. Event stream: about 3 MB per day, no collection.

**Review log**
- Pass 1 (2026-08-24): commands and mechanisms only. Missed §6 entirely.
- Pass 2 (2026-08-25): added ledger schema, hooks, MCP surface, integrations, env vars, probes.
  Found the skill installer, the surface-change notices, the bd cache, and the correction in §11.
- Pass 3 (2026-08-25): verdicts re-checked against the corrected attention numbers.
- Pass 4 (2026-08-25): every count re-derived from the code rather than from pass 2's prose. Three
  errors found and fixed (§11); five stale cross-references to a command that does not exist were
  cleaned up. Lesson: re-reading the document finds nothing; re-running the commands finds errors.
- Pass 6 (2026-08-25): deep-research results folded into §10.5 and §10.6; the herdr worktree error
  corrected (§11); herdr's licence verified from the LICENSE file rather than a badge.
- Pass 5 (2026-08-25): herdr read first-hand (§10.5) and the landscape entry rewritten; §6.2b added
  after a module-by-module walk found `attribution.rs` and `acceptance.rs` missing from every
  earlier pass, which is two more of the "integrations you missed" the owner predicted.

---

## 1. CLI commands (21)

Evidence-keeping, in dependency order:

- **`record`** — run a command, record its exit against the current HEAD, with duration, output
  bytes and a dirty flag. 70 rows. The unit everything else consults.
- **`handover`** — the one gate: four checks (recorded green at HEAD, main merged, claim held,
  digest present when configured). Advisory unless `--enforce`/`AIR_ENFORCE=1`.
- **`claim` / `release`** — atomic claim through bd plus a ledger row; release returns the bead to
  open and closes the claim with a reason. 45 claims recorded, 2 refusals.
- **`close`** — coordinator-only batched close: N beads in one bd process and one ledger
  transaction. Exists solely because bd costs about 1.4 s per process (air-869).
- **`land`** — merge a green hand-over into main, verify the merged result, rewind main if red,
  and report each bead's acceptance clauses against what the merge actually did. 6 landings.
- **`holdings`** — who has edits in which files across worktrees: uncommitted, committed,
  journaled.
- **`status`** — the coordinator's screen and the attention conditions; `--attention` for the
  conditions alone. The largest single module (1,634 lines).
- **`audit`** — what the ledger says about every registered mechanism: fires, subjects, repeats,
  last fired, removal condition, and whether the ledger can check that condition.
- **`doctor`** — ledger path, size, journal mode, schema version, row counts, bd version pin, and
  a live `bd list --json` probe.
- **`selftest`** — 32 red/green probes; a probe that matches nothing prints red.

`bd_latency` is **not** a command, despite pass 1 listing it as one: it is a module
(`crates/cli/src/cmd/bd_latency.rs`) that recomputes the median cost of one bd process from the
event log for `status` and `audit` to print. Correction recorded in §11.

Work intake and fleet:

- **`capture` / `inbox` / `triage`** — one line into a durable queue, oldest-first reading, and
  resolution into a bead (checked against bd first) or a drop with a reason. 37 captures.
- **`lease`** — `take|release|status|break|beat` for what two agents cannot share: ports,
  simulator, Docker, browser. **0 rows in this repo's entire ledger.**
- **`worker` / `coordinator`** — interactive Claude Code sessions with role, deny list, env, and
  worktree; detached tmux start when there is no tty.
- **`mcp`** — the stdio MCP server: tools, resources, and the push channel's poll thread.
- **`hook`** — the Claude Code hook entrypoint, one event line per invocation.
- **`init` / `install`** — give a project everything Air needs; wire Air into an existing repo.
  Dry run by default.

## 2. Crates

- **`crates/ledger`** (1,985 lines) — SQLite WAL at the main checkout plus NDJSON events.
  Modules: `schema`, `verify`, `claims`, `captures`, `landings`, `leases`, `events`, `paths`.
- **`crates/hooks`** (559) — hook I/O types, the pure hand-over gate, the edit journal, the stop
  nudge. Pure logic, no I/O, so the gate is testable without a repo.
- **`crates/bd`** (326) — the `WorkLedger` trait and its `bd --json` shell-out, plus latency stats.
- **`crates/cli`** (12,434) — everything above. Largest modules: `status` 1,634, `selftest` 1,482,
  `hook` 1,034, `mcp` 762, `land` 682, `audit` 645, `install` 631, `acceptance` 566, `launch` 555,
  `claim` 546.

## 3. Ledger (schema v10, 11 tables)

`verify_runs` (worker, sha, command, exit, duration_ms, output_bytes, dirty) · `edit_journal` ·
`claims` · `sessions` (role, pid, project) · `landings` (open_beads) · `captures` (status,
audience) · `leases` · `lease_wants` · `hook_emissions` (session_id, key, fingerprint: the
once-per-session suppression) · `conditions` (worker, kind, first_seen, last_seen, cleared_at: an
open row means the condition holds now) · `bd_cache` (last good bd answers so a slow bd degrades to
stale counts rather than an empty screen, after adopter saw 20 s bd calls, air-19u).

Forward-only migrations keyed by `PRAGMA user_version`. **`air doctor` reports 7 of the 11 tables**;
`hook_emissions`, `conditions`, `lease_wants` and `bd_cache` are invisible to it, which is how the
zero-lease finding nearly went unnoticed. Fix or note.

Event stream: `.air/events/YYYY-MM-DD.ndjson`, one line per hook invocation and per command, each
stamped with `bd_ms`/`bd_calls` when that command shelled out to bd. **About 3 MB per day** and
nothing collects it; `gc` is still on the roadmap.

### Daily growth, before and after air-5uz (measured 2026-08-29)

Counted over every recorded day in `.air/events`, then replayed through the change-only rule the
same way the code now applies it: a `status.attention` line is written when the condition SET
differs from the last one written, and an empty set clears the emission row.

| day | lines | bytes | of which `status.attention` | sets that actually changed | lines after | bytes after |
|---|---|---|---|---|---|---|
| 2026-08-22 | 8,691 | 3,084,158 | 2,912 | 60 | 5,839 | 1,648,176 |
| 2026-08-23 | 7,624 | 3,417,881 | 7,573 | 1 | 52 | 12,562 |
| 2026-08-24 | 6,176 | 2,708,804 | 6,015 | 1 | 162 | 38,965 |
| 2026-08-25 | 8,242 | 3,588,236 | 7,667 | 1 | 576 | 139,109 |
| 2026-08-26 | 2,279 | 1,025,385 | 2,276 | 1 | 4 | 1,160 |
| 2026-08-29 | 1,165 | 364,939 | 441 | 1 | 725 | 204,110 |

**Before**: 2.77 MB per active day (13.83 MB over the five days of 08-22 to 08-26).
**After**: 0.37 MB per active day (1.84 MB over the same five). 87% less, and 78% fewer lines
across the whole stream.

The row worth reading twice is 2026-08-23: **7,573 lines carrying one distinct condition set all
day**, 3.42 MB to say one thing. That is what `air audit` was counting as firings, and the reason
`owner-decision-waiting` read as 1,685 against a single push. 2026-08-22 is the only day whose
residue is large, and it is large because that day's remaining lines are hook traffic, which this
change does not touch.

**`air gc` took its window from the after figure** (air-i7s, 2026-08-29): 90 days, about 33 MB
at 0.37 MB per active day. The same window against the pre-fix 2.77 MB would have been 250 MB.
`air doctor` states the retention and what is collectable under it.

### What the channel poll costs, re-measured after air-5uz (2026-08-29, for air-djl)

air-5uz cut what the poll **writes**. It did not touch what the poll **runs**: every tick still
calls `status::gather`, which shells out to bd. Both halves are countable from the log, because
each line carries `inputs.duration_ms` and the process-cumulative `bd_calls`/`bd_ms`.

| day | poll ticks | pollers | median gap | median `gather` | bd calls | time waiting on bd |
|---|---|---|---|---|---|---|
| 2026-08-22 | 2,912 | 5 | 7.6 s | 3,832 ms | 704 | 0.32 h |
| 2026-08-23 | 7,573 | 3 | 8.2 s | 4,013 ms | 5,243 | 2.23 h |
| 2026-08-24 | 6,015 | 3 | 8.8 s | 3,852 ms | 2,011 | 0.78 h |
| 2026-08-25 | 7,667 | 3 | 8.6 s | 3,895 ms | 5,674 | 2.26 h |
| 2026-08-26 | 2,276 | 3 | 7.4 s | 3,982 ms | 1,572 | 0.66 h |

**The stated grounds for deleting the poll are gone, and a larger cost is in their place.**
0008 item 3 says the poll "writes about 3 MB of events a day to convey 45 pushes". After air-5uz
it writes 1 to 60 lines a day. What it actually costs is **~5,700 bd calls and ~2.3 hours of
waiting on bd per day**, at a median `gather` of 3.9 s, running around the clock whether or not
anyone is watching.

Where the bd calls come from: `gather` calls `in_progress`, then `show` once **per open claim**,
then `awaiting_review`, then `ready` (`status.rs:837-967`). Only one attention condition needs
any of it — `idle-without-claim` reads `ready_depth` — plus `review-waiting`, which is derived
from bd's `awaiting_review` and is dead in this repo since close-with-proof. Everything else is
ledger and git.

### After air-cmn (2026-08-29): bd out of the poll path

The poll now asks for `BdUse::CachedFor(10)` and answers from `bd_cache` while the cached counts
are under ten minutes old. Nothing new catches the fallback: `bd_cache` and the "answer from the
cache" arms already existed for a slow bd (air-19u), so this arms a tested path deliberately
instead of adding a second one. `air status` says which source it used, because "0 ready" from a
cache and "0 ready" from bd are different facts.

**Measured before** (the table above, 2026-08-25 as the representative day): 7,667 ticks across
3 pollers, 5,674 bd calls, 2.26 h waiting on bd, median `gather` 3,895 ms.

**Projected after, with the arithmetic shown rather than a number asserted.** Each poller ticks
about every 34 s (2,556 ticks per poller per day) and may now pay for bd once per 10 min, so 144
of those 2,556 ticks pay: **5.6%**. Applying that to the measured before-figures gives roughly
**320 bd calls and 7.6 minutes a day** waiting on bd, from 5,674 and 2.26 h.

**This projection is not the measurement the bead asks for, and must not be read as one.** The
real after-figure exists only once a coordinator has run this build for a day. Read it then, the
same way the before-figure was read:

    python3 - <<'EOF'   # difference bd_calls per (day, poller); never sum them
    ...  see the method note below
    EOF

and replace this paragraph with the observed numbers.

**Method note, because the number that is easy to get here is wrong.** `bd_calls` and `bd_ms` on
an event line are `air_bd::stats::snapshot()`, which is cumulative for the life of the process
(`crates/bd/src/lib.rs:52-64`). They must be differenced per (day, poller), never summed.
Summing them gives 12.4 million bd calls for 2026-08-25, which is nonsense and reads exactly
like a measurement. This is air-21c again, inside the bead that exists to fix air-21c.

## 4. Hook events (9)

Installed into `.claude/settings.json` as `air hook` with a 5 s timeout, merged idempotently:
`SessionStart`, `PreToolUse(Edit|Write|MultiEdit|Bash)`, `PostToolUse(Edit|Write|MultiEdit|Bash)`,
`PermissionRequest`, `PermissionDenied`, `PostToolUseFailure`, `Stop`, `SubagentStop`, `SessionEnd`.

What they do: journal touched files (PostToolUse); warn once per session when a peer is journaled
in the same file (PreToolUse, via `hook_emissions`); **run the gate and refuse `bd close` or
`bd update -s awaiting_review|closed` without a recorded green** (PreToolUse on Bash); deny a tmux
command naming another project's session; nudge once at WIP 0 with ready beads (Stop); record
session state, role and pid (SessionStart/End, PermissionRequest, PostToolUse). Fails open, about
100 ms, one event line per invocation.

## 5. MCP surface

Tools (13): `air_status`, `air_attention`, `air_holdings`, `air_handover`, `air_claim`,
`air_release`, `air_capture`, `air_inbox`, `air_triage`, `air_close`, `air_lease_take`,
`air_lease_release`, `air_lease_status`.
Resources (6): `air://status`, `air://attention`, `air://inbox`, `air://holdings`,
`air://owner-queue`, `air://leases`.
Prompts: **none**. `prompts/list` returns an empty array, and a selftest probe asserts that it is
empty. A comment in `install.rs` says the three embedded skills are "served as MCP prompts by
`air mcp`"; they are not. Either wire them or delete the sentence (§11).
Channel: a poll thread re-evaluates `status::attention` from the ledger every
`AIR_CHANNEL_POLL_SECS` and pushes **only on change** (`hook_emissions`-style suppression, proved
by two selftest probes). See §11 before quoting any number about this.

## 6. Integrations (the section pass 1 missed)

### 6.1 beads / `bd` — much deeper than "Air wraps claims"

Air pins **bd 1.2.2** and refuses to run against another version (`doctor::BD_PINNED`); 1.2.1
corrupted the Dolt schema during the adopter adoption on 2026-08-21.

- **Trait boundary**: `WorkLedger` in `crates/bd` with `ready`, `in_progress`, `by_status`, `show`,
  `show_all`, `claim`, `set_status`, `comment`, `close_all`. Every call shells out to `bd` with a
  timeout (`AIR_BD_TIMEOUT_MS`, `AIR_BD_PROBE_TIMEOUT_MS`, `AIR_NUDGE_BD_TIMEOUT_MS`) and drains
  the child so a hung bd cannot wedge a hook.
- **Exact argv used**: `bd ready --json`, `bd list --status <s> --json`, `bd show <id> --json`,
  `bd update <id> --claim --actor <a>`, `bd update <id> -s <status>`, `bd comment <id> <text>`,
  and a batched `bd close <id> <id> … --reason <r> --actor <a>` built by `close_argv`.
- **Cost, measured**: about **1.4 s per bd process** whatever it is asked (air-869).
  `air_bd::stats` records `bd_ms`/`bd_calls` onto every event line, and the `bd_latency` module
  recomputes the median from the event log, so the number stays a measurement rather than a claim
  in a doc.
- **Two caches exist because of that cost**: the `bd_cache` table holds the last good answers for
  `awaiting_review` and `ready_depth` so `air status` degrades to stale counts instead of an empty
  screen; and `.air/ready.json` plus `ready_cache.rs` hold the claimable ready ids for the Stop
  nudge.
- **Where bd is called from**: `claim`, `release`, `close`, `triage` (every bead id is checked
  against bd before a capture is passed, and an id bd does not have refuses the pass), `land`
  (beads named in the merge range are confirmed against bd), `status` (in_progress, show,
  by_status `awaiting_review`, ready), `doctor` (version pin plus a live probe), `init` (`bd init`).
  **Never from a hook.**
- **Identity**: `BEADS_ACTOR` is injected into the worker session's `settings.env` by the launcher,
  so bd attributes the claim to the worker name rather than to whoever is logged in.
- **The gate's subject**: `is_handover_command` matches `bd close` and `bd update -s
  awaiting_review|closed` on the PreToolUse Bash path. Beads is not a side integration; it is the
  thing the refusal is attached to.
- **Deliberately not installed**: the `beads` skill is excluded from the installer because its
  frontmatter describes the bd 1.2.1 surface while Air pins 1.2.2. Shipping it would document the
  version Air refuses (air-ha8).

### 6.2 Claude Code

- **Hooks**: nine events merged idempotently into `.claude/settings.json` (§4).
- **MCP registration**: the `air` server merged into `<repo>/.mcp.json` the same way.
- **Launcher**: `claude` invoked with `--worktree <name>`, optionally `--tmux[=classic]`, the role
  prose appended, the deny list applied, and env set. `AIR_CLAUDE_BIN` overrides the binary.
- **Deny list**: `.claude/air.json` with `coordinator_deny` and `worker_deny`. **Currently holds
  one pattern**, `Bash(cargo publish *)`.
- **Skills**: `air install` embeds three of Air's own skills into the target repo, renaming the
  frontmatter to `air-do-less`, `air-decomposition`, `air-phase-transitions` so they cannot collide
  with the repo's own. A comment says the same text is served as MCP prompts; it is not (§5, §11).
- **Roles**: `.air/roles.md` written from a copy embedded in the binary, so role text is versioned
  with `air` rather than drifting per worktree.
- **Surface-change notices**: `installed.json` records which surface a repo last adopted, and
  `surface_diff` prints what changed since, with a headline and an action per change. An adopting
  repo is told, for example, that a worker without a recorded green is now denied the bd write.
- **`.gitignore`**: advises when `.air/` is not ignored and **does not edit the file**.

### 6.2b Git as a data source, not just a VCS

Two modules read structured facts out of git, and both exist because reading prose failed:

- **`attribution.rs`** (260 lines) — which beads a branch carries. Every commit doing a bead's
  work carries a repeatable **`Bead: <id>` commit trailer**; Air reads trailers and refuses to read
  prose. The recorded failure (air-4re): `air land` used to scan commit messages for anything
  shaped like an id, but ids appear both because a commit *did* that bead and because someone
  mentioned it ("builds on air-3pz"), and nothing in the text separates the two. air-7kp piled an
  authorship filter and a branch-point time bound on top of the scan and the count on one real
  branch went 8 → 6 → 3. `AIR_BEAD_TRAILER_SINCE` / `FALLBACK_BEFORE` (2026-08-23) is the dated
  cutover so older branches still resolve by the old path.
- **`acceptance.rs`** (566 lines) — what a landing may close versus only land. From a measurement
  on adopter's own closer: **99 of 532 beads (18.6%) closed on branch containment alone**,
  never reading acceptance criteria, and accelerating to 84 of the last 172 closes (48.8%) over
  two days. Verdicts on those 99: 82 done, 14 partial, 1 not done, 1 unverifiable, 1 moot. The
  misses were not random; they were beads with a clause the merging agent could not discharge
  itself (an owner decision, an owner-only config write, a deploy). Air discharges a clause only
  by lookup (a green recorded at the landed sha, or a path the merge changed) and reports
  everything else as unreadable rather than judging it. A clause the merge CONTRADICTS is kept on
  the landings row and named by `air status`.

Both are the `anti-brittleness` skill's subject in production: a structured field the writer
declares, not a fact inferred from how a sentence was phrased. They are also the strongest
individual pieces of evidence in the repo, because each carries a counted failure.

### 6.3 Everything else Air shells out to

`git` (14 call sites: `rev-parse`, `status`, `merge-base`, `worktree`, `log`, `show`, `diff`,
`merge`, `reset`, `config`, `init`), `tmux` (14 call sites: detached sessions named
`<project>-<name>`, `AIR_TMUX_SOCKET` for `tmux -L`, `AIR_TMUX_MODE`), `sh` (running the repo's
verify command), `ps` (session liveness behind gone-with-claim), and `bd`.

### 6.4 Files Air touches in a target repo

`.claude/settings.json` (hooks, merged) · `.claude/air.json` (deny patterns, from a scan) ·
`.claude/skills/air-*` (three embedded skills) · `.mcp.json` (the air server, merged) ·
`.air/` (ledger, events, `roles.md`, `ready.json`, `installed.json`) · `.beads/` (via `bd init` in
`air init`) · `.gitignore` (advice only). Dry run by default; `--write` to act.

## 7. Environment variables (22)

Exactly 25, from `grep -rhoE '"(AIR|BEADS|CLAUDE)_[A-Z_]+"' crates/ | sort -u`:
`AIR_ATTENTION_IDLE_MIN`, `AIR_ATTENTION_IDLE_NOCLAIM_MIN`, `AIR_ATTENTION_LAUNCH_GRACE_MIN`,
`AIR_ATTENTION_SILENT_MIN`, `AIR_ATTENTION_STUCK_MIN`, `AIR_BD_BIN`, `AIR_BD_PROBE_TIMEOUT_MS`,
`AIR_BD_TIMEOUT_MS`, `AIR_BEAD_TRAILER_SINCE`, `AIR_CHANNEL_POLL_MS`, `AIR_CHANNEL_POLL_SECS`,
`AIR_CHANNELS_FLAG`, `AIR_CLAUDE_BIN`, `AIR_DIGEST_FRONTMATTER_SINCE`, `AIR_ENFORCE`,
`AIR_LEASE_PID`, `AIR_LEASE_STALE_SECS`, `AIR_NUDGE_BD_TIMEOUT_MS`, `AIR_PROJECT`, `AIR_ROLE`,
`AIR_TMUX_MODE`, `AIR_TMUX_SOCKET`, `BEADS_ACTOR`, `CLAUDE_PID`, `CLAUDE_SESSION_ID`.
Five of them are attention thresholds, which is a lot of tuning surface for conditions that
fire on two subjects.

The two `*_SINCE` variables are dated cutovers: a rule that only applies to work after a date, so
old digests and commits are not retroactively judged. That pattern is worth keeping.

## 8. Registered mechanisms (14) and what the ledger says

Over 2026-08-15 to 2026-08-24 (`air audit --since 2026-08-15`), as **event-log firings**, not as
things a human saw (§11):

| mechanism | class | fires | subjects | repeats |
|---|---|---|---|---|
| handover-gate | refusal | 1 | 1 | 0 |
| handover-would-refuse | refusal | 10 | 9 | 1 |
| handover-not-green | attention | 19 | 3 | 16 |
| review-waiting | attention | 8,272 | 23 | 8,249 |
| idle-without-claim | attention | 25,958 | 2 | 25,956 |
| owner-decision-waiting | attention | 14,382 | 1 | 14,381 |
| stuck | attention | 0 | 0 | 0 |
| landed-not-closed | attention | 0 | 0 | 0 |
| cross-project-fence | refusal | 0 | 0 | 0 |
| peer-warning | warning | 33 | 32 | 1 |
| stop-nudge | nudge | 15 | 2 | 13 |
| claim-refusal | refusal | 2 | 2 | 0 |
| coordinator-send-keys | refusal | 4 | 2 | 2 |
| audit | report | 22 | 2 | 20 |

Gate cost: 70 verify runs over 68 distinct commits, 2 repeats, 30 closes passed = 2.33 verify runs
per close.

## 9. Probes and skills

32 selftest probes, all green on 2026-08-25, each red/green (the red arm proves the check can
fail). They cover the gate, the claim path, the channel's repeat suppression, the launcher's
detached path, the close batching, triage's bd check, land's refusals and acceptance reporting,
the project fence, and the audit registry's own integrity (a mechanism with no removal condition
is a defect; a firing with no registry row is a defect).

34 skills in `.claude/skills/`, indexed in `PROVENANCE.md`, three of them shipped to target repos.

## 10. Verdicts

### 10.1 Keep: the evidence core

`record`, `handover` + `crates/hooks` + `air hook`, `crates/ledger`, `claim`/`release`, `land` +
`acceptance`, `audit` + `mechanisms`, `selftest`, `doctor`, the `bd_latency` module, `holdings` + the
peer-warning hook.

Why these survive: each either writes the ledger or reads it to refuse something. Prior art gets
close on the run level (loki-mode's Evidence Receipts, bernstein's signed receipts and HMAC audit
chain, MartinLoop's `VERIFIED`/`NEEDS REVIEW`, IM.codes' `PASS`/`REWORK`/`BLOCKED`) but none
attaches the refusal to a **durable tracked work item** in an **interactive** session, and none
requires the green to sit at a HEAD that already contains main.

`peer-warning` is the model the rest should copy: 33 fires over 32 distinct subjects, 1 repeat.
Speak once per real change.

### 10.2 Replace: the harness does this now

**Launchers** (`launch.rs` 555 + `tmux.rs` 226). Claude Code 2.1.241 ships `-w/--worktree [name]`,
`--tmux[=classic]`, `--agent`/`--agents`, `--append-system-prompt`, `--disallowed-tools`,
`--settings` and an `env` block. Air's wrapper adds role prose, a deny list holding one pattern,
four env vars, worker-name allocation, and a detached start. Replacement: `.claude/agents/worker.md`
plus `permissions.deny` and `env` in `settings.json`, launched as
`claude -w <name> --tmux --agent worker`. **Keep the detached path**; it has no first-party
equivalent. Prior art if isolation ever needs to be stronger than a worktree: scion (Google,
26 contributors, container per agent, harness-agnostic).

**The channel's poll thread**, on the corrected grounds in §11: not because the coordinator is
flooded (it is not; the push dedupes) but because re-evaluating from the ledger on a timer writes
about 3 MB of events a day to convey 26 facts, and nothing collects it. Replacements available
first-party: `SendMessage` with `notify_when_idle: true` (a one-shot idle subscription per session,
with explicit guidance never to poll instead) and `Monitor` for event streams. Keep
`air status --attention` as the query; keep the conditions; stop writing an event per evaluation.

**`install`/`init` packaging** — a Claude Code **plugin** is the first-party way to ship hooks,
skills and an MCP server into a repo. Not urgent; revisit once §10.2 deletions land. Keep the
surface-change notices whatever happens: nothing found in 186 projects tells an adopting repo what
changed since its last adoption.

### 10.3 Prove or delete

**`air lease`** (368 lines, plus two ledger tables and three MCP tools) — **0 rows in this repo's
entire ledger**. Check the adopter ledger before deleting; if it is also zero, delete. If
resource contention ever becomes real, read Zaivern Code (line-range leases in a per-repo ledger,
git hooks refusing colliding writes, with published numbers) and wit (Tree-sitter symbol locks)
first.

**`cross-project-fence`** — 0 firings ever, but its own removal condition asks for a quarter of
silence and it has had ten days. Keep until November, then delete on the recorded condition.

**`capture`/`inbox`/`triage`** — keep. 37 captures is small but the queue is durable and
`SendMessage` is not. The problem was never the queue.

### 10.4 beads: keep

Raised by the owner after pass 1 listed replacements without mentioning the task store.

`bd` is what the gate refuses **on**. The measured cost is about 1.4 s per process, already worked
around by `air close`'s batching and two caches. Three projects in the 186 converged on beads
(**orc** uses the tool itself with the same coordinator-plus-worker-per-bead shape, **gastown** uses
git-backed beads tracking, **LoopTroop** implements the methodology without the tool), which is
evidence for the choice rather than against it. The only credible swap is **guild** (Go binary,
SQLite, quests with atomic claims and dependencies, served over MCP, no per-call process cost), and
GitHub Issues is the option if work ever needs to be visible off this machine.

Revisit if: the 1.4 s becomes a per-hook cost rather than per-command; guild grows past 8
contributors; or the work needs an off-machine surface. Air's own exposure is small because the
gate is keyed to an id and a sha, which any store can provide.

### 10.5 herdr: the fleet layer to rent

Checked first-hand 2026-08-25 after the owner asked whether it had been looked at. It had not, in
any real sense: the landscape sweep took it from a roster line and filed it as Commodity. That was
wrong.

**herdrdev/herdr** (the roster's `ogulcancelik/herdr` redirects): 32,266★, Rust, Apache-2.0,
created 2026-03-27, pushed 2026-08-24, **77 contributors**, v0.8.2 released 2026-08-19 with daily
preview builds. On the health bar in the landscape doc §2.5 it is stronger than every other
neighbour by an order of magnitude: tutti has 3 contributors, orc has 1, Zaivern 2, MartinLoop 4.

What it is: a terminal workspace manager (workspaces → tabs → panes) keeping real processes alive
across detach, with agents classified `blocked`/`working`/`done`/`idle`/`unknown` across 16+ agent
CLIs, and a **socket API** over newline-delimited JSON: pane and workspace lifecycle,
`list/inspect/read/prompt/wait on/start/attach` for agents, **`herdr agent wait <pane> --until
done`**, **long-lived event subscriptions** for agent-status changes, notifications, input
injection, and **`pane.report_agent`** for an integration to report its own semantic state into
herdr.

What it does **not** have, confirmed across the ~75-method socket API, the full changelog (0.1.0
2026-03-27 through 0.8.2 2026-08-19), the docs index and `AGENTS.md`: no task store, no queue, no
ticket tracking, no verification-run recording, no commit-bound evidence, no refusal primitive, no
MCP, no tracker integration. A whole-file grep of the 1,099-line changelog returns zero hits for
budget, spend, tracker, beads, ledger and sqlite. Its plugin docs close the escape hatch
deliberately: "There is no Herdr-managed plugin storage API in v1. Plugins that need durable state
should own their files or database." Session persistence is UI state (workspaces, tabs, panes, cwd,
layout, focus, scrollback), not work artifacts. Maintainer thread herdrdev/herdr#741: herdr "has the
low-level primitives ... but there is currently no higher-level protocol for one agent to address
another by name, delegate a sub-task, or await a result."

**It does have native git worktree management**, which a first pass here got wrong (§11):
`herdr worktree list|create|open|remove` over both CLI and socket API, landed in 0.6.2 (2026-05-23)
with sidebar worktree groups in 0.6.1, still maintained through 0.8.2 (worktree creation reuses an
existing local branch rather than failing; removal refocuses the parent workspace).

**Two things it does not do that Air's launcher does**: role-prose injection and the
permission/deny list. The deny list is the mechanism enforcing the one-project rule, so a thin Air
wrapper survives even in the most aggressive version of this adoption.

**Its agent state cannot feed the gate.** `done` is self-reported by an integration or inferred
from the screen; there is no proof behind it. herdr can tell Air that a worker is idle or blocked.
It can never tell Air that the work is verified.

**A plugin is a structurally available shape.** `herdr plugin install <owner>/<repo>`, a
`herdr-plugin.toml` manifest with declarative `[[actions]]`, `[[events]]`, `[[panes]]`,
`[[startup]]`, and "the entire Herdr CLI is the plugin API ... a Bash script, JavaScript app, Lua
script, Rust binary, or any other argv command". A plugin owning its own SQLite is the documented
expected pattern, which is exactly Air's shape. Precedent exists: `miiraheart/herdr-beads` puts bd
behind a herdr board. The marketplace is an unreviewed 30-minute-refresh index of repos tagged
`herdr-plugin`, and herdr does not sandbox plugin code.

**Therefore**: herdr and Air do not overlap. herdr is the half Air should stop maintaining; the
ledger and the gate are the half herdr will never grow. Three concrete consequences:

1. **The poll thread goes.** `herdr agent wait --until done` and the event subscription are a push
   source for exactly the transitions Air polls for. Combined with `SendMessage`'s
   `notify_when_idle`, there is no remaining reason to re-evaluate conditions on a timer and write
   3 MB of events a day (§11).
2. **`air status` stops needing to be a screen.** `pane.report_agent` lets Air report
   "holding air-123, no green at HEAD" into herdr's own agent state, so the conditions surface
   where the owner is already looking. Air keeps computing them; herdr renders them.
3. **The launcher shrinks further than §10.2 said.** herdr owns panes, persistence and remote
   access, including the detached-start case that had no first-party Claude Code equivalent.

**Before acting**: herdr is five months old and pre-1.0 (v0.8.2), so the socket API may move. The
integration should be one adapter module behind a trait, the way `crates/bd` wraps `bd`, and it
should be probe-covered like everything else. A red/green probe that the adapter reports state and
receives events is the price of admission. License verified first-hand on 2026-08-25: the `LICENSE`
file is Apache-2.0 (some third-party writeups say AGPL-3.0 with a commercial option; they are
wrong, but re-check before adopting since the star count reported for this project also ranges
15k-32.3k depending on source).

### 10.6 What the deep-research pass found (2026-08-25, 108 agents, 25 sources, 124 claims extracted, 25 adversarially verified, 9 killed)

**The gate is still unclaimed, stated honestly.** No project reached gates completion of a durable
**externally tracked** work item on commit-bound machine-checkable evidence. The two closest both
gate something they own:

- **agentic-os** (KbWen/agentic-os) is the strongest match on mechanism and the weakest on scope.
  `validate.sh` reads a work trail and fails when "a required phase was skipped or its evidence is
  missing", running as an opt-in pre-commit hook and in CI, blocking a ship with no review or test
  evidence. But the gated unit is a phase of its own Markdown workflow keyed to a branch (one
  branch, one owner; per-task log at `.agentcortex/context/work/<branch>.md`), and the repo has no
  reference to GitHub Issues, Jira, Linear or beads. It is also not a runner: it installs by copying
  Markdown and scripts into a target repo, with no launcher, supervisor or daemon anywhere in the
  tree.
- **AgentOps** (boshu2/agentops, Apache-2.0, 428★, created 2025-11-05, pushed 2026-08-25) gates in a
  different place, and is the most interesting find in this pass because **it also uses beads**. It
  has a section headed "Intent lives in a bead": plan writes BDD acceptance and DDD ubiquitous
  language into the bead, implement builds against it, and validate judges a hashed snapshot under
  `.agents/ao/intents/sha256/`. Its evidence is content-addressed to filesystem state and works
  with or without beads, so it is not commit-bound the way Air's is.

State the negative as **"unclaimed among what was reached"**, not "unclaimed": this is
absence-of-evidence over a bounded set, from READMEs, changelogs, docs indexes and one file-tree
walk. Three sharper phrasings of it were voted down during verification for over-reach, and
herdr's unreviewed plugin marketplace could host something nobody found.

**bd is a safer bet than it was.** Two independent third-party adoptions now exist (AgentOps above,
and `miiraheart/herdr-beads` treating bd as the system of record behind a herdr board), neither
connected to Yegge. But both are prose-level or display-level: an agent skill writing into a bead, a
board reading out of one. **Nobody found shells out to `bd --json` from a program for atomic claim
arbitration.** That part of Air is still bespoke.

**Not answered, and it matters**: the research produced no verified claim about guild,
swarm-protocol or gnap, so bd's competitive position among claim-capable stores is unassessed. Air's
claim and close layer is the part most exposed if bd stalls. Carry this to the next refresh.

## 11. Corrections

**The attention-condition numbers do not mean what pass 1 said they meant.** Pass 1 reported
"48,612 alerts to tell you 26 things" and called it noise the human reads. `air audit` counts
**event-log firings**: every time the poll thread re-evaluated a condition and found it true and
wrote a line. The MCP channel pushes **only on change**, and two selftest probes assert exactly
that ("new condition pushed once, repeat suppressed"; "an unchanged set pushes once however old it
gets"). One probe records that a push was already deleted in favour of naming the fact on demand.

So the coordinator is not being spammed. What is true: the event stream carries about 3 MB a day of
re-evaluations, nothing collects it, and `air audit`'s own counts are inflated by it to the point
of being misleading about human-visible behaviour. The corrected recommendation in §10.2 is
narrower than the one it replaces.

**Fixed 2026-08-29 (air-5uz).** The poll now writes that line on change only, and the audit reports
`evaluated` and `pushed` as separate columns because they are separate facts. A push is recorded
where the hooks record theirs, in `hook_emissions`, and leaves a `channel.push` event line, so the
push count comes from the push rather than from a re-reading of the evaluation. Growth before and
after is in §3.

This is the failure the `do-less` skill's raw-record rule exists for (air-21c): a derived number
reads exactly like an observed one. It was caught here only because §9's probe list was read
before the verdict was rewritten.

**Three inventory errors found in pass 3, by checking the doc against the code rather than
re-reading the doc.** (1) `bd-latency` was listed as a CLI command; `air bd-latency` returns
"unrecognized subcommand". It is a module. (2) The MCP surface was listed as serving three prompts,
on the strength of a comment in `install.rs`; `prompts/list` returns `[]` and a probe asserts it.
That is Air's own version of the failure air-ha8 records: a surface describing something untrue.
(3) The env-var and skill counts were approximations; the exact figures are 25 and 34.

**Corrected in pass 6, from the deep-research findings.** §10.5 as first written said herdr had no
worktree management. It has had native `herdr worktree list|create|open|remove` over CLI and socket
API since 0.6.2 (2026-05-23), verified afterwards by reading the changelog directly (worktree groups
in the sidebar, branch reuse on creation, parent refocus on removal). The error came from trusting a
single docs page that happened not to mention it, which is the same shape as every other correction
on this list: one source, no cross-check.

**Also corrected**: pass 1 said Air's launcher "adds role prose, deny list and env" as though that
were the whole of it, and separately implied the harness might not have `--worktree`. Both
`-w/--worktree` and `--tmux` exist in Claude Code 2.1.241 and were verified by reading
`claude --help` directly.

## 12. Refresh

Re-run the commands in the preamble and update every count. Trigger on any of: a Claude Code
release that touches orchestration, tasks, permissions or hooks; a refresh of
[`../research/harness-and-orchestrator-landscape.md`](../research/harness-and-orchestrator-landscape.md);
any change to Air's own surface; or monthly, whichever comes first. Invoke `check-resources` before
adding anything and record the four-line answer in the bead.

A row with no "why" is a deletion candidate by default. A number quoted here without saying what it
counts is a defect, per §11.
