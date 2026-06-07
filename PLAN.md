# Architecture Deepening Plan

Candidate refactors that turn shallow modules into deep ones. Each is evaluated by the deletion test: if deleting the module concentrates complexity elsewhere, it was earning its keep.

---

## 1. GitForgeApp God Object → Extract RepoSession

**Files:** `gitforge-app/src/views/app.rs` (467 lines), `ops/git_ops.rs` (889), `ops/tab_ops.rs` (356), `ops/dialog_ops.rs` (368), `ops/hosting_ops.rs` (325), `ops/ai_ops.rs` (139)

**Problem:** `GitForgeApp` is a 32-field struct with 76+ `pub(crate)` methods spread across 12 `impl` files (3,885 lines total). It directly mutates `status_panel.commit_message_mut()`, constructs `CommitDiffState` on behalf of `DiffPanel`, builds the `Graph` layout before handing it to `GraphPanel`, and manages `SidebarState` as a raw data struct. Every cross-panel communication path runs through it. The deletion test confirms it's earning its keep — complexity would redistribute across N panels if deleted — but at the cost of being the only seam in the application.

The core friction: understanding any single operation (e.g., "what happens when you commit?") requires reading `status_panel.show_commit()` (status_panel.rs:283), `ai_ops::generate_commit_message` (ai_ops.rs:6), `git_ops::perform_commit` (git_ops.rs:306), and `render_commit_editor` (status_panel.rs:796-1061). The logic is scattered by file-splitting convention, not concentrated by domain concept.

**Solution:** Extract a **RepoSession** module that owns tab data (`open_repo_tabs`, `active_repo_tab_id`, `next_repo_tab_id`), the `Arc<Mutex<Option<Repository>>>` handles, `RepoState`, and panel coordination (`apply_repo_state_to_panels`, `on_graph_selection_changed`, `refresh_repository`). `GitForgeApp` would hold a `RepoSession` and delegate repo operations to it. UI-only concerns (dialogs, settings, titlebar state) stay in `GitForgeApp`.

**Benefits:**
- **Locality** — all repo-level state mutations concentrate in one module. The selection-coordination logic (graph → diff → status) currently split across `git_ops.rs:66-100` would live inside `RepoSession`.
- **Leverage** — tests could exercise the full repo lifecycle (open → select commit → view diff → stage → commit → refresh) through `RepoSession`'s interface without GPUI.

---

## 2. Commit Editor — Maximally Shallow Interface

**Files:** `gitforge-app/src/views/status_panel.rs:796-1061` (render_commit_editor), `ops/ai_ops.rs:6-135`, `ops/git_ops.rs:283-350`

**Problem:** The commit message is a `String` inside `StatusPanel` exposed via `commit_message_mut() -> &mut String`. This is as shallow as an interface can be — the interface _is_ the implementation. The AI operations (`ai_ops.rs:107-111`) directly clear and overwrite this string. The commit editor rendering (266 lines) is the largest single function in `StatusPanel`, implementing a custom text input with cursor, placeholder, and AI alternative cycling — yet it has zero encapsulation. The deletion test confirms: if you delete the `commit_message` field and its accessor, the complexity reappears identically in the callers that currently use `commit_message_mut()`.

**Solution:** Extract a **CommitEditor** module that owns the commit message buffer, cursor state, AI alternatives, and rendering. Its interface would be `set_message(msg)`, `accept_ai_suggestion(idx)`, `take_message()`, and `render(colors, entity)`. Both `StatusPanel` and `GitForgeApp.ai_ops` would interact through this deeper interface instead of mutating a raw `String`.

**Benefits:**
- **Locality** — the AI → message → render pipeline concentrates in one place. Currently a bug in AI suggestion cycling requires reading `ai_ops.rs:107-111` + `status_panel.rs:1044-1061`.
- **Leverage** — the editor's text-handling logic (backspace, character append, cursor) is a single implementation used by both the main commit editor and the graph-staging commit editor.

---

## 3. Text Input Duplication (4 Sites)

**Files:** `sidebar.rs:393-489` (render_search_bar), `status_panel.rs:796-1061` (render_commit_editor), `command_palette.rs:118-284`, `settings_window.rs:1312-1390` (text_field_control)

