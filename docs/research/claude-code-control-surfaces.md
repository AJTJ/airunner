# Claude Code Control Surfaces: Comprehensive Reference

**Checked 2026-08-17** | **Claude Code v2.1.234+**

This document provides authoritative control surfaces available for external Rust orchestration runtimes to supervise Claude Code sessions across git worktrees, enforce process workflows (claim → work → verify → review → land → close), and track cost/progress mechanically.

---

## 1. Hooks: Lifecycle Control & Event Interception

**Reference:** https://code.claude.com/docs/en/hooks.md

### Hook Events (Comprehensive List)

| Event | Trigger | Can Block | Modifies | Notes |
|-------|---------|-----------|----------|-------|
| **SessionStart** | Session begins or resumes | Yes | None | Runs once per session |
| **SessionEnd** | Session terminates | No | None | 1.5s budget shared across all SessionEnd hooks |
| **Setup** | Init-only or maintenance mode | Yes | None | With `--init-only` or maintenance matcher |
| **UserPromptSubmit** | User submits prompt | Yes | `updatedInput` | Can modify the user's prompt before processing |
| **PreToolUse** | Before any tool executes | **Yes (exit 2)** | `permissionDecision`, `updatedInput` | **Primary enforcement point** |
| **PostToolUse** | After tool succeeds | No | Context only | Cannot block; informational |
| **PostToolUseFailure** | Tool execution fails | No | Context only | Informational |
| **PostToolBatch** | Parallel tool batch resolves | Yes | `blockAgentic` | Can stop agentic loop |
| **Stop** | Claude finishes responding | No | None | Informational |
| **StopFailure** | Turn ends due to API error | No | None | Informational |
| **PermissionRequest** | Permission prompt needed | Yes | `decision` (deny/allow/escalate) | Permission interception |
| **PermissionDenied** | Permission denied | Yes | `retry: true` | Can retry a denied tool |
| **SubagentStart** | Subagent spawned | Yes | None | Can block subagent launch |
| **SubagentStop** | Subagent finishes | No | None | Informational |
| **TaskCreated** | Task spawned | Yes | None | Task control |
| **TaskCompleted** | Task finishes | No | None | Informational |
| **Notification**, **MessageDisplay** | UI events | No | None | Informational |
| **PreCompact**, **PostCompact** | Context compaction | Yes (PreCompact) | None | Context lifecycle |
| **CwdChanged**, **DirectoryAdded** | Working directory changes | No | None | Informational |
| **FileChanged** | File written | No | None | Informational |
| **WorktreeCreate**, **WorktreeRemove** | Git worktree lifecycle | Yes | None | **For external VCS** |
| **ConfigChange**, **InstructionsLoaded** | Config/CLAUDE.md loaded | No | None | Informational |
| **Elicitation**, **ElicitationResult** | Interactive prompt/response | Yes/No | Varies | UI interaction hooks |
| **UserPromptExpansion** | Slash command expansion | Yes | `updatedPrompt` | Can rewrite `/` expansions |

### Hook Handler Types

```json
{
  "type": "command",      // Shell script (stdin/stdout)
  "type": "http",         // POST to URL (request/response body)
  "type": "mcp_tool",     // Call MCP server tool
  "type": "prompt",       // Send to Claude model
  "type": "agent"         // Spawn subagent for decision-making
}
```

### JSON Input Schema (All Events)

```json
{
  "session_id": "uuid-string",
  "prompt_id": "uuid-string",
  "transcript_path": "/home/user/.claude/projects/..../session-id.jsonl",
  "cwd": "/current/working/directory",
  "permission_mode": "default|plan|acceptEdits|auto|dontAsk|bypassPermissions",
  "hook_event_name": "PreToolUse|SessionStart|etc",
  "agent_id": "optional-subagent-name",
  "agent_type": "Explore|Plan|custom-agent-name",
  
  // Tool-specific (PreToolUse only):
  "tool_name": "Bash|Edit|Read|Write|Agent|etc",
  "tool_input": {
    "command": "...",     // Bash command text
    "path": "...",        // File path
    "skill": "skill-name" // For Skill tool calls
  }
}
```

### JSON Output Schema (Hook Response)

