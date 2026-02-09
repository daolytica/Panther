import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { api } from '../api';

interface Project {
  id: string;
  name: string;
  description?: string;
  created_at: string;
  updated_at: string;
}

export function Projects() {
  const navigate = useNavigate();
  const { projects, setProjects, sessions, setSessions } = useAppStore();
  const [showModal, setShowModal] = useState(false);
  const [editingProject, setEditingProject] = useState<Project | null>(null);
  const [formData, setFormData] = useState({ name: '', description: '' });
  const [movingSession, setMovingSession] = useState<{ sessionId: string; currentProjectId: string } | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [projectsData, sessionsData] = await Promise.all([
        api.listProjects(),
        api.listSessions(),
      ]);
      setProjects(projectsData);
      setSessions(sessionsData);
    } catch (error) {
      console.error('Failed to load data:', error);
    }
  };

  const handleCreate = () => {
    setEditingProject(null);
    setFormData({ name: '', description: '' });
    setShowModal(true);
  };

  const handleEdit = (project: Project) => {
    setEditingProject(project);
    setFormData({ name: project.name, description: project.description || '' });
    setShowModal(true);
  };

  const handleSave = async () => {
    if (!formData.name.trim()) {
      alert('Please enter a project name');
      return;
    }

    try {
      if (editingProject) {
        await api.updateProject({
          id: editingProject.id,
          name: formData.name,
          description: formData.description || undefined,
        });
      } else {
        await api.createProject({
          name: formData.name,
          description: formData.description || undefined,
        });
      }
      setShowModal(false);
      await loadData();
    } catch (error) {
      console.error('Failed to save project:', error);
      alert('Failed to save project');
    }
  };

  const handleDelete = async (projectId: string) => {
    if (!window.confirm('Are you sure you want to delete this project? All sessions in this project will also be deleted.')) {
      return;
    }

    try {
      await api.deleteProject(projectId);
      await loadData();
    } catch (error) {
      console.error('Failed to delete project:', error);
      alert('Failed to delete project');
    }
  };

  const handleMoveSession = async (sessionId: string, newProjectId: string) => {
    try {
      await api.moveSessionToProject(sessionId, newProjectId);
      setMovingSession(null);
      await loadData();
    } catch (error) {
      console.error('Failed to move session:', error);
      alert('Failed to move session');
    }
  };

  const getSessionsForProject = (projectId: string) => {
    return sessions.filter(s => s.project_id === projectId);
  };

  return (
    <div>
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px', marginBottom: '10px' }}>
          <button
            onClick={() => navigate('/home')}
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
        <h1>Local Training Session</h1>
        <p>Train local LLM models (Ollama) with your data. Create projects to organize training sessions.</p>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <button className="btn btn-primary" onClick={handleCreate}>
          Create Project
        </button>
      </div>

      {projects.length === 0 ? (
        <div className="card">
          <p style={{ color: 'var(--text-secondary)' }}>No projects yet. Create your first project to get started.</p>
        </div>
      ) : (
        <div className="grid grid-2">
          {projects.map((project) => {
            const projectSessions = getSessionsForProject(project.id);
            return (
              <div key={project.id} className="card">
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: '10px' }}>
                  <div style={{ flex: 1 }}>
                    <h3 style={{ margin: 0 }}>{project.name}</h3>
                    {project.description && (
                      <p style={{ color: 'var(--text-secondary)', marginTop: '5px', fontSize: '14px' }}>{project.description}</p>
                    )}
                  </div>
                  <div style={{ display: 'flex', gap: '5px' }}>
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: '11px', padding: '4px 8px' }}
                      onClick={() => navigate(`/project-training/${project.id}`)}
                    >
                      🎓 Train
                    </button>
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: '11px', padding: '4px 8px' }}
                      onClick={() => handleEdit(project)}
                    >
                      ✏️ Edit
                    </button>
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: '11px', padding: '4px 8px', color: '#dc3545' }}
                      onClick={() => handleDelete(project.id)}
                    >
                      🗑️ Delete
                    </button>
                  </div>
                </div>
                <div style={{ marginTop: '15px', paddingTop: '15px', borderTop: '1px solid var(--border-color)' }}>
                  <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                    <strong>{projectSessions.length}</strong> session{projectSessions.length !== 1 ? 's' : ''}
                  </p>
                  {projectSessions.length > 0 && (
                    <div style={{ fontSize: '12px' }}>
                      {projectSessions.slice(0, 3).map((session) => (
                        <div key={session.id} style={{ marginBottom: '5px', color: 'var(--text-secondary)' }}>
                          • {session.title}
                        </div>
                      ))}
                      {projectSessions.length > 3 && (
                        <div style={{ color: 'var(--text-tertiary)', fontStyle: 'italic' }}>
                          + {projectSessions.length - 3} more
                        </div>
                      )}
                    </div>
                  )}
                </div>
                <p style={{ color: 'var(--text-tertiary)', fontSize: '11px', marginTop: '15px' }}>
                  Created: {new Date(project.created_at).toLocaleDateString()}
                </p>
              </div>
            );
          })}
        </div>
      )}

      {/* Create/Edit Modal */}
      {showModal && (
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
          onClick={() => setShowModal(false)}
        >
          <div
            style={{
              background: 'var(--card-bg)',
              padding: '24px',
              borderRadius: '8px',
              width: '500px',
              maxWidth: '90%',
              border: '1px solid var(--border-color)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <h2 style={{ marginTop: 0 }}>{editingProject ? 'Edit Project' : 'Create Project'}</h2>
            <div className="form-group" style={{ marginBottom: '15px' }}>
              <label>Project Name *</label>
              <input
                type="text"
                value={formData.name}
                onChange={(e) => setFormData({ ...formData, name: e.target.value })}
                placeholder="Enter project name"
                style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
              />
            </div>
            <div className="form-group" style={{ marginBottom: '20px' }}>
              <label>Description</label>
              <textarea
                value={formData.description}
                onChange={(e) => setFormData({ ...formData, description: e.target.value })}
                placeholder="Enter project description (optional)"
                rows={3}
                style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid var(--border-color)' }}
              />
            </div>
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button className="btn btn-secondary" onClick={() => setShowModal(false)}>
                Cancel
              </button>
              <button className="btn btn-primary" onClick={handleSave}>
                {editingProject ? 'Update' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Move Session Modal */}
      {movingSession && (
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
          onClick={() => setMovingSession(null)}
        >
          <div
            style={{
              background: 'var(--card-bg)',
              padding: '24px',
              borderRadius: '8px',
              width: '400px',
              maxWidth: '90%',
              border: '1px solid var(--border-color)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <h3 style={{ marginTop: 0 }}>Move Session to Project</h3>
            <p style={{ fontSize: '14px', color: 'var(--text-secondary)', marginBottom: '15px' }}>
              Select a project to move this session to:
            </p>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '20px' }}>
              {projects
                .filter(p => p.id !== movingSession.currentProjectId)
                .map((project) => (
                  <button
                    key={project.id}
                    className="btn btn-secondary"
                    onClick={() => handleMoveSession(movingSession.sessionId, project.id)}
                    style={{ textAlign: 'left', padding: '10px' }}
                  >
                    {project.name}
                  </button>
                ))}
            </div>
            <button className="btn btn-secondary" onClick={() => setMovingSession(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
