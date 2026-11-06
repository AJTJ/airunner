# Pulling result files back over an exec transport

Research for air-uyn, 2026-11-06, read against this tree at `53327f6`. Why: in the pod trial
(air-fzl) every `air run --pull` through `kubectl exec` printed "the tar stream failed (target
failed, unpack exit 0)" and recorded nothing pulled, although the file had arrived whole
(capture 01M6Q3X8M2WQXA0H2WW8FX032F, bug air-b50). The question is how established tools carry
files back over an exec transport and judge success, so that air-b50 is fixed the idiomatic
way. Research only; nothing in Air changes with this document.

The short answer: the field's idiom is a tar stream whose verdict is the receiving tar's.
`kubectl cp` returns only the local unpack's error and never collects the exec's; the shell
pipeline `kubectl exec … tar cf - | tar xf -` exits with the last command's status; and GNU tar
1.24 made a writer's SIGPIPE death silent precisely so that the reader's verdict is the one
that counts. The 141 Air sees is the local transport program, not the pod's tar: once the
end-of-archive marker has arrived the local tar exits and closes the pipe, and `kubectl`, a Go
program, is terminated by SIGPIPE when it writes the rest of the 10 KiB record to a closed
stdout. One refinement to the bug's two-line fix: a sender killed by SIGPIPE after a good
unpack is the normal end, but a sender that exits with a non-zero code is still a failure,
because the local tar exits 0 on an empty stream and would otherwise hide a target-side
`tar` that could not read a file. rsync is the alternative for incremental pulls and needs
rsync on every target plus a wrapper; not worth it for result files.

## 1. What already exists

### 1.1 `kubectl cp`: a tar stream, judged by the local unpack alone

