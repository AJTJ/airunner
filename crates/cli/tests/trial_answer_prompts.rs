//! `scripts/trial/answer-prompts.sh`, the part of `make trial` that answers the
//! development-channels prompt in every pane of the trial fleet (air-fkb): one Enter per
//! session whose pane shows the prompt, nothing to a pane past its prompts, and exit 1 naming
//! the sessions that showed neither in time. Stub panes on a private tmux socket stand in for
//! the sessions.
#![allow(clippy::unwrap_used)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn write_stub(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

/// `(exit, stdout)` of the script against `project` on `socket`, waiting `seconds`.
fn answer(socket: &str, project: &str, seconds: &str) -> (i32, String) {
    let out = common::command("sh")
        .arg(repo_root().join("scripts/trial/answer-prompts.sh"))
        .args([project, seconds])
        .env("AIR_TMUX_SOCKET", socket)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

#[test]
fn the_prompt_gets_one_enter_and_a_pane_past_it_gets_nothing() {
    if common::command("tmux").arg("-V").output().is_err() {
        eprintln!("SKIP: tmux not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path().canonicalize().unwrap();
    // The prompt, then whatever stdin brings, then the status line a session shows once past
    // its prompts. `answered` exists only once a keystroke arrived.
    let prompting = write_stub(
        &d,
        "prompting.sh",
        "#!/bin/sh\nprintf '\\n Loading development channels\\n\\n > I am using this for local development\\n   Exit\\n'\nread -r answer\nprintf '%s\\n' \"$answer\" > answered.tmp && mv answered.tmp answered\nprintf '\\n  auto mode on (shift+tab to cycle)\\n'\nexec sleep 60\n",
    );
    let past = write_stub(
        &d,
        "past.sh",
        "#!/bin/sh\nprintf '  auto mode on (shift+tab to cycle)\\n'\nread -r answer\nprintf '%s\\n' \"$answer\" > typed-at\nexec sleep 60\n",
    );
    let silent = write_stub(&d, "silent.sh", "#!/bin/sh\nexec sleep 60\n");
    let socket = format!("air-test-{}", std::process::id());
    let pid = std::process::id();
    let tmux = |args: &[&str]| {
        let out = common::command("tmux")
            .args(["-L", &socket])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    let cwd = d.display().to_string();
    let p = format!("tt{pid}");
    tmux(&[
        "new-session",
        "-d",
        "-s",
        &format!("{p}-coordinator"),
        "-c",
        &cwd,
        "--",
        "sh",
        prompting.to_str().unwrap(),
    ]);
    tmux(&[
        "new-session",
        "-d",
        "-s",
        &format!("{p}-lane"),
        "-c",
        &cwd,
        "--",
        "sh",
        past.to_str().unwrap(),
    ]);

    let (code, out) = answer(&socket, &p, "20");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(&format!(
            "answered the development-channels prompt in {p}-coordinator"
        )),
        "{out}"
    );
    assert!(!out.contains(&format!("in {p}-lane")), "{out}");
    // The keystroke reached the pane that showed the prompt, and only that one.
    assert_eq!(std::fs::read_to_string(d.join("answered")).unwrap(), "\n");
    assert!(!d.join("typed-at").exists());

    // A pane showing neither within the wait is named, and the exit says so.
    let q = format!("tq{pid}");
    tmux(&[
        "new-session",
        "-d",
        "-s",
        &format!("{q}-worker-1"),
        "-c",
        &cwd,
        "--",
        "sh",
        silent.to_str().unwrap(),
    ]);
    let (code, out) = answer(&socket, &q, "2");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("{q}-worker-1")), "{out}");

    // No session of that name at all.
    let (code, out) = answer(&socket, "nothing-here", "1");
    assert_eq!(code, 1, "{out}");
    tmux(&["kill-server"]);
}
