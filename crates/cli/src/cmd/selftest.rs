//! `air selftest`: red/green probes for every check, run against an in-memory ledger and a
//! scratch git repo. A check that matches nothing prints RED (corpus: guards that pass on
//! nothing are the anti-pattern). Exit 1 if any probe fails.

use std::path::Path;
use std::process::Command;

use air_hooks::{GateFacts, handover_verdict};
use air_ledger::Ledger;
use air_ledger::verify::{Kind, VerifyRun, new_id};
use serde::Serialize;
use serde_json::Value;

use crate::cmd::emit;
use crate::cmd::hook::{handover_gate, is_handover_command};

#[derive(Debug, Serialize)]
pub struct Probe {
    pub name: &'static str,
    pub red_fires: bool,
    pub green_passes: bool,
}

impl Probe {
    fn ok(&self) -> bool {
        self.red_fires && self.green_passes
    }
}

pub fn run(json: bool) -> i32 {
    let probes = vec![
        probe_gate_verify(),
        probe_gate_main(),
        probe_handover_matcher(),
        probe_ledger_roundtrip(),
        probe_git_ancestor(),
        probe_gate_claim(),
        probe_claim_cas(),
        probe_attention(),
        probe_channel_dedupe(),
        probe_install_merge(),
        probe_gate_digest(),
        probe_lease_take(),
        probe_launch_no_tty(),
        probe_worker_task_prompt(),
        probe_stop_nudge(),
        probe_nudge_names_only_claimable(),
        probe_standstill(),
        probe_enforced_gate(),
        probe_batch_close(),
        probe_triage_bead_exists(),
        probe_surface_diff(),
        probe_change_only_push(),
        probe_review_fact_survives(),
        probe_bead_attribution_reads_a_trailer(),
        probe_digest_names_its_bead(),
        probe_land_selection_is_never_silent(),
        probe_audit_registry(),
        probe_audit_unregistered_firing(),
        probe_land_refusals(),
        probe_project_is_taken_from_what_it_is_told(),
        probe_project_fence(),
        probe_audit_help_names_only_what_it_prints(),
        probe_landed_but_open(),
        probe_close_with_proof_sequence(),
        probe_verify_in_flight(),
        probe_landing_state(),
    ];
    let all_ok = probes.iter().all(Probe::ok);
    emit(json, &probes, || {
        let mut s = String::new();
        for p in &probes {
            s.push_str(&format!(
                "{} {}: red {} / green {}\n",
                if p.ok() { "PASS" } else { "FAIL" },
                p.name,
                if p.red_fires { "fires" } else { "SILENT" },
                if p.green_passes { "passes" } else { "BLOCKED" },
            ));
        }
        s.push_str(&format!("{} probes", probes.len()));
        s
    });
    if all_ok { 0 } else { 1 }
}

/// air-s7c: `review-waiting` and `owner-decision-waiting` became change-only pushes. Red: the
/// same set evaluated twice pushes once — the repeat is suppressed, and a merely older
/// condition is still a repeat. Green: a real change (a bead joins the waiting set, the owner
/// queue depth moves) pushes again. peer-warning-repeat was deleted for allegedly failing to
/// suppress, so this proves the suppression suppresses.
fn probe_change_only_push() -> Probe {
    use crate::cmd::mcp::{Pushed, select_new};
    use crate::cmd::status::Attention;

    let review = |bead: &str, mins: i64| Attention {
        worker: bead.to_string(),
        kind: "review-waiting",
        detail: format!("{bead} handed over {mins} min ago"),
        for_minutes: mins,
        fingerprint: format!("{bead}/alpha"),
    };
    let queue = |depth: usize, mins: i64| Attention {
        worker: "owner".to_string(),
        kind: "owner-decision-waiting",
        detail: format!("{depth} waiting, oldest {mins} min"),
        for_minutes: mins,
        fingerprint: format!("depth:{depth}"),
    };

    let mut pushed = Pushed::new();
    // First evaluation: both are new, both push.
    let first = select_new(&mut pushed, &[review("air-1", 5), queue(2, 5)]);
    // Same facts, much later: age is not a change, so nothing is pushed. Under the old
    // doubling rule 5 -> 40 min would have re-pushed both.
    let same_again = select_new(&mut pushed, &[review("air-1", 40), queue(2, 40)]);
    let red = first.len() == 2 && same_again.is_empty();

    // A bead joins the set, and the queue depth moves: both are real changes.
    let changed = select_new(
        &mut pushed,
        &[review("air-1", 45), review("air-2", 1), queue(3, 45)],
    );
    let green = changed.len() == 2
        && changed.iter().any(|a| a.worker == "air-2")
        && changed.iter().any(|a| a.worker == "owner")
        // ...and the unchanged bead did NOT ride along with them.
        && !changed.iter().any(|a| a.worker == "air-1");
    Probe {
        name: "channel: an unchanged set pushes once however old it gets; a changed set pushes again",
        red_fires: red,
        green_passes: green,
    }
}

/// air-s7c: deleting the review-waiting PUSH must not delete the FACT. `air status` renders
/// review waits and the owner queue on demand, which is a pull and costs nobody a
/// notification. Red: a snapshot with waits and a queue says so. Green: an empty one says
/// zero rather than going silent, so "no waits" and "not reported" stay distinguishable.
fn probe_review_fact_survives() -> Probe {
    use crate::cmd::status::{Snapshot, render_for_probe};

    let mut s = Snapshot {
        at: "2026-08-22T10:00:00Z".to_string(),
        ..Default::default()
    };
    s.review_waits = vec![("air-1".to_string(), "alpha".to_string(), 40)];
    s.owner_queue_depth = 3;
    let with = render_for_probe(&s);
    let red = with.contains("review: 1 waiting")
        && with.contains("air-1")
        && with.contains("owner queue: 3");

    let empty = render_for_probe(&Snapshot {
        at: "2026-08-22T10:00:00Z".to_string(),
        ..Default::default()
    });
    let green = empty.contains("review: 0 waiting") && empty.contains("owner queue: 0");
    Probe {
        name: "status: review waits and the owner queue are still named on demand (push deleted, fact kept)",
        red_fires: red,
        green_passes: green,
    }
}

/// air-6u5: selection never answers "nothing" when it means "something broke", and a bead
/// stays attributed after the branch merges `main`.
///
/// Red, the bug that made `air land` unusable in adopter: the old narrowing required a claim
/// `claimed_at >= branch_point`, and merging `main` moves the branch point FORWARD past the
/// claim that started the work — while landing requires merging main. So preparing to land
/// destroyed the attribution. Here the claim is older than the branch point, as it is for
/// every real branch that has merged main, and it must still be attributed.
///
/// Green: a ledger error is an error, not an empty queue. `{"landed": [], "ok": true}` was the
/// worst answer available because there was nothing to disbelieve.
fn probe_land_selection_is_never_silent() -> Probe {
    use crate::cmd::status::Skipped;

    // The narrowing that broke it is gone: what remains is claimed-by-this-worker and
    // not-already-landed, neither of which moves when git does.
    let red = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        // Claimed BEFORE the branch point, which is what merging main produces.
        l.record_claim("fd-1", "alpha", &[], "2026-08-22T10:00:00Z")
            .map_err(|e| e.to_string())?;
        let kept = crate::cmd::status::attributable_for_test(&l, &["fd-1".to_string()], "alpha")?;
        // ...and a bead already landed is dropped, which is the bound that replaced the time.
        let dropped =
            crate::cmd::status::attributable_for_test(&l, &["ad-other".to_string()], "alpha")?;
        Ok(kept == ["fd-1"] && dropped.is_empty())
    })()
    .unwrap_or(false);

    // A Skipped row carries the check name and the fixing command, so nothing is ever a bare
    // absence.
    let sk = Skipped {
        worker: "alpha".to_string(),
        check: "green-at-head",
        detail: "alpha has no recorded green at its head abc12345".to_string(),
        fix: "in that worktree: air record verify -- <the repo's verify>".to_string(),
    };
    let green = !sk.fix.is_empty() && !sk.detail.is_empty() && sk.check == "green-at-head";
    Probe {
        name: "land: a claim older than the branch point still attributes; every skip names its check and fix",
        red_fires: red,
        green_passes: green,
    }
}

