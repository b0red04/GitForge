# Architecture Deepening Plan

Candidate refactors that turn shallow modules into deep ones. Each is evaluated by the deletion test: if deleting the module concentrates complexity elsewhere, it was earning its keep.

---

## Completed

- **gitforge-git Error Type — Structured variants** — done. Expanded `GitError` from 2 to 10 variants (`MergeConflict`, `AuthenticationFailed`, `NetworkError`, `IndexLock`, `EmptyCommit`, `BranchNotFound`, `BranchNotFullyMerged`, `InvalidReference`, plus original `RepositoryNotFound`/`OperationFailed`). Centralised classification in `classify_git_failure(args, output)` in `error.rs`, routed through all 3 `run_git*` methods + 4 hand-rolled `Command::new("git")` sites. App layer: added `report_git_error` using `GitError::toast_message()` + variant-aware toast kind (`EmptyCommit` → Info, not Error). Deleted `clean_error_message` (the string-parsing compensating layer) and its 6 tests. 11 new classifier/toast tests in `error.rs`.

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
- **Sidebar state ownership consolidation** — done (uncommitted, +123/−73 across 6 files). `SidebarState` is now the single seam for its own state: toggle/filter/`set_context_menu` methods own the mutations, matching the peer-panel pattern (DiffPanel/GraphPanel/StatusPanel). `SidebarExpansion` sub-struct centralises the snapshot shape and the "which fields persist" rule (`apply_persisted_from_settings`/`write_persisted_to_settings`). `toggle_sidebar_worktrees` relocated from `dialog_ops.rs` to `git_ops.rs`. Collapsed 3 copies of the Settings↔SidebarState 4-boolean mapping (`app.rs` init + `save_settings` + `settings_ops` draft-apply) and 2 copies of the TabSnapshot 6-field mapping (`repo_session.rs`) into method calls; `TabSnapshot` now embeds one `sidebar_expansion` field. Investigation corrected the PLAN's scope: it understated the problem (missed the two duplicated sync layers — the real payoff) and incorrectly included `navigate_to_ref`, which is a graph-navigation action, not sidebar state, so it was left in `git_ops.rs`. `worktrees_expanded`'s non-persistence is now a visible omission in `write_persisted_to_settings` rather than silent.

---

## Active candidates

---

## New candidates

### 8. `run_git_op` abstraction has its seam in the wrong place — 11 data-returning ops rebuild the spawn scaffold

**Files:** `ops/git_ops.rs:519-609` (`run_git_op`, `run_git_op_with_status`), 11 hand-rolled duplicates at `git_ops.rs:124, 260, 332, 381, 790, 852, 890, 940` + `hosting_ops.rs:52, 120, 177, 231, 290` + `ai_ops.rs:33-133`

**Problem:** The two helpers cover only void-returning "do git op then refresh" ops. Anything that returns data (file bytes, diff text, RepoState, blame lines) re-implements `cx.spawn` + `spawn_blocking` + `open_repo.lock()` + the 3-arm `Ok(Ok(_))/Ok(Err(_))/Err(_)` match + `tracing::error!` pair + `this.update`. The pattern is duplicated ~14 times. `let repo_lock = open_repo.lock();` appears 11 times; `"No repository open"` wrapped in `GitError::OperationFailed` 9 times; the "task panicked" match arm duplicated 12 times.

**Solution:** Generalise the helpers to thread a result back to the `this.update` closure — e.g., `run_git_op_with_result<F, T>(open_repo, op: F, on_done: impl FnOnce(&mut Self, Result<T>) + 'static)`.

**Benefits:**
- The two existing helpers pass the deletion test (15 call sites each); the hand-rolled spawns each wrap a real operation, so individually they pass — but the **scaffolding around the unique body** is duplicated. The seam needs to move to admit a returned value.

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
