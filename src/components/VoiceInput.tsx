import { useState, useCallback, useRef, useEffect } from 'react';
import { useAppStore } from '../store';
import type { VoiceInputProps, STTFeatureStatus } from '../voice/types';
import { api } from '../api';
import { startRecording, type RecordingSession } from '../utils/audioRecorder';

// SpeechRecognition is in Chromium; extend Window
interface SpeechRecognitionConstructor {
  new (): {
    continuous: boolean;
    interimResults: boolean;
    lang: string;
    onresult: (e: { results: SpeechRecognitionResultList }) => void;
    onend: () => void;
    onerror: (e: { error: string }) => void;
    start: () => void;
    stop: () => void;
  };
}
declare global {
  interface Window {
    SpeechRecognition?: SpeechRecognitionConstructor;
    webkitSpeechRecognition?: SpeechRecognitionConstructor;
  }
}

function detectSTT(useLocal: boolean): STTFeatureStatus {
  if (typeof window === 'undefined') {
    return { supported: false, message: 'Not in browser' };
  }
  if (useLocal) {
    return { supported: !!navigator.mediaDevices?.getUserMedia, message: 'Microphone required for local STT' };
  }
  const SpeechRecognitionAPI = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SpeechRecognitionAPI) {
    return { supported: false, message: 'Speech recognition not supported in this browser' };
  }
  return { supported: true };
}

export function VoiceInput({
  value,
  onTranscript,
  disabled = false,
  lang = 'en-US',
  className = '',
  style,
}: VoiceInputProps) {
  const { voiceEnabled, useLocalVoice } = useAppStore();
  const [status, setStatus] = useState<STTFeatureStatus>(() => detectSTT(useLocalVoice));
  const [isListening, setIsListening] = useState(false);
  const [isTranscribing, setIsTranscribing] = useState(false);
  const recognitionRef = useRef<{ stop: () => void } | null>(null);
  const recordingRef = useRef<RecordingSession | null>(null);

  // Re-detect when useLocalVoice changes
  useState(() => {
    setStatus(detectSTT(useLocalVoice));
  });

  const startListening = useCallback(async () => {
    if (disabled || !voiceEnabled) return;

    if (useLocalVoice) {
      try {
        const session = await startRecording();
        recordingRef.current = session;
        setIsListening(true);
      } catch (e) {
        console.warn('Failed to start recording:', e);
        alert('Could not access microphone. Check permissions.');
      }
      return;
    }

    if (!status.supported) return;
    const SpeechRecognitionAPI = window.SpeechRecognition || window.webkitSpeechRecognition;
    if (!SpeechRecognitionAPI) return;

    const recognition = new SpeechRecognitionAPI();
    recognition.continuous = false;
    recognition.interimResults = false;
    recognition.lang = lang;

    recognition.onresult = (event: { results: SpeechRecognitionResultList }) => {
      const transcript = Array.from(event.results)
        .map((r) => r[0].transcript)
        .join(' ')
        .trim();
      if (transcript) {
        const newText = value ? `${value} ${transcript}` : transcript;
        onTranscript(newText);
      }
    };

    recognition.onend = () => {
      setIsListening(false);
      recognitionRef.current = null;
    };

    recognition.onerror = (event: { error: string }) => {
      console.warn('Speech recognition error:', event.error);
      setIsListening(false);
      recognitionRef.current = null;
    };

    try {
      recognition.start();
      recognitionRef.current = recognition;
      setIsListening(true);
    } catch (e) {
      console.warn('Failed to start speech recognition:', e);
      setIsListening(false);
    }
  }, [status.supported, disabled, voiceEnabled, useLocalVoice, lang, value, onTranscript]);

  const stopListening = useCallback(async () => {
    if (useLocalVoice && recordingRef.current) {
      setIsTranscribing(true);
      try {
        const audioBase64 = await recordingRef.current.stop();
        recordingRef.current = null;
        setIsListening(false);
        if (audioBase64) {
          const { text } = await api.transcribeAudio(audioBase64);
          if (text) {
            const newText = value ? `${value} ${text}` : text;
            onTranscript(newText);
          }
        }
      } catch (e) {
        console.warn('Transcription failed:', e);
        const msg = e instanceof Error ? e.message : String(e);
        if (!msg.includes('not built in')) alert(msg);
      } finally {
        setIsTranscribing(false);
      }
      return;
    }

    if (recognitionRef.current) {
      try {
        recognitionRef.current.stop();
      } catch {
        // ignore
      }
      recognitionRef.current = null;
      setIsListening(false);
    }
  }, [useLocalVoice, value, onTranscript]);

  const toggle = useCallback(() => {
    if (isListening) {
      stopListening();
    } else {
      startListening();
    }
  }, [isListening, startListening, stopListening]);

  const supported = useLocalVoice ? status.supported : status.supported;
  if (!voiceEnabled || !supported) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={toggle}
      disabled={disabled || isTranscribing}
      title={isListening ? 'Stop listening' : 'Start voice input'}
      aria-label={isListening ? 'Stop listening' : 'Start voice input'}
      className={className}
      style={{
        padding: '6px 10px',
        background: isListening ? '#dc3545' : 'var(--bg-secondary)',
        border: '1px solid var(--border-color)',
        borderRadius: '4px',
        cursor: disabled ? 'not-allowed' : 'pointer',
        color: isListening ? '#fff' : 'var(--text-primary)',
        opacity: disabled ? 0.5 : 1,
        ...style,
      }}
    >
      {isListening ? '⏹' : '🎤'}
    </button>
  );
}
