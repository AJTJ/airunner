# Tick 0315 — Rust crates and the hook latency budget

Written 2026-08-18 ~03:15 PDT for ai_runner (Air). Question: which crates and settings let one Rust
binary, invoked as a Claude Code hook several times a minute per worker, (a) start, open the shared
WAL ledger at the main checkout, run a few queries, and exit well under 300 ms on macOS; (b) derive
git facts fast; (c) spawn `bd --json` / `git` with timeouts; (d) log NDJSON; (e) parse hook JSON —
all fail-open. Inputs: `docs/plans/0001-first-slice.md` §2, §5, §10; `docs/plans/0003-rust-conventions.md`;
`docs/research/prior-art-landscape.md` §I; `docs/research/verification/protocols-leases-resources.md`
rows 33/58. No code was built. Crate versions/licences/dates are from the crates.io API
(`/api/v1/crates/<name>` and `/versions`) fetched 2026-08-18; everything else is cited inline.
Local machine: macOS 26.5.2, rustc 1.97.1 (2026-07-14), git 2.51.1, system `sqlite3` 3.39.2 (2022).

Legend: **M** = measured on this machine today (Python `subprocess` loop, n=10–20, so numbers include
~1–3 ms of Python fork/exec overhead — treat as upper bounds); **D** = documented in a primary source;
**U** = unverified / to be measured in M0.

## 1. Crate table

