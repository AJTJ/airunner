//! `air ui [--port N]`: a read-only, loopback browser view of the fleet (air-o6x.1).
//!
//! One page and five JSON endpoints, and nothing written anywhere. The server is std only: a
//! `TcpListener` on 127.0.0.1, one thread per connection, the request line parsed and nothing
//! else, GET answered and every other method refused with 405. `mcp.rs` hand-rolls its protocol
//! loop for the same reason this does: the tree has no async runtime and no HTTP crate
//! (`docs/design.md` §11), and owning a hundred lines is smaller than auditing one. The
//! research and the shapes are `docs/research/live-ui.md` §3, §4 and §6.
//!
//! The ledger is opened read-only for each request (`Ledger::open_read_only`): never
//! `open_in`, which migrates, so a UI built from a newer tree cannot migrate the ledger under
//! the fleet's older binary; and never `immutable=1`, which skips change detection on a live
//! file. A repository with no ledger yet gets every endpoint's empty shape.
//!
//! The server's working directory is `/`. The readers check lists processes by their cwd
//! (`readers.rs`), and a server sitting in a worktree would read as that worker's own process,
//! silencing `idle-without-claim` and showing up in `air land`'s reader list.
//!
//! This bead is the skeleton: the modules beside this one hold the final shapes with empty
//! bodies, so the page bead and the data beads can be built in parallel against them.
//!
//! Removed when `claude agents` or its successor shows Air's facts, or when a round passes in
//! which the page was not opened.

pub mod attention;
pub mod beads;
pub mod feed;
pub mod fleet;
pub mod lane;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use air_ledger::Ledger;

/// The default port, clear of Dolt's 3400..3900 and 3806, bd's 3307 and 3308, and the other
/// dashboards the research lists (8080, 7321, 9464, 4317).
pub const DEFAULT_PORT: u16 = 7311;

/// The page, embedded so the binary is the whole install. A placeholder in this bead: it
/// fetches `/api/fleet` and prints the JSON; air-o6x.4 writes the real one.
const PAGE: &str = include_str!("page.html");

/// A connection that has not sent its request line in this long is dropped, so a stuck client
/// never holds a thread.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// What every request reads: the `.air` directory of the main checkout.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub air_dir: PathBuf,
}

impl Ctx {
    /// A read-only ledger, or `None` when there is none yet or it cannot be opened; every
    /// endpoint then answers its empty shape.
    pub fn ledger(&self) -> Option<Ledger> {
        Ledger::open_read_only(&self.air_dir).ok()
    }

    /// The main checkout, where the worktrees live: the parent of `.air`.
    pub fn main(&self) -> PathBuf {
        self.air_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.air_dir.clone())
    }
}

/// One answer: the status, the content type and the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Response {
    fn json(v: &impl serde::Serialize) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: serde_json::to_vec(v).unwrap_or_default(),
        }
    }

    fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }

    fn reason(&self) -> &'static str {
        match self.status {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            405 => "Method Not Allowed",
            _ => "Error",
        }
    }
}

/// Pure over the request line: the method and the target, or nothing for a line that is not a
/// request. Only the line is read; headers and bodies are ignored, since every answer closes
/// the connection.
pub fn parse_request_line(line: &str) -> Option<(&str, &str)> {
    let mut it = line.split_whitespace();
    let method = it.next()?;
    let target = it.next()?;
    Some((method, target))
}

