//! Probes for Air's messaging (air-1vri): what reaches which session, and what stays silent.
//!
//! In a child module so the probes for one feature sit together and away from the file six
//! lanes edit at once; they use the parent's scratch-repo helpers through `super`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Probe, blocked, probe_git, probe_repo};

/// A main checkout at `<tmp>/main` with worker worktrees under `.claude/worktrees/<name>`,
/// the layout Air makes. Returns (tmp root, main, [worktree paths]).
pub(super) fn fleet_repo(names: &[&str]) -> Result<(PathBuf, PathBuf, Vec<PathBuf>), String> {
    let root = probe_repo()?;
    let main = root.join("main");
    std::fs::create_dir_all(&main).map_err(|e| e.to_string())?;
    probe_git(&main, &["init", "-q", "-b", "main"])?;
    probe_git(&main, &["commit", "-q", "--allow-empty", "-m", "base"])?;
    let mut wts = Vec::new();
    for n in names {
        let wt = main.join(".claude/worktrees").join(n);
        probe_git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &format!("air/{n}"),
                &wt.display().to_string(),
            ],
        )?;
        wts.push(wt);
    }
    Ok((root, main, wts))
}

fn pushes(repo: &Path) -> Result<Vec<Value>, String> {
    let mut got = Vec::new();
    crate::cmd::mcp::deliver_once(repo, &mut |v| got.push(v.clone()))?;
    Ok(got)
}

