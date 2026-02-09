import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '../api';
import { useAppStore } from '../store';
import { ResponseCard } from '../components/ResponseCard';

interface RunResult {
  id: string;
  profile_id: string;
  status: string;
  raw_output_text?: string;
  error_message_safe?: string;
  started_at: string;
  finished_at?: string;
}

interface ContinueModalProps {
  isOpen: boolean;
  profileName: string;
  onClose: () => void;
  onSubmit: (message: string) => void;
}

function ContinueModal({ isOpen, profileName, onClose, onSubmit }: ContinueModalProps) {
  const [message, setMessage] = useState('');

  if (!isOpen) return null;

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
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: 'white',
          padding: '24px',
          borderRadius: '8px',
          width: '500px',
          maxWidth: '90%',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 style={{ margin: '0 0 16px 0' }}>Continue with {profileName}</h3>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '16px' }}>
          Enter a follow-up message to continue the conversation with this agent.
        </p>
        <textarea
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          placeholder="Enter your follow-up message..."
          style={{
            width: '100%',
            minHeight: '100px',
            padding: '10px',
            borderRadius: '4px',
            border: '1px solid #ddd',
            fontSize: '14px',
            resize: 'vertical',
            boxSizing: 'border-box',
          }}
        />
        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end', marginTop: '16px' }}>
          <button className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              if (message.trim()) {
                onSubmit(message);
                setMessage('');
              }
            }}
            disabled={!message.trim()}
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}

