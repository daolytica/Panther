import { useState, useEffect } from 'react';
import { api } from '../api';

interface ExportChatModalProps {
  isOpen: boolean;
  onClose: () => void;
  selectedMessageIds: string[];
  chatType: 'profile' | 'coder';
  chatIds?: string[]; // For coder chats
}

export function ExportChatModal({
  isOpen,
  onClose,
  selectedMessageIds,
  chatType,
  chatIds = [],
}: ExportChatModalProps) {
  const [projects, setProjects] = useState<any[]>([]);
  const [selectedProject, setSelectedProject] = useState<string>('');
  const [localModels, setLocalModels] = useState<any[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [_loading, _setLoading] = useState(false); // For future use
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    if (isOpen) {
      loadProjects();
    }
  }, [isOpen]);

  useEffect(() => {
    if (selectedProject) {
      loadLocalModels();
    } else {
      setLocalModels([]);
      setSelectedModel('');
    }
  }, [selectedProject]);

  const loadProjects = async () => {
    try {
      const data = await api.listProjects();
      setProjects(data);
    } catch (error) {
      console.error('Failed to load projects:', error);
    }
  };

  const loadLocalModels = async () => {
    if (!selectedProject) return;
    try {
      const models = await api.listLocalModels(selectedProject);
      setLocalModels(models);
    } catch (error) {
      console.error('Failed to load local models:', error);
      setLocalModels([]);
    }
  };

  const handleExport = async () => {
    if (!selectedProject || selectedMessageIds.length === 0) {
      alert('Please select a project and ensure messages are selected');
      return;
    }

    setExporting(true);
    try {
      let exportedCount = 0;
      
      if (chatType === 'profile') {
        exportedCount = await api.exportChatMessagesToTraining({
          message_ids: selectedMessageIds,
          project_id: selectedProject,
          local_model_id: selectedModel || undefined,
        });
      } else {
        // For coder chats, export entire chats or selected messages
        exportedCount = await api.exportCoderChatsToTraining({
          chat_ids: chatIds.length > 0 ? chatIds : [],
          message_ids: selectedMessageIds.length > 0 ? selectedMessageIds : undefined,
          project_id: selectedProject,
          local_model_id: selectedModel || undefined,
        });
      }

      alert(`Successfully exported ${exportedCount} training examples to project.`);
      onClose();
    } catch (error: any) {
      console.error('Export failed:', error);
      alert(`Export failed: ${error?.message || error}`);
    } finally {
      setExporting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div style={{
      position: 'fixed',
      top: 0,
      left: 0,
      right: 0,
      bottom: 0,
      background: 'rgba(0,0,0,0.5)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      zIndex: 1000
    }}>
      <div style={{
        background: 'var(--card-bg)',
        border: '1px solid var(--border-color)',
        padding: '24px',
        borderRadius: '8px',
        maxWidth: '500px',
        width: '90%',
        boxShadow: '0 4px 20px rgba(0,0,0,0.3)'
      }}>
        <h2 style={{ margin: '0 0 20px 0', fontSize: '18px', fontWeight: '600' }}>
          Export Chat to Training Data
        </h2>
        
        <div style={{ marginBottom: '16px' }}>
          <label style={{ display: 'block', marginBottom: '6px', fontSize: '13px', fontWeight: '500' }}>
            Project
          </label>
          <select
            value={selectedProject}
            onChange={(e) => setSelectedProject(e.target.value)}
            style={{
              width: '100%',
              padding: '8px 12px',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '14px'
            }}
          >
            <option value="">Select a project...</option>
            {projects.map(p => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </div>

        <div style={{ marginBottom: '16px' }}>
          <label style={{ display: 'block', marginBottom: '6px', fontSize: '13px', fontWeight: '500' }}>
            Local Model (Optional)
          </label>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            disabled={!selectedProject}
            style={{
              width: '100%',
              padding: '8px 12px',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              background: selectedProject ? 'var(--bg-primary)' : 'var(--bg-secondary)',
              color: 'var(--text-primary)',
              fontSize: '14px',
              opacity: selectedProject ? 1 : 0.6
            }}
          >
            <option value="">All models in project</option>
            {localModels.map(m => (
              <option key={m.id} value={m.id}>{m.name}</option>
            ))}
          </select>
        </div>

        <div style={{ 
          marginBottom: '20px', 
          padding: '12px', 
          background: 'var(--bg-secondary)',
          borderRadius: '4px',
          fontSize: '13px',
          color: 'var(--text-secondary)'
        }}>
          {selectedMessageIds.length > 0 ? (
            <span>Exporting {selectedMessageIds.length} selected message{selectedMessageIds.length !== 1 ? 's' : ''} as training examples.</span>
          ) : (
            <span>No messages selected. Please select messages in the chat to export.</span>
          )}
        </div>

        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
          <button
            onClick={onClose}
            disabled={exporting}
            style={{
              padding: '8px 16px',
              background: 'var(--bg-secondary)',
              color: 'var(--text-primary)',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              cursor: exporting ? 'not-allowed' : 'pointer',
              fontSize: '14px'
            }}
          >
            Cancel
          </button>
          <button
            onClick={handleExport}
            disabled={!selectedProject || selectedMessageIds.length === 0 || exporting}
            style={{
              padding: '8px 16px',
              background: exporting ? '#6c757d' : '#4a90e2',
              color: 'white',
              border: 'none',
              borderRadius: '4px',
              cursor: (!selectedProject || selectedMessageIds.length === 0 || exporting) ? 'not-allowed' : 'pointer',
              fontSize: '14px',
              fontWeight: '500'
            }}
          >
            {exporting ? 'Exporting...' : 'Export'}
          </button>
        </div>
      </div>
    </div>
  );
}
