import { Link, useNavigate } from 'react-router-dom';
import { useEffect, useState } from 'react';
import { useAppStore } from '../store';
import { api } from '../api';

interface Session {
  id: string;
  title: string;
  project_id: string;
  user_question: string;
  mode: string;
  created_at: string;
  updated_at?: string;
}

export function Sessions() {
  const { sessions, setSessions, setProfiles, projects, setProjects } = useAppStore();
  const navigate = useNavigate();
  const [deleteModal, setDeleteModal] = useState<{ session: Session } | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      const [sessionsData, profilesData, projectsData] = await Promise.all([
        api.listSessions(),
        api.listProfiles(),
        api.listProjects(),
      ]);
      setSessions(sessionsData);
      setProfiles(profilesData);
      setProjects(projectsData);
    } catch (error) {
      console.error('Failed to load data:', error);
    }
  };

  const getProjectName = (projectId: string) => {
    const project = projects.find(p => p.id === projectId);
    return project?.name || 'Unknown Project';
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
        <h1>Sessions</h1>
        <p>View and manage your AI sessions</p>
      </div>

      <div style={{ 
        marginBottom: '20px', 
        padding: '12px 16px', 
        background: '#e7f3ff', 
        borderRadius: '6px',
        border: '1px solid #b3d9ff',
        fontSize: '13px',
        color: '#004085'
      }}>
        <strong>💡 What are Sessions?</strong>
        <p style={{ margin: '5px 0 0 0', fontSize: '12px' }}>
          Sessions are individual AI activities where you ask a question and get responses from multiple AI agents. 
          Each session belongs to a <strong>Project</strong> (organizational container). You can have multiple sessions per project.
        </p>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <Link to="/session-builder" className="btn btn-primary">
          New Session
        </Link>
      </div>

      {sessions.length === 0 ? (
        <div className="card">
          <p style={{ color: 'var(--text-secondary)' }}>No sessions yet. Create your first session to get started.</p>
        </div>
      ) : (
        <div className="grid grid-2">
          {sessions.map((session) => (
            <div key={session.id} className="card">
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: '10px' }}>
                <h3 style={{ margin: 0 }}>{session.title}</h3>
                <span style={{ 
                  fontSize: '11px', 
                  padding: '4px 8px', 
                  background: '#e9ecef', 
                  borderRadius: '4px',
                  color: 'var(--text-secondary)'
                }}>
                  {getProjectName(session.project_id)}
                </span>
              </div>
              <p style={{ color: 'var(--text-secondary)', marginTop: '10px' }}>{session.user_question}</p>
              <p style={{ color: '#999', fontSize: '12px', marginTop: '10px' }}>
                Mode: {session.mode} • {new Date(session.created_at).toLocaleString()}
              </p>
              <div style={{ marginTop: '15px', display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                <button
                  className="btn btn-secondary"
                  onClick={async () => {
                    try {
                      const runData = await api.getSessionRun(session.id);
                      if (runData && runData.run_id) {
                        if (session.mode === 'parallel') {
                          navigate(`/parallel-brainstorm/${runData.run_id}`);
                        } else {
                          navigate(`/debate-room/${runData.run_id}`);
                        }
                      } else {
                        alert('No run found for this session. Please create a new session.');
                      }
                    } catch (error) {
                      console.error('Failed to load session run:', error);
                      alert('Failed to load session');
                    }
                  }}
                >
                  View
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  style={{ color: '#dc3545' }}
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setDeleteModal({ session });
                  }}
                >
                  🗑️ Delete
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Delete confirmation modal - delete only happens when user clicks Confirm */}
      {deleteModal && (
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
          onClick={() => !deleting && setDeleteModal(null)}
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
            <h3 style={{ margin: '0 0 16px 0' }}>Delete Session</h3>
            <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '20px' }}>
              Delete session &quot;{deleteModal.session.title}&quot;? This will also delete all associated runs and results.
            </p>
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button
                type="button"
                className="btn btn-secondary"
                disabled={deleting}
                onClick={() => !deleting && setDeleteModal(null)}
              >
                Cancel
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                style={{ background: '#dc3545', color: 'white', borderColor: '#dc3545' }}
                disabled={deleting}
                onClick={async (e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  setDeleting(true);
                  try {
                    await api.deleteSession(deleteModal.session.id);
                    setDeleteModal(null);
                    await loadData();
                  } catch (error) {
                    console.error('Failed to delete session:', error);
                    alert('Failed to delete session');
                  } finally {
                    setDeleting(false);
                  }
                }}
              >
                {deleting ? 'Deleting...' : 'Confirm Delete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
