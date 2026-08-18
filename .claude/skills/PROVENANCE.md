# Skill provenance

Append-only log of where each skill under `.claude/skills/` came from and how it was adapted.
Source SHAs are `git -C <repo> rev-parse --short HEAD` at port time. Full detail lives in each
skill's own `## Provenance` section.

| Skill | Source | Source HEAD | Ported | Class | Adaptations |
|---|---|---|---|---|---|
| review | `adopter/.claude/skills/review/` (49 files) + Beads block from `another-project/.claude/skills/review/SKILL.md` | adopter `f2ca891`; another-project | 2026-08-18 | ADAPT | Beads block grafted (bd/air, PR); repo constraints → Air hook path/ledger/beads boundary; fp-review delegation removed; rubric + 48 references verbatim. Lean variant noted: `another-project/.claude/skills/review` (`cce8fc0`), not ported. |
| architecture-review | `adopter/.claude/skills/architecture-review/` (6 files) | adopter `f2ca891` | 2026-08-18 | ADOPT | SIMP → plan doc (`docs/plans/`); otherwise verbatim. |
| system-design-review | `adopter/.claude/skills/system-design-review/` (7 files) | adopter `f2ca891` | 2026-08-18 | ADOPT | SIMP → plan doc (`docs/plans/`); otherwise verbatim. |
| systems-architecture | `adopter/.claude/skills/systems-architecture/` (3 files) | adopter `f2ca891` | 2026-08-18 | ADOPT | Verbatim. |
| rfc-review | `another-project/.claude/skills/rfc-review/` (6 files) | another-project | 2026-08-18 | ADAPT | De-GitLab'd (glab/MR → docs/plans path + gh); cross-refs → CLAUDE.md/decisions/research/bd/ledger; Asana + sibling-repo + BPF lines retargeted; domain golden examples kept as calibration. |
| writing-style | `another-project/.claude/skills/writing-style/` (SKILL.md + `references/slop-patterns.md`); em-dash rule + "What to cut" from `another-project/.claude/skills/writing-style/SKILL.md` | another-project; another-project | 2026-08-18 | ADAPT | MR→PR; Asana/GitLab dropped; hard em-dash ban merged (heading `NNNN — Title` and pre-existing docs exempt); Air examples (ledger, hooks, `bd`); slop-patterns.md verbatim. |
| deslopify | `adopter/.claude/skills/deslopify/SKILL.md` | adopter `f2ca891` | 2026-08-18 | ADOPT | Verbatim; catalog/style paths now `../writing-style/...` (adopter's pointed at a missing file); em-dash check aligned with the ban. |
| writing-docs | `adopter/.claude/skills/writing-docs/SKILL.md` + `another-project/.claude/skills/writing-docs/SKILL.md` (per-section bullets); `adopter/docs/rules/writing.md` → `docs/rules/writing.md` | adopter `f2ca891`; another-project | 2026-08-18 | ADAPT | Merged; retargeted to `docs/plans/NNNN-*.md`, `docs/research/`, `docs/decisions.md`, index in `docs/README.md`/`CLAUDE.md`; `docs/adr/` marked not-yet-existing; source-trail rule added; Air examples; em dashes removed. |
| planning-doc-hub | `adopter/.claude/skills/planning-doc-hub/SKILL.md` | adopter `f2ca891` | 2026-08-18 | ADOPT | Verbatim + one paragraph mapping the set onto Air's `docs/` layout. |
| mermaid-diagrams | `adopter/.claude/skills/mermaid-diagrams/SKILL.md` | adopter `f2ca891` | 2026-08-18 | ADOPT | Verbatim; description "ADRs" → "plans, decision records". |
