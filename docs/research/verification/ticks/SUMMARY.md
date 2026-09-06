# Research-deepening window — summary (2026-08-18, 02:15–05:24 PDT)

Owner-scheduled: one tick every 15 minutes for three hours. Ticks fired at 02:15 (early, jitter),
02:30, 02:45, 03:00, 03:15, 03:30, 03:45, 04:00, 04:15, 04:30, 04:45, 05:09, 05:24. Every
artefact is committed on `main` with author `ajtj`; every claim carries a URL + access date or a
`path:line`.

## What was finished (Step 0 — work that had stalled on the 23:xx session limit)

| Item | Result |
|---|---|
| Skills port, 5 groups | 29 skills + the 48-file review reference library in `.claude/skills/`, each with `## Provenance`; index in `.claude/skills/PROVENANCE.md`; `.claude/settings.json` (cargo-fmt hook); `docs/rules/{writing,worktree-protocol}.md`; `docs/plans/0003-rust-conventions.md` (open decisions). |
| Verification slices, 5 | `specs-guards-tooling.md` (44 rows), `fleet-size-partition-cadence.md` (33), `protocols-leases-resources.md` (60), `mas-literature-part1.md` (50), `mas-literature-part2.md` (66) — ~250 load-bearing claims re-checked against primary sources. |
| Corrections applied | `SYNTHESIS.md` §1b/§0 and plan 0001 §4/§5/§7 edited in place with citations (see below). |

## What each deepening tick found (Step 1)

| Tick | File | Finding that changed something |
|---|---|---|
| 02:30 | `2026-08-18-0230-what-to-work-on.md` + **draft `docs/plans/0002-what-to-work-on.md`** | Every fleet system pulls leaves from a ready frontier computed from edges; only Symphony and the adopter 0022 have a real commitment line; executable check, not prose, is the done signal. MVP: owner names the feature; epic `--design` with one end-to-end check; bead claimable only with executable acceptance + lane + edges + resolving citations; `bd ready` traversal, ≤4 workers, WIP≤2. Do not build formulas/molecules/wisps, convoys, refinery, phase machine, LLM triage. Six owner decisions in plan 0002 §7. |
| 02:45 | `…-0245-claude-code-hook-edge-cases.md` | `PermissionRequest` is a real hook (the `stuck` state is observable); `SessionEnd` **not guaranteed** on SIGKILL (sessions rows must also expire on dead transcript/worktree); PreCompact too late for evidence capture (Stop is primary); hooks parallel, 600 s default timeout; blocking Stop must honour `stop_hook_active`. → plan 0001 §5. |
| 03:00 | `…-0300-bd-1-2-x-facts.md` | Latest beads is still v1.2.2 (= 1.1.2 tree); 1.2.1 "published by accident". 1.2.2 also lacks CAS flags, `unclaim`, `--force`. Leases on main: 5-min TTL, actor-string auth only. `bd ready` = status open only, cap 100. Embedded Dolt serialises writers with backoff. → **ledger owns CAS and leases**; the adopter → 1.2.2 + `brew pin` after cursor rollback; minimal `WorkLedger` surface verified in 1.2.2. Correction note added to `beads-and-gastown.md §0`. → plan 0001 §4/§7. |
| 03:15 | `…-0315-rust-crates-latency.md` | Crate table with versions/licences; SQLite WAL semantics quoted; local measurements: `git` ≈10 ms spawn, status 22–25 ms/worktree, `bd --version` 122 ms, `bd ready --json` 1.1 s → **`bd` never on a hook path**. Budget p50 10–25 ms, p99 ≤150 ms, 250 ms watchdog; fail-open via `catch_unwind`. → plan 0003 §7a. |
| 03:30 | `…-0330-synthesis-claim-ledger.md` | 94-row claim ledger for SYNTHESIS: 60 verified, 9 stale fixed in place, 19 unverified, 7 opinion. Notable: the adopter itself had retracted "`--claim` reopens closed beads" — synthesis/plan now use the corrected form; enforcement report still repeats it (flagged). |
| 03:45 | (ledger addendum) | Copied the adopter's plan 0021, `.beads/config.yaml`, `scripts/land.sh` into `adopter-notes/` (provenance addendum) → 5 adopter-internal rows VERIFIED-LOCAL; 14 external rows remain (check when borrowing). |
| 04:00 | `…-0400-lane-granularity.md` | No study compares same-file vs same-directory as conflict predictors on one dataset; Dias measured slices (42.7% of conflicts in slice-disjoint work); every agent-era system enforces/detects at **file** level. → journal stores file paths; `air next` ranks same-file first, directory tie-break, committed `shared_files` always flagged; lane label is a hint; no slice detection/lock registry/sub-file admission. → plan 0001 §2/§3. |
| 04:15 | `…-0415-billing-refetch.md` | Pricing and SDK-auth policy re-verified from Anthropic pages; **"3–5 concurrent workers" is not derivable** from any published allowance — labelled ESTIMATE; Air should record `rate_limits` used-percentage during 4-agent rounds. → SYNTHESIS §0. |
| 04:30 | `…-0430-measurement-spec.md` | Six round-comparable metrics defined (review latency split wait/rewind; success S1/S2 ≠ pass@1; discarded hours; imported-red; stale-`next`; relays) with formulas, edge cases, 9 extra columns + 2 event kinds; reviewer-habituation instrument from 2606.22721. → plan 0001 §2/§8. |
| 04:45 | `…-0445-air-land-spec.md` | 20-step port of `land.sh` with every refusal string cited; shim keeps `land-prove` green; `--sha X` semantics + receipt fields; per-repo `regenerate` list; per-step capture; recorded-green advisory (land always verifies merged tree); close-by-evidence ported; conflicts = abort + print command; batching deferred behind a ledger trigger; ~35 probes. → plan 0001 §4. |
| 05:09 | `…-0509-topology-declaration.md` | ~25-line `.air/topology.toml` schema where every key binds to an existing column/check; `the adopter` preset filled; `solo-ralph`/`pair-review`/`refinery` sketched; explicit not-declared list; `air topology check` probe; build only when a second real shape exists. → SYNTHESIS §4.4. |

