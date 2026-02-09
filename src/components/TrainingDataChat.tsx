import { useState, useEffect } from 'react';
import { api } from '../api';
import { useAppStore } from '../store';
import { VoiceInput } from './VoiceInput';
import { VoiceOutput } from './VoiceOutput';

interface TrainingDataChatProps {
  projectId: string;
  trainingDataCount: number;
}

export function TrainingDataChat({ projectId, trainingDataCount }: TrainingDataChatProps) {
  const { profiles, setProfiles } = useAppStore();
  const [query, setQuery] = useState('');
  const [response, setResponse] = useState<string | null>(null);
  const [examplesUsed, setExamplesUsed] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState<string>('');
  const [providerUsed, setProviderUsed] = useState<string>('');
  const [modelUsed, setModelUsed] = useState<string>('');
  const [useLocal, setUseLocal] = useState(false);
  const [localModelName, setLocalModelName] = useState('llama3');
  const [availableLocalModels, setAvailableLocalModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);

  useEffect(() => {
    // Load profiles if not already loaded
    if (profiles.length === 0) {
      api.listProfiles().then(data => {
        setProfiles(data);
        // Auto-select first profile if available and not using local
        if (data.length > 0 && !useLocal) {
          setSelectedProfileId(data[0].id);
        }
      }).catch(err => {
        console.error('Failed to load profiles:', err);
      });
    } else if (profiles.length > 0 && !selectedProfileId && !useLocal) {
      // Auto-select first profile if available
      setSelectedProfileId(profiles[0].id);
    }
  }, [profiles.length, setProfiles, selectedProfileId, useLocal]);

  useEffect(() => {
    // Load available Ollama models when using local mode
    if (useLocal) {
      setLoadingModels(true);
      // Find Ollama provider and get its models
      api.listProviders().then(providers => {
        const ollamaProvider = providers.find((p: any) => p.provider_type === 'ollama');
        if (ollamaProvider) {
          api.listProviderModels(ollamaProvider.id)
            .then(models => {
              setAvailableLocalModels(models || []);
              // Auto-select first model if available
              if (models && models.length > 0 && !models.includes(localModelName)) {
                setLocalModelName(models[0]);
              }
            })
            .catch(err => {
              console.error('Failed to load Ollama models:', err);
              // Default models if API fails
              setAvailableLocalModels(['llama3', 'llama2', 'mistral', 'codellama']);
            })
            .finally(() => setLoadingModels(false));
        } else {
          setAvailableLocalModels(['llama3', 'llama2', 'mistral', 'codellama']); // Default list
          setLoadingModels(false);
        }
      }).catch(err => {
        console.error('Failed to load providers:', err);
        setAvailableLocalModels(['llama3', 'llama2', 'mistral', 'codellama']); // Default list
        setLoadingModels(false);
      });
    }
  }, [useLocal]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim() || !projectId) return;

    setLoading(true);
    setError(null);
    setResponse(null);
    setExamplesUsed([]);

    try {
      const result = await api.chatWithTrainingData({
        project_id: projectId,
        query: query.trim(),
        profile_id: useLocal ? undefined : (selectedProfileId || undefined),
        max_examples: 5,
        use_local: useLocal,
        local_model_name: useLocal ? localModelName : undefined,
      });

      setResponse(result.response);
      setExamplesUsed(result.examples_used || []);
      setProviderUsed(result.provider_used || '');
      setModelUsed(result.model_used || '');
    } catch (err: any) {
      setError(err.toString());
    } finally {
      setLoading(false);
    }
  };

  if (trainingDataCount === 0) {
    return (
      <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)' }}>
        <p>No training data available. Import training data first to use this feature.</p>
      </div>
    );
  }

  return (
    <div>
      <div style={{ marginBottom: '15px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '15px' }}>
          <input
            type="checkbox"
            id="use-local"
            checked={useLocal}
            onChange={(e) => {
              setUseLocal(e.target.checked);
              if (e.target.checked) {
                setSelectedProfileId(''); // Clear profile selection when using local
              }
            }}
            style={{ width: '18px', height: '18px', cursor: 'pointer' }}
          />
          <label htmlFor="use-local" style={{ fontSize: '14px', fontWeight: '500', cursor: 'pointer' }}>
            🏠 Use Local Model (Ollama)
          </label>
        </div>

        {useLocal ? (
          <div>
            <label style={{ display: 'block', marginBottom: '8px', fontSize: '14px', fontWeight: '500' }}>
              Select Local Model:
            </label>
            <select
              value={localModelName}
              onChange={(e) => setLocalModelName(e.target.value)}
              disabled={loading || loadingModels}
              style={{
                width: '100%',
                padding: '10px 15px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                background: 'var(--card-bg)',
                color: 'var(--text-primary)',
                fontSize: '14px',
                marginBottom: '10px',
              }}
            >
              {loadingModels ? (
                <option value="">Loading models...</option>
              ) : availableLocalModels.length === 0 ? (
                <option value="">No models available. Make sure Ollama is running.</option>
              ) : (
                availableLocalModels.map((model) => (
                  <option key={model} value={model}>
                    {model}
                  </option>
                ))
              )}
            </select>
            <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
              💡 Using local Ollama models. Make sure Ollama is running and the model is downloaded.
            </small>
          </div>
        ) : (
          <div>
            <label style={{ display: 'block', marginBottom: '8px', fontSize: '14px', fontWeight: '500' }}>
              Select Provider/Profile:
            </label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              disabled={loading}
              style={{
                width: '100%',
                padding: '10px 15px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                background: 'var(--card-bg)',
                color: 'var(--text-primary)',
                fontSize: '14px',
                marginBottom: '10px',
              }}
            >
              {profiles.length === 0 ? (
                <option value="">No profiles available. Create a profile first.</option>
              ) : (
                <>
                  <option value="">Auto-select (any available provider)</option>
                  {profiles.map((profile) => (
                    <option key={profile.id} value={profile.id}>
                      {profile.name} ({profile.model_name || 'default model'})
                    </option>
                  ))}
                </>
              )}
            </select>
            {profiles.length === 0 && (
              <small style={{ color: '#ff9800', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                ⚠️ No profiles found. Please create a profile in the Profiles section first, or use Local Model option above.
              </small>
            )}
          </div>
        )}
      </div>

      <form onSubmit={handleSubmit} style={{ marginBottom: '15px' }}>
        <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Ask a question about your training data..."
            disabled={loading || (!useLocal && profiles.length === 0)}
            style={{
              flex: 1,
              padding: '10px 15px',
              borderRadius: '4px',
              border: '1px solid var(--border-color)',
              background: 'var(--card-bg)',
              color: 'var(--text-primary)',
              fontSize: '14px',
            }}
          />
          <VoiceInput
            value={query}
            onTranscript={setQuery}
            disabled={loading || (!useLocal && profiles.length === 0)}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={loading || !query.trim() || (!useLocal && profiles.length === 0)}
            style={{ whiteSpace: 'nowrap' }}
          >
            {loading ? '⏳ Asking...' : '💬 Ask'}
          </button>
        </div>
      </form>

      {error && (
        <div style={{
          padding: '12px',
          background: '#fee',
          border: '1px solid #fcc',
          borderRadius: '4px',
          color: '#c33',
          marginBottom: '15px',
          fontSize: '13px',
        }}>
          {error}
        </div>
      )}

      {response && (
        <div style={{ marginBottom: '15px' }}>
          <div style={{
            padding: '15px',
            background: 'var(--bg-secondary)',
            borderRadius: '4px',
            border: '1px solid var(--border-color)',
            marginBottom: '10px',
          }}>
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '8px' }}>
              💡 AI Response (based on {examplesUsed.length} training example{examplesUsed.length !== 1 ? 's' : ''})
              {providerUsed && (
                <span style={{ marginLeft: '10px', color: '#888' }}>
                  • Using: {providerUsed} {modelUsed && `(${modelUsed})`}
                </span>
              )}
            </div>
            <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: '8px' }}>
              <div style={{ flex: 1, whiteSpace: 'pre-wrap', lineHeight: '1.6' }}>
                {response}
              </div>
              <VoiceOutput text={response} />
            </div>
          </div>

          {examplesUsed.length > 0 && (
            <details style={{ fontSize: '12px' }}>
              <summary style={{ cursor: 'pointer', color: 'var(--text-secondary)', marginBottom: '8px' }}>
                📚 View {examplesUsed.length} training example{examplesUsed.length !== 1 ? 's' : ''} used
              </summary>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                {examplesUsed.map((example, idx) => (
                  <div
                    key={idx}
                    style={{
                      padding: '10px',
                      background: 'var(--bg-secondary)',
                      borderRadius: '4px',
                      border: '1px solid var(--border-color)',
                    }}
                  >
                    <div style={{ marginBottom: '5px', color: 'var(--text-secondary)' }}>
                      <strong>Input:</strong> {example.input || example.input_text || 'N/A'}
                    </div>
                    <div style={{ color: 'var(--text-secondary)' }}>
                      <strong>Output:</strong> {example.output || example.output_text || 'N/A'}
                    </div>
                  </div>
                ))}
              </div>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
