// Workspace commands - File operations and terminal execution for the Coder IDE

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// File entry in the workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

/// Result of a terminal command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Get the user's home directory
fn get_user_home() -> Result<PathBuf, String> {
    if cfg!(windows) {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            Ok(PathBuf::from(profile))
        } else if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
            Ok(PathBuf::from(format!("{}{}", drive, path)))
        } else {
            Err("Failed to get user home directory".to_string())
        }
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .map_err(|_| "Failed to get user home directory".to_string())
    }
}

/// Get the workspace root directory (for creating files)
fn get_workspace_root() -> PathBuf {
    // Use a dedicated workspace folder in app data
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("panther").join("workspace"))
            .unwrap_or_else(|_| PathBuf::from("./workspace"))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("workspace"))
            .unwrap_or_else(|_| PathBuf::from("./workspace"))
    }
}

/// Ensure workspace directory exists
#[allow(dead_code)]
fn ensure_workspace() -> Result<PathBuf, String> {
    let root = get_workspace_root();
    fs::create_dir_all(&root).map_err(|e| format!("Failed to create workspace: {}", e))?;
    Ok(root)
}

/// Validate and resolve path - allow user home directory access
fn resolve_path(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() {
        return get_user_home();
    }
    
    // If path starts with ~, expand to home directory
    let expanded = if path.starts_with('~') {
        let home = get_user_home()?;
        if path == "~" {
            home
        } else {
            home.join(&path[2..])
        }
    } else if Path::new(path).is_absolute() {
        // Absolute path - use as-is
        PathBuf::from(path)
    } else {
        // Relative path - resolve from current directory or home
        let base = get_user_home()?;
        base.join(path)
    };
    
    // Canonicalize to resolve .. and symlinks
    expanded.canonicalize()
        .or_else(|_| Ok(expanded))
        .map_err(|e: std::io::Error| format!("Failed to resolve path: {}", e))
}

