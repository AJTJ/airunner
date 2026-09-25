# Air design

Air is a single Rust binary that lets a few Claude Code agents work on one repository at the
same time. Each agent runs in its own git worktree. Air keeps the facts the agents would
otherwise pass around in chat. It knows which commit passed verification, who is editing which
file, who holds which task, and who holds a shared resource. It refuses exactly one thing:
closing a task without a recorded green verification run at a commit that contains main.

This document describes the tree on 2026-09-14: commit aba00d8, version 0.3.5, plus the
change that day that moved every permission decision to the launcher's role. The numbers came
from commands run against that tree. On 2026-09-25 this became the one design record: the
proposals that were in docs/plans became TODO lines in section 10, and the research in
docs/research became the technology decisions in section 11.

## 1. Context and goals

A fleet is five sessions: three workers, a verification lane, and the coordinator. The workers
and the lane each live in their own worktree on their own branch. The coordinator still runs
in the main checkout today. All five share one task store, called beads, and one main branch.

Without a shared record, agents relay facts from memory. One says its branch is green, another
says nobody is in a file. Those relays drift, and most of the incidents in the decisions log
trace back to that drift. Air holds the facts where every session can read them. It answers
questions about them quickly enough to run inside an editor hook. Everything it produces is a
fact or an answer, apart from the one refusal. The owner can watch and type into every session,
and nothing Air launches runs headless.

Air does not choose or split the work. It does not review code. It does not run verification
itself; it runs the repository's own command and records the result. It does not replace beads
or Claude Code, and it never pushes, deploys, or publishes.

## 2. Overview

```mermaid
flowchart LR
  subgraph harness [Claude Code sessions]
    C[coordinator]
    W1[worker w1]
    W2[worker w2]
    W3[worker w3]
    L[verification lane]
  end
  subgraph air [Air, one binary]
    CLI[air CLI]
    HOOK[air hook]
    MCP[air mcp<br/>channel and tools]
  end
  subgraph stores [State]
    LEDGER[(ledger<br/>SQLite)]
    EV[(event stream<br/>one file per day)]
    GIT[(git)]
    BD[(beads)]
  end
  C -->|commands| CLI
  W1 -->|commands| CLI
  W2 -->|commands| CLI
  W3 -->|commands| CLI
  L -->|commands| CLI
  harness -->|every hook event| HOOK
  C <-->|channel| MCP
  CLI --> LEDGER
  HOOK --> LEDGER
  MCP -->|runs the CLI| CLI
  CLI --> EV
  HOOK --> EV
  CLI --> GIT
  CLI -->|one process per call| BD
```

The sessions are ordinary interactive Claude Code processes. Air starts each one with a role, a
list of commands it may not run, and a few environment variables. Sessions reach Air in three
ways. They run air commands from their shell. Claude Code calls the air hook on every tool call
and lifecycle event, which is how Air sees edits and session changes. The coordinator also has
Air attached as a channel, so conditions that need attention arrive without anyone polling.

All three entry points are the same binary, and they write to the same two stores in the main
checkout. The ledger holds current state and the event stream holds history. Air reads git and
beads through their own commands and never copies them. It records only what those two cannot
reconstruct.

## 3. Components

The workspace has four crates. Line counts are from 2026-09-14.

| Crate | Lines | What it does |
|---|---|---|
| ledger | 3,547 | The SQLite schema, one row type per table, and the event writer. It makes no network calls and never calls beads. |
| hooks | 1,342 | The hook input and output types, the hand-over gate, the edit journal, the worktree fence, and the nudge a claimless worker gets when it stops. The gate is a pure function of a set of facts, so it can be tested without git or a clock. |
| bd | 493 | A wrapper over the beads command line: list ready work, show, claim, set status, comment, close. It also counts time spent waiting on beads. |
| cli | 34,620 | Every command, the hook entry point, the channel server, the launchers, install and init, status and its attention conditions, landing, audit, and the self-test. The self-test file alone is 12,899 lines. |

The ledger is one SQLite file under .air in the main checkout, shared by every worktree. A row
stays until the thing it describes stops being true, and nothing expires on a timer. Beside it,
the event stream gets one line per command and per hook call. Each line says what Air was asked,
what it decided, why, and what it looked at.

The hook reads Claude Code's JSON from standard input and must finish within the five second
timeout the installer sets. It fails open. Any error or panic lets the tool call proceed and
writes an event line saying what happened. The hook never calls beads, with one exception: the
stop nudge checks its cached list of ready beads against beads before naming one.

Each command lives in its own source file, with a header naming the incident it exists for and
when it should be removed. Every command writes one event line and can print JSON. The channel
server runs the CLI with JSON output, so the two can never disagree.

The channel server is a small synchronous JSON-RPC server over standard input. Every 30 seconds
it re-reads the attention conditions from the ledger and pushes any that are new or worse. It
lives as long as the coordinator's session, so it limits line length, puts a time limit on every
child process, catches panics per request, and exits cleanly when its input closes.

The worker launcher creates a worktree under .claude/worktrees on its own branch. It copies in
the ignored files the repository lists, such as environment files. It writes any first task to a
file rather than the command line, then starts Claude Code in the worktree with the role prose,
the deny list, and the role's environment. When asked, or when run from a session without a
terminal, it starts a detached tmux session named after the project and the worker. The
coordinator launcher starts Claude Code in the main checkout with the channel attached.

