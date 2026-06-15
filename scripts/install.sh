#!/usr/bin/env sh
set -eu

# Downloads the latest GitForge release tarball from GitHub and installs it into
# ~/.local. The in-app auto-updater keeps installs current after the first run.
#
#   curl -f https://raw.githubusercontent.com/b0red04/gitforge/main/scripts/install.sh | sh
#
# Override the release with GITFORGE_VERSION=1.2.3 or a local bundle with
# GITFORGE_BUNDLE_PATH=/path/to/GitForge-x.y.z-arch.tar.gz.

GITHUB_REPO="b0red04/gitforge"
APP_ID="dev.gitforge.GitForge"

main() {
  platform="$(uname -s)"
  arch="$(uname -m)"
  version="${GITFORGE_VERSION:-latest}"

  if [ -n "${TMPDIR:-}" ] && [ -d "${TMPDIR}" ]; then
    temp="$(mktemp -d "$TMPDIR/gitforge-XXXXXX")"
  else
    temp="$(mktemp -d "/tmp/gitforge-XXXXXX")"
  fi

  if [ "$platform" = "Linux" ]; then
    platform="linux"
  else
    echo "Unsupported platform: $platform (Linux only)" >&2
    exit 1
  fi

  case "$arch" in
    arm64 | aarch64) arch="aarch64" ;;
    x86_64 | amd64) arch="x86_64" ;;
    *)
      echo "Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  if command -v curl >/dev/null 2>&1; then
    curl() {
      command curl -fL "$@"
    }
  elif command -v wget >/dev/null 2>&1; then
    curl() {
      wget -O- "$@"
    }
  else
    echo "Could not find 'curl' or 'wget' in your PATH" >&2
    exit 1
  fi

  linux "$arch" "$version" "$temp"
  print_path_hint
}

linux() {
  arch="$1"
  version="$2"
  temp="$3"
  tarball="$temp/gitforge-linux-${arch}.tar.gz"

  if [ -n "${GITFORGE_BUNDLE_PATH:-}" ]; then
    cp "$GITFORGE_BUNDLE_PATH" "$tarball"
    version="$(basename "$GITFORGE_BUNDLE_PATH" | sed -n 's/^GitForge-\(.*\)-'"$arch"'\.tar\.gz$/\1/p')"
    if [ -z "$version" ]; then
      version="bundle"
    fi
  else
    if [ "$version" = "latest" ]; then
      echo "Fetching latest GitForge release..."
      release_json="$(curl -s "https://api.github.com/repos/${GITHUB_REPO}/releases/latest")"
      tag="$(printf '%s\n' "$release_json" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
      version="${tag#v}"
      asset_name="GitForge-${version}-${arch}.tar.gz"
      download_url="$(printf '%s\n' "$release_json" | tr ',' '\n' | sed -n 's/.*"browser_download_url": *"\([^"]*'"$asset_name"'\)".*/\1/p' | head -n 1)"
      if [ -z "$download_url" ]; then
        echo "Release asset not found: $asset_name" >&2
        exit 1
      fi
    else
      asset_name="GitForge-${version}-${arch}.tar.gz"
      download_url="https://github.com/${GITHUB_REPO}/releases/download/v${version}/${asset_name}"
    fi

    echo "Downloading GitForge ${version} for ${arch}..."
    curl "$download_url" > "$tarball"
  fi

  install_dir="${HOME}/.local"
  app_dir="${install_dir}/gitforge.app"

  rm -rf "$app_dir"
  mkdir -p "$install_dir"
  tar -xzf "$tarball" -C "$install_dir"

  mkdir -p "${install_dir}/bin" "${install_dir}/share/applications"

  ln -sf "${app_dir}/usr/bin/gitforge" "${install_dir}/bin/gitforge"

  desktop_src="${app_dir}/usr/share/applications/${APP_ID}.desktop"
  desktop_dst="${install_dir}/share/applications/${APP_ID}.desktop"
  cp "$desktop_src" "$desktop_dst"

  icon_path="${app_dir}/usr/share/icons/hicolor/256x256/apps/${APP_ID}.png"
  if [ ! -f "$icon_path" ]; then
    icon_path="$(find "${app_dir}/usr/share/icons/hicolor" -name "${APP_ID}.png" 2>/dev/null | head -n 1 || true)"
  fi

  if [ -n "$icon_path" ] && [ -f "$icon_path" ]; then
    sed -i "s|^Icon=.*|Icon=${icon_path}|" "$desktop_dst"
  fi
  sed -i "s|^Exec=.*|Exec=${app_dir}/usr/bin/gitforge|" "$desktop_dst"

  echo "GitForge ${version} installed to ${app_dir}"
}

print_path_hint() {
  if [ "$(command -v gitforge)" = "${HOME}/.local/bin/gitforge" ]; then
    echo "Run GitForge with: gitforge"
    return
  fi

  echo "Add ~/.local/bin to your PATH to run 'gitforge' from a terminal."
  case "${SHELL:-}" in
    *zsh)
      echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
      echo "  source ~/.zshrc"
      ;;
    *fish)
      echo "  fish_add_path -U \$HOME/.local/bin"
      ;;
    *)
      echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
      echo "  source ~/.bashrc"
      ;;
  esac
  echo "To run GitForge now: ~/.local/bin/gitforge"
}

main "$@"
