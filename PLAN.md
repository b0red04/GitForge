# Architecture Deepening Plan

Candidate refactors that turn shallow modules into deep ones. Each is evaluated by the deletion test: if deleting the module concentrates complexity elsewhere, it was earning its keep.

---

## Completed

- **RepoSession extraction** — done in `64297f7`, finished by collapsing the duplicated `apply_repo_state` (branch `opencode/finish-repo-session-apply-state`). `RepoSession::apply_repo_state` is now the single seam for installing a fresh `RepoState`.
- **CommitEditor extraction** — done in `1f89936`. Owns commit message buffer, cursor, AI alternatives, rendering at `views/commit_editor.rs`.
- **UTF-8 diff truncation** — fixed in `0290776`.
- **Command dispatch** — already typed. `CommandAction` is an enum (`commands.rs:56-91`), `CommandEntry.action: CommandAction`, dispatch via `.on_action(cx.listener(...))`. The earlier "string-typed" claim was wrong; typos are compile errors. (Note: `FetchAll`, `PushCurrent`, `PullCurrent` are bound and handled but missing from the `CommandAction` enum, so they don't reach the palette/menu — a small consistency gap, not a deepening issue.)
- **i18n removal + syntax simplification** — done in `56f7c5d`.
- **Text Input extraction** — shared `TextInput` module in `gitforge-ui` (`text_input.rs`). Migrated sidebar filter, commit editor, command palette, settings (search, 11 draft fields, API key, PAT), dialogs (generic + worktree with separate focus handles), and create PR panel title/description. Worktree overlay bug (shared focus, missing cursor on field 2) fixed.
- **`tab_ops.rs` delegation facade** — done (uncommitted). Deleted 12 one-line forwarders to `RepoSession` (the 11 listed plus `push_closed_tab`, which was hidden by `#[allow(dead_code)]` and surfaced on removal). Investigation corrected the scope: the codebase had already migrated to direct `self.repo_session.X()` calls, so only `active_repo_state` still had callers (4 sites in `git_ops.rs`/`dialog_ops.rs`), rewritten direct; `normalize_repo_path` had 2 internal callers repointed to `RepoSession::`. `tab_ops.rs` now holds only tab lifecycle logic (+9/−327).
- **Dialog system refactor** — done. Pragmatic phased approach (kept `AppDialog` enum as router, not `Box<dyn Dialog>`). Shared primitives in `gitforge-ui/src/dialog.rs` (`DialogColors`, `dialog_overlay`, `dialog_surface`, `dialog_actions`, `attach_dialog_input_keys`). Per-dialog modules under `views/dialogs/` (`simple_input.rs` metadata table for 11 single-input dialogs, plus `credential_add`, `delete_branch`, `fork_confirm`, `worktree`, `remove_worktree`, `hosting_browse`, `create_pr`). `dialog_ops.rs` and `dialog_render.rs` are thin routers (~30 lines each). Create PR overlay moved from `create_pr_panel.rs` to `dialogs/create_pr.rs` with `TextInput` for title/description; unified dispatch in `app.rs`. Overlap with item #1: all dialog text inputs now use shared `TextInput` + `attach_dialog_input_keys`.
- **TextInput extraction** — done (PR stack #13–#15). `gitforge-ui/src/text_input.rs` owns focus, cursor, placeholder, masked display, and rendering. Migrated: dialogs, command palette, commit editor, sidebar filter, settings window. Added `render_static_text_input` for draft-owned fields. Deleted dead `components.rs`.
- **DiffViewer extraction** — done in PR #16. `views/diff_viewer.rs` owns `DiffViewer` with shared Diff/Code/Blame rendering, line selection, scroll handles, and binary/LFS handling. `DiffPanel` and `StatusPanel` embed it; duplicated path resolution and render scaffolding removed. `file_diff_path_or_empty` deduplicates path labels in stage/unstage ops.
- **Dead code cleanup** — done in PR #17. Removed `GitError::Io`, `extract_hunk_patch`, dead `HostingProvider` methods (`list_org_repos`, `file_url`, `commit_url`), `find_account`, and `gitforge-ui/src/components.rs`.
- **`run_git_op` seam generalisation + `run_hosting_op`** — done. Introduced `run_git_op_returning(label, cx, op, on_success)` (`git_ops.rs`) as the general async seam for git ops that produce a value: it owns `cx.spawn` + `spawn_blocking` + repo-handle lock + 3-arm match, hands the value to `on_success`, and hardcodes `report_op_error` on failure. Migrated 7 hand-rolled spawns (`view_file_at_commit`, `select_status_file`, `perform_commit`, `load_status`, `view_blame`, `refresh_repository`, `load_diff_for_selected`). Collapsed the old `run_git_op` into a 4-line specialization (`on_success` = `refresh_repository`); its 26 call sites unchanged. Added `run_hosting_op(label, cx, op, on_success, on_error)` (`hosting_ops.rs`) for the pure-async hosting-client seam and migrated 4 sites (`add_hosting_account`, `open_clone_from_hosting_dialog`, `search_hosting_repos`, `fork_repo`); their pre-flight guards (no account / unknown provider) moved out of the spawn to synchronous early-returns. **Error policy normalized** (the chosen direction): the 5 previously `warn!`-only read fetches and `refresh_repository` (was log-only) now surface failures via `report_op_error` — i.e. read-fetch failures are now user-visible toasts. Investigation corrected the scope: the original item lumped in the hosting pure-async sites, the AI pipeline, and the clones, but those are *different seams*. Left bespoke with rationale: `run_git_op_with_status` (its status-banner lifecycle spans all 3 arms, so it can't delegate without losing the clear-on-error), `clone_repository`/`clone_hosting_repo` (`Repository::clone_repo` constructor — no repo handle to lock), `restart_periodic_fetch` (timer loop), `ai_ops::generate_commit_message` (3-stage diff→provider→generate pipeline). Net −186 lines (1284 → 1098 across the two files).

---

## Active candidates

### 4. gitforge-git Error Type — Single String Variant

**Files:** `gitforge-git/src/error.rs` (2 variants), used across all `*_impl.rs` files

**Problem:** `GitError` has 2 variants: `RepositoryNotFound(String)`, `OperationFailed(String)`. `OperationFailed` absorbs **99%** of constructions (98/99 across 12 files). Structured gix error info is discarded via `.map_err(|e| GitError::OperationFailed(e.to_string()))` — 29 exact-form occurrences plus 22 more `format!`-wrapped variants. (Note: the prior claim that `InvalidReference` and `MergeConflict` variants exist-but-unused was wrong — those variants were never defined. They are candidates to add, not delete.)

**Solution:** Introduce domain-specific variants: `MergeConflict { paths: Vec<String> }`, `AuthenticationFailed { remote: String }`, `NetworkError { source: String }`, `IndexLock { path: PathBuf }`, `EmptyCommit`, `BranchNotFound { name: String }`, plus the missing `InvalidReference`. gix error mapping preserves structured cause.

**Benefits:**
- **Leverage** — app shows "Merge conflict in 3 files" instead of "Operation failed: ...git merge...exit status 1".
- **Locality** — error classification concentrates in `gitforge-git` instead of being parsed from strings in the app layer.

---

## New candidates

### 7. Sidebar state ownership has no home — mutations scattered across 3 files

**Files:** `ops/git_ops.rs:166-243` (sidebar toggles, filter, navigate_to_ref), `ops/dialog_ops.rs:221-225` (`toggle_sidebar_worktrees` — wrong file), `action_handlers.rs`, rendered by `sidebar.rs` (1093 lines, deep)

**Problem:** `SidebarState` exists to own sidebar state, but its mutations live in `git_ops.rs`, `dialog_ops.rs`, and `action_handlers.rs`. Three of four section toggles are in `git_ops.rs`; the fourth (`worktrees`) is in `dialog_ops.rs`. There is no single seam for sidebar transitions, despite `SidebarState` existing precisely to be that owner.

**Solution:** Move sidebar mutations into `SidebarState` methods. Each current caller (`toggle_sidebar_branches`, `update_sidebar_filter`, etc.) becomes a method on the owner.

**Benefits:**
- **Locality** — sidebar state transitions concentrate in one module. A reader debugging "why didn't the worktrees section toggle" goes to one place, not three.
- Each individual toggle fails the deletion test alone (one-liner + notify). As a consolidated group inside `SidebarState`, they pass — the friction is the missing owner.

---

### 9. `HostingProvider` trait — 3 parallel implementations

**Files:** `gitforge-hosting/src/provider.rs:6-20` (trait, 6 methods), `github.rs`, `gitlab.rs`, `codeberg.rs`

**Problem:** The 3 implementations are near-parallel copies: each has its own `make_client` (only auth header differs — `Bearer`/`PRIVATE-TOKEN`/`token`), paginated `list_repos` loop (only URL/page size differs — 100/100/50), `json_to_remote_repo` (only JSON keys differ — e.g. GitLab's `last_activity_at` vs GitHub's `updated_at`, Codeberg's `data` envelope), and identical `create_fork` error-extractor.

**Solution:** Lift the shared "authenticate, paginate, map JSON" shape into a generic helper that takes per-provider string templates. Each provider becomes a thin adapter of templates + JSON key mappings.

**Benefits:**
- Providers individually pass (provider-specific JSON quirks), so this is "the shared shape was never lifted out."

---

### 10. `patch.rs` — duplicated stage/unstage call sites

**Files:** `gitforge-diff/src/patch.rs:3-84` (`extract_patch_from_selection`), `ops/git_ops.rs:448-474` (`stage_selected_lines`), `:476-502` (`unstage_selected_lines`)

**Problem:** The two call sites are 25 lines each and differ in exactly 3 tokens (function name, label string `"Stage lines"`/`"Unstage lines"`, final boolean to `apply_patch`). The other 22 lines (fetch current diff, clone, get selected indices, extract path, build `--- a/{}\n+++ b/{}\n{}` header, call `run_git_op`) are duplicated verbatim.

**Solution:** Extract the shared preamble into a helper that takes the per-operation bits (label, the underlying `Repository::stage_lines`/`unstage_lines` selector, the boolean).

**Benefits:**
- The two callers individually pass the deletion test (real distinct operations) but the duplicated 22-line preamble does not earn its keep.

---

## Cross-cutting notes (not deepening candidates, but worth fixing)

- **Missing ADR** — `CONTEXT.md` references `docs/adr/0001-diff-panel-cached-mirror.md`, but the file does not exist and the directory is empty. Either write the ADR (documenting the `DiffViewMirror` / `DiffSnapshot` / `DiffViewKey` caching layer at `diff_panel.rs:102-176`) or remove the reference.
- **Palette gap** — `FetchAll`, `PushCurrent`, `PullCurrent` are bound to keys, have `.on_action` handlers, and have `git_ops` implementations, but are absent from the `CommandAction` enum — so they don't reach the command palette or menus.
- **Placeholder view mode** — `StatusViewMode::Code` may still be a thin wrapper over `DiffViewer`; verify whether it adds value or should be removed.
- **Pre-existing clippy error** — `graph_panel.rs:154` has a logic bug in branch-name matching (`r.name == *branch_name || (r.kind == RefKind::RemoteBranch && r.name == *branch_name)` — the second clause is redundant). Unrelated to any candidate but blocks `-D warnings`.
