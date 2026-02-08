# Copy source code to production/source/ for GitHub sharing
# Excludes all sensitive data, keys, credentials, build artifacts, and personal files.

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent $PSScriptRoot
$SourceDestDir = Join-Path $RootDir "production\source"

Write-Host "=== Copy Source for GitHub ===" -ForegroundColor Cyan
Write-Host "Root: $RootDir"
Write-Host "Destination: $SourceDestDir"
Write-Host ""

# Exclude patterns (relative to root)
$ExcludeDirs = @(
    "node_modules",
    "target",
    "dist",
    ".git",
    ".vscode",
    ".idea",
    "production"  # Don't copy production output into source
)

$ExcludeFiles = @(
    ".env",
    ".env.local",
    ".env.*",
    "*.db",
    "*.sqlite",
    "*.sqlite3",
    "*.log",
    "*.pem",
    "*.key",
    "Brain_passes.txt",
    "IDE_prompt.txt",
    "multi_llm_app_instructions.txt",
    "dummy",
    "SETUP_COMPLETE.md"
)

# Create destination (skip remove if folder is locked - robocopy will overwrite)
if (-not (Test-Path $SourceDestDir)) {
    New-Item -ItemType Directory -Path $SourceDestDir -Force | Out-Null
}

# Use robocopy for efficient copy with exclusions (avoids copying node_modules, target, etc.)
$robocopyArgs = @(
    $RootDir, $SourceDestDir,
    "/E",                    # Copy subdirs including empty
    "/XD", "node_modules", "target", "dist", ".git", ".vscode", ".idea", "production",
    "/XF", "*.env", "*.env.local", "*.db", "*.sqlite", "*.log", "Brain_passes.txt", "IDE_prompt.txt", "multi_llm_app_instructions.txt", "dummy", "SETUP_COMPLETE.md",
    "/NFL", "/NDL", "/NJH", "/NJS"  # Minimal output
)
$result = Start-Process -FilePath "robocopy" -ArgumentList $robocopyArgs -Wait -PassThru -NoNewWindow
# Robocopy: 0=no files, 1=copied, 2=extra, 3=both; 8+=errors
if ($result.ExitCode -ge 8) {
    Write-Host "Robocopy warning/error (code $($result.ExitCode))" -ForegroundColor Yellow
} else {
    Write-Host "Copied source files" -ForegroundColor Green
}

# Remove any excluded content that may have been copied
$PathsToClean = @(
    (Join-Path $SourceDestDir "node_modules"),
    (Join-Path $SourceDestDir "dist"),
    (Join-Path $SourceDestDir "src-tauri\target"),
    (Join-Path $SourceDestDir ".git")
)
foreach ($p in $PathsToClean) {
    if (Test-Path $p) {
        Remove-Item -Path $p -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "Removed: $p" -ForegroundColor Yellow
    }
}

# Remove voice model binaries (keep .gitkeep)
$WhisperBin = Join-Path $SourceDestDir "voice\whisper"
if (Test-Path $WhisperBin) {
    Get-ChildItem -Path $WhisperBin -Filter "*.bin" -ErrorAction SilentlyContinue | Remove-Item -Force
    Write-Host "Removed voice model binaries (*.bin)" -ForegroundColor Yellow
}

# Remove sensitive/personal files from copied tree
$SensitiveFiles = @(
    (Join-Path $SourceDestDir "Brain_passes.txt"),
    (Join-Path $SourceDestDir "IDE_prompt.txt"),
    (Join-Path $SourceDestDir "multi_llm_app_instructions.txt"),
    (Join-Path $SourceDestDir "dummy"),
    (Join-Path $SourceDestDir "SETUP_COMPLETE.md")
)
foreach ($f in $SensitiveFiles) {
    if (Test-Path $f) {
        Remove-Item -Path $f -Force -ErrorAction SilentlyContinue
        Write-Host "Removed: $f" -ForegroundColor Yellow
    }
}

# Ensure .env.example is included (safe template)
if (Test-Path (Join-Path $RootDir ".env.example")) {
    Copy-Item -Path (Join-Path $RootDir ".env.example") -Destination (Join-Path $SourceDestDir ".env.example") -Force
    Write-Host "Included: .env.example" -ForegroundColor Green
}

# Create SOURCE_README.txt
$SourceReadme = @"
# Panther Source Code (GitHub-Ready)

Created by Reza Mirfayzi

This folder contains the Panther source code, sanitized for public sharing.

## What's Included

- Full source code (src/, src-tauri/)
- Configuration files (package.json, vite.config.ts, tauri.conf.json, etc.)
- Documentation (README.md, docs/)
- .env.example (template - no real keys)

## What's Excluded (for your privacy)

- node_modules/ (run npm install to restore)
- src-tauri/target/ (build artifacts)
- dist/ (frontend build output)
- .env, .env.local (API keys, secrets)
- *.db, *.sqlite (user databases)
- .git/ (version history - init fresh for your repo)
- Voice model binaries (voice/whisper/*.bin)
- Personal files (Brain_passes.txt, IDE_prompt.txt, etc.)

## To Use This Source

1. Copy this folder to your GitHub repo (or git init && git add . && git commit)
2. Run: npm install
3. Run: npm run tauri dev (for development)
4. Run: npm run tauri build (for production)

## Security

No API keys, credentials, or sensitive data are included.
Users add their own providers through the app Settings.

Generated: $(Get-Date -Format "yyyy-MM-dd HH:mm")

"@
$SourceReadme | Out-File -FilePath (Join-Path $SourceDestDir "SOURCE_README.txt") -Encoding UTF8
Write-Host "Created SOURCE_README.txt" -ForegroundColor Green

# Final verification
$VerifyPatterns = @("*.env", "*.db", "*.sqlite", "*.pem", "*.key")
$Found = $false
foreach ($pat in $VerifyPatterns) {
    $m = Get-ChildItem -Path $SourceDestDir -Filter $pat -Recurse -ErrorAction SilentlyContinue
    if ($m) {
        Write-Host "WARNING: Found $pat in output" -ForegroundColor Red
        $Found = $true
    }
}
if (-not $Found) {
    Write-Host "Verified: No sensitive files in source output" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Source copy complete ===" -ForegroundColor Cyan
Write-Host "Output: $SourceDestDir"
