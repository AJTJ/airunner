# 0001 — First slice: the referee adopter can run next round

Status: proposed, 2026-08-17. Argument for what to build first and the contract it must meet.
Sources: `docs/research/SYNTHESIS.md`, `coordinator-interview-2026-08-17.md`,
`worker-interviews-2026-08-17.md`, `adopter-enforcement-and-skills.md §3`,
`claude-code-control-surfaces.md`, `beads-and-gastown.md §5`, `prior-art-landscape.md §H`.
Binary name: **Air** (`air`), owner's choice 2026-08-17.

## 1. The bet

One Rust binary, installed on the adopter machine, that (a) holds facts the fleet currently
carries in chat and in heads, (b) answers questions from those facts at hook time and on the CLI,
and (c) refuses exactly one thing — a hand-over whose evidence is missing. Nothing else is
refused; everything else warns and prints the fixing command. If it does not measurably reduce
relays and drift within one or two fleet rounds, we stop.

Why this and not a dispatcher/daemon: the independent research (enforcement audit ranks,
corpus principles, prior-art convergence) and the five live-session interviews point at the same
first primitive (a machine-readable *verified-at-sha* record) and the same missing information
(*who holds which file*). The interviews are **weighted, not followed** — see §1a — because the
questions were leading, the five sessions share one CLAUDE.md and one round's incidents, and
self-report is what the corpus says is unreliable. Where an item rests on interviews alone it is
marked so below.

## 1a. Evidence weighting (interviews vs research)

Interview caveats (method): questions 7 proposed (a)(b)(c) and asked for agreement — assent to
those is weak evidence; questions 1–6 asked for facts (counts, shas, incidents) — those are
stronger and several are checkable in adopter's git/bd. All five sessions were primed by the
same `CLAUDE.md` rules (so "wish enforced" often echoes an existing rule) and by one round's
salient failures (matrix citations, one imported-red merge). Weighting:

| Item | Independent research | Interviews | Weight |
|---|---|---|---|
| Evidence-gated hand-over (verify green at HEAD, never model text) | enforcement rank 2; 0022 §5.2; corpus §0.2 (self-report unsound; 51% false success); metis Stop-hook pattern | 5/5 asked for it (leading) + 3 factual drift incidents | **strong** — research-led, interviews confirm with facts |
| Verified-at-sha record as the primitive | implied by rank 2 and by "land verifies merged tree" (as-built §2.5); prior art: Paperclip/Overstory per-run records | 5/5; factual: `durations.tsv` accidental record exists, drift 3× | **strong** |
| File-level holdings, derived not declared | corpus §0.4 (file overlap OR 6.13; lock-only registries fail; partition quality dominates); enforcement rank 6; `file_overlap.py` exists | 4/4 factual ("learned only by message"), 2/4 reject up-front declaration | **strong** on the need; **medium** on derive-vs-declare (research says declared *partition* matters; interviews resist forms — resolve: derive as default, allow declaration) |
| Peer merges frequent; make them safe, don't forbid | corpus: the one barrier produced every conflict; every-merge verification | coord factual (5/round), workers factual (3–7/round); "warn not refuse" is preference | **medium-strong** on frequency; **medium** on warn-vs-refuse — research would accept refusing merges of *red* tips; keep warn in M0, revisit with data |
| `next` = live, overlap-ranked, choice left to human | corpus §0.6 (no LLM middle-manager), Symphony eligibility/sort; enforcement rank 4/5 | factual: ready lists ~40% stale; preference for choice | **strong** on liveness; ranking scheme is ours to measure |
| Never block on a prompt; warn inside worktree; refuse only at hand-over | Symphony "MUST NOT stall waiting for input"; corpus "deadlines ask, don't kill"; guards prior-art teaching-denial | 5/5 preference | **strong** (independently supported) |
| Never refuse WIP checkpoint commits | corpus §0.8 (checkpoint = incremental deliverable; crash-only) | frontend preference | **strong** |
| Hooks fast (<300 ms) | corpus (CLI cost), Claude Code hook timeouts | backend factual (4 s stop guard) | **medium** — number is ours |
| Land by sha; regenerate generated inputs; capture output | as-built §4.7; merge-automation note (shell land untested) | coord/backend factual incidents | **medium-strong** |
| Post-merge citation/fitness auto-check | fitness-function prior art (line-pinned citations brittle → symbol anchors) | fourth/third factual (most repeated cost) | **medium** — research suggests fixing the *citation scheme* (symbol anchors) beats auto-shifting; do the cheap print-the-fix now, fix the scheme later |
| Print denominator; `--json`; level-triggered | corpus (guards that fail confidently; level-triggered reconciliation) | third preference | **strong** |
| Coordinator keeps steering/rulings/arbitration | corpus §0.6 read carefully; owner's account | coord self-description | **medium** — self-report; the ledger should measure how many coordinator turns remain triage after M0 |
| Guard false-positive tax → rewriting linter | guards-prior-art (soft_deny; teaching denial) | 3/4 factual (~15 refusals each) | **out of scope** for this slice; note for adopter |
| "Both themes" rule unenforceable in worktree | none | 2/4 factual admission | not ours; report to adopter |