/// air-agq: the digest gate reads a declared `bead:` field instead of guessing from a
/// filename and an mtime.
///
/// It GUARDS, so every way it used to be wrong failed toward permitting, and a missing refusal
/// looks exactly like a satisfied one. Red covers the three ways it passed when it should not:
/// a digest for a different bead, a digest touched rather than written, and a file that merely
/// has the worker's name in it. Green: the digest that declares this bead is accepted, and a
/// pre-cutoff digest with no front matter still passes so today's work is not invalidated.
fn probe_digest_names_its_bead() -> Probe {
    use crate::cmd::handover::{declared_bead, digest_for_bead};

    let res = (|| -> Option<(bool, bool)> {
        let root = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&root).ok()?;
        let dir = root.as_path();
        // Strict: every file here counts as written after the cutoff, so only a declaration
        // is accepted. The lenient cutoff below is the history case.
        let cut: jiff::Timestamp = "2000-01-01T00:00:00Z".parse().ok()?;
        let write = |name: &str, body: &str| std::fs::write(dir.join(name), body).ok();
        // A digest for ANOTHER bead, by this worker, written now.
        write(
            "2026-08-23-beta-air-other.md",
            "---\nbead: air-other\n---\n# other\n",
        )?;
        let ours = vec!["air-agq".to_string()];

        // Red: it declares a different bead, so it is not this bead's digest, whatever its
        // name or mtime say.
        let wrong_bead = !digest_for_bead(dir, "beta", &ours, None, cut);
        // Red: a file carrying the worker's name and no declaration, written after the
        // cutoff, is not a substitute — this is the `touch` case and the substring case.
        write("2026-08-23-beta-notes.md", "# just some notes\n")?;
        let undeclared_after_cutoff = !digest_for_bead(dir, "beta", &ours, None, cut);
        let red =
            wrong_bead && undeclared_after_cutoff && declared_bead("# no front matter").is_none();

        // Green: the digest that declares this bead is accepted.
        write(
            "2026-08-23-beta-air-agq.md",
            "---\nbead: air-agq\n---\n# ours\n",
        )?;
        let declared_ok = digest_for_bead(dir, "beta", &ours, None, cut);

        // Green: history still passes. A digest written before the cutoff with no front
        // matter is matched the old way, so the change does not invalidate what exists.
        let old = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&old).ok()?;
        std::fs::write(old.join("2026-08-22-beta-air-old.md"), "# old\n").ok()?;
        let far_future: jiff::Timestamp = "2999-01-01T00:00:00Z".parse().ok()?;
        let fallback_ok = digest_for_bead(&old, "beta", &ours, None, far_future);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&old);
        Some((red, declared_ok && fallback_ok))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: a digest counts when it declares its bead; a different bead, a touch, or a name match do not",
        red_fires: red,
        green_passes: green,
    }
}

/// air-4re: which beads a branch carries is written by a machine and read by a machine.
///
/// Red: a commit whose body MENTIONS three beads and carries one `Bead:` trailer is attributed
/// to one — the case the prose scan cannot get right, because a mention and an attribution
/// have identical grammar. air-7kp needed an authorship filter and a branch-point bound on top
/// of the scan and still went 8 → 6 → 3 against one real branch.
///
/// Green: the dated fallback still reads history (a commit before the cutoff with no trailer),
/// and a commit after it with no trailer is attributed to nothing rather than guessed at.
fn probe_bead_attribution_reads_a_trailer() -> Probe {
    use crate::cmd::attribution::{Commit, cutoff, ids_of, prose_ids, trailer_ids};

    let mentions = "fix(land): select from the merge range\n\nBuilds on air-3pz, measured in \
                    air-869, supersedes air-7kp.\n\nBead: air-4re\n";
    let red = trailer_ids(mentions) == ["air-4re"]
        && ids_of(
            &[Commit {
                committed: "2026-08-22T10:00:00Z".to_string(),
                message: mentions.to_string(),
            }],
            prose_ids,
            cutoff(),
        ) == ["air-4re"]
        // ...and the scan on its own would indeed have taken all four.
        && prose_ids(mentions).len() >= 4;

    let old = Commit {
        committed: "2020-01-01T00:00:00Z".to_string(),
        message: "fix: the work (air-old)\n".to_string(),
    };
    let new = Commit {
        committed: "2999-01-01T00:00:00Z".to_string(),
        message: "fix: the work (air-new)\n".to_string(),
    };
    let green = ids_of(&[old], prose_ids, cutoff()) == ["air-old"]
        && ids_of(&[new], prose_ids, cutoff()).is_empty();
    Probe {
        name: "land: a `Bead:` trailer attributes the commit; a mention does not; the prose fallback is dated",
        red_fires: red,
        green_passes: green,
    }
}

/// air-zyo: the registry's job is that a mechanism nobody wrote a removal condition for is
/// visible. Red: an entry with nothing recorded is reported as a defect. Green: an entry
/// with a condition is not, and its counter reads back.
fn probe_audit_registry() -> Probe {
    use crate::cmd::audit::gather_from;

    let events = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["review-waiting:air-1"]},"decision":"attention"}"#,
        "\n",
    );
    let a = gather_from(
        &[("2026-08-22".to_string(), events.to_string())],
        "2026-08-22",
    );
    // Red: nothing is recorded for `stuck`, so it is a defect and says so. (This probe
    // pointed at `review-waiting` until air-s7c gave that one a condition, at which point it
    // went silent and said so, which is the probe doing its job.)
    let red = a.rows.iter().any(|r| r.id == "stuck" && r.defect.is_some());
    // Green: a mechanism that does carry one is not a defect, and the counter works.
    let green = a
        .rows
        .iter()
        .any(|r| r.id == "idle-without-claim" && r.defect.is_none())
        && a.rows
            .iter()
            .any(|r| r.id == "review-waiting" && r.fires == 1 && r.last_fired.is_some());
    Probe {
        name: "audit: a mechanism with no recorded removal condition is a defect; one with a condition counts",
        red_fires: red,
        green_passes: green,
    }
}

