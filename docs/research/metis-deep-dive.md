# Metis deep dive

**Subject:** `colliery-io/metis` — "Persistent, structured project management for AI coding agents."
**Local checkout:** `~/projects/metis` (remote `origin https://github.com/colliery-io/metis.git`; workspace version 2.1.0, latest commit `6745810 chore(angreal): add bump-version task`).
**Purpose of this note:** decide what ai_runner (an orchestration runtime: agent fleets in git worktrees, task queue, merge gates, human-decision queues) should borrow from metis, what it should not, and what metis leaves undone.
**Accessed:** 2026-08-17. All paths below are relative to `~/projects/metis` unless absolute. Anything marked *(inference)* is my reading, not something metis states.

---

## 1. Architecture

### 1.1 Crates and dependency graph

Cargo workspace with five members (`Cargo.toml:1-8`): `metis-code-index`, `metis-docs-cli`, `metis-docs-core`, `metis-docs-mcp`, `metis-docs-gui/src-tauri`. A sixth crate `metis-docs-tui` is listed in the README as deprecated (`README.md:225`) but is not a workspace member. Total Rust: ~35.8k lines across `crates/`.

Dependency graph (`docs/explanation/architecture.md:19-37`):

```
metis-docs-cli  -> metis-docs-core, metis-docs-mcp, metis-code-index
metis-docs-mcp  -> metis-docs-core, metis-code-index
metis-docs-gui  -> metis-docs-core         (src-tauri)
metis-code-index (standalone; vendored from colliery-io/muninn — crates/metis-code-index/src/lib.rs:7-8)
```

The CLI binary `metis` embeds the MCP server: `metis mcp` launches it (`crates/metis-docs-cli/src/cli.rs:43-44`, `crates/metis-docs-cli/Cargo.toml:26`).

### 1.2 Crate names + versions (for our own dependency choices)

Workspace-shared (`Cargo.toml:21-33`): `tokio 1 (full)`, `serde 1 (derive)`, `serde_json 1`, `anyhow 1`, `thiserror 1`, `tracing 0.1`, `tracing-subscriber 0.3 (env-filter)`, `tempfile 3`, `tokio-test 0.4`, `chrono 0.4 (serde)`.

| Concern | Crate (version) | Where |
|---|---|---|
| CLI parsing | `clap 4 (derive, color)` | `crates/metis-docs-cli/Cargo.toml:41` |
| CLI UX | `dialoguer 0.11`, `indicatif 0.17`, `colored 2`, `console 0.15`, `tabled 0.15` | `crates/metis-docs-cli/Cargo.toml:42-46` |
| MCP server | `rust-mcp-sdk 0.8.0` (features: server, macros, hyper-server, streamable-http, stdio), `async-trait 0.1`, `futures 0.3`, `schemars 0.8` | `crates/metis-docs-mcp/Cargo.toml:31-40` |
| SQLite / ORM | `diesel 2 (sqlite, chrono, uuid, serde_json, returning_clauses_for_sqlite_3_35)`, `diesel_migrations 2`, `libsqlite3-sys 0.30 (bundled)` | `crates/metis-docs-core/Cargo.toml:31-33` |
| Frontmatter / markdown | `gray_matter 0.2` (YAML frontmatter), `pulldown-cmark 0.9`, `serde_yaml 0.9` | `crates/metis-docs-core/Cargo.toml:29,38-39` |
| Templating | `tera 1.19`, `include_dir 0.7` | `crates/metis-docs-core/Cargo.toml:40-41` |
| Config | `toml 0.8`, `dirs 5` | `crates/metis-docs-core/Cargo.toml:28,30` |
| Hashing / ids | `sha2 0.10` (doc file hash), `uuid 1 (v4)`, `blake3 1` (code-index incremental) | `crates/metis-docs-core/Cargo.toml:34-35`, `crates/metis-code-index/Cargo.toml:26` |
| FS walking | `walkdir 2` (core), `ignore 0.4` (gitignore-aware, code-index) | `crates/metis-docs-core/Cargo.toml:42`, `crates/metis-code-index/Cargo.toml:19` |
| Code parsing | `tree-sitter 0.25` + rust 0.24 / python 0.23 / typescript 0.23 / javascript 0.25 / go 0.25, `streaming-iterator 0.1` | `crates/metis-code-index/Cargo.toml:11-22` |
| Desktop GUI | `tauri =2.11.2`, `tauri-build =2.6.2`, `tauri-plugin-log/shell/dialog` (exact pins, see comment on why), `winreg 0.52` (Windows) | `crates/metis-docs-gui/src-tauri/Cargo.toml:18-46` |
| Tests | `sqlx 0.8 (sqlite, runtime-tokio-rustls)` used only as a dev-dep in the MCP crate to inspect the DB, `regex 1` | `crates/metis-docs-mcp/Cargo.toml:44-48` |

**File watching: none.** No `notify` or similar anywhere (grep of all `Cargo.toml` for `notify|watch` returned nothing). Freshness is achieved by re-syncing on every operation (see 1.5).

### 1.3 Data model

Layered core (`docs/explanation/architecture.md:41-85`): domain (documents, phases, config, templates, traits) / application (services) / DAL (Diesel SQLite + filesystem).

**Document types** (`crates/metis-docs-core/src/domain/documents/types.rs:151-159`): `Vision`, `Initiative`, `Task`, `Adr`, `Specification`. Backlog items are Tasks in `backlog` phase living under `backlog/{bugs,features,tech-debt}/` (`crates/metis-docs-core/src/application/services/synchronization.rs:208-211`, `crates/metis-docs-mcp/instructions.md:16`). A `Strategy` type existed and was removed in 2.0 (`git log`: `631993b feat!: remove Strategy document type`; ADR-007).

