import { useState, useEffect } from 'react';
import { api } from '../api';

interface LoraTrainingModalProps {
  isOpen: boolean;
  onClose: () => void;
  onStartTraining: (config: LoraTrainingConfig) => void;
  model: any;
  projectId: string;
  trainingDataCount: number;
}

export interface LoraTrainingConfig {
  num_train_epochs: number;
  learning_rate: number;
  per_device_train_batch_size: number;
  gradient_accumulation_steps: number;
  warmup_ratio: number;
  weight_decay: number;
  max_seq_length: number;
  fp16: boolean;
  bf16: boolean;
  save_steps: number;
  logging_steps: number;
  save_total_limit: number;
  lora_config: {
    use_lora: boolean;
    use_qlora: boolean;
    lora_rank: number;
    lora_alpha: number;
    lora_dropout: number;
    target_modules: string[];
    bias: string;
    task_type: string;
  };
  max_train_samples?: number;
  max_tokens_per_sample?: number;
  use_8bit_adam: boolean;
  gradient_checkpointing: boolean;
  dry_run: boolean;
  dataset_format: string;
  prompt_template?: string;
  eval_split_ratio: number;
}

const DEFAULT_CONFIG: LoraTrainingConfig = {
  num_train_epochs: 3,
  learning_rate: 0.0002,
  per_device_train_batch_size: 4,
  gradient_accumulation_steps: 4,
  warmup_ratio: 0.03,
  weight_decay: 0.001,
  max_seq_length: 2048,
  fp16: true,
  bf16: false,
  save_steps: 100,
  logging_steps: 10,
  save_total_limit: 3,
  lora_config: {
    use_lora: true,
    use_qlora: false,
    lora_rank: 16,
    lora_alpha: 32,
    lora_dropout: 0.05,
    target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj'],
    bias: 'none',
    task_type: 'CAUSAL_LM',
  },
  max_train_samples: 50000,
  max_tokens_per_sample: 4096,
  use_8bit_adam: false,
  gradient_checkpointing: true,
  dry_run: false,
  dataset_format: 'completion',
  eval_split_ratio: 0,
};

// Preset configurations
const PRESETS: Record<string, Partial<LoraTrainingConfig>> = {
  quick: {
    num_train_epochs: 1,
    per_device_train_batch_size: 2,
    gradient_accumulation_steps: 2,
    max_seq_length: 512,
    lora_config: {
      use_lora: true,
      use_qlora: false,
      lora_rank: 8,
      lora_alpha: 16,
      lora_dropout: 0.1,
      target_modules: ['q_proj', 'v_proj'],
      bias: 'none',
      task_type: 'CAUSAL_LM',
    },
  },
  balanced: {
    num_train_epochs: 3,
    per_device_train_batch_size: 4,
    gradient_accumulation_steps: 4,
    max_seq_length: 2048,
    lora_config: {
      use_lora: true,
      use_qlora: false,
      lora_rank: 16,
      lora_alpha: 32,
      lora_dropout: 0.05,
      target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj'],
      bias: 'none',
      task_type: 'CAUSAL_LM',
    },
  },
  thorough: {
    num_train_epochs: 5,
    per_device_train_batch_size: 2,
    gradient_accumulation_steps: 8,
    max_seq_length: 4096,
    lora_config: {
      use_lora: true,
      use_qlora: false,
      lora_rank: 32,
      lora_alpha: 64,
      lora_dropout: 0.05,
      target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj', 'gate_proj', 'up_proj', 'down_proj'],
      bias: 'none',
      task_type: 'CAUSAL_LM',
    },
  },
  qlora_efficient: {
    num_train_epochs: 3,
    per_device_train_batch_size: 4,
    gradient_accumulation_steps: 4,
    max_seq_length: 2048,
    lora_config: {
      use_lora: true,
      use_qlora: true,
      lora_rank: 16,
      lora_alpha: 32,
      lora_dropout: 0.05,
      target_modules: ['q_proj', 'v_proj', 'k_proj', 'o_proj'],
      bias: 'none',
      task_type: 'CAUSAL_LM',
    },
    use_8bit_adam: true,
    gradient_checkpointing: true,
  },
};

