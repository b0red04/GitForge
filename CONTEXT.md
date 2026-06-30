# GitForge — Domain Context

## Project

GitForge is a Linux-first Git GUI client built with GPUI (Zed's GPU-accelerated UI framework) and gix (pure Rust Git backend). It visualises repository history as an interactive commit graph, with a diff viewer, sidebar ref tree, and AI-powered features.

## Core Domain Terms

**Repository** — a Git working tree opened by the user. The `gitforge-git` crate wraps `gix::Repository` and exposes a `Repository` struct with read operations (log, status, references, diff, worktree list). One `Repository` per open tab, stored as `RepoSession.open_repo_tabs[i].repo` behind `Arc<Mutex<Option<Repository>>>`, persists for the tab's lifetime and is not re-opened per operation.

**RepoState** — a snapshot of repository data (commits, references, remotes, status, worktrees). Created from a `Repository` via `RepoState::from_repository(&repo)`. Owned per-tab by `RepoSession.open_repo_tabs[i].repo_state`; the active tab's state is reached via `RepoSession::active_repo_state`. `RepoSession::apply_repo_state` is the single seam for installing a fresh snapshot — it writes the graph/status/diff panels and updates the active tab in one call. Render-path callers read derived data (e.g. origin's remote URL via `RepoState::remote_url`) from the snapshot rather than acquiring the live `Repository` lock, so scrolling never blocks on git I/O.

**Commit** — a point in repository history. Represented as `CommitInfo` (id, short_id, summary, author, dates, parent_ids). Commits are rendered as rows in the Commit Graph.

**Commit Graph** — a DAG visualisation where commits are placed on horizontal lanes, connected by arcs. The `gitforge-graph` crate provides the pure layout algorithm (`Graph::build`); the app renders it as a GPUI canvas with nodes, arcs, and continuing lane lines.

**Lane** — a vertical column in the commit graph. `LaneAssigner` decides which lane each commit occupies, keeping the main line straight.

**Reference** — a named pointer (branch, tag, remote branch, stash). `RefInfo` holds the name, kind (`RefKind`), target commit, and HEAD status. Displayed as coloured pills in the graph and listed in the Sidebar.

**Status** — the working tree state (staged, unstaged, untracked, conflicted files). `RepoStatus` aggregates `FileEntry` records with `FileStatus` classification.

**Diff** — a set of file changes between two trees or a commit against its parent. The `gitforge-diff` crate parses unified diff text into `FileDiff` with `DiffLine` records and `DiffHunk` ranges (populated by the parser, not re-derived). Displayed in the Diff Panel.

**Sidebar** — the left panel showing branches, remote branches, tags, and worktrees.

**Toolbar** — the top bar showing the app name and current repository path.

**Graph Panel** — a state-owning panel struct (`GraphPanel`) that holds commits, references, the Graph, selection state, and scroll handle. Renders the commit graph with virtual scrolling.

**Diff Panel** — a state-owning panel struct (`DiffPanel`) that holds diff state and scroll handle. Renders commit metadata, file list, and line-level diff content.

**Theme** — a JSON-defined colour palette (`Theme` → `AppColors`) providing all colour tokens for the UI.

**Operation Dispatch** — the module that runs git/hosting/AI operations off the UI thread and routes their result back into `RepoSession`. Its core is a pure classifier, `plan_dispatch` (`crates/gitforge-app/src/views/ops/dispatch.rs`), which turns a `Result<T, AppError>` plus a declarative `OpEffects` into a `DispatchAction` (surface + refresh flags + value) — the single decision point for how an op's outcome reaches the user. `AppError` spans `GitError` (git) and `RemoteError` (hosting/AI, with severity + a credential-redacted message). Thin shells on `GitForgeApp` (`run_op`, `run_git_blocking`) and the `with_repo_blocking` lock-dance helper wrap the classifier; together they replace the prior async seams (`run_git_op*`, `run_blocking_op_*`, `run_hosting_op`, `dispatch_bg_result*`) and the bespoke AI/PR pipelines. The classifier has no GPUI types, so the dispatch decision is unit-testable without a `TestAppContext`.