/// air-1vri: a message Air addresses to one session is pushed by that session's channel, once,
/// and by no other session's.
///
/// Red: a row queued for `w1` comes out of `w1`'s channel as one `notifications/claude/channel`
/// event carrying the text and `from=air`, and a second tick pushes nothing. Green: `w2`'s
/// channel pushes nothing of `w1`'s, a server the launcher did not attach as a channel
/// (`AIR_CHANNEL` unset) delivers nothing, and only the coordinator and the owner run the
/// attention poll.
pub(super) fn probe_a_message_reaches_only_its_session() -> Probe {
    use crate::cmd::mcp::{delivers, polls_attention};
    use air_ledger::deliveries::Outgoing;

    let res = (|| -> Result<(bool, bool), String> {
        let (root, main, wts) = fleet_repo(&["w1", "w2"])?;
        let out = (|| -> Result<(bool, bool), String> {
            let (w1, w2) = match wts.as_slice() {
                [a, b] => (a, b),
                _ => return Err("two worktrees expected".into()),
            };
            let (ledger, _) = crate::cmd::open(&main)?;
            ledger
                .enqueue_delivery(
                    &Outgoing {
                        to: "w1",
                        kind: "probe",
                        key: "k1",
                        subject: "",
                        content: "hello w1",
                        supersede: false,
                    },
                    "2026-09-26T00:00:00Z",
                )
                .map_err(|e| e.to_string())?;
            let other = pushes(w2)?;
            let first = pushes(w1)?;
            let again = pushes(w1)?;
            let red = first.len() == 1
                && first.first().is_some_and(|v| {
                    v.get("method").and_then(Value::as_str) == Some("notifications/claude/channel")
                        && v.pointer("/params/content").and_then(Value::as_str) == Some("hello w1")
                        && v.pointer("/params/meta/from").and_then(Value::as_str) == Some("air")
                })
                && again.is_empty();
            let green = other.is_empty()
                && !delivers(None)
                && !delivers(Some("0"))
                && delivers(Some("1"))
                && polls_attention("coordinator")
                && polls_attention("owner")
                && !polls_attention("worker")
                && !polls_attention("lane");
            Ok((red, green))
        })();
        let _ = std::fs::remove_dir_all(&root);
        out
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "channel: a message Air addresses to a session is pushed once by that session's channel and by no other; an unattached server delivers nothing",
        red_fires: red,
        green_passes: green,
    }
}

fn session(state: &str) -> crate::cmd::status::Session {
    crate::cmd::status::Session {
        session_id: format!("s-{state}"),
        state: state.into(),
        changed_at: "2026-09-26T00:00:00Z".into(),
        pid: Some(1),
        pid_alive: Some(true),
        has_transcript: true,
        ..Default::default()
    }
}

pub(super) fn view(
    name: &str,
    role: &str,
    state: &str,
    claim: Option<&str>,
) -> crate::cmd::status::WorkerView {
    crate::cmd::status::WorkerView {
        worker: name.into(),
        role: role.into(),
        session: Some(session(state)),
        claims: claim
            .map(|b| air_ledger::claims::Claim {
                bead: b.into(),
                worker: name.into(),
                claimed_at: "t".into(),
                declared_files: Vec::new(),
                first_handover_at: None,
                last_handover_at: None,
                handover_attempts: 0,
                released_at: None,
                release_reason: None,
            })
            .into_iter()
            .collect(),
        ..Default::default()
    }
}

/// air-dkm1, air-ludo: when the claimable ready set gains a bead, "beads are ready" is queued
/// once for each worker with a live session holding no claim, whatever its state, and for
/// nobody else.
///
/// Red: after a seeding tick, a tick where `zz-2` joins the set queues one row each for `idle`
/// and `busy` (mid-turn, holding nothing), naming both beads. Green: the worker holding a claim
/// and the lane get nothing; the seeding tick, a repeat of the same set, and a set that only shrank queue
/// nothing more.
pub(super) fn probe_new_beads_reach_idle_workers_once() -> Probe {
    use crate::cmd::fanout::fan_out_ready;
    use crate::cmd::status::Snapshot;

    let res = (|| -> Result<(bool, bool), String> {
        let l = air_ledger::Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let s = Snapshot {
            workers: vec![
                view("idle", "worker", "idle", None),
                view("holding", "worker", "idle", Some("zz-9")),
                view("lane", "lane", "idle", None),
                view("busy", "worker", "working", None),
            ],
            ..Default::default()
        };
        let ids = |v: &[&str]| v.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
        let tick =
            |set: &[&str], at: &str| fan_out_ready(&l, "coordinator", &s, &ids(set), &[], at);
        let seeded = tick(&["zz-1"], "2026-09-26T00:00:00Z");
        let grew = tick(&["zz-1", "zz-2"], "2026-09-26T00:00:30Z");
        let same = tick(&["zz-1", "zz-2"], "2026-09-26T00:01:00Z");
        let shrank = tick(&["zz-2"], "2026-09-26T00:01:30Z");
        let rows = l
            .deliveries_since("2026-09-26T00:00:00Z")
            .map_err(|e| e.to_string())?;
        // Recipients are told in name order, and the second reads the same beads rotated
        // by one (0.4.9 trial), so the two rows start with the two orders.
        let red = grew == ["busy", "idle"]
            && rows.len() == 2
            && rows
                .iter()
                .any(|d| d.content.starts_with("beads are ready: zz-1 zz-2."))
            && rows
                .iter()
                .any(|d| d.content.starts_with("beads are ready: zz-2 zz-1 ("));
        let green = seeded.is_empty()
            && same.is_empty()
            && shrank.is_empty()
            && !rows
                .iter()
                .any(|d| matches!(d.to_worker.as_str(), "holding" | "lane"));
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "fanout: new ready beads are queued once for each live worker without a claim, whatever its state, never for the lane or a worker holding one",
        red_fires: red,
        green_passes: green,
    }
}

fn batch_run(sha: &str, exit: i32, members: &[(&str, &str)]) -> air_ledger::verify::VerifyRun {
    air_ledger::verify::VerifyRun {
        id: format!("run-{sha}"),
        worker: "lane".into(),
        sha: sha.into(),
        kind: air_ledger::verify::Kind::Verify,
        exit_code: exit,
        trigger: "selftest".into(),
        failing_step: None,
        started_at: "2026-09-26T00:00:40Z".into(),
        finished_at: "2026-09-26T00:05:00Z".into(),
        log_path: Some("/tmp/red.log".into()),
        command: None,
        duration_ms: None,
        output_bytes: None,
        dirty: false,
        tree: None,
        members: members
            .iter()
            .map(|(w, s)| air_ledger::landings::Member {
                worker: (*w).into(),
                sha: (*s).into(),
            })
            .collect(),
        main_sha: Some("main0000".into()),
    }
}

/// air-1vri.2: the lane hears each newly batch-ready branch once, and each member hears its
/// batch's result or its drop at once. A command that already printed the batch-ready set
/// counts as telling the lane.
///
/// Red: a batch-ready branch queues one row for the lane; a green batch gives each named member
/// "batch green ... Close them now" with its beads, a red one gives the exit and the log path,
/// a drop gives "dropped from batch" with the other side and the paths; `air land` and a red
/// batch's `air record` end with the batch-ready set and `next: air batch cut`. Green: the same
/// branch again, and a branch the lane was already told in a command's output, queue nothing;
/// a killed batch and a member with no worker name say nothing; an empty set prints "nothing
/// is batch-ready".
pub(super) fn probe_the_lane_and_members_hear_batch_events_at_once() -> Probe {
    use crate::cmd::fanout::{
        batch_dropped, batch_ready_to_lane, batch_result_notes, next_cut_lines, told_lane,
    };
    use crate::cmd::status::{BatchReady, Snapshot};

    let res = (|| -> Result<(bool, bool), String> {
        let l = air_ledger::Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let br = |w: &str, h: &str| BatchReady {
            worker: w.into(),
            head: h.into(),
            beads: vec![format!("zz-{w}")],
        };
        let mut s = Snapshot {
            workers: vec![view("lane", "lane", "working", None)],
            batch_ready: vec![br("w1", "aaaa1111")],
            ..Default::default()
        };
        let first = batch_ready_to_lane(&l, "coordinator", &s, "2026-09-26T00:00:00Z");
        let again = batch_ready_to_lane(&l, "coordinator", &s, "2026-09-26T00:00:30Z");
        told_lane(&l, "lane", &[br("w2", "bbbb2222")], "2026-09-26T00:00:40Z");
        s.batch_ready.push(br("w2", "bbbb2222"));
        let already_told = batch_ready_to_lane(&l, "coordinator", &s, "2026-09-26T00:01:00Z");

        let beads = |sha: &str| vec![format!("zz-{}", sha.get(..1).unwrap_or(""))];
        let members = [("w1", "a1a1a1a1a1"), ("", "c3c3c3c3c3")];
        let green = batch_result_notes(&batch_run("g0g0g0g0", 0, &members), &beads);
        let red = batch_result_notes(&batch_run("r0r0r0r0", 2, &members), &beads);
        let killed = batch_result_notes(&batch_run("k0k0k0k0", 143, &members), &beads);
        batch_dropped(
            &l,
            "lane",
            &crate::cmd::batch_cut::Dropped {
                worker: "w3".into(),
                head: "d4d4d4d4".into(),
                beads: vec!["zz-3".into()],
                against: "w1".into(),
                against_sha: "a1a1a1a1".into(),
                paths: vec!["src/x.rs".into()],
                stage: "pre-check",
            },
        );
        let rows = l.deliveries_since("1970").map_err(|e| e.to_string())?;
        let dropped = rows.iter().find(|d| d.kind == "batch-dropped");
        let next = next_cut_lines(&[br("w1", "aaaa1111")]);
        let none = next_cut_lines(&[]);

        let red_half = first == 1
            && rows.iter().any(|d| {
                d.to_worker == "lane" && d.kind == "batch-ready" && d.key == "w1@aaaa1111"
            })
            && green.len() == 1
            && green.first().is_some_and(|n| {
                n.to == "w1"
                    && n.kind == "batch-green"
                    && n.beads == "zz-a"
                    && n.content.contains("Close them now")
            })
            && red.first().is_some_and(|n| {
                n.kind == "batch-red"
                    && n.content.contains("exit 2")
                    && n.content.contains("/tmp/red.log")
            })
            && dropped.is_some_and(|d| {
                d.to_worker == "w3"
                    && d.content.starts_with("dropped from batch")
                    && d.content.contains("w1")
                    && d.content.contains("src/x.rs")
            })
            && next.last().map(String::as_str) == Some("next: air batch cut")
            && next.iter().any(|x| x.contains("w1 at aaaa1111"));
        let green_half = again == 0
            && already_told == 0
            && killed.is_empty()
            && red.len() == 1
            && none.len() == 1
            && none.iter().all(|x| x.starts_with("nothing is batch-ready"));
        Ok((red_half, green_half))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "lane: the lane hears each newly batch-ready branch once and members hear a green, red or drop at once; land and a batch record end with the next cut",
        red_fires: red,
        green_passes: green,
    }
}

/// air-1vri.2: the two loop times come from rows, and a wait that has not ended is no sample.
///
/// Red: a branch told to the lane at 00:00:00 whose batch verify started at 00:00:40 measures
/// 40 s, and a member told "batch green" at 00:05:00 who closed at 00:05:55 measures 55 s, and
/// `air status` prints both medians. Green: a branch never batched and a bead never closed add
/// no sample, and with no rows at all status prints no loops line.
pub(super) fn probe_loop_times_are_measured_from_rows() -> Probe {
    use crate::cmd::loops::{line, measure};
    use air_ledger::deliveries::Outgoing;

    let res = (|| -> Result<(bool, bool), String> {
        let l = air_ledger::Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let e = |x: air_ledger::LedgerError| x.to_string();
        let empty = line(&measure(&l, "1970"));
        let q = |to: &str, kind: &str, key: &str, subject: &str, at: &str| {
            l.enqueue_delivery(
                &Outgoing {
                    to,
                    kind,
                    key,
                    subject,
                    content: "x",
                    supersede: false,
                },
                at,
            )
        };
        q(
            "lane",
            "batch-ready",
            "w1@a1",
            "a1a1a1a1a1",
            "2026-09-26T00:00:00Z",
        )
        .map_err(e)?;
        q(
            "lane",
            "batch-ready",
            "w2@b2",
            "b2b2b2b2b2",
            "2026-09-26T00:00:00Z",
        )
        .map_err(e)?;
        l.record_verify(&batch_run("g0g0g0g0", 0, &[("w1", "a1a1a1a1a1")]))
            .map_err(e)?;
        l.record_claim("zz-1", "w1", &[], "2026-09-26T00:00:00Z")
            .map_err(e)?;
        l.record_claim("zz-2", "w1", &[], "2026-09-26T00:00:00Z")
            .map_err(e)?;
        q(
            "w1",
            "batch-green",
            "run-g0",
            "zz-1 zz-2",
            "2026-09-26T00:05:00Z",
        )
        .map_err(e)?;
        l.release_claim("zz-1", "w1", "closed", "2026-09-26T00:05:55Z")
            .map_err(e)?;
        let t = measure(&l, "2026-09-25T00:00:00Z");
        let shown = line(&t).unwrap_or_default();
        let red = t.ready_to_batch == [40]
            && t.green_to_close == [55]
            && shown.contains("median 40 s over 1")
            && shown.contains("median 55 s over 1");
        let green = empty.is_none();
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "loops: batch-ready to its batch and batch green to close are measured from ledger rows; a wait that has not ended is no sample",
        red_fires: red,
        green_passes: green,
    }
}

/// air-1vri.1: a fleet stop refuses new work naming the stop and its author, and only the
/// coordinator and the owner may set or end it.
///
/// Red: with a stop set, the refusal names "the fleet is stopped since" and who set it, the
/// message every session gets says to commit and start nothing, and a worker and the lane are
/// refused `air fleet`. Green: with no stop, and after a resume, nothing is refused, and the
/// coordinator and the owner may steer.
pub(super) fn probe_a_fleet_stop_refuses_new_work_and_only_the_coordinator_sets_it() -> Probe {
    use crate::cmd::fleet::{may_steer, refusal, stop_text};
    use air_ledger::fleet::FleetStop;

    let res = (|| -> Result<(bool, bool), String> {
        let l = air_ledger::Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let before = refusal(&l, "air claim");
        let s = FleetStop {
            stopped_at: "2026-09-26T00:00:00Z".into(),
            by_worker: "coordinator".into(),
            by_role: "coordinator".into(),
            reason: "owner demo".into(),
        };
        l.set_fleet_stop(&s).map_err(|e| e.to_string())?;
        let during = refusal(&l, "air claim").unwrap_or_default();
        l.clear_fleet_stop().map_err(|e| e.to_string())?;
        let after = refusal(&l, "air batch cut");
        let red = during.starts_with("air claim: refused: the fleet is stopped since")
            && during.contains("by the coordinator (coordinator): owner demo")
            && stop_text(&s).contains("commit your work in progress")
            && may_steer("worker").is_err()
            && may_steer("lane").is_err();
        let green = before.is_none()
            && after.is_none()
            && may_steer("coordinator").is_ok()
            && may_steer("owner").is_ok();
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "fleet: a stop refuses new work naming the stop and who set it; only the coordinator and the owner stop or resume",
        red_fires: red,
        green_passes: green,
    }
}

/// air-1vri.3: a released lease is told to each worker that was refused it, oldest want first,
/// once, and the want goes when the message is delivered.
///
/// Red: `b` then `c` are refused `runtime` while `a` holds it; `a` releases and both are queued
/// "runtime is free", `b` first; delivering `b`'s message clears `b`'s want. Green: the releaser
/// is not told, a second pass for the same release queues nothing, and `c` still waits until
/// its own message is delivered.
pub(super) fn probe_a_freed_lease_reaches_those_who_wanted_it() -> Probe {
    use crate::cmd::fanout::{after_delivery, lease_free};
    use air_ledger::leases::Holder;

    let res = (|| -> Result<(bool, bool), String> {
        let l = air_ledger::Ledger::open_in_memory().map_err(|e| e.to_string())?;
        let e = |x: air_ledger::LedgerError| x.to_string();
        let h = |w: &'static str| Holder {
            worker: w,
            session_id: Some("s"),
            pid: Some(1),
            pid_started: Some("x"),
        };
        let healthy = |_: &air_ledger::leases::Lease| None;
        l.lease_take("runtime", &h("a"), "api", "2026-09-26T00:00:00Z", healthy)
            .map_err(e)?;
        l.lease_take("runtime", &h("b"), "sim", "2026-09-26T00:00:01Z", healthy)
            .map_err(e)?;
        l.lease_take("runtime", &h("c"), "sim", "2026-09-26T00:00:02Z", healthy)
            .map_err(e)?;
        l.lease_release("runtime", "a").map_err(e)?;
        let told = lease_free(&l, "a", "runtime", "2026-09-26T00:01:00Z");
        let again = lease_free(&l, "a", "runtime", "2026-09-26T00:01:00Z");
        let b_rows = l.take_deliveries("b", "2026-09-26T00:01:05Z").map_err(e)?;
        for d in &b_rows {
            after_delivery(&l, "b", d);
        }
        let waiting: Vec<String> = l
            .lease_wants("runtime")
            .map_err(e)?
            .into_iter()
            .map(|(w, _, _)| w)
            .collect();
        let red = told == ["b", "c"]
            && b_rows.len() == 1
            && b_rows
                .first()
                .is_some_and(|d| d.content.starts_with("runtime is free"))
            && !waiting.contains(&"b".to_string());
        let green = again.is_empty() && waiting == ["c"] && !told.contains(&"a".to_string());
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "lease: a freed lease is told once to each worker that wanted it, oldest first, and the want goes on delivery",
        red_fires: red,
        green_passes: green,
    }
}
