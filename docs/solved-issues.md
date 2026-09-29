# Solved issues: the register

Every problem Air has hit and demonstrably solved, so that a future change does not bring one
back. Started 2026-09-28 against main at `61f38f4`; the coordinator appends an entry when a fix lands with its incident, and never before.

Inclusion rule: an entry needs a recorded incident (a date and where it was seen, with the count
when one was recorded) and a landed fix (a commit on main, a closed bead, a test or selftest
probe, or a row in `crates/cli/src/cmd/mechanisms.rs`). Nothing planned, open, preemptive or
"should" is here; those are in `docs/design.md` §10. What looked like a fix but had no evidence
of being solved is listed under "Left out" at the end. Adopters are cited as "an adopter"
(CLAUDE.md). Paths are relative to the repo root; line numbers are from main at `61f38f4`.

## Landing and the close gate

**A close in the seconds between the fast-forward and the landing row was refused.**
Seen: an adopter, 2026-09-26/27 round on 0.4.10: 5 refusals in one day, each right after a
"close them now" notice, each passing on a retry about 20 s later.
Fix: the gate asks git whether every commit carrying the bead's `Bead:` trailer is in main
(`crates/cli/src/cmd/batch.rs:206` `in_main`), not the landings row. Commit 2406357 (air-5ynn);
tests `a_bead_in_main_is_covered_without_a_landing_row` and
`a_close_between_the_fast_forward_and_the_landing_row_passes` in `crates/cli/tests/claim_cli.rs`.
Do not: answer "work is in main" from a ledger row that is written after main moves.

**A green stopped counting the moment anyone wrote to main.**
Seen: an adopter's coordinator invalidated a whole batch with one prose commit on main,
2026-09-06; the refusal named no batch green at all.
Fix: `air record` stores main's sha before the run (`verify_runs.main_sha`, schema v19) and the
close gate asks "contains main" of that main. Commit d1a8925 (air-9ij); SURFACE notice
`close-asks-the-recorded-main`.
Do not: ask the close gate about current main. The landing gate still does, on purpose.

**A close was refused as behind main right after the worker's own batch landed.**
Seen: 0.4.6 live trial, 2026-09-26, defect D1.
Fix: the gate no longer refuses when main already contains the worker's head; the green is
still required. Commit ba8cd4d; SURFACE `close-after-own-landing`.
Do not: read "head not in main" as "behind main" when main contains the head.

**A close was refused because the worker had already committed the next bead on top.**
Seen: 0.4.7 live trial, 2026-09-26, defect D1.
Fix: the gate judges the bead's own trailer commits, not the branch head. Commit 1887a65;
SURFACE `close-judges-the-bead`; roles.md says a worker may start the next bead at once.
Do not: judge `main-merged` from the branch head when the bead has trailers.

**The close gate looked at only the newest 12 greens, and said "12".**
Seen: an adopter's gate messages always said "12"; the 2026-09-28 limits audit (tree 2cbc3af)
found it among seven silent caps.
Fix: `greens_for` (`crates/cli/src/cmd/batch.rs:177`) reads every green after the bead's oldest
commit. Commit a4830c0 (air-06k7); test `a_covering_green_thirteenth_newest_still_covers`.
Do not: reintroduce a candidate window in the gate.

**`air close` ran the whole gate on `"<id> <filename>"` and advised a two-word trailer.**
Seen: an adopter, a worker passed a bead id plus a filename as one id, 8 times.
Fix: `close::check_shape` refuses whitespace before anything runs and `check_known` names an id
bd does not have when the gate would refuse (`crates/cli/src/cmd/close.rs:131,150`). Commit
045deaf (air-a8ra); test `an_id_with_whitespace_or_nothing_in_it_is_malformed`.
Do not: feed an unvalidated id into the gate's advice.

**A fast-forward that timed out after succeeding was recorded as "main is untouched".**
Seen: an adopter's coordinator, 2026-09-06; the landing row had an empty `merge_commit`, no
`landed` row, and three beads attributed to nothing.
Fix: on any fast-forward error Air asks `merge-base --is-ancestor` and records what it finds
(`crates/cli/src/cmd/land.rs:367` `after_fast_forward`). Commit fe0945a (air-htmn).
Do not: read a killed git child as "it did not happen"; a ref update is atomic.

**`air land --all` answered `{"landed": [], "ok": true}` when it was broken.**
Seen: an adopter, 2026-08-22, every precondition met by hand, nothing landed; this repo hit it
twice the same day.
Fix: selection returns landings, skips and errors separately; an error refuses with its reason,
nothing landable exits 2. Commit 5caa9a7 (air-6u5).
Do not: let a git or ledger failure in selection read as an empty queue.

**Main's tip was read from the running worktree, so a branch compared against itself.**
Seen: 2026-09-06, a branch vanished from `landable` while looking batch-ready.
Fix: `git::main_tip` reads the ref (`crates/cli/src/git.rs:180`) at all three sites. Commit
0e7aa84 (air-i6fd).
Do not: use `git::head(cwd)` where main is meant.

**`air land` refused while any verify anywhere in the fleet was running.**
Seen: coordinator's deadlock analysis, 2026-09-28: the lane's green batch could wait
indefinitely behind workers' own runs, and the cut refused meanwhile.
Fix: only a run in the main checkout or the lane's tree holds a landing
(`crates/cli/src/cmd/land.rs:116` `at_risk`). Commit 5be77f4 (air-9yx6); test
`only_a_verify_main_moving_would_destroy_refuses_a_landing`.
Do not: count a worker's verify in its own worktree as at risk from main moving.

**A landing went ahead over a verify in flight after the warning was read.**
Seen: an adopter: 1,199 s of destroyed verify in two incidents plus six more runs invalidated
(mechanism `land-in-flight-refusal`).
Fix: `air land` refuses, naming each run and pid; `--despite-inflight` lands and is recorded
(`crates/cli/src/cmd/land.rs:90`). Commit 9b79fe7 (air-1bm).
Do not: demote the refusal back to a warning.

**`air land --repo <main>` from a worktree landed.**
Seen: 2026-08-22, a worker ran `cargo run -- --repo <main> land --all` and it merged its branch.
Fix: the role comes from the process, not from `--repo`; since 2026-09-14 from `AIR_ROLE`
(`crates/cli/src/cmd/land.rs:46` `may_land`). Commit 8b66d8a (air-29a); SURFACE
`role-is-the-launchers`.
Do not: derive who is landing from an argument the caller supplies.

**The landing recorded one bead when the merge carried five.**
Seen: 2026-09-05, four beads landed with no acceptance read.
Fix: a named bead selects its branch and the row records every bead the branch carries
(`crates/cli/src/cmd/land.rs:427` `resolve`). Commit 36cc695 (air-dnr).
Do not: filter the landing's beads by the ids typed.

**A bd timeout on the acceptance read became "states no acceptance criteria" after main moved.**
Seen: 2026-09-05.
Fix: the read happens before `commit-tree`; a bd error refuses with main untouched. Commit
e3457d9 (air-bh4).
Do not: read bd after the fast-forward.

**A batch of only the coordinator's commits was refused for naming no bead.**
Seen: 0.4.4 live trial, 2026-09-26.
Fix: such a batch lands carrying no bead; the lane is no longer told to `git commit --amend`,
which would discard its green. Commit 8675e7a; SURFACE `coordinator-commits-land-with-no-bead`.
Do not: suggest `--amend` on a verified head anywhere (air-ch9p asserts none remains).

