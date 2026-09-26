//! Every `command / decision` pair Air can write to the event log (air-hqj8).
//!
//! Owner, 2026-09-25: "ensure that everything air is doing is also being measured." `air audit`
//! counts a mechanism from the event lines its registry row names (`mechanisms::MECHANISMS`),
//! and until this file the set of lines Air could write was known only by reading every call
//! site. Six mechanisms shipped on 2026-09-25 with no row, and one line (`hook.Stop /
//! would-refuse`, 30 firings in this repo's record) had been written since air-bp0 with none.
//!
//! So a decision is a [`Trace`], and a `Trace` can only be one of the constants below: its
//! field that makes it constructible is private to this module, and `log_event` and the hook's
//! `Dispatched` take nothing else. A new decision is a new line here, and the probe
//! "audit: every decision Air can write has a registry row or is bookkeeping" reads [`ALL`] —
//! the compiler, not a scan of the source, is what makes the set complete.
//!
//! A command ending `.*` is the hook's: the line records the event that actually arrived
//! (`hook.<Event>`), and the constant says which events may carry it. Only bookkeeping words
//! are declared that way; a mechanism names its event.
//!
//! Removal: when the event log is written by something that declares its own schema (the
//! harness's own telemetry, say), so the set of lines is data rather than code.

/// One line's `command` and `decision`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trace {
    pub command: &'static str,
    pub decision: &'static str,
    _declared_here: (),
}

impl Trace {
    /// Whether an event line with `command` is this trace: exact, or under a `.*` prefix.
    pub fn matches_command(&self, command: &str) -> bool {
        match self.command.strip_suffix('*') {
            Some(prefix) => command.starts_with(prefix),
            None => self.command == command,
        }
    }
}

/// Tests read a decision as its word.
impl PartialEq<&str> for Trace {
    fn eq(&self, other: &&str) -> bool {
        self.decision == *other
    }
}

macro_rules! traces {
    ($($name:ident = $c:literal / $d:literal;)*) => {
        $(pub const $name: Trace = Trace { command: $c, decision: $d, _declared_here: () };)*
        /// Every trace above, for the probe.
        pub const ALL: &[Trace] = &[$($name),*];
    };
}