### 3.1 Roles and permissions

Every permission comes from the AIR_ROLE environment variable, which only the launchers set.
The worker launcher sets it to worker and the coordinator launcher sets it to coordinator. A
shell Air did not start has no role, and Air treats it as the owner, who may do anything. A
value Air does not recognise counts as a worker.

The directory a command runs in never decides what it may do. Neither does the repository path
passed on the command line. The coordinator can therefore move into a worktree and keep every
permission it has. The directory still names things, such as which worktree a verification run
belongs to, but it grants nothing.

Workers are kept from a small set of commands in two ways. Claude Code's deny list stops them
from running the commands at all. Air's own checks refuse the same commands again when AIR_ROLE
is worker, which covers the command being spelled a different way.

## 4. Interfaces

### 4.1 Commands

Any session may run these.

| Command | What it does |
|---|---|
| air record | Runs a check command and records the worker, the commit, the exit code, the duration, the output size, and whether the tree was dirty. It refuses a backgrounded command. A check killed by a signal counts as no verdict, not as a failure. The kinds are verify, docs-check, fitness and precheck; a precheck is a worker's cheap check under a lane and is never read as a verify green. |
| air handover | Says what the gate would decide about this worktree right now, and which command would fix anything missing. |
| air claim | The only way to claim a bead. It checks the ledger, asks beads about the bead, claims it in beads, then writes the ledger row. If beads times out, Air reads the bead again instead of guessing. |
| air release | Returns a bead in progress to open. It never reopens a closed bead. The coordinator can release a claim held by a worker that has gone. |
| air capture | Puts one item in the coordinator's inbox. It never blocks. |
| air holdings | Shows who has edits in which files across all worktrees. |
| air lease | Takes, releases, or reports a named shared resource such as a port. The holder is identified by worktree and process. `air lease needs "<cmd>"` says which lease a command needs, per `leases` in `.claude/air.json`; the PreToolUse hook refuses such a command from a worker not holding it. |
| air status | The one screen: sessions, claims, greens, overlapping edits, inbox, branches ready to batch, checks running (each named by its kind), what else is running in each tree (every process that is not a Claude Code session with its working directory in a worktree or the main checkout, by name and age, or `unknown` with why), and whether the install is out of date. |
| air batch cut | The verification lane's cut, run in its worktree and refused in the main checkout. It takes the branches ready for a batch, oldest ready first by the commit time of each listed head, and checks each against main and against each earlier accepted branch with git merge-tree, which writes nothing. A branch that conflicts is dropped, named with the other side and the paths, and written to the event stream. Then it merges main and each remaining branch at its listed commit into the lane's branch, judging each merge by the index and by leftover conflict markers, not by git's output. It needs git 2.38 or later. With dry run it only checks. It neither verifies nor lands; it prints the next command. |
| air doctor | Reports where the ledger is, its size and schema version, and whether beads is the pinned version. |
| air audit | For each mechanism Air ships, how often it fired, over what, when last, and its removal condition. It gives facts, not verdicts. |
| air selftest | Runs a red and a green probe for every check. There are 157 probes today. |
| air gc | Reports how much of the event stream a retention period would remove, and removes it only when told to. |

Workers may not run these. Air refuses them when AIR_ROLE is worker, wherever they are run.

| Command | What it does |
|---|---|
| air inbox | Lists open captures, oldest first. |
| air triage | Resolves one capture, either by linking the bead the coordinator filed or by dropping it with a reason. |
| air land | Builds a commit from the branch's tree on top of main and moves main forward to it. It refuses unless the branch contains main and has a green at its head, and it refuses while any verification is running. Before moving main it warns, without refusing, about every process that is not a session with its working directory in the main checkout, by pid. It records the beads the branch's commits name. It closes nothing, and it always updates main in the main checkout, wherever it is run from. |
| air close | Closes beads that have already landed, in one beads process, and releases their claims. |
| air worker | Starts a worker session, or prints the command it would run. It can also remove a worktree, but not while the worktree has uncommitted work or a live session. |
| air coordinator | Starts the coordinator session. |
| air install | Adds the hooks and the channel server to the repository's Claude Code settings and writes the role prose. It shows the change first and writes only when told to. It refuses when the air on the path is a different binary, when .air is not ignored by git, or when the repository was installed by a newer version. |
| air init | Sets up a new repository: checks for beads and Claude Code, initialises git and beads, writes the ignore file and Air's config, then installs. |

Two hidden commands, release-check and adopter-check, are run by the Makefile. Claude Code
itself starts air mcp and air hook.

For a sense of real use, here are the counts from an adopter's event stream over 17 days of
rounds, from 2026-08-21 to 2026-09-13. Status ran 4,265 times, claim 1,208, triage 924, capture
842, record 838, handover 506, land 222, holdings 173, lease 122, release 71, close 59, and audit
3. There were also about 148,000 hook events and 44,769 channel polls. This repository's own
counts are not a guide, because Air has mostly been built here rather than used. Usage is one
signal for the audit in section 10, not the verdict.

### 4.2 Channel tools, resources, and conditions

The channel server offers 13 tools and 5 resources. The tools cover status, attention, holdings,
hand-over, claim, release, capture, inbox, triage, close, and taking, releasing, and reporting
leases. The resources cover status, attention, inbox, holdings, and leases. Every one of them
runs the CLI.

