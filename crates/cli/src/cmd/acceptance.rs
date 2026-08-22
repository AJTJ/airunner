//! What a landing may close, and what it may only land (air-ayp).
//!
//! adopter measured this failure in its own closer: 99 of 532 beads (18.6%) closed on branch
//! containment alone, never reading acceptance, and accelerating — 84 of the last 172 closes,
//! 48.8%, over two days. Verdicts on the 99: 82 done, 14 partial, 1 not done, 1 unverifiable,
//! 1 moot. The misses are not random. They are beads carrying one clause the merging agent
//! could not discharge itself: an owner decision, an owner-only config write, a deploy. One
//! closed a privacy bead holding an unresolved owner ruling, which made the ruling stop
//! existing; another left a locked-out user unable to reset their password, waiting on a
//! config write nobody knew was outstanding.
//!
//! Source: `~/projects/adopter/docs/plans/0029-bead-closure.md` §D.6, relayed by that
//! project's coordinator on 2026-08-22 and recorded in air-ayp. Not read from here: a session
//! may only touch its own project (air-0lk), so these numbers are cited, not verified.
//!
//! Air shipped `air land` with the same shape on 2026-08-22 (air-3pz), before the `landings`
//! table had a writer, so the constraint is built in rather than retrofitted.
//!
//! ## Two layers, and which one is load-bearing
//!
//! adopter's owner ruled that the author closes its own bead with proof, so by the time a
//! branch lands there is nothing left to close, and the merge's job is to PRINT each bead
//! beside its acceptance so a wrong close is visible at the moment it lands. This repo's owner
//! has not ruled. So `air land` is built in two layers:
//!
//! 1. **The report**, which is true under both models: every bead in the range is printed with
//!    its acceptance clauses and Air's verdict on each. Nothing closes on branch containment.
//! 2. **The close**, layered on top: beads whose every clause is discharged are closed.
//!
//! If this repo adopts close-with-proof, layer 2 is deleted and layer 1 is untouched. That is
//! the point of the split (coordinator, 2026-08-22).
//!
//! **Air is not the judge of prose.** It discharges exactly two clause shapes, and both are
//! lookups rather than readings:
//!
//! 1. A clause asking for a recorded green → a `verify_runs` row at the landed sha. A ledger
//!    lookup.
//! 2. A clause naming repository paths → every path it names is in the merge's changed files.
//!    A git lookup.
//!
//! Everything else is `Undecidable`: Air has nothing to look up, which is a fact about Air and
//! not a criticism of the bead. 29 of adopter's 99 were of that kind, and the bead requires
//! that case be explicit — never silently closed, never blocked from landing.
//!
//! A bead closes only when it has at least one clause and every clause is discharged. Anything
//! else lands merged-but-not-closed, and a person closes it with `air close` when they have
//! looked. That is deliberately conservative: the cost of not closing is one command, and the
//! cost of closing wrongly is a decision that stops existing.
//!
//! Removal condition: remove when acceptance criteria are machine-checkable by construction,
//! at which point the merge either satisfies them or does not and no judgement is involved.

use serde::Serialize;

/// The bullets under `## Acceptance Criteria` in a bead's description. bd 1.2.2 has no
/// `acceptance_criteria` field; the section is what `bd create --validate` requires (roles.md,
/// bd `internal/types/types.go` `RequiredSections`). A bullet may wrap over several lines.
pub fn clauses(description: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    for line in description.lines() {
        let t = line.trim();
        if let Some(h) = t.strip_prefix("## ") {
            // Any following heading ends the section.
            inside = h.trim().eq_ignore_ascii_case("acceptance criteria");
            continue;
        }
        if !inside {
            continue;
        }
        match t.strip_prefix("- ").or_else(|| t.strip_prefix("• ")) {
            Some(rest) => out.push(rest.trim().to_string()),
            // A continuation of the previous bullet; a blank line is just spacing.
            None if !t.is_empty() => {
                if let Some(last) = out.last_mut() {
                    last.push(' ');
                    last.push_str(t);
                }
            }
            _ => {}
        }
    }
    out
}

/// What the merge lets Air say about one clause. Ordered by how much Air knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum Verdict {
    /// Air looked it up and it holds. `how` names the lookup, so the close is checkable.
    Discharged { how: String },
    /// Air looked it up and it does not hold. The strongest signal: the bead names a file the
    /// merge did not touch.
    Unevidenced { how: String },
    /// Air has nothing to look up. Not a defect in the bead.
    Undecidable,
}

impl Verdict {
    pub fn discharged(&self) -> bool {
        matches!(self, Verdict::Discharged { .. })
    }
}

/// What Air can point at for one landing. Both fields are facts it already has: the verify run
/// it recorded at the merge commit, and the merge's own file list.
#[derive(Debug, Clone, Copy)]
pub struct Evidence<'a> {
    /// A green `verify_runs` row exists at the landed sha.
    pub green_at_landed: bool,
    /// Repo-relative paths the merge changed.
    pub changed: &'a [String],
}