```json
{
  "continue": true|false,                          // false stops processing
  "stopReason": "message if continue=false",       // Shown to user
  "systemMessage": "warning/notice text",          // Always shown
  "terminalSequence": "\x07",                      // Bell, notify
  
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    
    // Permission decisions (PreToolUse, PermissionRequest)
    "permissionDecision": "deny|allow|escalate",
    "permissionDecisionReason": "Human-readable reason",
    
    // Input modification (PreToolUse, UserPromptSubmit)
    "updatedInput": {
      "command": "rewritten command",
      "path": "rewritten path"
    },
    
    // Prompt modification (UserPromptExpansion, UserPromptSubmit)
    "updatedPrompt": "rewritten user prompt",
    
    // Context enhancement (any event)
    "additionalContext": "text Claude sees",
    
    // Agentic loop control (PostToolBatch)
    "blockAgentic": true,
    
    // Retry denied tools (PermissionDenied)
    "retry": true
  }
}
```

### Blocking Mechanisms

- **Exit code 2**: **Blocks independently** — stops the action regardless of JSON output
- **JSON `continue: false`**: Stops processing; subject to schema validation
- **JSON `permissionDecision: "deny"`**: Blocks permission-gated actions (PreToolUse, PermissionRequest)
- **JSON `blockAgentic: true`**: Stops Claude's agentic loop after tool batch

### Hook Timeouts

| Type | Default | Context |
|------|---------|---------|
| command/http/mcp_tool | 600s | 30s for UserPromptSubmit |
| prompt | 30s | - |
| agent | 60s | - |
| SessionEnd (shared) | 1.5s total | Up to 60s aggregate |

### Matcher Syntax

```json
"matcher": "*",              // Match all (default)
"matcher": "Bash",           // Exact match
"matcher": "Bash|Edit|Write", // Alternation (pipe or comma)
"matcher": "Bash(rm *)",     // Tool + argument pattern
"matcher": "mcp__.*",        // Regex (anything not plain text)
"matcher": "mcp__plugin_my-plugin_db__query"  // MCP tool naming
```

### Configuration Locations & Precedence

| Scope | Path | Shared | When Loaded |
|-------|------|--------|------------|
| Managed | Server-deployed or `/Library/Application Support/ClaudeCode/` | Yes | At startup; per-org |
| User | `~/.claude/settings.json` | No | At startup |
| Project | `.claude/settings.json` | Yes (repo) | At startup; per-worktree |
| Local | `.claude/settings.local.json` | No (gitignored) | At startup |
| Plugin | `hooks/hooks.json` in plugin root | Yes (when enabled) | When plugin loads |
| Skill/Subagent frontmatter | Inline in `.claude/agents/*.md` | Yes | During skill/agent invocation |

**Hooks are watched for changes** — edits apply mid-session without restart.

### Environment Injection for Headless Sessions

Pass via:
1. `--settings '{"hooks": {...}}'` CLI flag (JSON)
2. `.claude/settings.json` in working directory
3. `~/.claude/settings.json` (user global)
4. Environment variable `CLAUDE_CONFIG_DIR=/path/to/dir` → reads from `$CLAUDE_CONFIG_DIR/settings.json`

**Cannot disable** managed policy hooks from user/project settings; only user can set `"disableAllHooks": true` locally.

---

## 2. Headless / Programmatic Driving

**Reference:** https://code.claude.com/docs/en/headless.md, https://code.claude.com/docs/en/cli-reference.md

### Non-Interactive Mode: `claude -p`

The `-p` flag runs Claude without interactive prompts. Combines with session, permission, tool, and output control flags.

### Critical CLI Flags for Orchestration

#### Session & Persistence

```bash
--continue, -c                # Resume most recent session in cwd
--resume SESSION_ID           # Resume by ID or name
--session-id UUID             # Explicit session UUID
--fork-session                # Create new session ID on resume
--no-session-persistence      # Don't save this run to disk
--init-only                   # Run Setup hooks, exit
--maintenance                 # Run Setup hooks (maintenance matcher)
```

#### Permissions & Access Control

```bash
--permission-mode MODE        # default|acceptEdits|plan|auto|dontAsk|bypassPermissions
--dangerously-skip-permissions # Alias for bypassPermissions (not recommended)
--allowedTools "Bash,Read,Edit" # Pre-approved tools (comma/space separated)
--disallowedTools "Bash(rm *)"  # Deny rules
--tools "Bash,Edit,Read"      # Restrict available tools (empty string = none)
--add-dir ../lib ../config    # Additional read-write directories
```

#### Output & Streaming

```bash
--output-format text|json|stream-json    # Response format
--input-format stream-json                # For streaming input
--verbose                                  # Verbose output
--include-partial-messages                # Include partial streaming events
--include-hook-events                     # Include hook lifecycle events
--forward-subagent-text                   # Emit subagent text/thinking
--json-schema '{schema}'                  # Validate output to schema
```