The channel pushes nine attention conditions. Each is computed from the ledger, the clock, and a
set of thresholds, with no git involved.

| Condition | Meaning |
|---|---|
| idle with claim | A worker holds a bead and has been idle past the threshold. |
| silent with claim | A worker holds a bead and its session has written nothing for a while. |
| gone with claim | A worker holds a bead and its process has gone. |
| idle without claim | A worker is idle with nothing claimed while beads are ready, no check of its own (verify or precheck) is running, and no process other than its session is running in its worktree. |
| handover not green | Someone tried to close a bead without the green the gate wants. |
| landed not closed | A bead's commits are on main but the bead is still open. |
| landable | A branch is green and contains main. |
| lease held by dead session | A shared resource is held by a process that has gone. |
| lease stale | A shared resource's holder has not refreshed it in time. |

### 4.3 Hook events

Installing Air registers the hook on ten Claude Code events, each with a five second timeout.

| Event | What happens |
|---|---|
| Session start and end | The session's row is created or removed. |
| Before a tool runs | For an edit, Air warns if another worker is editing the same file. If the session's role is worker and the file is outside its worktree, the edit is denied. For a shell command that closes a bead, the gate runs; it refuses only when enforcement is on, which the worker launcher turns on. For a message to another agent, Air records the message and its content. |
| After a tool runs | The edited file goes in the journal and the session is marked working. |
| Tool failure, permission request, permission denied, notification | The session's state is updated and an event line is written. |
| Stop and subagent stop | For a worker, Air adds the gate's verdict when something is missing, and nudges a worker with no claim once, naming ready beads it has confirmed. The verification lane named in the config is not nudged, since it claims no bead. Other roles get nothing. |

Every hook call, including the silent ones, writes one event line.

### 4.4 Files and environment

| Path | Purpose |
|---|---|
| .air/ledger.db and .air/events | The ledger and the event stream. Both must be ignored by git. |
| .air/roles.md | The role prose built into the binary, which every session receives. |
| .air/installed.json | The version and notices this repository has been installed with. |
| .air/ready.json | A cached list of ready beads, used by the stop nudge. |
| .air/tasks | A worker's first task, kept off the command line. |
| .claude/air.json | The repository's config: extra deny patterns for each role, which commands need which lease (`leases`), the verification lane's name, whether batch-ready wants a green precheck (`precheck`), how greens are matched, where digests and journals live, and whether Metis is attached. |
| .claude/settings.json and .mcp.json | The hook entries and the channel server entry, merged in by install. |
| .claude/worktrees | One worktree per worker. |
| .worktreeinclude | Ignored files to copy into new worktrees. |

The launchers set four variables on each session. AIR_ROLE is the role. BEADS_ACTOR is the
worker's name, AIR_PROJECT is the project, and AIR_ENFORCE turns the gate from advice into a
refusal for workers. Other variables override where Air finds Claude Code, beads, and tmux, and
change its timeouts and polling interval.

This is the worker launch, as the launcher prints it for a worker named w9 in this repository:

```
AIR_ROLE=worker BEADS_ACTOR=w9 AIR_ENFORCE=1 AIR_PROJECT=air claude
  --append-system-prompt-file .air/roles.md
  --settings '{"env":{...}}'
  --disallowed-tools 'Bash(air land *)' 'Bash(air close *)' 'Bash(git push *)'
    'Bash(bd create *)' 'Bash(bd sync *)' 'Bash(bd update *--claim*)' 'Bash(claude *)'
    'Bash(air worker *)' 'Bash(air coordinator *)' EnterWorktree ExitWorktree AskUserQuestion
```

The coordinator launch attaches the channel and denies only git push.

## 5. Data

```mermaid
erDiagram
  sessions ||--o{ edit_journal : "session"
  sessions ||--o{ hook_emissions : "session"
  sessions ||--o{ messages : "session"
  claims }o--|| verify_runs : "closes on a green"
  verify_runs ||--o| landings : "verified by"
  verify_inflight ||--|| verify_runs : "becomes"
  captures }o--o| claims : "becomes a bead"
  leases ||--o{ lease_wants : "resource"
  conditions }o--|| sessions : "worker"
```

| Table | Holds | Written by |
|---|---|---|
| verify runs | One row per recorded check, with the commit, exit code, command, duration, dirty flag, tree, and batch members. | record and land |
| verify in flight | The check currently running and its process. | record |
| edit journal | Which worker touched which file, and when. | the hook |
| claims | Which worker holds which bead, the files it declared, and when it let go. | claim, release, close, the gate |
| sessions | Each session's worker name, role, and state. | the hook |
| landings | Each landing attempt, its result, the beads it carried, and the commit it made. | land |
| captures | Inbox items and what became of them. | capture and triage |
| leases and lease wants | Who holds each shared resource and who is waiting. | lease and the hook |
| hook emissions, conditions, bd cache | What a hook last said in each session, attention conditions with their first and cleared times, and small cached beads answers. | the hook, status, commands that call beads |
| messages | Every message one agent sent another, with its content. | the hook |

On 2026-09-14 this repository's ledger held 214 claims, 157 landings, 145 captures, and 1,176
conditions. Ledger rows never expire. Event files are removed only when the owner runs air gc
with its apply option, and never while the ledger still points at them.

## 6. Flows

### 6.1 Claiming a bead