/// air-2zq: close-with-proof, end to end. air-i59 made the gate blocking on evidence measured
/// against the hand-over flow; air-7o3 replaced hand-over with closing, and the question raised
/// was whether the gate now bills a fresh verify per bead and whether an agent can stall.
///
/// Red: once HEAD moves, the next close IS refused until a verify is recorded there — the gate
/// still bites, which is the half worth keeping. Green: claim, close, take the next bead, close
/// again on an UNCHANGED HEAD, all on one verify run. So the second close is free and the
/// sequence in CLAUDE.md's work flow does not stall.
fn probe_close_with_proof_sequence() -> Probe {
    use crate::cmd::hook::handover_gate;
    use air_hooks::HookOutcome;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let head = g(&["rev-parse", "HEAD"])?;

        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let green_at = |sha: &str| -> Result<(), String> {
            l.record_verify(&VerifyRun {
                id: new_id(),
                worker: "probe".into(),
                sha: sha.to_string(),
                kind: Kind::Verify,
                exit_code: 0,
                trigger: "selftest".into(),
                failing_step: None,
                started_at: "t".into(),
                finished_at: "t".into(),
                log_path: None,
                command: None,
                duration_ms: None,
                output_bytes: None,
                dirty: false,
            })
            .map_err(|e| e.to_string())
        };
        let passes = |bead: &str| -> bool {
            matches!(
                handover_gate(&l, "probe", &dir, &format!("bd close {bead}"), true)
                    .map(|d| d.outcome),
                Ok(HookOutcome::Allow { .. })
            )
        };

        // Bead one: claim, work already committed, verify recorded, close.
        l.record_claim("fd-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        green_at(&head)?;
        let first = passes("fd-1");

        // Bead two, finished without moving HEAD (a docs bead already satisfied, a no-op fix).
        // ONE verify run exists in total, and this close must still pass: a green at a commit
        // that has not moved is still a green.
        l.record_claim("fd-2", "probe", &[], "t1")
            .map_err(|e| e.to_string())?;
        let second_free = passes("fd-2");
        let runs: i64 = l
            .conn()
            .query_row("SELECT count(*) FROM verify_runs", [], |r| r.get(0))
            .unwrap_or(-1);

        // Bead three, with a commit: HEAD moved, so the gate demands a verify there.
        g(&["commit", "-q", "--allow-empty", "-m", "b"])?;
        l.record_claim("fd-3", "probe", &[], "t2")
            .map_err(|e| e.to_string())?;
        let refused_after_commit = !passes("fd-3");
        // ...and recording one at the new HEAD clears it. No stall.
        let moved = g(&["rev-parse", "HEAD"])?;
        green_at(&moved)?;
        let cleared = passes("fd-3");

        Ok((
            refused_after_commit,
            first && second_free && runs == 1 && cleared,
        ))
    })()
    .unwrap_or((false, false));
    Probe {
        name: "gate: two closes on one unchanged HEAD cost one verify; a commit demands a new one and clears",
        red_fires: res.0,
        green_passes: res.1,
    }
}

/// air-ha8: `air audit --help` advertised "how often with nothing following" for a round after
/// the owner cut that metric — a derived statement reading as an observed one, in the help of
/// the command built to surface exactly that. The check is the containment: every field the
/// help names in backticks must appear in what the command prints.
///
/// Red: a help text that names one more field than the command prints is caught. Green: the
/// real help text passes.
fn probe_audit_help_names_only_what_it_prints() -> Probe {
    use crate::cmd::audit::{gather_from, render};
    use clap::CommandFactory;

    // Backticked names are the contract: prose around them is free, the names are checked.
    fn named(help: &str) -> Vec<String> {
        help.split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    }
    let help = crate::Cli::command()
        .find_subcommand("audit")
        .and_then(|c| c.get_long_about().or_else(|| c.get_about()).cloned())
        .map(|s| s.to_string())
        .unwrap_or_default();
    // One registered mechanism firing, so a row with a removal condition renders in full.
    let events = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"main","command":"status.attention","inputs":{"conditions":["review-waiting:air-1"]},"decision":"attention"}"#,
        "\n",
    );
    let printed = render(&gather_from(
        &[("2026-08-22".to_string(), events.to_string())],
        "2026-08-22",
    ));
    let all_printed = |h: &str| {
        let names = named(h);
        !names.is_empty() && names.iter().all(|n| printed.contains(n.as_str()))
    };
    Probe {
        name: "audit: every field the help names in backticks is one the command prints",
        red_fires: !all_printed(&format!("{help} and `how often with nothing following`")),
        green_passes: all_printed(&help),
    }
}

/// air-ayp: `air land` closes nothing (the worker closes its own bead with proof), so what a
/// landing carries past its print is the one signal meaning a wrong close: a clause the merge
/// CONTRADICTS. As a `landings` row, never as a bd status.
///
/// Red: a bead naming a file the merge did not touch is refuted, not discharged, the ledger
/// reports it with the clause, and the report SURVIVES the claim being claimed and released —
/// which is what air-dlw fixed, because close-with-proof reconciles the claim away at once and
/// a report keyed on it could never fire. Green: a bead whose every clause is discharged says
/// so; one Air merely cannot read is neither refuted nor discharged and is not reported; and a
/// later landing that stops refuting the bead clears it.
///
/// The no-blocking half is the second assertion: the whole representation is a ledger row, and
/// the bead's bd status is untouched, so a dependent is exactly as blocked as it was before
/// the merge. bd's blocking predicate never consults the workflow class, which is why parking
/// it in a done-class status would have blocked dependents indefinitely.
fn probe_landed_but_open() -> Probe {
    use crate::cmd::acceptance::{Evidence, judge_clauses};
    use air_ledger::landings::{Landing, OpenBead};

    let changed = vec!["docs/rules/roles.md".to_string()];
    let ev = Evidence {
        green_at_landed: true,
        changed: &changed,
    };
    // A clause the merge CONTRADICTS: the bead names a file it did not touch.
    let refutable = judge_clauses(
        "fd-2",
        vec!["docs/rules/writing.md names the rule.".into()],
        &ev,
    );
    // A clause Air simply cannot read. Not a defect, and not the wrong-close signal.
    let unreadable = judge_clauses(
        "fd-3",
        vec!["The owner rules on the counter-argument.".into()],
        &ev,
    );
    let discharged = judge_clauses(
        "fd-1",
        vec![
            "Verify recorded green at HEAD.".into(),
            "docs/rules/roles.md names the rule.".into(),
        ],
        &ev,
    );

    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&Landing {
            id: new_id(),
            worker: "alpha".into(),
            sha: "aaa".into(),
            tip_sha: Some("bbb".into()),
            result: "landed-open".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 1,
            beads: vec!["fd-1".into(), "fd-2".into()],
            open_beads: vec![OpenBead {
                bead: "fd-2".into(),
                why: refutable.why_open(),
                refuted: true,
            }],
            merge_commit: Some("ccc".into()),
            pid: None,
            started_at: "t0".into(),
            finished_at: "t1".into(),
        })
        .map_err(|e| e.to_string())?;
        let open = l.landed_open().map_err(|e| e.to_string())?;
        // Reported, with the clause, and the closable bead is NOT in the held-open set.
        let reported = open.len() == 1
            && open
                .first()
                .is_some_and(|o| o.bead == "fd-2" && o.why.contains("docs/rules/writing.md"));
        // air-dlw: the claim's lifetime must NOT decide this. Under close-with-proof the
        // worker closes at once and the reconcile releases the claim on the next tick, so a
        // report keyed on the claim could never fire. Claim it, release it as the reconcile
        // does, and the report has to survive both.
        l.record_claim("fd-2", "alpha", &[], "t2")
            .map_err(|e| e.to_string())?;
        l.release_claims_on(&["fd-2".to_string()], "closed", "t3")
            .map_err(|e| e.to_string())?;
        let survives_the_claim = l.landed_open().map_err(|e| e.to_string())?.len() == 1;

        // It clears when a LATER landing of the same bead stops refuting it.
        l.record_landing(&Landing {
            id: new_id(),
            worker: "alpha".into(),
            sha: "ddd".into(),
            tip_sha: Some("ccc".into()),
            result: "landed".into(),
            failing_step: None,
            verify_run_id: None,
            attempt_no: 2,
            beads: vec!["fd-2".into()],
            open_beads: vec![OpenBead {
                bead: "fd-2".into(),
                why: "a clause Air cannot read".into(),
                refuted: false,
            }],
            merge_commit: Some("eee".into()),
            pid: None,
            started_at: "t4".into(),
            finished_at: "t5".into(),
        })
        .map_err(|e| e.to_string())?;
        let cleared = l.landed_open().map_err(|e| e.to_string())?.is_empty();
        Ok((reported && survives_the_claim, cleared))
    })()
    .unwrap_or((false, false));
    let (reported, cleared) = res;

    Probe {
        name: "land: a contradicted clause is reported from the landing and survives the claim being reconciled away; one Air cannot read is not",
        red_fires: refutable.refuted() && !refutable.all_discharged() && reported,
        green_passes: discharged.all_discharged()
            && !unreadable.refuted()
            && !unreadable.all_discharged()
            && cleared,
    }
}

