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

## 5. The entry in `.claude/air.json`

One entry per session, keyed by the session's name. `exec` is the argv prefix that runs a
command on the machine with stdin and stdout attached; `path` is the repository directory
there. The `ssh` short form expands to the prefix `ssh <user>@<host>`:

```json
{"targets": {"lane": {"exec": ["kubectl", "exec", "-i", "<pod-or-deploy/name>", "-c", "<container>", "-n", "<namespace>", "--"],
                      "path": "/home/<user>/air/<project>"}}}
```

```json
{"targets": {"lane": {"kind": "ssh", "host": "<host>", "user": "<user>", "path": "/home/<user>/air/<project>"}}}
```

Three things to know before committing it. `.claude/air.json` is tracked, so every word in the
prefix goes into the project's history: name a deployment rather than a pod if the pod's name
changes, and keep a kubeconfig path or a user name out of it if that bothers you. The prefix
runs with the session's environment, which a launched session keeps small, so anything the
tool needs (`--kubeconfig <file>`, a `KUBECONFIG` the shell would have set) goes in the prefix.
And `kubectl exec` runs `sh -c` with the container's PATH, not a login shell's: a toolchain
installed into a home directory is not on it, and the prefix ends with
`"env", "PATH=/home/<user>/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"`
to put it there (`env` execs the `sh -c` and the push's `git-receive-pack` alike). The entry is
read from the main checkout's copy of the file, so it takes effect when the commit that carries
it lands, not when it is written in a worktree.

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