#### System Prompt & Configuration

```bash
--system-prompt "custom prompt"           # Replace default
--system-prompt-file /path                # Load from file
--append-system-prompt "additional text"  # Append to default
--append-system-prompt-file /path         # Append from file
--append-subagent-system-prompt "text"    # Append to all subagent prompts
--settings file.json|'{"key":"value"}'    # Load/inline settings
--setting-sources user,project,local      # Which sources to load
--safe-mode                               # Disable customizations
--bare                                    # Minimal mode (no hooks, skills, plugins)
```

#### Model & Cost Control

```bash
--model sonnet|opus|haiku|fable           # Model alias or full name
--effort low|medium|high|xhigh|max|ultracode
--max-budget-usd 5.00                     # Stop at dollar amount
--max-turns 10                            # Max agentic turns
--advisor opus                            # Enable advisor model
--fallback-model sonnet,haiku             # Fallback chain
```

#### Worktrees & Isolation

```bash
--worktree, -w BRANCH_NAME                # Create/use git worktree
--worktree "#1234"                        # Branch from PR/MR number
--tmux                                    # Create tmux session for worktree
```

#### Subagents & Agents

```bash
--agents '{"name": {...}}'                # Define custom subagents inline
--agent AGENT_NAME                        # Run as specific subagent
```

#### MCP & Tools

```bash
--mcp-config file.json|'{...}'            # Load MCP servers
--strict-mcp-config --mcp-config file.json # Only use specified servers
--plugin-dir ./my-plugin                  # Load plugin from directory
--plugin-url https://example.com/plugin.zip # Fetch plugin from URL
```

### Exit Codes

| Code | Meaning | Recovery |
|------|---------|----------|
| 0 | Success | Read result from stdout (or JSON output) |
| 1 | Failure | API error, missing auth, model not found, permissions denied, no input, invalid flags |
| 2 | Partial | Cost ceiling hit or auth failed before first run; results may be partial |
| 130 | Interrupted | SIGINT (Ctrl+C) |
| 143 | Terminated | SIGTERM (process killed); SessionEnd hooks still run |

### Result JSON Schema (`--output-format json`)

```json
{
  "result": "text response from Claude",
  "session_id": "uuid-string",
  "stop_reason": "max_tokens|end_turn|stop_sequence|agent_timeout|cost_limit_exceeded",
  "num_turns": 3,
  "duration_seconds": 45.2,
  "total_cost_usd": 0.042,
  "usage": {
    "input_tokens": 1200,
    "output_tokens": 450,
    "cache_creation_input_tokens": 500,
    "cache_read_input_tokens": 0
  },
  "model_breakdown": {
    "claude-sonnet-4-6": {
      "input_tokens": 1200,
      "output_tokens": 450,
      "cost_usd": 0.042
    }
  },
  "structured_output": {...},  // If --json-schema provided
  "transcript_path": "/home/user/.claude/projects/.../session-id.jsonl",
  "warnings": ["..."]
}
```

### Streaming JSON Events (`--output-format stream-json`)

Each line is a JSON event:

```json
{
  "type": "stream_event|system|result",
  "event": {
    "type": "message_start|content_block_start|content_block_delta|content_block_stop|message_delta|message_stop",
    "delta": {
      "type": "text_delta|thinking_delta",
      "text": "streamed text fragment"
    }
  },
  "session_id": "...",
  "index": 0
}
```

Special event types:
- `"type": "system"` with `"subtype": "init"` → Session metadata, plugins, MCP servers, model, tools
- `"type": "system"` with `"subtype": "api_retry"` → API retry event
- `"type": "system"` with `"subtype": "plugin_install"` → Plugin installation progress

### Detecting Agent Stop Reasons

Parse `stop_reason` field:
- `"end_turn"` → Claude voluntarily stopped (task complete)
- `"max_tokens"` → Ran out of output tokens
- `"agent_timeout"` → Exceeded `--max-turns` or internal timeout
- `"cost_limit_exceeded"` → Hit `--max-budget-usd`
- `"stop_sequence"` → Hit a stop sequence
- **Absence of result** → Error (read stdout/stderr for error message)

### Detecting Permission/Human Intervention Needed

- If exit code is 1 and stderr contains "permission", action was denied
- In `stream-json`, look for `"type": "system"` events with denial messages
- Cannot resume an interactive permission prompt in headless mode—plan ahead

---

## 3. Claude Agent SDK (Python/TypeScript)

