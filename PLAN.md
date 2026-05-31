# GitForge Development Plan

## Project Summary

| | |
|---|---|
| **Name** | GitForge |
| **Language** | Rust |
| **UI Framework** | GPUI (Zed's GPU-accelerated framework, Apache-2.0) |
| **Git Backend** | gix (pure Rust, gitoxide) |
| **License** | Apache-2.0 |
| **Platform** | Linux-first (Wayland + X11) |
| **License Status** | Clean-room rewrite — independent project, no GPLv3 obligation |
| **Visual Target** | Zed-like polish with robust theme engine |
| **Inspired by** | gitfourchette (features), Zed (UI) |

## Architecture & Crate Structure

```
gitforge/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── gitforge-app/           # Binary entry point, window lifecycle
│   ├── gitforge-ui/            # Reusable GPUI components, theme engine, icons
│   ├── gitforge-git/           # gix wrapper, porcelain API, status, diff, operations
│   ├── gitforge-graph/         # Commit graph algorithm (pure logic, no UI)
│   ├── gitforge-diff/          # Diff parsing, highlighting, patch generation
│   ├── gitforge-hosting/       # GitHub/GitLab/Codeberg API clients
│   ├── gitforge-ai/            # AI backend (local ollama + cloud APIs)
│   └── gitforge-syntax/        # Syntax highlighting (tree-sitter/syntect)
├── assets/
│   ├── icons/                  # SVG icon set (17 icons)
│   ├── themes/                 # Theme JSON files (default-dark, default-light)
│   └── lang/                   # i18n files (fluent)
├── PLAN.md                     # This file
└── tests/
```

## Key Technology Choices

| Component | Crate | Notes |
|-----------|-------|-------|
| UI framework | `gpui` 0.2.x | GPU-accelerated, tailwind-style styling, custom elements for graph |
| Git operations | `gix` | Pure Rust, async-friendly, no C dependencies |
| Async runtime | `tokio` | GPUI has its own executor but tokio for git/network IO |
| Syntax highlighting | `tree-sitter` + grammars | Zed uses tree-sitter, consistent with the visual target |
| HTTP client | `reqwest` | For hosting APIs + AI cloud backends |
| Serialization | `serde` + `serde_json` | Themes, settings, API responses |
| SVG rendering | `resvg` (bundled with GPUI) | Icons and graph decorations |
| i18n | `fluent` | Mozilla's localization system |
| AI - local | `ollama` API via reqwest | Local LLM inference |
| AI - cloud | OpenAI/Anthropic APIs | Via reqwest |
| Logging/tracing | `tracing` | Structured logging |
| Terminal spawning | `std::process::Command` | External terminal/editor launching |

## Theme Engine Design

Themes are JSON files loaded at runtime. Day-one feature.

Each theme defines 40+ color tokens covering:
- Background, surface, border colors
- Text (normal + muted)
- Accent colors (primary + secondary)
- Sidebar colors (background, selected, hover)
- Commit hash + ref label colors (branch, tag, remote, head)
- Diff colors (added/removed text + background)
- Graph lane colors (8 lanes, cycling)
- Scroll bar + selection colors
- Font families and sizes

Files: `assets/themes/default-dark.json`, `assets/themes/default-light.json`

---

## Development Phases

### Phase 1: Foundation (Weeks 1-3) — COMPLETED

**Goal**: Window opens, theme loads, basic layout renders.

- [x] Workspace + crate scaffolding
- [x] GPUI application lifecycle (`Application::new()`, `open_window()`)
- [x] Theme engine: JSON loading, `Theme` struct, GPUI color application
- [x] Default dark theme + default light theme
- [x] Main window layout: 3-column split (sidebar | graph | diff area)
- [x] Application menu bar
- [x] SVG icon system (17 icons loaded from assets)
- [x] Keyboard shortcut system (GPUI actions)
- [x] Welcome screen / "Open Repository" dialog
- [x] Settings persistence (`serde_json` to `~/.config/gitforge/`)

### Phase 2: Git Core — Reading (Weeks 4-7)

**Goal**: Open a repo and display its contents. Read-only.

- [x] `gitforge-git` crate: `Repository` wrapper around `gix`
  - [x] Open repo, read config
  - [x] List references (branches, tags, remotes)
  - [x] Walk commit history (with pagination for large repos)
  - [x] Read commit metadata (author, date, message, parents)
  - [x] Compute diff between any two trees
  - [x] Read file contents at a given commit
  - [x] Status (unstaged, staged, untracked) — full implementation via gix status API
  - [x] Blame a file
- [x] Async repo loading (background thread via tokio spawn_blocking)
- [x] Error handling types for all git operations
- [x] Connect git operations to UI (open repo → populate views)

### Phase 3: Commit Graph (Weeks 8-12) — COMPLETED

**Goal**: The commit graph renders beautifully. Hero feature.

- [x] `gitforge-graph` crate — pure logic, no UI:
  - [x] `Graph` struct: commits, lanes, arcs
  - [x] Lane assignment algorithm
  - [x] `CommitEntry` data structure
  - [x] Incremental update support (splice new commits into existing graph)
  - [x] Branch filtering / commit hiding
  - [x] Batch row system for efficient rendering
- [x] GPUI custom `Element` for graph rendering:
  - [x] Paint arcs (bezier curves) between lanes
  - [x] Paint commit nodes (filled circles)
  - [x] Paint lane lines
  - [x] Continuing lane lines across rows (semi-transparent pass-through)
  - [x] Merge node hollow ring indicator
  - [x] GPU-accelerated via GPUI's `PaintQuad` / custom shaders if needed
  - [x] Smooth scrolling through 100k+ commits
- [x] Commit row rendering: hash, author, date, message preview, ref labels (colored pills)
- [x] Virtual scrolling (only render visible rows) — GPUI has `UniformList` for this
- [x] "Uncommitted Changes" virtual row at top
- [x] Click-to-navigate, context menus on commits
- [x] Keyboard navigation (Up/Down arrows to select commits)
- [x] Native file dialog (rfd) for opening repositories
- [x] Commit info display in diff panel on selection

### Phase 4: Diff Viewer (Weeks 13-16) — COMPLETED

**Goal**: Side-by-side and unified diff views with syntax highlighting.

- [x] `gitforge-diff` crate:
  - [x] Parse unified diff output
  - [x] Line-level metadata (added, removed, context)
  - [x] Patch extraction from selections (partial staging)
- [x] `gitforge-syntax` crate:
  - [x] tree-sitter integration for common languages
  - [x] Theme-aware token coloring
  - [x] Incremental highlighting (cache per path+line, reused across renders)
- [x] Diff view GPUI element:
  - [x] Unified diff view with line numbers
  - [x] Added/removed line coloring (theme-driven)
  - [x] Binary file diff placeholder
  - [x] Rubber-band selection for line-level staging/discard (click + shift-click)
  - [x] Image diff (side-by-side or swipe)
  - [x] LFS pointer display
- [x] Code viewer (non-diff): read-only file contents at a commit
- [x] File list: staged files, unstaged files, committed files (per commit)

### Phase 5: Sidebar (Weeks 17-19) — NEAR COMPLETE

**Goal**: Interactive ref tree in the left panel.

- [x] Tree model: branches (local/remote), tags, stashes, submodules
- [x] GPUI tree view element with expand/collapse
- [x] Drag-and-drop (branch checkout by drag)
- [x] Context menus: delete branch, rename, create from here, etc.
- [x] Ref label pills in the graph (colored per type, themed)
- [x] Sidebar search/filter
- [x] Collapsible sections (branches, remotes, tags)
- [x] Click ref to navigate to commit in graph

### Phase 6: Write Operations — Stage & Commit (Weeks 20-23) — COMPLETED

**Goal**: Full staging, committing, and amending.

- [x] Stage/unstage files (via git CLI commands in gitforge-git write_impl)
- [x] Stage/unstage individual lines (partial staging via patch application)
- [x] Discard changes (file-level and line-level)
- [x] Commit dialog: message editor, author override, GPG signing
- [x] Amend previous commit
- [x] Undo last commit (soft reset)
- [x] File status indicators in file lists (M, A, D, R, etc.)
- [x] Conflict visualization and resolution marking
- [x] `.gitignore` management

#### Phase 6a: Completed Infrastructure
- **write_impl.rs** — stage_paths, stage_all, unstage_paths, unstage_all, commit, commit_amend, discard_worktree_changes, remove_untracked, apply_patch (with reverse support), soft_reset_head, diff_index_to_worktree, diff_head_to_index
- **StatusPanel** — shows staged/unstaged/untracked/conflicted files with M/A/D/R/? badges, diff preview for selected file, commit message editor with Commit/Amend buttons, line-level selection for partial staging, Stage/Unstage Lines buttons
- **Toolbar tabs** — History/Changes tabs to switch between commit history view and status view, Undo Commit button
- **File-level actions** — Stage (+), Unstage (−), Discard (×), Remove (×) buttons per file entry; Stage All / Unstage All section buttons
- **Refresh flow** — after commit/amend/stage/unstage/discard/reset, repository state is fully reloaded (commits, refs, status, graph)

### Phase 7: Write Operations — Branch & Merge (Weeks 24-27)

**Goal**: Branch management, merging, rebasing.

- [x] Create/delete/rename branches
- [x] Checkout branches and commits (including detached HEAD)
- [x] Fast-forward branches
- [x] Merge with conflict detection
- [x] Rebase (interactive if possible, or shell out to git CLI for this)
- [x] Reset (soft, mixed, hard)
- [x] Cherry-pick commits
- [x] Revert commits
- [x] Tag create/delete (annotated + lightweight)
- [x] Stash create/apply/drop/pop/list

### Phase 8: Remote Operations (Weeks 28-31) — NEAR COMPLETE

**Goal**: Fetch, push, pull, clone.

- [x] Remote management (add, edit, delete)
- [x] Fetch with progress reporting
- [x] Pull (fetch + merge/rebase)
- [x] Push with force option
- [x] Clone dialog (URL + path + options)
- [x] SSH key management / ssh-agent integration
- [x] Credential handling (libsecret on Linux)
- [x] Clone from hosting service (browse repos via integration)
- [x] Submodule support (status, init, update)

### Phase 9: Hosting Integrations (Weeks 32-35) — COMPLETED

**Goal**: GitHub, GitLab, and Codeberg account integration.

- [x] OAuth flow for each provider:
  - [x] GitHub (PAT authentication + API)
  - [x] GitLab (PAT authentication + API)
  - [x] Codeberg/Forgejo (PAT authentication + API)
- [x] Token storage in system keyring (`keyring-rs`)
- [x] Account management UI (add/remove accounts)
- [x] Browse remote repos (list, search, clone)
- [x] Create fork from UI
- [x] View/open remote in browser
- [x] Generate remote links (line-level URLs for GitHub/GitLab)
- [x] Provider abstraction trait — easy to add more later

### Phase 10: AI Features (Weeks 36-39)

**Goal**: AI-powered commit messages and more.

- [x] `gitforge-ai` crate with provider abstraction:
  - [x] `AiProvider` trait: `generate_commit_message(diff) -> Result<String>`
- [x] Local backend: ollama API (`POST /api/generate`)
- [x] Cloud backends:
  - [x] OpenAI API (GPT-4o-mini, GPT-4o)
  - [x] Anthropic API (Claude 3.5 Sonnet)
- [x] Commit message generation:
  - [x] Feed staged diff to AI
  - [x] Conventional commit format option
  - [x] Multi-message option (pick best one)
  - [x] Tone/style settings (concise, detailed, conventional)
- [x] Settings UI for AI: provider selection, API key, model choice
- [x] Diff summarization (hover tooltip on file with AI summary)
- [ ] Future: PR description generation, code review suggestions

### Phase 11: Worktree Support (Weeks 40-42)

**Goal**: Full git worktree management.

- [x] List existing worktrees
- [x] Create new worktree (from branch, tag, or commit)
- [x] Remove worktree
- [x] Prune stale worktrees
- [x] Visual indicator showing which worktree you're in
- [x] Switch between worktrees (open in new tab or window)

### Phase 12: Polish & Linux Distribution (Weeks 43-48) — IN PROGRESS

**Goal**: Production-ready Linux release.

- [x] Performance profiling and optimization
  - [ ] Large repo handling (100k+ commits, Linux kernel scale)
  - [ ] Memory usage optimization
  - [x] Smooth 60fps scrolling in graph view (virtual scrolling + canvas culling already in place)
  - [x] Criterion benchmarks for graph build
  - [x] Fix O(n²) branch filter → O(n) with HashMap index
  - [x] Parser reuse in syntax highlighter (thread-local pool)
  - [x] Instrumentation for Graph::build timing
- [x] Additional themes (community theme format documented)
  - [x] Runtime theme switching (Ctrl+Shift+T)
  - [x] Theme discovery from ~/.config/gitforge/themes/
  - [x] Theme format docs (docs/THEMES.md)
- [x] Keyboard-first workflow (all operations accessible via keyboard)
  - [x] Command palette (Ctrl+Shift+P, fuzzy search)
  - [x] External tool integration:
  - [x] Configurable diff/merge tools (Meld, Beyond Compare, etc.)
  - [x] Open in editor (VS Code, Vim, etc.)
  - [x] Open in terminal
- [x] User-defined custom commands with placeholders ({file}, {line}, {commit}, {repo})
- [ ] Accessibility (GPUI has accesskit integration) — deferred
- [x] i18n infrastructure (fluent, English .ftl file)
  - [ ] Wire up Localization to all UI strings
- [x] Packaging:
  - [x] AppImage build script
  - [x] Flatpak manifest
  - [x] Arch AUR PKGBUILD
  - [x] Debian package (cargo-deb)
- [x] Documentation (README, CONTRIBUTING, keyboard shortcuts, theme format)
- [x] CI/CD (GitHub Actions: build, test, clippy, fmt, release with artifacts)

---

## Estimated Timeline

| Phase | Duration | Cumulative |
|-------|----------|------------|
| 1. Foundation | 3 weeks | 3 weeks |
| 2. Git Core (read) | 4 weeks | 7 weeks |
| 3. Commit Graph | 5 weeks | 12 weeks |
| 4. Diff Viewer | 4 weeks | 16 weeks |
| 5. Sidebar | 3 weeks | 19 weeks |
| 6. Stage & Commit | 4 weeks | 23 weeks |
| 7. Branch & Merge | 4 weeks | 27 weeks |
| 8. Remote Operations | 4 weeks | 31 weeks |
| 9. Hosting Integrations | 4 weeks | 35 weeks |
| 10. AI Features | 4 weeks | 39 weeks |
| 11. Worktree Support | 3 weeks | 42 weeks |
| 12. Polish & Release | 6 weeks | **~48 weeks** |

**Realistic estimate: 9-12 months** for one person working full-time.

---

## Critical Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| **GPUI is pre-1.0** | Breaking changes, sparse docs | Pin to specific version, read Zed's source as reference |
| **gix API gaps** | Some git operations may not be supported | Fall back to `std::process::Command` (git CLI) for anything gix can't do |
| **GPUI Linux rendering bugs** | Visual glitches on some Wayland compositors | Test on multiple compositors (Sway, Hyprland, GNOME Mutter), contribute fixes upstream |
| **Scope creep** | Delays | Strict phase discipline, ship each phase before moving on |
| **Graph performance** | Laggy scrolling with huge repos | GPU-accelerated rendering from day one, virtual scrolling, benchmark against Linux kernel repo |

---

## What NOT to Copy from gitfourchette

To maintain clean-room independence:

| Don't Copy | Instead |
|------------|---------|
| `porcelain.py` API design | Design a Rust-idiomatic `Repository` API with `Result<T, E>`, ownership, and async |
| `graphweaver.py` lane algorithm | Design your own lane assignment algorithm (document it independently) |
| `.ui` dialog structure | Design dialogs fresh using GPUI's declarative style |
| File/module organization | Use Zed-inspired workspace crate structure instead |
| Settings JSON schema | Design your own settings format |

---

## Icon Prompt (for image generation)

> A modern app icon for a developer tool called "GitForge" — a stylized forge/anvil theme merged with git branch iconography. The design features a central anvil shape formed from interlocking git branch lines, with sparks of code emanating upward. Rendered in teal (#00ccaa) and amber (#f0a030) on a dark navy background (#0d0d1a). Small glowing data nodes along each branch represent commits. The overall feel is powerful, precise, and craftsman-like — forging your repository's history. Rounded square format, flat design, no text, professional.
