import { useState, useEffect, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '../api';
import { useAppStore } from '../store';
import { WebSearchModal } from '../components/WebSearchModal';
import { ResponseCard } from '../components/ResponseCard';

interface DebateMessage {
  id: string;
  author_type: string;
  profile_id?: string;
  round_index?: number;
  turn_index?: number;
  text: string;
  created_at: string;
  usage?: {
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
  };
}

interface AddMessageModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (text: string, afterMessageId?: string) => void;
  afterMessageId?: string;
}

function AddMessageModal({ isOpen, onClose, onSubmit, afterMessageId }: AddMessageModalProps) {
  const [text, setText] = useState('');

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
        <h3 style={{ margin: '0 0 16px 0' }}>Add Message</h3>
        <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginBottom: '16px' }}>
          Insert a message into the conversation. This will be visible to all agents.
        </p>
        <textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Enter your message..."
          style={{
            width: '100%',
            minHeight: '100px',
            padding: '10px',
            borderRadius: '4px',
            border: '1px solid var(--border-color)',
            fontSize: '14px',
            resize: 'vertical',
            boxSizing: 'border-box',
          }}
        />
        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end', marginTop: '16px' }}>
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => {
              if (text.trim()) {
                onSubmit(text, afterMessageId);
                setText('');
              }
            }}
            disabled={!text.trim()}
          >
            Add Message
          </button>
        </div>
      </div>
    </div>
  );
}