```mermaid
sequenceDiagram
  participant W as worker
  participant A as air claim
  participant L as ledger
  participant B as beads
  W->>A: claim a bead
  A->>L: does another worker hold it?
  alt held
    A-->>W: refused, names the holder
  end
  A->>B: show the bead
  alt owner's bead, assigned elsewhere, or closed
    A-->>W: refused, says why
  end
  A->>B: claim it (atomic)
  alt beads times out
    A->>B: show the bead again
    A-->>W: claim landed, or state unknown
  end
  A->>L: write the claim
  A-->>W: claimed
```

Air asks the ledger first because it is local and cheap. Beads decides races, because its claim
is the only atomic write. A timeout leaves the state unknown, so Air reads the bead again
instead of trying the claim twice.

### 6.2 Closing a bead, the one refusal

```mermaid
sequenceDiagram
  participant W as worker
  participant H as air hook
  participant G as gate
  participant L as ledger and git
  W->>H: shell command that closes a bead
  H->>L: green at this commit? contains main? claimed? digest committed?
  L-->>H: facts
  H->>G: decide
  alt everything present
    H-->>W: allowed
  else something missing, enforcement on
    H-->>W: refused, with what is missing and the fix
  else something missing, enforcement off
    H-->>W: allowed, with the verdict as advice
  end
  H->>L: one event line
```

A green means a successful recorded run at the exact commit, or, if the repository matches by
tree, at any commit with the same tree. When a verification lane runs, the lane's green at a
batch commit that contains both the worker's commit and main also counts. That lets workers
close without running verification themselves. A commit made after the batch was cut is refused,
and the refusal names it. A branch is ready for a batch when it is not landable on its own and
names a bead its worker holds; it does not have to contain main, because the lane merges main
in when it cuts the batch.

The lane cuts with air batch cut, then records its verification at the new head. Which of two
conflicting branches is dropped is decided by the order rule, oldest ready first, and never by
the order anyone typed. The dropped branch's worker resolves the conflict in its own worktree.

### 6.3 Landing a branch

```mermaid
sequenceDiagram
  participant C as coordinator
  participant A as air land
  participant L as ledger
  participant G as git
  participant B as beads
  C->>A: land a worker's branch
  A->>A: is AIR_ROLE worker?
  alt worker
    A-->>C: refused
  end
  A->>L: is any verification running?
  alt running
    A-->>C: refused, names the run
  end
  A->>G: does the branch contain main, with a green at its head?
  alt not landable
    A-->>C: refused, names what is missing
  end
  A->>G: build a commit from the branch's tree, move main forward to it
  A->>B: which beads do the commits name
  A->>L: write the landing
  A-->>C: landed, with the beads, nothing closed
```

Main never moves to a commit whose tree was not verified. The landing commit has exactly the
tree the green describes, so there is nothing to re-verify and nothing to roll back. The worker
closes its own bead, before or after the landing. If a bead stays open once its commits are on
main, the landed not closed condition names it.

### 6.4 A hook call

```mermaid
sequenceDiagram
  participant H as Claude Code
  participant K as air hook
  participant L as ledger
  H->>K: event, tool, directory, session
  K->>K: catch any panic
  alt edit finished
    K->>L: journal the file
  else edit outside the worktree by a worker
    K-->>H: denied, names the path
  else command that closes a bead
    K->>L: run the gate
  else command the repo declares as needing a lease
    K->>L: is it held by this session
  else worker stops with no claim and beads are ready
    K->>L: confirm the ready beads
    K-->>H: name them, once
  end
  K->>L: one event line
  K-->>H: allow, or refuse only for the gate, the fence, or a lease
```

The target is about a tenth of a second per call, so anything slow lives in the CLI instead. If
Claude Code kills a hook at the five second limit, the hook writes nothing. That is why air
audit also counts hook events that have no matching follow-up.

### 6.5 The coordinator's channel

```mermaid
sequenceDiagram
  participant C as coordinator
  participant M as air mcp
  participant L as ledger
  C->>M: connect
  M-->>C: tools, resources, channel
  loop every 30 seconds
    M->>L: compute attention conditions
    M-->>C: push each new or worse condition
  end
  C->>M: call a tool
  M->>M: run the CLI
  M-->>C: result
```

### 6.6 Session states

```mermaid
stateDiagram-v2
  [*] --> working: session start
  working --> running: tool starts
  running --> working: tool finishes
  working --> idle: stop
  idle --> running: tool starts
  working --> [*]: session end
  idle --> [*]: session end
```

Status adds two more states. A session is gone when its process disappeared without ending, and
a worktree has no session when nothing has run there. A stuck state for sessions waiting on a
permission prompt used to exist. It was removed because, in auto mode, the permission request
event never arrives.

### 6.7 Timing budgets

Every wait Air takes records how long it lasted and whether it hit its limit, and air audit
prints the distribution. Calls to beads get ten seconds. The status tick, acceptance reads, the
claim check, and the stop nudge get shorter limits. All of those fail closed, which means the
caller is told and nothing is assumed.

Three budgets fail open, because they sit on the hook path: git gets a second and a half, the
SQLite lock gets one second, and the hook itself gets five seconds. Claude Code enforces the last
one by killing the process, so hitting it leaves no trace except a hook event with no follow-up.
With the fleet running on 2026-09-06, git took 54 milliseconds at the median and 170 at the 99th
percentile. The SQLite lock waited at most 131 milliseconds with six writers.

## 7. Guarantees and failure model

