// Speech-to-Text via Whisper (when voice feature is enabled)

use crate::voice::get_whisper_dir;

#[cfg(feature = "voice")]
use base64::Engine;
#[cfg(feature = "voice")]
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[cfg(not(feature = "voice"))]
pub fn transcribe_audio(audio_base64: &str, _model_id: Option<String>) -> Result<String, String> {
    let _ = audio_base64;
    let dir = get_whisper_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    Err(format!(
        "Local STT (Whisper) not built in. Rebuild with: cargo build --features voice. \
         Requires libclang and cmake. Place ggml-base.en.bin in: {}",
        dir.display()
    ))
}

#[cfg(feature = "voice")]
fn get_or_load_context(model_id: &str) -> Result<std::sync::Arc<WhisperContext>, String> {
    use std::sync::OnceLock;
    static WHISPER_CTX: OnceLock<std::sync::Arc<WhisperContext>> = OnceLock::new();

    if let Some(ctx) = WHISPER_CTX.get() {
        return Ok(std::sync::Arc::clone(ctx));
    }

    let whisper_dir = get_whisper_dir()?;
    let model_path = whisper_dir.join(format!("ggml-{}.bin", model_id));

    if !model_path.exists() {
        return Err(format!(
            "Whisper model '{}' not found. Place ggml-{}.bin in: {}",
            model_id,
            model_id,
            whisper_dir.display()
        ));
    }

    let ctx = WhisperContext::new_with_params(
        model_path.to_str().unwrap(),
        WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Failed to load Whisper model: {}", e))?;

    let arc = std::sync::Arc::new(ctx);
    let _ = WHISPER_CTX.set(std::sync::Arc::clone(&arc));
    Ok(arc)
}

#[cfg(feature = "voice")]
fn decode_wav_to_pcm(audio_base64: &str) -> Result<Vec<f32>, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(audio_base64)
        .map_err(|e| format!("Invalid base64 audio: {}", e))?;

    let cursor = std::io::Cursor::new(bytes);
    let mut reader = hound::WavReader::new(cursor)
        .map_err(|e| format!("Failed to read WAV: {}", e))?;

    let spec = reader.spec();
    let samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read WAV samples: {}", e))?;

    let mono: Vec<i16> = if spec.channels == 2 {
        samples.chunks(2).map(|c| c[0]).collect()
    } else {
        samples
    };

    let target_rate = 16000u32;
    let mut mono_f32 = vec![0.0f32; mono.len()];
    whisper_rs::convert_integer_to_float_audio(&mono, &mut mono_f32)
        .map_err(|e| format!("Audio conversion failed: {}", e))?;

    let pcm = if spec.sample_rate != target_rate {
        let ratio = spec.sample_rate as f64 / target_rate as f64;
        let new_len = (mono_f32.len() as f64 / ratio) as usize;
        (0..new_len)
            .map(|i| {
                let src_idx = (i as f64 * ratio) as usize;
                mono_f32.get(src_idx).copied().unwrap_or(0.0)
            })
            .collect()
    } else {
        mono_f32
    };

    Ok(pcm)
}

#[cfg(feature = "voice")]
pub fn transcribe_audio(audio_base64: &str, model_id: Option<String>) -> Result<String, String> {
    let model_id = model_id.unwrap_or_else(|| "base.en".to_string());

    let pcm = decode_wav_to_pcm(audio_base64)?;
    if pcm.is_empty() {
        return Ok(String::new());
    }

    let ctx = get_or_load_context(&model_id)?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_translate(false);
    params.set_language(Some("en"));

    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create Whisper state: {}", e))?;

    state
        .full(params, &pcm)
        .map_err(|e| format!("Whisper transcription failed: {}", e))?;

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    for i in 0..num_segments {
        if let Some(segment) = state.get_segment(i) {
            if let Ok(s) = segment.to_str() {
                text.push_str(s);
            }
        }
    }

    Ok(text.trim().to_string())
}
