import { useState, useEffect, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { api } from '../api';
import { ImportTrainingDataModal } from '../components/ImportTrainingDataModal';
import { TrainingDataChat } from '../components/TrainingDataChat';
import { LoraTrainingModal, LoraTrainingConfig } from '../components/LoraTrainingModal';
import { ExportModelModal } from '../components/ExportModelModal';

export function ProjectTraining() {
  const { projectId } = useParams<{ projectId: string }>();
  const navigate = useNavigate();
  const { projects } = useAppStore();
  const [localModels, setLocalModels] = useState<any[]>([]);
  const [trainingData, setTrainingData] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [showImportModal, setShowImportModal] = useState(false);
  const [showEditModelModal, setShowEditModelModal] = useState(false);
  const [editingModel, setEditingModel] = useState<any | null>(null);
  const [modelName, setModelName] = useState('');
  const [baseModel, setBaseModel] = useState('llama3');
  const [creating, setCreating] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [trainingModelId, setTrainingModelId] = useState<string | null>(null);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaLoading, setOllamaLoading] = useState(true);
  const [ollamaError, setOllamaError] = useState<string | null>(null);
  const [selectedOllamaModel, setSelectedOllamaModel] = useState('');
  const [showLoraModal, setShowLoraModal] = useState(false);
  const [loraTrainingModel, setLoraTrainingModel] = useState<any>(null);
  const [showExportModal, setShowExportModal] = useState(false);
  const [exportingModel, setExportingModel] = useState<any>(null);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [importProgress, setImportProgress] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const loadProjectData = useCallback(async () => {
    if (!projectId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setLoadError(null);
    const timeoutMs = 20000;
    const timeoutPromise = new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error('Load timed out. Try again.')), timeoutMs)
    );
    try {
      const [models, data] = await Promise.race([
        Promise.all([
          api.listLocalModels(projectId),
          api.listTrainingData(projectId),
        ]),
        timeoutPromise,
      ]);
      setLocalModels(models);
      setTrainingData(data);
    } catch (error: any) {
      console.error('Failed to load project training data:', error);
      setLoadError(error?.message || 'Failed to load');
      setLocalModels([]);
      setTrainingData([]);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    loadProjectData();
  }, [loadProjectData]);

  useEffect(() => {
    const loadOllama = async () => {
      setOllamaLoading(true);
      setOllamaError(null);
      try {
        const health = await api.checkOllamaInstallation();
        const models = health?.models ?? [];
        setOllamaModels(models);
        if (models.length > 0) {
          setSelectedOllamaModel((prev) => prev || models[0]);
        }
      } catch (error: any) {
        setOllamaError(error?.message || 'Ollama not running');
        setOllamaModels([]);
      } finally {
        setOllamaLoading(false);
      }
    };
    loadOllama();
  }, []);

  const project = projects.find(p => p.id === projectId);

  return (
    <div>
      <div className="page-header">
        <button
          onClick={() => navigate('/projects')}
          style={{
            padding: '8px 15px',
            background: 'transparent',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            cursor: 'pointer',
            fontSize: '14px',
            color: 'var(--text-primary)',
            marginBottom: '10px',
          }}
        >
          ← Back
        </button>
        <h1>{project?.name || 'Project Training'}</h1>
        <p>Manage local models and training data for this project.</p>
      </div>

      <div className="grid grid-2" style={{ alignItems: 'flex-start', gap: '20px' }}>
        {/* Local Models */}
        <div className="card" style={{ padding: '20px' }}>
          <h2 style={{ margin: 0, marginBottom: '16px', fontSize: '18px', fontWeight: 600 }}>🧠 Local Models</h2>
          {localModels.some(m => m.training_status === 'training') && (
            <div style={{ marginBottom: '12px', padding: '8px 12px', background: 'rgba(23, 162, 184, 0.15)', borderRadius: '6px', border: '1px solid #17a2b8', fontSize: '13px' }}>
              <strong>Training status:</strong> Training in progress — progress and ETA shown below.
            </div>
          )}

          {/* Add model to train - inline Ollama selection */}
          <div style={{ marginBottom: '20px', padding: '16px', background: 'var(--bg-secondary)', borderRadius: '8px', border: '1px solid var(--border-color)' }}>
            <label style={{ display: 'block', marginBottom: '10px', fontWeight: '600', fontSize: '14px', color: 'var(--text-primary)' }}>
              Add base model (Ollama)
            </label>
            {ollamaLoading ? (
              <p style={{ margin: 0, fontSize: '13px', color: 'var(--text-secondary)' }}>Checking Ollama...</p>
            ) : ollamaError ? (
              <div>
                <p style={{ margin: 0, fontSize: '13px', color: '#dc3545' }}>Ollama not running. Start Ollama to add models.</p>
                <button
                  className="btn btn-secondary"
                  onClick={async () => {
                    setOllamaLoading(true);
                    setOllamaError(null);
                    try {
                      const health = await api.checkOllamaInstallation();
                      const models = health?.models ?? [];
                      setOllamaModels(models);
                      if (models.length > 0) setSelectedOllamaModel(models[0]);
                    } catch (e: any) {
                      setOllamaError(e?.message || 'Ollama not running');
                    } finally {
                      setOllamaLoading(false);
                    }
                  }}
                  style={{ marginTop: '8px', padding: '4px 10px', fontSize: '12px' }}
                >
                  Retry
                </button>
              </div>
            ) : ollamaModels.length === 0 ? (
              <p style={{ margin: 0, fontSize: '13px', color: 'var(--text-secondary)' }}>
                No Ollama models found. Run <code>ollama pull &lt;model&gt;</code> (e.g. ollama pull llama3) to install.
              </p>
            ) : (
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                <select
                  value={selectedOllamaModel}
                  onChange={(e) => setSelectedOllamaModel(e.target.value)}
                  disabled={creating}
                  style={{
                    flex: 1,
                    minWidth: '150px',
                    padding: '8px 12px',
                    borderRadius: '4px',
                    border: '1px solid var(--border-color)',
                    background: 'var(--card-bg)',
                    color: 'var(--text-primary)',
                  }}
                >
                  {ollamaModels.map((m) => (
                    <option key={m} value={m}>{m}</option>
                  ))}
                </select>
                <button
                  className="btn btn-primary"
                  onClick={async () => {
                    if (!projectId || !selectedOllamaModel || creating) return;
                    setCreating(true);
                    const timeoutMs = 15000;
                    const timeoutPromise = new Promise<never>((_, reject) =>
                      setTimeout(() => reject(new Error('Request timed out. The app may be busy. Try again.')), timeoutMs)
                    );
                    try {
                      await Promise.race([
                        (async () => {
                          await api.createLocalModel({
                            project_id: projectId,
                            base_model: selectedOllamaModel,
                          });
                          const models = await api.listLocalModels(projectId);
                          setLocalModels(models);
                          setLoadError(null);
                        })(),
                        timeoutPromise,
                      ]);
                    } catch (error: any) {
                      console.error('Failed to create local model:', error);
                      alert(`Failed to create local model: ${error?.message || error}`);
                    } finally {
                      setCreating(false);
                    }
                  }}
                  disabled={creating || !selectedOllamaModel}
                  style={{ padding: '8px 14px' }}
                >
                  {creating ? 'Adding...' : 'Add'}
                </button>
              </div>
            )}
          </div>

          {loading ? (
            <p>Loading models...</p>
          ) : loadError && localModels.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '20px', color: 'var(--text-secondary)' }}>
              <p style={{ margin: 0 }}>{loadError}</p>
              <button
                className="btn btn-secondary"
                onClick={() => loadProjectData()}
                style={{ marginTop: '12px', padding: '8px 16px' }}
              >
                Retry
              </button>
            </div>
          ) : localModels.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '20px', color: 'var(--text-secondary)' }}>
              <p>No local models yet.</p>
              <p style={{ fontSize: '12px', marginTop: '10px' }}>
                Create a local model to start training with your project's data.
              </p>
            </div>
          ) : (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
              {localModels.map(model => (
                <li key={model.id} style={{ padding: '8px 0', borderBottom: '1px solid #eee' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
                    <div style={{ flex: 1 }}>
                      <strong>{model.name}</strong>
                      <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                        Base: {model.base_model} • Status: <span 
                          style={{ 
                            color: model.training_status === 'complete' ? '#28a745' : 
                                   model.training_status === 'training' ? '#17a2b8' : 
                                   model.training_status === 'failed' ? '#dc3545' : '#6c757d'
                          }}
                          title={
                            model.training_status === 'pending' ? 'Model created but not trained yet. Import training data and start training to begin.' :
                            model.training_status === 'training' ? 'Training in progress...' :
                            (model.training_status === 'complete' || model.training_status === 'completed') ? 'Training completed successfully' :
                            model.training_status === 'failed' ? (() => {
                              try {
                                const metrics = model.training_metrics_json ? JSON.parse(model.training_metrics_json) : null;
                                return metrics?.error || 'Training failed. Check logs for details.';
                              } catch {
                                return 'Training failed. Check logs for details.';
                              }
                            })() :
                            'Unknown status'
                          }
                        >
                          {model.training_status === 'pending' ? '⏳ pending (ready to train)' : 
                           model.training_status === 'training' ? '🔄 training' :
                           model.training_status === 'complete' ? '✅ complete' :
                           model.training_status === 'failed' ? '❌ failed' :
                           model.training_status}
                        </span>
                        {model.training_status === 'failed' && (() => {
                          let errorMessage = null;
                          
                          // Try to extract error from training_metrics_json
                          if (model.training_metrics_json) {
                            try {
                              const metrics = typeof model.training_metrics_json === 'string' 
                                ? JSON.parse(model.training_metrics_json) 
                                : model.training_metrics_json;
                              if (metrics && metrics.error) {
                                errorMessage = metrics.error;
                              } else if (metrics && metrics.stderr) {
                                // If no error field but stderr exists, use that
                                errorMessage = `Python Error Output:\n\n${metrics.stderr}`;
                              }
                            } catch (e) {
                              // If parsing fails, try to use the raw string
                              if (typeof model.training_metrics_json === 'string') {
                                errorMessage = model.training_metrics_json;
                              }
                            }
                          }
                          
                          // If no error message found, show a generic message
                          if (!errorMessage) {
                            errorMessage = 'Training failed. No error details available. Check the console or try training again.';
                          }
                          
                          return (
                            <details style={{ marginTop: '8px', fontSize: '10px' }} open={true}>
                              <summary style={{ cursor: 'pointer', color: '#dc3545', fontWeight: '500' }}>
                                🔍 View Error Details
                              </summary>
                              <div style={{ 
                                marginTop: '8px', 
                                padding: '8px', 
                                background: '#ffebee', 
                                border: '1px solid #f44336', 
                                borderRadius: '4px',
                                whiteSpace: 'pre-wrap',
                                maxHeight: '200px',
                                overflow: 'auto',
                                fontFamily: 'monospace',
                                fontSize: '11px',
                                lineHeight: '1.4'
                              }}>
                                {errorMessage}
                              </div>
                            </details>
                          );
                        })()}
                      </div>
                    </div>
                    <div style={{ display: 'flex', gap: '5px', flexWrap: 'wrap' }}>
                      {/* Show Start Training button for pending, failed, or complete (to retrain) */}
                      {(model.training_status === 'pending' || model.training_status === 'failed' || model.training_status === 'complete' || model.training_status === 'completed' || !model.training_status) && (
                        <button
                          className="btn btn-primary"
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            
                            if (!projectId) {
                              alert('No project ID available');
                              return;
                            }
                            
                            if (trainingData.length === 0) {
                              alert('Please import training data first before starting training.');
                              return;
                            }
                            
                            // Check resource limit (max concurrent trainings)
                            api.canStartTraining()
                              .then((canStart) => {
                                if (!canStart?.can_start) {
                                  alert(canStart?.message || 'Another training is already in progress.');
                                  return;
                                }
                                setLoraTrainingModel(model);
                                setShowLoraModal(true);
                              })
                              .catch(() => {
                                // Proceed if check fails (e.g. browser mode)
                                setLoraTrainingModel(model);
                                setShowLoraModal(true);
                              });
                          }}
                          disabled={trainingModelId === model.id || model.training_status === 'training'}
                          style={{ padding: '4px 8px', fontSize: '12px' }}
                        >
                          {(() => {
                            if (trainingModelId === model.id) return '⏳ Starting...';
                            if (model.training_status === 'complete' || model.training_status === 'completed') return '🔄 Retrain';
                            return '🚀 Start Training';
                          })()}
                        </button>
                      )}
                      {(model.training_status === 'complete' || model.training_status === 'completed') && (
                        <button
                          className="btn btn-secondary"
                          onClick={(e) => {
                            e.preventDefault();
                            e.stopPropagation();
                            setExportingModel(model);
                            setShowExportModal(true);
                          }}
                          style={{ padding: '4px 8px', fontSize: '12px' }}
                        >
                          📤 Export
                        </button>
                      )}
                      {model.training_status === 'training' && (() => {
                        // Parse metrics for progress, ETA, and resource usage
                        let progress = 0;
                        let eta = '';
                        let gpuUsage = null;
                        let cpuUsage = null;
                        
                        if (model.training_metrics_json) {
                          try {
                            const metrics = typeof model.training_metrics_json === 'string' 
                              ? JSON.parse(model.training_metrics_json) 
                              : model.training_metrics_json;
                            progress = metrics.progress || 0;
                            eta = metrics.eta || '';
                            gpuUsage = metrics.gpu_usage || null;
                            cpuUsage = metrics.cpu_usage || null;
                          } catch (e) {
                            console.error('Failed to parse training metrics:', e);
                          }
                        }
                        
                        return (
                          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', padding: '12px', background: '#f8f9fa', borderRadius: '4px', marginTop: '8px', border: '1px solid #dee2e6' }}>
                            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '8px' }}>
                              <span style={{ color: '#17a2b8', fontWeight: 'bold' }}>🔄 Training...</span>
                              <button
                                className="btn btn-danger"
                                onClick={async () => {
                                  if (confirm('Stop training? This cannot be undone.')) {
                                    try {
                                      await api.stopTraining(model.id);
                                      const models = await api.listLocalModels(projectId);
                                      setLocalModels(models);
                                    } catch (error: any) {
                                      alert(`Failed to stop training: ${error.message || error}`);
                                    }
                                  }
                                }}
                                style={{ padding: '4px 8px', fontSize: '12px' }}
                              >
                                ⏹️ Stop
                              </button>
                            </div>
                            
                            {/* Always show progress section when training */}
                            <div>
                              {progress > 0 ? (
                                <>
                                  <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px', fontSize: '12px', fontWeight: '500' }}>
                                    <span>Progress: <strong>{progress}%</strong></span>
                                    {eta && <span style={{ color: 'var(--text-secondary)' }}>ETA: <strong>{eta}</strong></span>}
                                  </div>
                                  <div style={{ width: '100%', height: '10px', background: '#e0e0e0', borderRadius: '5px', overflow: 'hidden', marginBottom: '8px' }}>
                                    <div 
                                      style={{ 
                                        width: `${progress}%`, 
                                        height: '100%', 
                                        background: 'linear-gradient(90deg, #17a2b8, #138496)',
                                        transition: 'width 0.5s ease',
                                        boxShadow: '0 1px 3px rgba(0,0,0,0.2)'
                                      }} 
                                    />
                                  </div>
                                </>
                              ) : (
                                <div style={{ fontSize: '12px', color: 'var(--text-secondary)', fontStyle: 'italic', marginBottom: '8px' }}>
                                  Initializing training... Please wait.
                                </div>
                              )}
                            </div>
                            
                            {/* GPU/CPU Usage - Always show section, but content conditionally */}
                            <div style={{ fontSize: '11px', color: 'var(--text-secondary)', padding: '6px', background: 'var(--card-bg)', borderRadius: '3px', border: '1px solid var(--border-color)' }}>
                              {gpuUsage ? (
                                <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                                  <span><strong>GPU:</strong> {gpuUsage.percent}%</span>
                                  <span><strong>Memory:</strong> {gpuUsage.memory}</span>
                                </div>
                              ) : cpuUsage ? (
                                <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                                  <span><strong>CPU:</strong> {cpuUsage.percent}%</span>
                                  <span><strong>Memory:</strong> {cpuUsage.memory}</span>
                                </div>
                              ) : (
                                <div style={{ color: '#999', fontStyle: 'italic' }}>
                                  Resource usage will appear here once training starts...
                                </div>
                              )}
                            </div>
                          </div>
                        );
                      })()}
                      <button
                        className="btn btn-secondary"
                        onClick={() => {
                          setEditingModel(model);
                          setModelName(model.name);
                          setBaseModel(model.base_model);
                          setShowEditModelModal(true);
                        }}
                        disabled={model.training_status === 'training'}
                        style={{ padding: '4px 8px', fontSize: '12px' }}
                      >
                        ✏️ Edit
                      </button>
                    </div>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        {/* Training Data */}
        <div className="card" style={{ padding: '20px' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
            <h2 style={{ margin: 0, fontSize: '18px', fontWeight: 600 }}>📚 Training Data</h2>
            <button
              className="btn btn-primary"
              onClick={() => setShowImportModal(true)}
            >
              📥 Import
            </button>
          </div>
          {loading ? (
            <p>Loading training data...</p>
          ) : trainingData.length === 0 ? (
            <p style={{ color: 'var(--text-secondary)' }}>No training data imported yet.</p>
          ) : (
            <div style={{ display: 'grid', gap: '10px', maxHeight: '400px', overflowY: 'auto' }}>
              {trainingData.map((data) => {
                const associatedModel = localModels.find(m => m.id === data.local_model_id);
                return (
                  <div key={data.id} style={{ padding: '10px', border: '1px solid var(--border-color)', borderRadius: '4px' }}>
                    {associatedModel && (
                      <div style={{ fontSize: '11px', color: '#007bff', marginBottom: '5px' }}>
                        🔗 {associatedModel.name}
                      </div>
                    )}
                    <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '5px' }}>
                      <strong>Input:</strong> {data.input_text.length > 100 ? data.input_text.substring(0, 100) + '...' : data.input_text}
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                      <strong>Output:</strong> {data.output_text.length > 100 ? data.output_text.substring(0, 100) + '...' : data.output_text}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* Talk to Trained Data Section */}
      <div className="card" style={{ marginTop: '24px', padding: '20px' }}>
        <h2 style={{ margin: '0 0 16px 0', fontSize: '18px', fontWeight: 600 }}>💬 Talk to Trained Data</h2>
        <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
          Ask questions about your training data. The AI will use your imported examples to provide context-aware answers.
        </p>
        <TrainingDataChat projectId={projectId || ''} trainingDataCount={trainingData.length} />
      </div>

      {/* Import status banner - shown when import runs in background */}
      {importStatus && (
        <div style={{
          padding: '12px 16px',
          marginBottom: '16px',
          background: importStatus === 'done' ? 'rgba(40, 167, 69, 0.15)' : importStatus === 'error' ? 'rgba(220, 53, 69, 0.15)' : 'rgba(23, 162, 184, 0.15)',
          border: `1px solid ${importStatus === 'done' ? '#28a745' : importStatus === 'error' ? '#dc3545' : '#17a2b8'}`,
          borderRadius: '6px',
          display: 'flex',
          flexDirection: 'column',
          gap: '4px',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <span>
              {importStatus === 'importing' && '⏳ Importing training data...'}
              {importStatus === 'done' && '✅ Import complete'}
              {importStatus === 'error' && '❌ Import failed'}
            </span>
            <button
              className="btn btn-sm btn-secondary"
              onClick={() => { setImportStatus(null); setImportProgress(null); }}
              style={{ padding: '4px 10px', fontSize: '12px' }}
            >
              Dismiss
            </button>
          </div>
          {importStatus === 'importing' && importProgress && (
            <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
              {importProgress}
            </div>
          )}
        </div>
      )}

      {/* Import Training Data Modal */}
      {showImportModal && (
        <ImportTrainingDataModal
          isOpen={showImportModal}
          onClose={() => setShowImportModal(false)}
          projectId={projectId || ''}
          localModelId={undefined}
          onImportComplete={() => {
            if (!projectId) return;
            api.listTrainingData(projectId).then(setTrainingData).catch(console.error);
            setImportStatus('done');
          }}
          onImportRequest={(runImport) => {
            setShowImportModal(false);
            setImportStatus('importing');
            setImportProgress(null);
            runImport((msg) => setImportProgress(msg))
              .then(() => {
                if (projectId) {
                  api.listTrainingData(projectId).then(setTrainingData).catch(console.error);
                }
              })
              .catch(() => setImportStatus('error'))
              .finally(() => {
                setImportStatus((s) => (s === 'importing' ? 'done' : s));
                setImportProgress(null);
              });
          }}
        />
      )}

      {/* Edit Local Model Modal */}
      {showEditModelModal && editingModel && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: 'rgba(0, 0, 0, 0.5)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={() => {
            if (!creating && !updating) {
              setShowEditModelModal(false);
              setEditingModel(null);
              setModelName('');
              setBaseModel('llama3');
            }
          }}
        >
          <div
            className="card"
            style={{
              width: '90%',
              maxWidth: '500px',
              background: 'var(--card-bg)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
              <h2 style={{ margin: 0 }}>{editingModel ? '✏️ Edit Local Model' : '🧠 Create Local Model'}</h2>
              <button
                onClick={() => {
                  if (!creating && !updating) {
                    setShowEditModelModal(false);
                    setEditingModel(null);
                    setModelName('');
                    setBaseModel('llama3');
                  }
                }}
                disabled={creating || updating}
                style={{
                  background: 'transparent',
                  border: 'none',
                  fontSize: '24px',
                  cursor: creating ? 'not-allowed' : 'pointer',
                  color: 'var(--text-primary)',
                  opacity: creating ? 0.5 : 1,
                }}
              >
                ×
              </button>
            </div>

            <div style={{ marginBottom: '15px' }}>
              <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
                Model Name (optional)
              </label>
              <input
                type="text"
                value={modelName}
                onChange={(e) => setModelName(e.target.value)}
                placeholder="e.g., My Project Model"
                disabled={creating || updating}
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

            <div style={{ marginBottom: '20px' }}>
              <label style={{ display: 'block', marginBottom: '8px', fontWeight: '500' }}>
                Base Model *
              </label>
              <select
                value={baseModel}
                onChange={(e) => setBaseModel(e.target.value)}
                disabled={updating}
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--card-bg)',
                  color: 'var(--text-primary)',
                }}
              >
                {(ollamaModels.length > 0 ? ollamaModels : ['llama3', 'mistral', 'codellama', 'phi', 'phi-2', 'gemma']).map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
                {ollamaModels.length > 0 && !ollamaModels.includes(baseModel) && (
                  <option value={baseModel}>{baseModel} (current)</option>
                )}
              </select>
              <small style={{ color: 'var(--text-secondary)', display: 'block', marginTop: '5px' }}>
                The base model to fine-tune. Make sure it's available in your local Ollama installation.
              </small>
            </div>

            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button
                className="btn btn-secondary"
                onClick={() => {
                  setShowEditModelModal(false);
                  setEditingModel(null);
                  setModelName('');
                  setBaseModel('llama3');
                }}
                disabled={creating || updating}
              >
                Cancel
              </button>
              <button
                className="btn btn-primary"
                onClick={async () => {
                  if (!baseModel.trim() || !projectId || !editingModel) return;
                  setUpdating(true);
                  try {
                    await api.updateLocalModel(editingModel.id, {
                      name: modelName.trim() || baseModel.trim(),
                      base_model: baseModel.trim(),
                    });
                    const models = await api.listLocalModels(projectId);
                    setLocalModels(models);
                    setShowEditModelModal(false);
                    setEditingModel(null);
                    setModelName('');
                    setBaseModel('llama3');
                  } catch (error) {
                    console.error('Failed to update model:', error);
                    alert(`Failed to update model: ${error}`);
                  } finally {
                    setUpdating(false);
                  }
                }}
                disabled={updating || !baseModel.trim()}
              >
                {updating ? 'Updating...' : 'Update Model'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* LoRA Training Configuration Modal */}
      {showLoraModal && loraTrainingModel && projectId && (
        <LoraTrainingModal
          isOpen={showLoraModal}
          onClose={() => {
            setShowLoraModal(false);
            setLoraTrainingModel(null);
          }}
          model={loraTrainingModel}
          projectId={projectId}
          trainingDataCount={trainingData.length}
          onStartTraining={async (config: LoraTrainingConfig) => {
            console.log('[LoRA Training] Starting with config:', config);
            setTrainingModelId(loraTrainingModel.id);
            setShowLoraModal(false);
            
            try {
              const result = await api.startLoraTraining({
                model_id: loraTrainingModel.id,
                project_id: projectId,
                base_model: loraTrainingModel.base_model,
                config,
              });
              console.log('[LoRA Training] Started:', result);
              
              // Reload models to show updated status
              const models = await api.listLocalModels(projectId);
              setLocalModels(models);
              
              // Poll for status updates
              const checkStatus = setInterval(async () => {
                try {
                  const updatedModels = await api.listLocalModels(projectId);
                  setLocalModels(updatedModels);
                  const updatedModel = updatedModels.find((m: any) => m.id === loraTrainingModel.id);
                  if (updatedModel) {
                    if (updatedModel.training_metrics_json) {
                      try {
                        const metrics = typeof updatedModel.training_metrics_json === 'string' 
                          ? JSON.parse(updatedModel.training_metrics_json) 
                          : updatedModel.training_metrics_json;
                        if (metrics.progress) {
                          console.log(`[LoRA Training] Progress: ${metrics.progress}%`);
                        }
                      } catch (e) {
                        // Ignore parse errors
                      }
                    }
                    
                    if (updatedModel.training_status === 'completed' || 
                        updatedModel.training_status === 'failed' ||
                        updatedModel.training_status === 'dry_run_complete') {
                      console.log('[LoRA Training] Finished:', updatedModel.training_status);
                      clearInterval(checkStatus);
                      setTrainingModelId(null);
                      setLoraTrainingModel(null);
                    }
                  }
                } catch (err) {
                  console.error('[LoRA Training] Polling error:', err);
                }
              }, 2000);
              
              // Stop polling after 30 minutes
              setTimeout(() => {
                clearInterval(checkStatus);
                setTrainingModelId(null);
              }, 1800000);
              
            } catch (error: any) {
              console.error('[LoRA Training] Failed:', error);
              alert(`Failed to start LoRA training: ${error?.message || error}`);
              setTrainingModelId(null);
              setLoraTrainingModel(null);
            }
          }}
        />
      )}

      {/* Export Model Modal */}
      {showExportModal && exportingModel && (
        <ExportModelModal
          isOpen={showExportModal}
          onClose={() => {
            setShowExportModal(false);
            setExportingModel(null);
          }}
          model={exportingModel}
          onExportComplete={() => {
            if (projectId) {
              api.listLocalModels(projectId).then(setLocalModels).catch(console.error);
            }
          }}
        />
      )}
    </div>
  );
}