**Problem:** Four independent implementations of "GPUI text input": FocusHandle management, display text with cursor character `\u{2502}`, key-down handling for backspace/escape/character input, placeholder text. The deletion test is unambiguous — deleting any one copy doesn't reduce complexity because the other three still exist. Zero locality: a text-input bug must be fixed in 4 files.

**Solution:** A **TextInput** module in `gitforge-ui` that owns focus, cursor position, placeholder text, and rendering behind an interface like `new(placeholder)`, `set_text(&mut str)`, `text() -> &str`, `render(colors) -> Element`. Each current call site becomes a `TextInput` instance.

**Benefits:**
- **Locality** — text handling bugs fixed once.
- **Leverage** — the sidebar search bar, commit editor, command palette, and settings fields all get improvements (e.g., selection support, clipboard paste) from a single change. Tests can exercise keyboard → text → render through one interface.

---

## 4. Dialog System — 140-Line Match Dispatcher

**Files:** `ops/dialog_ops.rs:50-193` (confirm_dialog), `ops/dialog_render.rs` (909 lines), `app.rs:118-125` (AppDialog enum)

**Problem:** Adding a new dialog type requires changes in 3+ files: add a variant to `AppDialog` (app.rs), add `open_*_dialog` + match arm in `confirm_dialog` (dialog_ops.rs), add a `render_*_overlay` (dialog_render.rs). The `confirm_dialog` method is a 140-line match that knows about every dialog type and every downstream operation — a manual vtable. The dialog input key handler (backspace/escape/character) is copy-pasted 3 times (dialog_render.rs:167-204, 641-677, 705-743). The Cancel/Confirm button pair is repeated 4 times.

**Solution:** Each dialog becomes a self-contained module implementing a `Dialog` trait with `open()`, `confirm(input, cx)`, `cancel()`, and `render(input, colors, entity)`. The `AppDialog` enum becomes a `Box<dyn Dialog>` (or an enum of trait objects if needed). The shared rendering primitives (input handler, button pair, overlay wrapper) move into `gitforge-ui`.

**Benefits:**
- **Locality** — all knowledge of "what the Create Branch dialog does" lives in one file. Adding a new dialog is one file, one struct.
- **Leverage** — the shared rendering primitives benefit all dialogs simultaneously. Tests can exercise a dialog's confirm logic through its trait interface.

---

## 5. Diff Rendering Split Across Two Panels

**Files:** `diff_panel.rs:542-774` (render_diff_content), `status_panel.rs:676-794` (render_selected_diff), `diff_view.rs` (340 lines, shared utility)

**Problem:** `DiffPanel` and `StatusPanel` both own diff display state (selection, scroll handle, view mode toggle), both call `render_diff_lines()` from `diff_view.rs` with nearly identical `Rc<dyn Fn>` click handlers, both have "View File" and "Blame" buttons. A change to diff rendering (e.g., adding syntax highlighting cache invalidation) must be made in both panels. The shared `diff_view.rs` is already a partial consolidation, but it only covers the line list — not the file header, mode toggle, or action buttons.

**Solution:** Extract a **DiffViewer** module that owns `FileDiff`, `DiffLineSelection`, `DiffViewMode`, scroll handles, and the full rendering pipeline. Both `DiffPanel` and `StatusPanel` would embed a `DiffViewer` and delegate diff display to it. The interface would be `set_diff(FileDiff)`, `select_line()`, `selected_range()`, `render(colors, entity)`.

**Benefits:**
- **Locality** — diff display bugs fixed once (currently the binary/LFS detection logic at `diff_panel.rs:566-681` and `status_panel.rs:690-694` are parallel).
- **Leverage** — improvements to diff rendering (inline syntax highlighting, expand-context) apply everywhere diffs are shown.

---

## 6. gitforge-git Error Type — Single String Variant

**Files:** `gitforge-git/src/error.rs` (13 lines), used across all `*_impl.rs` files

