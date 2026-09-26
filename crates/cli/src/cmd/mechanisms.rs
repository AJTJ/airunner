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
    /// Waits that reached one of these budgets (`air_ledger::budgets` names), read from the
    /// `budgets` object on every event line: each hit is a decision taken on less than was
    /// asked for (air-d75). One row per recorded reason, several names where one reason
    /// covers them (air-hqj8).
    Budget(&'static [&'static str]),
}

/// The recorded condition under which a mechanism is removed.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum Removal {
    /// Nothing was recorded. `air audit` reports this as a defect rather than skipping it:
    /// a mechanism nobody wrote a removal condition for is the one that outlives its reason.
    ///
    /// air-byw: `stuck` was the last mechanism carrying this on 2026-08-29, and the variant
    /// was kept unconstructed so that the next author with nothing to record would not invent
    /// text. air-hqj8 is that author: the budget rows and `air lease take`'s refusal were
    /// registered on 2026-09-25 with nothing recorded, and say so.
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
    /// hook | attention | refusal | nudge | warning | report | budget
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
///
/// **Deleted, so nobody re-adds them without evidence:**
///
/// - `gone-with-claim` (2026-08-20, removed 2026-08-22, air-s7c): zero firings in the audited
///   window and never in any recorded day. A dead session holding a claim falls through to the
///   ordinary session states, which do fire.
/// - `cross-project-fence` (2026-08-22, removed 2026-08-29, air-9u6): zero firings ever.
/// - `stuck` (2026-08-20, plan 0004; removed 2026-09-06, air-12k, owner ruling "sure, both"):
///   a session waiting on a permission prompt past a threshold. Zero firings in every recorded
///   day, and the zero was case 3b, not case 1: the state was set only by
///   `HookEvent::PermissionRequest`, and `hook.PermissionRequest` arrived 0 times in 39,071
///   event lines over 8 days because the fleet runs in auto mode (`permissions.defaultMode:
///   auto`, `skipAutoPermissionPrompt: true`), so no prompt is ever shown and nothing waits on
///   one (air-byw). Proposed for deletion on the count by air-dqw and reverted on that finding;
///   deleted now on a different ground: a condition whose input the configuration suppresses
///   is a promise, not a mechanism, and the coordinator's 5-minute heartbeat did every catch
///   in the 2026-09-05 round. The heartbeat is the failsafe (`docs/rules/roles.md`). A wedge
///   that auto mode would show as a prompt is now caught by `idle-with-claim` or the
///   heartbeat, not by a state nothing ever wrote.
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
        id: "edit-outside-worktree",
        class: "refusal",
        what: "A worker's Edit/Write whose resolved path leaves its worktree is denied; the \
               harness's --worktree isolation it replaces is off.",
        added: "2026-09-06 (air-8gj)",
        source: "crates/hooks/src/fence.rs; private/notes/2026-09-06-answers-worktree-and-verify.md",
        fires: Fires::Decisions(&[("hook.PreToolUse", "refuse-outside-worktree")]),
        removal: Removal::Judgement(
            "the harness keys its isolation on the cwd rather than the flag; or a round records zero refuse-outside-worktree firings AND the owner prefers the harness block back",
        ),
    },
    Mechanism {
        id: "lease-needed",
        class: "refusal",
        what: "A Bash command matching a pattern under \"leases\" in .claude/air.json, from a \
               worker or coordinator that does not hold that lease: refused under \
               AIR_ENFORCE=1, advised otherwise. The owner is never asked.",
        added: "2026-09-25 (from an adopter's fleet protocol)",
        source: "crates/cli/src/cmd/hook.rs lease_gate; crates/cli/src/cmd/lease.rs",
        fires: Fires::Decisions(&[
            ("hook.PreToolUse", "lease-refuse"),
            ("hook.PreToolUse", "lease-would-refuse"),
        ]),
        removal: Removal::Judgement(
            "the harness can scope a tool permission to a held resource; or a round with `leases` declared records `lease-held` decisions (matching commands ran) and zero lease-refuse or lease-would-refuse",
        ),
    },
    Mechanism {
        id: "handover-would-refuse",
        class: "refusal",
        what: "The same gate, advisory: reports what it would refuse without AIR_ENFORCE=1.",
        added: "2026-08-18 (plan 0001)",
        source: "docs/design.md §6.2 (plan 0001 §5 at the time)",
        // `hook.handover` and `hook.stop` are the first slice's names for the same advisory
        // gate (2026-08-18), kept as entry points so days recorded then still attribute
        // (air-8br).
        fires: Fires::Decisions(&[
            ("hook.PreToolUse", "would-refuse"),
            ("handover", "would-refuse"),
            ("hook.handover", "would-refuse"),
            ("hook.stop", "would-refuse"),
            // The Stop hook's advisory under its dispatched name, written since the event was
            // dispatched by name and registered by nobody until air-hqj8 (30 in this repo's
            // record on 2026-09-25).
            ("hook.Stop", "would-refuse"),
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
        what: "A bead that landed with an acceptance clause naming a file the merge did not \
               change. A lookup that did not answer, not a contradiction and not by itself a \
               wrong close: of nine standing firings on 2026-09-06, six were clauses that held \
               (air-k6uh).",
        added: "2026-08-22 (air-ayp)",
        source: "crates/cli/src/cmd/acceptance.rs, crates/cli/src/cmd/land.rs",
        fires: Fires::Condition("landed-not-closed"),
        // Not ZeroFirings: this one firing is the mechanism working. The adopter's closer put
        // 14 partial and 1 not-done bead into `closed` by never asking
        // (the adopter's plan 0029 on bead closure, D.6, cited via air-ayp). It goes when acceptance is
        // machine-checkable by construction, at which point the merge either satisfies it or
        // does not and there is no judgement left to hold open.
        removal: Removal::Judgement(
            "acceptance criteria are machine-checkable by construction, so a landing either satisfies them or does not and nothing is held open for a person to read",
        ),
    },
    Mechanism {
        id: "closed-not-landed",
        class: "attention",
        what: "A bead bd has closed whose commits are in no tree but its author's worktree. \
               The inverse of the one above: closed and landed was reported, closed and NOT \
               landed was not, and no other condition reaches it — `landable` needs a branch \
               containing main so it goes quiet the moment main moves.",
        added: "2026-09-07 (air-gazh)",
        source: "crates/cli/src/cmd/status.rs (closed_not_landed)",
        fires: Fires::Condition("closed-not-landed"),
        // The removal condition is deliberately NOT "when it stops firing" (air-gazh names
        // this): a quiet round is this mechanism working. This repo had zero on 2026-09-07
        // only because its coordinator landed every branch within minutes of each close, and
        // an adopter that batches had six at once the same day. Absence here measures the
        // landing cadence, not the mechanism.
        //
        // So the condition names what would have to change about LANDING for the state to
        // become unreachable, which is a fact anyone can check without waiting for a number.
        removal: Removal::Judgement(
            "landing no longer requires a branch to contain main at the moment someone looks — a recorded green can be landed after main moves without a re-merge — so a closed bead's commits cannot be stranded by main moving underneath them",
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
    // `owner-decision-waiting` was here (plan 0006): captures sitting in the owner queue,
    // with the age of the oldest. DELETED by air-uef (owner, 2026-09-05) with the queue it
    // watched; the owner's queue is beads labelled `owner`, counted on the `ready:` line.
    // `stuck` was here (plan 0004). DELETED by air-12k (owner, 2026-09-06); the record is in
    // this file's header list so nobody re-adds it on the zero that never justified it.
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
        // KEPT on the wedge argument, not on its count. With `stuck` deleted (air-12k) this is
        // the one condition that notices a worker stalled holding work; the coordinator's
        // 5-minute heartbeat (air-arq) is the failsafe behind it. Delete the detector and the
        // heartbeat is the only thing left looking.
        removal: Removal::Judgement(
            "the heartbeat catches a stalled worker holding a bead first, twice, so this is the second thing to notice rather than the only one",
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
        added: "2026-08-21 (owner ruling A; ported from the adopter's lease.sh)",
        source: "crates/cli/src/cmd/status.rs, defect() in cmd/lease.rs",
        fires: Fires::Condition("lease-held-by-dead-session"),
        // KEPT, and for a different reason from the three above: this one's zero is about
        // USAGE, not about the mechanism. Addressed to the waiter rather than the holder
        // since air-q9c, like `lease-stale` below. The `leases` table has zero rows in this repo because
        // `air lease` is unused here. The adopter uses it every round. Deleting on our zero is
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
        added: "2026-08-21 (owner ruling A; ported from the adopter's lease.sh)",
        source: "crates/cli/src/cmd/status.rs, defect() in cmd/lease.rs",
        fires: Fires::Condition("lease-stale"),
        // Same zero-is-about-usage argument. The KNOWN defect this row was registered to hold
        // — the condition addressed to the lease's HOLDER, offering them `air lease break` on
        // the lease they were using — is FIXED as of air-q9c: both lease conditions now name
        // whoever is waiting in `lease_wants`, and a defect nobody is waiting on produces no
        // condition at all. The adopter reported six firings in one day on healthy leases (their
        //); the attribution half reproduced here, captured 01M17J9NSHXZBH56K7MVY7XAM8.
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
        // `hook.SubagentStop` was a trace here until air-bp0: a subagent stopping is not the
        // worker stopping, and the nudge (with its `bd ready` confirm) no longer runs there.
        fires: Fires::Decisions(&[("hook.Stop", "nudge")]),
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
        what: "A landing is refused: main dirty or moved off main, the branch does not contain \
               main, or its head carries no recorded green. The role refusal is \
               `land-role-refusal` since air-hqj8.",
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
        id: "land-in-flight-refusal",
        class: "refusal",
        what: "`air land` refuses while any verify is in flight, naming each run and its pid; \
               landing would move main and destroy every one of them.",
        added: "2026-09-05 (air-1bm; the warning it replaces was air-4cr, 2026-08-29)",
        source: "crates/cli/src/cmd/land.rs, in_flight_refusal",
        fires: Fires::Decisions(&[("land", "refuse-in-flight")]),
        // The adopter : the warning fired, was read, and the landing went ahead anyway,
        // 1,199 s of destroyed verify in two incidents plus six more runs invalidated. Two
        // ways to retire it, both counts: overrides at zero (below) mean it is only ever
        // waited out and could be a plain wait; refusals at zero while verifies and landings
        // overlap mean the coordinator waits without being told.
        removal: Removal::ZeroFirings(
            "a full round with zero `--despite-inflight` overrides, or a round with zero refusals while landings and verifies overlap, both measured from the landings and verify_inflight tables",
        ),
    },
    Mechanism {
        id: "land-main-readers",
        class: "warning",
        what: "`air land` names, before moving main, every process that is not a Claude Code \
               session with its cwd in the main checkout. A warning; nothing is refused.",
        added: "2026-09-25 (owner; an adopter's `make tree-readers`, 2026-09-07)",
        source: "crates/cli/src/cmd/readers.rs, main_warning",
        fires: Fires::Decisions(&[("land", "main-readers")]),
        // The subject is a landing with an unrecorded run in main. With no session in the
        // main checkout (docs/design.md §10) that is a program somebody ran by hand, so a zero here is
        // only evidence over a round in which landings happened (the `land` rows say so).
        removal: Removal::ZeroFirings(
            "a full round of landings with zero firings, or every run in the main checkout recorded through `air record`, so the in-flight refusal already sees it",
        ),
    },
    Mechanism {
        id: "land-in-flight-override",
        class: "report",
        what: "`air land --despite-inflight` landed over a verify in flight; the runs destroyed \
               are on the landings row (`despite_inflight`).",
        added: "2026-09-05 (air-1bm)",
        source: "crates/cli/src/cmd/land.rs, run",
        fires: Fires::Decisions(&[("land", "despite-inflight")]),
        // The override IS the measurement for the refusal above: this row exists so `air
        // audit` counts it rather than someone grepping for it.
        removal: Removal::ZeroFirings(
            "goes with `land-in-flight-refusal`: a round with zero overrides retires both",
        ),
    },
    Mechanism {
        id: "claim-retry",
        class: "report",
        what: "`air claim` retries bd once, and only on a timeout; a refusal is bd's answer and \
               is never retried.",
        added: "2026-09-05 (air-gsj)",
        source: "crates/cli/src/cmd/claim.rs, retry_once",
        fires: Fires::Decisions(&[("claim", "timeout-retry")]),
        // The adopter's w1 retried a claim by hand three times on 2026-08-31 and lost the bead to
        // a peer between retries; the message read as a denial. A fresh bd process starts at
        // the ~2 s floor again while any usable timeout is crossed by the same stalls
        // (air-bp0), which is why this is a retry and not a longer wait.
        removal: Removal::ZeroFirings(
            "a round passes with zero `timeout-retry` events, meaning bd no longer times out under load and air-bp0's reduction did the job",
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
    Mechanism {
        id: "land-role-refusal",
        class: "refusal",
        what: "`air land` from any role but the lane and the owner is refused before anything \
               is read.",
        added: "2026-08-22 (air-3pz); the coordinator lost it 2026-09-14 (air-jc2p.2)",
        source: "crates/cli/src/cmd/land.rs, may_land",
        fires: Fires::Decisions(&[("land", "refuse-role")]),
        // Written as `land / refuse` until air-hqj8, so days before 2026-09-25 count it under
        // `land-refusal`. The failures are on may_land: a coordinator's commit on main
        // invalidated four workers' landability at an adopter on 2026-09-06, and landing order
        // was the round's throughput limit while the coordinator landed by hand.
        removal: Removal::Judgement(
            "the lane goes (verify cheap enough that no round has a batch of more than one branch); landing then returns to whoever lands a worker's branch",
        ),
    },
    Mechanism {
        id: "no-precheck",
        class: "refusal",
        what: "Where the repo declares `precheck`, `air batch cut` leaves out a branch with no \
               recorded green precheck at its head; one line per branch left out.",
        added: "2026-09-25 (an adopter's fleet protocol)",
        source: "crates/cli/src/cmd/status.rs batch_ready_rule; crates/cli/src/cmd/batch_cut.rs run",
        // An adopter's lane script parsed a log file for this, 2026-09-05..07: a worker was
        // cut before its check finished, a check still running was relayed as "checked", and
        // a stale green trailer named a head two commits back.
        fires: Fires::Decisions(&[("batch-cut", "no-precheck")]),
        removal: Removal::Judgement(
            "the repo drops the `precheck` key: a round under it with no batch red that a member's precheck would have caught means the precheck only delays the cut",
        ),
    },
    Mechanism {
        id: "batch-cut-refusal",
        class: "refusal",
        what: "`air batch cut` refuses to start: in the main checkout, git without \
               `merge-tree --write-tree`, a dirty lane tree, or the lane's branch conflicting \
               with main.",
        added: "2026-09-25",
        source: "crates/cli/src/cmd/batch_cut.rs, run",
        // The failure is the adopter's hand cut (module header): merges judged by `tail`'s
        // exit status for eleven batches, a skipped dry-merge, three wrong conflict predictions.
        fires: Fires::Decisions(&[("batch-cut", "refuse")]),
        removal: Removal::Judgement(
            "`git merge` itself reports the conflicting pair and the lane's cut needs no set or order Air holds, or the lane goes (verify cheap enough that no batch of more than one branch forms)",
        ),
    },
    Mechanism {
        id: "batch-cut-drop",
        class: "refusal",
        what: "A batch-ready branch that conflicts with main or with an earlier member is left \
               out of the batch, naming the other side and the paths.",
        added: "2026-09-25",
        source: "crates/cli/src/cmd/batch_cut.rs, pre_check and merge",
        // `dropped` until air-hqj8: that word is bookkeeping for `air triage --drop`, so the
        // audit would have counted a drop as nothing.
        fires: Fires::Decisions(&[("batch-cut", "drop")]),
        removal: Removal::Judgement(
            "same as `batch-cut-refusal`: git names the pair itself, or the lane goes",
        ),
    },
    Mechanism {
        id: "main-checkout-session",
        class: "warning",
        what: "`air status` names every launched session whose process runs in the main \
               checkout, where no role works since air-jc2p.1. A warning; nothing is refused.",
        added: "2026-09-25 (air-jc2p.3)",
        source: "crates/cli/src/cmd/readers.rs, main_checkout_sessions",
        // A coordinator's prose commit in the main checkout invalidated four workers'
        // landability at an adopter, and a verify there moved main under a worker's green
        // (both 2026-09-06). One line per full `air status` that printed it.
        fires: Fires::Decisions(&[("status", "main-checkout-session")]),
        removal: Removal::ZeroFirings(
            "a round's status output shows no such line with the launchers as they are; if the line keeps appearing it becomes a launcher refusal instead",
        ),
    },
    Mechanism {
        id: "lease-take-refusal",
        class: "refusal",
        what: "`air lease take` refused: a live holder has the resource.",
        added: "2026-08-21 (owner ruling A; ported from the adopter's lease.sh)",
        source: "crates/cli/src/cmd/lease.rs, take",
        fires: Fires::Decisions(&[("lease.take", "denied")]),
        // None recorded. The deletion of `air lease` was reversed on 2026-08-29 (air-uae)
        // because a zero in a repo that takes no leases says nothing; that is a reason to
        // keep it, not a condition for removing it.
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "release-refusal",
        class: "refusal",
        what: "`air release` of a closed bead is refused: closed is closed, and unfinished work \
               is a new bead.",
        added: "2026-08-21 (plan 0006, owner rulings 2026-08-21)",
        source: "crates/cli/src/cmd/claim.rs, release",
        fires: Fires::Decisions(&[("release", "refuse")]),
        // Written since 2026-08-21 and registered by nobody until `decisions::ALL` made the
        // set enumerable (air-hqj8). No removal condition was recorded with it.
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "bd-budget-timeout",
        class: "budget",
        what: "A `bd` process reached its budget and was killed. Fails closed: the command is \
               refused and names the id count, the budget and AIR_BD_TIMEOUT_MS; status falls \
               back to cached counts; the Stop nudge says nothing.",
        added: "2026-09-06 (air-d75, measured); budgets made generous 2026-09-25 (owner)",
        source: "crates/cli/src/cmd/budgets.rs CATALOGUE; docs/design.md §6.7",
        // bd costs about 2 s per call with stalls to 44 s at an adopter (air-bp0).
        fires: Fires::Budget(&[
            air_ledger::budgets::BD,
            air_ledger::budgets::BD_ACCEPTANCE,
            air_ledger::budgets::BD_NUDGE,
            air_ledger::budgets::BD_PROBE,
            air_ledger::budgets::BD_STATUS,
        ]),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "mcp-tool-budget-kill",
        class: "budget",
        what: "An `air` subprocess behind an MCP tool call reached its budget; its process \
               group is killed and the tool answers with the timeout (`mcp.tool / timeout`).",
        added: "2026-09-25 (air-se4n)",
        source: "crates/cli/src/cmd/mcp.rs, tool_budget and run_self",
        // air-se4n: a flat 20 s killed `air close` through the channel whatever its own bd
        // budget.
        fires: Fires::Budget(&[air_ledger::budgets::MCP_TOOL]),
        removal: Removal::Judgement(
            "the commands behind the tools stop shelling out to bd, or the MCP tools stop running the CLI as a subprocess (mcp.rs, bd_calls)",
        ),
    },
    Mechanism {
        id: "hook-path-budget",
        class: "budget",
        what: "A budget on the hook path was reached: git (1.5 s), the SQLite lock, or the hook \
               itself. Fails OPEN: the hook allows, so a refusal does not happen. A hook killed \
               at its cap writes nothing; `air audit`'s unpaired-hook count is what shows it.",
        added: "2026-09-06 (air-d75)",
        source: "crates/cli/src/cmd/budgets.rs CATALOGUE; docs/design.md §6.7",
        fires: Fires::Budget(&[
            air_ledger::budgets::GIT,
            air_ledger::budgets::SQLITE_LOCK,
            air_ledger::budgets::HOOK,
        ]),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "git-worktree-budget",
        class: "budget",
        what: "A `git` call making or removing a worktree, or merging in `air batch cut`, \
               reached its 120 s budget. Fails closed: the command says so.",
        added: "2026-09-06 (air-d75)",
        source: "crates/cli/src/cmd/worktree.rs GIT_BUDGET",
        fires: Fires::Budget(&[air_ledger::budgets::GIT_WORKTREE]),
        removal: Removal::Unstated,
    },
    Mechanism {
        id: "tree-readers-budget",
        class: "budget",
        what: "The process listing behind `readers:`, the idle exemption and `air land`'s \
               warning reached its budget; the answer is `unknown (why)`, never an empty list.",
        added: "2026-09-25 (owner)",
        source: "crates/cli/src/cmd/readers.rs BUDGET",
        fires: Fires::Budget(&[air_ledger::budgets::TREE_READERS]),
        removal: Removal::Judgement(
            "every process that runs in a fleet tree is one Air recorded, so the ledger alone answers what is running there; or the harness exposes a session's background tasks (readers.rs header)",
        ),
    },
];