**Reference:** https://code.claude.com/docs/en/agent-sdk/overview.md, https://code.claude.com/docs/en/agent-sdk/typescript.md, https://code.claude.com/docs/en/agent-sdk/python.md

### Language Support

- **TypeScript**: `@anthropic-ai/claude-agent-sdk` (npm)
- **Python**: `claude-agent-sdk` (pip)
- **Rust**: No native SDK; use CLI subprocess (`claude -p`) with `--output-format json` or Agent SDK + FFI

### Built-In Tools (No Implementation Required)

All SDK hosts get these tools automatically:

| Tool | Capabilities |
|------|--------------|
| **Read** | File reads, glob patterns, MIME type detection |
| **Write** | Create/truncate files, append mode |
| **Edit** | In-place edits with diff-style syntax |
| **Bash** | Shell command execution (with timeout control) |
| **Glob** | File pattern matching |
| **Grep** | Text search in files |
| **WebSearch** | Bing search (requires API key) |
| **WebFetch** | HTTP GET with caching |

### SDK Capabilities Beyond CLI

1. **In-Process Hooks**: Define hook handlers in code
   ```typescript
   agent.on('preToolUse', (input) => {
     if (input.tool_name === 'Bash') {
       // Block/modify decision
       return { permissionDecision: 'deny' };
     }
   });
   ```

2. **Tool Approval Callbacks**:
   ```python
   def can_use_tool(tool_name, input):
     return tool_name in ['Read', 'Write']
   agent = Agent(tool_approval_callback=can_use_tool)
   ```

3. **Streaming Events** (not available in CLI):
   ```typescript
   agent.run(prompt, { streaming: true })
     .on('message', (msg) => console.log(msg))
     .on('tool_use', (call) => /* handle */)
   ```

4. **Custom Tools**: Define tools beyond built-ins
   ```python
   @agent.tool()
   def my_database_query(sql: str) -> str:
     """Query the database"""
     return execute(sql)
   ```

5. **Budget & Turn Limits**:
   ```typescript
   const result = await agent.run(prompt, {
     maxTurns: 5,
     maxBudgetUsd: 2.00,
     model: 'claude-sonnet-4-6'
   });
   ```

6. **Session Management** (in-process):
   ```python
   session = agent.session(session_id='abc-123')
   result = await session.run("follow-up prompt")
   ```

7. **Streaming Cost Tracking**:
   ```typescript
   const usage = result.usage;  // { input_tokens, output_tokens, cost_usd }
   ```

### MCP Integration

```typescript
agent.connectMCP({
  type: 'stdio',
  command: 'python',
  args: ['-m', 'mcp_server_name']
});
```

MCP tools become available automatically; no explicit tool definition needed.

### Subagent Spawning

```python
# Built into agent.run()
result = await agent.run("Use a subagent to research this...")
# Agent automatically delegates
```

---

## 4. Permissions & Settings

**Reference:** https://code.claude.com/docs/en/permissions.md, https://code.claude.com/docs/en/settings.md, https://code.claude.com/docs/en/permission-modes.md

### Settings File Precedence (Highest to Lowest)

1. **Managed** (organization policy, cannot override)
2. **Command line flags** (`--settings`, `--permission-mode`)
3. **Local** `.claude/settings.local.json` (project-specific, gitignored)
4. **Project** `.claude/settings.json` (repo-committed)
5. **User** `~/.claude/settings.json`

**Exception**: Permission rules *merge* across scopes instead of override.

### Settings File Locations

