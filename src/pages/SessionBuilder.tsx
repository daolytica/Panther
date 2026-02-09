import { useState, useEffect } from 'react';
import { useAppStore } from '../store';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';

export function SessionBuilder() {
  const { profiles, projects } = useAppStore();
  const navigate = useNavigate();
  const [question, setQuestion] = useState('');
  const [title, setTitle] = useState('');
  const [projectId, setProjectId] = useState('');
  const [mode, setMode] = useState<'parallel' | 'debate'>('parallel');
  const [selectedProfiles, setSelectedProfiles] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  // Load profiles if store is empty (e.g. user navigated directly to session builder)
  useEffect(() => {
    if (profiles.length === 0) {
      api.listProfiles()
        .then((data) => useAppStore.getState().setProfiles(data))
        .catch((err) => console.error('Failed to load profiles:', err));
    }
  }, [profiles.length]);

  useEffect(() => {
    // Create default project if none exists
    if (projects.length === 0) {
      api.createProject({ name: 'Default Project', description: 'Default project for sessions' })
        .then(() => {
          // Reload projects
          api.listProjects().then(data => {
            useAppStore.getState().setProjects(data);
            if (data.length > 0) {
              setProjectId(data[0].id);
            }
          });
        });
    } else if (!projectId && projects.length > 0) {
      setProjectId(projects[0].id);
    }
  }, [projects, projectId]);

  useEffect(() => {
    // Load local models when project is selected
    if (projectId) {
      api.listLocalModels(projectId)
        .catch(err => console.error('Failed to load local models:', err));
    }
  }, [projectId]);

  const handleStart = async () => {
    if (!question.trim() || selectedProfiles.length < 2) {
      alert('Please enter a question and select at least 2 profiles');
      return;
    }

    const effectiveProjectId = projectId || projects[0]?.id;
    if (!effectiveProjectId) {
      alert('No project selected. Please wait for projects to load or create a project first.');
      return;
    }

    if (!title.trim()) {
      setTitle(question.substring(0, 50));
    }

    setLoading(true);
    try {
      const runId = await Promise.race([
        api.createSession({
          project_id: effectiveProjectId,
          title: title || question.substring(0, 50),
          user_question: question,
          mode,
          selected_profile_ids: selectedProfiles,
          run_settings: { concurrency: 3, streaming: true },
        }),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error('Session creation timed out (45s). If a debate is running, use Stop & Leave first, then retry.')), 45000)
        ),
      ]);

      if (mode === 'parallel') {
        navigate(`/parallel-brainstorm/${runId}`);
      } else {
        navigate(`/debate-room/${runId}`);
      }
    } catch (error) {
      console.error('Failed to create session:', error);
      const msg = error instanceof Error ? error.message : String(error);
      alert(`Failed to create session: ${msg}\n\nIf you just returned from a debate, use Stop & Leave in the debate room first, then retry.`);
    } finally {
      setLoading(false);
    }
  };

  const toggleProfile = (profileId: string) => {
    setSelectedProfiles((prev) =>
      prev.includes(profileId)
        ? prev.filter((id) => id !== profileId)
        : [...prev, profileId]
    );
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
        <h1>New Session</h1>
        <p>Configure your AI session</p>
      </div>

      <div className="card">
        <div className="form-group">
          <label>Session Title</label>
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Enter session title..."
          />
        </div>

        <div className="form-group">
          <label>Question / Prompt</label>
          <textarea
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            placeholder="Enter your question or prompt here..."
            rows={5}
          />
        </div>

        <div className="form-group">
          <label>Mode</label>
          <select value={mode} onChange={(e) => setMode(e.target.value as 'parallel' | 'debate')}>
            <option value="parallel">Parallel Session</option>
            <option value="debate">Debate Room</option>
          </select>
        </div>

        <div className="form-group">
          <label>Select Profiles (2-10)</label>
          <div className="grid grid-3" style={{ marginTop: '10px' }}>
            {profiles.map((profile) => (
              <div
                key={profile.id}
                onClick={() => toggleProfile(profile.id)}
                style={{
                  padding: '15px',
                  border: '2px solid',
                  borderColor: selectedProfiles.includes(profile.id) ? '#007bff' : '#ddd',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  background: selectedProfiles.includes(profile.id) ? '#f0f8ff' : 'white',
                }}
              >
                <strong>{profile.name}</strong>
                <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '5px' }}>
                  {profile.model_name}
                </p>
              </div>
            ))}
          </div>
          {profiles.length === 0 && (
            <p style={{ color: 'var(--text-secondary)', marginTop: '10px' }}>
              No profiles available. Create profiles first.
            </p>
          )}
        </div>

        {projects.length === 0 && (
          <p style={{ color: 'var(--warning-text)', fontSize: '13px', marginTop: '10px' }}>
            Creating default project… Please wait.
          </p>
        )}
        <div style={{ marginTop: '20px' }}>
          <button 
            className="btn btn-primary" 
            onClick={handleStart}
            disabled={loading || !(projectId || projects[0]?.id)}
          >
            {loading ? 'Creating...' : 'Start Session'}
          </button>
        </div>
      </div>
    </div>
  );
}