**Main read `not green` after every landing.**
Seen: 0.4.8 live trial, 2026-09-26.
Fix: `air land` records the landing commit green when its tree is byte-identical to the verified
commit's. SURFACE `landing-commit-green`; earlier, greens keyed by commit not worker (air-7wf).
Do not: key a green by worker.

**A journal-only branch was refused with "no commit declares a bead".**
Seen: 2026-09-06.
Fix: a range of only journal commits lands with no trailer. Commit deb6802 (air-kexg).
Do not: require a trailer on a commit that is not work on a bead.

**A landing's `ok` verdict wore a stronger form than its lookup deserved.**
Seen: 2026-09-06 (air-rud0); an adopter, 7 of 8 `landed-not-closed` firings were paths the
acceptance text only mentioned; in this repo 6 of 9 held (air-k6uh).
Fix: the acceptance verdicts and `landed-not-closed` are deleted; `air land` prints each bead's
acceptance text with no verdict. Commit 9d76f0c (air-cizz); `audit::DELETED_KINDS`
(`crates/cli/src/cmd/audit.rs:527`).
Do not: match file names in acceptance prose against the merge.

**A hand-over refusal spelled four different not-green states identically.**
Seen: 2026-09-06; two of the four have opposite correct responses.
Fix: each state names its fact and carries its fix. Commit deffec5 (air-hgi9); on 2026-09-28
the refusal also names a red at HEAD with its log and a missing precheck (commit a5760eb,
air-erii).
Do not: collapse the not-green states into one sentence.

**Under a lane, the close told a worker waiting on its batch to `git merge main`.**
Seen: 0.4.8 live trial, 2026-09-26; merging moves the head off the sha the lane cut at.
Fix: `main-merged` is named only beside a green at the head that lacks main. SURFACE
`lane-gate-no-merge-main`.
Do not: prescribe a merge to a worker whose branch is in a batch.

**`air handover` printed "NOT landable" for work already on main.**
Seen: 0.4.9 live trial, 2026-09-26.
Fix: it says the head is already on main. SURFACE `handover-already-on-main`.

**`air release` left a pencilled assignee, and nobody else could claim the bead.**
Seen: an adopter, and this repo (air-an9), 2026-09-05; bd 1.2.x blocks every other `--claim` on
an assigned open bead. On bd 1.3.0 the reopen was then refused without the holder's actor
(0.4.6 trial).
Fix: reopen and unassign in one bd process, passing the holder's `--actor`
(`crates/bd/src/lib.rs:242` `reopen_argv`). Commits 35d48b3 (air-0kk) and 756a5ad.
Do not: split reopen and unassign into two processes, or drop `--actor`.

## The lane and batches

**`air batch cut` refused for good once the lane's green head fell behind main.**
Seen: 2026-09-28 21:30Z, this repo, after the owner landed a worker by hand; the lane could
neither cut nor land.
Fix: the cut refuses only a green head that could still land (contains main); a stale one is
named and re-cut from main (`crates/cli/src/cmd/batch_cut.rs:314,324` `land_first`,
`lane_start`). Commit f3b4769 (air-tml4); test
`a_green_batch_main_moved_past_is_re_cut_and_named_stale`.
Do not: guard a green batch that main has already moved past.

**After a red batch, the next cut built on the red merges.**
Seen: 0.4.8 live trial, 2026-09-26, workaround 1.
Fix: every cut resets the lane's branch to main first; it refuses while the lane has uncommitted
changes. Commit 9fcdc11; SURFACE `batch-cut-starts-from-main`.
Do not: let a lane's own commits survive a cut.

**A branch whose red batch had it as the only member stayed batch-ready, and was cut again.**
Seen: an adopter, twice in one day on 0.4.10; the lane reset and hand-built the batch. The 0.4.6
trial (D3) had already excluded a red recorded at the head itself, which a batch built on main
never is.
Fix: a one-member red excludes its member until it commits past that head
(`crates/cli/src/cmd/status.rs:764` `red_alone`). Commit c226d19 (air-hyli); test
`a_one_member_red_batch_excludes_its_member_until_it_moves`.
Do not: decide "red at head" from the batch commit's sha alone.

**A batch of one sent its member no result.**
Seen: 0.4.6 live trial, 2026-09-26, D2: members were read from merge commits and a batch of one
makes none.
Fix: `air batch cut` records its members (`batch_cuts`, schema v24). Commit ba8cd4d; SURFACE
`batch-cut-records-members`.
Do not: derive members from merge commits.

**A batch's member list was read from the worktree head at recording time.**
Seen: an adopter's lane, five times in one night (2026-09-06): a worker that committed between
the merge and `air record` dropped out of its own batch.
Fix: the run records the sha the batch took. Commit a8da024 (air-vsvt).

**A dropped branch was offered again before its worker committed, and the drop notice said
"merge main" for a conflict with an unlanded peer.**
Seen: 0.4.9 live trial, 2026-09-26; a worker merged the peer's unlanded commit.
Fix: a dropped head is not batch-ready until it moves; the notice names the branch and sha and
says to wait for it to land. SURFACE `dropped-head-not-batch-ready`.
Do not: tell a worker to merge a branch that is not on main.

**A cut with no members exited 0, so `air batch cut && air record verify` recorded a green at bare main.**
Seen: 2026-09-28 (air-k81s).
Fix: an empty cut prints what held each branch and exits 1; dry run exits 0. Commit 444c01e.

**A cut stopped by a leftover `index.lock` left no record, and a verify stopped by hand left no run.**
Seen: an adopter, 0.4.10 round.
Fix: a failed cut writes `batch-cut / failed` naming the lock holder; `air record` catches
SIGINT, SIGTERM and SIGHUP and writes the run as interrupted (`crates/cli/src/cmd/record.rs:330`
`Interrupt`). Commits f68d1bb and da5915f (air-nwat); tests
`a_cut_stopped_by_an_index_lock_is_recorded_as_failed`,
`killing_air_record_mid_run_leaves_an_interrupted_run`.
Do not: delete the in-flight row without writing a run.

**A red batch report silently stopped after 20 further runs.**
Seen: 2026-09-06; a dropped report looked like a fixed one.
Fix: no window; a standing red is repeated until a later green carries every member. Commit
e15fc7b (air-cyf).

**The lane's hand cut judged merges by `tail`'s exit status.**
Seen: an adopter: eleven batches judged that way, a skipped dry-merge, three wrong conflict
predictions (mechanism `batch-cut-refusal`, module header of `batch_cut.rs`).
Fix: `air batch cut` with `git merge-tree --write-tree` pre-checks. Commit a7c5401.
Do not: let the lane cut by hand.

**The auto-mode classifier denied the lane's `air land`.**
Seen: 0.4.1 live trial, 2026-09-25, denied as "[Modify Shared Resources]".
Fix: `permissions.allow: Bash(air land *)` on the lane's settings. Commit 32bfeeb (air-8cdh).

**The lane piped `air land` into `tail`, the classifier refused it, and the session refused every later landing.**
Seen: 2026-09-28 21:34Z, this repo; nothing landed for 45 minutes and the owner landed from a
shell.
Fix: the lane's whole loop is allowed bare (`crates/cli/src/cmd/launch.rs:413` `lane_allow`) and
roles.md tells it to run the commands with no pipe or redirect. Commit 7292918 (air-b1h8).
Do not: add a pipe to a lane command; the allow rule matches only the bare line.

