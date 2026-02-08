# Panther Production Build

This folder contains the production build of Panther (Advanced AI Agent Platform).

## Contents

- **brain-stormer.exe** - Standalone executable (no installer required)
- **Brain Stormer_*_x64-setup.exe** - NSIS installer (if present)
- **Brain Stormer_*_x64_en-US.msi** - MSI installer (if present)
- **source/** - GitHub-ready source code (no keys or sensitive data)

## Mac Builds

Mac (.dmg) builds cannot be created on Windows. Use GitHub Actions: push to the `release` branch or run the "Build Release" workflow. The source/ folder includes .github/workflows/release.yml for this.

## Security Notice

**No API keys or sensitive information are included in this build.**

- API keys are stored in the Windows Credential Manager (keychain) after the user adds providers
- The database is created at %APPDATA%\panther\panther.db on first run
- Users must add their own providers and API keys through the app Settings

## Requirements

- Windows 10/11 (64-bit)
- WebView2 runtime (usually pre-installed on Windows 11; may need to install on Windows 10)

## Usage

1. Run **brain-stormer.exe** or install via the MSI/NSIS installer
2. Add providers (Settings â†’ Providers) and enter your API keys
3. Create profiles and start using the app

## Build Date

2026-02-05 15:19

