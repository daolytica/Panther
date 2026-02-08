// Voice module: local STT (Whisper) and TTS (Piper)

pub mod stt;
pub mod tts;

use std::path::PathBuf;
use std::fs;

/// Project-root voice path: when exe is in src-tauri/target/{debug,release}/, project root is 4 levels up
fn project_voice_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut p = exe.parent()?;
    for _ in 0..3 {
        p = p.parent()?;
    }
    let project_root = PathBuf::from(p);
    if project_root.join("package.json").exists() {
        Some(project_root.join("voice"))
    } else {
        None
    }
}

/// App-directory voice path: next to the executable (e.g. ./voice/whisper)
fn app_voice_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .map(|p| p.join("voice"))
}

/// User data voice path: APPDATA/panther/voice or ~/.local/share/panther/voice
fn user_voice_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("panther").join("voice"))
            .unwrap_or_else(|_| PathBuf::from(".").join("voice"))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("voice"))
            .unwrap_or_else(|_| PathBuf::from(".").join("voice"))
    }
}

/// Get the voice models base directory. Order: project root, app dir (next to exe), user data.
pub fn get_voice_dir() -> Result<PathBuf, String> {
    let dir = project_voice_dir()
        .or_else(app_voice_dir)
        .unwrap_or_else(user_voice_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create voice directory: {}", e))?;
    Ok(dir)
}

/// Get Whisper models directory. Order: project root, app dir (next to exe), user data.
pub fn get_whisper_dir() -> Result<PathBuf, String> {
    let dir = project_voice_dir()
        .map(|p| p.join("whisper"))
        .or_else(|| app_voice_dir().map(|p| p.join("whisper")))
        .unwrap_or_else(|| user_voice_dir().join("whisper"));
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create whisper directory: {}", e))?;
    Ok(dir)
}

/// Get Piper voices directory
pub fn get_piper_dir() -> Result<PathBuf, String> {
    let dir = get_voice_dir()?.join("piper");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create piper directory: {}", e))?;
    Ok(dir)
}
