# Commit Graph Readability and Resizing Plan

## Goal

Improve the history commit graph so reference labels are readable and no longer clipped in a narrow dedicated `REFS` column. Match the requested style by rendering branch/tag/remote pills inline at the start of the commit description, then add practical column resizing for the history table.

## Relevant Code

- `crates/gitforge-app/src/views/graph_panel.rs`
  - Owns commit list state, row rendering, graph canvas overlay, column headers, and ref pill rendering.
  - Current dedicated refs column is rendered by `render_refs_column`, `empty_refs_column`, `render_column_headers`, and row children in `GraphPanel::render`.
- `crates/gitforge-app/src/views/layout.rs`
  - Defines current fixed widths: `GRAPH_LANE_WIDTH`, `REF_COL_MIN`, `HASH_COL`, `TIME_COL`.
- `crates/gitforge-app/src/views/ops/git_ops.rs`
  - Existing callbacks mutate `GitForgeApp`/`GraphPanel` via `WeakEntity<GitForgeApp>`; new resize callbacks should follow this pattern.

## Implementation Steps

1. Rework history columns
   - Remove the dedicated `REFS` column from commit rows and headers.
   - Change headers from `REFS | GRAPH | SHA | MESSAGE | TIME` to `GRAPH | SHA | DESCRIPTION | TIME`.
   - Move the graph canvas overlay from `left(REF_COL_MIN)` to the history content origin.
   - Update the uncommitted row so it starts with the graph spacer, then the description text, then time spacer.

2. Move ref pills into the description cell
   - Rename/rework `render_refs_column` into a width-neutral helper such as `render_ref_pills(refs, colors)`.
   - In each commit row, render the description cell as a horizontal flex row:
     - ref pills first,
     - then commit summary with `overflow_hidden` and `text_ellipsis`.
   - Keep row virtualization unchanged; continue deriving `refs_for_commit` inside the visible-row closure.
   - Preserve existing ref ordering from `gitforge-git` and cap visible pills to avoid one commit consuming the whole row; optionally add a compact `+N` overflow pill if refs exceed the visible cap.

3. Improve ref pill readability
   - Stop using the normal theme text color blindly on colored ref backgrounds.
   - Add a small luminance/contrast helper in `graph_panel.rs` that chooses dark or light foreground text for each pill background.
   - Increase pill padding and corner radius slightly so labels are more legible in the inline description flow.
   - Use stable colors by ref kind:
     - branch: `cl.ref_branch`,
     - remote: `cl.ref_remote`,
     - tag: `cl.ref_tag`,
     - `HEAD`: prefer `cl.ref_head`,
     - fallback/stash: muted/surface-high treatment.

4. Add column width state to `GraphPanel`
   - Add width fields initialized from existing layout constants:
     - `graph_col_width`,
     - `hash_col_width`,
     - `time_col_width`.
   - Add resize-tracking state, for example:
     - active column being resized,
     - mouse start x,
     - start width.
   - Define clamp bounds near the existing graph constants, e.g. graph `80..320`, SHA `48..140`, time `70..160`.
   - Replace uses of fixed `GRAPH_COL_WIDTH`, `HASH_COL`, and `TIME_COL` in row/header/canvas layout with current panel widths.

5. Implement column resize interactions
   - Add thin resize handles in the header after resizable columns, using `CursorStyle::ResizeColumn`.
   - On mouse down, store the active resize state through `entity.update(...)`.
   - On mouse move while active, compute `delta = current_x - start_x`, clamp the target width, update the relevant `GraphPanel` width, and call `cx.notify()`.
   - On mouse up, clear active resize state.
   - Prefer the GPUI global mouse-event pattern used by the data table example if local handle events do not keep firing after the pointer leaves the handle.

6. Keep graph rendering aligned
   - Pass the current graph width to `graph_spacer`, the canvas width, and the absolute overlay width.
   - Leave lane positioning based on `LEFT_PADDING` and `LANE_WIDTH`; resizing the graph column should reveal or clip additional graph lane space without changing the graph layout algorithm.
   - Confirm scroll alignment remains based on `UniformListScrollHandle` and `ROW_HEIGHT`, not column width, so no graph algorithm changes are needed.

## Verification

Run the smallest relevant checks first:

```bash
cargo fmt --all
cargo check -p gitforge-app
```

If time permits or if the changes touch shared UI types more broadly:

```bash
cargo clippy -p gitforge-app --all-targets -- -D warnings
cargo test --workspace
```

Manual UI checks:

- Ref tags appear inline before commit messages and are not in a separate `REFS` column.
- Ref pill text remains readable on dark/light bundled themes.
- Long ref names do not clip awkwardly; visible pills either truncate cleanly or show an overflow count.
- Resizing graph/SHA/time headers updates both headers and rows.
- The graph canvas remains aligned with commit rows while scrolling.
- The uncommitted row still selects and renders correctly.

## Notes and Risks

- The user explicitly prefers the second screenshot style, so the dedicated `REFS` column should be removed rather than widened.
- Persisting column widths to `AppSettings` is optional. Start with in-session widths to keep the first implementation small; add settings persistence later if desired.
- GPUI drag behavior may require global mouse tracking rather than only header-handle `on_mouse_move`; use the existing `window_chrome` patterns and GPUI data table example as reference during implementation.
