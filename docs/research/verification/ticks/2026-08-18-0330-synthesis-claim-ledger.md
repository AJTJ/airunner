# Tick 0330 — SYNTHESIS.md claim ledger

Written 2026-08-18 ~03:30 PDT for ai_runner (Air). Every sentence in `docs/research/SYNTHESIS.md`
that carries a **number**, a **named study/tool/person**, or a **causal assertion presented as
evidence** is listed once, with the report it came from and its verification status against the five
verification slices (`fleet-size-partition-cadence.md` = **FLEET**, `specs-guards-tooling.md` = **SPECS**,
`protocols-leases-resources.md` = **PROTO**, `mas-literature-part1.md` = **MAS1**,
`mas-literature-part2.md` = **MAS2**) and the ticks (**T0230** what-to-work-on, **T0245** hook edge
cases, **T0300** bd 1.2.x facts, **T0315** Rust crates). Line numbers for SYNTHESIS refer to the file
*before* this tick's edits (the header note added four lines; the fixes below did not change line
count except where noted).

**Status legend.**
- **VERIFIED** — a verification report/tick row confirms it (row cited).
- **VERIFIED-LOCAL** — an adopter-internal measurement or quotation, checked against the verbatim
  copies in `docs/research/adopter-notes/` (file:line cited). Not re-derivable from primary sources
  outside adopter; "verified" here means "the synthesis reports adopter's record faithfully".
- **CORRECTED-ALREADY** — a verification report corrected it and SYNTHESIS already carried the correction
  before this tick.
- **STALE → FIXED** — SYNTHESIS still stated something a verification report or adopter's own record
  corrected; fixed in place this tick (what changed is in the note).