/// air-0y9: a mechanism that fires with no registry row must be reported, not omitted. A
/// registry that silently drops one reads as complete when it is not. Red: an unclaimed
/// command/decision pair is a defect. Green: the same pair, once a row claims it, is counted
/// as that mechanism instead.
fn probe_audit_unregistered_firing() -> Probe {
    use crate::cmd::audit::{gather_from, registered_traces};

    let unclaimed = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"beta","command":"hook.Whatever","decision":"throttled"}"#,
        "\n",
    );
    let red = {
        let a = gather_from(
            &[("2026-08-22".to_string(), unclaimed.to_string())],
            "2026-08-22",
        );
        // Named, with its count, rather than dropped for being an unfamiliar decision word.
        a.unregistered == vec![("hook.Whatever / throttled".to_string(), 1)]
            && crate::cmd::audit::render(&a).contains("defect:")
    };
    // Green: a pair the registry does claim is attributed to its mechanism and is not a
    // defect. `claim / refuse` is the row air-0y9 added.
    let claimed = concat!(
        r#"{"at":"2026-08-22T01:00:00Z","worker":"beta","command":"claim","decision":"refuse"}"#,
        "\n",
    );
    let green = {
        let a = gather_from(
            &[("2026-08-22".to_string(), claimed.to_string())],
            "2026-08-22",
        );
        registered_traces().contains("claim / refuse")
            && a.unregistered.is_empty()
            && a.rows
                .iter()
                .any(|r| r.id == "claim-refusal" && r.fires == 1)
    };
    Probe {
        name: "audit: a firing with no registry row is a defect; a claimed pair counts as its mechanism",
        red_fires: red,
        green_passes: green,
    }
}

/// air-6g1: a repo installed before a surface change is told about it, and one already
/// current is told nothing. The diff is keyed to recorded ids, not a version string, so it
/// cannot silently report nothing because a number was not bumped.
fn probe_surface_diff() -> Probe {
    use crate::cmd::install::{SURFACE, surface_diff};

    // Red: a repo that knows about nothing sees every change, `air land` among them.
    let stale = surface_diff(&[]);
    let red = !stale.is_empty()
        && stale.iter().any(|c| c.id == "land")
        && stale.iter().any(|c| c.silent_break);
    // Green: a repo recorded at the current surface sees nothing.
    let current: Vec<String> = SURFACE.iter().map(|c| c.id.to_string()).collect();
    let green = surface_diff(&current).is_empty();
    Probe {
        name: "install: an older recorded surface diffs (names `air land`); the current one is empty",
        red_fires: red,
        green_passes: green,
    }
}

/// air-76z: a capture must not point at a bead bd does not have. bd omits an unknown id from
/// `bd show` and still exits 0, so the check is the comparison, not the exit code.
fn probe_triage_bead_exists() -> Probe {
    use crate::cmd::capture::missing_ids;

    let want: Vec<String> = ["fd-1", "zz-nope", "fd-2"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let issue = |id: &str| air_bd::Issue {
        id: id.to_string(),
        ..Default::default()
    };
    let red = missing_ids(&want, &[issue("fd-1"), issue("fd-2")]) == ["zz-nope"];
    let green = missing_ids(&want, &[issue("fd-1"), issue("zz-nope"), issue("fd-2")]).is_empty();
    Probe {
        name: "triage: a bead bd did not return is named; a full answer passes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-869: `air close` is the coordinator's, and it issues ONE bd process however many
/// beads it is given. Red: a worker is refused. Green: the coordinator's ten ids build a
/// single `bd close` argv and release ten claims in one transaction.
fn probe_batch_close() -> Probe {
    use crate::cmd::close::may_close;

    let red = may_close("beta").is_err();
    let green = (|| -> Result<bool, String> {
        let ids: Vec<String> = (1..=10).map(|i| format!("fd-{i}")).collect();
        let argv = air_bd::close_argv(&ids, "landed", "main");
        let one_process = argv.first().map(String::as_str) == Some("close")
            && ids.iter().all(|i| argv.contains(i));
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        for id in &ids {
            l.record_claim(id, "beta", &[], "t0")
                .map_err(|e| e.to_string())?;
        }
        let released = l
            .release_claims_on(&ids, "landed", "t1")
            .map_err(|e| e.to_string())?;
        Ok(may_close("main").is_ok() && one_process && released.len() == ids.len())
    })()
    .unwrap_or(false);
    Probe {
        name: "close: a worker is refused; ten coordinator closes are one bd argv, one transaction",
        red_fires: red,
        green_passes: green,
    }
}

/// air-3pz: `air land` refuses a worker, a dirty main, a branch that has not merged main, and
/// a recorded green that is not at the branch head; a clean green hand-over passes. Pure over
/// the facts, so the whole refusal set fires without a repo.
fn probe_land_refusals() -> Probe {
    use crate::cmd::land::{Facts, check, may_land};

    let none: Vec<String> = vec![];
    fn ok(dirty: &[String]) -> Facts<'_> {
        Facts {
            worker: "alpha",
            on_main: true,
            main_checkout: true,
            dirty,
            branch_exists: true,
            already_in_main: false,
            contains_main: true,
            branch_head: "abcdef99",
            green_at: Some("abcdef99"),
        }
    }
    let refusals = [
        Facts {
            on_main: false,
            ..ok(&none)
        },
        Facts {
            main_checkout: false,
            ..ok(&none)
        },
        Facts {
            branch_exists: false,
            ..ok(&none)
        },
        Facts {
            contains_main: false,
            ..ok(&none)
        },
        Facts {
            green_at: Some("00000000"),
            ..ok(&none)
        },
        Facts {
            green_at: None,
            ..ok(&none)
        },
    ];
    let dirty = vec!["src/a.rs".to_string()];
    // Every refusal fires, and every one names a command to run.
    let red = may_land("alpha").is_err()
        && check(&ok(&dirty)).is_err()
        && refusals.iter().all(|f| {
            check(f)
                .err()
                .is_some_and(|m| m.contains('`') && m.starts_with("refused: "))
        });
    let green = may_land("main").is_ok()
        && check(&ok(&none)) == Ok(true)
        && check(&Facts {
            already_in_main: true,
            ..ok(&none)
        }) == Ok(false);
    Probe {
        name: "land: worker, dirty main, stale branch and a green off the head are all refused with a fix; a clean green passes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-0lk, corrected by air-3oq: a session may ACT only on its own project, and may TALK to
/// any of them. With `AIR_PROJECT=air`, a PreToolUse call for `tmux kill-session -t fd-worker1`
/// denies with a refusal that names the fence, the project and what is still allowed; the same
/// call for `air-alpha` passes. `SendMessage` to another project's coordinator passes, which is
/// the half air-0lk got wrong: denying it broke the cross-project channel silently, in the
/// verification path.
/// air-7ah: which project a session belongs to must come from what it is told, not from what
/// happens to be in the ambient environment. `project_for` read `AIR_PROJECT` directly, so the
/// hook test asserting a scratch repo's prefix quietly got the ambient value of whatever
/// session ran it — green everywhere except inside a launched session, which is the only place
/// `air land` runs. Landing was blocked for every branch and the test was hiding it.
///
/// Red: with a project supplied, the supplied value wins over the checkout's beads prefix —
/// the case that was never exercised. Green: with none supplied, the prefix is used, and a
/// blank is not a value. Neither arm reads the environment, so this cannot regress the way it
/// did.
fn probe_project_is_taken_from_what_it_is_told() -> Probe {
    use crate::cmd::hook::project_from;

    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(dir.join(".beads")).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(".beads/config.yaml"), "issue-prefix: \"zz\"\n")
            .map_err(|e| e.to_string())?;
        let red = project_from(Some("air"), &dir) == "air";
        let green = project_from(None, &dir) == "zz"
            && project_from(Some("   "), &dir) == "zz"
            && project_from(Some(" air "), &dir) == "air";
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red, green))
    })()
    .unwrap_or((false, false));
    Probe {
        name: "project: the session's project comes from what it is told, not from ambient AIR_PROJECT",
        red_fires: res.0,
        green_passes: res.1,
    }
}

