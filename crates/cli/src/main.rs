//! `air` — hub and referee for a few concurrent coding agents in git worktrees.
//!
//! First slice (docs/design.md): `record`, `handover`, `holdings`, `hook`,
//! `doctor`, `selftest`. Everything prints `--json` on request and a denominator; refusals
//! name the fixing command. `hook` is fail-open: any internal error → exit 0 + an event line.

mod cmd;
mod git;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
// air-dwq5: clap's own `--version` is disabled and replaced by one flag Air owns, so there is
// exactly one implementation. clap's short-circuits before any of Air's code runs, which is
// why `--version --json` printed a plain string, and why the version could not carry the
// build. Two implementations of one answer is the drift air-avj is about, one layer down.
#[command(
    name = "air",
    disable_version_flag = true,
    about = "Hub and referee for a few concurrent coding agents"
)]
pub(crate) struct Cli {
    /// Print the version, the commit this binary was built from, and its surface version.
    #[arg(long, short = 'V')]
    version: bool,
    /// Emit JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Repository path (defaults to the current directory).
    #[arg(long, global = true)]
    repo: Option<PathBuf>,
    // Optional so `air --version` parses on its own; a bare `air` prints where to look.
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Debug, Subcommand)]
enum LeaseOp {
    /// Acquire, or print the holder and exit 1. Breaks a dead or stale holder's lease.
    Take {
        #[arg(default_value = "runtime")]
        resource: String,
        #[arg(long, default_value = "unspecified")]
        reason: String,
    },
    /// Release if this worktree holds it.
    Release {
        #[arg(default_value = "runtime")]
        resource: String,
    },
    /// Who holds what, with defects and waiters.
    Status,
    /// Clear a dead or stale lease; a healthy one needs --force.
    Break {
        #[arg(default_value = "runtime")]
        resource: String,
        #[arg(long)]
        force: bool,
    },
    /// Refresh heartbeats on every lease this worktree holds (hooks do this automatically).
    Beat,
    /// Which lease(s) a shell command needs, per `"leases"` in .claude/air.json, and whether
    /// this session holds them. The same rules the PreToolUse hook applies. Always exits 0.
    Needs {
        /// The command, quoted as one argument, e.g. `air lease needs "make api"`.
        command: String,
    },
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Run a check command and record its exit for the current HEAD, e.g.
    /// `air record verify -- make verify`.
    Record {
        /// verify | docs-check | fitness
        kind: String,
        /// The command to run (after `--`).
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// The one gate: is this worktree ready to hand over? (advisory unless --enforce)
    Handover {
        /// Bead id being handed over (checked against the ledger's claims).
        #[arg(long)]
        bead: Option<String>,
        /// Actually refuse (exit 2) instead of reporting what would be refused.
        #[arg(long)]
        enforce: bool,
    },
    /// Who has edits in which files, across worktrees (uncommitted, committed, journaled).
    Holdings {
        /// Only this file (repo-relative).
        #[arg(long)]
        file: Option<String>,
    },
    /// Claim a bead: `bd update --claim` (atomic) then the ledger row. The only claim path.
    Claim {
        bead: String,
        /// Files you expect to touch (repo-relative, comma-separated); informs peers' warnings.
        #[arg(long, value_delimiter = ',')]
        files: Vec<String>,
    },
    /// Give a bead back: bd in_progress → open (never a closed bead), ledger claim closed with a reason.
    Release {
        bead: String,
        /// landed | abandoned | reassigned | superseded | false-premise | owner-gated | unknown
        #[arg(long)]
        reason: String,
        /// Coordinator only: release a peer's claim (a gone worker's bead).
        #[arg(long)]
        worker: Option<String>,
    },
    /// One line into the inbox, or a whole finding with --file. Workers capture; the
    /// coordinator triages. Never blocks you.
    Capture {
        /// The capture text. Exactly one of this or --file.
        text: Option<String>,
        /// Read the capture from this file instead, whole and untruncated — for a finding too
        /// long to survive a command line (air-45pw). Exactly one of this or the positional.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Hidden since air-uef: `coordinator` is the only audience. `--for owner` is refused
        /// with the replacement named; the owner's queue is beads labelled `owner`.
        #[arg(long = "for", default_value = "coordinator", hide = true)]
        audience: String,
    },
    /// Open captures, oldest first. The coordinator triages every one into a bead or drops it.
    Inbox,
    /// Mutual exclusion for what two agents cannot share (ports, simulator, Docker, browser).
    Lease {
        #[command(subcommand)]
        op: LeaseOp,
    },
    /// Resolve ONE capture: --bead <id> after `bd create`, or --drop "<why>" (coordinator).
    /// The bead is checked against bd first; an id bd does not have refuses it. A capture that
    /// was already triaged is re-pointed, old target named in the event line.
    ///
    /// One at a time on purpose (air-zlq, 2026-08-29). Batching was measured, not assumed:
    /// `bd show` costs about a second PER ID, so one process for 26 ids took 27.9 s against a
    /// 5 s verification budget. Batching saved the process, which was never the cost.
    Triage {
        id: String,
        #[arg(long)]
        bead: Option<String>,
        #[arg(long)]
        drop: Option<String>,
    },
    /// Coordinator: close landed beads in ONE bd process and release their claims in one
    /// ledger transaction. `bd` costs ~1.4 s per process whatever it is asked (air-869).
    Close {
        #[arg(required = true)]
        bead: Vec<String>,
        /// Why they closed; bd records it on every id. Exactly one of this or --reason-file.
        #[arg(long)]
        reason: Option<String>,
        /// Read the reason from this file instead, whole and untruncated — for proof too long
        /// to survive a command line (air-lyjr). Exactly one of this or --reason. The same
        /// reason is recorded on every id named, exactly as --reason already is.
        #[arg(long)]
        reason_file: Option<PathBuf>,
    },
    /// Coordinator: land a green branch on main. The one allowed path onto main; it pushes
    /// nothing.
    ///
    /// **Main is never moved to a commit that has not been verified** (air-odv). The landing
    /// commit is built off main with `git commit-tree` and main is fast-forwarded onto it, so
    /// there is no window in which main holds unverified code and nothing to roll back. Because
    /// the branch must contain main, that commit's tree is byte-identical to the one the
    /// worker's recorded green describes, so the landing re-verifies nothing.
    ///
    /// A branch is landable when it contains main AND carries a recorded green at its head —
    /// exactly what `air status` checks, since both call the same predicate (air-y3v). The
    /// beads reported are the ones its merge range (`main..<head>`) names in its commit
    /// messages, confirmed against bd. Nothing is read from `awaiting_review`: the worker
    /// closes its own bead with proof and never sets it (air-7kp).
    ///
    /// One merge per branch, however many beads that branch carries; `--all` takes the oldest
    /// branch first and stops at the first refusal.
    ///
    /// **The branch is the unit, and `--worker <name>` names it** (air-09b). A bead id names a
    /// branch only while exactly one branch carries it; a bead on two branches (a batching
    /// lane and the worker it batched, the adopter 2026-08-30) is refused with every carrier and
    /// the `--worker` command for each, never resolved by ordering or by which one happens to
    /// be landable. `--worker` lands that branch with every bead its merge range names, and
    /// so does naming a bead: the argument SELECTS the branch, it does not filter what the
    /// merge carries or what the landing records (air-dnr). A bead no green branch names is
    /// still refused.
    ///
    /// It closes nothing. The worker closes its own bead with proof before the branch lands
    /// (owner ruling, 2026-08-22), so this prints every bead in the merge beside its
    /// acceptance criteria and Air's verdict on each clause — the only external check on that.
    /// Air discharges a clause only by lookup: a green verify recorded at the landed sha, or a
    /// path the merge changed. Everything else it reports as unreadable rather than judging.
    /// A clause naming a file the merge did not change is kept on the landings row and named
    /// by `air status` (air-ayp) — as a lookup that did not answer, never as a contradiction
    /// and never by itself as a wrong close: six of nine such firings were clauses that held
    /// (air-k6uh).
    Land {
        bead: Vec<String>,
        /// Land this worker's branch (`worktree-<name>`), whatever beads it carries. The
        /// unambiguous selector; repeatable.
        #[arg(long = "worker", value_name = "NAME", conflicts_with = "bead")]
        worker: Vec<String>,
        /// Land every green branch, oldest first, stopping at the first red.
        #[arg(long)]
        all: bool,
        /// Land even though a verify is in flight, destroying it (air-1bm). Refused
        /// otherwise, naming each run and its pid. Every override is recorded on the landings
        /// row and the event line: that count is the refusal's removal condition.
        #[arg(long = "despite-inflight")]
        despite_inflight: bool,
    },
    /// The coordinator's one screen: workers, sessions, claims, green, overlaps, inbox.
    Status {
        /// Only the conditions that need the owner or the coordinator (empty when quiet).
        #[arg(long)]
        attention: bool,
    },
    /// MCP server over stdio: the coordinator's channel (push) plus tools and resources.
    Mcp,
    /// Give a project everything Air needs: gate on bd/claude, git init, bd init, .gitignore,
    /// .claude/air.json (deny patterns from a scan), hooks, MCP, roles, skills, and the
    /// empty-but-ready scaffold (a failing Makefile verify target, .worktreeinclude, and a
    /// CLAUDE.md stub carrying the work flow and the `Bead:` trailer rule), each created only
    /// when absent and never edited. Dry run by default.
    Init {
        /// Beads issue prefix (default: from the directory name).
        #[arg(long)]
        prefix: Option<String>,
        #[arg(long)]
        write: bool,
    },
    /// Wire Air into this repo's Claude Code config (hooks, MCP server, .air/). Dry run by default.
    Install {
        /// Apply the changes (refuses if `air` on PATH is not this binary).
        #[arg(long)]
        write: bool,
    },
    /// Start an interactive worker session: Air creates `.claude/worktrees/<name>` (filling it
    /// from `.worktreeinclude`), then runs claude IN it with role prose, deny list, env. No
    /// `--worktree` (air-8gj): Air's PreToolUse hook is the fence.
    ///
    /// With --tmux or --task and a tty, opens a tmux session here. Without a tty (the
    /// coordinator's Bash tool, `</dev/null`) it starts a detached tmux session named
    /// <project>-<name> instead, prints `tmux attach -t <project>-<name>`, and exits 0.
    /// AIR_CLAUDE_BIN overrides the claude binary; AIR_TMUX_SOCKET selects a tmux socket.
    Worker {
        /// Worktree name for the lane. Omitted, Air picks the next free `w<N>` (air-5lg).
        name: Option<String>,
        /// Remove the lane's worktree instead of launching. Refused, naming what holds it,
        /// while it has uncommitted work, a harness lock, or a tmux session; the branch stays.
        #[arg(long)]
        remove: bool,
        /// Run in a tmux pane the owner can attach to (lets the coordinator launch workers).
        #[arg(long)]
        tmux: bool,
        /// Initial task for the worker, as its first prompt (implies --tmux).
        #[arg(long)]
        task: Option<String>,
        /// Model to launch on, e.g. `claude-opus-5`. Omitted, the session inherits whatever the
        /// harness gives it, and the ledger records what it actually got (air-air).
        #[arg(long)]
        model: Option<String>,
        /// Print the command instead of running it.
        #[arg(long)]
        print: bool,
        /// Extra arguments passed to `claude` (after `--`).
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Start the interactive coordinator session in the main checkout with the Air channel attached.
    Coordinator {
        /// Model to launch on (air-air); inherited when omitted.
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        print: bool,
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Claude Code hook entrypoint: reads the hook JSON on stdin.
    Hook,
    /// What the ledger says about every mechanism Air ships: how often each was `evaluated`
    /// in the window, over how many `subject(s)` and with how many `repeat(s)`, how often it
    /// was `pushed` at a person, when it `last fired`, and what it is `removed when`, with
    /// what the `ledger says` about that condition. Facts only; the pass over them is the
    /// coordinator's. Read-only.
    ///
    /// `evaluated` and `pushed` are different numbers on purpose (air-5uz). The channel
    /// re-evaluates every condition it holds on every poll; counting that as a firing read
    /// 10,722 log lines as 10,722 firings against 45 things anyone was actually told, and a
    /// deletion was nearly proposed on the inflated number. A "fired with nothing following"
    /// count was cut from air-zyo before it shipped (Air inferring intent it cannot see) but
    /// stayed in this help for a round: the same derived-reads-like-observed failure the
    /// command exists to surface (air-ha8). Every backticked name in this help is a field the
    /// command prints, and air selftest checks the containment, so the drift cannot come back
    /// quietly.
    ///
    /// It also prints `budgets`: every timing budget Air waits on, with which way each fails
    /// when it is hit, and `hits` — a budget reached, meaning a decision taken on less than
    /// was asked for. Three of them fail toward permitting, and the hook's own cap cannot
    /// record its overruns at all, so `hook pairing` counts what a killed hook leaves instead
    /// (air-d75).
    ///
    /// And `re-claim churn`: how often a worker took a bead it could not start, from the
    /// claims table alone. The ordering-edge question was answered with that number rather
    /// than with a rule (air-69u), so the number has to be re-runnable and the `THRESHOLD 10%`
    /// that would reopen the question is printed beside it (air-5nh).
    Audit {
        /// Inclusive YYYY-MM-DD to count from (default: today).
        #[arg(long)]
        since: Option<String>,
    },
    /// A stated `retention` for `.air/events/`: what is `COLLECT`able, what is kept and why,
    /// and how many `byte(s) collectable`. Prints and stops unless `--apply` is given.
    ///
    /// No automatic path, deliberately. The raw event stream is the only artefact that has
    /// caught the audit's own errors (0007 §11: re-reading the document found nothing,
    /// re-running the commands found three), so a day the ledger still points at is never
    /// collected and nothing is removed without being asked twice (air-i7s).
    Gc {
        /// Days of history to keep (default 90, chosen against the post-air-5uz rate).
        #[arg(long)]
        keep_days: Option<i64>,
        /// Actually remove the collectable days. Without it, gc reports and stops.
        #[arg(long)]
        apply: bool,
    },
    /// Ledger location, sizes, row counts, the journal mode and schema version in effect, and
    /// whether `bd` is the pinned version. (It printed one pragma while the help said
    /// "pragmas"; air-ha8.)
    Doctor,
    /// The release-time check: every surface notice is covered by a RELEASES row and
    /// Cargo.toml matches the last row. Run by `make release`; exit 2 names the row to append.
    #[command(hide = true)]
    ReleaseCheck,
    /// No tracked file names an adopter (air-bpj). Names are read from `private/adopters.md`,
    /// which is ignored, so a clone without it skips cleanly. Run by `make verify`; exit 2
    /// names every offending line.
    #[command(hide = true)]
    AdopterCheck,
    /// Red/green probes for every check (a check that matches nothing prints red).
    Selftest {
        /// air-682: run each probe's DECLARED mutation and report any probe that stays green.
        /// Neutralises the rule a probe names, rebuilds, and requires that probe to go red and
        /// the others to stay green. Edits tracked files, so it refuses a dirty tree.
        #[arg(long)]
        prove: bool,
    },
}

/// air-air: `--model <m>` becomes `--model <m>` in the passthrough, so `air worker --model x`
/// and `air worker -- --model x` produce the same argv and neither can drift from the other.
///
/// It goes FIRST in `extra`, and that placement is load-bearing rather than cosmetic. `extra` is
/// appended after `--disallowed-tools`, which is variadic: a bare value landing there is read as
/// one more deny rule (air-2ct, where three workers sat at an empty prompt because the only flag
/// terminating that list had been stripped). `--model` is a flag, so it terminates the deny list
/// and anything the caller passed after it keeps its own meaning.
fn with_model(model: Option<&str>, extra: &[String]) -> Vec<String> {
    let Some(m) = model.map(str::trim).filter(|m| !m.is_empty()) else {
        return extra.to_vec();
    };
    let mut v = vec!["--model".to_string(), m.to_string()];
    v.extend(extra.iter().cloned());
    v
}

/// `--repo` may not leave this checkout's repository (air-0lk). A worktree and its main
/// checkout share a git common dir, so `air --repo <worktree>` from anywhere in the repo is
/// fine; another project's path is not. Silent when either side is not a repository, because
/// then there is no project to leave (`air init` on a bare directory is the case).
fn foreign_repo(repo: &std::path::Path) -> Option<String> {
    let here = std::env::current_dir().ok()?;
    let (a, b) = (
        air_ledger::paths::air_dir_for(&here).ok()?,
        air_ledger::paths::air_dir_for(repo).ok()?,
    );
    if a == b {
        return None;
    }
    Some(format!(
        "air: refused: --repo {} is another project ({}); this session's is {}. A session may \
         only touch its own project (air-0lk). Run it from that project's own session.",
        repo.display(),
        b.display(),
        a.display()
    ))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    // air-dwq5: before anything that needs a repo, because "what binary is this" is a question
    // about the binary and must answer outside a checkout too.
    if cli.version {
        if cli.json {
            println!("{}", cmd::install::version_json());
        } else {
            println!("{}", cmd::install::version_line());
        }
        return ExitCode::SUCCESS;
    }
    let Some(cmd) = cli.cmd else {
        eprintln!("air: no command given; `air --help` lists them");
        return ExitCode::from(2);
    };
    let repo = cli
        .repo
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    if cli.repo.is_some()
        && let Some(why) = foreign_repo(&repo)
    {
        eprintln!("{why}");
        return ExitCode::from(2);
    }
    let code = match cmd {
        Cmd::Record { kind, command } => cmd::record::run(&repo, &kind, &command, cli.json),
        Cmd::Handover { bead, enforce } => {
            cmd::handover::run(&repo, bead.as_deref(), enforce, cli.json)
        }
        Cmd::Holdings { file } => cmd::holdings::run(&repo, file.as_deref(), cli.json),
        Cmd::Claim { bead, files } => cmd::claim::claim(&repo, &bead, &files, cli.json),
        Cmd::Release {
            bead,
            reason,
            worker,
        } => cmd::claim::release(&repo, &bead, &reason, worker.as_deref(), cli.json),
        Cmd::Capture {
            text,
            file,
            audience,
        } => cmd::capture::capture(&repo, text.as_deref(), file.as_deref(), &audience, cli.json),
        Cmd::Inbox => cmd::capture::inbox(&repo, cli.json),
        Cmd::Lease { op } => match op {
            LeaseOp::Take { resource, reason } => {
                cmd::lease::take(&repo, &resource, &reason, cli.json)
            }
            LeaseOp::Release { resource } => cmd::lease::release(&repo, &resource, cli.json),
            LeaseOp::Status => cmd::lease::status(&repo, cli.json),
            LeaseOp::Break { resource, force } => {
                cmd::lease::break_lease(&repo, &resource, force, cli.json)
            }
            LeaseOp::Beat => cmd::lease::beat(&repo),
            LeaseOp::Needs { command } => cmd::lease::needs_cmd(&repo, &command, cli.json),
        },
        Cmd::Triage { id, bead, drop } => {
            cmd::capture::triage(&repo, &id, bead.as_deref(), drop.as_deref(), cli.json)
        }
        Cmd::Close {
            bead,
            reason,
            reason_file,
        } => cmd::close::run(
            &repo,
            &bead,
            reason.as_deref(),
            reason_file.as_deref(),
            cli.json,
        ),
        Cmd::Land {
            bead,
            worker,
            all,
            despite_inflight,
        } => cmd::land::run(&repo, &bead, &worker, all, despite_inflight, cli.json),
        Cmd::Status { attention } => cmd::status::run(&repo, attention, cli.json),
        Cmd::Mcp => cmd::mcp::run(&repo),
        Cmd::Init { prefix, write } => cmd::init::run(&repo, prefix.as_deref(), write, cli.json),
        Cmd::Install { write } => cmd::install::run(&repo, write, cli.json),
        Cmd::Worker {
            name, remove: true, ..
        } => cmd::worktree::remove_cmd(&repo, name.as_deref()),
        Cmd::Worker {
            name,
            tmux,
            task,
            model,
            print,
            extra,
            ..
        } => cmd::launch::worker(
            &repo,
            name.as_deref(),
            &with_model(model.as_deref(), &extra),
            tmux,
            task.as_deref(),
            print,
        ),
        Cmd::Coordinator {
            model,
            print,
            extra,
        } => cmd::launch::coordinator(&repo, &with_model(model.as_deref(), &extra), print),
        Cmd::Hook => cmd::hook::run(&repo),
        Cmd::Audit { since } => cmd::audit::run(&repo, since.as_deref(), cli.json),
        Cmd::Gc { keep_days, apply } => cmd::gc::run(&repo, keep_days, apply, cli.json),
        Cmd::Doctor => cmd::doctor::run(&repo, cli.json),
        Cmd::ReleaseCheck => cmd::install::release_check_cmd(),
        Cmd::AdopterCheck => cmd::privacy::run(&repo, cli.json),
        Cmd::Selftest { prove } => {
            if prove {
                cmd::selftest::prove(&repo, cli.json)
            } else {
                cmd::selftest::run(cli.json)
            }
        }
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
