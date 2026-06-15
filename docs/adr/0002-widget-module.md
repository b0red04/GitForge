# ADR-0002: Widget module in gitforge-ui

- **Status:** Accepted
- **Date:** 2026-06-15
- **Supersedes:** —
- **Relates to:** Architecture review Candidate 02 ("Build the missing widget
  module behind the panels")

## Context

The largest view files in `gitforge-app` (`sidebar.rs`, `graph_panel.rs`,
`status_panel.rs`, `titlebar.rs`, `diff_viewer.rs`) were 65–96% GPUI markup,
and most of that markup was structural copies of a handful of idioms: the pill
button, the section header, the list row, the panel shell, and the per-frame
`rgba_to_hsla` color-unpack blocks. A walk of the crate graph (deletion test)
found that each panel re-derived these primitives per call site — its interface
to GPUI was wide.

The strongest precedent already existed: `gitforge-ui/src/dialog.rs` defined
`DialogColors::from_app(&AppColors)` (a `Copy` color bundle doing every
`rgba_to_hsla` conversion once) and `dialog_button<E>` (generic over entity,
owning the `entity.upgrade()` on-click boilerplate). That pattern was correct
but applied to one domain (dialogs).

A survey counted the recurring idioms:

| Idiom | Instances | Notes |
|---|---|---|
| `rgba_to_hsla` unpack blocks | 24 blocks / 233 call sites | `rgba_to_hsla` is a no-op (`c.into()`) type bridge |
| pill / action buttons | ~30 true pills + ~20 helper-routed | 7+ distinct shapes |
| `section_header` | ~18 | collapsible / non-collapsible / file-header |
| `entity.upgrade()` on-click dance | 93 copies | already factored once as `dialog_button<E>` |
| `panel_shell` | 4 | 3 factored helpers + 1 inline |
| `uniform_list` scaffolding | 4 | delegate + track_scroll |
| centered empty-state | ~10 | 2 already factored |

## Decision

Add a `widgets/` module to `gitforge-ui` that owns the idioms behind a small
interface, following the `dialog.rs` precedent:

- `WidgetColors` — a `Clone + Copy` color bundle with `from_app(&AppColors)`,
  the superset of tokens the widgets need. This is the `DialogColors` pattern
  generalized; callers build it once per render and pass it by value.
- A **family of small button functions** (chosen over a single enum-driven
  `pill_button` or a builder struct): `action_button`, `primary_button`,
  `icon_button`, `ghost_button`, plus the specialised `window_control_button`
  (moved here to kill a copy-paste dupe between `titlebar.rs` and
  `settings_window.rs`).
  - `ButtonKind` (`Accent` / `Muted` / `Warning` / `Danger`) resolves text +
    border + hover color. Outline `Accent`/`Muted` use the neutral `border`
    token; semantic `Warning`/`Danger` use the colored border.
  - `entity_on_click` / `entity_on_click_stop_propagation` collapse the 5-line
    `entity.upgrade()` dance into a composable closure wrapper, so the button
    functions can take the flexible raw `on_click` signature (matching the
    existing app-local helpers) while still killing the boilerplate.
- `section_header` (non-collapsible) and `collapsible_header` (with arrow + bg +
  click), split to stay under clippy's argument limit and keep each focused.
- `panel_shell` + `ShellWidth`, `list_row` + `RowPadding`, `virtual_list`,
  `empty_state` / `empty_state_with_bg`.

All items are re-exported flat from `gitforge-ui`'s lib root, matching the
existing `dialog_*` / `text_input` re-export convention.

### Conventions adopted (from `dialog.rs` + CONTRIBUTING.md)

- `use gpui::*;` in the module; free functions returning `Div` / `Stateful<Div>`.
- All colors sourced from `AppColors` via `WidgetColors::from_app` — never
  hardcoded.
- No unnecessary comments; `cargo fmt` + `cargo clippy` clean.
- Test modules must **not** `use super::*` (it glob-imports `gpui::*`, which
  shadows `#[test]` with `gpui::test` and overflows the macro recursion — see
  the test-import fix applied to the widget tests and two pre-existing tests).

## Consequences

### Positive

- The view files shrink by hundreds of lines of duplicated markup.
- The `titlebar`/`settings_window` `window_control_button` copy-paste dupe is
  deleted (one definition in the widget module).
- A theme change touches one widget, not ~150 call sites.
- The per-frame `rgba_to_hsla` churn is concentrated in `WidgetColors::from_app`
  (called once per render, not per primitive).
- Panel logic separates from panel markup — a button is one call, not an
  18-line GPUI chain.

### Negative / deferred

- `list_row` and `virtual_list` are implemented and tested but **not** broadly
  adopted: the sidebar row variants have different background semantics
  (transparent action rows vs `sidebar_background` ref items) that the current
  `list_row` scaffold doesn't capture cleanly, and `graph_panel`'s
  `uniform_list` uses `.with_decoration(...)` which `virtual_list` doesn't
  expose. Forcing adoption there had poor value/risk; the widgets remain
  available for future use.
- A few buttons with unique dynamic/colored styling are left open-coded:
  `commit_editor`'s Generate (dynamic accent/muted color) and AI-alternative
  toggle pills, the sidebar commit button (a compact-wide size not in
  `ButtonSize`), the sidebar remote `F`/`×` (neutral border + warning text +
  `sidebar_hover`), and the stage checkbox (a label-less toggle square).
- `toolbar_button` and `more_item` (already-factored helpers in `toolbar.rs`)
  are left in place: `more_item` is a borderless menu-item style that doesn't
  match `action_button` (which always has a border), and consolidating them
  would change appearance or require new variants for modest gain.

## Verification

- `cargo build -p gitforge-app` — clean.
- `cargo clippy -p gitforge-ui` — no warnings in the widget module.
- `cargo test --workspace` — green, including 7 new widget tests
  (`WidgetColors::from_app` field mapping, `ButtonKind` color resolution,
  `IconSize`, `Copy` bound).