#[tauri::command]
pub async fn get_workspace_path() -> Result<String, String> {
    let home = get_user_home()?;
    Ok(home.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_workspace_files(path: Option<String>) -> Result<Vec<FileEntry>, String> {
    let target = if let Some(p) = path {
        resolve_path(&p)?
    } else {
        get_user_home()?
    };
    
    if !target.exists() {
        return Ok(Vec::new());
    }
    
    let mut entries = Vec::new();
    // Use the user's home directory as the logical workspace root for
    // relative paths shown in the IDE.
    let home_root = get_user_home()?;
    
    let read_dir = fs::read_dir(&target)
        .map_err(|e| format!("Failed to read directory: {}", e))?;
    
    for entry in read_dir {
        if let Ok(entry) = entry {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let is_dir = path.is_dir();
            
            // Skip hidden files/folders
            if name.starts_with('.') {
                continue;
            }
            
            let relative_path = path.strip_prefix(&home_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            
            entries.push(FileEntry {
                name,
                path: relative_path,
                is_dir,
                children: None,
            });
        }
    }
    
    // Sort: directories first, then files, alphabetically
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    Ok(entries)
}

#[tauri::command]
pub async fn read_workspace_file(path: String) -> Result<String, String> {
    let full_path = resolve_path(&path)?;
    
    if !full_path.exists() {
        return Err("File not found".to_string());
    }
    
    if full_path.is_dir() {
        return Err("Cannot read a directory".to_string());
    }
    
    fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read file: {}", e))
}

#[tauri::command]
pub async fn write_workspace_file(path: String, content: String) -> Result<(), String> {
    let full_path = resolve_path(&path)?;
    
    // Create parent directories if needed
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    
    fs::write(&full_path, content)
        .map_err(|e| format!("Failed to write file: {}", e))
}

#[tauri::command]
pub async fn create_workspace_file(path: String, is_dir: bool) -> Result<(), String> {
    let full_path = resolve_path(&path)?;
    
    if full_path.exists() {
        return Err("File or directory already exists".to_string());
    }
    
    if is_dir {
        fs::create_dir_all(&full_path)
            .map_err(|e| format!("Failed to create directory: {}", e))
    } else {
        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
        fs::write(&full_path, "")
            .map_err(|e| format!("Failed to create file: {}", e))
    }
}

#[tauri::command]
pub async fn delete_workspace_file(path: String) -> Result<(), String> {
    let full_path = resolve_path(&path)?;
    
    if !full_path.exists() {
        return Err("File or directory not found".to_string());
    }
    
    if full_path.is_dir() {
        fs::remove_dir_all(&full_path)
            .map_err(|e| format!("Failed to delete directory: {}", e))
    } else {
        fs::remove_file(&full_path)
            .map_err(|e| format!("Failed to delete file: {}", e))
    }
}

#[tauri::command]
pub async fn rename_workspace_file(old_path: String, new_path: String) -> Result<(), String> {
    let old_full = resolve_path(&old_path)?;
    let new_full = resolve_path(&new_path)?;
    
    if !old_full.exists() {
        return Err("Source file not found".to_string());
    }
    
    if new_full.exists() {
        return Err("Destination already exists".to_string());
    }
    
    // Create parent directories for new path if needed
    if let Some(parent) = new_full.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directories: {}", e))?;
    }
    
    fs::rename(&old_full, &new_full)
        .map_err(|e| format!("Failed to rename: {}", e))
}

#[tauri::command]
pub async fn execute_command(
    command: String,
    working_dir: Option<String>,
) -> Result<CommandResult, String> {
    eprintln!("🔧 Executing command: '{}' in dir: {:?}", command, working_dir);

    let cwd = if let Some(dir) = working_dir {
        let resolved = resolve_path(&dir)?;
        eprintln!("🔧 Resolved working directory: {:?}", resolved);
        resolved
    } else {
        let home = get_user_home()?;
        eprintln!("🔧 Using home directory: {:?}", home);
        home
    };

    // Use PowerShell on Windows with proper execution policy
    let output = if cfg!(windows) {
        eprintln!("🔧 Using PowerShell to execute: {}", command);
        // Use -ExecutionPolicy Bypass to avoid UnauthorizedAccess
        Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&command)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?
    } else {
        eprintln!("🔧 Using sh to execute: {}", command);
        Command::new("sh")
            .arg("-c")
            .arg(&command)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    eprintln!("🔧 Command result - exit_code: {}, success: {}, stdout_len: {}, stderr_len: {}",
              exit_code, output.status.success(), stdout.len(), stderr.len());

    Ok(CommandResult {
        stdout,
        stderr,
        exit_code,
        success: output.status.success(),
    })
}

#[tauri::command]
pub async fn get_current_directory() -> Result<String, String> {
    let home = get_user_home()?;
    Ok(home.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn change_directory(path: String) -> Result<String, String> {
    let target = resolve_path(&path)?;
    if !target.exists() {
        return Err("Directory does not exist".to_string());
    }
    if !target.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn check_dependency(dependency: String) -> Result<bool, String> {
    let output = if cfg!(windows) {
        Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("Get-Command {} -ErrorAction SilentlyContinue", dependency))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to check dependency: {}", e))?
    } else {
        Command::new("which")
            .arg(&dependency)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to check dependency: {}", e))?
    };
    
    Ok(output.status.success())
}

#[tauri::command]
pub async fn install_dependency_command(_dependency: String, install_command: String) -> Result<CommandResult, String> {
    let output = if cfg!(windows) {
        Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(&install_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to install dependency: {}", e))?
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&install_command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("Failed to install dependency: {}", e))?
    };
    
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    
    Ok(CommandResult {
        stdout,
        stderr,
        exit_code,
        success: output.status.success(),
    })
}

#[tauri::command]
pub async fn get_file_language(path: String) -> Result<String, String> {
    let extension = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    
    let language = match extension.to_lowercase().as_str() {
        "rs" => "rust",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "cpp",
        "cs" => "csharp",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "xml" => "xml",
        "md" | "markdown" => "markdown",
        "sql" => "sql",
        "sh" | "bash" => "shellscript",
        "ps1" => "powershell",
        "bat" | "cmd" => "bat",
        "dockerfile" => "dockerfile",
        "toml" => "toml",
        "ini" | "conf" | "cfg" => "ini",
        "lua" => "lua",
        "r" => "r",
        "dart" => "dart",
        "vue" => "vue",
        "svelte" => "svelte",
        _ => "plaintext",
    };
    
    Ok(language.to_string())
}
