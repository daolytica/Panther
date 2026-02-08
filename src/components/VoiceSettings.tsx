import { useState, useEffect } from 'react';

export type VoiceGender = 'any' | 'male' | 'female' | 'neutral';

export interface VoiceSettingsProps {
  voiceGender: VoiceGender;
  voiceUri: string;
  onVoiceGenderChange: (g: VoiceGender) => void;
  onVoiceUriChange: (uri: string) => void;
  lang?: string;
}

function isVoiceGenderMatch(voice: SpeechSynthesisVoice, gender: VoiceGender): boolean {
  if (gender === 'any') return true;
  const name = (voice.name || '').toLowerCase();
  const uri = (voice.voiceURI || '').toLowerCase();
  const combined = `${name} ${uri}`;
  if (gender === 'female') {
    return /female|woman|samantha|victoria|karen|moira|fiona|kate|tessa|veena|zira|hazel|susan|emily|alice|lucy|nancy|sarah|claire|amy|joanna|ivy|kendra|kimberly|salli|nicole|raveena|aditi|carmen|conchita|penelope|maja|zuzana|nastja|laura|chiara|alice|federica|paulina|ines|carmen|tessa|moira|karen|fiona|samantha|victoria|kate|tessa/.test(combined);
  }
  if (gender === 'male') {
    return /male|man|david|alex|daniel|fred|ralph|bruce|eddy|george|patrick|rishi|joey|justin|matthew|guy|hiram|oskar|adam|nicolas|lekar|filip|luca|ruben|loek|abram|henrik|pablo|raúl|enrique|eduardo|ricardo|diego|giovanni|cosimo|kyoko|otoya|takumi|sin-ji|hiuga|daichi|yuma|tenzin|nils|arnold|william/.test(combined);
  }
  if (gender === 'neutral') {
    return /neutral|default|google|microsoft/.test(combined) && !isVoiceGenderMatch(voice, 'male') && !isVoiceGenderMatch(voice, 'female');
  }
  return true;
}

export function VoiceSettings({
  voiceGender,
  voiceUri,
  onVoiceGenderChange,
  onVoiceUriChange,
  lang = 'en-US',
}: VoiceSettingsProps) {
  const [voices, setVoices] = useState<SpeechSynthesisVoice[]>([]);
  const [voicesLoaded, setVoicesLoaded] = useState(false);

  useEffect(() => {
    const loadVoices = () => {
      const list = typeof window !== 'undefined' && window.speechSynthesis
        ? window.speechSynthesis.getVoices()
        : [];
      setVoices(list.filter((v) => !!v.name && !!v.voiceURI));
      setVoicesLoaded(true);
    };

    loadVoices();
    if (typeof window !== 'undefined' && window.speechSynthesis) {
      window.speechSynthesis.onvoiceschanged = loadVoices;
      return () => {
        window.speechSynthesis.onvoiceschanged = null;
      };
    }
  }, []);

  const filteredVoices = voices.filter((v) => {
    const langMatch = !lang || v.lang.startsWith(lang.split('-')[0]);
    const genderMatch = isVoiceGenderMatch(v, voiceGender);
    return langMatch && genderMatch;
  });

  const allVoicesForSelect = voiceGender === 'any'
    ? voices.filter((v) => !lang || v.lang.startsWith(lang.split('-')[0]))
    : filteredVoices;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
      <div>
        <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px' }}>Voice gender</label>
        <select
          value={voiceGender}
          onChange={(e) => onVoiceGenderChange(e.target.value as VoiceGender)}
          style={{
            width: '100%',
            padding: '8px',
            borderRadius: '4px',
            border: '1px solid var(--border-color)',
            background: 'var(--bg-primary)',
            color: 'var(--text-primary)',
          }}
        >
          <option value="any">Any</option>
          <option value="female">Female</option>
          <option value="male">Male</option>
          <option value="neutral">Neutral</option>
        </select>
      </div>
      <div>
        <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px' }}>Voice (vocals)</label>
        <select
          value={voiceUri || ''}
          onChange={(e) => onVoiceUriChange(e.target.value || '')}
          style={{
            width: '100%',
            padding: '8px',
            borderRadius: '4px',
            border: '1px solid var(--border-color)',
            background: 'var(--bg-primary)',
            color: 'var(--text-primary)',
          }}
        >
          <option value="">Default (system)</option>
          {voicesLoaded && allVoicesForSelect.length === 0 && (
            <option value="" disabled>No voices match. Try &quot;Any&quot; gender.</option>
          )}
          {allVoicesForSelect.map((v) => (
            <option key={v.voiceURI} value={v.voiceURI}>
              {v.name} ({v.lang})
            </option>
          ))}
        </select>
        <p style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '4px' }}>
          Used for TTS when you listen to this profile&apos;s responses. Requires browser support.
        </p>
      </div>
    </div>
  );
}
