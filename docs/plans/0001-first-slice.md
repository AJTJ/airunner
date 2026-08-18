# 0001 — First slice: the referee adopter can run next round

Status: proposed, 2026-08-17. Argument for what to build first and the contract it must meet.
Sources: `docs/research/SYNTHESIS.md`, `coordinator-interview-2026-08-17.md`,
`worker-interviews-2026-08-17.md`, `adopter-enforcement-and-skills.md §3`,
`claude-code-control-surfaces.md`, `beads-and-gastown.md §5`, `prior-art-landscape.md §H`.
Binary name is written `<bin>` — undecided.

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

Per worktree/worker (`.claude/worktrees/<name>`, actor from `BEADS_ACTOR`/worktree name):

| Fact | Written by | Today lives in | Source |
|---|---|---|---|
| `verified(sha, exit, run_id, ts, kind=verify|docs-check|fitness)` | `<bin> record verify` wrapped around `make verify` (or read from `logs/verify.log` END trailer / `durations.tsv` as fallback) | prose in chat + `bd close --reason`; accidentally `durations.tsv` | coord §5(a), §6(iv); backend §4,§7e; frontend §4,§7a; third §4; fourth §4,§7a |
| `holding(worker, path, since, kind=uncommitted|committed, sha)` — files a worker has edits in | derived: `git status`/`git diff <merge-base>` across worktrees on demand + `PostToolUse(Edit|Write)` journal for intent | chat ("are you in X?"), `make fleet` after divergence | 4/4 workers §3; coord §4 |
| `merged(worker, peer, sha, ts)` — which peer shas a worker already contains | derived from `git merge-base --is-ancestor` on demand | chat | third §3, §7b; frontend §5 |
| `announced_next(worker, bead)` / `promise(worker, peer, text, ts)` | `<bin> note` (optional, cheap) | chat | coord §4; frontend §5 |
| `claim(bead, worker, ts)` mirror + `lease(generation, heartbeat_ts)` | from `bd --json` + hooks | bd (1.2.1 leases — untested release) | beads-and-gastown §0 |
| event log (every question asked, every answer, every refusal + reason) | the binary | nowhere | corpus "code fails confidently"; third §7 denominator |

Storage: SQLite at `<repo>/.<bin>/ledger.db` (WAL), plus append-only NDJSON events. Filesystem
facts (git) are re-derived, never trusted from cache (level-triggered, third/fourth §7).

## 3. Questions it answers (CLI, `--json` always; each prints its denominator)

- `<bin> holdings [--file X]` — who has edits in which files, uncommitted vs committed, with shas.
  ("compared 4 worktrees, 6 pairs".)
- `<bin> next` — `bd ready --json` filtered live (already claimed, runtime label, WIP), ranked by
  file overlap with live holdings; shows *which peer, which file*, and whether the bead's
  `file:line` citations still resolve. Leaves the choice to worker/coordinator.
- `<bin> peer <name>` — peer's HEAD, last recorded green sha, red-or-green, whether it already
  contains my HEAD ("merge-back is cheap"), the exact `git merge <sha>` to run.
- `<bin> merge-advice` — "main moved to X touching your files [list]; peer P is green at Y touching
  [list]; P has merged your Z". Never performs the merge.
- `<bin> handover [--bead]` — the one gate (§4).
- `<bin> record verify -- make verify` — runs the command, records `(HEAD, exit, run_id)`; also
  `--kind docs-check|fitness`. Docs-only delta ⇒ docs-check suffices, no full re-run (fourth §7a).
- `<bin> post-merge` — after any merge: run the citation/fitness check and print the fix (fourth
  §7; matrix-citation churn was the most repeated merge cost).
- `<bin> status` — the coordinator's dashboard: per worker HEAD/green sha/holdings/last tool call
  age/claims; replaces reading `bd list --status in_progress` + `make fleet` by hand (coord §3).

## 4. The one refusal: hand-over