**`idle-without-claim` offered the lane 58 beads 945 s into a batch verify.**
Seen: an adopter's lane, 2026-09-06.
Fix: the condition is silent while the worker's own verify runs. Commit 8b1b7e1 (air-t6ap).

**Status showed a branch batch-ready while it sat inside the batch being verified.**
Seen: an adopter; the coordinator twice thought the branch was skipped.
Fix: `in batch: <worker> at <head> is in batch <sha>` and `not_batch_ready` reason `in-batch`
(`crates/cli/src/cmd/status.rs:616` `batch_ready_rule`). Commit 50a9d93 (air-ch9p); test
`a_head_in_a_running_batch_is_in_batch_not_batch_ready`.

**A bead closed on a green, then main moved, and its branch was neither landable nor batch-ready.**
Seen: 2026-09-28 23:12Z, this repo: worker-2 closed air-92o0 at 14863f40 with main merged and
claimed the next bead; worker-1's landing moved main; `air land --worker worker-2` refused (the
branch did not contain main) and batch-ready said "no commit names a bead worker-2 holds",
because a closed bead is not held. An adopter saw the same shape with six beads.
Fix: `batch_ready_rule` (`crates/cli/src/cmd/status.rs:615`) counts a carried bead that is held
or closed (ledger `closed_claims`, no bd); `air status` prints `<id>: closed, waiting for the
lane`. Commit cba816b (air-voeb); test `a_closed_bead_not_on_main_is_batch_ready_and_named`;
SURFACE `closed-bead-batch-ready`.
Do not: require a held claim for a branch whose commits name a closed, unlanded bead.

**Every bead's `CHANGELOG.md` line conflicted with every other branch's: nine lane drops in one day.**
Seen: 2026-09-28, this repo; each drop cost the worker a merge, a resolve and a re-verify, and
the lane a re-cut.
Fix: a bead writes `changelog.d/<bead-id>.md` with the heading as its first word; `make
changelog` runs `scripts/changelog-assemble.sh`, which folds every fragment under `[Unreleased]`
and deletes it, refusing an unknown heading; `make release` refuses while a fragment is left
(`Makefile:62`). Commits c8a73c7, a137a57 (air-be6d); tests
`fragments_fold_under_their_heading_and_are_deleted` and `this_repos_fragments_assemble` in
`crates/cli/tests/changelog.rs`.
Do not: write a bead's line into `CHANGELOG.md`; two beads on one heading conflict at every
landing.

**Splitting a red batch was raw git, and Air's record of the split batch was a guess.**
Seen: 2026-09-29 live trial: a batch of two went red; the lane split with `git reset` and `git merge`,
and the green batch it made (637b9dce) had no `batch_cuts` row, so its members were inferred. An
adopter's lane hand-built batches twice in a day.
Fix: `air batch cut --without <worker>...` and `--only <worker>...` filter the batch-ready list before
the existing path, so a split is a recorded cut with every refusal unchanged; nothing refuses a hand
cut. Commit dc5d796 (air-5fo5); test `batch_cut_splits_with_without_and_only`.
Do not: derive a batch's members from merge commits; do not require the flag.

**After a batch of several branches went red, every member was offered again, the red commit included.**
Seen: 2026-09-29 live trial: batch 686abd40 (two members) red; the lane split and landed the good
member; the next-cut list held the red member's same commit 8c47747, which was cut and went red
again (0af0065c). An adopter's lane hit it twice in a day.
Fix: `red_held` replaces `red_alone` (`crates/cli/src/cmd/status.rs`): a head is held while a red
batch recorded it as a member and no green recorded after that red contains it, so a split's green,
by cut or by hand, frees the good member and the red one waits for a commit. Commit 2549922
(air-vdst); test `a_red_batch_holds_its_members_until_they_move_or_a_later_green_carries_them`;
the model test covers the ordering.
Do not: offer a member of a red batch again before it moves or a green carries it.

## Sessions and liveness

**A dead registration with a reused pid read as alive and got most of the ready notices.**
Seen: an adopter, 2026-09-27: a pid from 2026-08-31 was a `<defunct>` process; that row was
flagged idle 39,000+ min on all 252 status runs and received 62 of 65 "beads are ready"
fan-outs (live workers got 3); a second dead row showed as a coordinator holding 158 files.
Fix: alive means the pid runs with the registered start time and is not a zombie
(`crates/cli/src/cmd/lease.rs:117` `liveness`); one row per role name; the main checkout is the
owner. Commit 42a6f95 (air-sai7); test `liveness_needs_pid_start_time_and_no_zombie`.
Do not: decide liveness from pid existence.

**`STOPPED at <t>` stuck 16.5 h after the session resumed; a 6 h permission prompt showed running; 491 SubagentStop events flipped the parent idle; ages read negative.**
Seen: an adopter, 0.4.10 round.
Fix: a sticky fact holds only until the session's next own event
(`crates/cli/src/cmd/status.rs:2678` `sticky_holds`); `waiting for the person since <t>` after a
prompt notification (`hook.rs:537` `is_waiting_kind`); subagent hooks never change the parent;
one clock read. Commit cb7489b (air-n9f3); test `sticky_facts_yield_to_the_sessions_next_own_event`.
Do not: let whichever hook fires last write the session state.

**A freshly started worker was never counted idle, so the fleet stalled at `ready: 3`.**
Seen: 2026-09-26 live trial.
Fix: a session is recorded idle at start; "beads are ready" goes to every live worker holding no
claim whatever its state. Commit ac8fd80 (air-ludo).

**`gone-with-claim` fired on a fresh idle worker.**
Seen: an adopter, 2026-08-21.
Fix: sessions record the claude pid; gone means the pid is gone or no row after a 3-minute
launch grace. Commit 1767fe2.

**A coordinator whose shell had stepped into a worker's worktree was taken for that worker.**
Seen: an adopter, twice, 2026-09-05; it ran that worker's gate at its next Stop.
Fix: the launcher's `AIR_ROLE` and `BEADS_ACTOR` decide identity, cwd only for a session Air did
not launch. Commit 2a33a2e (air-75u).
Do not: derive identity from the Bash tool's cwd.

**A second `--settings` replaced the blob carrying `AIR_ENFORCE`, and nothing said so.**
Seen: an adopter ran five hours with the one refusal off, 2026-09-05.
Fix: env set on the spawned process and by `tmux new-session -e`; pass-through settings are
merged; `air status` marks a worker `UNENFORCED` (ledger v15). Commit 8c08200 (air-9dg).
Do not: carry role env inside a settings blob.

**A session that hit the account limit with no wake sat dead for 79 minutes.**
Seen: 2026-09-06, seven sessions, one machine: the six with a wake or auto-continue resumed
within 90 s of the reset.
Fix: hooks on `Notification` and `StopFailure` record the stop (schema v20); the coordinator is
told once at SessionStart to create its 5-minute wake. Commit a6d4881 (air-1n3).

**The coordinator sat in a question prompt for six hours hearing no notices.**
Seen: an adopter's coordinator, 0.4.10 round, 6 h 9 min.
Fix: the coordinator is denied `AskUserQuestion` like every other role; roles.md says a question
for the owner is a bead labelled `owner`. Commit 101bb44 (air-jcfb).