| Scope | Path | Applies To |
|-------|------|-----------|
| Managed | `/Library/Application Support/ClaudeCode/` (macOS) | All org members |
| Managed | `/etc/claude-code/` (Linux/WSL) | All system users |
| Managed | `C:\Program Files\ClaudeCode\` (Windows) | All users |
| User | `~/.claude/settings.json` | All projects (this user) |
| Project | `.claude/settings.json` | This repo (all collaborators) |
| Local | `.claude/settings.local.json` | This repo (this user only) |

### Permission Modes

| Mode | What runs without asking | Use for |
|------|-------------------------|---------|
| **default** (Manual) | Read-only, approved rules | Interactive sessions with oversight |
| **acceptEdits** | File writes, common filesystem ops (`mkdir`, `touch`, `cp`) | Auto-approval of edits; commands still need approval |
| **plan** | Read-only exploration only | Planning before implementation |
| **auto** | Classifier reviews most actions | Development without manual prompts |
| **dontAsk** | Only explicit allow rules + read-only | Locked-down CI runs |
| **bypassPermissions** | Everything (dangerous) | Unattended automation; risky |

**Default for new sessions**: Manual on all plans (Pro starts in `auto`, requires opt-in).

### Permission Rule Syntax

```json
{
  "permissions": {
    "allow": [
      "Read",                          // All file reads
      "Read(./src)",                   // Reads under ./src only
      "Bash(git commit *)",            // Git commit commands (prefix match)
      "Bash(npm run test|npm run lint)", // Multiple commands (alternation)
      "Bash",                          // All bash (dangerous)
      "Edit",                          // All file edits
      "Write",                         // Create/truncate files
      "Bash(rm *)"                     // Destructive commands
    ],
    "deny": [
      "Bash(rm -rf /)",                // Block nuclear options
      "Read(.env)",                    // Hide secrets
      "Edit(.git)"                     // Protect git
    ],
    "ask": [                           // Explicit per-action prompts
      "Bash(curl *)"                   // Always ask before web requests
    ],
    "defaultMode": "acceptEdits"       // Start in this mode
  }
}
```

### Read-Only Commands (Never Need Permission)

- File reads (Read tool)
- Text search (Grep)
- Directory listings
- Git status, diff, log (not commit/push)
- Common filesystem queries

### Tool Name Prefixes

- `Bash(...)` — Shell commands with pattern
- `Edit(...)` — File edits with path patterns
- `Write(...)` — File creation with path
- `Read(...)` — File reads with path
- `Bash(git ...)` — Git commands
- `mcp__<server>__<tool>` — MCP tools
- `Agent(fork|worker|researcher)` — Subagent names

### Environment Variables (Settings File `env` Section)

```json
{
  "env": {
    "CLAUDE_CODE_ENABLE_TELEMETRY": "1",
    "OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:4317",
    "API_TIMEOUT_MS": "600000",
    "BASH_DEFAULT_TIMEOUT_MS": "120000"
  }
}
```

Only `EVAL_*` variables pass through to hooks. Managed settings can allow-list others.

### Injecting Settings for Headless Runs

```bash
# Via flag
claude -p "task" --settings '{"permissions":{"allow":["Bash","Read"]}}'

# Via file
claude -p "task" --settings /path/to/settings.json

# Via env variable
export CLAUDE_CONFIG_DIR=/custom/config
claude -p "task"  # Reads $CLAUDE_CONFIG_DIR/settings.json
```

---

## 5. Subagents, Skills, Plugins & MCP

**Reference:** https://code.claude.com/docs/en/sub-agents.md, https://code.claude.com/docs/en/workflows.md, https://code.claude.com/docs/en/skills.md, https://code.claude.com/docs/en/mcp.md

### Defining Subagents

**File**: `.claude/agents/<name>.md`

```markdown
---
name: code-reviewer
description: Reviews code for quality and security
tools: Read, Grep, Glob, Bash
model: sonnet
permissionMode: acceptEdits
maxTurns: 20
skills:
  - /security-checker
mcpServers:
  - my-database
memory: project
hooks:
  PreToolUse: [...]
background: true
---

You are a senior code reviewer...
```

**Discovery order** (highest priority first):
1. Managed settings (org-wide)
2. `--agents` CLI flag
3. `.claude/agents/` (project)
4. `~/.claude/agents/` (user home)
5. Plugin `agents/` directories

### Spawning Subagents

```bash
# Automatic delegation (based on description match)
"Use the code-reviewer agent to check auth.ts"

# Explicit @-mention
@"code-reviewer (agent)" look at this

# Session-wide
claude --agent code-reviewer

# CLI definition (one-off)
claude --agents '{"reviewer": {"description": "...", "prompt": "...", "tools": [...]}}'
```

### Skills (Reusable Prompts)

**File**: `.claude/skills/<name>.md` (or `.claude/skills/<name>.md` with `/skill-name` invocation)

```markdown
---
name: my-skill
description: What this skill does
tags: [refactoring, testing]
---

Instructions for Claude...
```

Invoked with `/my-skill` or auto-loaded if specified in subagent's `skills` field.

### Plugins (Bundled Skills + Agents + MCP + Hooks)

**Structure**:
```
my-plugin/
├── plugin.json
├── agents/
├── skills/
├── hooks/hooks.json
├── mcp/
│   ├── server1.py
│   └── server2.js
└── package.json (dependencies)
```

**Load via**:
- `--plugin-dir ./my-plugin`
- `--plugin-url https://example.com/plugin.zip`
- Marketplace (auto-enabled)
- `.claude/settings.json` `enabledPlugins` field

