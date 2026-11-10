# A live, read-only browser view of the fleet

Research, 2026-11-09, for the coordinator. Main is at 7b88456 (`git log -1` in
`the main checkout`); the installed binary is `air 0.4.16 (built from 06cc8046,
surface 46)` (`air --version`). Every path below is relative to the coordinator worktree
`the coordinator worktree` unless it starts with `/` or
`~`; every URL was read on 2026-11-09. Nothing was written under the repository; the only `air`
commands run were `air --help`, `air status --json` and `air audit`.

The question is: can the owner always see what each session is doing, which beads are where, what
the lane is verifying, which batches went red, who holds a lease and which conditions are open,
with as much detail as the sources allow, without adding a hook, a write, or any load to the agents?

The short answer is: the data already exists in five places that nothing has to be told to write
(the ledger, the event stream, Claude Code's own per-process session files, the transcripts and the
tmux panes), and the smallest honest build is one GET-only loopback server inside `air`, with no new
crate, serving one HTML file that polls. What does not exist anywhere is the fine-grained "what is
this agent doing right now"; that is in the transcript tail, and reading it costs nothing.

## 1. Does it already exist?

### 1a. beads

`bd serve` (bd 1.3.0, `bd --version`) is an HTTP API over loopback, not a page. Its help says it
serves "the same work surface the CLI answers, for automation clients that would otherwise fork a bd
subprocess per call", that "the wire contract is described by an OpenAPI document (/v0)", and it
names `GET /healthz` (liveness, never touches the database), `GET /v0/beads/ready?limit=1`
(readiness), `GET /v0/beads/context` (repo root, beads directory, database name) and
`POST /v0/beads/issues:sweep` (bulk delete of closed beads) (`bd serve --help`, run here). The
default bind is `127.0.0.1:0`, an ephemeral port printed on stdout, and it wants "a supervisor". Two
facts rule it out for a read-only page: "It does not run under --readonly, and refuses to start
rather than binding", and "Every server it binds publishes the issue-claim operation", so a UI that
started it would open a write surface it never uses (same help text). A page could poll it, since
it answers JSON over loopback with no token by default, but the UI does not need to: one `bd list
--json` process costs 118 ms median here (`air status --json`, `bd_latency.median_ms`; `air audit`
says "bd median 115 ms per process over 3768 process(es)"). The design doc already carries the open
question "Whether `bd serve` is worth a daemon" (`docs/design.md:695`); this research does not
answer it, because the UI does not depend on it.

beads ships no UI of its own (`bd --help` lists no `ui`, `web` or `dashboard` command; the only
network command is `serve`). Its community-tools page lists eight web UIs and six terminal UIs
(`gh api repos/steveyegge/beads/contents/docs/community-tools.md`, 2026-11-09). Three of them are
relevant precedents: `beady-eye`, "a live terminal viewer that follows agents as they work through a
tree of beads ... Read-only" in Rust; `btui`, which "reads through the installed `bd --readonly`
CLI" and uses "event-triggered refresh" on bd 1.3.0 "retaining polling fallback"; and `beads-web`,
which reads "server-mode workspaces ... directly from Dolt over SQL". All of them show beads and
nothing of Air's state (claims, greens, batches, leases, conditions), so none replaces what is asked
for, but the bead board half of a page has a dozen reference implementations and no novelty.

### 1b. Gas Town

