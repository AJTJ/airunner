//! A session may only touch its own project (air-0lk).
//!
//! Incident (owner, 2026-08-22, after the round): a coordinator sees far more than its own
//! project and nothing stopped it acting on the rest. `tmux ls` is machine-wide, so air-5lg
//! made the names legible but legible is not fenced: `tmux kill-session` reaches another
//! project's pane. `ListAgents` in this repo's coordinator listed `adopter-51`, a live agent
//! of `~/projects/adopter`, as a messageable peer next to `alpha-6d` and `beta-72`; a message
//! to it is an instruction to another project's fleet. In the same round worker beta proposed
//! running `air triage` against adopter's captures and the coordinator refused by hand, on
//! prose (capture 01M0N5783JTB354QQ5NAS0YPAA). CLAUDE.md's "never modify adopter's state" is
//! exactly the prose the repo's "Machinery over Markdown" rule says should become a check.
//!
//! Everything here is pure so `air selftest` fires every refusal without a tmux server, a peer,
//! or a second repo. The project name is `cmd::tmux::project_prefix`, the resolver air-5lg
//! already added for session names; there is not a second one.
//!
//! Removal condition: when the ledger shows a full quarter with zero cross-project denials AND
//! the agent channel has its own project scoping, this check has nothing left to catch.

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

/// Does this name belong to `project`? A session Air made is `<project>-<worker>`
/// (air-5lg); `<project>` alone is the project's own. Anything else is another project's, or
/// old enough that Air cannot tell — and both are refused, because the failure being prevented
/// is reaching a stranger.
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
        "refused: tmux session(s) {} are not this project's ({project}). `tmux ls` is \
         machine-wide; a session may only touch its own project (air-0lk). This project's \
         sessions are named `{project}-<worker>` and `air status` lists them. If another \
         project's fleet needs something, ask its owner.",
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

/// The parent conversation, which is never another project's.
const OWN_CONVERSATION: &str = "main";

/// Refuse a `SendMessage` to a peer that is not one of this project's. `known` are the bases a
/// peer name in this project can carry: every worker in this ledger's sessions plus the
/// project and the main checkout's directory name. A peer address is `<base>` or
/// `<base>-<suffix>` (`ListAgents` shows `alpha-6d`, `ai-runner-0e`, `adopter-51`).
///
/// Closed by default, per the owner's rule: a name that matches nothing here is refused. That
/// includes an in-process subagent this session spawned itself, which Air cannot see; the
/// refusal names the escape (`ListAgents`) rather than pretending the name is foreign.
pub fn peer_refusal(to: &str, project: &str, known: &[String]) -> Option<String> {
    let to = to.trim();
    if to.is_empty() || to == OWN_CONVERSATION {
        return None;
    }
    // A listing can print `name [ref]`; the name is the address.
    let name = to.split_whitespace().next().unwrap_or(to);
    let matches =
        |b: &String| !b.is_empty() && (name == b.as_str() || name.starts_with(&format!("{b}-")));
    if known.iter().any(matches) || in_project(name, project) {
        return None;
    }
    Some(format!(
        "refused: `{name}` is not a session of this project ({project}). `ListAgents` shows \
         every agent on this machine, including other projects' fleets, and a message to one \
         is an instruction to someone else's workers (air-0lk). This project's peers: {}. If \
         `{name}` is a subagent you spawned here, name it after this project's lane.",
        if known.is_empty() {
            "none recorded yet".to_string()
        } else {
            known.join(", ")
        }
    ))
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

    #[test]
    fn a_peer_outside_this_project_is_refused_and_unknown_names_are_too() {
        let known: Vec<String> = ["alpha", "beta", "ai-runner"].map(String::from).to_vec();
        assert_eq!(peer_refusal("alpha-6d", "air", &known), None);
        assert_eq!(peer_refusal("ai-runner-0e", "air", &known), None);
        assert_eq!(peer_refusal("air-w1", "air", &known), None);
        // The parent conversation is never another project's.
        assert_eq!(peer_refusal("main", "air", &known), None);
        // The incident.
        let r = peer_refusal("adopter-51", "air", &known).unwrap();
        assert!(
            r.contains("adopter-51") && r.contains("alpha, beta"),
            "{r}"
        );
        // Closed by default: a name Air cannot place is refused, not allowed.
        assert!(peer_refusal("researcher", "air", &known).is_some());
        // A `name [ref]` address is matched on the name.
        assert_eq!(peer_refusal("alpha-6d [3fa9c1]", "air", &known), None);
        assert!(peer_refusal("adopter-51 [aaaa]", "air", &known).is_some());
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