**Problem:** `GitError` has 3 variants, but `OperationFailed(String)` absorbs 95%+ of all errors. All structured error information from gix is discarded via `.map_err(|e| GitError::OperationFailed(e.to_string()))` — this pattern appears 40+ times. The `Io` variant (via `#[from]`) is declared but never used. The deletion test is brutal: replacing `GitError` with `anyhow::Error` would change nothing about how callers handle errors, because there's nothing to match on. The current type adds zero leverage.

**Solution:** Introduce domain-specific variants: `MergeConflict { paths: Vec<String> }`, `AuthenticationFailed { remote: String }`, `NetworkError { source: String }`, `IndexLock { path: PathBuf }`, `EmptyCommit`, `BranchNotFound { name: String }`. The gix error mapping would preserve the structured cause. Callers can then match and show specific UI (merge conflict dialog, auth prompt, retry button).

**Benefits:**
- **Leverage** — the app can show "Merge conflict in 3 files" instead of "Operation failed: ...git merge...exit status 1".
- **Locality** — error classification concentrates in `gitforge-git` instead of being parsed from strings in the app layer.

---

## 7. Command Dispatch String-Typed → Typed

**Files:** `gitforge-app/src/views/commands.rs` (269 lines), `gitforge-app/src/views/app.rs:623-717`

**Problem:** The command system uses string literals for action names. `execute_app_command()` matches on `"open_repository"`, `"fetch_all"`, etc. These strings are defined in `commands.rs` as `CommandEntry { action: &str }`, bound in `main.rs` as GPUI actions, and dispatched in `app.rs` via a string match. There is no compile-time checking that all three locations agree. A typo is silently ignored.

**Solution:** Replace the string match with an enum. `CommandEntry` would carry a `CommandAction` enum variant instead of a `&str`. `execute_app_command()` would match on the enum. The GPUI action binding would derive the string from the enum, or the enum would carry the string as a const.

**Benefits:**
- **Locality** — adding a new command requires touching exactly two places (the enum definition and the handler), and the compiler enforces completeness.
- **Leverage** — the command palette, keyboard shortcuts, and menu entries all share the same typed surface. Tests can enumerate all commands and verify they have handlers.

---

## 8. Remove Dead Code and Unused Enum Variants

**Files:** Multiple across crates

**Problem:** Several types have dead variants and unused modules that add cognitive overhead without earning their keep:

- `gitforge-graph/src/lane.rs` — `LaneAssigner` (87 lines) is never called by `Graph::build`. Marked `#[allow(dead_code)]`.
- `gitforge-git` — `GitError::InvalidReference` and `GitError::MergeConflict` are declared but never constructed.
- `gitforge-git` — `RefKind::Note` is defined but never produced by `references()`.
- `gitforge-git` — `FileStatus::Ignored` is defined but never produced.
- `gitforge-syntax` — `SyntaxTheme` and `TokenColor` are defined but never consumed by the highlighter.
- `gitforge-syntax` — 6 of 15 `HighlightScope` variants have no tree-sitter node kind mappings.
- `gitforge-ui` — 5 component module stubs are empty files.
- `gitforge-app` — `i18n.rs` (87 lines) is unused; all UI strings are hardcoded English.
- `gitforge-diff` — `anyhow` and `tracing` are declared dependencies but never used.

**Solution:** Delete unused code. If a variant is planned for future use, add a comment and a tracking issue — but right now it's noise that slows understanding.

**Benefits:**
- **Locality** — less code to read means faster understanding. The deletion test confirms these would not be missed: removing them concentrates no complexity elsewhere.

---

## 9. Fix gitforge-ai Byte-Level Diff Truncation Bug

**Files:** `gitforge-ai/src/prompt.rs` (truncate_diff function)

**Problem:** `truncate_diff` slices `&diff[..max_chars]` at the byte level. If `max_chars` falls in the middle of a multi-byte UTF-8 character (e.g., Chinese in a commit message), this will panic at runtime. This is a correctness bug that sits at the seam between the diff text and the AI provider.

**Solution:** Use `.floor_char_boundary(max_chars)` (Rust 1.82+) or manually find the nearest char boundary before slicing.

**Benefits:** Correctness. Testable by adding a test with multi-byte diff content.