**The model column showed `[?]` for every launched session.**
Seen: 2026-09-28, checked on the coordinator's own transcript: the first assistant line was past
the 64 KB the reader stopped at.
Fix: `model_of` reads to the first model line (`crates/cli/src/cmd/hook.rs:1057`). Commit
a4830c0 (air-06k7); test `the_model_is_found_past_the_first_64_kb`.

**A hook fired from a fixture wrote a session row that raised channel conditions.**
Seen: 2026-09-06.
Fix: the row is kept, printed `NO TRANSCRIPT`, and never announced as a worker. Commit d58f6a9
(air-3jv5).

**A coordinator's commit in the main checkout invalidated four workers' landability; a verify there moved main under a worker's green.**
Seen: an adopter, 2026-09-06 (both).
Fix: the coordinator works in its own worktree (`air-jc2p.1`, commit 30f312e) and `air status`
warns about any launched session running in the main checkout (mechanism
`main-checkout-session`, commit b4650b5).
Do not: put a role's session in the main checkout.

**`--task` was read as one more `--disallowed-tools` value.**
Seen: 2026-08-21, on a detached launch.
Fix: the prompt positional leads argv; `--print` shell-escapes. Commit a663c2d (air-2ct).

**`claude --tmux` from the coordinator's Bash tool died with `tcgetattr failed`.**
Seen: 2026-08-21.
Fix: with no tty, Air spawns `tmux new-session -d` itself. Commit 1c01f66 (air-tdc).

**Every launched session stopped at the "New MCP server found" prompt, whose default dropped the channel.**
Seen: 0.4.1 live trial, 2026-09-25.
Fix: each launch approves the `air` server in its own settings (`enabledMcpjsonServers`).
Commit 3a9a9ba (air-oe9k).

**`air fleet up` started a second claude in every worker's worktree after the project name changed.**
Seen: twice on 2026-09-28 (22:15Z and 23:59Z), this repo: `"project": "ai_runner"` landed, the
tmux sessions were renamed from `air-*` to `ai_runner-*`, and `fleet up` saw no
`ai_runner-worker-N` and started three duplicates in worktrees that already had live sessions;
the coordinator killed each at the channel prompt before it registered.
Fix: `launch_role` asks the ledger for a live session of the worker's name first
(`crates/cli/src/cmd/launch.rs:1364,1381` `running_session`, `live_session`; alive by pid and
start time, air-sai7) and prints `<name>: running (session <id>)`; a dead session is still
relaunched. Commit 2a4ffa8 (air-4n5t); tests `fleet_up_starts_the_lane_and_workers_once`,
`a_worker_with_a_live_ledger_session_is_running`; SURFACE `running-from-the-ledger`.
Do not: decide whether a worker is running from its tmux session name.

## The channel and notices

**The same condition was pushed on every poll: `closed-not-landed` 417 times in a day.**
Seen: an adopter, 0.4.10 round: 207 pushes each for two beads on one worker, evaluated 11 times;
226 of the coordinator's 227 failed turns that night came within 30 s of one; `landable` pushed
with `for_minutes: 5` for a 1 s old condition; the Stop nudge named the same ready set 13 times;
every one of 97 landings warned about the same six readers.
Fix: pushes keyed by (kind, subject) (`crates/cli/src/cmd/mcp.rs:741` `select_new_at`); a new
subject's age starts at 0; the Stop nudge fingerprints the ready set; the landing warning
fingerprints the pid set (`readers.rs:480`). Commit 5da51a6 (air-8goe); tests
`two_subjects_of_one_kind_on_one_worker_push_once_each`, `a_new_subject_starts_at_zero_minutes`.
Do not: key a push by (worker, kind).
Weight: the 417 pushes went mostly into a session that could not read them (dead or out of
tokens), so they are not the cost; the defect was the keying, and its weight is the two beads on
one worker (air-fd8p).

**A worker that joined, or finished its claim, while beads were already ready was never told.**
Seen: 2026-09-28 (air-oerm); the notice went out only when the set changed.
Fix: a worker newly without a claim is told the set as it stands, once
(`crates/cli/src/cmd/fanout.rs:128` `fan_out_ready`). Commit fbe623d.

**Three idle workers all tried the same first bead.**
Seen: 0.4.9 live trial, 2026-09-26.
Fix: the list is rotated within each priority group by the recipient's position. SURFACE
`beads-ready-rotated`.

**Workers walking a stale ready list caused most refused claims.**
Seen: 0.4.10 live trial.
Fix: the notice and the peer-held refusal say the list is a snapshot and point at `bd ready`.
Commit ced8a06 (air-0blv).

**Only the coordinator had a channel, so the lane and workers learned things on their 5-minute wakes.**
Seen: 2026-09-26 live trial: a bead waited minutes between done and closed; the coordinator
waited about 4 minutes after wave 1 landed; a capture sat until the next wake.
Fix: every launched session loads the channel and receives its deliveries (`mcp.rs:656`
`deliver_once`); batch-ready to the lane, batch results to members, main moved to all, captures
and queue-empty to the coordinator (`fanout.rs`). Commits 42eb407, e50a2b0, 42bfaa6, 95264e0
(air-1vri); mechanisms `channel-delivery` through `queue-empty-to-coordinator`.
Do not: leave a role to learn a batch result from a wake or a relayed message.

**The channel poll ran a full `air status` with bd every ~8 s.**
Seen: 2026-08-29, this repo: about 5,674 bd calls and 2.26 h a day waiting on bd, median gather
3,895 ms, for about 45 pushes.
Fix: the poll answers from `bd_cache` (`BdUse::CachedFor`, `status.rs:1864`). Commit 3eacecd
(air-cmn).

**`air audit` read 10,722 log lines as 45 real pushes.**
Seen: 2026-08-29.
Fix: separate `evaluated` and `pushed` counts. Commit 5b222f3 (air-5uz).

**`air audit` said 0 closes passed while 176 beads closed, counted live `uds:` recipients as unresolved, counted subjects per worker, and called the nudge "once" beside 13 repeats.**
Seen: an adopter, 0.4.10 round.
Fix: all four lines read the ledger. Commit 93e8e10 (air-1yxj); four unit tests in
`crates/cli/src/cmd/audit.rs`.

**Five condition kinds and several decisions had no registry row, so the audit could not see them.**
Seen: 2026-08-29: five kinds read zero over 38,130 lines because the audit counts only kinds the
registry names (air-sze); Air's one refusal showed as a firing with no row (air-8br).
Fix: every emitted decision and condition has a row; `decisions::ALL` makes the set enumerable.
Commits 4090344, 1282f5f, 75bf119 (air-hqj8).
Do not: add a decision word without a registry row.

**Lease conditions were addressed to the holder, offering `air lease break` on a lease in use.**
Seen: an adopter, six firings in one day on healthy leases.
Fix: both lease conditions name whoever is waiting in `lease_wants`; nobody waiting means no
condition. Commit e5049ca (air-q9c).

**Notices carried opinions and misleading text.**
Seen: an adopter: "the repo's test is the bug; file it"; "close them now" to a member whose
commits named no bead, about 15 times; batch-ready re-listing landed beads; a `landable` push
under a lane that made the coordinator tell the lane to land by hand.
Fix: 87 strings read, 36 rewritten to state the fact and the command (the list is a comment on
air-ch9p); `landable` is not pushed while a lane session is alive; "close them now" names only
beads the member still holds. Commit 50a9d93.