Source: `staging/src/k8s.io/kubectl/pkg/cmd/cp/cp.go` on the kubernetes master branch,
fetched 2026-11-06 (594 lines; line numbers below are from that copy,
https://raw.githubusercontent.com/kubernetes/kubernetes/master/staging/src/k8s.io/kubectl/pkg/cmd/cp/cp.go).

- What runs in the container. `initReadFrom` (cp.go:381-407) execs
  `[]string{"tar", "cf", "-", t.src.File.String()}` (cp.go:395) with the exec's stdout wired
  to one end of an in-process `io.Pipe()` (cp.go:382, 387). There is no operating-system pipe
  and no `sh -c`.
- How the stream is read. `copyFromPod` (cp.go:354-362) wraps the pipe's reader in a `TarPipe`
  and returns `o.untarAll(...)` (cp.go:361), which walks the stream with Go's `archive/tar`
  (`tar.NewReader(reader)`, cp.go:509) and returns `nil` when `Next()` reports the end of the
  archive (cp.go:574).
- How success is judged. The exec runs in a goroutine, `go func() { defer
  t.outStream.Close(); cmdutil.CheckErr(t.o.execute(options)) }()` (cp.go:403-406). Its
  error is never returned to `copyFromPod`: `CheckErr` prints it and calls `os.Exit`
  (`k8s.io/kubectl/pkg/cmd/util/helpers.go:120-122`, `fatal` at :95-108, fetched the same
  day), so an exec failure that arrives before the stream ends kills the process, and one
  that arrives after the unpack returned is never seen, because `Run` has already returned
  `copyFromPod`'s value (cp.go:257). The verdict is the unpack's and nothing else.
- Retries. `--retries` (cp.go:162) re-execs `sh -c "tar cf - '<path>' | tail -c+<n>"`
  (cp.go:398-401) from the byte after the last one received (cp.go:415), which is about a
  stream that stopped early, not about judging a complete one.
- What the container must have. The command's own help says "Requires that the 'tar' binary
  is present in your container image. If 'tar' is not present, 'kubectl cp' will fail."
  (cp.go:43-45), and for "symlinks, wildcard expansion or file mode preservation" it points at
  `kubectl exec` with `tar cf - | tar xf -` (cp.go:47-48, :551-554).

### 1.2 The shell idiom: tar piped over ssh or `kubectl exec`

The idiom `kubectl` and the GNU tar manual both point at is a sending tar on the far side piped
into a receiving tar here, `ssh host tar cf - dir | tar xf -`. Three documented facts decide
how it ends.

- The receiver stops at the marker. "Physically, an archive consists of a series of file
  entries terminated by an end-of-archive entry, which consists of two 512 blocks of zero
  bytes" (https://www.gnu.org/software/tar/manual/html_node/Standard.html, accessed
  2026-11-06), and "Normally, `tar` stops reading when it encounters a block of zeros between
  file entries (which usually indicates the end of the archive)"
  (https://www.gnu.org/software/tar/manual/html_node/Ignore-Zeros.html, same day).
- The sender writes more than the marker. "The default blocking factor is typically 20 (i.e.,
  10240 bytes)", "`tar` pads the archive out to the next record boundary"
  (https://www.gnu.org/software/tar/manual/html_node/Blocking-Factor.html, same day), and
  "When writing an archive, the last record of blocks should be written at the full size,
  with blocks after the zero block containing all zeros" (Standard.html). For a 3-byte file the
  marker ends at byte 4096 of a 10240-byte record (measured below), so the receiver can be
  done while 6 KiB is still in flight.
- A writer killed by SIGPIPE is the normal end, by GNU tar's own ruling. tar 1.23 had started
  ignoring SIGPIPE, which made `tar tf archive.tar | head -n 1` print "write error" and exit
  non-zero; 1.24 (2010-10-24) reverted it: "Spurious error diagnostics on broken pipe. When
  receiving SIGPIPE, tar would exit with error status and 'write error' diagnostics."
  (GNU tar NEWS, version 1.24 section,
  https://repo.or.cz/tar.git/blob_plain/085cace1805308589c6211429068f68047be0b0e:/NEWS,
  accessed 2026-11-06; the regression report is
  https://lists.gnu.org/archive/html/bug-tar/2010-03/msg00044.html). The exit a shell reports
  for that death is 128 plus signal 13, 141, which the 2005 bug-tar thread on tar's exit
  codes (0, 1 and 2 only) already spelled out
  (https://lists.nongnu.org/archive/html/bug-tar/2005-02/msg00022.html, same day). The
  manual's own exit codes are "0 Successful termination", "1 Some files differ", "2 Fatal
  error", and "If tar has invoked a subprocess and that subprocess exited with a nonzero exit
  code, tar exits with that code as well"
  (https://www.gnu.org/software/tar/manual/html_node/Synopsis.html, same day).

In the pipeline the shell's exit status is the last command's, the receiving tar's, unless
`pipefail` is set. That is the idiom's verdict rule, and the reason the 141 goes unnoticed
in the field.

### 1.3 Why the 141 is the transport program, not the pod's tar

Through `kubectl exec` the sending tar's stdout is the exec session, not a pipe to the laptop;
the process whose stdout is the laptop's pipe is `kubectl` itself. Go's rule for that: "A write
to a broken pipe on file descriptors 1 or 2 (standard output or standard error) will cause the
program to exit with a SIGPIPE signal" (https://pkg.go.dev/os/signal, "SIGPIPE", go1.27.3,
accessed 2026-11-06). The same holds for `ssh`, a C program with the default SIGPIPE
disposition. So for every exec transport Air supports, a receiver that closes early kills the
local sender process with signal 13, and `run.rs` reads that as exit 141 (`code()`,
`crates/cli/src/cmd/run.rs:633-646`, "a signal reads as a shell reports it, 128 plus the
signal") and as "target failed".

Measured here (this Mac, Darwin 27.2.0, bsdtar 3.5.3), with a 3-byte file whose archive is one 10240-byte
record and whose end-of-archive marker ends at byte 4096. A sender that writes the first N
blocks, pauses, then writes the rest, piped into `tar -xf -`:

| split after | sender's second write | receiver exit | file |
|---|---|---|---|
| 6 blocks (marker not yet sent) | exit 0 | 0 | intact |
| 8 blocks (marker just sent) | exit 141 | 0 | intact |
| 12 blocks | exit 141 | 0 | intact |
| 20 blocks (nothing left) | exit 0 | 0 | intact |

The same with `tar -tf -` as the receiver: 141 at 8 and 19. The capture's own direct test on
the pod agrees: the prefix running `tar -cf - -- README.md` piped into `tar -tf -` exits 141
after listing the file, and the same command with its output sent to a file exits 0. With the
whole record in one write (the local case, and `dd bs=512` through a pipe buffer) the
receiver sees everything in one read and nobody is killed, which is why the pull passed
every local test and failed on the first pod.

### 1.4 rsync over an exec prefix

rsync can run over any command: "you can specify any remote shell you like, either by using
the `-e` command line option, or by setting the `RSYNC_RSH` environment variable", and "Note
that rsync must be installed on both the source and destination machines" (rsync manual,
SETUP, https://download.samba.org/pub/rsync/rsync.1, accessed 2026-11-06; the option is
`--rsh=COMMAND, -e  specify the remote shell to use`, and `--rsync-path=PROGRAM` "specify what
program is to be run on the remote machine to start-up rsync", same page). rsync judges a
transfer by its own protocol and exit values, "23 Partial transfer due to error", "24 Partial
transfer due to vanished source files" (same page, EXIT VALUES), not by the shell's exit.

Over `kubectl exec` it needs a wrapper, because rsync runs the remote shell as
`COMMAND <host> rsync --server …` and `kubectl exec` wants `<pod> -- rsync --server …`: the
field's `krsync` script re-invokes itself as the remote shell and turns the host argument into
the pod (gist by avoidik, https://gist.github.com/avoidik/768b1ce51497a9f212ca7426e591baf9,
accessed 2026-11-06, which cites https://serverfault.com/a/887402 and
`dmrub/kube-utils/kube-rsync` as its sources; serverfault.com itself refuses automated
fetches, so the answer is cited through the gist). The maintained form is the `kubectl-rsync`
plugin (https://github.com/major1201/kubectl-rsync, README accessed 2026-11-06), which lists
what rsync adds over `kubectl cp`: incremental transfer, compression, progress, resumed
partial transfers, archive mode and `--delete`.

### 1.5 Air today

`pull::fetch` (`crates/cli/src/cmd/run.rs:375-507`) is the shell idiom with one extra rule.
It lists the matches on the target first, then runs the prefix with
`cd <path> && tar -cf - -- <paths>` and pipes that process's stdout into a local
`tar -xf - -C <repo>`; it then requires both the unpack and the sender to exit 0
(`Ok(s) if s.success() && sent`, run.rs:485), and otherwise prints the failure line and
returns an empty `Pulled`. The second half of that rule is what no established tool has: a
rule that the sending side's exit must be 0 after the receiver has finished.

## 2. What the target must have

| Way | On the target | On the laptop | Judged by |
|---|---|---|---|
| `kubectl cp` | `tar` (cp.go:43-45) | nothing beyond kubectl | the local unpack (Go `archive/tar`); exec errors print and exit, never return |
| tar over the prefix (the idiom; Air today) | `sh` and a `tar` that writes to stdout (GNU tar on the pod's Debian image; busybox tar also takes `-cf -`, not checked here) | a `tar` that reads the format (bsdtar on macOS reads GNU tar's default format, shown above) | the receiving tar; the pipeline's status is its status |
| rsync over the prefix | `rsync` (rsync.1 SETUP) plus whatever the wrapper needs | `rsync` and the wrapper | rsync's protocol and exit values |

A target is any VM or provider, not only a pod. `tar` is on every Linux image Air is likely to
meet and on macOS; `rsync` is on most but not all (a minimal container image has neither, and
`kubectl cp` fails there too, by its own help).

## 3. Options for air-b50

- A. Judge by the local unpack alone, note the sender's exit only when the unpack failed. The
  capture's two lines. What it misses: bsdtar exits 0 on an empty stream (measured: `: | tar
  -xf -` is 0), so a target-side `tar` that could not open a listed file (GNU tar exit 2 with
  an empty or short archive) would be recorded as pulled with 0 bytes. A stream cut inside a
  member still fails ("Truncated tar archive", exit 1, measured), so that half holds.
- B. Judge by the local unpack, and read the sender's exit as the field does: a death by
  SIGPIPE (signal 13, or exit code 141 from a prefix that is a shell script without `exec`)
  after a good unpack is the normal end; any other non-zero sender exit is a failure of the
  target's `tar` and is reported as one, with its code, even when the unpack passed. Two or
  three lines in `pull::fetch` and a test with a sender that exits 141 after a complete
  archive, one that exits 2 with an empty stream, and one that stops mid-member.
- C. Drain the stream to EOF in Air, copying bytes from the sender into the unpacking tar and
  reading on after tar closes, so the sender never sees a closed pipe. A thread and an EPIPE
  case; it makes the 141 disappear rather than reading it, and buys nothing B does not,
  since a sender that fails with a code fails with a code either way.
- D. rsync over the prefix. Incremental, resumable, with its own verdict. Costs rsync on every
  target, a wrapper that maps rsync's host argument to the prefix, and a second transport
  for the push, which is git's `ext::` helper today. For result files written once per run it
  is a second mechanism for the same job.

## 4. Recommendation and verdict

B. It is the idiom, not a workaround: the pipeline's verdict is the receiver's, `kubectl cp`
returns the unpack's error and nothing else, and GNU tar's maintainers reverted the one
release that treated a SIGPIPE death as an error. The one place Air should be stricter than
`kubectl cp` is the sender's non-zero exit *code*, which `kubectl exec` relays from the pod's
`tar` and which means a listed file was not archived; `kubectl cp` loses it, and the empty
stream shows why Air should not. Cost: the lines in `pull::fetch` named in B and three test
cases, no new mechanism, no notice (what an adopting repo sees is the `pulled` column being
right). The do-less answers: the recorded failure is every pull through `kubectl exec` in the
pod trial (air-fzl, 2026-11-06) recorded as empty; the fix is a measurement read correctly,
not a gate; it removes a rule rather than adding one. C and D are not worth building for this.
D is the one to reopen if a run's results ever need incremental sync, and then as rsync with
the prefix as `--rsh`, not as code in Air.
