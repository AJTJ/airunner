//! One home for every mechanism Air ships and the condition under which it is removed
//! (air-zyo).
//!
//! CLAUDE.md requires each constraint, hook, deny rule, role line or procedure to name the
//! recorded failure it prevents AND the condition under which it is removed. Until now those
//! conditions were prose scattered across `docs/rules/roles.md`, bead descriptions, and doc
//! comments, which is why the do-less pass the skill prescribes has never actually been run:
//! running it meant hand-reading a day of NDJSON and grepping five files.
//!
//! This table is that home. It is compiled in rather than shipped as data for the same reason
//! `install::SURFACE` is: the probe and the binary cannot then disagree, and there is no file
//! to keep in sync. `air audit` reads it and prints, per mechanism, what the ledger says.
//!
//! **This file holds no judgement, and it invents nothing.** A mechanism's `removal` is what
//! was actually recorded when it was added, in the words of whoever added it. Where nothing
//! was recorded the entry says `Unstated` and the audit reports a defect — writing a
//! plausible-sounding condition in place of the missing one would hide exactly what this
//! table is for. Deciding what to cut is the coordinator's pass, not something stated here.

use serde::Serialize;

/// What counts as this mechanism firing, in the event log.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum Fires {
    /// An attention condition of this kind appeared in a `status`/`status.attention` event's
    /// `inputs.conditions` list (entries are `kind:subject`).
    Condition(&'static str),
    /// An event line with this command and this decision.
    Decision {
        command: &'static str,
        decision: &'static str,
    },
}

