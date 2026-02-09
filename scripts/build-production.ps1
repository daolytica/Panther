# Build Panther for production and copy artifacts to production/ folder
# No API keys or sensitive data are included in the build.

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
$ProductionDir = Join-Path $RootDir "production"

Write-Host "=== Panther Production Build ===" -ForegroundColor Cyan
Write-Host "Root: $RootDir"
Write-Host "Output: $ProductionDir"
Write-Host ""

# Ensure production directory exists and is clean for fresh copy
if (Test-Path $ProductionDir) {
    Write-Host "Cleaning production folder..." -ForegroundColor Yellow
    Remove-Item -Path "$ProductionDir\*" -Recurse -Force -ErrorAction SilentlyContinue
} else {
    New-Item -ItemType Directory -Path $ProductionDir -Force | Out-Null
}

# Build the application (release mode)
Write-Host "Building application (this may take several minutes)..." -ForegroundColor Cyan
Push-Location $RootDir
try {
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }

    npx tauri build
    if ($LASTEXITCODE -ne 0) { throw "Tauri build failed" }
} finally {
    Pop-Location
}

# Paths
$TargetRelease = Join-Path $RootDir "src-tauri\target\release"
$TargetBundle = Join-Path $TargetRelease "bundle"
$BinaryDir = Join-Path $ProductionDir "binary"
New-Item -ItemType Directory -Path $BinaryDir -Force | Out-Null

# Copy installer(s)
$MsiDir = Join-Path $TargetBundle "msi"
$NsisDir = Join-Path $TargetBundle "nsis"

if (Test-Path $MsiDir) {
    Copy-Item -Path "$MsiDir\*" -Destination $BinaryDir -Recurse -Force
    Write-Host "Copied MSI installer" -ForegroundColor Green
}
if (Test-Path $NsisDir) {
    Copy-Item -Path "$NsisDir\*" -Destination $BinaryDir -Recurse -Force
    Write-Host "Copied NSIS installer" -ForegroundColor Green
}

# Copy standalone executable and required runtime files
$ExeName = "brain-stormer.exe"
if (Test-Path (Join-Path $TargetRelease $ExeName)) {
    Copy-Item -Path (Join-Path $TargetRelease $ExeName) -Destination $BinaryDir -Force
    Write-Host "Copied standalone executable: $ExeName" -ForegroundColor Green
}

# Copy webview2 runtime if bundled (Tauri may embed it)
$ResourcesDir = Join-Path $TargetRelease "resources"
if (Test-Path $ResourcesDir) {
    $DestResources = Join-Path $BinaryDir "resources"
    New-Item -ItemType Directory -Path $DestResources -Force | Out-Null
    Copy-Item -Path "$ResourcesDir\*" -Destination $DestResources -Recurse -Force -ErrorAction SilentlyContinue
}

# Create production README
$ReadmeContent = @"
# Panther Production Build

This folder contains the production build of Panther (Advanced AI Agent Platform).

## Contents

- **binary/** - Executables and installers (brain-stormer.exe, MSI, NSIS)
- **source/** - GitHub-ready source code (no keys or sensitive data)

## Security Notice

**No API keys or sensitive information are included in this build.**

- API keys are stored in the Windows Credential Manager (keychain) after the user adds providers
- The database is created at `%APPDATA%\panther\panther.db` on first run
- Users must add their own providers and API keys through the app Settings

## Requirements

- Windows 10/11 (64-bit)
- WebView2 runtime (usually pre-installed on Windows 11; may need to install on Windows 10)

## Mac Builds

Mac (.dmg) builds cannot be created on Windows. Use GitHub Actions: push to the `release` branch or run the "Build Release" workflow manually. See README.md for details.

## Usage

1. Run **binary\brain-stormer.exe** or install via the MSI/NSIS installer in binary/
2. Add providers (Settings → Providers) and enter your API keys
3. Create profiles and start using the app

## Build Date

$(Get-Date -Format "yyyy-MM-dd HH:mm")

"@
$ReadmeContent | Out-File -FilePath (Join-Path $ProductionDir "README.txt") -Encoding UTF8
Write-Host "Created README.txt" -ForegroundColor Green

# Verify no sensitive files
$SensitivePatterns = @("*.env", "*.db", "*.sqlite", "*.pem", "*.key")
$FoundSensitive = $false
foreach ($pattern in $SensitivePatterns) {
    $matches = Get-ChildItem -Path $ProductionDir -Filter $pattern -Recurse -ErrorAction SilentlyContinue
    if ($matches) {
        Write-Host "WARNING: Found potentially sensitive files: $($matches.FullName -join ', ')" -ForegroundColor Red
        $FoundSensitive = $true
    }
}
if (-not $FoundSensitive) {
    Write-Host "Verified: No sensitive files in production output" -ForegroundColor Green
}

# Copy source code for GitHub sharing (no sensitive data)
Write-Host ""
Write-Host "Copying source code for GitHub..." -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "copy-source-for-github.ps1")

Write-Host ""
Write-Host "=== Build complete ===" -ForegroundColor Cyan
Write-Host "Output: $ProductionDir"
Write-Host "Binary: $ProductionDir\binary"
Write-Host "Source (GitHub-ready): $ProductionDir\source"
Get-ChildItem $ProductionDir | ForEach-Object { Write-Host "  - $($_.Name)" }
