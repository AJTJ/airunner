//! What every timing budget cost, read back out of the event log (air-d75).
//!
//! [`air_ledger::budgets`] records the waits; this attaches the prose each number needs to be
//! read — what the budget protects and which way it fails when it is hit — and pools the
//! per-event samples into one distribution per budget.
//!
//! **Why fail direction is printed beside the number.** Three of these budgets fail toward
//! PERMITTING: a hook killed at its cap, a `git` call past 1.5 s inside a hook, and a SQLite
//! lock held past its timeout all make `air hook` fail open, which means the one refusal Air
//! makes silently does not refuse. A p99 is a different fact depending on which of those it
//! belongs to, and a reader who has to remember which is which will eventually not.
//!
//! **The `hook` row is censored and says so.** Claude Code kills the hook at its cap, and a
//! killed process writes no event line — so the one wait whose value would matter most is the
//! one that deletes its own sample. [`HookPairs`] is the substitute: PreToolUse invocations
//! with no PostToolUse to match them, which is what a killed hook leaves behind.
//!
//! Pure over the day texts, so `air selftest` drives it without a ledger.

use std::collections::BTreeMap;

use serde::Serialize;

use super::bd_latency::percentiles;

/// One budget, and what a reader needs to know before reading its numbers.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Budget {
    /// The name recorded on the event line; one of [`air_ledger::budgets`]'s constants.
    pub name: &'static str,
    /// Where the constant lives, so the number and the site stay one lookup apart.
    pub site: &'static str,
    /// What waits on it.
    pub protects: &'static str,
    /// What happens when the budget is hit. See [`Fails`].
    pub fails: Fails,
}

/// Which way a budget fails when it runs out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Fails {
    /// The caller is told: an error, a refusal, a named command. Nothing is permitted that
    /// would otherwise have been refused.
    Closed,
    /// A hook path: the error becomes `exit 0` with a `fail-open` event line, so a gate that
    /// would have refused allows instead. There IS a line saying so.
    Open,
    /// The process is killed by something outside Air, so it writes nothing at all. A gate
    /// that never ran and a gate that allowed are the same in the record.
    Silent,
}

impl Fails {
    pub fn as_str(self) -> &'static str {
        match self {
            Fails::Closed => "closed (the caller is told)",
            Fails::Open => "OPEN (the hook fails open; a refusal does not happen)",
            Fails::Silent => "SILENT (killed; no event line is written at all)",
        }
    }
}

/// Every budget Air waits on. Checked against [`air_ledger::budgets::NAMES`] by a probe: a
/// name that can be recorded and has no row here would be measured and never reported, which
/// is the shape of defect `air audit` already refuses to have (air-0y9).
pub const CATALOGUE: &[Budget] = &[
    Budget {
        name: air_ledger::budgets::BD,
        site: "crates/bd/src/lib.rs DEFAULT_TIMEOUT + PER_ID via budget_for (60 s + 5 s/id; AIR_BD_TIMEOUT_MS)",
        protects: "every `bd` process Air runs outside the four sites below",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::BD_ACCEPTANCE,
        site: "crates/bd/src/lib.rs budget_for, from crates/cli/src/cmd/status.rs acceptance_for (60 s + 5 s/id)",
        protects: "`air land`'s read of the acceptance criteria it prints beside each bead",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::BD_NUDGE,
        site: "crates/cli/src/cmd/ready_cache.rs (AIR_NUDGE_BD_TIMEOUT_MS, default 3 s; the 5 s hook cap forces it)",
        protects: "the Stop nudge's confirmation that a ready bead is still claimable",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::BD_PROBE,
        site: "crates/cli/src/cmd/claim.rs PROBE_DEFAULT_MS (AIR_BD_PROBE_TIMEOUT_MS, default 30 s)",
        protects: "the `bd show` that asks whether a timed-out `--claim` landed anyway",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::BD_STATUS,
        site: "crates/cli/src/cmd/bd_latency.rs status_bd_budget (4x bd's median, 2-8 s, flat per id; the 20 s MCP tool limit forces it)",
        protects: "`air status`'s reconcile, and through it the coordinator's channel",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::GIT,
        site: "crates/cli/src/git.rs TIMEOUT (1.5 s)",
        protects: "every `git` call Air makes, including the one every hook makes",
        fails: Fails::Open,
    },
    Budget {
        name: air_ledger::budgets::GIT_WORKTREE,
        site: "crates/cli/src/cmd/worktree.rs GIT_BUDGET (120 s)",
        protects: "`worktree add`, the include listing, worktree removal, and `air batch cut`'s merges",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::HOOK,
        site: "crates/cli/src/cmd/install.rs HOOK_TIMEOUT_SECS (5 s, written into settings.json)",
        protects: "every `air hook` invocation, the hand-over gate among them",
        fails: Fails::Silent,
    },
    Budget {
        name: air_ledger::budgets::MCP_TOOL,
        site: "crates/cli/src/cmd/mcp.rs TOOL_TIMEOUT (20 s)",
        protects: "one `air` subprocess behind an MCP tool call",
        fails: Fails::Closed,
    },
    Budget {
        name: air_ledger::budgets::SQLITE_LOCK,
        site: "crates/ledger/src/lib.rs BUSY_TIMEOUT (1 s)",
        protects: "every ledger write, hooks included",
        fails: Fails::Open,
    },
    Budget {
        name: air_ledger::budgets::TREE_READERS,
        site: "crates/cli/src/cmd/readers.rs BUDGET (4 s per child: ps, lsof)",
        protects: "`air status`'s readers lines, the idle exemption, `air land`'s warning",
        fails: Fails::Closed,
    },
];

