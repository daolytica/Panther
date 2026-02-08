import { useState, useEffect } from 'react';
import { api, CreateProviderRequest } from '../api';
import { useAppStore } from '../store';
import type { ProviderAccount, ProviderType } from '../types';
import { Notification } from '../components/Notification';
import { TestConnectionModal } from '../components/TestConnectionModal';

export function Providers() {
  const { providers, setProviders, addProvider, removeProvider } = useAppStore();
  const [showModal, setShowModal] = useState(false);
  const [editingProvider, setEditingProvider] = useState<string | null>(null);
  const [notification, setNotification] = useState<{ message: string; type: 'success' | 'error' | 'info' } | null>(null);
  const [apiKeyExists, setApiKeyExists] = useState(false);
  const [testingProvider, setTestingProvider] = useState<any | null>(null);
  const [formData, setFormData] = useState<CreateProviderRequest>({
    provider_type: 'openai_compatible',
    display_name: '',
    base_url: '',
    region: '',
    api_key: '',
    provider_metadata_json: {},
  });

  // Hybrid provider config (stored into provider_metadata_json on save)
  const [hybridPrimaryProviderId, setHybridPrimaryProviderId] = useState<string>('');
  const [hybridFallbackProviderId, setHybridFallbackProviderId] = useState<string>('');
  const [hybridFallbackModel, setHybridFallbackModel] = useState<string>('');
  const [hybridTriggerTimeoutError, setHybridTriggerTimeoutError] = useState<boolean>(true);
  const [hybridTriggerRefusalGeneric, setHybridTriggerRefusalGeneric] = useState<boolean>(false);
  const [hybridTriggerEmptyShort, setHybridTriggerEmptyShort] = useState<boolean>(false);
  const [hybridLocalFirst, setHybridLocalFirst] = useState<boolean>(false);
  const [hybridPrivacyEnabled, setHybridPrivacyEnabled] = useState<boolean>(false);
  const [hybridPrivacyScrubPii, setHybridPrivacyScrubPii] = useState<boolean>(true);
  const [hybridPrivacyScrubSecrets, setHybridPrivacyScrubSecrets] = useState<boolean>(true);
  const [hybridPrivacyScrubContext, setHybridPrivacyScrubContext] = useState<boolean>(false);
  const [hybridPreprocessEnabled, setHybridPreprocessEnabled] = useState<boolean>(false);
  const [hybridPreprocessNormalizeWhitespace, setHybridPreprocessNormalizeWhitespace] = useState<boolean>(true);
  const [hybridPreprocessStandardizePunct, setHybridPreprocessStandardizePunct] = useState<boolean>(true);
  const [hybridPreprocessRemoveControlChars, setHybridPreprocessRemoveControlChars] = useState<boolean>(true);
  const [hybridPreprocessMaxChars, setHybridPreprocessMaxChars] = useState<string>('');
  const [hybridRequireSafetyControlBlock, setHybridRequireSafetyControlBlock] = useState<boolean>(false);
  const [fallbackOllamaModels, setFallbackOllamaModels] = useState<string[]>([]);
  const [loadingFallbackModels, setLoadingFallbackModels] = useState(false);
  const [hybridPrimaryModel, setHybridPrimaryModel] = useState<string>('');
  const [primaryCloudModels, setPrimaryCloudModels] = useState<string[]>([]);
  const [loadingPrimaryModels, setLoadingPrimaryModels] = useState(false);

  // Common Ollama model suggestions (popular versions)
  const _COMMON_OLLAMA_MODELS = [
    'llama3.2:3b',
    'llama3.2:1b',
    'llama3.1:8b',
    'llama3.1:70b',
    'llama3:8b',
    'mistral',
    'mistral:7b',
    'codellama',
    'phi3',
    'phi3:medium',
    'gemma2:2b',
    'gemma2:9b',
    'qwen2',
    'qwen2.5:7b',
  ];

  // Fetch Ollama models when fallback provider (Ollama) is selected
  useEffect(() => {
    if (formData.provider_type !== 'hybrid' || !hybridFallbackProviderId) {
      setFallbackOllamaModels([]);
      return;
    }
    const fallbackProvider = providers.find((p) => p.id === hybridFallbackProviderId);
    if (fallbackProvider?.provider_type !== 'ollama') {
      setFallbackOllamaModels([]);
      return;
    }
    setLoadingFallbackModels(true);
    api
      .listProviderModels(hybridFallbackProviderId)
      .then((models) => setFallbackOllamaModels(models || []))
      .catch(() => setFallbackOllamaModels([]))
      .finally(() => setLoadingFallbackModels(false));
  }, [formData.provider_type, hybridFallbackProviderId, providers]);

  // Fetch cloud models when primary provider is selected (for hybrid)
  useEffect(() => {
    if (formData.provider_type !== 'hybrid' || !hybridPrimaryProviderId) {
      setPrimaryCloudModels([]);
      return;
    }
    setLoadingPrimaryModels(true);
    api
      .listProviderModels(hybridPrimaryProviderId)
      .then((models) => setPrimaryCloudModels(models || []))
      .catch(() => setPrimaryCloudModels([]))
      .finally(() => setLoadingPrimaryModels(false));
  }, [formData.provider_type, hybridPrimaryProviderId]);

  const handleEdit = async (provider: any) => {
    setEditingProvider(provider.id);
    setFormData({
      provider_type: provider.provider_type,
      display_name: provider.display_name,
      base_url: provider.base_url || '',
      region: provider.region || '',
      api_key: '', // Don't pre-fill API key for security
      provider_metadata_json: provider.provider_metadata_json || {},
    });

    // Hydrate hybrid fields for the UI
    if (provider.provider_type === 'hybrid') {
      const meta = (provider.provider_metadata_json || {}) as any;
      setHybridPrimaryProviderId(String(meta.primary_provider_id || ''));
      setHybridFallbackProviderId(String(meta.fallback_provider_id || ''));
      setHybridFallbackModel(String(meta.fallback_model || ''));
      setHybridPrimaryModel(String(meta.primary_model || ''));
      const triggers = (meta.fallback_triggers || {}) as any;
      setHybridTriggerTimeoutError(triggers.timeout_error !== false);
      setHybridTriggerRefusalGeneric(Boolean(triggers.refusal_generic));
      setHybridTriggerEmptyShort(Boolean(triggers.empty_short));
      setHybridLocalFirst(Boolean(meta.local_first));
      const privacy = (meta.privacy_transform || {}) as any;
      setHybridPrivacyEnabled(Boolean(privacy.enabled));
      setHybridPrivacyScrubPii(privacy.scrub_pii !== false);
      setHybridPrivacyScrubSecrets(privacy.scrub_secrets !== false);
      setHybridPrivacyScrubContext(Boolean(privacy.scrub_context));
      const preprocess = (meta.input_preprocess || {}) as any;
      setHybridPreprocessEnabled(Boolean(preprocess.enabled));
      setHybridPreprocessNormalizeWhitespace(preprocess.normalize_whitespace !== false);
      setHybridPreprocessStandardizePunct(preprocess.standardize_punctuation !== false);
      setHybridPreprocessRemoveControlChars(preprocess.remove_control_chars !== false);
      setHybridPreprocessMaxChars(
        preprocess.max_chars !== undefined && preprocess.max_chars !== null ? String(preprocess.max_chars) : ''
      );
      setHybridRequireSafetyControlBlock(Boolean(meta.require_safety_control_block));
    } else {
      setHybridPrimaryProviderId('');
      setHybridFallbackProviderId('');
      setHybridFallbackModel('');
      setHybridPrimaryModel('');
      setHybridTriggerTimeoutError(true);
      setHybridTriggerRefusalGeneric(false);
      setHybridTriggerEmptyShort(false);
      setHybridLocalFirst(false);
      setHybridPrivacyEnabled(false);
      setHybridPrivacyScrubPii(true);
      setHybridPrivacyScrubSecrets(true);
      setHybridPrivacyScrubContext(false);
      setHybridPreprocessEnabled(false);
      setHybridPreprocessNormalizeWhitespace(true);
      setHybridPreprocessStandardizePunct(true);
      setHybridPreprocessRemoveControlChars(true);
      setHybridPreprocessMaxChars('');
      setHybridRequireSafetyControlBlock(false);
    }
    
    // Check if API key exists in keychain
    if (provider.auth_ref && provider.provider_type !== 'local_http') {
      try {
        await api.retrieveApiKey('panther', provider.auth_ref);
        setApiKeyExists(true);
      } catch {
        setApiKeyExists(false);
      }
    } else {
      setApiKeyExists(false);
    }
    
    setShowModal(true);
  };

  useEffect(() => {
    loadProviders();
  }, []);

  useEffect(() => {
    // Load profiles when providers change
    loadProfiles();
  }, [providers]);

  const loadProfiles = async () => {
    try {
      const data = await api.listProfiles();
      useAppStore.getState().setProfiles(data);
    } catch (error) {
      console.error('Failed to load profiles:', error);
    }
  };

  const loadProviders = async () => {
    try {
      const data = await api.listProviders();
      setProviders(data);
    } catch (error) {
      console.error('Failed to load providers:', error);
    }
  };

  const handleSave = async () => {
    try {
      // Validate required fields
      if (!formData.display_name.trim()) {
        setNotification({ message: 'Please enter a display name for the provider.', type: 'error' });
        return;
      }

      const providerType = formData.provider_type as ProviderType;
      const isHybrid = providerType === 'hybrid';
      const requiresApiKey = ['openai_compatible', 'anthropic', 'google', 'grok'].includes(providerType);

      // Validate base URL format if provided
      if (!isHybrid && formData.base_url && formData.base_url.trim()) {
        try {
          let url = formData.base_url.trim();
          // Add protocol if missing
          if (!url.startsWith('http://') && !url.startsWith('https://')) {
            // For local providers, default to http://, otherwise https://
            url = (providerType === 'local_http' || providerType === 'ollama' ? 'http://' : 'https://') + url;
          }
          new URL(url); // This will throw if invalid
          // Update the form data with the corrected URL
          formData.base_url = url;
        } catch {
          setNotification({ 
            message: providerType === 'local_http' || providerType === 'ollama'
              ? 'Please enter a valid base URL (e.g., http://localhost:11434)' 
              : 'Please enter a valid base URL (e.g., https://api.openai.com/v1)', 
            type: 'error' 
          });
          return;
        }
      }

      // Validate API key for cloud providers
      if (requiresApiKey) {
        if (editingProvider) {
          // When editing, API key is only required if no existing key exists
          if (!formData.api_key?.trim() && !apiKeyExists) {
            setNotification({ 
              message: 'Please enter an API key for cloud providers. If you have a saved key, make sure it exists in the keychain.', 
              type: 'error' 
            });
            return;
          }
        } else {
          // When creating, API key is required
          if (!formData.api_key?.trim()) {
            setNotification({ message: 'Please enter an API key for cloud providers.', type: 'error' });
            return;
          }
          // Basic API key format validation (should start with sk- for OpenAI)
          if (formData.provider_type === 'openai_compatible' && !formData.api_key.trim().startsWith('sk-')) {
            // Warn but don't block - some providers might use different formats
            console.warn('API key does not start with "sk-". Make sure this is correct for your provider.');
          }
        }
      }

      // Validate base URL for local_http
      if (formData.provider_type === 'local_http' && !formData.base_url?.trim()) {
        setNotification({ message: 'Base URL is required for local HTTP providers.', type: 'error' });
        return;
      }

      // Validate hybrid configuration
      if (isHybrid) {
        if (!hybridPrimaryProviderId.trim()) {
          setNotification({ message: 'Please select a Primary (cloud) provider for the hybrid provider.', type: 'error' });
          return;
        }
        if (!hybridFallbackProviderId.trim()) {
          setNotification({ message: 'Please select a Fallback (local) provider for the hybrid provider.', type: 'error' });
          return;
        }
        if (!hybridFallbackModel.trim()) {
          setNotification({ message: 'Please enter a Fallback model name (Ollama model) for the hybrid provider.', type: 'error' });
          return;
        }
      }

      const baseMetadata = (formData.provider_metadata_json || {}) as Record<string, unknown>;
      const providerMetadataJson: Record<string, unknown> = isHybrid
        ? {
            ...baseMetadata,
            primary_provider_id: hybridPrimaryProviderId.trim(),
            primary_model: hybridPrimaryModel.trim() || undefined,
            fallback_provider_id: hybridFallbackProviderId.trim(),
            fallback_model: hybridFallbackModel.trim(),
            fallback_triggers: {
              timeout_error: hybridTriggerTimeoutError,
              refusal_generic: hybridTriggerRefusalGeneric,
              empty_short: hybridTriggerEmptyShort,
            },
            local_first: hybridLocalFirst,
            privacy_transform: {
              enabled: hybridPrivacyEnabled,
              scrub_pii: hybridPrivacyScrubPii,
              scrub_secrets: hybridPrivacyScrubSecrets,
              scrub_context: hybridPrivacyScrubContext,
            },
            input_preprocess: {
              enabled: hybridPreprocessEnabled,
              remove_bom: true,
              remove_control_chars: hybridPreprocessRemoveControlChars,
              normalize_whitespace: hybridPreprocessNormalizeWhitespace,
              standardize_punctuation: hybridPreprocessStandardizePunct,
              max_chars: hybridPreprocessMaxChars.trim() ? Number(hybridPreprocessMaxChars.trim()) : null,
              truncation_strategy: 'head_tail',
            },
            require_safety_control_block: hybridRequireSafetyControlBlock,
          }
        : baseMetadata;

      // Only send API key if it's not empty (for updates, empty means "don't change")
      const submitData = {
        ...formData,
        api_key: formData.api_key?.trim() || undefined,
        base_url: isHybrid ? undefined : formData.base_url?.trim() || undefined,
        provider_metadata_json: providerMetadataJson,
      };

      if (editingProvider) {
        await api.updateProvider(editingProvider, submitData);
        setNotification({ 
          message: isHybrid
            ? 'Hybrid provider updated successfully!'
            : 'Provider updated successfully! API key has been saved securely.',
          type: 'success' 
        });
      } else {
        const id = await api.createProvider(submitData);
        const provider: ProviderAccount = {
          id,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
          provider_type: formData.provider_type as ProviderType,
          display_name: formData.display_name,
          base_url: submitData.base_url,
          region: formData.region || undefined,
          provider_metadata_json: providerMetadataJson,
        };
        addProvider(provider);
        setNotification({ 
          message: isHybrid
            ? 'Hybrid provider created successfully!'
            : 'Provider created successfully! API key has been saved securely in Windows Credential Manager.',
          type: 'success' 
        });
      }
      
      // Close modal after a short delay to show notification
      setTimeout(() => {
        setShowModal(false);
        setEditingProvider(null);
        setApiKeyExists(false);
        setHybridPrimaryProviderId('');
        setHybridFallbackProviderId('');
        setHybridFallbackModel('');
        setHybridPrimaryModel('');
        setHybridTriggerTimeoutError(true);
        setHybridTriggerRefusalGeneric(false);
        setHybridTriggerEmptyShort(false);
        setHybridLocalFirst(false);
        setHybridPrivacyEnabled(false);
        setHybridPrivacyScrubPii(true);
        setHybridPrivacyScrubSecrets(true);
        setHybridPrivacyScrubContext(false);
        setHybridPreprocessEnabled(false);
        setHybridPreprocessNormalizeWhitespace(true);
        setHybridPreprocessStandardizePunct(true);
        setHybridPreprocessRemoveControlChars(true);
        setHybridPreprocessMaxChars('');
        setFormData({
          provider_type: 'openai_compatible',
          display_name: '',
          base_url: '',
          region: '',
          api_key: '',
          provider_metadata_json: {},
        });
        loadProviders();
      }, 500);
    } catch (error: any) {
      console.error('Failed to save provider:', error);
      const errorMsg = error?.message || error?.toString() || 'Unknown error';
      setNotification({ 
        message: `Failed to save provider: ${errorMsg}`, 
        type: 'error' 
      });
    }
  };

  const handleCancel = () => {
    setShowModal(false);
    setEditingProvider(null);
    setApiKeyExists(false);
    setHybridPrimaryProviderId('');
    setHybridFallbackProviderId('');
    setHybridFallbackModel('');
    setHybridPrimaryModel('');
    setHybridTriggerTimeoutError(true);
    setHybridTriggerRefusalGeneric(false);
    setHybridTriggerEmptyShort(false);
    setHybridPrivacyEnabled(false);
    setHybridPrivacyScrubPii(true);
    setHybridPrivacyScrubSecrets(true);
    setHybridPrivacyScrubContext(false);
    setHybridPreprocessEnabled(false);
    setHybridPreprocessNormalizeWhitespace(true);
    setHybridPreprocessStandardizePunct(true);
    setHybridPreprocessRemoveControlChars(true);
    setHybridPreprocessMaxChars('');
    setFormData({
      provider_type: 'openai_compatible',
      display_name: '',
      base_url: '',
      region: '',
      api_key: '',
      provider_metadata_json: {},
    });
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Are you sure you want to delete this provider?')) return;
    try {
      await api.deleteProvider(id);
      removeProvider(id);
    } catch (error) {
      console.error('Failed to delete provider:', error);
    }
  };

  const handleTest = async (provider: any) => {
    // Open test modal
    setTestingProvider(provider);
  };

  const performTest = async (providerId: string): Promise<boolean> => {
    console.log('Starting test for provider:', providerId);
    try {
      const result = await api.testProviderConnection(providerId);
      console.log('Test completed with result:', result);
      return result === true;
    } catch (error: any) {
      console.error('Test error caught in performTest:', error);
      const errorMsg = error?.message || error?.toString() || String(error) || 'Unknown error';
      throw new Error(errorMsg);
    }
  };

  const handleTestClose = () => {
    setTestingProvider(null);
  };

  const providerTypes: { value: ProviderType; label: string }[] = [
    { value: 'openai_compatible', label: 'OpenAI Compatible' },
    { value: 'anthropic', label: 'Anthropic' },
    { value: 'google', label: 'Google/Gemini' },
    { value: 'grok', label: 'Grok (xAI)' },
    { value: 'ollama', label: 'Ollama (Local)' },
    { value: 'local_http', label: 'Local HTTP (OpenAI-compatible)' },
    { value: 'hybrid', label: 'Hybrid (Cloud + Ollama Fallback)' },
  ];

  return (
    <div>
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px', marginBottom: '10px' }}>
          <button
            onClick={() => window.history.back()}
            style={{
              padding: '8px 15px',
              background: 'transparent',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              cursor: 'pointer',
              fontSize: '14px',
              color: 'var(--text-primary)'
            }}
          >
            ← Back
          </button>
        </div>
        <h1>Providers</h1>
        <p>Manage your LLM provider connections</p>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          Add Provider
        </button>
      </div>

      <div className="grid grid-3">
        {providers.map((provider) => {
          const isUnrestricted = provider.provider_metadata_json?.unrestricted === true;
          return (
            <div key={provider.id} className="card">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: '10px' }}>
                <h3 style={{ margin: 0 }}>{provider.display_name}</h3>
                {isUnrestricted && (
                  <span style={{
                    fontSize: '10px',
                    padding: '2px 6px',
                    background: '#dc3545',
                    color: 'white',
                    borderRadius: '3px',
                    fontWeight: 'bold'
                  }}>
                    UNRESTRICTED
                  </span>
                )}
              </div>
              <p style={{ color: 'var(--text-secondary)', marginTop: '5px' }}>{provider.provider_type}</p>
              {provider.base_url && (
                <p style={{ color: 'var(--text-secondary)', fontSize: '12px', marginTop: '5px' }}>
                  {provider.base_url}
                </p>
              )}
              
              {/* Unrestricted Mode Section */}
              <div style={{
                marginTop: '15px',
                padding: '12px',
                background: isUnrestricted ? '#fff3cd' : '#f8f9fa',
                border: `1px solid ${isUnrestricted ? '#ffc107' : '#dee2e6'}`,
                borderRadius: '4px',
                borderLeft: `4px solid ${isUnrestricted ? '#dc3545' : '#6c757d'}`
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <strong style={{ fontSize: '13px', color: isUnrestricted ? '#856404' : '#495057' }}>
                    ⚠️ Unrestricted Mode
                  </strong>
                  <label style={{
                    display: 'flex',
                    alignItems: 'center',
                    cursor: 'pointer',
                    fontSize: '12px'
                  }}>
                    <input
                      type="checkbox"
                      checked={isUnrestricted}
                      onChange={async (e) => {
                        const enabled = e.target.checked;
                        if (enabled) {
                          const confirmed = window.confirm(
                            '⚠️ WARNING: Unrestricted mode bypasses safety filters.\n\n' +
                            'You are responsible for how you use this feature. Use at your own risk and in compliance with applicable laws.\n\n' +
                            'Do you want to enable unrestricted mode?'
                          );
                          if (!confirmed) {
                            return;
                          }
                        }
                        try {
                          const metadata = {
                            ...(provider.provider_metadata_json || {}),
                            unrestricted: enabled
                          };
                          await api.updateProvider(provider.id, {
                            provider_type: provider.provider_type,
                            display_name: provider.display_name,
                            base_url: provider.base_url,
                            region: provider.region,
                            api_key: undefined, // Don't update API key
                            provider_metadata_json: metadata
                          });
                          await loadProviders();
                        } catch (error) {
                          console.error('Failed to update unrestricted mode:', error);
                          alert('Failed to update unrestricted mode');
                        }
                      }}
                      style={{ marginRight: '6px' }}
                    />
                    <span style={{ color: isUnrestricted ? '#856404' : '#495057' }}>
                      {isUnrestricted ? 'Enabled' : 'Disabled'}
                    </span>
                  </label>
                </div>
                <p style={{ fontSize: '11px', color: isUnrestricted ? '#856404' : '#6c757d', margin: 0 }}>
                  {isUnrestricted 
                    ? 'Jailbreak prompts active - safety filters bypassed'
                    : 'Enable to bypass safety filters'}
                </p>
              </div>

              <div style={{ marginTop: '15px', display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                <button
                  className="btn btn-secondary"
                  style={{ fontSize: '12px', padding: '5px 10px' }}
                  onClick={() => handleEdit(provider)}
                >
                  Edit
                </button>
                <button
                  className="btn btn-secondary"
                  style={{ 
                    fontSize: '12px', 
                    padding: '5px 10px',
                    opacity: testingProvider?.id === provider.id ? 0.6 : 1,
                    cursor: testingProvider?.id === provider.id ? 'wait' : 'pointer'
                  }}
                  onClick={() => handleTest(provider)}
                  disabled={testingProvider?.id === provider.id}
                >
                  {testingProvider?.id === provider.id ? 'Testing...' : 'Test'}
                </button>
                <button
                  className="btn btn-danger"
                  style={{ fontSize: '12px', padding: '5px 10px' }}
                  onClick={() => handleDelete(provider.id)}
                >
                  Delete
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {showModal && (
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
            style={{ width: '500px', maxWidth: '90%', maxHeight: '90vh', overflowY: 'auto', position: 'relative' }}
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={handleCancel}
              style={{
                position: 'absolute',
                top: '15px',
                right: '15px',
                background: 'none',
                border: 'none',
                fontSize: '24px',
                cursor: 'pointer',
                color: 'var(--text-secondary)',
                padding: '0',
                width: '30px',
                height: '30px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                borderRadius: '4px',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--surface-hover)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'none';
              }}
            >
              ×
            </button>
            
            <h2 style={{ marginRight: '40px' }}>{editingProvider ? 'Edit Provider' : 'Add Provider'}</h2>
            
            <div>
              <div className="form-group">
                <label>Provider Type</label>
                <select
                  aria-label="Provider Type"
                  title="Provider Type"
                  value={formData.provider_type}
                  onChange={(e) => {
                    const nextType = e.target.value as ProviderType;
                    setFormData({ ...formData, provider_type: nextType });
                    setApiKeyExists(false); // Reset when provider type changes

                    // Initialize hybrid fields when switching to hybrid
                    if (nextType === 'hybrid') {
                      setHybridPrimaryProviderId('');
                      setHybridFallbackProviderId('');
                      setHybridFallbackModel('');
                      setHybridPrimaryModel('');
                      setHybridTriggerTimeoutError(true);
                      setHybridTriggerRefusalGeneric(false);
                      setHybridTriggerEmptyShort(false);
                      setHybridLocalFirst(true);
                      setHybridPrivacyEnabled(false);
                      setHybridPrivacyScrubContext(false);
                    }
                  }}
                >
                  {providerTypes.map((type) => (
                    <option key={type.value} value={type.value}>
                      {type.label}
                    </option>
                  ))}
                </select>
              </div>

              <div className="form-group">
                <label>Display Name *</label>
                <input
                  type="text"
                  aria-label="Display Name"
                  title="Display Name"
                  placeholder="e.g., OpenAI, Claude, Local Ollama, Hybrid"
                  value={formData.display_name}
                  onChange={(e) => setFormData({ ...formData, display_name: e.target.value })}
                />
              </div>

              {formData.provider_type === 'hybrid' && (
                <div className="form-group" style={{ padding: '12px', background: 'var(--bg-secondary)', borderRadius: '6px' }}>
                  <label style={{ fontWeight: 'bold', color: 'var(--primary)' }}>Hybrid Mode: Ollama First, Cloud Backup</label>
                  <p style={{ fontSize: '12px', color: 'var(--text-secondary)', margin: '8px 0 12px 0' }}>
                    Ollama runs locally first. Cloud is only called when Ollama fails or can't answer.
                  </p>

                  {/* STEP 1: Local/Ollama Provider - FIRST */}
                  <div style={{ marginTop: '10px', padding: '10px', background: 'var(--bg-primary)', borderRadius: '4px', border: '2px solid var(--success, #28a745)' }}>
                    <label style={{ fontSize: '13px', fontWeight: 'bold', color: 'var(--success, #28a745)' }}>
                      Step 1: Ollama Provider (tried FIRST)
                    </label>
                    <select
                      aria-label="Ollama provider"
                      title="Ollama provider"
                      value={hybridFallbackProviderId}
                      onChange={(e) => setHybridFallbackProviderId(e.target.value)}
                      style={{ marginTop: '6px' }}
                    >
                      <option value="">Select your Ollama provider...</option>
                      {/* Show Ollama providers first */}
                      {providers
                        .filter((p) => p.provider_type === 'ollama')
                        .map((p) => (
                          <option key={p.id} value={p.id}>
                            ✓ {p.display_name} (ollama) - RECOMMENDED
                          </option>
                        ))}
                      {/* Show other local providers with warning */}
                      {providers
                        .filter((p) => p.provider_type === 'local_http')
                        .map((p) => (
                          <option key={p.id} value={p.id}>
                            ⚠ {p.display_name} (local_http) - Not Ollama!
                          </option>
                        ))}
                    </select>
                    {hybridFallbackProviderId && providers.find(p => p.id === hybridFallbackProviderId)?.provider_type === 'local_http' && (
                      <small style={{ color: '#dc3545', fontSize: '11px', display: 'block', marginTop: '4px', fontWeight: 'bold' }}>
                        ⚠️ Warning: local_http uses OpenAI API format, not Ollama. Create an "ollama" type provider instead!
                      </small>
                    )}
                    {providers.filter(p => p.provider_type === 'ollama').length === 0 && (
                      <small style={{ color: '#dc3545', fontSize: '11px', display: 'block', marginTop: '4px', fontWeight: 'bold' }}>
                        ⚠️ No Ollama provider found! Create one first: Provider Type = "Ollama (Local)", Base URL = http://localhost:11434
                      </small>
                    )}
                  </div>

                  <div style={{ marginTop: '10px' }}>
                    <label style={{ fontSize: '12px' }}>Ollama Model Name</label>
                    {/* Show dropdown if we have installed models, otherwise show input */}
                    {fallbackOllamaModels.length > 0 ? (
                      <select
                        aria-label="Ollama model"
                        title="Ollama model"
                        value={hybridFallbackModel}
                        onChange={(e) => setHybridFallbackModel(e.target.value)}
                        style={{ width: '100%' }}
                      >
                        <option value="">Select installed model...</option>
                        {fallbackOllamaModels.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        type="text"
                        value={hybridFallbackModel}
                        onChange={(e) => setHybridFallbackModel(e.target.value)}
                        placeholder={loadingFallbackModels ? "Loading models from Ollama..." : "e.g., llama3.2:3b"}
                        list="ollama-fallback-models"
                        style={{ width: '100%' }}
                      />
                    )}
                    <datalist id="ollama-fallback-models">
                      {fallbackOllamaModels.map((m) => (
                        <option key={m} value={m} />
                      ))}
                    </datalist>
                    
                    {/* Status messages */}
                    {loadingFallbackModels && (
                      <small style={{ color: 'var(--primary)', fontSize: '11px', display: 'block', marginTop: '4px' }}>
                        Loading installed models from Ollama...
                      </small>
                    )}
                    {!loadingFallbackModels && fallbackOllamaModels.length > 0 && (
                      <small style={{ color: 'var(--success, #28a745)', fontSize: '11px', display: 'block', marginTop: '4px' }}>
                        ✓ Found {fallbackOllamaModels.length} installed model{fallbackOllamaModels.length !== 1 ? 's' : ''} in Ollama
                      </small>
                    )}
                    {!loadingFallbackModels && fallbackOllamaModels.length === 0 && hybridFallbackProviderId && (
                      <small style={{ color: '#dc3545', fontSize: '11px', display: 'block', marginTop: '4px' }}>
                        ⚠️ No models found. Make sure Ollama is running and you have pulled models (e.g., <code>ollama pull llama3.2:3b</code>)
                      </small>
                    )}
                    
                    {/* Quick install suggestions only when no models found */}
                    {!loadingFallbackModels && fallbackOllamaModels.length === 0 && (
                      <div style={{ marginTop: '8px', padding: '8px', background: 'var(--bg-primary)', borderRadius: '4px', border: '1px dashed var(--border-color)' }}>
                        <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginBottom: '6px' }}>
                          To install a model, run in terminal:
                        </small>
                        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                          {['llama3.2:3b', 'llama3.2:1b', 'mistral', 'phi3', 'gemma2:2b'].map((model) => (
                            <button
                              key={model}
                              type="button"
                              onClick={() => {
                                navigator.clipboard.writeText(`ollama pull ${model}`);
                                setHybridFallbackModel(model);
                              }}
                              style={{
                                fontSize: '10px',
                                padding: '3px 6px',
                                background: 'var(--bg-secondary)',
                                color: 'var(--text-primary)',
                                border: '1px solid var(--border-color)',
                                borderRadius: '3px',
                                cursor: 'pointer',
                              }}
                              title={`Click to copy: ollama pull ${model}`}
                            >
                              {model} (copy cmd)
                            </button>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>

                  {/* STEP 2: Cloud Provider - BACKUP */}
                  <div style={{ marginTop: '14px', padding: '10px', background: 'var(--bg-primary)', borderRadius: '4px', border: '1px solid var(--border-color)' }}>
                    <label style={{ fontSize: '13px', fontWeight: 'bold', color: 'var(--text-secondary)' }}>
                      Step 2: Cloud Provider (backup when Ollama fails)
                    </label>
                    <select
                      aria-label="Cloud backup provider"
                      title="Cloud backup provider"
                      value={hybridPrimaryProviderId}
                      onChange={(e) => {
                        setHybridPrimaryProviderId(e.target.value);
                        setHybridPrimaryModel('');
                      }}
                      style={{ marginTop: '6px' }}
                    >
                      <option value="">Select cloud provider...</option>
                      {providers
                        .filter((p) => p.provider_type !== 'hybrid' && p.provider_type !== 'ollama' && p.provider_type !== 'local_http')
                        .map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.display_name} ({p.provider_type})
                          </option>
                        ))}
                    </select>
                  </div>

                  <div style={{ marginTop: '10px' }}>
                    <label style={{ fontSize: '12px' }}>Cloud Model (used when Ollama can't answer)</label>
                    {primaryCloudModels.length > 0 ? (
                      <select
                        aria-label="Cloud model"
                        title="Cloud model"
                        value={hybridPrimaryModel}
                        onChange={(e) => setHybridPrimaryModel(e.target.value)}
                        style={{ width: '100%' }}
                      >
                        <option value="">Select cloud model...</option>
                        {primaryCloudModels.map((m) => (
                          <option key={m} value={m}>
                            {m}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <input
                        type="text"
                        value={hybridPrimaryModel}
                        onChange={(e) => setHybridPrimaryModel(e.target.value)}
                        placeholder={loadingPrimaryModels ? 'Loading models...' : 'e.g., openai/gpt-4o-mini, gpt-4'}
                        list="cloud-model-suggestions"
                        style={{ width: '100%' }}
                      />
                    )}
                    {loadingPrimaryModels && (
                      <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginTop: '4px' }}>
                        Loading models from cloud provider...
                      </small>
                    )}
                    <datalist id="cloud-model-suggestions">
                      {primaryCloudModels.map((m) => (
                        <option key={m} value={m} />
                      ))}
                      {['openai/gpt-4o-mini', 'openai/gpt-4o', 'anthropic/claude-3-5-sonnet', 'google/gemini-1.5-pro'].map((m) => (
                        <option key={m} value={m} />
                      ))}
                    </datalist>
                  </div>

                  <div style={{ marginTop: '12px', padding: '8px', background: hybridLocalFirst ? 'rgba(40, 167, 69, 0.1)' : 'rgba(220, 53, 69, 0.1)', borderRadius: '4px' }}>
                    <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginBottom: '4px' }}>
                      <input
                        type="checkbox"
                        checked={hybridLocalFirst}
                        onChange={(e) => setHybridLocalFirst(e.target.checked)}
                      />
                      <strong style={{ color: hybridLocalFirst ? 'var(--success, #28a745)' : '#dc3545' }}>
                        {hybridLocalFirst ? '✓ Ollama First Mode ENABLED' : '⚠ Ollama First Mode DISABLED'}
                      </strong>
                    </label>
                    <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginLeft: '22px' }}>
                      {hybridLocalFirst 
                        ? 'Ollama is tried first. Cloud is only called when Ollama times out, refuses, or gives empty response.'
                        : 'Cloud is tried first! Enable this to use Ollama first and save cloud tokens.'}
                    </small>
                  </div>

                  <div style={{ marginTop: '12px' }}>
                    <label style={{ fontSize: '12px', fontWeight: 'bold', display: 'block', marginBottom: '6px' }}>
                      When to fall back to cloud
                    </label>
                    <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                      <input
                        type="checkbox"
                        checked={hybridTriggerTimeoutError}
                        onChange={(e) => setHybridTriggerTimeoutError(e.target.checked)}
                      />
                      Timeouts / network/provider errors
                    </label>
                    <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                      <input
                        type="checkbox"
                        checked={hybridTriggerRefusalGeneric}
                        onChange={(e) => setHybridTriggerRefusalGeneric(e.target.checked)}
                      />
                      Generic refusal responses (benign requests only)
                    </label>
                    <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                      <input
                        type="checkbox"
                        checked={hybridTriggerEmptyShort}
                        onChange={(e) => setHybridTriggerEmptyShort(e.target.checked)}
                      />
                      Empty / too-short responses
                    </label>
                  </div>

                  <div style={{ marginTop: '14px' }}>
                    <label style={{ fontSize: '12px', fontWeight: 'bold', display: 'block', marginBottom: '6px' }}>
                      Privacy & input preprocessing
                    </label>

                    <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                      <input
                        type="checkbox"
                        checked={hybridPrivacyEnabled}
                        onChange={(e) => setHybridPrivacyEnabled(e.target.checked)}
                      />
                      Enable privacy redaction
                    </label>

                    <div style={{ marginTop: '8px', marginLeft: '22px' }}>
                      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                        <input
                          type="checkbox"
                          checked={hybridPrivacyScrubPii}
                          onChange={(e) => setHybridPrivacyScrubPii(e.target.checked)}
                          disabled={!hybridPrivacyEnabled}
                        />
                        Redact PII (emails/phones/IPs)
                      </label>
                      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                        <input
                          type="checkbox"
                          checked={hybridPrivacyScrubSecrets}
                          onChange={(e) => setHybridPrivacyScrubSecrets(e.target.checked)}
                          disabled={!hybridPrivacyEnabled}
                        />
                        Redact secrets (API keys/tokens/private keys)
                      </label>
                      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                        <input
                          type="checkbox"
                          checked={hybridPrivacyScrubContext}
                          onChange={(e) => setHybridPrivacyScrubContext(e.target.checked)}
                          disabled={!hybridPrivacyEnabled}
                        />
                        Also scrub conversation context
                      </label>
                    </div>

                    <div style={{ marginTop: '12px' }}>
                      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                        <input
                          type="checkbox"
                          checked={hybridPreprocessEnabled}
                          onChange={(e) => setHybridPreprocessEnabled(e.target.checked)}
                        />
                        Enable input normalization
                      </label>

                      <div style={{ marginTop: '8px', marginLeft: '22px' }}>
                        <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                          <input
                            type="checkbox"
                            checked={hybridPreprocessRemoveControlChars}
                            onChange={(e) => setHybridPreprocessRemoveControlChars(e.target.checked)}
                            disabled={!hybridPreprocessEnabled}
                          />
                          Remove BOM + control characters
                        </label>
                        <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                          <input
                            type="checkbox"
                            checked={hybridPreprocessNormalizeWhitespace}
                            onChange={(e) => setHybridPreprocessNormalizeWhitespace(e.target.checked)}
                            disabled={!hybridPreprocessEnabled}
                          />
                          Normalize whitespace
                        </label>
                        <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px', marginTop: '6px' }}>
                          <input
                            type="checkbox"
                            checked={hybridPreprocessStandardizePunct}
                            onChange={(e) => setHybridPreprocessStandardizePunct(e.target.checked)}
                            disabled={!hybridPreprocessEnabled}
                          />
                          Standardize quotes/dashes/ellipses
                        </label>

                        <div style={{ marginTop: '10px' }}>
                          <label style={{ fontSize: '12px' }}>Max length (chars, optional)</label>
                          <input
                            type="number"
                            min={0}
                            aria-label="Hybrid max input length"
                            title="Hybrid max input length"
                            value={hybridPreprocessMaxChars}
                            onChange={(e) => setHybridPreprocessMaxChars(e.target.value)}
                            disabled={!hybridPreprocessEnabled}
                            placeholder="e.g., 8000"
                          />
                          <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '4px' }}>
                            Uses head+tail truncation when exceeded.
                          </small>
                        </div>
                      </div>
                    </div>

                    <div style={{ marginTop: '12px' }}>
                      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '12px' }}>
                        <input
                          type="checkbox"
                          checked={hybridRequireSafetyControlBlock}
                          onChange={(e) => setHybridRequireSafetyControlBlock(e.target.checked)}
                        />
                        Require output {'>>>'} SAFETY_CONTROL_BLOCK (transparent safety disclosure)
                      </label>
                      <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '4px' }}>
                        Adds a visible safety disclosure requirement to system instructions.
                      </small>
                    </div>

                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '6px' }}>
                      This is deterministic privacy + normalization (not instruction obfuscation).
                    </small>
                  </div>
                </div>
              )}

              {formData.provider_type !== 'hybrid' && (
                <div className="form-group">
                  <label>
                    Base URL {formData.provider_type === 'local_http' && '*'}
                  </label>
                  <input
                    type="text"
                    value={formData.base_url}
                    onChange={(e) => setFormData({ ...formData, base_url: e.target.value })}
                    placeholder={
                      formData.provider_type === 'local_http' || formData.provider_type === 'ollama'
                        ? 'http://localhost:11434'
                        : 'https://api.openai.com/v1 (optional)'
                    }
                  />
                  {formData.provider_type === 'local_http' ? (
                  <div style={{ marginTop: '8px' }}>
                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginBottom: '4px' }}>
                      <strong>Common local server URLs:</strong>
                    </small>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '4px' }}>
                      <button
                        type="button"
                        onClick={() => setFormData({ ...formData, base_url: 'http://localhost:11434' })}
                        style={{
                          fontSize: '11px',
                          padding: '4px 8px',
                          background: 'var(--surface-hover)',
                          border: '1px solid var(--border-color)',
                          borderRadius: '3px',
                          cursor: 'pointer',
                          color: 'var(--text-primary)'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = 'var(--border-color)'}
                        onMouseLeave={(e) => e.currentTarget.style.background = 'var(--surface-hover)'}
                      >
                        Ollama (11434)
                      </button>
                      <button
                        type="button"
                        onClick={() => setFormData({ ...formData, base_url: 'http://localhost:1234/v1' })}
                        style={{
                          fontSize: '11px',
                          padding: '4px 8px',
                          background: 'var(--surface-hover)',
                          border: '1px solid var(--border-color)',
                          borderRadius: '3px',
                          cursor: 'pointer',
                          color: 'var(--text-primary)'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = 'var(--border-color)'}
                        onMouseLeave={(e) => e.currentTarget.style.background = 'var(--surface-hover)'}
                      >
                        LM Studio (1234)
                      </button>
                      <button
                        type="button"
                        onClick={() => setFormData({ ...formData, base_url: 'http://localhost:8000/v1' })}
                        style={{
                          fontSize: '11px',
                          padding: '4px 8px',
                          background: 'var(--surface-hover)',
                          border: '1px solid var(--border-color)',
                          borderRadius: '3px',
                          cursor: 'pointer',
                          color: 'var(--text-primary)'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = 'var(--border-color)'}
                        onMouseLeave={(e) => e.currentTarget.style.background = 'var(--surface-hover)'}
                      >
                        vLLM (8000)
                      </button>
                      <button
                        type="button"
                        onClick={() => setFormData({ ...formData, base_url: 'http://localhost:8080' })}
                        style={{
                          fontSize: '11px',
                          padding: '4px 8px',
                          background: 'var(--surface-hover)',
                          border: '1px solid var(--border-color)',
                          borderRadius: '3px',
                          cursor: 'pointer',
                          color: 'var(--text-primary)'
                        }}
                        onMouseEnter={(e) => e.currentTarget.style.background = 'var(--border-color)'}
                        onMouseLeave={(e) => e.currentTarget.style.background = 'var(--surface-hover)'}
                      >
                        llama.cpp (8080)
                      </button>
                    </div>
                    <small style={{ color: 'var(--text-secondary)', fontSize: '11px', display: 'block', marginTop: '6px' }}>
                      💡 Make sure your local server is running before testing. The app supports both Ollama and OpenAI-compatible APIs.
                    </small>
                  </div>
                ) : (
                  <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                    Leave empty to use default: https://api.openai.com/v1
                  </small>
                )}
                </div>
              )}

              {['openai_compatible', 'anthropic', 'google', 'grok'].includes(formData.provider_type as any) && (
                <div className="form-group">
                  <label>API Key {editingProvider && '(leave empty to keep existing)'}</label>
                  <div style={{ position: 'relative' }}>
                    <input
                      type="password"
                      value={formData.api_key}
                      onChange={(e) => {
                        setFormData({ ...formData, api_key: e.target.value });
                        if (e.target.value) {
                          setApiKeyExists(false); // User is entering new key
                        }
                      }}
                      placeholder={
                        editingProvider 
                          ? (apiKeyExists ? "✓ API key saved (enter new key to update)" : "Leave empty to keep existing key")
                          : "sk-... (required)"
                      }
                      style={{
                        paddingRight: editingProvider && apiKeyExists && !formData.api_key ? '100px' : '10px',
                      }}
                    />
                    {editingProvider && apiKeyExists && !formData.api_key && (
                      <span
                        style={{
                          position: 'absolute',
                          right: '10px',
                          top: '50%',
                          transform: 'translateY(-50%)',
                          color: '#28a745',
                          fontSize: '12px',
                          fontWeight: 'bold',
                          pointerEvents: 'none',
                        }}
                      >
                        ✓ Saved
                      </span>
                    )}
                  </div>
                  <small style={{ color: 'var(--text-secondary)', fontSize: '12px', display: 'block', marginTop: '5px' }}>
                    {editingProvider 
                      ? (apiKeyExists 
                          ? '✓ API key is stored securely. Enter a new key only if you want to update it.'
                          : 'Your API key is stored securely. Enter a new key only if you want to update it.')
                      : 'Your API key will be stored securely in Windows Credential Manager.'}
                  </small>
                </div>
              )}

              <div className="form-group">
                <label>Region (optional)</label>
                <input
                  type="text"
                  aria-label="Region"
                  title="Region"
                  placeholder="e.g., us-east-1 (optional)"
                  value={formData.region}
                  onChange={(e) => setFormData({ ...formData, region: e.target.value })}
                />
              </div>

              <div style={{ display: 'flex', gap: '10px', marginTop: '20px', justifyContent: 'flex-end' }}>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={handleCancel}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleSave}
                >
                  {editingProvider ? 'Save Changes' : 'Create Provider'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {notification && (
        <Notification
          message={notification.message}
          type={notification.type}
          onClose={() => setNotification(null)}
        />
      )}

      {testingProvider && (
        <TestConnectionModal
          provider={testingProvider}
          onClose={handleTestClose}
          onTest={performTest}
        />
      )}
    </div>
  );
}
