//! `air bd-server up|status`: the project's Dolt server for bd, started and kept alive by Air.
//!
//! Owner, 2026-09-26: bd runs in server mode, one `dolt sql-server` per project, with its data
//! in `<main>/.air/dolt/`. Air starts it before sessions start (`air fleet up`, the launchers),
//! `air init --write` sets it up for a new project, and the coordinator's channel poll restarts
//! it when it stops answering and tells the coordinator once. Air still calls bd only through
//! the bd CLI; this module never talks to the database.
//!
//! Facts it reads, all bd's own: `"dolt_mode": "server"` and `"dolt_database"` in
//! `.beads/metadata.json`, and the port in `.beads/dolt-server.port`, which bd 1.3.0 names as
//! its primary source for the port (bd warns that `dolt_server_port` in `metadata.json` is
//! deprecated). A project in embedded mode is left alone.
//!
//! The server runs in a detached tmux session `<project>-dolt` so the owner can attach to it,
//! as `dolt sql-server --host 127.0.0.1 --port <p> --data-dir <main>/.air/dolt/data`, its
//! output appended to `<main>/.air/dolt/server.log`. Only 127.0.0.1: the server has a
//! passwordless `root`.
//!
//! Removal: when bd keeps its own server alive for a project, or Air's projects go back to
//! embedded mode.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use air_ledger::Ledger;
use air_ledger::deliveries::Outgoing;
use serde::Serialize;
use serde_json::json;

/// How long a start waits for the port to answer. A Dolt server on this machine answered in
/// under two seconds on 2026-09-26; the rest is headroom for a loaded machine.
pub const START_WAIT: Duration = Duration::from_secs(10);

/// What `.beads/` says about how bd reaches its data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum Mode {
    /// No `.beads/metadata.json`: no beads here.
    None,
    Embedded,
    Server {
        /// From `.beads/dolt-server.port`; `None` when the file is missing or not a number.
        port: Option<u16>,
        database: String,
    },
}

