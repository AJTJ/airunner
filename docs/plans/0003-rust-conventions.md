# 0003 — Rust conventions for the Air workspace

Status: **decided** 2026-08-18 (owner; `docs/decisions.md` 2026-08-18, "Plan 0003"). §1 to §4,
§6 and §7a are decided as recorded below; §5 is decided for the speed rule and layout, with the
`rstest` choice standing as the recommendation. Sections keep their original analysis so the
reasoning stays readable; each carries a **Decision** line citing `docs/decisions.md`. Written
while porting the Group 2 skills
(a private skills inventory), which prescribe conventions the ported skills now
reference conditionally.
Sources: `the adopter's .claude/skills/rust-safety/SKILL.md` (the adopter `f2ca891`),
`another-project/.claude/skills/rust-safety/SKILL.md` + `rust-toolchain.toml` +
`Cargo.toml` (another-project), `another-project/.claude/skills/rust-safety/SKILL.md` (no git),
`another-project/CLAUDE.md:69-76` + `Cargo.toml [workspace.lints]` + `rustfmt.toml` +
`rust-toolchain.toml` (another-project), `the adopter's backend/Cargo.toml` +
`rust-toolchain.toml`, `another-project/.claude/skills/writing-rust-tests/SKILL.md` +
`backend/Cargo.toml` (another-project), `another-project/Cargo.toml` +
`.claude/settings.json` (another-project), `docs/plans/0001-first-slice.md §10`.

## 0a. Decisions summary (2026-08-18) and toolchain policy

Per `docs/decisions.md` 2026-08-18:

| § | Decision |
|---|---|
| 1 Errors | `thiserror` enums per crate; `error-stack` noted as a possible later addition; no `anyhow` in libs. |
| 2 Lints | The small deny-list below; pedantic off. Warn first, deny in CI once green. |
| 3 Formatting | Default `rustfmt`, no `rustfmt.toml`; hook + `cargo fmt --check`. |
| 4 Clippy/CI | `cargo clippy --all-targets --all-features -- -D warnings` in the verify command, not a hook. |
| 5 Tests | **Tests must always be optimized for speed** (rule, `CLAUDE.md`). Inline `#[cfg(test)]` for logic; at most one `tests/` binary per crate; rusqlite `:memory:`; `cargo nextest`. `rstest` stands as the recommendation. |
| 6 Edition/toolchain | `edition = "2024"`; `rust-toolchain.toml` `channel = "stable"` + `rustfmt`/`clippy`; explicit `rust-version`, bumped deliberately. |
| 7a Crates/latency | Tick 0315 §6 proposals accepted as the starting point, to be measured in M0. |

**Toolchain policy: latest stable; nightly only when a wanted feature justifies it, recorded
in `docs/decisions.md`.** "Latest stable" means the current stable channel (rustup `stable`),
not a pinned old version; `rust-version` is the floor and is raised deliberately. A move to
nightly (for example `-Zthreads`, cargo-script, a sanitizer) needs a decisions entry naming
the feature and its tracking issue, and `rust-toolchain.toml` pinned to a dated nightly. The
`rust-toolchain` skill (`.claude/skills/rust-toolchain/`) is the catalogue of what stable and
nightly currently offer, with sources.

## 0. Why this is a plan and not a CLAUDE.md line

The inventory (§E Group 2, item 1) proposed recording `error-stack` + `thiserror` as *the*
decision in `CLAUDE.md`. That would be inheriting a choice made for kernel-ring-buffer and
audio-DSP codebases without asking whether it fits a <300 ms hook binary. Per `CLAUDE.md`
("Only build what makes sense", "Prefer using or borrowing … research must show why not"),
each convention below is listed with what the sources do, what Air's constraints add, and a
recommendation — for the owner to accept, reject, or defer. The ported skills (`rust-safety`,
`writing-rust-tests`, `test`) are written so that either outcome leaves them correct.

## 1. Error handling crate(s) — DECIDED

**What the sources do.** Every `rust-safety` copy (the adopter, another-project, another-project, another-project)
uses plain `thiserror` enums, one per crate. All but another-project layer `error-stack` 0.6 `Report`
on top (location capture, `change_context`, `ensure!`), and say "no `anyhow` in new code; only
at boundaries like `main()`" (`another-project/CLAUDE.md:69`). another-project uses `thiserror` alone.

**Air's constraints.** Four small crates; the CLI edge prints one line to a human or one JSON
object to a hook; hooks fail open. Location tracking is nice in stack traces but the operator
never sees a backtrace from a hook — they see the message. Dependency weight matters little
here; API surface consistency across `ledger`/`bd`/`hooks`/`cli` matters more.