| Always true | Enforced by | A violation shows up as |
|---|---|---|
| A bead closes only with a green at a commit that contains main and every commit naming the bead. | The gate, with enforcement on for workers. | A handover not green condition. |
| Main moves only forward, onto a commit whose tree has a green. | air land. | A commit on main with no verification run. |
| One worker holds a claim at a time. | The ledger check, then beads' atomic claim. | Two open claims for one bead. |
| A worker's edits stay inside its worktree. | The fence in the hook. | A denied edit in the event stream. |
| A worker runs a command the repo declares as needing a lease only while it holds that lease. | The lease gate in the hook, refusing for workers and advising the coordinator. | A lease-refuse or lease-would-refuse event. |
| Permissions come from AIR_ROLE and never from the directory or the repository path. | Every role check reads the role from the environment. | A self-test probe going red. |
| Workers cannot push, file beads, land, close, start sessions, or ask the owner a question directly. | Claude Code's deny list, backed by Air's role checks. | A permission denied event. |
| Every command and hook call leaves one event line. | Each command's logging and the hook's fail-open path. | A hook event with no follow-up. |
| A check that finds nothing says what it looked at. | A rule for every check, probed by the self-test. | A green with nothing examined. |
| Adopting repositories are told about every change that affects them. | A notice list, one release per round, and a release check. | The verify target failing. |

When something fails, Air behaves as follows. The hook fails open on any error and on the five
second kill, because it must never block the editor. The kill is silent, so it is measured by
unmatched events. A beads timeout during a claim is treated as unknown and the bead is read
again. A killed verification is recorded as no verdict.

Everything else fails closed and says why. Landing refuses while a verification runs, unless
the owner overrides it, and the override is counted. A worker is refused landing and closing
wherever it runs. A repository path outside this project is refused. Install refuses a downgrade, a .air directory that git does not
ignore, or a different air binary on the path.
Doctor and init refuse a missing or wrong version of beads.

The close gate is the only closed default an agent meets in normal work. Everything else is a
fact, advice, or a refusal that names the command to fix it.

## 8. Enforcement and judgement

Air enforces a short list. The deny list decides who may run what, and the role checks back it
up. Claims are atomic and their history is recorded. A shared resource has one live holder.
Closing a bead needs a green, main merged, a claim, and a committed digest. A verification run
is recorded honestly, with its duration, output, dirty tree, and any change to its command. Air
records who edited what. It pushes each attention condition once, and again only when it
changes. The owner's decisions are beads with the owner label. Main only moves onto a verified
tree.

The agents decide everything else. A capture might become a bead or might not. Agents choose
which bead to take and which resources count as shared. They judge whether a diff is good and
whether a verification covers enough. They decide how to split a file, what to do about a
condition, and when to land. The owner makes the owner's decisions.

Some things were considered and deliberately left out, with the rulings in the decisions log:

- a cap on work in progress, which is measured but never enforced
- a stored phase for epics
- automatic retries of a red verification
- headless or looping workers
- claims inferred by watching beads
- integration branches per epic, convoys, formulas, molecules, and a Gas Town style merge queue
- a coded phase machine, a spec and plan and tasks document set, and an AI triage daemon
- automatic prioritisation, timer or GitHub gates, and automatic re-splitting of work

Some commands were specified and never built, because no recorded problem called for them: air
next, air peer, merge advice, round metrics, a re-injection before compaction, and a topology
file. The outside projects considered and not adopted, and the choices of language, stores, and
harness, are in section 11.

## 9. Operations

Install Air with cargo from this repository. In a new repository, run air init. In one that
already uses beads, run air install. Both show their changes first and write only when told to.
Air doctor exits cleanly when the ledger, the schema, and the beads version are right.

Start the coordinator with air coordinator. From that session, start each worker with air worker
and a task. The launcher prints the tmux command to attach to it. Air status is the screen to
watch, and the channel brings the attention conditions to the coordinator.

Worktrees isolate the branch, not the machine. Cargo's build.jobs defaults to every logical core
per invocation, so two builds in two worktrees each ask for the whole machine; an adopter
measured a 15 second cargo check taking 220 seconds with the load average at 207 on 16 cores.
Air runs no build pool. A repository with heavy builds gives each worktree its own
CARGO_TARGET_DIR and a CARGO_BUILD_JOBS share of the cores through the launch's settings, and a
session reads uptime before it diagnoses a slow test. (From the worktree protocol, retired
2026-09-25.)

The verify target checks formatting, runs clippy and the tests, runs the adopter check, and runs
the self-test against the binary it just built. Record a run with air record. A release adds one
row to the release list and sets the same version in the Cargo manifest. Then the release target
refuses a dirty tree or any branch but main, runs the release check and the verify target, and
tags. Rows are only ever added. The last release row is 0.3.5, and four notices are waiting for
the next one.

In a repository, Air writes three tracked files: the Claude Code settings, the channel server
entry, and the role prose. Everything else it writes is ignored by git. That includes the full
text of every agent message since 2026-09-05 and the shell commands in the event stream. Air runs
git, beads, tmux, ps, Claude Code, and the repository's verify command. It contains no network
code.

## 10. TODO

One line per item, with the date it was added and where the detail lives. An item gets a bead id
when it is filed and is deleted when it lands. Rulings go in the decisions log.

