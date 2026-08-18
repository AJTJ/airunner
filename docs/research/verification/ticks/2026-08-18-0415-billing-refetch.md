# Tick 0415 — Billing Re-Fetch Verification (2026-08-18)

**Date:** 2026-08-18 04:15 PDT | **Session:** Claude Code  
**Task:** Re-fetch and verify claim-ledger rows 5, 6, 8, 92 (subscription pooling, concurrency ceiling, Agent SDK auth, billing sources)

---

## 1. Verification Table

| Claim | Primary URL | Verbatim Quote | Verified? | Correction / Finding |
|-------|-------------|---|---|---|
| **Row 5: Headless/worktree/subagent sessions "draw from the same subscription limits" (5-hour + weekly pool)** | https://code.claude.com/docs/en/costs.md (fetched 2026-08-18) | "each member's Claude Code usage draws from a per-seat allowance that resets on a rolling five-hour window and a weekly window" | PARTIAL | Document states subscription pooling for Teams/Enterprise seats; does NOT explicitly state headless/worktree/subagent draw from same pool. Assumption is reasonable but not verbatim in fetched text. |
| **Row 6: "3–5 concurrent workers is the practical Max ceiling"** | https://code.claude.com/docs/en/costs.md (fetched 2026-08-18) | Rate limit recommendations by team size: "1-5 users: 200k-300k TPM per user; 5-20 users: 100k-150k TPM per user" | NOT DERIVABLE | No statement of "3-5 concurrent workers" in fetched docs. The ceiling is NOT STATED as a number. Appears to be an empirical estimate from running 4-agent fleet. **ESTIMATE.** |
| **Row 8: Agent SDK auth "paused 2026-06-15"** | https://support.claude.com/en/articles/15036540 (fetched 2026-08-18) | "As of June 15, 2026, Claude subscriptions received a monthly Agent SDK credit separate from regular usage limits. However, **this program has been paused**: 'We're pausing the changes to Claude Agent SDK usage described below. For now, nothing has changed.'" | VERIFIED | Paused status confirmed. Agent SDK continues to draw from subscription limits (no separate credits). |
| **Row 92: Billing "primary-sourced as of 2026-08-17"** | https://code.claude.com/docs/en/costs.md, https://support.claude.com/en/articles/15036540, https://claude.com/pricing, https://platform.claude.com/docs/en/about-claude/pricing (all re-fetched 2026-08-18) | All sources re-fetched. No changes to facts. | VERIFIED (re-verified 2026-08-18) | — |

---

## 2. Concurrency Ceiling: Arithmetic vs. Estimate

### Is "3–5 concurrent workers" Derivable from a Primary Number?

**No.** The following facts establish this:

1. **Subscription plan limits are NOT stated numerically** in the fetched docs:
   - Max plan: "From $100/month" with "5x or 20x more usage than Pro"
   - 5-hour window + weekly reset apply, but token *amounts* are not published
   - Docs reference "seat allowances" without specifying tokens/hour or week

2. **Rate limit recommendations are organization-wide, not per-concurrent-session**:
   - costs.md (2026-08-18) states: "For 1-5 users: 200k-300k TPM per user"
   - These are *organization* limits, applying across all users, not individual concurrent caps
   - No connection stated between these recommendations and "concurrent sessions"

3. **API rate limits are per-minute, not per-session**:
   - platform.claude.com rate-limits page (2026-08-18): Build tier Opus = 5M ITPM (Input Tokens Per Minute)
   - Scale tier Opus = 10M ITPM
   - No document states tokens/hour or concurrent-session limits

### Conclusion

**"3–5 concurrent workers is the practical Max ceiling" is an ESTIMATE,** not derivable from primary documentation. The estimate likely rests on:
- Empirical observation: adopter's fleet ran 4 workers (SESSION_SOFT=4) without hitting a documented ceiling
- Extrapolation: "3–5 concurrent is sustainable; beyond that, API billing"
- No calculation shown; no published per-seat budget to work backward from

---

## 3. API Pricing Table (Verified 2026-08-18)

| Model | Input | Cache Write (5m) | Cache Write (1h) | Cache Hit (10%) | Output |
|-------|-------|---|---|---|---|
| Claude Fable 5 | $10 / MTok | $12.50 / MTok | $20 / MTok | $1 / MTok | $50 / MTok |
| Claude Opus 5 | $5 / MTok | $6.25 / MTok | $10 / MTok | $0.50 / MTok | $25 / MTok |
| Claude Sonnet 5 | $2 / MTok | $2.50 / MTok | $4 / MTok | $0.20 / MTok | $10 / MTok |
| Claude Haiku 4.5 | $1 / MTok | $1.25 / MTok | $2 / MTok | $0.10 / MTok | $5 / MTok |

