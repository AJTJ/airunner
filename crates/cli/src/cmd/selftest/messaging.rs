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
