# ADR-0006: TabSession extraction and tab snapshot restore

- **Status:** Accepted
- **Date:** 2026-07-08
- **Supersedes:** —
- **Relates to:** Architecture review Candidate 02 ("Decompose `RepoSession` — tabs
  vs panel coordination") and Candidate 03 ("Unify tab snapshot restore with
  Selection Cascade"). Builds on ADR-0003 and ADR-0001.

## Context

`RepoSession` (~4,500 lines before this change) owned two jobs with no seam
between them:

1. **Tab lifecycle** — open tabs, active tab id, per-tab `TabSnapshot`, drag/
   reorder, recently-closed stack, repo handles and `RepoState` per tab.
2. **Panel coordination** — graph/status/diff panels, Selection Cascade,
   `apply_repo_state_to_panels`, overlay, sidebar expansion.

Tab switching (`activate_repo_tab`, `close_repo_tab`) saved and restored panel
state through a **parallel path** that copied six subsystems independently and
never called `cascade` (ADR-0003). Meanwhile `apply_active_repo_tab_to_view`
ran `apply_repo_state_to_panels` with the **outgoing** tab's graph selection
still live, so reselect/cascade could clear diff before restore put it back.
Overlay file-index normalization was duplicated in three call sites.

`TabSnapshot` also stored `status_selection` and `status_view_mode`, but graph-
staging mode is already defined by the Selection Cascade invariant — snapshotting
it duplicated knowledge and could drift from `cascade` rules.

## Decision

### 1. Extract `TabSession`

Promote tab lifecycle to `TabSession` (`crates/gitforge-app/src/views/tab_session.rs`),
owned by `RepoSession` as `tabs: TabSession`.

| `TabSession` | `RepoSession` |
|---|---|
| `open_repo_tabs`, `active_repo_tab_id`, `next_repo_tab_id` | `graph_panel`, `diff_panel`, `status_panel`, … |
| `closed_repo_tabs`, drag/drop state | Selection Cascade (`cascade`, `sync_status_for_selection`) |
| `OpenRepoTab`, `TabSnapshot` storage | `save_snapshot_to_active_tab` / `restore_snapshot_from_tab` (reads/writes panels, stores via `tabs`) |
| Pure reorder: `reorder_repo_tab`, `move_repo_tab_to_end`, `drop_caret_index` | `apply_repo_state`, `apply_repo_state_to_panels` |

`RepoSession::active_tab()` delegates to `tabs.active_tab()`. Tab-bar and tab-op
call sites reach through `repo_session.tabs.*`; panel ops keep using
`repo_session.active_tab()` and panel fields.

### 2. Defer reselect on tab switch (`RefreshReselectPolicy::DeferToSnapshot`)

`apply_active_repo_tab_to_view(DeferToSnapshot)` rebuilds graph data and
refreshes status file lists but **does not** re-select from the outgoing tab's
graph selection. On this path `set_status` uses `preserve_staging = false` so
the outgoing tab's graph-staging mode does not leak into the incoming tab.

Selection is restored solely by `restore_snapshot_from_tab` (or
`apply_incoming_tab_after_switch`, which pairs defer + restore).

### 3. Unify restore with Selection Cascade — `PreservedTab`

Tab restore mirrors ADR-0003's `PreservedCommit` bypass:

- After restoring `view_mode`, graph selection, and editor state, if
  `preserved_tab_diff` holds (snapshot `diff_state.commit_id` matches restored
  commit selection), call `sync_status_for_selection` and restore diff from
  snapshot — **skip `cascade`** so ADR-0001's diff cache stays valid.
- Otherwise call `cascade(sel)` and return `SelectionEffect` for
  `GitForgeApp::apply_selection_effect` to spawn.

`normalized_overlay_file_idx` is a single shared helper (was duplicated in
`git_ops.rs` and restore).

### 4. Hybrid `TabSnapshot` — graph + view_mode, not status fields

Remove `status_selection` and `status_view_mode` from `TabSnapshot`. Graph-
staging mode is derived via `sync_status_for_selection` (extracted from
`cascade`; does not touch `diff_panel`). Full `cascade` still runs on the
non-preserved path.

**Explicit trade-off:** status-panel file selection / `StatusViewMode::Diff`
while in `MainViewMode::Status` is **not** preserved across tab switches. Only
the cascade-owned graph-staging relationship is derived. Restoring status UI
state would require a separate `StatusUiSnapshot` outside the cascade seam.

## Scope boundary (explicit non-goals)

- **`RepoSession` panel reach-ins stay direct** (ADR-0003 non-goals). `TabSession`
  is not a façade over panels.
- **No persistence format migration** for saved `TabSnapshot` on disk — snapshots
  are in-memory per session only; field removal is not a serde break.
- **Status-view file selection** across tab switches — deferred (see trade-off
  above).

## Behaviour changes

1. **Tab switch no longer applies outgoing selection during rebuild** — defer
   path skips `reselect_after_refresh`; incoming tab selection comes only from
   restore.
2. **Outgoing graph-staging no longer leaks** — `preserve_staging = false` on
   defer; restore derives staging from graph + `view_mode`.
3. **Preserved diff survives tab switch without clear-then-restore** — when
   `preserved_tab_diff` matches, diff cache is kept (ADR-0001 intent).
4. **Non-preserved restore may spawn diff load** — `cascade` →
   `LoadDiffForSelected` is acted on via `apply_selection_effect` in `tab_ops`
   (previously unspawned on tab switch when diff was absent from snapshot).

## Consequences

### Positive

- Tab bugs have a named home (`TabSession`); panel/cascade bugs stay on
  `RepoSession`. Locality for tab switch, drag, and closed-tab stack.
- Tab restore strengthens ADR-0003 without reopening its non-goals — restore
  routes through `cascade` or `PreservedTab` + `sync_status_for_selection`.
- `TabSnapshot` interface shrinks; status staging is derived, not copied.
- Reorder/caret pure functions and tests live with tab data (`tab_session.rs`).

### Negative / deferred

- Two modules to learn (`TabSession` + `RepoSession`) instead of one god-object.
  Mitigated by `CONTEXT.md` glossary and `repo_session.tabs` field name.
- Status-view UI state not preserved on tab switch (documented trade-off).
- `save_snapshot_to_active_tab` / `restore_snapshot_from_tab` remain on
  `RepoSession` because they bridge panels — a future deepening could pass a
  panel snapshot struct across the seam if the bridge grows further.

## Verification

- `cargo test -p gitforge-app` — green, including:
  - `tab_session::reorder_tests` (20 tests, moved from `repo_session`)
  - `tab_restore_preserves_cached_diff_when_preserved`
  - `tab_restore_derives_graph_staging_for_uncommitted`
  - `defer_reselect_does_not_leak_outgoing_graph_staging`
  - existing ADR-0003 cascade tests unchanged