/// The recorded condition under which a mechanism is removed.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum Removal {
    /// Nothing was recorded. `air audit` reports this as a defect rather than skipping it:
    /// a mechanism nobody wrote a removal condition for is the one that outlives its reason.
    Unstated,
    /// Recorded, but a person has to decide. The audit prints the text and says so; it does
    /// not pretend to evaluate it.
    Judgement(&'static str),
    /// Recorded as: remove when this stops firing. The audit can check that directly
    /// against the counter.
    ZeroFirings(&'static str),
    /// Recorded as permanent, with the incident that settled it.
    Never(&'static str),
}

impl Removal {
    pub fn text(&self) -> &'static str {
        match self {
            Removal::Unstated => "",
            Removal::Judgement(t) | Removal::ZeroFirings(t) | Removal::Never(t) => t,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Mechanism {
    pub id: &'static str,
    /// hook | attention | refusal | nudge | warning
    pub class: &'static str,
    /// What it does, in one line. Descriptive, never a recommendation.
    pub what: &'static str,
    /// When it was added, and the bead, so a row is traceable to its reason.
    pub added: &'static str,
    /// Where the condition below is recorded in prose, so the two can be compared.
    pub source: &'static str,
    pub fires: Fires,
    pub removal: Removal,
}

/// Every mechanism Air ships that leaves a trace in the event log.
///
/// Append when a mechanism is added; the audit reports a mechanism with `Removal::Unstated`
/// as a defect, so an entry with nothing recorded is visible rather than silently absent.
pub const MECHANISMS: &[Mechanism] = &[
    Mechanism {
        id: "handover-gate",
        class: "refusal",
        what: "`awaiting_review`/close needs a recorded green at HEAD that contains main.",
        added: "2026-08-22 (air-i59)",
        source: "docs/rules/roles.md, Worker section",
        fires: Fires::Decision {
            command: "hook.PreToolUse",
            decision: "refuse",
        },
        removal: Removal::ZeroFirings("a full round passes with zero `handover-not-green` events"),
    },
    Mechanism {
        id: "handover-would-refuse",
        class: "refusal",
        what: "The same gate, advisory: reports what it would refuse without AIR_ENFORCE=1.",
        added: "2026-08-18 (plan 0001)",
        source: "crates/cli/src/cmd/hook.rs",
        fires: Fires::Decision {
            command: "hook.PreToolUse",
            decision: "would-refuse",
        },
        removal: Removal::ZeroFirings(
            "every launcher sets AIR_ENFORCE=1, at which point the advisory path is unreachable",
        ),
    },
    Mechanism {
        id: "review-waiting",
        class: "attention",
        what: "A bead handed over and not yet landed is named, with how long it has waited.",
        added: "2026-08-22 (air-e7q)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("review-waiting"),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "idle-without-claim",
        class: "attention",
        what: "An idle worker holding no claim while beads are ready.",
        added: "2026-08-22 (air-e7q)",
        source: "crates/cli/src/cmd/status.rs, Thresholds::idle_noclaim_min",
        fires: Fires::Condition("idle-without-claim"),
        removal: Removal::ZeroFirings(
            "zero firings in a round once the Stop nudge (air-09i) is in",
        ),
    },
    Mechanism {
        id: "handover-not-green",
        class: "attention",
        what: "A worker handed over from a HEAD with no recorded green.",
        added: "2026-08-21 (plan 0006)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("handover-not-green"),
        removal: Removal::ZeroFirings(
            "a full round passes with zero `handover-not-green` events (same condition as the gate it reports)",
        ),
    },
    Mechanism {
        id: "owner-decision-waiting",
        class: "attention",
        what: "Captures sit in the owner queue, with the age of the oldest.",
        added: "2026-08-21 (plan 0006)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("owner-decision-waiting"),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "stuck",
        class: "attention",
        what: "A session in the same state past the stuck threshold.",
        added: "2026-08-20 (plan 0004)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("stuck"),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "gone-with-claim",
        class: "attention",
        what: "A claim held by a session whose pid is gone.",
        added: "2026-08-20 (plan 0004)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("gone-with-claim"),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "peer-warning",
        class: "warning",
        what: "Opening a file a peer is also editing warns once per session, naming them.",
        added: "2026-08-18 (plan 0001)",
        source: "crates/cli/src/cmd/hook.rs",
        fires: Fires::Decision {
            command: "hook.PreToolUse",
            decision: "warn",
        },
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "peer-warning-repeat",
        class: "warning",
        what: "The same peer warning, re-armed because the set of peers on that path changed.",
        added: "2026-08-21 (plan 0006)",
        source: "crates/cli/src/cmd/hook.rs",
        fires: Fires::Decision {
            command: "hook.PreToolUse",
            decision: "warn-repeat",
        },
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "stop-nudge",
        class: "nudge",
        what: "At WIP 0 with beads ready, the Stop hook names them once.",
        added: "2026-08-22 (air-09i)",
        source: "crates/hooks/src/gate.rs, stop_nudge",
        fires: Fires::Decision {
            command: "hook.Stop",
            decision: "nudge",
        },
        removal: Removal::Judgement(
            "a round shows nudges that led to a claim <= nudges ignored, or workers claim the next bead unprompted in > 90% of hand-overs",
        ),
    },
    // The audit is not exempt from its own instrument: it carries a removal condition and
    // shows up as a row like everything else (air-zyo).
    Mechanism {
        id: "audit",
        class: "report",
        what: "Prints what the ledger says about every mechanism, for a do-less pass.",
        added: "2026-08-22 (air-zyo)",
        source: "crates/cli/src/cmd/audit.rs",
        fires: Fires::Decision {
            command: "audit",
            decision: "reported",
        },
        removal: Removal::Judgement(
            "two consecutive rounds produce zero stale mechanisms, meaning the tree is small enough that a coordinator sees the whole thing without help",
        ),
    },
    Mechanism {
        id: "coordinator-send-keys",
        class: "refusal",
        what: "tmux panes are the owner's to watch; the coordinator reaches workers by SendMessage.",
        added: "2026-08-22",
        source: "docs/rules/roles.md, Coordinator section",
        fires: Fires::Decision {
            command: "hook.PermissionDenied",
            decision: "denied",
        },
        removal: Removal::Never(
            "send-keys was allowed once and denied 30 min later by the permission classifier, 2026-08-22",
        ),
    },
];
