/**
 * Record audio from microphone and return as base64-encoded WAV (16kHz mono).
 * Used for local Whisper STT.
 * Uses AudioWorklet when available to avoid ScriptProcessorNode deprecation.
 */
export interface RecordingSession {
  stop: () => Promise<string>;
}

export interface SilenceDetectionOptions {
  /** Silence duration (ms) before auto-stop */
  silenceThresholdMs?: number;
  /** Minimum recording time (ms) before silence can trigger */
  minRecordingMs?: number;
  /** Volume threshold (0-255, lower = more sensitive) */
  silenceLevel?: number;
  /** Called when silence detected - call stop() to finalize */
  onSilence: () => void;
}

const WORKLET_CODE = `
class RecorderProcessor extends AudioWorkletProcessor {
  process(inputs, outputs, parameters) {
    const input = inputs[0]?.[0];
    if (input && input.length > 0) {
      this.port.postMessage(input.slice(0));
    }
    return true;
  }
}
registerProcessor('recorder-processor', RecorderProcessor);
`;

async function loadWorklet(ctx: AudioContext): Promise<void> {
  const blob = new Blob([WORKLET_CODE], { type: 'application/javascript' });
  const url = URL.createObjectURL(blob);
  try {
    await ctx.audioWorklet.addModule(url);
  } finally {
    URL.revokeObjectURL(url);
  }
}

export async function startRecording(): Promise<RecordingSession> {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const ctx = new AudioContext({ sampleRate: 16000 });
  const source = ctx.createMediaStreamSource(stream);
  const samples: Float32Array[] = [];

  try {
    await loadWorklet(ctx);
    const worklet = new AudioWorkletNode(ctx, 'recorder-processor', { numberOfInputs: 1, numberOfOutputs: 1 });
    worklet.port.onmessage = (e: MessageEvent<Float32Array>) => {
      samples.push(new Float32Array(e.data));
    };
    source.connect(worklet);
    // Do not connect to destination - we only capture, no playback

    return {
      stop: async () => {
        worklet.disconnect();
        source.disconnect();
        stream.getTracks().forEach((t) => t.stop());
        await ctx.close();
        return encodeWav(samples, 16000);
      },
    };
  } catch {
    // Fallback to ScriptProcessorNode (deprecated but widely supported)
    return startRecordingScriptProcessor(stream, ctx, source, samples);
  }
}

/** Fallback when AudioWorklet is not available (e.g. Safari) */
function startRecordingScriptProcessor(
  stream: MediaStream,
  ctx: AudioContext,
  source: MediaStreamAudioSourceNode,
  samples: Float32Array[]
): RecordingSession {
  const bufferSize = 4096;
  const processor = ctx.createScriptProcessor(bufferSize, 1, 1);
  processor.onaudioprocess = (e: AudioProcessingEvent) => {
    const input = e.inputBuffer.getChannelData(0);
    if (input.length > 0) samples.push(new Float32Array(input));
  };
  const silentGain = ctx.createGain();
  silentGain.gain.value = 0;
  silentGain.connect(ctx.destination);
  source.connect(processor);
  processor.connect(silentGain); // Must connect to complete graph; silent gain avoids playback
  return {
    stop: async () => {
      processor.disconnect();
      source.disconnect();
      stream.getTracks().forEach((t) => t.stop());
      await ctx.close();
      return encodeWav(samples, 16000);
    },
  };
}

/**
 * Start recording with silence detection. When silence is detected for
 * silenceThresholdMs (after minRecordingMs), onSilence is called.
 * Call session.stop() from onSilence to finalize and get the WAV.
 */