traces! {
    AUDIT_REPORTED = "audit" / "reported";

    LAUNCH_REFUSE_INSTALL_UNCOMMITTED = "launch" / "refuse-install-uncommitted";
    FLEET_UP = "fleet" / "up";
    INSTALL_RETIRE_SKILL = "install" / "retire-skill";
    PIN_DELEGATED = "pin" / "delegated";

    BATCH_CUT_REFUSE = "batch-cut" / "refuse";
    BATCH_CUT_DROP = "batch-cut" / "drop";
    BATCH_CUT_NO_PRECHECK = "batch-cut" / "no-precheck";
    BATCH_CUT_CUT = "batch-cut" / "cut";
    BATCH_CUT_DRY_RUN = "batch-cut" / "dry-run";

    CAPTURE_CAPTURED = "capture" / "captured";
    TRIAGE_NO_SUCH_BEAD = "triage" / "no-such-bead";
    TRIAGE_UNKNOWN = "triage" / "unknown";
    TRIAGE_TRIAGED = "triage" / "triaged";
    TRIAGE_PARTIAL = "triage" / "partial";

    CHANNEL_PUSHED = "channel.push" / "pushed";
    CHANNEL_DELIVERED = "channel.deliver" / "delivered";
    FANOUT_BEADS_READY = "fanout" / "beads-ready";
    FANOUT_BATCH_READY = "fanout" / "batch-ready";
    FANOUT_BATCH_RESULT = "fanout" / "batch-result";
    FLEET_STOP = "fleet" / "stop";
    FLEET_RESUME = "fleet" / "resume";
    FLEET_REFUSE_ROLE = "fleet" / "refuse-role";
    CLAIM_FLEET_STOPPED = "claim" / "fleet-stopped";
    BATCH_CUT_FLEET_STOPPED = "batch-cut" / "fleet-stopped";
    LAND_FLEET_STOPPED = "land" / "fleet-stopped";
    MCP_TOOL_TIMEOUT = "mcp.tool" / "timeout";

    CLAIM_REFUSE = "claim" / "refuse";
    CLAIM_NO_SUCH_BEAD = "claim" / "no-such-bead";
    CLAIM_TIMEOUT = "claim" / "timeout";
    CLAIM_BD_REFUSED = "claim" / "bd-refused";
    CLAIM_TIMEOUT_RETRY = "claim" / "timeout-retry";
    CLAIM_RECLAIMED = "claim" / "reclaimed";
    CLAIM_CLAIMED = "claim" / "claimed";
    CLAIM_CLAIMED_LATE = "claim" / "claimed-late";
    CLAIM_CLAIMED_RETRIED = "claim" / "claimed-retried";
    RELEASE_NO_SUCH_BEAD = "release" / "no-such-bead";
    RELEASE_TIMEOUT = "release" / "timeout";
    RELEASE_REFUSE = "release" / "refuse";
    RELEASE_BD_REFUSED = "release" / "bd-refused";
    RELEASE_RELEASED = "release" / "released";
    RELEASE_NO_CLAIM = "release" / "no-claim";

    CLOSE_REFUSE = "close" / "refuse";
    CLOSE_BD_REFUSED = "close" / "bd-refused";
    CLOSE_CLOSED = "close" / "closed";

    GC_REPORTED = "gc" / "reported";
    GC_COLLECTED = "gc" / "collected";

    HANDOVER_PASS = "handover" / "pass";
    HANDOVER_REFUSE = "handover" / "refuse";
    HANDOVER_WOULD_REFUSE = "handover" / "would-refuse";

    HOLDINGS_OK = "holdings" / "ok";

    HOOK_FAIL_OPEN = "hook.*" / "fail-open";
    HOOK_IGNORED = "hook.*" / "ignored";
    SESSION_START_REGISTERED = "hook.SessionStart" / "registered";
    SESSION_END_ENDED = "hook.SessionEnd" / "ended";
    PERMISSION_DENIED_DENIED = "hook.PermissionDenied" / "denied";
    POST_TOOL_USE_FAILURE_FAILED = "hook.PostToolUseFailure" / "failed";
    POST_TOOL_USE_OBSERVED = "hook.PostToolUse" / "observed";
    POST_TOOL_USE_JOURNALED = "hook.PostToolUse" / "journaled";
    POST_TOOL_USE_RELEASED = "hook.PostToolUse" / "released";
    STOP_OBSERVED = "hook.Stop" / "observed";
    STOP_NUDGE = "hook.Stop" / "nudge";
    STOP_PASS = "hook.Stop" / "pass";
    STOP_NO_CLAIM = "hook.Stop" / "no-claim";
    STOP_WOULD_REFUSE = "hook.Stop" / "would-refuse";
    STOP_WOULD_REFUSE_REPEAT = "hook.Stop" / "would-refuse-repeat";
    SUBAGENT_STOP_OBSERVED = "hook.SubagentStop" / "observed";
    NOTIFICATION_STOPPED = "hook.Notification" / "stopped";
    NOTIFICATION_OBSERVED = "hook.Notification" / "observed";
    STOP_FAILURE_STOPPED = "hook.StopFailure" / "stopped";
    STOP_FAILURE_OBSERVED = "hook.StopFailure" / "observed";
    PRE_TOOL_USE_REFUSE_OUTSIDE_WORKTREE = "hook.PreToolUse" / "refuse-outside-worktree";
    PRE_TOOL_USE_WARN = "hook.PreToolUse" / "warn";
    PRE_TOOL_USE_WARN_REPEAT = "hook.PreToolUse" / "warn-repeat";
    PRE_TOOL_USE_CLEAR = "hook.PreToolUse" / "clear";
    PRE_TOOL_USE_MESSAGED = "hook.PreToolUse" / "messaged";
    PRE_TOOL_USE_OBSERVED = "hook.PreToolUse" / "observed";
    PRE_TOOL_USE_LEASE_HELD = "hook.PreToolUse" / "lease-held";
    PRE_TOOL_USE_LEASE_REFUSE = "hook.PreToolUse" / "lease-refuse";
    PRE_TOOL_USE_LEASE_WOULD_REFUSE = "hook.PreToolUse" / "lease-would-refuse";
    PRE_TOOL_USE_PASS = "hook.PreToolUse" / "pass";
    PRE_TOOL_USE_REFUSE = "hook.PreToolUse" / "refuse";
    PRE_TOOL_USE_WOULD_REFUSE = "hook.PreToolUse" / "would-refuse";

    LAND_REFUSE_ROLE = "land" / "refuse-role";
    LAND_REFUSE = "land" / "refuse";
    LAND_ERROR = "land" / "error";
    LAND_NONE_LANDABLE = "land" / "none-landable";
    LAND_REFUSE_IN_FLIGHT = "land" / "refuse-in-flight";
    LAND_DESPITE_INFLIGHT = "land" / "despite-inflight";
    LAND_MAIN_READERS = "land" / "main-readers";
    LAND_LANDED = "land" / "landed";
    LAND_STOPPED = "land" / "stopped";

    LEASE_TAKEN = "lease.take" / "taken";
    LEASE_ALREADY_MINE = "lease.take" / "already-mine";
    LEASE_TAKEN_AFTER_BREAK = "lease.take" / "taken-after-break";
    LEASE_DENIED = "lease.take" / "denied";
    LEASE_RELEASED = "lease.release" / "released";
    LEASE_NOT_HELD = "lease.release" / "not-held";
    LEASE_BROKEN = "lease.break" / "broken";

    RECORD_GREEN = "record" / "green";
    RECORD_RED = "record" / "red";
    RECORD_KILLED = "record" / "killed";

    STATUS_QUIET = "status" / "quiet";
    STATUS_ATTENTION = "status" / "attention";
    STATUS_MAIN_CHECKOUT_SESSION = "status" / "main-checkout-session";
    STATUS_ATTENTION_QUIET = "status.attention" / "quiet";
    STATUS_ATTENTION_ATTENTION = "status.attention" / "attention";
}