/// One budget's distribution over the window.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub name: String,
    pub site: &'static str,
    pub protects: &'static str,
    pub fails: &'static str,
    /// The largest budget seen in force at the site, in milliseconds.
    pub budget_ms: u64,
    pub waits: usize,
    /// Waits that reached the budget. Every one is a decision made on less than was asked for.
    pub hits: u64,
    pub p50_ms: u64,
    pub p90_ms: u64,
    pub p99_ms: u64,
    pub max_ms: u64,
    /// Waits at or past four fifths of the budget: the ones a slightly worse run turns into
    /// hits. For the hook's 5 s cap this is the count over 4 s.
    pub near_ms: u64,
    pub near_misses: usize,
}

/// PreToolUse invocations with nothing to pair them with — the `hook` budget's censored tail.
///
/// Claude Code fires PreToolUse and PostToolUse for an overlapping set of tools, and a tool
/// call that got its Pre should reach exactly one of PostToolUse, PermissionDenied or
/// PostToolUseFailure. When it reaches none, one of those hooks did not run, and the most
/// likely reason is that it was killed at its cap.
///
/// The components are printed, not just the difference, because this is an inference over
/// counts and not an observation: there is no tool-call id on a hook payload to pair on, so
/// pairing is per (session, tool) and a reader has to be able to check the arithmetic.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HookPairs {
    /// Tools matched by BOTH the PreToolUse and PostToolUse matchers Air installs; the only
    /// ones that can be paired. Derived from `install::hook_entries`, never hardcoded, so a
    /// matcher change moves this rather than silently invalidating it.
    pub paired_tools: Vec<String>,
    pub pre: usize,
    pub post: usize,
    pub denied: usize,
    pub failed: usize,
    /// `pre - (post + denied + failed)` summed per (session, tool), floored at zero.
    pub pre_unmatched: usize,
    /// `post - pre` summed the same way: a lost PreToolUse leaves this instead.
    pub post_unmatched: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Budgets {
    pub rows: Vec<Row>,
    /// Budget names read off event lines that [`CATALOGUE`] has no row for, so a budget
    /// recorded by a site nobody classified shows up rather than being silently dropped.
    pub uncatalogued: Vec<(String, usize)>,
    pub hooks: HookPairs,
}

/// The tools both installed matchers cover, in the order the PreToolUse matcher lists them.
pub fn paired_tools() -> Vec<String> {
    let alts = |event: &str| -> Vec<String> {
        super::install::hook_entries()
            .into_iter()
            .find(|(e, _)| *e == event)
            .and_then(|(_, m)| m)
            .unwrap_or_default()
            .split('|')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    };
    let post = alts("PostToolUse");
    alts("PreToolUse")
        .into_iter()
        .filter(|t| post.contains(t))
        .collect()
}