Rule for the rest of this plan: an item appears in §3–§5 only if it is **strong** or
**medium-strong** above, or is cheap and reversible (advisory-only).

## 2. Facts the ledger holds (state that dies today)

Principle: store only what cannot be re-derived from git/bd; recompute everything else on demand.
No time-based expiry anywhere — a row lives until its state condition is provably false (owner,
2026-08-17: "we either want to keep state or not").

Location: one SQLite file at the *main* checkout, `<repo>/.air/ledger.db` (found via
`git rev-parse --git-common-dir`, shared by all worktrees; gitignored; WAL), plus
`<repo>/.air/events/YYYY-MM-DD.ndjson` (append-only), plus `<repo>/.air/config.toml`
(verify command, worktree root, `bd` path — human-diffable, committed).

| # | Table | Row = | Written by | Lives until |
|---|---|---|---|---|
| 1 | `verify_runs` | worker, sha, kind (`verify`/`docs-check`/`fitness`), exit, start/finish, log path | `air record <kind> -- <cmd>` (fallback: parse `logs/verify.log` END trailer) | keep last 50 per worker+kind, plus any row referenced by a landing (`air gc`) |
| 2 | `edit_journal` | worker, path, first/last seen, session id — *intent to touch*; **file paths only** — directory overlap is derived at query time, never stored ([lane granularity tick](../research/verification/ticks/2026-08-18-0400-lane-granularity.md)) | `PostToolUse(Edit\|Write)` hook, zero tokens | the path is no longer in that worker's diff vs main (landed/reverted) or the branch/worktree is gone |
| 3 | `claims` | worker, bead, claimed-at, optional declared files, optional preconditions (`--after peer@sha`) — *the intent record* | `air claim` / `air release` (wraps `bd update --claim` with CAS + actor) | bead leaves `in_progress` in bd (bd is truth; reconciled on every command) |
| 4 | `sessions` | worker, session id, transcript path, state ∈ {working, running(tool) since, stuck(permission) since, idle since}, changed-at | `SessionStart`/`PreToolUse`/`PostToolUse`/`PermissionRequest`/`Stop`/`SessionEnd` hooks | `SessionEnd`, or transcript/worktree gone |
| 5 | `landings` | worker, sha, result (+ failing step), verify_run id, ts — *the receipt* | `air land` | forever (audit trail; measures review latency = green → landed) |
| 6 | events (NDJSON) | ts, worker, command, inputs, answer, decision, reason, denominator | every Air invocation | kept (small text); pruning is an owner decision, not a default |
| 7 | `leases` (M1) | worker, generation, heartbeat ts | hooks | expire by generation/TTL semantics of the lease itself (M1 design) |

Derived on demand, never stored: holdings (who has edits in which file: `git status`/`git diff
<merge-base>` across worktrees, cross-checked with `edit_journal`), `merged(worker, peer, sha)`
(`git merge-base --is-ancestor`), main-moved-and-touched-your-files, peer red/green (peer HEAD ⋈
`verify_runs`), the ranked `next` list.