**`batch-held` told every un-prechecked branch it was held, every cut.**
Seen: an adopter: 156 deliveries in a day, 247 `no-precheck` lines over 162 heads, 89 of them
while the precheck was already running.
Fix: the notice and the event line are deleted; `air status` still names `no-precheck`. Commit
a5f8666 (air-aepb).

**The `landable` condition was raised for the lane's own branch and offered `air land --all`.**
Seen: 0.4.8 live trial, 2026-09-26.
Fix: it skips the lane's branch and names who lands. SURFACE `landable-not-for-lane`.

**`handover-not-green` counted successful hand-overs and reported clean workers as failed.**
Seen: 2026-09-06.
Fix: only a refused hand-over is an attempt and a pass clears the counter; `air handover` never
stamps. Commits 23b9cbe (air-zqmi), b6e158f (air-eiv).

**The coordinator's local MCP server was not on the channel allowlist.**
Seen: live on Claude Code 2.1.239, 2026-08-21.
Fix: launch with `--dangerously-load-development-channels server:air`
(`crates/cli/src/cmd/launch.rs:31`). Commit 73576a1.

**Every fleet notice was produced only by the coordinator's channel poll, so a dead coordinator channel silenced the fleet.**
Seen: 2026-09-28 22:16Z to 2026-09-29, this repo: the coordinator's channel was killed and did not
reconnect; no batch-ready, beads-ready or queue-empty went out; workers idled 54 min beside a claimable
bead and the lane beside batch-ready branches five times, each nudged by hand. The 2026-09-29 trial:
a head made batch-ready by a precheck got no notice. An adopter's coordinator was down two hours.
Fix: the commands that change the fact queue the notice after their write: `air record` (batch-ready
to the lane), `air close`, `air claim` and `air release` (beads-ready, queue-empty); `air land` already
queued main-moved; the poll stays as a sweeper and the delivery key dedupes. Commit 8d12cf2 (air-4lci);
tests `a_precheck_that_makes_a_branch_batch_ready_tells_the_lane_with_no_poll`,
`closing_the_last_claim_tells_the_worker_what_is_ready_with_no_poll`.
Do not: make any notice depend on one session's poll being alive.

## Hooks

**The advisory gate was bypassed, so it became a refusal.**
Seen: 2026-08-22, the first bypass (air-i59, mechanism `handover-would-refuse` comment).
Fix: worker launches set `AIR_ENFORCE=1` (`crates/cli/src/cmd/hook.rs:738`). Commit eee108c.

**A `bd close` written on its own line after a commit in the same shell call was not checked.**
Seen: 0.4.5 live trial, 2026-09-26: a bead closed with no green containing its commits.
Fix: workers and the lane close with `air close`, which runs the gate itself; the hook's text
match on `bd close` and its claim release are deleted, and bd's write commands are on the deny
list. Commits 1cf6ba2, ad88172; standing ruling 2026-09-26 (design.md §8.1).
Do not: reintroduce a close path that reads the shell command's text.

**The Stop hook ran the whole close check on every stop and said "you are not done".**
Seen: an adopter: 443 lines in one day, 329 repeats, all describing the normal wait for the
lane; earlier (2026-08-21) a blocked worker got the identical advisory on every turn end.
Fix: the advisory is deleted; only the nudge remains, and it also fires when every held claim
is batch-ready (`crates/hooks/src/gate.rs:931` `stop_nudge`). Commit e0a0a45 (air-wau8); the
2026-08-21 dedupe by fingerprint was commit 1384467.
Do not: compute hand-over facts on Stop.

**The harness's worktree isolation cost 455 refusals in five days and stopped no write.**
Seen: an adopter's record: 388 of 455 (88%) with no git token in the command.
Fix: Air makes the worktree and starts claude in it, without `--worktree`. Commit 5b85a5a
(air-8gj). Air's own fence, added in its place, refused only writes to the session's scratchpad
(6 of 6 at an adopter; zero here) and was removed on 2026-09-28 (commit 21731f0, air-w18h);
roles.md carries the sentence instead.
Do not: re-add a path fence without a recorded write outside a worktree.

**The hook joined `digest_dir` onto the Bash tool's cwd, so a `cd` into a subdirectory refused a close whose digest existed.**
Seen: an adopter's worker, three times, 2026-09-06.
Fix: the worktree root is resolved once. Commit bb48888 (air-1r6).

**An untracked digest counted as proof.**
Seen: 2026-09-06.
Fix: the gate distinguishes Missing, Untracked and Tracked. Commit 9024929 (air-ahlf).

**The lease check missed a leased command followed by `2>&1`.**
Seen: 0.4.6 live trial, 2026-09-26, D4.
Fix: redirections are stripped before matching. SURFACE `lease-sees-redirects`.

**The peer warning dated nothing, so a live edit and a fortnight-old one read the same.**
Seen: 2026-09-06.
Fix: `is also journaled by w3 (14 d ago)`. Commit 1654748 (air-et0o).

**A recorded edit never expired.**
Seen: an adopter: a dead registration held 481 files, another 158; warnings cited edits 18 h old
and long landed.
Fix: a holding ends once the file is clean in the holder's worktree and unchanged against main
(`crates/cli/src/cmd/holdings.rs:121,135` `ended`, `expire`). Commit e77495c (air-84lz).

**A closed bead's claim row survived, so conditions kept firing on a landed bead.**
Seen: 2026-08-29; the reconcile only cleared it when bd answered inside its budget, which under
load it did not.
Fix: the claim is released on close. First by PostToolUse (commit 80f96b2, air-8p4); since
2026-09-26 by `air close` releasing the claims it closes.

**`bd_calls` on an event line was the process's lifetime total, restamped on every line.**
Seen: 2026-09-05: poll ticks summed to 570,989 calls on a day whose real total was 1,661.
Fix: each line carries its own share. Commit ff7b8fe (air-bp0).

## bd and Dolt

**Closing ten landed beads took minutes.**
Seen: 2026-08-22, this repo; bd costs about 1.4 s per process whatever it is asked (embedded
Dolt open, serialised on a lock).
Fix: one `bd close` process for every id, one ledger transaction; `bd_ms`/`bd_calls` on event
lines. Commit a17c9fc (air-869); test `ten_closes_are_one_bd_process_and_carry_bd_ms`.
Do not: add a bd process per id.

**`bd show` with many ids costs per id, and a flat budget timed out.**
Seen: 2026-08-22: 17 ids 19-21 s (air-7kp); 2026-09-06: 14 ids 21.27 s against 1.94 s for one,
so a landing carrying many beads timed out (air-fzv).
Fix: budgets scale by id count from one function; 60 s + 5 s per id since 2026-09-25. Commits
7437c10, 384f72c (air-8lj8).

**Under load bd took 20 s, so the coordinator's channel got nothing when the fleet was busiest.**
Seen: 2026-08-21.
Fix: `air status` wraps every bd call in `bd_try` (`status.rs:2552`); after a timeout the counts
come from `bd_cache` and one line says they are stale. Commit 713e411 (air-19u).

**A grandchild holding the pipe open turned a 2 s budget into a 25 s wait.**
Seen: 2026-08-21, in the full test run.
Fix: drain threads are not joined after a timed-out bd is killed. Commit 008fa85 (air-19u).

