// Text-to-Speech - stub for Piper (piper-rs has heavy deps: espeak-ng, onnxruntime, libclang)
// When Piper is integrated, replace this with actual synthesis.
// For now, return error so frontend falls back to browser speechSynthesis.

use crate::voice::get_piper_dir;

/// Synthesize text to WAV bytes (base64).
/// Returns error - use browser TTS when local Piper is not available.
pub fn synthesize_speech(text: &str, _voice_id: Option<String>) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }

    let piper_dir = get_piper_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    Err(format!(
        "Local TTS (Piper) not built in. Use browser TTS, or place Piper voice models in: {} \
         (piper-rs requires espeak-ng and ONNX runtime to enable local TTS)",
        piper_dir.display()
    ))
}