export function LoraTrainingModal({
  isOpen,
  onClose,
  onStartTraining,
  model,
  projectId,
  trainingDataCount,
}: LoraTrainingModalProps) {
  const [config, setConfig] = useState<LoraTrainingConfig>(DEFAULT_CONFIG);
  const [activeTab, setActiveTab] = useState<'basic' | 'lora' | 'advanced'>('basic');
  const [envCheck, setEnvCheck] = useState<any>(null);
  const [checkingEnv, setCheckingEnv] = useState(false);
  const [selectedPreset, setSelectedPreset] = useState<string>('balanced');

  useEffect(() => {
    if (isOpen) {
      // Check training environment when modal opens
      checkEnvironment();
    }
  }, [isOpen]);

  const checkEnvironment = async () => {
    setCheckingEnv(true);
    try {
      const result = await api.checkTrainingReadiness();
      setEnvCheck(result);
    } catch (error) {
      console.error('Failed to check training environment:', error);
    } finally {
      setCheckingEnv(false);
    }
  };

  const applyPreset = (presetName: string) => {
    const preset = PRESETS[presetName];
    if (preset) {
      setConfig(prev => ({
        ...DEFAULT_CONFIG,
        ...preset,
        lora_config: {
          ...DEFAULT_CONFIG.lora_config,
          ...(preset.lora_config || {}),
        },
      }));
      setSelectedPreset(presetName);
    }
  };

  const handleStartTraining = () => {
    onStartTraining(config);
    onClose();
  };

  if (!isOpen) return null;

  const estimatedMemoryGB = config.lora_config.use_qlora 
    ? 4 + (config.per_device_train_batch_size * 0.5)
    : 8 + (config.per_device_train_batch_size * 1);

  const gpuAvailable = envCheck?.gpu_available;
  const gpuMemory = envCheck?.gpu_memory_gb || 0;
  const memoryWarning = gpuAvailable && estimatedMemoryGB > gpuMemory * 0.8;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div 
        className="modal-content" 
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: '700px', maxHeight: '85vh', overflow: 'auto' }}
      >
        <div className="modal-header">
          <h2>🎯 LoRA Training Configuration</h2>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>

        {/* Environment Status */}
        <div style={{ 
          padding: '12px 16px', 
          background: envCheck?.ready ? 'rgba(40, 167, 69, 0.1)' : 'rgba(255, 193, 7, 0.1)',
          borderRadius: '6px',
          marginBottom: '16px',
          border: `1px solid ${envCheck?.ready ? '#28a745' : '#ffc107'}`
        }}>
          {checkingEnv ? (
            <span>⏳ Checking training environment...</span>
          ) : envCheck ? (
            <div>
              <div style={{ fontWeight: 600, marginBottom: '4px' }}>
                {envCheck.ready ? '✅ Environment Ready' : '⚠️ Environment Needs Setup'}
              </div>
              <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                {envCheck.gpu_available ? (
                  <>🎮 GPU: {envCheck.gpu_name} ({envCheck.gpu_memory_gb?.toFixed(1)}GB)</>
                ) : (
                  <>💻 CPU only (training will be slow)</>
                )}
                {envCheck.missing_packages?.length > 0 && (
                  <div style={{ color: '#dc3545', marginTop: '4px' }}>
                    Missing: {envCheck.missing_packages.join(', ')}
                  </div>
                )}
              </div>
            </div>
          ) : (
            <span>Unable to check environment</span>
          )}
        </div>

        {/* Model Info */}
        <div style={{ 
          padding: '12px 16px', 
          background: 'var(--bg-secondary)',
          borderRadius: '6px',
          marginBottom: '16px',
          fontSize: '14px'
        }}>
          <strong>Model:</strong> {model?.name || 'Unknown'}<br />
          <strong>Base:</strong> {model?.base_model || 'Unknown'}<br />
          <strong>Training Data:</strong> {trainingDataCount} examples
        </div>

        {/* Presets */}
        <div style={{ marginBottom: '16px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: 600 }}>
            Training Preset
          </label>
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {Object.entries(PRESETS).map(([name, _]) => (
              <button
                key={name}
                onClick={() => applyPreset(name)}
                className={selectedPreset === name ? 'btn btn-primary' : 'btn btn-secondary'}
                style={{ padding: '6px 12px', fontSize: '13px' }}
              >
                {name === 'quick' && '⚡ Quick'}
                {name === 'balanced' && '⚖️ Balanced'}
                {name === 'thorough' && '🔬 Thorough'}
                {name === 'qlora_efficient' && '💾 QLoRA (Low Memory)'}
              </button>
            ))}
          </div>
        </div>

        {/* Tabs */}
        <div style={{ 
          display: 'flex', 
          borderBottom: '1px solid var(--border-color)',
          marginBottom: '16px'
        }}>
          {(['basic', 'lora', 'advanced'] as const).map((tab) => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              style={{
                padding: '8px 16px',
                background: 'transparent',
                border: 'none',
                borderBottom: activeTab === tab ? '2px solid var(--primary-color)' : '2px solid transparent',
                cursor: 'pointer',
                fontWeight: activeTab === tab ? 600 : 400,
                color: activeTab === tab ? 'var(--primary-color)' : 'var(--text-secondary)',
              }}
            >
              {tab === 'basic' && '📊 Basic'}
              {tab === 'lora' && '🔧 LoRA'}
              {tab === 'advanced' && '⚙️ Advanced'}
            </button>
          ))}
        </div>

        {/* Tab Content */}
        <div style={{ minHeight: '280px' }}>
          {activeTab === 'basic' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Epochs
                </label>
                <input
                  type="number"
                  min="1"
                  max="20"
                  value={config.num_train_epochs}
                  onChange={(e) => setConfig(prev => ({ ...prev, num_train_epochs: parseFloat(e.target.value) || 1 }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Number of training passes</span>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Learning Rate
                </label>
                <input
                  type="number"
                  step="0.00001"
                  min="0.000001"
                  max="0.01"
                  value={config.learning_rate}
                  onChange={(e) => setConfig(prev => ({ ...prev, learning_rate: parseFloat(e.target.value) || 0.0002 }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Typical: 1e-4 to 3e-4</span>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Batch Size
                </label>
                <select
                  value={config.per_device_train_batch_size}
                  onChange={(e) => setConfig(prev => ({ ...prev, per_device_train_batch_size: parseInt(e.target.value) }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="1">1 (Low memory)</option>
                  <option value="2">2</option>
                  <option value="4">4 (Recommended)</option>
                  <option value="8">8 (High memory)</option>
                  <option value="16">16 (Very high memory)</option>
                </select>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Max Sequence Length
                </label>
                <select
                  value={config.max_seq_length}
                  onChange={(e) => setConfig(prev => ({ ...prev, max_seq_length: parseInt(e.target.value) }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="512">512 (Fast)</option>
                  <option value="1024">1024</option>
                  <option value="2048">2048 (Recommended)</option>
                  <option value="4096">4096 (Long context)</option>
                  <option value="8192">8192 (Very long)</option>
                </select>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Dataset Format
                </label>
                <select
                  value={config.dataset_format}
                  onChange={(e) => setConfig(prev => ({ ...prev, dataset_format: e.target.value }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="completion">Completion (Input → Output)</option>
                  <option value="alpaca">Alpaca (Instruction format)</option>
                  <option value="sharegpt">ShareGPT (Chat format)</option>
                </select>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Gradient Accumulation
                </label>
                <select
                  value={config.gradient_accumulation_steps}
                  onChange={(e) => setConfig(prev => ({ ...prev, gradient_accumulation_steps: parseInt(e.target.value) }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="1">1</option>
                  <option value="2">2</option>
                  <option value="4">4 (Recommended)</option>
                  <option value="8">8</option>
                  <option value="16">16</option>
                </select>
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Higher = less memory, larger effective batch</span>
              </div>
            </div>
          )}

          {activeTab === 'lora' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
              <div style={{ gridColumn: 'span 2' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.lora_config.use_lora}
                    onChange={(e) => setConfig(prev => ({
                      ...prev,
                      lora_config: { ...prev.lora_config, use_lora: e.target.checked }
                    }))}
                  />
                  <span style={{ fontWeight: 500 }}>Enable LoRA</span>
                  <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                    (Parameter-efficient fine-tuning)
                  </span>
                </label>
              </div>

              <div style={{ gridColumn: 'span 2' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.lora_config.use_qlora}
                    onChange={(e) => setConfig(prev => ({
                      ...prev,
                      lora_config: { ...prev.lora_config, use_qlora: e.target.checked }
                    }))}
                  />
                  <span style={{ fontWeight: 500 }}>Enable QLoRA (4-bit)</span>
                  <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                    (Reduces memory ~4x, requires bitsandbytes)
                  </span>
                </label>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  LoRA Rank (r)
                </label>
                <select
                  value={config.lora_config.lora_rank}
                  onChange={(e) => setConfig(prev => ({
                    ...prev,
                    lora_config: { ...prev.lora_config, lora_rank: parseInt(e.target.value) }
                  }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="4">4 (Minimal)</option>
                  <option value="8">8 (Light)</option>
                  <option value="16">16 (Recommended)</option>
                  <option value="32">32 (More capacity)</option>
                  <option value="64">64 (High capacity)</option>
                </select>
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Higher = more parameters, better quality</span>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  LoRA Alpha
                </label>
                <select
                  value={config.lora_config.lora_alpha}
                  onChange={(e) => setConfig(prev => ({
                    ...prev,
                    lora_config: { ...prev.lora_config, lora_alpha: parseInt(e.target.value) }
                  }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="8">8</option>
                  <option value="16">16</option>
                  <option value="32">32 (Recommended)</option>
                  <option value="64">64</option>
                </select>
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Scaling factor (typically 2× rank)</span>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  LoRA Dropout
                </label>
                <input
                  type="number"
                  step="0.01"
                  min="0"
                  max="0.5"
                  value={config.lora_config.lora_dropout}
                  onChange={(e) => setConfig(prev => ({
                    ...prev,
                    lora_config: { ...prev.lora_config, lora_dropout: parseFloat(e.target.value) || 0 }
                  }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Regularization (0.05-0.1 typical)</span>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Target Modules
                </label>
                <select
                  value={config.lora_config.target_modules.join(',')}
                  onChange={(e) => setConfig(prev => ({
                    ...prev,
                    lora_config: { ...prev.lora_config, target_modules: e.target.value.split(',') }
                  }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                >
                  <option value="q_proj,v_proj">Attention (q,v) - Minimal</option>
                  <option value="q_proj,v_proj,k_proj,o_proj">Attention (q,k,v,o) - Recommended</option>
                  <option value="q_proj,v_proj,k_proj,o_proj,gate_proj,up_proj,down_proj">All projections - Thorough</option>
                </select>
              </div>
            </div>
          )}

          {activeTab === 'advanced' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
              <div style={{ gridColumn: 'span 2' }}>
                <h4 style={{ margin: '0 0 12px 0', fontSize: '14px' }}>Memory Optimization</h4>
              </div>

              <div>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.gradient_checkpointing}
                    onChange={(e) => setConfig(prev => ({ ...prev, gradient_checkpointing: e.target.checked }))}
                  />
                  <span>Gradient Checkpointing</span>
                </label>
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)', marginLeft: '24px' }}>
                  Reduces memory, slightly slower
                </span>
              </div>

              <div>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.use_8bit_adam}
                    onChange={(e) => setConfig(prev => ({ ...prev, use_8bit_adam: e.target.checked }))}
                  />
                  <span>8-bit Adam Optimizer</span>
                </label>
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)', marginLeft: '24px' }}>
                  Requires bitsandbytes
                </span>
              </div>

              <div>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.fp16}
                    onChange={(e) => setConfig(prev => ({ ...prev, fp16: e.target.checked, bf16: e.target.checked ? false : prev.bf16 }))}
                  />
                  <span>FP16 (Half precision)</span>
                </label>
              </div>

              <div>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.bf16}
                    onChange={(e) => setConfig(prev => ({ ...prev, bf16: e.target.checked, fp16: e.target.checked ? false : prev.fp16 }))}
                  />
                  <span>BF16 (Better precision)</span>
                </label>
              </div>

              <div style={{ gridColumn: 'span 2', marginTop: '12px' }}>
                <h4 style={{ margin: '0 0 12px 0', fontSize: '14px' }}>Safety Limits</h4>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Max Training Samples
                </label>
                <input
                  type="number"
                  min="100"
                  max="1000000"
                  value={config.max_train_samples || ''}
                  onChange={(e) => setConfig(prev => ({ 
                    ...prev, 
                    max_train_samples: e.target.value ? parseInt(e.target.value) : undefined 
                  }))}
                  placeholder="No limit"
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Eval Split Ratio
                </label>
                <input
                  type="number"
                  step="0.05"
                  min="0"
                  max="0.3"
                  value={config.eval_split_ratio}
                  onChange={(e) => setConfig(prev => ({ ...prev, eval_split_ratio: parseFloat(e.target.value) || 0 }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>0 = no eval set, 0.1 = 10% for eval</span>
              </div>

              <div style={{ gridColumn: 'span 2', marginTop: '12px' }}>
                <h4 style={{ margin: '0 0 12px 0', fontSize: '14px' }}>Checkpointing</h4>
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Save Every N Steps
                </label>
                <input
                  type="number"
                  min="10"
                  max="1000"
                  value={config.save_steps}
                  onChange={(e) => setConfig(prev => ({ ...prev, save_steps: parseInt(e.target.value) || 100 }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
              </div>

              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                  Max Checkpoints
                </label>
                <input
                  type="number"
                  min="1"
                  max="10"
                  value={config.save_total_limit}
                  onChange={(e) => setConfig(prev => ({ ...prev, save_total_limit: parseInt(e.target.value) || 3 }))}
                  style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                />
              </div>

              <div style={{ gridColumn: 'span 2', marginTop: '12px' }}>
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={config.dry_run}
                    onChange={(e) => setConfig(prev => ({ ...prev, dry_run: e.target.checked }))}
                  />
                  <span style={{ fontWeight: 500 }}>🧪 Dry Run (Estimate only)</span>
                </label>
                <span style={{ fontSize: '12px', color: 'var(--text-secondary)', marginLeft: '24px' }}>
                  Check memory requirements without actual training
                </span>
              </div>
            </div>
          )}
        </div>

        {/* Memory Warning */}
        {memoryWarning && (
          <div style={{
            marginTop: '16px',
            padding: '12px',
            background: 'rgba(255, 193, 7, 0.15)',
            border: '1px solid #ffc107',
            borderRadius: '6px',
            fontSize: '13px'
          }}>
            ⚠️ <strong>Memory Warning:</strong> Estimated {estimatedMemoryGB.toFixed(1)}GB needed, 
            GPU has {gpuMemory.toFixed(1)}GB. Consider enabling QLoRA or reducing batch size.
          </div>
        )}

        {/* Footer */}
        <div style={{ 
          display: 'flex', 
          justifyContent: 'space-between', 
          alignItems: 'center',
          marginTop: '20px',
          paddingTop: '16px',
          borderTop: '1px solid var(--border-color)'
        }}>
          <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
            Est. memory: ~{estimatedMemoryGB.toFixed(1)}GB
            {config.lora_config.use_qlora && ' (QLoRA)'}
          </div>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button className="btn btn-secondary" onClick={onClose}>
              Cancel
            </button>
            <button 
              className="btn btn-primary" 
              onClick={handleStartTraining}
              disabled={!envCheck?.ready && !config.dry_run}
            >
              {config.dry_run ? '🧪 Run Dry Test' : '🚀 Start Training'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
