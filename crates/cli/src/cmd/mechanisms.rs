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
    /// Event lines with any of these `(command, decision)` pairs. Several, because one
    /// mechanism is often reachable by more than one entry point: the hand-over gate fires
    /// from the hook and from `air handover`, and the Stop nudge fires on `Stop` and on
    /// `SubagentStop` through the same function. Giving each trace its own row would mean
    /// copying one recorded condition onto several mechanisms, which is the invention this
    /// table exists to prevent (air-0y9).
    Decisions(&'static [(&'static str, &'static str)]),
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
}

impl Removal {
    pub fn text(&self) -> &'static str {
        match self {
            Removal::Unstated => "",
            Removal::Judgement(t) | Removal::ZeroFirings(t) => t,
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
        fires: Fires::Decisions(&[("hook.PreToolUse", "refuse"), ("handover", "refuse")]),
        removal: Removal::ZeroFirings("a full round passes with zero `handover-not-green` events"),
    },
    Mechanism {
        id: "handover-would-refuse",
        class: "refusal",
        what: "The same gate, advisory: reports what it would refuse without AIR_ENFORCE=1.",
        added: "2026-08-18 (plan 0001)",
        source: "docs/plans/0001-first-slice.md:143",
        fires: Fires::Decisions(&[
            ("hook.PreToolUse", "would-refuse"),
            ("handover", "would-refuse"),
        ]),
        // Recorded when the gate was designed: advisory in M0, "blocking only when the worker
        // has set awaiting_review/close in this turn and evidence is missing (M1, after one
        // round of advisory data)" (plan 0001, hook events table). air-i59 acted on that on
        // 2026-08-22 after the first bypass, which is what put AIR_ENFORCE=1 in the launcher.
        removal: Removal::Judgement(
            "one round of advisory data, then blocking (plan 0001, M0 -> M1); air-i59 acted on it, so what is left to decide is whether the advisory arm is still reachable",
        ),
    },
    Mechanism {
        id: "review-waiting",
        class: "attention",
        what: "A bead handed over and not yet landed is named, with how long it has waited.",
        added: "2026-08-22 (air-e7q)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("review-waiting"),
        // Recorded by the pass that kept it as a change-only push (air-s7c, owner
        // 2026-08-22). It is a Judgement rather than a counter: whether a push led to an
        // action is not something the ledger can see, which is why the "no downstream
        // action" metric was cut from the audit.
        removal: Removal::Judgement(
            "a round shows change-only pushes that led to no owner or coordinator action",
        ),
    },
    Mechanism {
        id: "cross-project-fence",
        class: "refusal",
        what: "A `tmux` command naming another project's session is denied; the refusal names \
               the fence, the project, and that reading and messaging stay open.",
        added: "2026-08-22 (air-0lk, narrowed by air-3oq)",
        source: "crates/cli/src/cmd/project.rs",
        fires: Fires::Decisions(&[("hook.PreToolUse", "refuse-cross-project")]),
        // air-0lk recorded the condition; air-3oq added that the messaging clause is gone
        // rather than suspended, after it broke the cross-project channel silently.
        removal: Removal::ZeroFirings(
            "a full quarter with zero cross-project denials AND the agent channel has its own project scoping; the messaging clause is not coming back, it is deleted",
        ),
    },
    Mechanism {
        id: "landed-not-closed",
        class: "attention",
        what: "A bead that landed while the merge CONTRADICTS one of its acceptance clauses: \
               a wrong close, not merely one Air could not read.",
        added: "2026-08-22 (air-ayp)",
        source: "crates/cli/src/cmd/acceptance.rs, crates/cli/src/cmd/land.rs",
        fires: Fires::Condition("landed-not-closed"),
        // Not ZeroFirings: this one firing is the mechanism working. adopter's closer put
        // 14 partial and 1 not-done bead into `closed` by never asking
        // (docs/plans/0029-bead-closure.md D.6, cited via air-ayp). It goes when acceptance is
        // machine-checkable by construction, at which point the merge either satisfies it or
        // does not and there is no judgement left to hold open.
        removal: Removal::Judgement(
            "acceptance criteria are machine-checkable by construction, so a landing either satisfies them or does not and nothing is held open for a person to read",
        ),
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
        // Recorded by the pass that kept it as a change-only push (air-s7c, owner
        // 2026-08-22). It is a Judgement rather than a counter: whether a push led to an
        // action is not something the ledger can see, which is why the "no downstream
        // action" metric was cut from the audit.
        removal: Removal::Judgement(
            "a round shows change-only pushes that led to no owner or coordinator action",
        ),
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
        id: "peer-warning",
        class: "warning",
        what: "Opening a file a peer is also editing warns once per session, naming them.",
        added: "2026-08-18 (plan 0001)",
        source: "crates/cli/src/cmd/hook.rs",
        fires: Fires::Decisions(&[("hook.PreToolUse", "warn")]),
        // Recorded by the pass that kept it (air-s7c). The "warns once per session" claim was
        // checked against the log before this was written, not assumed: 29 firings on
        // 2026-08-22 over 29 distinct (worker, path, peers), zero repeats, with 102 further
        // edits suppressed and recorded as `warn-repeat`. The dedupe works.
        removal: Removal::ZeroFirings(
            "a round passes with zero peer warnings, meaning file lanes alone keep two workers out of one file",
        ),
    },
    Mechanism {
        id: "stop-nudge",
        class: "nudge",
        what: "At WIP 0 with beads ready, the Stop hook names them once.",
        added: "2026-08-22 (air-09i)",
        source: "crates/hooks/src/gate.rs, stop_nudge",
        fires: Fires::Decisions(&[("hook.Stop", "nudge"), ("hook.SubagentStop", "nudge")]),
        removal: Removal::Judgement(
            "a round shows nudges that led to a claim <= nudges ignored, or workers claim the next bead unprompted in > 90% of hand-overs",
        ),
    },
    Mechanism {
        id: "claim-refusal",
        class: "refusal",
        what: "A claim is refused: held by a peer, labelled `owner`, closed, or bd said no.",
        added: "2026-08-20 (decisions: wrap beads, never watch it)",
        source: "crates/cli/src/cmd/claim.rs",
        fires: Fires::Decisions(&[("claim", "refuse")]),
        // Recorded by the do-less pass that kept it (air-s7c, docs/decisions.md 2026-08-22).
        // Both firings on 2026-08-22 were real collisions between two workers on one bead.
        removal: Removal::ZeroFirings(
            "a full round passes with zero claim refusals, meaning lane assignment alone keeps workers off each other's beads",
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
        fires: Fires::Decisions(&[("audit", "reported")]),
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
        fires: Fires::Decisions(&[("hook.PermissionDenied", "denied")]),
        // The incident (send-keys allowed once, then denied 30 min later by the permission
        // classifier, 2026-08-22) used to sit in this field, and the audit read it as
        // permanent. Nothing here is permanent: a rule never re-examined is the throttle the
        // do-less rule exists to prevent. Recorded properly by air-s7c.
        removal: Removal::ZeroFirings(
            "a round passes with zero denied send-keys attempts, meaning no coordinator reaches for it and SendMessage covers the need",
        ),
    },
];
