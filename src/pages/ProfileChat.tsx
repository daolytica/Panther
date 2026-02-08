import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { api } from '../api';
import { useAppStore } from '../store';
import { ExportChatModal } from '../components/ExportChatModal';
import { VoiceInput } from '../components/VoiceInput';
import { VoiceOutput } from '../components/VoiceOutput';
import { useStreamingLLM } from '../hooks/useStreamingLLM';
import { startRecording, startRecordingWithSilenceDetection, type RecordingSession } from '../utils/audioRecorder';
interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
  profile_id?: string;
}

type ModelPreference = 'default' | 'local' | 'cloud';

export function ProfileChat() {
  const { profileId } = useParams<{ profileId: string }>();
  const navigate = useNavigate();
  const { profiles, providers, projects, setProjects, language, setProviders, continuousAutoSend, setContinuousAutoSend, autoSpeakResponses } = useAppStore();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [streamingAssistantId, setStreamingAssistantId] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Array<{ id: string; title: string; created_at: string; updated_at: string }>>([]);
  const [currentConversationId, setCurrentConversationId] = useState<string | null>(null);
  const [selectedMessages, setSelectedMessages] = useState<Set<string>>(new Set());
  const [improvementPrompts, setImprovementPrompts] = useState<Record<string, string>>({});
  const [showExportModal, setShowExportModal] = useState(false);
  const [chatLanguage, setChatLanguage] = useState<string>(language || 'English');
  const [webSearchResults, setWebSearchResults] = useState<any[]>([]);
  const [searchWeb, setSearchWeb] = useState(false);
  const [modelPreference, setModelPreference] = useState<ModelPreference>('default');
  const [improvingMessageId, setImprovingMessageId] = useState<string | null>(null);
  const [attachedDocuments, setAttachedDocuments] = useState<Array<{ name: string; content: string; fromTraining?: boolean }>>([]);
  const [showLoadTrainingData, setShowLoadTrainingData] = useState(false);
  const [loadAsTalkTo, setLoadAsTalkTo] = useState(false); // true = "Talk to Trained Data" mode
  const [loadingTrainingData, setLoadingTrainingData] = useState(false);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [conversationMode, setConversationMode] = useState<'push-to-talk' | 'continuous'>('push-to-talk');
  const [isContinuousActive, setIsContinuousActive] = useState(false);
  const [continuousListening, setContinuousListening] = useState(false);
  const [autoPlayMessage, setAutoPlayMessage] = useState<{ id: string; content: string } | null>(null);
  const recordingRef = useRef<RecordingSession | null>(null);
  const messagesRef = useRef<ChatMessage[]>([]);
  const prevStreamingRef = useRef(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const trainingDataDropdownRef = useRef<HTMLDivElement>(null);
  messagesRef.current = messages;

  useEffect(() => {
    if (!showLoadTrainingData) return;
    const onOutside = (e: MouseEvent) => {
      if (trainingDataDropdownRef.current && !trainingDataDropdownRef.current.contains(e.target as Node)) {
        setShowLoadTrainingData(false);
      }
    };
    document.addEventListener('mousedown', onOutside);
    return () => document.removeEventListener('mousedown', onOutside);
  }, [showLoadTrainingData]);

  const profile = profiles.find(p => p.id === profileId);
  const profileProvider = profile ? providers.find(p => p.id === profile.provider_account_id) : null;
  const isHybrid = profileProvider?.provider_type === 'hybrid';

  const streamingConfig = useMemo(
    () =>
      profileId && profile
        ? {
            type: 'profile' as const,
            profileId,
            conversationId: currentConversationId ?? undefined,
            conversationContext: [] as any[],
            language: chatLanguage !== 'English' ? chatLanguage : undefined,
            modelPreference: isHybrid ? modelPreference : undefined,
          }
        : null,
    [profileId, profile, currentConversationId, chatLanguage, isHybrid, modelPreference]
  );
  const { streamedText, isStreaming, error, startStream } = useStreamingLLM(streamingConfig);

  useEffect(() => {
    if (!profileId) {
      navigate('/profiles');
      return;
    }

    if (profiles.length === 0) {
      api.listProfiles().then(data => {
        const { setProfiles } = useAppStore.getState();
        setProfiles(data);
      }).catch(err => {
        console.error('Failed to load profiles:', err);
      });
    }
    if (providers.length === 0) {
      api.listProviders().then(data => {
        setProviders(data);
      }).catch(err => {
        console.error('Failed to load providers:', err);
      });
    }
    
    // Load conversations and messages
    const loadConversations = async () => {
      try {
        const list = await api.listProfileConversations(profileId!);
        setConversations(list);
        if (list.length > 0 && !currentConversationId) {
          setCurrentConversationId(list[0].id);
        }
      } catch (error) {
        console.error('Failed to load conversations:', error);
        setConversations([]);
      }
    };

    const loadMessages = async (convId: string | null) => {
      try {
        const savedMessages = await api.loadChatMessages(profileId!, convId ?? undefined);
        const chatMessages: ChatMessage[] = savedMessages.map((msg: any) => ({
          id: msg.id,
          role: msg.role as 'user' | 'assistant',
          content: msg.content,
          timestamp: msg.timestamp,
          profile_id: msg.profile_id,
        }));
        setMessages(chatMessages);
      } catch (error) {
        console.error('Failed to load chat messages:', error);
        setMessages([]);
      }
    };

    setCurrentConversationId(null);
    loadConversations();
    setWebSearchResults([]);
  }, [profileId, navigate, profiles.length]);

  // Load messages when conversation changes
  useEffect(() => {
    if (!profileId || !currentConversationId) {
      setMessages([]);
      return;
    }
    api.loadChatMessages(profileId, currentConversationId).then((savedMessages) => {
      const chatMessages: ChatMessage[] = savedMessages.map((msg: any) => ({
        id: msg.id,
        role: msg.role as 'user' | 'assistant',
        content: msg.content,
        timestamp: msg.timestamp,
        profile_id: msg.profile_id,
      }));
      setMessages(chatMessages);
    }).catch(() => setMessages([]));
  }, [profileId, currentConversationId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  // Sync streamed text to assistant message
  useEffect(() => {
    if (!streamingAssistantId || streamedText === undefined) return;
    setMessages(prev =>
      prev.map((msg) =>
        msg.id === streamingAssistantId ? { ...msg, content: streamedText } : msg
      )
    );
  }, [streamedText, streamingAssistantId]);

  // Update assistant message with error and clear streaming id when done
  useEffect(() => {
    if (!isStreaming && streamingAssistantId && error) {
      setMessages(prev =>
        prev.map((msg) =>
          msg.id === streamingAssistantId ? { ...msg, content: `Error: ${error}` } : msg
        )
      );
      setStreamingAssistantId(null);
    } else if (!isStreaming && streamingAssistantId) {
      setStreamingAssistantId(null);
    }
  }, [isStreaming, streamingAssistantId, error]);

  // When streaming completes, auto-play the new assistant message if enabled
  useEffect(() => {
    // Auto-play if in continuous mode OR if autoSpeakResponses is enabled
    const shouldAutoPlay = isContinuousActive || autoSpeakResponses;
    if (prevStreamingRef.current && !isStreaming && shouldAutoPlay) {
      const timer = setTimeout(() => {
        const msgs = messagesRef.current;
        const lastAssistant = [...msgs].reverse().find(m => m.role === 'assistant' && m.content);
        if (lastAssistant) setAutoPlayMessage({ id: lastAssistant.id, content: lastAssistant.content });
      }, 100);
      prevStreamingRef.current = isStreaming;
      return () => clearTimeout(timer);
    }
    prevStreamingRef.current = isStreaming;
  }, [isStreaming, isContinuousActive, autoSpeakResponses]);

  const sendMessageWithDocs = useCallback(async (messageText: string, docs: Array<{ name: string; content: string }>) => {
    if (!messageText.trim() || !profile || isStreaming) return;

    let convId = currentConversationId;
    if (!convId) {
      try {
        convId = await api.createProfileConversation(profileId!);
        setCurrentConversationId(convId);
        setConversations(prev => [{ id: convId!, title: 'New conversation', created_at: new Date().toISOString(), updated_at: new Date().toISOString() }, ...prev]);
      } catch (err) {
        console.error('Failed to create conversation:', err);
        return;
      }
    }

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: messageText.trim(),
      timestamp: new Date().toISOString(),
    };
    const assistantId = (Date.now() + 1).toString();
    const assistantPlaceholder: ChatMessage = {
      id: assistantId,
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
      profile_id: profileId!,
    };
    setMessages(prev => [...prev, userMessage, assistantPlaceholder]);
    setStreamingAssistantId(assistantId);
    setAttachedDocuments(docs);

    const conversationContext = [
      ...messagesRef.current
        .filter((msg) => msg.role === 'user' || (msg.role === 'assistant' && msg.profile_id === profileId))
        .map((msg) => ({
          id: msg.id,
          run_id: `chat-${profileId}`,
          author_type: msg.role === 'user' ? 'user' : 'assistant',
          profile_id: msg.role === 'assistant' ? profileId : null,
          round_index: null,
          turn_index: null,
          text: msg.content,
          created_at: msg.timestamp,
          provider_metadata_json: null,
        })),
      { id: userMessage.id, run_id: `chat-${profileId}`, author_type: 'user' as const, profile_id: null, round_index: null, turn_index: null, text: userMessage.content, created_at: userMessage.timestamp, provider_metadata_json: null },
    ];
    setWebSearchResults([]);
    await startStream(messageText.trim(), {
      conversationContext,
      conversationId: convId,
      attachedDocuments: docs.length > 0 ? docs : undefined,
    });
    setAttachedDocuments([]);
  }, [profile, profileId, currentConversationId, isStreaming, startStream]);

  const handleSend = async () => {
    if (!inputMessage.trim() || !profile || isStreaming) return;

    // Ensure we have a conversation (create if first message)
    let convId = currentConversationId;
    if (!convId) {
      try {
        convId = await api.createProfileConversation(profileId!);
        setCurrentConversationId(convId);
        setConversations(prev => [{ id: convId!, title: 'New conversation', created_at: new Date().toISOString(), updated_at: new Date().toISOString() }, ...prev]);
      } catch (err) {
        console.error('Failed to create conversation:', err);
        return;
      }
    }

    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: inputMessage.trim(),
      timestamp: new Date().toISOString(),
    };

    const assistantId = (Date.now() + 1).toString();
    const assistantPlaceholder: ChatMessage = {
      id: assistantId,
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
      profile_id: profileId!,
    };

    setMessages(prev => [...prev, userMessage, assistantPlaceholder]);
    setStreamingAssistantId(assistantId);
    const currentInput = inputMessage.trim();
    setInputMessage('');

    const conversationContext = messages
      .filter((msg) => msg.role === 'user' || (msg.role === 'assistant' && msg.profile_id === profileId))
      .map((msg) => ({
        id: msg.id,
        run_id: `chat-${profileId}`,
        author_type: msg.role === 'user' ? 'user' : 'assistant',
        profile_id: msg.role === 'assistant' ? profileId : null,
        round_index: null,
        turn_index: null,
        text: msg.content,
        created_at: msg.timestamp,
        provider_metadata_json: null,
      }));

    setWebSearchResults([]);
    setAttachedDocuments([]);

    await startStream(currentInput, {
      conversationContext,
      conversationId: convId,
      webSearchResults: webSearchResults.length > 0 ? webSearchResults : undefined,
      attachedDocuments: attachedDocuments.length > 0 ? attachedDocuments : undefined,
    });
  };

  const handleNewConversation = async () => {
    if (!profileId) return;
    try {
      const convId = await api.createProfileConversation(profileId);
      setCurrentConversationId(convId);
      setMessages([]);
      setConversations(prev => [{ id: convId, title: 'New conversation', created_at: new Date().toISOString(), updated_at: new Date().toISOString() }, ...prev]);
    } catch (err) {
      console.error('Failed to create conversation:', err);
    }
  };

  const handleSelectConversation = (convId: string) => {
    setCurrentConversationId(convId);
  };

  const stopContinuousListeningAndSendRef = useRef<() => Promise<void>>(() => Promise.resolve());

  const handleSendWithText = useCallback(async (textToSend: string) => {
    if (!textToSend.trim() || !profile || isStreaming) return;
    let convId = currentConversationId;
    if (!convId) {
      try {
        convId = await api.createProfileConversation(profileId!);
        setCurrentConversationId(convId);
        setConversations(prev => [{ id: convId!, title: 'New conversation', created_at: new Date().toISOString(), updated_at: new Date().toISOString() }, ...prev]);
      } catch (err) {
        console.error('Failed to create conversation:', err);
        return;
      }
    }
    const userMessage: ChatMessage = {
      id: Date.now().toString(),
      role: 'user',
      content: textToSend.trim(),
      timestamp: new Date().toISOString(),
    };
    const assistantId = (Date.now() + 1).toString();
    const assistantPlaceholder: ChatMessage = {
      id: assistantId,
      role: 'assistant',
      content: '',
      timestamp: new Date().toISOString(),
      profile_id: profileId!,
    };
    setMessages(prev => [...prev, userMessage, assistantPlaceholder]);
    setStreamingAssistantId(assistantId);
    setInputMessage('');
    const conversationContext = [
      ...messages
        .filter((msg) => msg.role === 'user' || (msg.role === 'assistant' && msg.profile_id === profileId))
        .map((msg) => ({
          id: msg.id,
          run_id: `chat-${profileId}`,
          author_type: msg.role === 'user' ? 'user' : 'assistant',
          profile_id: msg.role === 'assistant' ? profileId : null,
          round_index: null,
          turn_index: null,
          text: msg.content,
          created_at: msg.timestamp,
          provider_metadata_json: null,
        })),
      { id: userMessage.id, run_id: `chat-${profileId}`, author_type: 'user' as const, profile_id: null, round_index: null, turn_index: null, text: userMessage.content, created_at: userMessage.timestamp, provider_metadata_json: null },
    ];
    await startStream(textToSend.trim(), { conversationContext, conversationId: convId });
  }, [profile, profileId, currentConversationId, messages, isStreaming, startStream]);

  const stopContinuousListeningAndSend = useCallback(async () => {
    if (!recordingRef.current || !profile || !profileId) return;
    setContinuousListening(false);
    try {
      const audioBase64 = await recordingRef.current.stop();
      recordingRef.current = null;
      if (!audioBase64) return;
      const { text } = await api.transcribeAudio(audioBase64);
      if (!text.trim()) return;
      await handleSendWithText(text);
    } catch (e) {
      console.warn('Continuous send failed:', e);
      const msg = e instanceof Error ? e.message : String(e);
      if (!msg.includes('not built in')) alert(msg);
    }
  }, [profile, profileId, handleSendWithText]);

  stopContinuousListeningAndSendRef.current = stopContinuousListeningAndSend;

  const startContinuousListening = useCallback(async () => {
    try {
      let session: RecordingSession;
      if (conversationMode === 'continuous' && continuousAutoSend) {
        session = await startRecordingWithSilenceDetection({
          silenceThresholdMs: 1500,
          minRecordingMs: 500,
          onSilence: () => {
            stopContinuousListeningAndSendRef.current();
          },
        });
      } else {
        session = await startRecording();
      }
      recordingRef.current = session;
      setContinuousListening(true);
    } catch (e) {
      console.warn('Failed to start recording:', e);
      alert('Could not access microphone.');
    }
  }, [conversationMode, continuousAutoSend]);

  const handleContinuousPlaybackEnd = useCallback(() => {
    setAutoPlayMessage(null);
    if (isContinuousActive && !isStreaming) {
      startContinuousListening();
    }
  }, [isContinuousActive, isStreaming, startContinuousListening]);

  const handleDeleteConversation = async (convId: string) => {
    if (!window.confirm('Delete this conversation?')) return;
    try {
      await api.deleteProfileConversation(convId);
      const remaining = conversations.filter(c => c.id !== convId);
      setConversations(remaining);
      if (currentConversationId === convId) {
        setCurrentConversationId(remaining[0]?.id ?? null);
      }
    } catch (err) {
      console.error('Failed to delete conversation:', err);
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleAttachDocument = () => {
    fileInputRef.current?.click();
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files?.length) return;
    const maxChars = 50_000;
    const readFile = (file: File): Promise<{ name: string; content: string }> => {
      return new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
          let content = (reader.result as string) || '';
          if (content.length > maxChars) {
            content = content.slice(0, maxChars) + `\n\n[Truncated - document was ${content.length} chars]`;
          }
          resolve({ name: file.name, content });
        };
        reader.onerror = () => reject(reader.error);
        reader.readAsText(file, 'UTF-8');
      });
    };
    Promise.all(Array.from(files).map(readFile))
      .then((docs) => {
        setAttachedDocuments((prev) => [...prev, ...docs]);
      })
      .catch((err) => {
        console.error('Failed to read file:', err);
        alert('Failed to read file. Try a text-based file (txt, md, json, etc.).');
      });
    e.target.value = '';
  };

  const removeAttachedDocument = (index: number) => {
    setAttachedDocuments((prev) => prev.filter((_, i) => i !== index));
  };

  if (!profile) {
    return (
      <div>
        <div className="page-header">
          <h1>Profile Chat</h1>
          <p>Loading profile...</p>
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 100px)' }}>
      <div className="page-header" style={{ marginBottom: '10px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '15px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '15px' }}>
            <button
              onClick={() => navigate('/profiles')}
              style={{
                padding: '8px 15px',
                background: 'transparent',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '14px',
                color: 'var(--text-primary)',
              }}
            >
              ← Back to Profiles
            </button>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              {profile.photo_url && (
                <img
                  src={profile.photo_url}
                  alt={profile.name}
                  style={{
                    width: '40px',
                    height: '40px',
                    borderRadius: '50%',
                    objectFit: 'cover',
                    border: '2px solid var(--border-color)'
                  }}
                />
              )}
              <div>
                <h1 style={{ margin: 0, fontSize: '24px', color: 'var(--text-primary)' }}>{profile.name}</h1>
                <p style={{ margin: 0, color: 'var(--text-secondary)', fontSize: '14px' }}>Chat with {profile.name}</p>
              </div>
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', flexWrap: 'wrap' }}>
            {isHybrid && (
              <>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>Use model:</label>
                <select
                  value={modelPreference}
                  onChange={(e) => setModelPreference(e.target.value as ModelPreference)}
                  style={{
                    padding: '6px 10px',
                    borderRadius: '4px',
                    border: '1px solid var(--border-color)',
                    fontSize: '13px',
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                    cursor: 'pointer'
                  }}
                  title="Choose which model to use for this chat"
                >
                  <option value="default">Default (local first)</option>
                  <option value="local">Local only</option>
                  <option value="cloud">Cloud only</option>
                </select>
              </>
            )}
            <label style={{ fontSize: '12px', color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>Language:</label>
            <select
              value={chatLanguage}
              onChange={(e) => setChatLanguage(e.target.value)}
              style={{
                padding: '6px 10px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                fontSize: '13px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                cursor: 'pointer'
              }}
            >
              <option value="English">English</option>
              <option value="Farsi">Farsi</option>
              <option value="Spanish">Spanish</option>
              <option value="French">French</option>
              <option value="German">German</option>
              <option value="Arabic">Arabic</option>
              <option value="Chinese">Chinese</option>
              <option value="Japanese">Japanese</option>
            </select>
            <button
              onClick={() => setShowExportModal(true)}
              disabled={selectedMessages.size === 0}
              style={{
                padding: '6px 12px',
                fontSize: '12px',
                background: selectedMessages.size > 0 ? '#4a90e2' : '#6c757d',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: selectedMessages.size > 0 ? 'pointer' : 'not-allowed',
                opacity: selectedMessages.size > 0 ? 1 : 0.6
              }}
            >
              Export ({selectedMessages.size})
            </button>
            <button
              onClick={async () => {
                if (window.confirm(currentConversationId ? 'Clear this conversation?' : 'Clear all chat messages? This cannot be undone.')) {
                  try {
                    if (currentConversationId) {
                      await api.clearConversationMessages(currentConversationId);
                    } else {
                      await api.clearChatMessages(profileId!);
                    }
                    setMessages([]);
                    setSelectedMessages(new Set());
                  } catch (error) {
                    console.error('Failed to clear chat:', error);
                    alert('Failed to clear chat messages');
                  }
                }
              }}
              style={{
                padding: '6px 12px',
                fontSize: '12px',
                background: '#dc3545',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer'
              }}
            >
              Clear Chat
            </button>
          </div>
        </div>
      </div>

      <div style={{ 
        flex: 1, 
        display: 'flex', 
        flexDirection: 'row',
        gap: '12px',
        minHeight: 0
      }}>
        {/* Conversation sidebar */}
        <div style={{
          width: '220px',
          flexShrink: 0,
          background: 'var(--surface-elevated)',
          borderRadius: '8px',
          border: '1px solid var(--border-color)',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden'
        }}>
          <div style={{ padding: '12px', borderBottom: '1px solid var(--border-color)' }}>
            <button
              onClick={handleNewConversation}
              style={{
                width: '100%',
                padding: '8px 12px',
                fontSize: '13px',
                background: 'var(--primary)',
                color: 'white',
                border: 'none',
                borderRadius: '6px',
                cursor: 'pointer',
                fontWeight: 500
              }}
            >
              + New conversation
            </button>
          </div>
          <div style={{ flex: 1, overflowY: 'auto', padding: '8px' }}>
            {conversations.length === 0 ? (
              <p style={{ fontSize: '12px', color: 'var(--text-secondary)', padding: '8px' }}>
                No conversations yet. Start chatting to create one.
              </p>
            ) : (
              conversations.map(conv => (
                <div
                  key={conv.id}
                  onClick={() => handleSelectConversation(conv.id)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (window.confirm('Delete this conversation?')) handleDeleteConversation(conv.id);
                  }}
                  style={{
                    padding: '10px 12px',
                    marginBottom: '4px',
                    borderRadius: '6px',
                    cursor: 'pointer',
                    background: currentConversationId === conv.id ? 'var(--highlight-bg)' : 'transparent',
                    border: currentConversationId === conv.id ? '1px solid var(--primary)' : '1px solid transparent'
                  }}
                >
                  <div style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-primary)' }}>
                    {conv.title}
                  </div>
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '2px' }}>
                    {new Date(conv.updated_at).toLocaleDateString()}
                  </div>
                </div>
              ))
            )}
          </div>
        </div>

        {/* Messages area */}
        <div style={{ 
          flex: 1, 
          display: 'flex',
          flexDirection: 'column',
          background: 'var(--bg-primary)',
          borderRadius: '8px',
          border: '1px solid var(--border-color)',
          overflow: 'hidden'
        }}>
        <div style={{ 
          flex: 1, 
          overflowY: 'auto', 
          padding: '20px',
          display: 'flex',
          flexDirection: 'column',
          gap: '15px'
        }}>
          {messages.length === 0 && (
            <div style={{ 
              textAlign: 'center', 
              color: 'var(--text-secondary)',
              padding: '40px 20px'
            }}>
              <p style={{ fontSize: '16px', marginBottom: '10px' }}>Start a conversation with {profile.name}</p>
              <p style={{ fontSize: '13px' }}>Ask questions, discuss ideas, or get their perspective on topics.</p>
            </div>
          )}
          
          {messages.map((msg) => (
            <div
              key={msg.id}
              style={{
                display: 'flex',
                justifyContent: msg.role === 'user' ? 'flex-end' : 'flex-start',
                marginBottom: '10px',
                gap: '8px'
              }}
            >
              <input
                type="checkbox"
                checked={selectedMessages.has(msg.id)}
                onChange={(e) => {
                  const newSelected = new Set(selectedMessages);
                  if (e.target.checked) {
                    newSelected.add(msg.id);
                  } else {
                    newSelected.delete(msg.id);
                  }
                  setSelectedMessages(newSelected);
                }}
                style={{
                  marginTop: '8px',
                  cursor: 'pointer'
                }}
              />
              <div style={{
                maxWidth: '70%',
                padding: '12px 16px',
                borderRadius: '12px',
                background: selectedMessages.has(msg.id) 
                  ? (msg.role === 'user' ? 'var(--highlight-bg)' : 'var(--surface-hover)')
                  : (msg.role === 'user' ? 'var(--highlight-bg)' : 'var(--surface-elevated)'),
                border: selectedMessages.has(msg.id)
                  ? '2px solid var(--primary)'
                  : `1px solid ${msg.role === 'user' ? 'var(--highlight-bg)' : 'var(--border-color)'}`,
                wordWrap: 'break-word',
                transition: 'all 0.2s'
              }}>
                {msg.role === 'assistant' && (
                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '6px', flexWrap: 'wrap' }}>
                    {profile.photo_url && (
                      <>
                        <img
                          src={profile.photo_url}
                          alt={profile.name}
                          style={{
                            width: '20px',
                            height: '20px',
                            borderRadius: '50%',
                            objectFit: 'cover'
                          }}
                        />
                        <strong style={{ fontSize: '13px', color: 'var(--primary)' }}>{profile.name}</strong>
                      </>
                    )}
                    {msg.id.startsWith('improved-') && (
                      <span style={{ fontSize: '10px', color: 'var(--text-secondary)', fontStyle: 'italic' }}>
                        ☁️ Cloud improved
                      </span>
                    )}
                  </div>
                )}
                <div style={{ whiteSpace: 'pre-wrap', fontSize: '14px', lineHeight: '1.5' }}>
                  {msg.content}
                </div>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '8px', marginTop: '6px', flexWrap: 'wrap' }}>
                  <span style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                      {new Date(msg.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                    {msg.role === 'assistant' && msg.content && (
                      <VoiceOutput
                        text={msg.content}
                        lang={chatLanguage === 'English' ? 'en-US' : chatLanguage === 'Arabic' ? 'ar' : 'en-US'}
                        voiceUri={profile?.voice_uri}
                      />
                    )}
                  </span>
                  {msg.role === 'assistant' && isHybrid && msg.content && (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                      <input
                        type="text"
                        placeholder="What to change? (e.g. make concise, fix syntax)"
                        aria-label="Improvement instructions for cloud"
                        value={improvementPrompts[msg.id] ?? ''}
                        onChange={(e) => setImprovementPrompts(prev => ({ ...prev, [msg.id]: e.target.value }))}
                        style={{
                          flex: 1,
                          minWidth: '120px',
                          maxWidth: '200px',
                          fontSize: '11px',
                          padding: '4px 8px',
                          borderRadius: '4px',
                          border: '1px solid var(--border-color)',
                          background: 'var(--surface)',
                          color: 'var(--text-primary)',
                        }}
                        onKeyDown={(e) => e.key === 'Enter' && (e.currentTarget.nextElementSibling as HTMLButtonElement)?.click()}
                      />
                      <button
                        type="button"
                        onClick={async () => {
                          if (!profileId || improvingMessageId) return;
                          setImprovingMessageId(msg.id);
                          try {
                            const improved = await api.improveResponseWithCloud({
                              profile_id: profileId,
                              assistant_message: msg.content,
                              user_improvement_prompt: improvementPrompts[msg.id]?.trim() || undefined,
                            });
                          const newMsg: ChatMessage = {
                            id: `improved-${msg.id}-${Date.now()}`,
                            role: 'assistant',
                            content: improved,
                            timestamp: new Date().toISOString(),
                            profile_id: profileId!,
                          };
                          setMessages(prev => {
                            const idx = prev.findIndex(m => m.id === msg.id);
                            if (idx === -1) return prev;
                            return [...prev.slice(0, idx + 1), newMsg, ...prev.slice(idx + 1)];
                          });
                          try {
                            await api.insertChatMessage(profileId!, 'assistant', improved);
                          } catch (_) {
                            // Persistence optional; UI shows the new message
                          }
                          } catch (err: any) {
                            console.error('Improve with cloud failed:', err);
                            alert(err?.message || 'Failed to improve with cloud');
                          } finally {
                            setImprovingMessageId(null);
                          }
                        }}
                        disabled={!!improvingMessageId}
                        style={{
                          fontSize: '11px',
                          padding: '3px 8px',
                          background: 'var(--primary)',
                          color: '#fff',
                          border: 'none',
                          borderRadius: '4px',
                          cursor: improvingMessageId ? 'wait' : 'pointer',
                          opacity: improvingMessageId ? 0.7 : 1,
                        }}
                        title="Local model picks a section to improve; only that section is sent to cloud"
                      >
                        {improvingMessageId === msg.id ? 'Improving…' : '☁️ Improve with cloud'}
                      </button>
                    </div>
                  )}
                </div>
              </div>
            </div>
          ))}
          
          {isStreaming && (
            <div style={{ display: 'flex', justifyContent: 'flex-start' }}>
              <div style={{
                padding: '12px 16px',
                borderRadius: '12px',
                background: 'var(--surface-elevated)',
                border: '1px solid var(--border-color)',
                color: 'var(--text-secondary)',
                fontSize: '14px'
              }}>
                {profile.name} is thinking...
              </div>
            </div>
          )}
          
          <div ref={messagesEndRef} />
        </div>
        
        {/* Hidden VoiceOutput for auto-play (continuous mode or autoSpeakResponses) */}
        {autoPlayMessage && (isContinuousActive || autoSpeakResponses) && (
          <div style={{ position: 'absolute', left: -9999, opacity: 0, pointerEvents: 'none' }}>
            <VoiceOutput
              text={autoPlayMessage.content}
              autoPlay
              onPlaybackEnd={isContinuousActive ? handleContinuousPlaybackEnd : () => setAutoPlayMessage(null)}
              lang={chatLanguage === 'English' ? 'en-US' : chatLanguage === 'Arabic' ? 'ar' : 'en-US'}
              voiceUri={profile?.voice_uri}
            />
          </div>
        )}

        <ExportChatModal
          isOpen={showExportModal}
          onClose={() => {
            setShowExportModal(false);
            setSelectedMessages(new Set());
          }}
          selectedMessageIds={Array.from(selectedMessages)}
          chatType="profile"
        />

        {/* Input area */}
        <div style={{ 
          padding: '15px', 
          borderTop: '1px solid var(--border-color)',
          background: 'var(--bg-secondary)'
        }}>
          <div style={{ display: 'flex', gap: '8px', marginBottom: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Voice mode:</span>
            <button
              type="button"
              onClick={async () => {
                setConversationMode('push-to-talk');
                setIsContinuousActive(false);
                setContinuousListening(false);
                if (recordingRef.current) {
                  try { await recordingRef.current.stop(); } catch { /* ignore */ }
                  recordingRef.current = null;
                }
              }}
              style={{
                padding: '4px 10px',
                fontSize: '12px',
                background: conversationMode === 'push-to-talk' ? 'var(--primary)' : 'var(--bg-primary)',
                color: conversationMode === 'push-to-talk' ? '#fff' : 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Push to talk
            </button>
            <button
              type="button"
              onClick={() => setConversationMode('continuous')}
              style={{
                padding: '4px 10px',
                fontSize: '12px',
                background: conversationMode === 'continuous' ? 'var(--primary)' : 'var(--bg-primary)',
                color: conversationMode === 'continuous' ? '#fff' : 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                cursor: 'pointer',
              }}
            >
              Conversation mode
            </button>
            {conversationMode === 'continuous' && (
              <>
                <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '12px', cursor: 'pointer' }}>
                  <input
                    type="checkbox"
                    checked={continuousAutoSend}
                    onChange={(e) => setContinuousAutoSend(e.target.checked)}
                    style={{ cursor: 'pointer' }}
                  />
                  <span>Auto-send on silence</span>
                </label>
                {!isContinuousActive ? (
                  <button
                    type="button"
                    onClick={() => { setIsContinuousActive(true); startContinuousListening(); }}
                    disabled={isStreaming}
                    style={{ padding: '4px 12px', fontSize: '12px', background: '#28a745', color: '#fff', border: 'none', borderRadius: '4px', cursor: 'pointer' }}
                  >
                    Start conversation
                  </button>
                ) : (
                  <>
                    <span style={{ fontSize: '12px', color: continuousListening ? 'var(--primary)' : 'var(--text-secondary)' }}>
                      {continuousListening ? 'Listening...' : 'Processing...'}
                    </span>
                    <button
                      type="button"
                      onClick={stopContinuousListeningAndSend}
                      disabled={!continuousListening || isStreaming}
                      style={{ padding: '4px 12px', fontSize: '12px', background: '#dc3545', color: '#fff', border: 'none', borderRadius: '4px', cursor: 'pointer' }}
                    >
                      Stop and send
                    </button>
                    <button
                      type="button"
                      onClick={async () => {
                        setIsContinuousActive(false);
                        setContinuousListening(false);
                        if (recordingRef.current) {
                          try { await recordingRef.current.stop(); } catch { /* ignore */ }
                          recordingRef.current = null;
                        }
                      }}
                      style={{ padding: '4px 12px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', borderRadius: '4px', cursor: 'pointer' }}
                    >
                      End conversation
                    </button>
                  </>
                )}
              </>
            )}
          </div>
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept=".txt,.md,.json,.csv,.xml,.html,.htm,.log,.py,.js,.ts,.tsx,.jsx,.rs,.go,.java,.c,.cpp,.h,.yaml,.yml"
            onChange={handleFileSelect}
            style={{ display: 'none' }}
            aria-hidden
          />
          <div style={{ display: 'flex', gap: '10px', marginBottom: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={searchWeb}
                onChange={(e) => setSearchWeb(e.target.checked)}
                style={{ cursor: 'pointer' }}
              />
              <span>🌐 Search web for recent news</span>
            </label>
            <button
              type="button"
              onClick={handleAttachDocument}
              disabled={isStreaming}
              style={{
                padding: '4px 10px',
                fontSize: '12px',
                background: 'var(--bg-primary)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                cursor: isStreaming ? 'not-allowed' : 'pointer',
                color: 'var(--text-primary)',
              }}
              title="Attach documents (txt, md, code, etc.) to provide context"
            >
              📎 Attach document
            </button>
            <div ref={trainingDataDropdownRef} style={{ position: 'relative', display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
              <button
                type="button"
                onClick={() => {
                  setLoadAsTalkTo(false);
                  setShowLoadTrainingData(!showLoadTrainingData);
                  if (projects.length === 0) {
                    setLoadingProjects(true);
                    api.listProjects().then((data) => setProjects(data)).catch(console.error).finally(() => setLoadingProjects(false));
                  }
                }}
                disabled={isStreaming}
                style={{
                  padding: '8px 12px',
                  background: 'var(--surface)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: isStreaming ? 'not-allowed' : 'pointer',
                  color: 'var(--text-primary)',
                }}
                title="Load training data from a project as context for this chat"
              >
                📚 Load training data
              </button>
              <button
                type="button"
                onClick={() => {
                  setLoadAsTalkTo(true);
                  setShowLoadTrainingData(!showLoadTrainingData);
                  if (projects.length === 0) {
                    setLoadingProjects(true);
                    api.listProjects().then((data) => setProjects(data)).catch(console.error).finally(() => setLoadingProjects(false));
                  }
                }}
                disabled={isStreaming}
                style={{
                  padding: '8px 12px',
                  background: 'var(--surface)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: isStreaming ? 'not-allowed' : 'pointer',
                  color: 'var(--text-primary)',
                }}
                title="Load training data and start a discussion about it"
              >
                💬 Talk to Trained Data
              </button>
              {showLoadTrainingData && (
                <div
                  style={{
                    position: 'absolute',
                    top: '100%',
                    left: 0,
                    marginTop: '4px',
                    padding: '8px',
                    background: 'var(--card-bg)',
                    border: '1px solid var(--border-color)',
                    borderRadius: '6px',
                    boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
                    zIndex: 100,
                    minWidth: '220px',
                  }}
                >
                  <div style={{ fontSize: '12px', fontWeight: 600, marginBottom: '8px', color: 'var(--text-secondary)' }}>
                    {loadingTrainingData ? 'Loading training data...' : 'Select project'}
                  </div>
                  {projects.length === 0 ? (
                    <div style={{ fontSize: '12px', color: 'var(--text-secondary)', padding: '8px 0' }}>
                      {loadingProjects ? 'Loading projects...' : 'No projects. Create one in Projects.'}
                    </div>
                  ) : (
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', maxHeight: '200px', overflowY: 'auto' }}>
                      {projects.map((p) => (
                        <button
                          key={p.id}
                          type="button"
                          onClick={async () => {
                            setLoadingTrainingData(true);
                            try {
                              const data = await api.listTrainingData(p.id);
                              const maxContentLen = 1500;
                              const docs = data.slice(0, 20).map((d: any, i: number) => {
                                const inp = (d.input_text || '').slice(0, maxContentLen);
                                const out = (d.output_text || '').slice(0, maxContentLen);
                                return {
                                  name: `Training ${i + 1}: ${(d.input_text || '').slice(0, 35)}...`,
                                  content: `Input: ${inp}${(d.input_text || '').length > maxContentLen ? '...' : ''}\nOutput: ${out}${(d.output_text || '').length > maxContentLen ? '...' : ''}`,
                                };
                              });
                              setShowLoadTrainingData(false);
                              if (loadAsTalkTo) {
                                if (docs.length > 0) {
                                  await sendMessageWithDocs(
                                    `I've loaded ${docs.length} training example${docs.length !== 1 ? 's' : ''} from "${p.name}". Please help me understand and discuss it.`,
                                    docs
                                  );
                                } else {
                                  alert(`No training data in "${p.name}". Add training data in Project Training first.`);
                                }
                              } else {
                                const docsWithFlag = docs.map((d) => ({ ...d, fromTraining: true }));
                                setAttachedDocuments((prev) => [...prev, ...docsWithFlag]);
                              }
                            } catch (e) {
                              console.error('Failed to load training data:', e);
                              alert('Failed to load training data');
                            } finally {
                              setLoadingTrainingData(false);
                            }
                          }}
                          style={{
                            padding: '8px 10px',
                            textAlign: 'left',
                            background: 'var(--bg-secondary)',
                            border: '1px solid var(--border-color)',
                            borderRadius: '4px',
                            cursor: 'pointer',
                            fontSize: '13px',
                            color: 'var(--text-primary)',
                          }}
                        >
                          {p.name}
                        </button>
                      ))}
                    </div>
                  )}
                  <button
                    type="button"
                    onClick={() => setShowLoadTrainingData(false)}
                    style={{ marginTop: '8px', fontSize: '11px', color: 'var(--text-secondary)' }}
                  >
                    Close
                  </button>
                </div>
              )}
            </div>
          </div>
          {attachedDocuments.length > 0 && (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px', marginBottom: '8px', alignItems: 'center' }}>
              {(() => {
                const trainingDocs = attachedDocuments.filter((d) => d.fromTraining);
                const regularDocs = attachedDocuments.filter((d) => !d.fromTraining);
                return (
                  <>
                    {trainingDocs.length > 0 && (
                      <span
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: '4px',
                          padding: '4px 8px',
                          fontSize: '12px',
                          background: 'var(--surface)',
                          border: '1px solid var(--border-color)',
                          borderRadius: '4px',
                        }}
                      >
                        📚 {trainingDocs.length} training example{trainingDocs.length !== 1 ? 's' : ''} loaded
                        <button
                          type="button"
                          onClick={() => setAttachedDocuments((prev) => prev.filter((d) => !d.fromTraining))}
                          aria-label="Remove training data"
                          style={{
                            padding: '0 4px',
                            background: 'none',
                            border: 'none',
                            cursor: 'pointer',
                            fontSize: '14px',
                            color: 'var(--text-secondary)',
                            lineHeight: 1,
                          }}
                        >
                          ×
                        </button>
                      </span>
                    )}
                    {regularDocs.map((doc, i) => {
                      const actualIndex = attachedDocuments.findIndex((d) => d === doc);
                      return (
                        <span
                          key={`${doc.name}-${actualIndex}`}
                          style={{
                            display: 'inline-flex',
                            alignItems: 'center',
                            gap: '4px',
                            padding: '4px 8px',
                            fontSize: '12px',
                            background: 'var(--surface)',
                            border: '1px solid var(--border-color)',
                            borderRadius: '4px',
                          }}
                        >
                          📄 {doc.name}
                          <button
                            type="button"
                            onClick={() => removeAttachedDocument(actualIndex)}
                            aria-label={`Remove ${doc.name}`}
                            style={{
                              padding: '0 4px',
                              background: 'none',
                              border: 'none',
                              cursor: 'pointer',
                              fontSize: '14px',
                              color: 'var(--text-secondary)',
                              lineHeight: 1,
                            }}
                          >
                            ×
                          </button>
                        </span>
                      );
                    })}
                  </>
                );
              })()}
            </div>
          )}
          <div style={{ display: 'flex', gap: '10px', alignItems: 'flex-end' }}>
            <textarea
              value={inputMessage}
              onChange={(e) => setInputMessage(e.target.value)}
              onKeyPress={handleKeyPress}
              placeholder={`Message ${profile.name}...`}
              rows={2}
              style={{
                flex: 1,
                padding: '10px',
                borderRadius: '8px',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
                resize: 'none',
                fontFamily: 'inherit'
              }}
              disabled={isStreaming}
            />
            {!(conversationMode === 'continuous' && isContinuousActive) && (
              <VoiceInput
                value={inputMessage}
                onTranscript={setInputMessage}
                disabled={isStreaming}
                lang={chatLanguage === 'English' ? 'en-US' : chatLanguage === 'Arabic' ? 'ar' : 'en-US'}
              />
            )}
            <button
              onClick={handleSend}
              disabled={isStreaming || !inputMessage.trim()}
              className="btn btn-primary"
              style={{
                padding: '10px 20px',
                fontSize: '14px',
                alignSelf: 'flex-end'
              }}
            >
              {isStreaming ? 'Sending...' : 'Send'}
            </button>
          </div>
        </div>
        </div>
      </div>
    </div>
  );
}