/// The answer for one request line. Pure over the line and the ledger it opens.
pub fn handle(ctx: &Ctx, line: &str) -> Response {
    let Some((method, target)) = parse_request_line(line) else {
        return Response::text(400, "bad request\n");
    };
    if method != "GET" {
        return Response::text(405, "GET only: this view is read-only\n");
    }
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (target, None),
    };
    let at = crate::cmd::now();
    match path {
        "/" | "/index.html" => Response {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: PAGE.as_bytes().to_vec(),
        },
        "/api/fleet" => Response::json(&fleet::fleet(ctx.ledger().as_ref(), &ctx.main(), &at)),
        "/api/lane" => Response::json(&lane::lane(ctx.ledger().as_ref(), &at)),
        "/api/beads" => Response::json(&beads::beads(ctx.ledger().as_ref(), &at)),
        "/api/feed" => Response::json(&feed::feed(
            ctx.ledger().as_ref(),
            &ctx.air_dir,
            &at,
            feed::since(query),
        )),
        "/api/attention" => Response::json(&attention::attention(ctx.ledger().as_ref(), &at)),
        _ => Response::text(404, "no such path\n"),
    }
}

fn write_response(stream: &mut TcpStream, r: &Response) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        r.status,
        r.reason(),
        r.content_type,
        r.body.len()
    )?;
    stream.write_all(&r.body)?;
    stream.flush()
}

fn serve_one(ctx: &Ctx, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let Ok(reading) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reading);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    // The headers, up to the blank line, so the client has finished sending before the answer
    // and the close; a client still writing to a closed socket would see a reset.
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|n| n > 0) {
        if header == "\r\n" || header == "\n" {
            break;
        }
        header.clear();
    }
    let r = handle(ctx, &line);
    let _ = write_response(&mut stream, &r);
}

/// Serve on `listener` until the process ends: one thread per connection, each short-lived.
pub fn serve(listener: TcpListener, ctx: Ctx) {
    let ctx = Arc::new(ctx);
    for conn in listener.incoming() {
        let Ok(stream) = conn else {
            continue;
        };
        let ctx = Arc::clone(&ctx);
        std::thread::spawn(move || serve_one(&ctx, stream));
    }
}

/// The one line printed at start, as text or as JSON.
pub fn url_line(port: u16, air_dir: &Path, json: bool) -> String {
    let url = format!("http://127.0.0.1:{port}");
    if json {
        serde_json::json!({"url": url, "port": port, "air_dir": air_dir}).to_string()
    } else {
        format!("air ui: {url} (read-only, loopback only; Ctrl-C stops it)")
    }
}

