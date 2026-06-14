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
- **`run_git_op` seam generalisation + `run_hosting_op`** — done. Introduced `run_git_op_returning(label, cx, op, on_success)` (`git_ops.rs`) as the general async seam for git ops that produce a value: it owns `cx.spawn` + `spawn_blocking` + repo-handle lock + 3-arm match, hands the value to `on_success`, and hardcodes `report_op_error` on failure. Migrated 7 hand-rolled spawns (`view_file_at_commit`, `select_status_file`, `perform_commit`, `load_status`, `view_blame`, `refresh_repository`, `load_diff_for_selected`). Collapsed the old `run_git_op` into a 4-line specialization (`on_success` = `refresh_repository`); its 26 call sites unchanged. Added `run_hosting_op(label, cx, op, on_success, on_error)` (`hosting_ops.rs`) for the pure-async hosting-client seam and migrated 4 sites (`add_hosting_account`, `open_clone_from_hosting_dialog`, `search_hosting_repos`, `fork_repo`); their pre-flight guards (no account / unknown provider) moved out of the spawn to synchronous early-returns. **Error policy normalized** (the chosen direction): the 5 previously `warn!`-only read fetches and `refresh_repository` (was log-only) now surface failures via `report_op_error` — i.e. read-fetch failures are now user-visible toasts. Investigation corrected the scope: the original item lumped in the hosting pure-async sites, the AI pipeline, and the clones, but those are *different seams*. Left bespoke with rationale: `run_git_op_with_status` (its status-banner lifecycle spans all 3 arms, so it can't delegate without losing the clear-on-error), `clone_repository`/`clone_hosting_repo` (`Repository::clone_repo` constructor — no repo handle to lock), `restart_periodic_fetch` (timer loop), `ai_ops::generate_commit_message` (3-stage diff→provider→generate pipeline). Net −186 lines (1284 → 1098 across the two files).
- **Sidebar state ownership consolidation** — done (uncommitted, +123/−73 across 6 files). `SidebarState` is now the single seam for its own state: toggle/filter/`set_context_menu` methods own the mutations, matching the peer-panel pattern (DiffPanel/GraphPanel/StatusPanel). `SidebarExpansion` sub-struct centralises the snapshot shape and the "which fields persist" rule (`apply_persisted_from_settings`/`write_persisted_to_settings`). `toggle_sidebar_worktrees` relocated from `dialog_ops.rs` to `git_ops.rs`. Collapsed 3 copies of the Settings↔SidebarState 4-boolean mapping (`app.rs` init + `save_settings` + `settings_ops` draft-apply) and 2 copies of the TabSnapshot 6-field mapping (`repo_session.rs`) into method calls; `TabSnapshot` now embeds one `sidebar_expansion` field. Investigation corrected the PLAN's scope: it understated the problem (missed the two duplicated sync layers — the real payoff) and incorrectly included `navigate_to_ref`, which is a graph-navigation action, not sidebar state, so it was left in `git_ops.rs`. `worktrees_expanded`'s non-persistence is now a visible omission in `write_persisted_to_settings` rather than silent.
- **`HostingProvider` trait — 3 parallel implementations** — done (uncommitted). Three-layer refactor: (1) new `http.rs` module with shared `make_client(headers)`, `ensure_success(response, context)`, and `paginate(client, url_fn, page_size, context, extract_fn, map_fn)` — killed 9 pagination loops + 12 status-check sites across all 3 providers. (2) Unified GitHub + Codeberg (which share ~95% of their API shape via Codeberg's Forgejo/Gitea lineage) into a single `GiteaStyleProvider` parameterised by a `GiteaStyleConfig` const — per-provider differences (auth scheme, page param, page size, JSON keys, response envelope, URL paths) are data, not code. Deleted `github.rs` (334 lines) and `codeberg.rs` (322 lines); `GitHubProvider`/`CodebergProvider` are now type aliases for `GiteaStyleProvider`. (3) GitLab adopted `http.rs` primitives but kept bespoke: `url_encode`, `fetch_project_id` cross-fork lookup, MR terminology (`merge_requests`/`iid`/`source_branch`). Adding a 4th Gitea-style provider is now ~30 lines (a config const + constructor), not ~320 lines. 30 characterization tests added (`tests/{github,codeberg,gitlab}_api.rs`) using `httpmock` — first tests in the hosting crate. Net −272 lines in `src/` (1367 → 1095).

---

## Active candidates

---

## New candidates

---

### 8. `patch.rs` — duplicated stage/unstage call sites

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
