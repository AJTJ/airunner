//! The one way an integration test spawns a process (air-8o59).
//!
//! The launchers put `AIR_ROLE`, `AIR_ENFORCE`, `AIR_PROJECT`, `AIR_CHANNEL` and `BEADS_ACTOR`
//! on every fleet session, and a child inherits them. So the repo's own verify answered
//! differently depending on which session ran it: on 2026-09-28 every batch was red from the
//! lane's session, because the `air mcp` a test spawned ran as the lane and never pushed the
//! event the test waited for. The `selftest` has the same helper for its own spawns
//! (`air_command` in `crates/cli/src/cmd/selftest.rs`).
//!
//! Every `AIR_*` variable, `BEADS_ACTOR` and `CLAUDE_PID` (a hook records it as the session's
//! live process, air-4n5t) is removed; a test sets what it needs on the
//! returned `Command`. Removed when the launchers stop passing identity through the
//! environment.

use std::ffi::OsStr;
use std::process::Command;

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut c = Command::new(program);
    for (k, _) in std::env::vars_os() {
        if k.to_str()
            .is_some_and(|k| k.starts_with("AIR_") || k == "BEADS_ACTOR" || k == "CLAUDE_PID")
        {
            c.env_remove(k);
        }
    }
    c
}
