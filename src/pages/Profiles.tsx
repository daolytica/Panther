import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { api } from '../api';
import { ProfileEditor } from './ProfileEditor';

export function Profiles() {
  const navigate = useNavigate();
  const { profiles, setProfiles, providers } = useAppStore();
  const [showModal, setShowModal] = useState(false);
  const [editingProfile, setEditingProfile] = useState<string | null>(null);

  useEffect(() => {
    loadProfiles();
  }, []);

  const loadProfiles = async () => {
    try {
      const data = await api.listProfiles();
      setProfiles(data);
    } catch (error) {
      console.error('Failed to load profiles:', error);
    }
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
        <h1>Profiles</h1>
        <p>Manage agent profiles with character definitions and model features</p>
      </div>

      <div style={{ marginBottom: '20px' }}>
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          Create Profile
        </button>
      </div>

      <div className="grid grid-3">
        {profiles.length === 0 ? (
          <div className="card">
            <p style={{ color: 'var(--text-secondary)' }}>No profiles yet. Create your first profile to get started.</p>
          </div>
        ) : (
          profiles.map((profile) => (
            <div key={profile.id} className="card">
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '10px' }}>
                {profile.photo_url && (
                  <img
                    src={profile.photo_url}
                    alt={profile.name}
                    style={{
                      width: '50px',
                      height: '50px',
                      borderRadius: '50%',
                      objectFit: 'cover',
                      border: '2px solid var(--border-color)'
                    }}
                  />
                )}
                <h3 style={{ margin: 0 }}>{profile.name}</h3>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginTop: '5px' }}>
                <span style={{ color: 'var(--text-secondary)' }}>Model:</span>
                <strong style={{ color: '#007bff' }}>{profile.model_name}</strong>
              </div>
              {profile.provider_account_id && (
                <p style={{ color: 'var(--text-tertiary)', fontSize: '11px', marginTop: '3px' }}>
                  Provider: {providers.find(p => p.id === profile.provider_account_id)?.display_name || 'Unknown'}
                </p>
              )}
              
              {profile.character_definition && (
                <div style={{ marginTop: '10px', padding: '10px', background: 'var(--surface-elevated)', borderRadius: '4px' }}>
                  <strong style={{ fontSize: '12px' }}>Character:</strong>{' '}
                  {profile.character_definition.name || 'Unnamed'} - {profile.character_definition.role}
                  {profile.character_definition.personality && profile.character_definition.personality.length > 0 && (
                    <div style={{ marginTop: '5px', fontSize: '11px', color: 'var(--text-secondary)' }}>
                      Traits: {profile.character_definition.personality.slice(0, 3).join(', ')}
                      {profile.character_definition.personality.length > 3 && '...'}
                    </div>
                  )}
                </div>
              )}

              {profile.model_features && (
                <div style={{ marginTop: '10px', fontSize: '11px', color: 'var(--text-secondary)' }}>
                  {profile.model_features.supports_vision && '👁️ '}
                  {profile.model_features.supports_function_calling && '🔧 '}
                  {profile.model_features.supports_streaming && '📡 '}
                  {profile.model_features.max_context_length && (
                    <span>📏 {profile.model_features.max_context_length.toLocaleString()}</span>
                  )}
                </div>
              )}

              <p style={{ color: 'var(--text-tertiary)', fontSize: '12px', marginTop: '10px' }}>
                {profile.persona_prompt.substring(0, 100)}...
              </p>

              <div style={{ marginTop: '15px', display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                <button
                  className="btn btn-primary"
                  style={{ fontSize: '12px', padding: '5px 10px' }}
                  onClick={() => navigate(`/profile-chat/${profile.id}`)}
                >
                  💬 Chat
                </button>
                <button
                  className="btn btn-secondary"
                  style={{ fontSize: '12px', padding: '5px 10px' }}
                  onClick={() => {
                    setEditingProfile(profile.id);
                    setShowModal(true);
                  }}
                >
                  Edit
                </button>
              </div>
            </div>
          ))
        )}
      </div>

      {showModal && (
        <ProfileEditor
          profileId={editingProfile || undefined}
          onClose={() => {
            setShowModal(false);
            setEditingProfile(null);
          }}
          onSave={() => {
            loadProfiles();
            setShowModal(false);
            setEditingProfile(null);
          }}
        />
      )}
    </div>
  );
}
