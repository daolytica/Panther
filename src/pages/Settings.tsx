import { useState, useEffect } from 'react';
import { api } from '../api';
import { useAppStore } from '../store';
import { VoiceSettings, type VoiceGender } from '../components/VoiceSettings';

interface OllamaHealth {
  installed: boolean;
  running: boolean;
  version?: string;
  models: string[];
  base_url: string;
}

interface DependencyStatus {
  name: string;
  installed: boolean;
  version?: string;
  install_command: string;
  description: string;
}

interface CudaStatus {
  available: boolean;
  version?: string;
  device_name?: string;
  device_count: number;
  message: string;
}

interface OptionalDependenciesStatus {
  huggingface_hub: DependencyStatus;
  hf_xet: DependencyStatus;
  psutil: DependencyStatus;
  safetensors?: DependencyStatus;
  peft?: DependencyStatus;
  trl?: DependencyStatus;
  bitsandbytes?: DependencyStatus;
}

interface DependenciesStatus {
  python: DependencyStatus;
  pip: DependencyStatus;
  transformers: DependencyStatus;
  datasets: DependencyStatus;
  torch: DependencyStatus;
  accelerate: DependencyStatus;
  cuda: CudaStatus;
  optional: OptionalDependenciesStatus;
}

function DependenciesTab() {
  const [dependencies, setDependencies] = useState<DependenciesStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);
  const [installMessage, setInstallMessage] = useState<string | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [hfToken, setHfToken] = useState<string>('');
  const [hfTokenSaved, setHfTokenSaved] = useState<boolean>(false);
  const [savingToken, setSavingToken] = useState(false);

  useEffect(() => {
    checkDependencies();
    loadHfToken();
  }, []);

  const loadHfToken = async () => {
    try {
      const token = await api.getHfToken();
      if (token) {
        setHfToken(token);
        setHfTokenSaved(true);
      }
    } catch (error) {
      // Token doesn't exist yet, that's fine
    }
  };

  const saveHfToken = async () => {
    if (!hfToken.trim()) {
      alert('Please enter a Hugging Face token');
      return;
    }
    setSavingToken(true);
    try {
      await api.saveHfToken(hfToken.trim());
      setHfTokenSaved(true);
      setInstallMessage('Hugging Face token saved successfully!');
    } catch (error: any) {
      setInstallError(`Failed to save token: ${error.message || error}`);
    } finally {
      setSavingToken(false);
    }
  };

  const deleteHfToken = async () => {
    if (!confirm('Delete saved Hugging Face token?')) return;
    try {
      await api.deleteHfToken();
      setHfToken('');
      setHfTokenSaved(false);
      setInstallMessage('Hugging Face token deleted');
    } catch (error: any) {
      setInstallError(`Failed to delete token: ${error.message || error}`);
    }
  };

  const checkDependencies = async () => {
    setLoading(true);
    setInstallMessage(null);
    setInstallError(null);
    try {
      const deps = await api.checkDependencies();
      setDependencies(deps);
    } catch (error: any) {
      setInstallError(`Failed to check dependencies: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  const installDependency = async (name: string) => {
    setInstalling(name);
    setInstallMessage(null);
    setInstallError(null);
    try {
      const result = await api.installDependency(name);
      setInstallMessage(result);
      // Recheck dependencies after installation
      setTimeout(() => {
        checkDependencies();
      }, 1000);
    } catch (error: any) {
      setInstallError(`Failed to install ${name}: ${error}`);
    } finally {
      setInstalling(null);
    }
  };

  const installAll = async () => {
    setInstalling('all');
    setInstallMessage(null);
    setInstallError(null);
    try {
      const result = await api.installAllDependencies();
      setInstallMessage(result);
      // Recheck dependencies after installation
      setTimeout(() => {
        checkDependencies();
      }, 2000);
    } catch (error: any) {
      setInstallError(`Failed to install dependencies: ${error}`);
    } finally {
      setInstalling(null);
    }
  };

  const upgradeDependency = async (name: string) => {
    setInstalling(name);
    setInstallMessage(null);
    setInstallError(null);
    try {
      const result = await api.upgradeDependency(name);
      setInstallMessage(result);
      setTimeout(() => {
        checkDependencies();
      }, 1000);
    } catch (error: any) {
      setInstallError(`Failed to upgrade ${name}: ${error.message || error}`);
    } finally {
      setInstalling(null);
    }
  };

  const uninstallDependency = async (name: string) => {
    if (!confirm(`Are you sure you want to uninstall ${name}?`)) return;
    setInstalling(name);
    setInstallMessage(null);
    setInstallError(null);
    try {
      const result = await api.uninstallDependency(name);
      setInstallMessage(result);
      setTimeout(() => {
        checkDependencies();
      }, 1000);
    } catch (error: any) {
      setInstallError(`Failed to uninstall ${name}: ${error.message || error}`);
    } finally {
      setInstalling(null);
    }
  };

  const renderDependency = (dep: DependencyStatus, key: string) => {
    const isInstalling = installing === key || installing === 'all';
    const isRequired = ['python', 'pip', 'transformers', 'datasets', 'torch'].includes(key);
    return (
      <div
        key={key}
        style={{
          padding: '15px',
          border: '1px solid var(--border-color)',
          borderRadius: '4px',
          marginBottom: '15px',
          background: dep.installed ? 'var(--bg-secondary)' : 'var(--card-bg)',
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
          <div>
            <h3 style={{ margin: 0, fontSize: '16px', display: 'flex', alignItems: 'center', gap: '8px' }}>
              {dep.installed ? '[OK]' : '[Missing]'} {dep.name}
              {dep.version && (
                <span style={{ fontSize: '12px', color: 'var(--text-secondary)', fontWeight: 'normal' }}>
                  (v{dep.version})
                </span>
              )}
            </h3>
            <p style={{ margin: '5px 0 0 0', fontSize: '13px', color: 'var(--text-secondary)' }}>
              {dep.description}
            </p>
          </div>
          <div style={{ display: 'flex', gap: '8px' }}>
            {!dep.installed ? (
              <button
                className="btn btn-primary"
                onClick={() => installDependency(key)}
                disabled={isInstalling}
                style={{ whiteSpace: 'nowrap' }}
              >
                {isInstalling ? 'Installing...' : 'Install'}
              </button>
            ) : (
              <>
                <button
                  className="btn btn-success"
                  onClick={() => upgradeDependency(key)}
                  disabled={isInstalling || key === 'python' || key === 'pip'}
                  style={{ whiteSpace: 'nowrap', fontSize: '12px' }}
                  title={key === 'python' || key === 'pip' ? 'Cannot upgrade Python/pip from here' : 'Upgrade to latest version'}
                >
                  {isInstalling ? 'Upgrading...' : 'Upgrade'}
                </button>
                {!isRequired && (
                  <button
                    className="btn btn-danger"
                    onClick={() => uninstallDependency(key)}
                    disabled={isInstalling}
                    style={{ whiteSpace: 'nowrap', fontSize: '12px' }}
                  >
                    {isInstalling ? 'Uninstalling...' : 'Uninstall'}
                  </button>
                )}
              </>
            )}
          </div>
        </div>
        {!dep.installed && (
          <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '8px' }}>
            <strong>Install command:</strong> <code>{dep.install_command}</code>
          </div>
        )}
      </div>
    );
  };

  if (loading && !dependencies) {
    return (
      <div className="card">
        <p>Checking dependencies...</p>
      </div>
    );
  }

  if (!dependencies) {
    return (
      <div className="card">
        <p>Failed to load dependencies. Please try again.</p>
        <button className="btn btn-secondary" onClick={checkDependencies}>
          Retry
        </button>
      </div>
    );
  }

  const allInstalled = dependencies.python.installed 
    && dependencies.pip.installed 
    && dependencies.transformers.installed 
    && dependencies.datasets.installed 
    && dependencies.torch.installed
    && dependencies.accelerate.installed;

  const missingCount = [
    dependencies.python,
    dependencies.pip,
    dependencies.transformers,
    dependencies.datasets,
    dependencies.torch,
    dependencies.accelerate,
  ].filter(d => !d.installed).length;

  return (
    <div>
      <div className="card" style={{ marginBottom: '20px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <div>
            <h2>Dependencies Management</h2>
            <p style={{ color: 'var(--text-secondary)', margin: '5px 0 0 0' }}>
              Install and manage Python dependencies required for model training
            </p>
          </div>
          <div style={{ display: 'flex', gap: '10px' }}>
            <button className="btn btn-secondary" onClick={checkDependencies} disabled={loading}>
              {loading ? 'Checking...' : 'Refresh'}
            </button>
            {missingCount > 0 && (
              <button
                className="btn btn-primary"
                onClick={installAll}
                disabled={installing === 'all' || loading}
              >
                {installing === 'all' ? 'Installing All...' : 'Install All'}
              </button>
            )}
          </div>
        </div>

        {installMessage && (
          <div style={{
            padding: '10px',
            background: '#e8f5e9',
            border: '1px solid #4caf50',
            borderRadius: '4px',
            marginBottom: '15px',
            color: '#2e7d32',
          }}>
            {installMessage}
          </div>
        )}

        {installError && (
          <div style={{
            padding: '10px',
            background: '#ffebee',
            border: '1px solid #f44336',
            borderRadius: '4px',
            marginBottom: '15px',
            color: '#c62828',
          }}>
            {installError}
          </div>
        )}

        {allInstalled ? (
          <div style={{
            padding: '15px',
            background: '#e8f5e9',
            border: '1px solid #4caf50',
            borderRadius: '4px',
            marginBottom: '20px',
          }}>
            <h3 style={{ margin: '0 0 10px 0', color: '#2e7d32' }}>All Dependencies Installed!</h3>
            <p style={{ margin: 0, color: 'var(--text-secondary)' }}>
              You're all set to start training models. All required dependencies are installed and ready to use.
            </p>
          </div>
        ) : (
          <div style={{
            padding: '15px',
            background: '#fff3e0',
            border: '1px solid #ff9800',
            borderRadius: '4px',
            marginBottom: '20px',
          }}>
            <strong>Missing Dependencies:</strong> {missingCount} dependency{missingCount !== 1 ? 'ies' : ''} need to be installed.
            Click "Install All" to install them automatically, or install them individually below.
          </div>
        )}

        <h3 style={{ marginTop: '20px', marginBottom: '15px' }}>Required Dependencies</h3>
        {renderDependency(dependencies.python, 'python')}
        {renderDependency(dependencies.pip, 'pip')}
        {renderDependency(dependencies.transformers, 'transformers')}
        {renderDependency(dependencies.datasets, 'datasets')}
        {renderDependency(dependencies.torch, 'torch')}
        {dependencies.accelerate && renderDependency(dependencies.accelerate, 'accelerate')}

        {/* Optional Dependencies */}
        <h3 style={{ marginTop: '30px', marginBottom: '15px' }}>📚 Optional Dependencies</h3>
        {dependencies && (
          <>
            {renderDependency(dependencies.optional.huggingface_hub, 'huggingface_hub')}
            {renderDependency(dependencies.optional.hf_xet, 'hf_xet')}
            {renderDependency(dependencies.optional.psutil, 'psutil')}
            {dependencies.optional.safetensors && renderDependency(dependencies.optional.safetensors, 'safetensors')}
            {dependencies.optional.peft && renderDependency(dependencies.optional.peft, 'peft')}
            {dependencies.optional.trl && renderDependency(dependencies.optional.trl, 'trl')}
            {dependencies.optional.bitsandbytes && renderDependency(dependencies.optional.bitsandbytes, 'bitsandbytes')}
          </>
        )}

        {/* Hugging Face Token Management */}
        <div style={{
          marginTop: '20px',
          padding: '15px',
          background: '#e3f2fd',
          border: '1px solid #2196f3',
          borderRadius: '4px',
        }}>
          <h3 style={{ marginTop: 0, marginBottom: '10px' }}>🔐 Hugging Face Authentication</h3>
          <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
            Some models (like Llama-2, Mistral) are gated and require authentication. 
            Enter your token below and the app will automatically use it during training.
          </p>
          
          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', marginBottom: '10px' }}>
            <input
              type="password"
              placeholder="Enter your Hugging Face token (hf_...)"
              value={hfToken}
              onChange={(e) => setHfToken(e.target.value)}
              style={{
                flex: 1,
                padding: '8px',
                border: '1px solid #ccc',
                borderRadius: '4px',
                fontSize: '13px',
              }}
              disabled={savingToken}
            />
            {hfTokenSaved ? (
              <>
                <button
                  className="btn btn-success"
                  onClick={saveHfToken}
                  disabled={savingToken}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  {savingToken ? 'Saving...' : 'Update Token'}
                </button>
                <button
                  className="btn btn-danger"
                  onClick={deleteHfToken}
                  disabled={savingToken}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  Delete
                </button>
              </>
            ) : (
              <button
                className="btn btn-primary"
                onClick={saveHfToken}
                disabled={savingToken || !hfToken.trim()}
                style={{ whiteSpace: 'nowrap' }}
              >
                {savingToken ? 'Saving...' : 'Save Token'}
              </button>
            )}
          </div>
          
          {hfTokenSaved && (
            <div style={{ fontSize: '12px', color: '#2e7d32', marginTop: '8px' }}>
              Token saved! It will be automatically used during training.
            </div>
          )}
          
          <div style={{ marginTop: '10px', fontSize: '12px', color: 'var(--text-secondary)' }}>
            <p style={{ margin: '5px 0' }}>
              <strong>Don't have a token?</strong> Get one from{' '}
              <a href="https://huggingface.co/settings/tokens" target="_blank" rel="noopener noreferrer">
                https://huggingface.co/settings/tokens
              </a>
            </p>
            <p style={{ margin: '5px 0', fontStyle: 'italic' }}>
              Note: The training script automatically falls back to non-gated models (like gpt2 or DialoGPT) if no token is provided.
            </p>
          </div>
        </div>

        {/* CUDA Status */}
        <div style={{
          marginTop: '20px',
          padding: '15px',
          background: dependencies.cuda.available ? '#e8f5e9' : '#fff3e0',
          border: `1px solid ${dependencies.cuda.available ? '#4caf50' : '#ff9800'}`,
          borderRadius: '4px',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '10px' }}>
            <span style={{ fontSize: '24px' }}>
              {dependencies.cuda.available ? '[OK]' : '[!]'}
            </span>
            <h4 style={{ margin: 0 }}>
              GPU (CUDA) Status: {dependencies.cuda.available ? 'Available' : 'Not Available'}
            </h4>
          </div>
          <p style={{ margin: '5px 0', fontSize: '14px', color: 'var(--text-secondary)' }}>
            {dependencies.cuda.message}
          </p>
          {dependencies.cuda.available && (
            <div style={{ marginTop: '10px', fontSize: '13px', color: '#2e7d32' }}>
              {dependencies.cuda.device_name && (
                <div><strong>GPU Device:</strong> {dependencies.cuda.device_name}</div>
              )}
              {dependencies.cuda.version && (
                <div><strong>CUDA Version:</strong> {dependencies.cuda.version}</div>
              )}
              {dependencies.cuda.device_count > 0 && (
                <div><strong>GPU Count:</strong> {dependencies.cuda.device_count}</div>
              )}
            </div>
          )}
          {!dependencies.cuda.available && dependencies.torch.installed && (
            <div style={{ marginTop: '15px', padding: '10px', background: 'white', borderRadius: '4px', fontSize: '13px' }}>
              <strong>To enable GPU training, install PyTorch with CUDA support:</strong>
              <div style={{ marginTop: '8px', fontFamily: 'monospace', background: '#f5f5f5', padding: '8px', borderRadius: '4px' }}>
                pip uninstall torch torchvision torchaudio
                <br />
                pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu118
              </div>
              <div style={{ marginTop: '8px', fontSize: '12px', color: 'var(--text-secondary)' }}>
                <strong>Steps:</strong>
                <ol style={{ marginTop: '5px', paddingLeft: '20px' }}>
                  <li>Check your CUDA version: Run <code>nvidia-smi</code> in terminal to see your CUDA version</li>
                  <li>Uninstall CPU-only PyTorch: <code>pip uninstall torch torchvision torchaudio</code></li>
                  <li>Install PyTorch with CUDA: Replace "cu118" with your CUDA version (cu121, cu117, cu118, etc.)</li>
                  <li>Verify installation: Run <code>python -c "import torch; print(torch.cuda.is_available())"</code> - should print <code>True</code></li>
                </ol>
                <div style={{ marginTop: '8px' }}>
                  <strong>Common CUDA versions:</strong>
                  <ul style={{ marginTop: '5px', paddingLeft: '20px' }}>
                    <li>CUDA 12.1: <code>--index-url https://download.pytorch.org/whl/cu121</code></li>
                    <li>CUDA 11.8: <code>--index-url https://download.pytorch.org/whl/cu118</code></li>
                    <li>CUDA 11.7: <code>--index-url https://download.pytorch.org/whl/cu117</code></li>
                  </ul>
                </div>
                <div style={{ marginTop: '8px' }}>
                  Visit <a href="https://pytorch.org/get-started/locally/" target="_blank" rel="noopener noreferrer">pytorch.org</a> for the correct installation command based on your system.
                </div>
              </div>
            </div>
          )}
        </div>

        <div style={{
          marginTop: '30px',
          padding: '15px',
          background: 'var(--bg-secondary)',
          borderRadius: '4px',
          fontSize: '13px',
        }}>
          <h4 style={{ marginTop: 0 }}>💡 Tips:</h4>
          <ul style={{ margin: '10px 0', paddingLeft: '20px' }}>
            <li>If Python is not installed, download it from <a href="https://www.python.org/downloads/" target="_blank" rel="noopener noreferrer">python.org</a></li>
            <li>Make sure Python is added to your system PATH</li>
            <li>If installation fails, try running the install commands manually in your terminal</li>
            <li><strong>For GPU training:</strong> Install PyTorch with CUDA support (see CUDA Status above)</li>
            <li><strong>For CPU training:</strong> Regular PyTorch installation works fine (slower but no GPU required)</li>
            <li>Some installations may take several minutes, especially PyTorch</li>
            <li>Training will automatically use GPU if available, otherwise falls back to CPU</li>
            <li><strong>Optional:</strong> Install <code>huggingface_hub</code>, <code>hf_xet</code>, and <code>psutil</code> for better functionality (see Optional Dependencies above)</li>
            <li><strong>Gated models:</strong> Set <code>HF_TOKEN</code> environment variable to use models like Llama-2 (see Hugging Face Authentication above)</li>
          </ul>
        </div>
      </div>
    </div>
  );
}

// Voice Settings Tab
function VoiceTab() {
  const {
    voiceEnabled,
    setVoiceEnabled,
    useLocalVoice,
    setUseLocalVoice,
    continuousAutoSend,
    setContinuousAutoSend,
    autoSpeakResponses,
    setAutoSpeakResponses,
    voiceGender,
    setVoiceGender,
    voiceUri,
    setVoiceUri,
    language,
  } = useAppStore();

  const lang = (language === 'en' ? 'en-US' : language) || 'en-US';

  return (
    <div>
      <p style={{ color: 'var(--text-secondary)', marginBottom: '20px', fontSize: '13px' }}>
        Configure voice input (STT) and output (TTS) for chat conversations.
      </p>

      <div style={{
        padding: '15px',
        background: 'var(--bg-secondary)',
        borderRadius: '4px',
        marginBottom: '20px',
      }}>
        <h3 style={{ marginTop: 0, marginBottom: '15px', fontSize: '14px' }}>Voice Features</h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={voiceEnabled}
              onChange={(e) => setVoiceEnabled(e.target.checked)}
            />
            <span><strong>Enable Voice (TTS/STT)</strong> – Use speech input and hear spoken responses</span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={useLocalVoice}
              onChange={(e) => setUseLocalVoice(e.target.checked)}
            />
            <span><strong>Use Local Voice</strong> – Whisper for STT (requires build with <code>--features voice</code>)</span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={continuousAutoSend}
              onChange={(e) => setContinuousAutoSend(e.target.checked)}
            />
            <span><strong>Auto-send on silence</strong> – In conversation mode, send when you stop speaking</span>
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={autoSpeakResponses}
              onChange={(e) => setAutoSpeakResponses(e.target.checked)}
            />
            <span><strong>Auto-speak responses</strong> – Automatically read AI responses aloud in chat</span>
          </label>
        </div>
      </div>

      <div style={{
        padding: '15px',
        background: 'var(--bg-secondary)',
        borderRadius: '4px',
        marginBottom: '20px',
      }}>
        <h3 style={{ marginTop: 0, marginBottom: '15px', fontSize: '14px' }}>Voice Output (TTS)</h3>
        <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
          Select the default voice (vocals) for text-to-speech. Open via <strong>Settings → Voice</strong> or when editing a profile. Profiles can override this.
        </p>
        <VoiceSettings
          voiceGender={voiceGender}
          voiceUri={voiceUri}
          onVoiceGenderChange={(g) => setVoiceGender(g)}
          onVoiceUriChange={(uri) => setVoiceUri(uri)}
          lang={lang}
        />
      </div>
    </div>
  );
}

// Privacy Settings Tab
interface PrivacySettings {
  redact_pii: boolean;
  private_mode: boolean;
  custom_identifiers: string[];
  retention_days: number | null;
}

interface RedactionPreview {
  original_text: string;
  redacted_text: string;
  stats: {
    emails_redacted: number;
    phones_redacted: number;
    urls_redacted: number;
    credit_cards_redacted: number;
    addresses_redacted: number;
    national_ids_redacted: number;
    custom_tokens_redacted: number;
    name_patterns_redacted: number;
    total_redactions: number;
  };
}

function PrivacyTab() {
  const [settings, setSettings] = useState<PrivacySettings>({
    redact_pii: true,
    private_mode: false,
    custom_identifiers: [],
    retention_days: 30,
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [newIdentifier, setNewIdentifier] = useState('');
  const [testText, setTestText] = useState('');
  const [testResult, setTestResult] = useState<RedactionPreview | null>(null);
  const [testLoading, setTestLoading] = useState(false);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const data = await api.getPrivacySettings();
      setSettings(data);
    } catch (error) {
      console.error('Failed to load privacy settings:', error);
    } finally {
      setLoading(false);
    }
  };

  const saveSettings = async (newSettings: PrivacySettings) => {
    setSaving(true);
    try {
      await api.savePrivacySettings(newSettings);
      setSettings(newSettings);
    } catch (error) {
      console.error('Failed to save settings:', error);
      alert('Failed to save settings');
    } finally {
      setSaving(false);
    }
  };

  const addIdentifier = async () => {
    if (!newIdentifier.trim()) return;
    try {
      const updated = await api.addCustomIdentifier(newIdentifier.trim());
      setSettings(updated);
      setNewIdentifier('');
    } catch (error) {
      alert('Failed to add identifier');
    }
  };

  const removeIdentifier = async (identifier: string) => {
    try {
      const updated = await api.removeCustomIdentifier(identifier);
      setSettings(updated);
    } catch (error) {
      alert('Failed to remove identifier');
    }
  };

  const testRedaction = async () => {
    if (!testText.trim()) return;
    setTestLoading(true);
    try {
      const result = await api.previewRedaction(testText);
      setTestResult(result);
    } catch (error) {
      alert('Failed to test redaction');
    } finally {
      setTestLoading(false);
    }
  };

  const deleteAllConversations = async () => {
    if (!confirm('This will permanently delete ALL conversation history. This cannot be undone. Continue?')) {
      return;
    }
    try {
      await api.deleteAllConversations();
      alert('All conversations deleted successfully');
    } catch (error) {
      alert('Failed to delete conversations');
    }
  };

  if (loading) {
    return <div>Loading privacy settings...</div>;
  }

  return (
    <div>
      <p style={{ color: 'var(--text-secondary)', marginBottom: '20px', fontSize: '13px' }}>
        Control how your data is protected when using AI chat features.
      </p>

      {/* Main Privacy Controls */}
      <div style={{ marginBottom: '30px' }}>
        <h3 style={{ marginBottom: '15px', fontSize: '14px', color: 'var(--text-secondary)' }}>Privacy Controls</h3>
        
        <div style={{ 
          display: 'flex', 
          flexDirection: 'column', 
          gap: '15px',
          padding: '15px',
          background: 'var(--bg-secondary)',
          borderRadius: '4px'
        }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={settings.redact_pii}
              onChange={(e) => saveSettings({ ...settings, redact_pii: e.target.checked })}
              disabled={saving}
            />
            <span>
              <strong>Redact PII before sending</strong>
              <br />
              <small style={{ color: 'var(--text-secondary)' }}>
                Automatically detect and redact emails, phone numbers, URLs, and other personal information before sending to AI providers.
              </small>
            </span>
          </label>

          <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
            <input
              type="checkbox"
              checked={settings.private_mode}
              onChange={(e) => saveSettings({ ...settings, private_mode: e.target.checked })}
              disabled={saving}
            />
            <span>
              <strong>Private Mode</strong>
              <br />
              <small style={{ color: 'var(--text-secondary)' }}>
                When enabled, chat messages are not stored on disk. Only active session data is kept in memory.
              </small>
            </span>
          </label>

          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <label style={{ flex: 0 }}>
              <strong>Retention:</strong>
            </label>
            <select
              value={settings.retention_days ?? 30}
              onChange={(e) => saveSettings({ ...settings, retention_days: parseInt(e.target.value) || null })}
              disabled={saving || settings.private_mode}
              style={{ padding: '5px 10px' }}
            >
              <option value={7}>7 days</option>
              <option value={30}>30 days</option>
              <option value={90}>90 days</option>
              <option value={365}>1 year</option>
            </select>
            <small style={{ color: 'var(--text-secondary)' }}>How long to keep encrypted chat history</small>
          </div>
        </div>
      </div>

      {/* Custom Identifiers */}
      <div style={{ marginBottom: '30px' }}>
        <h3 style={{ marginBottom: '15px', fontSize: '14px', color: 'var(--text-secondary)' }}>Custom Identifiers to Redact</h3>
        <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
          Add words or phrases (like your name, employer, or other personal identifiers) that should always be redacted.
        </p>

        <div style={{ display: 'flex', gap: '10px', marginBottom: '15px' }}>
          <input
            type="text"
            value={newIdentifier}
            onChange={(e) => setNewIdentifier(e.target.value)}
            placeholder="e.g., John Smith, Acme Corp"
            style={{ flex: 1, padding: '8px 12px' }}
            onKeyPress={(e) => e.key === 'Enter' && addIdentifier()}
          />
          <button className="btn btn-primary" onClick={addIdentifier}>
            Add
          </button>
        </div>

        {settings.custom_identifiers.length > 0 ? (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {settings.custom_identifiers.map((id, idx) => (
              <span
                key={idx}
                style={{
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '5px',
                  padding: '5px 10px',
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  fontSize: '13px'
                }}
              >
                {id}
                <button
                  onClick={() => removeIdentifier(id)}
                  style={{
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    color: '#dc3545',
                    fontSize: '14px',
                    padding: '0 2px'
                  }}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        ) : (
          <p style={{ color: 'var(--text-secondary)', fontSize: '13px', fontStyle: 'italic' }}>
            No custom identifiers added yet.
          </p>
        )}
      </div>

      {/* Test Redaction */}
      <div style={{ marginBottom: '30px' }}>
        <h3 style={{ marginBottom: '15px', fontSize: '14px', color: 'var(--text-secondary)' }}>Test Redaction</h3>
        <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
          Preview how your text will look after redaction.
        </p>

        <textarea
          value={testText}
          onChange={(e) => setTestText(e.target.value)}
          placeholder="Enter text with PII to test (e.g., 'My email is test@example.com and my phone is 555-1234567')"
          rows={3}
          style={{ width: '100%', marginBottom: '10px', padding: '10px' }}
        />
        <button 
          className="btn btn-secondary" 
          onClick={testRedaction}
          disabled={testLoading || !testText.trim()}
        >
          {testLoading ? 'Testing...' : 'Preview Redaction'}
        </button>

        {testResult && (
          <div style={{ 
            marginTop: '15px', 
            padding: '15px', 
            background: 'var(--bg-secondary)',
            borderRadius: '4px'
          }}>
            <div style={{ marginBottom: '10px' }}>
              <strong>Redacted Text:</strong>
              <pre style={{ 
                background: 'var(--bg-primary)', 
                padding: '10px', 
                borderRadius: '4px',
                whiteSpace: 'pre-wrap',
                fontSize: '13px',
                marginTop: '5px'
              }}>
                {testResult.redacted_text}
              </pre>
            </div>
            <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
              <strong>Redactions:</strong> {testResult.stats.total_redactions} total
              {testResult.stats.emails_redacted > 0 && ` • ${testResult.stats.emails_redacted} email(s)`}
              {testResult.stats.phones_redacted > 0 && ` • ${testResult.stats.phones_redacted} phone(s)`}
              {testResult.stats.urls_redacted > 0 && ` • ${testResult.stats.urls_redacted} URL(s)`}
              {testResult.stats.custom_tokens_redacted > 0 && ` • ${testResult.stats.custom_tokens_redacted} custom`}
              {testResult.stats.name_patterns_redacted > 0 && ` • ${testResult.stats.name_patterns_redacted} name(s)`}
            </div>
          </div>
        )}
      </div>

      {/* Data Deletion */}
      <div style={{ marginBottom: '20px' }}>
        <h3 style={{ marginBottom: '15px', fontSize: '14px', color: 'var(--text-secondary)' }}>Data Deletion</h3>
        <button 
          className="btn" 
          onClick={deleteAllConversations}
          style={{ background: '#dc3545', color: 'white' }}
        >
          Delete All Conversations
        </button>
        <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '8px' }}>
          This permanently deletes all chat history and encrypted data.
        </p>
      </div>

      {/* Privacy Notice */}
      <div style={{
        padding: '15px',
        background: 'var(--bg-secondary)',
        borderRadius: '4px',
        fontSize: '12px',
        color: 'var(--text-secondary)'
      }}>
        <strong>Privacy Notice:</strong> PII redaction helps minimize data exposure but cannot guarantee complete anonymity. 
        Network-level metadata and AI provider logs may still exist. Redaction uses pattern matching which may miss some PII or create false positives.
        For maximum privacy, enable Private Mode and regularly delete conversations.
      </div>
    </div>
  );
}

// Modal wrapper component
function ModalWrapper({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
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
        zIndex: 2000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: 'var(--card-bg)',
          borderRadius: '8px',
          padding: '24px',
          maxWidth: '90%',
          maxHeight: '90vh',
          overflow: 'auto',
          width: '700px',
          boxShadow: '0 4px 20px rgba(0,0,0,0.3)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <h1 style={{ margin: 0, fontSize: '20px' }}>{title}</h1>
          <button
            onClick={onClose}
            style={{
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
        </div>
        {children}
      </div>
    </div>
  );
}

// Privacy Modal
export function PrivacyModal({ onClose }: { onClose: () => void }) {
  return (
    <ModalWrapper title="🔒 Privacy Settings" onClose={onClose}>
      <PrivacyTab />
    </ModalWrapper>
  );
}

// Dependencies Modal
export function DependenciesModal({ onClose }: { onClose: () => void }) {
  return (
    <ModalWrapper title="Dependencies" onClose={onClose}>
      <DependenciesTab />
    </ModalWrapper>
  );
}

// Voice Modal
export function VoiceModal({ onClose }: { onClose: () => void }) {
  return (
    <ModalWrapper title="Voice Settings" onClose={onClose}>
      <VoiceTab />
    </ModalWrapper>
  );
}

// Ollama Modal
export function OllamaModal({ onClose }: { onClose: () => void }) {
  const [ollamaHealth, setOllamaHealth] = useState<OllamaHealth | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    checkOllama();
  }, []);

  const checkOllama = async () => {
    setLoading(true);
    try {
      const health = await api.checkOllamaInstallation();
      setOllamaHealth(health);
    } catch (error) {
      console.error('Failed to check Ollama:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <ModalWrapper title="🦙 Ollama" onClose={onClose}>
      {loading ? (
        <p>Checking Ollama...</p>
      ) : ollamaHealth ? (
        <div>
          <p><strong>Installed:</strong> {ollamaHealth.installed ? 'Yes' : 'No'}</p>
          <p><strong>Running:</strong> {ollamaHealth.running ? 'Yes' : 'No'}</p>
          {ollamaHealth.version && <p><strong>Version:</strong> {ollamaHealth.version}</p>}
          <p><strong>Base URL:</strong> {ollamaHealth.base_url}</p>
          {ollamaHealth.models && ollamaHealth.models.length > 0 && (
            <div style={{ marginTop: '20px' }}>
              <h3>Installed Models</h3>
              <ul>
                {ollamaHealth.models.map((model, idx) => (
                  <li key={idx}>{model}</li>
                ))}
              </ul>
            </div>
          )}
          <button 
            className="btn btn-secondary" 
            onClick={checkOllama}
            style={{ marginTop: '15px' }}
          >
            Refresh
          </button>
        </div>
      ) : (
        <div>
          <p>Failed to check Ollama status</p>
          <button 
            className="btn btn-secondary" 
            onClick={checkOllama}
            style={{ marginTop: '15px' }}
          >
            Retry
          </button>
        </div>
      )}
    </ModalWrapper>
  );
}

// Training Cache Tab
interface CacheStats {
  total_entries: number;
  total_size_bytes: number;
  total_size_gb: number;
  max_size_bytes: number;
  max_size_gb: number;
  usage_percent: number;
}

interface CacheSettings {
  max_size_gb: number;
  eviction_threshold_percent: number;
  enable_compression: boolean;
  enable_memory_mapped_files: boolean;
  memory_mapped_threshold_mb: number;
}

interface TrainingSettings {
  streaming_chunk_size: number;
  enable_adaptive_memory: boolean;
  min_chunk_size: number;
  max_chunk_size: number;
  memory_pressure_threshold_mb: number;
  enable_progress_tracking: boolean;
  progress_update_interval: number;
  enable_parallel_hashing: boolean;
  parallel_hash_threshold: number;
}

interface AutoTrainingSettings {
  auto_training_enabled: boolean;
  train_from_chat: boolean;
  train_from_coder: boolean;
  train_from_debate: boolean;
}

interface AppSettings {
  cache: CacheSettings;
  training: TrainingSettings;
  auto_training: AutoTrainingSettings;
  global_system_prompt_file?: string | null;
}

function TrainingCacheTab() {
  const [stats, setStats] = useState<CacheStats | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingSettings, setLoadingSettings] = useState(true);
  const [savingSettings, setSavingSettings] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [projectId, setProjectId] = useState<string>('');
  const [modelId, setModelId] = useState<string>('');

  useEffect(() => {
    loadStats();
    loadSettings();
  }, []);

  const loadSettings = async () => {
    setLoadingSettings(true);
    try {
      const data = await api.getAppSettings();
      setSettings(data);
    } catch (err: any) {
      setError(`Failed to load settings: ${err.message || err}`);
    } finally {
      setLoadingSettings(false);
    }
  };

  const saveSettings = async (newSettings: AppSettings) => {
    setSavingSettings(true);
    try {
      await api.saveAppSettings(newSettings);
      setSettings(newSettings);
      setMessage('Settings saved successfully!');
      setTimeout(() => setMessage(null), 3000);
    } catch (err: any) {
      setError(`Failed to save settings: ${err.message || err}`);
    } finally {
      setSavingSettings(false);
    }
  };

  const loadStats = async () => {
    setLoading(true);
    setMessage(null);
    setError(null);
    try {
      const data = await api.getTrainingCacheStats();
      setStats(data);
    } catch (err: any) {
      setError(`Failed to load cache stats: ${err.message || err}`);
    } finally {
      setLoading(false);
    }
  };

  const clearCache = async (clearAll: boolean = false) => {
    if (!clearAll && !projectId.trim()) {
      alert('Please enter a project ID or select "Clear All Cache"');
      return;
    }

    if (!confirm(clearAll 
      ? 'Are you sure you want to clear ALL training cache? This cannot be undone.'
      : `Clear cache for project "${projectId}"${modelId ? ` and model "${modelId}"` : ''}?`
    )) {
      return;
    }

    setClearing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await api.clearTrainingCache(
        clearAll ? undefined : projectId || undefined,
        clearAll ? undefined : modelId || undefined
      );
      setMessage(result.message || 'Cache cleared successfully');
      setProjectId('');
      setModelId('');
      await loadStats();
    } catch (err: any) {
      setError(`Failed to clear cache: ${err.message || err}`);
    } finally {
      setClearing(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round((bytes / Math.pow(k, i)) * 100) / 100 + ' ' + sizes[i];
  };

  if (loading && !stats) {
    return (
      <div className="card">
        <p>Loading cache statistics...</p>
      </div>
    );
  }

  return (
    <div>
      <p style={{ color: 'var(--text-secondary)', marginBottom: '20px', fontSize: '13px' }}>
        Manage cached training data files for faster training runs. Configure cache size limits and behavior below.
      </p>
      
      <div className="card" style={{ marginBottom: '20px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <div>
            <h3>Cache Statistics</h3>
          </div>
          <button className="btn btn-secondary" onClick={loadStats} disabled={loading}>
            {loading ? 'Refreshing...' : 'Refresh'}
          </button>
        </div>

        {message && (
          <div style={{
            padding: '10px',
            background: '#e8f5e9',
            border: '1px solid #4caf50',
            borderRadius: '4px',
            marginBottom: '15px',
            color: '#2e7d32',
          }}>
            {message}
          </div>
        )}

        {error && (
          <div style={{
            padding: '10px',
            background: '#ffebee',
            border: '1px solid #f44336',
            borderRadius: '4px',
            marginBottom: '15px',
            color: '#c62828',
          }}>
            {error}
          </div>
        )}

        {stats && (
          <>
            {/* Cache Statistics */}
            <div style={{
              padding: '15px',
              background: 'var(--bg-secondary)',
              borderRadius: '4px',
              marginBottom: '20px',
            }}>
              <h3 style={{ marginTop: 0, marginBottom: '15px' }}>Cache Statistics</h3>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '15px' }}>
                <div>
                  <div style={{ fontSize: '24px', fontWeight: 'bold', color: 'var(--primary-color)' }}>
                    {stats.total_entries}
                  </div>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Cached Files</div>
                </div>
                <div>
                  <div style={{ fontSize: '24px', fontWeight: 'bold', color: 'var(--primary-color)' }}>
                    {formatBytes(stats.total_size_bytes)}
                  </div>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Total Size</div>
                </div>
                <div>
                  <div style={{ fontSize: '24px', fontWeight: 'bold', color: 'var(--primary-color)' }}>
                    {formatBytes(stats.max_size_bytes)}
                  </div>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Max Size</div>
                </div>
                <div>
                  <div style={{ fontSize: '24px', fontWeight: 'bold', color: stats.usage_percent > 80 ? '#f44336' : stats.usage_percent > 60 ? '#ff9800' : '#4caf50' }}>
                    {stats.usage_percent.toFixed(1)}%
                  </div>
                  <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Usage</div>
                </div>
              </div>

              {/* Usage Bar */}
              <div style={{ marginTop: '15px' }}>
                <div style={{ 
                  width: '100%', 
                  height: '20px', 
                  background: 'var(--bg-primary)', 
                  borderRadius: '10px',
                  overflow: 'hidden',
                  border: '1px solid var(--border-color)'
                }}>
                  <div style={{
                    width: `${Math.min(100, stats.usage_percent)}%`,
                    height: '100%',
                    background: stats.usage_percent > 80 ? '#f44336' : stats.usage_percent > 60 ? '#ff9800' : '#4caf50',
                    transition: 'width 0.3s ease',
                  }} />
                </div>
              </div>
            </div>

            {/* Cache Information */}
            <div style={{
              padding: '15px',
              background: '#e3f2fd',
              border: '1px solid #2196f3',
              borderRadius: '4px',
              marginBottom: '20px',
              fontSize: '13px',
            }}>
              <h4 style={{ marginTop: 0, marginBottom: '10px' }}>How Cache Works</h4>
              <ul style={{ margin: '5px 0', paddingLeft: '20px', color: 'var(--text-secondary)' }}>
                <li>Training data is cached based on content hash for faster subsequent training runs</li>
                <li>Cached files are automatically compressed (gzip) to save disk space</li>
                <li>Cache is automatically evicted when it exceeds {stats.max_size_gb.toFixed(1)} GB (LRU policy)</li>
                <li>Cache is invalidated when training data is added, updated, or deleted</li>
                <li>Cache hit rate significantly speeds up training data preparation</li>
              </ul>
            </div>

            {/* Clear Cache Section */}
            <div style={{
              padding: '15px',
              background: 'var(--bg-secondary)',
              borderRadius: '4px',
              marginBottom: '20px',
            }}>
              <h3 style={{ marginTop: 0, marginBottom: '15px' }}>Clear Cache</h3>
              <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
                Clear cached training data files. This will force regeneration of cache files on next training run.
              </p>

              <div style={{ marginBottom: '15px' }}>
                <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                  Project ID (optional):
                </label>
                <input
                  type="text"
                  value={projectId}
                  onChange={(e) => setProjectId(e.target.value)}
                  placeholder="Leave empty to clear all"
                  style={{
                    width: '100%',
                    padding: '8px',
                    border: '1px solid var(--border-color)',
                    borderRadius: '4px',
                    fontSize: '13px',
                  }}
                  disabled={clearing}
                />
              </div>

              <div style={{ marginBottom: '15px' }}>
                <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                  Model ID (optional):
                </label>
                <input
                  type="text"
                  value={modelId}
                  onChange={(e) => setModelId(e.target.value)}
                  placeholder="Leave empty to clear all models for project"
                  style={{
                    width: '100%',
                    padding: '8px',
                    border: '1px solid var(--border-color)',
                    borderRadius: '4px',
                    fontSize: '13px',
                  }}
                  disabled={clearing}
                />
              </div>

              <div style={{ display: 'flex', gap: '10px' }}>
                <button
                  className="btn btn-primary"
                  onClick={() => clearCache(false)}
                  disabled={clearing || (!projectId.trim() && !modelId.trim())}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  {clearing ? 'Clearing...' : 'Clear Selected Cache'}
                </button>
                <button
                  className="btn btn-danger"
                  onClick={() => clearCache(true)}
                  disabled={clearing}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  {clearing ? 'Clearing...' : 'Clear All Cache'}
                </button>
              </div>
            </div>

            {/* Settings Configuration */}
            {settings && (
              <div style={{
                padding: '15px',
                background: 'var(--bg-secondary)',
                borderRadius: '4px',
                marginBottom: '20px',
              }}>
                <h3 style={{ marginTop: 0, marginBottom: '15px' }}>Cache Settings</h3>
                <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '20px' }}>
                  Configure cache behavior and limits. Changes take effect on next cache operation.
                </p>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '15px' }}>
                  {/* Cache Size */}
                  <div>
                    <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                      Max Cache Size (GB):
                    </label>
                    <input
                      type="number"
                      min="1"
                      max="100"
                      value={settings.cache.max_size_gb}
                      onChange={(e) => {
                        const newSettings = {
                          ...settings,
                          cache: { ...settings.cache, max_size_gb: parseInt(e.target.value) || 10 }
                        };
                        setSettings(newSettings);
                      }}
                      style={{
                        width: '100%',
                        padding: '8px',
                        border: '1px solid var(--border-color)',
                        borderRadius: '4px',
                        fontSize: '13px',
                      }}
                      disabled={savingSettings}
                    />
                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                      Maximum total size of cached files. Oldest files are evicted when limit is reached.
                    </small>
                  </div>

                  {/* Eviction Threshold */}
                  <div>
                    <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                      Eviction Threshold (%):
                    </label>
                    <input
                      type="number"
                      min="50"
                      max="95"
                      value={settings.cache.eviction_threshold_percent}
                      onChange={(e) => {
                        const newSettings = {
                          ...settings,
                          cache: { ...settings.cache, eviction_threshold_percent: parseInt(e.target.value) || 80 }
                        };
                        setSettings(newSettings);
                      }}
                      style={{
                        width: '100%',
                        padding: '8px',
                        border: '1px solid var(--border-color)',
                        borderRadius: '4px',
                        fontSize: '13px',
                      }}
                      disabled={savingSettings}
                    />
                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                      Cache is evicted when usage reaches this percentage of max size.
                    </small>
                  </div>

                  {/* Compression */}
                  <div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={settings.cache.enable_compression}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            cache: { ...settings.cache, enable_compression: e.target.checked }
                          };
                          setSettings(newSettings);
                        }}
                        disabled={savingSettings}
                      />
                      <span>
                        <strong>Enable Compression</strong>
                        <br />
                        <small style={{ color: 'var(--text-secondary)' }}>
                          Compress cached files with gzip (saves 60-80% disk space)
                        </small>
                      </span>
                    </label>
                  </div>

                  {/* Memory Mapped Files */}
                  <div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={settings.cache.enable_memory_mapped_files}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            cache: { ...settings.cache, enable_memory_mapped_files: e.target.checked }
                          };
                          setSettings(newSettings);
                        }}
                        disabled={savingSettings}
                      />
                      <span>
                        <strong>Enable Memory-Mapped Files</strong>
                        <br />
                        <small style={{ color: 'var(--text-secondary)' }}>
                          Use memory mapping for large cache files (reduces memory usage)
                        </small>
                      </span>
                    </label>
                  </div>

                  {settings.cache.enable_memory_mapped_files && (
                    <div>
                      <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                        Memory-Mapped Threshold (MB):
                      </label>
                      <input
                        type="number"
                        min="10"
                        max="1000"
                        value={settings.cache.memory_mapped_threshold_mb}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            cache: { ...settings.cache, memory_mapped_threshold_mb: parseInt(e.target.value) || 100 }
                          };
                          setSettings(newSettings);
                        }}
                        style={{
                          width: '100%',
                          padding: '8px',
                          border: '1px solid var(--border-color)',
                          borderRadius: '4px',
                          fontSize: '13px',
                        }}
                        disabled={savingSettings}
                      />
                      <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                        Files larger than this will use memory mapping.
                      </small>
                    </div>
                  )}
                </div>

                <h3 style={{ marginTop: '30px', marginBottom: '15px' }}>Training Settings</h3>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '15px' }}>
                  {/* Streaming Chunk Size */}
                  <div>
                    <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                      Streaming Chunk Size:
                    </label>
                    <input
                      type="number"
                      min="100"
                      max="10000"
                      value={settings.training.streaming_chunk_size}
                      onChange={(e) => {
                        const newSettings = {
                          ...settings,
                          training: { ...settings.training, streaming_chunk_size: parseInt(e.target.value) || 1000 }
                        };
                        setSettings(newSettings);
                      }}
                      style={{
                        width: '100%',
                        padding: '8px',
                        border: '1px solid var(--border-color)',
                        borderRadius: '4px',
                        fontSize: '13px',
                      }}
                      disabled={savingSettings}
                    />
                    <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                      Number of records to process before flushing to disk.
                    </small>
                  </div>

                  {/* Adaptive Memory */}
                  <div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={settings.training.enable_adaptive_memory}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            training: { ...settings.training, enable_adaptive_memory: e.target.checked }
                          };
                          setSettings(newSettings);
                        }}
                        disabled={savingSettings}
                      />
                      <span>
                        <strong>Enable Adaptive Memory Management</strong>
                        <br />
                        <small style={{ color: 'var(--text-secondary)' }}>
                          Automatically adjust chunk sizes based on available system memory
                        </small>
                      </span>
                    </label>
                  </div>

                  {settings.training.enable_adaptive_memory && (
                    <>
                      <div>
                        <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                          Min Chunk Size:
                        </label>
                        <input
                          type="number"
                          min="10"
                          max="1000"
                          value={settings.training.min_chunk_size}
                          onChange={(e) => {
                            const newSettings = {
                              ...settings,
                              training: { ...settings.training, min_chunk_size: parseInt(e.target.value) || 100 }
                            };
                            setSettings(newSettings);
                          }}
                          style={{
                            width: '100%',
                            padding: '8px',
                            border: '1px solid var(--border-color)',
                            borderRadius: '4px',
                            fontSize: '13px',
                          }}
                          disabled={savingSettings}
                        />
                      </div>

                      <div>
                        <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                          Max Chunk Size:
                        </label>
                        <input
                          type="number"
                          min="1000"
                          max="100000"
                          value={settings.training.max_chunk_size}
                          onChange={(e) => {
                            const newSettings = {
                              ...settings,
                              training: { ...settings.training, max_chunk_size: parseInt(e.target.value) || 10000 }
                            };
                            setSettings(newSettings);
                          }}
                          style={{
                            width: '100%',
                            padding: '8px',
                            border: '1px solid var(--border-color)',
                            borderRadius: '4px',
                            fontSize: '13px',
                          }}
                          disabled={savingSettings}
                        />
                      </div>

                      <div>
                        <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                          Memory Pressure Threshold (MB):
                        </label>
                        <input
                          type="number"
                          min="512"
                          max="16384"
                          value={settings.training.memory_pressure_threshold_mb}
                          onChange={(e) => {
                            const newSettings = {
                              ...settings,
                              training: { ...settings.training, memory_pressure_threshold_mb: parseInt(e.target.value) || 2048 }
                            };
                            setSettings(newSettings);
                          }}
                          style={{
                            width: '100%',
                            padding: '8px',
                            border: '1px solid var(--border-color)',
                            borderRadius: '4px',
                            fontSize: '13px',
                          }}
                          disabled={savingSettings}
                        />
                        <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                          When available memory drops below this, reduce chunk sizes.
                        </small>
                      </div>
                    </>
                  )}

                  {/* Progress Tracking */}
                  <div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={settings.training.enable_progress_tracking}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            training: { ...settings.training, enable_progress_tracking: e.target.checked }
                          };
                          setSettings(newSettings);
                        }}
                        disabled={savingSettings}
                      />
                      <span>
                        <strong>Enable Progress Tracking</strong>
                        <br />
                        <small style={{ color: 'var(--text-secondary)' }}>
                          Track and report progress for large cache generation operations
                        </small>
                      </span>
                    </label>
                  </div>

                  {settings.training.enable_progress_tracking && (
                    <div>
                      <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                        Progress Update Interval:
                      </label>
                      <input
                        type="number"
                        min="100"
                        max="10000"
                        value={settings.training.progress_update_interval}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            training: { ...settings.training, progress_update_interval: parseInt(e.target.value) || 1000 }
                          };
                          setSettings(newSettings);
                        }}
                        style={{
                          width: '100%',
                          padding: '8px',
                          border: '1px solid var(--border-color)',
                          borderRadius: '4px',
                          fontSize: '13px',
                        }}
                        disabled={savingSettings}
                      />
                      <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                        Update progress every N records processed.
                      </small>
                    </div>
                  )}

                  {/* Parallel Hashing */}
                  <div>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '10px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={settings.training.enable_parallel_hashing}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            training: { ...settings.training, enable_parallel_hashing: e.target.checked }
                          };
                          setSettings(newSettings);
                        }}
                        disabled={savingSettings}
                      />
                      <span>
                        <strong>Enable Parallel Hash Computation</strong>
                        <br />
                        <small style={{ color: 'var(--text-secondary)' }}>
                          Use multiple CPU cores for hash computation on large datasets
                        </small>
                      </span>
                    </label>
                  </div>

                  {settings.training.enable_parallel_hashing && (
                    <div>
                      <label style={{ display: 'block', marginBottom: '5px', fontSize: '13px', fontWeight: 'bold' }}>
                        Parallel Hash Threshold:
                      </label>
                      <input
                        type="number"
                        min="1000"
                        max="100000"
                        value={settings.training.parallel_hash_threshold}
                        onChange={(e) => {
                          const newSettings = {
                            ...settings,
                            training: { ...settings.training, parallel_hash_threshold: parseInt(e.target.value) || 10000 }
                          };
                          setSettings(newSettings);
                        }}
                        style={{
                          width: '100%',
                          padding: '8px',
                          border: '1px solid var(--border-color)',
                          borderRadius: '4px',
                          fontSize: '13px',
                        }}
                        disabled={savingSettings}
                      />
                      <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                        Use parallel hashing for datasets with more than this many records.
                      </small>
                    </div>
                  )}
                </div>

                <button
                  className="btn btn-primary"
                  onClick={() => saveSettings(settings)}
                  disabled={savingSettings || loadingSettings}
                  style={{ marginTop: '20px', width: '100%' }}
                >
                  {savingSettings ? 'Saving...' : 'Save Settings'}
                </button>
              </div>
            )}

            {/* Global System Prompt (file link) */}
            {settings && (
              <div style={{
                padding: '15px',
                background: 'var(--bg-secondary)',
                borderRadius: '4px',
                marginBottom: '20px',
              }}>
                <h3 style={{ marginTop: 0, marginBottom: '15px' }}>Global System Prompt</h3>
                <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
                  Link a file whose content is prepended to all LLM calls (chat, Coder, Debate, etc.). Path is relative to the project root. Edit the file to change the prompt.
                </p>
                <div style={{ display: 'flex', gap: '10px', marginBottom: '10px', flexWrap: 'wrap' }}>
                  <input
                    type="text"
                    value={settings.global_system_prompt_file ?? ''}
                    onChange={(e) => {
                      const newSettings = {
                        ...settings,
                        global_system_prompt_file: e.target.value || null,
                      };
                      setSettings(newSettings);
                    }}
                    placeholder="e.g. IDE_prompt.txt (relative to project root)"
                    style={{
                      flex: 1,
                      minWidth: '200px',
                      padding: '8px 10px',
                      border: '1px solid var(--border-color)',
                      borderRadius: '4px',
                      fontSize: '13px',
                    }}
                    disabled={savingSettings}
                  />
                  <button
                    className="btn btn-secondary"
                    onClick={() => {
                      const newSettings = {
                        ...settings,
                        global_system_prompt_file: 'IDE_prompt.txt',
                      };
                      setSettings(newSettings);
                      setMessage('Set to IDE_prompt.txt');
                      setTimeout(() => setMessage(null), 3000);
                    }}
                    disabled={savingSettings || loadingSettings}
                  >
                    Use IDE_prompt.txt
                  </button>
                  <button
                    className="btn btn-primary"
                    onClick={() => saveSettings(settings)}
                    disabled={savingSettings || loadingSettings}
                  >
                    {savingSettings ? 'Saving...' : 'Save'}
                  </button>
                </div>
                <small style={{ color: 'var(--text-secondary)', fontSize: '12px' }}>
                  The file is read at runtime. Changes to the file take effect on the next LLM call.
                </small>
              </div>
            )}

            {/* Auto-Training Settings */}
            {settings && (
              <div style={{
                padding: '15px',
                background: 'var(--bg-secondary)',
                borderRadius: '4px',
                marginBottom: '20px',
              }}>
                <h3 style={{ marginTop: 0, marginBottom: '15px' }}>Auto-Training</h3>
                <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
                  Control whether different parts of the app automatically convert interactions into training examples.
                </p>

                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={settings.auto_training?.auto_training_enabled ?? true}
                      onChange={(e) => {
                        const newSettings: AppSettings = {
                          ...settings,
                          auto_training: {
                            ...settings.auto_training,
                            auto_training_enabled: e.target.checked,
                            train_from_chat: settings.auto_training?.train_from_chat ?? true,
                            train_from_coder: settings.auto_training?.train_from_coder ?? true,
                            train_from_debate: settings.auto_training?.train_from_debate ?? true,
                          },
                        };
                        setSettings(newSettings);
                      }}
                      disabled={savingSettings}
                    />
                    <span><strong>Enable Auto-Training</strong> (global master switch)</span>
                  </label>

                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', marginLeft: '20px' }}>
                    <input
                      type="checkbox"
                      checked={settings.auto_training?.train_from_chat ?? true}
                      onChange={(e) => {
                        const newSettings: AppSettings = {
                          ...settings,
                          auto_training: {
                            ...settings.auto_training,
                            train_from_chat: e.target.checked,
                          },
                        };
                        setSettings(newSettings);
                      }}
                      disabled={savingSettings || !settings.auto_training?.auto_training_enabled}
                    />
                    <span>Profile Chat → training data</span>
                  </label>

                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', marginLeft: '20px' }}>
                    <input
                      type="checkbox"
                      checked={settings.auto_training?.train_from_coder ?? true}
                      onChange={(e) => {
                        const newSettings: AppSettings = {
                          ...settings,
                          auto_training: {
                            ...settings.auto_training,
                            train_from_coder: e.target.checked,
                          },
                        };
                        setSettings(newSettings);
                      }}
                      disabled={savingSettings || !settings.auto_training?.auto_training_enabled}
                    />
                    <span>Coder IDE → training data</span>
                  </label>

                  <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', marginLeft: '20px' }}>
                    <input
                      type="checkbox"
                      checked={settings.auto_training?.train_from_debate ?? true}
                      onChange={(e) => {
                        const newSettings: AppSettings = {
                          ...settings,
                          auto_training: {
                            ...settings.auto_training,
                            train_from_debate: e.target.checked,
                          },
                        };
                        setSettings(newSettings);
                      }}
                      disabled={savingSettings || !settings.auto_training?.auto_training_enabled}
                    />
                    <span>Debate Room → training data</span>
                  </label>
                </div>

                <div style={{ marginTop: '15px', display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
                  <button
                    className="btn btn-primary"
                    onClick={() => saveSettings(settings)}
                    disabled={savingSettings || loadingSettings}
                  >
                    {savingSettings ? 'Saving...' : 'Save Auto-Training Settings'}
                  </button>
                </div>
              </div>
            )}

            {loadingSettings && (
              <div className="card">
                <p>Loading settings...</p>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

// Training Cache Modal
export function TrainingCacheModal({ onClose }: { onClose: () => void }) {
  return (
    <ModalWrapper title="Training Data Cache" onClose={onClose}>
      <TrainingCacheTab />
    </ModalWrapper>
  );
}

// Token Usage Modal
interface TokenUsageEntry {
  provider_id?: string | null;
  model_name: string;
  source: string;
  total_prompt_tokens: number;
  total_completion_tokens: number;
  total_tokens: number;
  first_used_at: string;
  last_used_at: string;
}

export function TokenUsageModal({ onClose }: { onClose: () => void }) {
  const [entries, setEntries] = useState<TokenUsageEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [resetting, setResetting] = useState(false);

  useEffect(() => {
    loadSummary();
  }, []);

  const loadSummary = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.getTokenUsageSummary();
      setEntries(data || []);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setLoading(false);
    }
  };

  const resetAll = async () => {
    if (!confirm('Reset all token usage counters? This will delete all usage records.')) return;
    setResetting(true);
    try {
      await api.resetTokenUsage();
      await loadSummary();
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setResetting(false);
    }
  };

  const resetEntry = async (entry: TokenUsageEntry) => {
    if (!confirm(`Reset usage for model "${entry.model_name}"${entry.provider_id ? ` (provider ${entry.provider_id})` : ''}?`)) {
      return;
    }
    setResetting(true);
    try {
      await api.resetTokenUsage(entry.provider_id ?? undefined, entry.model_name);
      await loadSummary();
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setResetting(false);
    }
  };

  const formatDate = (iso: string) => {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  };

  return (
    <ModalWrapper title="Token Usage" onClose={onClose}>
      <div>
        <p style={{ color: 'var(--text-secondary)', marginBottom: '15px', fontSize: '13px' }}>
          View and reset token usage per provider/model/source. These stats are computed locally from the <code>token_usage</code> table.
        </p>

        {error && (
          <div style={{
            padding: '10px',
            background: '#ffebee',
            border: '1px solid #f44336',
            borderRadius: '4px',
            marginBottom: '10px',
            color: '#c62828',
            fontSize: '13px',
          }}>
            {error}
          </div>
        )}

        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '10px', alignItems: 'center' }}>
          <button
            className="btn btn-secondary"
            onClick={loadSummary}
            disabled={loading}
          >
            {loading ? 'Refreshing...' : 'Refresh'}
          </button>
          <button
            className="btn btn-danger"
            onClick={resetAll}
            disabled={resetting || loading || entries.length === 0}
          >
            {resetting ? 'Resetting...' : 'Reset All'}
          </button>
        </div>

        {loading && (
          <div className="card">
            <p>Loading token usage...</p>
          </div>
        )}

        {!loading && entries.length === 0 && (
          <div className="card">
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
              No token usage has been recorded yet.
            </p>
          </div>
        )}

        {!loading && entries.length > 0 && (
          <div className="card" style={{ maxHeight: '60vh', overflowY: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '13px' }}>
              <thead>
                <tr style={{ textAlign: 'left', borderBottom: '1px solid var(--border-color)' }}>
                  <th style={{ padding: '8px' }}>Provider</th>
                  <th style={{ padding: '8px' }}>Model</th>
                  <th style={{ padding: '8px' }}>Source</th>
                  <th style={{ padding: '8px' }}>Prompt</th>
                  <th style={{ padding: '8px' }}>Completion</th>
                  <th style={{ padding: '8px' }}>Total</th>
                  <th style={{ padding: '8px' }}>Last Used</th>
                  <th style={{ padding: '8px' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry, idx) => (
                  <tr key={idx} style={{ borderBottom: '1px solid var(--border-color)' }}>
                    <td style={{ padding: '8px', whiteSpace: 'nowrap' }}>
                      {entry.provider_id || 'local/unknown'}
                    </td>
                    <td style={{ padding: '8px', whiteSpace: 'nowrap' }}>{entry.model_name}</td>
                    <td style={{ padding: '8px', whiteSpace: 'nowrap' }}>{entry.source}</td>
                    <td style={{ padding: '8px' }}>{entry.total_prompt_tokens.toLocaleString()}</td>
                    <td style={{ padding: '8px' }}>{entry.total_completion_tokens.toLocaleString()}</td>
                    <td style={{ padding: '8px', fontWeight: 'bold' }}>{entry.total_tokens.toLocaleString()}</td>
                    <td style={{ padding: '8px', whiteSpace: 'nowrap' }}>{formatDate(entry.last_used_at)}</td>
                    <td style={{ padding: '8px' }}>
                      <button
                        className="btn btn-secondary"
                        style={{ fontSize: '11px', padding: '4px 8px' }}
                        onClick={() => resetEntry(entry)}
                        disabled={resetting}
                      >
                        Reset
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </ModalWrapper>
  );
}