`rm -rf .air/` is always safe: only verify history and landing receipts are lost.

## 3. Questions it answers (CLI, `--json` always; each prints its denominator)

- `air holdings [--file X]` — who has edits in which files, uncommitted vs committed, with shas.
  ("compared 4 worktrees, 6 pairs".)
- `air next` — `bd ready --json` filtered live (already claimed, runtime label, WIP), ranked by
  **same-file** overlap with live holdings, then same-directory overlap as a tie-breaker only;
  files on the committed cross-cutting list (`shared_files` in `.air/config.toml`: `features.md`,
  `authorization-matrix.md`, openapi, lockfiles, Makefile) held by a live peer are always flagged
  regardless of rank ([lane granularity tick](../research/verification/ticks/2026-08-18-0400-lane-granularity.md)); shows *which peer, which file*, and whether the bead's
  `file:line` citations still resolve. A declared lane on the claim is a hint that seeds ranking
  before the first edit; the journal supersedes it. Leaves the choice to worker/coordinator.
- `air peer <name>` — peer's HEAD, last recorded green sha, red-or-green, whether it already
  contains my HEAD ("merge-back is cheap"), the exact `git merge <sha>` to run.
- `air merge-advice` — "main moved to X touching your files [list]; peer P is green at Y touching
  [list]; P has merged your Z". Never performs the merge.
- `air handover [--bead]` — the one gate (§4).
- `air record verify -- make verify` — runs the command, records `(HEAD, exit, run_id)`; also
  `--kind docs-check|fitness`. Docs-only delta ⇒ docs-check suffices, no full re-run (fourth §7a).
- `air post-merge` — after any merge: run the citation/fitness check and print the fix (fourth
  §7; matrix-citation churn was the most repeated merge cost).
- `air status` — the coordinator's dashboard: per worker HEAD/green sha/holdings/last tool call
  age/claims; replaces reading `bd list --status in_progress` + `make fleet` by hand (coord §3).

## 4. The one refusal: hand-over

`air handover` (and, wired as a `Stop`/`SubagentStop` hook in **advisory** mode first) refuses
`awaiting_review`/close unless **all** hold, and prints exactly which failed and the command to fix:

1. `verified(HEAD, exit=0, kind=verify)` exists for this worktree — *evidence, never model text*
   (0022 §5.2; enforcement rank 2).
2. HEAD contains current `main` (`merge-base --is-ancestor main HEAD`) — "merge main first".
3. `fitness`/`docs-check` green at HEAD *as a peer would see it* (third §7a).
4. The bead is claimed by this actor — CAS is owned by the **ledger** (`bd` 1.2.2 has no
   `--if-assignee/--if-status`, [bd facts tick](../research/verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md)) — and closing is refused if the commit is not on `main` yet —
   `awaiting_review` is the correct transition (frontend §6e).
5. Optional (M1): digest present (`docs-check` already enforces size).

WIP checkpoint commits on the worker's own branch are **never** blocked (frontend §7). Merges are
**never** refused (backend §7). No hook ever blocks on a question (all workers §7).

`land`: `air land <worker> [--sha X]` — port of `scripts/land.sh` semantics
(refuse dirty/main/agent → digest → `--no-ff` → regenerate generated inputs → verify merged tree
with output captured → rewind on red → close attributable beads by evidence), plus **land by sha**
so a green point survives later commits (backend §7d), and a check that the recorded green sha ==
what is being landed (frontend §7a). Ships behind `land-prove` staying green.

## 5. Hook wiring (Claude Code)

All hooks call the same binary; each returns in < 300 ms or degrades to "unknown" (backend: 3×
`bd` per stop ≈ 4 s is too slow). Installed by `air install`, which verifies the *resolved*
path is the binary, not a worktree copy (enforcement rank 10).

