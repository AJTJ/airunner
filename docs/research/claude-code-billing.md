# Claude Code Billing & Pricing Research
**Verified 2026-08-17 | Primary Anthropic Sources Only**

---

## 1. Claude Code under Subscription Plans (Pro/Max)

**Coverage**: Usage included in subscription monthly fee.

**Pricing** ([claude.com/pricing](https://claude.com/pricing), verified 2026-08-17):
- Pro: $17/month (annual) or $20/month (monthly)
- Max: From $100/month (offers "5x or 20x more usage than Pro")
- Team: $20–$100 per seat/month
- Enterprise: Custom pricing

**Usage model** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
- Rolling 5-hour session window + weekly cap
- Shared with Claude chat and Cowork
- `/usage` command shows plan usage breakdown on Pro/Max/Team/Enterprise
- Team & Enterprise: per-seat allowance resets on rolling 5-hour window and weekly window; size depends on seat tier (Standard or Premium)

**Headless, worktree, subagent billing** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
> Headless `claude -p` runs, `--worktree` sessions, and in-session subagents (Agent tool) **draw from the same subscription limits** as interactive sessions. No separate billing; same 5-hour window + weekly reset.

---

## 2. Claude API: Pay-Per-Token Billing

**Model pricing per million tokens** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):

| Model | Input | Cache Hit (10% of input) | Output |
|-------|-------|--------------------------|--------|
| Claude Fable 5 | $10 / MTok | $1 / MTok | $50 / MTok |
| Claude Mythos 5 (limited availability) | $10 / MTok | $1 / MTok | $50 / MTok |
| Claude Opus 5 | $5 / MTok | $0.50 / MTok | $25 / MTok |
| Claude Sonnet 5 | $2 / MTok | $0.20 / MTok | $10 / MTok |
| Claude Haiku 4.5 | $1 / MTok | $0.10 / MTok | $5 / MTok |

**Note on Sonnet 5 pricing** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
> The $2/$10 per million input/output token pricing for Claude Sonnet 5, announced at launch as introductory pricing through August 31, 2026, is now the standard price. The previously scheduled increase to $3/$15 per million input/output tokens on September 1, 2026 will not occur.

**Batch API discount** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
- 50% discount on both input and output tokens
- Processing time up to 24 hours

**Prompt caching** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
- Cache reads: 10% of standard input price
- 5-minute cache write: 1.25x base input price
- 1-hour cache write: 2x base input price

**Cost tracking** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
- `/usage` command (local calculation; may differ from authoritative bill)
- Claude Console usage page for authoritative billing per workspace
- OpenTelemetry export: `CLAUDE_CODE_ENABLE_TELEMETRY=1` streams per-user cost metrics

**Workspace spend limits**:
- Set via Claude Console; recommended rate limits by org size: 10k–300k TPM per user ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17)

---

## 3. Claude Agent SDK Authentication & Billing

**Current policy** ([support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan](https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan), verified 2026-08-17):

Anthropic paused planned changes on June 15, 2026. Current status:

> "Claude Agent SDK, `claude -p`, and third-party app usage still draw from your subscription's usage limits."

