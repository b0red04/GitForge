1. Split GitForgeApp into domain-focused operation modules
   Files: gitforge-app/src/views/app.rs (4,531 lines)

Problem: GitForgeApp is a god struct. It holds 30+ fields and handles repository tab management, all git operations (via run_git_op/run_git_op_with_status), SSH key management, hosting account management, AI commit generation, external tool dispatch, browser integration, dialog rendering, settings persistence, and the top-level Render implementation. Understanding any single feature requires scanning the entire file. The deletion test confirms this is earning its keep (complexity would scatter across N callers if deleted), but the module is so wide that it has almost no locality — a bug in hosting account logic requires understanding the surrounding 4,500 lines.

Solution: Extract cohesive groups of methods behind seams on GitForgeApp. The struct remains the orchestrator, but each group of operations becomes a focused module with its own file and clear interface:

Repository Tab module — tab lifecycle: open, close, activate, load, restore (currently ~300 lines of methods on GitForgeApp)
Git Operations module — all ~30 run_git_op-powered methods (currently ~600 lines)
Hosting Operations module — account CRUD, clone, search, fork (currently ~300 lines)
SSH Operations module — key management, agent status, connection testing (currently ~200 lines)
Each module would be impl GitForgeApp blocks in separate files (already partly the Rust pattern), but with a clear discipline: the module's methods only touch a subset of fields, and the field accesses are documented in the module's interface.

Benefits: Locality improves dramatically — a hosting bug is found in hosting_ops.rs, not somewhere in a 4,500-line file. Leverage stays the same (callers still call methods on the app entity). Test surface stays the same (GPUI entity integration tests), but each module's method group becomes easier to reason about in isolation.

2. Deepen the Diff Panel / Status Panel shared diff rendering
   Files: gitforge-app/src/views/diff_panel.rs (1,326 lines), gitforge-app/src/views/status_panel.rs (1,572 lines)

Problem: Both panels independently implement line-level diff rendering with syntax highlighting, line numbers, +/- prefixes, hunk headers, shift-click selection, and scroll handles. The rendering logic is ~400 lines in each panel. Understanding "how diffs are rendered" requires reading both files and noticing the subtle differences. The deletion test says: if you deleted the duplicate, complexity wouldn't vanish — it would reappear in the other panel. This is a real duplication, not a pass-through.

Solution: Extract a shared Diff View module — a struct (or set of functions) that owns a Vec<DiffLine>, scroll handle, highlight state, selection state, and exposes a render() method. Both DiffPanel and StatusPanel would delegate diff rendering to this module rather than implementing it independently.

Benefits: Locality — diff rendering bugs fix once, fixed everywhere. Leverage — both panels get improvements to diff rendering (e.g., better syntax highlighting, selection, performance) from a single change. Tests can target the shared module's render output without needing the full panel context.

3. Make the command dispatch string-typed → typed
   Files: gitforge-app/src/views/commands.rs (269 lines), gitforge-app/src/views/app.rs (lines 623-717)

Problem: The command system uses string literals for action names. execute_app_command() matches on "open_repository", "fetch_all", etc. These strings are defined in commands.rs as CommandEntry { action: &str }, bound in main.rs as GPUI actions, and dispatched in app.rs via a string match. There is no compile-time checking that all three locations agree. A typo is silently ignored. The seam exists (commands → app), but the interface is informal.

Solution: Replace the string match with an enum. CommandEntry would carry a CommandAction enum variant instead of a &str. execute_app_command() would match on the enum. The GPUI action binding would derive the string from the enum, or the enum would carry the string as a const.

Benefits: Locality — adding a new command requires touching exactly two places (the enum definition and the handler), and the compiler enforces completeness. Leverage — the command palette, keyboard shortcuts, and menu entries all share the same typed surface. Tests can enumerate all commands and verify they have handlers.

4. Deepen gitforge-git's Repository — collapse the dual-backend strategy behind a single seam
   Files: gitforge-git/src/repository/\*.rs (all 6 impl files, ~1,174 lines)

Problem: The Repository module has a dual-backend strategy: gix for reads (log, refs, status, tree diff, objects) and git CLI for writes and complex queries (blame, unified diff, numstat, staging, branching, merging, pushing). This is a pragmatic split, but it's invisible to callers. The interface doesn't communicate which methods shell out (slow, fallible, environment-dependent) and which use gix (fast, deterministic). Understanding "what will this method actually do?" requires reading the implementation, not the interface. Additionally, write_impl.rs at 566 lines is a flat collection of ~40 methods that all follow the same pattern (build args, run git, check exit), with no further organization.

