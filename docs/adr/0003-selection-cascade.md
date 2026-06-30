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
selection is its input, not its output, so whoever calls the cascade is
responsible for having set `graph_panel.selection` first (or in the same
atomic update).

Two public entry methods funnel through `cascade`:

- `RepoSession::set_selection(sel)` — for clicks and programmatic selection.
  Writes `graph_panel`, forces `view_mode = CommitHistory` (explicit
  navigation), then calls `cascade(sel)`.
- `RepoSession::cascade_current()` — for keyboard navigation. The graph panel
  has already moved its own selection via `select_prev`/`select_next`, so this
  reads `graph_panel.selection()` and calls `cascade(sel)` without re-writing
  the graph.

The snapshot path (`apply_repo_state_to_panels`) keeps its rebuild +
preservation responsibilities, but after `reselect_after_refresh` decides the
post-refresh selection it writes `graph_panel` in its match arms and calls
`cascade(sel)` directly. It does **not** force `view_mode` — a refresh while
the user is in Status view must not yank them back to CommitHistory.

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
the snapshot path:

1. Calls `graph_panel.select_commit(idx)` (re-selects at the possibly-shifted
   index).
2. Does **not** call `cascade` — the invariant already held before the
   refresh and still holds. Calling `cascade` would `diff_panel.clear()` and
   force a reload, defeating ADR-0001's cache.
3. Returns `()` (no async work; the caller's `cx.notify()` suffices).

### `view_mode` is read by the cascade, written by entries

`view_mode` is written by three different policies: `set_selection` forces
`CommitHistory` (explicit navigation); the snapshot path never writes it
(refresh respects the user's current view); the status-view action handler
writes `Status` orthogonally. The cascade **reads** `view_mode` to gate
`enter_graph_staging` (only enter staging when in history view) but never
writes it. The invariant the cascade enforces is therefore "given the current
`view_mode`, the panels are consistent" — not "`view_mode` is always
CommitHistory."

## Scope boundary (explicit non-goals)

- The ~60 other `self.repo_session.<panel>.<method>` reach-ins across `ops/`
  are **not** selection coordination (`set_code_view`, `set_blame`,
  `select_file`, `clear`, etc.). They stay direct. The cascade is the single
  home for the *selection invariant*, not a full panel façade. Wrapping every
  panel mutation would turn `RepoSession` into a shallow pass-through
  namespace — the opposite of deepening.
- The two-writers issue (`graph_panel` writes its own selection on keyboard
  navigation; the app writes it on clicks) is **acknowledged, not fixed**.
  Both paths now route the *cascade* part through one private method, which is
  where the invariant lives. Unifying `select_prev`/`select_next` to
  compute-without-writing would give a single graph-selection writer, but
  that is a larger change to `GraphPanel`'s API for modest gain over the
  cascade-as-invariant-home outcome.

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
   (`cascade_current`) and the snapshot path (`apply_repo_state_to_panels`),
   where `view_mode` is not forced.

## Consequences

### Positive

- The four-step selection dance lives in exactly one tested private method
  (`cascade`). Adding a new entry path means writing a thin wrapper, not
  re-implementing the invariant.
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

- Two public entry methods (`set_selection`, `cascade_current`) rather than
  one. This reads honestly — the click and keyboard paths really do differ
  (who writes the graph) — but it is one more method to learn than a single
  entry point would be.
- The two-writers issue for `graph_panel.selection` is documented here, not
  resolved. A future change that makes `select_prev`/`select_next`
  compute-and-return would let everything route through `set_selection`,
  but that is deferred.
- `view_mode` remains a `pub(crate)` field on `RepoSession` rather than being
  fully encapsulated behind the cascade. This is deliberate (the status-view
  action handler is an orthogonal writer), but means the cascade's gating
  depends on callers having set `view_mode` correctly for their context.
