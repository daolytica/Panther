import { useState, useEffect } from 'react';
import { useAppStore } from '../store';
import { api } from '../api';
import type { CharacterDefinition, ModelFeatures, GenerationParams } from '../types';
import { VoiceSettings, type VoiceGender } from '../components/VoiceSettings';

interface ProfileEditorProps {
  profileId?: string;
  onClose: () => void;
  onSave: () => void;
}

export function ProfileEditor({ profileId, onClose, onSave }: ProfileEditorProps) {
  const { providers, profiles, setProviders } = useAppStore();
  const [formData, setFormData] = useState({
    name: '',
    provider_account_id: '',
    model_name: '',
    persona_prompt: '',
    character_definition: null as CharacterDefinition | null,
    model_features: null as ModelFeatures | null,
    params: {
      temperature: 0.7,
      top_p: 1.0,
      max_tokens: 2000,
    } as GenerationParams,
    voice_gender: 'any' as VoiceGender,
    voice_uri: '',
  });

  const [showCharacterEditor, setShowCharacterEditor] = useState(false);
  const [showFeaturesEditor, setShowFeaturesEditor] = useState(false);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const [loadModelsError, setLoadModelsError] = useState<string>('');
  const [modelInputMode, setModelInputMode] = useState<'select' | 'manual'>('select');
  const [profileUrls, setProfileUrls] = useState<string>('');
  const [personName, setPersonName] = useState<string>('');
  const [loadingCharacter, setLoadingCharacter] = useState(false);
  const [photoUrl, setPhotoUrl] = useState<string>('');
  const [abortController, setAbortController] = useState<AbortController | null>(null);
  const [cancellationToken, setCancellationToken] = useState<string | null>(null);
  const [useLocalModel, setUseLocalModel] = useState(false);
  const [availableLocalModels, setAvailableLocalModels] = useState<string[]>([]);
  const [trainedOllamaModels, setTrainedOllamaModels] = useState<string[]>([]);
  const [localModelName, setLocalModelName] = useState<string>('');
  const [loadingLocalModels, setLoadingLocalModels] = useState(false);

  useEffect(() => {
    // Load providers if not already loaded
    if (providers.length === 0) {
      api.listProviders().then(data => {
        setProviders(data);
      }).catch(err => {
        console.error('Failed to load providers:', err);
      });
    }
  }, [providers.length, setProviders]);

  useEffect(() => {
    if (profileId) {
      // Load existing profile from store
      const profile = profiles.find(p => p.id === profileId);
      if (profile) {
        // Check if this profile uses a local model (Ollama provider)
        const provider = providers.find(p => p.id === profile.provider_account_id);
        const isLocalModel = provider?.provider_type === 'ollama';
        
        setFormData({
          name: profile.name || '',
          provider_account_id: profile.provider_account_id || '',
          model_name: profile.model_name || '',
          persona_prompt: profile.persona_prompt || '',
          character_definition: profile.character_definition || null,
          model_features: profile.model_features || null,
          params: profile.params_json || {
            temperature: 0.7,
            top_p: 1.0,
            max_tokens: 2000,
          },
          voice_gender: (profile.voice_gender as VoiceGender) || 'any',
          voice_uri: profile.voice_uri || '',
        });
        setPhotoUrl(profile.photo_url || '');
        
        // Set local model state if using Ollama
        if (isLocalModel) {
          setUseLocalModel(true);
          setLocalModelName(profile.model_name || '');
        } else {
          setUseLocalModel(false);
          setLocalModelName('');
        }
      }
    } else {
      // Reset form for new profile
      setFormData({
        name: '',
        provider_account_id: '',
        model_name: '',
        persona_prompt: '',
        character_definition: null,
        model_features: null,
        params: {
          temperature: 0.7,
          top_p: 1.0,
          max_tokens: 2000,
        },
        voice_gender: 'any',
        voice_uri: '',
      });
      setUseLocalModel(false);
      setLocalModelName('');
    }
  }, [profileId, profiles, providers]);

  useEffect(() => {
    // When provider changes, fetch available models
    if (formData.provider_account_id && !useLocalModel) {
      loadAvailableModels();
    } else {
      setAvailableModels([]);
      setLoadModelsError('');
    }
  }, [formData.provider_account_id, useLocalModel]);

  useEffect(() => {
    // Load available Ollama models when using local mode
    if (useLocalModel) {
      setLoadingLocalModels(true);
      
      // First, try to find Ollama provider in store
      let ollamaProvider = providers.find((p: any) => p.provider_type === 'ollama');
      
      // If not found, fetch from API
      const findOllamaProvider = async () => {
        if (!ollamaProvider) {
          try {
            const allProviders = await api.listProviders();
            ollamaProvider = allProviders.find((p: any) => p.provider_type === 'ollama');
            // Update store with fresh providers
            if (allProviders.length > 0) {
              setProviders(allProviders);
            }
          } catch (err) {
            console.error('Failed to fetch providers:', err);
          }
        }
        
        if (ollamaProvider) {
          // Auto-select Ollama provider
          setFormData(prev => ({ 
            ...prev, 
            provider_account_id: ollamaProvider!.id,
            model_name: localModelName || prev.model_name || ''
          }));
          
          Promise.all([
            api.listProviderModels(ollamaProvider.id),
            (typeof api.listTrainedOllamaModels === 'function'
              ? api.listTrainedOllamaModels().catch(() => [] as string[])
              : Promise.resolve([] as string[])),
          ]).then(([models, trained]) => {
            setAvailableLocalModels(models || []);
            setTrainedOllamaModels(Array.isArray(trained) ? trained : []);
              // Auto-select first model if available and none selected
              if (models && models.length > 0) {
                const currentModel = localModelName || formData.model_name;
                if (!currentModel || !models.includes(currentModel)) {
                  const firstModel = models[0];
                  setLocalModelName(firstModel);
                  setFormData(prev => ({ 
                    ...prev, 
                    provider_account_id: ollamaProvider!.id,
                    model_name: firstModel
                  }));
                } else {
                  // Ensure form data is synced with current model
                  setFormData(prev => ({ 
                    ...prev, 
                    provider_account_id: ollamaProvider!.id,
                    model_name: currentModel
                  }));
                }
              }
            })
            .catch(err => {
              console.error('Failed to load Ollama models:', err);
              // Don't set default models - allow manual entry instead
              setAvailableLocalModels([]);
            })
            .finally(() => setLoadingLocalModels(false));
        } else {
          // No Ollama provider found - allow manual entry
          setAvailableLocalModels([]);
          setLoadingLocalModels(false);
        }
      };
      
      findOllamaProvider();
    } else {
      // Reset local model state when disabled
      setAvailableLocalModels([]);
      if (!profileId) {
        // Only clear local model name when creating new profile
        setLocalModelName('');
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [useLocalModel, providers.length]);

  const loadAvailableModels = async () => {
    if (!formData.provider_account_id) return;
    
    setLoadingModels(true);
    setLoadModelsError('');
    try {
      const models = await api.listProviderModels(formData.provider_account_id);
      setAvailableModels(models || []);
      if (models && models.length > 0) {
        setModelInputMode('select');
        const sel = providers.find(p => p.id === formData.provider_account_id);
        const isHybrid = sel?.provider_type === 'hybrid';
        const providerPrimaryModel = isHybrid && sel ? (sel.provider_metadata_json as any)?.primary_model : null;
        const primaryProvider = isHybrid && sel
          ? providers.find(p => p.id === (sel.provider_metadata_json as any)?.primary_provider_id)
          : sel;
        const baseUrl = (primaryProvider?.base_url || '').toLowerCase();
        const isOpenRouter = baseUrl.includes('openrouter');
        // Use provider's cloud model if set and valid; else prefer openai/gpt-4o-mini for OpenRouter; else first model
        let suggested = models[0];
        if (providerPrimaryModel && models.includes(providerPrimaryModel)) {
          suggested = providerPrimaryModel;
        } else if (isOpenRouter && models.includes('openai/gpt-4o-mini')) {
          suggested = 'openai/gpt-4o-mini';
        }
        setFormData(prev => ({
          ...prev,
          model_name: !prev.model_name || !models.includes(prev.model_name) ? suggested : prev.model_name,
        }));
      }
    } catch (error) {
      console.error('Failed to load models:', error);
      setLoadModelsError(String((error as any)?.message || error));
      setModelInputMode('manual');
      setAvailableModels([]);
    } finally {
      setLoadingModels(false);
    }
  };

  const handleSave = async () => {
    try {
      // Validate required fields
      if (!formData.name.trim()) {
        alert('Please enter a profile name');
        return;
      }

      let finalProviderId = formData.provider_account_id;
      let finalModelName = formData.model_name;

      const selectedProvider = providers.find((p: any) => p.id === formData.provider_account_id);
      const isHybridProvider = selectedProvider?.provider_type === 'hybrid';
      const hybridPrimaryModel = (selectedProvider?.provider_metadata_json as any)?.primary_model;

      // If using local model, ensure we have the provider and model set
      if (useLocalModel) {
        // First, try to find Ollama provider in store
        let ollamaProvider = providers.find((p: any) => p.provider_type === 'ollama');
        
        // If not found in store, fetch providers from API
        if (!ollamaProvider) {
          try {
            const allProviders = await api.listProviders();
            console.log('All providers from API:', allProviders);
            ollamaProvider = allProviders.find((p: any) => p.provider_type === 'ollama');
            console.log('Found Ollama provider:', ollamaProvider);
            // Update store with fresh providers
            if (allProviders.length > 0) {
              setProviders(allProviders);
            }
          } catch (err) {
            console.error('Failed to fetch providers:', err);
          }
        } else {
          console.log('Found Ollama provider in store:', ollamaProvider);
        }
        
        if (!ollamaProvider) {
          alert('No Ollama provider found. Please create an Ollama provider in the Providers section first.');
          return;
        }
        
        if (!localModelName || localModelName.trim() === '') {
          alert('Please select or enter a local model name');
          return;
        }
        
        // Validate that the provider ID is valid (non-empty string)
        if (!ollamaProvider.id || ollamaProvider.id.trim() === '') {
          alert('Invalid Ollama provider. Please recreate the provider in the Providers section.');
          return;
        }
        
        // Verify provider exists in database before using it
        try {
          const allProviders = await api.listProviders();
          const providerExists = allProviders.some((p: any) => p.id === ollamaProvider.id);
          if (!providerExists) {
            alert(`The Ollama provider (ID: ${ollamaProvider.id}) no longer exists in the database. Please recreate it in the Providers section.`);
            return;
          }
          console.log('Provider verified in database:', ollamaProvider.id);
        } catch (err) {
          console.error('Failed to verify provider:', err);
          // Continue anyway, let backend handle the error
        }
        
        finalProviderId = ollamaProvider.id;
        finalModelName = localModelName.trim();
        console.log('Using provider ID:', finalProviderId, 'Model:', finalModelName);
      } else {
        if (!formData.provider_account_id) {
          alert('Please select a provider');
          return;
        }
        // For hybrid providers, models are configured in the provider - use primary_model from metadata
        if (isHybridProvider) {
          finalModelName = (hybridPrimaryModel && String(hybridPrimaryModel).trim()) || 'hybrid';
        } else if (!formData.model_name.trim()) {
          alert('Please enter or select a model name');
          return;
        }
      }

      if (!formData.persona_prompt.trim()) {
        alert('Please enter a persona prompt');
        return;
      }

      // Final verification - ensure provider exists
      if (!finalProviderId || finalProviderId.trim() === '') {
        alert('Provider ID is missing. Please select a provider.');
        return;
      }

      if (!finalModelName || finalModelName.trim() === '') {
        alert('Model name is missing. Please select or enter a model name.');
        return;
      }

      // Verify provider exists in database one more time
      try {
        const allProviders = await api.listProviders();
        const providerExists = allProviders.some((p: any) => p.id === finalProviderId);
        if (!providerExists) {
          alert(`The selected provider (ID: ${finalProviderId}) does not exist in the database. Please create the provider first in the Providers section.`);
          console.error('Provider not found:', finalProviderId);
          console.log('Available providers:', allProviders.map((p: any) => ({ id: p.id, type: p.provider_type, name: p.display_name })));
          return;
        }
        console.log('Provider verified before save:', finalProviderId);
      } catch (err) {
        console.error('Failed to verify provider:', err);
        // Continue anyway, let the backend handle the error
      }

      const request = {
        name: formData.name.trim(),
        provider_account_id: finalProviderId,
        model_name: finalModelName.trim(),
        persona_prompt: formData.persona_prompt.trim(),
        character_definition_json: formData.character_definition || undefined,
        model_features_json: formData.model_features || undefined,
        params_json: formData.params,
        photo_url: photoUrl || undefined,
        voice_gender: formData.voice_gender !== 'any' ? formData.voice_gender : undefined,
        voice_uri: formData.voice_uri || undefined,
      };

      console.log('Saving profile with request:', { 
        name: request.name,
        provider_account_id: request.provider_account_id,
        model_name: request.model_name,
        persona_prompt_length: request.persona_prompt.length
      });

      if (profileId) {
        await api.updateProfile(profileId, request);
      } else {
        await api.createProfile(request);
      }
      onSave();
      onClose();
    } catch (error: any) {
      console.error('Failed to save profile:', error);
      const errorMessage = error?.message || error || 'Unknown error occurred';
      
      // Provide more helpful error messages
      if (errorMessage.includes('FOREIGN KEY')) {
        alert(`Failed to save profile: The selected provider doesn't exist in the database. Please create the provider first in the Providers section, or select a different provider.`);
      } else {
        alert(`Failed to save profile: ${errorMessage}`);
      }
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
      }}
    >
      <div
        className="card"
        style={{ width: '800px', maxWidth: '90%', maxHeight: '90vh', overflowY: 'auto' }}
        onClick={(e) => e.stopPropagation()}
      >
        <h2>{profileId ? 'Edit Profile' : 'Create Profile'}</h2>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '20px' }}>
          Create an agent profile with a specific model. You can create multiple profiles using the same provider/API key, each with different models and personas.
        </p>

        <div className="form-group">
          <label>Profile Name</label>
          <input
            type="text"
            value={formData.name}
            onChange={(e) => setFormData({ ...formData, name: e.target.value })}
            placeholder="e.g., Architect, Critic, PM"
            required
          />
        </div>

        <div className="form-group">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <label>Provider</label>
            <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer', fontSize: '14px', userSelect: 'none' }}>
              <input
                type="checkbox"
                checked={useLocalModel}
                onChange={(e) => {
                  console.log('Checkbox changed:', e.target.checked);
                  setUseLocalModel(e.target.checked);
                  if (!e.target.checked) {
                    // Reset form when disabling local model
                    setFormData({ 
                      ...formData, 
                      provider_account_id: '',
                      model_name: ''
                    });
                  }
                }}
                style={{ cursor: 'pointer' }}
              />
              <span style={{ cursor: 'pointer' }}>🏠 Use Local Model (Ollama)</span>
            </label>
          </div>

          {useLocalModel ? (
            <div>
              <div style={{ 
                padding: '12px', 
                background: 'var(--highlight-bg)', 
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                marginBottom: '10px'
              }}>
                <label style={{ fontSize: '13px', fontWeight: 'bold', marginBottom: '8px', display: 'block' }}>
                  Select Local Model:
                </label>
                {availableLocalModels.length > 0 ? (
                  <select
                    value={localModelName}
                    onChange={(e) => {
                      const selectedModel = e.target.value;
                      setLocalModelName(selectedModel);
                      // Find Ollama provider
                      const ollamaProvider = providers.find((p: any) => p.provider_type === 'ollama');
                      if (ollamaProvider) {
                        setFormData(prev => ({ 
                          ...prev, 
                          provider_account_id: ollamaProvider.id,
                          model_name: selectedModel
                        }));
                      }
                    }}
                    required
                    disabled={loadingLocalModels}
                    style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)', cursor: loadingLocalModels ? 'wait' : 'pointer' }}
                  >
                    <option value="">
                      {loadingLocalModels ? 'Loading models...' : 'Select a local model...'}
                    </option>
                    {availableLocalModels.map((model) => (
                      <option key={model} value={model}>
                        {trainedOllamaModels.includes(model) ? `⭐ ${model} (trained)` : model}
                      </option>
                    ))}
                  </select>
                ) : (
                  <div>
                    <input
                      type="text"
                      value={localModelName}
                      onChange={(e) => {
                        const selectedModel = e.target.value;
                        console.log('Local model entered:', selectedModel);
                        setLocalModelName(selectedModel);
                        // Find Ollama provider
                        const ollamaProvider = providers.find((p: any) => p.provider_type === 'ollama');
                        if (ollamaProvider) {
                          setFormData(prev => ({ 
                            ...prev, 
                            provider_account_id: ollamaProvider.id,
                            model_name: selectedModel
                          }));
                        }
                      }}
                      placeholder={loadingLocalModels ? 'Loading models...' : 'Enter model name (e.g., llama3, llama2, mistral)'}
                      required
                      disabled={loadingLocalModels}
                      style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                    />
                    {!loadingLocalModels && (
                      <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginTop: '5px' }}>
                        💡 Enter the model name manually if auto-detection failed (e.g., llama3, llama2, mistral, codellama)
                      </small>
                    )}
                  </div>
                )}
                {loadingLocalModels && (
                  <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                    🔄 Loading available models from Ollama...
                  </small>
                )}
                {!loadingLocalModels && availableLocalModels.length > 0 && (
                  <small style={{ color: '#28a745', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                    ✓ Found {availableLocalModels.length} available model{availableLocalModels.length !== 1 ? 's' : ''}
                  </small>
                )}
                <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginTop: '8px' }}>
                  💡 Using local Ollama models. Make sure Ollama is running and the model is downloaded.
                </small>
              </div>
            </div>
          ) : (
            <>
              <select
                value={formData.provider_account_id}
                onChange={(e) => {
                  setFormData({ 
                    ...formData, 
                    provider_account_id: e.target.value,
                    model_name: '', // Clear model when provider changes
                  });
                }}
                required
              >
                <option value="">Select a provider</option>
                {providers.length === 0 ? (
                  <option value="" disabled>Loading providers...</option>
                ) : (
                  providers.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.display_name} ({p.provider_type})
                    </option>
                  ))
                )}
              </select>
              {providers.length === 0 && (
                <small style={{ color: '#ff9800', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                  ⚠️ No providers found. Please create a provider first in the Providers section.
                </small>
              )}
              {providers.length > 0 && (
                <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                  💡 You can create multiple profiles (agents) using the same provider/API key, each with a different model and persona.
                </small>
              )}
            </>
          )}
        </div>

        {!useLocalModel && (() => {
          const sel = providers.find(p => p.id === formData.provider_account_id);
          const isHybrid = sel?.provider_type === 'hybrid';
          if (isHybrid) {
            return (
              <div className="form-group">
                <div style={{ padding: '12px', background: 'var(--bg-secondary)', borderRadius: '6px', border: '1px solid var(--border-color)' }}>
                  <strong style={{ color: 'var(--text-secondary)', fontSize: '13px' }}>Models configured in provider</strong>
                  <p style={{ margin: '8px 0 0 0', fontSize: '12px', color: 'var(--text-secondary)' }}>
                    This profile uses a hybrid provider. Ollama and cloud models are configured in the provider settings.
                    Edit the provider in <strong>Providers</strong> to change models.
                  </p>
                </div>
              </div>
            );
          }
          return (
          <div className="form-group">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '5px' }}>
              <label>Model Name / Version</label>
              <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                {formData.provider_account_id && (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    style={{ fontSize: '12px', padding: '5px 10px' }}
                    onClick={loadAvailableModels}
                    disabled={loadingModels}
                  >
                    {loadingModels ? 'Loading...' : '🔄 Refresh Models'}
                  </button>
                )}
                {availableModels.length > 0 && (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    style={{ fontSize: '12px', padding: '5px 10px' }}
                    onClick={() => setModelInputMode(modelInputMode === 'select' ? 'manual' : 'select')}
                  >
                    {modelInputMode === 'select' ? '✏️ Manual Entry' : '📋 Select from List'}
                  </button>
                )}
              </div>
            </div>

            {modelInputMode === 'select' && availableModels.length > 0 ? (
              <select
                value={formData.model_name}
                onChange={(e) => setFormData({ ...formData, model_name: e.target.value })}
                required
              >
                <option value="">Select a model...</option>
                {availableModels.map((model) => (
                  <option key={model} value={model}>
                    {model}
                  </option>
                ))}
              </select>
            ) : (
              <div>
                <input
                  type="text"
                  value={formData.model_name}
                  onChange={(e) => setFormData({ ...formData, model_name: e.target.value })}
                  placeholder={
                    loadingModels
                      ? 'Loading available models...'
                      : availableModels.length === 0
                      ? (() => {
                          const sel = providers.find(p => p.id === formData.provider_account_id);
                          const isHybrid = sel?.provider_type === 'hybrid';
                          const primaryProvider = isHybrid && sel
                            ? providers.find(p => p.id === (sel.provider_metadata_json as any)?.primary_provider_id)
                            : sel;
                          const baseUrl = primaryProvider?.base_url || '';
                          const isOpenRouter = baseUrl.toLowerCase().includes('openrouter');
                          return isOpenRouter
                            ? 'e.g., openai/gpt-4o-mini, anthropic/claude-3-5-sonnet'
                            : 'e.g., gpt-4, gpt-4-turbo, gpt-3.5-turbo, claude-3-opus, llama-3-70b';
                        })()
                      : 'Enter model name manually...'
                  }
                  required
                  list="model-suggestions"
                />
                {(availableModels.length > 0 || (() => {
                  const sel = providers.find(p => p.id === formData.provider_account_id);
                  const isHybrid = sel?.provider_type === 'hybrid';
                  const primaryProvider = isHybrid && sel
                    ? providers.find(p => p.id === (sel.provider_metadata_json as any)?.primary_provider_id)
                    : sel;
                  const baseUrl = primaryProvider?.base_url || '';
                  return baseUrl.toLowerCase().includes('openrouter');
                })()) && (
                  <datalist id="model-suggestions">
                    {availableModels.length > 0
                      ? availableModels.map((model) => (
                          <option key={model} value={model} />
                        ))
                      : [
                          'openai/gpt-4o-mini',
                          'openai/gpt-4o',
                          'openai/gpt-4-turbo',
                          'anthropic/claude-3-5-sonnet',
                          'anthropic/claude-3-opus',
                          'google/gemini-1.5-pro',
                          'deepseek/deepseek-v3',
                        ].map((model) => (
                          <option key={model} value={model} />
                        ))}
                  </datalist>
                )}
              </div>
            )}

            {formData.provider_account_id && (() => {
              const sel = providers.find(p => p.id === formData.provider_account_id);
              const isHybrid = sel?.provider_type === 'hybrid';
              const primaryProvider = isHybrid && sel
                ? providers.find(p => p.id === (sel.provider_metadata_json as any)?.primary_provider_id)
                : sel;
              const baseUrl = primaryProvider?.base_url || '';
              const isOpenRouter = baseUrl.toLowerCase().includes('openrouter');
              return (
                <>
                  {isHybrid && (
                    <small style={{ color: 'var(--warning-text)', fontSize: '12px', display: 'block', marginTop: '5px', marginBottom: '4px' }}>
                      ⚠️ Hybrid provider: Use the <strong>cloud</strong> model name here (e.g. gpt-4o, claude-3-5-sonnet). The local model is set in the hybrid provider settings. A 404 error usually means this model name is invalid for your cloud provider.
                    </small>
                  )}
                  {isOpenRouter && (
                    <small style={{ color: 'var(--warning-text)', fontSize: '12px', display: 'block', marginTop: '5px', marginBottom: '4px' }}>
                      📌 OpenRouter: Use <strong>provider/model</strong> format (e.g. openai/gpt-4o-mini, anthropic/claude-3-5-sonnet). Plain model names like gpt-4o-mini will cause 404.
                    </small>
                  )}
                  {availableModels.length === 0 && !loadingModels && (
                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                      {loadModelsError ? (
                        <>⚠️ Could not load models: {loadModelsError}. Enter the model name manually (use provider/model for OpenRouter).</>
                      ) : (
                        <>
                          💡 Tip: Click "Refresh Models" to auto-detect available models, or enter the model name manually.
                          <br />
                          You can create multiple profiles with the same provider but different models.
                        </>
                      )}
                    </small>
                  )}
                </>
              );
            })()}

            {availableModels.length > 0 && (
              <small style={{ color: '#28a745', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                ✓ Found {availableModels.length} available model{availableModels.length !== 1 ? 's' : ''}
              </small>
            )}
          </div>
          );
        })()}

        <div className="form-group">
          <label>Profile Photo (Optional)</label>
          <div style={{ display: 'flex', gap: '15px', alignItems: 'center', marginBottom: '10px' }}>
            {photoUrl && (
              <img 
                src={photoUrl} 
                alt="Profile" 
                style={{ 
                  width: '80px', 
                  height: '80px', 
                  borderRadius: '8px', 
                  objectFit: 'cover',
                  border: '1px solid var(--border-color)'
                }} 
              />
            )}
            <div style={{ flex: 1 }}>
              <input
                type="file"
                accept="image/*"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) {
                    const reader = new FileReader();
                    reader.onload = (event) => {
                      const result = event.target?.result as string;
                      setPhotoUrl(result);
                    };
                    reader.readAsDataURL(file);
                  }
                }}
                style={{ fontSize: '13px' }}
              />
              {photoUrl && (
                <button
                  type="button"
                  onClick={() => setPhotoUrl('')}
                  style={{
                    marginTop: '8px',
                    padding: '4px 12px',
                    fontSize: '12px',
                    background: '#dc3545',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer'
                  }}
                >
                  Remove Photo
                </button>
              )}
            </div>
          </div>
          <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
            Upload a photo for this profile. Supported formats: JPG, PNG, GIF
          </small>
        </div>

        <div className="form-group">
          <label>Persona Prompt</label>
          <textarea
            value={formData.persona_prompt}
            onChange={(e) => setFormData({ ...formData, persona_prompt: e.target.value })}
            placeholder="Enter the role and behavior instructions for this agent..."
            rows={4}
            required
          />
        </div>

        {/* Character Definition Section */}
        <div className="form-group">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <label>Character Definition (Optional)</label>
            <button
              type="button"
              className="btn btn-secondary"
              style={{ fontSize: '12px', padding: '5px 10px' }}
              onClick={() => setShowCharacterEditor(!showCharacterEditor)}
            >
              {showCharacterEditor ? 'Hide' : formData.character_definition ? 'Edit' : 'Add'}
            </button>
          </div>

          {/* Generate from URL - show when provider selected; for hybrid, model comes from provider */}
          {formData.provider_account_id && (formData.model_name || (() => {
            const p = providers.find(x => x.id === formData.provider_account_id);
            return p?.provider_type === 'hybrid';
          })()) && !useLocalModel && (
            <div style={{ 
              marginBottom: '15px', 
              padding: '12px', 
              background: '#f8f9fa', 
              borderRadius: '4px',
              border: '1px solid #dee2e6'
            }}>
              <label style={{ fontSize: '13px', fontWeight: 'bold', marginBottom: '8px', display: 'block' }}>
                🌐 Generate Character from Profile URLs
              </label>
              
              <div style={{ marginBottom: '10px' }}>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)', display: 'block', marginBottom: '5px' }}>
                  Person's Name (helps identify the correct person):
                </label>
                <input
                  type="text"
                  value={personName}
                  onChange={(e) => setPersonName(e.target.value)}
                  placeholder="e.g., John Doe, Jane Smith"
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                  disabled={loadingCharacter}
                />
              </div>
              
              <div style={{ marginBottom: '10px' }}>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)', display: 'block', marginBottom: '5px' }}>
                  Profile URLs (one per line):
                </label>
                <textarea
                  value={profileUrls}
                  onChange={(e) => setProfileUrls(e.target.value)}
                  placeholder="https://linkedin.com/in/johndoe&#10;https://johndoe.com/about&#10;https://twitter.com/johndoe"
                  rows={4}
                  style={{ 
                    width: '100%', 
                    padding: '8px', 
                    borderRadius: '4px', 
                    border: '1px solid var(--border-color)',
                    fontFamily: 'monospace',
                    fontSize: '12px'
                  }}
                  disabled={loadingCharacter}
                />
              </div>
              
              <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                <button
                  type="button"
                  className="btn btn-primary"
                  style={{ fontSize: '12px', padding: '8px 15px' }}
                  onClick={async () => {
                    const urls = profileUrls.split('\n').map(u => u.trim()).filter(u => u.length > 0);
                    if (urls.length === 0) {
                      alert('Please enter at least one URL');
                      return;
                    }
                    const sel = providers.find((x: any) => x.id === formData.provider_account_id);
                    const isHybrid = sel?.provider_type === 'hybrid';
                    const modelToUse = isHybrid ? ((sel?.provider_metadata_json as any)?.primary_model || 'hybrid') : formData.model_name;
                    if (!formData.provider_account_id || !modelToUse) {
                      alert('Please select a provider (and model for non-hybrid) first');
                      return;
                    }
                    // Create abort controller and cancellation token for cancellation
                    const controller = new AbortController();
                    const token = crypto.randomUUID();
                    setAbortController(controller);
                    setCancellationToken(token);
                    setLoadingCharacter(true);
                    
                    try {
                      const response = await api.generateCharacterFromUrl(
                        urls,
                        personName.trim() || undefined,
                        formData.provider_account_id,
                        modelToUse,
                        controller.signal,
                        token
                      );
                      
                      // Check if cancelled
                      if (controller.signal.aborted) {
                        return;
                      }
                      
                      // Populate persona_prompt with extracted text so it's visible when editing
                      setFormData({ 
                        ...formData, 
                        character_definition: response.character,
                        persona_prompt: response.extracted_text || formData.persona_prompt
                      });
                      setShowCharacterEditor(true);
                      setProfileUrls('');
                      setPersonName('');
                      alert('Character definition generated successfully! The extracted text has been added to the Persona Prompt field.');
                    } catch (error: any) {
                      if (controller.signal.aborted || error?.message === 'Request cancelled' || error?.includes('cancelled')) {
                        console.log('Generation cancelled by user');
                        return;
                      }
                      console.error('Failed to generate character:', error);
                      alert(`Failed to generate character: ${error?.message || error || 'Unknown error'}`);
                    } finally {
                      setLoadingCharacter(false);
                      setAbortController(null);
                      setCancellationToken(null);
                    }
                  }}
                  disabled={loadingCharacter || !profileUrls.trim() || !formData.provider_account_id || (!formData.model_name && providers.find((x: any) => x.id === formData.provider_account_id)?.provider_type !== 'hybrid')}
                >
                  {loadingCharacter ? '⏳ Generating...' : '✨ Generate'}
                </button>
                {loadingCharacter && abortController && cancellationToken && (
                  <button
                    type="button"
                    onClick={async () => {
                      abortController.abort();
                      try {
                        await api.cancelCharacterGeneration(cancellationToken);
                      } catch (error) {
                        console.error('Failed to cancel generation:', error);
                      }
                      setLoadingCharacter(false);
                      setAbortController(null);
                      setCancellationToken(null);
                    }}
                    style={{
                      padding: '8px 15px',
                      fontSize: '12px',
                      background: '#dc3545',
                      color: 'white',
                      border: 'none',
                      borderRadius: '4px',
                      cursor: 'pointer'
                    }}
                  >
                    ⛔ Stop
                  </button>
                )}
                {profileUrls.split('\n').filter(u => u.trim().length > 0).length > 0 && (
                  <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                    {profileUrls.split('\n').filter(u => u.trim().length > 0).length} URL{profileUrls.split('\n').filter(u => u.trim().length > 0).length !== 1 ? 's' : ''} entered
                  </span>
                )}
              </div>
              <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginTop: '6px' }}>
                Enter one or more URLs (one per line) to profiles, websites, or social media pages about the person. 
                {personName && ` The system will look for information about "${personName}" specifically.`}
                The system will automatically extract and create a character definition from all provided sources.
              </small>
            </div>
          )}
          {formData.character_definition && !showCharacterEditor && (
            <div style={{ padding: '10px', background: '#f5f5f5', borderRadius: '4px', marginTop: '5px' }}>
              <strong>{formData.character_definition.name}</strong> - {formData.character_definition.role}
            </div>
          )}
          {showCharacterEditor && (
            <CharacterEditor
              character={formData.character_definition}
              onSave={(char) => {
                setFormData({ ...formData, character_definition: char });
                setShowCharacterEditor(false);
              }}
              onCancel={() => setShowCharacterEditor(false)}
            />
          )}
        </div>

        {/* Model Features Section */}
        <div className="form-group">
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <label>Model Features (Optional)</label>
            <button
              type="button"
              className="btn btn-secondary"
              style={{ fontSize: '12px', padding: '5px 10px' }}
              onClick={() => setShowFeaturesEditor(!showFeaturesEditor)}
            >
              {showFeaturesEditor ? 'Hide' : formData.model_features ? 'Edit' : 'Add'}
            </button>
          </div>
          {formData.model_features && !showFeaturesEditor && (
            <div style={{ padding: '10px', background: '#f5f5f5', borderRadius: '4px', marginTop: '5px', fontSize: '12px' }}>
              {formData.model_features.supports_vision && <span>👁️ Vision</span>}
              {formData.model_features.supports_function_calling && <span>🔧 Functions</span>}
              {formData.model_features.supports_streaming && <span>📡 Streaming</span>}
              {formData.model_features.max_context_length && (
                <span>📏 {formData.model_features.max_context_length.toLocaleString()} tokens</span>
              )}
            </div>
          )}
          {showFeaturesEditor && (
            <FeaturesEditor
              features={formData.model_features}
              onSave={(features) => {
                setFormData({ ...formData, model_features: features });
                setShowFeaturesEditor(false);
              }}
              onCancel={() => setShowFeaturesEditor(false)}
            />
          )}
        </div>

        {/* Voice (TTS) Settings */}
        <div className="form-group">
          <label>Voice (TTS)</label>
          <VoiceSettings
            voiceGender={formData.voice_gender}
            voiceUri={formData.voice_uri}
            onVoiceGenderChange={(g: VoiceGender) => setFormData((prev) => ({ ...prev, voice_gender: g }))}
            onVoiceUriChange={(uri: string) => setFormData((prev) => ({ ...prev, voice_uri: uri }))}
            lang="en-US"
          />
        </div>

        {/* Generation Parameters */}
        <div className="form-group">
          <label>Generation Parameters</label>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: '10px' }}>
            <div>
              <label style={{ fontSize: '12px' }}>Temperature</label>
              <input
                type="number"
                step="0.1"
                min="0"
                max="2"
                value={formData.params.temperature || 0.7}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    params: { ...formData.params, temperature: parseFloat(e.target.value) },
                  })
                }
              />
            </div>
            <div>
              <label style={{ fontSize: '12px' }}>Top P</label>
              <input
                type="number"
                step="0.1"
                min="0"
                max="1"
                value={formData.params.top_p || 1.0}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    params: { ...formData.params, top_p: parseFloat(e.target.value) },
                  })
                }
              />
            </div>
            <div>
              <label style={{ fontSize: '12px' }}>Max Tokens</label>
              <input
                type="number"
                min="1"
                value={formData.params.max_tokens || 2000}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    params: { ...formData.params, max_tokens: parseInt(e.target.value) },
                  })
                }
              />
            </div>
          </div>
        </div>

        <div style={{ display: 'flex', gap: '10px', marginTop: '20px' }}>
          <button type="button" className="btn btn-primary" onClick={handleSave}>
            Save Profile
          </button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}

