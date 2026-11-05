# Verifying on another machine

A setup guide: from "the lane should run the verify somewhere else" to the first green there.
The reference for every key is the `targets` row in [`adopting-air.md`](adopting-air.md); this
page is the order to do things in, with one command per step. It names no machine of ours:
the worked examples are a generic `kubectl exec` target and a generic `ssh` one.

## 1. What a target is

A target is a machine one session owns, named in `.claude/air.json` under `"targets"`. `air run
-- <command>` and `air record <kind> -- <command>` push that session's commit into a clone on
the machine, run the line there under `sh -c`, stream the output back, and record where it
ran (hostname, architecture, core count, CPU) beside the commit. Nothing else moves: no remote
in `.git/config` is written or read, nothing comes back without `--pull`, and a green is
recorded at the local commit exactly as a local run's is, so the close gate and the landing
never know where the check ran (`crates/cli/src/cmd/run.rs`, the module doc).

## 2. Which session gets the target

Give it to the lane. Under `"verify_lane": true` the lane is the only session that runs the
verify, so one entry, `targets.lane`, moves every batch's check; a worker runs nothing it
would need a machine for.

The fact behind that, stated plainly: every Bash line a launched session runs in auto mode
goes through Claude Code's permission classifier, which reads the conversation as context and
refuses a line it judges to be a remote shell write, unless an allow rule on the session's
settings matches the bare line first. On 2026-11-04 a worker's direct `kubectl exec` was
refused (air-kzt); on 2026-11-05 a worker's bare `air run -- true` against a pod was refused
twice, the second time with no reason given (air-tdh). The lane never met the classifier,
because its `air record` line is on its allow list
(`crates/cli/src/cmd/launch.rs:417` `lane_allow`, from air-8cdh and air-b1h8). Since
air-nyi (2026-11-05) `air worker` passes the same kind of rule for `air run *` and `air record
*` (`launch.rs:453` `worker_allow`), so a worker's bare line resolves before the classifier
too; a session launched before that needs a relaunch to have it. Two rules follow from the
mechanism: the line must be bare, since a pipe or redirect takes the whole line back to the
classifier (air-b1h8); and a session Air did not launch has no such rule, so a target for it
works only from a shell with no classifier, or with an exact allow rule in the project's
`.claude/settings.json`, which this guide does not recommend.

A coordinator that deploys or resizes the machine itself needs the deploy commands in
`coordinator_allow` in `.claude/air.json` (the table in
[`adopting-air.md`](adopting-air.md)), for the same reason: its `gh workflow run` and
`kubectl` lines were refused by the classifier on 2026-11-04 and 2026-11-05 until the owner
typed them (air-xji). The patterns are the adopter's own, since Air names no provider, and a
session launched before the key was set needs a relaunch.

## 3. What the machine needs

Everything the verify spawns, on the PATH a non-login `sh -c` sees: git, sh, tar, the
project's toolchain, and whatever the check itself runs. For this repository that is cargo
with clippy and rustfmt, make, tmux, ps, kill and perl. Check it in one read-only line from
the session that will own the target, once step 5 is in place, or by hand through the same
prefix before that:

```
air run -- sh -c 'for t in git sh tar make cargo; do command -v $t >/dev/null || echo "MISSING: $t"; done; echo checked'
```

If a tool is on PATH in a login shell but missing here, the prefix has to set PATH itself; see
step 5.

## 4. Gitignored files the verify reads

A push carries commits only, so a gitignored file the check reads is absent on the machine.
Copy it once into the checkout path there; a detached checkout leaves an ignored untracked
file alone, so one copy survives every run. For this repository that file is
`private/adopters.md`, which `air adopter-check` refuses to run without (air-jsz).

```
tar -cf - private/adopters.md | kubectl exec -i <pod> -c <container> -n <namespace> -- sh -c 'mkdir -p <path> && cd <path> && tar -xf -'
tar -cf - private/adopters.md | ssh <user>@<host> 'mkdir -p <path> && cd <path> && tar -xf -'
```

Credentials, kubeconfigs and keys never go: the machine needs the repository's tree and the
names the check reads, nothing that authenticates.

## 5. The entry in `.claude/air.local.json`

One entry per session, keyed by the session's name. `exec` is the argv prefix that runs a
command on the machine with stdin and stdout attached; `path` is the repository directory
there. The `ssh` short form expands to the prefix `ssh <user>@<host>`. The entry normally
lives in `.claude/air.local.json`, which is gitignored and whose top-level keys replace the
committed `.claude/air.json`'s keys of the same name (adopting-air.md, "Configure"): a
machine's name, the kubeconfig that reaches it and the user's home path then stay out of the
project's history, and a change takes effect when the file is written, not when a landing
carries it. The committed file keeps only what Air does, with `targets` absent or a
placeholder such as the first block below for a reader, and each laptop's own file carries the
real entries:

```json
{"targets": {"lane": {"exec": ["kubectl", "exec", "-i", "<pod-or-deploy/name>", "-c", "<container>", "-n", "<namespace>", "--"],
                      "path": "/home/<user>/air/<project>"}}}
```

```json
{"targets": {"lane": {"kind": "ssh", "host": "<host>", "user": "<user>", "path": "/home/<user>/air/<project>"}}}
```

