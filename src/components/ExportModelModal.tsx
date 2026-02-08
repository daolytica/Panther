import { useState, useEffect } from 'react';
import { api } from '../api';

interface ExportModelModalProps {
  isOpen: boolean;
  onClose: () => void;
  model: any;
  onExportComplete?: () => void;
}

type ExportFormat = 'huggingface' | 'ollama';

export function ExportModelModal({
  isOpen,
  onClose,
  model,
  onExportComplete,
}: ExportModelModalProps) {
  const [exportOptions, setExportOptions] = useState<any>(null);
  const [loading, setLoading] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<any>(null);
  const [format, setFormat] = useState<ExportFormat>('huggingface');
  const [mergeAdapters, setMergeAdapters] = useState(false);
  const [ollamaModelName, setOllamaModelName] = useState('');
  const [ollamaSystemPrompt, setOllamaSystemPrompt] = useState('');
  const [quantizationOptions, setQuantizationOptions] = useState<any>(null);
  const [convertingToGguf, setConvertingToGguf] = useState(false);
  const [selectedQuantization, setSelectedQuantization] = useState('q4_k_m');

  const isTrained = model?.training_status === 'complete' || model?.training_status === 'completed';

  useEffect(() => {
    if (isOpen && model?.id) {
      loadExportOptions();
    }
  }, [isOpen, model?.id]);

  useEffect(() => {
    if (isOpen) {
      api.getGgufQuantizationOptions().then(setQuantizationOptions).catch(() => {});
    }
  }, [isOpen]);

  const loadExportOptions = async () => {
    setLoading(true);
    setError(null);
    try {
      const opts = await api.checkExportOptions(model.id);
      setExportOptions(opts);
      setOllamaModelName(`panther-${(model?.name || 'model').toLowerCase().replace(/\s+/g, '-')}`);
    } catch (err: any) {
      setError(err?.message || 'Failed to check export options');
    } finally {
      setLoading(false);
    }
  };

  const handleExport = async () => {
    setExporting(true);
    setError(null);
    setResult(null);
    try {
      const request = {
        model_id: model.id,
        export_format: format,
        export_path: undefined,
        merge_adapters: mergeAdapters,
        quantization: undefined,
        ollama_model_name: format === 'ollama' ? ollamaModelName : undefined,
        ollama_system_prompt: format === 'ollama' ? (ollamaSystemPrompt || undefined) : undefined,
      };

      if (format === 'huggingface') {
        const res = await api.exportModelHuggingface(request);
        setResult(res);
      } else {
        const res = await api.exportModelOllama(request);
        setResult(res);
      }
      onExportComplete?.();
    } catch (err: any) {
      setError(err?.message || 'Export failed');
    } finally {
      setExporting(false);
    }
  };

  if (!isOpen) return null;

  const hfAvailable = exportOptions?.export_options?.huggingface?.available ?? false;
  const ollamaAvailable = exportOptions?.export_options?.ollama?.available ?? false;
  const ggufRequired = exportOptions?.export_options?.ollama?.gguf_required ?? true;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: '520px' }}
      >
        <div className="modal-header">
          <h2>📤 Export Model</h2>
          <button className="modal-close" onClick={onClose}>×</button>
        </div>

        {loading ? (
          <div style={{ padding: '24px', textAlign: 'center', color: 'var(--text-secondary)' }}>
            Checking export options...
          </div>
        ) : (
          <>
            <div style={{
              padding: '12px 16px',
              background: 'var(--bg-secondary)',
              borderRadius: '6px',
              marginBottom: '16px',
              fontSize: '14px',
            }}>
              <strong>{model?.name}</strong> (base: {model?.base_model})
            </div>

            {!isTrained && (
              <div style={{
                padding: '12px',
                background: 'rgba(255, 193, 7, 0.15)',
                border: '1px solid #ffc107',
                borderRadius: '6px',
                marginBottom: '16px',
                fontSize: '13px',
              }}>
                ⚠️ Model must be trained before export. Current status: {model?.training_status || 'unknown'}
              </div>
            )}

            {exportOptions && isTrained && (
              <>
                <div style={{ marginBottom: '16px' }}>
                  <label style={{ display: 'block', marginBottom: '8px', fontWeight: 600 }}>
                    Export Format
                  </label>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <label style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: '10px',
                      padding: '12px',
                      border: `2px solid ${format === 'huggingface' ? 'var(--primary-color)' : 'var(--border-color)'}`,
                      borderRadius: '6px',
                      cursor: hfAvailable ? 'pointer' : 'not-allowed',
                      opacity: hfAvailable ? 1 : 0.6,
                    }}
                    onClick={() => hfAvailable && setFormat('huggingface')}
                    >
                      <input
                        type="radio"
                        name="format"
                        checked={format === 'huggingface'}
                        onChange={() => setFormat('huggingface')}
                        disabled={!hfAvailable}
                      />
                      <div>
                        <strong>HuggingFace (ZIP)</strong>
                        <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                          Export model files as a zip archive with training_config.json. Use for sharing or further training.
                        </div>
                      </div>
                    </label>

                    <label style={{
                      display: 'flex',
                      alignItems: 'flex-start',
                      gap: '10px',
                      padding: '12px',
                      border: `2px solid ${format === 'ollama' ? 'var(--primary-color)' : 'var(--border-color)'}`,
                      borderRadius: '6px',
                      cursor: ollamaAvailable ? 'pointer' : 'not-allowed',
                      opacity: ollamaAvailable ? 1 : 0.6,
                    }}
                    onClick={() => ollamaAvailable && setFormat('ollama')}
                    >
                      <input
                        type="radio"
                        name="format"
                        checked={format === 'ollama'}
                        onChange={() => setFormat('ollama')}
                        disabled={!ollamaAvailable}
                      />
                      <div>
                        <strong>Ollama</strong>
                        <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                          Register as Ollama model for local inference. Requires GGUF conversion first.
                          {ggufRequired && (
                            <span style={{ color: '#dc3545', display: 'block', marginTop: '4px' }}>
                              ⚠️ GGUF file not found. Convert to GGUF first using the button below.
                            </span>
                          )}
                        </div>
                      </div>
                    </label>
                  </div>
                </div>

                {format === 'huggingface' && (
                  <div style={{ marginBottom: '16px' }}>
                    <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={mergeAdapters}
                        onChange={(e) => setMergeAdapters(e.target.checked)}
                      />
                      <span>Merge LoRA adapters into base model (larger file, standalone)</span>
                    </label>
                  </div>
                )}

                {format === 'ollama' && exportOptions?.export_options?.gguf?.available && ggufRequired && (
                  <div style={{
                    padding: '12px',
                    background: 'rgba(255, 193, 7, 0.1)',
                    border: '1px solid #ffc107',
                    borderRadius: '6px',
                    marginBottom: '16px',
                  }}>
                    <strong>Convert to GGUF first</strong>
                    <div style={{ display: 'flex', gap: '8px', marginTop: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                      {quantizationOptions?.options && (
                        <select
                          value={selectedQuantization}
                          onChange={(e) => setSelectedQuantization(e.target.value)}
                          style={{ padding: '6px 10px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
                        >
                          {quantizationOptions.options.map((opt: any) => (
                            <option key={opt.id} value={opt.id}>
                              {opt.name} {opt.recommended ? '(recommended)' : ''}
                            </option>
                          ))}
                        </select>
                      )}
                      <button
                        className="btn btn-secondary"
                        onClick={async () => {
                          setConvertingToGguf(true);
                          setError(null);
                          try {
                            await api.convertModelToGguf(model.id, selectedQuantization);
                            await loadExportOptions();
                          } catch (err: any) {
                            setError(err?.message || 'GGUF conversion failed');
                          } finally {
                            setConvertingToGguf(false);
                          }
                        }}
                        disabled={convertingToGguf}
                      >
                        {convertingToGguf ? '⏳ Converting...' : '🔄 Convert to GGUF'}
                      </button>
                    </div>
                  </div>
                )}

                {format === 'ollama' && ollamaAvailable && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '16px' }}>
                    <div>
                      <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                        Ollama Model Name
                      </label>
                      <input
                        type="text"
                        value={ollamaModelName}
                        onChange={(e) => setOllamaModelName(e.target.value)}
                        placeholder="panther-my-model"
                        style={{
                          width: '100%',
                          padding: '8px 12px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                          background: 'var(--card-bg)',
                          color: 'var(--text-primary)',
                        }}
                      />
                    </div>
                    <div>
                      <label style={{ display: 'block', marginBottom: '4px', fontSize: '13px', fontWeight: 500 }}>
                        System Prompt (optional)
                      </label>
                      <textarea
                        value={ollamaSystemPrompt}
                        onChange={(e) => setOllamaSystemPrompt(e.target.value)}
                        placeholder="You are a helpful assistant..."
                        rows={3}
                        style={{
                          width: '100%',
                          padding: '8px 12px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                          background: 'var(--card-bg)',
                          color: 'var(--text-primary)',
                          resize: 'vertical',
                        }}
                      />
                    </div>
                  </div>
                )}

                {error && (
                  <div style={{
                    padding: '12px',
                    background: 'rgba(220, 53, 69, 0.1)',
                    border: '1px solid #dc3545',
                    borderRadius: '6px',
                    marginBottom: '16px',
                    fontSize: '13px',
                    color: '#dc3545',
                  }}>
                    {error}
                  </div>
                )}

                {result && (
                  <div style={{
                    padding: '12px',
                    background: 'rgba(40, 167, 69, 0.1)',
                    border: '1px solid #28a745',
                    borderRadius: '6px',
                    marginBottom: '16px',
                    fontSize: '13px',
                  }}>
                    ✅ {result.message}
                    {result.export_path && (
                      <div style={{ marginTop: '8px', wordBreak: 'break-all' }}>
                        Path: {result.export_path}
                      </div>
                    )}
                  </div>
                )}
              </>
            )}
          </>
        )}

        <div style={{
          display: 'flex',
          justifyContent: 'flex-end',
          gap: '8px',
          marginTop: '20px',
          paddingTop: '16px',
          borderTop: '1px solid var(--border-color)',
        }}>
          <button className="btn btn-secondary" onClick={onClose}>
            {result ? 'Close' : 'Cancel'}
          </button>
          {exportOptions && isTrained && !result && (
            <button
              className="btn btn-primary"
              onClick={handleExport}
              disabled={
                exporting ||
                (format === 'huggingface' && !hfAvailable) ||
                (format === 'ollama' && !ollamaAvailable)
              }
            >
              {exporting ? '⏳ Exporting...' : '📤 Export'}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
