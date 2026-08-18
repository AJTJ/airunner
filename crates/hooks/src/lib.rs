//! Claude Code hook contract and Air's pure decision logic.
//!
//! Sources: https://code.claude.com/docs/en/hooks (input JSON, exit-code semantics, JSON output
//! fields), verified 2026-08-18 in docs/research/verification/ticks/2026-08-18-0245-claude-code-hook-edge-cases.md.
//! Rules (plan 0001 §5): answer fast, fail open, never block on a prompt, never block a WIP
//! commit, refuse only at hand-over — and only once advisory mode has run a round.

pub mod gate;
pub mod input;
pub mod journal;

pub use gate::{GateFacts, Verdict, handover_verdict};
pub use input::{HookEvent, HookInput};

/// What the hook binary writes to stdout / returns as exit code.
///
/// Exit 0 = allow (stdout may carry JSON with `additionalContext`); exit 2 = block, stderr is
/// shown to the model. Anything else is treated by Claude Code as a non-blocking error, which
/// for Air means "allow" (fail-open).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookOutcome {
    Allow { context: Option<String> },
    Block { reason: String },
}

impl HookOutcome {
    pub fn exit_code(&self) -> i32 {
        match self {
            HookOutcome::Allow { .. } => 0,
            HookOutcome::Block { .. } => 2,
        }
    }

    /// JSON for stdout on allow, per the hooks docs (`hookSpecificOutput.additionalContext`
    /// for PreToolUse/SessionStart-style events; a top-level `systemMessage` is also accepted).
    pub fn stdout_json(&self, event: HookEvent) -> Option<serde_json::Value> {
        match self {
            HookOutcome::Allow { context: Some(ctx) } => Some(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": event.as_str(),
                    "additionalContext": ctx,
                }
            })),
            _ => None,
        }
    }
}