Gas Town has both a web dashboard and a TUI. `gt dashboard` runs on port 8080 by default and is "a
single-page overview of everything happening in your workspace" covering agents, convoys, hooks,
queues, issues and escalations; `gt feed` "launches an interactive terminal dashboard for monitoring
all agent activity in real-time" with an Agent Tree, a Convoy Panel and an Event Stream, and a
Problems view that groups agents into "GUPP Violation, Stalled, Zombie, Working, and Idle" from
"structured beads data" (https://github.com/gastownhall/gastown README). The dashboard source
confirms how it reads: "Auto-refresh every 30 seconds via htmx", a port flag defaulting to 8080, and
it sets `BEADS_DOLT_PORT` so "bd/gt subprocesses connect" to the right Dolt server, so its data comes
from `bd` subprocesses and its own daemon
(`gh api repos/gastownhall/gastown/contents/internal/cmd/dashboard.go` lines 37, 48, 74-76); the
page also opens an `EventSource('/api/events')` stream (`internal/web/static/dashboard.js:33`). The
shape to take from it is "one page, poll every 30 s, read bd as a subprocess". The part not to take
is the daemon and the scale it is built for ("Scale comfortably to 20-30 agents", README); Air's
design row on Gas Town stands (`docs/design.md:708`).

### 1c. Claude Code

Claude Code has three things near this, none of which shows Air's facts.

The closest is agent view, `claude agents` (https://code.claude.com/docs/en/agent-view). It is
"one screen for all your background sessions: what's running, what needs your input, and what's
done", and `claude agents --json` "is the supported way to read session state from outside Claude
Code". Run here, it listed every launched session with `pid`, `cwd`, `kind: interactive`,
`startedAt`, `sessionId`, `name` (`ai_runner-worker-1` and so on) and `status: idle`
(`claude agents --json`, 2.1.311). The documented fields are `cwd`, `kind`, `startedAt` always;
`pid` and `status` (`busy`, `waiting`, `idle`) while the process is alive; `waitingFor`
(`permission prompt`, `input needed`, `sandbox request`, `worker request`, `dialog open`) when
waiting; `state` (`working`, `blocked`, `done`, `failed`, `stopped`) for background sessions. The
TUI's per-row text is "generated by a Haiku-class model" and "updates at most once every 15 seconds
from the session's own recent output", so the TUI itself spends model calls on summaries; the JSON
does not. The page says the files under `~/.claude/jobs/<id>/` "are not a stable interface". It
knows nothing of beads, batches or leases.

Behind that JSON is a file per process, `~/.claude/sessions/<pid>.json`, about 600 bytes, that
Claude Code itself keeps current: `pid`, `sessionId`, `cwd`, `startedAt`, `procStart`, `version`,
`kind`, `tmux` (the exact pane, `"ai_runner-worker-1:@16.%16"`), `messagingSocketPath`, `name`,
`status` (`idle` for worker-1, `busy` for the coordinator at the time), `updatedAt` and
`statusUpdatedAt` (`cat ~/.claude/sessions/64687.json`, `cat ~/.claude/sessions/46163.json`).
Lookout, a VS Code sidebar in the orchestrators list, reads exactly this: "`<config>/sessions/<pid>.json`:
one file per Claude Code process", and the `waitingFor` field in it for "permission prompt",
"input needed" and "dialog open" (https://github.com/nimrod-teich/lookout/blob/main/docs/how-it-works.md).
This is not documented as stable either, but it is the same data the supported `--json` prints,
readable without starting a `claude` process.

The other two are not observation. Agent teams (experimental,
`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`) show teammates "in the agent panel below the prompt
input" or in tmux split panes, for sessions one lead spawned; Air's sessions are independent and
not a team (https://code.claude.com/docs/en/agent-teams). Cross-session messaging gives
`/list-agents`, which lists "your other local sessions ... A session appears only when it binds an
inbox socket", for messaging, not watching (https://code.claude.com/docs/en/cross-session-messaging).

OpenTelemetry is a real fourth source but it costs the sessions something. With
`CLAUDE_CODE_ENABLE_TELEMETRY=1` and `OTEL_LOGS_EXPORTER=otlp` each session exports
`claude_code.tool_result` events carrying `tool_name`, `tool_use_id`, `success`, `duration_ms`,
`tool_input_size_bytes` and `session.id`, and `claude_code.api_request` events with `model`,
`cost_usd`, `input_tokens`, `output_tokens` and cache tokens; the Bash command text appears only
with `OTEL_LOG_TOOL_DETAILS` ("Bash commands, MCP server and tool names ... and tool input"); logs
flush every `OTEL_LOGS_EXPORT_INTERVAL`, default 5000 ms; a `console` exporter exists; and
"Claude Code ignores the OpenTelemetry exporter variables in a repository's `.claude/settings.json`",
so Air's launcher would have to put them in each session's environment
(https://code.claude.com/docs/en/monitoring-usage). That is an exporter thread and a network
client inside every agent process plus an OTLP receiver for Air to write, to learn what the
transcript on disk already says. It is the fallback if the transcript format ever breaks, not the
first choice.

The status line is per session and receives `session_id`, `transcript_path`, `model.id`,
`cost.total_cost_usd`, `context_window.used_percentage` and more on stdin, debounced at 300 ms
(https://code.claude.com/docs/en/statusline, "Available data" and "Claude Code debounces updates
at 300ms"). A status-line script that wrote a file would be a new process per update in every
session. Not needed: the transcript's `message.usage` carries the same token counts (see 2c).

### 1d. The orchestrators list and Overstory

The awesome-agent-orchestrators list (https://github.com/andyrewlee/awesome-agent-orchestrators)
describes well over a hundred projects with a live UI for several agent sessions, under "Parallel
Coding Agents (Terminal, Desktop & Web)", "Multi-Agent Swarms" and others. Where the list states a
data source it is tmux (agent-manager, agent-session-manager, repomon, ReevesAgents, dev-3.0,
Muxflow, MulmoTerminal), hooks (Termio, KanVibe, Ouijit, MulmoTerminal), a daemon (pando, Skiff,
Open Walnut, tuios), transcripts (agent-console: "Rust TUI listing Codex and Claude Code sessions,
resuming their native UI. Data: provider transcripts") or "files agents already write to disk, no
hooks" (Lookout). Every one of them is a session supervisor; none reads a merge queue, a verify
ledger or a claims table, which is the half Air has and they do not. The design row that keeps Air
over an orchestrator (`docs/design.md:710`) is unchanged by this.

Overstory (https://github.com/jayminwest/overstory) is not in the list. Its README names `ov
dashboard`, a "Live TUI dashboard for agent monitoring" with `--interval`, and `ov serve`, an "HTTP
+ WebSocket surface for the web UI" at `http://localhost:7321`, "the primary operator surface for
the swarm"; its mail bus and merge queue are SQLite in WAL mode and its session metrics "come from
runtime JSONL transcript files". The repository "is archived and read-only as of May 28, 2026" and
the README says it is no longer maintained. The lesson it leaves is the data path, SQLite plus
transcripts, which is Air's too.

### 1e. Air itself

`air status --json` (run in the main checkout) returns three top-level keys: `air` (`name`,
`version`, `built_from`, `surface_version`), `attention` (the conditions array, empty at the time)
and `snapshot`, with 45 keys: `at`, `workers`, `sessions`, `inbox_depth`, `oldest_capture_at`,
`leases`, `lease_wants`, `ready_depth`, `claimable_depth`, `epic_depth`, `epics_to_decompose`,
`ancestor_deadlocks`, `without_initiative`, `verifies_in_flight`, `landings_in_flight`,
`tree_readers`, `main_checkout_sessions`, `landable`, `land_skipped`, `land_errors`, `batch_ready`,
`not_batch_ready`, `loops`, `red_batch`, `red_split`, `install_lag`, `idle_sleep_min`,
`fleet_stop`, `heartbeat_minutes`, `unknown_config_keys`, `config_overlay`,
`config_overlay_tracked`, `model_setting`, `lane_model_setting`, `base_branch`, `bd_mode`,
`bd_server_up`, `bd_listener`, `pin`, `rewound_carried`, `overlaps`, `errors`, `duration_ms`,
`bd_latency`, `bd_source` (`air status --json | python3 -c ...`, keys listed; the struct is
`crates/cli/src/cmd/status.rs:152-305`). That run took 1259 ms (`duration_ms`); `air audit` puts
`air status` at "p50 1071 ms, p90 1718 ms, p99 2888 ms ... over 29 run(s)", and its event line
shows what it spends: 3 `bd` processes (`bd-status` 106, 87, 92 ms), 49 `git` calls and one process
listing (`tree-readers` 152 ms) (`.air/events/2026-11-09.ndjson`, the `status` line at
21:37:58Z). It is the one screen the design names (`docs/design.md:578`), and it is not read-only:
`gather` writes `bd_cache` (`status.rs:2368-2376`) and the process reading (`status.rs:2461`), the
command writes an event line with the conditions it opened and cleared (`status.rs:2692`), and
opening the ledger at all runs the schema migration (`crates/ledger/src/lib.rs:61-69`,
`schema.rs:473-478`). That matters for §3 and §6.

`air mcp` lists five resources, each of which shells out to the CLI: `air://status` (`--json
status`), `air://attention` (`--json status --attention`), `air://inbox`, `air://holdings`,
`air://leases` (`crates/cli/src/cmd/mcp.rs:344-353`, `486-493`). They are the same data as the
commands, reachable only from inside a Claude Code session, so a browser cannot use them, and they
carry the same writes.

`air audit` is a read-only register: per mechanism "how often it fired, over what, when last, and
its removal condition; per timing budget its p50, p90, p99 and maximum" (`docs/design.md:145`; run
here, it reported 51 mechanisms and the budget table). It is a round-end report, not a live view,
and the page should link to it rather than re-render it.

Nothing in Air's docs mentions a dashboard, browser UI or web page (`rg -i 'dashboard|browser
ui|web ui' docs/design.md docs/solved-issues.md docs/rules/roles.md CHANGELOG.md` returned
nothing). The nearest prior ruling is "No notice filter and no feed pane for the coordinator yet;
its chat keeps every notice" (`docs/design.md:723`, air-ank), which was about the coordinator's
context, not the owner's eyes; a page the owner reads costs the coordinator nothing. The recorded
pain a page removes is written in the code: "the owner found out by reading a tmux pane (capture
01M0KZETBMSHDXNX6PW15HVSJV)" on 2026-08-22 (`status.rs:396-399`), and "Owner, 2026-08-22, after a
status report they refused to read" (CLAUDE.md, "Talking to the owner").

## 2. Air's own data, by source

Row counts and sizes are from `sqlite3 "file:.air/ledger.db?immutable=1"` and `ls` in the main
checkout on 2026-11-09, 17:35 local; the ledger file was 1,187,840 bytes. The schema is
`.air/ledger.db .schema` (same tool), and the same tables are described in `docs/design.md:261-277`.

| Source | Holds | Changes | A page shows |
|---|---|---|---|
| `sessions` (5 rows) | One row per worker: `role`, `state` (`working`, `running`, `idle`), `detail` (the tool name while running), `model`, `pid`, `pid_started`, `changed_at`, `stopped_*` (the sticky fact), `transcript_path` | Every hook call of that session: `PreToolUse` sets running, `PostToolUse` working, `Stop` idle (`hook.rs:1171-1180`; `docs/design.md:472-484`) | The fleet cards: role, state, which tool is running, model, alive or gone, when it last changed, where its transcript is |
| `claims` (79) | `bead`, `worker`, `claimed_at`, `declared_files`, `done_at`, `done_head`, `done_proof`, `handover_attempts`, `released_at`, `release_reason` (`landed`, `closed`, …) | On `air claim`, `air done`, `air release`, `air close`, `air land`, `air reclaim` | In progress (open, no `done_at`), done-not-landed (`done_at` set, `released_at` null), landed (`release_reason = landed`), and per bead who held it and how long |
| `verify_inflight` (0 now) | The check running now: `worker`, `sha`, `kind`, `command`, `pid`, `started_at` | Written at `air record` start, deleted at exit | "Lane verifying 886b5426 for 1m12s", with the pid's liveness |
| `verify_runs` (181) | Every finished check: `sha`, `tree`, `kind`, `exit_code`, `trigger`, `duration_ms`, `output_bytes`, `dirty`, `members` (JSON), `main_sha`, `log_path`, `hostname`, `image`, `arch`, `nproc`, `cpu` | One row per `air record` and per landing | Red and green history, durations (83,613 ms and 113,347 ms for the last two greens), which members a batch carried, where a red's log tail is, which machine verified |
| `batch_cuts` (91) | `head`, `lane`, `main_sha`, `members`, `cut_at` | Every `air batch cut` | The current batch's members and what main it was built on |
| `landings` (79) | `worker`, `sha`, `result` (`landed`, `rewound`, `refused`), `beads` (JSON), `merge_commit`, `members`, `finished_at`, `pid` | Every `air land` | The landings timeline with the beads each carried; in-flight landings by pid |
| `conditions` (99) | `worker`, `kind`, `first_seen`, `last_seen`, `cleared_at`, `detail` | Every status tick that finds one (the coordinator's channel every 30 s: `mcp.rs:19`) | Open attention conditions with age and text, and the history of cleared ones |
| `captures` (40) | `worker`, `text`, `captured_at`, `status` (`open`, `promoted`, `dropped`), `bead`, `note`, `head_sha` | On `air capture` and `air triage` | The inbox and what each capture became |
| `leases` (0), `lease_wants` (0) | `resource`, `worker`, `pid`, `pid_started`, `reason`, `taken_at`, `heartbeat_at`; who waits | On `air lease` and the hook's lease gate | Who holds what, since when, last heartbeat, and the queue behind it |
| `deliveries` (762) | Every notice Air queued to a worktree: `to_worker`, `kind` (`main-moved`, `batch-ready`, …), `subject`, `content`, `created_at`, `delivered_at` | On every fan-out; `delivered_at` set by the recipient's channel within 5 s (`mcp.rs:109-118`) | A notice feed: what each session was told and when it got it |
| `messages` (65) | Every `SendMessage`: `at`, `from_worker`, `to`, `bytes`, `content` | On the `PreToolUse` hook for `SendMessage` (`hook.rs:1508-1509` shows the shape) | The agent-to-agent traffic, in full |
| `hook_emissions` (72) | What a hook last said per session and key, with a fingerprint and time | Hook calls that emit advice | Mostly internal; the `land:main-readers` row says what `air land` last warned about |
| `bd_cache` (8) | `ready_depth`, `claimable_depth`, `epic_depth` (counts), `tree_readers_raw` (90 KB process list), `fanout_ready_seen`, `fanout_ready_recipients`, two "told" markers | Each status tick and fan-out (`seen_at` 00:34:21Z at the time) | The ready counts without calling bd; nothing else of the board |
| `offers` (5), `runs` (15), `fleet_stop` (0), `edit_journal` (0) | Coordinator heads offered to the lane; `air run` rows with target and machine; the one stop row; which worker touched which file | Rarely; on each `air run`; on stop and resume; on edits | Whether the coordinator's branch may be batched; the remote runs; the stop banner; overlapping edits |
| `.air/events/<day>.ndjson` (2,927 lines on 2026-11-09, 16:11Z to 23:59Z, so 6.3 lines a minute; 2.3 MB) | One line per command and hook call: `at`, `worker`, `command`, `inputs`, `decision`, `reason`, `denominator`, `budgets`, and `bd_ms`/`bd_calls` on commands that called bd | Appended immediately by every `air` process | A decision feed: refusals, cuts, drops, reds, lands, pushes, with Air's reason in prose |
| `~/.claude/sessions/<pid>.json` (5 files, ~600 bytes) | `status` (`busy`, `idle`, `waiting`), `waitingFor`, `tmux` pane, `name`, `cwd`, `sessionId`, `version`, `updatedAt` | Rewritten by Claude Code as the session's status changes (`updatedAt` 17:10 for worker-1) | Busy, idle or waiting-for-the-person, from the harness itself, with no hook in the path |
| `~/.claude/projects/<escaped>/<session>.jsonl` (worker-1: 31.06 MB, 11,938 lines since 2026-11-02T23:18Z, about 4.4 MB a day; the coordinator's 13.0 MB) | Every turn: `assistant` records with `tool_use` blocks and `usage`, `user` records with `tool_result`, plus `system`, `attachment`, `api-request`, `queue-operation`, `last-prompt` and other bookkeeping types | Appended on every API response and tool result | The exact command, file and text of what the agent is doing now (see 2c) |
| `tmux capture-pane -p -t <session>` (8.8 ms measured) | The literal screen of the pane | Live | The last lines the owner would see if attached; the `❯` prompt line and the harness's footer |

### 2a. Which events carry what

The `inputs` object differs per `command`; the other fields are the same on every line. Sampled from
`.air/events/2026-11-09.ndjson` (one line each, 21:31Z to 22:04Z):

- `hook.PreToolUse`: `inputs: {session_id, tool}`; `decision: observed`; `reason: "working -> running"`. For an `Edit` or `Write` the inputs add `path` and `peers` (`hook.rs:654`); for a `SendMessage`, `to` and `bytes`. Over the last 1.5 MB of the day the key sets were `(session_id, tool)` 1,084 times, `(bytes, session_id, to, tool)` 16 and `(path, peers, session_id, tool)` 2, and the tools were Bash 1,061, Write 20, SendMessage 16, Edit 5. **The Bash command text is never on the line**: the hook inserts only `session_id` and `tool` (`hook.rs:157-161`), and Air's stance elsewhere is "Never its argv, which can carry prompt text" (`readers.rs:31-32`).
- `hook.PostToolUse`: the same inputs, `reason: "running -> working"`.
- `hook.Stop`: `inputs: {session_id}`, `reason: "working -> idle; coordinator: no hand-over check"`.
- `hook.Notification`: `inputs: {kind: idle_prompt, session_id}`, `reason: "Claude is waiting for your input"`.
- `channel.push`: `inputs: {conditions: ["closed-not-landed:lane"], for_minutes}`, `decision: pushed`, the condition's text in `reason`.
- `channel.deliver`: `inputs: {id, kind: batch-ready, queued_at, subject: <sha>, waited_s}`, `decision: delivered`, the notice text in `reason`.
- `fanout`: `inputs: {branches: ["worker-3@<sha>"]}`, `decision: batch-ready`, `bd_calls`, `bd_ms`, and a `budgets.git.ms` array with every git call's duration (224 of them on that line).
- `record`: `inputs: {command: ["make","verify"], flags: [...], kind: verify, sha}`, `decision: red`, `reason: "exit 2 in 29353 ms, 1078 bytes; on <exec prefix> (<pod>)"`. Note the `exec` prefix in `flags` and `reason` carries the kube config path from the owner's `.claude/air.local.json`.
- `land`: `inputs: {all, beads, repo_worker, role, workers}`, `decision: landed`, `reason` with the landed branch, the beads, the next batch-ready branch and `next: air batch cut`.
- `claim`: `inputs: {bead}`, `decision: refuse`, the refusal text in `reason`.
- `done`: `inputs: {beads: [...], reason: <the proof text>}`.
- `batch-cut`: `inputs: {worker, head, beads, against, against_sha, paths, stage}`, `decision: drop`, `reason: "dropped from batch: worker-3 at 20375372 conflicts with main at 89b8cb7d in docs/design.md"`.
- `status`: `inputs: {}`, `decision: quiet`, `denominator: "6 workers, 15 overlaps, inbox 1"`, and the budgets it spent.

### 2b. The transcript

One file per session; `sessions.transcript_path` points at it (five rows, all under
`~/.claude/projects/<escaped main checkout path>--claude-worktrees-<name>/`). In the last
2 MB of worker-1's file the record types were `attachment` 174, `assistant` 142, `user` 82,
`api-request-blob` 72, `api-request` 65, then 40 each of `last-prompt`, `custom-title`,
`agent-name`, `mode`, `permission-mode`, `atis-latch`, `queue-operation` 34 and `system` 22; the
median line was 722 bytes and the largest 107,940 bytes (a large tool result). The reader keeps
only two types.

An `assistant` record carries `timestamp`, `sessionId`, `cwd`, `gitBranch` (`worktree-worker-1`),
`version` (`2.1.309`), `uuid`, `parentUuid`, and `message.model` (`claude-fable-5-1`),
`message.usage` (`input_tokens`, `cache_read_input_tokens` 309,106 in the sample,
`cache_creation_input_tokens`, `output_tokens`) and `message.content`, a list of blocks. A
`tool_use` block has `id` (`toolu_…`), `name` (`Bash`) and `input`; for Bash that is `command` and
`description`, and the sampled command was the full `sqlite3 -header "file:…ledger.db?immutable=1"
"select id, worker, sha, …"` the worker ran. A `Write` block's input is `file_path` and `content`.
A `text` block is what the agent wrote to its person: "The sweep is committed as c4e0d5b with the
bead's trailer, the proof is on the bead, and the branch is batch-ready…".

A `user` record with a `tool_result` block carries `tool_use_id` (matches the `tool_use` id),
`content` (the stdout, as text) and a `toolUseResult` object with `stdout`, `stderr`,
`interrupted`, `isImage` for Bash, or `filePath`, `content`, `structuredPatch`, `originalFile` for
a Write. A `tool_use` whose id has no `tool_result` yet is the call in flight; Lookout derives the
"doing now" line the same way ("A tool call with no result yet is treated as what the session is
doing", its how-it-works page).

Tailing by byte offset is cheap: a 64 KiB tail of the 31 MB file read in 0.03 ms (`python3`
`seek` and `read`, measured here). A reader remembers the file size it last saw, reads only the
new bytes, splits on newlines, parses the last complete lines and drops the rest. The usage block
gives context size and cache use per turn for free, which is the number the status line and OTel
would otherwise supply.

### 2c. Which source answers "what is this agent doing right now"

At the coarsest grain, `sessions.state` plus `detail` says "running Bash" and changes on every
hook call, which the ledger already has. One step finer, `~/.claude/sessions/<pid>.json` says
busy, idle or waiting and what it waits for, from the harness, also free. The finest grain is the
transcript tail: the exact Bash command, the file being written and the sentence the agent last
said, at the moment the API response lands, before the tool even runs, for the cost of one `stat`
and a few KB read per poll per session. The tmux pane is a different thing: it is what a person
would see, including the harness's "Cogitated for 48s" line and the prompt, but it is 80x24 of
rendered text with no structure, and the Claude Code UI redraws it, so it is a glance, not a
source. The recommendation is transcript first, pane last line as decoration, ledger for every
Air fact.

## 3. Reading without touching the agents

**SQLite.** The ledger is opened with `journal_mode=WAL` and `synchronous=NORMAL`, and a busy
handler that behaves like `busy_timeout` at 1 s (`crates/ledger/src/lib.rs:291-300`, `237`). In
WAL mode "readers do not block writers and a writer does not block readers"
(https://sqlite.org/wal.html §1). A second process reading is the normal case: every worktree's
hook already is one (`lib.rs:76-78`). Two cautions. First, `Ledger::open_in` is read-write and runs
`schema::migrate` (`lib.rs:61-69`), so a UI must not use it; it opens its own connection with
`rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY` (`mode=ro`) and runs no migration, or a UI built from a
newer tree would migrate the ledger under the fleet's older binary. Second, `immutable=1` is wrong
for a long-lived reader: SQLite then "skips all file locking and change detection" and "might
return incorrect query results and/or SQLITE_CORRUPT errors" if the file changes
(https://sqlite.org/uri.html §3.3). It was right for this research's one-shot `sqlite3` calls and
is wrong for a process that polls. A read-only open of a WAL database needs the `-shm` and `-wal`
files to exist or the directory to be writable so they can be created (wal.html §5); `.air/` is the
owner's directory, so this holds, and it is why `sqlite3 -readonly` failed here when no writer had
the file open (no `-shm` existed; `ls .air/` showed only `ledger.db`).

**NDJSON and transcripts.** Append-only files: remember the byte offset, read what is new, parse
complete lines. A day file rolls at midnight UTC (`2026-11-09.ndjson` ends 23:59:58Z and
`2026-11-10.ndjson` begins), so the reader also lists the directory for a newer file. A transcript
is 0600 (`ls -la` shows `-rw-------`) and the UI runs as the same user. Neither file is locked by
its writer.

**bd.** Three choices. Calling `bd list --json` and `bd ready --json` from the UI is one or two
processes per poll at about 118 ms each (`air status --json`, `bd_latency`); at one poll every
30 s that is four processes a minute against a fleet that already runs about 2.6 a minute (3,768 in
a day, `air audit`), and it goes to the same Dolt server on port 3806 (`.beads/dolt-server.port`;
`bd_mode.port` in the status JSON). `bd serve` is a daemon with a write surface and no read-only
mode (§1a), so no. `bd_cache` holds only three counts (§2), so it is not a board. The UI calls `bd`
itself, at the slow tier, through `crates/bd`'s `WorkLedger` so the per-call timeout and JSON parsing
are shared (`docs/design.md:60`). `.air/ready.json` (`{"at","ids","priorities"}`, written by the
fan-out; `ready_cache.rs:33`) is a free, 30-second-fresh ready list for the fast tier.

**tmux.** `tmux capture-pane -p -t <pane>` is a read of the server's buffer, 8.8 ms measured, and
touches no process in the pane. The exact pane id is in `~/.claude/sessions/<pid>.json` (`"tmux":
"ai_runner-worker-1:@16.%16"`), so the UI need not guess from names. Air already lists sessions
with `tmux list-sessions` (`crates/cli/src/cmd/tmux.rs:124-140`).

**Polling and cost.** Two tiers. The fast tier every 2 s: about ten `SELECT`s on a 1.2 MB SQLite
file (sub-millisecond each), five `stat`s and tail reads of transcripts (microseconds when nothing
changed), five `~/.claude/sessions` reads, five `tmux capture-pane` (45 ms). Per minute: 300 SQL
reads, 150 transcript tails, 150 session-file reads, 150 tmux captures, under 2 s of CPU in total,
no process in an agent's tree touched. The slow tier every 30 s: one `bd list --json` and one `bd
ready --json` (about 240 ms), five `git rev-parse` for heads, one `git log --first-parent -20
main`; per minute, four bd processes and about twelve git calls. The coordinator's channel already
runs the full `status::gather` every 30 s (`mcp.rs:19`), with its three bd calls and its process
listing, and writes `conditions`, so the UI reads that table and never computes conditions itself.
If a panel wants something only `gather` derives (`batch_ready`, `not_batch_ready`, `landable`,
`land_skipped`, `red_split`, `loops`), the honest options are to call those functions with a
read-only ledger where their signatures allow (`batch::red_split_standing(ledger, repo)`,
`batch.rs:556`; `landable` "reads git and the ledger only", `status.rs:221-223`), or to run `gather`
no more than once a minute and accept its writes (one `bd_cache` update and one `status` event
line a minute, attributed to a `ui` worker, which `air audit` would count). The first is the one
that keeps "no writes" true.

## 4. The smallest build

`Cargo.lock` has no HTTP server and no async runtime: `rg -n 'name = "(tiny_http|axum|hyper|
rouille|astra|may_minihttp|tokio|ureq|reqwest|mio|warp|actix-web|rocket|http|httparse)"'
Cargo.lock` matched nothing among its 124 packages (`rg -c '^name = ' Cargo.lock`). The CLI's
dependencies are the three workspace crates, `rusqlite`, `clap`, `serde`, `serde_json`, `jiff`,
`thiserror`, `wait-timeout` and `signal-hook` (`crates/cli/Cargo.toml`), and the workspace forbids
unsafe code (`Cargo.toml`, `unsafe_code = "forbid"`). The design row says "Rust, one binary, no
async runtime" (`docs/design.md:704`).

Three ways to answer GET on loopback:

- `tiny_http` 0.12.0: "Low level HTTP server library", synchronous, four non-optional
  dependencies (`ascii`, `chunked_transfer`, `httpdate`, `log`), 76 million downloads, last
  published 2022-10-06 (https://crates.io/api/v1/crates/tiny_http and
  `/tiny_http/0.12.0/dependencies`). Small and stable, but four years without a release.
- `axum` 0.8.9: seventeen non-optional dependencies (`axum-core`, `tower`, `http`, `http-body`,
  `futures-util`, …) and it serves through `hyper` and `tokio`, which are optional on the crate but
  required to listen (https://crates.io/api/v1/crates/axum/0.8.9/dependencies). It brings the
  async runtime the design excludes. No.
- std only: `std::net::TcpListener` on `127.0.0.1`, read the request line, match the path, write
  `HTTP/1.1 200 OK`, `Content-Type`, `Content-Length` and the body, close. Air already hand-rolls a
  line protocol server rather than take an SDK, for the stated reason that "owning the read loop,
  the write lock, and the poll thread is smaller than auditing an async runtime for the same
  guarantees" (`mcp.rs:10-14`). A GET-only server for one browser on loopback is about a hundred
  lines and needs nothing new in `Cargo.lock`.

The recommendation is the std-only server, with `tiny_http` as the fallback if keep-alive or
concurrent requests turn out to matter (they should not: one page, one poll at a time).

**Option A, `air ui [--port N]`.** One command, one thread accepting connections. It serves `/`
(the HTML, embedded with `include_str!`), `/api/fleet` (sessions, session files, transcript tails,
pane lines), `/api/beads` (the slow tier's bd answers, cached in memory), `/api/lane` (inflight,
batch cuts, red batch and split, verify history, landings), `/api/attention` (open conditions,
captures, leases, wants, fleet stop), `/api/events?since=<offset>` (new event lines) and
`/api/notices` (deliveries and messages). The page polls `/api/fleet` every 2 s and the rest every
10 to 30 s, each endpoint telling it the `at` of the data. The server opens the ledger read-only
once, keeps the per-file offsets in memory, and writes nothing anywhere. Estimated size: 600 to
900 lines of Rust (server and routing about 120, read-only queries about 200, transcript tailer
about 120, session files and tmux about 70, bd and git about 80, assembly about 100, tests about
200) and 400 to 600 lines of HTML, CSS and vanilla JavaScript in one file. For scale, `tmux.rs` is
320 lines, `bd_server.rs` 853 and `mcp.rs` 1,155 (`wc -l`).

**Option B, `air ui --export`**, writing a JSON snapshot every N seconds for an external page.
It still needs a server: a `file://` page cannot fetch a sibling file in current browsers without
flags, so the owner runs a second process anyway. It also writes a file every few seconds, which is
one more moving part on a machine where the rule is no writes. Rejected.

Under `do-less`: the failure this removes is the owner finding out by attaching to panes
(`status.rs:396-399`) and refusing to read status prose (CLAUDE.md); the probe is a test that
starts the server on port 0 against a temporary repo with a seeded ledger and transcript and
asserts each endpoint's JSON; and it is removed when `claude agents` or its successor shows Air's
facts, or when a round passes in which the page was not opened (count GETs in memory, print the
count at exit).

## 5. What the page shows

Each panel names its source and its poll.

1. **Fleet**, one card per session, every 2 s. Role, state and running tool from `sessions`; busy,
   idle or waiting-for and what for from `~/.claude/sessions/<pid>.json`; model from
   `sessions.model`; head from `git rev-parse` of the worktree branch (slow tier); claim and
   declared files from `claims`; the current or last `tool_use` (command text, file path) and the
   last `text` block from the transcript tail, with the usage block as a context gauge; the last
   non-empty line of `tmux capture-pane`; alive or gone from `pid` and `pid_started`. `air status`
   already assembles the first, third, fourth and fifth of these in `WorkerView` and `Session`
   (`status.rs:69-149`) but not the transcript or pane parts.
2. **Beads board**, every 30 s. Ready and claimable from `bd ready --json` with `.air/ready.json`
   as the 2-second fallback; in progress and done-not-landed from `claims` joined to `bd list
   --json` for titles and blockers; closed and landed from `landings.beads`; epics to decompose
   and deadlocks are `gather`'s and should be read from its JSON only when it is otherwise run.
3. **Lane**, every 2 s for the in-flight row, 30 s for the rest. `verify_inflight` with elapsed
   time and whether its pid lives; the current batch from the newest `batch_cuts` row; the standing
   red batch and the members not yet verified alone from `red_split_standing` (`batch.rs:548-551`,
   `RedSplit.untested`); the last twenty `verify_runs` with exit, duration, members and `log_path`;
   `landings` as a timeline with beads and merge commits; `loops` (`ready_to_batch`,
   `done_to_landed` seconds, `loops.rs:25-30`) as two sparklines. `air status --json` has
   `verifies_in_flight`, `landings_in_flight`, `batch_ready`, `not_batch_ready`, `red_batch`,
   `red_split` and `loops` already (§1e).
4. **Attention and inbox**, every 10 s. Open rows of `conditions` with age and `detail`; open
   `captures`; `fleet_stop`. `air status --json` has these as `attention`, `inbox_depth`,
   `oldest_capture_at` and `fleet_stop`.
5. **Leases**, every 10 s. `leases` with heartbeat age and pid liveness, `lease_wants` beneath.
6. **Main**, every 30 s. `git log --first-parent` of main with the `landings` row for each merge
   commit, so each line reads "7b884565 air-clf landed by lane at 00:09".
7. **Feed**, every 5 s. New event lines by offset, filtered to decisions a person reads (`refuse`,
   `drop`, `red`, `landed`, `pushed`, `delivered`), plus `deliveries` and `messages` as they land.
   This is the "feed pane" air-ank declined for the coordinator (`docs/design.md:723`), at zero cost
   to any session.

## 6. Risks

**The reader check.** `air land` and `air status` list "every process whose working directory is
inside a worktree of this repo" with `lsof -d cwd` on macOS (`readers.rs:1-2`, `616-619`); it is
"a fact, never a refusal" (`readers.rs:13-14`), so a reader cannot make `air land` refuse. It can
make it warn, and worse, it can silence a condition: `idle-without-claim` reads the same listing
("nothing of its own runs in its tree", `docs/design.md:185`; `readers.rs:13`), so an `air ui`
process whose cwd is a worker's worktree would make that worker look busy. Holding `.air/ledger.db`
open does not count; only the cwd does (`holders_of`, the open-file `lsof`, is used for git lock
files only: `readers.rs:535-537`). The fix is one line: `air ui` resolves the repository path and
then changes its working directory to `/` before serving, so it is in no tree. It then appears
nowhere in `tree_readers` and in no `land:main-readers` warning.

**Secrets.** The transcript's `tool_result` content is raw stdout, and `record` lines carry the
owner's exec prefix with a kube config path (§2a). The page binds `127.0.0.1` only, on the trust
model `bd serve` states for the same situation: "the trust model is the loopback boundary itself,
the same one the database behind it already relies on" (`bd serve --help`). No token, no TLS, no
redaction: the files are already readable by the same user, and a redaction layer is a mechanism
with no recorded failure behind it. The page shows command text, which is more than `air status`
shows of a process ("Never its argv", `readers.rs:31-32`); that is the owner's ask and the page is
the owner's.

**Ports.** Dolt is on 3806 here (`.beads/dolt-server.port`), and `air init` picks a port in
3400..3900 (`bd_server.rs:499-503`, `init.rs:491`); bd's own defaults are 3307 and 3308
(`bd_server.rs:499-500`); `bd serve` defaults to an ephemeral port; Gas Town's dashboard takes
8080, Overstory's 7321, Claude Code's Prometheus exporter 9464 and OTLP 4317. `air ui` should
default to a fixed port outside all of these and refuse to start if it is taken, naming the holder,
the way `bd serve` does on a fixed port ("On a fixed port the second one fails to bind, which is
the intended behavior").

**Writes by accident.** `status::gather`, the MCP resources and `air status` all write (§1e). A UI
that reaches for them "because the struct is there" becomes a second status loop writing
`bd_cache`, `conditions` and event lines at its own cadence, and `air audit`'s "evaluated" counts
would include it. The read-only connection and the per-function reuse in §3 are the guard.

**Format drift.** The transcript and `~/.claude/sessions/<pid>.json` are undocumented; agent view's
page says its job files "are not a stable interface". Every field the UI reads is one it
already reads for the model (`hook.rs` reads the transcript for `sessions.model`,
`status.rs:83-86`), and the fallback is `claude agents --json`, which is documented as "the
supported way to read session state from outside Claude Code", at the cost of one `claude` process
per poll. If the transcript shape changes, the fleet card loses its "doing now" line and nothing
else; the page says so rather than guessing.

## Sources read

- CLAUDE.md; `docs/design.md` §3 to §6, §9 to §11; `crates/ledger/src/lib.rs`;
  `crates/cli/src/cmd/{status,mcp,hook,land,readers,budgets,batch,loops,bd_server,init,tmux}.rs`;
  `crates/cli/Cargo.toml`, `Cargo.toml`, `Cargo.lock`.
- `air --help`, `air --version`, `air status --json`, `air audit`, `bd --help`, `bd --version`,
  `bd serve --help`, `claude --help`, `claude --version`, `claude agents --help`, `claude agents
  --json`, `tmux ls`, `tmux list-panes -a`, `tmux capture-pane -p`, `sqlite3 "file:…?immutable=1"`,
  `ls`, `wc`, `stat`, `python3 -I` one-liners, all on 2026-11-09.
- https://github.com/gastownhall/gastown and its `internal/cmd/dashboard.go`,
  `internal/web/static/dashboard.js` via `gh api`; https://github.com/jayminwest/overstory;
  https://github.com/andyrewlee/awesome-agent-orchestrators;
  https://github.com/nimrod-teich/lookout and `docs/how-it-works.md`;
  https://github.com/buhuipao/agent-console; `steveyegge/beads` `docs/community-tools.md`,
  `docs/CLI_REFERENCE.md` via `gh api`.
- https://code.claude.com/docs/en/agent-view, /monitoring-usage, /statusline, /agent-teams,
  /cross-session-messaging.
- https://sqlite.org/wal.html, https://sqlite.org/uri.html; https://crates.io/api/v1/crates/tiny_http,
  …/tiny_http/0.12.0/dependencies, …/axum, …/axum/0.8.9/dependencies.
