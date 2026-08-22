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
struct Cli {
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
    /// Resolve captures: --bead <id> after `bd create`, or --drop "<why>" (coordinator).
    /// Several captures at once map positionally, in one ledger transaction:
    /// `air triage c1 c2 --bead air-1 --bead air-2`. One --drop covers every capture named.
    /// Every bead is checked against bd first; an id bd does not have refuses the pass. A
    /// capture that was already triaged is re-pointed, old target named in the event line.
    Triage {
        #[arg(required = true)]
        id: Vec<String>,
        #[arg(long)]
        bead: Vec<String>,
        #[arg(long)]
        drop: Vec<String>,
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
    /// The coordinator's one screen: workers, sessions, claims, green, overlaps, inbox.
    Status {
        /// Only the conditions that need a human or the coordinator (empty when quiet).
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
    /// Ledger location, sizes, row counts, and the pragmas in effect.
    Doctor,
    /// Red/green probes for every check (a check that matches nothing prints red).
    Selftest,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let repo = cli
        .repo
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
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
        Cmd::Triage { id, bead, drop } => cmd::capture::triage(&repo, &id, &bead, &drop, cli.json),
        Cmd::Close { bead, reason } => cmd::close::run(&repo, &bead, &reason, cli.json),
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
        Cmd::Doctor => cmd::doctor::run(&repo, cli.json),
        Cmd::Selftest => cmd::selftest::run(cli.json),
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
