# Open Repository Tab Bar

## Summary

Add a repository tab bar directly below the custom window title bar. Each opened repository appears as a tab sized to its repo name, new tabs are appended to the right, clicking a tab switches the active repository, and tabs have close icons. The app will persist the full open tab list plus the active tab so reopening GitForge restores the previous repo tabs.

## Current State

- `GitForgeApp` currently supports one active repository:
  - `open_repo: Arc<Mutex<Option<Repository>>>`
  - `repo_state: Option<RepoState>`
- Opening a repository replaces that single active repo.
- Settings currently persist only `last_repo_path: Option<String>`.
- The title bar is rendered by `crates/gitforge-app/src/views/titlebar.rs`.
- Main layout assembly happens in `crates/gitforge-app/src/views/app.rs`.
- Existing repo operations assume the single active repo through `self.open_repo` and `self.repo_state`.

## Data Model Changes

Add a tab/session model in `views/app.rs`:

```rust
struct OpenRepoTab {
    id: u64,
    path: std::path::PathBuf,
    repo: Arc<Mutex<Option<Repository>>>,
    repo_state: Option<RepoState>,
    loading: bool,
    last_error: Option<String>,
}
```

Add these fields to `GitForgeApp`:

```rust
open_repo_tabs: Vec<OpenRepoTab>,
active_repo_tab_id: Option<u64>,
next_repo_tab_id: u64,
```

Keep the existing `open_repo` and `repo_state` during the first implementation pass only if it reduces churn, but route them through helper methods so the active tab becomes the source of truth:

```rust
fn active_tab(&self) -> Option<&OpenRepoTab>;
fn active_tab_mut(&mut self) -> Option<&mut OpenRepoTab>;
fn active_repo_state(&self) -> Option<&RepoState>;
fn active_repo_handle(&self) -> Option<Arc<Mutex<Option<Repository>>>>;
```

After callers are migrated, remove or stop using the single-repo fields.

## Settings Changes

Update `AppSettings` in `views/settings.rs`:

```rust
pub open_repo_paths: Vec<String>,
pub active_repo_path: Option<String>,
```

Serde compatibility:

- Add `#[serde(default)]` to both new fields.
- Keep `last_repo_path` for backward compatibility.
- On load:
  - If `open_repo_paths` is empty and `last_repo_path` is present, seed `open_repo_paths` with `last_repo_path`.
  - If `active_repo_path` is missing, use the first path in `open_repo_paths`.
- On save:
  - Write `open_repo_paths` from current tabs in order.
  - Write `active_repo_path` from the active tab.
  - Also keep writing `last_repo_path` to the active path for compatibility.

## Startup Restore

In `GitForgeApp::new`:

1. Load settings.
2. Create the app with no active repo data.
3. After UI state setup, call a new `restore_open_repo_tabs(cx)` method.
4. For each persisted path:
   - Create a tab immediately with `loading = true`.
   - Start async repository discovery/loading.
   - When loaded, populate that tab’s `repo` and `repo_state`.
5. Select `active_repo_path` as the active tab.
6. If an active tab fails to load, keep the tab visible with an error state and show the error banner.
7. If every restore path fails, show the normal empty welcome state.

Default restore behavior: restore all previously open tabs, preserving order and active tab.

## Opening Repositories

Replace the current “open repo replaces active repo” behavior with “open repo adds or activates tab”:

1. User triggers Open Repository.
2. File dialog returns a folder.
3. Normalize/discover the repo path.
4. If that path is already open:
   - Switch to its tab.
   - Do not create a duplicate.
5. Otherwise:
   - Append a new tab to the right.
   - Mark it active.
   - Load repo data asynchronously into that tab.
6. Save settings after successful tab creation/load.

Apply this to:

- `spawn_open_dialog`
- `open_repo_from_path`
- Clone flows that currently call `open_repo_from_path`

## Switching Tabs

Add:

```rust
pub fn activate_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>);
```

Behavior:

- Set `active_repo_tab_id`.
- Rebuild visible panel data from that tab’s `repo_state`.
- Clear per-active view state that should not leak between repos:
  - diff panel selection/content
  - graph selected index if invalid for the new repo
  - status panel content from previous repo
- Preserve global UI state:
  - theme
  - titlebar menus
  - sidebar expansion preferences
  - command palette state
- Save settings.

Implementation detail: because `graph_panel`, `diff_panel`, and `status_panel` are currently global, switching tabs should call an adapted `apply_repo_state_to_panels(&RepoState)` for the active tab.

## Closing Tabs

Add:

```rust
pub fn close_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>);
```

Behavior:

- Remove the tab.
- If closing the active tab:
  - Activate the tab immediately to the left if one exists.
  - Otherwise activate the tab now at the same index.
  - Otherwise clear active repo state and show empty welcome state.
- Save settings.
- If the closed tab had in-flight loading, ignore the async result when it returns by checking whether the tab id still exists.

## Tab Bar UI

