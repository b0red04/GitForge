# ADR-0003: Selection Cascade

- **Status:** Accepted
- **Date:** 2026-06-29
- **Supersedes:** —
- **Relates to:** Architecture review Candidate 02 ("Give cross-panel selection
  coordination a single home")

## Context

The invariant "graph selection, status-panel mode, diff-panel state, and
`view_mode` move together" was, before this ADR, re-implemented at every site
that touched selection:

- `RepoSession::apply_repo_state_to_panels` (the snapshot path, run after a
  fetch) captured the prior selection, rebuilt the graph, then branched on
  `reselect_after_refresh` to re-select — performing the four-step panel dance
  inline at each branch.
- `GitForgeApp::on_graph_selection_changed` (the keyboard reaction) read
  `graph_panel.selection()` and performed the same dance inline, specialised
  for the "graph already moved" case.
- `GitForgeApp::select_uncommitted` and `select_commit` (the click /
  programmatic path) performed the dance a third and fourth time.

`RepoSession`'s panels were `pub(crate)`, so the seam between the app and the
panels was a namespace, not an interface. Around sixty call sites of the form
`self.repo_session.<panel>.<method>` reached straight through. The
"panels are private, behaviour goes through methods" claim in `CONTEXT.md` was
aspirational for selection — it held at the render seam but not at the
coordination seam.

## Decision

Promote one private method, `RepoSession::cascade`, to be the single home for
the selection invariant. It takes a `GraphSelection` and makes `status_panel`
(enter/exit graph-staging, gated on `view_mode`) and `diff_panel` (clear)
consistent with it. The cascade does **not** write `graph_panel` — the
selection is its input, not its output.

Graph selection writes funnel through one private method,
`RepoSession::write_graph_selection`, which is the only place that calls
`graph_panel.select_*`. Public entry methods compose writes and cascades:

| Entry | Writes graph | Forces history view | Cascades |
|-------|--------------|---------------------|----------|
| `set_selection(sel)` | via `apply_graph_selection` | yes | yes |
| `navigate_selection_delta(delta)` | via `apply_graph_selection` | no | yes |
| `apply_graph_selection(sel)` | yes | no | yes |
| Refresh `PreservedCommit` | `write_graph_selection` only | no | **no** (cache) |
| Tab snapshot restore | `write_graph_selection` only | no | maybe later |

- `RepoSession::write_graph_selection(sel)` — graph write only. Used when the
  cascade must be skipped (`PreservedCommit`, ADR-0001 cache) or deferred
  (tab snapshot restore before the caller decides whether to cascade).
- `RepoSession::apply_graph_selection(sel)` — writes the graph, then calls
  `cascade(sel)`. Does not touch `view_mode`.
- `RepoSession::set_selection(sel)` — for clicks and programmatic selection.
  Forces `view_mode = CommitHistory` (explicit navigation), then calls
  `apply_graph_selection(sel)`.
- `RepoSession::navigate_selection_delta(delta)` — for keyboard navigation.
  Proposes the next selection via the pure `GraphPanelModel::propose_delta`
  (no mutation), then applies through `apply_graph_selection`. Does not touch
  `view_mode` (the user may be in Status view while arrowing the graph).

`GraphPanel`'s mutating `select_*` methods are `pub(crate)` so only
`RepoSession` can write graph selection.

The refresh path (`apply_repo_state_to_panels`) keeps its rebuild +
preservation responsibilities. After `reselect_after_refresh` decides the
post-refresh selection it routes through `write_graph_selection` or
`apply_graph_selection` as appropriate. It does **not** force `view_mode` — a
refresh while the user is in Status view must not yank them back to
CommitHistory.

The public entries return a `SelectionEffect`:

```rust
pub(crate) enum SelectionEffect {
    ClearDiff,           // cascade already cleared diff_panel; caller notifies
    LoadDiffForSelected, // caller calls load_diff_for_selected(cx)
}
```

`RepoSession` stays GPUI-free. The async diff load stays on
`GitForgeApp` (which has the `&mut Context` needed to spawn); a thin
`apply_selection_effect` helper interprets the effect.

The snapshot path (`apply_repo_state_to_panels`) returns `()` rather than
`SelectionEffect`. Its cascade outcomes are always `ClearDiff` (a "notify"),
and its callers (`apply_repo_state` → `refresh_repository`, and
`apply_active_repo_tab_to_view` → `tab_ops`) already call `cx.notify()`,
which is all `ClearDiff` asks for. Propagating `SelectionEffect` through the
six `tab_ops` call sites of `apply_active_repo_tab_to_view` was judged
disproportionate plumbing for a value that only ever means "notify."

### `PreservedCommit` bypasses the cascade

`reselect_after_refresh` returns `RefreshSelection::PreservedCommit(idx)`
when the user's previously-selected commit is still present after a fetch.
Because commits are immutable, that commit's cached diff is still valid, so
the refresh path:

1. Calls `write_graph_selection(Commit(idx))` (re-selects at the possibly-
   shifted index).
2. Does **not** call `cascade` — the invariant already held before the
   refresh and still holds. Calling `cascade` would `diff_panel.clear()` and
   force a reload, defeating ADR-0001's cache.
3. Returns `()` (no async work; the caller's `cx.notify()` suffices).

### `view_mode` is read by the cascade, written by entries

`view_mode` is written by three different policies: `set_selection` forces
`CommitHistory` (explicit navigation); the refresh and keyboard paths never
write it; the status-view action handler writes `Status` orthogonally. The
cascade **reads** `view_mode` to gate `enter_graph_staging` (only enter
staging when in history view) but never writes it. The invariant the cascade
enforces is therefore "given the current `view_mode`, the panels are
consistent" — not "`view_mode` is always CommitHistory."

## Scope boundary (explicit non-goals)

- The ~60 other `self.repo_session.<panel>.<method>` reach-ins across `ops/`
  are **not** selection coordination (`set_code_view`, `set_blame`,
  `select_file`, `clear`, etc.). They stay direct. The cascade is the single
  home for the *selection invariant*, not a full panel façade. Wrapping every
  panel mutation would turn `RepoSession` into a shallow pass-through
  namespace — the opposite of deepening.

## Behaviour changes

1. **`GraphSelection::None` does not `exit_graph_staging`** — this matches
   the pre-ADR behaviour. If `None` should also exit staging, `cascade` is
   the single place to add it.
2. **Click path is unchanged** — `set_selection` writes
   `view_mode = CommitHistory` *before* it calls `cascade`, so the cascade
   always runs with history view active on the click / programmatic path and
   always enters/exits staging exactly as `select_commit` /
   `select_uncommitted` did pre-ADR. The cascade's `view_mode` gating is
   therefore never exercised here; it only matters on the keyboard path
   (`navigate_selection_delta`) and the refresh path
   (`apply_repo_state_to_panels`), where `view_mode` is not forced.

## Consequences

### Positive

- The four-step selection dance lives in exactly one tested private method
  (`cascade`). Adding a new entry path means writing a thin wrapper, not
  re-implementing the invariant.
- Graph selection has a single write authority (`write_graph_selection`).
  Keyboard navigation no longer mutates the graph panel before cascading.
- The "panels are private, behaviour goes through methods" claim in
  `CONTEXT.md` becomes true end-to-end for selection, not just at the render
  seam.
- The cascade is testable via the existing `#[gpui::test]` +
  `TestAppContext` fixture pattern (same as `active_repo_ready_tests`),
  asserting on observable outcomes (`is_graph_staging()`, `diff_state()`,
  `view_mode`, `SelectionEffect`).
- The async spawn stays on `GitForgeApp` (where `&mut Context` is available);
  `RepoSession` stays GPUI-free and unit-test-friendly, matching the
  `reselect_after_refresh` precedent.

### Negative / deferred

- Three public entry methods (`set_selection`, `apply_graph_selection`,
  `navigate_selection_delta`) plus one write-only helper
  (`write_graph_selection`). The matrix reads honestly — click, keyboard,
  refresh, and cache-bypass paths really do differ — but it is more surface
  area than a single entry point would be.
- `view_mode` remains a `pub(crate)` field on `RepoSession` rather than being
  fully encapsulated behind the cascade. This is deliberate (the status-view
  action handler is an orthogonal writer), but means the cascade's gating
  depends on callers having set `view_mode` correctly for their context.