**Options.**

| | `thiserror` only | `thiserror` + `error-stack` | `thiserror` in libs + `anyhow` at `main()` |
|---|---|---|---|
| Typed, matchable errors in libs | yes | yes | yes |
| Context chains / caller location | manual (`#[source]`, message) | automatic (`Report`) | `anyhow::Context` at the edge only |
| Testing errors | `matches!(err, E::V{..})` | `report.current_context()` (Report has no `Display`) | same as libs |
| Extra dep / learning curve | none | one crate, one idiom | one crate, well known |
| Matches source skills | another-project | the adopter, another-project, another-project | none exactly (all forbid anyhow in *new* code) |

**Decision (`docs/decisions.md` 2026-08-18):** `thiserror` enums per crate; `error-stack` a
possible later addition; `anyhow` out of library crates. `rust-safety` v1.1.0 encodes this.

**Recommendation (as written before the decision).** `thiserror` enums in every library crate is the invariant
in all sources — adopt that. Whether to add `error-stack` is the open part: adopt it *if* the
CLI's `--json` error output or `air doctor` wants context chains without hand-threading them;
otherwise start with `thiserror` alone and an explicit `From` per boundary, and revisit when
the first "which layer failed?" bug appears (named pain first). `anyhow` stays out of library
crates either way.

## 2. Lints — DECIDED

**Sources.** another-project pins a large `[workspace.lints.clippy]` table (191 active `deny` entries and ~576 more commented out, `Cargo.toml:225+`, e.g.
`cast_*`, `arc_with_non_send_sync`; `arithmetic_side_effects` is among the commented-out ones;
run via `just clippy-strict` = `-D warnings`, `another-project/CLAUDE.md:14`); the adopter, another-project, another-project, another-project have no lint tables and
rely on default `cargo clippy`. The `rust-safety` skill's rules (no `unwrap`/`expect`/`panic`
in runtime code, no `as` casts, no unchecked arithmetic) are prose, not lints, in every source.

**Recommendation.** Enforce the skill's prose as a *small* `[workspace.lints]` table rather than
importing another-project's 191-entry list — a check beats a rule (`CLAUDE.md`, "replace one prose rule
with one enforced check"): `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`,
`clippy::unreachable`, `clippy::indexing_slicing`, `clippy::as_conversions` (or the
`cast_*` family), `clippy::arithmetic_side_effects` — `warn` first, `deny` in CI once the
crates are green; `#[allow]` inside `#[cfg(test)]`. `rust::unsafe_code = "forbid"` (Air needs
none — `rust-safety` "Unsafe Code").

**Decision (`docs/decisions.md` 2026-08-18):** this small deny-list; pedantic off. Warn first,
deny in CI once green. Encode as `[workspace.lints]` + `[lints] workspace = true` per crate
(syntax: `rust-toolchain/references/cargo-and-tooling.md`).

## 3. Formatting — DECIDED

**Sources.** another-project's `rustfmt.toml`: `group_imports = "StdExternalCrate"`,
`imports_granularity = "Crate"` (both nightly-only options — accepted silently on stable, only
applied on nightly). Others: default `rustfmt`. another-project enforces formatting with a
`PostToolUse` hook (`cargo fmt --all` on every `.rs` write) — now copied to Air's
`.claude/settings.json`.

**Recommendation.** Default `rustfmt` on stable, no `rustfmt.toml`, formatting enforced by the
hook plus `cargo fmt --check` in the verify step.

**Decision (`docs/decisions.md` 2026-08-18):** default rustfmt; no `rustfmt.toml`, so another-project's
two nightly-only import options are not added.

## 4. Clippy invocation and CI — DECIDED

**Sources.** No source runs clippy in a hook; the adopter's `Makefile` verify targets run
`cargo test` (with named binaries, `Makefile:619-640`) but not clippy. another-project's `just clippy-strict` runs
clippy with `-D warnings` (`another-project/CLAUDE.md:14,131`).

**Recommendation.** `cargo clippy --all-targets --all-features -- -D warnings` as part of
whatever `air record`'s verify command becomes; not a hook (too slow for <300 ms).

**Decision (`docs/decisions.md` 2026-08-18, with §2):** as recommended.

## 5. Test layout and stack — DECIDED (speed rule); `rstest` = standing recommendation

**Sources.** Two lineages (a private skills inventory): the adopter's another-project use
`rstest` + `rstest_reuse`, inline `#[cfg(test)]` modules, a `prelude_test.rs` re-export;
another-project uses a single integration binary (`backend/tests/main.rs` + `tests/common/`)
with `#[sqlx::test]` per-test databases and *no* inline test modules. `cargo-nextest` is
"preferred, not required" in the adopter.

