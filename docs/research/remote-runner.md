# Is `air run` the idiomatic, minimal way to give a session a machine?

Research for air-9ao, 2026-11-06, read against this tree at `53327f6` (the running binary is
not what was read; every count below is a query or a `grep` over this tree, named beside it).
The question, from the owner on 2026-11-05: the AI should be free to work with a pod as it
would like to, and the models are getting better; so is the remote runner (`air run`,
`targets`, the push, checkout, run and pull cycle, one pod per session) the right shape, does
it work on any VM, and is it doing more than it should?

The short answer: nobody in the field ships a local agent's commits and commands to a remote
machine. Every product that gives an agent a machine puts the agent's process on that machine
and lets it work in a clone it owns; the one thing that keeps the shape Air has is continuous
integration, which pushes a commit, runs a check there and records the result against that
commit. So `air run` is the CI shape, and that is the right shape for the lane's verify: the
green must belong to a local commit the close gate and the landing can see. For a worker it is
the wrong shape, and the two trial defects are what managing a working tree from another
machine costs. The recommendation is to keep the runner as the lane's verify transport, fix the
two defects because they are one and two lines and the lane's path shares the code, and stop
extending the runner for workers; when a worker needs a machine, the session goes to the
machine (shape b), which Claude Code now supplies for a Kubernetes runner in beta and which Air
can try with no code change by running a whole fleet on one pod.

## 1. What already exists

### 1.1 Claude Code

Claude Code has three features in this area, and none of them syncs files to a machine or
runs a local session's commands remotely. All files below were downloaded from
`code.claude.com/docs/en/` on 2026-11-06.

- Cloud sessions. "A cloud session is a Claude Code session that runs on cloud infrastructure
  instead of on your machine" (`claude-code-on-the-web.md`). "Each session gets a fresh virtual
  machine (VM) running Ubuntu 24.04 on x86_64 ... with your repository cloned"; "You don't get
  a shell into the session VM. Claude runs every command for you" (`cloud-environments.md`).
  The terminal starts one with `claude --cloud "<task>"`, which "clones your current
  directory's GitHub remote at your current branch, not your local checkout", and pulls one
  back with `claude --teleport <session-id>`; "session handoff is one-way". Results come back
  as a pushed branch.
- Self-hosted environments, public beta on Team and Enterprise plans, off by default. "A
  self-hosted environment executes Claude Code cloud sessions on infrastructure your
  organization operates." Its parts: an environment, a "Runner: a program running on hosts
  inside your network. Runners execute the sessions; the idea is the same as a self-hosted CI
  runner", and a session. "A runner claims it, clones the repository the developer chose, and
  starts a Claude Code process on your host to run it"; "no inbound connectivity from
  Anthropic is required"; the deploy page has Kubernetes and Compose recipes
  (`self-hosted-environments.md`). This is shape (b) below, supplied by the harness, with
  Anthropic's control plane doing the queueing.
- Remote Control is the other direction: it connects the phone or the web app "to a Claude
  Code session running on your machine ... Claude keeps running locally the entire time"
  (`remote-control.md`).
- Dev containers: "Claude Code runs inside the container, so it sees the same files,
  dependencies, and tools as the rest of your project's toolchain", and "you can pass
  `--dangerously-skip-permissions` for unattended operation" (`devcontainer.md`). The Bash
  sandbox is a local OS boundary, Seatbelt on macOS and bubblewrap on Linux, not a remote
  machine (`sandboxing.md`). The sandbox page notes that plain `ssh`, `scp` and `rsync` over
  SSH fail inside it.

### 1.2 Editors, dev containers and sync

