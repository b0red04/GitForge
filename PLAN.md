# Architecture Deepening Plan

Candidate refactors that turn shallow modules into deep ones. Each is evaluated by the deletion test: if deleting the module concentrates complexity elsewhere, it was earning its keep.

---

## Completed

- **RepoSession extraction** — done in `64297f7`, finished by collapsing the duplicated `apply_repo_state` (branch `opencode/finish-repo-session-apply-state`). `RepoSession::apply_repo_state` is now the single seam for installing a fresh `RepoState`.
- **CommitEditor extraction** — done in `1f89936`. Owns commit message buffer, cursor, AI alternatives, rendering at `views/commit_editor.rs`.
- **UTF-8 diff truncation** — fixed in `0290776`.
- **Command dispatch** — already typed. `CommandAction` is an enum (`commands.rs:56-91`), `CommandEntry.action: CommandAction`, dispatch via `.on_action(cx.listener(...))`. The earlier "string-typed" claim was wrong; typos are compile errors. (Note: `FetchAll`, `PushCurrent`, `PullCurrent` are bound and handled but missing from the `CommandAction` enum, so they don't reach the palette/menu — a small consistency gap, not a deepening issue.)
- **i18n removal + syntax simplification** — done in `56f7c5d`.

---

## Active candidates

### 1. Text Input Duplication (8-9 sites, growing)

**Files:** `sidebar.rs:396-492` (`render_search_bar`), `commit_editor.rs:69-338` (`CommitEditor::render`), `command_palette.rs:99-285` (`CommandPalette::render`), `settings_window.rs:1176-1312` (settings search), `:1643-1703` (`text_field_control`), `:1855-1965` (`api_key_field_control`), `ops/dialog_render.rs:113-215` (main dialog input), `:687-944` (`render_create_worktree_overlay`, two inputs)

**Problem:** Eight independent implementations of "GPUI text input": `FocusHandle` + `track_focus`, display text with cursor character `\u{2502}`, `on_key_down` handling backspace/escape/character, placeholder text. There is no shared `TextInput` in `gitforge-ui` — `crates/gitforge-ui/src/components.rs` is a dead file referencing nonexistent submodules. The duplication has grown from 4 to 8-9 sites since the prior review; new copies appear in `settings_window.rs` (search + API key) and `dialog_render.rs` (generic dialog + two worktree inputs).

**Solution:** A `TextInput` module in `gitforge-ui` owning focus, cursor position, placeholder, and rendering behind an interface like `new(placeholder)`, `set_text(&mut self, &str)`, `text() -> &str`, `handle_key_down(...)`, `render(colors) -> Element`. Each current site becomes a `TextInput` instance.

**Benefits:**
- **Locality** — text handling bugs fixed once instead of 8-9 times.
- **Leverage** — sidebar search, commit editor, command palette, settings, dialogs all gain features (selection, clipboard paste, multi-byte safety) from one change. Tests can exercise keyboard → text → render through one interface.

---

### 2. Dialog System — 19-arm dispatcher + 5× button pair

**Files:** `app.rs:64-113` (`AppDialog` enum, 20 variants incl. `None`), `ops/dialog_ops.rs:61-207` (`confirm_dialog`, 19-arm match), `ops/dialog_render.rs` (1043 lines, six `render_*_overlay` functions)

**Problem:** Adding a new dialog requires edits in 3 files / 5+ sites: enum variant, `open_*_dialog` method, `confirm_dialog` match arm, title match arm, placeholder match arm, and (for non-standard dialogs) a copy-pasted 90-260 line `render_*_overlay`. The duplications inside `dialog_render.rs`:
- Input key handler (enter/escape/backspace/char): 3 full copies (`:171-208`, `:776-812`, `:840-878`) + 1 partial (`:602-622`).
- Cancel + action button pair: 5 copies (`:216-261`, `:498-550`, `:636-683`, `:886-931`, `:985-1030`).
- Overlay wrapper (`dialog-overlay` div): 6 copies.
- Color-binding preamble: 6 copies.
- `dialog-box` surface wrapper: 6 copies.
- Three parallel `AppDialog` match dispatches inside `dialog_render.rs` (title, placeholder, hosting-title).

**Solution:** Each dialog becomes a self-contained module implementing a `Dialog` trait with `open()`, `confirm(input, cx)`, `cancel()`, `render(input, colors, entity)`. `AppDialog` becomes `Box<dyn Dialog>`. Shared rendering primitives (input handler, button pair, overlay wrapper) move to `gitforge-ui`.

**Benefits:**
- **Locality** — all knowledge of "what CreateBranch does" lives in one file. Adding a dialog is one file, one struct.
- **Leverage** — shared primitives benefit all dialogs simultaneously. Tests exercise a dialog's confirm logic through the trait interface.

---

### 3. Diff Rendering Split Across Two Panels

**Files:** `diff_panel.rs:764-999` (`render_diff_content`), `status_panel.rs:652-770` (`render_selected_diff`), `diff_view.rs` (350 lines, shared utility)

**Problem:** Both panels independently own diff display state (scroll handle, line selection, view mode). Both repeat the path-label resolution `diff.new_path.as_deref().or(diff.old_path.as_deref()).unwrap_or("(unknown)")` (and `diff_panel.rs:419-422` has a third copy). Both build near-identical `on_click` closures and file-header scaffolds. The shared `diff_view.rs` covers only the line list and empty state — not headers, mode toggle, or per-panel action buttons.

**Corrections to the prior claim:** "View File"/"Blame" buttons and binary/LFS detection exist **only in `DiffPanel`**; `StatusPanel` has "Stage/Unstage Lines" instead and renders binary/LFS files as raw lines (a real bug). Syntax highlighting is passed `Some(...)` by `DiffPanel` and `None` by `StatusPanel` — same `render_diff_lines` call, different behaviour. `StatusViewMode::Code` is a placeholder (`status_panel.rs:234-236`), unimplemented. The cached mirror (`DiffViewMirror`) wraps `DiffPanel` only; `StatusPanel` re-renders every frame.

**Solution:** Extract a `DiffViewer` module owning `FileDiff`, `DiffLineSelection`, `DiffViewMode`, scroll handles, and the full rendering pipeline. Both panels embed a `DiffViewer`. Interface: `set_diff(FileDiff)`, `select_line()`, `selected_range()`, `render(colors, entity)`.

**Benefits:**
- **Locality** — diff display bugs fixed once; binary/LFS handling reaches the status panel.
- **Leverage** — improvements to diff rendering (syntax highlighting, expand-context) apply everywhere diffs are shown.

---

### 4. gitforge-git Error Type — Single String Variant

**Files:** `gitforge-git/src/error.rs` (3 variants), used across all `*_impl.rs` files

**Problem:** `GitError` has 3 variants: `RepositoryNotFound(String)`, `OperationFailed(String)`, `Io(#[from] std::io::Error)`. `OperationFailed` absorbs **99%** of constructions (98/99 across 12 files). Structured gix error info is discarded via `.map_err(|e| GitError::OperationFailed(e.to_string()))` — 29 exact-form occurrences plus 22 more `format!`-wrapped variants. The `Io` variant is **dead** — no `?`-triggering call site exists. (Note: the prior claim that `InvalidReference` and `MergeConflict` variants exist-but-unused was wrong — those variants were never defined. They are candidates to add, not delete.)

**Solution:** Introduce domain-specific variants: `MergeConflict { paths: Vec<String> }`, `AuthenticationFailed { remote: String }`, `NetworkError { source: String }`, `IndexLock { path: PathBuf }`, `EmptyCommit`, `BranchNotFound { name: String }`, plus the missing `InvalidReference`. gix error mapping preserves structured cause. Delete the dead `Io` variant.

**Benefits:**
- **Leverage** — app shows "Merge conflict in 3 files" instead of "Operation failed: ...git merge...exit status 1".
- **Locality** — error classification concentrates in `gitforge-git` instead of being parsed from strings in the app layer.

---

### 5. Dead Code — Multiple Sites

**Files:** Multiple across crates

**Problem:** Several types and modules add cognitive overhead without earning their keep:
- `gitforge-git` — `GitError::Io` variant (dead — see candidate 4).
- `gitforge-diff/src/patch.rs:86-135` — `extract_hunk_patch` has zero callers; re-exported from `lib.rs:5`.
- `gitforge-hosting/src/provider.rs:12, 21, 22` — `list_org_repos`, `file_url`, `commit_url` trait methods implemented in all 3 providers but called from nowhere in the app (9 dead bodies).
- `gitforge-hosting/src/lib.rs:23` — `find_account` has zero callers (the app has its own `find_hosting_account`).
- `gitforge-ui/src/components.rs` — dead file referencing nonexistent submodules (`sidebar`, `graph_panel`, etc.); not declared in `lib.rs`, not compiled.
- `crates/gitforge-syntax` — `SyntaxTheme` and `TokenColor` may be unused (verify after `56f7c5d`).
- `crates/gitforge-diff` — `anyhow` and `tracing` declared as dependencies but unused.

**Solution:** Delete unused code. If a variant is planned for future use, add a comment and a tracking issue.

**Benefits:**
- **Locality** — less code to read means faster understanding. The deletion test confirms these concentrate no complexity elsewhere.

---

## New candidates

### 6. `tab_ops.rs` — 11-method delegation facade

**Files:** `ops/tab_ops.rs:15-57` (forwarders) vs `:59-360` (real lifecycle logic)

**Problem:** Eleven one-line forwarders (`active_tab`, `active_tab_mut`, `active_repo_state`, `active_repo_handle`, `require_active_repo_handle`, `repo_tab_views`, `normalize_repo_path`, `find_tab_by_path`, `clear_repo_panels`, `clear_active_repo_view`, `apply_active_repo_tab_to_view`) whose bodies are literally `self.repo_session.X()`. Reached ~60× across the codebase. They dilute `tab_ops.rs`'s actual purpose (tab lifecycle: `start_loading_repo_tab`, `finish_repo_tab_load`, `activate_repo_tab`, `close_repo_tab`).

**Solution:** Delete the forwarders; callers write `self.repo_session.active_tab()` etc. directly. Keep the lifecycle methods.

**Benefits:**
- **Deletion test** — forwarders fail cleanly. Deleting them concentrates no complexity; the call gets one token longer and loses one layer of indirection.

---

### 7. Sidebar state ownership has no home — mutations scattered across 3 files

**Files:** `ops/git_ops.rs:166-243` (sidebar toggles, filter, navigate_to_ref), `ops/dialog_ops.rs:221-225` (`toggle_sidebar_worktrees` — wrong file), `action_handlers.rs`, rendered by `sidebar.rs` (1093 lines, deep)

**Problem:** `SidebarState` exists to own sidebar state, but its mutations live in `git_ops.rs`, `dialog_ops.rs`, and `action_handlers.rs`. Three of four section toggles are in `git_ops.rs`; the fourth (`worktrees`) is in `dialog_ops.rs`. There is no single seam for sidebar transitions, despite `SidebarState` existing precisely to be that owner.

**Solution:** Move sidebar mutations into `SidebarState` methods. Each current caller (`toggle_sidebar_branches`, `update_sidebar_filter`, etc.) becomes a method on the owner.

**Benefits:**
- **Locality** — sidebar state transitions concentrate in one module. A reader debugging "why didn't the worktrees section toggle" goes to one place, not three.
- Each individual toggle fails the deletion test alone (one-liner + notify). As a consolidated group inside `SidebarState`, they pass — the friction is the missing owner.

---

### 8. `run_git_op` abstraction has its seam in the wrong place — 11 data-returning ops rebuild the spawn scaffold

**Files:** `ops/git_ops.rs:519-609` (`run_git_op`, `run_git_op_with_status`), 11 hand-rolled duplicates at `git_ops.rs:124, 260, 332, 381, 790, 852, 890, 940` + `hosting_ops.rs:52, 120, 177, 231, 290` + `ai_ops.rs:33-133`

**Problem:** The two helpers cover only void-returning "do git op then refresh" ops. Anything that returns data (file bytes, diff text, RepoState, blame lines) re-implements `cx.spawn` + `spawn_blocking` + `open_repo.lock()` + the 3-arm `Ok(Ok(_))/Ok(Err(_))/Err(_)` match + `tracing::error!` pair + `this.update`. The pattern is duplicated ~14 times. `let repo_lock = open_repo.lock();` appears 11 times; `"No repository open"` wrapped in `GitError::OperationFailed` 9 times; the "task panicked" match arm duplicated 12 times.

**Solution:** Generalise the helpers to thread a result back to the `this.update` closure — e.g., `run_git_op_with_result<F, T>(open_repo, op: F, on_done: impl FnOnce(&mut Self, Result<T>) + 'static)`.

**Benefits:**
- The two existing helpers pass the deletion test (15 call sites each); the hand-rolled spawns each wrap a real operation, so individually they pass — but the **scaffolding around the unique body** is duplicated. The seam needs to move to admit a returned value.

---

### 9. `HostingProvider` trait — dead surface + 3 parallel implementations

**Files:** `gitforge-hosting/src/provider.rs:6-23` (trait, 8 methods), `github.rs` (247 lines), `gitlab.rs` (248), `codeberg.rs` (214)

**Problem:** Only 6 of 8 trait methods are exercised by the app (`name`, `authenticate`, `list_repos`, `search_repos`, `create_fork`, `repo_url` once). `list_org_repos`, `file_url`, `commit_url` are implemented in all 3 providers but called from nowhere — 9 dead method bodies. Separately, the 3 implementations are near-parallel copies: each has its own `make_client` (only auth header differs — `Bearer`/`PRIVATE-TOKEN`/`token`), paginated `list_repos` loop (only URL/page size differs — 100/100/50), `json_to_remote_repo` (only JSON keys differ — e.g. GitLab's `last_activity_at` vs GitHub's `updated_at`, Codeberg's `data` envelope), and identical `create_fork` error-extractor.

**Solution:** Delete the 3 dead trait methods. Lift the shared "authenticate, paginate, map JSON" shape into a generic helper that takes per-provider string templates. Each provider becomes a thin adapter of templates + JSON key mappings.

**Benefits:**
- Dead methods fail the deletion test cleanly. Providers individually pass (provider-specific JSON quirks), so this is "the trait promised more than callers need, and the shared shape was never lifted out."

---

### 10. `patch.rs` — duplicated stage/unstage call sites

**Files:** `gitforge-diff/src/patch.rs:3-84` (`extract_patch_from_selection`), `ops/git_ops.rs:448-474` (`stage_selected_lines`), `:476-502` (`unstage_selected_lines`)

**Problem:** The two call sites are 25 lines each and differ in exactly 3 tokens (function name, label string `"Stage lines"`/`"Unstage lines"`, final boolean to `apply_patch`). The other 22 lines (fetch current diff, clone, get selected indices, extract path, build `--- a/{}\n+++ b/{}\n{}` header, call `run_git_op`) are duplicated verbatim. (`extract_hunk_patch` in the same file is dead — covered by candidate 5.)

**Solution:** Extract the shared preamble into a helper that takes the per-operation bits (label, the underlying `Repository::stage_lines`/`unstage_lines` selector, the boolean).

**Benefits:**
- The two callers individually pass the deletion test (real distinct operations) but the duplicated 22-line preamble does not earn its keep.

---

## Cross-cutting notes (not deepening candidates, but worth fixing)

- **Missing ADR** — `CONTEXT.md` references `docs/adr/0001-diff-panel-cached-mirror.md`, but the file does not exist and the directory is empty. Either write the ADR (documenting the `DiffViewMirror` / `DiffSnapshot` / `DiffViewKey` caching layer at `diff_panel.rs:102-176`) or remove the reference.
- **Palette gap** — `FetchAll`, `PushCurrent`, `PullCurrent` are bound to keys, have `.on_action` handlers, and have `git_ops` implementations, but are absent from the `CommandAction` enum — so they don't reach the command palette or menus.
- **Placeholder view mode** — `StatusViewMode::Code` is a render placeholder (`status_panel.rs:234-236`); either implement it or remove it.
- **Pre-existing clippy error** — `graph_panel.rs:154` has a logic bug in branch-name matching (`r.name == *branch_name || (r.kind == RefKind::RemoteBranch && r.name == *branch_name)` — the second clause is redundant). Unrelated to any candidate but blocks `-D warnings`.