**Selection Cascade** — the invariant that graph selection, status-panel mode, and diff-panel state move together. Enforced by `RepoSession::cascade`, the single private method that propagates a `GraphSelection` to `status_panel` (enter/exit graph-staging, gated on `view_mode`) and `diff_panel` (clear). The cascade does **not** write `graph_panel` — the selection is its input, not its output. Public entries: `set_selection` (clicks / programmatic — writes graph + forces `CommitHistory` + cascades), `cascade_current` (keyboard — graph already moved via `select_prev`/`select_next`, just cascades). Both return a `SelectionEffect` (`ClearDiff` / `LoadDiffForSelected`) describing the async work the caller must spawn; the caller (`GitForgeApp`) interprets the effect and calls `load_diff_for_selected` or just notifies. The snapshot path (`apply_repo_state_to_panels`) returns `()` and calls `cascade` directly after `reselect_after_refresh` decides the post-refresh selection; `PreservedCommit` bypasses the cascade because the invariant already holds and the diff cache stays valid. See `docs/adr/0003-selection-cascade.md`.

## Crate Structure

```
gitforge-app        Binary entry point, window lifecycle, view modules
gitforge-ui         Reusable GPUI components, theme engine, icons
gitforge-git        gix wrapper, porcelain API, status, diff, worktree operations
gitforge-graph      Commit graph layout algorithm (pure logic, no UI)
gitforge-diff       Diff parsing, highlighting, patch generation
gitforge-hosting    GitHub/GitLab/Codeberg API clients (full implementation)
gitforge-ai         AI backend — local ollama + cloud APIs (stubs)
gitforge-syntax     Syntax highlighting (tree-sitter)
```

## View Module Structure

```
gitforge-app/src/views/
  app.rs            GitForgeApp — state management, action handlers, Render composition
  graph_panel.rs    GraphPanel — owns graph state, renders commit graph (canvas, nodes, arcs, lanes)
  diff_panel.rs     DiffPanel — owns diff state, renders file list and diff content
  sidebar.rs        Sidebar rendering (branches, remotes, tags)
  toolbar.rs        Toolbar rendering (app name, repo path, shortcuts)
```

## Key Patterns

- **Repository loading** goes through `RepoState::from_repository(&repo)` which takes a snapshot from an already-open repo. The `Repository` object persists for the session, one per open tab (`RepoSession.open_repo_tabs[i].repo`), each behind its own `Arc<Mutex<Option<Repository>>>`.
- **Panel ownership** — `GraphPanel` and `DiffPanel` own their own state and scroll handles. `GitForgeApp` delegates rendering to them via method calls. Selection coordination (graph selection ↔ status mode ↔ diff state) is delegated to `RepoSession::cascade` — the single home for the Selection Cascade invariant (see Core Domain Terms). Other panel mutations (`set_code_view`, `select_file`, `set_blame`, etc.) stay direct reach-ins; the cascade is the seam for the *selection invariant*, not a full panel façade. Panels are plain structs owned by the app (not GPUI entities), so they all live inside the single root `GitForgeApp` view.
- **Diff view caching** — because the whole app is one root view, scrolling the commit list dirties `GitForgeApp` and re-renders everything every frame. To keep scrolling cheap, the diff panel is mirrored by a cached GPUI entity (`DiffViewMirror`) embedded with `.cached(...)`. `DiffPanel` stays the single source of truth; `GitForgeApp::render` rebuilds a render snapshot into the mirror only when a cheap `DiffViewKey` (selected commit, diff/file selection, view mode, theme, loading, line selection) changes — never on scroll — so GPUI recycles the diff paint while the commit list scrolls. See `docs/adr/0001-diff-panel-cached-mirror.md`.
- **Async work** uses `tokio::spawn_blocking` for blocking git I/O. Results are moved into `this.update(cx, ...)` closures to update panel state on the GPUI main thread. Each tab's repository handle (`RepoSession.open_repo_tabs[i].repo`) uses `Arc<Mutex<>>` for cross-thread access. This is being centralized behind **Operation Dispatch** (see Core Domain Terms): `plan_dispatch` owns the result→surface decision and the shells own the spawn; the legacy per-concern seams (`run_git_op*`, `run_blocking_op_*`, `run_hosting_op`, `dispatch_bg_result*`) and the bespoke AI/PR pipelines are being migrated onto it.
- **Rendering** is declarative GPUI — each panel has a `render` method that produces GPUI elements from its owned state + colours. Diff content (`CommitDiffState.file_diffs`) and diff lines are stored behind `Arc`s and per-file add/remove counts are precomputed at load time, so rendering never deep-clones or recounts diff data per frame.
