import { useState, useCallback, useRef, useEffect } from 'react';
import { useAppStore } from '../store';
import type { VoiceOutputProps, TTSFeatureStatus } from '../voice/types';
import { api } from '../api';

function detectTTS(useLocal: boolean): TTSFeatureStatus {
  if (typeof window === 'undefined') {
    return { supported: false, message: 'Not in browser' };
  }
  if (useLocal) return { supported: true };
  if (!('speechSynthesis' in window)) {
    return { supported: false, message: 'Speech synthesis not supported' };
  }
  return { supported: true };
}

export function VoiceOutput({
  text,
  isPlaying: controlledPlaying,
  onPlayingChange,
  onPlaybackEnd,
  autoPlay = false,
  lang = 'en-US',
  voiceUri,
  className = '',
  style,
}: VoiceOutputProps) {
  const { voiceEnabled, useLocalVoice, voiceUri: storeVoiceUri } = useAppStore();
  const effectiveVoiceUri = voiceUri ?? storeVoiceUri;
  const [status] = useState<TTSFeatureStatus>(() => detectTTS(useLocalVoice));
  const [internalPlaying, setInternalPlaying] = useState(false);
  const utteranceRef = useRef<SpeechSynthesisUtterance | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);

  const isPlaying = controlledPlaying ?? internalPlaying;
  const hasAutoPlayedRef = useRef(false);

  // Auto-play when autoPlay prop is true and text changes
  useEffect(() => {
    if (autoPlay && text.trim() && voiceEnabled && status.supported && !hasAutoPlayedRef.current) {
      hasAutoPlayedRef.current = true;
      // Small delay to ensure component is mounted
      const timer = setTimeout(() => {
        if (useLocalVoice) {
          // Trigger speak for local voice
          api.synthesizeSpeech(text.trim())
            .then((audioBase64) => {
              if (!audioBase64) return;
              const binary = atob(audioBase64);
              const bytes = new Uint8Array(binary.length);
              for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
              const ctx = new AudioContext();
              audioContextRef.current = ctx;
              return ctx.decodeAudioData(bytes.buffer);
            })
            .then((buffer) => {
              if (!buffer || !audioContextRef.current) return;
              const source = audioContextRef.current.createBufferSource();
              source.buffer = buffer;
              source.connect(audioContextRef.current.destination);
              source.onended = () => {
                setInternalPlaying(false);
                onPlayingChange?.(false);
                onPlaybackEnd?.();
                audioContextRef.current = null;
              };
              source.start(0);
              setInternalPlaying(true);
              onPlayingChange?.(true);
            })
            .catch((e) => {
              const msg = e instanceof Error ? e.message : String(e);
              if (msg.includes('not built in')) {
                doBrowserTTSAutoPlay();
              }
            });
        } else {
          doBrowserTTSAutoPlay();
        }
      }, 100);
      return () => clearTimeout(timer);
    }
  }, [autoPlay, text, voiceEnabled, status.supported, useLocalVoice]);

  function doBrowserTTSAutoPlay() {
    if (!('speechSynthesis' in window)) return;
    const utterance = new SpeechSynthesisUtterance(text.trim());
    utterance.lang = lang;
    if (effectiveVoiceUri) {
      const voices = window.speechSynthesis.getVoices();
      const voice = voices.find((v) => v.voiceURI === effectiveVoiceUri || v.name === effectiveVoiceUri);
      if (voice) utterance.voice = voice;
    }
    utterance.onend = () => {
      setInternalPlaying(false);
      onPlayingChange?.(false);
      onPlaybackEnd?.();
      utteranceRef.current = null;
    };
    utterance.onerror = () => {
      setInternalPlaying(false);
      onPlayingChange?.(false);
      onPlaybackEnd?.();
      utteranceRef.current = null;
    };
    utteranceRef.current = utterance;
    window.speechSynthesis.speak(utterance);
    setInternalPlaying(true);
    onPlayingChange?.(true);
  }

  const stop = useCallback(() => {
    if (typeof window !== 'undefined' && window.speechSynthesis) {
      window.speechSynthesis.cancel();
    }
    if (audioContextRef.current?.state === 'running') {
      audioContextRef.current.suspend();
    }
    setInternalPlaying(false);
    onPlayingChange?.(false);
    utteranceRef.current = null;
  }, [onPlayingChange]);

  const speak = useCallback(async () => {
    if (!status.supported || !voiceEnabled || !text.trim()) return;

    if (isPlaying) {
      stop();
      return;
    }

    if (useLocalVoice) {
      try {
        const audioBase64 = await api.synthesizeSpeech(text.trim());
        if (!audioBase64) return;
        const binary = atob(audioBase64);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        const ctx = new AudioContext();
        audioContextRef.current = ctx;
        const buffer = await ctx.decodeAudioData(bytes.buffer);
        const source = ctx.createBufferSource();
        source.buffer = buffer;
        source.connect(ctx.destination);
        source.onended = () => {
          setInternalPlaying(false);
          onPlayingChange?.(false);
          onPlaybackEnd?.();
          audioContextRef.current = null;
        };
        source.start(0);
        setInternalPlaying(true);
        onPlayingChange?.(true);
      } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes('not built in')) {
          doBrowserTTS();
        } else {
          console.warn('Local TTS failed:', e);
          alert(msg);
        }
      }
      return;
    }

    doBrowserTTS();
  }, [status.supported, voiceEnabled, useLocalVoice, text, lang, voiceUri, isPlaying, stop, onPlayingChange]);

  function doBrowserTTS() {
    if (!('speechSynthesis' in window)) return;
    const utterance = new SpeechSynthesisUtterance(text.trim());
    utterance.lang = lang;
    if (effectiveVoiceUri) {
      const voices = window.speechSynthesis.getVoices();
      const voice = voices.find((v) => v.voiceURI === effectiveVoiceUri || v.name === effectiveVoiceUri);
      if (voice) utterance.voice = voice;
    }
    utterance.onend = () => {
      setInternalPlaying(false);
      onPlayingChange?.(false);
      onPlaybackEnd?.();
      utteranceRef.current = null;
    };
    utterance.onerror = () => {
      setInternalPlaying(false);
      onPlayingChange?.(false);
      onPlaybackEnd?.();
      utteranceRef.current = null;
    };
    utteranceRef.current = utterance;
    window.speechSynthesis.speak(utterance);
    setInternalPlaying(true);
    onPlayingChange?.(true);
  }

  if (!voiceEnabled || !status.supported || !text.trim()) {
    return null;
  }

  return (
    <button
      type="button"
      onClick={speak}
      title={isPlaying ? 'Stop speaking' : 'Listen'}
      aria-label={isPlaying ? 'Stop speaking' : 'Listen'}
      className={className}
      style={{
        padding: '4px 8px',
        background: isPlaying ? '#dc3545' : 'var(--bg-secondary)',
        border: '1px solid var(--border-color)',
        borderRadius: '4px',
        cursor: 'pointer',
        color: isPlaying ? '#fff' : 'var(--text-primary)',
        fontSize: '12px',
        ...style,
      }}
    >
      {isPlaying ? '⏹' : '🔊'}
    </button>
  );
}
