# Prior-art landscape: AI coding-agent orchestration runtimes

**Purpose**: before building `ai_runner` (a Rust orchestration runtime that runs fleets of coding agents in git worktrees against a beads task queue, with enforced steps claim → worktree → work → verify → review → land/merge → close, human-decision queues, leases/heartbeats, cost tracking and observability), catalogue what exists to USE outright or BORROW from.

**Method / date**: web survey performed **2026-08-17**. Star counts, licenses and "pushed" dates come from the GitHub REST API on that day; crate versions from the crates.io API on that day; feature claims from primary READMEs/docs/specs fetched that day, plus subagent deep-dives. Anything not confirmed from a primary source is marked **unverified**. Every entry is "checked 2026-08-17" unless stated otherwise.

**Sections**
- A. Core: beads, Gas Town, Codex CLI, Symphony, Claude Code (deep dives)
- B. Rust-implemented orchestrators/runtimes/tools (goose, Vibe Kanban, ACP/Zed, herdr, harness, ralph-orchestrator, OpenSymphony, tutti, forge, pueue, …)
- C. Non-Rust orchestrators / fleet managers (design borrowing)
- D. Influential platforms and 2026 papers/blogs
- E. Rust LLM/agent frameworks
- F. Rust workflow / durable-execution engines & job queues
- G. Process supervision / PTY / terminal control crates
- H. Candidates to adopt outright vs build (ranked)
- I. Rust crate shortlist
- J. One-page synthesis: what to USE vs BORROW

---

## A. Core deep dives: beads, Gas Town, Codex CLI, Symphony, Claude Code

Note on sourcing: everything below comes from pages fetched 2026-08-17 unless marked **unverified**. GitHub star counts / dates are from the GitHub REST API. Steve Yegge's Medium posts return 403 to fetchers; his mirror (steveyegge.spicytakes.org / yegge.ai), third-party writeups and search snippets were used for those, flagged where it matters.

### 1. beads (`bd`)

