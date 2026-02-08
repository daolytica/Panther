import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../api';
import { useAppStore } from '../store';
import { ExportChatModal } from '../components/ExportChatModal';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface CoderMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: string;
  model?: string;
  provider?: string;
}

interface CoderChat {
  id: string;
  title: string;
  messages: CoderMessage[];
  created_at: string;
  updated_at: string;
}

export function Coder() {
  const navigate = useNavigate();
  const { providers } = useAppStore();
  const [chats, setChats] = useState<CoderChat[]>([]);
  const [activeChat, setActiveChat] = useState<string | null>(null);
  const [messages, setMessages] = useState<CoderMessage[]>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [loading, setLoading] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState<string>('');
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [selectedMessages, setSelectedMessages] = useState<Set<string>>(new Set());
  const [showExportModal, setShowExportModal] = useState(false);
  const activeRequestIdRef = useRef<string | null>(null);
  const streamUnlistenRef = useRef<UnlistenFn | null>(null);
  const [renamingChat, setRenamingChat] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState<string>('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadChats();
    loadProviders();
  }, []);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    if (selectedProvider) {
      loadModels();
    }
  }, [selectedProvider]);

  const loadProviders = async () => {
    try {
      const data = await api.listProviders();
      useAppStore.getState().setProviders(data);
      if (data.length > 0 && !selectedProvider) {
        setSelectedProvider(data[0].id);
      }
    } catch (error) {
      console.error('Failed to load providers:', error);
    }
  };

  const loadModels = async () => {
    if (!selectedProvider) return;
    try {
      const models = await api.listProviderModels(selectedProvider);
      setAvailableModels(models);
      if (models.length > 0 && !selectedModel) {
        setSelectedModel(models[0]);
      }
    } catch (error) {
      console.error('Failed to load models:', error);
      setAvailableModels([]);
    }
  };

  const loadChats = async () => {
    try {
      const savedChats = await api.loadCoderChats();
      setChats(savedChats);
      if (savedChats.length > 0 && !activeChat) {
        setActiveChat(savedChats[0].id);
        setMessages(savedChats[0].messages || []);
      }
    } catch (error) {
      console.error('Failed to load chats:', error);
      // Create default chat if none exist
      const newChat: CoderChat = {
        id: crypto.randomUUID(),
        title: 'New Chat',
        messages: [],
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString()
      };
      setChats([newChat]);
      setActiveChat(newChat.id);
    }
  };

  const createNewChat = async () => {
    const newChat: CoderChat = {
      id: crypto.randomUUID(),
      title: 'New Chat',
      messages: [],
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString()
    };
    setChats(prev => [newChat, ...prev]);
    setActiveChat(newChat.id);
    setMessages([]);
    try {
      await api.saveCoderChat(newChat);
    } catch (error) {
      console.error('Failed to save new chat:', error);
    }
  };

  const deleteChat = async (chatId: string) => {
    if (!window.confirm('Delete this chat?')) return;
    try {
      await api.deleteCoderChat(chatId);
      setChats(prev => prev.filter(c => c.id !== chatId));
      if (activeChat === chatId) {
        const remaining = chats.filter(c => c.id !== chatId);
        if (remaining.length > 0) {
          setActiveChat(remaining[0].id);
          setMessages(remaining[0].messages || []);
        } else {
          createNewChat();
        }
      }
    } catch (error) {
      console.error('Failed to delete chat:', error);
    }
  };

  const startRename = (chat: CoderChat) => {
    setRenamingChat(chat.id);
    setRenameValue(chat.title);
  };

  const saveRename = async (chatId: string) => {
    if (!renameValue.trim()) {
      alert('Chat title cannot be empty');
      return;
    }
    try {
      const chat = chats.find(c => c.id === chatId);
      if (chat) {
        const updatedChat = { ...chat, title: renameValue.trim(), updated_at: new Date().toISOString() };
        await api.saveCoderChat(updatedChat);
        setChats(prev => prev.map(c => c.id === chatId ? updatedChat : c));
        setRenamingChat(null);
        setRenameValue('');
      }
    } catch (error) {
      console.error('Failed to rename chat:', error);
      alert('Failed to rename chat');
    }
  };

  const cancelRename = () => {
    setRenamingChat(null);
    setRenameValue('');
  };

  const selectChat = (chat: CoderChat) => {
    setActiveChat(chat.id);
    setMessages(chat.messages || []);
  };

  const handleSend = async () => {
    if (!inputMessage.trim() || loading) return;
    if (!selectedProvider || !selectedModel) {
      alert('Please select a provider and model first');
      return;
    }

    const userMessage: CoderMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: inputMessage.trim(),
      timestamp: new Date().toISOString()
    };

    const updatedMessages = [...messages, userMessage];
    setMessages(updatedMessages);
    const currentInput = inputMessage.trim();
    setInputMessage('');
    setLoading(true);

    const requestId = crypto.randomUUID();
    activeRequestIdRef.current = requestId;

    try {
      // Build honest, capability-aligned system prompt
      const systemPrompt = `You are Panther Coder, an AI coding assistant.
You MUST be honest about your capabilities and NEVER claim to have performed actions that you did not actually perform.
In this chat view you CANNOT directly create folders, modify the user's filesystem, or run real terminal commands.
Instead, you should:
- Propose concrete commands or file changes the user can run or apply
- Clearly mark code blocks and shell commands
- Avoid statements like "I have created X" or "The command has been run" – instead say "Run this:" or "You can create X by..."
Always prioritize clarity, safety, and truthfulness.`;

      // Convert messages to conversation context
      const conversationContext = updatedMessages.map(msg => ({
        id: msg.id,
        run_id: `coder-${activeChat}`,
        author_type: msg.role === 'user' ? 'user' : 'assistant',
        profile_id: null,
        round_index: null,
        turn_index: null,
        text: msg.content,
        created_at: msg.timestamp,
        provider_metadata_json: null
      }));

      // Placeholder assistant message to stream into
      const assistantId = crypto.randomUUID();
      const assistantPlaceholder: CoderMessage = {
        id: assistantId,
        role: 'assistant',
        content: '',
        timestamp: new Date().toISOString(),
        model: selectedModel,
        provider: providers.find(p => p.id === selectedProvider)?.display_name
      };
      setMessages(prev => [...prev, assistantPlaceholder]);

      // Set up streaming listener
      if (streamUnlistenRef.current) {
        await streamUnlistenRef.current();
      }
      const unlisten = await listen<{
        stream_id: string;
        chunk: string;
        done: boolean;
        error?: string | null;
      }>('panther://coder_stream', (event) => {
        console.log('📡 Received streaming event:', event.payload);
        const { stream_id, chunk, done, error } = event.payload;
        if (stream_id !== requestId) {
          console.log('📡 Ignoring event for different stream:', stream_id, 'vs', requestId);
          return;
        }
        if (activeRequestIdRef.current !== requestId) {
          console.log('📡 Ignoring event for cancelled request');
          return;
        }

        if (error) {
          console.log('❌ Streaming error:', error);
          setMessages(prev => [...prev, {
            id: crypto.randomUUID(),
            role: 'assistant',
            content: `Error: ${error}`,
            timestamp: new Date().toISOString(),
          }]);
        } else if (chunk) {
          console.log('📝 Streaming chunk received, length:', chunk.length);
          setMessages(prev => prev.map(msg => msg.id === assistantId
            ? { ...msg, content: (msg.content || '') + chunk }
            : msg
          ));
        }

        if (done) {
          console.log('✅ Streaming completed');
          if (streamUnlistenRef.current) {
            streamUnlistenRef.current();
            streamUnlistenRef.current = null;
          }
          if (activeRequestIdRef.current === requestId) {
            activeRequestIdRef.current = null;
            setLoading(false);
          }
        }
      });
      streamUnlistenRef.current = unlisten;

      // Add timeout to prevent hanging
      const timeoutPromise = new Promise((_, reject) => {
        setTimeout(() => reject(new Error('Request timed out after 125 seconds')), 125000);
      });

      const response = await Promise.race([
        api.coderChatStream({
          provider_id: selectedProvider,
          model_name: selectedModel,
          user_message: currentInput,
          conversation_context: conversationContext,
          system_prompt: systemPrompt
        }, requestId),
        timeoutPromise
      ]).catch((error) => {
        console.error('❌ Request failed or timed out:', error);
        throw error;
      });

      // If user clicked "Stop" and this request is no longer active, ignore the result
      if (activeRequestIdRef.current !== requestId) {
        return;
      }

      // Ensure final content is set to full response (in case streaming missed anything)
      const responseStr = typeof response === 'string' ? response : String(response);
      setMessages(prev => prev.map(msg => msg.id === assistantId
        ? { ...msg, content: responseStr }
        : msg
      ));

      // Update chat title if it's the first message
      const chat = chats.find(c => c.id === activeChat);
      const messagesWithAssistant: CoderMessage[] = [...updatedMessages, { ...assistantPlaceholder, content: responseStr }];
      if (chat && chat.title === 'New Chat' && messagesWithAssistant.length === 2) {
        const title = currentInput.slice(0, 50) + (currentInput.length > 50 ? '...' : '');
        const updatedChat = { ...chat, title, messages: messagesWithAssistant, updated_at: new Date().toISOString() };
        setChats(prev => prev.map(c => c.id === activeChat ? updatedChat : c));
        try {
          await api.saveCoderChat(updatedChat);
        } catch (error) {
          console.error('Failed to save chat:', error);
        }
      } else if (chat) {
        const updatedChat = { ...chat, messages: messagesWithAssistant, updated_at: new Date().toISOString() };
        setChats(prev => prev.map(c => c.id === activeChat ? updatedChat : c));
        try {
          await api.saveCoderChat(updatedChat);
        } catch (error) {
          console.error('Failed to save chat:', error);
        }
      }
    } catch (error: any) {
      console.error('Coder chat error:', error);
      const errorMessage: CoderMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: `Error: ${error?.message || error || 'Failed to get response'}`,
        timestamp: new Date().toISOString()
      };
      // Only append error if this request is still active
      if (activeRequestIdRef.current === requestId) {
        setMessages(prev => [...prev, errorMessage]);
      }
    } finally {
      if (activeRequestIdRef.current === requestId) {
        setLoading(false);
        activeRequestIdRef.current = null;
      }
    }
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div style={{ display: 'flex', height: 'calc(100vh - 50px)', background: 'var(--bg-primary)' }}>
      {/* Sidebar */}
      <div style={{
        width: '280px',
        background: 'var(--card-bg)',
        borderRight: '1px solid var(--border-color)',
        display: 'flex',
        flexDirection: 'column'
      }}>
        <div style={{ padding: '20px' }}>
          <button
            onClick={() => navigate('/home')}
            style={{
              width: '100%',
              padding: '10px',
              background: 'transparent',
              border: '1px solid var(--border-color)',
              borderRadius: '8px',
              cursor: 'pointer',
              marginBottom: '10px',
              color: 'var(--text-primary)',
              fontSize: '14px'
            }}
          >
            ← Back to Home
          </button>
          <button
            onClick={createNewChat}
            style={{
              width: '100%',
              padding: '12px',
              background: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
              color: 'white',
              border: 'none',
              borderRadius: '8px',
              cursor: 'pointer',
              fontWeight: '600',
              fontSize: '14px'
            }}
          >
            + New Chat
          </button>
        </div>

        <div style={{ flex: 1, overflow: 'auto', padding: '0 10px' }}>
          {chats.map(chat => (
            <div
              key={chat.id}
              onClick={() => renamingChat !== chat.id && selectChat(chat)}
              style={{
                padding: '12px 15px',
                margin: '5px 0',
                borderRadius: '8px',
                cursor: renamingChat === chat.id ? 'default' : 'pointer',
                background: activeChat === chat.id ? 'var(--bg-secondary)' : 'transparent',
                border: activeChat === chat.id ? '1px solid var(--primary-color)' : '1px solid transparent',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                transition: 'all 0.2s',
                gap: '8px'
              }}
            >
              {renamingChat === chat.id ? (
                <>
                  <input
                    type="text"
                    value={renameValue}
                    onChange={(e) => setRenameValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') {
                        saveRename(chat.id);
                      } else if (e.key === 'Escape') {
                        cancelRename();
                      }
                    }}
                    onClick={(e) => e.stopPropagation()}
                    autoFocus
                    style={{
                      flex: 1,
                      padding: '4px 8px',
                      border: '1px solid var(--border-color)',
                      borderRadius: '4px',
                      background: 'var(--bg-primary)',
                      color: 'var(--text-primary)',
                      fontSize: '14px'
                    }}
                  />
                  <button
                    onClick={(e) => { e.stopPropagation(); saveRename(chat.id); }}
                    style={{
                      background: '#4a90e2',
                      border: 'none',
                      color: 'white',
                      cursor: 'pointer',
                      fontSize: '12px',
                      padding: '4px 8px',
                      borderRadius: '4px'
                    }}
                  >
                    ✓
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); cancelRename(); }}
                    style={{
                      background: '#6c757d',
                      border: 'none',
                      color: 'white',
                      cursor: 'pointer',
                      fontSize: '12px',
                      padding: '4px 8px',
                      borderRadius: '4px'
                    }}
                  >
                    ✕
                  </button>
                </>
              ) : (
                <>
                  <span style={{ 
                    fontSize: '14px', 
                    color: 'var(--text-primary)',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    flex: 1
                  }}>
                    {chat.title}
                  </span>
                  <button
                    onClick={(e) => { e.stopPropagation(); startRename(chat); }}
                    style={{
                      background: 'none',
                      border: 'none',
                      color: '#4a90e2',
                      cursor: 'pointer',
                      fontSize: '14px',
                      opacity: 0.6,
                      padding: '2px 4px'
                    }}
                    title="Rename"
                  >
                    ✏️
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); deleteChat(chat.id); }}
                    style={{
                      background: 'none',
                      border: 'none',
                      color: '#dc3545',
                      cursor: 'pointer',
                      fontSize: '14px',
                      opacity: 0.6,
                      padding: '2px 4px'
                    }}
                    title="Delete"
                  >
                    🗑️
                  </button>
                </>
              )}
            </div>
          ))}
        </div>
      </div>

      {/* Main Chat Area */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        {/* Header */}
        <div style={{
          padding: '15px 20px',
          background: 'var(--card-bg)',
          borderBottom: '1px solid var(--border-color)',
          display: 'flex',
          alignItems: 'center',
          gap: '15px'
        }}>
          <h2 style={{ margin: 0, fontSize: '20px', color: 'var(--text-primary)' }}>
            🤖 Panther Coder
          </h2>
          <span style={{
            background: '#dc3545',
            color: 'white',
            padding: '3px 10px',
            borderRadius: '12px',
            fontSize: '11px',
            fontWeight: 'bold'
          }}>
            UNRESTRICTED MODE
          </span>
          <div style={{ flex: 1 }} />
        {loading && (
          <button
            onClick={() => {
              activeRequestIdRef.current = null;
              setLoading(false);
              if (streamUnlistenRef.current) {
                streamUnlistenRef.current();
                streamUnlistenRef.current = null;
              }
            }}
            style={{
              padding: '6px 10px',
              fontSize: '12px',
              background: '#6c757d',
              color: 'white',
              border: 'none',
              borderRadius: '6px',
              cursor: 'pointer',
              marginRight: '8px',
            }}
          >
            Stop
          </button>
        )}
          {loading && (
            <button
              onClick={() => {
                activeRequestIdRef.current = null;
                setLoading(false);
              }}
              style={{
                padding: '6px 10px',
                fontSize: '12px',
                background: '#6c757d',
                color: 'white',
                border: 'none',
                borderRadius: '6px',
                cursor: 'pointer',
                marginRight: '8px',
              }}
            >
              Stop
            </button>
          )}
          <button
            onClick={() => setShowExportModal(true)}
            disabled={selectedMessages.size === 0 && !activeChat}
            style={{
              padding: '8px 16px',
              fontSize: '13px',
              background: (selectedMessages.size > 0 || activeChat) ? '#4a90e2' : '#6c757d',
              color: 'white',
              border: 'none',
              borderRadius: '6px',
              cursor: (selectedMessages.size > 0 || activeChat) ? 'pointer' : 'not-allowed',
              opacity: (selectedMessages.size > 0 || activeChat) ? 1 : 0.6,
              fontWeight: '500'
            }}
          >
            Export {selectedMessages.size > 0 ? `(${selectedMessages.size})` : activeChat ? 'Chat' : ''}
          </button>
          <select
            value={selectedProvider}
            onChange={(e) => {
              setSelectedProvider(e.target.value);
              setSelectedModel('');
            }}
            style={{
              padding: '8px 12px',
              borderRadius: '6px',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '13px'
            }}
          >
            <option value="">Select Provider</option>
            {providers.map(p => (
              <option key={p.id} value={p.id}>{p.display_name}</option>
            ))}
          </select>
          <select
            value={selectedModel}
            onChange={(e) => setSelectedModel(e.target.value)}
            style={{
              padding: '8px 12px',
              borderRadius: '6px',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              fontSize: '13px',
              minWidth: '150px'
            }}
          >
            <option value="">Select Model</option>
            {availableModels.map(m => (
              <option key={m} value={m}>{m}</option>
            ))}
          </select>
        </div>

        {/* Messages */}
        <div style={{ 
          flex: 1, 
          overflow: 'auto', 
          padding: '20px',
          display: 'flex',
          flexDirection: 'column',
          gap: '15px'
        }}>
          {messages.length === 0 && (
            <div style={{ 
              textAlign: 'center', 
              color: 'var(--text-secondary)',
              marginTop: '100px'
            }}>
              <div style={{ fontSize: '48px', marginBottom: '20px' }}>🐆</div>
              <h3 style={{ marginBottom: '10px' }}>Panther Coder</h3>
              <p>Your unrestricted AI coding assistant. Ask anything.</p>
            </div>
          )}
          {messages.map(msg => (
            <div
              key={msg.id}
              style={{
                maxWidth: '85%',
                alignSelf: msg.role === 'user' ? 'flex-end' : 'flex-start',
                display: 'flex',
                gap: '8px',
                alignItems: 'flex-start'
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
              <div
                style={{
                  flex: 1,
                  background: selectedMessages.has(msg.id)
                    ? (msg.role === 'user' 
                      ? 'linear-gradient(135deg, #5568d3 0%, #653a8f 100%)'
                      : '#e0e0e0')
                    : (msg.role === 'user' 
                      ? 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)'
                      : 'var(--card-bg)'),
                  color: msg.role === 'user' ? 'white' : 'var(--text-primary)',
                  padding: '15px 20px',
                  borderRadius: msg.role === 'user' ? '20px 20px 5px 20px' : '20px 20px 20px 5px',
                  boxShadow: '0 2px 10px rgba(0,0,0,0.1)',
                  border: selectedMessages.has(msg.id)
                    ? '2px solid #4a90e2'
                    : (msg.role === 'assistant' ? '1px solid var(--border-color)' : 'none'),
                  transition: 'all 0.2s'
                }}
              >
              <pre style={{ 
                margin: 0, 
                whiteSpace: 'pre-wrap', 
                wordBreak: 'break-word',
                fontFamily: msg.content.includes('```') ? 'monospace' : 'inherit',
                fontSize: '14px',
                lineHeight: '1.6'
              }}>
                {msg.content}
              </pre>
              {msg.role === 'assistant' && msg.model && (
                <div style={{ 
                  fontSize: '11px', 
                  color: 'var(--text-secondary)', 
                  marginTop: '8px',
                  opacity: 0.7
                }}>
                  {msg.provider} • {msg.model}
                </div>
              )}
              </div>
            </div>
          ))}
          {loading && (
            <div style={{
              alignSelf: 'flex-start',
              background: 'var(--card-bg)',
              padding: '15px 20px',
              borderRadius: '20px 20px 20px 5px',
              border: '1px solid var(--border-color)'
            }}>
              <span style={{ animation: 'pulse 1s infinite' }}>Thinking...</span>
            </div>
          )}
          <div ref={messagesEndRef} />
        </div>
        
        <ExportChatModal
          isOpen={showExportModal}
          onClose={() => {
            setShowExportModal(false);
            setSelectedMessages(new Set());
          }}
          selectedMessageIds={Array.from(selectedMessages)}
          chatType="coder"
          chatIds={activeChat ? [activeChat] : []}
        />

        {/* Input */}
        <div style={{
          padding: '20px',
          background: 'var(--card-bg)',
          borderTop: '1px solid var(--border-color)'
        }}>
          <div style={{ display: 'flex', gap: '10px', alignItems: 'flex-end' }}>
            <textarea
              value={inputMessage}
              onChange={(e) => setInputMessage(e.target.value)}
              onKeyDown={handleKeyPress}
              placeholder="Ask anything... No restrictions."
              style={{
                flex: 1,
                padding: '15px',
                borderRadius: '12px',
                border: '1px solid var(--border-color)',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                fontSize: '15px',
                resize: 'none',
                minHeight: '50px',
                maxHeight: '150px',
                fontFamily: 'inherit'
              }}
              rows={1}
            />
            <button
              onClick={handleSend}
              disabled={loading || !inputMessage.trim()}
              style={{
                padding: '15px 30px',
                background: loading ? '#6c757d' : 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
                color: 'white',
                border: 'none',
                borderRadius: '12px',
                cursor: loading ? 'not-allowed' : 'pointer',
                fontWeight: '600',
                fontSize: '15px'
              }}
            >
              {loading ? '...' : 'Send'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