fn probe_project_fence() -> Probe {
    use crate::cmd::hook::project_fence;
    use air_hooks::{HookInput, HookOutcome};

    let call = |raw: String| -> Option<HookOutcome> {
        let input = HookInput::parse(&raw).ok()?;
        project_fence(&input, "air").map(|d| d.outcome)
    };
    let bash = |cmd: &str| {
        call(serde_json::json!({"tool_name": "Bash", "tool_input": {"command": cmd}}).to_string())
    };
    let send = |to: &str| {
        call(serde_json::json!({"tool_name": "SendMessage", "tool_input": {"to": to}}).to_string())
    };
    let denied = |o: Option<HookOutcome>, needle: &str| {
        matches!(o, Some(HookOutcome::Block { reason })
            if reason.contains(needle) && reason.contains("air-0lk"))
    };
    Probe {
        name: "project: acting on another project's tmux session is denied and the refusal is readable; messaging any project is allowed",
        red_fires: denied(bash("tmux kill-session -t fd-worker1"), "fd-worker1")
            // The refusal has to be interpretable, or a fenced peer reads it as "not you"
            // and stops trying (air-3oq).
            && denied(bash("tmux kill-session -t fd-worker1"), "MESSAGING")
            && denied(bash("tmux send-keys -t fd-w1:0.1 hi"), "air-3oq"),
        // Messaging ANY project is allowed, including one this ledger has never seen: the
        // fence is about acting, not talking, and denying it broke the channel silently.
        green_passes: send("adopter-51").is_none()
            && send("alpha-6d").is_none()
            && bash("tmux kill-session -t air-alpha").is_none()
            && bash("tmux ls").is_none(),
    }
}

/// Check 5 (ruling D): digest configured but absent → missing `digest-present`; not
/// configured → not applicable.
fn probe_gate_digest() -> Probe {
    let mut red = base_facts();
    red.digest_present = Some(false);
    red.digest_dir = Some("docs/log.d".into());
    let mut green = base_facts();
    green.digest_present = None;
    Probe {
        name: "gate: digest required when the repo configures digest_dir",
        red_fires: handover_verdict(&red)
            .missing
            .iter()
            .any(|m| m.check == "digest-present"),
        green_passes: handover_verdict(&green).pass,
    }
}

/// air-tdc: `air worker --task` from a socket stdin (the coordinator's Bash tool) must not
/// exec `claude --tmux` (tcgetattr fails there). Red: the socket case is routed away from
/// exec. Green: a detached tmux session is actually created (pure check only when tmux is
/// absent; the probe name says so).
fn probe_launch_no_tty() -> Probe {
    use crate::cmd::launch::{Launch, launch_mode, tmux_detached_argv};
    let red =
        launch_mode(false, true) == Launch::Detached && launch_mode(true, true) == Launch::Exec;
    if Command::new("tmux").arg("-V").output().is_err() {
        return Probe {
            name: "launch: socket stdin never execs claude --tmux (tmux absent: pure check only)",
            red_fires: red,
            green_passes: launch_mode(false, false) == Launch::Exec,
        };
    }
    let socket = format!("air-selftest-{}", std::process::id());
    let name = "air-selftest";
    let argv = tmux_detached_argv(
        name,
        Path::new("/"),
        Some(&socket),
        "sh",
        &["-c".to_string(), "sleep 30".to_string()],
    );
    let started = Command::new("tmux")
        .args(&argv)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let exists = started
        && Command::new("tmux")
            .args(["-L", &socket, "has-session", "-t", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    let _ = Command::new("tmux")
        .args(["-L", &socket, "kill-server"])
        .output();
    Probe {
        name: "launch: socket stdin starts a detached tmux session instead of exec",
        red_fires: red,
        green_passes: exists,
    }
}

/// Leases: a healthy holder denies a second taker; a dead holder is broken and taken.
fn probe_lease_take() -> Probe {
    use air_ledger::leases::{Holder, Lease, Take};
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let a = Holder {
            worker: "a",
            session_id: None,
            pid: Some(1),
            pid_started: None,
        };
        let b = Holder {
            worker: "b",
            session_id: None,
            pid: Some(2),
            pid_started: None,
        };
        let healthy = |_: &Lease| None;
        l.lease_take("runtime", &a, "api", "t0", healthy)
            .map_err(|e| e.to_string())?;
        let denied = matches!(
            l.lease_take("runtime", &b, "sim", "t1", healthy)
                .map_err(|e| e.to_string())?,
            Take::Held(_)
        );
        let dead = |_: &Lease| Some("dead".to_string());
        let taken = matches!(
            l.lease_take("runtime", &b, "sim", "t2", dead)
                .map_err(|e| e.to_string())?,
            Take::TakenAfter(_)
        );
        Ok((denied, taken))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "lease: healthy holder denies; dead holder is broken and taken",
        red_fires: red,
        green_passes: green,
    }
}

/// air-i59: with `AIR_ENFORCE=1` the PreToolUse gate denies `bd update x -s awaiting_review`
/// when no green is recorded at HEAD, and the reason names the fixing command; once a green
/// verify run is recorded at HEAD (main merged) the same command is allowed.
fn probe_enforced_gate() -> Probe {
    use air_hooks::HookOutcome;
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let head = g(&["rev-parse", "HEAD"])?;
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("fd-1", "probe", &[], "t0")
            .map_err(|e| e.to_string())?;
        let cmd = "bd update fd-1 -s awaiting_review";
        let red = handover_gate(&l, "probe", &dir, cmd, true)?;
        let red_fires = matches!(&red.outcome, HookOutcome::Block { reason }
            if reason.contains("air record verify -- make verify"));
        l.record_verify(&VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: head,
            kind: Kind::Verify,
            exit_code: 0,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "t1".into(),
            finished_at: "t1".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
        })
        .map_err(|e| e.to_string())?;
        let green = handover_gate(&l, "probe", &dir, cmd, true)?;
        let green_passes = matches!(green.outcome, HookOutcome::Allow { context: None });
        let _ = std::fs::remove_dir_all(&dir);
        Ok((red_fires, green_passes))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "gate: AIR_ENFORCE=1 denies bd update -s awaiting_review without green at HEAD (names the fix); allows with green",
        red_fires: red,
        green_passes: green,
    }
}