**A child writing more than 64 KB blocked against `wait_timeout` and was killed as a hang.**
Seen: 2026-08-20: `git status` on a busy worktree, `air status --json` on a full ledger.
Fix: stdout and stderr are drained on threads. Commit 5e11c7d.

**A claim timeout read as a denial, and hand retries lost the bead to a peer.**
Seen: an adopter, 2026-08-31, three hand retries.
Fix: `air claim` retries once on a timeout only (`crates/cli/src/cmd/claim.rs:183`
`retry_once`); mechanism `claim-retry`. Commit be4f866 (air-gsj).

**A prefix claim was recorded under the prefix, and the next reconcile released it while the work continued.**
Seen: 2026-09-06.
Fix: the resolved id is recorded; only "bd knows it and no longer holds it" releases, and a
timed-out `show` releases nothing. Commit cd796fe (air-x1ha).
Do not: release claims on absence of evidence.

**`air reclaim` on a live lease exited 0 with a count of 0.**
Seen: 2026-09-26, bd 1.3.0 (`bd reclaim --id` behaves so).
Fix: `air reclaim` names the expiry and exits non-zero (mechanism `reclaim-lease-live`).
Do not: pass `--force`.

**`bd ready` and `bd list --status` were capped by bd's defaults of 100 and 50.**
Seen: an adopter started a round with 88 ready beads.
Fix: both calls pass `-n 0`. Commit a4830c0 (air-06k7).

**The ready count said 2 claimable when both were epics, and a worker could claim an epic.**
Seen: an adopter, 2026-09-05.
Fix: `ready_cache::split` (`crates/cli/src/cmd/ready_cache.rs:82`); `air claim` refuses an epic.
Commit 43e7bdf (air-f10).

**`air claim` took a bead with an open blocker.**
Seen: an adopter, 0.4.10 round; the worker noticed from the acceptance and released it.
Fix: the claim refuses, naming the blocker and its status (`crates/bd/src/lib.rs:160`
`open_blockers`). Commit 62dca08 (air-g7hr); test `claim_refuses_a_bead_with_an_open_blocker`.

**A bead blocked by its own ancestor can never become ready and reads like any queued bead.**
Seen: 2026-09-06, on bd 1.2.2 two of nine routes plant the edge (`bd create --graph`,
`--parent X --deps <ancestor>`).
Fix: `air status` names it with the `bd dep remove` that clears it. Commit 960e1c1 (air-btz).

**Epics whose children were all closed were listed "ready to decompose" on every run.**
Seen: an adopter and this repo, three epics each.
Fix: after a landing `air land` closes epics with children all closed, reading `bd epic status`
and filtering out childless ones. Commit e137f0f (air-92o0, on main at 94dfe20); test
`a_landing_closes_a_finished_epic_and_not_a_childless_one`.

**bd started its own empty Dolt server on the project's port while Air's was down.**
Seen: 0.4.6 to 0.4.7 live trials, 2026-09-26.
Fix: `dolt.auto-start: false` at init; `BEADS_DOLT_AUTO_START=0` on every session
(`crates/cli/src/cmd/launch.rs:343`); `air bd-server up` and status count a server as up only
when it serves this project's data, and assign a port when the file is missing. Commit 6c0cff8.
Do not: let bd auto-start a server.

**With bd in server mode, every bd command failed while the server was down, and the server was started by hand.**
Seen: 2026-09-26 (owner).
Fix: `air bd-server up`, the launchers and `air fleet up` start it; the coordinator's channel
restarts it and says so once (`crates/cli/src/cmd/bd_server.rs:583` `keep_alive`). Commit
3dbdc18; mechanisms `bd-server-start`, `bd-server-keep-alive`.

**A scratch `air init --write` left a `<project>-dolt` tmux session and a server on the owner's tmux.**
Seen: 2026-09-28 ~21:30Z, this repo.
Fix: the Dolt server is a plain background process in its own process group, pid in
`.air/dolt/server.pid`. Commit d69d889 (air-8iu7).

**bd 1.2.0 and 1.2.1 were published by accident, and 1.2.1 migrated the schema so 1.2.2 could not open it.**
Seen: 2026-08-17 (design.md §11).
Fix: `air doctor` pins the version (`crates/cli/src/cmd/doctor.rs:14` `BD_PINNED`, now 1.3.0).

## Install and releases

**This round moved five things under an adopter, and nothing told it.**
Seen: 2026-08-22.
Fix: `install::SURFACE`, diffed against `.air/installed.json`. Commit 03c41c1 (air-6g1).

**`air install --write` never updated a hook matcher a repo already had.**
Seen: 2026-08-29.
Fix: a stale matcher is rewritten. Commit 9aa4701 (air-q07).

**An adopter's hooks ran 0.2.18 for days on a record saying 0.1.0 / surface 2.**
Seen: 2026-09-06.
Fix: `air doctor` and `air status` print the lag and the unread-notice count. Commit a54f46c
(air-d61).

**Two binaries with different behaviour reported the same version string.**
Seen: 2026-09-06; a round's numbers were quoted from the running binary and the tree
interchangeably for twelve hours (CLAUDE.md).
Fix: `air --version` prints the build commit and surface version. Commit 40f2ad5 (air-dwq5).

**Air had no release concept: `0.0.1` since the first commit, and a downgrade quietly stopped noticing.**
Seen: 2026-08-29 (CLAUDE.md, air-w9d).
Fix: forward-only install on a monotonic surface version; `make release` refuses a dirty tree,
runs `air release-check` and tags. Commits 4fad84f (air-w9d), da11ad0 (air-9ee).

**Nineteen releases and five row collisions in one day.**
Seen: 2026-09-05/06.
Fix: one release row per round; `air release-check` refuses until the notice count and version
agree. Commit 91ca605 (air-mir).

**Twelve surface changes shipped in one round with no notice.**
Seen: 2026-09-06: a sweep of 66 landings found 12 gaps (one in six); 3 of 12 roles commits lacked
coverage (air-bh6n).
Fix: the notices were written and the sweep is a round-end duty (CLAUDE.md). Commits 2eaa03f
(air-wt1v), 2b6a03f.

**A session launched while `air install`'s output was uncommitted ran without hooks, channel or skills.**
Seen: an adopter's re-audit, 2026-09-25.
Fix: the launchers refuse, naming the files (`crates/cli/src/cmd/launch.rs:1088`); mechanism
`launch-install-refusal`. Commit e24760e (air-rr98).

**An adopter kept `air-phase-transitions`, which contradicted the roles text.**
Seen: 2026-09-25.
Fix: `air install --write` removes retired `air-*` skills (`RETIRED_SKILLS`). Commit e24760e.

**In a pinned repo every session's bare `air` ran the 0.2.19 on PATH.**
Seen: 0.4.0 live trial, 2026-09-25; the Bash tool's shell re-orders PATH.
Fix: any other `air` re-executes the pin (`crates/cli/src/cmd/delegate.rs`); mechanism
`pin-delegation`. Commit 7649aac (air-qyrm).

**`--print` rewrote `.air/roles.md` and wrote task files.**
Seen: an adopter's re-audit, 2026-09-25.
Fix: `--print` writes nothing. Commit e24760e.

**`air adopter-check` skipped silently for a whole round, including on the sweep's own verify.**
Seen: 2026-09-06.
Fix: `"adopters": true` declared with no list is a refusal. Commit fe6d8d3 (air-jsz).

