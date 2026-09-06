//! What a landing may close, and what it may only land (air-ayp).
//!
//! The adopter measured this failure in its own closer: 99 of 532 beads (18.6%) closed on branch
//! containment alone, never reading acceptance, and accelerating — 84 of the last 172 closes,
//! 48.8%, over two days. Verdicts on the 99: 82 done, 14 partial, 1 not done, 1 unverifiable,
//! 1 moot. The misses are not random. They are beads carrying one clause the merging agent
//! could not discharge itself: an owner decision, an owner-only config write, a deploy. One
//! closed a privacy bead holding an unresolved owner ruling, which made the ruling stop
//! existing; another left a locked-out user unable to reset their password, waiting on a
//! config write nobody knew was outstanding.
//!
//! Source: `the adopter's docs/plans/0029-bead-closure.md` §D.6, relayed by that
//! project's coordinator on 2026-08-22 and recorded in air-ayp. Not read from here: a session
//! may only touch its own project (air-0lk), so these numbers are cited, not verified.
//!
//! ## Where the criteria live, and how that was got wrong three times
//!
//! bd keeps acceptance in two places and OMITS the `acceptance_criteria` key entirely when it
//! is unset. So a key listing taken over beads that never set it reads as "bd has no such
//! field" — which is what this repo's beads show (0 of 33 carry it) and what two coordinators
//! and this module concluded, each having checked. The adopter's implementing agent surveyed all
//! 711 of its beads and inverted it: 647 field, 57 section, 0 both, 7 neither. The shape is a
//! property of how a repo files beads, and `air land` runs in every repo, so `clauses_of`
//! reads the UNION. A fixture with one shape proves nothing about the other.
//!
//! Their formulation, which is the lesson: a verification that does not have the shape of the
//! use is not a verification. Three parties, two real checks, still wrong, because all three
//! sampled where the use required a survey.
//!
//! Air shipped `air land` with the same shape on 2026-08-22 (air-3pz), before the `landings`
//! table had a writer, so the constraint is built in rather than retrofitted.
//!
//! ## The worker closes its own bead with proof
//!
//! Owner ruling, 2026-08-22: there is no hand-over-for-review. The worker that did the work
//! closes its own bead, and the close reason is PROOF — a command and its output, a
//! `file:line`, a passing test — not a description of what was built. So by the time a branch
//! lands there is nothing left to close.
//!
//! `air land` therefore closes nothing. Its whole acceptance behaviour is to PRINT every bead
//! in the merge range beside its criteria and Air's verdict on each clause, so a wrong close is
//! visible at the moment it lands. That print is the only external check on the honour system.
//! (This was built as a report layer plus a deletable closing layer; the ruling deleted the
//! closing layer, which is what the split was for.)
//!
//! The green-at-HEAD gate matters more under this model, not less, since an agent is proving
//! its own work: `is_handover_command` already matches `bd close` and `-s closed`, so the one
//! refusal covers close-with-proof exactly as it covered hand-over.
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
//! not a criticism of the bead. 29 of the adopter's 99 were of that kind, and the bead requires
//! that case be explicit — never silently closed, never blocked from landing.
//!
//! `Discharged` on every clause is what lets a reader trust a close at a glance. A clause Air
//! can actively REFUTE — a bead naming a file the merge did not touch — is the signal worth
//! carrying past scrollback, because that is a wrong close rather than an unreadable one.
//!
//! ## A path Air cannot resolve is unreadable, not refuted (air-dqa)
//!
//! `paths_named` reads path-like tokens out of prose, and a token it trims wrongly (the adopter
//! wrote `docs/reference/tooling.md`'s, and the possessive survived the trim) matched nothing in
//! the merge and was reported as CONTRADICTED. Three firings, zero true, in the mechanism whose
//! job is to be believed. So a named path that is NOT in the merge's file list is checked
//! against the tree at the landed commit before anything is asserted: a file that exists and
//! was not touched is refuted (the ai_runner case, where the work correctly landed in a
//! different file, and a person must read it); a token that is no file at all is reported as
//! a path Air cannot resolve, which is the unreadable direction. No further trimming rule was
//! added: a stricter trimmer is the same brittleness with a longer regex.
//!
//! **Decided, 2026-09-05: acceptance clauses are NOT required to declare paths in a fixed
//! form.** The digest declares its bead (air-agq) because that fact guards a gate and fails
//! toward permitting. This mechanism reports; its failure is a person reading a wrong line,
//! and with the tree check the wrong line can only be the true fact "exists, untouched". A
//! declared form would be a syntax every bead author in every adopting repo must know, for a
//! reporter. Revisit if a false refutation is recorded after this change.
//!
//! Removal condition: remove when acceptance criteria are machine-checkable by construction,
//! at which point the merge either satisfies them or does not and no judgement is involved.

