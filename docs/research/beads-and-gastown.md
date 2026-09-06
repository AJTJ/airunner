# Beads and Gas Town: what they are, what they give us, and what ai_runner should build on top

Research note for ai_runner (Rust orchestration runtime for fleets of coding agents in git worktrees).
Written 2026-08-17/18. Every claim cites a URL or a local path; things that could not be verified are marked as such.

Reading order if short on time: §0 (findings that change decisions), §1.7 (leases), §2.6 (should we use Gas Town?), §5 (implications).

---

## 0. Headline findings (the ones that affect ai_runner decisions)

> **Correction note, 2026-08-18 03:00 (tick 0300, `docs/research/verification/ticks/2026-08-18-0300-bd-1-2-x-facts.md`).** Re-verified against the GitHub releases/tags/commits API, main `CHANGELOG.md`, and the `v1.1.2` tag in the local clone. Still true: latest release is v1.2.2 (2026-08-15) = v1.1.2 code; no 1.2.3/1.3.0 exists or is announced. Two things below are incomplete: (i) item 2's list of what v1.2.2 lacks must also include **`bd unclaim` (whole command), `bd update --if-assignee/--if-status` (CAS, exit 13), `bd update --force` and its anti-steal refusal, `--brief`, `claim.pools`** — §1.5's `bd unclaim --if-assignee` and "refuses to steal / `--force`" lines are 1.2.1-only; (ii) §1.5's `bd ready` rule is really `status = 'open'` only (custom statuses such as the adopter's `awaiting_review` are excluded — upstream #5831), plus type/pinned/ephemeral/`defer_until` exclusions, default `--limit 100`. Also: lease TTL is a compile-time 5 m constant with no flag/config; leases live in a dolt_ignored per-clone `leases` table; heartbeat is authorised by actor string only. The adopter has 0 `in_progress` issues today (the four leased issues cited in item 2 are gone). Homebrew now offers 1.2.2 (`brew outdated beads`).

