# GitForge 

A Linux-first Git GUI client built with [GPUI](https://github.com/zed-industries/zed) (Zed's GPU-accelerated UI framework) and [gix](https://github.com/GitoxideLabs/gitoxide) (pure Rust Git backend).

## Quick start

Install the latest release into `~/.local`:

```bash
curl -f https://raw.githubusercontent.com/b0red04/gitforge/main/scripts/install.sh | sh
```

Then run GitForge:

```bash
~/.local/bin/gitforge
```

If `gitforge` is not on your PATH, add `~/.local/bin` to your shell profile (the install script prints the exact command for your shell).

The install script downloads `GitForge-{version}-{arch}.tar.gz` from [GitHub Releases](https://github.com/b0red04/GitForge/releases), extracts it to `~/.local/gitforge.app/`, symlinks `~/.local/bin/gitforge`, and installs a `.desktop` entry so GitForge appears in your app launcher.

Supported platforms: **Linux x86_64 and aarch64**.

## Auto-updates

Release builds installed via the install script check GitHub hourly for newer releases and apply them automatically. Updates require `rsync` on your system (`sudo pacman -S rsync` on Arch).

- Updates are enabled by default. Toggle them in **Settings → General → Auto-update**.
- You can also trigger a manual check from the update indicator in the title bar.
- The updater compares semver versions, downloads the matching tarball, verifies its SHA-256 checksum, and replaces `~/.local/gitforge.app/` in place. Restart GitForge when prompted.

Auto-updates are disabled in debug builds (`cargo run`). Use a release install to test the updater. See [docs/TESTING-UPDATES.md](docs/TESTING-UPDATES.md) for local and automated test workflows.

## Features

- **Interactive commit graph** — GPU-accelerated DAG visualization with bezier curves, virtual scrolling, and smooth performance on large repos
- **Full diff viewer** — Unified diff view with syntax highlighting (tree-sitter), rubber-band line selection for partial staging
- **Stage & commit** — File-level and line-level staging, commit, amend, undo commit, discard changes
- **Branch management** — Create, delete, rename, checkout branches; merge, rebase, cherry-pick, revert
- **Remote operations** — Fetch, pull, push, clone with SSH key management and credential handling
- **Hosting integration** — GitHub, GitLab, and Codeberg account management, clone from hosting, fork repos, open in browser
- **AI-powered commit messages** — Local Ollama or cloud backends (OpenAI, Anthropic) generate commit messages from your staged diff
- **Sidebar** — Branches, remotes, tags, worktrees with drag-and-drop checkout
- **Theme engine** — JSON-defined color themes with 40+ color tokens, runtime theme switching
- **Command palette** — `Ctrl+Shift+P` to access all actions with fuzzy search
- **External tools** — Configurable editor, terminal, diff/merge tool integration
- **Worktree support** — Create, remove, and switch between git worktrees

## Keyboard shortcuts

| Shortcut       | Action           |
| -------------- | ---------------- |
| `Ctrl+O`       | Open repository  |
| `Ctrl+N`       | Create branch    |
| `Ctrl+Shift+P` | Command palette  |
| `Ctrl+Shift+T` | Cycle theme      |
| `Ctrl+Shift+F` | Fetch all        |
| `Ctrl+Shift+U` | Pull             |
| `Ctrl+Shift+H` | Push             |
| `Ctrl+Shift+S` | Stash changes    |
| `Ctrl+Shift+O` | Pop stash        |
| `Ctrl+,`       | Open settings    |
| `↑` / `↓`      | Navigate commits |
| `Escape`       | Close dialog     |

See [docs/KEYBOARD-SHORTCUTS.md](docs/KEYBOARD-SHORTCUTS.md) for the full list.

## Configuration

Settings are stored at `~/.config/gitforge/settings.json`.

### Custom themes

Place theme JSON files in `~/.config/gitforge/themes/`. Each theme defines 40+ color tokens. See [docs/THEMES.md](docs/THEMES.md).

### External tools

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

### Custom commands

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

## Building from source

### Prerequisites

**Rust 1.85+** (Rust Edition 2024)

**Ubuntu / Debian:**

```bash
sudo apt install libssl-dev libfreetype-dev libfontconfig-dev libwayland-dev \
  libx11-dev libxkbcommon-dev libxkbcommon-x11-dev libegl-dev libvulkan-dev \
  libclang-dev pkg-config
```

**Fedora:**

```bash
sudo dnf install openssl-devel freetype-devel fontconfig-devel wayland-devel \
  libX11-devel libxkbcommon-devel libxkbcommon-x11-devel mesa-libEGL-devel \
  vulkan-devel clang-devel pkgconfig
```

**Arch Linux:**

```bash
sudo pacman -S openssl freetype2 fontconfig wayland libx11 libxkbcommon \
  libxkbcommon-x11 mesa vulkan-devel clang pkg-config
```

### Build & run

```bash
cargo build -p gitforge-app --release
./target/release/gitforge
```

For development:

```bash
cargo run -p gitforge-app
```

### Install a local build

After building, create an update tarball and install it without GitHub:

```bash
./scripts/build-update-tarball.sh 0.3.0
GITFORGE_BUNDLE_PATH="$(pwd)/GitForge-0.3.0-$(uname -m).tar.gz" bash scripts/install.sh
```

## Releasing (maintainers)

GitForge is distributed via GitHub Releases. Each release must include:

| Asset                                     | Purpose                                 |
| ----------------------------------------- | --------------------------------------- |
| `GitForge-{version}-x86_64.tar.gz`        | Install script and auto-updater payload |
| `GitForge-{version}-x86_64.tar.gz.sha256` | Checksum verification for the updater   |

The version in `Cargo.toml`, the git tag (`v0.3.0`), and the tarball filename must all match.

### Publish a release

Releases are cut with [cargo-release](https://github.com/crate-ci/cargo-release), which bumps the shared workspace version, updates `Cargo.lock`, refreshes the version examples in this README, commits, tags, and pushes — all in one step. The pushed `v*` tag triggers the Release workflow.

```bash
cargo install cargo-release            # one-time
cargo release patch                    # preview the bump / commit / tag / push (dry-run is the default)
cargo release patch -x                 # execute (level: patch | minor | major)
```

For a pre-release, pass a full version with a pre-release tag — the Release workflow will mark it as a prerelease on GitHub:

```bash
cargo release 0.2.0-rc.1 -x
```

Watch progress under **Actions → Release**. You can still trigger the workflow manually from the Actions tab (`workflow_dispatch`), but `cargo release ... -x` is the normal path.

The tag pins the exact commit that gets built. If CI fails, fix the issue on `main`, then either move the tag or cut a new patch version.

### Build release artifacts locally

```bash
cargo build -p gitforge-app --release
./scripts/build-update-tarball.sh 0.3.0
```

Legacy packaging (AppImage, `.deb`, Flatpak, AUR) lives in `packaging/` but is not the supported install path.

## Architecture

```
gitforge-app        Binary entry point, window lifecycle, view modules
gitforge-ui         Reusable GPUI components, theme engine, icons
gitforge-git        gix wrapper, porcelain API, status, diff, worktree operations
gitforge-graph      Commit graph layout algorithm (pure logic, no UI)
gitforge-diff       Diff parsing, highlighting, patch generation
gitforge-hosting    GitHub/GitLab/Codeberg API clients
gitforge-ai         AI backend — local Ollama + cloud APIs
gitforge-syntax     Syntax highlighting (tree-sitter)
gitforge-update     GitHub release fetching, checksum verification, auto-update install
```

## License

Apache-2.0
