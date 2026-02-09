# Voice (STT/TTS) Setup

Panther supports full voice conversations: speak to the AI and hear spoken replies. You can use **browser APIs** (no setup) or **local models** (Whisper for STT, Piper for TTS) for privacy and offline use.

## Quick Start (Browser Mode)

- Enable voice: **View → Enable Voice (TTS/STT)**
- Use browser APIs: **View → Use Browser Voice** (default)
- No installation required. Works in Chrome/Edge (SpeechRecognition + speechSynthesis).

## Local Voice (Whisper STT)

For local, private speech-to-text:

### Prerequisites

1. **LLVM/Clang** (provides libclang)
   - **Windows**: Install [LLVM](https://github.com/llvm/llvm-project/releases) or [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++"
   - **macOS**: `xcode-select --install` or `brew install llvm`
   - **Linux**: `sudo apt install clang libclang-dev` (Debian/Ubuntu) or equivalent

2. **CMake**
   - **Windows**: [cmake.org](https://cmake.org/download/) or `choco install cmake`
   - **macOS**: `brew install cmake`
   - **Linux**: `sudo apt install cmake`

3. **Whisper model**
   - Download `ggml-base.en.bin` from [whisper.cpp](https://github.com/ggml-org/whisper.cpp) or [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp)
   - Place in **project root** (preferred for dev): `voice/whisper/` at the project root (same level as `package.json`)
   - Or next to the executable: `voice/whisper/` in `target/debug/` (dev) or the app install folder (production)
   - Or in user data: `%APPDATA%\panther\voice\whisper\` (Windows) or `~/.local/share/panther/voice/whisper/` (Linux/macOS)

### Build with Voice

```bash
cargo build --features voice
# or for dev
npm run tauri dev -- --features voice
```

### Model Paths

The app checks these locations (in order):

1. **Project root** (preferred for development): `voice/whisper/` at the project root (same level as `package.json`)
2. **App directory**: `voice/whisper/` next to the executable  
   - Dev: `target/debug/voice/whisper/` or `target/release/voice/whisper/`  
   - Production: same folder as the installed `.exe` / app
3. **User data** (fallback):  
   - Windows: `%APPDATA%\panther\voice\whisper\`  
   - Linux/macOS: `~/.local/share/panther/voice/whisper/`

Place `ggml-base.en.bin` (or `ggml-small.en.bin`, etc.) in one of those folders.

## Local TTS (Piper)

Piper TTS is **not yet built in** due to heavy dependencies (espeak-ng, ONNX runtime). The app falls back to browser speechSynthesis. To enable Piper in the future:

- **Piper models**: [Hugging Face piper-voices](https://huggingface.co/rhasspy/piper-voices)
- **Path**: `%APPDATA%\panther\voice\piper\<voice_id>\` with `<voice_id>.onnx` and `<voice_id>.onnx.json`

## Repositories & References

- [whisper.cpp](https://github.com/ggml-org/whisper.cpp) – Whisper inference
- [whisper-rs](https://crates.io/crates/whisper-rs) – Rust bindings for whisper.cpp
- [Piper TTS](https://github.com/rhasspy/piper) – Neural TTS
- [piper-rs](https://crates.io/crates/piper-rs) – Rust Piper bindings (optional, not in default build)

## Troubleshooting

**"Local STT (Whisper) not built in"**
- Rebuild with `cargo build --features voice`
- Ensure libclang and cmake are installed and on PATH

**"Unable to find libclang"**
- Set `LIBCLANG_PATH` to your LLVM `bin` directory (e.g. `C:\Program Files\LLVM\bin`)
- Or install Visual Studio Build Tools with C++ workload

**"Whisper model not found"**
- Download `ggml-base.en.bin` and place it in the whisper folder (see paths above)