pub fn mode(main: &Path) -> Mode {
    let Some(v) = std::fs::read_to_string(main.join(".beads/metadata.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    else {
        return Mode::None;
    };
    if v.get("dolt_mode").and_then(|m| m.as_str()) != Some("server") {
        return Mode::Embedded;
    }
    Mode::Server {
        port: std::fs::read_to_string(main.join(".beads/dolt-server.port"))
            .ok()
            .and_then(|s| s.trim().parse().ok()),
        database: v
            .get("dolt_database")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

pub fn dolt_dir(main: &Path) -> PathBuf {
    main.join(".air/dolt")
}

pub fn data_dir(main: &Path) -> PathBuf {
    dolt_dir(main).join("data")
}

pub fn session(repo: &Path) -> String {
    super::tmux::session_name(&super::tmux::project_prefix(repo), "dolt")
}

/// Whether something accepts a TCP connection on 127.0.0.1:`port`.
pub fn answers(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(300),
    )
    .is_ok()
}

/// Who answers on the project's port. "Something answers" is not "the project's server is
/// up": with Air's server down, bd started its own with an empty database on the same port,
/// twice on 2026-09-26, and every bd command then saw an empty task store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "listener", rename_all = "kebab-case")]
pub enum Listener {
    /// Nothing answers.
    None,
    /// A server whose data directory is this project's `.air/dolt/data`.
    Ours,
    /// Something else answers: its pid and command line.
    Other { what: String },
    /// Something answers and `lsof`/`ps` could not say what; treated as up, as before.
    Unknown,
}

/// Pure: does a listener with this command line and working directory serve `data`? The data
/// directory is `--data-dir` (relative to `cwd` when relative), else `cwd`, which is what
/// `dolt sql-server` serves without the flag.
pub fn serves(data: &Path, command: &str, cwd: Option<&str>) -> bool {
    let flag = command.find("--data-dir").map(|i| {
        let rest = command
            .get(i.saturating_add("--data-dir".len())..)
            .unwrap_or("");
        let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
        let end = rest.find(" --").unwrap_or(rest.len());
        rest.get(..end).unwrap_or(rest).trim().to_string()
    });
    let served = match (flag, cwd) {
        (Some(d), Some(c)) if !Path::new(&d).is_absolute() => Path::new(c).join(d),
        (Some(d), _) => PathBuf::from(d),
        (None, Some(c)) => PathBuf::from(c),
        (None, None) => return false,
    };
    let norm = |p: &Path| {
        p.canonicalize()
            .unwrap_or_else(|_| p.components().collect::<PathBuf>())
    };
    norm(&served) == norm(data)
}

fn output(prog: &str, args: &[&str]) -> Option<String> {
    Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Who answers on `port`, judged against `main`'s data directory.
pub fn listener(main: &Path, port: u16) -> Listener {
    if !answers(port) {
        return Listener::None;
    }
    let tcp = format!("-iTCP:{port}");
    let Some(pid) = output("lsof", &["-nP", &tcp, "-sTCP:LISTEN", "-t"])
        .and_then(|s| s.lines().next().map(str::to_string))
    else {
        return Listener::Unknown;
    };
    let Some(command) = output("ps", &["-o", "command=", "-p", &pid]) else {
        return Listener::Unknown;
    };
    let cwd = output("lsof", &["-a", "-p", &pid, "-d", "cwd", "-Fn"]).and_then(|s| {
        s.lines()
            .find_map(|l| l.strip_prefix('n').map(str::to_string))
    });
    if serves(&data_dir(main), &command, cwd.as_deref()) {
        Listener::Ours
    } else {
        Listener::Other {
            what: format!("pid {pid}: {command}"),
        }
    }
}

/// What `up` and `status` say when something else holds the port.
pub fn other_text(port: u16, main: &Path, what: &str) -> String {
    format!(
        "port {port} is answered by a process that does not serve this project's data ({}): \
         {what}. Air started nothing. Stop that process, then run `air bd-server up`.",
        data_dir(main).display()
    )
}

/// One line for `air doctor`, `air status` and `air bd-server status`.
pub fn line(m: &Mode, up: bool) -> String {
    match m {
        Mode::None => "bd: no .beads here".to_string(),
        Mode::Embedded => "bd: embedded".to_string(),
        Mode::Server { port: None, .. } => {
            "bd: server mode, but .beads/dolt-server.port is missing".to_string()
        }
        Mode::Server { port: Some(p), .. } if up => format!("bd: server 127.0.0.1:{p}, up"),
        Mode::Server { port: Some(p), .. } => {
            format!("bd: server 127.0.0.1:{p}, DOWN; `air bd-server up` starts it")
        }
    }
}

/// What a start did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// Not server mode: nothing to do.
    NotServer,
    /// The port already answered.
    Up {
        port: u16,
    },
    Started {
        port: u16,
        session: String,
    },
    Failed {
        why: String,
    },
}

impl Outcome {
    pub fn text(&self) -> String {
        match self {
            Outcome::NotServer => "bd is not in server mode here; nothing to start".to_string(),
            Outcome::Up { port } => format!("bd server is up on 127.0.0.1:{port}"),
            Outcome::Started { port, session } => {
                format!("started the bd server on 127.0.0.1:{port} in tmux session {session}")
            }
            Outcome::Failed { why } => format!("bd server not started: {why}"),
        }
    }
}

/// The programs a start runs, found on PATH by default. Absolute, because the command runs in
/// the tmux server's environment, whose PATH may be older than this process's.
#[derive(Debug, Clone)]
pub struct Bins {
    pub tmux: PathBuf,
    pub dolt: PathBuf,
}

fn which(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join(name))
            .find(|c| c.is_file())
    })
}

pub fn bins() -> Result<Bins, String> {
    Ok(Bins {
        tmux: which("tmux").ok_or("tmux is not on PATH")?,
        dolt: which("dolt").ok_or("dolt is not on PATH (`brew install dolt`)")?,
    })
}

fn tmux(bins: &Bins, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(&bins.tmux)
        .args(super::tmux::socket_args())
        .args(args)
        .stdin(Stdio::null())
        .output()
}

