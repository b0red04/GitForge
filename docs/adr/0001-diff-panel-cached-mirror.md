# ADR 0001: Diff panel cached mirror

- **Status:** Accepted
- **Date:** 2026-06-13

## Context

`GitForgeApp` is the single root GPUI view for the whole application. As a
result, scrolling the commit list dirties the root view and re-renders
everything every frame. The diff panel is comparatively expensive to rebuild
(diffs, file lists, blame), so re-rendering it on every scroll tick makes the
commit graph feel sluggish.

`DiffPanel` (owned by `RepoSession`) remains the single source of truth for diff
state. The question is how to keep scrolling cheap without splitting the diff
panel into a separate GPUI entity (which would conflict with the "one root view"
ownership model documented in `CONTEXT.md`).

## Decision

Mirror the diff panel through a cached GPUI view (`DiffViewMirror`) and embed it
with `.cached(...)`. GPUI recycles the mirror's painted output as long as the
rendered element tree is unchanged.

To make recycling effective, `GitForgeApp::render` does **not** feed live
`DiffPanel` state into the mirror every frame. Instead it rebuilds a render
snapshot (`DiffSnapshot`) only when a cheap fingerprint of the panel's visible
state — `DiffViewKey` (selected commit, diff/file selection, view mode, theme,
loading, line selection) — changes. Because scrolling the commit history does
not change the key, the snapshot — and therefore the mirror's painted output —
stays constant across scroll frames.

`build_key` and `build_snapshot` intentionally include only state that is
actually visible in the current view. For example, blame data is copied into the
key/snapshot only while the active view is `DiffViewMode::Blame`, so a stale
`blame` field never perturbs the cache key or triggers an unnecessary clone
when the user is in Diff or Code view.

## Consequences

- Scrolling the commit graph no longer rebuilds the diff panel; it recycles the
  mirror's last paint.
- `DiffPanel` stays a plain struct owned by the app, preserving the single-root
  ownership model.
- Anyone adding new visible state to `DiffPanel` must add it to `DiffViewKey`
  (and `DiffSnapshot`) so the mirror refreshes when it changes — and must gate
  it by view mode when it is only relevant to a specific view.