- **UNVERIFIED** — no verification slice covers it. Sub-tags: *ext* (external fact, origin report is
  primary-sourced but not independently re-fetched), *int* (adopter-internal, source file not in
  `adopter-notes/` copies), *design* (a claim about a tool's shape taken from the landscape report).
- **OPINION** — marked inference/recommendation; no verification needed.

## 1. Ledger

| # | Quote (short) | SYNTHESIS § / line | Origin report + line | Status | Verification / note |
|---|---|---|---|---|---|
| 1 | "66 process rules are only 27% enforced" | §0 table, L10 | enforcement §2 table `:156` (18/66 E) | **STALE → FIXED** (attribution) | The 66/27% is the *enforcement report's* tally, not "adopter's own diagnosis"; adopter's own self-counts are "six rules still prose only" (`fitness.sh`) and 8/9 of 14 (`enforcement:157-160`). Text now says "by our tally … adopter's self-counts agree in shape". |
| 2 | "plan 0022 already specifies the missing hooks and none is written" | §0, L10 | as-built §5 `:292-306`; enforcement §0 | VERIFIED-LOCAL | `adopter-notes/plans/0022:164-212` names PreCompact/Stop/PostToolUse hooks; `0022:259` "Waits: PreCompact, the PostToolUse journal…". |
| 3 | rank-1/rank-2 pains = stalls; exit gated on a promise | §0, L10 | enforcement §3 `:180-183` | VERIFIED-LOCAL | Ranked table rows 1–2 as cited. Ranking itself is the report's judgement (opinion). |
| 4 | Layer 2 required because stall detection "cannot be done from inside a stalled session" | §0, L11 | as-built §4.2 `:208`; control-surfaces §2 | VERIFIED-LOCAL + T0245 | Retro: "A blocked session cannot receive" (`overnight-fleet-retrospective.md:177`); T0245: no hook observes permission-prompt/blocked state; SessionEnd not guaranteed on SIGKILL. |
| 5 | Headless/worktree/subagent sessions "draw from the same 5-hour + weekly pool" | §0, L12 | billing §1 `:16-23` (costs.md, 2026-08-17) | UNVERIFIED (ext) | Billing report is primary-sourced but no verification slice re-fetched it. Suggest: re-fetch `code.claude.com/docs/en/costs.md` before any capacity decision (SYNTHESIS §6 already says so). |
| 6 | "~3–5 concurrent workers is the practical Max ceiling" | §0, L12 | billing §5 `:105`, §7 `:174` | UNVERIFIED (ext) | The billing report attributes this to costs.md but the "3–5" reads as an inference; the docs page states pooling, not a number. Suggest: quote as the report's estimate; measure with `rate_limits.five_hour.used_percentage` (PROTO row 45 confirms the statusline field). |
| 7 | "matches adopter's fleet of 4" | §0, L12 | as-built `:152` (`SESSION_SOFT=4/HARD=8`); corpus `:1343` | VERIFIED-LOCAL | `fleet.sh` soft 4 / hard 8; corpus §4.1 notes the pair cites a superseded section (FLEET row 5: hard 8 unsupported by Li et al.). |
| 8 | "Agent SDK may currently use subscription auth (separation paused 2026-06-15)" | §0, L12 | billing §3 `:63-72` (support.claude.com article 15036540) | UNVERIFIED (ext) | Primary-sourced 2026-08-17; policy, not contract. Suggest: re-fetch the support article at each milestone. |
| 9 | Haiku API call for triage/digest | §0, L12 | billing §7 `:145-157` | OPINION | Design choice; Haiku price row `:37` is a vendor table (not re-verified). |
| 10 | "No wholesale adoption exists; nothing is a Rust, beads-native, policy-enforcing runtime" | §0, L13 | landscape §H/§J `:540-549` | OPINION | Survey conclusion. |
| 11 | Adopt beads via `bd --json`; Symphony SPEC; `agent-client-protocol`; Vibe Kanban Rust crates | §0, L13 | landscape `:542-545`, `:154-169` | UNVERIFIED (design) / partly VERIFIED | Symphony SPEC/WORKFLOW.md front matter re-fetched in T0230 §1.4 (VERIFIED). ACP crate "2.0.0 (2026-07-23)" and Vibe Kanban crate names are landscape's own crates.io/GitHub reads (2026-08-17), not re-fetched. Suggest: `cargo info agent-client-protocol` when adapters start. |
| 12 | Gas Town: "copy the bones, don't run it" | §0, L13 | beads-and-gastown §2.5–2.6 `:154-172` | OPINION | Recommendation; the cost/criticism facts behind it are rows 60–61. |
| 13 | "second round had zero fleet conflicts and zero idle" | §1, L18 | as-built §3.1 `:183` | VERIFIED-LOCAL, caveat added | `round-2026-08-15-evening-retrospective.md:104-118` ("no lane reported a single conflict"; "Every lane reports zero idle"). Same file `:30`: idle is "self-reported, unverified" — SYNTHESIS now says "(idle self-reported)". |
| 14 | "`make land` caught 'green alone / red together' twice" | §1, L18 | as-built §3.3 `:185` | VERIFIED-LOCAL | `round-2026-08-15-evening-retrospective.md:77-82` ("§2.5 … twice"; "`main` was rewound once"). |
| 15 | "verify went 238–284 s → 50 s" | §1, L18 | as-built §3.4 `:186` | VERIFIED-LOCAL | `round-2026-08-15-evening-retrospective.md:120-124` (measured on three trees with a control run). |
| 16 | "~131 beads/day" | §1, L18 | as-built §3.2 `:184` | VERIFIED-LOCAL | `research-actions.md:89` "131 beads closed that day, against 46 on 08-14 and 27 on 08-13". Note it is one day's peak, not a rate. |
| 17 | "friction beads fixed same day" | §1, L18 | as-built §3.6 `:188` | VERIFIED-LOCAL | `reference/multi-agent.md:163-175` (five in the first hour; two became `make worktree-setup`/`make verify` the same day). |
| 18 | "dedup only ~3%" | §1, L18 | as-built §3.11 `:193` | VERIFIED-LOCAL | `plans/0022:87-90` (6 pairs in 202); `bead-admission-control.md:24` (1 in 152). |
| 19 | `make land` = refuse → digest → `--no-ff` → verify merged tree → rewind → close by evidence | §1, L18; §4.1 L86 | as-built §2.5 | UNVERIFIED (int) | `scripts/land.sh` is not in the notes copies; the design is described in `merge-automation-research.md:247-363` (corpus `:137`). Low risk. Suggest: read `land.sh` when `air land` is ported (M1). |
| 20 | "ship path deferred and invisible for a month" | §1, L21 | as-built §4.1 `:202` (plan `0021-release-cut.md:13-66`) | UNVERIFIED (int) | Plan 0021 is not in `adopter-notes/plans/`. `defer-sweep-2026-08-17.md:21` confirms `ad-7vw` P1 epic deferred; the "month" is 0021's. Suggest: re-copy plan 0021 into adopter-notes (bump PROVENANCE). |
| 21 | "113/304 closed beads were fleet-on-fleet" | §1, L21 | as-built §4.1 `:202` (plan 0021) | UNVERIFIED (int) | Same source gap as row 20. Corroborating local datum: second round 32 meta + 9 mixed of 76 closed (`round-2026-08-15-evening-retrospective.md:33-37`). Suggest: copy 0021. |
| 22 | "no coordinator digest" | §1, L21 | as-built §4.1 `:203` | VERIFIED-LOCAL | `round-2026-08-15-evening-retrospective.md:43-49` "the coordinator wrote none". |
| 23 | "coordinator relays were the least reliable channel" | §1, L21; §1b L41 | as-built `:218` | VERIFIED-LOCAL | `round-2026-08-15-evening-retrospective.md:56-58` verbatim; `overnight-fleet-retrospective.md:147-148` (four wrong relays). |
| 24 | "a 4.5 h interactive-prompt stall" | §1, L22 | as-built §4.2 `:208`; enforcement rank 1 | VERIFIED-LOCAL | `overnight-fleet-retrospective.md:167-188` ("dead for 4.5 hours, holding a claim"). |
| 25 | "51 min at WIP 0" | §1, L22 | as-built `:209` | VERIFIED-LOCAL | `rules/main-agent-protocol.md:45-47` ("idled 51 minutes waiting"); evening retro `:115`. |
| 26 | "reclaim threshold 8 h vs 4.5 h damage" | §1, L22 | enforcement rank 1 `:182`; as-built `:154` | VERIFIED-LOCAL | `overnight-fleet-retrospective.md:618-619` ("PARTIALLY ENFORCED, at four times the useful granularity … 8 hours"). |
| 27 | "`--claim` reopens closed beads" | §1, L23 | enforcement rank 3 `:184`; as-built `:210` | **STALE → FIXED** | adopter retracted it: `rules/main-agent-protocol.md:136-139` "**That is false** … `--claim` on a closed bead fails … 'issue not claimable: status closed'". as-built `:210` already notes the retraction; enforcement rank 3 still repeats it. SYNTHESIS now names `bd close`-on-closed as the real defect and `bd update -s in_progress` as the silent reopen. |
| 28 | "`bd close` on closed echoes success" | §1, L23 | as-built `:210` | VERIFIED-LOCAL | `main-agent-protocol.md:141-152` (proved with FIRST/SECOND reason). |
| 29 | "`ready` caps at 100" | §1, L23 | as-built `:211` | VERIFIED-LOCAL + T0300 | `bead-dedup-audit-2026-08-17.md:134-140` (100 of 149); T0300 row 4.1 `DefaultReadyLimit = 100`, `--limit 0` unlimited. |
| 30 | "`--notes` overwrites" | §1, L23 | as-built `:212` | VERIFIED-LOCAL | `bead-dedup-audit-2026-08-17.md:31-44`; enforcement `:295-296`. |
| 31 | "anonymous claims from main" | §1, L23 | as-built `:213` | VERIFIED-LOCAL | `main-agent-protocol.md:63-93` (7 beads all `AJTJ`). |
| 32 | "validation *warns*, teaching fabrication" | §1, L23 | enforcement B4 `:59`, rank 4 `:185` (`.beads/config.yaml:71-96`) | UNVERIFIED (int) | `.beads/config.yaml` not in copies; as-built `:38` records `validation.on-create: warn`. "Teaching fabrication" is the enforcement report's inference. Suggest: `bd config get validation.on-create` in adopter (read-only). |
| 33 | "`CLAUDE.md` 977 lines / ~16.6k tokens per session" | §1, L24 | as-built `:230` | VERIFIED-LOCAL | `plans/0022:19` "977 lines, 12,309 words, roughly 16.6k tokens"; as-built `:49`. |
| 34 | "rules written three times before they bound" | §1, L24 | as-built `:230` | VERIFIED-LOCAL | `plans/0022:21-22` ("had to be written three separate times before they bound"). |
| 35 | "'add a rule to CLAUDE.md' measured as a dead end" | §1, L24 | as-built `:232` | VERIFIED-LOCAL (external basis MAST Wordle) | `research-actions.md:597-601`; the "measurement" is MAST's Wordle example (MAS2 row 55 confirms MAST v1 tables; the Wordle example itself not re-read). |
| 36 | "hooks resolve to the committing worktree's copy; guards that pass on nothing" | §1, L25 | enforcement W1 `:85`, rank 10 `:191`; as-built §4.6 `:236-238` | **STALE → FIXED** (conflation) | Two distinct facts: git hooks resolve to the committing worktree's copy (agent-editable; `guard-inventory.md:132`), and the Claude Code lease-guard runs `main`'s copy (worktree edit inert; as-built `:237`). SYNTHESIS now states both. "Two fitness checks matched nothing ever" — `guard-inventory.md:15-19` VERIFIED-LOCAL. |
| 37 | "9 of 14 protocol items `NOT ENFORCED`" | §1, L26 | as-built `:173`; research-actions `:268` | VERIFIED-LOCAL | Nine literal markers counted in `overnight-fleet-retrospective.md` (grep = 9); `research-actions.md:268` lists items 1,2,3,6,8,9,10,13,14. |
| 38 | adopter's one-line lesson: enforced = negative/artifact-shaped; prose = positive/sequential | §1, L28 | enforcement §0 `:20-27` | VERIFIED-LOCAL (report's own words, marked as such) | The framing is the enforcement report's, built on `0022:29-39`. |
| 39 | "a single-machine design — nothing here travels" | §1, L30; §3 L71 | as-built §5 `:268-272` (plan `0010:990-1004`) | VERIFIED-LOCAL | `plans/0010` §12.0c (quoted in as-built). |
| 40 | Rust pays "when the binary leaves the machine" | §1, L30 | as-built `:278-281` (`metis-comparison.md:283-289`) | **STALE → FIXED** (quote) | Verbatim is "Rust pays for tooling when the binary has to run on a machine that does not have your toolchain" (`metis-comparison.md:285`); SYNTHESIS now quotes it verbatim. |
| 41 | 0022 wants "a working procedure … lock into … not held in context"; PreCompact/Stop/PostToolUse; none built | §1, L30 | as-built `:300-302`; `0022:9-13` | **STALE → FIXED** (quote) + VERIFIED-LOCAL | Quote made verbatim ("a working procedure that multiple agents can lock into, and doesn't need to be held in context"). Hooks at `0022:164-212`; "none built" per `0022:259` and as-built §5. |
| 42 | LLM MAS literature (MAST, MetaGPT, AutoGen, debate) does not describe worktree fleets | §1b, L36 | corpus §0.1 `:15` | OPINION (corpus inference) | MAST bibliographic facts confirmed MAS1 row 26 / MAS2 row 55; the "does not describe" is the corpus's reading. |
| 43 | "51% on impossible tasks, Opus 4.1 addendum; anti-hack → 19% ≈ 2.7×; 55%→35% on Opus 4.5 — not 9×" | §1b, L37 | corpus §0.2; MAS2 rows 56–57 | CORRECTED-ALREADY | Commit 3e208d8 applied MAS2 rows 56–57. |
| 44 | "5–30% of 'verified' patches wrong" | §1b, L37 | corpus §0.2 `:17` | VERIFIED (nuance) | MAS2 row 41: confirmed as a synthesis; the 32.67% input is leakage, so the wrong-patch range proper is ≈4–31%. Text left as is (range unchanged in substance). |
| 45 | "Ontario checklist null result" | §1b, L38 | corpus §0.3 `:19` | VERIFIED | MAS1 row 4 (101 hospitals; OR 0.91, P=0.13). |
| 46 | "CAID soft-isolation below single agent" | §1b, L38 | corpus §0.3 | VERIFIED, condition added | FLEET rows 6/8: true on PaperBench (55.5 < 57.2), not on Commit0-Lite (56.1 > 53.1). SYNTHESIS now says "(PaperBench arm)". |
| 47 | adopter's "eight of fourteen are wishes" | §1b, L38 | corpus §0.3 (`overnight-fleet-retrospective.md:520-523`) | VERIFIED-LOCAL | Verbatim at `overnight-fleet-retrospective.md:520-523`; corpus `:1357` records the 8-vs-9 miscount (row 37). |
| 48 | Dias et al. 2020: OR 6.13 for overlap; 73,504 / 125; not "6× vs branch lifetime" | §1b, L39 | corpus §0.4 `:21`; FLEET rows 9–10 | CORRECTED-ALREADY + **STALE → FIXED** (variable) | Population correction was already in; FLEET row 10 / not-repeat #2 also says "common slice ≠ same file" and slice-disjoint work still conflicts on config files — SYNTHESIS now says "common MVC *slice*". |
| 49 | "Partition quality dominates existence (CAID)" | §1b, L39 | corpus §0.4 (`does-the-prior-art-transfer.md:2971-2975`) | VERIFIED | FLEET row 7 (8.7% vs 34.3%, minitorch; one repo/model — case study). |
| 50 | "sub-file claiming and lock-only registries measurably fail" | §1b, L39 | corpus §0.4 (`does-the-prior-art-transfer.md:2654-2668` Claim Plane 2608.00947; `:2633-2650` grite 2606.19616) | UNVERIFIED (ext), caveat added | Neither paper is in a verification slice. The note itself flags grite as a *simulation* and Claim Plane as a single-author preprint; SYNTHESIS now carries both caveats. Suggest: fetch both abstracts (10 min). |
| 51 | "keep exactly one inter-agent message" | §1b, L39 | corpus §0.4 / research-actions | VERIFIED with tension | FLEET row 14 (CooperBench: communication beyond first-turn plan does not help) vs row 13 (AgentRadio: passive messaging is the gain on interdependent read-only subtasks) — FLEET "gaps" #3 names the A/B. |
| 52 | "throughput = 2/L; review latency measured nowhere" | §1b, L40 | corpus §0.5 `:23` | VERIFIED | FLEET row 23 (arithmetic; L unmeasured); FLEET row 33 adds 2026 field evidence (latency +3.5×). |
| 53 | Kim et al.: thin beyond 3–4 agents; β<0 where single agent already succeeds | §1b, L41 | corpus §0.6 `:24` | VERIFIED | FLEET rows 1–2 (verbatim; caveats: Fig. 5 only, n on *one* task). |
| 54 | Kim's 58/285/515% describe redundant solvers, so "5–9× orchestrator overhead" does not follow | §1b, L41 | FLEET row 3 | CORRECTED-ALREADY | Commit 7676440. |
| 55 | AgentRadio +29.8 pp needs its messaging channel; division of labour alone +7.2 pp at ~6× cost | §1b, L41 | FLEET row 13 | CORRECTED-ALREADY | Commit 7676440. (Row 13 says 39.5% vs 32.3% = +7.2; cost $19.45 vs $2.96 ≈ 6.6×.) |
| 56 | Coordinator "tells workers when to merge and assigns beads based on what peers hold" (owner, 2026-08-17) | §1b, L41; §4.4 L105 | coordinator-interview `:21-34` | VERIFIED-LOCAL | Interview answers 2 (merge timing) and 3 ("name 1–3 candidates … filtered by what peers hold by file"). |
| 57 | Resources: supervisor owns count; `flock`/`SEM_UNDO`; level-triggered; deadlines ask; cheap liveness signal | §1b, L42 | corpus §0.7 `:25` | VERIFIED | PROTO rows 22–23 (`SEM_UNDO`, applied at `exec()` on Darwin), 27–30 (k8s level-triggered, probes, systemd watchdog), 33 (Rust crates). "Deadlines ask, don't kill" is the note's maxim (opinion). |
| 58 | Checkpoint = write deliverable incrementally; crash-only; wrap-up hook protects the wrong case | §1b, L43 | corpus §0.8 `:26` | VERIFIED | PROTO "claims strengthened": Candea & Fox, Young 1974 quoted correctly (rows 46–47); T0245: PreCompact fires too late for evidence capture. |
| 59 | Task specs: acceptance = one observable condition; write the check | §1b, L44 | corpus §0.9 `:27` | VERIFIED (design) | SPECS "claims strengthened" (acceptance criteria are the weak link; ConInstruct row 42; SpecBench row 9); T0230 §1.5. |
| 60 | CLI "5–28× cheaper" misattributed; equal failure frequency; 12.9% vs 2.2% wasted spend | §1b, L45 | MAS2 row 42 | CORRECTED-ALREADY | Commit 3e208d8. |
| 61 | Rust: 0.3 s gap, 21 s cold build, guard blocks `cargo run`; reopens for a shipped binary (Metis) | §1b, L46 | corpus §5 `:1404-1422` | VERIFIED-LOCAL + VERIFIED | `repo-tooling-language.md:19,111,119,142` (LOCAL per SPECS row 33); SPECS row 35 (Metis ships via curl/Tauri; angreal). "Code fails confidently" verbatim at `metis-comparison.md:360`, `round-2026-08-15-evening-retrospective.md:73`. |
| 62 | Plan 0010 §8 rejected a blocking Stop hook; merge-automation advisory; Metis's is the model | §1b, L47 | corpus §4.1 `:1352` | VERIFIED-LOCAL + SPECS | `plans/0010:673` ("a blocking `Stop` hook can wedge a session"); SPECS row 32 (advisory Stop shape confirmed); T0245 (exit-2 blocks; 8-block cap). |
| 63 | CAID vs STORM: worktree ≈ soft isolation; single-agent baseline moved 53.1→66.4 | §1b, L47 | FLEET row 8 | CORRECTED-ALREADY | Commit 7676440. |
| 64 | "`make lease-break` is both the priority-inversion fix and the split-brain path" | §1b, L47 | corpus §4.1 `:1351` (`agent-protocol-prior-art.md:895-900`) | VERIFIED-LOCAL + PROTO | PROTO row 56 (lease-break ↔ split-brain reasoning holds against Kleppmann/Chubby). |
| 65 | Open questions: `bd heartbeat` while blocked on stdin; four simultaneous `make verify` never timed | §1b, L48 | corpus §4.2 `:1368-1369` | VERIFIED-LOCAL | `research-actions.md:650-660`; `fleet-run-structure.md:839-844`; MAS1 gaps #1 (219.7 s / load-207 datum carries §3). |
| 66 | beads lease schema (`lease_expires_at`, `heartbeat_at`, `granted_node`, grace = 2×TTL — on `main`, not in a tested release) | §2 table, L56 | landscape `:44`; beads-and-gastown §1.7 | VERIFIED (T0300) | T0300 rows 3.1 (fields; dolt_ignored table), 3.2 (TTL 5 m constant; `reclaim --older-than` default 2×TTL), 1.2 (not in v1.2.2). |
| 67 | "Use beads via `bd --json` (`ready --claim`, CAS `--if-*`, gates, formulas/molecules, events journal, JSONL export)" | §2, L56 | landscape `:585`; beads-and-gastown §1.5 | **STALE → FIXED** | T0300 row 2.3: `--if-*`, `unclaim`, events, leases absent from 1.2.2; T0300 §3c minimal surface; T0230: do not build/rely on formulas/molecules/gates. Cell now lists the 1.2.2 surface and marks the rest. |
| 68 | Symphony SPEC: `WORKFLOW.md` front matter; tick = reconcile→validate→fetch→sort→dispatch; states; stall/turn timeouts; `/api/v1/state` | §2, L57; §4.1 L85 | landscape `:95` | VERIFIED (partial, T0230 §1.4) | T0230 re-fetched SPEC.md/WORKFLOW.md (front matter, §5.3.1, §8.2 eligibility + order, §8.5, §10.5). Claim-state names and run-attempt states are landscape's SPEC read (2026-08-17), not re-fetched — low risk. |
| 69 | `agent-client-protocol` 2.0; Codex `app-server` | §2, L58 | landscape `:164-169`, `:73-76` | UNVERIFIED (ext) | crates.io 2.0.0 (2026-07-23) per landscape. Suggest: `cargo info agent-client-protocol` before writing the adapter. |
| 70 | Claude adapter: `claude -p --output-format stream-json`, `--worktree`, hooks exit-2 blocking, `total_cost_usd` | §2, L58, L63 | control-surfaces `:279`; landscape `:113-120` | VERIFIED (T0245) | T0245: exit 2 blocks (PreToolUse/Stop), `--worktree` semantics, WorktreeCreate/Remove; `total_cost_usd` in control-surfaces JSON sample. |
| 71 | Vibe Kanban `executors` / `worktree-manager`; Factory cleanup policy | §2, L58–59 | landscape `:154-159`, `:428`, `:544` | UNVERIFIED (design) | Landscape's GitHub reads. Suggest: clone and `cargo metadata` when borrowing. |
| 72 | Gas Town Refinery (batch + bisect + gates) | §2, L60; §4.4 L107 | landscape `:545`; beads-and-gastown `:18` | VERIFIED | SPECS row 28 (README verbatim: "Bors-style bisecting queue"). |
| 73 | Overstory 4-tier conflict ladder + merge lock; "CI is the only gate" (multiclaude) | §2, L60 | landscape `:197-199`, `:394` | UNVERIFIED (design) | Landscape reads of archived repos. Low load-bearing. |
| 74 | ACP `session/request_permission`; beads `human` gates; adopter `human`/`owner` labels; Gas Town escalation severities + ack; "Needs You" queue (Conductor) | §2, L61 | landscape `:167`, `:60`, `:327`; enforcement B14; SPECS row 29 | Mixed | `bd gate` exists (SPECS row 29 LOCAL; T0300 row 2.3 present in 1.2.2); adopter labels VERIFIED-LOCAL (`enforcement:69`); ACP/Gas Town/Conductor items UNVERIFIED (design). |
| 75 | Gas Town Witness/Deacon + GUPP ("no progress = violation") | §2, L62; §4.1 L85 | landscape `:56-63`, `:591` | UNVERIFIED (design) | Landscape's Gas Town docs read. Suggest: none needed unless GUPP is copied literally. |
| 76 | Cost: `total_cost_usd`/`modelUsage`; OTLP; Paperclip budget hard-stops; Overstory per-bead cost | §2, L63 | landscape `:592`, `:375`, `:61` | Mixed | `total_cost_usd` VERIFIED (row 70); Paperclip/Overstory UNVERIFIED (design). |
| 77 | metis: filesystem is truth, SQLite disposable index; forward-only transition tables; short codes; mtime guard | §2, L64; §4.2 L89 | metis-deep-dive §4 (local checkout `~/projects/metis`) | VERIFIED-LOCAL (repo read) | SPECS row 35 confirms Metis repo facts (six crates now, Tauri/MCP/plugin). |
| 78 | Anthropic ralph-loop plugin state file; metis Stop-hook contract "block exit until evidence" — gate on `make verify`, never on a token | §2, L65 | landscape `:248-251`; metis §3; `0022:175-177` | VERIFIED-LOCAL (Metis `<promise>` mechanism at `0022:177`) / ralph UNVERIFIED (ext) | Metis's Stop hook parses `<promise>TASK COMPLETE</promise>` (`0022:177`). ralph-wiggum plugin location is landscape's read. |
| 79 | Gas Town: "chaotic and sloppy", ~$100/h, "verification chain remains open" | §2, L67 | beads-and-gastown §2.5 `:156-159` (Yegge; Sehn 2026-01-15; Atwood 2026-05-14) | UNVERIFIED (ext, quoted with URLs) | Origin quotes with URLs; not re-fetched by any slice. Suggest: none — used only to reject wholesale adoption. |
| 80 | Restate/Temporal/Windmill "external server, wrong granularity"; `beads_rust`/`br` pre-Dolt; AGPL tools excluded | §2, L67 | landscape `:547`, `:40`, `:205-210` | OPINION / UNVERIFIED (design) | Rejection reasons; `br` licence rider is landscape's GitHub read. |
| 81 | Commitment point so `bd create` ≠ `bd ready` | §3, L72 | enforcement §4.6 `:292-303` | VERIFIED-LOCAL + T0230 | `enforcement:300` "every observation … is instantly committed work"; T0230 verdict: enforce the triage commitment point. |
| 82 | Installed hooks must resolve to the runtime binary; verify the resolved path | §3, L73 | enforcement rank 10 `:191` | VERIFIED-LOCAL | `guard-inventory.md:132`. |
| 83 | Guards that fail confidently are the anti-pattern | §3, L75 | as-built §4.6 `:236-238` | VERIFIED-LOCAL | `round-2026-08-15-evening-retrospective.md:69-75`; `guard-inventory.md:15-19`. |
| 84 | Hook events `SessionStart`, `PreToolUse`, `PostToolUse`, `PreCompact`, `Stop`, `SubagentStop` return allow/deny/additionalContext | §4.1, L83 | control-surfaces; T0245 | VERIFIED (T0245) | All exist; note `SubagentStop` is informational (cannot block) and `SessionEnd` is not guaranteed on SIGKILL (T0245). |
| 85 | "CLI-first (corpus: CLI 5–28× cheaper than MCP)" | §4.1, L84 | corpus §5.3 | **STALE → FIXED** | MAS2 row 42 / not-repeat #3: the multiplier is a scaffolding effect; paired ratios 0.43×–29×. L84 now matches L45. |
| 86 | Ten checks = enforcement's ranked list (1 stall … 10 hook currency) | §4.3, L91–101 | enforcement §3 `:180-191` | VERIFIED-LOCAL | Mapping rank-for-rank matches; each row's evidence items are adopter's own record (rows 24–37 above). |
| 87 | Symphony `max_concurrent_agents_by_state` | §4.4, L104 | landscape `:95` | VERIFIED (T0230 §8.2 "global and per-state slots free") | — |
| 88 | M0 measure: protocol items enforced 5/14 → ≥9/14 in `make fitness` | §5, L113 | research-actions `:268` | VERIFIED-LOCAL | 2 E + 2 P + 1 not-a-check = 5 of 14 today. `make fitness` counting hook is adopter-internal (not copied). |
| 89 | Install alongside `cmd-guard.py` in `~/.claude/settings.json`; `air land` replaces `scripts/land.sh` with `land-prove` green | §5, L113–114 | as-built `:137`, enforcement `:135` | UNVERIFIED (int) | Script names from as-built's read of adopter; not in notes copies. Suggest: confirm paths at M0 install time. |
| 90 | bd version trap: 1.2.1 accidental/untested; 1.2.2 drops leases/heartbeat/reclaim/events/`sync`/serve; refuses v65 schema | §6, L121 | beads-and-gastown §0, §1.7 | **STALE → FIXED** (incomplete + decision taken) | T0300 rows 1.2, 2.3, 5.4: 1.2.2 also lacks `unclaim`, `--if-*` (exit 13), `update --force`, `--brief`; recovery = cursor rollback; recommendation = pin 1.2.2 + ledger owns CAS (plan 0001 §7, commit cf65fad). SYNTHESIS updated. |
| 91 | Hooks in `~/.claude/settings.json` are user-scoped; per-worktree settings must be written by the runtime | §6, L122 | control-surfaces; as-built `:270` | VERIFIED (T0245) | T0245: parent repo's `.claude/settings.json` hooks apply inside worktrees; user scope per docs. |
| 92 | Billing "primary-sourced as of 2026-08-17" | §6, L123 | billing `:2` | UNVERIFIED (ext) | Self-description; no slice re-fetched billing (rows 5–8). |
| 93 | `air`/"Air" name — owner's choice 2026-08-17 | §4, L80 | commit 68d8cd5 | OPINION (record) | — |
| 94 | Non-goals, M0–M2 scope, topologies, backends | §4.4, §5 | — | OPINION | Design. |