- **URL**: https://github.com/steveyegge/beads → HTTP 301 to **https://github.com/gastownhall/beads** (Go module path is still `github.com/steveyegge/beads`). Docs: https://beads.gascity.com/
- **Language / License**: Go / MIT
- **Maturity** (checked 2026-08-17): 26,401 stars, 1,776 forks, created 2025-10-12, last push 2026-08-15. Latest release **v1.2.2 (2026-08-15)** — a "recovery release": v1.2.0/v1.2.1 (2026-08-11) were published by accident and untested; v1.2.1 migrated DBs to schema v65; v1.2.2 is the v1.1.2 code re-released; `go.mod` retracts 1.2.0/1.2.1/1.1.1 (recovery guide at beads.gascity.com/recovery/accidental-1-2-1-release). v1.0.0 was 2026-04-03. Distribution: brew, npm `@beads/bd`, `go install`, winget, AUR, Nix. Ships a Claude Code plugin (`/plugin marketplace add gastownhall/beads`).
- **What it does**: A "distributed graph issue tracker for AI agents" — a CLI-first work/memory store where issues, dependencies, comments and events are the durable state agents read/write; designed so `bd ready` gives an agent the next unblocked thing to do and `bd prime` injects a ~1–2k-token workflow context at session start (explicitly favoured over MCP for token cost).
- **Model**:
  - *Tasks/queues*: issue fields id, title, description, type (`bug|feature|task|epic|chore|decision|spike|story|milestone` + custom), status (`open|in_progress|blocked|closed` + custom), priority 0–4, labels, assignee, design/notes/acceptance, due/estimate/defer, external_ref, metadata, ephemeral, wisp_type, parent, pinned. **Hash-based IDs** `bd-a1b2` (prefix + hash of title+timestamp+salt, adaptive length; hierarchical `bd-a3f8.1.1`) to avoid collisions across concurrent agents/branches. **Dependency types**: blocking — `blocks`, `parent-child`, `conditional-blocks`, `waits-for`; non-blocking — `related`, `tracks`, `discovered-from`, `caused-by`, `validates`, `supersedes` (CLI also lists `until`, `relates-to`, `duplicates`, `replies-to`). Only blocking types remove an issue from `bd ready`. **Storage today (v1.x)**: **Dolt is source of truth** — embedded in-process by default (`.beads/embeddeddolt/`, single-writer file lock) or `bd init --server` against `dolt sql-server` for concurrent writers; every write is a Dolt commit; sync via `bd dolt push|pull` to a Dolt remote auto-configured on the git origin (history in `refs/dolt/data`). `.beads/issues.jsonl` is now an interchange/backup export refreshed by a pre-commit hook, *not* the sync channel. **Legacy (v0.x, Oct 2025–Feb 2026)**: git-tracked append-only JSONL as source of truth + SQLite read cache with a 5-second debounced sync — this is what the Rust ports preserve. Formulas/molecules: TOML **formula** → `bd cook` → **proto** → `bd mol pour --var k=v` → **molecule** (epic + child steps flowing through `bd ready --mol <id>`, parallel unless deps say otherwise); **wisps** = ephemeral molecules/beads (`--ephemeral`, excluded from federation, GC'd; `--wisp-type heartbeat|ping|patrol|gc_report|recovery|error|escalation`); **gates** = beads that block a step until a condition holds: `human`, `timer`, `gh:run`, `gh:pr`, `bead`; `bd gate check` in cron/CI auto-closes them.
  - *Workers*: no agent registry — assignee is a plain string. **Claims**: `bd update <id> --claim` / `bd ready --claim --json` are atomic CAS ("first claim wins", anti-steal except configured `claim.pools`). **Leases/heartbeat** (schema v54, on `main` and the retracted 1.2.1 — **not in the tested v1.2.2**): claim stamps `lease_expires_at = now + TTL` (default 5m) and `heartbeat_at`; `bd heartbeat <id>` extends (owner-only, node-local table, no Dolt commit); `bd reclaim --older-than <dur>` (default grace 2×TTL) reverts stale `in_progress` → `open`, clears assignee, writes a `lease_reclaimed` event; replica-aware via `granted_node`/`bd config set node_id`. Also `bd merge-slot create|acquire|release` (single-holder lock for conflict-prone ops), `bd mail`, `bd swarm`, `bd serve` HTTP API (`POST /v0/beads/issues/{id}:release`, `:claimNext`, `:batchClose`) on main.
  - *Isolation*: none built in (`bd worktree` helper exists; isolation is Gas Town's job).
  - *Merge/landing*: none built in (gates `gh:pr`/`gh:run` can wait on merges/CI).
  - *Human approval*: `type="human"` formula steps and `human` gates; `bd gate resolve`.
  - *Observability/cost*: events table, `bd metrics`, `bd sql`, `bd dolt` diff/history (`dolt_diff_<table>` audit); no cost tracking.
  - *Hooks/integration*: `bd hooks install` installs **git** hooks (pre-commit export, post-merge, pre-push, post-checkout, prepare-commit-msg agent trailer); `bd setup claude` installs a Claude Code **SessionStart** hook running `bd prime --hook-json`; `bd setup codex|factory`; Jira/Linear/GitHub/GitLab/ADO/Notion sync.
- **Rust rewrites**:
  - **beads_rust (`br`)** — https://github.com/Dicklesworthstone/beads_rust — Rust, "MIT with OpenAI/Anthropic Rider" (GitHub shows NOASSERTION; rider denies rights to OpenAI/Anthropic and forbids ML-training use), 1,052 stars, pushed 2026-08-17, v0.3.2 (2026-08-15), crates.io `beads_rust`. SQLite+JSONL only, **no Dolt**, no daemon/auto-git; same `.beads/` layout & JSONL format as classic bd; explicit `br sync --flush-only|--import-only|--merge`; `br coordination status --json` reports stale/competing claims (no auto-reclaim); `br capabilities --format json`; ~5–8 MB binary.
  - Others (all Rust, small): delightful-ai/beads-rs (MIT, 24★, independent design, crates.io `beads-rs`); rrnewton/minibeads (MIT, 12★, markdown-based drop-in); 3x3xX3N0N/BIR (MIT, 0★, "SQLite + Dolt backends"); fwindolf/beads-rs, Toshik1978/beads (0★); `rusty-beads` on crates.io (repo 404, unverified). crates.io names `beads`, `br`, `bd` are unrelated old crates.
- **USE or BORROW**:
  - USE: `bd` binary directly as the coordination layer (shell out with `--json`); the Claude Code plugin/`bd prime` SessionStart pattern; `bd ready --claim --json` as the atomic dequeue; gates (`human`/`gh:pr`/`gh:run`) as human-decision and CI-wait primitives; formulas (TOML) as the shape of enforced process steps (claim→worktree→work→verify→review→land→close as a molecule).
  - BORROW: hash-based collision-free IDs; the dependency-type taxonomy (esp. `conditional-blocks`, `waits-for`, `discovered-from`); lease schema (`lease_expires_at`, `heartbeat_at`, `granted_node`, `lease_reclaimed` event, grace = 2×TTL, "TTL > sync interval" invariant); merge-slot lock; wisp TTL compaction; the `.beads/issues.jsonl` line format (Rust ports already parse it — `beads_rust` is a ready reference for a serde model).
  - If a Rust-native tracker instead of shelling out is wanted: fork/depend on `beads_rust`'s SQLite+JSONL model (note its license rider), or `beads-rs`.
- **Why not adopt wholesale**: Go binary + embedded Dolt (heavy, CGO/ICU on some paths, single-writer lock in embedded mode); the lease/heartbeat/reclaim features are only on `main`/retracted releases as of today; the Aug-2026 accidental-release episode shows release-hygiene risk; no isolation, merge queue, cost, or process enforcement — those are exactly the runtime's job. Rust ports lag (`br` has no leases/reclaim, no Dolt).
- **Fetched**: github.com/steveyegge/beads (+ releases, commits, docs/ tree), raw README/CLI_REFERENCE/docs (hash-ids, dependencies, sync-concepts, molecules, gates, claude-code), GitHub API for gastownhall/beads and all Rust ports, shallow clone of gastownhall/beads (CHANGELOG, docs/multi-agent, cmd/bd/{heartbeat,reclaim,sync,claim}.go), beads.gascity.com/architecture/dolt, raw beads_rust README/LICENSE, crates.io API, virtuslab.com/blog/ai/beads-give-ai-memory.

### 2. Gas Town (`gt`) — and its successor Gas City

- **URL**: https://github.com/steveyegge/gastown (→ https://github.com/gastownhall/gastown) ; successor SDK https://github.com/gastownhall/gascity ; blog mirror https://yegge.ai/gastown, https://steveyegge.spicytakes.org/post/2026-01-20-welcome-to-gas-town
- **Language / License**: Go / MIT (both repos)
- **Maturity** (checked 2026-08-17): gastown 17,648 stars, 1,622 forks, 389 open issues, created 2025-12-16, last push 2026-08-13, latest release **v1.2.1 (2026-06-06)** (v1.0 announced 2026-04-03 alongside beads v1.0). Requires Go 1.26.2+, Git 2.20+, beads 0.57+, Dolt, sqlite3, tmux 3.0+, Claude Code CLI. **Gas City** (gastownhall/gascity): 1,133 stars, created 2026-02-22, pushed 2026-08-18, v1.0.0 April 2026 — "Gas Town torn apart and rewritten from the ground up as an SDK", composable "packs", default Gas Town pack is a drop-in; Yegge says Gas Town stays maintained with new maintainers. Yegge posts: "Welcome to Gas Town" (2026-01-01), "The Future of Coding Agents" (2026-01-05), "Software Survival 3.0" (2026-01-29), "Welcome to the Wasteland" (2026-03-04), "from Clown Show to v1.0" (2026-04-03), "Welcome to Gas City" (2026-04-24) — dates from search snippets/mirror, Medium bodies **unverified** (403).
- **What it does**: "Multi-agent workspace manager" that runs 20–30 Claude Code (or Codex/Gemini/Copilot/Cursor/…) sessions in tmux against a beads/Dolt work graph, with supervisor agents, a Bors-style merge queue, escalation routing and OTel telemetry. Yegge's framing: "K8s asks 'Is it running?' while Gas Town asks 'Is it done?'"; "an agent is not a session."
- **Model**:
  - *Tasks/queues*: everything is a bead (`gt-abc12`). **Hook** = "a special pinned Bead for each agent… the agent's primary work queue"; **GUPP** (Gas Town Universal Propulsion Principle) = "If there is work on your Hook, YOU MUST RUN IT"; a "GUPP violation" = hooked work with no progress → surfaced in problems view. **Convoy** = work-order bundling beads (`gt convoy create <name> <ids>`, `mountain` label enables stall detection/smart skip). `gt sling <bead> <rig>` puts work on an agent's hook. Formulas/molecules/wisps come from beads (`bd cook`, `bd mol pour`); MEOW = "Molecular Expression of Work" / "Mayor-Enhanced Orchestration Workflow". Scheduler: `scheduler.max_polecats` capacity-governed dispatch (deferred dispatch when full), `gt scheduler pause|resume|status`.
  - *Workers*: **Mayor** (coordinator Claude session you talk to), **Polecats** ("persistent identity but ephemeral sessions"; each in its own git worktree), **Crew** (human workspaces), **Witness** (per-rig patrol: stall detection, nudge/recovery, session cleanup), **Refinery** (per-rig merge queue), **Deacon** (cross-rig patrol daemon), **Dogs** (Deacon's maintenance workers, e.g. Boot for triage), daemon heartbeat every 3 min. Agent runtimes are presets (`claude`, `codex`, `gemini`, `kiro`, `cursor`, `copilot`, `amp`, `opencode`, …) set per rig. Comms: **mail** (async, injected at startup `gt mail check --inject`), **nudge** (real-time), **handoff** (context refresh), **seance** (query predecessor sessions via `.events.jsonl`).
  - *Isolation*: git worktrees ("hooks" are "git worktree-based persistent storage for agent work"); one **rig** = one repo; tmux sessions for every long-lived role (`gt up` boots Dolt, daemon, Deacon, Mayor, Witnesses, Refineries).
  - *Merge/landing*: **Refinery** — polecat runs `gt done` → branch pushed + MR bead → Refinery batches MRs, runs verification gates on the merged stack, green ⇒ merge all, red ⇒ bisect, merge the good ones, failing MR fixed inline or re-dispatched as a new bead. "Polecats never push directly to main."
  - *Human approval*: `gt escalate -s CRITICAL|HIGH|MEDIUM` creates tracked escalation beads routed Deacon → Mayor → Overseer (human); `gt escalate list|ack`; problems view (`gt feed --problems`) shows GUPP Violation/Stalled/Zombie/Working/Idle with `n` nudge / `h` handoff keys; beads `human` gates.
  - *Observability/cost*: `gt feed` 3-panel TUI (agent tree, convoys, event stream), `gt dashboard` (htmx web, port 8080), OTel logs+metrics to OTLP (default VictoriaMetrics/Logs) — metrics like `gastown.session.starts.total`, `gastown.bd.calls.total`, `gastown.polecat.spawns.total`, `gastown.done.total`; docs/otel-data-model.md. **No cost accounting in the tool**; reported spend: DoltHub trial ≈ $100/hour of Claude tokens (~10× a normal session); third-party estimates $2k–$5k/month (search-snippet level, **not verified from Yegge directly**).
- **USE or BORROW**:
  - BORROW (design): the role split (worker / per-repo witness / merge-queue refinery / cross-repo deacon / human overseer); GUPP as the invariant the lease/heartbeat check enforces ("hooked work with no progress for N min ⇒ violation"); Bors-style batched, bisecting merge queue with verification gates; convoy = batch handle for a fleet run; escalation severities as beads with ack; "persistent identity, ephemeral session" (identity + work state in the tracker/worktree, process is cattle); seance/`.events.jsonl` per-agent event log; OTel metric names as a starter set; `gt feed --problems` health states (GUPP violation, stalled, zombie, idle).
  - USE: beads formulas/molecules (already in bd) as workflow templates; the agent-preset table (`docs/agent-provider-integration.md`) for how each CLI is launched/resumed headlessly.
- **Why not adopt wholesale**: Go + tmux + Dolt + Claude-Code-in-tmux assumptions; hardwired topology (Gas City exists precisely because of that); very high open-issue count and self-described "vibe-designed" architecture (Maggie Appleton: "overlapping and ad hoc concepts"; Klabnik: "aggressively not rigorous"); no cost tracking; heavy token burn by design; not a library you can embed in a Rust runtime.
- **Fetched**: github.com/steveyegge/gastown, raw README, raw docs/glossary.md, GitHub API repo+releases (gastown, gascity), yegge.ai/essays/welcome-to-gas-city, steveyegge.spicytakes.org (Welcome to Gas Town; v1.0 post summary page), steveklabnik.com/writing/how-to-think-about-gas-town, maggieappleton.com/gastown, dolthub.com/blog/2026-01-15-a-day-in-gas-town. Medium originals: 403.

### 3. OpenAI Codex CLI

- **URL**: https://github.com/openai/codex ; docs https://learn.chatgpt.com/docs/… (developers.openai.com/codex/* now 308-redirects there)
- **Language / License**: Rust (workspace `codex-rs/`, 100+ crates) / Apache-2.0
- **Maturity** (checked 2026-08-17): 106,525 stars, 16,181 forks, pushed 2026-08-18; latest tag `rust-v0.148.0-alpha.21` (2026-08-17); created 2025-04-13. crates.io: only `codex-app-server-protocol` 0.63.0 published (2025-12-11, 1.4k downloads) — the rest are git-only.
- **What it does**: OpenAI's terminal coding agent (TUI + `codex exec` + `codex app-server`), with OS-level sandboxing, MCP client, hosted cloud tasks, and (2026) native multi-agent spawning.
- **Model**:
  - *Non-interactive*: `codex exec "<task>" --json` streams JSONL events: `thread.started`, `turn.started`, `turn.completed`, `item.started`/`item.completed` (commands, messages, reasoning, file changes), `error`; completed items/turns include `usage` with `input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`. Flags: `--output-schema <schema.json>` (structured final message), `-o/--output-last-message <path>`, `--sandbox read-only|workspace-write|danger-full-access`, `--ephemeral` (no session files), `--skip-git-repo-check`, `-C/--cd`, `-m`, `--ignore-user-config`, `--ignore-rules`, `codex exec resume [SESSION_ID]`, `codex exec -` (prompt on stdin), `--full-auto` deprecated → `--sandbox workspace-write`; auth via `CODEX_API_KEY` env for CI.
  - *App-server protocol* (`codex app-server`): JSON-RPC 2.0, bidirectional, over stdio JSONL (default), WebSocket (experimental, `/readyz` `/healthz`), or Unix socket. Handshake `initialize` (clientInfo, capabilities incl. `optOutNotificationMethods`, `experimentalApi`) → `initialized`. Primitives **Thread / Turn / Item**. Methods: `thread/start|resume|fork|list|read|archive|delete|unsubscribe|compact/start`, `thread/goal/set|get|clear` (token budgets), `thread/queue/*`, `turn/start` (inputs text/image/audio/skill/mention; overrides `model`, `effort`, `cwd`, `approvalPolicy`, `permissions`, `outputSchema`, `approvalsReviewer: "user"|"auto_review"` (Guardian subagent)), `turn/steer`, `turn/interrupt`, `review/start` (targets `uncommittedChanges|baseBranch|commit|custom`, `inline|detached`). Approvals: server sends `item/commandExecution/requestApproval` (or file/network variants), client replies `item/<id>/approval`. Notifications: `thread/status/changed`, `turn/completed` (with token usage), `item/*/delta`, `item/commandExecution/outputDelta`, `fs/changed`, etc. Backpressure: JSON-RPC error `-32001` "Server overloaded; retry later" ⇒ exponential backoff w/ jitter. Threads auto-unload after 30 min idle. Schema gen: `codex app-server generate-ts --out DIR` / `generate-json-schema`. Multi-agent hierarchy is visible in the protocol: `thread/list` with `parentThreadId`/`ancestorThreadId`, `thread/fork` with `deferGoalContinuation`.
  - *Sandboxing*: modes `read-only` (default outside git repos), `workspace-write` (default in git repos; writable = cwd + `/tmp`/`$TMPDIR`; `.git`, `.agents/`, `.codex/` stay read-only; network **off** by default, enable via `[sandbox_workspace_write] network_access = true`, optional `network_proxy` domain allowlist), `danger-full-access`. Mechanisms: macOS **Seatbelt** via `sandbox-exec`; Linux **bwrap + seccomp** by default now (repo has `linux-sandbox`, `bwrap`, `sandboxing`, `execpolicy` crates; earlier docs said Landlock — Landlock still in `linux-sandbox`, **exact current default unverified beyond the doc statement**); Windows via WSL2 or native "unelevated/elevated" sandbox. Approval policies: `untrusted`, `on-request`, `on-failure` (older docs), `never`, `granular`. Test with `codex sandbox macos|linux|windows [--permissions-profile <name>] -- CMD` (alias `codex debug seatbelt|landlock`). Container guidance: dev container with bubblewrap, or `--sandbox danger-full-access` inside your own container.
  - *MCP*: client via `[mcp_servers.<name>]` in `config.toml` or `codex mcp add <name> --env K=V -- <cmd>`; stdio and streamable-HTTP (bearer/OAuth/ChatGPT auth), per-tool approval modes, tool filtering. Codex-as-MCP-server: `codex-rs/mcp-server` crate exists in the tree; current official doc page for it **not found (unverified)**.
  - *Multi-agent/worktrees (2026)*: `[features] multi_agent_v2 = true`, `agent_max_depth` (default 3), roles in `.codex/agents/<role>.md`, tools `spawn_agent`, `send_message`, `followup_task`, `wait_agent`, `list_agents`, `close_agent`, path addressing `/root/researcher/...`, `spawn_agent` `fork_turns` semantics (issue #20077, 2026-04-28) — config keys from a third-party guide + one GitHub issue, so **partially unverified**; the Codex *app* keeps agents "isolated with worktrees" (search snippet); CLI users mostly do `git worktree add … && codex exec …` per task themselves.
  - *Cost*: token usage per item/turn in `--json` and in `turn/completed`; no USD figure emitted; `thread/goal` token budgets.
  - *Human approval*: approval requests over app-server; `approvalsReviewer: auto_review` delegates to a Guardian subagent; `--sandbox`/`approval_policy` combos for headless.
- **USE or BORROW**:
  - USE: run workers as `codex exec --json --sandbox workspace-write -C <worktree> --output-schema …` (JSONL parse with serde) or, better, drive **`codex app-server` over stdio JSON-RPC** for interrupt/steer/approvals — generate Rust types from `codex app-server generate-json-schema` (or depend on `codex-app-server-protocol` git crate; crates.io copy is stale 0.63.0). Copy its backpressure convention (`-32001` + jittered backoff) and Thread/Turn/Item event model as the worker-adapter abstraction. Reuse `execpolicy`/`sandboxing` crates as reference for Rust Seatbelt/bwrap wrappers.
  - BORROW: writable-roots + protected `.git` rule; approval-policy vocabulary; `outputSchema` structured-final-message idea for machine-readable "done" reports; per-thread token goal/budget.
- **Why not adopt wholesale**: it is a single-agent harness (multi-agent is intra-session, OpenAI-model-only, experimental); no issue tracker, worktree fleet management, merge queue, or cross-vendor workers; alpha releases daily; internal crates aren't published/semver-stable.
- **Fetched**: GitHub API repo+releases, github.com/openai/codex/tree/main/codex-rs, raw docs/exec.md & docs/sandbox.md (pointer pages), raw codex-rs/app-server/README.md, learn.chatgpt.com/docs/non-interactive-mode, /docs/agent-approvals-security, /docs/security, /docs/extend/mcp, github.com/openai/codex/issues/20077, crates.io API codex-app-server-protocol, codex.danielvaughan.com multi-agent v2 guide (third-party).

### 4. Symphony (OpenAI) — spec + Elixir reference (verified; not Rust)

- **URL**: https://github.com/openai/symphony (spec: `SPEC.md`; ref impl: `elixir/`); announcement https://openai.com/index/open-source-codex-orchestration-symphony/ (403 to fetcher; date 2026-04-28 per HelpNetSecurity/Codex KB coverage). Rust implementation of the spec: **OpenSymphony** https://github.com/kumanday/OpenSymphony (community, not OpenAI). Go port: https://github.com/junhoyeo/contrabass.
- **Language / License**: openai/symphony — Elixir (1.19+/OTP 28) / Apache-2.0. OpenSymphony — Rust (MSRV 1.97.1) / MIT.
- **Maturity** (checked 2026-08-17): openai/symphony 26,724 stars, 2,727 forks, created 2026-02-26, pushed 2026-08-12, self-described "low-key engineering preview for testing in trusted environments". OpenSymphony 79 stars, created 2026-03-21, pushed 2026-08-14, v1.0.0 = "GraphQL-only Linear rewrite" boundary. OpenAI says they had Codex implement the spec in TS/Go/Rust/Java/Python to polish it (search snippet; those ports **not published/unverified**).
- **What it does**: A language-agnostic *spec* for a scheduler that polls an issue tracker (Linear), gives each eligible issue a per-issue workspace, runs a Codex `app-server` session in it with a `WORKFLOW.md` prompt, retries with backoff, reconciles with tracker state, and lets the agent itself write status/comments/PR links back. "Manage work instead of supervising coding agents."
- **Model** (from SPEC.md):
  - *Tasks/queues*: `WORKFLOW.md` YAML front matter: `tracker.kind`, `provider`, `required_labels`, `active_states`, `terminal_states`; `polling.interval_ms` (30000); `workspace.root`; `hooks.after_create|before_run|after_run|before_remove` + `timeout_ms` (60000); `agent.max_concurrent_agents` (10), `max_turns` (20), `max_retry_backoff_ms` (300000), `max_concurrent_agents_by_state`; `codex.command` (`codex app-server` via `bash -lc`), `approval_policy`, `thread_sandbox`, `turn_sandbox_policy`, `turn_timeout_ms` (3.6M), `read_timeout_ms` (5000), `stall_timeout_ms` (300000). Dispatch eligibility = has id/identifier/title/state, state active & not terminal, adapter `dispatchable`, has all required labels, not already running/claimed, global + per-state slots free. Priority: "priority ascending 1..4, then created_at oldest first, then identifier lexicographic". Claim states: Unclaimed / Claimed (Running | RetryQueued) / Released. Retries: normal exit ⇒ 1000 ms continuation retry (re-check tracker); failure ⇒ `min(10000·2^(attempt−1), max_retry_backoff_ms)`. Run-attempt state machine: PreparingWorkspace → BuildingPrompt → LaunchingAgentProcess → InitializingSession → StreamingTurn → Finishing → {Succeeded, Failed, TimedOut, Stalled, CanceledByReconciliation}. Every tick: stall detection (`stall_timeout_ms`) + tracker refresh (terminal ⇒ terminate & clean; inactive ⇒ terminate w/o cleanup).
  - *Workers*: one Codex app-server process per issue; tracker credentials **stay host-side** — Symphony executes tracker tools (e.g. `linear_graphql`) on the agent's behalf; agent keeps a single persistent "workpad" comment as source of truth.
  - *Isolation*: `<workspace.root>/<sanitized identifier[+hash]>` directory (clone), path-prefix-checked; not git worktrees per se (hooks decide).
  - *Merge/landing*: agent opens PRs; "proof of work" via CI status, PR review, complexity analysis, walkthrough videos; merge on acceptance (README). No merge queue.
  - *Human approval*: via tracker state transitions/labels and PR review; `approval_policy` passthrough to Codex; "A run MUST NOT stall indefinitely waiting for user input"; approval posture is implementation-defined and must be documented.
  - *Observability/cost*: optional "Status Surface" / `GET /api/v1/state` (running sessions, retry queue, aggregate token/runtime totals, latest rate limits); structured logs with `issue_id/issue_identifier/session_id`; cumulative runtime seconds per issue; no cost tracking in spec. OpenSymphony adds TUI, desktop task-graph dashboard, DuckDB memory index; no cost tracking either.
- **USE or BORROW**: BORROW the entire scheduler spec — it is the closest thing to a formal spec for our dispatcher: eligibility predicate, per-state concurrency caps, deterministic priority ordering, run-attempt state machine, exponential backoff with cap, per-tick reconciliation vs. tracker truth, stall timeout, hook lifecycle (`after_create/before_run/after_run/before_remove` with timeouts and fatal/non-fatal semantics), workspace-key sanitization + hash suffix, credential-stays-in-orchestrator tool bridging. Swap "Linear" for beads as the tracker adapter (`bd ready --json` = `fetch_candidate_issues`). OpenSymphony is a usable Rust reference for talking to `codex app-server` over stdio from tokio.
- **Why not adopt wholesale**: Linear-centric, Codex-only agent runner (OpenSymphony adds OpenHands), no worktrees/merge queue/leases/cost, engineering preview, Elixir. OpenSymphony is a small single-maintainer project.
- **Fetched**: raw README.md and SPEC.md of openai/symphony, GitHub API for openai/symphony and kumanday/OpenSymphony, github.com/kumanday/OpenSymphony README.

### 5. Anthropic Claude Code as an orchestrated worker (+ Claude Agent SDK)

- **URL**: https://code.claude.com/docs/en/headless (+ cli-reference, worktrees, agent-teams, agent-view, hooks, agent-sdk/cost-tracking, sandboxing); SDK: `@anthropic-ai/claude-agent-sdk` (TS, https://github.com/anthropics/claude-agent-sdk-typescript), `claude-agent-sdk` (Python, https://github.com/anthropics/claude-agent-sdk-python, 7.9k★, MIT)
- **Language / License**: CLI is a closed-source binary (npm/native installer); Agent SDKs are open (TS/Python). **No official Rust SDK** — community crates only: `claude-agent-sdk-rs` (github.com/tyrchen/claude-agent-sdk-rs, MIT, 0.6.4, 2026-02-09, ~23.5k downloads, "bidirectional streaming, hooks, custom tools, plugin support"), `claude-agent-sdk` 0.1.1 (2025-09-30; its listed repo `anthropics/claude-agent-sdk-rust` returns 404), `claude-agents-sdk`, `claude-code-agent-sdk`, `claude-agent-sdk-rust`, `cc-sdk` 0.8.1 (ZhangHanDong). Open-source sandbox: github.com/anthropic-experimental/sandbox-runtime (TypeScript, Apache-2.0, 4,995 stars, pushed 2026-08-18).
- **Maturity** (checked 2026-08-17): docs reference CLI versions up to v2.1.234; features below note the minimum version where the docs state it.
- **What it does**: Full agent harness runnable headless (`claude -p`) with JSON/JSONL I/O, rich lifecycle hooks, subagents/teams, native worktree isolation, background-agent supervisor, OTel, and client-side cost estimates.
- **Model**:
  - *Headless invocation*: `claude -p "<prompt>" --output-format text|json|stream-json [--input-format stream-json] [--include-partial-messages] [--replay-user-messages] [--json-schema '<schema>'] [--max-turns N] [--max-budget-usd X] [--model …] [--effort low|medium|high|xhigh|max|ultracode] [--fallback-model …] [--append-system-prompt …] [--agents '<json>'] [--mcp-config …] [--strict-mcp-config] [--settings …] [--add-dir …] [--session-id <uuid>] [--resume <id|name>|--continue] [--fork-session] [--name …] [--bare]`. **`--bare`** = skip hooks/skills/plugins/MCP/CLAUDE.md discovery, API-key only, "recommended mode for scripted and SDK calls, and will become the default for `-p`". Exit 0 on success, non-zero on failure; SIGTERM ⇒ abort turn, kill Bash process tree, run `SessionEnd` hooks, exit 143. stdin capped at 10 MB. Background Bash tasks killed ~5 s after result; background subagents waited on up to 10 min (`CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS`).
  - *JSON output*: `--output-format json` result includes `result`, `session_id`, `total_cost_usd`, `usage` {input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}, `modelUsage` (per-model inputTokens/outputTokens/cache*/costUSD), `num_turns`, `duration_ms`, `structured_output` (with `--json-schema`), `subtype` (`success` | `error_max_budget_usd` | `error_during_execution` | …). Cost caveats: `total_cost_usd`/`costUSD` are **client-side estimates from a bundled price table**; result `usage` excludes subagents while `total_cost_usd`/`modelUsage` include them; per-step assistant `output_tokens` is a placeholder — read output from result; dedupe assistant messages by `message.id`; crash results may be zeroed. `stream-json`: `system/init` (model, tools, `mcp_servers`, `mcp_server_errors`, `plugins`, `plugin_errors`, `capabilities` array for feature detection ≥ v2.1.205), `assistant`, `user`, `stream_event` (with `--include-partial-messages`), `system/api_retry` (attempt, retry_delay_ms, error category), `system/plugin_install`, hook_started/progress/response, final `result`. Subagent messages carry `parent_tool_use_id`; `--forward-subagent-text` (≥ v2.1.211) emits their text/thinking; nested depth supported ≥ v2.1.219.
  - *Permissions*: `--permission-mode default|acceptEdits|plan|auto|dontAsk|bypassPermissions` (`manual` alias); `--dangerously-skip-permissions`; `--allowedTools "Bash(git diff *),Read,Edit"` / `--disallowedTools`; **`--permission-prompt-tool <mcp tool>`** to route approvals to the orchestrator's MCP server in headless mode; `auto` = classifier reviews actions; `dontAsk` = deny anything not allow-listed (good for locked-down CI). `-p` runs skip the workspace-trust dialog and *will* run project `.claude/settings.json` hooks and `.mcp.json` servers unless `--bare`.
  - *Hooks* (settings.json / plugin `hooks.json` / agent frontmatter): events `SessionStart, SessionEnd, Setup, UserPromptSubmit, UserPromptExpansion, Stop, StopFailure, PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest, PermissionDenied, PostToolBatch, SubagentStart, SubagentStop, TaskCreated, TaskCompleted, TeammateIdle, WorktreeCreate, WorktreeRemove, Notification, ConfigChange, InstructionsLoaded, CwdChanged, FileChanged, DirectoryAdded, MessageDisplay, PreCompact, PostCompact, Elicitation, ElicitationResult`. Types `command|http|mcp_tool|prompt|agent`; exit 0 = ok (JSON on stdout may carry decisions), **exit 2 = blocking**; JSON fields `continue`, `stopReason`, `systemMessage`, `additionalContext`, `hookSpecificOutput.permissionDecision allow|deny|ask`, `updatedInput`, `retry`; `async: true` + `asyncRewake`; default timeout 600 s (command/http/mcp), 30 s prompt, 60 s agent; `if: "Bash(git *)"` matchers.
  - *Isolation*: **`--worktree|-w <name>`** creates `.claude/worktrees/<name>` on branch `worktree-<name>` (base `worktree.baseRef: "fresh"` = origin default branch, or `"head"`); `--worktree "#1234"`/PR URL branches from a PR; `-p --worktree` skips trust check and does **not** clean up on exit (leaves `git worktree lock` until a stale-lock sweep); enforced isolation blocks Edits/Bash/git redirects into the main checkout ("You can't turn this check off"); subagents `isolation: worktree` frontmatter; `.worktreeinclude` copies gitignored files; `WorktreeCreate`/`WorktreeRemove` hooks replace git logic (return path on stdout); worktrees share `.git`, project plugins, and permission approvals. Sandboxed Bash: macOS Seatbelt, Linux/WSL2 bubblewrap + socat (+ optional seccomp), domain allowlist proxy with credential masking; native Windows unsupported; `allowUnsandboxedCommands` escape hatch. Alternatives doc: dev containers, Docker, VMs.
  - *Fleet/teams (2026)*: **Agent teams** (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`, experimental, v2.1.178+ semantics): lead + teammates (separate sessions), shared task list at `~/.claude/tasks/<team>/` with dependencies and **file-lock-based claiming**, mailboxes at `~/.claude/teams/<team>/inboxes/<agent>.json`, plan-approval gating, `TeammateIdle`/`TaskCreated`/`TaskCompleted` hooks, in-process or tmux/iTerm2 panes; **not available under `-p`/SDK** ("Claude doesn't spawn teammates" headless), no nested teams, no resume of in-process teammates. **Agent view / background agents** (research preview, ≥ v2.1.140): `claude --bg "task"`, `claude agents [--json]`, `claude attach|logs|stop|rm <id>`, `claude daemon status`; background sessions auto-move into `.claude/worktrees/` before editing; Haiku-generated one-line status; local-only; `--bg` cannot combine with `-p`. Cross-session messaging (`SendMessage` between local sessions/machines/web) and dynamic workflows (script-driven subagent orchestration) also exist. Cloud: `--cloud`, self-hosted runners with Prometheus metrics (docs exist; not fetched in depth).
  - *Observability*: OTel export for CLI and SDK (monitoring-usage / agent-sdk/observability pages), `system/api_retry` events, per-model usage.
- **USE or BORROW**:
  - USE: spawn workers as `claude -p --bare --output-format stream-json --input-format stream-json --include-partial-messages --permission-mode dontAsk|acceptEdits --allowedTools … --max-budget-usd … --max-turns … --json-schema <done-report> --session-id <uuid> --worktree <task-id>` (or our own `git worktree add` + cwd, since `-p --worktree` won't clean up anyway); parse the JSONL with serde (`system/init`, `assistant`, `result`); use `--resume <session_id>` for follow-up review/fix turns; run our own MCP server and pass **`--permission-prompt-tool`** to turn every approval into a human-decision-queue item; install `PreToolUse`/`Stop`/`SubagentStop`/`SessionEnd` hooks (command or **http** type — HTTP hooks let the runtime receive events without shelling) to enforce process gates (exit 2 to block a `git push` to main, block `Stop` until tests ran, etc.); read `total_cost_usd`/`modelUsage` (treat as estimate) plus `usage` cache fields; `--fallback-model`; `system/init.capabilities` for feature detection.
  - BORROW: hooks event taxonomy + exit-2 blocking + JSON decision schema as the design of *our* worker-agnostic hook layer; team task file-locking + mailbox JSON as a minimal local coordination format; agent-view state model (Working / Needs input / Completed / Failed) and `claude agents --json` shape; worktree enforcement rules (block edits/cwd/git-redirects into main checkout) and `.worktreeinclude`; sandbox-runtime (Seatbelt/bwrap+socat+proxy allowlist) as a Rust-portable spec.
- **Why not adopt wholesale**: closed binary; agent teams/agent view are interactive-only and experimental (no `-p`), single-machine, one team per session, no leases/heartbeats, no merge queue, no tracker; costs are estimates and Max-plan usage isn't priced; SDKs are TS/Python only (Rust crates are community, lagging protocol changes); `-p` without `--bare` executes untrusted project hooks/MCP.
- **Fetched**: code.claude.com/docs/en/{headless, cli-reference, llms.txt, worktrees.md, agent-teams.md, agent-view.md, hooks.md, agent-sdk/cost-tracking.md, sandboxing.md}; crates.io API (`claude-agent-sdk`, `claude-agent-sdk-rs`); GitHub API (anthropics/claude-agent-sdk-rust → 404; anthropic-experimental/sandbox-runtime).

---

## B. Rust-implemented orchestrators, runtimes and worker tools

### B1. Runtimes and worker tools surveyed in depth (goose, Vibe Kanban, ACP/Zed, cmux, Conductor, Overstory, Claude Squad, ruflo, agent-deck, container-use, Kilo/OpenCode/Amp, Ralph tooling)

#### 1. goose (Block → Agentic AI Foundation)

- **URL**: https://github.com/aaif-goose/goose (block/goose redirects here; docs at block.github.io/goose)
- **Language / License**: Rust (desktop is Electron/TS) / Apache-2.0
- **Maturity**: ★52.9k; pushed 2026-08-18; latest release v1.46.0 (2026-08-12); governed under the Agentic AI Foundation (Linux Foundation). Checked 2026-08-17.
- **What it does**: General-purpose local AI agent (desktop app, CLI, `goose-server` API). MCP-native ("extensions"), 15+ providers, "recipes" (YAML workflows), subagents/sub-recipes, cron scheduler, skills, ACP support (goose is listed as an ACP agent).
- **Model**:
  - tasks/queues: none as a task queue; recipes are runnable units; scheduler runs recipes on cron (`tokio-cron-scheduler` wrapper, persisted in `schedule.json`; a Temporal-backed scheduler variant also exists per discussion #4389).
  - workers: subagents = in-process `Agent::new()` instances (sequential by default, parallel on request; max turns 25 via `GOOSE_SUBAGENT_MAX_TURNS`, 5-min timeout, cannot recurse); sub-recipes may spawn `goose run --recipe` CLI processes.
  - isolation: **no worktree/container isolation verified** — third-party blogs claim "worktree isolation" but it is not in goose docs (unverified; may be conflated with Claude Code).
  - merge/landing: none. human approval: per-tool permission prompts only.
  - observability/cost: v1.44–1.46 added per-message token/cost/throughput usage stats and session totals; "model interactions viewer".
- **USE or BORROW**: Recipe YAML shape (instructions, prompt, extensions, parameters, sub_recipes) as a "job spec" format; `goose run --recipe X.yaml` as a headless worker backend behind our adapter; scheduler design (cron + persisted job file); its `rmcp`-based MCP client crates (goose upgraded to rmcp 2.0). Goose is an ACP agent, so one ACP adapter covers it.
- **Why not adopt wholesale**: It is a single-agent runtime, not a fleet controller: no queue, no leases, no worktrees, no merge queue. Repo is huge (desktop + server + CLI); its internal crates are not published as reusable libraries.

#### 2. Vibe Kanban (BloopAI)

- **URL**: https://github.com/BloopAI/vibe-kanban ; docs https://vibekanban.com/docs ; shutdown notice https://www.vibekanban.com/blog/shutdown
- **Language / License**: Rust backend (axum/sqlx/tokio, ts-rs types) + React/TS / Apache-2.0
- **Maturity**: ★27.8k; **company shut down 2026-04-10** ("couldn't find a business model"); repo continues as community-maintained OSS; last push 2026-04-24, last release v0.1.44 (2026-04-24). Remote/cloud services were turned off; local architecture remains. Checked 2026-08-17.
- **What it does**: Kanban board → per-task "workspace" (a git worktree on a new branch) → run a coding agent in it → diff review with inline comments → create/merge PR. `npx vibe-kanban`. Also exposes an MCP server so agents can create/update tasks.
- **Model**:
  - tasks/queues: kanban tasks with "task attempts"; sessions inside a workspace; statuses Running/Idle etc.
  - workers: `crates/executors` — one executor per agent: `claude.rs, codex.rs, copilot.rs, cursor.rs, droid.rs, gemini.rs, opencode.rs, qwen.rs, amp.rs` plus an `acp/` executor. Claude is driven with `--output-format=stream-json --input-format=stream-json --include-partial-messages --replay-user-messages --permission-prompt-tool=stdio --permission-mode=…`, `--resume <id>` / `--resume-session-at`; Codex via `codex app-server` JSON-RPC using the `codex-app-server-protocol` crate (pinned `rust-v0.124.0`); ACP via `agent-client-protocol = "0.8"`. Logs normalized to `json_patch::Patch` streams. Process groups via `command-group`.
  - isolation: git worktrees (`worktree-manager`, `workspace-manager` crates); setup/dev/cleanup scripts per project.
  - merge/landing: PR creation + merge via `git-host` crate; nothing pushed until user acts.
  - human approval: permission requests surfaced in UI (stdio permission tool for Claude; Codex `ask_for_approval`/sandbox modes).
  - observability/cost: not a focus (unverified for cost).
- **USE or BORROW**: The single most directly reusable Rust code for our purposes: the executor abstraction (per-agent process launch + streaming-log normalization + session resume), the `worktree-manager` crate, the ACP executor, the Codex app-server client, the ts-rs typed API pattern. Apache-2.0 → can vendor/fork crates.
- **Why not adopt wholesale**: It is a GUI-first single-user kanban, not a daemon with queue/leases/policies; company gone, so maintenance uncertain; crates are workspace-internal (path deps, `version = "0.1.44"`), so vendoring means forking.

#### 3. Zed Agent Panel / Agent Client Protocol (ACP)

- **URL**: https://github.com/agentclientprotocol/agent-client-protocol (redirect from zed-industries) ; https://agentclientprotocol.com ; Zed docs https://zed.dev/docs/ai/external-agents , https://zed.dev/docs/ai/parallel-agents
- **Language / License**: Rust (plus TS/Python/Kotlin/Java SDKs) / Apache-2.0
- **Maturity**: ★4.0k; pushed 2026-08-18; crates.io `agent-client-protocol` **2.0.0** (2026-07-23, 3.7M downloads), `agent-client-protocol-schema` 1.6.0; latest schema release `schema-v1.20.0` (2026-07-21). Stable protocol version 1; v2 schema directory exists. Very broad adoption. Checked 2026-08-17.
- **What it does**: JSON-RPC 2.0 over stdio between an editor/client and an agent process: `initialize` (capabilities/version), `session/new` (`cwd`, `mcpServers[]`), `session/load` (replays history), `session/prompt` → `session/update` notifications (message chunks, tool call status, plans), `session/request_permission`, `session/set_mode`, `session/cancel`. Agents implementing ACP: Gemini CLI, goose, OpenCode, Kilo, Copilot (preview), Cursor, Kiro, Qwen Code, OpenHands, Docker cagent, Junie, Factory Droid, Mistral Vibe, and adapters: **Claude Agent** (`@agentclientprotocol/claude-agent-acp`, formerly `@zed-industries/claude-code-acp`) and **Codex CLI** (Zed adapter). Zed 1.0 (2026-04-29) ships "Parallel Agents": Threads sidebar, one worktree per thread (worktree picker, `create_worktree` hook, auto-cleanup when thread archived), Terminal Threads (1.3.5, 2026-05-20). Merge = "your normal git workflow".
- **Model**: tasks/queues: none. workers: one agent subprocess per session; sessions are the unit. isolation: Zed creates linked worktrees per thread. merge/landing: none. human approval: `session/request_permission` (structured options), permission modes. observability/cost: none in the protocol (usage not part of ACP; unverified).
- **USE or BORROW**: **Use the `agent-client-protocol` crate directly** as our primary agent-process interface: it gives us one wire format for goose/OpenCode/Kilo/Gemini natively and Claude Code + Codex via the maintained adapters, plus structured permission requests we can route to a human-decision queue, `session/load` for resume, `mcpServers` injection (e.g., inject a "beads/orchestrator" MCP server per session), and `cwd` = the worktree path. Also borrow Zed's `create_worktree` hook idea and "clean worktree auto-removed on archive" semantics.
- **Why not adopt wholesale**: ACP is a session protocol, not an orchestrator: no queue, no lifecycle beyond a session, no cost telemetry. Claude Code goes through a Node adapter (extra process) — for Claude we may still want the native `stream-json` path (see Vibe Kanban) for usage/cost fields.

#### 4. cmux (manaflow-ai)

- **URL**: https://github.com/manaflow-ai/cmux
- **Language / License**: Swift (libghostty) + TS + Shell / GPL-3.0-or-later with commercial terms (GitHub shows "Other")
- **Maturity**: ★26.2k; pushed 2026-08-18; nightly builds, TestFlight iOS beta, "Founder's Edition" sponsorship. Checked 2026-08-17.
- **What it does**: Native macOS Ghostty-based terminal with vertical tabs, per-pane "needs attention" notifications (blue ring/badges), sidebar showing git branch/PR status/cwd/ports per pane, split-pane browser with scriptable API, SSH workspaces, session restore, and a Unix-socket API/CLI for automating workspaces/panes. Not a scheduler.
- **Model**: tasks/queues: none. workers: whatever CLI you run in a pane. isolation: none (you bring worktrees). merge: none. approval: attention notifications only. cost: none.
- **USE or BORROW**: The attention-state UX (agent waiting → ring/badge → jump to it) and the socket/CLI control API pattern for a terminal front-end; could be a *front-end* for our runtime (open panes per worktree via its CLI).
- **Why not adopt wholesale**: macOS-only Swift terminal; GPL; no orchestration logic.

#### 5. Conductor (conductor.build)

- **URL**: https://www.conductor.build ; docs https://www.conductor.build/docs/ ; changelog https://www.conductor.build/changelog
- **Language / License**: closed source; implementation language **unverified**. Proprietary (free tier + pricing page; YC company).
- **Maturity**: v0.81.0 "Cloud Polish #2" (2026-08-13); active weekly releases; Conductor Cloud launched v0.78.0. Checked 2026-08-17.
- **What it does**: Mac app running Claude Code, Codex (Plan/Fast/Skills modes), Cursor and OpenCode in parallel, one isolated workspace (worktree + branch + terminal + diff) per task; review, PR creation, archive; checkpoints; `conductor.json` setup/run scripts; Linear/GitHub issue integration; Graphite stacks; GitHub Actions/Vercel status; background tasks; multiplayer early access; remote/cloud workspaces.
- **Model**: tasks: workspaces created from prompts or Linear/GitHub issues; workers: agent CLIs; isolation: git worktrees (and cloud workspaces); merge: PR flow; human approval: interactive plan approval, "Manual Mode"; cost: **unverified**.
- **USE or BORROW**: `conductor.json` (setup/run/archive scripts per repo) as the pattern for per-worktree bootstrap hooks; checkpoint UX; issue-tracker → workspace linkage.
- **Why not adopt wholesale**: closed source, macOS-only GUI, no API for a headless fleet.

#### 6. Overstory (verified) → successor Warren

- **URL**: https://github.com/jayminwest/overstory (archived) ; https://github.com/jayminwest/warren
- **Language / License**: TypeScript (Bun) / MIT (both)
- **Maturity**: overstory ★1.3k, **archived 2026-05-28** ("no longer maintained… moved to Warren"). Warren ★319, pushed 2026-08-17, "Stable (0.17.0), running on GKE". Checked 2026-08-17.
- **What it does**: Overstory: CLI (`ov`) that turns Claude Code sessions into an orchestrated swarm: each worker in its own git worktree, spawned headless or in tmux; SQLite WAL "mail" bus with typed messages (`worker_done`, `merge_ready`, `dispatch`, `escalation`, broadcast `@all/@builders`); **FIFO merge queue with 4-tier conflict resolution** (clean merge → heuristic same-file non-overlap → Sonnet-assisted merge → human escalation), sentinel-file merge lock, dry-run merge prediction; tiered watchdog (mechanical PID/tmux poll → AI triage → monitor agent); pluggable `AgentRuntime` adapters (Claude Code stable; Pi, Codex, Gemini, Aider, goose, Amp, OpenCode experimental) handling spawn/config deploy/guard enforcement/readiness/transcript parsing; tool-call **guards** via Claude `settings.local.json` hooks; `ov costs` per agent/run/**bead**/capability from JSONL transcripts; NDJSON event log; `ov serve` web dashboard, `ov dashboard` TUI; **beads integration** for tasks. Warren: control plane (Bun HTTP API + React) dispatching ephemeral sandboxed runs (bubblewrap locally, k8s pods in cluster) against GitHub repos, live streaming, mid-run steering, cron/plan-run scheduling, pushes a branch back.
- **Model**: tasks: beads (overstory) / dispatches (warren); workers: subprocesses per worktree (overstory) or sandboxes (warren); merge: FIFO queue (overstory), branch push (warren); approval: escalation tier + `ov nudge`; cost: transcript-derived per-bead costs.
- **USE or BORROW**: The 4-tier merge escalation ladder + merge lock + dry-run; watchdog tiers; the `AgentRuntime` adapter contract (spawn, deploy config, guards, readiness, transcript parse); per-bead cost attribution from JSONL; typed mail message set. Read its STEELMAN.md ("swarms are not universal… compounding error rates, cost amplification, merge conflicts are the normal case").
- **Why not adopt wholesale**: archived; TypeScript/Bun; tmux-centric; author explicitly pivoted away.

#### 7. Claude Squad (smtg-ai)

- **URL**: https://github.com/smtg-ai/claude-squad
- **Language / License**: Go / AGPL-3.0
- **Maturity**: ★8.3k; pushed 2026-07-30; latest v1.0.19 (2026-06-17). Checked 2026-08-17.
- **What it does**: TUI (`cs`) that runs Claude Code / Codex / Gemini / Aider / OpenCode / Amp each in a detached tmux session inside its own git worktree; keys to attach, review diff, commit+push, checkout, resume; experimental `--autoyes`.
- **Model**: tasks: none (you name instances); workers: tmux sessions; isolation: worktrees; merge: manual checkout/push; approval: attach & answer; cost: none.
- **USE or BORROW**: Minimal reference for tmux-backed detached sessions + worktree lifecycle; UX keys (n/↵/s/c/r).
- **Why not adopt wholesale**: AGPL; no queue/policy; interactive-only.

#### 8. claude-flow → ruflo (ruvnet)

- **URL**: https://github.com/ruvnet/ruflo (claude-flow redirects)
- **Language / License**: TypeScript (+ some Rust/WASM) / MIT
- **Maturity**: ★68.1k; pushed 2026-08-17; 7.3k commits. Checked 2026-08-17.
- **What it does**: "Agent meta-harness" for Claude Code: `npx ruflo init` installs 98–100+ agent definitions, ~210 MCP tools, hooks, a daemon, "hive-mind"/swarm topologies (hierarchical/mesh/adaptive; "Raft/Byzantine/Gossip consensus"), HNSW vector memory ("AgentDB"), "SONA self-learning", federation with ed25519/mTLS, PII pipeline, hosted flo.ruv.io/goal.ruv.io.
- **Model**: tasks: internal swarm task routing; workers: Claude Code subagents/MCP; isolation: none verified (no worktree isolation documented); merge: none; approval: none specific; cost: "cost-tracking built-in" claimed (unverified).
- **USE or BORROW**: Nothing structural. At most, ideas for a hooks-driven "post-edit/post-task" pipeline.
- **Why not adopt wholesale / credibility**: README is dominated by unverifiable performance claims ("1.3×–1953× wins", "89% routing accuracy", consensus algorithms for local subagents); huge surface area; not a coordination runtime with real isolation. High star count does not reflect production use of the orchestration layer. Treat as marketing-heavy.

#### 9. agent-deck (verified)

- **URL**: https://github.com/asheshgoplani/agent-deck
- **Language / License**: Go / MIT
- **Maturity**: ★739; pushed 2026-08-17. Checked 2026-08-17.
- **What it does**: tmux-based TUI dashboard for Claude Code / Gemini / OpenCode / Codex sessions: session forking, per-session MCP toggling with socket pooling, global search across conversations, worktree-per-task, jump-to-waiting-session from tmux status line.
- **Model**: no queue; workers = tmux sessions; isolation = worktrees; merge manual; approval via attach; cost none.
- **USE or BORROW**: MCP socket-pooling idea (share one MCP server across sessions); "waiting" detection surfaced in tmux status.
- **Why not adopt wholesale**: interactive TUI only.

#### 10. container-use (Dagger)

- **URL**: https://github.com/dagger/container-use
- **Language / License**: Go / Apache-2.0
- **Maturity**: ★4.0k; pushed 2026-08-17 but last release v0.4.2 (2025-08-19); README says "Experimental". Checked 2026-08-17.
- **What it does**: MCP server (`container-use stdio`, alias `cu`) giving each agent an "environment" = a fresh container (Dagger) bound to its own git branch; `cu watch/log/checkout/merge/apply/diff/terminal`; works with Claude Code, Cursor, goose, VS Code, any MCP client.
- **Model**: tasks: none; workers: whichever agent uses the MCP tools; isolation: **containers + git branches** (environment history stored in git); merge: `cu merge`/`cu apply`; approval: human reviews branch; cost: none.
- **USE or BORROW**: The "environment = branch + container, exposed as MCP tools" pattern; using git notes/refs to store environment history; `cu apply` (apply without commit) vs `cu merge`.
- **Why not adopt wholesale**: Requires Dagger engine; experimental; container-per-agent is heavier than worktrees for our default path; no queue/policy.

#### 11. Kilo / OpenCode / Amp (brief)

- **OpenCode** — https://github.com/anomalyco/opencode (sst/opencode redirects) — TypeScript / MIT — ★198.5k, v1.18.18 (2026-08-13). Headless: `opencode run "…" --format json --session <id> --continue --attach http://localhost:4096 --agent --model provider/model`; `opencode serve` (HTTP + OpenAPI at `/doc`, `@opencode-ai/sdk`, `OPENCODE_SERVER_PASSWORD`); `opencode acp` (ACP over stdio); `opencode stats` (tokens + cost, by project/model/timeframe); `opencode export --sanitize`; parallel sessions via worktrees are supported in the TUI/desktop (no `opencode worktree` CLI command documented). BORROW: attach-to-server model (one long-lived server, many `run` clients), `stats` schema, ACP.
- **Kilo Code** — https://github.com/Kilo-Org/kilocode — TypeScript / MIT — ★26.9k, pushed 2026-08-17. CLI `npx @kilocode/cli`: `kilo run "…" --auto` (autonomous), `kilo --continue`, `kilo stats` (tokens/cost), `kilo export`, `kilo acp`, `kilo mcp`, `kilo cloud` (Cloud Agents; `/remote` to expose local session). VS Code "Agent Manager" with parallel agents on git worktrees. No `--json` documented (unverified). BORROW: same as OpenCode (it's a fork lineage); ACP.
- **Amp (Sourcegraph)** — https://ampcode.com/manual — closed source; language **unverified**. Headless `amp -x "…"` / `--execute`, `--stream-json`, `--stream-json-thinking`, `--stream-json-input` (multi-turn JSON on stdin), `amp threads list|continue <id>|fork <id>` (threads sync to ampcode.com), subagents, Oracle (second-opinion model), Librarian (cross-repo search), `amp usage` for balance, costs shown by default (`amp.showCosts`), `amp.mcpPermissions`. No worktree feature documented (unverified). BORROW: `--stream-json-input` bidirectional JSONL and thread fork semantics.

#### 12. Ralph Wiggum loop / ralph tooling

- **Origin**: Geoffrey Huntley, https://ghuntley.com/ralph/ (2025-07-14): `while :; do cat PROMPT.md | claude-code ; done` — one task per iteration, same spec/plan files each loop ("deterministic stack allocation"), backpressure via tests/compilation, fresh context per iteration, learnings logged to AGENT.md/fix_plan.md.
- **Official Anthropic plugin**: `ralph-wiggum` in https://github.com/anthropics/claude-code/tree/main/plugins/ralph-wiggum and `ralph-loop` in anthropics/claude-plugins-official — a **Stop hook** blocks exit and re-injects the prompt; state in `.claude/ralph-loop.local.md` (YAML frontmatter: active, iteration, max_iterations, completion_promise, start time; body = prompt).
- **ralph-orchestrator** — https://github.com/mikeyobrien/ralph-orchestrator — **Rust** / MIT — ★3.1k, pushed 2026-08-16, crate `ralph-cli` 2.10.1 (2026-06-23). "Hat"-based personas (code-assist/debug/research/review), event loop, `ralph.yml`, backends: Claude Code, Codex, Gemini, Kiro, Amp, Copilot, OpenCode, Forge; exits on `LOOP_COMPLETE` or iteration cap; backpressure gates (tests/lint/typecheck); Telegram human-in-the-loop that blocks until answered; MCP server mode; alpha web dashboard. Worktrees: not documented (unverified).
- **ralphex** — https://github.com/umputun/ralphex — Go / MIT — ★1.4k, pushed 2026-08-17: fresh session per plan task, validation, retries, multi-phase review, auto-commit (Claude Code, Codex).
- Others: ralphy (bash, resting since 2026-02), open-ralph-wiggum (OpenCode), ralph-tui, wreckit, lalph (issue-driven), awesome-ralph list.
- **USE or BORROW**: The loop contract itself for our "work" step: fresh process per iteration, PROMPT/spec files as inputs, explicit completion sentinel, iteration cap, verification gate before re-loop. ralph-orchestrator is a Rust codebase to mine for backend adapters and gate wiring (MIT).
- **Why not adopt wholesale**: single-goal loops, no fleet/queue/worktree/merge model.


### B2. Other Rust-implemented orchestrators / runtimes found in the sweep

#### herdr — "the runtime your coding agents live on"
- **URL**: https://github.com/herdrdev/herdr (docs: https://herdr.dev)
- **Language / License**: Rust / Apache-2.0
- **Maturity**: ~30.1k stars, v0.4.0, pushed 2026-08-18 (checked 2026-08-17 via GitHub API)
- **What it does**: Single Rust binary daemon that hosts persistent, reconnectable terminal sessions ("panes") for Claude Code / Codex / Cursor / etc. Sessions survive lid-close, network drops, machine restarts; reattach from any terminal or over ssh; per-pane status (working / blocked / idle); tmux-style multiplexing; plugin marketplace.
- **Model**: tasks/queues — none (it is a session runtime, not a scheduler); workers — one agent per pane, agents can spawn panes and "prompt each other" and "wait until another agent is genuinely blocked" via CLI + socket API; isolation — sessions, no built-in worktree/merge policy (unverified beyond README); human approval — n/a; observability/cost — status detection only (unverified detail).
- **USE or BORROW**: The pane/status ("blocked" detection) and socket API are the exact primitive we need for a *worker session layer*: consider running each ai_runner worker inside a herdr pane rather than writing our own PTY supervisor. Borrow: single-binary daemon + CLI + socket API design; "agent is genuinely blocked" detection heuristics.
- **Why not adopt wholesale**: It has no task model, no leases, no merge queue, no cost accounting; it is the terminal/session substrate only.

#### majiayu000/harness — Rust control plane for Claude Code & Codex
- **URL**: https://github.com/majiayu000/harness
- **Language / License**: Rust / MIT
- **Maturity**: 60 stars, ~1.65k commits, created 2026-03-02, pushed 2026-08-17 (checked 2026-08-17). Solo-maintained; no semver tags.
- **What it does**: "Run fleets of parallel coding agents with governance": JSON-RPC (30 methods) router over threads/tasks/turns/exec-plans; adapters for Claude CLI, Codex CLI, Anthropic API; Starlark policy rules; cross-agent review; OTLP observability; GC/remediation drafts.
- **Model**: tasks — REST/GitHub-webhook "workflow runtime submissions" persisted in **Postgres** (SQLite removed), per-project queue permits; workers — subprocess adapters, sandbox tiers (read-only / workspace-write / danger-full-access; Landlock/bubblewrap on Linux, Seatbelt on macOS); isolation — one git worktree per implementation task; merge — delegated to GitHub PR flow; human approval — none explicit; observability/cost — OTLP traces+metrics, per-signal USD budgets for GC.
- **USE or BORROW**: Crate decomposition (`harness-core/protocol/agents/rules/observe/exec/server`) is a good template for our workspace layout; the flow "acquire project queue permit → create worktree → agent executes → validate + review → persist evidence" mirrors ours; Starlark-as-policy idea; ExecPlan model serialized as Markdown; TOML config with `[[projects]]` and per-repo `.harness/config.toml`.
- **Why not adopt wholesale**: Postgres dependency, single maintainer, no beads integration, immature (60 stars), no human-decision queue, no leases model exposed.

#### ralph-orchestrator (Rust implementation of the Ralph Wiggum loop)
- **URL**: https://github.com/mikeyobrien/ralph-orchestrator
- **Language / License**: Rust (+ React web UI) / MIT
- **Maturity**: ~3.1k stars, created 2025-09-07, pushed 2026-08-16, ~536 commits (checked 2026-08-17)
- **What it does**: Loops a coding agent (Claude Code, Codex, Gemini CLI, Amp, Copilot CLI, OpenCode, Kiro, Forge) until `LOOP_COMPLETE` or iteration limit; "hats" (personas) with event-driven handoffs; quality gates (tests/lint/type) as backpressure; Telegram human-in-the-loop ("RObot"); ratatui TUI; MCP server; web dashboard (alpha).
- **Model**: tasks — YAML config (`ralph.yml`, per-hat variants) + task/plan files under workspace root; workers — one loop per workspace root; isolation — workspace-scoped, no worktree fan-out documented (unverified); merge — n/a; human approval — Telegram blocking prompts; cost — not documented.
- **USE or BORROW**: Backend adapter trait covering 8 CLIs (how each is invoked headlessly, how completion is detected); the "hat" handoff event model; ratatui TUI patterns; Telegram HITL as a cheap human-decision channel.
- **Why not adopt wholesale**: It is a single-agent loop runner; no fleet scheduling, leases, or worktree/merge pipeline.

#### OpenSymphony — Rust implementation of OpenAI's Symphony spec
- **URL**: https://github.com/kumanday/OpenSymphony
- **Language / License**: Rust / MIT
- **Maturity**: 79 stars, created 2026-03-21, pushed 2026-08-14 (checked 2026-08-17)
- **What it does**: Implements the Symphony daemon (poll Linear → per-issue workspace → run agent) with two harnesses: OpenHands agent-server (managed) or local `codex app-server`.
- **Model**: per Symphony spec (see Symphony entry): tracker states, `WORKFLOW.md`, retries with exponential backoff, stall detection; isolation — per-issue workspace dir; merge/human approval — agent-driven via tracker; observability — `/api/v1/state`.
- **USE or BORROW**: A worked example of driving `codex app-server` (JSON-RPC over stdio) from Rust; Symphony's `WORKFLOW.md` front-matter schema; retry/backoff formula `min(10000*2^(attempt-1), max_retry_backoff_ms)`.
- **Why not adopt wholesale**: Linear-only tracker, small community, no beads, no worktree/merge/lease semantics beyond the spec.

#### anantjain-xyz/symphony-rust — Symphony desktop app in Rust
- **URL**: https://github.com/anantjain-xyz/symphony-rust
- **Language / License**: Rust / MIT
- **Maturity**: 11 stars, created 2026-05-31, pushed 2026-08-13 (checked 2026-08-17). Very early.
- **What it does**: Desktop app watching a Linear board, dispatching Codex or Claude Code into freshly cloned workspaces per issue.
- **USE or BORROW**: Another reference for launching Claude Code / Codex headlessly from Rust. Otherwise too small.

#### pueue — Rust daemon/CLI job queue for shell commands
- **URL**: https://github.com/Nukesor/pueue (crates: `pueue` 4.0.4, `pueue-lib` 0.31.1, both 2026-03-02)
- **Language / License**: Rust / Apache-2.0
- **Maturity**: ~6.3k stars, v4.0.4 (2026-03-02), pushed 2026-08-16 (checked 2026-08-17)
- **What it does**: `pueued` daemon + `pueue` client: queue shell tasks with groups (per-group parallelism), dependencies (`--after`), delayed start, pause/resume/kill, stdout/stderr capture, edit-in-editor, callbacks on finish, state persisted to disk, TLS-secured local socket protocol.
- **Model**: tasks — numeric IDs, groups, status enum (Queued/Running/Paused/Done(Success|Failed|Killed|DependencyFailed)), dependencies; workers — daemon spawns processes via `command-group`/process groups; isolation — none; human approval — n/a; observability — `pueue status/log/follow`, callbacks.
- **USE or BORROW**: `pueue-lib` is reusable as a crate (message protocol, state serialization, daemon/client split, secure local socket with secret file). Borrow the group-based concurrency limits, callback hooks, `follow`/`log` UX, and the daemon/client architecture. Could literally run agent invocations as pueue tasks in an MVP.
- **Why not adopt wholesale**: No leases/heartbeats, no task semantics beyond shell exit codes, no worktree/merge, single-machine, and no structured events from the child (agents' stream-json would be opaque).



---

## C. Non-Rust orchestrators / fleet managers (design borrowing)

### Symphony (OpenAI)
See the deep dive in section A.4 (spec vocabulary, states, timeouts, `/api/v1/state`). Cross-references: Rust ports OpenSymphony / symphony-rust (B2), Go port Contrabass (C2).

### Agent Orchestrator (Untrivial-ai)
- **URL**: https://github.com/Untrivial-ai/agent-orchestrator
- **Language / License**: Go / Apache-2.0
- **Maturity**: ~9.6k stars, pushed 2026-08-18 (checked 2026-08-17)
- **What it does**: Desktop "Agent IDE" for fleets: persistent "project orchestrator" planning agent breaks work into tasks and spawns/redirects workers; each Git-backed worker gets its own branch+worktree ("Scratch workers" get branchless dirs); Kanban derived from session + PR + CI + review facts (Working / Needs You / In Review / Ready to Merge); 26 harnesses (Claude Code, Codex, Aider, OpenCode, Cursor, Copilot, Goose, Amp, Droid, Kimi...).
- **Model**: tasks — planner-driven, PR-centric; isolation — worktree per worker; merge — via PR/CI; human approval — "Needs You" column; observability — status aggregation, browser preview per worker.
- **USE or BORROW**: The 26-harness adapter matrix (how each CLI is launched/steered) and the *derived-status* idea (card position computed from facts, not stored) — that is exactly how a beads-backed state should be projected. Borrow "Needs You" as our human-decision queue UX.
- **Why not adopt wholesale**: Go desktop app; planner-in-the-loop instead of an issue tracker; not beads-aware; PR-only landing.

### Orca (stablyai) — ADE for a fleet of parallel agents
- **URL**: https://github.com/stablyai/orca
- **Language / License**: TypeScript/Electron / MIT
- **Maturity**: ~47.6k stars, ~8.9k commits, pushed 2026-08-18 (checked 2026-08-17)
- **What it does**: Desktop app running any CLI agent (30+) in parallel worktrees, compare outputs, annotate diffs and send comments back to agents; GitHub/Linear integration; usage/rate-limit tracker for Claude/Codex accounts; `orca.yaml`; CLI (`orca worktree create`, `snapshot`).
- **USE or BORROW**: Diff-annotation → agent feedback loop for the *review* step; account usage/rate-limit-reset display; "open a worktree from any task".
- **Why not adopt wholesale**: GUI-first Electron; human drives everything; no queue/leases.

### Bernstein — deterministic orchestrator for CLI coding agents
- **URL**: https://github.com/sipyourdrink-ltd/bernstein
- **Language / License**: Python / Apache-2.0
- **Maturity**: ~0.9k stars, pushed 2026-08-18, solo-maintained beta (checked 2026-08-17)
- **What it does**: One LLM call decomposes a goal into tasks with owned files; then "no model in the coordination loop": plain scheduler runs agents in per-task worktrees behind merge gates; a "janitor" verifies concrete signals (tests/files/lint) before merge; Ed25519-signed run receipts + optional HMAC audit chain; declarative `plan.yaml` DAG (agent/command/loop nodes); backlog claimed atomically.
- **USE or BORROW**: "Owned files" partitioning to pre-empt merge conflicts; deterministic replay + signed receipts as an audit trail (a lightweight version fits our event log); the janitor role = our verify step; explicit "the only shared state is the task backlog".
- **Why not adopt wholesale**: Python, single maintainer, own backlog format.

### Emdash — open-source Agentic Development Environment (YC W26)
- **URL**: https://github.com/generalaction/emdash — TypeScript/Electron / Apache-2.0 — ~5.4k stars, pushed 2026-08-17 (checked 2026-08-17). Multi-provider CLI runner with worktrees and port-collision prevention. Borrow: per-worktree port allocation for dev servers.

### Baton / Code Conductor / Microsoft Conductor
- **Baton** https://github.com/mraza007/baton — Python/MIT, 20 stars, pushed 2026-03-27: polls GitHub Issues → Claude Code in worktrees via "poll → dispatch → reconcile" loop (same shape as Symphony). Borrow the reconcile loop naming.
- **Code Conductor** https://github.com/ryanmac/code-conductor — Python/MIT, 111 stars, pushed 2026-03-30: GitHub-native, labels issues as claimable tasks for Claude Code sub-agents. Borrow: label-based claim protocol as inspiration for beads claim.
- **Microsoft Conductor** https://github.com/microsoft/conductor — Python/MIT, 393 stars, pushed 2026-08-17: YAML-defined multi-agent workflows over GitHub Copilot SDK + Anthropic. Borrow: YAML workflow schema examples.
- (checked 2026-08-17 via GitHub API; READMEs not deep-read beyond the Augment survey https://www.augmentcode.com/tools/open-source-agent-orchestrators, updated 2026-08-12)

### Overstory → Warren (jayminwest)
See B1.6 (archived TS orchestrator with beads integration, SQLite-WAL mail bus, 4-tier merge ladder; successor Warren).

### Archon (coleam00) — "harness builder" with isolated worktrees
- **URL**: https://github.com/coleam00/Archon — TypeScript/MIT, ~23.2k stars, pushed 2026-08-17 (checked 2026-08-17). Workflow engine for AI coding agents, deterministic/repeatable, worktree isolation. Not deep-read; note for design comparison only (unverified internals).

### Omnigent — Python meta-harness (~9k stars, Apache-2.0, https://github.com/omnigent-ai/omnigent, pushed 2026-08-18; checked 2026-08-17). Orchestrates Claude Code / Codex / Cursor with shared sessions. Not deep-read.

### Crystal → Nimbalyst
- https://github.com/stravu/crystal — TS/MIT, ~3.1k stars, last push 2026-02-26; "Crystal is now Nimbalyst" (https://nimbalyst.com), a commercial desktop app for parallel Codex/Claude Code sessions in worktrees. (checked 2026-08-17)

### CloudCLI / claudecodeui (siteboon) — AGPL-3.0 web/mobile control surface for Claude Code, OpenCode, Cursor CLI, Codex; ~13.3k stars; https://github.com/siteboon/claudecodeui (checked 2026-08-17). Borrow: remote-control UX only; AGPL.

### agent-deck
See B1.9.

### C2. Additional finds from the sweep (Paperclip, sortie, kandev, coder/mux, tutti, forge-orchestrator, multiclaude, Contrabass, worktree/session managers)


#### Paperclip
- https://github.com/paperclipai/paperclip — TypeScript/React, Postgres / MIT — ★78.7k, pushed 2026-08-18. Agents wake on **heartbeats**, atomically check out tickets, blocker deps, org chart/roles, **per-agent monthly token budgets with hard stops**, board approval gates, adapters (Claude Code, Codex, Cursor, bash, HTTP). BORROW: heartbeat-wake + budget-check + ticket-checkout loop; budget hard-stop policy. Not adopt: TS/Postgres, "company OS" framing.

#### sortie
- https://github.com/sortie-ai/sortie — Go / Apache-2.0 — ★125, pushed 2026-08-18. Tracker adapters (GitHub/GitLab/Gitea/Linear/Jira) → per-issue workspace → agent over stdio (Claude Code, Copilot, OpenCode, Codex, Kiro); SQLite retry queues/session metadata/run history; stall detection/timeouts; "orchestrator is the single authority for scheduling". Small but architecturally the closest single-binary analogue.

#### kandev
- https://github.com/kdlbs/kandev — Go + React / AGPL-3.0 — ★649, pushed 2026-08-18. Kanban + multi-step workflows with a different agent per step behind human gates; executors local/Docker/SSH/cloud; worktrees; 20+ agents via **ACP**; integrated review/PR. BORROW: per-step agent + gate pipeline model; ACP-everywhere. AGPL blocks code reuse.

#### coder/mux
- https://github.com/coder/mux — TypeScript / AGPL-3.0 — ★2.0k, pushed 2026-08-17. Desktop/browser parallel agentic dev: worktrees, SSH runtimes, review UI, plan/execute, git divergence view, **cost/token dashboard**, "opportunistic compaction". AGPL.

#### tutti (Rust)
- https://github.com/nutthouse/tutti — Rust / MIT — ★112, v0.10.0 (2026-05), pushed 2026-07-28. `tutti.toml` team topology; stages intake→execution→review→gate→record; typed artifact passing between steps; per-agent worktrees; run ledger + checkpoints + SQLite event log; Claude Code/Codex/Aider/OpenClaw. BORROW: typed-artifact pipeline & gate/ledger schema; small enough to read fully.

#### forge-orchestrator (Rust)
- https://github.com/nxtg-ai/forge-orchestrator — Rust / **FSL-1.1-ALv2** (→ Apache-2.0 on 2028-03-18) — ★154, v1.6.1, pushed 2026-08-09. Single 4.7 MB binary; MCP server (11 tools) giving Claude Code/Codex/Gemini shared **file locking**, knowledge capture (`.forge/knowledge/`), spec drift detection. BORROW: file-lock/queue semantics for same-repo multi-tool sessions. License is source-available, not OSS yet.

#### multiclaude (Dan Lorenc)
- https://github.com/dlorenc/multiclaude — Go / MIT — ★562, pushed 2026-01-28 (quiet). Daemon + workers in tmux windows + worktrees; workers open PRs; CI as one-way filter; merge queue auto-merges passing PRs ("Brownian ratchet"). BORROW: "CI is the only gate; redundant work is acceptable" merge-queue stance.

#### Contrabass (Go port of Symphony)
- https://github.com/junhoyeo/contrabass — Go/Charm, Apache-2.0, ★213, pushed 2026-07-17. Adds Linear/GitHub/file board, claim/release teams, plan→exec→verify pipeline, stage classification, TUI/headless/dashboard.

#### Worktree/session managers (thin, mostly UX)
- **ccmanager** https://github.com/kbwo/ccmanager — TS/MIT, ★1.2k, pushed 2026-08-10: no tmux; per-agent state detection (waiting/busy/idle) for 8 CLIs; status-change hooks; worktree hooks; devcontainer; experimental AI auto-approve. BORROW: the per-agent terminal-state detection heuristics + status hooks.
- **uzi** https://github.com/devflowinc/uzi — Go/MIT, ★581, **stale since 2025-06**: `uzi.yaml` (devCommand with `$PORT`, portRange), `uzi checkpoint` rebases agent worktree into current branch, `uzi auto` presses Enter. BORROW: port-range allocation per worktree.
- **gwq** https://github.com/d-kuro/gwq — Go/Apache-2.0, ★462, pushed 2026-05-02: worktree fuzzy finder, global discovery, tmux; a `gwq task` parallel-execution feature is claimed in third-party sources but **unverified** in README.
- **para** https://github.com/2mawi2/para — **Rust**/MIT, ★19, stale 2025-09: worktree sessions + VS Code windows, sandbox/Docker options, `para finish` → feature branch, MCP for coordination. Small; only for ideas.
- **crystal → Nimbalyst** https://github.com/stravu/crystal (deprecated 2026-02) → https://github.com/nimbalyst/nimbalyst — TS/MIT, ★1.5k, pushed 2026-08-17: desktop parallel worktree sessions + kanban + visual editing.
- **kimaki** https://github.com/remorses/kimaki — TS/MIT, ★1.3k, pushed 2026-08-09: OpenCode in Discord (channel=project, thread=session), worktrees + `/merge-worktree`, message queueing/interrupt. BORROW: chat-thread-as-session mapping for the human-decision queue.
- **forestui** (Rust TUI worktree+Claude manager, ★23), **agent-console** (Rust TUI discovering Codex/Claude sessions from transcripts, ★15), **amux** (Go TUI, ★147) — small.
- **Ouijit** (TS/AGPL, ★153): kanban + terminals wired by lifecycle hooks; per-task worktrees; optional VM sandbox.
- **swarm-protocol** (TS/MIT, ★53, stale 2026-03): MCP-only coordination — claim work, file-conflict detection, heartbeat, handoff. Concept overlaps our lease/heartbeat design.
- **Not found / not verifiable as coding-agent orchestrators**: "trellis" (exists only as a memory/workflow framework, mindfold-ai/Trellis), "waddle", "shipyard", "swarm-cli", "codeflow", "orchestra", "loom", "wsp" — GitHub searches returned no relevant repos (unverified/nonexistent as named). "claude-code-worktree" (kbwo) 404.

#### Claude Code native worktree features (https://code.claude.com/docs/en/worktrees, checked 2026-08-17)
`claude --worktree <name>` / `-w` → `.claude/worktrees/<name>` on branch `worktree-<name>` (from `origin/HEAD` by default; `worktree.baseRef: "head"`), `--worktree "#1234"` branches from a PR, `.worktreeinclude` copies gitignored files, `WorktreeCreate`/`WorktreeRemove` hooks replace git logic, `git worktree lock` while running, periodic sweep honoring `cleanupPeriodDays`, subagent frontmatter `isolation: worktree`, `EnterWorktree`/`ExitWorktree` tools, and hard enforcement blocking edits/commands/git redirects into the main checkout. Headless `-p` runs skip trust checks and do not auto-clean.

---

## D. Influential platforms (non-Rust, for design borrowing)

### Cursor background agents / parallel agents / Bugbot
- **URL**: https://cursor.com/blog/agent-best-practices ; https://cursor.com/docs
- **Maturity/date**: Cursor auto-manages a git worktree per parallel agent (up to 10 workers/user, 50/team per third-party writeups); Background Agents run in sandboxed cloud env and deliver a PR; Bugbot is a paid PR reviewer ($40/user/mo), June-2026 update: 3x faster, 22% cheaper (per https://www.digitalapplied.com/blog/cursor-bugbot-90-second-reviews-june-2026-release). Figures are from secondary sources — treat as approximate. (checked 2026-08-17)
- **Borrow**: separate "implementer" and "reviewer" agents; PR as the unit of landing; per-team concurrency caps.

### Devin / Cognition (MultiDevin, Devin Review, Outposts, Security Swarm)
- **URL**: https://docs.devin.ai/release-notes/2026
- **What**: MultiDevin = manager Devin fanning out to parallel worker Devins; Devin Review (Jan 2026) reviews any GitHub PR; Devin Outposts (Jul 2026) runs workloads in your own environment; Devin Desktop (Jun 2026, ex-Windsurf) as an "agent command center". Sources are press/blogs; official release-notes URL above. (checked 2026-08-17)
- **Borrow**: manager/worker split with a *reviewer as separate agent*; "outpost" = self-hosted worker pool concept.

### Factory Droids — `droid exec` + `--worktree`
- **URL**: https://docs.factory.ai/droid-exec/overview ; https://docs.factory.ai/droid-cli/cli-reference
- **What**: `droid exec` is a one-shot headless runner (mutations opt-in); `--worktree <name>` creates a sibling worktree + dedicated branch; on exit clean worktrees auto-removed, dirty ones preserved (branch never deleted). (checked 2026-08-17)
- **Borrow**: exact worktree lifecycle policy (auto-remove clean, preserve dirty, never delete branch); per-job model flag.

### GitHub Copilot coding agent / Agent HQ / Mission Control / Copilot app
- **URL**: https://github.blog/news-insights/company-news/welcome-home-agents/ (Agent HQ, Oct 2025); Copilot desktop app GA 2026-06-17, opened to all plans 2026-07-07 (per Help Net Security / ecorpit writeups); each session in its own worktree; agents from Anthropic/OpenAI/Google/Cognition/xAI selectable. (checked 2026-08-17)
- **Borrow**: "mission control" single view for direct/monitor/steer/review; issue → agent session → PR lifecycle.

### Google Jules — Jules Tools CLI + API
- **URL**: https://jules.google/docs/cli/reference/ ; https://developers.google.com/jules/api ; https://blog.google/technology/google-labs/jules-tools-jules-api/
- **What**: Async cloud agent; `jules remote new --parallel <n>` starts N sessions on the same task; public API for CI integration. (checked 2026-08-17)
- **Borrow**: `--parallel N` best-of-N sampling for a task as a first-class scheduler option.

### OpenHands (+ Software Agent SDK / agent-server / ACP)
- **URL**: https://github.com/OpenHands/OpenHands (~84.3k stars, MIT, pushed 2026-08-18); SDK https://github.com/OpenHands/agent-sdk ; docs https://docs.openhands.dev/sdk ; paper arXiv:2511.03690; ACP support blog 2026-06-18 https://www.openhands.dev/blog/use-any-coding-agent-in-openhands-with-acp (checked 2026-08-17)
- **What**: Sandboxed agent runtime with a REST/WebSocket **agent-server**, multi-LLM routing, security analyzer, and (2026) Agent Client Protocol so Claude Code / Codex / Gemini CLI can be driven as workers.
- **Borrow**: agent-server as a remote worker protocol; ACP adoption trend (see ACP entry).

### SWE-agent, Aider, Sourcegraph Amp, Kilo, opencode
- SWE-agent https://github.com/SWE-agent/SWE-agent — Python/MIT, ~20k stars, pushed 2026-08-17: benchmark-oriented single-issue agent; borrow its trajectory/log format idea. Aider https://github.com/Aider-AI/aider — Python/Apache-2.0, ~48.3k stars, last push 2026-05-22 (slowing); scriptable `--message`, auto-commits per edit (borrow: commit-per-step attribution). Amp https://ampcode.com/manual — CLI + threads (shared, persistent); Kilo https://github.com/Kilo-Org/kilocode — TS/MIT ~26.9k; opencode https://github.com/sst/opencode — TS/MIT ~198.5k stars, pushed 2026-08-18. All are *workers* we may drive, not orchestrators. (checked 2026-08-17; second subagent covers headless modes in more depth where available.)

### spec-kit, superpowers, gstack, ECC, awesome lists
- spec-kit https://github.com/github/spec-kit — Python/MIT, ~129.9k stars, pushed 2026-08-17: spec-driven development scaffolding (`/specify`, `/plan`, `/tasks`). Borrow: task-file format that agents can consume.
- superpowers https://github.com/obra/superpowers — Shell/MIT, ~273k stars: skills + mandatory workflows (brainstorm → plan → TDD → review). Borrow: enforced-workflow phrasing for worker prompts.
- gstack https://github.com/garrytan/gstack — TS/MIT, ~128k stars: 23 opinionated tools/personas for Claude Code.
- ECC https://github.com/affaan-m/ECC — JS/MIT, ~240k stars: cross-harness operator system (skills, hooks, security).
- Awesome lists: https://github.com/Picrew/awesome-agent-harness (~1.6k), https://github.com/ai-boost/awesome-harness-engineering, https://github.com/VoltAgent/awesome-ai-agent-papers. (all checked 2026-08-17 via GitHub API)

### Anthropic engineering posts on harnesses
- "Effective harnesses for long-running agents" https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents (2025) — initializer agent + incremental coding agent leaving artifacts (feature list, progress file, clean git state) for the next context window.
- "Harness design for long-running application development" (2026-03-24) and Code with Claude 2026 (2026-05-08): Managed-agents multi-agent orchestration (lead + parallel specialists on shared FS, auditable in Console), "Outcomes" (separate grading agent scores outputs against rubrics), "Dreaming" (between-session memory curation). Sources: https://www.anthropic.com/engineering ; https://www.mindstudio.ai/blog/code-with-claude-2026-new-agent-features (checked 2026-08-17)
- **Borrow**: separate grader/reviewer agent with a rubric = our verify/review step; artifact-per-session handoff = our worker "progress note" on the bead.

### 2026 papers on parallel coding agents / merge conflicts (checked 2026-08-17)
- Co-Coder — "When Parallelism Pays Off: Cohesion-Aware Task Partitioning for Multi-Agent Coding" https://arxiv.org/abs/2606.00953 (+14% pass, 2.1x speedup vs file-based partition). Borrow: partition tasks by code cohesion, not by file list.
- ATM — "CID-Brokered Pre-Write Admission for Multi-Agent Code Co-Synthesis" https://arxiv.org/pdf/2607.00041 — broker admits writes before they happen (pre-write locks). Borrow: optional path-lease broker.
- AgenticFlict — dataset of merge conflicts in AI-agent PRs https://arxiv.org/abs/2604.03551 ; Rover — LLM context-aware conflict resolution https://arxiv.org/abs/2605.17279 ; Semantic Consensus https://arxiv.org/abs/2604.16339 ; Verified Multi-Agent Orchestration (plan-execute-verify) https://arxiv.org/pdf/2603.11445 ; Nexa parallel-to-sequential https://arxiv.org/abs/2605.15573.
- OpenHands SDK paper https://arxiv.org/abs/2511.03690.


---

## E. Rust LLM/agent frameworks (all checked 2026-08-17 via crates.io + GitHub API)

None of these orchestrate *external agent processes*; they call LLM APIs in-process. Relevant to us only if ai_runner itself needs to call a model (e.g. triage, review-grading, conflict resolution).

| Crate / repo | Version (date) | Stars / last push | License | Notes / borrow |
|---|---|---|---|---|
| **rig** `rig-core` https://github.com/0xPlaygrounds/rig | 0.42.0 (2026-08-17) | 8.3k / 2026-08-18 | MIT | Most active general Rust LLM framework: providers, agents, tools, vector stores, pipelines. **Use** if we need in-process LLM calls (review-grader). |
| **genai** https://github.com/jeremychone/rust-genai | 0.6.5 (2026-08-04) | 853 / 2026-08-18 | Apache-2.0 | Thin multi-provider client (OpenAI, Anthropic, Gemini, Ollama, Bedrock...). Simpler alternative to rig for one-off calls. |
| **async-openai** https://github.com/64bit/async-openai | 0.41.3 (2026-07-31) | 2.0k / 2026-07-31 | MIT | OpenAI API only. |
| **AutoAgents** https://github.com/liquidos-ai/AutoAgents | 0.4.0 (2026-07-08) | 734 / 2026-08-14 | Apache-2.0 | Multi-agent framework (ReAct, actor-ish); in-process only. |
| **swarms-rs** https://github.com/The-Swarm-Corporation/swarms-rs | 0.2.1 (2025-09-08) | 175 / 2025-12-15 | Apache-2.0 | Stale (~8 months). Skip. |
| **graph-flow** https://github.com/a-agmon/rs-graph-llm | 0.6.0 (2026-07-19) | 361 / 2026-07-19 | MIT | Typed graph workflows with Postgres/in-memory session storage; borrow: graph-of-steps + persisted session pattern. |
| **kowalski** https://github.com/yarenty/kowalski | 1.3.0 (2026-06-15) | 63 / 2026-08-14 | MIT | Ollama-centric agent lib. Skip. |
| **agentai** https://github.com/AdamStrojek/rust-agentai | 0.1.5 (2025-07-20) | 168 / 2025-09-22 | MIT | Stale. Skip. |
| **anda** (`anda_core`) https://github.com/ldclabs/anda | 0.15.0 (2026-08-07) | 439 / 2026-08-07 | Apache-2.0 | ICP/TEE-oriented. Skip. |
| **llm-chain** https://github.com/sobelio/llm-chain | 0.13.0 (2023-11) | 1.6k / 2024-10-31 | MIT | Dead. Skip. |
| **adk-rust** https://github.com/zavora-ai/adk-rust | 2.0.0 (2026-08-10) | 610 / 2026-08-18 | NOASSERTION (check) | *Community* "Rust Agent Development Kit" — **not** an official Google ADK (google/adk-rust returns 404; unverified whether Google endorses it). |
| **kalosm** https://github.com/floneum/floneum | 0.4.0 (2025-02-09) | 2.2k / 2026-08-16 | Apache-2.0 | Local models (candle). Not relevant. |
| **agent-sdk** (bipa-app) https://github.com/bipa-app/agent-sdk | 0.18.0 (2026-08-04) | 5 / 2026-08-08 | MIT | Tiny; skip. |
| **cc-sdk** https://github.com/ZhangHanDong/claude-code-api-rs | 0.8.1 (2026-04-03) | 173 / 2026-04-03 | NOASSERTION | Unofficial Rust wrapper for the Claude Code CLI (spawns `claude`, parses stream-json). Reference for our Claude Code adapter; do not depend on it. |
| Official Anthropic Claude Agent SDK for Rust | — | — | — | **Not found** (anthropics/claude-agent-sdk-rust 404; crates `claude-agent-sdk` 0.1.1 is third-party, 2025-09). Unverified/none as of 2026-08-17. |


---

## F. Rust workflow / durable-execution engines & job queues (checked 2026-08-17)

| Project | Version / maturity | License | Fit |
|---|---|---|---|
| **Restate** https://github.com/restatedev/restate (server) + `restate-sdk` 0.11.1 (2026-08-14) https://github.com/restatedev/sdk-rust | server v1.7.3 (2026-08-07), 4.3k stars | BSL-1.1 (server; NOASSERTION on GH), MIT (SDK) | Durable execution + virtual objects + awakeables; excellent fit for "claim → work → verify → land" as a durable workflow with human awakeables — but requires running the Restate server (Rust binary, single-node OK). Consider for v2 if single-binary constraint relaxes. |
| **Temporal Rust SDK** `temporalio-sdk` 0.7.0 (2026-08-17) https://github.com/temporalio/sdk-rust | Public Preview (prerelease API), 506 stars | MIT | Needs Temporal server; heavy for local-first. Borrow concepts only (activities w/ heartbeats, workflow histories). https://temporal.io/changelog/rust-sdk-public-preview |
| **Windmill** https://github.com/windmill-labs/windmill | v1.791.0 (2026-08-17), 17.6k stars | AGPL-3.0 (+ EE) | Full platform (Postgres). Too heavy; AGPL. Borrow job-run UI ideas. |
| **apalis** https://github.com/apalis-dev/apalis | 0.7.4 (2026-05-06), 1.4k stars | MIT | Tower-based background jobs; SQLite/Postgres/Redis backends; retries, cron. Plausible for internal job scheduling but its model is short jobs, not hour-long supervised subprocesses with leases. |
| **fang** https://github.com/ayrat555/fang | 0.11.0 (2026-07-02), 719 stars | MIT | Postgres/SQLite job queue. Similar caveats. |
| **underway** https://github.com/maxcountryman/underway | 0.2.0 (2025-07-16), 171 stars | Apache-2.0 | Durable step functions on Postgres. Postgres-only. |
| **sqlxmq** | 0.6.0 (2025-05-25) | Apache-2.0 | Postgres-only. |
| **obelisk** https://github.com/obeli-sk/obelisk | crate 0.5.0 (2024-10), repo pushed 2026-08-17, 740 stars | AGPL-3.0 | Deterministic WASM workflow engine; AGPL and WASM-only activities — poor fit. |
| **Rivet** actors https://github.com/rivet-dev/rivet | 6.1k stars, pushed 2026-08-13 | Apache-2.0 | Stateful actors platform; overkill locally. |
| **ractor** https://github.com/slawlor/ractor 0.16.5 / **kameo** https://github.com/tqwewe/kameo 0.22.2 | active | MIT / Apache-2.0 | Actor frameworks — usable as the in-process supervision model (one actor per worker, supervisor restarts). kameo has supervision trees; ractor is Erlang-style with factories. Optional; plain tokio tasks + channels may suffice. |
| **tokio-cron-scheduler** 0.15.1 | 2025-10 | MIT/Apache | For periodic reconcile ticks (or just `tokio::time::interval`). |
| **cadence** Rust client | — | — | **Unverified/none**: no maintained Rust Cadence client found on crates.io as of 2026-08-17. |
| **DBOS Rust** | — | — | **Unverified/none**: dbos-inc/dbos-transact-rs 404. |


---

## G. Process supervision / PTY / terminal control (checked 2026-08-17)

| Crate | Version | Notes |
|---|---|---|
| `tokio::process` (tokio 1.53.1) | — | Baseline: async spawn, piped stdio, `kill_on_drop`. Use with `process-wrap` for process-group / job-object semantics. |
| **process-wrap** https://github.com/watchexec/process-wrap 9.1.0 (2026-03-08) | — | Wraps std/tokio Command with process groups, sessions, Windows Job Objects, kill-on-drop; successor of `command-group` (5.0.1, 2023, maintenance). **Use** for killing whole agent trees on lease loss. |
| **nix** 0.31.3 | — | `killpg`, `setsid`, signals. |
| **sysinfo** 0.39.6 | — | Process tree / CPU / RSS sampling for per-worker resource metrics. |
| **portable-pty** 0.9.0 (wezterm) https://github.com/wezterm/wezterm | 28.4k stars (wezterm) | Cross-platform PTY; needed if we drive interactive TUIs (Claude Code interactive) rather than `-p` headless. |
| **pty-process** 0.5.3 / **expectrl** 0.9.0 (2026-05-11) | — | Unix PTY + expect-style automation (expectrl is more active). |
| **tmux_interface** 0.4.0 (2026-03-10) | — | Drive tmux for human-attachable sessions (Claude Squad approach). Alternative: herdr panes (above). |
| **pueue-lib** 0.31.1 | — | See pueue entry. |
| **interprocess** 2.4.3 | — | Local sockets (Unix domain / named pipes) for daemon↔CLI. |
| **fs4** 1.1.0 / **** 4.0.4 | — | File locks for worktree/lease files. |
| **notify** 8.2.0 | — | Watch beads JSONL / worktree changes. |

---

## H. Candidates to adopt outright vs build — ranked recommendation

**Bottom line**: nothing in the landscape is a Rust, beads-native, policy-enforcing fleet runtime with leases/heartbeats, a human-decision queue and cost ledger. The pieces do exist as (a) a task/issue layer we already chose (beads), (b) a de-facto *spec* for the tick loop and config (Symphony), (c) a stable Rust *protocol crate* for talking to agent processes (ACP), and (d) Apache-2.0 Rust code for executors/worktrees (Vibe Kanban) — so we build the runtime and adopt those four.

| Rank | Decision | What | Why |
|---|---|---|---|
| 1 | **ADOPT** | **beads** (`bd`, Go, MIT; v1.2.2 2026-08-15; Dolt-backed with JSONL export; `bd ready`, `bd update <id> --claim` atomic claim, hash IDs, JSON output) — shell out to `bd --json`; do **not** depend on beads_rust (`br`, 1.05k stars, license NOASSERTION, unverified compatibility with the Dolt-era schema) except as a read-only fallback. | Already the coordination layer; Gas Town/Overstory prove the "bead = unit of work" model; JSONL export gives us a stable interchange format. |
| 2 | **ADOPT (as spec)** | **OpenAI Symphony SPEC.md** vocabulary: `WORKFLOW.md` (YAML front matter + prompt body), tick = reconcile → validate → fetch candidates → sort → dispatch ≤ limits → observe; states `Unclaimed/Claimed/Running/RetryQueued/Released`; run states `PreparingWorkspace → … → Succeeded|Failed|TimedOut|Stalled|CanceledByReconciliation`; `stall_timeout_ms`, `turn_timeout_ms`, `max_concurrent_agents(_by_state)`, backoff `min(10s·2^(n-1), cap)`; `GET /api/v1/state`. | Language-agnostic, already has Rust/Go ports to compare against; gives us a defensible design and interoperability story. We add what it lacks: worktrees, leases that survive restart, merge queue, cost ledger, human queue. |
| 3 | **ADOPT (crate)** | **`agent-client-protocol` 2.0.0** (Apache-2.0) as the primary worker interface; plus native adapters for **Claude Code** (`claude -p --output-format stream-json --input-format stream-json --permission-prompt-tool=stdio`, `--resume`, `--worktree`, hooks) and **Codex** (`codex app-server` JSON-RPC via `codex-app-server-protocol` crate) where we need usage/cost fields ACP does not carry. | One wire format covers goose/OpenCode/Kilo/Gemini/Copilot/Droid natively; permission requests become structured items in our human-decision queue; `session/load` = resume after lease re-acquire. |
| 4 | **FORK/BORROW code** | **Vibe Kanban** crates `executors`, `worktree-manager`, `workspace-manager`, `git-host` (Rust, Apache-2.0, last release 2026-04-24, company defunct). | Battle-tested Rust for launching 9 agent CLIs headlessly, normalizing their logs, and managing worktrees; forking is fine because upstream is community-maintained and path-dep only. |
| 5 | **BORROW design** | Gas Town **Refinery** (Bors-style batched/bisecting merge queue with verification gates) + Overstory **4-tier conflict ladder** (clean → same-file-non-overlap heuristic → LLM-assisted → human) + Factory's worktree cleanup policy (auto-remove clean, preserve dirty, never delete branch) + Paperclip **budget hard-stops** + Anthropic "separate grader agent with rubric". | These are the parts every serious 2026 orchestrator converged on; none ship as a Rust library. |
| 6 | **OPTIONAL substrate** | **herdr** (Rust, Apache-2.0, 30k★) panes for human-attachable, reconnectable worker sessions; or **pueue-lib** for a quick MVP job daemon. | Saves writing PTY/tmux plumbing; keep behind a `SessionBackend` trait so headless (no PTY) stays the default. |
| — | **DO NOT ADOPT** | Gas Town wholesale (Go, tmux, Claude-as-Mayor, Dolt/ICU deps); Restate/Temporal/Windmill (external server, wrong granularity for hour-long supervised subprocesses; Restate is a plausible v2 if we go multi-host); claude-flow/ruflo (unverifiable claims); AGPL tools (Claude Squad, kandev, coder/mux, CloudCLI) for code reuse; in-process LLM frameworks (rig etc.) as an orchestration base. | |

**Build ourselves (the core)**: leases + heartbeats persisted in SQLite (survive restart — Symphony explicitly does not), enforced step machine (claim → worktree → work → verify → review → land → close) with per-step policies, human-decision queue (ACP permission requests + escalations + merge conflicts) exposed via CLI/TUI/MCP, cost ledger per bead/run/model from stream-json usage + `opencode stats`/`amp usage`, event-sourced run log (NDJSON + SQLite), OTLP export.

## I. Rust crate shortlist for building (versions from crates.io, checked 2026-08-17)

| Area | Crate | Version (date) | Notes |
|---|---|---|---|
| CLI | `clap` https://crates.io/crates/clap | 4.6.6 (2026-08-06) | derive API; add `clap_complete`. |
| CLI UX | `indicatif` 0.18.6, `console` 0.16.4, `cliclack` 0.5.6, `inquire` 0.9.4, `comfy-table` 8.0.0 / `tabled` 0.21.0 | 2026 | prompts, tables, progress. |
| TUI | `ratatui` https://ratatui.rs | 0.30.2 (2026-06-19) | + `crossterm` 0.29.0; `tui-logger` 0.18.3 for log pane. |
| Async | `tokio` | 1.53.1 (2026-07-20) | + `tokio-util` 0.7.19 (cancellation tokens). |
| SQLite | `rusqlite` https://crates.io/crates/rusqlite | 0.40.2 (2026-08-08) | bundled feature; WAL; sync API is fine behind a task. Alt: `sqlx` 0.9.0 (2026-05-21) if we want async + compile-time-checked queries (Vibe Kanban used sqlx). |
| Embedded KV (optional) | `redb` 4.2.0 (2026-08-17), `fjall` 3.1.9 | — | Only if we want an append-only event store outside SQLite; SQLite is enough. |
| Git | **`gix`** (gitoxide) https://github.com/GitoxideLabs/gitoxide 0.86.0 (2026-07-23), pure Rust, Apache/MIT, 11.8k★ **vs** `git2` 0.21.0 (2026-05-18, libgit2 bindings, 2.1k★). | — | Recommendation: **shell out to `git` for worktree/merge/rebase** (worktree API in gix is still incomplete — verify against gix docs before relying on it; git2 lacks `git worktree` ergonomics too), use `gix` read-only for status/diff/log/refs. Both Vibe Kanban and Claude Code shell out for worktrees. |
| Process supervision | `tokio::process` + `process-wrap` 9.1.0 (2026-03-08) | — | process groups/sessions, kill-on-drop of the whole tree; `nix` 0.31.3 for `killpg`; `sysinfo` 0.39.6 for RSS/CPU per worker. |
| PTY (only if driving interactive TUIs) | `portable-pty` 0.9.0 (wezterm), `expectrl` 0.9.0 (2026-05-11) | — | Prefer headless JSON modes; PTY as fallback. `tmux_interface` 0.4.0 if we adopt tmux sessions. |
| Agent protocol | `agent-client-protocol` https://crates.io/crates/agent-client-protocol | 2.0.0 (2026-07-23) | Apache-2.0; `agent-client-protocol-schema` 1.6.0. |
| Codex protocol | `codex-app-server-protocol` (openai/codex workspace, Apache-2.0) | pinned by tag (Vibe Kanban pinned rust-v0.124.0) | git dependency; verify current tag. |
| MCP server/client | **`rmcp`** (official modelcontextprotocol/rust-sdk) https://github.com/modelcontextprotocol/rust-sdk | 3.1.3 (2026-08-17) | expose `ai_runner` tools (claim, heartbeat, ask_human, report_cost) to workers via `mcpServers` injection (ACP) or `--mcp-config` (Claude Code). goose uses rmcp 2.x. |
| JSON / schema | `serde_json` 1.0.151, `schemars` 1.2.2 (2026-07-27), `jsonschema` 0.49.9 (2026-08-09) | — | schemars to publish JSON Schema for WORKFLOW/config/events; jsonschema to validate agent-emitted artifacts. |
| Config | `figment` 0.10.19 (2024-05-17, stable but slow-moving) or `config` 0.15.25 (2026-06-26); `toml` 1.1.4; front-matter parse via `serde_yaml` (deprecated 0.9.34) → prefer `serde_yml`/`saphyr` or keep YAML front matter tiny and parse with `yaml-rust2`. | — | Layered: defaults → repo `WORKFLOW.md` front matter → user TOML → env → CLI. |
| Tracing / OTel | `tracing` 0.1.44, `tracing-subscriber` 0.3.23, `opentelemetry` 0.32.0, `opentelemetry-otlp` 0.32.0, `tracing-opentelemetry` 0.33.0 (2026-05) | — | span per bead/run/step; OTLP export optional. |
| IDs / time | `ulid` 3.0.0 / `uuid` 1.24.1; `jiff` 0.2.35 (or `chrono` 0.4.45) | — | ULIDs sort by time — good for event logs. |
| Errors | `thiserror` 2.0.20, `anyhow` 1.0.104 (or `miette` 7.6.0 for pretty CLI diagnostics) | — | |
| Locks / IPC | `` 4.0.4 / `fs4` 1.1.0; `interprocess` 2.4.3 | — | worktree/lease lock files; daemon ↔ CLI over local socket. |
| HTTP API (optional) | `axum` 0.8.9 | — | Symphony-style `/api/v1/state`. |
| Actors (optional) | `kameo` 0.22.2 or `ractor` 0.16.5 | — | supervision trees per worker; plain tokio tasks + `CancellationToken` likely sufficient. |
| Scheduling | `tokio-cron-scheduler` 0.15.1 or `tokio::time::interval` | — | reconcile tick. |
| LLM calls (only for triage/grader/conflict-resolver) | `rig-core` 0.42.0 or `genai` 0.6.5 | — | keep optional feature-flag. |
| Event sourcing | none mature and small — build: append-only `events` table (ULID, run_id, bead_id, kind, json) + NDJSON mirror (Gas Town `.events.jsonl` / Overstory NDJSON) | — | |

---

## J. One-page synthesis: what to USE vs BORROW

| Need | Use directly | Borrow design from |
|---|---|---|
| Task/issue layer | beads (`bd ready --claim --json`, gates `human`/`gh:pr`/`gh:run`, formulas/molecules, JSONL export) | Paperclip heartbeat/budget checkout; Symphony tracker-adapter fields; beads lease schema (`lease_expires_at`, `heartbeat_at`, `granted_node`, `lease_reclaimed`) which is on `main` only |
| Lifecycle state machine & config | — | Symphony SPEC states + `WORKFLOW.md` keys; sortie; harness (Rust) crate layout |
| Agent process protocol | `agent-client-protocol` 2.0 crate (+ `@agentclientprotocol/claude-agent-acp`, Codex adapter); native Claude `stream-json` + `--permission-prompt-tool` + HTTP hooks; Codex `app-server` JSON-RPC | Vibe Kanban `executors` (Apache-2.0), Overstory `AgentRuntime` contract, ralph-orchestrator backends |
| Worktrees | `git worktree` + Claude Code semantics (lock, sweep, `.worktreeinclude`, `WorktreeCreate/Remove` hooks) | Vibe Kanban `worktree-manager`, Zed `create_worktree` hook, Factory cleanup policy, uzi/Emdash port ranges |
| Merge/landing | — | Gas Town Refinery (Bors-style batch + bisect + gates), Overstory 4-tier conflict ladder + merge lock + dry-run, multiclaude "CI is the only gate", Bernstein owned-files partitioning, Co-Coder cohesion partitioning |
| Human decisions | ACP `session/request_permission`; Claude `--permission-prompt-tool`; Codex `requestApproval`; beads `human` gates | Gas Town escalation severities + ack, ralph-orchestrator Telegram blocking Q&A, kimaki thread mapping, kandev gates, AO "Needs You" column |
| Watchdogs/leases | — | Gas Town Witness/Deacon + GUPP violation, Overstory tiered watchdog, Symphony stall/turn timeouts + backoff, swarm-protocol heartbeats, ccmanager state detection |
| Cost | Claude `total_cost_usd`/`modelUsage` (estimates), Codex `usage` per turn, `opencode stats` / `kilo stats` / `amp usage` | Overstory per-bead cost from JSONL, Paperclip budget hard-stops, coder/mux dashboard, Codex `thread/goal` token budgets |
| Sessions (attachable) | herdr panes / tmux (`tmux_interface`) / pueue-lib | cmux attention badges, Claude agent-view (`claude agents --json`) |
| Loop pattern | Anthropic ralph-loop plugin state file | Huntley's contract; ralph-orchestrator gates |
| Observability | `tracing` + OTLP | Gas Town metric names / health states; Symphony `/api/v1/state`; harness `harness-observe` |