**`air fleet up` said it started the fleet when every launch was refused.**
Seen: 2026-09-28 (air-eeuf).
Fix: each failed launch is named and the event reads `fleet / incomplete`. Commit 95b7d72.

## Tests and verify

**Tests that spawn `air` inherited the launcher's `AIR_ROLE`, so verify was red from every fleet session and nothing could land.**
Seen: 2026-09-28, found by the lane: every batch, including main's own tree, red on `mcp_stdio`.
Fix: one helper strips `AIR_*` and `BEADS_ACTOR` from every spawn
(`crates/cli/tests/common/mod.rs:17` `command`). Commit 4312544 (air-8o59).
Do not: call `Command::new` directly in `crates/cli/tests`.

**A test asserted a fixed 3 s bar and went red under load.**
Seen: 2026-09-28, 3.24 s with three sessions verifying; a second fixed bar in the same test
turned the lane's batch red at 762cfdc.
Fix: every timing assertion is relative to the stall it injects. Commits 5503319, 78cd62a
(air-8gnk).
Do not: assert a fixed number of seconds.

**A `bd_server` test read a neighbour's listener as a running server.**
Seen: 2026-09-28, red twice in a row at one commit, green on the third run.
Fix: `start` checks for the database before probing the port, and the test holds a listener on
its port. Commit 687335d (air-df53).

**Fixtures held a copy of the clock: seven tests went red with no code change and main stayed red six days.**
Seen: 2026-08-23 to 2026-08-29.
Fix: fixtures carry today's rules; `air doctor` reported dated rules (commit e0bec4d, air-24e)
until both fallbacks and the report were deleted (commit ef3f73b, air-jsee).
Do not: write a test whose verdict depends on the wall clock.

**A probe hard-coded 30 min against a threshold of 5.**
Seen: 2026-08-29, within the hour of the clock-copy fix.
Fix: probes read the threshold they test. Commit a0ecd20 (air-jc0).

**`air selftest --json` emitted a stray line, so `--prove` reported all 82 mutations broken.**
Seen: 2026-09-06.
Fix: `--json` is only the array; 80 proven, 3 vacuous, 1 broken after. Commit c4c2be0
(air-e21v).

**Two declared mutations had been broken for days, and four flaky probes made others look vacuous.**
Seen: 2026-09-06: at load 145, eight concurrent selftests failed 10 times in 8 runs.
Fix: mutations re-anchored; the four budgets in probes made generous; at load 311, 106 PASS each
and zero FAIL. Commit b2426f7 (air-g7e).

**A test asserted the fixture's `AIR_PROJECT` only while the ambient one was unset, and hid a bug that blocked every landing.**
Seen: 2026-08-22.
Fix: `project_from(env, cwd)` (`crates/cli/src/cmd/hook.rs:1016`) tested without touching the
environment; the sweep runs with every launcher variable set. Commit 4dc5732 (air-7ah).

**A blocking `read_line` hung the MCP test for minutes instead of failing at its deadline.**
Seen: 2026-09-26.
Fix: a reader thread with `recv_timeout`. Commit 82596c9.

**A verify chained after a conflicting `git merge` wrote a red at that sha.**
Seen: 2026-09-06, two in this repo the same night.
Fix: `air record` refuses over unmerged paths (`crates/cli/src/git.rs:250` `unmerged_files`),
writes no row and exits 2. Commit 2452d0d (air-88av).
Do not: use `MERGE_HEAD` as the signal, or refuse a merely dirty tree.

**A verify killed by signal read as red, and a green/kill pair read as flaky.**
Seen: 2026-09-05.
Fix: exits 137 and 143 are no verdict (`crates/ledger/src/verify.rs:87` `KILLED_EXITS`).
Commit b81f17b (air-ppm).

**A backgrounded verify reported exit 0, and `make verify` silently skipped a suite.**
Seen: an adopter, 2026-08-21 (evidence.md F9).
Fix: `air record` refuses a backgrounded run and records command, duration, bytes and dirty
tree. Commit 8cbe879.

**`suspicious` fired on 14 good greens in one day, and `flaky-at-head` blamed a test for a missing build directory.**
Seen: an adopter, 0.4.10 round; the 0.4.6 trial had already hit a silent precheck flagged
suspicious.
Fix: both labels deleted; the green and red counts at the commit are stated. Commit 99d0abd
(air-qlx3); mechanisms.rs deleted list.
Do not: label a run; state its counts.

**Red run logs were pruned to the newest 20 while messages still pointed at them.**
Seen: 5 of 7 red-batch logs deleted within a day.
Fix: `KEEP_LOGS` deleted; nothing prunes `.air/logs`. Commit a4830c0 (air-06k7).

**CLI integration tests took 24 s.**
Seen: 2026-08-21.
Fix: fake bd written once per binary, `AIR_BD_BIN` pointed at nothing where bd is not under
test, `rev-parse` memoised; under 10 s. Commit 5ca8737 (air-4vu).

**`make trial` did not turn on the verification lane.**
Seen: 0.4.1 live trial, 2026-09-25.
Fix: commit 6111ebe.

**A selftest probe's synthetic hook inherited `CLAUDE_PID` and registered a worker as a live session owned by the claude running the suite.**
Seen: 2026-09-29, the lane's batch red at 2a4ffa8: the probe "env reaches the hook" went
BLOCKED because `air worker w` then read worker w as running.
Fix: `air_command` (`crates/cli/src/cmd/selftest.rs:81`) and the tests' `common::command`
(`crates/cli/tests/common/mod.rs:22`) clear `CLAUDE_PID` with the `AIR_*` variables. Commit
b5f287c (air-4n5t).
Do not: let a spawned probe or test inherit a variable a hook reads as its session's identity.

**The bd-server probe and the tmux launch test waited a fixed 10 s for a freshly written executable, and went red under fleet load.**
Seen: 2026-09-28, worker-2 at 7789c160 with three sessions verifying: the keep-alive probe
failed "red SILENT / green BLOCKED" and passed on two reruns; the tmux test failed once; 2 of 3
whole-file runs red at load 11. At 22:48Z `syspolicyd` sat at 412% CPU checking new
executables, so a first exec took 62-165 s on failing runs against 13 s passing.
Fix: the keep-alive probe drives `keep_alive` through its seams with no real server, port or
fake dolt; the init test raises the start wait through `AIR_BD_START_WAIT_MS`
(`crates/cli/src/cmd/bd_server.rs:44` `start_wait`); the tmux test polls up to 60 s and prints
its wait; the MCP stdio reply wait is 60 s. Commits bb40cfa, 1b8946f (air-1fg7); 17 of 17 runs
passed afterwards at loads 17 to 85, in up to 20.1 s. SURFACE `bd-start-wait-override`.
Do not: put a wall-clock bar on the first exec of a file a test just wrote; macOS checks it
before it runs.