Create a new module:

```text
crates/gitforge-app/src/views/repo_tabs.rs
```

Public renderer:

```rust
pub fn render_repo_tab_bar(
    tabs: &[RepoTabView],
    active_tab_id: Option<u64>,
    colors: &AppColors,
    entity: WeakEntity<GitForgeApp>,
) -> impl IntoElement
```

Use a lightweight view model so the renderer does not depend on full repo internals:

```rust
pub struct RepoTabView {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub loading: bool,
    pub has_error: bool,
}
```

Layout requirements:

- Render directly below `titlebar` in `GitForgeApp::render`.
- Height: about `30px`.
- Full width.
- Horizontal row.
- Tabs are `flex_none`, not equal width.
- Width is content-driven by repo name with padding.
- New tabs append to the right by preserving `open_repo_tabs` order.
- Active tab uses selected/background styling.
- Inactive tabs use hover styling.
- Each tab includes:
  - repo name
  - optional loading indicator text or subtle spinner marker
  - close `x` icon using existing `assets/icons/x.svg`
- Close icon click stops propagation so it does not also activate the tab.
- Empty state: hide the tab bar when there are no open tabs.

## Title Bar Relationship

Keep `titlebar.rs` focused on the OS/window title bar.

The new visual stack in `GitForgeApp::render` becomes:

```rust
.child(titlebar)
.child(repo_tab_bar_if_any)
.child(error_banner_if_any)
.child(main_content)
.child(status_bar)
```

This satisfies “below the window title bar” without overloading the client-side drag/menu area.

## Active Repo Operation Routing

Update all repo operations that currently use `self.open_repo` or `self.repo_state` to use active-tab helpers.

Examples:

- `refresh_repository`
- `load_diff_for_selected`
- `load_status`
- branch/tag/stash/worktree actions
- remote/fetch/pull/push actions
- open in editor/terminal/browser
- blame
- custom commands

Default behavior when no active tab exists:

- Commands that require a repo no-op and set `last_error = Some("No repository open")` where user-visible feedback is appropriate.
- Open/clone/settings commands continue to work.

## Error Handling

For tab loading failures:

- Keep the tab in the tab bar.
- Mark `has_error = true`.
- Store the error in the tab.
- If the failed tab is active, show the existing error banner with `Failed to load repository: ...`.
- Closing the tab clears that error if it was active.

For duplicate paths:

- Do not create another tab.
- Activate the existing tab.
- If existing tab is in error state, retry loading it.

For deleted/moved repos on app restart:

- Create the tab, attempt load, then show it as errored if loading fails.
- User can close the tab with the close icon.

## Persistence Timing

Call `save_settings()` after:

- opening a new repo tab
- activating a tab
- closing a tab
- successful restore ordering/active selection changes
- existing sidebar expansion setting changes

Do not wait for app shutdown because there is no clear existing `Drop`/window-close settings save path.

## Public API / Interface Changes

Settings JSON gains:

```json
{
  "open_repo_paths": ["/path/to/repo-a", "/path/to/repo-b"],
  "active_repo_path": "/path/to/repo-b"
}
```

Existing setting retained:

```json
{
  "last_repo_path": "/path/to/repo-b"
}
```

Internal view module added:

```rust
mod repo_tabs;
```

New app methods:

```rust
pub fn activate_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>);
pub fn close_repo_tab(&mut self, tab_id: u64, cx: &mut Context<Self>);
```

## Tests And Verification

Add or update tests where the project currently supports them. If no UI test harness exists, verify with targeted build checks and manual runtime smoke testing.

Scenarios:

1. Open one repository:
   - One tab appears below title bar.
   - Tab width follows repo name.
   - Repository content loads normally.

2. Open second repository:
   - Second tab appears to the right.
   - Second tab becomes active.
   - First tab remains available.

3. Click first tab:
   - Active content switches back to first repo.
   - Graph/sidebar/diff/status reflect first repo.

4. Reopen existing repo path:
   - No duplicate tab is created.
   - Existing tab becomes active.

5. Close inactive tab:
   - Tab disappears.
   - Active tab remains unchanged.

6. Close active tab:
   - Neighbor tab becomes active.
   - If no tabs remain, app returns to empty welcome state.

7. Restart app:
   - All previously open tabs are restored in order.
   - Previously active repo is active.
   - Missing repos show errored tabs instead of silently disappearing.

8. Clone repo flow:
   - Newly cloned repo opens as a new rightmost active tab.

9. Run:
   - `cargo check`
   - `cargo test` if the workspace has meaningful tests

## Assumptions And Defaults

- Restore all open repo tabs, not just the active repo.
- Tabs include a close icon.
- Opening a repo that is already tabbed activates the existing tab instead of duplicating it.
- The tab bar is hidden when there are no open repos.
- Tab order is the order in which repos were opened, persisted across restarts.
- New tabs always append to the right.
- Repo tab labels use the final path component as the repo name.
