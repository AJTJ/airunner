//! A session may ACT only on its own project. Talking to another one is fine (air-0lk,
//! air-3oq).
//!
//! Incident (owner, 2026-08-22, after the round): a coordinator sees far more than its own
//! project and nothing stopped it acting on the rest. `tmux ls` is machine-wide, so air-5lg
//! made the names legible but legible is not fenced: `tmux kill-session` reaches another
//! project's pane. In the same round worker beta proposed running `air triage` against
//! adopter's captures and the coordinator refused by hand, on prose (capture
//! 01M0N5783JTB354QQ5NAS0YPAA). CLAUDE.md's "never modify adopter's state" is exactly the
//! prose the repo's "Machinery over Markdown" rule says should become a check.
//!
//! ## Acting, not talking
//!
//! The owner's rule is that another project's worktrees, tmux sessions and workers are never
//! ours to kill, restart, re-model or tidy — and that **reading and messaging are fine**.
//! air-0lk implemented the fence tighter than the rule and denied `SendMessage` too. That was
//! removed by air-3oq, and the removal is permanent: if a message ever causes harm, that is an
//! incident to file, not a reason to re-tighten. The cost of the fence is paid in every
//! exchange that does not happen, and that cost cannot be observed.
//!
//! The failure it caused was silent and uninterpretable. A adopter coordinator launched with
//! `AIR_PROJECT` set would try to reach this fleet and get a denial it had no way to read as
//! "the fence, not you" — and the cross-project channel is the one that caught three wrong
//! claims on 2026-08-22, including `acceptance_criteria`, where both coordinators had it
//! backwards and only an implementing agent got it right. Had the fence existed that morning,
//! air-ayp would have shipped section-only and printed nothing for 647 of adopter's beads.
//!
//! So every refusal here names the fence, the project it is protecting, and what is still
//! allowed. A denial a peer cannot interpret is the defect, as much as the denial itself.
//!
//! Everything here is pure so `air selftest` fires every refusal without a tmux server or a
//! second repo. The project name is `cmd::tmux::project_prefix`, the resolver air-5lg already
//! added for session names; there is not a second one.
//!
//! Removal condition: when the ledger shows a full quarter with zero cross-project denials AND
//! the agent channel has its own project scoping, this check has nothing left to catch. The
//! messaging clause is not part of that: it is gone, not suspended.

/// A tmux session name this command names, from `-t <target>` and `-s <name>`. A target may
/// address a window or pane (`session:window.pane`), so only the part before `:` is the
/// session. Empty when the command names no session (`tmux ls`, `tmux list-sessions`).
pub fn tmux_sessions_named(cmd: &str) -> Vec<String> {
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        // `-t x` / `-s x`, and the glued forms `-tx` / `-sx`.
        let value = match *t {
            "-t" | "-s" => toks.get(i.saturating_add(1)).copied(),
            _ if t.len() > 2 && (t.starts_with("-t") || t.starts_with("-s")) => t.get(2..),
            _ => None,
        };
        let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
            continue;
        };
        if v.starts_with('-') {
            continue;
        }
        let session = v.split(':').next().unwrap_or(v).trim_matches(['"', '\'']);
        if !session.is_empty() {
            out.push(session.to_string());
        }
    }
    out
}

/// Does this session name belong to `project`? A session Air made is `<project>-<worker>`
/// (air-5lg); `<project>` alone is the project's own. Anything else is another project's, or
/// old enough that Air cannot tell — and both are refused, because the failure being prevented
/// is ACTING on a stranger's fleet.
pub fn in_project(name: &str, project: &str) -> bool {
    !project.is_empty() && (name == project || name.starts_with(&format!("{project}-")))
}

