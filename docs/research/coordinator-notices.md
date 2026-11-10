# Coordinator chat vs a notice feed pane

Research for air-ank (2026-11-02, worker-2). Source of the question: an adopter's
air-updates.md entry of 2026-10-28, "the coordinator's chat should carry only what asks it to
act", quoted on the bead. The entry proposes three changes: a per-role notice filter, a second
tmux pane with a live feed, and explicit "no reply" text in the roles. This answers the bead's
five questions and ends with a recommendation. No code changed.

Window for every count below: this repo's ledger and events on 2026-11-02, 16:47Z to 23:50Z.
The ledger was rebuilt today (`sessions.started_at` begins 22:58Z; `deliveries.created_at`
begins 16:47Z) and `.air/events/` holds one file, `2026-11-02.ndjson`, 657 lines. That is one
day with one landing-free round in it, not "the last rounds"; the older record is the
narrative in `docs/solved-issues.md`, cited where used.

## 1. One terminal, two panes: can the launcher do it, and what does it cost?

Tried on this machine (tmux 3.8, macOS 27.2) on a private socket (`tmux -L`), 200 by 50:

    tmux new-session -d -s two-panes -x 200 -y 50 '<chat>'
    tmux split-window -h -t two-panes -l 70 '<feed>'
    # pane 0 129x50 (chat), pane 1 70x50 (feed)

    tmux new-session -d -s two-windows -x 200 -y 50 '<chat>'
    tmux new-window -d -t two-windows -n feed '<feed>'
    # window 0 200x50 (chat), window 1 "feed" 200x50

Both work with one extra tmux call after the session exists. In the launcher that is one more
invocation in `spawn_tmux` (`crates/cli/src/cmd/launch.rs:903`) after `new-session`
(`tmux_session_argv`, `launch.rs:859`): `split-window -h -t <session> -c <main checkout> --
<feed command>`, plus the `--print` line and a test; about ten lines. The cost is not the pane
but the feed command. Air has no `air status --follow` (`air status --help`: `--attention`,
`--json`, `--repo` only), macOS ships no `watch(1)` (`which watch`: not found), and `tail -f`
on the events file shows raw JSON, one line per hook call: of the coordinator's 208 event
lines today, 173 are `hook.*`. A readable feed is a new command or a filter, and that is the
real line count.

Which reads better. Side by side at 200 columns the chat keeps 129 and both are readable. On a
laptop terminal of about 120 columns the same split leaves the chat 50 to 75 columns, where
Claude Code's input box and status line wrap. A second window (prefix then `n`) or a second
session the owner attaches in another terminal tab keeps the chat at full width and the feed
one keystroke away. Two windows read better; a pane is for a wide screen. Neither needs the
launcher: the owner can open the window by hand today, which is the smallest version (§5).

## 2. Can Air route the no-reply notices to a feed, as a view and not a store?

How a notice reaches a session today. Each session's `air mcp` takes its own rows from
`deliveries` every 5 seconds (`crates/cli/src/cmd/mcp.rs:656`, `deliver_once`); a row is unique
per `(to_worker, kind, key)` (`deliveries_change` index, `sqlite3 .air/ledger.db .schema
deliveries`). Who gets which notice is decided per notice in `crates/cli/src/cmd/fanout.rs`,
and `docs/design.md` §6.5 lists them. The coordinator's channel also pushes attention
conditions from the poll loop (`mcp.rs:901`) and, from the same loop, session joined and left
(`session_changes`, `mcp.rs:867`), which are sent straight to the channel: no `deliveries` row
and no event line.

| Notice to the coordinator | Needs a reply? | Decided at | Routed by |
|---|---|---|---|
| main moved | no (roles.md: "It needs no reply") | `main_moved_recipients`, `fanout.rs:762`: every worktree but main, the lane and the lander | worktree name; role is not consulted |
| batch green or red | no, when its branch is a member; a green names no bead for it (air-5ynn, `fanout.rs:609`) | `batch_result_notes` | membership |
| session joined, session left | no | `session_changes`, `mcp.rs:867` | pushed from the poll, coordinator only |
| capture from | yes: `air inbox`, `air triage` | `capture_to_coordinator`, `fanout.rs:308` | fixed recipient |
| the ready queue is empty | yes: file or decompose | `queue_empty`, `fanout.rs:371` | fixed recipient |
| bd server was down | yes | the tick | fixed recipient |
| attention conditions (ten kinds) | yes | `push_tick`, `mcp.rs:824` | coordinator and owner only (`polls_attention`, `mcp.rs:629`) |
| fleet stop, fleet resumed | no (own command) | `air fleet` | every session |

