//! Which beads a branch carries (air-4re).
//!
//! A machine writes the answer and a machine reads it: every commit that does a bead's work
//! carries a `Bead: <id>` trailer, repeatable. Air reads trailers; it does not read prose.
//!
//! Before this, `air land` worked the beads out by scanning commit messages for anything
//! shaped like an id. Bead ids appear in commit messages for two different reasons — because
//! the commit *did* that bead, and because someone mentioned it ("builds on air-3pz",
//! "measured in air-869") — and nothing in the text tells the two apart. air-7kp needed an
//! authorship filter and a branch-point time bound on top of the scan, and against a real
//! branch the count went 8 → 6 → 3 as each was added. Every one of those was a rule about how
//! a person happened to write a sentence, and the pile was still not obviously finished.
//!
//! The owner, watching that: *"parsing just open prose is kind of brittle… if we are going to
//! start inferring task IDs from somewhere, then they should probably be in JSON format or
//! something that is useful."*
//!
//! The prose scan stays for one reason only: commits that predate the trailer. It is dated so
//! it can be deleted (see [`FALLBACK_BEFORE`]), and it is not extended — the point of air-4re
//! is to stop adding rules to it, so nothing here touches [`prose_ids`] beyond calling it.

use std::path::Path;

use crate::git;

/// Commits committed strictly before this instant may be attributed by the prose scan;
/// from it onward, only a `Bead:` trailer counts.
///
/// **Delete the fallback, and `prose_ids`, once no branch in play predates this.** That is the
/// whole removal condition: the fallback exists for history, not for convenience, and a dated
/// one can be removed on evidence instead of argument. `AIR_BEAD_TRAILER_SINCE` overrides it
/// for tests.
pub const FALLBACK_BEFORE: &str = "2026-08-23T00:00:00Z";

pub fn cutoff() -> jiff::Timestamp {
    std::env::var("AIR_BEAD_TRAILER_SINCE")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| FALLBACK_BEFORE.parse().ok())
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
}

/// One commit, reduced to what attribution needs.
pub struct Commit {
    pub committed: String,
    pub message: String,
}