### MCP (Model Context Protocol) Servers

**Reference**: https://code.claude.com/docs/en/mcp.md

MCP servers add custom tools available to Claude. Tools appear as `mcp__<server>__<tool>`.

**Configuration**:

```json
{
  ".mcp.json": {
    "mcpServers": {
      "my-database": {
        "type": "stdio",
        "command": "python",
        "args": ["-m", "my_database_server"]
      },
      "github": {
        "type": "http",
        "url": "http://localhost:5000"
      }
    }
  }
}
```

**CLI flag**:
```bash
claude --mcp-config '{"mcpServers": {"db": {...}}}'
```

**Tool naming**:
- `mcp__<server>__<tool>` for stdio/http
- `mcp__plugin_<plugin>_<server>__<tool>` for plugin-bundled servers

### Workflows (Dynamic Agent Orchestration)

**Reference**: https://code.claude.com/docs/en/workflows.md

Workflows are JavaScript programs that orchestrate many subagents in parallel (fan-out/reduce pattern). Claude writes them; you save and reuse them.

**Trigger**:
```bash
ultracode: audit every route handler for missing auth checks
# or
/workflow my-audit  # If previously saved
```

**Saved location**:
- Project: `.claude/workflows/<name>.js`
- User: `~/.claude/workflows/<name>.js`

**Script structure**:
```javascript
export const meta = {
  name: 'audit-routes',
  description: '...'
};

const files = await agent('List all .ts files', {
  schema: { /* ... */ }
});

const audits = await pipeline(files, file =>
  agent(`Audit ${file}...`)
);

return audits.filter(Boolean);
```

**Concurrency limits**: 16 agents per run (CPU-bound), 1000 agents total per workflow.

### External Task Injection via Agents/Skills

**For a Rust runtime to inject task-specific context**:

1. **Write a skill per task** (`.claude/skills/task-name.md`):
   ```markdown
   ---
   name: task-verify-pr-123
   ---
   Verify PR 123 against this checklist:
   - [ ] Unit tests pass
   - [ ] No console.warn left
   - [ ] Accessibility checks
   ```
   
2. **Invoke programmatically**:
   ```bash
   claude -p "/task-verify-pr-123" --allowedTools "Bash,Read"
   ```

3. **Or define subagent per task**:
   ```bash
   claude --agents '{"pr_checker": {"description": "Verify PR 123", "prompt": "..."}}'
   claude -p "@pr_checker" --allowedTools "Bash,Read"
   ```

---

## 6. Sessions, Transcripts & Cost Tracking

**Reference:** https://code.claude.com/docs/en/sessions.md, https://code.claude.com/docs/en/costs.md

### Session Storage on Disk

**Location**: `~/.claude/projects/<PROJECT_HASH>/<SESSION_ID>.jsonl`

Where:
- `<PROJECT_HASH>` = working directory path with non-alphanumeric chars → `-` (capped 200 chars + hash suffix if longer)
- `<SESSION_ID>` = UUID
- Format = newline-delimited JSON (one entry per line)

**Retention**: Cleaned up automatically after 30 days (configurable via `cleanupPeriodDays` in settings).

**Configuration**:
```json
{
  "cleanupPeriodDays": 30,
  "env": {
    "CLAUDE_CONFIG_DIR": "/custom/path"  // Changes ~/.claude location
  }
}
```

**Disable persistence**:
```bash
claude -p "task" --no-session-persistence
# or
export CLAUDE_CODE_SKIP_PROMPT_HISTORY=1
```

### Transcript File Format

**Not a stable interface** — format changes between versions. Internal structure for Claude Code only.

**For monitoring**:
- Use `--output-format json` JSON result (stable)
- Use `/export` command (readable transcript)
- Parse `transcript_path` field from JSON output

### Cost Tracking (Headless)

From `--output-format json`:

```json
{
  "total_cost_usd": 0.042,
  "usage": {
    "input_tokens": 1200,
    "output_tokens": 450,
    "cache_creation_input_tokens": 500,
    "cache_read_input_tokens": 0
  },
  "model_breakdown": {
    "claude-sonnet-4-6": {
      "input_tokens": 1200,
      "output_tokens": 450,
      "cache_creation_input_tokens": 500,
      "cache_read_input_tokens": 0,
      "cost_usd": 0.042
    }
  }
}
```

**Note**: Client-side estimate at list rates; may differ from actual billing.

### Usage Reporting (Interactive)