export function DebateRoom() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const { profiles, setProfiles } = useAppStore();
  const [messages, setMessages] = useState<DebateMessage[]>([]);
  const [runStatus, setRunStatus] = useState<any>(null);
  const [started, setStarted] = useState(false);
  const [rounds] = useState(2);
  const [maxWords, setMaxWords] = useState<number | undefined>(undefined);
  const [debateLanguage, setDebateLanguage] = useState<string>('');
  const [debateTone, setDebateTone] = useState<string>('');
  const [webSearchModalOpen, setWebSearchModalOpen] = useState(false);
  const [webSearchResults, setWebSearchResults] = useState<any[]>([]);
  const [sessionTitle, setSessionTitle] = useState<string>('');
  const [addMessageModal, setAddMessageModal] = useState<{ isOpen: boolean; afterMessageId?: string }>({
    isOpen: false,
  });
  const [userAgreement, setUserAgreement] = useState<boolean>(false);
  const [totalTokens, setTotalTokens] = useState<number>(0);
  const [starting, setStarting] = useState<boolean>(false);
  const [pausing, setPausing] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [profilesError, setProfilesError] = useState<string | null>(null);
  const [longWait, setLongWait] = useState(false);
  const isMountedRef = useRef(true);

  const initializeDebate = async () => {
    if (!runId) return;
    setLoading(true);
    setLoadError(null);
    setProfilesError(null);
    try {
      // Always load profiles to ensure they're available
      try {
        const profilesData = await api.listProfiles();
        setProfiles(profilesData);
      } catch (error) {
        console.error('Failed to load profiles:', error);
        setProfilesError(error instanceof Error ? error.message : 'Failed to load profiles');
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

      // Get profile IDs from run status (which includes selected_profile_ids)
      let profileIds: string[] = [];
      if (status.selected_profile_ids && status.selected_profile_ids.length > 0) {
        profileIds = status.selected_profile_ids;
      } else {
        try {
          const results = await api.getRunResults(runId);
          if (results.length > 0) {
            profileIds = results.map((r: any) => r.profile_id);
          }
        } catch (error) {
          console.error('Failed to get results:', error);
        }
      }

      if (status.status !== 'queued') {
        setStarted(true);
      } else if (profileIds.length === 0) {
        console.error('No profile IDs found for debate. Status:', status);
      }
    } catch (error) {
      console.error('Failed to initialize debate:', error);
      setLoadError(error instanceof Error ? error.message : 'Failed to load debate');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!runId) {
      navigate('/home');
      return;
    }
    initializeDebate();
  }, [runId, navigate]);

  // Track mount state to avoid setState after unmount (prevents crash on Stop/navigate)
  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  // Poll for messages and status - use runId only to avoid restarting interval on every status update
  useEffect(() => {
    if (!runId) return;

    const poll = async () => {
      try {
        const status = await api.getRunStatus(runId);
        if (!isMountedRef.current) return;
        setRunStatus(status);

        const debateMessages = await api.getDebateMessages(runId);
        if (!isMountedRef.current) return;
        setMessages(debateMessages);

        const tokens = debateMessages.reduce((sum, msg) => {
          if (msg.usage?.total_tokens) return sum + msg.usage.total_tokens;
          return sum;
        }, 0);
        setTotalTokens(tokens);

        if (status.status === 'complete' || status.status === 'failed' || status.status === 'cancelled') {
          clearInterval(intervalId);
        }
      } catch (error) {
        if (isMountedRef.current) console.error('Failed to fetch debate messages:', error);
      }
    };

    poll(); // Immediate first poll
    const intervalId = setInterval(poll, 1000);

    return () => clearInterval(intervalId);
  }, [runId]);

  // Show "taking long" hint after 15s of running with no agent messages yet
  const hasAgentMessages = !!messages.find(m => m.author_type === 'agent');
  useEffect(() => {
    if (!runId || !runStatus || runStatus.status !== 'running' || hasAgentMessages) {
      setLongWait(false);
      return;
    }
    const t = setTimeout(() => setLongWait(true), 15000);
    return () => clearTimeout(t);
  }, [runId, runStatus?.status, hasAgentMessages]);

  const getProfileName = (profileId?: string) => {
    if (!profileId) return 'Unknown';
    const profile = profiles.find(p => p.id === profileId);
    return profile?.name || profileId;
  };

  const getProfilePhoto = (profileId?: string) => {
    if (!profileId) return null;
    const profile = profiles.find(p => p.id === profileId);
    return profile?.photo_url || null;
  };


  // Sort messages chronologically (like a messaging app)
  const sortedMessages = [...messages].sort((a, b) => {
    const timeA = new Date(a.created_at).getTime();
    const timeB = new Date(b.created_at).getTime();
    return timeA - timeB;
  });

  const formatDateTime = (dateString: string) => {
    const date = new Date(dateString);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();
    
    if (isToday) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } else {
      return date.toLocaleString([], { 
        month: 'short', 
        day: 'numeric', 
        hour: '2-digit', 
        minute: '2-digit' 
      });
    }
  };

  const wordCount = (text: string) => {
    return text.trim().split(/\s+/).filter(w => w.length > 0).length;
  };

  return (
    <div>
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px', marginBottom: '10px' }}>
          <button
            type="button"
            onClick={async () => {
              if (runId && runStatus && (runStatus.status === 'running' || runStatus.status === 'paused')) {
                try {
                  await api.cancelDebate(runId, { timeoutMs: 3000 });
                } catch (e) {
                  console.warn('Cancel on back may have timed out:', e);
                }
              }
              navigate('/home');
            }}
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
          {(runStatus?.status === 'running' || runStatus?.status === 'paused') && (
            <button
              type="button"
              className="btn btn-secondary"
              style={{ color: '#dc3545' }}
              disabled={stopping}
              onClick={async () => {
                if (!window.confirm('Stop the debate and return home?')) return;
                setStopping(true);
                try {
                  await api.cancelDebate(runId!, { timeoutMs: 5000 });
                } catch (e) {
                  console.warn('Cancel may have timed out:', e);
                }
                setStopping(false);
                navigate('/home');
              }}
            >
              {stopping ? 'Stopping...' : '⏹️ Stop & Leave'}
            </button>
          )}
        </div>
        <h1>{sessionTitle || 'Debate Room'}</h1>
        <p>Real-time agent discussion</p>
      </div>

      {loading && (
        <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
          <div style={{ fontSize: '24px', marginBottom: '12px' }}>⏳</div>
          <p>Loading debate...</p>
        </div>
      )}

      {loadError && !loading && (
        <div className="card" style={{ padding: '24px', borderColor: 'var(--error-text)' }}>
          <p style={{ color: 'var(--error-text)', marginBottom: '16px' }}>
            Failed to load debate: {loadError}
          </p>
          {profilesError && (
            <p style={{ color: 'var(--text-secondary)', fontSize: '13px', marginBottom: '16px' }}>
              Profiles: {profilesError}
            </p>
          )}
          <button type="button" className="btn btn-primary" onClick={() => initializeDebate()}>
            Retry
          </button>
        </div>
      )}

      {runStatus && !loading && (
        <div className="card" style={{ marginBottom: '20px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', justifyContent: 'space-between', flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
              <span style={{ fontSize: '24px' }}>
                {runStatus.status === 'running' ? '⏳' : runStatus.status === 'complete' ? '✅' : runStatus.status === 'paused' ? '⏸️' : runStatus.status === 'failed' ? '❌' : '⏹️'}
              </span>
              <div>
                <strong>Status: </strong>
                <span style={{ 
                  color: runStatus.status === 'complete' ? '#28a745' : 
                         runStatus.status === 'running' ? '#17a2b8' : 
                         runStatus.status === 'paused' ? '#ffc107' : 
                         runStatus.status === 'failed' ? '#dc3545' : '#6c757d' 
                }}>
                  {runStatus.status.toUpperCase()}
                </span>
                {runStatus.status === 'failed' && runStatus.error_message && (
                  <div style={{ fontSize: '13px', color: '#dc3545', marginTop: '6px', maxWidth: '400px' }}>
                    Error: {runStatus.error_message}
                  </div>
                )}
              </div>
              {totalTokens > 0 && (
                <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                  Total Tokens: {totalTokens.toLocaleString()}
                </div>
              )}
            </div>
            <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center' }}>
              {runStatus.status === 'queued' && runStatus.selected_profile_ids && runStatus.selected_profile_ids.length > 0 && (
                <>
                  <div style={{ 
                    display: 'flex', 
                    gap: '15px', 
                    alignItems: 'center', 
                    flexWrap: 'wrap',
                    padding: '15px',
                    background: '#f8f9fa',
                    borderRadius: '8px',
                    border: '1px solid #dee2e6',
                    marginBottom: '10px',
                    width: '100%'
                  }}>
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flex: '1', minWidth: '200px' }}>
                      <label style={{ fontSize: '12px', color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>Max words:</label>
                      <input
                        type="number"
                        placeholder="Optional"
                        value={maxWords || ''}
                        onChange={(e) => setMaxWords(e.target.value ? parseInt(e.target.value) : undefined)}
                        min="1"
                        style={{
                          width: '100px',
                          padding: '5px 10px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                        }}
                      />
                    </div>
                    
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flex: '1', minWidth: '200px' }}>
                      <label style={{ fontSize: '12px', color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>Language:</label>
                      <select
                        value={debateLanguage}
                        onChange={(e) => setDebateLanguage(e.target.value)}
                        style={{
                          flex: 1,
                          padding: '5px 10px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                          fontSize: '13px'
                        }}
                      >
                        <option value="">Default (English)</option>
                        <option value="English">English</option>
                        <option value="Farsi">فارسی (Farsi)</option>
                        <option value="Spanish">Español</option>
                        <option value="French">Français</option>
                        <option value="German">Deutsch</option>
                        <option value="Arabic">العربية (Arabic)</option>
                        <option value="Chinese">中文 (Chinese)</option>
                        <option value="Japanese">日本語 (Japanese)</option>
                      </select>
                    </div>
                    
                    <div style={{ display: 'flex', gap: '8px', alignItems: 'flex-start', flex: '2', minWidth: '300px' }}>
                      <label style={{ fontSize: '12px', color: 'var(--text-secondary)', whiteSpace: 'nowrap', marginTop: '5px' }}>Tone:</label>
                      <textarea
                        placeholder="e.g., friendly, serious, technical, funny, professional, casual, academic..."
                        value={debateTone}
                        onChange={(e) => setDebateTone(e.target.value)}
                        rows={2}
                        style={{
                          flex: 1,
                          padding: '5px 10px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                          fontSize: '13px',
                          resize: 'vertical',
                          minHeight: '40px'
                        }}
                      />
                    </div>
                  </div>
                  
                  <div style={{ display: 'flex', gap: '10px', marginTop: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
                    <button
                      type="button"
                      onClick={() => setWebSearchModalOpen(true)}
                      className="btn"
                      style={{
                        padding: '6px 12px',
                        fontSize: '13px',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '6px'
                      }}
                    >
                      🌐 {webSearchResults.length > 0 ? `Web Search (${webSearchResults.length})` : 'Web Search'}
                    </button>
                    {webSearchResults.length > 0 && (
                      <button
                        type="button"
                        onClick={() => setWebSearchResults([])}
                        style={{
                          padding: '6px 12px',
                          fontSize: '12px',
                          background: 'var(--error-bg)',
                          color: 'var(--error-text)',
                          border: '1px solid #fcc',
                          borderRadius: '4px',
                          cursor: 'pointer'
                        }}
                      >
                        Clear Results
                      </button>
                    )}
                  </div>
                  
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={starting}
                    onClick={async () => {
                      if (!runStatus.selected_profile_ids || runStatus.selected_profile_ids.length === 0) {
                        alert('No profiles selected');
                        return;
                      }
                      setStarting(true);
                      try {
                        console.log('Starting debate with:', { runId, rounds, profileIds: runStatus.selected_profile_ids, maxWords, language: debateLanguage, tone: debateTone });
                        await api.startDebate(
                          runId!, 
                          rounds, 
                          runStatus.selected_profile_ids, 
                          maxWords,
                          debateLanguage || undefined,
                          debateTone || undefined,
                          webSearchResults.length > 0 ? webSearchResults : undefined
                        );
                        setStarted(true);
                        // Give it a moment to update status
                        setTimeout(() => {
                          setStarting(false);
                        }, 1000);
                      } catch (error) {
                        console.error('Failed to start debate:', error);
                        alert(`Failed to start debate: ${error}`);
                        setStarting(false);
                      }
                    }}
                  >
                    {starting ? 'Starting...' : '▶️ Start Debate'}
                  </button>
                  
                  <WebSearchModal
                    isOpen={webSearchModalOpen}
                    onClose={() => setWebSearchModalOpen(false)}
                    onConfirm={(results) => setWebSearchResults(results)}
                    initialQuery={runStatus?.user_question || ''}
                    initialResults={webSearchResults}
                  />
                </>
              )}
              {runStatus.status === 'running' && (
                <>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={pausing || stopping}
                    onClick={async (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      setPausing(true);
                      try {
                        await api.pauseDebate(runId!);
                        // Poll will pick up status within 1s; avoid extra getRunStatus that may block
                        setRunStatus((prev: any) => (prev ? { ...prev, status: 'paused' } : prev));
                      } catch (error) {
                        console.error('Failed to pause:', error);
                        alert(error instanceof Error ? error.message : 'Failed to pause debate');
                      } finally {
                        setPausing(false);
                      }
                    }}
                  >
                    {pausing ? '⏸️ Pausing...' : '⏸️ Pause'}
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={pausing || stopping}
                    style={{ color: '#dc3545', cursor: 'pointer' }}
                    onClick={async (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      if (!window.confirm('Stop the debate? The current agent may finish its response first (up to 60s).')) return;
                      setStopping(true);
                      try {
                        await api.cancelDebate(runId!, { timeoutMs: 8000 });
                      } catch (error) {
                        console.warn('Cancel may have timed out:', error);
                      }
                      setRunStatus((prev: any) => (prev ? { ...prev, status: 'cancelled' } : prev));
                      setStopping(false);
                    }}
                  >
                    {stopping ? '⏹️ Stopping...' : '⏹️ Stop'}
                  </button>
                </>
              )}
              {runStatus.status === 'paused' && (
                <>
                  <button
                    type="button"
                    className="btn btn-primary"
                    disabled={stopping}
                    onClick={async (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      try {
                        await api.resumeDebate(runId!);
                        setRunStatus((prev: any) => (prev ? { ...prev, status: 'running' } : prev));
                      } catch (error) {
                        console.error('Failed to resume:', error);
                        alert(error instanceof Error ? error.message : 'Failed to resume debate');
                      }
                    }}
                  >
                    ▶️ Resume
                  </button>
                  <button
                    type="button"
                    className="btn btn-secondary"
                    disabled={stopping}
                    style={{ color: '#dc3545', cursor: 'pointer' }}
                    onClick={async (e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      if (!window.confirm('Stop the debate? The current agent may finish its response first (up to 60s).')) return;
                      setStopping(true);
                      try {
                        await api.cancelDebate(runId!, { timeoutMs: 8000 });
                      } catch (error) {
                        console.warn('Cancel may have timed out:', error);
                      }
                      setRunStatus((prev: any) => (prev ? { ...prev, status: 'cancelled' } : prev));
                      setStopping(false);
                    }}
                  >
                    {stopping ? '⏹️ Stopping...' : '⏹️ Stop'}
                  </button>
                </>
              )}
              {(runStatus.status === 'complete' || runStatus.status === 'partial') && (
                <>
                  {!userAgreement && (
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={async () => {
                        try {
                          await api.continueDebate(runId!, rounds);
                        } catch (error) {
                          console.error('Failed to continue:', error);
                          alert('Failed to continue debate');
                        }
                      }}
                    >
                      🔄 Continue Debate
                    </button>
                  )}
                  <button
                    className="btn btn-secondary"
                    onClick={() => setUserAgreement(!userAgreement)}
                    style={{ background: userAgreement ? '#28a745' : undefined, color: userAgreement ? 'white' : undefined }}
                  >
                    {userAgreement ? '✅ Agreement Verified' : '✓ Verify Agreement'}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      <div style={{ 
        maxHeight: '600px', 
        overflowY: 'auto', 
        marginBottom: '20px',
        background: 'var(--surface-elevated)',
        borderRadius: '8px',
        padding: '20px',
        border: '1px solid var(--border-color)'
      }}>
        {sortedMessages.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: '40px' }}>
            No messages yet. Start the debate to begin the conversation.
          </div>
        ) : (
          sortedMessages.map((msg, idx) => {
            const isAgent = msg.author_type === 'agent';
            const isUser = msg.author_type === 'user';
            const prevMsg = idx > 0 ? sortedMessages[idx - 1] : null;
            const showDateSeparator = !prevMsg || 
              new Date(msg.created_at).toDateString() !== new Date(prevMsg.created_at).toDateString();
            
            return (
              <div key={msg.id}>
                {showDateSeparator && (
                  <div style={{ 
                    textAlign: 'center', 
                    margin: '20px 0',
                    color: 'var(--text-secondary)',
                    fontSize: '12px',
                    fontWeight: 'bold'
                  }}>
                    {new Date(msg.created_at).toLocaleDateString([], { 
                      weekday: 'long', 
                      year: 'numeric', 
                      month: 'long', 
                      day: 'numeric' 
                    })}
                  </div>
                )}
                <div style={{
                  display: 'flex',
                  flexDirection: 'column',
                  marginBottom: '12px',
                  alignItems: isAgent ? 'flex-start' : isUser ? 'flex-end' : 'flex-start',
                }}>
                  <div style={{
                    maxWidth: '70%',
                    background: isAgent ? '#e3f2fd' : isUser ? '#e8f5e9' : '#fff',
                    borderRadius: '12px',
                    padding: '12px 16px',
                    boxShadow: '0 1px 2px rgba(0,0,0,0.1)',
                    border: `1px solid ${isAgent ? 'var(--highlight-bg)' : isUser ? 'var(--success-bg)' : 'var(--border-color)'}`,
                  }}>
                    <div style={{ 
                      display: 'flex', 
                      alignItems: 'center', 
                      gap: '8px', 
                      marginBottom: '6px',
                      flexWrap: 'wrap'
                    }}>
                      {isAgent && getProfilePhoto(msg.profile_id) && (
                        <img
                          src={getProfilePhoto(msg.profile_id)!}
                          alt={getProfileName(msg.profile_id)}
                          style={{
                            width: '24px',
                            height: '24px',
                            borderRadius: '50%',
                            objectFit: 'cover',
                            border: '1px solid var(--border-color)'
                          }}
                        />
                      )}
                      <strong style={{ 
                        color: isAgent ? '#1976d2' : isUser ? '#388e3c' : '#666',
                        fontSize: '14px'
                      }}>
                        {isAgent ? getProfileName(msg.profile_id) : isUser ? 'You' : msg.author_type}
                      </strong>
                      <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                        {formatDateTime(msg.created_at)}
                      </span>
                      {msg.usage && (
                        <span style={{ 
                          fontSize: '10px', 
                          color: 'var(--text-secondary)', 
                          background: 'rgba(0,0,0,0.05)', 
                          padding: '2px 6px', 
                          borderRadius: '10px' 
                        }}>
                          {msg.usage.total_tokens || (msg.usage.prompt_tokens || 0) + (msg.usage.completion_tokens || 0)} tokens
                        </span>
                      )}
                      {maxWords && isAgent && (
                        <span style={{ 
                          fontSize: '10px', 
                          color: wordCount(msg.text) > maxWords ? '#d32f2f' : '#388e3c', 
                          background: 'rgba(0,0,0,0.05)', 
                          padding: '2px 6px', 
                          borderRadius: '10px' 
                        }}>
                          {wordCount(msg.text)}/{maxWords} words
                        </span>
                      )}
                    </div>
                    <div style={{ 
                      lineHeight: '1.5',
                      color: 'var(--text-primary)',
                      fontSize: '14px'
                    }}>
                      {msg.author_type === 'agent' && msg.id ? (
                        <ResponseCard
                          runResultId={msg.id}
                          text={msg.text}
                          showMetadata={false}
                          skipMetadataLoading={true}
                        />
                      ) : (
                        <span style={{ whiteSpace: 'pre-wrap' }}>{msg.text}</span>
                      )}
                    </div>
                    <div style={{ 
                      display: 'flex', 
                      gap: '4px', 
                      marginTop: '8px',
                      justifyContent: 'flex-end'
                    }}>
                      {isAgent && (
                        <button
                          className="btn btn-secondary"
                          style={{ fontSize: '10px', padding: '2px 6px', minWidth: 'auto' }}
                          onClick={() => {
                            alert(`Award token to ${getProfileName(msg.profile_id)} - Feature coming soon!`);
                          }}
                          title="Award token to this agent"
                        >
                          🏆
                        </button>
                      )}
                      <button
                        className="btn btn-secondary"
                        style={{ fontSize: '10px', padding: '2px 6px', minWidth: 'auto', color: '#d32f2f' }}
                        onClick={async () => {
                          if (window.confirm('Delete this message?')) {
                            try {
                              await api.deleteDebateMessage(msg.id);
                            } catch (error) {
                              console.error('Failed to delete:', error);
                              alert('Failed to delete message');
                            }
                          }
                        }}
                        title="Delete message"
                      >
                        🗑️
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        style={{ fontSize: '10px', padding: '2px 6px', minWidth: 'auto' }}
                        onClick={() => {
                          setAddMessageModal({ isOpen: true, afterMessageId: msg.id });
                        }}
                        title="Add message after this"
                      >
                        ➕
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      {runStatus && (
      <div style={{ marginBottom: '20px', display: 'flex', gap: '10px', alignItems: 'center' }}>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={() => setAddMessageModal({ isOpen: true })}
        >
          ➕ Add Message
        </button>
        {(runStatus.status === 'complete' || runStatus.status === 'partial') && (
          <>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => navigate(`/compare/${runId}`)}
            >
              Compare & Synthesis
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => navigate('/home')}
            >
              Back to Home
            </button>
          </>
        )}
      </div>
      )}

      {messages.length === 0 && runStatus && runStatus.status === 'queued' && (
        <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
          <div style={{ fontSize: '48px', marginBottom: '20px' }}>⏸️</div>
          <p>Debate Ready to Start</p>
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginTop: '10px' }}>
            Configure word limit (optional) and click "Start Debate" to begin.
          </p>
        </div>
      )}

      {messages.length === 0 && started && runStatus && runStatus.status === 'running' && (
        <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
          <div style={{ fontSize: '48px', marginBottom: '20px' }}>⏳</div>
          <p>Debate starting...</p>
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px', marginTop: '10px' }}>
            Agents are preparing their opening statements.
          </p>
          {longWait && (
            <p style={{ color: 'var(--warning-text)', fontSize: '13px', marginTop: '16px' }}>
              Taking longer than expected? Ensure your AI providers (Settings → Providers) have valid API keys. Check the terminal/console for [Debate] logs. You can use Stop &amp; Leave to exit.
            </p>
          )}
        </div>
      )}
      {messages.length > 0 && !!messages.find(m => m.author_type === 'user') && !messages.find(m => m.author_type === 'agent') && runStatus?.status === 'running' && (
        <div className="card" style={{ textAlign: 'center', padding: '16px', marginTop: '12px' }}>
          <p style={{ color: 'var(--text-secondary)', fontSize: '14px', margin: 0 }}>
            ⏳ Agents are preparing their opening statements…
          </p>
          {longWait && (
            <p style={{ color: 'var(--warning-text)', fontSize: '13px', marginTop: '10px' }}>
              Taking longer than expected? Check your AI providers (Settings → Providers) and terminal for [Debate] logs.
            </p>
          )}
        </div>
      )}

      <AddMessageModal
        isOpen={addMessageModal.isOpen}
        onClose={() => setAddMessageModal({ isOpen: false })}
        onSubmit={async (text, afterMessageId) => {
          try {
            await api.addUserMessage(runId!, text, afterMessageId);
            setAddMessageModal({ isOpen: false });
          } catch (error) {
            console.error('Failed to add message:', error);
            alert('Failed to add message');
          }
        }}
        afterMessageId={addMessageModal.afterMessageId}
      />
    </div>
  );
}
