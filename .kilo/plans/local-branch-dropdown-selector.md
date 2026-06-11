# Plan: Local Branch Dropdown Selector

## Goal
Add a GitKraken-style local branch selector that lets users switch between local branches and shows a conflict indicator beside any local branch that would conflict when merged with the repository's main branch.

## Current Context
- The current branch is rendered in `crates/gitforge-app/src/views/titlebar.rs` as a static breadcrumb segment.
- Branch checkout already exists through `GitForgeApp::checkout_branch` in `crates/gitforge-app/src/views/ops/git_ops.rs` and `Repository::checkout_branch` in `crates/gitforge-git/src/repository/branch_impl.rs`.
- Local branch data is already available in `RepoState.references` as `RefInfo { kind: RefKind::Branch, is_head, ... }`.
- A merge-conflict SVG already exists at `assets/icons/git_merge_conflict.svg` and can be reused/styled with the theme warning/error color.

## Implementation Steps

1. Extend repository snapshot data for branch conflicts
   - Add a field to `gitforge_git::RepoState`, e.g. `conflicting_local_branches: std::collections::HashSet<String>`.
   - Populate it during `RepoState::from_repository_with_options` after `repo.references()` is loaded.
   - Keep this snapshot-based so the dropdown renders synchronously from `RepoState` and refreshes after checkout/fetch/refresh operations.

2. Add conflict detection in `gitforge-git`
   - Add repository helpers in `crates/gitforge-git/src/repository/branch_impl.rs`:
     - `main_branch_name(&self) -> GitResult<Option<String>>` that prefers local `main`, then local `master`, then remote `origin/main`, then `origin/master` if needed.
     - `local_branches_conflicting_with_main(&self, branches: &[String]) -> GitResult<HashSet<String>>`.
     - `branch_conflicts_with_base(&self, base: &str, branch: &str) -> GitResult<bool>`.
   - Use a non-working-tree merge simulation, preferably modern `git merge-tree --write-tree <base> <branch>` or `git merge-tree --name-only <base> <branch>` depending on available Git semantics; treat a conflict exit/status as `true`, clean merge as `false`, and unexpected command failures as non-fatal warnings rather than blocking repository loading.
   - Skip the base branch itself and any branch whose name equals the current main/master base.
   - Keep branch names passed as args, not interpolated into shell strings.

3. Add UI state for dropdown open/closed
   - Add `local_branch_dropdown_open: bool` to `GitForgeApp` in `crates/gitforge-app/src/views/app.rs`.
   - Add small action-handler methods in `crates/gitforge-app/src/views/ops/action_handlers.rs` or nearby app ops:
     - `toggle_local_branch_dropdown`
     - `close_local_branch_dropdown`
   - Close this dropdown when opening titlebar menus, toolbar more menu, dialogs, or after selecting a branch.

4. Replace static branch breadcrumb with a branch selector button
   - Update `render_titlebar` and helper functions in `crates/gitforge-app/src/views/titlebar.rs` to accept:
     - the app entity,
     - whether the local branch dropdown is open.
   - Render the current branch area as a compact button matching the GitKraken pattern:
     - branch icon on the left,
     - selected branch name,
     - down-chevron on the right,
     - active/hover background and rounded border using existing theme colors.
   - Keep detached HEAD behavior: show `(detached)` as disabled/non-clickable or clickable with an empty dropdown message, whichever fits existing style best.

5. Render the branch dropdown overlay
   - Add a `render_local_branch_dropdown(...)` function in `titlebar.rs`, similar to existing `render_titlebar_menu_dropdown`.
   - Position it under the branch selector area in the titlebar with an absolute panel, dark/surface background, subtle border, rounded corners, shadow-like contrast via `surface_high`, and GitKraken-like dense rows.
   - List only `RefKind::Branch` references, sorted with the current branch first or using existing reference order.
   - Each row should include:
     - branch icon,
     - branch name,
     - current-branch check/highlight,
     - conflict indicator icon (`icons/git_merge_conflict.svg`) tinted warning/error when `RepoState.conflicting_local_branches` contains that branch.
   - On row click, call `this.checkout_branch(name, cx)`, close the dropdown, and rely on existing refresh behavior after checkout.

6. Integrate overlay into app rendering
   - In `crates/gitforge-app/src/views/app.rs`, pass `self.local_branch_dropdown_open` into `render_titlebar`.
   - After existing titlebar menu overlay rendering, conditionally render `render_local_branch_dropdown(active_repo_state, &self.colors, entity.clone())` when `local_branch_dropdown_open` is true.
   - Ensure click handlers stop propagation where needed so selecting/toggling the dropdown does not trigger titlebar drag behavior.

7. Styling details
   - Use existing icons (`icons/git-branch.svg`, `icons/chevron-down.svg`, `icons/check.svg`, `icons/git_merge_conflict.svg`).
   - Use `AppColors` tokens only; likely `surface`, `surface_high`, `border`, `text`, `text_muted`, `warning`/`error`, and `accent`.
   - Match the GitKraken visual cue by making the conflict indicator compact, high-contrast, and adjacent to the branch entry rather than using text labels.

8. Verification
   - Run `cargo fmt`.
   - Run `cargo check -p gitforge-git` and `cargo check -p gitforge-app`.
   - If practical, add or update lightweight unit tests for the branch conflict helper command parsing only if the helper is structured testably without invoking Git.

## Risks / Notes
- Exact GitKraken screenshot dimensions are not available in the workspace, so the implementation should approximate the interface using the existing titlebar/menu styling and GitKraken-like compact branch rows.
- Conflict detection via `git merge-tree` depends on the installed Git version. The implementation should fail soft: absence of `merge-tree` or unsupported flags should simply omit indicators and log a warning rather than breaking repository load.
- Computing conflicts for many branches can be expensive. If performance becomes an issue, move conflict checks to a background cache after the first implementation; for now, snapshot-time detection is the simplest reviewable approach.