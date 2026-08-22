# Air

Air runs several AI coding agents on one repository at the same time. One Rust binary, `air`. It works beside [beads](https://github.com/steveyegge/beads) (the issue tracker) and Claude Code (the agents).

It keeps the facts agents would otherwise carry in chat: who is editing which file, which commit passed verification, who holds a shared resource.

## Five rules

1. **Do less.** Every rule names the failure it prevents and when it gets removed.
2. **Facts, not procedures.** Air tells agents what they cannot know. It does not tell them how to work.
3. **One refusal.** No hand-over without a recorded pass at that commit.
4. **A human is always in the loop.** Every agent is a terminal you can watch and type into.
5. **The project comes first.** Air earns features in real rounds of work, or loses them.

## What it does

- **Records:** verification results per commit, file edits, claims, leases, captures, and every decision it made.
- **Answers:** `air status`, `air holdings`, `air handover`.
- **Refuses:** review or close without a recorded pass at HEAD that contains `main`.
- **Informs** the coordinator when something needs a person: a stuck worker, a hand-over waiting.
- **Launches:** `air coordinator` and `air worker <name> --task "..."`.
- **Sets up:** `air init` on any repository.

## Your project vs Air

Your project decides what verification runs, the code rules, what to build, and how changes land. Air records the results and tells you who is waiting on whom.

Air will not choose work, review code, or make an agent smarter. It makes several agents and one person cost fewer messages and fewer false passes.

## Quick start

```bash
cargo install --path crates/cli
cd <your repo>
air init --prefix <beads prefix> --write
air record verify -- <your verification command>
air coordinator
air worker w1 --task "<a complete task>"
```

Adopting Air in an existing process: `docs/rules/adopting-air.md`.

## Day to day

- **Worker:** `air claim <id>` → work → `git merge main` → `air record verify -- <cmd>` → `air handover` → next bead. Found something? `air capture "<one line>"`.
- **Coordinator:** `air status`, act on channel events, `air inbox` → `bd create` → `air triage`. Keep the ready list full. Launch workers.
- **Owner:** `air status` in any terminal. `air inbox --owner` for what waits on you.

## Status

Early. In use on two repositories, including this one. Decisions: `docs/decisions.md`. Roadmap: `docs/plans/0005-roadmap.md`.

## Map

`crates/ledger`, `crates/hooks`, `crates/bd`, `crates/cli`. Agents read `docs/rules/roles.md`. Evidence lives in `docs/research/`.