**Common core** (`crates/metis-docs-core/src/domain/documents/traits.rs:180-191`):

```rust
pub struct DocumentCore {
    pub title: String,
    pub metadata: DocumentMetadata,   // created_at, updated_at, exit_criteria_met, short_code  (metadata.rs:5-11)
    pub content: DocumentContent,
    pub parent_id: Option<DocumentId>,
    pub blocked_by: Vec<DocumentId>,
    pub tags: Vec<Tag>,               // Tag::Phase(Phase) | Tag::Label(String)   (types.rs:324-329)
    pub archived: bool,
    pub initiative_id: Option<DocumentId>,
}
```

The `Document` trait (`traits.rs:7-170`) supplies `id()` (slug from title), `phase()` (parsed from the first `#phase/...` tag, `traits.rs:34-43`), `can_transition_to`, `transition_phase(Option<Phase>)`, `update_section(content, heading, append)` (H2-section replace/append, `traits.rs:55-128`), `validate`, `exit_criteria_met`, and template accessors.

**Identity.** Two ids per document: a title-derived slug `DocumentId` capped at 35 chars (`types.rs:5-57`) and a **short code** `PREFIX-T-NNNN` generated from a per-type counter stored in the SQLite `configuration` table (`crates/metis-docs-core/src/dal/database/configuration_repository.rs:141-181`; type letters V/I/T/A/S at lines 163-168). Prefix is uppercased, capped at 6 chars, default `PROJ` (`.../workspace/initialization.rs:83-96`). Short codes are the handle used by every MCP tool and CLI command.

**Configuration** (`crates/metis-docs-core/src/domain/configuration.rs:5-115`): `FlightLevelConfig { initiatives_enabled }` — presets `streamlined` (Vision→Initiative→Task) and `direct` (Vision→Task). Persisted in both `.metis/config.toml` and the DB `configuration` table, kept in sync (`docs/reference/configuration.md:3-15`).

**Frontmatter shape** (Task template, `crates/metis-docs-core/src/domain/documents/task/frontmatter.yaml:1-16`):

```yaml
id: {{ slug }}
level: task
title: "{{ title }}"
short_code: "{{ short_code }}"
created_at: ...
updated_at: ...
parent: {{ parent_id }}
blocked_by: {{ blocked_by }}
archived: {{ archived }}
tags:
  - "#task"
  - "#phase/todo"
exit_criteria_met: {{ exit_criteria_met }}
initiative_id: {{ initiative_id }}
```

