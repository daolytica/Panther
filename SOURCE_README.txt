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

Generated: 2026-02-09 09:09

