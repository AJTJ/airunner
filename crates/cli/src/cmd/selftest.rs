//! `air selftest`: red/green probes for every check, run against an in-memory ledger and a
//! scratch git repo. A check that matches nothing prints RED (corpus: guards that pass on
//! nothing are the anti-pattern). Exit 1 if any probe fails.

use std::process::Command;

use air_hooks::{GateFacts, handover_verdict};
use air_ledger::Ledger;
use air_ledger::verify::{Kind, VerifyRun, new_id};
use serde::Serialize;
use serde_json::Value;

use crate::cmd::emit;
use crate::cmd::hook::is_handover_command;

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
        probe_worker_task_prompt(),
        probe_stop_nudge(),
        probe_standstill(),
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
    let mut old = worker_argv("w", std::path::Path::new("/r/roles.md"), &[]);
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
        let stub = dir.join("claude-stub");
        std::fs::write(&stub, "#!/bin/sh\nprintf '%s\\0' \"$@\"\n").map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let out = Command::new(exe)
            .current_dir(&dir)
            .env("AIR_CLAUDE_BIN", &stub)
            .env_remove("AIR_TMUX_MODE")
            .args(["worker", "w", "--task", task])
            .output()
            .map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&dir);
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).to_string());
        }
        let argv: Vec<String> = String::from_utf8_lossy(&out.stdout)
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
fn probe_stop_nudge() -> Probe {
    use air_hooks::stop_nudge;
    let ready = vec!["fd-1".to_string()];
    let red = stop_nudge("worker", false, &ready, false, false).is_some();
    let once = stop_nudge("worker", false, &ready, false, false)
        .is_some_and(|r| r.contains("air claim fd-1"));
    let then_pass = stop_nudge("worker", false, &ready, true, false).is_none()
        && stop_nudge("coordinator", false, &ready, false, false).is_none()
        && stop_nudge("worker", true, &ready, false, false).is_none();
    Probe {
        name: "stop: nudge once when ready beads and no claim",
        red_fires: red,
        green_passes: once && then_pass,
    }
}