| Event | Action |
|---|---|
| `SessionStart` | inject `status` for this worker (claims, green sha, holdings, main moved?) as `additionalContext` |
| `PreToolUse(Edit\|Write\|MultiEdit)` | if the target path is held by a peer (uncommitted or committed since my merge-base): **warn** with peer + sha in `additionalContext`; never deny (fourth §7b: "before I start, not after"; third: derive from diff, don't ask) |
| `PostToolUse(Edit\|Write)` | journal `holding(intent)` — zero token cost (0022 §5.3) |
| `PreCompact` | write the compaction packet: claims, holdings, green sha, merged peers. Caveat: PreCompact fires late for *evidence capture*; the Stop hook is the primary point, PreCompact a re-injection point ([hook edge cases](../research/verification/ticks/2026-08-18-0245-claude-code-hook-edge-cases.md)) |
| `Stop` / `SubagentStop` | advisory `handover` result as context in M0; **blocking** only when the worker has set `awaiting_review`/close in this turn and evidence is missing (M1, after one round of advisory data) |

Liveness for the coordinator/`status`: last **tool call** (transcript mtime), not last message
(coord §6(v)). Verified constraints ([hook edge cases](../research/verification/ticks/2026-08-18-0245-claude-code-hook-edge-cases.md)): `PermissionRequest` is a real hook event (can auto-allow/deny) — the `stuck` state is observable; `SessionEnd` is **not guaranteed** on SIGKILL/crash — the `sessions` row must also expire when the transcript stops changing and the worktree/process is gone; hooks run in parallel with a 600 s default timeout — Air sets its own short `timeout`; Stop-hook blocking must honour `stop_hook_active` to avoid loops.

## 6. What stays with people

Coordinator: steering, priorities, rulings onto beads, rename/modify ownership, WIP arbitration,
scope discipline, machine-level actions, the honest digest (coord §5). Owner: land, decisions.
The binary never sends messages; SendMessage stays the routing channel (it worked; the *facts*
were the problem — coord §1).

## 7. Beads boundary

Read via `bd --json` only; write only through `bd` (claim/status/comment). CAS and leases are
owned by the ledger — `bd` 1.2.2 (the only supported release, 2026-08-18) has no CAS flags,
leases, heartbeat, reclaim, events, or `--force`; 1.2.1 is an accidental release. Recommendation:
adopter moves to 1.2.2 + `brew pin` after the documented cursor rollback
([bd facts tick](../research/verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md)). Minimal
`WorkLedger` surface verified present in 1.2.2: `ready`, `show`, `list`, `update --claim`,
`update -s/-a`, `comment`, `close`, `dep`, `blocked`, `recompute-blocked`. Never make adopter's
tooling depend on this repo's *build* — install a binary (`corpus §5.4`).

## 8. Probes and measurement

- Every check ships with a red/green probe (`air selftest` runs them; a check that matches
  nothing prints red).
- Metrics the ledger makes free: relays about merge/overlap per round (coordinator's ~5/30
  messages), imported-red incidents (1–2/round today), drift incidents ("green" ≠ landed), stale
  suggestions in `next` (frontend: ~40%), review latency (awaiting_review → landed), time from
  green to land, single-agent success per bead.
- Success for M0: adopter runs one round with the binary installed; `make fitness` "enforced"
  count rises (5/14 → ≥ 8/14); coordinator reports fewer merge/overlap relays; zero imported-red.

## 9. Non-goals for this slice

Daemon/dispatcher, spawning sessions, TUI, MCP, multi-host, API-backed roles, replacing beads,
running Gas Town, named topologies (config comes once two shapes exist). All remain in
`SYNTHESIS.md §4–5` for M1/M2.

## 10. Proposed workspace

`Cargo.toml` workspace: `crates/ledger` (SQLite + events + git derivations), `crates/bd`
(`WorkLedger` over `bd --json`, version-gated), `crates/hooks` (Claude Code hook I/O types),
`crates/cli` (`air`). Crates from the shortlist: clap, rusqlite (bundled), serde/serde_json,
gix (read-only) + shell `git`, tokio only where needed, tracing. No async in hooks (latency).

## 11. Open decisions for the owner

1. Binary name.
2. M0 in advisory-only mode for one round before the Stop hook can block? (Recommended: yes.)
3. Pin `bd` at 1.2.1 or move adopter's leases into the ledger now?
