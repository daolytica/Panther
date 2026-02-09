// Tauri commands for voice (STT/TTS)

use crate::voice::{stt, tts};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeRequest {
    pub audio_base64: String,
    #[serde(default)]
    pub model_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesizeRequest {
    pub text: String,
    #[serde(default)]
    pub voice_id: Option<String>,
}

#[tauri::command]
pub async fn transcribe_audio(request: TranscribeRequest) -> Result<serde_json::Value, String> {
    let text = tokio::task::spawn_blocking(move || {
        stt::transcribe_audio(&request.audio_base64, request.model_id)
    })
    .await
    .map_err(|e| format!("Transcription task failed: {}", e))?
    .map_err(|e| e)?;

    Ok(serde_json::json!({ "text": text }))
}

#[tauri::command]
pub async fn synthesize_speech(request: SynthesizeRequest) -> Result<String, String> {
    let text = request.text;
    let voice_id = request.voice_id;

    let audio_base64 = tokio::task::spawn_blocking(move || {
        tts::synthesize_speech(&text, voice_id)
    })
    .await
    .map_err(|e| format!("Synthesis task failed: {}", e))?
    .map_err(|e| e)?;

    Ok(audio_base64)
}

#[tauri::command]
pub fn get_whisper_models_dir() -> Result<String, String> {
    let dir = crate::voice::get_whisper_dir()?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_piper_voices_dir() -> Result<String, String> {
    let dir = crate::voice::get_piper_dir()?;
    Ok(dir.to_string_lossy().to_string())
}