/// Start the server for `main` on `port` if nothing answers there, and wait up to `wait` for
/// it to answer. `allow_empty` is for `air init`, which starts the server before bd creates
/// the database; everywhere else a data directory without the database is refused, because bd
/// would create an empty one into which nothing of the project's is.
pub fn start(
    main: &Path,
    session: &str,
    port: u16,
    database: &str,
    allow_empty: bool,
    bins: &Bins,
    wait: Duration,
) -> Outcome {
    if answers(port) {
        return Outcome::Up { port };
    }
    let data = data_dir(main);
    if !allow_empty && !data.join(database).join(".dolt").is_dir() {
        return Outcome::Failed {
            why: format!(
                "{} has no database `{database}`, so this is not a server Air set up; Air does \
                 not start an empty one",
                data.display()
            ),
        };
    }
    if let Err(e) = std::fs::create_dir_all(&data) {
        return Outcome::Failed {
            why: format!("{}: {e}", data.display()),
        };
    }
    let exists = tmux(bins, &["has-session", "-t", &format!("={session}")])
        .is_ok_and(|o| o.status.success());
    if !exists {
        let q = |p: &Path| super::launch::shell_quote(&p.to_string_lossy());
        let cmd = format!(
            "{} sql-server --host 127.0.0.1 --port {port} --data-dir {} 2>&1 | tee -a {}",
            q(&bins.dolt),
            q(&data),
            q(&dolt_dir(main).join("server.log")),
        );
        let data_s = data.to_string_lossy();
        match tmux(
            bins,
            &["new-session", "-d", "-s", session, "-c", &data_s, &cmd],
        ) {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                return Outcome::Failed {
                    why: format!(
                        "tmux new-session: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    ),
                };
            }
            Err(e) => {
                return Outcome::Failed {
                    why: format!("tmux: {e}"),
                };
            }
        }
    }
    let deadline = Instant::now()
        .checked_add(wait)
        .unwrap_or_else(Instant::now);
    while Instant::now() < deadline {
        if answers(port) {
            return Outcome::Started {
                port,
                session: session.to_string(),
            };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Outcome::Failed {
        why: format!(
            "port {port} did not answer within {}s{}; see {} or `tmux attach -t {session}`",
            wait.as_secs(),
            if exists {
                " and the tmux session was already there, so Air started nothing"
            } else {
                ""
            },
            dolt_dir(main).join("server.log").display()
        ),
    }
}

/// `start` for the repo as configured: nothing unless `.beads/` says server mode.
pub fn up(repo: &Path) -> Outcome {
    let main = super::worktree::main_checkout(repo);
    let (port, database) = match mode(&main) {
        Mode::None | Mode::Embedded => return Outcome::NotServer,
        // A fresh clone or a new machine: the port file is per machine and git ignores it.
        // Pick a free port the way `air init` does and write it where bd reads it first.
        Mode::Server {
            port: None,
            database,
        } => match assign_port(&main, free_port(&main)) {
            Ok(p) => (p, database),
            Err(why) => return Outcome::Failed { why },
        },
        Mode::Server {
            port: Some(p),
            database,
        } => (p, database),
    };
    match listener(&main, port) {
        Listener::Ours | Listener::Unknown => return Outcome::Up { port },
        Listener::Other { what } => {
            return Outcome::Failed {
                why: other_text(port, &main, &what),
            };
        }
        Listener::None => {}
    }
    match bins() {
        Ok(b) => start(
            &main,
            &session(repo),
            port,
            &database,
            false,
            &b,
            START_WAIT,
        ),
        Err(why) => Outcome::Failed { why },
    }
}

fn log(
    repo: &Path,
    o: &Outcome,
    trace_ok: super::decisions::Trace,
    trace_bad: super::decisions::Trace,
) {
    let trace = match o {
        Outcome::Started { .. } => trace_ok,
        Outcome::Failed { .. } => trace_bad,
        _ => return,
    };
    if let Ok((ledger, me)) = super::open(repo) {
        super::log_event(&ledger, &me, trace, o, &o.text(), "1 start");
    }
}

/// `up`, recorded, for a launcher: says something only when it started the server or could
/// not. A launch goes ahead either way; bd reports its own connection error to the session.
pub fn ensure_for_launch(repo: &Path, who: &str) {
    let o = up(repo);
    log(
        repo,
        &o,
        super::decisions::BD_SERVER_STARTED,
        super::decisions::BD_SERVER_FAILED,
    );
    if matches!(o, Outcome::Started { .. } | Outcome::Failed { .. }) {
        eprintln!("{who}: {}", o.text());
    }
}

/// `air bd-server up`.
pub fn up_cmd(repo: &Path, json: bool) -> i32 {
    let o = up(repo);
    log(
        repo,
        &o,
        super::decisions::BD_SERVER_STARTED,
        super::decisions::BD_SERVER_FAILED,
    );
    super::emit(json, &o, || o.text());
    i32::from(matches!(o, Outcome::Failed { .. }))
}

/// `air bd-server status`. "Up" is this project's server answering, not any process on the
/// port.
pub fn status_cmd(repo: &Path, json: bool) -> i32 {
    let main = super::worktree::main_checkout(repo);
    let m = mode(&main);
    let who = match &m {
        Mode::Server { port: Some(p), .. } => listener(&main, *p),
        _ => Listener::None,
    };
    let up = matches!(who, Listener::Ours | Listener::Unknown);
    super::emit(
        json,
        &json!({"bd": m, "up": up, "listener": who, "session": session(repo)}),
        || match (&m, &who) {
            (Mode::Server { port: Some(p), .. }, Listener::Other { what }) => {
                format!(
                    "bd: server 127.0.0.1:{p}, NOT this project's: {}",
                    other_text(*p, &main, what)
                )
            }
            _ => line(&m, up),
        },
    );
    0
}

/// Whether the configured port answers; false when there is none.
pub fn view(m: &Mode) -> bool {
    matches!(m, Mode::Server { port: Some(p), .. } if answers(*p))
}

/// A free port for a new project's server, in 3400..3900, which keeps clear of bd's defaults
/// (3307, 3308). The search starts at a point derived from the checkout's path, so two projects
/// set up while the other's server is down are unlikely to pick the same one, and skips any
/// port something already listens on.
pub fn free_port(main: &Path) -> Option<u16> {
    let h = main
        .to_string_lossy()
        .bytes()
        .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    let off = h.checked_rem(500).unwrap_or(0);
    (0..500u32)
        .filter_map(|i| off.saturating_add(i).checked_rem(500))
        .filter_map(|x| u16::try_from(x.saturating_add(3400)).ok())
        .find(|p| {
            !answers(*p)
                && std::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], *p))).is_ok()
        })
}