export async function startRecordingWithSilenceDetection(
  options: SilenceDetectionOptions
): Promise<RecordingSession> {
  const {
    silenceThresholdMs = 1500,
    minRecordingMs = 500,
    silenceLevel = 15,
    onSilence,
  } = options;

  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const ctx = new AudioContext({ sampleRate: 16000 });
  const source = ctx.createMediaStreamSource(stream);
  const samples: Float32Array[] = [];

  let silenceStart: number | null = null;
  let hasSpeech = false;
  let intervalId: ReturnType<typeof setInterval> | null = null;

  const analyser = ctx.createAnalyser();
  analyser.fftSize = 256;
  analyser.smoothingTimeConstant = 0.5;
  source.connect(analyser);

  try {
    await loadWorklet(ctx);
    const worklet = new AudioWorkletNode(ctx, 'recorder-processor', { numberOfInputs: 1, numberOfOutputs: 1 });
    worklet.port.onmessage = (e: MessageEvent<Float32Array>) => {
      samples.push(new Float32Array(e.data));
    };
    source.connect(worklet);
    // Do not connect worklet to destination - we only capture

    const data = new Uint8Array(analyser.frequencyBinCount);
    const startTime = Date.now();

    intervalId = setInterval(() => {
      analyser.getByteFrequencyData(data);
      const avg = data.reduce((a, b) => a + b, 0) / data.length;
      const elapsed = Date.now() - startTime;

      if (elapsed < minRecordingMs) return;

      if (avg > silenceLevel) {
        hasSpeech = true;
        silenceStart = null;
      } else if (hasSpeech) {
        if (silenceStart === null) silenceStart = Date.now();
        else if (Date.now() - silenceStart >= silenceThresholdMs) {
          if (intervalId) clearInterval(intervalId);
          intervalId = null;
          onSilence();
        }
      }
    }, 100);

    return {
      stop: async () => {
        if (intervalId) clearInterval(intervalId);
        worklet.disconnect();
        source.disconnect();
        stream.getTracks().forEach((t) => t.stop());
        await ctx.close();
        return encodeWav(samples, 16000);
      },
    };
  } catch {
    if (intervalId) clearInterval(intervalId);
    // Fallback: use ScriptProcessor for recording, keep analyser for silence detection
    return startRecordingWithSilenceDetectionScriptProcessor(
      stream, ctx, source, samples, analyser,
      { silenceThresholdMs, minRecordingMs, silenceLevel, onSilence }
    );
  }
}

/** Fallback when AudioWorklet is not available */
function startRecordingWithSilenceDetectionScriptProcessor(
  stream: MediaStream,
  ctx: AudioContext,
  source: MediaStreamAudioSourceNode,
  samples: Float32Array[],
  analyser: AnalyserNode,
  options: SilenceDetectionOptions
): RecordingSession {
  const { silenceThresholdMs = 1500, minRecordingMs = 500, silenceLevel = 15, onSilence } = options;
  let silenceStart: number | null = null;
  let hasSpeech = false;
  let intervalId: ReturnType<typeof setInterval> | null = null;

  const bufferSize = 4096;
  const processor = ctx.createScriptProcessor(bufferSize, 1, 1);
  processor.onaudioprocess = (e: AudioProcessingEvent) => {
    const input = e.inputBuffer.getChannelData(0);
    if (input.length > 0) samples.push(new Float32Array(input));
  };
  const silentGain = ctx.createGain();
  silentGain.gain.value = 0;
  silentGain.connect(ctx.destination);
  source.connect(processor);
  processor.connect(silentGain);

  const data = new Uint8Array(analyser.frequencyBinCount);
  const startTime = Date.now();
  intervalId = setInterval(() => {
    analyser.getByteFrequencyData(data);
    const avg = data.reduce((a, b) => a + b, 0) / data.length;
    const elapsed = Date.now() - startTime;
    if (elapsed < minRecordingMs) return;
    if (avg > silenceLevel) {
      hasSpeech = true;
      silenceStart = null;
    } else if (hasSpeech) {
      if (silenceStart === null) silenceStart = Date.now();
      else if (Date.now() - silenceStart >= silenceThresholdMs) {
        if (intervalId) clearInterval(intervalId);
        intervalId = null;
        onSilence();
      }
    }
  }, 100);

  return {
    stop: async () => {
      if (intervalId) clearInterval(intervalId);
      processor.disconnect();
      source.disconnect();
      stream.getTracks().forEach((t) => t.stop());
      await ctx.close();
      return encodeWav(samples, 16000);
    },
  };
}

function encodeWav(chunks: Float32Array[], sampleRate: number): string {
  const totalLen = chunks.reduce((s, c) => s + c.length, 0);
  const buffer = new ArrayBuffer(44 + totalLen * 2);
  const view = new DataView(buffer);
  const writeStr = (offset: number, str: string) => {
    for (let i = 0; i < str.length; i++) view.setUint8(offset + i, str.charCodeAt(i));
  };
  writeStr(0, 'RIFF');
  view.setUint32(4, 36 + totalLen * 2, true);
  writeStr(8, 'WAVE');
  writeStr(12, 'fmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeStr(36, 'data');
  view.setUint32(40, totalLen * 2, true);
  let offset = 44;
  for (const chunk of chunks) {
    for (let i = 0; i < chunk.length; i++) {
      const s = Math.max(-1, Math.min(1, chunk[i]));
      view.setInt16(offset, s < 0 ? s * 0x8000 : s * 0x7fff, true);
      offset += 2;
    }
  }
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}
