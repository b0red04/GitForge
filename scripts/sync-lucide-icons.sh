#!/usr/bin/env bash
# Sync assets/icons/*.svg from Lucide (https://lucide.dev).
# Brand icons (github, gitlab) use Lucide v0.544.0 — removed in Lucide v1.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICONS_DIR="${ROOT}/assets/icons"
CACHE_DIR="${TMPDIR:-/tmp}/gitforge-lucide-sync"
LUCIDE_TAG="main"
BRAND_TAG="0.544.0"

mkdir -p "${CACHE_DIR}"
TARBALL="${CACHE_DIR}/lucide-${LUCIDE_TAG}.tar.gz"
if [[ ! -f "${TARBALL}" ]]; then
  curl -sL "https://codeload.github.com/lucide-icons/lucide/tar.gz/${LUCIDE_TAG}" -o "${TARBALL}"
fi
EXTRACT="${CACHE_DIR}/lucide-${LUCIDE_TAG}"
if [[ ! -d "${EXTRACT}/icons" ]]; then
  rm -rf "${EXTRACT}"
  mkdir -p "${EXTRACT}"
  tar -xzf "${TARBALL}" -C "${CACHE_DIR}"
  mv "${CACHE_DIR}/lucide-"* "${EXTRACT}" 2>/dev/null || true
fi
LUCIDE_SRC="$(find "${CACHE_DIR}" -maxdepth 2 -type d -name icons | head -1)"
if [[ -z "${LUCIDE_SRC}" || ! -d "${LUCIDE_SRC}" ]]; then
  echo "Could not find Lucide icons directory" >&2
  exit 1
fi

copy_lucide() {
  local dest_name="$1"
  local lucide_name="$2"
  local src="${LUCIDE_SRC}/${lucide_name}.svg"
  if [[ ! -f "${src}" ]]; then
    echo "Missing Lucide icon: ${lucide_name}.svg" >&2
    exit 1
  fi
  cp "${src}" "${ICONS_DIR}/${dest_name}"
  echo "  ${dest_name} <- ${lucide_name}"
}

fetch_brand() {
  local dest_name="$1"
  local lucide_name="$2"
  curl -sL "https://raw.githubusercontent.com/lucide-icons/lucide/${BRAND_TAG}/icons/${lucide_name}.svg" \
    -o "${ICONS_DIR}/${dest_name}"
  echo "  ${dest_name} <- ${lucide_name} (Lucide ${BRAND_TAG})"
}

echo "Syncing Lucide icons into ${ICONS_DIR}..."

# UI / window chrome
copy_lucide "generic_close.svg" "x"
copy_lucide "generic_minimize.svg" "minus"
copy_lucide "generic_maximize.svg" "square"
copy_lucide "generic_restore.svg" "copy"
copy_lucide "x.svg" "x"

# Git
copy_lucide "git-commit.svg" "git-commit-vertical"
copy_lucide "git-branch.svg" "git-branch"
copy_lucide "git-merge.svg" "git-merge"
copy_lucide "git-pull-request.svg" "git-pull-request"
copy_lucide "git_graph.svg" "git-graph"
copy_lucide "git_merge_conflict.svg" "git-merge-conflict"
copy_lucide "git_branch_plus.svg" "git-branch-plus"
copy_lucide "git_worktree.svg" "git-compare-arrows"
copy_lucide "file_git.svg" "folder-git"
copy_lucide "git.svg" "git-fork"

# Files & navigation
copy_lucide "file.svg" "file"
copy_lucide "folder.svg" "folder"
copy_lucide "tag.svg" "tag"
copy_lucide "search.svg" "search"
copy_lucide "settings.svg" "settings"
copy_lucide "plus.svg" "plus"
copy_lucide "check.svg" "check"
copy_lucide "chevron-down.svg" "chevron-down"
copy_lucide "chevron-right.svg" "chevron-right"
copy_lucide "arrow-down.svg" "arrow-down"
copy_lucide "arrow-up.svg" "arrow-up"
copy_lucide "refresh.svg" "refresh-cw"
copy_lucide "cloud.svg" "cloud"
copy_lucide "terminal.svg" "terminal"
copy_lucide "globe.svg" "globe"
copy_lucide "laptop.svg" "laptop"

# Brand (legacy Lucide release)
fetch_brand "github.svg" "github"
fetch_brand "gitlab.svg" "gitlab"

echo "Done. See assets/icons/ATTRIBUTION.md for license."