So a filter exists already, per notice, and the two no-reply kinds that reach the coordinator
are each one decision: drop the coordinator's worktree in `main_moved_recipients`, and stop
pushing `session_changes` on the channel. No per-role table is needed.

What a feed can read without a new store, from `air status --json` (`snapshot` keys, run
2026-11-02): `sessions` (who is up, state, model, pid liveness), `batch_ready`,
`not_batch_ready`, `landable`, `red_batch`, `closed_not_landed`, `leases`, `lease_wants`,
`verifies_in_flight`, `landings_in_flight`, `loops`, `inbox_depth`, `ready_depth`,
`fleet_stop`. From the ledger: `landings` (every main move), `verify_runs` (every batch
result), `sessions` (every join; a leave is the row's state), `deliveries` (every notice with
its delivered time), `messages` (every `SendMessage`). From `.air/events/`: `fanout`,
`channel.deliver`, `channel.push`, `status.attention` lines. One gap: a session leaving is
computed in the poll's memory (`known` set, `mcp.rs:867`) and written nowhere, so a feed that
wants "left" reads the `sessions` row going `gone` (`hook.SessionEnd`, `idle -> gone`, which
is an event line) rather than the push.

## 3. What the coordinator loses, and what it acts on today

What it loses. roles.md gives main moved one use: "continue whatever waited on the landing,
such as filing the next wave". Without it the next wave waits for `the ready queue is empty`
(sent when a worker is free and nothing is claimable, `fanout.rs:371`) or the 5-minute
heartbeat. Batch results to the coordinator carry no close for it (air-5ynn) and are lost
without loss. Session joined and left tell it the fleet came up, which `air status` and the
`air fleet up` output already say.

What it acted on today, by input, from `.air/events/2026-11-02.ndjson` (worker `coordinator`,
`channel.deliver` and `channel.push` lines, and the tool calls in the five minutes after each):

| Input | Count | Followed within 5 min by |
|---|---|---|
| `bd server was down` (deliver) | 1 | `air status`, six Bash calls |
| `capture from worker-1` (deliver) | 1 | `air triage` at 23:16:52 (dropped, fixed in place) |
| `gone-with-claim` ×3 (push, 23:17:02) | 3 | `air status` twice, Bash |
| `idle-with-claim` ×3 (push, 23:38:23) | 3 | `air status`, three `SendMessage` |
| main moved | 0 | no landing today (`landings` table) |
| batch green or red | 0 | the coordinator's branch was in no batch |
| session joined, session left | not recorded | zero lines in events; no table |

Every logged input was acted on. The no-reply kinds sent nothing today, so the cost the
adopter names has a count of zero here, and it is a zero of the second and third kinds in
`do-less` ("the subject never occurred", and "the input never arrives"): no landing happened,
and the join and leave pushes leave no record at all.

Turns. The coordinator had 30 turns today (`hook.Stop`): 16 ended with tool use
(`working -> idle`) and 14 with none (`idle -> idle`). Only 2 of the 14 came within 120 seconds
of a logged channel input. The other 12 cannot be attributed by the record: a join or leave push
and the owner typing both leave no event line, and the two cannot be told apart. The adopter's
"every no-reply notice costs a turn" is therefore not measurable in this ledger today, and the
older number that looks like it (226 of 227 failed turns within 30 s of a push, 0.4.10 at an
adopter, `docs/solved-issues.md` "The channel and notices") was the repeat defect, fixed by
keying pushes per subject (air-8goe), not the no-reply kinds.

## 4. Prior art

