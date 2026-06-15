# Build a zip for in-app auto-update on Windows (Zed-style staging layout).
# Requires: cargo build already done (target/release/gitforge.exe and
# target/release/gitforge-update-helper.exe).
param(
    [Parameter()]
    [string]$Version
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path "$PSScriptRoot/..").Path
$AppId = "dev.gitforge.GitForge"
$Binary = Join-Path $Root "target/release/gitforge.exe"
$Staging = Join-Path $Root "target/update-zip/gitforge"

if ([string]::IsNullOrEmpty($Version)) {
    $Version = (& cargo pkgid -p gitforge-app --manifest-path (Join-Path $Root "Cargo.toml") 2>$null) -replace '^.*@', ''
}
if ([string]::IsNullOrEmpty($Version)) {
    throw "version required (pass release tag version as -Version)"
}

$Arch = & {
    $env:PROCESSOR_ARCHITECTURE = $env:PROCESSOR_ARCHITECTURE
    # cargo's TARGET_ARCH convention: x86_64 / aarch64
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
}

if (-not (Test-Path $Binary)) {
    throw "build gitforge first (cargo build -p gitforge-app --release)"
}

Write-Host "Building update zip version=$Version arch=$Arch..."

if (Test-Path $Staging) { Remove-Item -Recurse -Force $Staging }
$null = New-Item -ItemType Directory -Force -Path $Staging
$null = New-Item -ItemType Directory -Force -Path (Join-Path $Staging "themes")

Copy-Item $Binary (Join-Path $Staging "gitforge.exe")

# Stage the auto-update helper alongside the main binary (Phase 5).
$Helper = Join-Path $Root "target/release/gitforge-update-helper.exe"
if (Test-Path $Helper) {
    Copy-Item $Helper (Join-Path $Staging "gitforge-update-helper.exe")
}

# Bundle themes.
$ThemesSrc = Join-Path $Root "assets/themes"
if (Test-Path $ThemesSrc) {
    Copy-Item (Join-Path $ThemesSrc "*.json") (Join-Path $Staging "themes/") -Force
}

$Output = Join-Path $Root "GitForge-$Version-windows-$Arch.zip"
$StagingParent = Split-Path -Parent $Staging
$StagingLeaf = Split-Path -Leaf $Staging
# .NET ZipFile.CreateFromDirectory needs a parent/child layout to get the
# top-level folder name right inside the archive.
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($Staging, $Output, [System.IO.Compression.CompressionLevel]::Optimal, $false)

# Compute checksum (sha256sum-compatible format: "<hash>  <filename>").
$Hash = (Get-FileHash $Output -Algorithm SHA256).Hash.ToLower()
$HashFile = "$Output.sha256"
"$Hash  $(Split-Path -Leaf $Output)" | Out-File -Encoding ascii -FilePath $HashFile

Write-Host "Update zip created: $Output"
Write-Host "Checksum: $HashFile"