/// `Bead:` trailer values in a commit message, in order, deduplicated.
///
/// A trailer is a line whose whole content is `Bead: <id>`; leading whitespace is tolerated
/// because editors add it, and the key is matched case-insensitively because git's own trailer
/// handling is. Nothing else about the message is read — a commit that mentions twelve beads in
/// its body and carries one trailer is attributed to one bead.
pub fn trailer_ids(message: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in message.lines() {
        let t = line.trim();
        let Some((key, value)) = t.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("bead") {
            continue;
        }
        let id = value.trim();
        // One id per trailer. A list would be a parsing rule, which is what this replaces;
        // repeat the trailer instead, the way git's own `Co-authored-by` does.
        if !id.is_empty() && !id.contains(char::is_whitespace) && !out.iter().any(|x| x == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// The beads `commits` are attributed to.
///
/// Per commit: its trailers if it has any, otherwise the prose scan **only** if it predates
/// the cutoff. A commit at or after the cutoff with no trailer contributes nothing, which is
/// the forcing function — it is meant to be noticed, and `air handover` names the missing
/// trailer before the branch is ever landed.
pub fn ids_of(
    commits: &[Commit],
    prose: impl Fn(&str) -> Vec<String>,
    cutoff: jiff::Timestamp,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in commits {
        let ids = match trailer_ids(&c.message) {
            t if !t.is_empty() => t,
            _ => {
                let old = c
                    .committed
                    .parse::<jiff::Timestamp>()
                    .is_ok_and(|t| t < cutoff);
                if old { prose(&c.message) } else { Vec::new() }
            }
        };
        for id in ids {
            if !out.iter().any(|x| x == &id) {
                out.push(id);
            }
        }
    }
    out
}

/// Split `git log`'s output for [`ids_of`]. Records are separated by RS (0x1e) and the two
/// fields by US (0x1f), so neither can occur inside a commit message.
pub fn parse_log(text: &str) -> Vec<Commit> {
    text.split('\u{1e}')
        .filter_map(|rec| {
            let rec = rec.trim_start_matches('\n');
            let (committed, message) = rec.split_once('\u{1f}')?;
            (!committed.trim().is_empty()).then(|| Commit {
                committed: committed.trim().to_string(),
                message: message.to_string(),
            })
        })
        .collect()
}

/// The beads named by the commits in `range`, trailers first.
pub fn beads_in_range(repo: &Path, range: &str) -> Vec<String> {
    let text = git::run(repo, &["log", "--format=%cI%x1f%B%x1e", range]).unwrap_or_default();
    ids_of(&parse_log(&text), prose_ids, cutoff())
}

/// The old prose scan, kept only for commits older than [`FALLBACK_BEFORE`].
///
/// **Frozen.** No rule may be added here (air-4re): every rule is another statement about how
/// a person happened to write a sentence, and the replacement is the trailer above. It goes
/// when the fallback goes.
pub fn prose_ids(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        let Some((pre, suf)) = raw.split_once('-') else {
            continue;
        };
        let looks_like_id = !pre.is_empty()
            && pre.len() <= 12
            && pre.chars().all(|c| c.is_ascii_lowercase())
            && !suf.is_empty()
            && suf.len() <= 12
            && suf.chars().all(|c| c.is_ascii_alphanumeric());
        if looks_like_id && out.len() < 64 && !out.iter().any(|x| x == raw) {
            out.push(raw.to_string());
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn c(committed: &str, message: &str) -> Commit {
        Commit {
            committed: committed.to_string(),
            message: message.to_string(),
        }
    }

    fn at(s: &str) -> jiff::Timestamp {
        s.parse().unwrap()
    }

    const CUT: &str = "2026-08-23T00:00:00Z";

    #[test]
    fn a_trailer_wins_and_prose_in_the_same_commit_is_ignored() {
        let msg = "fix(land): select from the merge range\n\nBuilds on air-3pz and air-869, \
                   measured against air-7kp.\n\nBead: air-4re\n";
        // The body names three beads it did not do. Only the trailer counts.
        assert_eq!(trailer_ids(msg), ["air-4re"]);
        let got = ids_of(&[c("2026-08-22T10:00:00Z", msg)], prose_ids, at(CUT));
        assert_eq!(got, ["air-4re"], "a mention is not an attribution");
    }

    #[test]
    fn several_trailers_are_several_beads_and_repeats_collapse() {
        let msg = "chore: two at once\n\nBead: air-1a\nbead: air-2b\nBead: air-1a\n";
        assert_eq!(trailer_ids(msg), ["air-1a", "air-2b"]);
        // A value with a space is not an id; a list would be a parsing rule.
        assert!(trailer_ids("Bead: air-1a air-2b\n").is_empty());
    }

    #[test]
    fn the_prose_fallback_is_dated_and_applies_only_to_older_commits() {
        let old = c("2026-08-22T10:00:00Z", "fix: the work (air-old)\n");
        let new = c("2026-08-24T10:00:00Z", "fix: the work (air-new)\n");
        assert_eq!(
            ids_of(&[old], prose_ids, at(CUT)),
            ["air-old"],
            "history still reads"
        );
        assert!(
            ids_of(&[new], prose_ids, at(CUT)).is_empty(),
            "after the cutoff a commit without a trailer is not attributed at all"
        );
    }

    #[test]
    fn parse_log_survives_a_message_containing_newlines_and_colons() {
        let text = "2026-08-22T10:00:00Z\u{1f}feat: a thing\n\nNote: not a bead\nBead: air-9z\n\u{1e}\n\
                    2026-08-22T11:00:00Z\u{1f}chore: another\n\nBead: air-8y\n\u{1e}";
        let commits = parse_log(text);
        assert_eq!(commits.len(), 2);
        assert_eq!(ids_of(&commits, prose_ids, at(CUT)), ["air-9z", "air-8y"]);
    }
}
