//! `air` — hub and referee for a few concurrent coding agents in git worktrees.
//!
//! First slice (docs/plans/0001-first-slice.md): `record`, `handover`, `holdings`, `hook`,
//! `doctor`, `selftest`. Everything prints `--json` on request and a denominator; refusals
//! name the fixing command. `hook` is fail-open: any internal error → exit 0 + an event line.

mod cmd;
mod git;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "air",
    version,
    about = "Hub and referee for a few concurrent coding agents"
)]
pub(crate) struct Cli {
    /// Emit JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Repository path (defaults to the current directory).
    #[arg(long, global = true)]
    repo: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
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
    /// One line into the inbox. Workers capture; the coordinator triages. Never blocks you.
    Capture {
        text: String,
        /// Audience: coordinator (default) or owner (the owner's decision queue).
        #[arg(long = "for", default_value = "coordinator")]
        audience: String,
    },
    /// Open captures, oldest first (coordinator). --owner shows the owner's decision queue.
    Inbox {
        #[arg(long)]
        owner: bool,
    },
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
        /// Why they closed; bd records it on every id.
        #[arg(long)]
        reason: String,
    },
    /// Coordinator: merge a green hand-over into main, verify the merged result, and rewind
    /// main if it goes red. The one allowed path onto main; it pushes nothing.
    ///
    /// A branch is landable when it carries a recorded green at its head; the beads reported
    /// are the ones its merge range (`main..<head>`) names in its commit messages, confirmed
    /// against bd (`air status` lists them). Nothing is read from `awaiting_review`: the
    /// worker closes its own bead with proof and never sets it (air-7kp).
    ///
    /// One merge per branch, however many beads that branch carries; `--all` takes the oldest
    /// branch first and stops at the first red. The repo's verify comes from
    /// `.claude/air.json` `verify_command`, default `make verify`.
    ///
    /// It closes nothing. The worker closes its own bead with proof before the branch lands
    /// (owner ruling, 2026-08-22), so this prints every bead in the merge beside its
    /// acceptance criteria and Air's verdict on each clause — the only external check on that.
    /// Air discharges a clause only by lookup: a green verify recorded at the landed sha, or a
    /// path the merge changed. Everything else it reports as unreadable rather than judging.
    /// A clause the merge CONTRADICTS is a wrong close, kept on the landings row and named by
    /// `air status` (air-ayp).
    Land {
        bead: Vec<String>,
        /// Land every green branch, oldest first, stopping at the first red.
        #[arg(long)]
        all: bool,
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
    /// .claude/air.json (deny patterns from a scan), hooks, MCP, roles, skills. Dry run by default.
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
    /// Start an interactive worker session: `claude --worktree <name>` with role prose, deny list, env.
    ///
    /// With --tmux or --task and a tty, execs `claude --tmux`. Without a tty (the coordinator's
    /// Bash tool, `</dev/null`) it starts a detached tmux session named <project>-<name>
    /// instead, prints `tmux attach -t <project>-<name>`, and exits 0. AIR_CLAUDE_BIN overrides
    /// the claude binary; AIR_TMUX_SOCKET selects a tmux socket (`tmux -L`).
    Worker {
        /// Worktree name for the lane. Omitted, Air picks the next free `w<N>` (air-5lg).
        name: Option<String>,
        /// Run in a tmux pane the owner can attach to (lets the coordinator launch workers).
        #[arg(long)]
        tmux: bool,
        /// Initial task for the worker, as its first prompt (implies --tmux).
        #[arg(long)]
        task: Option<String>,
        /// Print the command instead of running it.
        #[arg(long)]
        print: bool,
        /// Extra arguments passed to `claude` (after `--`).
        #[arg(last = true)]
        extra: Vec<String>,
    },
    /// Start the interactive coordinator session in the main checkout with the Air channel attached.
    Coordinator {
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
    /// Red/green probes for every check (a check that matches nothing prints red).
    Selftest,
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
    let code = match cli.cmd {
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
        Cmd::Capture { text, audience } => cmd::capture::capture(&repo, &text, &audience, cli.json),
        Cmd::Inbox { owner } => cmd::capture::inbox(&repo, owner, cli.json),
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
        },
        Cmd::Triage { id, bead, drop } => {
            cmd::capture::triage(&repo, &id, bead.as_deref(), drop.as_deref(), cli.json)
        }
        Cmd::Close { bead, reason } => cmd::close::run(&repo, &bead, &reason, cli.json),
        Cmd::Land { bead, all } => cmd::land::run(&repo, &bead, all, cli.json),
        Cmd::Status { attention } => cmd::status::run(&repo, attention, cli.json),
        Cmd::Mcp => cmd::mcp::run(&repo),
        Cmd::Init { prefix, write } => cmd::init::run(&repo, prefix.as_deref(), write, cli.json),
        Cmd::Install { write } => cmd::install::run(&repo, write, cli.json),
        Cmd::Worker {
            name,
            tmux,
            task,
            print,
            extra,
        } => cmd::launch::worker(&repo, name.as_deref(), &extra, tmux, task.as_deref(), print),
        Cmd::Coordinator { print, extra } => cmd::launch::coordinator(&repo, &extra, print),
        Cmd::Hook => cmd::hook::run(&repo),
        Cmd::Audit { since } => cmd::audit::run(&repo, since.as_deref(), cli.json),
        Cmd::Gc { keep_days, apply } => cmd::gc::run(&repo, keep_days, apply, cli.json),
        Cmd::Doctor => cmd::doctor::run(&repo, cli.json),
        Cmd::Selftest => cmd::selftest::run(cli.json),
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