Solution: Two sub-candidates:

(a) Organize write_impl.rs into logical groups (staging, branching, merging, network, stash, etc.) — either as separate files or with clear section headers and shared helper functions. This is a locality improvement.

(b) Make the dual-backend strategy visible at the interface — consider marking CLI-backed methods with a documentation convention (e.g., /// CLI-backed: spawns git subprocess) so callers know the cost model. Alternatively, split the interface into RepositoryRead (gix-backed, fast) and RepositoryWrite (CLI-backed, slow) traits, so the seam is explicit.

Benefits: Locality — the 40 write methods in write_impl.rs are easier to navigate when grouped. Leverage — callers understand the cost model from the interface, not from reading implementation. Testability — the gix-backed read methods are deterministic and can be tested with fixture repos; the CLI-backed methods require a git binary and are inherently integration tests.

5. Extract RepoState snapshot building from gitforge-git's loader.rs
   Files: gitforge-git/src/loader.rs (86 lines), gitforge-app/src/views/app.rs (tab loading logic)

Problem: RepoState::from_repository_with_options is a monolithic snapshot builder that sequentially calls 6 different Repository methods, hardcodes the commit limit to 1000, and silently swallows worktree errors. Meanwhile, GitForgeApp does its own snapshot orchestration in start_loading_repo_tab(): it calls Repository::discover() + RepoState::from_repository_with_options() inside a spawn_blocking, then manually pushes results into panels. The seam between "loading a repo" and "displaying a repo" is spread across two crates. RepoLoader exists but is unused by the app.

Solution: Deepen RepoState::from_repository to own the full loading contract: discovery options, commit limits, which data to include/exclude, error recovery policy (e.g., should worktree errors be fatal?). Then either use RepoLoader in the app (it already exists but is unused) or inline the loading logic more cleanly. The key is that the "how to load a RepoState" knowledge sits in one place, not split between loader.rs and app.rs.

Benefits: Locality — loading logic is in one module. Leverage — changing the loading strategy (e.g., adding pagination, lazy loading, partial snapshots) requires editing one place. Testability — RepoState::from_repository_with_options can be tested with fixture repos without involving GPUI.

6. Remove dead code and unused enum variants
   Files: Multiple across crates

Problem: Several types have dead variants and unused modules that add cognitive overhead without earning their keep:

gitforge-graph/src/lane.rs — LaneAssigner (87 lines) is never called by Graph::build. Marked #[allow(dead_code)].
gitforge-git — GitError::InvalidReference and GitError::MergeConflict are declared but never constructed.
gitforge-git — RefKind::Note is defined but never produced by references().
gitforge-git — FileStatus::Ignored is defined but never produced.
gitforge-syntax — SyntaxTheme and TokenColor are defined but never consumed by the highlighter.
gitforge-syntax — 6 of 15 HighlightScope variants (Operator, Tag, Attribute, Constant, Module, Punctuation) have no tree-sitter node kind mappings.
gitforge-ui — 5 component module stubs are empty files.
gitforge-app — i18n.rs (87 lines) is unused; all UI strings are hardcoded English.
gitforge-diff — anyhow and tracing are declared dependencies but never used.
Solution: Delete unused code. If a variant is planned for future use, add a comment and a tracking issue — but right now it's noise that slows understanding.

Benefits: Locality — less code to read means faster understanding. The deletion test confirms these would not be missed: removing them concentrates no complexity elsewhere.

7. Fix the gitforge-ai byte-level diff truncation bug
   Files: gitforge-ai/src/prompt.rs (truncate_diff function)

Problem: truncate_diff slices &diff[..max_chars] at the byte level. If max_chars falls in the middle of a multi-byte UTF-8 character (e.g., Chinese in a commit message), this will panic at runtime. This is a correctness bug, not an architectural issue, but it sits at the seam between the diff text and the AI provider.

Solution: Use .floor_char_boundary(max_chars) (Rust 1.82+) or manually find the nearest char boundary before slicing.

Benefits: Correctness. Testable by adding a test with multi-byte diff content.