| Need | Crate @ version (today) | License | Last release | Why this one | Settings that matter | Source (accessed 2026-08-18) |
|---|---|---|---|---|---|---|
| SQLite ledger | `rusqlite` 0.40.2 (+ `libsqlite3-sys` 0.38.2) | MIT | 2026-08-08 | Sync API, no runtime, mature; the shortlist choice (§I). MSRV **1.88.0** (`Cargo.toml` `rust-version`). | `features = ["bundled"]`, `default-features = false` (as dcg does, row §4). Bundled SQLite is **3.53.x** (README: "currently SQLite 3.53.2 (as of rusqlite 0.40.1)"; master `sqlite3.h` says 3.53.4). macOS ships **3.39.2** — old (pre-STRICT-tables-era features present but no 3.4x/3.5x fixes) and Apple's build is not the amalgamation we test against; the README: "*bundled* … avoids depending on the version of SQLite on the users system … which may be old or missing. It's the right choice for most". Per connection: `PRAGMA busy_timeout=<n>` (`Connection::busy_timeout`), `PRAGMA synchronous=NORMAL`; once at `air init`: `PRAGMA journal_mode=WAL` (persistent). Use `prepare_cached` for the 3–4 hot statements. | https://github.com/rusqlite/rusqlite/blob/master/README.md ; https://crates.io/api/v1/crates/rusqlite |
| SQLite semantics | (SQLite docs) | — | 3.53.4 (2026) | — | WAL: "writers and readers can run at the same time. However, since there is only one WAL file, there can only be one writer at a time" (wal.html §2.2); "unlimited number of simultaneous readers, but … only one writer at any instant" (whentouse.html); `synchronous=NORMAL` in WAL: "always consistent … but WAL mode does lose durability. A transaction committed … might roll back following a power loss or system crash" while "Transactions are durable across application crashes regardless of the synchronous setting" (pragma.html) — acceptable for a ledger that is re-derivable from git + hooks; `busy_timeout` and `synchronous` are per-connection, `journal_mode=WAL` is persistent; "When the last connection to a database closes, that connection does one last checkpoint and then deletes the WAL and its associated shared-memory file" (wal.html) — with per-invocation processes this happens on nearly every exit (small WAL, cheap, but see §3 daemon question); "All processes using a database must be on the same host computer; WAL does not work over a network filesystem" — `.air/ledger.db` must stay on local disk (not iCloud/SMB). | https://sqlite.org/wal.html ; https://sqlite.org/pragma.html ; https://sqlite.org/whentouse.html |
| CLI parsing | `clap` 4.6.6 | MIT OR Apache-2.0 | 2026-08-06 | The shortlist choice; derive API; MSRV 1.85; edition 2024. Runtime parse cost is not a differentiator: rosetta-rs measures release-mode parse at ~2 ms for clap, lexopt 0.3.2, argh 0.1.19, pico-args alike vs 1 ms for no parser (Linux, rustc 1.94). Binary overhead: clap 574 KiB vs lexopt 37 KiB / argh 38 KiB — irrelevant to a 300 ms budget, relevant only to `cargo build` time (clap ~3 s full build vs lexopt 329 ms). | Keep `clap` (derive) for `air`; do **not** add a second parser for hooks — the hook subcommand should not parse much (`air hook <event>` and read stdin). If build time ever matters, `lexopt` 0.3.2 (MIT, 2026-02-28) is the swap. | https://github.com/rosetta-rs/argparse-rosetta-rs/blob/main/README.md |
| Git facts | shell `git` (2.51.1 local) first; `gix` 0.86.0 for read-only in-process work if measured need | gix: MIT OR Apache-2.0 | gix 2026-07-23 | gix has `Repository::merge_base*` (feature `merge`/`revision`), `head_id`/`head_commit`, `status()`/`is_dirty()` (feature `status`), `diff_tree_to_tree` (feature `blob-diff`), `worktrees()`/`worktree()`; crate-status marks merge-base, status incl. untracked, tree↔tree and tree/index↔worktree diff, and "open a repository with worktrees" as `[x]`; worktree create/move/remove/prune are `[ ]`. Perf: "gix works best with 3 to 4 threads, which is when it is about 10% faster than git, at least on MacOS" (2023 discussion) — i.e. `status` is IO-bound either way; the win from gix is skipping the ~10 ms `git` process spawn, not the walk. `git2` 0.21.0 (2026-05-18) needs libgit2 (C) and adds no worktree ergonomics — not preferred (§I already says so). gix MSRV 1.85. | Measured here (M): `git rev-parse HEAD` 10.9 ms median, `git merge-base --is-ancestor` 13.1 ms, `git status --porcelain` 22–25 ms (the adopter's main and a worktree, 518 tracked files), `git diff --name-only main` 17.9 ms in a worktree; `git --version` alone 10.4 ms ⇒ **spawn floor ≈ 10 ms per `git` call on this Mac** (the task's "5–15 ms" range is confirmed at the upper end; the raw `fork+exec` floor from Python was 3.3 ms for `/usr/bin/true`). Consequence: 4–5 worktrees × status ≈ 100–125 ms — too much for a hook, fine for the CLI. | https://docs.rs/gix/latest/gix/struct.Repository.html ; https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md ; https://github.com/Byron/gitoxide/discussions/1074 ; local measurement (see §2) |
| Spawn with timeout | `std::process::Command` + `wait-timeout` 0.2.1 | MIT/Apache-2.0 | 2025-02-03 (stable, tiny; alexcrichton) | Adds `ChildExt::wait_timeout(Duration) -> Option<ExitStatus>`; on Unix it "registers a `SIGCHLD` handler" (caveat: "If your application is otherwise handling `SIGCHLD` then bugs may arise" — Air's hook binary does not). Pattern: spawn with piped stdout, read stdout on a thread, `wait_timeout`, on `None` → `kill()` + `wait()`. Alternative single-crate: `subprocess` 1.2.1 (Apache-2.0/MIT, 2026-08-05, MSRV 1.88, `communicate_start().limit_time()`); `duct` 1.1.1 (MIT, 2025-11-09) has no timeout. `process-wrap` 9.1.0 (Apache-2.0 OR MIT, 2026-03-08, MSRV 1.87) gives `ProcessGroup`/`ProcessSession` for killing the whole tree — needed by the *daemon/CLI* that runs `bd`/`cargo`, not by hooks; it does not add a std wait-with-timeout. tokio is not needed on the hook path (plan 0001 §10). | Hook path: never spawn `bd` (see §2, `bd --version` = 122 ms, `bd ready --json` = 1.1 s). Spawn `git` only for O(1) facts, `timeout ≤ 100 ms`, kill on expiry, treat as "unknown". CLI path: `git` 2 s, `bd` 5 s, process-group kill. | https://docs.rs/wait-timeout/latest/wait_timeout/ ; https://docs.rs/process-wrap/latest/process_wrap/ ; crates.io API |
| JSON in/out | `serde` 1.0.229 + `serde_json` 1.0.151 | MIT OR Apache-2.0 | 2026-07-18 / 2026-07-20 | Hook input "arrives on stdin" as JSON with `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `permission_mode` (+ event fields); output is JSON on stdout (`continue`, `hookSpecificOutput.{hookEventName,additionalContext,permissionDecision}`), capped at 10 000 chars. | `#[derive(Deserialize)]` with `#[serde(default)]` and unknown fields ignored (default) so a new Claude Code field never fails parse; read stdin with a byte cap (e.g. 1 MiB) — a parse failure is **allow + log**, never exit 2. | https://code.claude.com/docs/en/hooks |
| NDJSON events | `serde_json` + `std::fs::OpenOptions::append` (no `tracing` on the hook path); `tracing` 0.1.44 / `tracing-subscriber` 0.3.23 (MIT; 2025-12-18 / 2026-03-13, MSRV 1.65) reserved for the CLI/daemon | — | — | One `serde_json::to_writer` + `\n` per event to `.air/events.ndjson` (mirror of the `events` table, §I "Event sourcing"). `tracing-subscriber`'s `json` feature pulls `tracing-serde` and formats spans — more than a one-shot process needs and it is initialised on every start. | Timestamps: `jiff` 0.2.35 (Unlicense OR MIT, 2026-07-25, MSRV 1.70) for RFC 3339; ids: `ulid` 3.0.0 (MIT, 2026-07-16). Atomicity of a single small `write(2)` in `O_APPEND` mode across concurrent hooks is **U** (POSIX guarantees the offset, not the write size); keep lines < 4 KiB and accept the rare torn line, or write the event to SQLite (authoritative) and let the CLI export NDJSON. | crates.io API |
| Errors | `thiserror` 2.0.20 | MIT OR Apache-2.0 | 2026-08-08 | Plan 0003 §1 (open). Nothing here changes it. | — | crates.io API |
| Unix bits (only if needed) | `nix` 0.31.3 | MIT | 2026-05-11 | `nix::fcntl::Flock` for the `.air` init lock; `nix::sys::event` (kqueue) for the daemon — not hooks (protocols row 33). | — | crates.io API |

## 2. Local measurements (M) — the adopter's checkout, 518 tracked files, 4 linked worktrees

| Command | median | p90 | n |
|---|---|---|---|
| `/usr/bin/true` (fork+exec floor from Python) | 3.3 ms | 3.7 | 20 |
| `git --version` | 10.4 ms | 10.9 | 20 |
| `git rev-parse HEAD` | 10.9 ms | 11.5 | 20 |
| `git merge-base --is-ancestor HEAD~3 HEAD` | 13.1 ms | 14.1 | 20 |
| `git status --porcelain` (main) | 22.4 ms | 26.4 | 10 |
| `git status --porcelain` (linked worktree) | 25.1 ms | 27.7 | 10 |
| `git diff --name-only main` (worktree, committed delta) | 17.9 ms | 18.3 | 10 |
| `git diff --name-only HEAD` (worktree, uncommitted) | 15.6 ms | 16.2 | 10 |
| `sqlite3 :memory: 'select 1'` (CLI, includes readline init — not the library open cost) | 21.0 ms | 21.6 | 20 |
| `bd --version` (Go, embedded Dolt) | **121.9 ms** | 124.3 | 5 |
| `bd ready --json` (the adopter `.beads`) | **1.08–1.09 s** | — | 3 |
| `bd list --json --limit 5` | **1.91 s** | — | 2 |

Reading: `git` costs ~10 ms of spawn plus 1–15 ms of work per call on this machine (**verified** for the
"~5–15 ms" claim, at the top of that range). `bd` is two orders of magnitude slower than the whole hook
budget — **`bd` must never be on a hook's path**; the ledger caches bead state and the CLI/daemon
refreshes it. Not measured (no code built): Rust binary start, `rusqlite` open, query time — §3 marks
them "M0".

## 3. Proposed latency budget for one hook invocation (target p50 ≤ 30 ms, p99 ≤ 150 ms, self-imposed hard stop 250 ms)

| Stage | Budget | Basis |
|---|---|---|
| exec + dyld + Rust runtime init | ≤ 5 ms | M: `/usr/bin/true` 3.3 ms via Python; a static Rust binary with `panic="abort"`/LTO is in the same class (dcg claims "sub-millisecond" for its whole hook, §4). **M0: measure `air hook --noop`.** |
| read stdin + `serde_json` parse of the hook payload | ≤ 1 ms | payload is a few KiB (hooks docs); **M0**. |
| `clap` dispatch | ≤ 2 ms | D: rosetta-rs 2 ms release-mode parse. |
| locate ledger (`git rev-parse --git-common-dir` **or** walk up for `.git`/`gitdir:` file in-process) | 0–11 ms | M: `git rev-parse` 10.9 ms; in-process file walk ≈ 0 ms → prefer in-process (`.git` file → `gitdir:` → `commondir`); or cache the path in `.air/config.toml` written by `air init`. |
| `rusqlite` open + `busy_timeout` + `synchronous=NORMAL` + `prepare_cached` × 3–4 + 1–3 queries + 1 insert (WAL append) | ≤ 5 ms typical; worst case bounded by `busy_timeout` | D: WAL write "very fast … writing the content once … sequential" (wal.html §2.3); one-writer rule means a concurrent hook may wait — set `busy_timeout = 100 ms` on hooks (a lock "lasts no more than a few dozen milliseconds", whentouse.html) and treat `SQLITE_BUSY` after that as "unknown"; **M0: measure open+first-query on a warm and cold cache.** |
| git facts | HEAD ≤ 1 ms in-process (read `HEAD`/packed-refs — or `gix::Repository::head_id`); at most **one** `git` spawn (`merge-base --is-ancestor`, 13 ms M) when a hook truly needs it; **no** `status`/`diff` across worktrees inside a hook (100–125 ms M) — those are computed by the CLI (`air who/next`) or a refresh and stored in the ledger; the `PreToolUse` warn reads the ledger's `edit_journal` + last known committed diff. | M (§2) |
| `bd` | 0 ms in hooks | M: 122 ms–1.9 s. |
| NDJSON/event append | ≤ 1 ms | one `write(2)`; **M0**. |
| exit | ~0 ms | `std::process::exit` after flushing stdout; no destructors needed. |
| **Total** | **≈ 10–25 ms p50; ≤ 150 ms p99** (one git spawn + one busy wait) | leaves > 100 ms headroom under 300 ms |

Daemon vs per-invocation open: not justified for M0. Per-invocation open is a few ms (U, M0), SQLite
WAL supports concurrent readers + one writer natively, and a daemon adds a socket round-trip, a
lifecycle (start/stop/crash) and a second failure mode to fail-open around. Revisit only if M0 shows
open+query > 20 ms or if the last-connection-close checkpoint on every hook exit shows up in traces
(wal.html: last connection checkpoints and deletes the WAL); the escape hatch then is a long-lived
`air daemon` holding one connection so the WAL persists, or `SQLITE_FCNTL_PERSIST_WAL`.

## 4. Fail-open mechanics (allow + log on panic/timeout)

- Claude Code's own semantics already fail open: exit 0 with no JSON = "no decision"; "Any other exit
  code doesn't block on its own for most hook events"; a hook that reaches its `timeout` "is canceled:
  Claude Code discards the hook's output, and the hook renders no decision"; only **exit 2** blocks —
  "even a JSON `permissionDecision` of `"allow"` can't override it" (hooks docs). So the invariants for
  Air are: (1) never `exit(2)` from a path that is not the deliberate hand-over refusal; (2) set a
  short `timeout` in `settings.json` (plan 0001 §5) as the outer guard.
- `panic = "abort"` vs `catch_unwind`: `catch_unwind` "only catches unwinding panics, not those that
  abort the process" (std docs), so the two are mutually exclusive. With `abort`, a panic exits via
  SIGABRT (non-zero, ≠ 2 → non-blocking) but **the event log line is lost** and stderr shows a Rust
  abort. Recommendation: **`panic = "unwind"` in the hook binary**, `std::panic::catch_unwind(AssertUnwindSafe(run))`
  in `main`, on `Err` → append a `hook_panic` event (best-effort) and `exit(0)` with no decision.
  Binary-size cost of unwinding is small vs the 300 ms budget (min-sized-rust: `abort` "removes the need
  for this extra unwinding code" — a size optimisation, not a latency one). ripgrep uses `panic="abort"`
  only in its opt-in `release-lto` profile; codex-rs's release profile is `lto="thin"`, `codegen-units=4`,
  no `panic` override; dcg (a shipping Claude Code hook) uses `abort` and documents fail-open in policy.
- Internal watchdog: spawn a thread at start that sleeps `250 ms` then writes nothing (or the
  no-decision JSON) and `process::exit(0)` — guarantees an answer even if SQLite or `git` stalls; the
  `busy_timeout` and `wait_timeout` are the inner guards.
- Release profile for `air`: `lto = "thin"` (or `true`), `codegen-units = 1`, `opt-level = 3` (speed,
  not `z` — dcg needed per-package `opt-level = 3` overrides for regex crates after choosing `z`),
  `strip = true`, `debug = "line-tables-only"` only in a `profiling` profile (codex-rs pattern).
- MSRV/edition: `edition = "2024"` is fine — the binding constraints are `rusqlite`/`libsqlite3-sys`
  **1.88.0**, `process-wrap` 1.87, `gix` 1.85, `subprocess` 1.88, `clap` 1.85; stable is 1.97.1 today
  ⇒ set `rust-version = "1.88"` (or 1.97 if we pin exact, plan 0003 §6).

## 5. How comparable Rust CLIs/hooks keep startup low

1. **destructive_command_guard (dcg)** 0.11.0 — a Rust Claude Code `PreToolUse` hook (Dicklesworthstone,
   crates.io 0.7.8 published 2026-07-31; repo `Cargo.toml` at 0.11.0): `rusqlite 0.40.1 default-features=false, bundled`,
   `clap 4.5 derive`, `serde_json`, `panic="abort"`, `opt-level="z"` + per-package `opt-level=3` for regex/aho-corasick/memchr,
   README claims "Sub-Millisecond Latency", tiered "<100µs / <1ms / <5ms" pipeline, and a written
   Bounded Failure Policy: "Malformed or oversized raw hook JSON → Allow with an audit warning";
   "Transient hook stdin I/O error → Allow … Always fail-open"; `DCG_FAIL_CLOSED=1` opt-in.
   https://github.com/Dicklesworthstone/destructive_command_guard (README, Cargo.toml).
2. **beads_rust `br`** 0.3.2 (edition 2024, MSRV 1.88, pushed 2026-08-17): SQLite (`fsqlite` 0.3.1 pure-Rust,
   WAL) + JSONL, `clap 4.6.6`, release profile `opt-level="z", lto=true, codegen-units=1, panic="abort", strip=true`.
   Its README stresses "never … installs hooks, or runs as a daemon" — one-shot process design.
   https://github.com/Dicklesworthstone/beads_rust (Cargo.toml, README).
3. **ripgrep**: default `[profile.release] debug = 1`; the distribution profile `release-lto` is
   `opt-level=3, lto="fat", panic="abort", codegen-units=1, strip="symbols"` — speed-first, size second.
   https://github.com/BurntSushi/ripgrep/blob/master/Cargo.toml. **codex-rs**: `lto="thin"`,
   `codegen-units=4`, `debug="line-tables-only"`, `strip=false` until packaging — a large tokio binary,
   not a hook; useful only as the "thin LTO is enough" data point.
   https://github.com/openai/codex/blob/main/codex-rs/Cargo.toml.

## 6. Decisions to record in plan 0003 (proposed, not decided)

1. `rusqlite` 0.40.x `bundled` (SQLite 3.53.x), never system SQLite on macOS (3.39.2).
2. Connection recipe: `journal_mode=WAL` once at `air init`; per open `busy_timeout` (hooks 100 ms, CLI
   2 s), `synchronous=NORMAL`, `prepare_cached`; ledger file must be on local disk.
3. Per-invocation open, no daemon in M0; revisit on M0 numbers (§3).
4. `clap` derive only; hooks take `air hook <event>` + stdin, no second parser.
5. Git: shell `git` for M0 (one spawn max per hook, `wait_timeout` ≤ 100 ms); cross-worktree
   `status`/`diff` only in CLI/refresh paths; adopt `gix` (features `status`, `merge`, `blob-diff`,
   read-only) only when M0 shows a hook needs in-process git beyond reading `HEAD`.
6. `bd` never on a hook path (122 ms–1.9 s measured); CLI spawns it with a 5 s timeout and process-group kill (`process-wrap` when the daemon exists).
7. `std::process` + `wait-timeout` on the hook path; no tokio.
8. Hook events: `serde_json` NDJSON append + SQLite row; `tracing` only in CLI/daemon.
9. Fail-open: `panic="unwind"` + `catch_unwind` in `main` + 250 ms watchdog thread + never `exit(2)`
   outside the hand-over refusal; `settings.json` hook `timeout` as the outer guard.
10. `edition = "2024"`, `rust-version = "1.88"` floor (rusqlite), stable toolchain.
11. Release profile: `opt-level=3`, `lto="thin"`, `codegen-units=1`, `strip=true`; measure `air hook --noop`
    startup in M0 before touching `opt-level`/`panic` for size.

## 7. Open / to measure in M0

- Rust binary cold/warm start on this Mac; `rusqlite` open + first query; end-to-end `air hook PostToolUse` p50/p99.
- Whether the last-connection checkpoint on every hook exit costs anything visible.
- NDJSON append atomicity under concurrent hooks (or drop the mirror and export from SQLite).
- gix compile-time/binary-size impact if adopted (2023 discussion notes only "marginal" size savings from feature toggles).

## Sources

- crates.io API `/api/v1/crates/{rusqlite,libsqlite3-sys,clap,lexopt,argh,gix,git2,wait-timeout,process-wrap,serde,serde_json,tracing,tracing-subscriber,thiserror,jiff,ulid,nix,duct,subprocess}` and `/versions` (2026-08-18)
- https://github.com/rusqlite/rusqlite/blob/master/README.md ; `Cargo.toml` (`rust-version = "1.88.0"`); `libsqlite3-sys/sqlite3/sqlite3.h` (`SQLITE_VERSION "3.53.4"`)
- https://sqlite.org/wal.html ; https://sqlite.org/pragma.html ; https://sqlite.org/whentouse.html
- https://docs.rs/gix/latest/gix/struct.Repository.html ; https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md ; https://github.com/Byron/gitoxide/discussions/1074
- https://github.com/rosetta-rs/argparse-rosetta-rs/blob/main/README.md
- https://docs.rs/wait-timeout/latest/wait_timeout/ ; https://docs.rs/process-wrap/latest/process_wrap/
- https://code.claude.com/docs/en/hooks
- https://doc.rust-lang.org/std/panic/fn.catch_unwind.html ; https://github.com/johnthagen/min-sized-rust
- https://github.com/Dicklesworthstone/destructive_command_guard ; https://github.com/Dicklesworthstone/beads_rust ; https://github.com/BurntSushi/ripgrep/blob/master/Cargo.toml ; https://github.com/openai/codex/blob/main/codex-rs/Cargo.toml
- Local: `python3 subprocess` timing loop in `the adopter's checkout` (2026-08-18); `sqlite3 --version`, `rustc --version`, `git --version`, `sw_vers`