`<bin> handover` (and, wired as a `Stop`/`SubagentStop` hook in **advisory** mode first) refuses
`awaiting_review`/close unless **all** hold, and prints exactly which failed and the command to fix:

1. `verified(HEAD, exit=0, kind=verify)` exists for this worktree — *evidence, never model text*
   (0022 §5.2; enforcement rank 2).
2. HEAD contains current `main` (`merge-base --is-ancestor main HEAD`) — "merge main first".
3. `fitness`/`docs-check` green at HEAD *as a peer would see it* (third §7a).
4. The bead is claimed by this actor (CAS via `bd --if-assignee`), and closing is refused if the
   commit is not on `main` yet — `awaiting_review` is the correct transition (frontend §6e).
5. Optional (M1): digest present (`docs-check` already enforces size).

WIP checkpoint commits on the worker's own branch are **never** blocked (frontend §7). Merges are
**never** refused (backend §7). No hook ever blocks on a question (all workers §7).

`land`: `<bin> land <worker> [--sha X]` — port of `scripts/land.sh` semantics
(refuse dirty/main/agent → digest → `--no-ff` → regenerate generated inputs → verify merged tree
with output captured → rewind on red → close attributable beads by evidence), plus **land by sha**
so a green point survives later commits (backend §7d), and a check that the recorded green sha ==
what is being landed (frontend §7a). Ships behind `land-prove` staying green.

## 5. Hook wiring (Claude Code)

All hooks call the same binary; each returns in < 300 ms or degrades to "unknown" (backend: 3×
`bd` per stop ≈ 4 s is too slow). Installed by `<bin> install`, which verifies the *resolved*
path is the binary, not a worktree copy (enforcement rank 10).

| Event | Action |
|---|---|
| `SessionStart` | inject `status` for this worker (claims, green sha, holdings, main moved?) as `additionalContext` |
| `PreToolUse(Edit\|Write\|MultiEdit)` | if the target path is held by a peer (uncommitted or committed since my merge-base): **warn** with peer + sha in `additionalContext`; never deny (fourth §7b: "before I start, not after"; third: derive from diff, don't ask) |
| `PostToolUse(Edit\|Write)` | journal `holding(intent)` — zero token cost (0022 §5.3) |
| `PreCompact` | write the compaction packet: claims, holdings, promises, green sha, merged peers |
| `Stop` / `SubagentStop` | advisory `handover` result as context in M0; **blocking** only when the worker has set `awaiting_review`/close in this turn and evidence is missing (M1, after one round of advisory data) |

Liveness for the coordinator/`status`: last **tool call** (transcript mtime), not last message
(coord §6(v)).

## 6. What stays with people

Coordinator: steering, priorities, rulings onto beads, rename/modify ownership, WIP arbitration,
scope discipline, machine-level actions, the honest digest (coord §5). Owner: land, decisions.
The binary never sends messages; SendMessage stays the routing channel (it worked; the *facts*
were the problem — coord §1).

## 7. Beads boundary

Read via `bd --json` only; write only through `bd` (claim/CAS/status/comment). Leases mirrored
in the ledger with generations; **do not** rely on `bd 1.2.1` lease fields (accidental release).
Pin `bd` in adopter before the next round (`beads-and-gastown.md §0`). Never make adopter's
tooling depend on this repo's *build* — install a binary (`corpus §5.4`).

## 8. Probes and measurement

- Every check ships with a red/green probe (`<bin> selftest` runs them; a check that matches
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
`crates/cli` (`<bin>`). Crates from the shortlist: clap, rusqlite (bundled), serde/serde_json,
gix (read-only) + shell `git`, tokio only where needed, tracing. No async in hooks (latency).

## 11. Open decisions for the owner

1. Binary name.
2. M0 in advisory-only mode for one round before the Stop hook can block? (Recommended: yes.)
3. Pin `bd` at 1.2.1 or move adopter's leases into the ledger now?
