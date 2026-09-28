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
//! The prose scan stayed for commits that predated the trailer, dated 2026-08-23 so it could be
//! deleted on evidence. It was deleted on 2026-09-28, when no branch in play predated that
//! date (air-jsee): a commit with no trailer is attributed to nothing.

use std::path::Path;

use crate::git;

/// One commit, reduced to what attribution needs.
pub struct Commit {
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

/// The beads `commits` are attributed to: their `Bead:` trailers. A commit with no trailer
/// contributes nothing, which is the forcing function — it is meant to be noticed, and
/// `air handover` names the missing trailer before the branch is ever landed.
pub fn ids_of(commits: &[Commit]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in commits {
        for id in trailer_ids(&c.message) {
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
                message: message.to_string(),
            })
        })
        .collect()
}

/// What a range attributes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Attributed {
    /// Declared by a `Bead:` trailer. Authoritative: a machine wrote it, so nothing further
    /// is inferred about it.
    pub declared: Vec<String>,
}

/// The beads the commits in `range` declare by trailer.
pub fn attributed_in_range(repo: &Path, range: &str) -> Attributed {
    let text = git::run(repo, &["log", "--format=%cI%x1f%B%x1e", range]).unwrap_or_default();
    Attributed {
        declared: ids_of(&parse_log(&text)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn c(message: &str) -> Commit {
        Commit {
            message: message.to_string(),
        }
    }

    #[test]
    fn a_trailer_wins_and_prose_in_the_same_commit_is_ignored() {
        let msg = "fix(land): select from the merge range\n\nBuilds on air-3pz and air-869, \
                   measured against air-7kp.\n\nBead: air-4re\n";
        // The body names three beads it did not do. Only the trailer counts.
        assert_eq!(trailer_ids(msg), ["air-4re"]);
        let got = ids_of(&[c(msg)]);
        assert_eq!(got, ["air-4re"], "a mention is not an attribution");
    }

    #[test]
    fn several_trailers_are_several_beads_and_repeats_collapse() {
        let msg = "chore: two at once\n\nBead: air-1a\nbead: air-2b\nBead: air-1a\n";
        assert_eq!(trailer_ids(msg), ["air-1a", "air-2b"]);
        // A value with a space is not an id; a list would be a parsing rule.
        assert!(trailer_ids("Bead: air-1a air-2b\n").is_empty());
    }

    /// air-jsee: with the prose fallback gone, a commit without a trailer is attributed to
    /// nothing, however old it is.
    #[test]
    fn a_commit_without_a_trailer_attributes_nothing_at_any_date() {
        let old = c("fix: the work (air-old)\n");
        let new = c("fix: the work (air-new)\n");
        assert!(ids_of(&[old, new]).is_empty());
    }

    #[test]
    fn parse_log_survives_a_message_containing_newlines_and_colons() {
        let text = "2026-08-22T10:00:00Z\u{1f}feat: a thing\n\nNote: not a bead\nBead: air-9z\n\u{1e}\n\
                    2026-08-22T11:00:00Z\u{1f}chore: another\n\nBead: air-8y\n\u{1e}";
        let commits = parse_log(text);
        assert_eq!(commits.len(), 2);
        assert_eq!(ids_of(&commits), ["air-9z", "air-8y"]);
    }
}