1. **beads is now Dolt-backed, not SQLite+JSONL.** "Distributed graph issue tracker for AI agents, powered by Dolt." Embedded Dolt (single writer, file-locked) is the default; a `dolt sql-server` mode exists for concurrent writers. `.beads/issues.jsonl` is "an export for viewers and interchange, not the source of truth or a backup." (README, https://github.com/steveyegge/beads — the repo redirects to `gastownhall/beads`; local `metadata.json` confirms `"backend":"dolt","dolt_mode":"embedded"` at `the adopter's .beads/metadata.json`.)
2. **The installed `bd 1.2.1` is an *accidental, untested* release.** v1.2.0/1.2.1 "were published by accident on 2026-08-11, without release testing. v1.2.2 superseded them by re-releasing the tested 1.1 line." The 1.2.x-only features — **work leases (`bd heartbeat`/`bd reclaim`), events journal, `bd sync` federation loop, HTTP API (`bd serve`), provenance events — are NOT in v1.2.2**; they "will return in a properly tested release." Running 1.2.1 once migrated the DB schema v53→v65, so a `brew upgrade` to 1.2.2 will refuse to open the adopter's DB until the schema cursor is rolled back. (https://github.com/steveyegge/beads/blob/main/docs/recovery/accidental-1-2-1-release.md; CHANGELOG https://github.com/steveyegge/beads/blob/main/CHANGELOG.md; local: `bd --version` → `bd version 1.2.1 (Homebrew)`, `the adopter's .beads/.local_version` = `1.2.1`.) Latest GitHub release: **v1.2.2 (2026-08-15)**; releases API also lists v1.2.1 (2026-08-11), v1.1.2 (2026-07-26), v1.1.0 (2026-07-04), v1.0.0 (2026-04-03) (https://api.github.com/repos/steveyegge/beads/releases).
   - The adopter is *actively using* 1.2.x-only features: `config.yaml` sets `events-journal: true`, and the four `in_progress` issues carry live `lease_expires_at`/`heartbeat_at` stamps 5 minutes apart (local `bd list --status in_progress --json`, 2026-08-18). **Design ai_runner so it does not hard-depend on lease/heartbeat/serve until they ship in a tested release, or pin `bd` at 1.2.1 deliberately.**
3. **beads is Go (≈225k+ lines, "100% vibe coded"), and there is no official Rust library.** Rust ports exist but track the *old* SQLite+JSONL architecture (`Dicklesworthstone/beads_rust`, binary `br`, 1,052 stars) or a git-refs redesign (`delightful-ai/beads-rs`, alpha). None is Dolt-compatible with current `bd`. ai_runner should treat `bd` as an external CLI (JSON in/out) or, later, the HTTP API. (§1.9.)
4. **Gas Town is a Go orchestrator over beads that already implements most of our lifecycle** (sling → worktree → work → `gt done` → Refinery merge queue → close), plus supervision (Witness/Deacon/Dogs), heartbeats, nudges, convoys, formulas, mail. It is tmux-centric, Claude-Code-centric, expensive (≈$100/hour at scale per its own v1.0 metrics), and widely criticised as over-complex. Its successor **Gas City** (`gastownhall/gascity`, Go, "orchestration-builder SDK") decomposes it into a controller/supervisor loop with pluggable runtime providers. **We can use it as a reference architecture and possibly interoperate via beads, but adopting it wholesale would replace ai_runner rather than inform it.** (§2.)

---

## 1. beads (`bd`)

### 1.1 What it is, language, ownership, docs

- One-liner: "Beads provides a persistent, structured memory for coding agents. It replaces messy markdown plans with a dependency-aware graph, allowing agents to handle long-horizon tasks without losing context." (README, https://github.com/steveyegge/beads, accessed 2026-08-17.)
- Repo now lives at `github.com/gastownhall/beads` (badges and links in README point there; `steveyegge/beads` redirects). Language: **Go** (GitHub API `language: Go`, 26.4k stars). Docs site: https://beads.gascity.com/ (Mintlify build of `docs/`).
- Origin story (Yegge): "In October, I told Claude in frustration to put all my work in a lightweight issue tracker. I wanted Git for it. Claude wanted SQLite. We compromised on both, and Beads was born, in about 15 minutes of mad design." Later: "I've never looked at Beads either, and it's 225k lines of Go code" (Welcome to Gas Town, https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04, 2026-01-01; text retrieved via web.archive.org).
- v1.0 (2026-04-03) coincided with the full migration to Dolt: "celebrates the migration to Dolt (a Git-compatible database) that resolved architectural fragility" (Gas Town: from Clown Show to v1.0, https://steve-yegge.medium.com/gas-town-from-clown-show-to-v1-0-c239d9a407ec).

### 1.2 Data model

Source: https://github.com/steveyegge/beads/blob/main/docs/architecture/index.md and `bd schema` (run locally, bd 1.2.1).

Five record kinds: **issues** (beads), **dependencies** (typed edges), **labels**, **comments**, **events** (audit trail).

Issue fields (from `bd schema`, JSON Schema draft 2020-12, `schema_version: 1`):
- Identity/content: `id`, `title`, `description`, `design`, `acceptance_criteria`, `notes`, `spec_id`, `external_ref`, `source_system`, `metadata` (free JSON), `labels[]`, `comments[]`, `dependencies[]`.
- State: `status` ∈ {`open`, `in_progress`, `blocked`, `deferred`, `closed`, `pinned`, `hooked`} (+ custom via `status.custom`; the adopter adds `awaiting_review`, `awaiting_testing` — local `bd statuses`), `priority` int 0–4 (0 = critical), `issue_type` ∈ {`bug`,`feature`,`task`,`epic`,`chore`,`decision`,`message`,`molecule`,`gate`,`spike`,`story`,`milestone`}, `is_blocked` (denormalised), `assignee`, `owner`, `estimated_minutes`, `pinned`, `is_template`.
- Timestamps/audit: `created_at`, `created_by`, `updated_at`, `started_at`, `closed_at`, `close_reason`, `closed_by_session`.
- Scheduling: `due_at`, `defer_until`.
- **Leases (1.2.x):** `lease_expires_at`, `heartbeat_at`, `lease_granted_node`.
- Compaction: `compaction_level`, `compacted_at`, `compacted_at_commit`, `original_size`.
- Wisps/molecules: `ephemeral`, `no_history`, `wisp_type`, `storage_class`, `bonded_from[]{source_id,bond_type,bond_point}`, `source_formula`, `source_location`, `mol_type`, `work_type`.
- Gates: `await_type`, `await_id`, `timeout`, `waiters[]`.
- Events-as-issues: `event_kind`, `actor`, `target`, `payload`, `sender`.
- Required: `id`, `title`, `priority`, `created_at`, `updated_at`. `additionalProperties: false`.

Dependency record: `issue_id`, `depends_on_id`, `type`, `created_at`, `created_by`, `metadata`, `thread_id`. **Type enum (19):** `blocks`, `parent-child`, `conditional-blocks`, `waits-for`, `related`, `discovered-from`, `replies-to`, `relates-to`, `duplicates`, `supersedes`, `authored-by`, `assigned-to`, `approved-by`, `attests`, `tracks`, `until`, `caused-by`, `validates`, `delegated-from`.

Blocking vs non-blocking (https://github.com/steveyegge/beads/blob/main/docs/core-concepts/dependencies.md): blocking = `blocks` (default), `parent-child` (children blocked when parent blocked), `conditional-blocks` (B runs only if A fails), `waits-for` (B waits for all of A's children). Non-blocking = `related`, `tracks`, `discovered-from`, `caused-by`, `validates`, `supersedes`, etc. Cycles are rejected at write time (`bd dep add` checks) and `bd dep cycles` audits.

Local sample (the adopter, 515 issues in `issues.jsonl`): statuses closed 312 / deferred 136 / open 62 / in_progress 4 / awaiting_review 1; edge types parent-child 203, blocks 136, discovered-from 35, relates-to 20, related 11, supersedes 3. Export rows also carry `_type:"issue"`, `dependency_count`, `dependent_count`, `comment_count`. (Local analysis of `the adopter's .beads/issues.jsonl`.)

### 1.3 IDs

Hash IDs: "content-derived hashes (of title, description, creator, and creation time, plus a collision nonce), not sequence numbers" so "two agents (or two branches) creating beads at the same time cannot mint the same ID." Hierarchical children: `bd-a3f8.1`, `bd-a3f8.1.1` (up to 3 levels). Adaptive length: 4 chars for 0–500 issues, 5 for 501–1500, 6 beyond, at a 25% max collision probability (configurable `min_hash_length`, `max_hash_length`, `max_collision_prob`); counter mode (`issue_id_mode=counter`) is available. (https://github.com/steveyegge/beads/blob/main/docs/core-concepts/hash-ids.md, https://github.com/steveyegge/beads/blob/main/docs/core-concepts/adaptive-ids.md.) The adopter uses prefix `fd-` with 3–4 char hashes (e.g. ``, `.3`).

### 1.4 Storage, sync, git integration

- **Modes** (https://github.com/steveyegge/beads/blob/main/docs/architecture/index.md): Embedded (`bd init`) — Dolt in-process at `.beads/embeddeddolt/`, "single writer, file-locked"; Server (`bd init --server`) — external `dolt sql-server`, `.beads/dolt/`, many writers; opt-in shared server at `~/.beads/shared-server/` for all projects. "Embedded mode is single-writer (enforced via file lock). If you need concurrent access, switch to server mode." (docs/architecture/dolt.md.) `bd serve` help says claims "are arbitrated in the SQL server."
- **Sync**: `bd dolt push` / `bd dolt pull` against `refs/dolt/data` on the git remote (or DoltHub/S3/GCS/filesystem). "Every write auto-commits to Dolt history." Cell-level merge. `bd sync` (1.2.x) = pull → positive conflict check → `recompute-blocked` → push with bounded retries; exit codes 0/1/2(conflict, halt)/3(retries exhausted)/4(stuck dirty working set). (`bd sync --help`, local.) The adopter's `config.yaml` sets `sync.remote: "git+ssh://git@github.com/owner/app.git"` and `export: auto: true, interval: 5s`.
- **JSONL**: passive export (`bd export`, `bd import`), and "not the database, not the sync protocol, and not a backup." Compaction: "Semantic 'memory decay' summarizes old closed tasks to save context window" (README); `bd compact`, `bd flatten`, `bd gc`, `bd prune`, `bd purge`, `bd restore` (local `bd help`).
- **Git hooks** (`bd hooks install`): pre-commit, post-merge, pre-push, post-checkout, pre-merge-commit, prepare-commit-msg ("Add agent identity trailers for forensics"). Thin shims call `bd hooks run <hook>` under a 300 s timeout and are chained to the repo's own hook (local `the adopter's .beads/hooks/pre-commit`, "BEGIN BEADS INTEGRATION v1.2.1"). Docs: https://github.com/steveyegge/beads/blob/main/docs/reference/git-integration.md.
- **Worktrees**: "All worktrees in the same repository use the same beads workspace unless you override discovery with `BEADS_DIR`"; `bd` discovers `.beads` via the git common dir; `bd worktree create|list|remove|info` exists. (https://github.com/steveyegge/beads/blob/main/docs/reference/worktrees.md; `bd worktree --help`.) Caveat from the architecture doc: "When multiple git clones ... run sync operations simultaneously, race conditions can occur ... Worktree-based development workflows" — prevention: use embedded mode for automated workflows or a Dolt server.
- **Git-free**: `BEADS_DIR=… bd init --stealth` works with no `.git` (README "Git-Free Usage").
- **Three event systems** (https://github.com/steveyegge/beads/blob/main/docs/reference/events-journal.md): script hooks (`.beads/hooks/on_create|on_update|on_close`, fire-and-forget), audit history (`bd history <id> --events`), and the 1.2.x **events journal** (`bd config set events-journal true`; ordered, resumable, "a binlog, not a notification"; readable via `bd events` and over HTTP; retention 7 days / 100k rows by default). The adopter has it on with 90-day/500k retention (local `config.yaml`).

### 1.5 Ready/blocked semantics and claiming

- "Ready work is the claimable frontier of the graph: open beads with no open blockers, excluding anything in progress, blocked, deferred, or held by a gate." `bd ready` "Excludes in_progress, blocked, deferred, and hooked issues" and honours `--label/--label-any/--exclude-label/--parent/--assignee/--unassigned/--type/--priority/--mol/--gated/--sort priority|hybrid|oldest/--brief/--explain`. (https://github.com/steveyegge/beads/blob/main/docs/core-concepts/index.md; `bd ready --help` local.)
- **Atomic claim**: `bd update <id> --claim` "Atomically claim the issue (sets assignee to you, status to in_progress; idempotent if already claimed by you; issues assigned to a pool alias listed in the claim.pools config are claimable too)". `bd ready --claim --json` claims the first ready match. `--claim` refuses to steal another actor's live claim; `--force` overrides ("prefer bd reclaim"). (`bd update --help`.)
- **Compare-and-set guards** (1.2.x): `bd update --if-assignee X --if-status Y …` "applies only if the issue's current assignee/status still equals the expected value — one atomic transaction, nothing written on a mismatch"; exit code **13** for a stale guard vs 1 for other failures; `--json` failures carry `guard_mismatch: true`. `bd unclaim <id> --if-assignee worker-7` is the CAS inverse. (CHANGELOG 1.2.1; `bd unclaim --help`.)
- **Pools** (1.2.x): `bd config set claim.pools "fable-crew,night-crew"` lets any actor claim issues pre-assigned to a pool alias (CHANGELOG 1.2.1).
- `bd blocked` lists blocked issues and their blockers; `bd recompute-blocked` repairs the denormalised `is_blocked` after a pull.

### 1.6 Molecules, formulas, protos, wisps, gates, swarms, merge slots

(https://github.com/steveyegge/beads/blob/main/docs/workflows/molecules.md, .../formulas.md, .../wisps.md, .../gates.md; `bd mol|formula|cook|gate|swarm|merge-slot --help` local.)

- **Formula**: TOML/JSON source with `[vars.x]`, `[[steps]] id/title/needs/type`, optional `[steps.gate]`, `[[compose.bond_points]]`, aspects (`type = "aspect"`, `[[advice]] target = "*.deploy"`), inheritance via `extends`. Search paths: `.beads/formulas/`, `~/.beads/formulas/`, `$GT_ROOT/.beads/formulas/`.
- **Cook** → **proto** (template epic labelled `template`, `{{vars}}` intact or substituted). **Pour** → **molecule** (persistent epic + children with deps). **Wisp** → same but `ephemeral=true`, "excluded from federation push", purged by `bd purge`/`bd mol wisp gc`, promoted with `bd promote` or `bd mol squash`, deleted with `bd mol burn`. `bd mol bond` combines proto/mol/formula (sequential/parallel/conditional). "Under the hood, a molecule is just an epic." Children parallel by default; only `needs`/`blocks` sequence them. `bd ready --mol <id>`, `bd mol current`, `bd mol progress`, `bd mol stale`, `bd mol distill <epic> <formula>`. Doc note: "Step-completion hooks are not currently exposed as runnable formula actions."
- **Gates**: issue type `gate` with `await_type` ∈ {`human`, `timer`, `gh:run`, `gh:pr`, `bead`}; wired into the graph with `bd dep add <step> <gate>`; `bd gate check` (cron/CI/agent-hook) auto-closes resolved gates; `bd gate resolve` for manual; `bd gate discover` matches gh:run IDs; `--escalate` marks failed gates. Rationale: with Dolt, "issue state is decoupled from code state" so a closed bead ≠ merged code — gates bridge that.
- **Swarm**: `bd swarm create|list|status|validate` — "a structured body of work defined by an epic and its children, with dependencies forming a DAG."
- **Merge slot**: `bd merge-slot create|check|acquire|release` — one bead per rig (`<prefix>-merge-slot`, label `gt:slot`), `status=in_progress` when held, `metadata.holder`, `metadata.waiters`; "prevents 'monkey knife fights' where multiple polecats race to resolve conflicts."
- **State dimensions**: `bd set-state <id> patrol=muted --reason …` writes an event bead and a `<dimension>:<value>` label; `bd state <id> patrol` reads it (Gas Town's agent-bead health model).

### 1.7 Leases / heartbeats / reclaim (1.2.x — currently only in the accidental 1.2.1)

From CHANGELOG 1.2.1 and `bd heartbeat|reclaim --help` (local):
- "Claiming stamps `lease_expires_at = now + TTL` (default 5m) and `heartbeat_at`." `bd heartbeat <id>` is owner-only and "Fails once the lease is gone." `bd reclaim --older-than <dur>` reverts expired in_progress issues to open, clears assignee/started_at, records a `lease_reclaimed` event; default grace 2×TTL; scope filters `--label/--label-any/--exclude-label/--assignee/--id`.
- "Leases live in an ephemeral, node-local table: heartbeats write no Dolt commit and no history … Leases are only enforceable on the node that granted them; cross-machine claim visibility rides the issue's status and assignee, which do commit." Replica guard: reclaim skips leases another replica granted unless `--any-replica`; set `node_id` per store via `~/.config/bd/config.yaml`.
- Because "Dolt has no row locking and merges concurrent commits cell-by-cell, every status/ownership/lease-mutating path also rewrites a shared `row_lock` cell, forcing a racing heartbeat vs. reclaim to a serialization conflict that the retry layer replays."
- Verified live in the adopter: ` frontend-leaning lease 03:42:59Z heartbeat 03:37:59Z` etc. (local `bd list --status in_progress --json`, 2026-08-18).

### 1.8 CLI surface (bd 1.2.1, `bd help`, local)

Working with issues: `assign children close comment comments create create-form delete edit gate heartbeat label link list merge-slot note priority promote provenance q query reclaim reopen search set-state show state tag todo unclaim update`.
Views/reports: `count diff find-duplicates history lint stale status statuses types`.
Deps/structure: `dep duplicate duplicates epic graph supersede swarm`.
Sync/data: `backup branch conflicts export federation import restore sync vc`.
Setup/config: `bootstrap config context dolt forget hooks human info init kv memories migrate-personal onboard prime quickstart recall remember setup where`.
Maintenance: `batch compact doctor events flatten gc migrate ping preflight prune purge recompute-blocked rename-prefix rules sql upgrade worktree`.
Integrations/advanced: `admin jira linear repo ado audit blocked completion cook defer formula github gitlab init-safety mail metrics mol notion orphans ready rename schema serve ship undefer version`.
Global flags of note: `--json`, `--actor` (audit identity; default `$BEADS_ACTOR`/git user), `-C dir`, `--readonly` ("for worker sandboxes"), `--sandbox` (disables Dolt auto-push), `--dolt-auto-commit off|on|batch`, `--global`, `--ignore-schema-skew`. Full generated reference: https://github.com/steveyegge/beads/blob/main/docs/CLI_REFERENCE.md (≈230 KB) and https://github.com/steveyegge/beads/blob/main/docs/reference/json-schema.md.
Memory: `bd remember/recall/forget/memories` and `bd prime` (injects workflow context + memories); README tells agents "Use `bd remember "insight"` for persistent project memory; do not create MEMORY.md files."

### 1.9 MCP server, HTTP API, and Rust ports

- **MCP**: PyPI `beads-mcp` (`uv tool install beads-mcp`), tools `ready list show create claim update close reopen dep comment comments note blocked stats context admin discover_tools get_tool_info`; docs say "Prefer CLI + hooks when shell is available" (context overhead ~1–2k tokens vs 10–50k for MCP). (https://github.com/steveyegge/beads/blob/main/docs/integrations/mcp-server.md.)
- **HTTP** (1.2.x only): `bd serve` — loopback OpenAPI `/v0` surface (ready, claimNext, release, batchClose, count, related, dependencies/tree, issues:query, config, sweep, `expected_version` optimistic concurrency, bearer-token file auth, `Bd-Project-Id` header). "Hooks do not fire" over HTTP. (`bd serve --help`, CHANGELOG 1.2.1.)
- **Rust**: no official port. `Dicklesworthstone/beads_rust` (`br`, 1,052 stars, v0.3.2 2026-08-15) is Jeffrey Emanuel's freeze of "classic beads" — "SQLite + JSONL … no automatic commits/pushes/pulls … No background daemon"; explicitly diverged because "the hybrid SQLite + JSONL-git architecture … is being replaced with approaches better suited to Steve's vision" (https://github.com/Dicklesworthstone/beads_rust). `delightful-ai/beads-rs` (24 stars, alpha) is a git-refs redesign ("everything lives in git on refs/heads/beads/store", missing mail/multi-repo/compaction) (https://github.com/delightful-ai/beads-rs). Others (`fwindolf/beads-rs`, `3x3xX3N0N/BIR`, `jedarden/bead-forge`) are ≤2 stars (GitHub search API, 2026-08-17). **None reads/writes the Dolt store that current `bd` uses**, so a Rust process must shell out to `bd --json` (or call `bd serve` once it ships).

### 1.10 What changed in 2026 (CHANGELOG headings, https://github.com/steveyegge/beads/blob/main/CHANGELOG.md)

0.55–0.63 (Feb–Mar), **1.0.0 (2026-04-02) Dolt-only**, 1.0.1–1.0.5 (Apr–May), 1.1.0-rc.1/rc.2 (Jun/Jul, schema v52/v53 migration safety, remote-migrate gate; fixed `--label-any` being silently dropped on `bd ready --claim` — a lane-fencing bug), 1.1.0 (2026-07-04), 1.1.2 (2026-07-26 hotfix), **1.2.1 (2026-08-11, accidental)**: `--brief`, `bd serve` v0 HTTP + bearer auth, events journal, `bd events`, `bd sync`, replica-aware leases, CAS updates, `claim.pools`, work leases (schema v54), provenance log, `beads.Storage` public interface (`IssueClaimer`, `ReadyClaimer`, …) for out-of-tree backends; **1.2.2 (2026-08-15) = 1.1.2 code, retracts 1.2.1**. Unreleased on main: quieter `bd reclaim` replica audit.

### 1.11 Local install summary

- `which bd` → `/usr/local/bin/bd`; `bd --version` → `bd version 1.2.1 (Homebrew)`. Latest tag v1.2.2 (2026-08-15) is *older code* than 1.2.1.
- `~/.beads/` contains only `eventsData/` and `machine-id`.
- `the adopter's .beads/`: `embeddeddolt/`, `backup/`, `hooks/` (6 git hooks), `issues.jsonl` (2.2 MB, 515 rows), `config.yaml` (sync remote, auto-export 5 s, `validation.on-create: warn`, events journal on, plus a long owner note about the compiled-in "Steps to Reproduce" lint for bugs), `metadata.json`, `.gitignore` (excludes `embeddeddolt/`, `*.lock`, `redirect`, `daemon.*`, etc.), `README.md` (bd-generated). No formulas (`bd formula list` → none). Custom statuses `awaiting_review`, `awaiting_testing`.

---

## 2. Gas Town (`gt`)

### 2.1 What it is

"Multi-agent orchestration system for Claude Code, GitHub Copilot, and other AI agents with persistent work tracking … a workspace manager that lets you coordinate multiple AI coding agents … Instead of losing context when agents restart, Gas Town persists work state in git-backed hooks." Repo `gastownhall/gastown` (redirect from `steveyegge/gastown`), **Go**, 17.6k stars, latest release v1.2.1 (2026-06-06), last push 2026-08-13. (https://github.com/steveyegge/gastown README; GitHub API.) Yegge: "Like it or not, Gas Town is built on Beads … There is no 'alternate backend' for Gas Town. Beads is the Universal Git-Backed data plane (and control plane, it turns out)." (Welcome to Gas Town.)

### 2.2 Roles and concepts (README + docs/overview.md + essay)

- **Town** (`~/gt/`), **Rig** (a git repo under management; `mayor/rig/` is the canonical clone; per-rig `.beads/`), **Crew** (persistent human/agent full clones), **Polecats** (ephemeral workers: "persistent identity but ephemeral sessions"), **Mayor** (singleton coordinator; "your concierge and chief-of-staff"), **Witness** (per-rig lifecycle manager: detects stuck polecats, nudges/handoffs, cleanup), **Deacon** (town-level patrol daemon; "the daemon beacon"), **Dogs** (Deacon's helpers; **Boot** wakes every 5 min to check the Deacon), **Refinery** (per-rig merge queue), **Overseer** (the human).
- **Hooks**: "A special pinned Bead for each agent. The Hook is an agent's primary work queue" (docs/glossary.md). Agents, roles and hooks are all pinned beads with singleton addresses (essay).
- **GUPP** — Gas Town Universal Propulsion Principle: "If there is work on your hook, YOU MUST RUN IT." Admitted weakness: "Claude Code is so miserably polite that GUPP doesn't always work in practice … It just sits there waiting for user input," hence the **GUPP nudge** (`gt nudge` via `tmux send-keys`, 30–60 s after start, always within ~5 min) and the daemon→Boot→Deacon→Witness heartbeat chain. (essay; README "Monitoring & Health"; docs/concepts/propulsion-principle.md.)
- **Convoys**: "a special bead that wraps a bunch of work into a unit that you track for delivery. It doesn't use the Epic structure, because the tracked issues in a Convoy are not its children." `gt convoy create|add|list|show`; `mountain`-labelled convoys get stall detection. **Sling**: `gt sling <bead> <rig>` puts work on a hook and spawns a polecat.
- **MEOW stack** (Molecular Expression of Work): beads → epics → molecules → protomolecules → formulas (TOML, "cooked" into protos, poured into mols or wisps). "Nondeterministic Idempotence": the agent, hook and molecule are all git-backed beads, so "it doesn't matter if Claude Code crashes, or runs out of context … the outcome … eventually finishes, 'guaranteed', as long as you keep throwing agents at it." Wisps: "All the patrol agents … create wisp molecules for every single patrol or workflow run … without polluting Git with orchestration noise." Newer README: two modes, "root-only wisps (steps materialized at runtime, lightweight) and poured wisps".
- **Mail**: agent-to-agent inbox beads (`gt mail`, `bd mail` delegates to `gt mail`), injected via Claude Code hooks; **Seance** (`gt seance --talk <id>`) queries predecessor sessions via `.events.jsonl` + `claude --resume`; **Escalation** (`gt escalate -s HIGH`) routes Deacon → Mayor → Overseer; **Scheduler** (`scheduler.max_polecats`) throttles dispatch for rate limits; **Wasteland** (`gt wl`) federated wanted-board over DoltHub with reputation "stamps"; `gt feed` TUI with a problems view (GUPP violation / Stalled / Zombie / Working / Idle); `gt dashboard` htmx web UI. Runtimes: presets `claude gemini codex kiro cursor auggie amp opencode copilot pi omp`.

### 2.3 Architecture: worktrees, storage, sessions

- "Polecats and refinery are git worktrees, not full clones … The worktree base is `mayor/rig`: `git worktree add -b polecat/<name>-<timestamp> polecats/<name>`." Crew are full clones. Worktrees have no DB of their own: `polecats/alpha/.beads/redirect → ../../mayor/rig/.beads`. "All beads data is stored in a single Dolt SQL Server process per town." (https://github.com/steveyegge/gastown/blob/main/docs/design/architecture.md.)
- Three polecat layers — **Identity** (agent bead, CV chain: permanent), **Sandbox** (worktree + branch: per assignment), **Session** (Claude context in a tmux pane: ephemeral, cycles on `gt handoff`, compaction, crash). States: Working / Idle / Done / Stalled / Zombie. "Clean completion retires the live polecat session" — no reuse of a done sandbox until cleanup resolves. (https://github.com/steveyegge/gastown/blob/main/docs/concepts/polecat-lifecycle.md.)
- **tmux** is the primary UI and session backend ("Gas Town uses tmux as its primary UI"; `gt up` boots Dolt, daemon, Deacon, Mayor, Witnesses, Refineries). Minimal mode without tmux exists ("Gas Town just tracks state").
- Prereqs: Git 2.20+, Go 1.26.2+, `bd` 0.57.0+, sqlite3 (convoy queries), ICU4C headers, tmux 3.0+, Claude Code CLI; `brew install gastown` bundles `gt`, `bd`, `dolt`. Docker Compose path exists. (README "Installation".)

### 2.4 Merge queue (Refinery), leases/heartbeats, supervision

- **Refinery**: "When polecats complete work via `gt done`, the Refinery batches merge requests, runs verification gates, and merges to main using a Bors-style bisecting queue" — batch-then-bisect: rebase A..D as a stack, test tip, on failure bisect; gates (test/lint) pluggable, batching core; integration-branch path per epic ("MRs from epic children merge to integration/<epic> … land to main as one commit"). Conflicts create a task for another polecat; `bd merge-slot` serialises conflict resolution. Polecat completion is self-managed ("The Witness observes but does NOT gate completion"). (docs/design/architecture.md; docs/concepts/polecat-lifecycle.md; `bd merge-slot --help`.)
- **Heartbeats**: three stores — Deacon `heartbeat.json` (daemon thresholds 5 m stale / 20 m very-stale → poke), per-session heartbeat (`gt heartbeat --state=working|idle|exiting|stuck`, read by Witness), and a `heartbeat:<EPOCH>` label on the agent bead "because `bd agent heartbeat` was never shipped." Rule: "never declare an agent stuck from a single store. Cross-check tmux session activity." (https://github.com/steveyegge/gastown/blob/main/docs/concepts/heartbeats.md.) Note these are *agent* heartbeats layered above beads; beads' own *issue* leases only arrived in bd 1.2.x.
- **Supervision chain**: "Daemon (Go process) ← heartbeat every 3 min → Boot (AI agent) → Deacon (AI agent) → Witnesses & Refineries." Nudge, handoff, `gt polecat nuke`, escalations. (README.)

### 2.5 Cost and known problems / criticism

- Yegge himself: "the code base is under 3 weeks old … 'You probably don't want to use it yet.' … It's also 100% vibe coded. I've never seen the code"; "Work in Gas Town can be chaotic and sloppy … Some bugs get fixed 2 or 3 times … Other fixes get lost"; "Do not use Gas Town if you care about money"; GUPP "still a bit flaky"; "17 days, 75k lines of code, 2000 commits." (Welcome to Gas Town, 2026-01-01.) Later: "I've merged over 100 PRs from nearly 50 contributors, adding 44k lines of code that no human has looked at" (Emergency User Manual, https://steve-yegge.medium.com/gas-town-emergency-user-manual-cf0e4556d74b, 2026-01-13); v1.0 recounts "the chaotic early days of data loss and instability" fixed by the Dolt migration (Clown Show to v1.0).
- Tim Sehn (DoltHub, 2026-01-15): 60-minute session ≈ $100 (~10× a normal Claude Code session); "Gas Town merged a pull request despite failing integration tests"; Mayor claimed all bugs fixed when only two PRs existed; "None of the PRs were good." (https://www.dolthub.com/blog/2026-01-15-a-day-in-gas-town/.)
- Tenzin Wangdhen (2026-02-19): needed to patch the mail system, daemons not auto-starting, 141 orphaned Claude processes; poor observability ("6 PRs merged and I had no idea when I'd slung them"); requires `--dangerously-skip-permissions`; "cash guzzler"; too complex for iterative work needing human feedback — but 6 of 7 queued tasks landed overnight. (https://tenzinwangdhen.com/posts/gastown-good-bad-ugly/.)
- Mark Atwood (2026-05-14): works for "parity work" with an external oracle (DumboDB: 2,400 PRs submitted/1,500 merged) but "collapses on novel design work"; "the verification chain remains open"; self-reported "$100 per hour at scale" (v1.0 metrics, April 2026); "This is real engineering … The thing runs." (https://reviewcommit.substack.com/p/gas-town-a-review.)
- Maggie Appleton: "vibe coded" and "vibe designed too," an "overwhelming" system of overlapping concepts; useful mainly as "speculative design fiction"; design becomes the bottleneck once agents write the code. (https://maggieappleton.com/gastown.)
- HN thread (https://news.ycombinator.com/item?id=46458936): cost ("must cost $1000's"), "mashing together the complexity of k8s with a hodge podge of lotr and mad max references is not it," beads "more like a stream of consciousness converted directly into code," vs. defenders reporting real throughput and "an opinionated glimpse into the future." Pivot to AI's hostile take: https://pivot-to-ai.com/2026/01/22/steve-yegges-gas-town-vibe-coding-goes-crypto-scam/ (not independently verified beyond the search snippet).
- Bill de hÓra's thoughts: https://dehora.net/journal/2026/2/initial-thoughts-on-welcome-to-gas-town (found via search; not fetched).

### 2.6 Overlap with ai_runner, and could we just use Gas Town / Gas City?

Overlap: Gas Town already provides claim (sling/hook), worktree sandbox per worker, work loop, verification gates + Bors-style merge queue, close, supervision (Witness/Deacon), heartbeats, nudges, escalation, cost throttling (scheduler), identity/attribution, and a beads-native ledger. That is essentially our claim → worktree → work → verify → review → land → close lifecycle.

Arguments **for** using it: it exists, runs, has 17.6k stars and a Homebrew tap, is beads-native, and its designs (three-layer polecat, batch-then-bisect refinery, redirect-to-shared-.beads, wisps for patrol noise, merge-slot) are worth copying. Gas City (`gastownhall/gascity`, "orchestration-builder SDK … runtime providers, work routing, formulas, orders, health patrol, and a declarative city configuration"; providers tmux/subprocess/exec/ACP/Kubernetes/herdr; "controller/supervisor loop that reconciles desired state to running state"; `city.toml`; beads provider `bd` or file) is closer to a library than Gas Town is. (https://github.com/gastownhall/gascity.)

Arguments **against**: (a) it is Go, so a Rust runtime cannot link it — we'd be driving `gt`/`gc` CLIs from Rust and inheriting their process model; (b) hard dependencies on tmux, Claude Code hooks, Dolt server, ICU, sqlite3, `--dangerously-skip-permissions`; (c) design is explicitly agent-prompt-driven ("all the other workers are also Claude Code") — supervision, merge conflict resolution and triage are done by LLM agents (Witness, Deacon, Boot, Refinery), which is where the cost ($100/hour class) and the "chaotic and sloppy" behaviour come from; ai_runner's premise is a deterministic runtime that does those jobs in code; (d) verification chain is acknowledged open by users; (e) `gt` requires bd ≥0.57 and Gas City requires bd ≥1.0 / Dolt ≥2.1.0 — versions churn fast and 1.2.x just showed a broken release; (f) it is a "You probably don't want to use it yet"/"Do not use Gas Town" product by its author's own framing, and its own successor (Gas City) exists because Gas Town was too monolithic.

Balanced conclusion: **do not adopt Gas Town as the runtime**; treat it as prior art. Keep the beads data plane compatible so a Gas Town/Gas City user could point either at the same `.beads` (their labels `gt:*`, statuses `hooked`, `pinned` beads, `bd merge-slot`, and formulas are all plain beads features), and consider Gas City's `city.toml`/provider model as a shape to compare against.

---

## 3. Lineage: Yegge's 2025–2026 essays and the design principles behind beads/Gas Town

(All Medium URLs below are 403 for direct fetch; titles/dates and summaries were taken from Yegge's own index at https://steveyegge.spicytakes.org/ (built by Wes McKinney), and the Gas Town text from the archived Medium page. Accessed 2026-08-17.)

| Date | Essay | Principle relevant to fleet orchestration |
|---|---|---|
| 2025-03 | Revenge of the Junior Developer (Sourcegraph blog, https://sourcegraph.com/blog/revenge-of-the-junior-developer) | Predicted "someone would lash the Claude Code camels together into chariots" — orchestrators are next (quoted in Welcome to Gas Town). |
| 2025-10-15 | The Beads Revolution (https://steve-yegge.medium.com/the-beads-revolution-how-i-built-the-todo-system-that-ai-agents-actually-want-to-use-228a5f9be2a9) | Agents need a structured TODO/issue graph, not markdown. |
| 2025-11-02 | Zero Framework Cognition (https://steve-yegge.medium.com/zero-framework-cognition-a-way-to-build-resilient-ai-applications-56b090ed3e69) | Resilient AI apps avoid heavy frameworks; let the model reason over simple state (cited as "ZFC" in Gas Town's heartbeat design). |
| 2025-11-12 | Introducing Beads (https://steve-yegge.medium.com/introducing-beads-a-coding-agent-memory-system-637d7d92514a) | "AI coding agents need structured, queryable issue trackers with first-class dependencies — not markdown plans"; "A missing test is a passing test." |
| 2025-11-14 | The Death of the Stubborn Developer (https://steve-yegge.medium.com/the-death-of-the-stubborn-developer-b5e8f78d326b) | Chat-oriented programming is mandatory. |
| 2025-11-17 | Cheese Wars: Rise of the Vibe Coder (https://steve-yegge.medium.com/cheese-wars-rise-of-the-vibe-coder-6839a6b15982) | Vibe coding (term: Karpathy; Yegge & Gene Kim wrote the *Vibe Coding* book) as the operating mode. |
| 2025-11-27 | Beads Best Practices (https://steve-yegge.medium.com/beads-best-practices-2db636b9760c) | "Beads is an execution tool" not a planning tool; "crummy architecture … that requires AI in order to work around all its edge cases"; "agent villages" of 30+ workers via git worktrees + messaging (MCP Agent Mail); "By starting new sessions often, you save money." |
| 2026-01-01 | Welcome to Gas Town (https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04) | Sessions are cattle, work molecules are durable; GUPP; nondeterministic idempotence; "K8s asks 'Is it running?' while Gas Town asks 'Is it done?'"; the 8-stage dev-evolution chart (Stage 8 = "Building your own orchestrator"). |
| 2026-01-13 | Six New Tips (https://steve-yegge.medium.com/six-new-tips-for-better-coding-with-agents-d4e9c86e42a9) | Software is disposable; design tools for agent usability; 40% time on code health; "Rule of Five" (agents review 4–5 times to converge); "AI cognition takes a hit every time it crosses a boundary in the code"; avoid merge conflicts when swarming. |
| 2026-01-13 | Gas Town Emergency User Manual (https://steve-yegge.medium.com/gas-town-emergency-user-manual-cf0e4556d74b) | Outer/middle/inner loops; "Don't watch your agents work"; "PR sheriffs"; "There is no such thing as an idle polecat; it's not a pool." |
| 2026-01-20 | The Future of Coding Agents (https://steve-yegge.medium.com/the-future-of-coding-agents-e9451a84207c) | Colonies/factories over single super-agents; Go as the preferred AI-coded language ("with Go, it's just boring"). |
| 2026-01-23 | Stevey's Birthday Blog (https://steve-yegge.medium.com/steveys-birthday-blog-34f437139cb5) | Compares orchestrators "Ralph Wiggum, Loom, Claude Flow, Gas Town"; teases Gas City. The **Ralph Wiggum loop** itself is Geoffrey Huntley's (https://ghuntley.com/ralph/): a bash loop re-feeding the same prompt until done; Yegge treats it as the simplest orchestrator ("Ralph loops are effectively tasks"). Also see https://linearb.io/dev-interrupted/podcast/inventing-the-ralph-wiggum-loop. |
| 2026-01-29 | Software Survival 3.0 (https://steve-yegge.medium.com/software-survival-3-0-97a2a6255f7b) | "Software tends to survive if it saves cognition"; CPU beats GPU for grep-like work; minimise agent friction. |
| 2026-03-04 | Welcome to the Wasteland (https://steve-yegge.medium.com/welcome-to-the-wasteland-a-thousand-gas-towns-a5eb9bc8dc1f) | Federation of towns over Dolt; peer-attested reputation ("work is the only input, and reputation is the only output"). |
| 2026-03-31 | Vibe Maintainer (https://steve-yegge.medium.com/vibe-maintainer-a2273a841040) | ~50 AI PRs/day; fix-don't-request-changes; "the last 25% or so of pull requests need human review." |
| 2026-04-03 | Gas Town: from Clown Show to v1.0 (https://steve-yegge.medium.com/gas-town-from-clown-show-to-v1-0-c239d9a407ec) | Dolt migration cured data loss; "Beads is the Why — the missing piece in your commit history"; "People who switch to Beads soon realize they can build their own workflows and orchestration using nothing but Beads." |
| 2026-04-24 | Welcome to Gas City (https://steve-yegge.medium.com/welcome-to-gas-city-57f564bb3607) | "light factories" with full observability; "Reliability, friends, is a dial"; "You should almost never deploy a single-agent pack for a real business process." |

Distilled design principles: (1) durable, queryable, dependency-aware work graph as the agent's memory; (2) sessions ephemeral, identity/work persistent; (3) the graph — not a dispatcher — decides what is ready; (4) push agents to act (GUPP) but expect to nudge; (5) serialise merges through a queue and never let workers push to main; (6) ephemeral (wisp) records for orchestration noise; (7) frequent session restarts to control cost/context; (8) attribution/ledger for every action; (9) reliability is a dial — accept some lost work for throughput (this is the point ai_runner can reasonably disagree with).

---

## 4. Alternatives to beads as the coordination layer

- **Claude Code Tasks (Anthropic, built-in).** Since Claude Code v2.1.16 (Jan 2026): dependency tracking, session isolation, optional persistence via `CLAUDE_CODE_TASK_LIST_ID` to `~/.claude/tasks/<id>/`, shared across terminals. Lacks repo awareness, MCP, dashboards, and is Claude-Code-specific ("Tasks is SQLite. Beads/Flux are PostgreSQL"). Good enough for one-machine Claude-only fleets; not a cross-runtime ledger. (https://paddo.dev/blog/from-beads-to-tasks/, 2026-01-23.)
- **beads_rust (`br`).** Jeffrey Emanuel's Rust freeze of classic SQLite+JSONL beads, "no automatic commits/pushes/pulls," no daemon, part of his "Agent Flywheel" with MCP Agent Mail. If ai_runner wanted an *in-process Rust* tracker with beads semantics this is the only serious candidate — but it is a fork of the pre-Dolt design and not interoperable with `bd` 1.x stores. (https://github.com/Dicklesworthstone/beads_rust; MCP Agent Mail: https://github.com/Dicklesworthstone/mcp_agent_mail — "identities, inboxes, searchable threads, and advisory file leases over FastMCP + Git + SQLite", 2,094 stars.)
- **Backlog.md** (MrLesk, TypeScript, 6.5k stars): "Markdown-native Task Manager & Kanban visualizer for any Git repository"; one task = one `.md` file with acceptance criteria; three review checkpoints (spec, plan, code); MCP + CLI. Human-legible and diff-friendly, but no atomic claim/lease semantics or graph queries; concurrency relies on git. (https://github.com/MrLesk/Backlog.md.)
- **claude-task-master** (eyaltoledano, JS, 28k stars, last push 2026-04-28): "AI-powered task-management system you can drop into Cursor, Lovable, Windsurf, Roo" — PRD → tasks via LLM, MCP-first; planning-oriented rather than a multi-agent execution ledger. (https://github.com/eyaltoledano/claude-task-master.)
- **tk / ticket** (wedow/ticket, single bash script, 864 stars): "git-native ticket tracking … Dependency graphs, priority levels, zero setup"; markdown+YAML frontmatter in `.tickets/`; Go port `radutopala/ticket`; MCP-friendly `trevorgrayson/tkts`. Minimal and portable; no leases, no server mode. (https://github.com/wedow/ticket.)
- **Linear / GitHub Issues via MCP.** Linear MCP (https://linear.app/docs/mcp) and `github/github-mcp-server` (Go, 32k stars, https://github.com/github/github-mcp-server) give hosted issue graphs with humans in the loop; beads itself bridges to them (`bd linear`, `bd github`, `bd jira`, `bd ado`, `bd notion`, `external_ref`). Downsides for a runtime: network dependency, rate limits, no offline atomic claim, coarse dependency semantics; beads' own docs recommend them for 10+ person teams and real-time collaboration.
- **beads_viewer (`bv`)** (Go, 1,653 stars): not an alternative but a complement — "PageRank, critical path, kanban, dependency DAG visualization, and robot-mode JSON API" over a beads DB (https://github.com/Dicklesworthstone/beads_viewer).
- **"Metis"** — could not be verified as a beads-comparable tool; the only "Metis" found (https://www.withmetis.ai/) is an enterprise agent post-training lab. **"TaskWarrior for agents"** — no established project found beyond tiny repos (e.g. `sznicolas/taskmajor`, 3 stars). Both remain unverified.

Net: beads is the only local-first tracker that already has atomic claims, typed blocking dependencies, ready-frontier queries, worktree-shared discovery, molecules/gates, and (soon) leases and an HTTP API, and it is what the owner's other project already runs. The competition is either simpler (files) or hosted (Linear/GitHub). The decision to build on beads is defensible; the risk is version churn (see §0.2) and the single-writer embedded mode.

---

## 5. Implications for ai_runner

### 5.1 What to build ON beads vs what beads/Gas Town already provide

Provided by beads (do not rebuild): the work graph, IDs, ready/blocked computation, atomic claim + CAS guards, labels/assignee/owner, comments/notes/audit history, epics/molecules/formulas/gates, merge-slot mutex bead, worktree-aware discovery, Dolt sync/federation, JSONL export, events journal (1.2.x), HTTP API (1.2.x), MCP server, `bd prime`/memories for agent context.

Provided by Gas Town but **not** by beads (ai_runner must build, in Rust): process/session supervision, worktree lifecycle, agent identity/runtime adapters, verification gates execution, merge queue, nudges/timeouts, cost/concurrency governor, escalation, dashboards. Gas Town does these with LLM patrol agents + tmux; ai_runner should do them deterministically.

Owner's `.beads/config.yaml` note is a good example of what beads does *not* police: it grep-checks headings, so quality gates (real repro, real tests) belong to ai_runner's verify stage.

### 5.2 Mapping our lifecycle onto beads primitives

| ai_runner stage | beads primitive | Notes / gaps |
|---|---|---|
| **select** | `bd ready --json --brief --label-any <lane> --exclude-label human --unassigned --sort priority` (or `--parent <epic>`, `--mol <id>`) | 1.1.0 fixed `--label-any` on the claim path; `--brief` is 1.2.x-only. Custom `deferred`/`pinned`/`hooked` are excluded automatically. |
| **claim** | `bd ready --claim --json …` or `bd update <id> --claim --actor <agent-id>` (exit 13 = lost race when using `--if-*`) | Actor string = agent identity (`BEADS_ACTOR`). Pool dispatch via `claim.pools`. Leases stamp `lease_expires_at` (5 m default) — only on 1.2.x; on 1.2.2 a claim is permanent, so ai_runner needs its own liveness (see below). |
| **worktree** | none in beads (`bd worktree create` is a thin helper); Gas Town: `git worktree add -b polecat/<name>-<ts>` from a canonical clone + `.beads/redirect` | Keep one `.beads` per repo; worktrees discover it via git common dir. Record worktree path/branch in `--set-metadata worktree=…,branch=…` on the issue. |
| **work** | `bd heartbeat <id>` on a timer < TTL (1.2.x); `bd note`/`bd comment` for progress; `bd create --parent`/`discovered-from` for spin-off work; `bd remember` for durable insights | On non-1.2.x builds, emulate liveness with a `heartbeat:<epoch>` label or metadata key exactly as Gas Town did "because bd agent heartbeat was never shipped." |
| **verify** | custom status `awaiting_testing` (already configured in the adopter) or label `needs-verify`; a `gate` bead of type `gh:run` for CI, `bd gate check` in the runtime loop; `bd lint` for template sections | Gates are the sanctioned way to make "closed bead ≠ merged code" explicit. |
| **review** | status `awaiting_review` (configured), `--add-label needs-review`, `approved-by` dependency edge, `bd comment` for findings; human gate (`await_type=human`) when policy requires | Gas Town uses MR beads (`bd update --mr-ready`, `-t merge-request` alias exists in `bd ready --type`) — mirror that with a `merge-request` type or label so a future Refinery/Gas City could interoperate. |
| **land** | `bd merge-slot acquire/release` around the merge; `gh:pr` gate → auto-close; record commit SHA in `--set-metadata landed_sha=…`; `prepare-commit-msg` hook adds agent trailers | Batch-then-bisect queue is ai_runner code; the slot bead is the shared mutex. |
| **close** | `bd close <id> --reason … --session $CLAUDE_SESSION_ID`; `bd close` refuses if open children/live blocker unless `--force`; closing releases dependents into `bd ready` | Then `bd epic close-eligible`, `bd purge` of wisps, `bd sync`/`bd dolt push` at end of run. |
| **crash recovery** | `bd reclaim --older-than <grace> --label <lane>` from the supervisor (1.2.x); `bd unclaim <id> --if-assignee <dead-agent>` (CAS) | On 1.2.2: supervisor must `bd unclaim --if-assignee` based on its own liveness data. |
| **orchestration noise** | wisps (`--ephemeral`) for per-run patrol/molecule steps; `bd set-state agent-x health=failing`; agent identity as a `pinned` bead | Keeps Dolt history and federation clean. |

### 5.3 Concrete engineering recommendations

1. **Integrate via `bd --json` subprocess first**, behind a Rust trait (`WorkLedger`) so a `bd serve` HTTP client (or a future Rust store) can replace it. Parse with the `bd schema` JSON Schema; treat `additionalProperties:false` as a contract but tolerate unknown fields (schema_version 1 today).
2. **Pin and detect `bd` version.** Feature-gate leases/heartbeat/serve/`--brief`/`--if-*` on `bd version` ≥ 1.2.1-and-not-1.2.2, or on `bd help` probing. Document the 1.2.1→1.2.2 schema trap for the owner (recovery guide above); do not auto-upgrade `bd` on the adopter's machine.
3. **Concurrency model.** Embedded Dolt is single-writer/file-locked; N agents calling `bd` concurrently serialise on the lock (the adopter runs 4 agents this way today). If ai_runner runs many workers, either funnel all `bd` writes through the runtime process (one writer, agents talk to ai_runner), or run `bd init --server`/shared server. `bd serve` help notes "claims are arbitrated in the SQL server."
4. **Copy Gas Town's good bones, skip its LLM supervisors**: three-layer worker model (identity/sandbox/session), redirect-to-shared-`.beads`, batch-then-bisect merge queue with pluggable gates, merge-slot mutex, wisps for patrol runs, `heartbeat:<epoch>` cross-check with real process/tmux activity before declaring stuck, scheduler cap on concurrent workers for rate limits.
5. **Reliability dial**: unlike Gas Town's "some work gets lost," make claim/heartbeat/close transitions CAS-guarded (`--if-assignee/--if-status`, `expected_version` over HTTP) and never `--force` except from the reaper.
6. **Keep beads plain**: use `metadata` (documented extension point) and labels rather than new fields, per the beads Project Charter's schema boundary; use `external_ref` for PR URLs so `bd github` sync stays possible.
7. **Watch**: beads main for the "properly tested" 1.2.x re-release; Gas City's `city.toml` and provider interfaces as a compatibility target; `beads-mcp` if agents run without shell.

---

## 6. Sources (all accessed 2026-08-17/18)

Local
- `/usr/local/bin/bd` — `bd --version` = `bd version 1.2.1 (Homebrew)`; `bd help`; `bd schema`; `bd ready|update|heartbeat|reclaim|unclaim|mol|formula|cook|gate|merge-slot|worktree|swarm|set-state|state|promote|federation|sync|hooks|serve --help`; `bd statuses`; `bd types`; `bd formula list`; `bd info`; `bd config list`; `bd list --status in_progress --json` (all run read-only in `the adopter's checkout`).
- `the adopter's .beads/{README.md,config.yaml,metadata.json,.local_version,export-state.json,.gitignore,issues.jsonl,hooks/pre-commit}`; `~/.beads/` listing.

beads (repo `steveyegge/beads` → `gastownhall/beads`)
- README: https://github.com/steveyegge/beads
- Releases API: https://api.github.com/repos/steveyegge/beads/releases
- CHANGELOG: https://github.com/steveyegge/beads/blob/main/CHANGELOG.md
- Recovery, accidental 1.2.1: https://github.com/steveyegge/beads/blob/main/docs/recovery/accidental-1-2-1-release.md
- Architecture: https://github.com/steveyegge/beads/blob/main/docs/architecture/index.md ; Dolt backend: https://github.com/steveyegge/beads/blob/main/docs/architecture/dolt.md
- Core concepts: https://github.com/steveyegge/beads/blob/main/docs/core-concepts/index.md ; dependencies: https://github.com/steveyegge/beads/blob/main/docs/core-concepts/dependencies.md ; hash IDs: https://github.com/steveyegge/beads/blob/main/docs/core-concepts/hash-ids.md ; adaptive IDs: https://github.com/steveyegge/beads/blob/main/docs/core-concepts/adaptive-ids.md ; labels: https://github.com/steveyegge/beads/blob/main/docs/core-concepts/labels.md
- Workflows: https://github.com/steveyegge/beads/blob/main/docs/workflows/molecules.md , https://github.com/steveyegge/beads/blob/main/docs/workflows/formulas.md , https://github.com/steveyegge/beads/blob/main/docs/workflows/wisps.md , https://github.com/steveyegge/beads/blob/main/docs/workflows/gates.md
- Multi-agent: https://github.com/steveyegge/beads/blob/main/docs/multi-agent/index.md , https://github.com/steveyegge/beads/blob/main/docs/multi-agent/coordination.md , https://github.com/steveyegge/beads/blob/main/docs/multi-agent/federation.md
- Reference: https://github.com/steveyegge/beads/blob/main/docs/reference/worktrees.md , https://github.com/steveyegge/beads/blob/main/docs/reference/git-integration.md , https://github.com/steveyegge/beads/blob/main/docs/reference/events-journal.md , https://github.com/steveyegge/beads/blob/main/docs/reference/json-schema.md , https://github.com/steveyegge/beads/blob/main/docs/CLI_REFERENCE.md
- MCP: https://github.com/steveyegge/beads/blob/main/docs/integrations/mcp-server.md ; related projects: https://github.com/steveyegge/beads/blob/main/docs/related-projects.md ; community tools: https://github.com/steveyegge/beads/blob/main/docs/community-tools.md
- Docs site: https://beads.gascity.com/

Rust ports / complements
- https://github.com/Dicklesworthstone/beads_rust ; https://github.com/delightful-ai/beads-rs ; https://github.com/Dicklesworthstone/beads_viewer ; https://github.com/Dicklesworthstone/mcp_agent_mail ; GitHub search API queries for "beads rust" (2026-08-17)

Gas Town / Gas City
- README: https://github.com/steveyegge/gastown (→ gastownhall/gastown); GitHub API metadata and releases
- docs: https://github.com/steveyegge/gastown/blob/main/docs/overview.md , https://github.com/steveyegge/gastown/blob/main/docs/glossary.md , https://github.com/steveyegge/gastown/blob/main/docs/HOOKS.md , https://github.com/steveyegge/gastown/blob/main/docs/concepts/propulsion-principle.md , https://github.com/steveyegge/gastown/blob/main/docs/concepts/heartbeats.md , https://github.com/steveyegge/gastown/blob/main/docs/concepts/polecat-lifecycle.md , https://github.com/steveyegge/gastown/blob/main/docs/design/architecture.md
- Gas City: https://github.com/gastownhall/gascity ; docs https://docs.gascityhall.com

Yegge essays (index: https://steveyegge.spicytakes.org/)
- Welcome to Gas Town: https://steve-yegge.medium.com/welcome-to-gas-town-4f25ee16dd04 (2026-01-01; text via web.archive.org)
- Others as listed in §3 (Introducing Beads, Beads Best Practices, Six New Tips, Emergency User Manual, Future of Coding Agents, Birthday Blog, Software Survival 3.0, Wasteland, Vibe Maintainer, Clown Show to v1.0, Welcome to Gas City, Death of the Stubborn Developer, Cheese Wars, Zero Framework Cognition, Beads Revolution)
- Revenge of the Junior Developer: https://sourcegraph.com/blog/revenge-of-the-junior-developer
- Ralph Wiggum loop (Huntley): https://ghuntley.com/ralph/ ; podcast https://linearb.io/dev-interrupted/podcast/inventing-the-ralph-wiggum-loop

Critiques / reports
- https://www.dolthub.com/blog/2026-01-15-a-day-in-gas-town/
- https://reviewcommit.substack.com/p/gas-town-a-review
- https://tenzinwangdhen.com/posts/gastown-good-bad-ugly/
- https://maggieappleton.com/gastown
- https://news.ycombinator.com/item?id=46458936
- https://pivot-to-ai.com/2026/01/22/steve-yegges-gas-town-vibe-coding-goes-crypto-scam/ (snippet only)
- https://dehora.net/journal/2026/2/initial-thoughts-on-welcome-to-gas-town (not fetched)
- https://medium.com/@enterprisevibecode/10-hours-with-gas-town-out-of-a-possible-48-17a6b2801a73 (not fetched)

Alternatives
- https://paddo.dev/blog/from-beads-to-tasks/ ; https://github.com/MrLesk/Backlog.md ; https://github.com/eyaltoledano/claude-task-master ; https://github.com/wedow/ticket ; https://github.com/radutopala/ticket ; https://github.com/trevorgrayson/tkts ; https://linear.app/docs/mcp ; https://github.com/github/github-mcp-server ; https://www.withmetis.ai/ (not a tracker)