export function ParallelBrainstorm() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const { profiles, setProfiles } = useAppStore();
  const [runStatus, setRunStatus] = useState<any>(null);
  const [results, setResults] = useState<RunResult[]>([]);
  const [started, setStarted] = useState(false);
  const [sessionTitle, setSessionTitle] = useState<string>('');
  const [continueModal, setContinueModal] = useState<{ isOpen: boolean; resultId: string; profileName: string }>({
    isOpen: false,
    resultId: '',
    profileName: '',
  });

  useEffect(() => {
    if (!runId) {
      navigate('/home');
      return;
    }

    // Check run status first, then start if needed
    const initializeRun = async () => {
      try {
        // Always load profiles to ensure they're available
        try {
          const profilesData = await api.listProfiles();
          setProfiles(profilesData);
        } catch (error) {
          console.error('Failed to load profiles:', error);
        }

        const status = await api.getRunStatus(runId);
        setRunStatus(status);
        
        // Load session title
        if (status.session_id) {
          try {
            const session = await api.getSession(status.session_id);
            if (session && session.title) {
              setSessionTitle(session.title);
            }
          } catch (error) {
            console.error('Failed to load session:', error);
          }
        }
        
        // Load initial results
        try {
          const runResults = await api.getRunResults(runId);
          const uniqueResults = runResults.reduce((acc: RunResult[], result: RunResult) => {
            const existing = acc.find(r => r.profile_id === result.profile_id);
            if (!existing || new Date(result.started_at) > new Date(existing.started_at)) {
              return [...acc.filter(r => r.profile_id !== result.profile_id), result];
            }
            return acc;
          }, []);
          setResults(uniqueResults);
        } catch (error) {
          console.error('Failed to load initial results:', error);
        }
        
        // Only start if status is 'queued'
        if (status.status === 'queued') {
          await api.startRun(runId);
          setStarted(true);
        } else {
          setStarted(true);
          // Don't set loading to false here - let the polling interval handle it
          // This ensures we continue polling even if the run is already in progress
        }
      } catch (error) {
        console.error('Failed to initialize run:', error);
      }
    };

    initializeRun();

    // Poll for status and results
    const interval = setInterval(async () => {
      try {
        const status = await api.getRunStatus(runId);
        setRunStatus(status);

        const runResults = await api.getRunResults(runId);
        // Filter out duplicates by profile_id (keep the most recent one)
        const uniqueResults = runResults.reduce((acc: RunResult[], result: RunResult) => {
          const existing = acc.find(r => r.profile_id === result.profile_id);
          if (!existing || new Date(result.started_at) > new Date(existing.started_at)) {
            return [...acc.filter(r => r.profile_id !== result.profile_id), result];
          }
          return acc;
        }, []);
        setResults(uniqueResults);

        // Check if all results are complete (not just the run status)
        const allResultsComplete = uniqueResults.length > 0 && 
          uniqueResults.every((r: RunResult) => 
            r.status === 'complete' || 
            r.status === 'failed' || 
            r.status === 'cancelled'
          );

        // Stop polling only when:
        // 1. Run status is complete/failed/partial/cancelled AND
        // 2. All individual results are also complete/failed/cancelled
        // 3. We have at least one result (to avoid stopping too early)
        const shouldStop = (status.status === 'complete' || status.status === 'failed' || status.status === 'partial' || status.status === 'cancelled') 
            && allResultsComplete
            && uniqueResults.length > 0;
            
        if (shouldStop) {
          clearInterval(interval);
        }
      } catch (error) {
        console.error('Failed to fetch run status:', error);
      }
    }, 1000); // Poll every second

    return () => clearInterval(interval);
  }, [runId, navigate]);

  const getProfileName = (profileId: string) => {
    const profile = profiles.find(p => p.id === profileId);
    return profile?.name || profileId;
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'complete':
        return '#28a745';
      case 'running':
        return '#17a2b8';
      case 'failed':
        return '#dc3545';
      default:
        return '#6c757d';
    }
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'complete':
        return '✅';
      case 'running':
        return '⏳';
      case 'failed':
        return '❌';
      default:
        return '⏸️';
    }
  };

  if (!runId) {
    return null;
  }

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
        <h1>{sessionTitle || 'Parallel Session'}</h1>
        <p>Running parallel AI session</p>
      </div>

      {runStatus && (
        <div className="card" style={{ marginBottom: '20px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', justifyContent: 'space-between' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              <span style={{ fontSize: '24px' }}>{getStatusIcon(runStatus.status)}</span>
              <div>
                <strong>Status: </strong>
                <span style={{ color: getStatusColor(runStatus.status) }}>
                  {runStatus.status.toUpperCase()}
                </span>
              </div>
            </div>
            {runStatus.status === 'running' && (
              <button
                className="btn btn-secondary"
                style={{ fontSize: '12px', padding: '5px 15px' }}
                onClick={async () => {
                  if (window.confirm('Stop all agents?')) {
                    try {
                      await api.cancelRun(runId!);
                    } catch (error) {
                      console.error('Failed to cancel run:', error);
                      alert('Failed to cancel run');
                    }
                  }
                }}
              >
                ⏹️ Stop All
              </button>
            )}
          </div>
          {runStatus.status === 'running' && (
            <p style={{ color: 'var(--text-secondary)', marginTop: '10px', fontSize: '14px' }}>
              Agents are working on your question...
            </p>
          )}
        </div>
      )}

      <div className="grid grid-2">
        {results.map((result) => {
          const profileName = getProfileName(result.profile_id);
          return (
            <div 
              key={result.id} 
              className="card"
              style={{
                display: 'flex',
                flexDirection: 'column',
                maxHeight: '500px',
                overflow: 'hidden',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '15px', flexShrink: 0 }}>
                <span style={{ fontSize: '20px' }}>{getStatusIcon(result.status)}</span>
                <h3 style={{ margin: 0 }}>{profileName}</h3>
                <span
                  style={{
                    marginLeft: 'auto',
                    padding: '4px 8px',
                    background: getStatusColor(result.status),
                    color: 'white',
                    borderRadius: '4px',
                    fontSize: '12px',
                    fontWeight: 'bold',
                  }}
                >
                  {result.status}
                </span>
              </div>

              {result.status === 'running' && (
                <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)', flexShrink: 0 }}>
                  <div style={{ marginBottom: '10px' }}>⏳ Processing...</div>
                  <div style={{ fontSize: '12px', marginBottom: '10px' }}>Agent is thinking...</div>
                  <button
                    className="btn btn-secondary"
                    style={{ fontSize: '11px', padding: '4px 10px' }}
                    onClick={async () => {
                      if (window.confirm(`Stop ${profileName}?`)) {
                        try {
                          await api.cancelRunResult(result.id);
                        } catch (error) {
                          console.error('Failed to cancel result:', error);
                          alert('Failed to cancel agent');
                        }
                      }
                    }}
                  >
                    ⏹️ Stop
                  </button>
                </div>
              )}

              {result.status === 'complete' && result.raw_output_text && (
                <div
                  style={{
                    padding: '15px',
                    background: '#f8f9fa',
                    borderRadius: '4px',
                    flex: 1,
                    overflowY: 'auto',
                    minHeight: 0,
                  }}
                >
                  <ResponseCard
                    runResultId={result.id}
                    text={result.raw_output_text}
                    showMetadata={true}
                  />
                </div>
              )}

              {result.status === 'failed' && result.error_message_safe && (
                <div
                  style={{
                    padding: '15px',
                    background: '#fff3cd',
                    border: '1px solid #ffc107',
                    borderRadius: '4px',
                    color: '#856404',
                    flex: 1,
                    overflowY: 'auto',
                    minHeight: 0,
                  }}
                >
                  <strong>Error:</strong>
                  <div style={{ marginTop: '10px' }}>{result.error_message_safe}</div>
                </div>
              )}

              <div style={{ marginTop: '15px', fontSize: '12px', color: 'var(--text-secondary)', flexShrink: 0 }}>
                Started: {new Date(result.started_at).toLocaleTimeString()}
                {result.finished_at && (
                  <> • Finished: {new Date(result.finished_at).toLocaleTimeString()}</>
                )}
              </div>

              {(result.status === 'complete' || result.status === 'failed' || result.status === 'cancelled') && (
                <div style={{ marginTop: '10px', flexShrink: 0, display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                  {/* Rerun button - retry the same prompt */}
                  <button
                    className="btn btn-secondary"
                    style={{ fontSize: '11px', padding: '4px 10px' }}
                    onClick={async () => {
                      if (window.confirm(`Rerun ${profileName}?`)) {
                        try {
                          await api.rerunSingleAgent(runId!, result.profile_id);
                        } catch (error) {
                          console.error('Failed to rerun:', error);
                          alert('Failed to rerun agent');
                        }
                      }
                    }}
                  >
                    🔄 Rerun
                  </button>

                  {/* Continue button - follow up conversation */}
                  {result.status === 'complete' && (
                    <button
                      className="btn btn-secondary"
                      style={{ fontSize: '11px', padding: '4px 10px' }}
                      onClick={() => {
                        setContinueModal({
                          isOpen: true,
                          resultId: result.id,
                          profileName,
                        });
                      }}
                    >
                      💬 Continue
                    </button>
                  )}

                  {/* Edit button - edit the profile */}
                  <button
                    className="btn btn-secondary"
                    style={{ fontSize: '11px', padding: '4px 10px' }}
                    onClick={() => {
                      navigate(`/profiles?edit=${result.profile_id}`);
                    }}
                  >
                    ✏️ Edit Profile
                  </button>

                  {/* Delete button - remove this result */}
                  <button
                    className="btn btn-secondary"
                    style={{ fontSize: '11px', padding: '4px 10px', color: '#dc3545' }}
                    onClick={async () => {
                      if (window.confirm(`Delete this result from ${profileName}?`)) {
                        try {
                          await api.deleteRunResult(result.id);
                          // Remove from local state
                          setResults(results.filter(r => r.id !== result.id));
                        } catch (error) {
                          console.error('Failed to delete:', error);
                          alert('Failed to delete result');
                        }
                      }
                    }}
                  >
                    🗑️ Delete
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {results.length === 0 && started && (
        <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
          <div style={{ fontSize: '48px', marginBottom: '20px' }}>⏳</div>
          <p>Starting agents...</p>
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginTop: '10px' }}>
            Profiles are being loaded and agents are being initialized.
          </p>
        </div>
      )}

      {runStatus && (runStatus.status === 'complete' || runStatus.status === 'partial' || runStatus.status === 'cancelled') && (
        <div style={{ marginTop: '20px', display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
          <button
            className="btn btn-primary"
            onClick={() => navigate(`/compare/${runId}`)}
          >
            Compare Results
          </button>
          <button
            className="btn btn-secondary"
            onClick={async () => {
              // Create a new run for follow-up
              try {
                const sessionId = runStatus.session_id;
                if (!sessionId) {
                  alert('Session ID not available');
                  return;
                }
                // For now, navigate to session builder with pre-filled data
                // In future, we can add a reply feature
                navigate('/session-builder');
              } catch (error) {
                console.error('Failed to create follow-up:', error);
              }
            }}
          >
            Reply to All
          </button>
          <button
            className="btn btn-secondary"
            onClick={() => navigate('/home')}
          >
            Back to Home
          </button>
        </div>
      )}

      {/* Continue Modal */}
      <ContinueModal
        isOpen={continueModal.isOpen}
        profileName={continueModal.profileName}
        onClose={() => setContinueModal({ isOpen: false, resultId: '', profileName: '' })}
        onSubmit={async (message) => {
          try {
            await api.continueAgent(continueModal.resultId, message);
            setContinueModal({ isOpen: false, resultId: '', profileName: '' });
          } catch (error) {
            console.error('Failed to continue:', error);
            alert('Failed to send follow-up message');
          }
        }}
      />
    </div>
  );
}