Fleet shape, added 2026-09-14. This is the target the owner ruled on 2026-09-14 (every role in
a worktree, the verification lane first-class) and revised 2026-09-25 against an adopter's fleet
protocol. It replaces plan 0009, retired 2026-09-25; what that plan proposed and is already
built is described in sections 4 and 6.

- [ ] Describe the fleet everywhere as three workers, a verification lane, and the coordinator,
      in every document, prompt, and status line.
- [ ] Air creates every tmux session, including the coordinator's. Today only the worker
      launcher does. `claude --tmux` still needs `--worktree` (2.1.272, 2026-09-14), so Air's
      detached start stays.
- [ ] The coordinator works in its own worktree on its own branch, and nobody works in the main
      checkout. Its prose and its fixes reach main through the lane like a worker's branch.
      Its permissions no longer depend on the directory, so this is a launcher change. Then
      status warns when any session's working directory is the main checkout. The worktree
      module's header still says Claude Code's worktree isolation is used, and it is not.
- [ ] The verification lane lands, and landing is refused to every other role, the coordinator
      included. The coordinator keeps filing beads, triage, and launching.
- [ ] A launch command for the verification lane (air lane, or air worker with a lane role),
      alongside the worker and coordinator launchers. It sets AIR_ROLE to lane, the lane's deny
      list, and the repository's verify scope, so the lane can land and nothing else can. Once
      it exists, the stop nudge stops reading `verify_lane` to skip the lane.
- [ ] Record the failing step on every red run. An adopter's ledger has 142 reds with none, so
      the ledger cannot say why a batch went red.
- [ ] Measure how long branches wait to be batched, and show the count and the oldest wait in
      status. At an adopter's mean verify of 1,131 seconds (2026-09-07) the lane cuts about
      three batches an hour. Removed when a round shows a median wait under one verify.
- [ ] Red-batch policy. A red batch lands nothing. The lane drops the member whose changed paths
      the failing step names, and otherwise halves the batch, oldest half first; a suspected
      flake gets one re-run of the same head, never more. Record "retried once at this commit"
      so a second red reads as a second red. Recommended shape: a program the lane calls
      (air batch next) for the split, with the lane reading the log and making the flake call.
- [ ] Record batch drops in the ledger once something reads them back. Today air batch cut
      writes each drop as an event line.
- [x] The verification lane key in the config is read by no code. Either read it or remove it.
      The stop nudge reads it, to leave the lane alone (2026-09-25).
- [x] Prove with a test that a bead can close after its commits land on main, then drop the
      adopter's rule to wait for every close before landing. Settled 2026-09-25:
      `a_bead_already_in_main_closes_on_its_landing` in crates/cli/tests/claim_cli.rs, and the
      adopter removed its rule the same day.

Owner rulings still open on the fleet shape, each with the recommendation:

- [ ] Does the coordinator keep air close? Yes, with its use counted: it recovers a bead whose
      worker is gone, and it is how the adopter recovered six stranded beads on 2026-09-07.
- [ ] Red batches: drop by the failing step, or always bisect? Drop by step, and bisect when the
      step names nothing. With three members a bisect costs at most two extra verifies.
- [ ] Does Air own session respawn, which an adopter runs as a keepalive loop? Not yet: it needs
      its own check-resources pass, and the harness gives no exit signal to hook.

