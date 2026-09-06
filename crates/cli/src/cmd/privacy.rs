//! `air adopter-check`: no tracked file names an adopter (air-bpj).
//!
//! Owner ruling, 2026-09-06: no adopter's content should be public, and the Air project should
//! be separate from theirs. Everything that names an adopter, quotes their files or copies
//! their corpus moved to an ignored `private/`; tracked text says "an adopter". The ruling's
//! own words are in the bead and in `private/README.md`, because they name the adopter and
//! this check would refuse the file quoting them — which is the check working.
//!
//! **Why this is a check and not a rule in CLAUDE.md.** A rule that must be remembered fails
//! toward publishing, and publishing is the direction that cannot be undone: one push and the
//! name is in somebody's clone. The sweep touched 228 files, and the next mention will arrive
//! one line at a time in a digest nobody re-reads.
//!
//! **The names are private, so the check reads them from the private file.** `private/`
//! is ignored, so a clone without it has no list, and the check SKIPS rather than failing —
//! that is the open-source contributor's case and it is deliberate. The alternative, a list of
//! names compiled into the binary, would publish exactly what it exists to hide.
//!
//! Removal: when no adopter is worked with intimately enough to be quoted.

use std::path::Path;

use serde::Serialize;

/// Where the names live. Ignored by git, so its absence is normal and not an error.
pub const ADOPTERS: &str = "private/adopters.md";

/// Pure: the adopter names declared in `private/adopters.md`.
///
/// A DECLARED field, `name: <x>` on its own line, not every word in the file
/// (`anti-brittleness`). The file also carries paths, a prefix and prose that mention the same
/// name, and a check that grepped its whole text would refuse the file it reads.
pub fn names(adopters_md: &str) -> Vec<String> {
    adopters_md
        .lines()
        .filter_map(|l| l.trim().strip_prefix("name:"))
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect()
}

/// One tracked file that names an adopter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Leak {
    pub path: String,
    /// 1-indexed line and its text, so the fix is one lookup away.
    pub line: usize,
    pub text: String,
    pub name: String,
}

/// Pure: every leak in `files`, given `(path, contents)` pairs. Case-insensitive, because the
/// sweep's own miss was an upper-case table row that a case-sensitive pass let through.
pub fn leaks(names: &[String], files: &[(String, String)]) -> Vec<Leak> {
    let mut out = Vec::new();
    for (path, text) in files {
        for (i, line) in text.lines().enumerate() {
            let low = line.to_ascii_lowercase();
            if let Some(name) = names.iter().find(|n| low.contains(n.as_str())) {
                out.push(Leak {
                    path: path.clone(),
                    line: i.saturating_add(1),
                    text: line.trim().chars().take(120).collect(),
                    name: name.clone(),
                });
            }
        }
    }
    out
}

/// The refusal, naming what to do. `None` when nothing leaked.
pub fn refusal(leaks: &[Leak]) -> Option<String> {
    if leaks.is_empty() {
        return None;
    }
    let mut s = format!(
        "adopter-check: {} tracked line(s) name an adopter (air-bpj). Tracked text says \"an \
         adopter\"; the incident keeps its date, its count and its air- bead, and anything that \
         quotes their files moves to private/.\n",
        leaks.len()
    );
    for l in leaks.iter().take(40) {
        s.push_str(&format!("  {}:{}: {}\n", l.path, l.line, l.text));
    }
    if leaks.len() > 40 {
        s.push_str(&format!(
            "  … and {} more\n",
            leaks.len().saturating_sub(40)
        ));
    }
    Some(s)
}

/// Read the tracked files git knows about, as `(path, contents)`. Binary and unreadable files
/// are skipped, never fatal.
fn tracked(repo: &Path) -> Result<Vec<(String, String)>, String> {
    let listing = crate::git::run(repo, &["ls-files"]).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for rel in listing.lines() {
        let p = repo.join(rel);
        if let Ok(text) = std::fs::read_to_string(&p) {
            out.push((rel.to_string(), text));
        }
    }
    Ok(out)
}

pub fn run(repo: &Path, json: bool) -> i32 {
    let list = repo.join(ADOPTERS);
    let Ok(md) = std::fs::read_to_string(&list) else {
        if !json {
            println!(
                "adopter-check: no {ADOPTERS}, so no names to check. Skipped, which is the \
                 case for a clone that works with no adopter."
            );
        }
        return 0;
    };
    let names = names(&md);
    if names.is_empty() {
        if !json {
            println!("adopter-check: {ADOPTERS} declares no `name:` line. Nothing to check.");
        }
        return 0;
    }
    let files = match tracked(repo) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("adopter-check: {e}");
            return 1;
        }
    };
    let found = leaks(&names, &files);
    if json {
        super::emit(true, &found, String::new);
    }
    match refusal(&found) {
        Some(msg) => {
            eprint!("{msg}");
            2
        }
        None => {
            if !json {
                println!(
                    "adopter-check: {} tracked file(s) checked against {} name(s); none names \
                     an adopter.",
                    files.len(),
                    names.len()
                );
            }
            0
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn names_are_declared_lines_and_the_match_is_case_insensitive() {
        let md = "# Adopters\n\n    name: acme\n    prefix: ac\n    checkout: ~/projects/acme\n";
        assert_eq!(names(md), vec!["acme".to_string()]);
        // The prose and the paths in the same file are not names, or the check would refuse
        // the file it reads.
        assert_eq!(
            names("acme is the adopter\ncheckout: /acme\n"),
            Vec::<String>::new()
        );

        let n = names(md);
        let files = vec![
            ("a.md".to_string(), "clean\nACME-KEEPS a row\n".to_string()),
            ("b.rs".to_string(), "// nothing here\n".to_string()),
        ];
        let found = leaks(&n, &files);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].path.as_str(), found[0].line), ("a.md", 2));
        assert!(refusal(&found).unwrap().contains("a.md:2"));
        assert!(refusal(&[]).is_none());
    }
}
