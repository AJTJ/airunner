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
    /// Give a bead back: bd status → open, ledger claim closed with a reason.
    Release {
        bead: String,
        /// landed | abandoned | reassigned | superseded | false-premise | owner-gated | unknown
        #[arg(long)]
        reason: String,
    },
    /// One line into the inbox. Workers capture; the coordinator triages. Never blocks you.
    Capture { text: String },
    /// Open captures, oldest first (coordinator).
    Inbox,
    /// Resolve a capture: --bead <id> after `bd create`, or --drop "<why>" (coordinator).
    Triage {
        id: String,
        #[arg(long)]
        bead: Option<String>,
        #[arg(long)]
        drop: Option<String>,
    },
    /// The coordinator's one screen: workers, sessions, claims, green, overlaps, inbox.
    Status {
        /// Only the conditions that need a human or the coordinator (empty when quiet).
        #[arg(long)]
        attention: bool,
    },
    /// MCP server over stdio: the coordinator's channel (push) plus tools and resources.
    Mcp,
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
        Cmd::Release { bead, reason } => cmd::claim::release(&repo, &bead, &reason, cli.json),
        Cmd::Capture { text } => cmd::capture::capture(&repo, &text, cli.json),
        Cmd::Inbox => cmd::capture::inbox(&repo, cli.json),
        Cmd::Triage { id, bead, drop } => {
            cmd::capture::triage(&repo, &id, bead.as_deref(), drop.as_deref(), cli.json)
        }
        Cmd::Status { attention } => cmd::status::run(&repo, attention, cli.json),
        Cmd::Mcp => cmd::mcp::run(&repo),
        Cmd::Hook => cmd::hook::run(&repo),
        Cmd::Doctor => cmd::doctor::run(&repo, cli.json),
        Cmd::Selftest => cmd::selftest::run(cli.json),
    };
    ExitCode::from(u8::try_from(code).unwrap_or(1))
}
