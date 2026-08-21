//! Air's beads boundary.
//!
//! Rules (plan 0001 §7; tick 0300): read via `bd --json`; write only through `bd`; CAS and
//! leases are owned by Air's ledger because bd 1.2.2 has neither. The minimal command surface
//! below is verified present in bd 1.2.2: `ready`, `show`, `list`, `update --claim`,
//! `update -s/-a`, `comment`, `close`, `dep`, `blocked`, `recompute-blocked`.
//!
//! `bd` is slow (`ready --json` ≈ 1.1 s locally, tick 0315), so nothing here is called from a
//! hook path — CLI and reconcile paths only.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

#[derive(Debug, thiserror::Error)]
pub enum BdError {
    #[error("bd not runnable at {bin}: {source}")]
    Spawn {
        bin: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("bd timed out after {0:?}")]
    Timeout(Duration),
    #[error("bd exited {code}: {stderr}")]
    Failed { code: i32, stderr: String },
    #[error("bd output was not the JSON we expected: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BdError>;

/// The subset of a bead Air reads. Unknown fields are ignored so minor bd changes do not
/// break us; missing fields default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: i64,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    pub parent: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// What Air needs from a work tracker. `BdCli` is the only implementation today; tests use
/// an in-memory fake.
pub trait WorkLedger {
    fn ready(&self) -> Result<Vec<Issue>>;
    fn in_progress(&self) -> Result<Vec<Issue>>;
    /// `bd list --status <status> --json` (custom statuses such as `awaiting_review` included).
    fn by_status(&self, status: &str) -> Result<Vec<Issue>>;
    fn show(&self, id: &str) -> Result<Option<Issue>>;
    /// `bd update <id> --claim` (assignee = actor, status = in_progress). Air's ledger checks
    /// CAS *before* calling this.
    fn claim(&self, id: &str, actor: &str) -> Result<()>;
    fn set_status(&self, id: &str, status: &str) -> Result<()>;
    fn comment(&self, id: &str, text: &str) -> Result<()>;
}

/// Shell-out implementation.
#[derive(Debug, Clone)]
pub struct BdCli {
    pub bin: PathBuf,
    pub cwd: PathBuf,
    pub timeout: Duration,
}

impl BdCli {
    pub fn new(cwd: &Path) -> Self {
        Self {
            bin: PathBuf::from("bd"),
            cwd: cwd.to_path_buf(),
            timeout: Duration::from_secs(10),
        }
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let child = Command::new(&self.bin)
            .args(args)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| BdError::Spawn {
                bin: self.bin.clone(),
                source,
            })?;
        let (status, stdout, stderr) =
            match wait_drained(child, self.timeout).map_err(|source| BdError::Spawn {
                bin: self.bin.clone(),
                source,
            })? {
                Some(x) => x,
                None => return Err(BdError::Timeout(self.timeout)),
            };
        if !status.success() {
            return Err(BdError::Failed {
                code: status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&stdout).to_string())
    }
}

/// (exit status, stdout, stderr) of a finished child.
type Drained = (std::process::ExitStatus, Vec<u8>, Vec<u8>);

/// Wait for a child with a timeout while draining its pipes on threads, so a child that
/// writes more than the pipe buffer (64 KB) cannot deadlock against us and be mistaken for
/// a hang. Kills and reaps on timeout. Returns (status, stdout, stderr).
fn wait_drained(
    mut child: std::process::Child,
    timeout: std::time::Duration,
) -> std::io::Result<Option<Drained>> {
    use std::io::Read;
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = out_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let err_t = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(p) = err_pipe.as_mut() {
            let _ = p.read_to_end(&mut v);
        }
        v
    });
    let status = match child.wait_timeout(timeout)? {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            // Pipes close when the child dies; the drain threads finish.
            let _ = out_t.join();
            let _ = err_t.join();
            return Ok(None);
        }
    };
    let stdout = out_t.join().unwrap_or_default();
    let stderr = err_t.join().unwrap_or_default();
    Ok(Some((status, stdout, stderr)))
}

/// Parse `bd … --json` list output. Tolerates both a bare array and `{"issues": [...]}`.
pub fn parse_issues(json: &str) -> Result<Vec<Issue>> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    let arr = match v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(mut o) => match o.remove("issues") {
            Some(serde_json::Value::Array(a)) => a,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    arr.into_iter()
        .map(|x| serde_json::from_value(x).map_err(BdError::from))
        .collect()
}

impl WorkLedger for BdCli {
    fn ready(&self) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["ready", "--json"])?)
    }

    fn in_progress(&self) -> Result<Vec<Issue>> {
        self.by_status("in_progress")
    }

    fn by_status(&self, status: &str) -> Result<Vec<Issue>> {
        parse_issues(&self.run(&["list", "--status", status, "--json"])?)
    }

    fn show(&self, id: &str) -> Result<Option<Issue>> {
        let out = self.run(&["show", id, "--json"])?;
        let v: serde_json::Value = serde_json::from_str(&out)?;
        // `bd show --json` returns a single object (or an array of one).
        let obj = match v {
            serde_json::Value::Array(mut a) if !a.is_empty() => a.remove(0),
            other => other,
        };
        if obj.is_null() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_value(obj)?))
    }

    fn claim(&self, id: &str, actor: &str) -> Result<()> {
        self.run(&["update", id, "--claim", "--actor", actor])
            .map(|_| ())
    }

    fn set_status(&self, id: &str, status: &str) -> Result<()> {
        self.run(&["update", id, "-s", status]).map(|_| ())
    }

    fn comment(&self, id: &str, text: &str) -> Result<()> {
        self.run(&["comment", id, text]).map(|_| ())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn parses_bare_array_and_wrapped_object() {
        let bare = r#"[{"id":"fd-1","title":"a","status":"open","priority":1,"labels":["x"]}]"#;
        let wrapped =
            r#"{"issues":[{"id":"fd-2","title":"b","status":"in_progress","assignee":"w1"}]}"#;
        let a = parse_issues(bare).unwrap();
        assert_eq!(a[0].id, "fd-1");
        assert_eq!(a[0].labels, vec!["x"]);
        let b = parse_issues(wrapped).unwrap();
        assert_eq!(b[0].assignee.as_deref(), Some("w1"));
        // Unknown fields and missing ones are tolerated.
        let c = parse_issues(r#"[{"id":"fd-3","weird":true}]"#).unwrap();
        assert_eq!(c[0].id, "fd-3");
        assert_eq!(c[0].priority, 0);
    }
}