Parsing is done with `gray_matter` into a `Pod::Hash` and a hand-rolled `FrontmatterParser` (`task/mod.rs:131-206`); serialisation re-renders the Tera frontmatter template (`task/mod.rs:240-260`). Note the phase lives *only* in the tags list — there is no `phase:` scalar (though ADR-004 originally specified one; the docs' example at `docs/reference/project-structure.md:50-66` still shows a stale `status:` field).

### 1.4 Lifecycle / state machine

Single source of truth is `DocumentType::valid_transitions_from` (`types.rs:188-238`):

| Type | Phases | Notes |
|---|---|---|
| Vision | draft → review → published | |
| Initiative | discovery → design → ready → decompose → active → completed | |
| Task | backlog → todo → {active, blocked}; active → {completed, blocked}; blocked → {todo, active} | only backwards edges are out of `blocked` |
| ADR | draft → discussion → decided → superseded | |
| Specification | discovery → drafting → review → published | published still editable ("living doc") |

Forward-only, no skipping (`crates/metis-docs-mcp/instructions.md:22-65`). `next_phase()` returns the *first* valid transition, so "auto-advance" (omit target phase) is well-defined (`types.rs:236-238`). The transition service (`.../workspace/transition.rs:33-100`) resolves short code → file, loads the typed doc, validates via `valid_transitions_from`, mutates the phase tag, rewrites the file. Blocked tasks must list `blocked_by` (`task/mod.rs:379-381`).

**Gates that are enforced in code:**
- Tasks may only be (re)assigned to initiatives in `decompose` or `active` (`.../workspace/reassignment.rs:169-176`).
- Phase adjacency (above).

**Gates that are prose only (not enforced):**
- "Exit criteria" checkboxes (ADR-003) — `exit_criteria_met()` returns a hard-coded `false` placeholder for Vision, Initiative, and Task (`vision/mod.rs:303-308`, `initiative/mod.rs:419-424`, `task/mod.rs:388-393`). `read_document` does parse `- [ ]`/`- [x]` lines for display (`crates/metis-docs-mcp/src/tools/read_document.rs:203-215`) but nothing blocks a transition on them.
- The MCP `transition_phase` tool declares a `force: Option<bool>` "skip exit criteria validation" (`crates/metis-docs-mcp/src/tools/transition_phase.rs:30`) but the field is never read (only one occurrence in the file).
- "Human must approve initiative transitions" is entirely instruction text (`instructions.md:236-277`, `session-start-hook.sh:123-130`); nothing in the server distinguishes agent from human callers.

### 1.5 Storage layout and the dual-store model

`.metis/` in the repo root (`docs/reference/project-structure.md:9-44`; constants in `crates/metis-docs-core/src/constants.rs:4-20`):

```
.metis/
  config.toml                  # [project] prefix, [flight_levels] initiatives_enabled
  metis.db (+ -wal/-shm)       # SQLite, gitignored — disposable index
  metis-mcp-server.log
  vision.md
  initiatives/PROJ-I-0001/initiative.md
  initiatives/PROJ-I-0001/tasks/PROJ-T-0001.md
  backlog/{bugs,features,tech-debt}/PROJ-T-00nn.md
  adrs/PROJ-A-0001.md
  specifications/PROJ-S-0001/specification.md
  archived/...                 # same tree, moved on archive
  code-index.md, code-index-hashes.json, code-index-symbols.json, .index-dirty   # all gitignored
```

The generated `.metis/.gitignore` is written at init (`.../workspace/initialization.rs:39-60`).

**Filesystem is source of truth; SQLite is a derived cache** (`docs/explanation/architecture.md:87-100`, README "The filesystem is the source of truth; the database is a disposable cache"). Schema (`crates/metis-docs-core/src/dal/database/migrations/001_initial_schema/up.sql:5-73`, current `schema.rs:3-66`): `documents (filepath PK, id, title, document_type, created_at REAL, updated_at REAL, archived, exit_criteria_met, file_hash, frontmatter_json, content, phase, initiative_id, short_code, parent_id)`, `document_relationships`, `document_tags`, FTS5 virtual table `document_search` (tokenizer `porter unicode61`) kept in sync by triggers, and `configuration (key, value, updated_at)`. Nine Diesel migrations, run automatically on open (`docs/reference/project-structure.md:188-214`).

**Sync** (`SyncService`, `crates/metis-docs-core/src/application/services/synchronization.rs`): walks `.metis/`, parses each file via `DocumentFactory`, computes a file hash, and imports/updates/deletes/moves DB rows (`SyncResult` enum at `:904-928`). Lineage (`initiative_id`, backlog-ness) is *derived from the path*, with the filesystem overriding frontmatter (`:133-150`, `:171-220`). It also detects **short-code collisions** (e.g. after a git merge) and renumbers the deeper/later file, rewriting references (`:294-376`). `Application::sync_directory` first runs `ConfigurationRecoveryService` — recreates a missing/corrupt DB, restores counters by scanning existing short codes, and syncs `config.toml` → DB (`crates/metis-docs-core/src/application/mod.rs:53-111`, `.../workspace/recovery.rs:29,170,202`).

**Every MCP tool call calls `WorkspaceDetectionService::prepare_workspace`**, which auto-corrects a project-root path to `.metis`, runs the v1→v2 filesystem migration, opens the DB, and does a **full sync** before proceeding (`.../workspace/detection.rs:120-159`). That is how they get away with no file watcher: freshness is bought per-call. *(inference: fine for a handful of docs, O(files) per tool call otherwise.)*

### 1.6 MCP tool surface

Registered via `rust_mcp_sdk::tool_box!` (`crates/metis-docs-mcp/src/tools/all_tools.rs:12-27`) and dispatched by name in `server.rs:64-129`. Each tool is a `#[mcp_tool(...)]`-annotated serde struct with doc-comments as parameter descriptions (e.g. `edit_document.rs:15-35`). All tools take `project_path` (the `.metis` dir).

| Tool | Params | Notes |
|---|---|---|
| `initialize_project` | project_path, prefix? | |
| `list_documents` | project_path, include_archived? | |
| `search_documents` | project_path, query, document_type?, limit?, include_archived? | FTS5 |
| `read_document` | project_path, short_code | records a read in `DocumentReadTracker` |
| `create_document` | project_path, document_type, title, parent_id?, complexity?, stakeholders?, decision_maker?, backlog_category? | `create_document.rs:28-42` |
| `edit_document` | project_path, short_code, search, replace, replace_all? | search-and-replace, **read-before-edit guard** |
| `transition_phase` | project_path, short_code, phase?, force? | `force` unused |
| `archive_document` | project_path, short_code | archives children too |
| `reassign_parent` | project_path, short_code, new_parent_id?, backlog_category? | |
| `index_code` | project_path, structure_only?, incremental? | tree-sitter index → `.metis/code-index.md` |
| `open_document` | project_path, short_code, include_children, viewer? | opens VS Code / system editor; pluggable `DocumentViewer` trait (`server.rs:36-40`) |

Two design touches worth copying:

- **Read-before-edit / stale-read guard** (`crates/metis-docs-mcp/src/read_tracker.rs:1-78`): server keeps an in-memory `PathBuf → SystemTime` of last reads; `edit_document` is rejected if the file was never read this session or its mtime is newer than the last read (1s tolerance). Instructions tell the model why (`instructions.md:228`).
- **Dynamic server instructions**: at startup the server reads the workspace config and prepends "Current Project Configuration / Available Operations" to a static `instructions.md` (`crates/metis-docs-mcp/src/lib.rs:66-126`, `:175`). The static part is a ~340-line methodology brief (phases, short codes, human-in-the-loop rules, "tasks as working memory") — the *primary* behavioural control surface, delivered via MCP `instructions` rather than a CLAUDE.md.
- **Uniform tool output** via a `ToolOutput` markdown builder with status icons (`crates/metis-docs-mcp/src/formatting.rs:9-80`) and `error_result(title, detail, hint)`.

Transport is stdio only in `run()` (`lib.rs:180`), though the crate enables `hyper-server`/`streamable-http` features. Logging goes to `.metis/metis-mcp-server.log` if a workspace is found, else stderr at WARN (`lib.rs:130-150`).

### 1.7 CLI surface

`metis` (`crates/metis-docs-cli/src/cli.rs:10-49`): `init [--name --prefix --preset --initiatives]`, `sync`, `create {vision|initiative|task|adr|specification} ...`, `search`, `transition <short_code> [phase]`, `list [-t type -p phase -a --include-archived -f table|compact|json]`, `status [--include-archived -f ...]` (compact format is what the hooks parse), `archive`, `validate`, `mcp [--log-level]`, `config {show|set --preset|--initiatives}`, `index [--structure-only --incremental]`. Verbosity `-v/-vv/-vvv` maps to tracing levels (`cli.rs:52-64`). Workspace discovery walks up from cwd looking for `.metis/` (`crates/metis-docs-cli/src/workspace.rs:10-33`).

### 1.8 GUI (Tauri 2 + Vue 3)

`crates/metis-docs-gui/src-tauri/src/lib.rs:17-63`: `AppState { current_project }` behind a `Mutex`, ~18 `#[tauri::command]`s wrapping core services (`initialize_project, load_project, list_documents, read_document, search_documents, get_available_parents, create_document, update_document, archive_document, transition_phase, get_project_config, sync_project, get_app_version, get_cli_install_status, install_cli, install_cli_elevated, uninstall_cli`). Kanban boards per type with drag-and-drop = phase transition; Tiptap markdown editor (`package.json`, `docs/explanation/architecture.md:134-155`). It also bundles and installs the `metis` CLI on first launch (`services/cli_installer.rs`). Tauri crates are pinned with `=` because `Cargo.lock` is gitignored and CI re-resolves (`src-tauri/Cargo.toml:18-24`).

### 1.9 Code index (metis-code-index)

`walk_directory() → parse_file() → extract_symbols() → format_index()` (`docs/explanation/architecture.md:157-171`). Incremental: `HashManifest` (BLAKE3 per file, `.metis/code-index-hashes.json`) diffed into `IncrementalDiff { changed, unchanged, deleted }`, symbols cached in `code-index-symbols.json`, AI-written module summaries preserved across regeneration (`crates/metis-code-index/src/hasher.rs:14-30`, README "Code Indexing"). Output is `.metis/code-index.md` which the plugin tells the agent to read *before* grepping.

### 1.10 Build / release / tests

- `.angreal/task_dev.py` (angreal, Python task runner): `test`, `build`, `check`, `coverage`, `gui`; per-crate test strategies (`task_dev.py:15-32`). `.angreal/task_release.py`: `bump-version` rewrites every manifest (workspace, Tauri, plugin) so versions don't drift (`task_release.py:11-20`).
- `scripts/install.sh`: curl installer for the desktop app; `tarpaulin.toml` for coverage.
- Tests: core integration tests for collision resolution, configuration recovery, database reconstruction, id/path consistency, reassignment, specifications (`crates/metis-docs-core/tests/*.rs`); MCP functional/integration tests spin the server; CLI comprehensive workflow test (`crates/metis-docs-cli/src/cli.rs:95-220`); Playwright e2e for the GUI (`tests/e2e/*.spec.ts`).

---

## 2. Design decisions from the ADRs

All under `.metis/adrs/`. Frontmatter phase tags show status.

| ADR | File | Decision | Rationale | Status |
|---|---|---|---|---|
| ADR-001 Document Format and Storage | `.metis/adrs/METIS-A-0001.md:30-102` | Markdown + YAML frontmatter in a hierarchical directory tree; slug filenames; `vision.md` fixed name. Alternatives: JSON, XML. | Human-readable, git-diffable, tool-compatible while keeping machine metadata. | superseded (by ADR-006) |
| ADR-002 Obsidian Markdown Format | `METIS-A-0002.md:30-108` | Use Obsidian-flavoured markdown (`[[wikilinks]]`, callouts, folding). | Superset of markdown; better authoring. | superseded (by ADR-005) |
| ADR-003 Exit Criteria Format | `METIS-A-0003.md:29-116` | GitHub-style `- [ ]` checkboxes in a dedicated `## Exit Criteria` section; ≤7 per doc; all must be checked before spawning children; criteria may be refined but not removed. | Familiar, regex-parseable, visual progress. Explicitly names the gap "No built-in approval workflow". | decided (note: not enforced in code — §1.4) |
| ADR-004 Frontmatter Metadata System | `METIS-A-0004.md:29-168` | Core fields for all docs (`id, level, status, created_at, updated_at, parent, blocked_by, phase, tags, exit_criteria_met`) plus per-type fields (initiative `estimated_complexity`, task `assignee/estimated_hours/pr_links`, ADR `decision_date/decision_maker/superseded_by`); templates carry all phases commented-out and you uncomment the current one. | Consistent, queryable, self-documenting templates. | decided (implementation drifted: phase now lives only in tags) |
| ADR-005 Generic Markdown Format | `METIS-A-0005.md:24-104` | Drop Obsidian extensions; CommonMark + YAML frontmatter + checkboxes + tables only. | Multi-interface (CLI/TUI/GUI/AI) compatibility; "Open Formats Over Vendor Lock-in". | decided, supersedes ADR-002 |
| ADR-006 Short Code Document Identification | `METIS-A-0006.md:31-151` | `PREFIX-TYPE-NNNN` as the primary id: filename, DB key, and cross-reference; sequential per type; immutable, never reused; also stored in frontmatter. Alternatives: slugs, UUIDs, hybrid. | Slugs caused conflicts, path-construction pain, and DB/FS mismatch; AI agents need predictable handles. | decided, supersedes ADR-001 |
| ADR-007 Multi-Team / Cross-Repo Work Management Is Out of Scope | `METIS-A-0007.md:24-75` | Metis stays repo-scoped and zero-infrastructure: no central DB, no API layer, no cross-repo sync; abandon `metis-sync` crate and the multi-workspace branch; remove the Strategy type. | Repo is the natural scope for agent work; centralisation destroys "clone the repo, you have the plan"; do one thing well. Leaves the door open for a read-only aggregator over many `.metis/` dirs. | decided |

Two active initiatives are also design-relevant: **METIS-I-0026 "Back Metis with JIRA"** (`.metis/initiatives/METIS-I-0026/initiative.md:24-120`) proposes a fork whose MCP server is a stateless pass-through to JIRA (initiatives=Epics, tasks=Tasks, phases as labels, Metis still enforcing phase gates); and **METIS-I-0029 review agents** (`METIS-I-0029/initiative.md:24-60`) — on-demand `/review-architecture` (ADR-vs-code drift) and `/review-docs` (Diataxis) agents, explicitly *not* gating.

---

## 3. The Claude Code plugin

Layout (`plugins/metis/`): `.claude-plugin/plugin.json`, `.mcp.json` (registers `metis mcp`, `plugins/metis/.mcp.json:1-9`), `agents/{flight-levels,code-index-summarizer}.md`, `commands/{metis-ralph,metis-ralph-tasks,metis-ralph-initiative,cancel-metis-ralph,help}.md`, `hooks/hooks.json` + four shell hooks, `scripts/setup-metis-ralph*.sh`, `skills/{code-index,decomposition,document-selection,phase-transitions,project-patterns}/SKILL.md` (+ `references/`). Marketplace manifest at `.claude-plugin/marketplace.json`.

### 3.1 Hooks (`plugins/metis/hooks/hooks.json:4-47`)

| Event | Script | What it does |
|---|---|---|
| `SessionStart` (matcher `*`) | `session-start-hook.sh` | Exports `CLAUDE_SESSION_ID` into `$CLAUDE_ENV_FILE` (`:8-12`); if `.metis/` exists, runs `metis status --format compact`, counts blocked/active/todo, runs `metis index --incremental`, counts missing semantic summaries, and returns a long `additionalContext` block: "Metis IS your system of record AND working memory", current actionable items, code-index-first rule, tool list, create→read→edit rule, human-in-the-loop for initiatives, task workflow (`:77-142`). |
| `PreCompact` (`*`) | `pre-compact-hook.sh` | Re-indexes if `.metis/.index-dirty` is non-empty, then emits `systemContext` re-injecting the same state summary + "you were just compacted — re-read your active task" (`:24-32`, `:62-104`). |
| `PostToolUse` (`Write\|Edit\|NotebookEdit`) | `post-tool-use-hook.sh` | Appends the edited source path (`.rs .py .ts .tsx .js .jsx .go`, not under `.metis/`) to `.metis/.index-dirty`, deduped (`:14-32`). |
| `Stop` (`*`) | `stop-hook.sh` | The Ralph loop driver (below). Also flushes the dirty index (`:13-18`). |

### 3.2 The Ralph loop (`/metis-ralph SHORT_CODE [--max-iterations N]`)

1. **Command prompt** (`plugins/metis/commands/metis-ralph.md:1-70`). Frontmatter restricts `allowed-tools` to the setup script and `mcp__metis__read_document`. Step 1: verify the task exists via `read_document` (refuse to start otherwise). Step 2: run `setup-metis-ralph.sh $ARGUMENTS`. Step 3: orient via `.metis/code-index.md` if needed → `transition_phase` to `active` → implement → log to the task's `## Status Updates` via `edit_document` → when done output `<promise>TASK COMPLETE</promise>`. Rules: never transition to `completed` yourself; never emit a false promise; if stuck, keep iterating.

2. **Setup script** (`plugins/metis/scripts/setup-metis-ralph.sh`). Validates the code matches `^[A-Z]+-T-[0-9]+$` (`:98`), auto-detects `.metis` by walking up (`:107-126`), and writes a **session-scoped state file** `.claude/metis-ralph-active-${CLAUDE_SESSION_ID}.yaml` (`:137-157`):

   ```yaml
   session_id: "..."
   short_code: "PROJ-T-0001"
   project_path: "/abs/.metis"
   mode: task            # task | tasks | initiative | decompose
   iteration: 1
   max_iterations: 0     # 0 = unlimited
   completion_promise: "TASK COMPLETE"
   started_at: "..."
   ```

3. **Stop hook** (`plugins/metis/hooks/stop-hook.sh`). On every Stop: read hook JSON from stdin, take `session_id`, find *this session's* state file (legacy unscoped file is honoured only if its `session_id` matches — `:23-40`; the scoping was added in `970687a fix: scope Ralph loop state files to session ID`). Parse fields with grep/sed (`:44-50`). If `max_iterations` reached → print, delete state, allow exit (`:74-86`). Otherwise read `transcript_path`, take the last `"role":"assistant"` line, extract text blocks with `jq` (`:88-124`), and look for `<promise>…</promise>` matching `completion_promise` exactly (`:126-141`). If matched → delete state, allow exit. If not → bump `iteration`, and emit `{"decision":"block","reason":<mode-specific continue prompt>,"systemMessage":"Metis Ralph iteration N | Task: … "}` (`:143-236`). The continue prompt says: re-read the task via MCP, review Status Updates, continue, log progress, output the promise when fully done.

4. **Modes.** `task` (single; human completes it), `tasks` (`/metis-ralph-tasks A B C`, serial, the agent *does* transition each to `completed`, promise `ALL TASKS COMPLETE`), `initiative` (`/metis-ralph-initiative`, list tasks under it, complete each, human transitions the initiative), `decompose` (create tasks under an initiative, human transitions to `active`) — see prompts at `stop-hook.sh:155-226` and `commands/metis-ralph-initiative.md:35-60`.

5. **Cancel** (`commands/cancel-metis-ralph.md`): find and `rm` the state file; explicitly does *not* revert phase transitions.

6. **Docs on the rationale** (`docs/explanation/ralph-loops.md:90-108`): re-injecting the task forces the model to re-read acceptance criteria and look at disk rather than its memory; a Stop hook rather than a self-loop keeps context bounded; a file outside the context window survives compaction; an exact string promise is unambiguous. Safety advice: always set `--max-iterations`, keep tasks small (2–3 iterations ideal) (`:139-143`). Note the docs describe an older `.claude/metis-ralph.local.md` path with `active: true` (`:34,44-58`); the code uses the YAML file above.

### 3.3 How tasks are picked up and completed; how humans approve

- **Pick-up** is manual: a human types `/metis-ralph PROJ-T-0001` (or `-tasks`/`-initiative`); the SessionStart hook surfaces "ready to start" items but nothing auto-claims. There is no queue, no lease, no assignment field on tasks (ADR-004 proposed `assignee`; not in the current frontmatter template).
- **Progress** is the task doc's `## Status Updates` section, edited via `edit_document` search/replace ("every few tool calls" — `instructions.md:279-312`).
- **Approval** in `task` mode is the human running `transition_phase active→completed` after review; the agent signals with the promise. In `tasks`/`initiative` modes the agent self-completes tasks and the human reviews the initiative. Nothing verifies (tests, diff, PR) before `completed`; the "gate" is that the model is told not to lie (`metis-ralph.md:62-68`).
- **Human-in-the-loop for strategy** (initiative transitions, decomposition, design choices) is prose in `instructions.md:236-277` and the SessionStart context; `open_document` gives the human a review checkpoint in their editor and the model is told to wait for confirmation and re-read (`instructions.md:153-163`).
- **Autonomy sandboxing** is delegated to Docker sandboxes with bypass-permissions (`docs/docker-sandbox.md:1-80`).

### 3.4 Agents and skills

- `agents/flight-levels.md` — methodology expert; `tools:` restricted to Read/Grep/Glob + the metis MCP tools (`:35`); maps "bug ticket" etc. to `create_document` calls (`:73-80`).
- `agents/code-index-summarizer.md` — background agent that fills module summaries in `code-index.md`; needs `Edit(.metis/code-index.md)` permission (`:1-16`).
- Skills carry the "when to decompose", "phase flow", "anti-patterns", "greenfield/feature/incident/tech-debt patterns" content (`skills/*/SKILL.md`, `skills/*/references/*.md`).

---

## 4. What to borrow for ai_runner vs what not to

### Borrow (with concrete pointers)

1. **Documents = markdown + YAML frontmatter, DB = disposable derived index.** The dual store with "file wins, sync rebuilds" (`docs/explanation/architecture.md:87-100`) is exactly right for anything humans review in PRs. Copy the pattern of `SyncService` producing a `SyncResult` enum per file (`synchronization.rs:904-928`) and a `RecoveryService` that can rebuild counters/config from the files (`recovery.rs`). Use `gray_matter` + `serde_yaml` for frontmatter but deserialize straight into `serde` structs rather than metis's hand-rolled `FrontmatterParser` (`task/mod.rs:131-206`) — that code is duplicated per doc type.
2. **Short codes `PREFIX-T-NNNN`** as the universal handle (ADR-006), plus the merge-collision renumbering in sync (`synchronization.rs:294-376`). For a multi-worktree fleet the collision case is the *normal* case, so ai_runner should either allocate ids centrally (a coordinator holds the counter) or use beads-style hash ids and keep short codes as display aliases. *(inference)*
3. **A single-source-of-truth transition table** on the type enum (`types.rs:188-238`) with `valid_transitions_from / can_transition / next_phase / phase_sequence`, exercised by unit tests (`types.rs:506-594`). Forward-only + one "parking" state (`blocked`) with edges back is a good minimal shape for a task lifecycle. Add explicit `Failed`/`Cancelled` terminals for a runtime *(inference)*.
4. **Read-before-edit guard** (`read_tracker.rs`) — mtime-based optimistic concurrency for agent edits. Generalise to a content-hash / etag on every read → required on every write; that is what a merge gate needs when several agents touch the same doc.
5. **MCP server shape**: `rust-mcp-sdk` `#[mcp_tool]` structs + `tool_box!` (`all_tools.rs`), typed params documented by doc-comments, `ToolOutput` markdown builder + `error_result(title, detail, hint)` (`formatting.rs`), and **dynamic `instructions`** assembled from workspace state at startup (`lib.rs:66-126`). Also copy `prepare_workspace`'s "auto-correct the path the model gave you" (`detection.rs:120-128`) — small, saves many failed calls.
6. **The Ralph Stop-hook loop mechanics**: session-scoped state file outside the context (`setup-metis-ralph.sh:137-157`), `Stop` hook returning `{"decision":"block","reason":…}` (`stop-hook.sh:229-236`), exact-string completion promise, `max_iterations` safety, `PreCompact` re-injection of "you were compacted, re-read your task", `PostToolUse` dirty-tracking. For ai_runner the *supervisor* should own iteration state and inject the continue prompt, but the hook contract is the proven interface to Claude Code.
7. **Templates via Tera with project → global → embedded fallback** (`services/template.rs:103-128`, README "Custom Templates"); `include_str!` of `content.md`/`frontmatter.yaml` per type (`task/mod.rs:31`).
8. **`H2-section update` helper** (`traits.rs:55-128`) — updating a named section (`## Status Updates`) is a better agent-facing primitive than raw search/replace; expose it as an MCP tool.
9. **Session bootstrap via SessionStart hook** printing a compact status (`metis status --format compact`, `session-start-hook.sh:35-60`) — the runtime should provide a `status --format compact` for exactly this.
10. **ADR discipline itself**: `.metis/adrs/METIS-A-000N.md` with Context/Decision/Alternatives/Rationale/Consequences/Review Schedule (`METIS-A-0001.md:30-102`) — adopt for `docs/plans/`.
11. **Code index** (`metis-code-index`) is standalone and could be reused as-is by agents in worktrees; the incremental BLAKE3 manifest design is sound (`hasher.rs:14-30`).
12. **Version-bump-everything task** (`.angreal/task_release.py:11-20`) and exact-pin rationale for GUI deps — cheap lessons.

### Do not borrow (and why)

1. **Diesel + hand-written repository layer.** For a runtime that is mostly append-only events + a few tables, `rusqlite` or `sqlx` with plain SQL is less ceremony than Diesel's schema macros/migrations dance; metis itself uses `sqlx` in tests to poke at the DB (`metis-docs-mcp/Cargo.toml:47`). *(inference/opinion)*
2. **Full re-sync on every tool call** (`detection.rs:150-152`). With N agents hammering the server that is O(files) per call and racy across processes (SQLite file lock, no WAL config seen). ai_runner needs a long-lived daemon that owns the DB and watches files (`notify`) or receives explicit change events.
3. **Phase stored only in a `#phase/x` tag** with the phase enum flattened across all doc types (`types.rs:271-299`, `Tag::from_str` re-listing every phase at `:364-395`). Use a per-kind typed `status` scalar in frontmatter.
4. **Per-type copy-pasted `from_content`/`to_content`/`transition_phase` implementations** (`task/mod.rs`, `initiative/mod.rs`, `vision/mod.rs`, …; the transition service matches on type five times, `transition.rs:103-225`). Use one generic document with a `kind` and a schema table.
5. **Human-approval-as-prose.** Metis's "ALWAYS check in before transitioning an initiative" lives in `instructions.md` and hooks; nothing enforces it and `force` is a no-op. ai_runner's whole point is that gates are machinery: approvals must be first-class records with an actor, and transitions must check them.
6. **Exit criteria as unparsed checkboxes** that the code never evaluates (`exit_criteria_met` = `false` placeholders). Either enforce or don't ship the flag.
7. **Bash+jq+grep hooks parsing YAML with `sed`** (`stop-hook.sh:44-50`). Fine for a plugin, wrong for a runtime — the hook should shell out to the runtime binary (`ai_runner hook stop`) which owns the state.
8. **Two ids per document** (slug + short code + `initiative_id` slug + path-derived lineage that overrides frontmatter). Pick one canonical id.
9. **Tauri GUI early.** ADR-007's own logic applies: do the core well first; a read-only aggregator/dashboard can come later.
10. **JIRA pass-through fork idea** (METIS-I-0026) is not relevant to us; but its table of "MCP tool → external API" is a nice illustration that the tool surface can be decoupled from storage.

---

## 5. Gaps — what metis does not do that an orchestration runtime needs

All of these are *(inference)* unless a metis file is cited saying so.

1. **No process supervision.** Metis never spawns, monitors, or kills an agent; the loop exists only inside one Claude Code session via the Stop hook. There is no daemon, no PID/heartbeat, no restart, no timeout other than `max_iterations`.
2. **No parallelism / no claiming.** Nothing prevents two sessions taking the same task; there is no lease, owner, or `assignee` field (ADR-004 proposed one, `METIS-A-0004.md:104-107`, but the current template lacks it). Session-scoped state files (`970687a`) only stop the *loops* from interfering, not the work.
3. **No git worktrees, branches, or merges.** Metis is unaware of git beyond `.gitignore` generation; short-code collision handling (`synchronization.rs:294`) is its only concession to concurrent branches. No merge queue, no rebase policy, no conflict handling.
4. **No merge/verification gate.** "Done" is a string the model prints (`stop-hook.sh:126-141`) and a human clicking `completed`. No test run, diff review, CI status, or reviewer sign-off is recorded or required. Exit criteria are not evaluated (§1.4).
5. **No human-decision queue.** Approvals are conversational ("Do you want me to proceed?" — `instructions.md:258-268`). There is no durable list of pending decisions with owner, deadline, options, and resolution; `open_document` is the closest thing (a review checkpoint in an editor).
6. **No event log / audit trail.** State is the current file + `updated_at`; `Status Updates` prose is the history. No append-only journal of who transitioned what when, no reasons on transitions.
7. **No cross-agent messaging or dependency scheduling.** `blocked_by` exists in frontmatter but nothing computes readiness from it or wakes a blocked task when its blocker completes.
8. **No multi-repo / central coordination — by design** (ADR-007, `METIS-A-0007.md:37-49`). A fleet running across repos or worktrees of one repo needs a coordinator metis explicitly refused to be; ADR-007 leaves room for "a future tool that consumes `.metis` directories as a read-only aggregation layer" (`:75`).
9. **No permission/sandbox model.** Autonomy relies on Docker sandbox with bypass permissions (`docs/docker-sandbox.md`); metis has no notion of what an agent may touch.
10. **No file watching / push updates**; freshness = re-sync per call (§1.5), and MCP is stdio-only in practice (`lib.rs:180`), one server per client process.
11. **Metrics/costs**: nothing tracks iterations, tokens, wall time, or outcomes per task beyond the `iteration` counter in the state file.

What metis *does* give us that fills a real hole in adopter-style setups: a durable, agent-writable *task document* format with a phase machine and an MCP surface that models actually use well — that is the "working memory" layer, and it composes with (rather than replaces) a coordinator like beads. *(inference)*

---

## 6. Sources (files read, with key ranges)

Repo metadata: `git log --oneline | head -40`, `git remote -v` (origin `colliery-io/metis`), `README.md:1-260` (architecture table `:213-227`, MCP tools `:141-158`, plugin `:174-183`).

Workspace / crates:
- `Cargo.toml:1-33`
- `crates/metis-code-index/Cargo.toml:1-40`, `src/lib.rs:1-25`, `src/hasher.rs:1-50`
- `crates/metis-docs-cli/Cargo.toml:1-50`, `src/main.rs:1-20`, `src/cli.rs:1-220`, `src/workspace.rs:1-33`, `src/commands/{init,list,status,config,index,mcp,create/mod}.rs` (arg definitions via grep)
- `crates/metis-docs-core/Cargo.toml:1-53`, `src/lib.rs:1-31`, `src/constants.rs:1-77`, `src/error.rs:1-70`, `src/domain/documents/types.rs:1-595`, `src/domain/documents/traits.rs:1-200`, `src/domain/documents/metadata.rs:1-52`, `src/domain/documents/task/{mod.rs:1-260,379-400; frontmatter.yaml:1-16; content.md:1-116}`, `src/domain/documents/initiative/mod.rs:11-166,419-424`, `src/domain/documents/vision/mod.rs:303-308`, `src/domain/configuration.rs:1-150`, `src/application/mod.rs:1-117`, `src/application/services/synchronization.rs:1-400,904-960`, `src/application/services/workspace/{initialization.rs:20-150; detection.rs:11-175; transition.rs:25-300; reassignment.rs:169-176; recovery.rs (fn list); archive.rs:1-60}`, `src/application/services/document/{creation.rs,discovery.rs}` (fn lists), `src/application/services/template.rs` (fn list), `src/dal/database/{schema.rs:1-66; models.rs:1-102; repository.rs (fn list); configuration_repository.rs:141-181; migrations/001_initial_schema/up.sql:1-73}`, `tests/*.rs` (names/counts)
- `crates/metis-docs-mcp/Cargo.toml:1-53`, `src/lib.rs:1-202`, `src/server.rs:1-130`, `src/tools/all_tools.rs:1-27`, `src/tools/edit_document.rs:1-120`, `src/tools/read_document.rs:180-215`, `src/tools/transition_phase.rs:30`, param structs of all other tools (grep), `src/read_tracker.rs:1-80`, `src/formatting.rs:1-80`, `instructions.md:1-344`, `tests/*.rs` (names)
- `crates/metis-docs-gui/Cargo.toml`, `src-tauri/Cargo.toml:1-50`, `src-tauri/src/lib.rs:1-63`, `src-tauri/src/services/sync.rs:1-40`, `package.json:1-40`

Docs: `docs/explanation/architecture.md:1-211`, `docs/explanation/ralph-loops.md:1-168`, `docs/reference/project-structure.md:1-233`, `docs/reference/configuration.md:1-100`, `docs/docker-sandbox.md:1-80`; file list of `docs/{explanation,how-to,reference,tutorials}`.

Plugin: `.claude-plugin/marketplace.json`, `plugins/metis/.claude-plugin/plugin.json`, `plugins/metis/.mcp.json:1-9`, `plugins/metis/hooks/hooks.json:1-49`, `plugins/metis/hooks/{session-start-hook.sh:1-154; pre-compact-hook.sh:1-112; post-tool-use-hook.sh:1-34; stop-hook.sh:1-238}`, `plugins/metis/commands/{metis-ralph.md:1-70; metis-ralph-initiative.md:1-60; metis-ralph-tasks.md:1-50; cancel-metis-ralph.md:1-32}`, `plugins/metis/scripts/setup-metis-ralph.sh:1-179`, `plugins/metis/agents/{flight-levels.md:1-80; code-index-summarizer.md:1-40}`, `plugins/metis/skills/{phase-transitions/SKILL.md:1-80; decomposition/SKILL.md:1-60}`, `plugins/metis/README.md:1-60`.

`.metis/`: `config.toml`, `vision.md:1-57`, `adrs/METIS-A-0001.md` … `METIS-A-0007.md` (read fully), `initiatives/METIS-I-0026/initiative.md:14-120`, `initiatives/METIS-I-0029/initiative.md:1-60`, `initiatives/METIS-I-0028/initiative.md:1-25`, `specifications/METIS-S-000{1,2}/specification.md:1-20`, directory listing (120 files incl. `archived/`).

Build/tests: `.angreal/task_dev.py:1-80`, `.angreal/task_release.py:1-40`, `scripts/install.sh` (listed), `tarpaulin.toml`, `tests/e2e/*` (listed).
