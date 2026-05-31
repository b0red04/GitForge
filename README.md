# GitForge

A Linux-first Git GUI client built with [GPUI](https://github.com/zed-industries/zed) (Zed's GPU-accelerated UI framework) and [gix](https://github.com/GitoxideLabs/gitoxide) (pure Rust Git backend).

## Features

- **Interactive commit graph** — GPU-accelerated DAG visualization with bezier curves, virtual scrolling, and smooth 60fps performance on repos with 100k+ commits
- **Full diff viewer** — Unified diff view with syntax highlighting (tree-sitter), rubber-band line selection for partial staging
- **Stage & commit** — File-level and line-level staging, commit, amend, undo commit, discard changes
- **Branch management** — Create, delete, rename, checkout branches; merge, rebase, cherry-pick, revert
- **Remote operations** — Fetch, pull, push, clone with SSH key management and credential handling
- **Hosting integration** — GitHub, GitLab, and Codeberg account management, clone from hosting, fork repos, open in browser
- **AI-powered commit messages** — Local ollama or cloud backends (OpenAI, Anthropic) generate commit messages from your staged diff
- **Sidebar** — Branches, remotes, tags, worktrees with drag-and-drop checkout
- **Theme engine** — JSON-defined color themes with 40+ color tokens, runtime theme switching
- **Command palette** — Ctrl+Shift+P to access all actions with fuzzy search
- **External tools** — Configurable editor, terminal, diff/merge tool integration
- **Worktree support** — Create, remove, and switch between git worktrees

## Screenshots

*(Coming soon)*

## Building

### Prerequisites

**Rust 1.85+** (required for Rust Edition 2024)

**System dependencies** (Ubuntu/Debian):
```bash
sudo apt install libssl-dev libfreetype-dev libfontconfig-dev libwayland-dev \
  libx11-dev libegl-dev libvulkan-dev libclang-dev pkg-config
```

**System dependencies** (Fedora):
```bash
sudo dnf install openssl-devel freetype-devel fontconfig-devel wayland-devel \
  libX11-devel mesa-libEGL-devel vulkan-devel clang-devel pkgconfig
```

**System dependencies** (Arch Linux):
```bash
sudo pacman -S openssl freetype2 fontconfig wayland libx11 mesa vulkan-devel clang pkg-config
```

### Build & Run

```bash
cargo build -p gitforge-app --release
./target/release/gitforge
```

For development:
```bash
cargo run -p gitforge-app
```

### Install Desktop Entry

```bash
./scripts/install-desktop.sh
```

This installs the .desktop file and icons so GitForge appears in your app launcher.

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+O` | Open Repository |
| `Ctrl+N` | Create Branch |
| `Ctrl+Shift+P` | Command Palette |
| `Ctrl+Shift+T` | Toggle Theme |
| `Ctrl+Shift+F` | Fetch All |
| `Ctrl+Shift+U` | Pull |
| `Ctrl+Shift+H` | Push |
| `Ctrl+Shift+S` | Stash Changes |
| `Ctrl+Shift+O` | Pop Stash |
| `↑/↓` | Navigate commits |
| `Escape` | Close dialog |

## Configuration

Configuration is stored at `~/.config/gitforge/settings.json`.

### Custom Themes

Place theme JSON files in `~/.config/gitforge/themes/`. Each theme defines 40+ color tokens. See the [Theme Format](docs/THEMES.md) documentation for details.

### External Tools

Configure in `~/.config/gitforge/settings.json`:

```json
{
  "tools": {
    "editor_command": "code",
    "terminal_command": "alacritty",
    "diff_tool": "meld",
    "merge_tool": "meld"
  }
}
```

### Custom Commands

```json
{
  "custom_commands": [
    {
      "name": "Open in VS Code",
      "command": "code {repo}",
      "description": "Open repo in VS Code"
    }
  ]
}
```

Placeholders: `{file}`, `{line}`, `{commit}`, `{repo}`

## Packaging

### AppImage
```bash
./scripts/build-appimage.sh
```

### Debian Package
```bash
./scripts/build-deb.sh
```

### Flatpak
```bash
flatpak-builder build-dir packaging/flatpak/dev.gitforge.GitForge.yml
```

### Arch AUR
See `packaging/aur/PKGBUILD`.

## Architecture

```
gitforge-app        Binary entry point, window lifecycle, view modules
gitforge-ui         Reusable GPUI components, theme engine, icons
gitforge-git        gix wrapper, porcelain API, status, diff, worktree operations
gitforge-graph      Commit graph layout algorithm (pure logic, no UI)
gitforge-diff       Diff parsing, highlighting, patch generation
gitforge-hosting    GitHub/GitLab/Codeberg API clients
gitforge-ai         AI backend — local ollama + cloud APIs
gitforge-syntax     Syntax highlighting (tree-sitter)
```

## License

Apache-2.0
