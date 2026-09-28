//! A bead's acceptance clauses, read for `air land` to print beside each landed bead (air-ayp).
//!
//! `air land` closes nothing: the worker closes its own bead with proof (owner ruling,
//! 2026-08-22). It prints every landed bead with its acceptance text so a wrong close is
//! visible where it lands, and draws no verdict from it.
//!
//! It used to judge each clause by lookup (a green at the landed sha, a named path in the
//! merge) and raise `landed-not-closed` for a path the merge did not change. Deleted
//! 2026-09-28 (air-cizz, owner): at an adopter 7 of 8 firings were paths the text only
//! mentioned, in this repo 6 of 9 held (air-k6uh), it never cleared for a bead closed outside
//! `air close`, and it misparsed a possessive path. Close-with-proof already covers the
//! failure it was built for (the adopter's 99 beads closed on branch containment alone).
//!
//! ## Where the criteria live
//!
//! bd keeps acceptance in two places and OMITS the `acceptance_criteria` key entirely when it
//! is unset, so a key listing over beads that never set it reads as "bd has no such field".
//! The adopter surveyed all 711 of its beads: 647 field, 57 section, 0 both, 7 neither. The
//! shape is a property of how a repo files beads, and `air land` runs in every repo, so
//! `clauses_of` reads the UNION.

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

/// Every landed bead beside its acceptance text, with no verdict (air-cizz). Pure.
pub fn listing(beads: &[(String, Vec<String>)]) -> String {
    if beads.is_empty() {
        return String::new();
    }
    let mut s = format!(
        "\nacceptance for {} bead(s) in this merge (air land closes no worker's bead; the \
         worker closes its own with proof):\n",
        beads.len()
    );
    for (bead, clauses) in beads {
        s.push_str(&format!("\n  {bead}\n"));
        if clauses.is_empty() {
            s.push_str("    (no acceptance criteria in the field or the description)\n");
        }
        for c in clauses {
            s.push_str(&format!("    - {c}\n"));
        }
    }
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

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
        // Neither: the adopter measured 7. No clauses, and the listing says so.
        let none = clauses_of("", "## Incident\n\nno criteria anywhere\n");
        assert!(none.is_empty());
        assert!(listing(&[("zz-7".to_string(), none)]).contains("no acceptance criteria"));
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

    /// air-cizz: the listing names every bead and every clause, and judges none.
    #[test]
    fn the_listing_names_every_bead_and_clause_and_no_verdict() {
        let out = listing(&[
            ("zz-1".to_string(), clauses_of("", BEAD)),
            ("zz-2".to_string(), vec![]),
        ]);
        assert!(
            out.contains("  zz-1\n") && out.contains("  zz-2\n"),
            "{out}"
        );
        assert!(
            out.contains("    - docs/rules/roles.md names the rule."),
            "{out}"
        );
        for word in ["ok (", "MISS", "UNCONFIRMED", "discharged"] {
            assert!(!out.contains(word), "{word}: {out}");
        }
    }
}