## Corrections to the synthesis that came out of verification

- Kim et al. topology overheads (58%/285%/515%) describe redundant solvers of one task, **not** one-agent-per-bead fleets; the "5–9× orchestrator overhead" claim was dropped. Fleet-size ≤4 (thin returns) stands.
- Dias "OR 6.13 vs 1.04–1.09" is not a ratio (binary vs standardised variable); population resolved (73,504 / 125 projects). Slice ≠ file.
- "9× less hacking with an anti-hack sentence" → ~2.7× (Opus 4.1 addendum), 1.6× on Opus 4.5.
- "CLI 5–28× cheaper than MCP" is misattributed; only equal failure frequency and 12.9% vs 2.2% wasted spend survive; CLI-first still supported.
- CAID vs STORM: worktree ≈ soft isolation in both; the moving part was the solo baseline. Worktrees are for conflict avoidance and process safety, not a benchmark win.
- "3–5 concurrent workers" is an estimate.
- Several adopter-internal wordings retracted by the adopter itself were replaced.

## Effect on the plans

- **Plan 0001** (first slice): ledger owns CAS/leases; `bd` off the hook path; hook budget; sessions-state caveats (SIGKILL, PreCompact); file-level holdings with `shared_files`; measurement columns; `air land` spec pointer; topology remains a non-goal until a second shape.
- **Plan 0002** (what to work on): drafted; six owner decisions.
- **Plan 0003** (Rust conventions): proposed crate/setting choices; nothing decided.

## Still open (for the owner)

1. Plan 0001 §11: advisory-first (recommended yes); `bd` 1.2.2 + pin (recommended); the six plan-0002 decisions; plan-0003 crate choices.
2. 14 external/design ledger rows remain unverified — check each only when the corresponding piece is borrowed (M1/M2).
3. Enforcement report still repeats a retracted the adopter claim (rank 3, `--claim` reopens closed) — fix when that report is next edited.

## Housekeeping

The cron jobs for this window are session-only and expire; they should be deleted at the end of the last tick.