pub fn run(repo: &Path, port: u16, json: bool) -> i32 {
    let air_dir = match air_ledger::paths::air_dir_for(repo) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("air ui: {e}");
            return 1;
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "air ui: cannot bind 127.0.0.1:{port}: {e}; another process holds the port, \
                 pick one with --port <n> (0 takes a free one)"
            );
            return 1;
        }
    };
    let bound = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(e) => {
            eprintln!("air ui: {e}");
            return 1;
        }
    };
    println!("{}", url_line(bound, &air_dir, json));
    // The server reads the tree from nowhere: `air_dir` is absolute, and a cwd inside a
    // worktree would make this process a reader of it (`readers.rs`).
    if let Err(e) = std::env::set_current_dir("/") {
        eprintln!("air ui: cannot leave the tree (chdir /): {e}");
        return 1;
    }
    serve(listener, Ctx { air_dir });
    0
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    use super::*;

    /// A raw request, so the test exercises the real parser and writer.
    fn get(addr: std::net::SocketAddr, line: &str) -> (u16, String, String) {
        let mut s = TcpStream::connect(addr).unwrap();
        write!(s, "{line}\r\nHost: localhost\r\n\r\n").unwrap();
        let mut raw = String::new();
        s.read_to_string(&mut raw).unwrap();
        let (head, body) = raw.split_once("\r\n\r\n").unwrap();
        let status: u16 = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        let ctype = head
            .lines()
            .find_map(|l| l.strip_prefix("Content-Type: "))
            .unwrap_or_default()
            .to_string();
        (status, ctype, body.to_string())
    }

    fn seeded() -> (tempfile::TempDir, Ctx) {
        let tmp = tempfile::tempdir().unwrap();
        let air_dir = tmp.path().join(".air");
        let l = Ledger::open_in(&air_dir).unwrap();
        l.conn()
            .execute(
                "INSERT INTO sessions (session_id, worker, state, changed_at, started_at, role, model) \
                 VALUES ('s1', 'w1', 'idle', 't0', 't0', 'worker', 'claude-x')",
                [],
            )
            .unwrap();
        (tmp, Ctx { air_dir })
    }

    #[test]
    fn the_request_line_is_all_that_is_parsed() {
        assert_eq!(
            parse_request_line("GET /api/feed?since=3 HTTP/1.1\r\n"),
            Some(("GET", "/api/feed?since=3"))
        );
        assert_eq!(parse_request_line("\r\n"), None);
        assert_eq!(parse_request_line("GET"), None);
    }

    /// The page and every endpoint answer with their content type, an unknown path is 404,
    /// a method other than GET is 405, and the seeded session is on the fleet.
    #[test]
    fn the_server_answers_the_page_and_every_endpoint_on_a_free_port() {
        let (_tmp, ctx) = seeded();
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || serve(listener, ctx));

        let (status, ctype, body) = get(addr, "GET / HTTP/1.1");
        assert_eq!((status, ctype.as_str()), (200, "text/html; charset=utf-8"));
        assert!(body.contains("/api/fleet"), "{body}");
        for path in [
            "/api/fleet",
            "/api/lane",
            "/api/beads",
            "/api/feed",
            "/api/attention",
        ] {
            let (status, ctype, body) = get(addr, &format!("GET {path} HTTP/1.1"));
            assert_eq!(
                (status, ctype.as_str()),
                (200, "application/json"),
                "{path}"
            );
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert!(v["at"].is_string(), "{path}: {body}");
        }
        let (status, _, body) = get(addr, "GET /api/fleet HTTP/1.1");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["sessions"][0]["worker"], "w1", "{body}");
        assert_eq!(v["sessions"][0]["model"], "claude-x", "{body}");
        assert_eq!(v["sessions"][0]["claims"], serde_json::json!([]), "{body}");
        let (status, _, body) = get(addr, "GET /api/feed?since=7 HTTP/1.1");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["next_offset"], 7, "{body}");
        assert_eq!(get(addr, "GET /nope HTTP/1.1").0, 404);
        assert_eq!(get(addr, "GET /api/nope HTTP/1.1").0, 404);
        assert_eq!(get(addr, "POST /api/fleet HTTP/1.1").0, 405);
        assert_eq!(get(addr, "DELETE / HTTP/1.1").0, 405);
    }

    /// No ledger at all: every endpoint still answers its empty shape.
    #[test]
    fn a_repository_with_no_ledger_gets_the_empty_shapes() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = Ctx {
            air_dir: tmp.path().join(".air"),
        };
        let r = handle(&ctx, "GET /api/fleet HTTP/1.1");
        assert_eq!(r.status, 200);
        let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(v["sessions"], serde_json::json!([]));
        assert_eq!(handle(&ctx, "nonsense").status, 400);
    }

    /// The five shapes, field by field, as the page bead and the data beads build against
    /// them. A renamed or dropped field fails here before it fails in the page.
    #[test]
    fn the_five_shapes_have_the_documented_fields() {
        // serde_json's map is sorted, so the documented order is compared sorted too.
        fn keys(v: &serde_json::Value) -> Vec<String> {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        }
        fn sorted<const N: usize>(fields: [&str; N]) -> Vec<String> {
            let mut k: Vec<String> = fields.iter().map(|f| (*f).to_string()).collect();
            k.sort();
            k
        }
        let at = "2026-11-10T00:00:00Z";
        let f = serde_json::to_value(fleet::fleet(None, Path::new("/m"), at)).unwrap();
        assert_eq!(keys(&f), sorted(["at", "sessions"]));
        let s =
            serde_json::to_value(fleet::Session::empty("w", "worker", "idle", "", "t0")).unwrap();
        assert_eq!(
            keys(&s),
            sorted([
                "worker",
                "role",
                "state",
                "model",
                "pid_alive",
                "head",
                "head_short",
                "claims",
                "since",
                "waiting_for",
                "harness_status",
                "current_tool",
                "last_text",
                "context_tokens",
                "pane_tail"
            ])
        );
        let l = serde_json::to_value(lane::lane(None, at)).unwrap();
        assert_eq!(
            keys(&l),
            sorted([
                "at",
                "lane",
                "inflight",
                "batch_cuts",
                "verify_runs",
                "red",
                "landings",
                "loops"
            ])
        );
        assert_eq!(
            keys(&l["loops"]),
            sorted(["batch_ready_to_verify_s", "done_to_landed_s"])
        );
        let b = serde_json::to_value(beads::beads(None, at)).unwrap();
        assert_eq!(
            keys(&b),
            sorted([
                "at",
                "fetched_at",
                "ready",
                "not_claimable",
                "in_progress",
                "done_waiting",
                "closed_recent",
                "blocked"
            ])
        );
        let bead = serde_json::to_value(beads::Bead::empty("zz-1", "t", "open", "task")).unwrap();
        assert_eq!(
            keys(&bead),
            sorted([
                "id",
                "title",
                "status",
                "type",
                "priority",
                "assignee",
                "labels",
                "blocked_by",
                "claim",
                "done",
                "landed"
            ])
        );
        let fd = serde_json::to_value(feed::feed(None, Path::new("/m/.air"), at, 0)).unwrap();
        assert_eq!(keys(&fd), sorted(["at", "next_offset", "items"]));
        let item = serde_json::to_value(feed::Item {
            at: at.into(),
            source: feed::Source::Event,
            worker: "w".into(),
            kind: "k".into(),
            text: "t".into(),
        })
        .unwrap();
        assert_eq!(
            keys(&item),
            sorted(["at", "source", "worker", "kind", "text"])
        );
        assert_eq!(item["source"], "event");
        let a = serde_json::to_value(attention::attention(None, at)).unwrap();
        assert_eq!(
            keys(&a),
            sorted([
                "at",
                "fleet_stop",
                "conditions",
                "captures",
                "leases",
                "lease_wants"
            ])
        );
        assert_eq!(feed::since(Some("since=12")), 12);
        assert_eq!(feed::since(Some("x=1&since=5")), 5);
        assert_eq!(feed::since(Some("since=bad")), 0);
        assert_eq!(feed::since(None), 0);
    }

    /// air-o6x.4: the embedded page is HTML that names each endpoint path exactly once (the
    /// `API` table), so a renamed route fails here, and it has no line over 120 characters
    /// outside its inline style.
    #[test]
    fn the_page_names_each_endpoint_once_and_keeps_its_lines_short() {
        assert!(PAGE.starts_with("<!doctype html>"), "{}", &PAGE[..40]);
        assert!(PAGE.contains("<html") && PAGE.contains("</html>"));
        for path in [
            "/api/fleet",
            "/api/lane",
            "/api/beads",
            "/api/feed",
            "/api/attention",
        ] {
            assert_eq!(PAGE.matches(path).count(), 1, "{path}");
        }
        let mut in_style = false;
        for (i, line) in PAGE.lines().enumerate() {
            if line.contains("<style>") {
                in_style = true;
            }
            if line.contains("</style>") {
                in_style = false;
            }
            assert!(
                in_style || line.chars().count() <= 120,
                "line {}: {line}",
                i + 1
            );
        }
    }

    #[test]
    fn the_url_line_names_the_port_and_json_carries_the_air_dir() {
        let t = url_line(7311, Path::new("/r/.air"), false);
        assert!(t.starts_with("air ui: http://127.0.0.1:7311"), "{t}");
        let j: serde_json::Value =
            serde_json::from_str(&url_line(0, Path::new("/r/.air"), true)).unwrap();
        assert_eq!(j["url"], "http://127.0.0.1:0");
        assert_eq!(j["air_dir"], "/r/.air");
    }
}