/// Check 4: a hand-over names a bead the worker does not hold → missing `claim`.
fn probe_gate_claim() -> Probe {
    let mut red = base_facts();
    red.bead = Some("fd-1".into());
    red.bead_claimed_by_worker = false;
    let mut green = base_facts();
    green.bead = Some("fd-1".into());
    green.bead_claimed_by_worker = true;
    Probe {
        name: "gate: claim required for the named bead",
        red_fires: handover_verdict(&red)
            .missing
            .iter()
            .any(|m| m.check == "claim"),
        green_passes: handover_verdict(&green).pass,
    }
}

/// The ledger half of `air claim`: a second worker finds the open claim; the same worker
/// re-claiming after release gets a fresh row.
fn probe_claim_cas() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_claim("fd-1", "w1", &[], "t0")
            .map_err(|e| e.to_string())?;
        let held_by_other = l
            .open_claim("fd-1")
            .map_err(|e| e.to_string())?
            .is_some_and(|c| c.worker != "w2");
        l.release_claim("fd-1", "w1", "abandoned", "t1")
            .map_err(|e| e.to_string())?;
        let free = l.open_claim("fd-1").map_err(|e| e.to_string())?.is_none();
        Ok((held_by_other, free))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "claim: ledger sees another worker's open claim; release frees it",
        red_fires: red,
        green_passes: green,
    }
}

/// Attention conditions fire on a stale stuck session and stay quiet on a fresh one.
fn probe_attention() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    let mk = |changed: &str| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            session: Some(Session {
                session_id: "s".into(),
                state: "stuck".into(),
                detail: None,
                changed_at: changed.into(),
                pid: None,
                pid_alive: None,
                project: String::new(),
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let red = attention(&mk("2026-08-20T11:00:00Z"), now, Thresholds::default());
    let green = attention(&mk("2026-08-20T11:59:00Z"), now, Thresholds::default());
    Probe {
        name: "attention: stale stuck session fires; fresh one is quiet",
        red_fires: red.iter().any(|a| a.kind == "stuck"),
        green_passes: green.is_empty(),
    }
}

/// air-e7q, the standstill: a green hand-over waiting on review, an idle worker with no
/// claim and beads ready. Red: both conditions fire on those facts (the old `attention`
/// was silent on them). Green: the same fleet with the review landed, the worker fresh, and
/// nothing ready is quiet.
fn probe_standstill() -> Probe {
    use crate::cmd::status::{Session, Snapshot, Thresholds, WorkerView, attention};
    let mk = |changed: &str, waits: Vec<(String, String, i64)>, ready: usize| Snapshot {
        workers: vec![WorkerView {
            worker: "w".into(),
            role: "worker".into(),
            head: Some("abc".into()),
            green_at_head: Some(true),
            session: Some(Session {
                session_id: "s".into(),
                state: "idle".into(),
                detail: None,
                changed_at: changed.into(),
                pid: None,
                pid_alive: None,
                project: String::new(),
            }),
            ..Default::default()
        }],
        review_waits: waits,
        ready_depth: Some(ready),
        ..Default::default()
    };
    let now = "2026-08-20T12:00:00Z";
    let red = attention(
        &mk(
            "2026-08-20T11:40:00Z",
            vec![("fd-1".into(), "w".into(), 20)],
            5,
        ),
        now,
        Thresholds::default(),
    );
    let green = attention(
        &mk("2026-08-20T11:59:00Z", vec![], 0),
        now,
        Thresholds::default(),
    );
    Probe {
        name: "attention: review-waiting and idle-without-claim fire; landed and fresh is quiet",
        red_fires: red
            .iter()
            .any(|a| a.kind == "review-waiting" && a.worker == "fd-1")
            && red.iter().any(|a| a.kind == "idle-without-claim"),
        green_passes: green.is_empty(),
    }
}

/// The channel pushes a new condition once and not again until it escalates.
fn probe_channel_dedupe() -> Probe {
    use crate::cmd::mcp::{Pushed, select_new};
    use crate::cmd::status::Attention;
    let a = |m: i64| Attention {
        worker: "w".into(),
        kind: "stuck",
        detail: String::new(),
        for_minutes: m,
        fingerprint: String::new(),
    };
    let mut p = Pushed::new();
    let first = select_new(&mut p, &[a(5)]).len() == 1;
    let quiet = select_new(&mut p, &[a(6)]).is_empty();
    Probe {
        name: "channel: new condition pushed once, repeat suppressed",
        red_fires: first,
        green_passes: quiet,
    }
}

/// `air install` merge adds our hooks to an empty config and changes nothing the second time.
fn probe_install_merge() -> Probe {
    use crate::cmd::install::merge_hooks;
    let once = merge_hooks(serde_json::json!({}));
    let added = once
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .is_some_and(Value::is_array);
    let idempotent = merge_hooks(once.clone()) == once;
    Probe {
        name: "install: hook merge adds once, idempotent after",
        red_fires: added,
        green_passes: idempotent,
    }
}

fn base_facts() -> GateFacts {
    GateFacts {
        worker: "probe".into(),
        head: "0123456789abcdef".into(),
        green_at_head: true,
        last_green_sha: None,
        main_is_ancestor: true,
        bead_claimed_by_worker: true,
        runs_at_head: (1, 0),
        digest_present: None,
        digest_dir: None,
        bead: None,
        advisory: false,
    }
}

fn probe_gate_verify() -> Probe {
    let green = handover_verdict(&base_facts()).pass;
    let mut f = base_facts();
    f.green_at_head = false;
    let red = handover_verdict(&f).block;
    Probe {
        name: "gate: verify-green-at-head",
        red_fires: red,
        green_passes: green,
    }
}

fn probe_gate_main() -> Probe {
    let green = handover_verdict(&base_facts()).pass;
    let mut f = base_facts();
    f.main_is_ancestor = false;
    let red = handover_verdict(&f).block;
    Probe {
        name: "gate: main-merged",
        red_fires: red,
        green_passes: green,
    }
}

fn probe_handover_matcher() -> Probe {
    Probe {
        name: "hook: handover command matcher",
        red_fires: is_handover_command("bd close fd-1")
            && is_handover_command("bd update fd-1 -s awaiting_review"),
        green_passes: !is_handover_command("git commit -am wip")
            && !is_handover_command("bd update fd-1 --claim"),
    }
}

fn probe_ledger_roundtrip() -> Probe {
    let ok = (|| -> Result<(bool, bool), String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let run = VerifyRun {
            id: new_id(),
            worker: "probe".into(),
            sha: "abc".into(),
            kind: Kind::Verify,
            exit_code: 0,
            trigger: "selftest".into(),
            failing_step: None,
            started_at: "2026-01-01T00:00:00Z".into(),
            finished_at: "2026-01-01T00:00:00Z".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
        };
        l.record_verify(&run).map_err(|e| e.to_string())?;
        let green = l
            .is_green_at("probe", "abc", Kind::Verify)
            .map_err(|e| e.to_string())?;
        let red = !l
            .is_green_at("probe", "zzz", Kind::Verify)
            .map_err(|e| e.to_string())?;
        Ok((red, green))
    })();
    let (red, green) = ok.unwrap_or((false, false));
    Probe {
        name: "ledger: verify_runs round-trip",
        red_fires: red,
        green_passes: green,
    }
}