**Three flow defects in one day were each a lifecycle transition nobody had written down, and each was judged in isolation.**
Seen: 2026-09-28: the cut deadlock (air-tml4), the stranded closed bead (air-voeb) and the
landing race (air-5ynn); the owner: "we don't know what the effects of our decisions are going
to be".
Fix: `docs/design.md` §6 opens with the states x events table, every row citing the function
it was read from; `crates/cli/src/cmd/lifecycle.rs` drives those same functions
(`batch_ready_rule`, `land_first`, `cover`, `handover_verdict`, `red_alone`) through every
interleaving of commit, cut, verify, land, close, merge and a move of main for two workers and
a lane, and fails on any reachable state that cannot still land and close every bead. Commit
9963e31 (air-k52n); test `every_reachable_state_can_still_land_and_close_every_bead` (86,895
states in 2.96 s, debug build); shown failing on main at d23ef60f, before air-tml4: 9,718 of
38,602 reachable states stuck, the first reached by `[Commit(0), Cut, Verify(true), MainMoves]`.
Do not: add a transition, rule or notice from the model without an observed incident; a hole
is answered first by removing the refusal that makes it (owner, 2026-09-28).

**After a release emptied `[Unreleased]`, the round's first changelog fragment turned `make verify` red on every branch.**
Seen: 2026-09-29, this repo, right after the 0.4.11 header was written: `this_repos_fragments_assemble`
failed with "changelog: no ### Added heading under [Unreleased]" (worker-1's capture).
Fix: the assembler adds a heading `[Unreleased]` lacks, in the standard order
(`scripts/changelog-assemble.sh:70`). Commit 5972971 (with air-zese).
Do not: make a fragment's fold depend on a heading a release may have removed.

## Performance and load

**Five sessions polling every few seconds kept three or four `lsof` scans at 40-50% CPU each.**
Seen: 2026-09-28 23:05Z, load 38 on 16 cores; the readers budget hit 47.6 s at an adopter under
load 500+. `air status` ran a median 191 git subprocesses per run plus one `lsof`, and the
coordinator's poll ran it every 5 s.
Fix: the channel poll never runs `lsof`; it reads a person's last reading while under 30 s old
(`crates/cli/src/cmd/readers.rs:123,137` `CACHE_SECS`, `cached`). Commit fcdc8e2 (air-czbg).
Do not: spawn a process listing from the poll path.

**The channel poll shelled out to bd every tick.**
Seen: 2026-08-29: 5,674 bd calls and 2.26 h a day waiting, for about 45 pushes.
Fix: `BdUse::CachedFor` (see the channel section). Commit 3eacecd (air-cmn).

**`review-waiting` cost one bd process per status for a state that no longer existed.**
Seen: last fired 2026-08-22, silent five recorded days.
Fix: deleted with its whole input chain; 1,430 ms off a 3,905 ms median status. Commit eb834ed
(air-okc).

**A subagent stopping ran the Stop nudge's `bd ready`: 241 processes in one day.**
Seen: an adopter, 2026-08-30.
Fix: `SubagentStop` runs no nudge and marks nothing idle; status looks up every reconciled claim
in one `bd show`. Commit ff7b8fe (air-bp0).

**A flat 20 s MCP tool budget killed `air close` whatever its own bd budget.**
Seen: 2026-09-25.
Fix: a tool's budget is derived from its command's (`crates/cli/src/cmd/mcp.rs:545`
`tool_budget`), and a timeout kills the whole process group. Commit 30b0e57 (air-se4n).

**Eleven budgets were guesses, and each hit read as a defect.**
Seen: an adopter under load 500+: the 4 s process-listing budget hit at 47.6 s, the Stop hook
overran its 5 s cap, `air status` took 30 s.
Fix: every budget names its measurement; `air audit` prints p50 and max beside the budget and
`max x 10`; a hit reads `over budget`. Commit fd9e910 (air-onqi).
Do not: pick a budget without naming the measurement it came from.

**Workers were frozen 7-20 minutes after every bead while their precheck read the tree.**
Seen: an adopter's worker logged it as a mistake and re-ran; the owner kept prechecks
(2026-09-28).
Fix: the cost is measured: `precheck p50 Ns, wait after p50 Ns` per worker in `air status`
(`verify_runs.resumed_at`, schema v26). Commit 9acd8e9 (air-1pu5).

**bd was called from the poll and the hook paths at 0.7 to 1.4 s a call.**
Seen: 2026-08-21 (bd-facts.md).
Fix: no hook calls bd; the Stop nudge reads `.air/ready.json` written by commands that already
paid for a `bd ready`. Commit 8f061e6 (air-09i).

**Three workers and the lane each verifying the same Rust tree put the machine at load 40.**
Seen: 2026-09-28, this repo. A shared `CARGO_TARGET_DIR` was tried first and rejected: cargo
built another worktree's code under the one target dir (capture 01M3N4F902SVT1BR2149JDR153).
Fix: `.claude/air.json` sets `"verify_lane": true`; the lane verifies once per batch and
workers verify nothing. Commit 2cd5e37 (air-hxws part 2); every later bead of the round closed
on the lane's batch green (air-be6d, air-gcjp, air-4n5t, air-fd8p, air-4l5w).
Do not: share a cargo target dir across worktrees.

## Left out

Found while reading, but with no evidence of being solved, or solved by something other than a
landed Air change. The coordinator can check each.

- Shared `CARGO_TARGET_DIR` for fleet builds (air-hxws part 1): rejected, cargo ran another
  worktree's code under a shared target dir.
- `air status` running 191 git calls per run, the poll running gather at most every 30 s, and
  per-worktree caching keyed by head and index mtime (air-czbg comment, items 2 and 3): only the
  `lsof` half landed.
- Seven stale `agent-*` worktrees doubling status cost: removed by hand on 2026-09-28; no
  mechanism skips a worktree with no merge base.
- Batch-ready behind main (SURFACE `batch-ready-behind-main`, 2026-09-25): from an adopter's
  protocol, no incident count recorded.
- The Stop nudge offering beads to the lane (SURFACE `nudge-skips-the-lane`): no incident
  recorded.
- `bd ready` excluding custom statuses (upstream #5831), so `awaiting_review` never showed: gone
  because Air stopped using the status, not because anything was fixed.
- A stray `awaiting_review` releasing a claim (air-3eu, commit e450a5d): the state no longer
  exists, so the mechanism's current presence was not checked.
- Precheck hold re-read before a `batch-held` notice (0.4.9, SURFACE `precheck-hold-rereads`):
  superseded by deleting `batch-held`.
- `air status --json` saying why a branch cannot land (air-72t7): no incident count.
- One predicate deciding `landable` (commit 1c2758b, 2026-08-29): no count recorded.
- `air install --write` refusing while `.air/` is not ignored (air-6di): advice made a refusal,
  no incident recorded.
- `bd init --skip-agents --skip-hooks` (commit 586b4b0): "its commands conflict with Air's", no
  incident recorded.
- The stale `bd prime` hook report (air-b5k): no incident recorded.
- Workers denied `AskUserQuestion` (air-bm3): an owner ruling of 2026-08-30, no incident.
- `make verify` killing a hung test step after 300 s, raised to 900 s (commits 01b571d,
  f9f6071): no incident detail beyond the subject line.
- `air fleet stop` (air-1vri.1): an owner request; "one SendMessage per session" is not a
  recorded failure.
- A lease refusal for `air lease take X && <cmd>` (CHANGELOG): no incident detail.
- The `dated-cutoffs` doctor report: superseded by deleting the fallbacks (air-jsee); listed
  above under tests.
- `stuck`, `cross-project-fence`, `gone-with-claim` history and `owner-decision-waiting`:
  deletions on zero firings or owner rulings, recorded in mechanisms.rs, not problems solved.
- Recording the failing step on red runs, batch wait ages, red-batch policy, tree changed during
  a verify: open in `docs/design.md` §10.
