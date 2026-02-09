// Shared prop interfaces for VoiceInput and VoiceOutput components

export interface VoiceInputProps {
  /** Current value of the text input (for appending transcript) */
  value: string;
  /** Callback when transcript is received from STT */
  onTranscript: (text: string) => void;
  /** Whether the input is disabled (e.g. while loading) */
  disabled?: boolean;
  /** Optional language code for recognition (e.g. 'en-US') */
  lang?: string;
  /** Optional placeholder when voice is unavailable */
  placeholder?: string;
  /** Optional className for the button */
  className?: string;
  /** Optional inline styles */
  style?: React.CSSProperties;
}

export interface VoiceOutputProps {
  /** Text content to speak */
  text: string;
  /** Whether TTS is currently playing */
  isPlaying?: boolean;
  /** Callback when play/pause state changes */
  onPlayingChange?: (playing: boolean) => void;
  /** Callback when playback ends (for continuous mode) */
  onPlaybackEnd?: () => void;
  /** Auto-play when text is set (for continuous mode) */
  autoPlay?: boolean;
  /** Optional language code for synthesis (e.g. 'en-US') */
  lang?: string;
  /** Optional voice URI or name */
  voiceUri?: string;
  /** Optional className for the button */
  className?: string;
  /** Optional inline styles */
  style?: React.CSSProperties;
}

/** Result of feature detection for STT */
export interface STTFeatureStatus {
  supported: boolean;
  message?: string;
}

/** Result of feature detection for TTS */
export interface TTSFeatureStatus {
  supported: boolean;
  message?: string;
}
