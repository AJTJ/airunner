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
        // `hook.handover` is what the first slice called the same gate before hook events
        // were dispatched by name (2026-08-18). It is not a second mechanism and never was, so
        // it is a second entry point here rather than a row of its own — which is exactly what
        // `Fires::Decisions` is for. Without it the audit reported Air's ONE refusal as a
        // firing with no registry row (air-8br).
        fires: Fires::Decisions(&[
            ("hook.PreToolUse", "refuse"),
            ("handover", "refuse"),
            ("hook.handover", "refuse"),
        ]),
        removal: Removal::ZeroFirings("a full round passes with zero `handover-not-green` events"),
    },
    Mechanism {
        id: "handover-would-refuse",
        class: "refusal",
        what: "The same gate, advisory: reports what it would refuse without AIR_ENFORCE=1.",
        added: "2026-08-18 (plan 0001)",
        source: "docs/plans/0001-first-slice.md:143",
        // `hook.handover` and `hook.stop` are the first slice's names for the same advisory
        // gate (2026-08-18), kept as entry points so days recorded then still attribute
        // (air-8br).
        fires: Fires::Decisions(&[
            ("hook.PreToolUse", "would-refuse"),
            ("handover", "would-refuse"),
            ("hook.handover", "would-refuse"),
            ("hook.stop", "would-refuse"),
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
        id: "rewound-and-carried",
        class: "attention",
        what: "A rewound merge that some worktree's HEAD still contains, named with what its \
               holder must do. Read from history only: no new rewind can occur.",
        added: "2026-08-29 (air-ob0), narrowed the same day (air-odv)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("rewound-and-carried"),
        // air-ob0 shipped this to cover a real obligation: a rollback un-lands a branch from
        // main and cannot un-merge it from a worker who took it during the armed window.
        // air-odv removed the window hours later — main is fast-forwarded onto an already-green
        // commit, so nothing can rewind and no NEW rewound row can be written. What is left is
        // reading the two rows that already exist, which is why the condition is a count rather
        // than a judgement: when no rewound row is still carried, it has nothing left to say.
        removal: Removal::ZeroFirings(
            "no `rewound` landing row is still contained by any worktree HEAD; air-odv made new ones impossible, so this empties rather than being argued about",
        ),
    },
    Mechanism {
        id: "landable",
        class: "attention",
        what: "A branch `air land --all` would take right now: green at its head with main \
               merged, naming the beads it carries. Once per branch head, never while it sits.",
        added: "2026-08-29 (air-03w)",
        source: "crates/cli/src/cmd/status.rs",
        fires: Fires::Condition("landable"),
        // The worker signalling on close is the intent (roles.md, owner 2026-08-29); this is
        // the failsafe, so a missed signal is not a lost one. If the signal turns out to be
        // reliable, the failsafe has nothing left to catch — and that is a count, not a
        // judgement: every landable branch landed before this pushed.
        removal: Removal::Judgement(
            "a round in which every landable branch was landed before this condition pushed, i.e. the worker's own signal arrived first every time",
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
    // air-sze: the five condition kinds that shipped with no row at all. An unregistered
    // DECISION was merely uncounted (air-8br); an unregistered CONDITION was invisible, because
    // the audit can only count kinds the registry already names. All five read zero over
    // 38,130 event lines across eight days — counted from the raw log, since `air audit` by
    // definition could not see them.
    //
    // Zero firings is not one verdict here. It means two different things, and the difference is
    // the whole judgement:
    Mechanism {
        id: "gone-with-claim",
        class: "attention",
        what: "No live session, but the worker still holds a claim.",
        added: "2026-08-20 (plan 0004); launch grace added 2026-08-29 (air-eiv)",
        source: "crates/cli/src/cmd/status.rs, the `None if has_claim` arm",
        fires: Fires::Condition("gone-with-claim"),
        // KEPT, and the comment above it in status.rs was wrong rather than the code. It said
        // this was "deleted on 2026-08-22 (air-s7c)" because "a dead session holding a claim
        // now falls through to the ordinary session states below, which do fire". It never was
        // deleted, and that reason cannot hold for this arm: it is the arm for a worker with NO
        // session row, so there are no session states below to fall through to. A crashed
        // worker holding a bead is exactly what nothing else reports.
        removal: Removal::Judgement(
            "a round in which a worker's session dies holding a claim and some other condition names it first; until then this is the only thing that would",
        ),
    },
    Mechanism {
        id: "idle-with-claim",
        class: "attention",
        what: "A worker idle past the threshold while holding a bead.",
        added: "2026-08-20 (plan 0004)",
        source: "crates/cli/src/cmd/status.rs, Thresholds::idle_with_claim_min",
        fires: Fires::Condition("idle-with-claim"),
        // KEPT on the wedge argument, not on its count. `stuck` has never fired either and
        // records no removal condition, so with both of these gone nothing at all notices a
        // worker that has stalled holding work — which is the gap the coordinator's 5-minute
        // heartbeat was added to cover (air-arq). Delete the detector and the heartbeat is the
        // only thing left looking.
        removal: Removal::Judgement(
            "the heartbeat or `stuck` catches a stalled worker holding a bead first, twice, so this is the second thing to notice rather than the only one",
        ),
    },
    Mechanism {
        id: "silent-with-claim",
        class: "attention",
        what: "No hook event for the threshold while holding a bead; the session may have died.",
        added: "2026-08-20 (plan 0004)",
        source: "crates/cli/src/cmd/status.rs, Thresholds::silent_with_claim_min",
        fires: Fires::Condition("silent-with-claim"),
        removal: Removal::Judgement(
            "same as `idle-with-claim`: something else names a silent worker holding a bead first, twice",
        ),
    },
    Mechanism {
        id: "lease-held-by-dead-session",
        class: "attention",
        what: "A lease whose holder's process is gone or was reused.",
        added: "2026-08-21 (owner ruling A; ported from adopter's lease.sh)",
        source: "crates/cli/src/cmd/status.rs, defect() in cmd/lease.rs",
        fires: Fires::Condition("lease-held-by-dead-session"),
        // KEPT, and for a different reason from the three above: this one's zero is about
        // USAGE, not about the mechanism. Addressed to the waiter rather than the holder
        // since air-q9c, like `lease-stale` below. The `leases` table has zero rows in this repo because
        // `air lease` is unused here. adopter uses it every round. Deleting on our zero is
        // precisely the error the owner reversed on `air lease` itself (air-uae, 2026-08-29):
        // a verdict from an absence in one repo is not a verdict about a mechanism.
        removal: Removal::Judgement(
            "a round in a repo that actually takes leases shows a dead holder going unnoticed or the condition firing on a healthy one; a zero in a repo with no leases says nothing",
        ),
    },
    Mechanism {
        id: "lease-stale",
        class: "attention",
        what: "A lease whose heartbeat has aged past the stale threshold.",
        added: "2026-08-21 (owner ruling A; ported from adopter's lease.sh)",
        source: "crates/cli/src/cmd/status.rs, defect() in cmd/lease.rs",
        fires: Fires::Condition("lease-stale"),
        // Same zero-is-about-usage argument. The KNOWN defect this row was registered to hold
        // — the condition addressed to the lease's HOLDER, offering them `air lease break` on
        // the lease they were using — is FIXED as of air-q9c: both lease conditions now name
        // whoever is waiting in `lease_wants`, and a defect nobody is waiting on produces no
        // condition at all. the adopter reported six firings in one day on healthy leases (their
        // ad-m07x); the attribution half reproduced here, captured 01M17J9NSHXZBH56K7MVY7XAM8.
        //
        // So the first half of the recorded condition is discharged and the second is what is
        // left to observe, which needs a repo that actually takes leases.
        removal: Removal::Judgement(
            "attribution fixed (air-q9c); what is left is a round in a lease-using repo showing it firing at someone who can act on it, or showing nobody acting on it at all",
        ),
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
        // air-7q5 asked whether this is an auto-start, since starting a session must not start
        // work. It is NOT, and it is left alone rather than gated. A Stop hook fires only after
        // the model has produced a turn, and a worker launched with no `--task` is given no
        // prompt at all (`worker_argv_tmux`; the roles prose arrives via
        // `--append-system-prompt-file`, which is context, not a turn). So the session has
        // already been triggered by the time this can fire, and gating it on "has been
        // triggered" would be a mechanism for a state that is unreachable. Probe:
        // "launch: a task is the prompt; no task means no prompt, so an untriggered worker
        // never runs", which carries a declared mutation.
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
    Mechanism {
        id: "gc",
        class: "report",
        what: "Names what is collectable from `.air/events/` under the stated retention, and \
               removes it only when asked twice.",
        added: "2026-08-29 (air-i7s)",
        source: "crates/cli/src/cmd/gc.rs",
        // Registered because air-8br found it firing invisibly: its `reported` decision was in
        // the audit's bookkeeping list, a word `air audit` had a registry row for and this did
        // not. A command that reports is a mechanism, and declares itself.
        removal: Removal::Judgement(
            "`events::append` bounds the stream itself, or nothing reads a day older than the window, at which point the window goes and this goes with it",
        ),
        fires: Fires::Decisions(&[("gc", "reported"), ("gc", "collected")]),
    },
    Mechanism {
        id: "land-refusal",
        class: "refusal",
        what: "A landing is refused: not the coordinator, main dirty or moved off main, the \
               branch does not contain main, or its head carries no recorded green.",
        added: "2026-08-22 (air-3pz)",
        source: "crates/cli/src/cmd/land.rs, may_land and the precondition checks",
        fires: Fires::Decisions(&[("land", "refuse")]),
        // Not ZeroFirings. A round with no land refusals means every landing was prepared
        // correctly, which is the mechanism working, not the mechanism being unnecessary —
        // the opposite reading from `peer-warning`, where a zero means the lanes did the job
        // instead. What would retire this is main ceasing to be a thing only the coordinator
        // writes to, which is a decision rather than a count.
        removal: Removal::Judgement(
            "main stops being a branch only the coordinator writes to, at which point the role half goes and the green-at-head half belongs to the gate",
        ),
    },
    Mechanism {
        id: "close-refusal",
        class: "refusal",
        what: "`air close` is the coordinator's; a worker asking for it is refused and told so.",
        added: "2026-08-20 (decisions: the worker closes its own bead with bd, not with air close)",
        source: "crates/cli/src/cmd/close.rs, may_close",
        fires: Fires::Decisions(&[("close", "refuse")]),
        removal: Removal::ZeroFirings(
            "a round passes with zero close refusals, meaning no worker reaches for `air close` and the deny list alone covers it",
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