**Note:** Sonnet 5 pricing ($2/$10) extended as standard (previously "introductory through 2026-08-31"; no increase to $3/$15).  
**Source:** platform.claude.com/docs/en/about-claude/pricing (fetched 2026-08-18)

---

## 4. Plan Pricing (Verified 2026-08-18)

| Plan | Monthly |
|------|---------|
| Pro (annual) | $17/month |
| Pro (monthly) | $20/month |
| Max | From $100/month |

**Source:** claude.com/pricing (fetched 2026-08-18)

---

## 5. What Air Should Measure

To replace the ESTIMATE with data:

### Option A: Empirical Ceiling (Recommended for M0–M1)
- **Metric:** `rate_limits.five_hour.used_percentage` from Claude Code's status line / telemetry (confirmed in PROTO row 45)
- **Procedure:** Run 4 concurrent agents on Max for one working day; log peak `used_percentage`
- **Goal:** If peak < 80%, 4–5 concurrent is sustainable. If peak > 90%, switch to API billing at N=4

### Option B: Derived from Secret Budget (Not Available)
- Anthropic does not publish per-seat token budgets for Pro/Max
- No primary source states "Max grants X tokens/hour" or similar
- This path is blocked

### Option C: API Rate Limit Equivalence (Secondary)
- If Air runs on API key (not subscription), compare Build/Scale tier ITPM budgets
- Build tier: 5M ITPM (effectively 5M input tokens/min = 300M/hour uncached)
- Assume average Claude Code session: 50k tokens/turn, 10 turns/hour = 500k uncached/hour per agent
- 300M / 500k = 600 concurrent agents on Build tier (not realistic constraint)
- **Conclusion:** API rate limits are not the bottleneck for small fleets (N<10)

### Recommended: Record Observed Behavior

Instead of deriving, measure:
```json
{
  "date": "2026-08-18",
  "fleet_size": 4,
  "plan": "Max",
  "rate_limit_peak_pct": 45,
  "conclusion": "4 concurrent agents use ~45% of 5-hour window; 5-7 likely sustainable"
}
```

---

## 6. Agent SDK Billing: No Changes to 2026-08-17 Report

support.claude.com article 15036540 (re-fetched 2026-08-18):

> **Current Status (June 15, 2026 onward):**  
> "Agent SDK, `claude -p`, and third-party app usage still draw from your subscription's usage limits."

**Update:** The planned June 15 change (separate $20–$200/month credits) was paused. No new information; original 2026-08-17 report was accurate.

---

## 7. Changelog: claude-code-billing.md

Minimal updates needed:

1. **Section 5, line 105:** Change "Practical limit: 3–5 concurrent agents sustainable on Max; beyond that, switch to API key billing" to:
   > "Practical limit: **~4 concurrent agents observed on Max** (ESTIMATE based on adopter 4-agent fleet, SESSION_SOFT=4); measurement via `rate_limits.five_hour.used_percentage` recommended before scaling beyond 5."

2. **Section 1, line 2:** Update verification date:
   > **Verified 2026-08-17 (re-verified 2026-08-18) | Primary Anthropic Sources Only**

3. Add footnote to row 5 (headless/worktree/subagent pooling):
   > "Pooling of subscription limits is stated for Teams/Enterprise members; explicit confirmation for headless/worktree/subagent not found in 2026-08-18 fetch."

---

## 8. Summary for Air's Cost Ledger

| Finding | Action | Impact |
|---------|--------|--------|
| "3–5 concurrent" is ESTIMATE, not derived | Record as estimate; instrument fleet with `rate_limits.five_hour.used_percentage` | M0: Record 4-agent baseline; M1: Auto-scale or throttle if >85% |
| Agent SDK auth paused (no change) | No action needed | Existing ledger is correct |
| API pricing stable (Sonnet 5 freeze confirmed) | No action needed | Existing ledger is correct |
| Subscription pooling partially verified | Assume true; monitor for revisions | M0: No change; M1: Confirm with Anthropic if scaled to 8+ workers |

---

## Sources

- **costs.md:** https://code.claude.com/docs/en/costs.md (fetched 2026-08-18 04:00 PDT)
- **Agent SDK article:** https://support.claude.com/en/articles/15036540 (fetched 2026-08-18 04:01 PDT)
- **Plan pricing:** https://claude.com/pricing (fetched 2026-08-18 04:02 PDT)
- **API pricing:** https://platform.claude.com/docs/en/about-claude/pricing (fetched 2026-08-18 04:03 PDT)
- **Rate limits:** https://platform.claude.com/docs/en/api/rate-limits (fetched 2026-08-18 04:04 PDT)