Eight systems, each read from a primary page on 2026-11-02 by a background agent of this
session; the Claude Code row was then checked against the page a second time and one cell
corrected. Systems with no primary page confirming their behaviour were dropped: OpenAI Codex cloud (https://learn.chatgpt.com/docs/cloud describes review and follow-ups only, no feed or context details), mprocs (README moved to the dekit successor, not read), Cursor background agents, Goose, Aider, CrewAI (not checked; no primary page read).

| System | Operator feed (where) | Agent context receives | No-reply notices kept out of context? | Source (URL, accessed 2026-11-02) |
|---|---|---|---|---|
| Gas Town (steveyegge/gastown) | `gt feed`: "interactive terminal dashboard for monitoring all agent activity in real-time", three-panel TUI with an Event Stream of "creates, completions, slings, nudges, and more" and a convoy panel; `gt feed --window` opens it in "a dedicated tmux window"; `gt dashboard`: web page, "single-page overview of everything happening in your workspace: agents, convoys, hooks, queues, issues, and escalations". Problems view: `n` nudges the selected agent. | Mail and priming via Claude Code hooks: default base config is SessionStart and PreCompact run `gt prime --hook`, UserPromptSubmit runs `gt mail check --inject` (hook `mail-check`, enabled for all roles). Runtimes without hooks get a "startup nudge fallback". | Partly. The event stream (feed, dashboard) is operator-only; agents get mail, not the feed. The README does not say mail is filtered by whether a reply is needed; it is injected on every user prompt. | https://github.com/steveyegge/gastown (README: Activity Feed, Dashboard, Notes); https://github.com/steveyegge/gastown/blob/main/docs/HOOKS.md (registry table, default base config) |
| claude-squad (smtg-ai) | One TUI window: session list, `tab` toggles a preview pane and a diff pane, `shift-up/down` scrolls the diff. Each agent is its own tmux session in its own git worktree. | Only what the operator types: `N` creates a session "with a prompt"; `enter`/`o` attaches to the agent's terminal to prompt again. `-y/--autoyes` (experimental) auto-accepts confirmation prompts. | Yes, by construction: the tool injects nothing; the operator sees status through preview/diff panes, the agent sees only its own tmux session. | https://github.com/smtg-ai/claude-squad |
| agent-deck (asheshgoplani) | TUI session list with four states (running, waiting for input, idle, error) from "smart polling"; waiting sessions also appear in the tmux status bar ("Notification Bar", `Ctrl+b` then `1`-`6` jumps to them); `C` opens a context inspector showing "what the session is sent, ranked by cost". | `agent-deck session send <session> "<text>"` types a prompt into the session's pane; `session ref` types a reference without sending. A transition notifier daemon "sends parent nudges when child sessions move running -> waiting, error, idle" (suppressible per session with `--no-transition-notify`). Watchers forward "a short event string to the conductor". Conductor identity is a `CLAUDE.md`/`AGENTS.md` file. | Mostly. Status, the notification bar and the inspector are operator-only. Transition nudges do enter a parent (conductor) session's context, but only the parent's, and can be turned off per child. | https://github.com/asheshgoplani/agent-deck (README: Notification Bar, keybindings, Conductor, Watchers, "Built-in status-driven notifications") |
| Claude Code hooks (the harness under Air) | Hook stdout: "For most events, Claude Code writes stdout to the debug log and doesn't show it in the transcript." Exit-2 stderr on SessionStart "Shows stderr to user only"; `systemMessage` is a "Warning message shown to the user". | Plain-text stdout is added to context only for `UserPromptSubmit`, `UserPromptExpansion`, `SessionStart`, `PostModelSwitch`; other events reach Claude only through `hookSpecificOutput.additionalContext` (wrapped in a system reminder) or exit-2 stderr. | Yes, by event type. The event table gives `Notification` "No decision control. Used for side effects like logging or cleanup" and "Exit code and stderr are ignored", so a notice can be routed to the operator without touching context (checked against the page a second time, 2026-11-02; the first read overstated this as forwarding to an external service). | https://code.claude.com/docs/en/hooks |
| Devin (Cognition) | Devin Desktop "Agent Command Center": "a single view" of local and cloud agents, agent sidebar lists every session (filter, sort, group, folders); running sessions are greyed out and read-only; optional native OS notification when a session "finishes or needs your input". For managed Devins, the coordinator session, not the human, "monitors progress, resolves conflicts, and compiles results". | The user's chat messages (a message sent while the agent works is queued and editable); `Cmd/Ctrl+L` adds selected transcript text to the input; rules pulled in via @-mention. Child sessions receive "follow-up instructions or clarifications" from the coordinator or via the Devin MCP. The coordinator "is woken automatically" when a child completes or asks for input. | Partly. Status is a sidebar and OS notification, not a message. But child completion/input requests wake the coordinator, i.e. they do enter the coordinator's context; the pages do not say whether that is filtered. | https://docs.devin.ai/desktop/agent-command-center; https://docs.devin.ai/work-with-devin/advanced-capabilities |
| Google Jules | Web task view: "An activity feed as each step completes", inline explanations, a mini diff per file and a full diff editor; browser notification "when the task completes or needs your input". | The task prompt, the plan approval, and chat-box messages mid-task ("You can intervene at any time, you're in control"); Jules "replans or revises the task" on a message. | Yes. The activity feed and notifications are the human's view; the pages describe no path by which they are fed back to the agent. | https://jules.google/docs/running-tasks/; https://jules.google/docs |
| LangGraph Studio (LangChain) | Browser: thread history in the right-hand pane with a detail slider, collapsible "turns, nodes, and keys of the state", Pretty/JSON rendering; chat mode shows the conversation with a toggle to hide tool calls; breakpoints pause "before or after" a node. | Only state: the input form from the graph's state schema, `Continue` on an interrupt, "Edit node state" then `Fork`, or an edited human message. | Yes. The trace is a read of checkpointed state; nothing shown in Studio is appended to the graph's messages unless the human submits it. | https://docs.langchain.com/langsmith/use-studio |
| Metis (colliery-io) | Human: `metis status` ("Project overview"), `metis list`, and a Tauri desktop GUI, "A visual kanban interface for managing projects"; the TUI is marked "deprecated". | MCP tools (`create_document`, `transition_phase`, `search_documents`) plus plugin hooks that "detect the project at session start and re-inject state after compaction". | Not documented. Both sides read the same markdown and SQLite index; the README describes no operator event stream distinct from the agent's MCP/hook channel. | https://github.com/colliery-io/metis |

Every system that has both a feed and agents keeps them on different media: the operator reads a
pull surface (a TUI pane, a tmux status bar, a web dashboard, a thread trace, a kanban) built from
state the agents already wrote (events, beads, checkpoints, session status), while the agent only
ever receives text that a specific sender chose to push into its prompt (a typed message, mail at
`UserPromptSubmit`, a nudge, an interrupt resume). The harnesses that make this explicit do it by
channel type rather than by content: Claude Code adds stdout to context only for four hook events
and leaves Notification output to side effects; Gas Town routes events to `gt feed` and mail to
the prompt hook; agent-deck shows status in the bar and only sends a nudge to a parent session, and
lets you switch that off per child. None of them documents filtering a push by "needs a reply".
The closest is agent-deck's per-child `--no-transition-notify` and Claude Code's event split.

What it says for Air. Air already follows the common pattern on the medium: `air status`, the
ledger and the events file are the pull surface, and the channel is the push. The one place it
departs is pushing two pull-surface facts (main moved, session joined or left) into the
coordinator's context, which none of the eight do for their operator feed; and like all eight,
Air has no "needs a reply" filter, only a per-notice recipient list (§2).

## 5. The do-less questions for each proposed change

**A per-role notice filter.**
Recorded failure: the adopter's 2026-10-28 entry, one coordinator line per no-reply notice; no
count with it, and this ledger has zero no-reply deliveries to count (§3). Fact or judgement:
a fact (who is told what), which Air may decide. Measurement first: log every channel push
including session joined and left as an event line with its kind, then count coordinator turns
with no tool use within 60 s of a no-reply kind over one round. Removal: when Claude Code lets a
channel event be delivered without starting a turn, or when the count is zero for a round.
Smallest version: not a table; drop the coordinator from `main_moved_recipients` and stop
pushing `session_changes` (two lines), keeping `the ready queue is empty` as the next-wave
trigger. Not before the count.

**A second pane with a live feed.**
Recorded failure: none in this record; the nearest is "the owner had to attach to find out",
which named the tmux sessions (`crates/cli/src/cmd/tmux.rs:5`). Fact or judgement: a view of
facts, read by the owner, so it directs nobody. Measurement first: whether the owner opens one
by hand; `tmux new-window -n feed` on the coordinator's session costs nothing to try. Removal:
when the chat carries only what asks the coordinator to act, at which point the feed is `air
status`. Smallest version: no launcher change; a documented one-liner the owner runs, and `air
status --follow` only if the owner keeps running it.

**Explicit "no reply" roles text.**
Already shipped: roles.md says of main moved "It needs no reply" in both the worker and the
coordinator sections (`docs/rules/roles.md`, shipped as `.air/roles.md`). Recorded failure: the
adopter's coordinator answering anyway, on 2.1.308; here unmeasured (§3). The remaining question
is whether the model obeys the line, which is the same count as the filter's. Smallest version:
nothing to add.

## Recommendation

Build nothing yet. First log every channel push, session joined and left included, as an event
line with its kind, and run one round. If the count of no-tool coordinator turns within 60 s of
a no-reply kind is material, the fix is two lines in `fanout.rs` and `mcp.rs`, not a filter
table. The feed is `air status` in a second tmux window the owner opens by hand; a launcher
pane waits for the owner to keep opening one.