/// Refuse a `tmux` command that names another project's session. `None` when there is nothing
/// to refuse: not a tmux command, no session named, or every session named is ours.
pub fn tmux_refusal(cmd: &str, project: &str) -> Option<String> {
    if !is_tmux_command(cmd) {
        return None;
    }
    let foreign: Vec<String> = tmux_sessions_named(cmd)
        .into_iter()
        .filter(|s| !in_project(s, project))
        .collect();
    if foreign.is_empty() {
        return None;
    }
    Some(format!(
        "refused by Air's cross-project fence (air-0lk), protecting project `{project}`: tmux \
         session(s) {} are not `{project}`'s. `tmux ls` is machine-wide, so a command naming \
         another project's session would kill or re-model a fleet that is not yours. STILL \
         ALLOWED: reading anything, and MESSAGING any session on this machine, including \
         another project's — the fence is about acting, not talking (air-3oq). This project's \
         sessions are `{project}-<worker>`; `air status` lists them. If another project's \
         fleet needs something done, message it and ask.",
        foreign.join(", ")
    ))
}

/// Does a shell command run `tmux`? First token, or right after a shell separator, so
/// `echo tmux` and `grep -r tmux .` are not tmux commands.
pub fn is_tmux_command(cmd: &str) -> bool {
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    toks.iter().enumerate().any(|(i, t)| {
        *t == "tmux"
            && (i == 0
                || toks
                    .get(i.saturating_sub(1))
                    .is_some_and(|p| matches!(*p, "&&" | "||" | ";" | "|" | "(" | "{")))
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn a_tmux_command_naming_another_projects_session_is_refused() {
        // The incident's own example.
        let r = tmux_refusal("tmux kill-session -t fd-worker1", "air").unwrap();
        assert!(r.contains("fd-worker1") && r.contains("air-0lk"), "{r}");
        assert_eq!(tmux_refusal("tmux kill-session -t air-alpha", "air"), None);
        // Window and pane targets address a session.
        assert!(tmux_refusal("tmux send-keys -t fd-w1:0.1 hi", "air").is_some());
        assert_eq!(tmux_refusal("tmux send-keys -t air-w1:0.1 hi", "air"), None);
        // Nothing named, nothing to refuse.
        assert_eq!(tmux_refusal("tmux ls", "air"), None);
        assert_eq!(
            tmux_refusal("tmux list-sessions -F '#{session_name}'", "air"),
            None
        );
        // Creating one of ours is fine; creating a bare name is not ours.
        assert_eq!(tmux_refusal("tmux new-session -d -s air-w1", "air"), None);
        assert!(tmux_refusal("tmux new-session -d -s scratch", "air").is_some());
        // A tmux word that is not a tmux command.
        assert_eq!(tmux_refusal("grep -rn tmux crates -t fd-x", "air"), None);
        assert!(tmux_refusal("cd /x && tmux attach -t fd-x", "air").is_some());
    }

    /// air-3oq: a denial a peer cannot interpret is the defect. It has to say which fence,
    /// which project, and what is still open — otherwise a fenced coordinator reads it as
    /// "you are not allowed to talk to them" and stops trying.
    #[test]
    fn the_refusal_names_the_fence_the_project_and_what_is_still_allowed() {
        let r = tmux_refusal("tmux kill-session -t fd-worker1", "air").unwrap();
        assert!(r.contains("air-0lk"), "names the fence: {r}");
        assert!(r.contains("`air`"), "names the project it protects: {r}");
        assert!(
            r.contains("MESSAGING") && r.contains("air-3oq"),
            "says messaging is still allowed, and why: {r}"
        );
        assert!(r.contains("message it and ask"), "says what to do: {r}");
    }

    #[test]
    fn glued_flags_and_quotes_are_read_the_same_way() {
        assert_eq!(tmux_sessions_named("tmux attach -tfd-x"), vec!["fd-x"]);
        // Split on whitespace, so a quoted name with a space is read up to the space; the
        // quote is trimmed. A name Air made never has one (`session_name` maps them to `-`).
        assert_eq!(tmux_sessions_named("tmux new -s 'air-w 1'"), vec!["air-w"]);
        assert_eq!(tmux_sessions_named("tmux ls"), Vec::<String>::new());
        // A flag after -t is not a target.
        assert_eq!(
            tmux_sessions_named("tmux attach -t -d"),
            Vec::<String>::new()
        );
    }
}