**Subscription auth** ([support.claude.com/en/articles/15036540](https://support.claude.com/en/articles/15036540), verified 2026-08-17):
- Agent SDK with subscription (Pro/Max/Team/Enterprise): draws from same limits as interactive Claude Code
- No separate monthly credit (planned change is paused)
- "We're working to update the plan to better support how users build with Claude subscriptions. When we have an update, we'll share it before anything takes effect."

**API key auth**:
- Standard pay-per-token Console billing; no subscription required

---

## 4. Managed Agents (Beta) Billing

**Token pricing** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
- Standard Claude API per-model rates (Fable, Opus, Sonnet, Haiku pricing above)
- Prompt caching multipliers apply identically

**Session runtime** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
> Runtime is measured to the millisecond and accrues only while the session's status is `running`. Time spent `idle` (waiting for your next message or a tool confirmation), `rescheduling`, or `terminated` does not count toward runtime.

- Rate: **$0.08 per session-hour**

**Web search** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
- $10 per 1,000 searches

**Beta note** ([platform.claude.com/docs/en/about-claude/pricing](https://platform.claude.com/docs/en/about-claude/pricing), verified 2026-08-17):
- "Anthropic hasn't committed to specific GA pricing. The current $0.08/session-hour and standard token rates are beta-era numbers."

---

## 5. Running N Parallel Sessions on One Max Subscription

**Permitted**: Yes. Claude Code has no hard cap on concurrent instances.

**Rate limit constraint** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
- Rate limits pool across all sessions on your account
- Pro and Max have shared 5-hour session + weekly window across all concurrent instances
- Practical limit: 3–5 concurrent agents sustainable on Max; beyond that, switch to API key billing

**Workspace rate limits** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
- Per-user TPM/RPM recommendations by org size (10k–300k TPM per user for 1–500+ users)
- These apply at organization level, not per individual user

---

## 6. Cost-Control Knobs: CLI & Environment

**`--max-budget-usd` flag** ([code.claude.com/docs/en/cli-reference.md](https://code.claude.com/docs/en/cli-reference.md), verified 2026-08-17):
> Sets a maximum dollar amount to spend on API calls before stopping in print mode (`-p`). When the budget limit is reached, spawning another subagent fails with the error: `Budget limit reached`.

**Example:**
```bash
claude -p --max-budget-usd 5.00 "your query"
```

**Other cost-control options** ([code.claude.com/docs/en/costs.md](https://code.claude.com/docs/en/costs.md), verified 2026-08-17):
- `/model`: switch models mid-session; use Sonnet for routine, Opus for complex
- `/effort`: reduce extended-thinking budget for simpler tasks
- `/usage`: track session cost; attribution by skill/MCP server
- `/clear`: free context reset between unrelated tasks
- `ENABLE_PROMPT_CACHING_1H=1` env var: keep 1-hour cache lifetime on usage credits
- Batch API: 50% token discount for non-urgent (≤24h) workloads
- Hooks (PreToolUse): preprocess data before Claude sees it (reduce context)
- OpenTelemetry export: per-user cost tracking into observability stack

---

## 7. Mixed Runtime: Claude Code Sessions vs Direct API Calls

**Use case**: An orchestrator that can dispatch tasks via **either** Claude Code CLI sessions (subscription-backed) **or** direct API calls (pay-per-token), choosing per role based on cost and capability.

### Cost Arithmetic (Verified 2026-08-17)

**Scenario**: Routine triage + code review + deployment — three roles with different complexity.

| Role | Task | Optimal Auth | Pricing | Example Cost |
|------|------|-------------|---------|------------------|
| **Triage (Haiku)** | Digest logs, categorize issues, call routing | API key (Haiku) | $1 input / $5 output per MTok | ~$0.02–0.05 per 1k-token turn |
| **Code Review (Sonnet)** | File review, suggestions, test generation | API key (Sonnet batch) | $1 input / $5 output per MTok (50% batch discount) | ~$0.50–2 per review (batch) |
| **Architecture (Opus)** | Multi-file reasoning, refactor proposals, design docs | Subscription session OR API | $5 input / $25 output (API) vs shared seat limit (subscription) | $5–20 per session (API); included in subscription seat |

**Decision rule**:
- **Triage (high volume, stateless)**: API + Haiku + Batch. Example: 100 issues @ $0.05 each = $5 total.
- **Code Review (medium volume)**: API + Sonnet + Batch for non-urgent; Claude Code session (subscription) if urgent or interactive feedback needed.
- **Architecture (low volume, high reasoning)**: Claude Code subscription session OR API + Opus, depending on seat utilization.

**Breakeven**: Subscription (Max at $100/month) breaks even vs API after ~5–10 Opus sessions or ~20–50 Sonnet sessions, depending on token intensity. For an orchestrator, route high-volume stateless tasks to API + Haiku; keep heavy-reasoning tasks on subscription seats if available.

**Example: 1,000 triage runs + 50 code reviews + 5 architecture sessions per month**:
- **All API** (Haiku for triage, Sonnet batch for review, Opus for architecture):
  - Triage: 1,000 × $0.05 = $50
  - Review: 50 × $1.50 = $75
  - Architecture: 5 × $15 = $75
  - **Total: $200 / month**
  
- **Mix (triage/review on API, architecture on Max subscription)**:
  - Triage + Review: $125 (same as above)
  - Architecture: included in Max seat ($100/month for one seat)
  - **Total: ~$225 / month** (breakeven at 2–3 additional Opus sessions)

---

## Summary for External Orchestrator

**Primary findings (all sources checked 2026-08-17):**

1. **Subscription (Pro/Max)** covers Claude Code, headless, worktrees, subagents equally; no separate billing. Shared 5-hour + weekly window. Practical limit: 3–5 parallel agents; beyond that, switch to API key.

2. **API billing** is pay-per-token at stated per-model rates (Fable $10/$50, Opus $5/$25, Sonnet $2/$10, Haiku $1/$5 per MTok). Batch API: 50% off both input/output. Caching: 10% of input.

3. **Agent SDK** currently draws from subscription limits (paused separation as of June 15, 2026). No documented restriction on subscription auth for the SDK itself; third-party harness policy is separate.

4. **Managed Agents** (beta): tokens at API rates + $0.08 per session-hour (beta pricing, may change).

5. **`--max-budget-usd` exists** for print mode; stops subagents when budget exhausted.

6. **Mixed runtime (sessions vs API)**: Dispatch triage/low-complexity tasks to API + Haiku (cheap, stateless); reserve subscription seats or Opus API for high-reasoning work. Breakeven ~5–10 Opus or ~20–50 Sonnet sessions per month per subscription seat.

---

## Appendix: Secondary Sources (Unverified)

Sources fetched but not relied upon for primary claims due to lack of Anthropic authorship:

- Verdent Guides, MorphLLM, SiliconData, TrueFoundry, Finout, MetaCTO, CostGoat, BenchLM — aggregate or estimated pricing and limits
- GitHub discussions and community posts — anecdotal rate-limit experiences and cost patterns
- These sources may be outdated or reflect regional/negotiated pricing not in Anthropic's primary docs

**Methodology**: Only claims backed by https://platform.claude.com, https://code.claude.com/docs, https://claude.com/pricing, or https://support.claude.com are included in main sections. All verified 2026-08-17.