use serde::Serialize;

/// Every acceptance clause a bead states, from BOTH places bd keeps them (air-ayp).
///
/// bd has a first-class `acceptance_criteria` field (`bd create/update --acceptance`) AND
/// repos that file with `-d` put a `## Acceptance Criteria` section in the description. Which
/// one a bead uses depends on how its repo files beads, and `air land` runs in every repo, so
/// this reads the union rather than either half. Measured: this repo is 33 section / 0 field;
/// The adopter is 647 field / 57 section / 0 both / 7 neither, across all 711 of its beads.
///
/// Both empty is a real case (the adopter's 7): no clauses, which never closes.
pub fn clauses_of(field: &str, description: &str) -> Vec<String> {
    let mut out = bullets(field);
    out.extend(section(description));
    out
}

/// Bullets in a block of text. A block with no bullets at all is one clause: `--acceptance
/// "criterion A"` is the single-criterion form, and dropping it would read as "states none".
fn bullets(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        push_line(line.trim(), &mut out);
    }
    if out.is_empty() && !text.trim().is_empty() {
        return vec![text.split_whitespace().collect::<Vec<_>>().join(" ")];
    }
    out
}

/// One line of a criteria block: a new bullet, or a continuation of the previous one.
fn push_line(t: &str, out: &mut Vec<String>) {
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

/// The bullets under a heading that is EXACTLY `Acceptance Criteria`. Anchored on the whole
/// heading, not a substring: the adopter has three descriptions with headings merely including
/// the word ("## Ownership, and the acceptance"), and any following heading ends the section
/// so a later one is never swallowed.
pub fn section(description: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    for line in description.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            let heading = t.trim_start_matches('#').trim();
            inside = heading.eq_ignore_ascii_case("acceptance criteria");
            continue;
        }
        if inside {
            push_line(t, &mut out);
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
    /// Air has nothing to look up. Not a defect in the bead. `how` says why: plain prose, or a
    /// path-like token that is no file at the landed commit (air-dqa).
    Undecidable { how: String },
}

/// The one reason most clauses are undecidable: they are prose.
const PROSE: &str = "nothing Air can look up";

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
    /// Every path in the tree at the landed commit (air-dqa). A named path in neither list is
    /// a token Air cannot resolve, not a file the merge failed to touch.
    pub tree: &'a [String],
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
        return Verdict::Undecidable { how: PROSE.into() };
    }
    let missing: Vec<&String> = paths
        .iter()
        .filter(|p| !ev.changed.iter().any(|c| c == *p))
        .collect();
    if missing.is_empty() {
        return Verdict::Discharged {
            how: format!("the merge changed {}", paths.join(", ")),
        };
    }
    // air-dqa: before asserting the merge did not change a file, make sure it IS a file.
    // A token the trimmer got wrong is no file at the landed commit, and saying "did not
    // change" about it is a confident false accusation. An existing file the merge left alone
    // is the true fact, and the one Air must keep saying.
    let (untouched, unresolved): (Vec<&String>, Vec<&String>) = missing
        .into_iter()
        .partition(|p| ev.tree.iter().any(|t| t == *p));
    let list = |v: &[&String]| v.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ");
    if !untouched.is_empty() {
        Verdict::Unevidenced {
            how: format!("the merge did not change {}", list(&untouched)),
        }
    } else {
        Verdict::Undecidable {
            how: format!(
                "names {}, which is not a path in the tree at the landed commit, so Air \
                 cannot resolve it",
                list(&unresolved)
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
    /// Every clause looked up and held. What lets a reader trust a close at a glance. No
    /// clauses at all is not a pass: it means Air read nothing (the adopter had 7 such beads).
    pub fn all_discharged(&self) -> bool {
        !self.clauses.is_empty() && self.clauses.iter().all(|(_, v)| v.discharged())
    }

    /// At least one clause Air could actively REFUTE. Not "could not read" — refuted. This is
    /// the wrong-close signal, and the only one carried past the print (air-ayp).
    pub fn refuted(&self) -> bool {
        self.clauses
            .iter()
            .any(|(_, v)| matches!(v, Verdict::Unevidenced { .. }))
    }

    /// One line naming every clause Air could not discharge, refuted and unreadable alike, for
    /// the landings row. Complete, so the record is; NOT what the condition says (air-ppf).
    pub fn why_open(&self) -> String {
        if self.clauses.is_empty() {
            return "the bead states no acceptance criteria, in the field or the description, \
                    so Air read nothing to check"
                .to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        for (text, v) in &self.clauses {
            let short = text.chars().take(70).collect::<String>();
            match v {
                Verdict::Discharged { .. } => {}
                Verdict::Unevidenced { how } | Verdict::Undecidable { how } => {
                    parts.push(format!("\"{short}\": {how}"));
                }
            }
        }
        parts.join("; ")
    }

    /// One line naming only the clauses the merge CONTRADICTS, for the sentence that says so
    /// (air-ppf). `why_open` under a CONTRADICTS headline presented every unreadable clause as
    /// a contradiction; on 2026-08-30 that read as two wrong closes that were sound.
    pub fn why_contradicted(&self) -> String {
        self.clauses
            .iter()
            .filter_map(|(text, v)| match v {
                Verdict::Unevidenced { how } => {
                    let short = text.chars().take(70).collect::<String>();
                    Some(format!("\"{short}\": {how}"))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// **Layer 1.** Every bead the merge carries, printed beside its acceptance and Air's verdict
/// on each clause, so a wrong close is visible at the moment it lands. True under both closure
/// models: if this repo adopts close-with-proof, the closing layer goes and this stays
/// (the adopter `docs/plans/0029-bead-closure.md`, where the print is the only external check on
/// the honour system). Pure.
pub fn report(judged: &[Judged]) -> String {
    if judged.is_empty() {
        return String::new();
    }
    let mut s = format!(
        "\nacceptance for {} bead(s) in this merge (air land closes nothing; the worker closes \
         its own bead with proof, so this is the check on that):\n",
        judged.len()
    );
    for j in judged {
        s.push_str(&format!(
            "\n  {} — {}\n",
            j.bead,
            if j.refuted() {
                "REFUTED: a clause is contradicted by what this merge contains"
            } else if j.all_discharged() {
                "every clause discharged"
            } else {
                "not fully checkable by Air; read it"
            }
        ));
        if j.clauses.is_empty() {
            s.push_str(
                "    (no acceptance criteria in the field or the description; Air read \
                 nothing)\n",
            );
        }
        for (text, v) in &j.clauses {
            let (mark, how) = match v {
                Verdict::Discharged { how } => ("ok  ", how.as_str()),
                Verdict::Unevidenced { how } => ("MISS", how.as_str()),
                Verdict::Undecidable { how } => ("?   ", how.as_str()),
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
        judge_clauses(bead, clauses_of("", description), ev)
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
        let c = clauses_of("", BEAD);
        assert_eq!(c.len(), 3, "{c:?}");
        assert!(c[0].ends_with("and allows with one."), "{:?}", c[0]);
        assert_eq!(c[1], "docs/rules/roles.md names the rule.");
        assert!(!c.iter().any(|x| x.contains("not a criterion")));
        // A bead with no section reads as no clauses, which never closes.
        assert!(clauses_of("", "## Incident\n\n- a thing\n").is_empty());
    }

    #[test]
    fn a_path_the_merge_touched_discharges_and_one_it_did_not_is_unevidenced() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let tree = vec![
            "docs/rules/roles.md".to_string(),
            "docs/rules/writing.md".to_string(),
        ];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
            tree: &tree,
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
            tree: &none,
        };
        let red = Evidence {
            green_at_landed: false,
            changed: &none,
            tree: &none,
        };
        assert!(judge("Verify recorded green at HEAD.", &green).discharged());
        assert!(matches!(
            judge("Verify recorded green at HEAD.", &red),
            Verdict::Unevidenced { .. }
        ));
        // Air is not the judge of prose: it says so rather than guessing.
        assert!(matches!(
            judge("The owner is told what changed.", &green),
            Verdict::Undecidable { .. }
        ));
        // "verify" without a request for evidence is not a ledger question.
        assert!(matches!(
            judge("Verify the merged result reads well.", &green),
            Verdict::Undecidable { .. }
        ));
    }

    /// The bead's own rule: close on evidence, land-but-hold on anything else.
    #[test]
    fn a_bead_closes_only_when_every_clause_is_discharged() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
            tree: &changed,
        };
        // The probe clause is prose to Air, so this one lands and stays open.
        let j = judge_desc("air-1", BEAD, &ev);
        assert!(!j.all_discharged());
        assert!(
            j.why_open().contains("nothing Air can look up"),
            "{}",
            j.why_open()
        );

        let all_evidenced = "## Acceptance Criteria\n\n- docs/rules/roles.md names the rule.\n- Verify recorded green at HEAD.\n";
        assert!(judge_desc("air-2", all_evidenced, &ev).all_discharged());

        // No section at all: Air read nothing, so it does not close.
        let j = judge_desc("air-3", "## Incident\n\nnothing\n", &ev);
        assert!(!j.all_discharged());
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
            tree: &changed,
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
        assert!(r.contains("air land closes nothing"), "{r}");
        assert!(r.contains("air-1 — not fully checkable by Air"), "{r}");
        assert!(r.contains("air-2 — every clause discharged"), "{r}");
        assert!(
            r.contains("no acceptance criteria in the field or the"),
            "{r}"
        );
        // Every clause is shown, discharged or not, so a wrong close is visible.
        assert!(
            r.contains("ok   docs/rules/roles.md names the rule."),
            "{r}"
        );
        assert!(r.contains("?    Red/green probe"), "{r}");
        assert_eq!(report(&[]), "");
    }

    /// air-ayp: the two storage shapes are BOTH real and which one a bead uses depends on how
    /// its repo files beads — 33 of 33 section-only here, 647 field / 57 section / 0 both /
    /// 7 neither in the adopter. `air land` runs in both, so a fixture with one shape proves
    /// nothing. Every shape, including neither.
    #[test]
    fn both_storage_shapes_are_read_and_neither_is_reported() {
        // Shape 1: bd's own field, set by `bd create --acceptance`. Single criterion, no
        // bullets — dropping it would read as "states none", which is the dangerous direction.
        assert_eq!(clauses_of("criterion A", ""), vec!["criterion A"]);
        // The field can hold bullets too.
        assert_eq!(
            clauses_of("- one\n- two\n", ""),
            vec!["one".to_string(), "two".to_string()]
        );
        // Shape 2: the section, for a repo that files with -d.
        assert_eq!(
            clauses_of("", "## Acceptance Criteria\n\n- from the section\n"),
            vec!["from the section"]
        );
        // Both, which the adopter has none of today but nothing forbids: the union, not either.
        assert_eq!(
            clauses_of("field one", "## Acceptance Criteria\n\n- section one\n"),
            vec!["field one".to_string(), "section one".to_string()]
        );
        // Neither: the adopter measured 7. No clauses, so it is reported, never assumed met.
        let j = judge_clauses(
            "zz-7",
            clauses_of("", "## Incident\n\nno criteria anywhere\n"),
            &Evidence {
                green_at_landed: true,
                changed: &[],
                tree: &[],
            },
        );
        assert!(!j.all_discharged() && !j.refuted());
        assert!(
            j.why_open().contains("no acceptance criteria"),
            "{}",
            j.why_open()
        );
    }

    /// The heading is exactly `Acceptance Criteria`. The adopter has three descriptions with
    /// headings merely including the word, and a following heading must not be swallowed.
    #[test]
    fn only_the_exact_heading_opens_the_section() {
        let d = "## Ownership, and the acceptance\n\n\
                 - not a criterion\n\n\
                 ## Acceptance Criteria\n\n\
                 - a real one\n\n\
                 ### A sub-heading\n\n\
                 - also not\n";
        assert_eq!(section(d), vec!["a real one"]);
    }

    /// A refuted clause is a wrong close; an unreadable one is not. Only the first is carried
    /// past the print.
    #[test]
    fn refuted_and_unreadable_are_different_signals() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
            tree: &changed,
        };
        let tree = vec![
            "docs/rules/roles.md".to_string(),
            "docs/absent.md".to_string(),
        ];
        let ev = Evidence { tree: &tree, ..ev };
        let refuted = judge_clauses("a", vec!["docs/absent.md says it.".into()], &ev);
        let unreadable = judge_clauses("b", vec!["The owner rules on it.".into()], &ev);
        assert!(refuted.refuted() && !refuted.all_discharged());
        assert!(!unreadable.refuted() && !unreadable.all_discharged());
    }

    /// air-ppf: the CONTRADICTS sentence names the refuted clause alone; the row's `why` keeps
    /// the unreadable ones beside it.
    #[test]
    fn the_contradicted_line_omits_what_air_merely_could_not_read() {
        let changed = vec!["docs/rules/roles.md".to_string()];
        let tree = vec![
            "docs/rules/roles.md".to_string(),
            "docs/absent.md".to_string(),
        ];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
            tree: &tree,
        };
        let j = judge_clauses(
            "a",
            vec![
                "docs/absent.md says it.".into(),
                "The owner rules on it.".into(),
                "docs/rules/roles.md names the rule.".into(),
            ],
            &ev,
        );
        assert_eq!(
            j.why_contradicted(),
            "\"docs/absent.md says it.\": the merge did not change docs/absent.md"
        );
        assert!(j.why_open().contains("nothing Air can look up"));
        assert!(j.why_open().contains("docs/absent.md"));
        let unreadable = judge_clauses("b", vec!["The owner rules on it.".into()], &ev);
        assert_eq!(unreadable.why_contradicted(), "");
    }

    /// air-dqa, the two cases the bead names. The possessive: the adopter wrote
    /// `docs/reference/tooling.md`'s, the trim stopped at the `s`, the token matched nothing
    /// in a merge that HAD changed that file, and Air reported a contradiction. Now: no file by
    /// that name at the landed commit, so unreadable, with the token named. The ai_runner case:
    /// a clause naming a file the work correctly did not touch (the pin landed in install.rs
    /// rather than install_and_launch.rs). The file exists and the merge left it alone, which is
    /// the true fact, and stays refuted for a person to read.
    #[test]
    fn a_token_that_is_no_file_is_unreadable_and_an_untouched_file_is_refuted() {
        let changed = vec!["docs/reference/tooling.md".to_string()];
        let tree = vec![
            "docs/reference/tooling.md".to_string(),
            "crates/cli/tests/install_and_launch.rs".to_string(),
            "crates/cli/src/cmd/install.rs".to_string(),
        ];
        let ev = Evidence {
            green_at_landed: true,
            changed: &changed,
            tree: &tree,
        };
        // The possessive, verbatim from.
        let v = judge(
            "Air's own `docs/reference/tooling.md`'s section is updated.",
            &ev,
        );
        assert!(
            matches!(&v, Verdict::Undecidable { how } if how.contains("cannot resolve")
                && how.contains("docs/reference/tooling.md`'s")),
            "{v:?}"
        );
        // The same clause written plainly discharges, so the tree check costs nothing real.
        assert!(judge("docs/reference/tooling.md is updated.", &ev).discharged());
        // The true positive Air must keep: an existing file the merge did not touch.
        let v = judge(
            "Pin it in crates/cli/tests/install_and_launch.rs.",
            &Evidence {
                changed: &["crates/cli/src/cmd/install.rs".to_string()],
                ..ev
            },
        );
        assert!(
            matches!(&v, Verdict::Unevidenced { how }
                if how == "the merge did not change crates/cli/tests/install_and_launch.rs"),
            "{v:?}"
        );
        // A file the merge deleted is in `changed` and not in `tree`: still discharged.
        assert!(
            judge(
                "docs/old.md is removed.",
                &Evidence {
                    changed: &["docs/old.md".to_string()],
                    ..ev
                }
            )
            .discharged()
        );
        // Mixed: one real untouched file beside one unresolvable token is still refuted, and
        // names only the real one.
        let v = judge(
            "crates/cli/tests/install_and_launch.rs and docs/nope.md`'s are touched.",
            &ev,
        );
        assert!(
            matches!(&v, Verdict::Unevidenced { how }
                if how == "the merge did not change crates/cli/tests/install_and_launch.rs"),
            "{v:?}"
        );
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