```bash
/usage          # Shows token usage, session cost, model breakdown
/insights       # Generate usage patterns report (HTML)
/usage-credits  # Manage usage credits (subscription plans)
```

### OpenTelemetry Integration

**Reference**: https://code.claude.com/docs/en/monitoring-usage.md

Claude Code exports metrics to OTLP (OpenTelemetry Line Protocol). Configure:

```json
{
  "env": {
    "OTEL_EXPORTER_OTLP_ENDPOINT": "http://localhost:4317",
    "OTEL_EXPORTER_OTLP_HEADERS": "Authorization=Bearer <token>",
    "OTEL_METRICS_EXPORTER": "otlp"
  }
}
```

Exported metrics:
- `claude_code.session.tokens.input`
- `claude_code.session.tokens.output`
- `claude_code.session.cost.usd`
- `claude_code.tool.calls` (per-tool counters)
- `claude_code.subagent.spawned`

**Label attributes**:
- `session_id`
- `model`
- `tool_name`
- `user_id` (if available from auth)

---

## 7. First-Party Features Overlapping with External Orchestration

### Features NOT to Rebuild

| Feature | What it does | Use case |
|---------|-------------|----------|
| **`/loop`** | Run a prompt repeatedly on interval | `claude -p "task" &` + sleep loop; but `/loop` is built-in |
| **Scheduled tasks** | Recurring prompts on cron schedule | `--schedule` in settings or `/schedule` command |
| **Remote Control (`--remote-control`)** | Headless CLI session controllable from claude.ai web UI | Hybrid human + automation workflows |
| **Workflows** | Claude-written orchestration scripts (JavaScript) | Multi-agent coordination; save & reuse |
| **Agent teams** (`--teammate-mode`, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS`) | Multiple Claude instances coordinating as peers | Parallel work with shared task list |
| **Background sessions** (`--bg`) | Sessions that run in background, managed by agent view | Task queue, progress monitoring via `/workflows` |
| **Worktrees** (`--worktree`, `-w`) | Automatic git worktree creation per session | Parallel branches with automatic isolation |
| **Cloud sessions** (`--cloud`) | Sessions stored on claude.ai servers, resumable from web | Hybrid web + CLI workflows |

### When to Use Each

**For a Rust orchestration runtime**:

1. **Don't rebuild `--worktree`** — use it directly
   ```bash
   claude --worktree task-name -p "claim and work" --allowedTools "Read,Edit,Bash"
   ```

2. **Don't rebuild cost tracking** — parse `--output-format json`
   ```json
   { "total_cost_usd": 0.042, "session_id": "...", "stop_reason": "end_turn" }
   ```

3. **Don't rebuild workflows** — use `/loop` or custom `WorktreeCreate` hooks if you need custom logic

4. **Consider using hooks** for:
   - Pre-flight checks (PreToolUse hook blocking destructive commands)
   - Post-action logging (PostToolUse → webhook to your orchestrator)
   - Workflow enforcement (SessionEnd hook → webhook with final state)

5. **Use permissions/settings injection** for task-scoped tool allowlists:
   ```bash
   --allowedTools "Read,Bash(git *),Bash(npm test)" --permission-mode dontAsk
   ```

---

## 8. Environment Variables for Runtime Control

**Reference**: https://code.claude.com/docs/en/env-vars.md

### Key Variables for Orchestration

| Variable | Purpose | Value |
|----------|---------|-------|
| `CLAUDE_CONFIG_DIR` | Override `~/.claude` location | `/path/to/config` |
| `ANTHROPIC_API_KEY` | API authentication | Valid key (required in bare mode) |
| `ANTHROPIC_MODEL` | Default model | `claude-sonnet-4-6`, etc. |
| `API_TIMEOUT_MS` | API request timeout | Milliseconds (default 600000) |
| `BASH_DEFAULT_TIMEOUT_MS` | Bash command timeout | Milliseconds (default 120000) |
| `BASH_MAX_OUTPUT_LENGTH` | Bash output cap | Characters (default 30000, max 150000) |
| `DISABLE_TELEMETRY` | Disable usage reporting | Set to any value |
| `CLAUDE_CODE_DISABLE_WORKFLOWS` | Disable dynamic workflows | `1` |
| `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS` | Background task wait | Milliseconds (default 10000) |
| `CLAUDE_CODE_CHILD_SESSION` | Detect if spawned by tool | Set by Claude Code (read-only) |
| `CLAUDECODE` | Detect running inside Claude Code | Set to `1` by Claude Code (read-only) |
| `MCP_TIMEOUT` | MCP server startup timeout | Milliseconds (default 30000) |

### Passing Environment Variables

**File-based** (preferred for reproducibility):
```json
{
  "env": {
    "ANTHROPIC_API_KEY": "...",
    "BASH_DEFAULT_TIMEOUT_MS": "300000"
  }
}
```

**Shell-based**:
```bash
export ANTHROPIC_API_KEY="sk-..."
export BASH_DEFAULT_TIMEOUT_MS="300000"
claude -p "task"
```

---

## Example: Mechanical Enforcement Workflow

```bash
#!/bin/bash
# External orchestrator: claim task → work → verify → review → land → close

TASK_ID="PR-123"
BRANCH="pr-123-review"

# 1. CLAIM: Create worktree
claude --worktree "$BRANCH" -p "create branch for review" \
  --allowedTools "Bash(git *)" \
  --permission-mode dontAsk \
  --output-format json > claim.json

SESSION_ID=$(jq -r '.session_id' claim.json)

# 2. WORK: Run in worktree with verified hooks
claude -p "review auth changes" \
  --resume "$SESSION_ID" \
  --allowedTools "Read,Grep,Bash(git diff *)" \
  --append-system-prompt "Enforce these checks: no credentials in code, all errors handled" \
  --output-format json > work.json

WORK_COST=$(jq -r '.total_cost_usd' work.json)

# 3. VERIFY: Run test suite
claude -p "run tests and report status" \
  --resume "$SESSION_ID" \
  --allowedTools "Bash(npm test),Read" \
  --max-turns 3 \
  --output-format json > verify.json

# 4. REVIEW: Summarize findings
SUMMARY=$(jq -r '.result' verify.json)
echo "Review Complete: $SUMMARY"

# 5. LAND: Commit and push (if no issues)
if jq -e '.result | contains("PASSED")' verify.json > /dev/null; then
  claude -p "commit changes and push branch" \
    --resume "$SESSION_ID" \
    --allowedTools "Bash(git commit *),Bash(git push origin $BRANCH)" \
    --permission-mode bypassPermissions \
    --output-format json > land.json
fi

# 6. CLOSE: Clean up and track metrics
TOTAL_COST=$(echo "$WORK_COST" | bc)
echo "Task $TASK_ID complete. Cost: \$$TOTAL_COST"
```

---

## Quick Reference: Key URLs

| Topic | Documentation |
|-------|---------------|
| Hooks (complete) | https://code.claude.com/docs/en/hooks.md |
| Hooks guide | https://code.claude.com/docs/en/hooks-guide.md |
| CLI reference | https://code.claude.com/docs/en/cli-reference.md |
| Headless mode | https://code.claude.com/docs/en/headless.md |
| Permissions | https://code.claude.com/docs/en/permissions.md |
| Permission modes | https://code.claude.com/docs/en/permission-modes.md |
| Settings | https://code.claude.com/docs/en/settings.md |
| Sessions | https://code.claude.com/docs/en/sessions.md |
| Worktrees | https://code.claude.com/docs/en/worktrees.md |
| Subagents | https://code.claude.com/docs/en/sub-agents.md |
| Workflows | https://code.claude.com/docs/en/workflows.md |
| Skills | https://code.claude.com/docs/en/skills.md |
| MCP | https://code.claude.com/docs/en/mcp.md |
| Agent SDK (overview) | https://code.claude.com/docs/en/agent-sdk/overview.md |
| Agent SDK (TypeScript) | https://code.claude.com/docs/en/agent-sdk/typescript.md |
| Agent SDK (Python) | https://code.claude.com/docs/en/agent-sdk/python.md |
| Costs | https://code.claude.com/docs/en/costs.md |
| Monitoring/OpenTelemetry | https://code.claude.com/docs/en/monitoring-usage.md |
| Environment variables | https://code.claude.com/docs/en/env-vars.md |

---

## Flags & Verification

**Unable to verify** (documentation unclear, feature in flux, or no public reference):
- Exact OTEL metric names and cardinality labels (documented generally; specific field names subject to change)
- `--permission-prompt-tool` behavior with non-standard MCP tool names (reference brief, examples minimal)
- Internal format of transcript JSONL (explicitly NOT stable interface)
- Precise hook timeout behavior under SIGTERM (documented, but edge cases vary by OS)

**All other items checked against official docs & v2.1.234+ behavior.**

---

**Document generated**: 2026-08-17
**Claude Code version range**: v2.1.230–v2.1.234 (stable; may differ in beta/canary)
