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

/// air-dkm1: when the claimable ready set gains a bead, "beads are ready" is queued once for
/// each live idle worker holding no claim, and for nobody else.
///
/// Red: after a seeding tick, a tick where `zz-2` joins the set queues one row for `idle`
/// naming both beads. Green: the worker holding a claim, the lane and the working worker get
/// nothing; the seeding tick, a repeat of the same set, and a set that only shrank queue
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
        let tick = |set: &[&str], at: &str| fan_out_ready(&l, "coordinator", &s, &ids(set), at);
        let seeded = tick(&["zz-1"], "2026-09-26T00:00:00Z");
        let grew = tick(&["zz-1", "zz-2"], "2026-09-26T00:00:30Z");
        let same = tick(&["zz-1", "zz-2"], "2026-09-26T00:01:00Z");
        let shrank = tick(&["zz-2"], "2026-09-26T00:01:30Z");
        let rows = l
            .deliveries_since("2026-09-26T00:00:00Z")
            .map_err(|e| e.to_string())?;
        let red = grew == ["idle"]
            && rows.len() == 1
            && rows.first().is_some_and(|d| {
                d.to_worker == "idle" && d.content.starts_with("beads are ready: zz-1 zz-2")
            });
        let green = seeded.is_empty()
            && same.is_empty()
            && shrank.is_empty()
            && !rows
                .iter()
                .any(|d| matches!(d.to_worker.as_str(), "holding" | "lane" | "busy"));
        Ok((red, green))
    })();
    let (red, green) = res.unwrap_or_else(blocked);
    Probe {
        name: "fanout: new ready beads are queued once for each live idle worker without a claim, never for the lane or a worker holding one",
        red_fires: red,
        green_passes: green,
    }
}