function CharacterEditor({
  character,
  onSave,
  onCancel,
}: {
  character: CharacterDefinition | null;
  onSave: (char: CharacterDefinition) => void;
  onCancel: () => void;
}) {
  const [char, setChar] = useState<CharacterDefinition>(
    character || {
      name: '',
      role: '',
      personality: [],
      expertise: [],
      communication_style: '',
      background: '',
      goals: [],
      constraints: [],
    }
  );

  const addItem = (field: 'personality' | 'expertise' | 'goals' | 'constraints', value: string) => {
    if (value.trim()) {
      const current = char[field] || [];
      setChar({ ...char, [field]: [...current, value.trim()] });
    }
  };

  const removeItem = (field: 'personality' | 'expertise' | 'goals' | 'constraints', index: number) => {
    const current = char[field] || [];
    setChar({ ...char, [field]: current.filter((_, i) => i !== index) });
  };

  return (
    <div style={{ padding: '15px', border: '1px solid var(--border-color)', borderRadius: '4px', marginTop: '10px' }}>
      <div className="form-group">
        <label>Character Name</label>
        <input
          type="text"
          value={char.name}
          onChange={(e) => setChar({ ...char, name: e.target.value })}
          placeholder="e.g., Dr. Sarah Chen"
        />
      </div>

      <div className="form-group">
        <label>Role</label>
        <input
          type="text"
          value={char.role}
          onChange={(e) => setChar({ ...char, role: e.target.value })}
          placeholder="e.g., Senior Architect, Product Manager"
        />
      </div>

      <div className="form-group">
        <label>Personality Traits</label>
        <div style={{ display: 'flex', gap: '5px', marginBottom: '5px' }}>
          <input
            type="text"
            placeholder="Add trait..."
            onKeyPress={(e) => {
              if (e.key === 'Enter') {
                addItem('personality', e.currentTarget.value);
                e.currentTarget.value = '';
              }
            }}
          />
        </div>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px' }}>
          {char.personality.map((trait, i) => (
            <span
              key={i}
              style={{
                padding: '5px 10px',
                background: 'var(--tag-bg-1)',
                borderRadius: '15px',
                fontSize: '12px',
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
              }}
            >
              {trait}
              <button
                type="button"
                onClick={() => removeItem('personality', i)}
                style={{ border: 'none', background: 'none', cursor: 'pointer' }}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      </div>

      <div className="form-group">
        <label>Expertise Areas</label>
        <div style={{ display: 'flex', gap: '5px', marginBottom: '5px' }}>
          <input
            type="text"
            placeholder="Add expertise..."
            onKeyPress={(e) => {
              if (e.key === 'Enter') {
                addItem('expertise', e.currentTarget.value);
                e.currentTarget.value = '';
              }
            }}
          />
        </div>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px' }}>
          {char.expertise.map((exp, i) => (
            <span
              key={i}
              style={{
                padding: '5px 10px',
                background: '#f3e5f5',
                borderRadius: '15px',
                fontSize: '12px',
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
              }}
            >
              {exp}
              <button
                type="button"
                onClick={() => removeItem('expertise', i)}
                style={{ border: 'none', background: 'none', cursor: 'pointer' }}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      </div>

      <div className="form-group">
        <label>Communication Style</label>
        <textarea
          value={char.communication_style}
          onChange={(e) => setChar({ ...char, communication_style: e.target.value })}
          placeholder="How does this character communicate? (e.g., direct, diplomatic, technical)"
          rows={2}
        />
      </div>

      <div className="form-group">
        <label>Background (Optional)</label>
        <textarea
          value={char.background || ''}
          onChange={(e) => setChar({ ...char, background: e.target.value })}
          placeholder="Character background story..."
          rows={2}
        />
      </div>

      <div style={{ display: 'flex', gap: '10px', marginTop: '15px' }}>
        <button type="button" className="btn btn-primary" onClick={() => onSave(char)}>
          Save Character
        </button>
        <button type="button" className="btn btn-secondary" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

function FeaturesEditor({
  features,
  onSave,
  onCancel,
}: {
  features: ModelFeatures | null;
  onSave: (features: ModelFeatures) => void;
  onCancel: () => void;
}) {
  const [feat, setFeat] = useState<ModelFeatures>(
    features || {
      supports_vision: false,
      supports_function_calling: false,
      supports_streaming: true,
      max_context_length: undefined,
      supports_tools: false,
      custom_capabilities: [],
    }
  );

  const addCapability = (value: string) => {
    if (value.trim() && !feat.custom_capabilities?.includes(value.trim())) {
      setFeat({
        ...feat,
        custom_capabilities: [...(feat.custom_capabilities || []), value.trim()],
      });
    }
  };

  const removeCapability = (index: number) => {
    setFeat({
      ...feat,
      custom_capabilities: feat.custom_capabilities?.filter((_, i) => i !== index) || [],
    });
  };

  return (
    <div style={{ padding: '15px', border: '1px solid var(--border-color)', borderRadius: '4px', marginTop: '10px' }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '15px' }}>
        <div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="checkbox"
              checked={feat.supports_vision || false}
              onChange={(e) => setFeat({ ...feat, supports_vision: e.target.checked })}
            />
            Supports Vision (Image Input)
          </label>
        </div>

        <div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="checkbox"
              checked={feat.supports_function_calling || false}
              onChange={(e) => setFeat({ ...feat, supports_function_calling: e.target.checked })}
            />
            Supports Function Calling
          </label>
        </div>

        <div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="checkbox"
              checked={feat.supports_streaming !== false}
              onChange={(e) => setFeat({ ...feat, supports_streaming: e.target.checked })}
            />
            Supports Streaming
          </label>
        </div>

        <div>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="checkbox"
              checked={feat.supports_tools || false}
              onChange={(e) => setFeat({ ...feat, supports_tools: e.target.checked })}
            />
            Supports Tools
          </label>
        </div>
      </div>

      <div className="form-group" style={{ marginTop: '15px' }}>
        <label>Max Context Length (tokens)</label>
        <input
          type="number"
          value={feat.max_context_length || ''}
          onChange={(e) =>
            setFeat({
              ...feat,
              max_context_length: e.target.value ? parseInt(e.target.value) : undefined,
            })
          }
          placeholder="e.g., 8192, 32768"
        />
      </div>

      <div className="form-group">
        <label>Custom Capabilities</label>
        <div style={{ display: 'flex', gap: '5px', marginBottom: '5px' }}>
          <input
            type="text"
            placeholder="Add capability..."
            onKeyPress={(e) => {
              if (e.key === 'Enter') {
                addCapability(e.currentTarget.value);
                e.currentTarget.value = '';
              }
            }}
          />
        </div>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px' }}>
          {feat.custom_capabilities?.map((cap, i) => (
            <span
              key={i}
              style={{
                padding: '5px 10px',
                background: 'var(--tag-bg-3)',
                borderRadius: '15px',
                fontSize: '12px',
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
              }}
            >
              {cap}
              <button
                type="button"
                onClick={() => removeCapability(i)}
                style={{ border: 'none', background: 'none', cursor: 'pointer' }}
              >
                ×
              </button>
            </span>
          ))}
        </div>
      </div>

      <div style={{ display: 'flex', gap: '10px', marginTop: '15px' }}>
        <button type="button" className="btn btn-primary" onClick={() => onSave(feat)}>
          Save Features
        </button>
        <button type="button" className="btn btn-secondary" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}
