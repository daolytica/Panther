# Backup instructions

The full project folder can be **several GB to 16GB+** because of build artifacts, dependencies, and git history. You can reduce it to **a few MB to tens of MB** (often ~1–50 MB) by excluding what can be recreated.

## Why is my folder so big? (e.g. 16 GB)

| Folder / path           | Typical size   | Safe to omit? |
|-------------------------|----------------|---------------|
| `src-tauri/target/`     | **2–10+ GB**   | Yes – recreated by `cargo build` |
| `node_modules/`         | **250 MB–2 GB**| Yes – recreated by `npm install` |
| `.git/`                 | **100 MB–5 GB**| Omit for small backup; use `-IncludeGit` to keep history |
| `dist/`                 | tens of MB     | Yes – recreated by `npm run build` |
| `.cursor/`              | can be large   | Yes – IDE cache |
| `*.db` / `*.sqlite`     | varies         | Omit from zip if you back up DB separately |
| Your source + config    | ~50–150 MB    | **Include** – this is what you need |

## Small backup (source-only)

Back up the project **excluding**: `node_modules`, `src-tauri/target`, `dist`, `.git`, `.cursor`, and `*.db`/`*.sqlite`. That reduces 16 GB to roughly **a few MB to tens of MB** (e.g. 1.5–50 MB).

### Option 1: Run the backup script (recommended)

From the project root (PowerShell):

```powershell
.\scripts\backup-project.ps1
```

This creates `Panther_backup_YYYYMMDD.zip` in the **parent** of the project folder (e.g. `E:\Application_developments\Panther_backup_20250202.zip`). To **include** full git history (larger backup):

```powershell
.\scripts\backup-project.ps1 -IncludeGit
```

Unzip elsewhere, then run `npm install` and `npm run tauri build` (or `cargo build --release` in `src-tauri`) to restore.

### Option 2: Manual Zip (Windows)

1. In File Explorer, go to the project folder.
2. Select all **except**: `node_modules`, `src-tauri\target`, `dist`, `.git`, `.cursor` (and optionally `*.db` / `*.sqlite` if you back them up separately).
3. Right‑click → **Send to** → **Compressed (zipped) folder**.

### Option 3: 7-Zip (smallest size, full control)

If you have 7-Zip installed, you can exclude heavy folders and get better compression:

```powershell
cd E:\Application_developments
7z a -tzip Panther_backup.zip Panther_v2\ -xr!node_modules -xr!target -xr!dist -xr!.git -xr!.cursor -x!*.db -x!*.sqlite
```

For reliable exclusion of nested folders, the backup script (Option 1) or 7-Zip is best; `Compress-Archive` doesn’t exclude subfolders like `target` easily.

## Restore from a source-only backup

1. Unzip the backup to a new folder.
2. In the project root:
   - `npm install`
   - `cd src-tauri && cargo build --release` (or from root: `npm run tauri build`)

Your backup stays small (often 1–50 MB, sometimes as low as ~1.5 MB) instead of 16 GB+.