/// Does this token look like a repository path? Deliberately narrow: a slash and a dot, no
/// spaces. `docs/rules/roles.md` yes; `bd` and `awaiting_review` no.
fn path_like(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| {
        !c.is_ascii_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
    });
    t.contains('/') && t.rsplit('/').next().is_some_and(|f| f.contains('.')) && !t.ends_with('/')
}

/// The path-like tokens in a clause, in order, deduplicated.
pub fn paths_named(clause: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in clause.split_whitespace() {
        // `path.rs:277` and `path.rs:12-30` are how this repo cites code; the path is the
        // part before the colon.
        let t = raw
            .split(':')
            .next()
            .unwrap_or(raw)
            .trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-'
            })
            .trim_end_matches('.');
        if path_like(t) && !out.iter().any(|p| p == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// Does this clause ask for a recorded green? Both words, so "verify the merged result reads
/// well" is not read as a request for a ledger row.
fn asks_for_green(clause: &str) -> bool {
    let c = clause.to_ascii_lowercase();
    (c.contains("verify") || c.contains("make verify"))
        && (c.contains("green") || c.contains("record"))
}

/// Judge one clause against what Air can look up. Pure.
pub fn judge(clause: &str, ev: &Evidence<'_>) -> Verdict {
    if asks_for_green(clause) {
        return if ev.green_at_landed {
            Verdict::Discharged {
                how: "a green verify is recorded at the landed sha".into(),
            }
        } else {
            Verdict::Unevidenced {
                how: "no green verify is recorded at the landed sha".into(),
            }
        };
    }
    let paths = paths_named(clause);
    if paths.is_empty() {
        return Verdict::Undecidable;
    }
    let missing: Vec<&String> = paths
        .iter()
        .filter(|p| !ev.changed.iter().any(|c| c == *p))
        .collect();
    if missing.is_empty() {
        Verdict::Discharged {
            how: format!("the merge changed {}", paths.join(", ")),
        }
    } else {
        Verdict::Unevidenced {
            how: format!(
                "the merge did not change {}",
                missing
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

/// One bead's acceptance, judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Judged {
    pub bead: String,
    pub clauses: Vec<(String, Verdict)>,
}

impl Judged {
    /// A bead closes only when it states acceptance and every clause is discharged. No
    /// clauses at all is not a pass: it means Air read nothing.
    pub fn may_close(&self) -> bool {
        !self.clauses.is_empty() && self.clauses.iter().all(|(_, v)| v.discharged())
    }

    /// One line naming what stopped the close, for the landings row and the condition.
    pub fn why_open(&self) -> String {
        if self.clauses.is_empty() {
            return "the bead states no acceptance criteria, so Air read nothing to check"
                .to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        for (text, v) in &self.clauses {
            let short = text.chars().take(70).collect::<String>();
            match v {
                Verdict::Discharged { .. } => {}
                Verdict::Unevidenced { how } => parts.push(format!("\"{short}\": {how}")),
                Verdict::Undecidable => {
                    parts.push(format!("\"{short}\": nothing Air can look up"));
                }
            }
        }
        parts.join("; ")
    }
}

/// **Layer 1.** Every bead the merge carries, printed beside its acceptance and Air's verdict
/// on each clause, so a wrong close is visible at the moment it lands. True under both closure
/// models: if this repo adopts close-with-proof, the closing layer goes and this stays
/// (adopter `docs/plans/0029-bead-closure.md`, where the print is the only external check on
/// the honour system). Pure.
pub fn report(judged: &[Judged]) -> String {
    if judged.is_empty() {
        return String::new();
    }
    let mut s = format!("\nacceptance for {} bead(s) in this merge:\n", judged.len());
    for j in judged {
        s.push_str(&format!(
            "\n  {} — {}\n",
            j.bead,
            if j.may_close() {
                "every clause discharged"
            } else {
                "NOT closing"
            }
        ));
        if j.clauses.is_empty() {
            s.push_str("    (no acceptance criteria stated; Air read nothing)\n");
        }
        for (text, v) in &j.clauses {
            let (mark, how) = match v {
                Verdict::Discharged { how } => ("ok  ", how.as_str()),
                Verdict::Unevidenced { how } => ("MISS", how.as_str()),
                Verdict::Undecidable => {
                    ("?   ", "nothing Air can look up; a person must read this")
                }
            };
            s.push_str(&format!("    {mark} {text}\n         {how}\n"));
        }
    }
    s
}

/// Judge already-parsed clauses. `air land` takes this path: the clauses came from the same
/// `bd list --json` that produced the landing list, so nothing re-reads bd.
pub fn judge_clauses(bead: &str, clauses: Vec<String>, ev: &Evidence<'_>) -> Judged {
    Judged {
        bead: bead.to_string(),
        clauses: clauses
            .into_iter()
            .map(|c| {
                let v = judge(&c, ev);
                (c, v)
            })
            .collect(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Test convenience: parse then judge, the way `air land` does across two call sites.
    fn judge_desc(bead: &str, description: &str, ev: &Evidence<'_>) -> Judged {
        judge_clauses(bead, clauses(description), ev)
    }

    const BEAD: &str = "\
## Incident

Something happened. See docs/rules/roles.md for the rule.

## Acceptance Criteria

- Red/green probe in air selftest: the gate denies without a green
  and allows with one.
- docs/rules/roles.md names the rule.
- Verify recorded green at HEAD.

## Notes

- not a criterion
";

    #[test]
    fn clauses_come_from_the_section_only_and_wrapped_bullets_join() {
        let c = clauses(BEAD);
        assert_eq!(c.len(), 3, "{c:?}");
        assert!(c[0].ends_with("and allows with one."), "{:?}", c[0]);
        assert_eq!(c[1], "docs/rules/roles.md names the rule.");
        assert!(!c.iter().any(|x| x.contains("not a criterion")));
        // A bead with no section reads as no clauses, which never closes.
        assert!(clauses("## Incident\n\n- a thing\n").is_empty());
    }

    #[test]
    fn a_path_the_merge_touched_discharges_and_one_it_did_not_is_unevidenced() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
        };
        assert!(judge("docs/rules/roles.md names the rule.", &ev).discharged());
        let v = judge("docs/rules/writing.md names the rule.", &ev);
        assert!(
            matches!(&v, Verdict::Unevidenced { how } if how.contains("docs/rules/writing.md")),
            "{v:?}"
        );
    }

    #[test]
    fn a_green_clause_reads_the_ledger_and_prose_is_undecidable() {
        let none: Vec<String> = vec![];
        let green = Evidence {
            green_at_landed: true,
            changed: &none,
        };
        let red = Evidence {
            green_at_landed: false,
            changed: &none,
        };
        assert!(judge("Verify recorded green at HEAD.", &green).discharged());
        assert!(matches!(
            judge("Verify recorded green at HEAD.", &red),
            Verdict::Unevidenced { .. }
        ));
        // Air is not the judge of prose: it says so rather than guessing.
        assert_eq!(
            judge("The owner is told what changed.", &green),
            Verdict::Undecidable
        );
        // "verify" without a request for evidence is not a ledger question.
        assert_eq!(
            judge("Verify the merged result reads well.", &green),
            Verdict::Undecidable
        );
    }

    /// The bead's own rule: close on evidence, land-but-hold on anything else.
    #[test]
    fn a_bead_closes_only_when_every_clause_is_discharged() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
        };
        // The probe clause is prose to Air, so this one lands and stays open.
        let j = judge_desc("air-1", BEAD, &ev);
        assert!(!j.may_close());
        assert!(
            j.why_open().contains("nothing Air can look up"),
            "{}",
            j.why_open()
        );

        let all_evidenced = "## Acceptance Criteria\n\n- docs/rules/roles.md names the rule.\n- Verify recorded green at HEAD.\n";
        assert!(judge_desc("air-2", all_evidenced, &ev).may_close());

        // No section at all: Air read nothing, so it does not close.
        let j = judge_desc("air-3", "## Incident\n\nnothing\n", &ev);
        assert!(!j.may_close());
        assert!(j.why_open().contains("states no acceptance criteria"));
    }

    /// Layer 1 is the part that survives if this repo adopts close-with-proof: every bead is
    /// printed beside its acceptance whether it closes or not.
    #[test]
    fn the_report_names_every_bead_and_every_clauses_verdict() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
        };
        let r = report(&[
            judge_desc("air-1", BEAD, &ev),
            judge_desc(
                "air-2",
                "## Acceptance Criteria\n\n- docs/rules/roles.md names the rule.\n",
                &ev,
            ),
            judge_desc("air-3", "## Incident\n\nnothing\n", &ev),
        ]);
        assert!(r.contains("air-1 — NOT closing"), "{r}");
        assert!(r.contains("air-2 — every clause discharged"), "{r}");
        assert!(r.contains("no acceptance criteria stated"), "{r}");
        // Every clause is shown, discharged or not, so a wrong close is visible.
        assert!(
            r.contains("ok   docs/rules/roles.md names the rule."),
            "{r}"
        );
        assert!(r.contains("?    Red/green probe"), "{r}");
        assert_eq!(report(&[]), "");
    }

    #[test]
    fn path_detection_is_narrow() {
        assert_eq!(
            paths_named("`crates/cli/src/cmd/land.rs:277` and bd list --json"),
            vec!["crates/cli/src/cmd/land.rs".to_string()]
        );
        assert!(paths_named("air land --all closes in one bd process").is_empty());
        assert!(paths_named("status <> 'closed' AND status <> 'pinned'").is_empty());
    }
}