/// Real `git merge-base --is-ancestor` on a scratch repo: proves the spawn path and the
/// exit-code interpretation (0 yes / 1 no).
fn probe_git_ancestor() -> Probe {
    let res = (|| -> Result<(bool, bool), String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let g = |args: &[&str]| -> Result<String, String> {
            let out = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "air")
                .env("GIT_AUTHOR_EMAIL", "air@example.invalid")
                .env("GIT_COMMITTER_NAME", "air")
                .env("GIT_COMMITTER_EMAIL", "air@example.invalid")
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        g(&["init", "-q", "-b", "main"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "a"])?;
        let a = g(&["rev-parse", "HEAD"])?;
        g(&["checkout", "-q", "-b", "wt"])?;
        g(&["commit", "-q", "--allow-empty", "-m", "b"])?;
        let b = g(&["rev-parse", "HEAD"])?;
        let yes = crate::git::is_ancestor(&dir, &a, &b).map_err(|e| e.to_string())?;
        let no = crate::git::is_ancestor(&dir, &b, &a).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir);
        Ok((!no, yes))
    })();
    let (red, green) = res.unwrap_or((false, false));
    Probe {
        name: "git: is-ancestor exit codes",
        red_fires: red,
        green_passes: green,
    }
}

/// air-2ct: the `--task` text must reach claude as the prompt, not as a trailing value of
/// the variadic `--disallowed-tools` list. Red: the old ordering (task appended after the
/// deny list) is reported as eaten. Green: `air worker --task` launched against a stub
/// `claude` (`AIR_CLAUDE_BIN`) hands the stub the task as its first argument.
fn probe_worker_task_prompt() -> Probe {
    use crate::cmd::launch::{task_is_prompt, worker_argv};
    let task = "say hello, it's $HOME";
    let mut old = worker_argv("w", "air", std::path::Path::new("/r/roles.md"), &[]);
    old.push(task.to_string());
    let red = !task_is_prompt(&old, task);

    let green = (|| -> Result<bool, String> {
        let dir = std::env::temp_dir().join(format!("air-selftest-{}", new_id()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let git = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q", "-b", "main"])
            .output()
            .map_err(|e| e.to_string())?;
        if !git.status.success() {
            return Err(String::from_utf8_lossy(&git.stderr).to_string());
        }
        // The stub records its argv in a file rather than on stdout: without a tty (this
        // probe under `air record verify`, a Bash tool) the launcher starts the stub inside a
        // detached tmux session (air-tdc), where stdout is the pane. With a tty it execs
        // the stub directly. Either way the file appears; the socket keeps tmux private.
        let stub = dir.join("claude-stub");
        let argv_file = dir.join("argv");
        std::fs::write(
            &stub,
            format!(
                "#!/bin/sh\nprintf '%s\\0' \"$@\" > {}.tmp && mv {}.tmp {}\n",
                argv_file.display(),
                argv_file.display(),
                argv_file.display()
            ),
        )
        .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let socket = format!("air-selftest-{}", new_id());
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out = Command::new(exe)
            .current_dir(&dir)
            .env("AIR_CLAUDE_BIN", &stub)
            .env("AIR_TMUX_SOCKET", &socket)
            .env_remove("AIR_TMUX_MODE")
            .args(["worker", "w", "--task", task])
            .output()
            .map_err(|e| e.to_string())?;
        let mut raw = None;
        // Up to 10 s: a fresh executable's first exec can take seconds on macOS.
        for _ in 0..1000 {
            if let Ok(b) = std::fs::read(&argv_file) {
                raw = Some(b);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = Command::new("tmux")
            .args(["-L", &socket, "kill-server"])
            .output();
        let _ = std::fs::remove_dir_all(&dir);
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        let raw = raw.ok_or_else(|| "stub never ran".to_string())?;
        let argv: Vec<String> = String::from_utf8_lossy(&raw)
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        Ok(argv.first().is_some_and(|a| a == task) && task_is_prompt(&argv, task))
    })()
    .unwrap_or(false);
    Probe {
        name: "launch: --task reaches claude as the prompt",
        red_fires: red,
        green_passes: green,
    }
}

/// air-09i: a worker stopping with no claim while beads are ready is nudged once. Red: the
/// gate fires on those facts (ready beads, no claim, fresh stop). Green: the block names the
/// beads, then passes once `stop_hook_active` is set (the loop guard) and never for the
/// coordinator.
/// air-ouw: the nudge must never name a bead `air claim` would refuse. Both recorded triggers
/// invalidate a cached list, and neither is a label problem in general:
/// an `owner`-labelled bead (2026-08-22, the migration) and a bead a PEER already claimed
/// (2026-08-22 19:07, the nudge offered alpha the bead beta was holding). `claimable` applied
/// to a LIVE `bd ready` answers both, because bd's ready set is open-and-unblocked, so a
/// claimed bead is already absent and only the label needs filtering. Validating a cached list
/// against labels would have caught the first and missed the second.
///
/// Red: the cache still holds both, which is the behaviour this bead reports. Green: what the
/// nudge actually names, `claimable(live)`, holds neither.
fn probe_nudge_names_only_claimable() -> Probe {
    use crate::cmd::ready_cache::claimable;
    use air_hooks::stop_nudge;

    let issue = |id: &str, status: &str, labels: &[&str]| air_bd::Issue {
        id: id.to_string(),
        status: status.to_string(),
        labels: labels.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    // What the cache was written from, before anything moved.
    let cached: Vec<String> = ["ad-free", "ad-owner", "ad-taken"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Red: nudging from the cache offers all three, including the two that moved.
    let red = stop_nudge("worker", false, &cached, false)
        .is_some_and(|m| m.contains("ad-owner") && m.contains("ad-taken"));

    // Live bd: the peer's claim left `bd ready` entirely; the owner label did not.
    let live = [
        issue("ad-free", "open", &[]),
        issue("ad-owner", "open", &["owner"]),
    ];
    let confirmed = claimable(&live);
    let green = confirmed == ["ad-free"]
        && stop_nudge("worker", false, &confirmed, false).is_some_and(|m| {
            m.contains("air claim ad-free") && !m.contains("ad-owner") && !m.contains("ad-taken")
        })
        // ...and nothing anywhere admits it might be wrong.
        && !stop_nudge("worker", false, &confirmed, false)
            .is_some_and(|m| m.contains("stale"));
    Probe {
        name: "stop: the nudge names only what air claim would accept (no owner label, no peer's claim)",
        red_fires: red,
        green_passes: green,
    }
}

fn probe_stop_nudge() -> Probe {
    use air_hooks::stop_nudge;
    let ready = vec!["fd-1".to_string()];
    let red = stop_nudge("worker", false, &ready, false).is_some();
    let once =
        stop_nudge("worker", false, &ready, false).is_some_and(|r| r.contains("air claim fd-1"));
    let then_pass = stop_nudge("worker", false, &ready, true).is_none()
        && stop_nudge("coordinator", false, &ready, false).is_none()
        && stop_nudge("worker", true, &ready, false).is_none();
    Probe {
        name: "stop: nudge once when ready beads and no claim",
        red_fires: red,
        green_passes: once && then_pass,
    }
}

/// air-4cr: a verify in flight is a fact `air status` shows and `air land` names.
///
/// The failure: adopter's coordinator invalidated three workers' verifies in one round by
/// landing under them, with nothing to consult. A full verify is ~420 s there and the landing
/// rate is faster, so their answer was a hand protocol (worker warns, coordinator holds).
///
/// Red: with one run in flight, `air status` prints a line naming the worker and `air land`
/// warns. Green: with nothing running both are silent, and a run whose process died is not
/// running — the reader prunes it rather than leaving a row nobody can clear.
fn probe_verify_in_flight() -> Probe {
    use crate::cmd::land::in_flight_warnings;
    use crate::cmd::status::{Snapshot, render_for_probe};
    use air_ledger::verify::InFlight;

    let at = "2026-08-29T12:07:00Z";
    let flight = |worker: &str, pid: Option<i64>| InFlight {
        id: format!("id-{worker}"),
        worker: worker.into(),
        sha: "abcdef1234".into(),
        kind: Kind::Verify,
        command: "make verify".into(),
        pid,
        started_at: "2026-08-29T12:00:00Z".into(),
    };
    let snap = |flights: Vec<InFlight>| Snapshot {
        at: at.into(),
        verifies_in_flight: flights,
        ..Default::default()
    };

    let shown = render_for_probe(&snap(vec![flight("alpha", Some(1))]));
    let warned = in_flight_warnings(&[flight("alpha", Some(1))], at);
    let red = shown.contains("verify in flight: alpha")
        // Elapsed in seconds: rounding a just-started run to "0 min" is what makes it look
        // ignorable, and 420 s is the number that decided this bead.
        && shown.contains("420s")
        && warned.len() == 1
        && warned
            .first()
            .is_some_and(|w| w.contains("alpha") && w.contains("invalidates it"));

    // A crashed `air record` leaves a row; the next reader clears it, so nothing accumulates.
    let pruned = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.verify_started(&flight("beta", Some(424_242)))
            .map_err(|e| e.to_string())?;
        let live = l.in_flight_pruned(|_| false).map_err(|e| e.to_string())?;
        let left = l.verifies_in_flight().map_err(|e| e.to_string())?;
        Ok(live.is_empty() && left.is_empty())
    })()
    .unwrap_or(false);

    let green = render_for_probe(&snap(vec![]))
        .lines()
        .all(|x| !x.starts_with("verify in flight"))
        && in_flight_warnings(&[], at).is_empty()
        && pruned;
    Probe {
        name: "verify: a run in flight is named by status and warned about by land; nothing running is silent and a dead pid clears",
        red_fires: red,
        green_passes: green,
    }
}

/// air-bxe: a landing has a state, and `air status` holds it.
///
/// The failure: adopter's coordinator reported a land done three times before the process
/// exited, because the merge commit appears minutes before the verify finishes with the
/// rollback armed. Their workaround was `pgrep`, which produced two defects of its own —
/// `pgrep` printing nothing makes the `ps` after it list every process the user owns, and they
/// read a thirty-line listing as evidence a land was running when it was evidence of the
/// opposite. Separately, a land killed by a closed pipe merged, verified, and wrote no row at
/// all, leaving main green at a sha no landing mentioned.
///
/// Red: an in-flight landing is named, and one whose process is gone says so and names the sha
/// to rewind to. Green: no in-flight landing is silent, and reporting an outcome retires the
/// row without counting as a second attempt.
fn probe_landing_state() -> Probe {
    use crate::cmd::status::{LandingInFlight, Snapshot, landing_in_flight_line, render_for_probe};
    use air_ledger::landings::Landing;

    let at = "2026-08-29T12:02:00Z";
    let row = |result: &str| Landing {
        id: "L1".into(),
        worker: "alpha".into(),
        sha: "branchhead".into(),
        tip_sha: Some("bbbbbbbb99".into()),
        result: result.into(),
        failing_step: None,
        verify_run_id: None,
        attempt_no: 1,
        beads: vec!["air-1".into()],
        open_beads: vec![],
        merge_commit: Some("cccccccc99".into()),
        pid: Some(4242),
        started_at: "2026-08-29T12:00:00Z".into(),
        finished_at: "2026-08-29T12:00:00Z".into(),
    };
    let snap = |flights: Vec<LandingInFlight>| Snapshot {
        at: at.into(),
        landings_in_flight: flights,
        ..Default::default()
    };

    let running = LandingInFlight {
        landing: row("in-flight"),
        alive: Some(true),
    };
    let killed = LandingInFlight {
        landing: row("in-flight"),
        alive: Some(false),
    };
    let shown = render_for_probe(&snap(vec![running.clone()]));
    let killed_line = landing_in_flight_line(&killed, at);
    let red = shown.contains("landing in flight: alpha (air-1) merged at cccccccc")
        && shown.contains("verifying now")
        // "in main" is not "survived": the line has to name the armed rollback target, which
        // is the thing `git merge-base --is-ancestor` cannot tell anyone.
        && shown.contains("rollback armed to bbbbbbbb")
        && killed_line.contains("GONE")
        && killed_line.contains("git reset --hard bbbbbbbb99");

    // The row retires when the outcome is written, and updating it is not a new attempt.
    let lifecycle = (|| -> Result<bool, String> {
        let l = Ledger::open_in_memory().map_err(|e| e.to_string())?;
        l.record_landing(&row("in-flight"))
            .map_err(|e| e.to_string())?;
        let mid = l.landings_in_flight().map_err(|e| e.to_string())?.len();
        l.record_landing(&row("landed"))
            .map_err(|e| e.to_string())?;
        let after = l.landings_in_flight().map_err(|e| e.to_string())?.len();
        let attempts = l.landing_attempts("alpha").map_err(|e| e.to_string())?;
        Ok(mid == 1 && after == 0 && attempts == 1)
    })()
    .unwrap_or(false);

    let green = render_for_probe(&snap(vec![]))
        .lines()
        .all(|x| !x.starts_with("landing in flight"))
        && lifecycle;
    Probe {
        name: "land: a landing says in-flight from the merge until it reports, a killed one says so with the rewind sha, and reporting retires the row",
        red_fires: red,
        green_passes: green,
    }
}
