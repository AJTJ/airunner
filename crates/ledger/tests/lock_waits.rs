//! What waiting for the SQLite write lock costs under the concurrency the fleet actually runs
//! at (air-d75), and the red/green for the recording that measures it.
//!
//! `air-ledger`'s busy budget guards a wait whose failure direction is OPEN: a hook that cannot
//! get the lock errors, and `air hook` turns an error into exit 0, so the one refusal Air makes
//! silently does not refuse. Nothing recorded a single lock wait before this, so the ledger's
//! zero busy errors were the absence of a measurement, not the presence of headroom.
//!
//! Its own test binary because the budget table is process-global: a unit test sharing a
//! process with the crate's other tests could not tell its own lock waits from theirs.
//!
//! Re-run the measurement with:
//!
//! ```text
//! cargo test -p air-ledger --test lock_waits -- --nocapture
//! ```

#![allow(clippy::expect_used)]

use std::path::Path;
use std::time::Instant;

use air_ledger::{Ledger, budgets};

/// One writer's share: `n` immediate transactions against the shared file, which is what every
/// `air` command and every `air hook` invocation does.
fn hammer(dir: &Path, worker: &str, n: usize) {
    let ledger = Ledger::open_in(dir).expect("open");
    for i in 0..n {
        ledger
            .conn()
            .execute(
                "INSERT INTO edit_journal (worker, path, session_id, first_seen, last_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?4) \
                 ON CONFLICT(worker, path) DO UPDATE SET last_seen = excluded.last_seen",
                rusqlite::params![
                    worker,
                    format!("f{i}.rs"),
                    format!("s-{worker}"),
                    "2026-09-06T00:00:00Z"
                ],
            )
            .expect("journal write");
    }
}

/// Six concurrent writers, the fleet's shape at its busiest (a coordinator, four workers and a
/// hook), each writing 60 rows to one WAL file. Prints the distribution and asserts the two
/// things the budget has to be true for: the waits are recorded at all, and none of them gave
/// up. A give-up here is a refusal Air would have skipped.
#[test]
fn lock_waits_under_six_writers() {
    const WRITERS: usize = 6;
    const EACH: usize = 60;

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join(".air");
    // Create the schema once before the race, so the measurement is of ordinary writes and not
    // of six processes racing to migrate.
    drop(Ledger::open_in(&dir).expect("open"));
    let _ = budgets::take();

    let t0 = Instant::now();
    std::thread::scope(|s| {
        for w in 0..WRITERS {
            let dir = dir.clone();
            s.spawn(move || hammer(&dir, &format!("w{w}"), EACH));
        }
    });
    let wall = t0.elapsed();

    let waits = budgets::take();
    let lock = waits.get(budgets::SQLITE_LOCK).cloned().unwrap_or_default();
    let mut ms = lock.ms.clone();
    ms.sort_unstable();
    let at = |num: usize, den: usize| -> u64 {
        if ms.is_empty() {
            return 0;
        }
        let i = ms
            .len()
            .saturating_mul(num)
            .checked_div(den)
            .unwrap_or(0)
            .min(ms.len().saturating_sub(1));
        ms.get(i).copied().unwrap_or(0)
    };
    println!(
        "sqlite-lock: {WRITERS} writers x {EACH} txns in {} ms; {} wait(s) against a {} ms \
         budget, p50 {} ms, p90 {} ms, p99 {} ms, max {} ms, {} give-up(s)",
        wall.as_millis(),
        lock.n,
        lock.budget_ms,
        at(1, 2),
        at(9, 10),
        at(99, 100),
        ms.last().copied().unwrap_or(0),
        lock.hits,
    );

    // Red without the busy handler: `busy_timeout` waits exactly as well and records nothing,
    // so this is zero and the distribution above is a row of dashes.
    assert!(
        lock.n > 0,
        "six concurrent writers produced no recorded lock wait, so the handler is not \
         installed: the waits still happen, they are just invisible again"
    );
    // The direction that matters. A give-up is an `SQLITE_BUSY` returned to a caller that, on
    // a hook path, becomes a fail-open.
    assert_eq!(
        lock.hits, 0,
        "a writer gave up on the lock inside the budget under ordinary fleet concurrency"
    );
}
