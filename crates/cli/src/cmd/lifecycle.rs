//! The bead and branch lifecycle as a model (air-k52n).
//!
//! Every flow defect found on 2026-09-28 was a transition nobody had written down: a green
//! batch that main moved past (air-tml4), a closed bead that main moved past (air-voeb), a
//! close between two steps of a landing (air-5ynn). This module drives the SAME pure decision
//! functions the commands call, through every interleaving of the events below for two workers
//! and one lane, and asks one question of every state it reaches: can Air-permitted actions
//! still get every bead landed and closed from here? A state from which they cannot is stuck.
//!
//! The decision functions it calls, and nothing else decides:
//!
//! | Decision | Function |
//! |---|---|
//! | Is a worker's branch batch-ready? | `status::batch_ready_rule` |
//! | Does `air batch cut` refuse the lane's head? | `batch_cut::land_first` |
//! | Does a recorded green cover a bead? | `batch::cover` |
//! | Does `air close` pass? | `air_hooks::handover_verdict` |
//! | Is a one-member red holding a head out? | `status::red_alone` |
//!
//! What the model supplies itself, because the commands read it from git: a head is the set of
//! commits it contains, so "A contains B" is set inclusion, and a green stands for every head
//! with the same contents, as `verify_key: tree` makes it. `air land` moves main when the lane's
//! head has a green and contains main (`land::check`). Workers under a lane run no verify.
//!
//! Test-only: it has no caller outside `cargo test`.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

    use air_hooks::{GateFacts, handover_verdict};
    use air_ledger::landings::Member;
    use air_ledger::verify::{Kind, Verdict, VerifyRun};

    use crate::cmd::batch::{BeadCommit, Candidate, cover};
    use crate::cmd::batch_cut::land_first;
    use crate::cmd::status::{BatchFacts, batch_ready_rule, red_alone};

    /// A head: the commits it contains, as bits. Ten commit ids at most, so a `u16` does it
    /// and a state hashes in a few words instead of walking trees.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
    struct Set(u16);

    impl Set {
        fn bit(c: u8) -> u16 {
            1u16 << if c == ELSEWHERE { 9 } else { c }
        }
        fn contains(&self, c: &u8) -> bool {
            self.0 & Self::bit(*c) != 0
        }
        fn insert(&mut self, c: u8) {
            self.0 |= Self::bit(c);
        }
        fn extend(&mut self, other: Set) {
            self.0 |= other.0;
        }
        fn is_subset(&self, other: &Set) -> bool {
            self.0 & !other.0 == 0
        }
        fn iter(&self) -> impl Iterator<Item = u8> + '_ {
            (0..10u8)
                .filter(|b| self.0 & (1 << b) != 0)
                .map(|b| if b == 9 { ELSEWHERE } else { b })
        }
        fn difference(&self, other: &Set) -> impl Iterator<Item = u8> {
            let d = Set(self.0 & !other.0);
            (0..10u8)
                .filter(move |b| d.0 & (1 << b) != 0)
                .map(|b| if b == 9 { ELSEWHERE } else { b })
        }
    }

    impl From<[u8; 1]> for Set {
        fn from(c: [u8; 1]) -> Self {
            let mut s = Set::default();
            s.insert(c[0]);
            s
        }
    }

    /// The base commit every head starts from.
    const BASE: u8 = 0;
    /// A commit that reaches main by some other road (a hand landing, the coordinator's prose).
    const ELSEWHERE: u8 = 100;
    /// Commits per worker, plus one more for each red (a one-member red holds its head out
    /// until the worker commits past it, air-hyli); reds per run of the model.
    const MAX_COMMITS: u8 = 1;
    const MAX_REDS: u8 = 1;

    /// Which close rule the model runs (air-k52n item 3).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CloseRule {
        /// (a) Today's: the worker closes before landing, on a batch green or once its work is
        /// in main; a closed bead's branch is batch-ready (air-voeb).
        BeforeLanding,
        /// (b) The worker's close is "done with proof"; the bead stays open, and Air closes it
        /// when a landing puts all its commits in main.
        AtLanding,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Bead {
        Held,
        /// Rule (b) only: the worker said done with proof; the bead is still open.
        Done,
        Closed,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Worker {
        head: Set,
        bead: Bead,
        commits: u8,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Run {
        head: Set,
        main: Set,
        green: bool,
        members: Vec<Set>,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct State {
        main: Set,
        w: [Worker; 2],
        lane: Set,
        /// A cut whose verify has not been recorded: the member heads.
        cut: Option<Vec<Set>>,
        /// The newest run at each lane head. A cut is verified once, so this loses nothing
        /// the decisions read, and keeps the order of history out of the state.
        runs: BTreeMap<Set, Run>,
        moved: bool,
        reds: u8,
    }

    // The payloads are read only through `Debug`, for the path a failure prints.
    #[allow(dead_code)]
    #[derive(Clone, Debug)]
    enum Event {
        Commit(usize),
        MergeMain(usize),
        Close(usize),
        Cut,
        Verify(bool),
        Land,
        MainMoves,
    }

    fn sha(s: &Set) -> String {
        s.iter().map(|c| format!("{c:02x}")).collect()
    }

    fn commit_id(worker: usize, n: u8) -> u8 {
        1 + u8::try_from(worker).unwrap() * 4 + n
    }

    fn bead_of(c: u8) -> Option<usize> {
        match c {
            1..=4 => Some(0),
            5..=8 => Some(1),
            _ => None,
        }
    }

    fn bead_name(i: usize) -> String {
        format!("zz-{i}")
    }

    fn is_sub(a: &Set, b: &Set) -> bool {
        a.is_subset(b)
    }

    fn start() -> State {
        let base: Set = [BASE].into();
        let w = Worker {
            head: base,
            bead: Bead::Held,
            commits: 0,
        };
        State {
            main: base,
            w: [w.clone(), w],
            lane: base,
            cut: None,
            runs: BTreeMap::new(),
            moved: false,
            reds: 0,
        }
    }

    fn verify_run(r: &Run) -> VerifyRun {
        VerifyRun {
            id: sha(&r.head),
            worker: "lane".into(),
            sha: sha(&r.head),
            kind: Kind::Verify,
            exit_code: if r.green { 0 } else { 2 },
            trigger: "model".into(),
            failing_step: None,
            started_at: "t".into(),
            finished_at: "t".into(),
            log_path: None,
            command: None,
            duration_ms: None,
            output_bytes: None,
            dirty: false,
            tree: None,
            members: r
                .members
                .iter()
                .map(|m| Member {
                    worker: String::new(),
                    sha: sha(m),
                })
                .collect(),
            main_sha: Some(sha(&r.main)),
        }
    }

    /// The newest verdict recorded at exactly this head.
    fn verdict_at(s: &State, head: &Set) -> Option<Verdict> {
        s.runs.get(head).map(|r| {
            if r.green {
                Verdict::Green
            } else {
                Verdict::Red
            }
        })
    }

    fn open(rule: CloseRule, b: Bead) -> bool {
        match rule {
            CloseRule::BeforeLanding => b == Bead::Held,
            CloseRule::AtLanding => b != Bead::Closed,
        }
    }

    /// `status::batch_ready_rule` for worker `i`, from the model's facts.
    fn batch_ready(s: &State, rule: CloseRule, i: usize) -> bool {
        let w = &s.w[i];
        let carried: Vec<String> = w
            .head
            .difference(&s.main)
            .filter_map(bead_of)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(bead_name)
            .collect();
        let closed: Vec<String> = (0..2)
            .filter(|j| s.w[*j].bead == Bead::Closed)
            .map(bead_name)
            .collect();
        let reds: Vec<VerifyRun> = s
            .runs
            .values()
            .filter(|r| !r.green)
            .map(verify_run)
            .collect();
        let f = BatchFacts {
            worker: format!("w{i}"),
            head: sha(&w.head),
            contains_main: is_sub(&s.main, &w.head),
            green_at_head: verdict_at(s, &w.head) == Some(Verdict::Green),
            carried,
            held: if open(rule, w.bead) {
                vec![bead_name(i)]
            } else {
                vec![]
            },
            closed,
            red_batch_at_head: red_alone(&reds, &sha(&w.head)),
            in_batch: s
                .cut
                .as_ref()
                .filter(|m| m.contains(&w.head))
                .map(|_| sha(&s.lane)),
            ..BatchFacts::default()
        };
        batch_ready_rule(&f).is_ok()
    }

    /// `air_hooks::handover_verdict` for worker `i`'s bead, from the model's facts, with the
    /// batch scan (`batch::cover`) and the in-main check computed as `batch::for_bead` does.
    fn close_passes(s: &State, rule: CloseRule, i: usize) -> bool {
        let w = &s.w[i];
        let mine: Vec<u8> = (0..w.commits).map(|n| commit_id(i, n)).collect();
        let outstanding: Vec<u8> = mine
            .iter()
            .copied()
            .filter(|c| w.head.contains(c) && !s.main.contains(c))
            .collect();
        let batch_green = if outstanding.is_empty() {
            (!mine.is_empty() && mine.iter().all(|c| s.main.contains(c)))
                .then(|| "in main".to_string())
        } else {
            let commits: Vec<BeadCommit> = outstanding
                .iter()
                .rev()
                .map(|c| BeadCommit {
                    sha: format!("{c:02x}"),
                    subject: String::new(),
                })
                .collect();
            let candidates: Vec<Candidate> = s
                .runs
                .values()
                .filter(|r| r.green)
                .map(|r| Candidate {
                    sha: sha(&r.head),
                    worker: "lane".into(),
                    contains_main: is_sub(&r.main, &r.head),
                    contains: outstanding
                        .iter()
                        .rev()
                        .map(|c| r.head.contains(c))
                        .collect(),
                })
                .collect();
            cover(&candidates, &commits).covering.map(|(sha, _)| sha)
        };
        let main_is_ancestor = is_sub(&s.main, &w.head);
        let carried = w.head.difference(&s.main).any(|c| bead_of(c) == Some(i));
        let f = GateFacts {
            worker: format!("w{i}"),
            head: sha(&w.head),
            green_at_head: verdict_at(s, &w.head) == Some(Verdict::Green),
            batch_green,
            main_is_ancestor,
            work_in_main: !main_is_ancestor && is_sub(&w.head, &s.main),
            main_sha: sha(&s.main),
            bead_claimed_or_carried: open(rule, w.bead) || carried,
            bead: Some(bead_name(i)),
            held_beads: vec![bead_name(i)],
            under_lane: true,
            ..GateFacts::default()
        };
        handover_verdict(&f).pass
    }

    /// Forget what no decision can read any more, so equal futures are one state. A run at the
    /// lane's head or at a worker's head is kept (the cut's refusal, `air land`, the gate's head
    /// check). Otherwise a green whose head is all in main covers only commits in main, which
    /// the scan never asks about, and a red matters only while a worker's head is its lone
    /// member (air-hyli).
    fn canon(mut s: State) -> State {
        let heads: Vec<Set> = s.w.iter().map(|w| w.head).collect();
        let (main, lane) = (s.main, s.lane);
        s.runs.retain(|h, r| {
            // The lane's verdict, and any worker's green-at-head (the gate's head check).
            if *h == lane || heads.contains(h) {
                return true;
            }
            if r.green {
                !is_sub(h, &main)
            } else {
                matches!(r.members.as_slice(), [m] if heads.contains(m))
            }
        });
        s
    }

    /// Every event Air permits from `s`, with the state it leads to.
    fn next(s: &State, rule: CloseRule) -> Vec<(Event, State)> {
        next_raw(s, rule)
            .into_iter()
            .map(|(e, t)| (e, canon(t)))
            .collect()
    }

    fn next_raw(s: &State, rule: CloseRule) -> Vec<(Event, State)> {
        let mut out = Vec::new();
        for i in 0..2 {
            let w = &s.w[i];
            // A commit is not refused for a closed bead: Air does not, and a closed bead's
            // branch that went red alone gets back into a batch only by moving (air-hyli).
            if w.commits < MAX_COMMITS + s.reds {
                let mut t = s.clone();
                t.w[i].head.insert(commit_id(i, w.commits));
                t.w[i].commits += 1;
                out.push((Event::Commit(i), t));
            }
            if !is_sub(&s.main, &w.head) {
                let mut t = s.clone();
                t.w[i].head.extend(s.main);
                out.push((Event::MergeMain(i), t));
            }
            if w.bead == Bead::Held && close_passes(s, rule, i) {
                let mut t = s.clone();
                let landed = (0..w.commits).all(|n| s.main.contains(&commit_id(i, n)));
                t.w[i].bead = match rule {
                    CloseRule::BeforeLanding => Bead::Closed,
                    CloseRule::AtLanding if landed => Bead::Closed,
                    CloseRule::AtLanding => Bead::Done,
                };
                out.push((Event::Close(i), t));
            }
        }
        // The lane cuts only between batches, and only what is batch-ready.
        if s.cut.is_none() {
            let in_main = is_sub(&s.lane, &s.main);
            let contains_main = is_sub(&s.main, &s.lane);
            let refused = land_first(in_main, contains_main, verdict_at(s, &s.lane));
            let members: Vec<Set> = (0..2)
                .filter(|i| batch_ready(s, rule, *i))
                .map(|i| s.w[i].head)
                .collect();
            if !refused && !members.is_empty() {
                let mut t = s.clone();
                let mut lane = s.main;
                for m in &members {
                    lane.extend(*m);
                }
                t.lane = lane;
                t.cut = Some(members);
                out.push((Event::Cut, t));
            }
        }
        if let Some(members) = &s.cut {
            for green in [true, false] {
                if !green && s.reds >= MAX_REDS {
                    continue;
                }
                let mut t = s.clone();
                t.runs.insert(
                    s.lane,
                    Run {
                        head: s.lane,
                        main: s.main,
                        green,
                        members: members.clone(),
                    },
                );
                t.cut = None;
                t.reds += u8::from(!green);
                out.push((Event::Verify(green), t));
            }
        }
        if s.cut.is_none()
            && verdict_at(s, &s.lane) == Some(Verdict::Green)
            && is_sub(&s.main, &s.lane)
            && !is_sub(&s.lane, &s.main)
        {
            let mut t = s.clone();
            t.main.extend(s.lane);
            if rule == CloseRule::AtLanding {
                for i in 0..2 {
                    let w = &t.w[i];
                    let all_in =
                        w.commits > 0 && (0..w.commits).all(|n| t.main.contains(&commit_id(i, n)));
                    if all_in && w.bead != Bead::Closed {
                        t.w[i].bead = Bead::Closed;
                    }
                }
            }
            out.push((Event::Land, t));
        }
        if !s.moved {
            let mut t = s.clone();
            t.main.insert(ELSEWHERE);
            t.moved = true;
            out.push((Event::MainMoves, t));
        }
        out
    }

    /// Every bead is closed and all its commits are in main. A bead closed with no commit is
    /// done too: `air close` passes it on a green at the worker's head (the head check), which
    /// is Air's rule today and is reported on air-k52n rather than judged here.
    fn done(s: &State) -> bool {
        (0..2).all(|i| {
            let w = &s.w[i];
            (0..w.commits).all(|n| s.main.contains(&commit_id(i, n))) && w.bead == Bead::Closed
        })
    }

    /// Explore every reachable state, then walk back from the finished ones. Returns
    /// (reachable, stuck, one path to a stuck state for the report).
    fn explore(rule: CloseRule) -> (usize, Vec<State>, Vec<String>) {
        let mut index: HashMap<State, usize> = HashMap::new();
        let mut states: Vec<State> = Vec::new();
        let mut parent: Vec<Option<(usize, String)>> = Vec::new();
        let mut preds: Vec<Vec<usize>> = Vec::new();
        let mut queue = VecDeque::new();
        let s0 = start();
        index.insert(s0.clone(), 0);
        states.push(s0);
        parent.push(None);
        preds.push(Vec::new());
        queue.push_back(0);
        while let Some(k) = queue.pop_front() {
            let s = states[k].clone();
            for (e, t) in next(&s, rule) {
                let j = match index.get(&t) {
                    Some(j) => *j,
                    None => {
                        let j = states.len();
                        index.insert(t.clone(), j);
                        states.push(t);
                        parent.push(Some((k, format!("{e:?}"))));
                        preds.push(Vec::new());
                        queue.push_back(j);
                        j
                    }
                };
                preds[j].push(k);
            }
        }
        let mut can = vec![false; states.len()];
        let mut back: VecDeque<usize> = VecDeque::new();
        for (k, s) in states.iter().enumerate() {
            if done(s) {
                can[k] = true;
                back.push_back(k);
            }
        }
        while let Some(k) = back.pop_front() {
            for &p in &preds[k] {
                if !can[p] {
                    can[p] = true;
                    back.push_back(p);
                }
            }
        }
        let stuck: Vec<usize> = (0..states.len()).filter(|k| !can[*k]).collect();
        let path = stuck
            .first()
            .map(|&k| {
                let mut p = Vec::new();
                let mut at = k;
                while let Some((q, e)) = &parent[at] {
                    p.push(e.clone());
                    at = *q;
                }
                p.reverse();
                p
            })
            .unwrap_or_default();
        (
            states.len(),
            stuck.into_iter().map(|k| states[k].clone()).collect(),
            path,
        )
    }

    /// air-k52n: from every state Air can reach, some sequence of Air-permitted actions still
    /// lands and closes every bead. Fails, with the shortest path to a stuck state, on the code
    /// as it stood before air-tml4 and air-voeb.
    #[test]
    fn every_reachable_state_can_still_land_and_close_every_bead() {
        let t0 = std::time::Instant::now();
        let (n, stuck, path) = explore(CloseRule::BeforeLanding);
        assert!(n > 100, "the model reached only {n} states");
        assert!(
            stuck.is_empty(),
            "{} of {n} reachable states are stuck; the first is reached by {path:?}: {:?}",
            stuck.len(),
            stuck.first()
        );
        // How long it took is reported, not asserted (air-8gnk: no wall-clock bars in tests).
        eprintln!("lifecycle model: {n} states in {:?}", t0.elapsed());
    }

    /// air-k52n item 3: the same model under rule (b), close at landing. Printed for the
    /// owner's ruling, asserted only to have run: no rule is changed by that bead.
    #[test]
    fn close_at_landing_is_evaluated() {
        let (n, stuck, path) = explore(CloseRule::AtLanding);
        eprintln!(
            "close at landing: {n} reachable states, {} stuck; first by {path:?}",
            stuck.len()
        );
        assert!(n > 100, "the model reached only {n} states");
    }
}
