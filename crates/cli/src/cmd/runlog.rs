//! What a RED run printed, kept where the next reader can find it (air-5ik).
//!
//! `verify_runs.log_path` has existed since schema v1 and has been `None` on every row ever
//! written. A green run's output is nobody's business; a red one's is the only output anybody
//! reads, and Air kept none of it. Four load-related flakes on 2026-09-06 — alerts' red at
//! 37b15cb, `install_and_launch`'s tmux test, the `SubagentStop` probe and the land
//! acceptance-budget probe, each red once and green on re-run under load 66-67 — are all
//! undiagnosable now for exactly that reason. This is not a new mechanism; it is an existing
//! column being filled.
//!
//! **Removal condition**: when a round records no red run whose cause was wanted after the
//! fact. That is a measurement, not an argument: the reds are in `verify_runs` and whether
//! anybody went looking is in the round log.
//!
//! # The three decisions, and what each rests on
//!
//! **Where.** `<main>/.air/logs/<run-id>.log`, beside the ledger and inside the `.air/` that
//! adopting repos already gitignore. Deliberately NOT `.air/events/`: that is an append-only
//! NDJSON stream, one line per invocation, and `air gc` reasons about it by DAY. A build log
//! is neither a line nor a day, and putting it there would make `gc`'s size figures describe
//! two different things.
//!
//! **How much.** The last [`TAIL_BYTES`]. A verify fails at the END — the failing test, the
//! panic, the assertion — so the tail is the part a reader wants, and a whole build log is
//! mostly the part they do not. The number is measured rather than picked: this repo's entire
//! `make verify` output is 32,670 bytes green and 32,700 red (two runs, 2026-09-06), so 64 KiB
//! keeps the WHOLE log here and the tail in a repo whose build talks more.
//!
//! **When it is dropped.** Never, by Air (owner, 2026-09-28, air-06k7). It kept the newest 20,
//! and an adopter's store deleted 5 of 7 red-batch logs within a day while the messages that
//! named them still pointed there. Each red run adds at most [`TAIL_BYTES`]. `air gc` collects
//! only `.air/events/` today, so nothing ages these out.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// How much of a red run's output is kept: the last 64 KiB. See the module doc for why this
/// number and not another — it is the size of this repo's entire verify output, doubled.
pub const TAIL_BYTES: usize = 64 * 1024;

/// The last `cap` bytes of a stream, kept while it runs.
///
/// The verdict is not known until the child exits, so the tail is buffered for EVERY run and
/// written only for a red one. That costs `cap` of memory and keeps the promise the other way
/// round: a green run leaves nothing on disk.
#[derive(Debug)]
pub struct Tail {
    buf: VecDeque<u8>,
    cap: usize,
}

impl Tail {
    pub fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::new(),
            cap,
        }
    }

    /// Append, dropping from the front so the buffer never exceeds `cap`. A single write
    /// larger than `cap` keeps only its own tail, which is the same rule applied once.
    pub fn push(&mut self, bytes: &[u8]) {
        if self.cap == 0 {
            return;
        }
        let start = bytes.len().saturating_sub(self.cap);
        self.buf.extend(bytes.get(start..).unwrap_or(&[]));
        while self.buf.len() > self.cap {
            self.buf.pop_front();
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf.into()
    }
}

/// Where the logs live for a repo whose `.air` is `air_dir`.
pub fn dir(air_dir: &Path) -> PathBuf {
    air_dir.join("logs")
}

/// Pure: does a run with this exit keep its output? Every non-green one, so red AND killed.
///
/// A kill is not a verdict (air-ppm) but its output is exactly what somebody wants: a killed
/// run is the one most likely to be a loaded machine rather than a defect, which is the whole
/// population air-5ik was filed about. A green keeps nothing: nobody reads it, and the store is
/// never pruned, so greens would grow it by an order of magnitude more than reds.
pub fn keeps_output(exit_code: i32) -> bool {
    exit_code != 0
}

/// Write `bytes` as the log for run `id` and answer the path recorded on the row. `None` when nothing could be written: a log is a courtesy and
/// must never turn a recorded verdict into no verdict at all.
pub fn write(air_dir: &Path, id: &str, bytes: &[u8]) -> Option<PathBuf> {
    let d = dir(air_dir);
    std::fs::create_dir_all(&d).ok()?;
    let path = d.join(format!("{id}.log"));
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_keeps_the_end_and_never_exceeds_the_cap() {
        let mut t = Tail::new(4);
        t.push(b"abc");
        t.push(b"de");
        assert_eq!(t.into_bytes(), b"bcde");
        // One write larger than the cap keeps its own tail, not its head.
        let mut t = Tail::new(3);
        t.push(b"abcdefgh");
        assert_eq!(t.into_bytes(), b"fgh");
        // A zero cap keeps nothing rather than panicking.
        let mut t = Tail::new(0);
        t.push(b"x");
        assert!(t.into_bytes().is_empty());
    }
}