/// Write `port` to `.beads/dolt-server.port` for a project that has none (a fresh clone or a
/// new machine). `None` means no free port in the range.
pub fn assign_port(main: &Path, port: Option<u16>) -> Result<u16, String> {
    let port = port.ok_or("no free port in 3400..3900 for the bd server")?;
    let path = main.join(".beads/dolt-server.port");
    std::fs::write(&path, port.to_string()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(port)
}

/// What `air init --write` appends to `.beads/config.yaml` in server mode.
pub const NO_AUTO_START: &str = "\n# Air starts this project's Dolt server (air bd-server up); bd must not start its own,\n# which would serve an empty database from .beads/dolt on the same port.\ndolt.auto-start: false\n";

/// Stop bd starting its own Dolt server when Air's is down. Seen twice on 2026-09-26: bd's
/// server came up with an empty database on the same port and rewrote the port file. Appends
/// `dolt.auto-start: false` to `.beads/config.yaml` unless the key is already set there.
pub fn disable_bd_auto_start(main: &Path) -> Result<(), String> {
    let path = main.join(".beads/config.yaml");
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("dolt.auto-start:"))
    {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(NO_AUTO_START);
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// After `bd init --server` in a new project: bd wrote the port into the tracked
/// `metadata.json`. Move it to `.beads/dolt-server.port`, which bd reads first and which its
/// own `.beads/.gitignore` ignores, so no machine's port is committed.
pub fn move_port_out_of_metadata(main: &Path, port: u16) -> Result<(), String> {
    let path = main.join(".beads/metadata.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(o) = v.as_object_mut()
        && o.remove("dolt_server_port").is_some()
    {
        let s = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
        std::fs::write(&path, format!("{s}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let port_file = main.join(".beads/dolt-server.port");
    std::fs::write(&port_file, port.to_string())
        .map_err(|e| format!("{}: {e}", port_file.display()))
}

// ---------- keep-alive, on the coordinator's channel poll ----------

/// bd cache key: the outage the coordinator has been told could not be fixed, or `""`.
const OUTAGE_TOLD: &str = "bd_server_outage_told";

pub fn restarted_text(port: u16, session: &str) -> String {
    format!(
        "bd server was down; restarted it on 127.0.0.1:{port} (tmux session {session}). A bd \
         command that failed in the meantime can be run again."
    )
}

pub fn failed_text(why: &str) -> String {
    format!(
        "bd server is down and Air could not restart it: {why}. bd commands fail until it is \
         up; `air bd-server up` tries again. Air retries on every poll and says nothing more \
         until it is up."
    )
}

/// One keep-alive pass: nothing while the server answers or the project is not in server mode.
/// When it is down, start it; tell the coordinator once that it was restarted, or once per
/// outage that it could not be. `start` is the start to run, so the probe can pass its own.
pub fn keep_alive(
    ledger: &Ledger,
    worker: &str,
    mode: &Mode,
    at: &str,
    start: &mut dyn FnMut(u16, &str) -> Outcome,
) -> Option<Outcome> {
    let Mode::Server {
        port: Some(port),
        database,
    } = mode
    else {
        return None;
    };
    let told = ledger
        .bd_cache_get(OUTAGE_TOLD)
        .ok()
        .flatten()
        .is_some_and(|(v, _)| !v.is_empty());
    if answers(*port) {
        if told {
            let _ = ledger.bd_cache_put(OUTAGE_TOLD, "", at);
        }
        return None;
    }
    let o = start(*port, database);
    let (content, trace) = match &o {
        Outcome::Started { port, session } => {
            let _ = ledger.bd_cache_put(OUTAGE_TOLD, "", at);
            (
                restarted_text(*port, session),
                super::decisions::BD_SERVER_RESTARTED,
            )
        }
        Outcome::Failed { why } if !told => {
            let _ = ledger.bd_cache_put(OUTAGE_TOLD, at, at);
            (failed_text(why), super::decisions::BD_SERVER_RESTART_FAILED)
        }
        _ => return Some(o),
    };
    let _ = ledger.enqueue_delivery(
        &Outgoing {
            to: "coordinator",
            kind: "bd-server",
            key: at,
            subject: "",
            content: &content,
            supersede: true,
        },
        at,
    );
    super::log_event(ledger, worker, trace, &o, &content, "1 outage");
    Some(o)
}

/// The poll's call: the repo's own mode and the real start.
pub fn keep_alive_tick(repo: &Path, ledger: &Ledger, worker: &str) {
    let main = super::worktree::main_checkout(repo);
    let session = session(repo);
    keep_alive(
        ledger,
        worker,
        &mode(&main),
        &super::now(),
        &mut |port, db| match bins() {
            Ok(b) => start(&main, &session, port, db, false, &b, START_WAIT),
            Err(why) => Outcome::Failed { why },
        },
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn mode_reads_bds_own_files() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path();
        assert_eq!(mode(m), Mode::None);
        std::fs::create_dir_all(m.join(".beads")).unwrap();
        std::fs::write(m.join(".beads/metadata.json"), r#"{"dolt_database":"zz"}"#).unwrap();
        assert_eq!(mode(m), Mode::Embedded);
        std::fs::write(
            m.join(".beads/metadata.json"),
            r#"{"dolt_mode":"server","dolt_database":"zz","dolt_server_port":4000}"#,
        )
        .unwrap();
        assert_eq!(
            mode(m),
            Mode::Server {
                port: None,
                database: "zz".into()
            }
        );
        move_port_out_of_metadata(m, 3999).unwrap();
        assert_eq!(
            mode(m),
            Mode::Server {
                port: Some(3999),
                database: "zz".into()
            }
        );
        let meta = std::fs::read_to_string(m.join(".beads/metadata.json")).unwrap();
        assert!(!meta.contains("dolt_server_port"), "{meta}");
    }

    #[test]
    fn a_server_without_its_database_is_not_started() {
        let dir = tempfile::tempdir().unwrap();
        let b = Bins {
            tmux: "/nonexistent/tmux".into(),
            dolt: "/nonexistent/dolt".into(),
        };
        let port = free_port(dir.path()).unwrap();
        let o = start(dir.path(), "x-dolt", port, "zz", false, &b, Duration::ZERO);
        assert!(
            matches!(&o, Outcome::Failed { why } if why.contains("no database `zz`")),
            "{o:?}"
        );
    }

    /// A listener is this project's only when it serves `.air/dolt/data`. The first command
    /// line is the one Air starts; bd's own server runs from `.beads/dolt`.
    #[test]
    fn a_listener_serving_other_data_is_not_this_projects_server() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path();
        let data = data_dir(main);
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(main.join(".beads/dolt")).unwrap();
        let d = data.to_string_lossy().to_string();
        let ours =
            format!("/usr/local/bin/dolt sql-server --host 127.0.0.1 --port 3417 --data-dir {d}");
        assert!(serves(&data, &ours, None));
        assert!(serves(
            &data,
            &format!("dolt sql-server --data-dir={d} --port 1"),
            None
        ));
        assert!(
            serves(&data, "dolt sql-server --port 1", Some(&d)),
            "no flag: the cwd"
        );
        let bds = main.join(".beads/dolt").to_string_lossy().to_string();
        assert!(!serves(&data, "dolt sql-server --port 3417", Some(&bds)));
        assert!(!serves(
            &data,
            "dolt sql-server --data-dir . --port 1",
            Some(&bds)
        ));
        assert!(!serves(&data, "nc -l 3417", None));
    }

    /// A server-mode project with no port file gets one, in init's range.
    #[test]
    fn a_missing_port_file_is_assigned_a_free_port() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path();
        std::fs::create_dir_all(m.join(".beads")).unwrap();
        std::fs::write(
            m.join(".beads/metadata.json"),
            r#"{"dolt_mode":"server","dolt_database":"zz"}"#,
        )
        .unwrap();
        assert!(matches!(mode(m), Mode::Server { port: None, .. }));
        let p = assign_port(m, free_port(m)).unwrap();
        assert!((3400..3900).contains(&p), "{p}");
        assert_eq!(
            mode(m),
            Mode::Server {
                port: Some(p),
                database: "zz".into()
            }
        );
        assert!(assign_port(m, None).is_err());
    }

    /// Init's config line is added once, and a setting already there is left alone.
    #[test]
    fn bd_auto_start_is_turned_off_once() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path();
        std::fs::create_dir_all(m.join(".beads")).unwrap();
        let path = m.join(".beads/config.yaml");
        std::fs::write(&path, "# bd's own comments\n# no-db: false").unwrap();
        disable_bd_auto_start(m).unwrap();
        disable_bd_auto_start(m).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.matches("dolt.auto-start: false").count(), 1, "{text}");
        assert!(
            text.starts_with("# bd's own comments\n# no-db: false\n"),
            "{text}"
        );
        std::fs::write(&path, "dolt.auto-start: true\n").unwrap();
        disable_bd_auto_start(m).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "dolt.auto-start: true\n"
        );
    }

    #[test]
    fn free_port_stays_in_its_range() {
        let dir = tempfile::tempdir().unwrap();
        let p = free_port(dir.path()).unwrap();
        assert!((3400..3900).contains(&p), "{p}");
    }
}