/// Pool every budget sample on these day texts into one distribution per budget.
pub fn budgets_of(days: &[(String, String)], since: &str) -> Budgets {
    let mut samples: BTreeMap<String, (u64, u64, Vec<u64>)> = BTreeMap::new();
    let paired = paired_tools();
    // (session, tool) -> (pre, post, denied, failed)
    let mut pairs: BTreeMap<(String, String), [usize; 4]> = BTreeMap::new();

    for (day, text) in days {
        if day.as_str() < since {
            continue;
        }
        for line in text.lines() {
            let hooky = line.contains("\"hook.");
            if !line.contains("\"budgets\"") && !hooky {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if let Some(map) = v.get("budgets").and_then(serde_json::Value::as_object) {
                for (name, w) in map {
                    let e = samples.entry(name.clone()).or_default();
                    e.0 = e.0.max(
                        w.get("budget_ms")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0),
                    );
                    e.1 = e.1.saturating_add(
                        w.get("hits")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0),
                    );
                    if let Some(ms) = w.get("ms").and_then(serde_json::Value::as_array) {
                        e.2.extend(ms.iter().filter_map(serde_json::Value::as_u64));
                    }
                }
            }
            if !hooky {
                continue;
            }
            let command = v
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let slot = match command {
                "hook.PreToolUse" => 0,
                "hook.PostToolUse" => 1,
                "hook.PermissionDenied" => 2,
                "hook.PostToolUseFailure" => 3,
                _ => continue,
            };
            let field = |k: &str| {
                v.get("inputs")
                    .and_then(|i| i.get(k))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let tool = field("tool");
            if !paired.contains(&tool) {
                continue;
            }
            let counts = pairs.entry((field("session_id"), tool)).or_default();
            if let Some(c) = counts.get_mut(slot) {
                *c = c.saturating_add(1);
            }
        }
    }

    let by_name: BTreeMap<&str, &Budget> = CATALOGUE.iter().map(|b| (b.name, b)).collect();
    let mut rows = Vec::new();
    let mut uncatalogued = Vec::new();
    for (name, (budget_ms, hits, mut ms)) in samples {
        let Some(b) = by_name.get(name.as_str()) else {
            uncatalogued.push((name, ms.len()));
            continue;
        };
        let Some(p) = percentiles(&mut ms) else {
            continue;
        };
        // Four fifths of the budget: 4 s of the hook's 5 s cap.
        let near_ms = budget_ms.saturating_mul(4).checked_div(5).unwrap_or(0);
        rows.push(Row {
            name: name.clone(),
            site: b.site,
            protects: b.protects,
            fails: b.fails.as_str(),
            budget_ms,
            waits: p.runs,
            hits,
            p50_ms: p.p50_ms,
            p90_ms: p.p90_ms,
            p99_ms: p.p99_ms,
            max_ms: p.max_ms,
            near_ms,
            near_misses: ms.iter().filter(|m| **m >= near_ms).count(),
        });
    }
    // Loudest first: a budget with hits, then one with near misses, then by tail.
    rows.sort_by(|a, b| {
        b.hits
            .cmp(&a.hits)
            .then_with(|| b.near_misses.cmp(&a.near_misses))
            .then_with(|| b.max_ms.cmp(&a.max_ms))
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut hooks = HookPairs {
        paired_tools: paired,
        ..HookPairs::default()
    };
    for [pre, post, denied, failed] in pairs.into_values() {
        hooks.pre = hooks.pre.saturating_add(pre);
        hooks.post = hooks.post.saturating_add(post);
        hooks.denied = hooks.denied.saturating_add(denied);
        hooks.failed = hooks.failed.saturating_add(failed);
        let accounted = post.saturating_add(denied).saturating_add(failed);
        hooks.pre_unmatched = hooks
            .pre_unmatched
            .saturating_add(pre.saturating_sub(accounted));
        hooks.post_unmatched = hooks
            .post_unmatched
            .saturating_add(post.saturating_sub(pre));
    }

    Budgets {
        rows,
        uncatalogued,
        hooks,
    }
}

/// The budgets block. Says so explicitly when nothing was recorded: a silent zero here is
/// indistinguishable from the state this replaced, where the waits happened and nothing wrote
/// them down.
pub fn render(b: &Budgets) -> String {
    let mut s = String::from(
        "\nbudgets: every wait Air makes, against the budget it was made under (air-d75). \
         `hits` is a budget reached — a decision taken on less than was asked for.\n",
    );
    if b.rows.is_empty() {
        s.push_str(
            "  nothing recorded in this window. If commands ran, the binary that ran them \
             predates the recording; every `air` command and every hook writes a `budgets` \
             object once it does not.\n",
        );
    }
    for r in &b.rows {
        s.push_str(&format!(
            "  {} [{}]: {} wait(s) under {} ms; p50 {} ms, p90 {} ms, p99 {} ms, max {} ms; \
             {} hit(s), {} within {} ms of the budget\n",
            r.name,
            r.fails,
            r.waits,
            r.budget_ms,
            r.p50_ms,
            r.p90_ms,
            r.p99_ms,
            r.max_ms,
            r.hits,
            r.near_misses,
            r.budget_ms.saturating_sub(r.near_ms),
        ));
        s.push_str(&format!("      {} · {}\n", r.protects, r.site));
    }
    if !b.uncatalogued.is_empty() {
        s.push_str(
            "  defect: budgets recorded with no catalogue row, so nothing above says which way \
             they fail:\n",
        );
        for (name, n) in &b.uncatalogued {
            s.push_str(&format!("    {n:5}  {name}\n"));
        }
    }
    s.push_str(&render_hooks(&b.hooks));
    s
}

/// The censored-tail block for the `hook` budget. Its own function because the definition has
/// to travel with the number: this is an inference over counts, and an unexplained "3 hooks
/// lost" is the derived-reads-like-observed failure again.
fn render_hooks(h: &HookPairs) -> String {
    let mut s = format!(
        "  hook pairing over {}: {} PreToolUse, {} PostToolUse, {} PermissionDenied, {} \
         PostToolUseFailure\n",
        if h.paired_tools.is_empty() {
            "no tool matched by both matchers".to_string()
        } else {
            h.paired_tools.join("|")
        },
        h.pre,
        h.post,
        h.denied,
        h.failed,
    );
    s.push_str(&format!(
        "      {} PreToolUse with nothing to match, {} PostToolUse with no PreToolUse\n",
        h.pre_unmatched, h.post_unmatched
    ));
    s.push_str(
        "      A hook killed at its 5 s cap writes no event line, so the `hook` row above is \
         censored at exactly the value worth sizing against. These two counts are what a \
         killed hook leaves instead. They are an inference, not an observation: hook payloads \
         carry no tool-call id, so pairing is per (session, tool), and a session still running \
         has an open call in flight. A steady non-zero count is the signal; a small one is \
         the boundary.\n",
    );
    s
}