- [ ] Rewrite the roles text for the five-session fleet, along with the launcher change. It
      ships to adopters, so it needs a notice. Partly done 2026-09-25: the roles text now
      carries the whole protocol (closing, the lane's loop, landing), with notices; the
      coordinator-in-a-worktree and lane-lands parts wait for the launcher.
- [ ] Keep the adopting guide current and condense it lightly.
- [ ] The fleet starts from the coordinator. The owner starts only the coordinator and asks it to
      set up the fleet, which by default starts three workers and a verification lane, each in
      its own worktree and tmux session. The same launches stay available as air commands the
      owner can run by hand. (Owner, 2026-09-14.)
- [ ] A launch command for the verification lane, alongside the worker and coordinator
      launchers. It sets the lane's role, so the lane can land and nothing else can.
- [ ] Update the README's quick start once the coordinator sets up the default fleet on request.
- [ ] On a fresh repository, air init proposes Metis as on and then warns that Metis is not
      installed. Default it to off unless Metis is found. (Seen running the quick start,
      2026-09-14.)
- [ ] On a fresh repository, air doctor reports two expired dated rules and says to delete their
      fallbacks. Delete them, since both expired on 2026-08-23. (Same run.)

- [ ] A verify whose tracked tree or HEAD changed between start and exit records a green for a
      tree that never existed; `air record` checks the tree only at the start. Flag it the way
      a dirty start is flagged, as a measurement, not a refusal. An adopter's worker edited a
      tracked file during its own precheck an hour after citing the rule (2026-09-07).

Surface audit, added 2026-09-14:

- [ ] Audit every command and channel tool. For each, record what it is for, the incident it
      answers, whether something else already does it, and whether it stays. Usage is a signal,
      not the verdict. Use the adopter's counts in section 4.1, never this repository's.
      Leases are unused here and used by the adopter every round, which is the kind of thing a
      count alone gets wrong.
- [ ] Cut the source comments down to what a maintainer needs: what a piece is responsible for,
      its invariants, and its contract. Incident stories move to the decisions log. Go crate by
      crate and run verify after each. The self-test file alone is 12,899 lines.
- [ ] Settle the digest directory. This repository does not set it, so the digest check is off
      here, while CLAUDE.md says it is enforced.
- [ ] Still owed from 2026-08-29: the do-less questions answered for each mechanism, the
      failure-direction question for each place Air parses text, a sentence-level pass over
      the roles text, and the multi-agent question once coordination traffic is measured.

Docs:

- [ ] Rewrite the system design skill in this document's voice. Its output is now lines in this
      section and rulings in the decisions log, not a plan file.
- [ ] Refresh the Claude Code facts. The inventory is from version 2.1.241, and 2.1.272 is
      installed. (`.claude/skills/check-resources/references/harness-facts.md`, 2026-09-14.)
- [ ] Refresh the orchestrator roster; it was due 2026-09-24.
      (`.claude/skills/check-resources/references/field.md`, 2026-09-25.)
- [ ] When bd 1.3.0 leaves release candidate, ask whether bd serve removes enough of the
      per-process cost to be worth a daemon, and whether upstream compare-and-set lets the
      claims table stop being the authority. (Section 11, 2026-09-25.)
- [ ] Turn the two findings indexes about the adopter into beads.

## 11. Technology decisions

Each choice, why it was made, and where the evidence is. A URL carries the date it was read.
The research documents these lines came from were retired on 2026-09-25; what an agent needs at
work time moved into skill references (`.claude/skills/check-resources/references/`,
`.claude/skills/beads/references/bd-facts.md`, `.claude/skills/do-less/references/evidence.md`,
`.claude/skills/system-design/references/verification-lane.md`). Rulings and their dates are in
the decisions log.

| Decision | Why | Source |
|---|---|---|
| Rust, one binary, no async runtime. Edition 2024 on stable, one error enum per crate, no anyhow in libraries, tests on in-memory SQLite and temporary git repositories with no sleeps and no network. | Air is installed into other repositories, so it ships as a binary and a target repo never depends on Air's build. The hook starts on every tool call and has a tenth of a second to answer. | CLAUDE.md, "Rust" and "Tests"; decisions log, 2026-08-18; section 6.7 |
| One SQLite file in WAL mode for current state, beside one NDJSON event file per day for history, both in the main checkout's .air and shared by every worktree. No daemon, no server. | One machine and many short-lived writer processes. With six writers the lock waited at most 131 ms (2026-09-06). Rows hold only what git and beads cannot rebuild, and nothing expires on a timer. | crates/ledger; section 6.7; decisions log, 2026-08-17 |
| beads (bd) is the task store, pinned at 1.2.2 and checked by air doctor. Air calls it only as `bd --json` behind one trait, never from a hook. | It supplies the one thing Air cannot compute cheaply, a dependency-aware ready list, and an atomic claim. 1.2.0 and 1.2.1 were published by accident and 1.2.2 re-released the tested 1.1 code, so the pin is the only line upstream stands behind; Air's ledger does the compare-and-set that 1.2.2 lacks. A bd process costs about 1.4 s (median over 751,673 processes, 2026-09-06). | https://github.com/steveyegge/beads/blob/main/docs/recovery/accidental-1-2-1-release.md (2026-08-17); crates/cli/src/cmd/doctor.rs:13; crates/bd/src/lib.rs; `.claude/skills/beads/references/bd-facts.md` |
| Stay on bd rather than switch to beads_rust or a table of Air's own. Watch bd 1.3.0. | beads_rust is store-incompatible with bd and ships no library, so Air would still pay per process. A table of Air's own means building a work tracker, and the ledger records no failure caused by bd. bd 1.3.0-rc.1 (2026-08-31) adds leases, compare-and-set, and bd serve; re-ask when it is stable. | https://github.com/Dicklesworthstone/beads_rust (2026-09-06); https://api.github.com/repos/steveyegge/beads/releases/tags/v1.3.0-rc.1 (2026-09-06) |
| Gas Town is prior art, not a runtime. | It supervises with LLM agents, costs about $100 an hour, and was seen merging a pull request over failing integration tests. Air copied its batch-then-bisect queue and its rule to cross-check a heartbeat against the real process before calling anything stuck. | https://www.dolthub.com/blog/2026-01-15-a-day-in-gas-town/ (2026-08-17); https://github.com/steveyegge/gastown (2026-08-17) |
| Claude Code is the harness. | Its deny rules hold in every permission mode, including bypass. Its PreToolUse hook sees a close before it runs, and exit 2 is the one outcome nothing overrides. Sessions stay interactive terminals the owner can watch. The gate itself ports to any harness with a pre-tool hook, such as OpenCode's tool.execute.before; Codex was not checked. | https://code.claude.com/docs/en/permission-modes (2026-09-05); https://code.claude.com/docs/en/hooks.md (2026-08-17); https://opencode.ai/docs/plugins/ (2026-08-24) |
| Keep Air rather than adopt an orchestrator. | Of 186 rostered orchestrators read one by one, eleven overlap part of Air, and none refuses to close a tracked work item without a recorded green at a commit containing main, in a session a person is watching. The closest (tutti, orc, loki-mode) had one to six contributors or a BUSL licence. herdr, scion, tutti and loki-mode were declined on 2026-08-29. Spawning, isolating and watching sessions are commodity. | https://github.com/andyrewlee/awesome-agent-orchestrators (snapshot 2026-08-24); decisions log, 2026-08-29; `.claude/skills/check-resources/references/field.md` |
| Not adopted on 2026-08-17: the agent client protocol, Symphony's dispatcher, Vibe Kanban's executors, pueue, Codex's app server, and an Air daemon. | Air drives no agent process and runs no daemon. Workers are interactive Claude Code sessions, and the gate needs a hook and a bead id, not a wire protocol or a job queue. | decisions log, 2026-08-17 and 2026-08-20 |
| One git worktree per worker, made by Air, with Air's hook as the edit fence. | Enforced partitions beat declared ones: on PaperBench a single agent scored 57.2, prompt-declared isolation 55.5, worktrees 63.3; four agents on distinct files passed 92.1% against 44.3% for eight on one file. Air makes the worktree itself so it can copy ignored files in and fence edits by role. | https://arxiv.org/abs/2603.21489 (2026-08-21); `air worker --help` |
| Detached tmux sessions named for the project and the worker. | Every session must stay attachable by the owner. The only launch need with no first-party equivalent was a detached start for a session without a terminal. | CLAUDE.md, "A human is always in the loop"; `.claude/skills/check-resources/references/harness-facts.md` §1.3 (2026-08-24) |
| One stdio MCP server as the coordinator's channel, polling the ledger every 30 seconds; its tools run the CLI with JSON output. | It lives and dies with the coordinator's session, so there is no daemon, and the channel and the CLI cannot disagree. Agent-facing operations stay CLI-first: MCP and CLI runs failed equally often, but MCP failures wasted 12.9% of spend against 2.2%. | crates/cli/src/cmd/mcp.rs; https://arxiv.org/abs/2608.08654 (2026-08-18) |
| The verification lane is a merge queue: batch, one verify at a time, main moves only by fast-forward onto a verified tree, a conflict goes back to its author, a red batch is split. No speculation, no rebase. | Every merge queue with a slow verify batches and splits on red, and all but Google's TAP keep main on a tested tree. On one machine the verify is the CPU, so parallel speculation is contention. Greens are keyed by commit, and a rebase gives every commit a new sha. | https://raw.githubusercontent.com/bors-ng/bors-ng/master/README.md, https://zuul-ci.org/docs/zuul/latest/gating.html, the GitHub merge queue documentation (all 2026-09-14); `.claude/skills/system-design/references/verification-lane.md` |
| git merge-tree --write-tree for the conflict check before a cut; git 2.38 or later. | It merges without touching the index or the working tree, so every conflict with main or between members is known before the lane's branch changes. | https://git-scm.com/docs/git-merge-tree (2026-09-14); crates/cli/src/cmd/batch_cut.rs |
| A few sessions, not many: three workers, a lane, and the coordinator. | Returns thin beyond three or four agents and turn negative where one agent already succeeds often. Subscription usage pools across every session on the account, with no published concurrency limit, so five sessions is an estimate to measure, not a quota. | https://arxiv.org/abs/2512.08296 (2026-08-18); https://code.claude.com/docs/en/costs.md (2026-08-17) |
| The one refusal is an external recorded check, not the agent's report. | Models still claim success they did not achieve at every capability measured; an anti-cheating prompt lowers the rate and leaves it high (Opus 4.5, 55% to 35%). | https://assets.anthropic.com/m/64823ba7485345a7/Claude-Opus-4-5-System-Card.pdf (2026-08-18); https://arxiv.org/abs/2605.21384 (2026-08-21) |
| Facts and refusals over procedure prose, each mechanism with a removal condition. | Context files do not raise task success and cost over 20% more inference. Harness pieces that encode what a model cannot do get removed on the next model; the piece that survived was verification. | https://arxiv.org/abs/2602.11988, https://www.anthropic.com/engineering/harness-design-long-running-apps (both 2026-08-21); `.claude/skills/do-less/references/evidence.md` |
| Metis is attached to the coordinator for planning; Air stores no phase. | Metis enforces only forward-only phase moves on its own documents; its exit-criteria check returns a hard-coded false, and nothing reads its complexity estimate. Air borrows the decomposition reasoning into the decomposition skills and derives an epic's state instead of storing it. | metis at 6745810, `crates/metis-docs-core/src/domain/documents/types.rs:188-238` and `vision/mod.rs:303-308` (2026-08-17); decisions log, 2026-09-05 |

## 12. Glossary

| Term | Meaning |
|---|---|
| bead | One task in beads, the task store. |
| claim | The ledger's record that a worker holds a bead. |
| coordinator | The session that triages, files, prioritises, and lands. |
| worker | A session in its own worktree that claims, implements, and closes beads. |
| verification lane | The session that verifies batches of the workers' branches. |
| owner | The person running the fleet, and any shell Air did not start. |
| role | The value of AIR_ROLE, set by the launcher. It decides what a session may do. |
| worktree | A separate checkout of the repository on its own branch, one per worker. |
| main checkout | The repository's own directory, on main, where the ledger lives. |
| green | A successful recorded verification run at a commit, a tree, or a batch. |
| batch ready | A branch that names a bead its worker holds and is not landable on its own; where the repo declares `precheck`, it also has a green precheck at its head. |
| landing | Moving main forward onto a verified tree. |
| digest | A worker's committed note for a bead, naming the bead. |
| capture | One item in the coordinator's inbox. |
| lease | Exclusive use of a named shared resource. |
| condition | One of the nine attention conditions. |
| notice | An entry telling adopting repositories what changed. |