- VS Code Remote - SSH: "the extension runs commands and other extensions directly on the
  remote machine"; "No source code needs to be on your local machine to get these benefits"
  (https://code.visualstudio.com/docs/remote/ssh and `remote-overview`, 2026-11-06). The
  editor's UI is local, the server, the terminals and the code are on the host.
- The Development Container Specification: "An open specification for enriching containers
  with development specific content and settings" (https://containers.dev, 2026-11-06); the
  supporting-tools page names Codespaces, VS Code, DevPod, Gitpod, JetBrains and the
  reference CLI. The container is where the tools run; the editor attaches to it.
- Mutagen synchronises a directory "between your laptop and a remote container in effective
  real-time", with transports Local, SSH and Docker only, and modes from `two-way-safe` to
  `one-way-replica`; a conflict in the safe modes is resolved "by deleting the content on the
  endpoint which you wish to have lose the conflict"
  (https://mutagen.io/documentation/synchronization and `/transports`, 2026-11-06). No
  Kubernetes transport.

### 1.3 Other agent products

Each one runs the agent's commands on the machine it was given, and gets the code there by a
clone; what comes back is a branch or a pull request.

- Cursor cloud agents: "Each Cloud Agent runs on its own dedicated virtual machine with a full
  development environment: your repository, dependencies, secrets, and network access"
  (https://cursor.com/help/ai-features/background-agents.md, 2026-11-06). Its self-hosted
  option is the nearest thing to what the owner asked for: "Self-Hosted Machines runs tool
  calls on hardware you control ... The agent loop still runs" in Cursor's cloud, "My
  Machines" for "a personal laptop, devbox, or remote VM" and "Team Pools" for "Kubernetes,
  autoscaling, labels, and fleet monitoring". The page names the cost of owning the machine
  in the same words as Air's trial defect: "You also own cleanup: wiping state, refreshing the
  checkout, repairing dependencies, and keeping the machine ready for the next"
  (https://cursor.com/docs/cloud-agent/self-hosted/choose-runtime.md, 2026-11-06).
- OpenAI Codex cloud: "Codex creates a container and checks out your repo at the selected
  branch or commit SHA"; the current page says "Each task has its own workspace", with an
  install script, network policy and secrets configured per environment
  (https://learn.chatgpt.com/docs/cloud and `/docs/environments/cloud-environment`,
  2026-11-06). Nothing about the customer's own machines.
- GitHub Copilot coding agent: "An environment is the place where the agent executes its work,
  such as a cloud sandbox, a GitHub Actions runner, or a remote custom environment"; the
  Actions option "Supports ... self-hosted runners"
  (https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent,
  2026-11-06).
- One container per agent, open source, as reported by a web search on 2026-11-06 and not
  read further: Container Use (each agent a fresh container on its own git branch, on Docker
  and Dagger, https://container-use.com/introduction), Sandcastle (each agent in its own
  container, "commits are patched back to the host", https://agentconn.com/agents/sandcastle),
  agentbox and demesne. The same shape: the agent lives in the container.

### 1.4 The shape Air has, elsewhere

Pushing a commit to a machine and running the tests there is a build-server pattern older
than any of the above: the Scala project's `remotetest` script "pushes the current git repo to
a remote bare repo, then checks out the source tree in a workspace repo and starts the build
and all the tests" over ssh
(https://sources.debian.org/src/scala/2.9.2%2Bdfsg-2/tools/remotetest, 2026-11-06). Every CI
system is this shape: the commit goes to the runner, the check runs there, the verdict is
recorded against the commit. The agent is not on the machine in this shape because there is
no agent; the developer is at their desk.

### 1.5 Air today

The runner is four days old: `crates/cli/src/cmd/run.rs` was first committed on 2026-11-02 and
has six commits (`git log -- crates/cli/src/cmd/run.rs`). It is 802 lines, plus the record
path in `record.rs` (537), the `runs` table (`crates/ledger/src/runs.rs`, 189), its tests
(`crates/cli/tests/run_cli.rs`, 859) and this repository's replica resolver
(`scripts/kube-exec-nth`, 58): about 2,400 lines (`wc -l`, this tree). What it did in its
four days, from the ledger (`sqlite3 ... ?immutable=1`, 2026-11-06):

| Table | Rows | Of which |
|---|---|---|
| `runs` | 11 | lane 3 on its pod; worker-3 4 and coordinator 1 through `kube-exec-nth`; worker-2 3 local. Two of the worker runs used `--pull`, both reported failed, both files arrived (air-b50). |
| `verify_runs`, kind verify, on a pod | 26 | 19 green. On 2026-11-05 a green pod verify took 51 s on average against 142 s for a green local one the same day. |

The two open defects are both the target's working tree: a file a run wrote stays untracked
there and blocks the next checkout of a commit that tracks it (air-32u), and a complete tar
stream whose sender exits 141 through `kubectl exec` is reported as failed (air-b50). The
seven-part write-ups are captures 01M6Q3X8J1VGENZD84HPBEDAZM and 01M6Q3X8M2WQXA0H2WW8FX032F.

## 2. The four shapes

| Shape | Where the model's commands run | Who owns the checkout | Prior art |
|---|---|---|---|
| (a) Air ships a commit, runs the command, pulls results (today) | the machine, one line at a time | Air (`prepare_target`) | CI runners, `remotetest` |
| (b) the session's whole Claude Code runs on the machine | the machine | the session | Claude Code self-hosted runners, dev containers, Cursor Team Pools, Codex, Copilot, Container Use |
| (c) a shared mount or synced directory | either side | both, with conflicts | Mutagen, a RWX volume |
| (d) the session uses ssh or kubectl itself; Air records | the machine, one line at a time | the session | Cursor My Machines (closest) |

What each costs, against the three questions in the bead.

(a) Air must manage the clone, the push, the checkout and whatever the last command left
there (air-32u), the quoting for a remote shell versus argv (the `shell` flag, air-tfi), the
toolchain on a non-login PATH (remote-verify.md §5), the gitignored files the check reads
(§4), the result paths coming back (`--pull`, air-b50), and the pod name that changes at every
redeploy (`kube-exec-nth`). The model is free to run one bare line and get its exit and output;
it cannot look around on the machine, keep state there, or run a pipeline without the line
going back to the classifier (air-b1h8). On any VM the transport is `ssh`, which the short
form covers, and the rest is unchanged; what breaks is nothing, since a prefix is a prefix.
What this shape buys and the others do not: the verdict is recorded at the local commit, so
the close gate and the landing never know where the check ran (`run.rs` module doc;
design.md §4 `air record`).

(b) Air manages nothing remote. Every mechanism Air has runs unchanged, because the session,
its worktree, its hook and its channel are on the machine, and the owner attaches to the pane
there (`kubectl exec -it <pod> -- tmux attach`, or ssh). The model is free to do whatever the
pod allows. What breaks: the ledger is one SQLite file "for one machine and many short-lived
writers" (design.md §11), so the fleet is one machine, not one pod per session, unless the
main checkout moves with it; SQLite itself says locking "is especially true of network
filesystems and NFS in particular ... database corruption might result"
(https://www.sqlite.org/howtocorrupt.html §2.1, 2026-11-06), so the `.air` directory stays
off the shared home. The pod's lifetime becomes the fleet's lifetime: this adopter's pods are
deleted when their reservation expires (coordinator journal, 2026-11-05T02:03Z). The session's
Claude Code needs its own login on the machine, which is its credential and not the laptop's.
And the launcher's folder-trust refusal applies there as anywhere (air-ekj).

(c) Air manages nothing, and the session edits locally while the check runs on the mount. What
breaks: a mount from a laptop into a Kubernetes pod does not exist; Mutagen has no Kubernetes
transport; and two writers on one directory is the conflict the worktree-per-session rule
exists to prevent (design.md §11, "enforced partitions beat declared ones"). Not a candidate.

(d) Air records what it is told. The model does the push, the checkout, the clean and the
copy back itself, and `air record` records the exit of the local `kubectl` line with the
laptop's hostname unless the machine is passed in. What breaks: every one of those lines is a
remote shell write to the classifier (air-kzt), an allow rule covers only a bare line
(air-b1h8), and `tar | kubectl exec` is never bare. This is the shape the owner's sentence
describes, and on this harness it is the one that stalls most.

## 3. Today's parts, by the failure each prevents

Per the do-less skill: the recorded failure, the removal condition, and whether the part is
friction the model would handle itself under (b). Sources: `run.rs` module doc and
`prepare_target` (lines 1-60 and 572-597 of this tree), design.md §4 rows `air record` and
`air run`, solved-issues.md "A worker's `air run` against a target was refused" and "A
`targets` change was a landing".

| Part | Failure it prevents | Removed when | Friction under (b)? |
|---|---|---|---|
| `targets`: a prefix per session, `ssh` short form | the adopter's pods are reachable only by `kubectl exec`, so no target could name one and runs stayed outside Air (air-tfi) | the harness or git carries a transport Air can read, or a round of `runs` rows shows only ssh | yes: the session is already there |
| init at `path`, once | the first run on a fresh machine | with (b) | yes |
| push to `refs/air/<session>` over `ext::`, forced | a number with no commit behind it (air-opq); the target holds only commits, so the green is the local commit's | the gate stops recording greens at local commits | no for the lane: this is what makes a remote green honest |
| detached checkout, plus the clean air-32u asks for | the run runs the pushed commit, not what was left there | with (b) | yes: a session cleans its own tree |
| the `sh -c` line and the `shell` flag | ssh takes one line, `kubectl exec` takes argv (air-tfi) | a harness transport | yes |
| the machine probe and the `runs` row | "no record said which machine a number came from" (air-opq) | the harness records the machine | no: the record is the point, and it costs one line |
| `--pull` as one tar stream | result files back without a copy per path (air-opq.4); a bare line, which a pipe is not (air-b1h8) | the classifier stops judging piped lines, or (b) | yes: `kubectl cp` or a commit |
| `air record` through the target, dirty tree refused | the verify runs where it is fast and Linux (51 s against 142 s) and is recorded at the local commit | the fleet runs on the machine | no for the lane |
| `--local`, "NOTHING was recorded" | a machine that is gone is not a red (air-tfi) | with (b) | yes |
| `worker_allow` for `air run *` and the setup guide | the classifier refused a worker's run (air-tdh, air-nyi) | the classifier stops judging launched sessions' lines | moot under (b) |
| `kube-exec-nth` (this repository's, not Air's) | `deploy/<name>` always hits one replica; pod names change | the adopter uses ssh or (b) | yes |

Six of eleven parts are friction a session on the machine would not have; the four that are
not are the push, the record, the recorded verify and the fallback, which together are the
CI shape, and all four serve the lane.

## 4. Recommendation and verdict

Keep the runner as the lane's verify transport and stop extending it for workers.

- The lane keeps `targets.lane` and `air record verify` through it. It is the CI shape, it
  recorded 26 pod verifies in two days with the close gate none the wiser, and its cost is a
  push and a checkout per batch.
- Fix both defects. air-32u is one line in `prepare_target` and the lane's checkout runs the
  same line, so a verify that writes an unignored file would hit it too; air-b50 is two lines
  in `pull::fetch`. Both are cheaper than the notice that removing `--pull` would cost, and
  neither adds a mechanism.
- Do not build more of shape (a) for workers: no result sync, no per-session state on the
  target, no second resolver. A worker that needs a machine today runs one bare `air run` line
  for a Linux-only or heavy command, which is what the allow rule covers, and nothing more.
- The shape to try for workers is (b), and it needs no Air change to try: start one fleet on
  one pod, with the main checkout on the pod's own disk and the owner attaching through
  `kubectl exec -it`. One round there answers whether the pod's lifetime and the login are
  acceptable costs. If they are, `targets` becomes the lane's alone or goes, and the removal
  conditions in §3 are met for six parts at once. The harness's own self-hosted runner is
  shape (b) with Anthropic's queue in front of it; it is in beta, Team and Enterprise only, and
  it would replace the launchers rather than the runner, so it is noted and not recommended
  here.

The do-less answers: the recorded failures are air-tfi, air-opq and the two trial defects; the
runner supplies facts (where a command ran, at which commit) and no judgement; each part's
removal condition is in §3; nothing is added by this research.