**Recommendation.** Inline `#[cfg(test)]` modules for pure logic (the ledger's derivations,
the hand-over verdict), one `tests/` integration binary per crate only for things that need
the built CLI (`assert_cmd`-style end-to-end: `air claim` → `air handover`), rusqlite
`:memory:` fixtures per `writing-rust-tests`. `rstest`(+`rstest_reuse`) is the standing
recommendation; the ported skill assumes it because every source does.

**Decision (`docs/decisions.md` 2026-08-18):** tests must always be optimized for speed; they
run constantly, so per-test cost is a first-class constraint (in-memory SQLite, temp git
repos, no sleeps, no network, parallel-safe; runner `cargo nextest`). Layout as recommended.
Proposed budgets (to measure in M0): unit < 50 ms, integration < 500 ms, suite < 10 s. Detail
and sources: `writing-rust-tests` "Speed is a requirement" and
`rust-toolchain/references/fast-tests.md`.

## 6. Edition and MSRV — DECIDED

**Sources.** another-project: `edition = "2024"`, `rust-version = "1.89.0"`, toolchain pinned
`1.89.0`. another-project: `edition = "2024"`, toolchain `1.93.0` with `rustfmt`+`clippy` components.
another-project: `edition = "2024"`. The adopter: `edition = "2021"`, toolchain `channel = "stable"`
("never silently require nightly"). another-project: `edition = "2021"`.

**Recommendation.** `edition = "2024"`; `rust-toolchain.toml` with `channel = "stable"` and
`components = ["rustfmt", "clippy"]` (the adopter's reasoning); set `rust-version` to whatever
stable is at first commit and only bump it deliberately. Note: `rusqlite` bundled and `gix`
both have their own MSRVs — check at workspace creation.

**Decision (`docs/decisions.md` 2026-08-18):** edition 2024; `channel = "stable"` (latest
stable, not an exact pin); explicit `rust-version`; nightly only when a wanted feature
justifies it, recorded in `docs/decisions.md` (see §0a toolchain policy). §7a's
`rust-version = "1.88"` floor is the starting value.

## 7. Cargo workspace shape — from plan 0001 §10, restated

Not open here: `crates/{ledger,bd,hooks,cli}`, `rusqlite` (bundled), `clap`, `serde`/`serde_json`,
`gix` (read-only) + shell `git`, `tracing`, `tokio` only where needed, no async in hooks. Listed so
the conventions above are read against the actual dependency set.

## 7a. Crates, settings, hook latency budget — DECIDED as starting point (2026-08-18)

**Decision (`docs/decisions.md` 2026-08-18):** accepted as the starting point, to be measured in
M0. The tick
[`2026-08-18-0315-rust-crates-latency.md`](../research/verification/ticks/2026-08-18-0315-rust-crates-latency.md)
verifies today's crate versions/licences/MSRVs, measures `git`/`bd` spawn cost on this Mac (`git`
≈ 10–25 ms per call; `bd ready --json` ≈ 1.1 s — never on a hook path), and proposes a per-hook budget
(p50 ≈ 10–25 ms, p99 ≤ 150 ms, 250 ms watchdog) plus eleven candidate decisions (§6 there): rusqlite
`bundled` + WAL/`busy_timeout`/`synchronous=NORMAL`, per-invocation open (no daemon in M0), shell `git`
first with `gix` deferred to measured need, `wait-timeout` not tokio, `panic="unwind"` + `catch_unwind`
for fail-open, `rust-version = "1.88"` floor. Owner accepts/rejects alongside §8.

## 8. What the owner needed to say (answered 2026-08-18; kept for the record)

Answers are in §0a and `docs/decisions.md` 2026-08-18. Remaining follow-ups: encode §2 in
`Cargo.toml` at workspace creation; measure §5 budgets and §7a in M0.

1. Error crate: `thiserror` only / `thiserror`+`error-stack` / defer until first named pain.
2. Lints: the small deny-list in §2 — warn or deny; pedantic on or off.
3. Formatting: default rustfmt (yes/no to another-project's two import options).
4. Tests: `rstest` yes/no; inline modules + per-crate integration binary (§5).
5. Edition 2024 + `stable` toolchain + explicit `rust-version` (§6).
6. The crate/settings/latency proposals in §7a (tick 0315 §6) — accept, reject, or defer to M0 measurement.

Done 2026-08-18: decisions recorded in `docs/decisions.md`; `rust-safety` v1.1.0 and
`writing-rust-tests` v1.1.0 dropped their "if adopted" hedges; `rust-toolchain` skill added.
Open: encode §2 in `Cargo.toml` when the workspace is created.