Three things to know before writing it. Both files are read from the main checkout, so the
overlay is written there, never in a worktree, and `air status` run anywhere shows what it
set; if you do commit an entry in `.claude/air.json` instead, every word of the prefix goes
into the project's history, it takes effect only when the lane lands it after verifying that
batch on the old target, and a pod name that changes at every redeploy is better named as a
deployment or resolved when the command runs (step 9). The prefix runs with the session's
environment, which a launched session keeps small, so anything the tool needs
(`--kubeconfig <file>`, a `KUBECONFIG` the shell would have set) goes in the prefix. And
`kubectl exec` runs `sh -c` with the container's PATH, not a login shell's: a toolchain
installed into a home directory is not on it, and the prefix ends with
`"env", "PATH=/home/<user>/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"`
to put it there (`env` execs the `sh -c` and the push's `git-receive-pack` alike).

## 6. The first run

The lane's first batch after the entry lands is cold: the toolchain may download, the
dependencies compile from nothing, and the run's duration is that build plus the check. On a
96-core machine this repository's cold compile took about 71 s and the warm verify 46 s
against 118 s on the laptop (air-tdh, 2026-11-05). The row records where it ran:

```
air status --json        # the verify_runs row carries hostname, arch, nproc and cpu
```

When the first run is red for a missing tool rather than a failing check, the lane keeps the
fleet moving with the local form, and the tool is fixed on the machine or in the prefix:

```
air record verify --local -- make verify
```

A red whose cause is the machine, not the tree, is still recorded as a red at that commit, so
read the kept output before treating it as one. Expect the first run on a second operating
system to find tests that only ever ran on the first: this repository's first Linux verify
found five such defects in an hour (air-dj7, air-6kt, air-xiu, air-3sh, air-c14), each fixed on
its own bead while the lane recorded locally.

## 7. When the machine is gone

A target that cannot be reached (scaled down, renamed, the network off) fails the first step
before anything runs, and that is no verdict: `air run` prints `init failed (exit <n>)` with
the transport's exit, `air record` prints `NOTHING was recorded: the target could not take
<sha>`, and no row is written (`crates/cli/tests/run_cli.rs`,
`a_transport_that_fails_before_the_command_names_the_step_and_records_no_green`, air-tfi). The
lane continues with `air record verify --local -- <command>` until the machine is back. As of
2026-11-05 the test is the proof: the live case had not yet happened.

## 8. Keeping the machine alive

That is the adopter's job, not Air's: reservations, idle reapers and redeploys belong to
whoever runs the machine, and Air records what it finds when it runs. One line, no mechanism.

## 9. One machine per session

Once workers run `air run` while the lane verifies, one machine is one CPU budget shared by
all of them, and the rule is one machine per launched session, the coordinator included, since
the owner runs tests from that session too. Nothing in Air changes for this: `targets` takes
one entry per session name, each a prefix and a path, and a session with no entry keeps
running locally.

Machines reachable by ssh, a cloud VM each from any provider, are the simple case: one
short-form entry per session, each with its own path, and no resolver at all. Each machine
needs what step 3 lists and each path the gitignored files of step 4.

```json
{"targets": {"coordinator": {"kind": "ssh", "host": "<host-1>", "user": "<user>", "path": "/home/<user>/air/coordinator"},
             "lane":        {"kind": "ssh", "host": "<host-2>", "user": "<user>", "path": "/home/<user>/air/lane"},
             "worker-1":    {"kind": "ssh", "host": "<host-3>", "user": "<user>", "path": "/home/<user>/air/worker-1"},
             "worker-2":    {"kind": "ssh", "host": "<host-4>", "user": "<user>", "path": "/home/<user>/air/worker-2"},
             "worker-3":    {"kind": "ssh", "host": "<host-5>", "user": "<user>", "path": "/home/<user>/air/worker-3"}}}
```

Replicas of one deployment are the special case, and three facts shape the entries. Every
replica mounts the same home volume, so the user's home is one directory from every pod, and
what separates the sessions is the `path`: one directory each, with its own build directory,
while the toolchain in the home is installed once for all of them. `kubectl exec deploy/<name>`
resolves to one pod of the deployment, the same one every time, so with that prefix every
session lands on one replica and the others sit idle. And a pod's name changes at every
redeploy, so the pod for a session cannot be written into the file; it has to be resolved when
the command runs. This repository resolves it with `scripts/kube-exec-nth`, which reads the
deployment's selector, lists its Running pods sorted by name, and runs the command on the Nth:
the coordinator is 1, the lane 2 and worker-N is N+2, so every session sees the same order
while the pod set is unchanged. The entry is the script, its arguments, and the `env PATH=` tail of step 5; the
push and the command both go through it, since git's `ext::` helper runs the prefix as argv.

```json
{"targets": {"worker-1": {"exec": ["<repo>/scripts/kube-exec-nth", "<kubeconfig>", "<namespace>", "<deployment>", "<container>",
                                   "3", "worker-1",
                                   "env", "PATH=/home/<user>/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"],
                          "path": "/home/<user>/air/worker-1"}}}
```

A session whose replica does not exist fails before anything runs, with one line naming the
session and the count, and never falls through to another session's pod; `air run` reports it
as the init step failing (step 7), and the lane records locally until the replicas are back.

```
kube-exec-nth: session worker-3 wants replica 5 of <deployment>; Running pods: 1
```

The script is this repository's configuration and not part of Air: `air install` does not ship
it, and its test runs it against a fake `kubectl` (`crates/cli/tests/kube_exec_nth.rs`). A
path that already holds a warm build may keep its name when the entries are added; this
repository's lane kept the path it verified at before, with the step 4 file already in it.
Switching from VMs to replicas or back, or adding a session, edits `targets` only: the session
names, the path convention and the proof stay the same, and the proof is a `runs` or
`verify_runs` row whose hostname differs per session.