## 2. Counts

| Status | Rows |
|---|---|
| VERIFIED (a verification slice or tick row) | 17 — rows 44, 45, 46, 49, 51, 52, 53, 57, 58, 59, 66, 68, 70, 72, 84, 87, 91 (+ the verified halves of mixed rows 74, 76, 78) |
| VERIFIED-LOCAL (checked against `adopter-notes/` copies) | 37 — rows 2, 3, 4, 7, 13–18, 22–26, 28–31, 33–35, 37–39, 47, 56, 61, 62, 64, 65, 77, 81–83, 86, 88 |
| CORRECTED-ALREADY | 6 — rows 43, 48 (population), 54, 55, 60, 63 |
| STALE → FIXED this tick | 9 — rows 1, 27, 36, 40, 41, 48 (variable), 67, 85, 90 (+ caveats added in rows 13, 46, 50) |
| UNVERIFIED — external / design | 14 — rows 5, 6, 8, 11 (ACP/Vibe), 50, 69, 71, 73, 74 (part), 75, 76 (part), 79, 80 (part), 92 |
| UNVERIFIED — adopter-internal, source not in copies | 5 — rows 19, 20, 21, 32, 89 (+ row 88's `make fitness` counter) |
| OPINION | 7 — rows 9, 10, 12, 42, 80 (part), 93, 94 (+ maxims inside 57) |

Total ledgered: **94** rows (several rows bundle two or three sentences from the same line; rows 48 and 80 appear in two categories). Of the 94: 60 verified (17 external + 37 local + 6 already corrected), 9 fixed this tick, 19 unverified (14 external/design, 5 internal), 7 opinion.

## 3. Remaining UNVERIFIED items and how to close each

| Row | Item | Suggested check |
|---|---|---|
| 5, 6, 8, 92 | Subscription pooling; "3–5 concurrent"; Agent SDK auth paused 2026-06-15 | Re-fetch `code.claude.com/docs/en/costs.md` and support article 15036540 before any capacity decision; treat "3–5" as an estimate and replace with the measured `rate_limits.five_hour.used_percentage` at fleet size 4 (PROTO row 45 gives the field). |
| 11, 69 | `agent-client-protocol` 2.0.0; Codex `app-server` | `cargo info agent-client-protocol`; `codex app-server --help` when the adapter milestone (M2) starts. |
| 11, 71, 73, 75, 76 | Vibe Kanban crates; Factory cleanup policy; Overstory ladder; GUPP; Paperclip/Overstory cost | Landscape reads of 2026-08-17; only re-check the one being borrowed at the time (clone + read). |
| 19 | `make land` step order | Read `~/projects/adopter/scripts/land.sh` (read-only) when porting `air land` (M1); or copy `merge-automation-research.md`'s design section is already in notes. |
| 20, 21 | "invisible for a month"; 113/304 fleet-on-fleet | Copy `docs/plans/0021-release-cut.md` from adopter `f2ca891` into `adopter-notes/plans/` and bump PROVENANCE; then cite `0021:13-66`. |
| 32 | `validation.on-create: warn` | `bd config get validation.on-create` in adopter (read-only) or copy `.beads/config.yaml:71-96`. |
| 50 | Claim Plane (2608.00947) / grite (2606.19616) | Fetch both abstracts; confirm Claim Plane's static-vs-dynamic table and grite's "simulation" disclaimer (10 min). |
| 79 | Gas Town ~$100/h, "verification chain remains open" | Not needed for the decision (wholesale adoption rejected on structural grounds); the URLs are in `beads-and-gastown.md:157-159`. |
| 89 | `cmd-guard.py`, `scripts/land.sh`, `land-prove` paths | Confirm at M0 install time against the live adopter checkout. |

## 4. Edits made to `docs/research/SYNTHESIS.md` this tick

1. Header: added a 4-line "Verification status" note linking here.
2. §0 L10: 66/27% re-attributed to the enforcement report's tally (row 1).
3. §1 L18: "zero idle (idle self-reported …)" (row 13).
4. §1 L23: `--claim`-reopens claim replaced by adopter's retraction and the real defects (row 27).
5. §1 L25: hook-copy sentence split into the two true mechanisms (row 36).
6. §1 L30: two quotations made verbatim with file:line (rows 40–41).
7. §1b L38: "(PaperBench arm …)" (row 46).
8. §1b L39: Dias "common MVC slice" + population wording; Claim Plane/grite caveats (rows 48, 50).
9. §2 L56: beads "Use" cell rewritten to the 1.2.2 surface (row 67).
10. §4.1 L84: CLI 5–28× replaced with the MAS2 row 42 wording (row 85).
11. §6 L121: bd version trap completed and the pin-1.2.2 decision recorded (row 90).

Not changed: the enforcement report's rank-3 row still repeats the retracted `--claim` claim
(`adopter-enforcement-and-skills.md:184`) — out of scope for this tick (SYNTHESIS only); flagged here.

## Sources

- `docs/research/SYNTHESIS.md` (pre-edit line numbers)
- `docs/research/verification/{fleet-size-partition-cadence,specs-guards-tooling,protocols-leases-resources,mas-literature-part1,mas-literature-part2}.md`
- `docs/research/verification/ticks/2026-08-18-{0230,0245,0300,0315}-*.md`
- Origin reports: `adopter-as-built.md`, `adopter-enforcement-and-skills.md`, `adopter-research-corpus.md`, `claude-code-billing.md`, `beads-and-gastown.md`, `prior-art-landscape.md`, `claude-code-control-surfaces.md`, `metis-deep-dive.md`, `coordinator-interview-2026-08-17.md`, `worker-interviews-2026-08-17.md`
- `docs/research/adopter-notes/` verbatim copies (commit `f2ca891`): `notes/{round-2026-08-15-evening-retrospective,overnight-fleet-retrospective,research-actions,bead-dedup-audit-2026-08-17,bead-admission-control,guard-inventory,repo-tooling-language,metis-comparison,does-the-prior-art-transfer,defer-sweep-2026-08-17}.md`, `plans/{0010,0022}*.md`, `rules/main-agent-protocol.md`, `reference/multi-agent.md`
- Git history: commits 7676440, 3e208d8 (prior SYNTHESIS corrections), cf65fad (plan 0001 ledger owns CAS)


## Addendum 2026-08-18 03:45 — adopter-internal rows closed

| Row | Item | Now | Evidence |
|---|---|---|---|
| 19 | `make land` step order | VERIFIED-LOCAL | `adopter-notes/config/land.sh:554-663`: "what lands" → merge `--no-ff` (560) → `yarn install` (601) → fast checks (622) → `make verify` on the MERGED result (649) → `close_beads` (663); digest tiering at 305; rewind logic in the merge/verify blocks. |
| 20 | "for a month the ship path has been…" | VERIFIED-LOCAL | `adopter-notes/plans/0021-release-cut.md:48`. |
| 21 | 113/304 closed beads fleet-on-fleet | VERIFIED-LOCAL | `adopter-notes/plans/0021-release-cut.md:29,35` ("Of 304 closed beads, 113 are pure agent infrastructure"; 145 product-only, 26 mixed, 20 unlabelled). |
| 32 | `validation.on-create: warn` teaches fabrication | VERIFIED-LOCAL | `adopter-notes/config/beads-config.yaml:89-94,109-110` ("`on-create: warn` does NOT refuse"). |
| 89 | guard/land/land-prove paths | VERIFIED-LOCAL (paths exist 2026-08-18) | `scripts/lib/cmd-guard.py` (28.6K), `scripts/land.sh` (39.3K), `scripts/lib/stop_guard.py` (8.6K) present in the adopter checkout; `make fitness` prints "still prose only — not enforced by anything" at `scripts/fitness.sh:531-537`. |

Remaining UNVERIFIED after this tick: external/design rows 5, 6, 8, 11, 50, 69, 71, 73, 74(part), 75, 76(part), 79, 80(part), 92 (14).
