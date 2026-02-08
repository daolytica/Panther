# Create a small backup by excluding everything that can be recreated (reduces 16GB+ to ~1–50 MB).
# Run from project root: .\scripts\backup-project.ps1
# Restore: unzip, then npm install and cargo build (or npm run tauri build).

param(
    [switch]$IncludeGit   # Pass -IncludeGit to include .git (larger backup, full history)
)

$ErrorActionPreference = "Stop"
$root = if ($PSScriptRoot) { Split-Path $PSScriptRoot -Parent } else { Get-Location }
$parent = Split-Path $root -Parent
$stamp = Get-Date -Format "yyyyMMdd"
$destDir = Join-Path $parent "Panther_backup_$stamp"
$zipPath = Join-Path $parent "Panther_backup_$stamp.zip"

# Exclude: build artifacts, deps, caches, and optionally .git (saves a lot if repo is big)
$excludeDirs = @("node_modules", "dist", "target", ".cursor", ".vscode", ".idea")
if (-not $IncludeGit) {
    $excludeDirs += ".git"
}

Write-Host "Backing up (excluding: $($excludeDirs -join ', '))..." -ForegroundColor Cyan
Write-Host "  From: $root"
Write-Host "  To:   $zipPath"

if (Test-Path $destDir) {
    Remove-Item $destDir -Recurse -Force
}
New-Item -ItemType Directory -Path $destDir -Force | Out-Null

# Robocopy: /XD = exclude dirs (by name, any depth), /XF = exclude files
& robocopy $root $destDir /E /NFL /NDL /NJH /NJS /NC /NS /XD $excludeDirs /XF *.db *.sqlite *.sqlite3 2>&1 | Out-Null
# Exit 0=nothing to copy, 1=files copied, 2+ = extra; 8+ = failure
if ($LASTEXITCODE -ge 8) {
    Write-Host "Robocopy failed with exit code $LASTEXITCODE" -ForegroundColor Red
    exit $LASTEXITCODE
}

# Ensure target and node_modules are gone (robocopy /XD can miss some edge cases)
@("src-tauri\target", "node_modules") | ForEach-Object {
    $p = Join-Path $destDir $_
    if (Test-Path $p) {
        Remove-Item $p -Recurse -Force
        Write-Host "  Removed $_ from backup." -ForegroundColor Yellow
    }
}

if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
Compress-Archive -Path $destDir -DestinationPath $zipPath -CompressionLevel Optimal
Remove-Item $destDir -Recurse -Force

$sizeMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
Write-Host "Done. Backup: $zipPath ($sizeMB MB)" -ForegroundColor Green
Write-Host "Restore: unzip, then 'npm install' and 'npm run tauri build' (or 'cargo build --release' in src-tauri)."
exit 0
