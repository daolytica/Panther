import { useState, useEffect, useRef, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import Editor, { DiffEditor } from '@monaco-editor/react';
import { api } from '../api';
import { useAppStore } from '../store';
import { VoiceInput } from '../components/VoiceInput';
import { VoiceOutput } from '../components/VoiceOutput';
import { useStreamingLLM } from '../hooks/useStreamingLLM';

// Types
interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children?: FileEntry[];
  expanded?: boolean;
}

interface OpenFile {
  path: string;
  name: string;
  content: string;
  language: string;
  modified: boolean;
}

interface TerminalLine {
  type: 'input' | 'output' | 'error';
  content: string;
  timestamp: string;
}

interface CoderMessage {
  id: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  timestamp: string;
  model?: string;
  provider?: string;
  context_files?: string[];
  terminal_output?: string;
}

interface PantherProjectConfig {
  provider_id?: string;
  model_name?: string;
  project_type?: string;
  stack?: string;
  verification?: {
    test_command?: string;
  };
  workflow_prompts?: Record<string, {
    system_prompt?: string;
    model_name?: string;
    provider_id?: string;
  }>;
  default_workflows?: Array<{
    id: string;
    description?: string;
  }>;
}

interface CoderIDEConversation {
  id: string;
  title: string;
  messages: CoderMessage[];
  created_at: string;
  updated_at: string;
}

// Cline tool execution (for Approve mode)
interface ClineToolExecution {
  id: string;
  step_index: number;
  tool_type: string;
  tool_params: any;
  approval_status: 'pending' | 'approved' | 'rejected';
  result?: any;
}

export function SimpleCoder() {
  const navigate = useNavigate();
  // State
  const { providers } = useAppStore();
  const [backendStatus, setBackendStatus] = useState<'unknown' | 'ok' | 'error'>('unknown');
  const [backendError, setBackendError] = useState<string>('');
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [_expandedDirs, _setExpandedDirs] = useState<Set<string>>(new Set());
  const [currentPath, setCurrentPath] = useState<string>('');
  const [openFiles, setOpenFiles] = useState<OpenFile[]>([]);
  const [activeFile, setActiveFile] = useState<string | null>(null);
  const [terminalLines, setTerminalLines] = useState<TerminalLine[]>([]);
  const [terminalInput, setTerminalInput] = useState('');
  const [terminalRunning, setTerminalRunning] = useState(false);
  const [chatMessages, setChatMessages] = useState<CoderMessage[]>([]);
  const [chatInput, setChatInput] = useState('');
  const [chatLoading, setChatLoading] = useState(false);
  const [projectConfig, setProjectConfig] = useState<PantherProjectConfig | null>(null);
  const [streamingAssistantId, setStreamingAssistantId] = useState<string | null>(null);
  const prevStreamingRef = useRef(false);

  // Agent mode state - automatically enabled with full permissions
  const [agentTask, setAgentTask] = useState('');
  const [agentAllowWrites] = useState(true); // Always enabled
  const [agentAllowCommands] = useState(true); // Always enabled
  const [agentScope, setAgentScope] = useState<'file' | 'folder' | 'workspace'>('folder');
  const [agentStyle, setAgentStyle] = useState<'propose' | 'approve'>('propose'); // Propose = Panther, Approve = Cline
  const [agentRunning, setAgentRunning] = useState(false);
  // Cline (Approve mode) state
  const [pendingTools, setPendingTools] = useState<ClineToolExecution[]>([]);
  const [clineRunId, setClineRunId] = useState<string | null>(null);
  const [clineConversationMessages, setClineConversationMessages] = useState<Array<{ role: 'user' | 'assistant'; content: string }>>([]);
  const [agentResponseInput, setAgentResponseInput] = useState('');
  const [toolBusyIds, setToolBusyIds] = useState<string[]>([]);
  const [agentSummary, setAgentSummary] = useState<string | null>(null);
  const [agentError, setAgentError] = useState<string | null>(null);
  const [agentChanges, setAgentChanges] = useState<{
    run_id: string;
    items: Array<{
      file_path: string;
      description?: string;
      new_content: string;
    }>;
  } | null>(null);
  const [selectedProvider, setSelectedProvider] = useState<string>('');
  const [selectedModel, setSelectedModel] = useState<string>('');
  const [availableModels, setAvailableModels] = useState<string[]>([]);

  const _grantAgentAdminPrivileges = () => {
    // These are always enabled now
    setAgentScope('workspace');
  };
  const [chatMode, setChatMode] = useState<'manual' | 'auto' | 'agent'>('manual');
  const [useStreaming, setUseStreaming] = useState(true); // Allow disabling streaming if it causes issues
  const coderStreamingConfig = useMemo(
    () =>
      selectedProvider && selectedModel && useStreaming && chatMode !== 'auto'
        ? {
            type: 'coder' as const,
            providerId: selectedProvider,
            modelName: selectedModel,
            systemPrompt: undefined as string | undefined,
          }
        : null,
    [selectedProvider, selectedModel, useStreaming, chatMode]
  );
  const { streamedText, isStreaming, error, startStream } = useStreamingLLM(coderStreamingConfig);
  const [trainingProjects, setTrainingProjects] = useState<any[]>([]);
  const [trainingProjectId, setTrainingProjectId] = useState<string>('');
  const [trainingLocalModels, setTrainingLocalModels] = useState<any[]>([]);
  const [trainingLocalModelId, setTrainingLocalModelId] = useState<string>('');
  const [workspacePath, setWorkspacePath] = useState<string>('');
  const [currentConversationId, setCurrentConversationId] = useState<string | null>(null);
  const [conversations, setConversations] = useState<CoderIDEConversation[]>([]);
  const [showExportModal, setShowExportModal] = useState(false);
  const [selectedConversations, setSelectedConversations] = useState<Set<string>>(new Set());
  const [selectedMessages, setSelectedMessages] = useState<Set<string>>(new Set());
  const [includeContext, setIncludeContext] = useState(true);
  const [leftPanelWidth, _setLeftPanelWidth] = useState(250);
  const [rightPanelWidth, _setRightPanelWidth] = useState(350);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; path: string; isDir: boolean } | null>(null);
  const [creatingFile, setCreatingFile] = useState<{ parentPath: string; isDir: boolean } | null>(null);
  const [newFileName, setNewFileName] = useState('');
  const [renamingFile, setRenamingFile] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [editorInstance, setEditorInstance] = useState<any>(null);
  const [showFindReplace, setShowFindReplace] = useState(false);
  const [findText, setFindText] = useState('');
  const [replaceText, setReplaceText] = useState('');
  const [showGoToLine, setShowGoToLine] = useState(false);
  const [goToLineNumber, setGoToLineNumber] = useState('');
  const [showFileSearch, setShowFileSearch] = useState(false);
  const [fileSearchQuery, setFileSearchQuery] = useState('');
  const [fileSearchResults, setFileSearchResults] = useState<FileEntry[]>([]);
  // Workflows
  const [workflows, setWorkflows] = useState<Array<{
    id: string;
    name: string;
    description?: string;
    workflow_json: any;
  }>>([]);
  const [_showWorkflowEditor, setShowWorkflowEditor] = useState(false);
  const [editingWorkflow, setEditingWorkflow] = useState<string | null>(null);
  const [workflowName, setWorkflowName] = useState('');
  const [workflowDescription, setWorkflowDescription] = useState('');
  const [workflowTemplate, setWorkflowTemplate] = useState('');
  const [diffModal, setDiffModal] = useState<{
    filePath: string;
    original: string;
    modified: string;
    language: string;
  } | null>(null);
  const HISTORY_DIR = 'panther_chat_history';

  const startNewConversation = () => {
    setCurrentConversationId(null);
    setChatMessages([]);
    setAgentSummary(null);
    setAgentError(null);
    setAgentChanges(null);
    setPendingTools([]);
    setClineRunId(null);
    setClineConversationMessages([]);
  };

  const isWorkspaceRelativePath = (path: string) => {
    // Disallow absolute Windows paths or UNC paths; we only work inside the workspace
    if (!path) return false;
    if (/^[A-Za-z]:[\\/]/.test(path)) return false;
    if (path.startsWith('\\\\')) return false;
    return true;
  };

  const terminalEndRef = useRef<HTMLDivElement>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Load workspace on mount
  useEffect(() => {
    loadWorkspace();
    loadProviders();
    loadConversations();
    loadWorkflows();
    loadTrainingProjects();
  }, []);

  // Load project config from .panther in the workspace root
  useEffect(() => {
    const loadProjectConfig = async () => {
      if (!workspacePath) return;
      try {
        const content = await api.readWorkspaceFile('.panther');
        if (content && content.trim().length > 0) {
          const parsed: PantherProjectConfig = JSON.parse(content);
          setProjectConfig(parsed);
          // Apply default provider/model if configured and not already selected
          if (!selectedProvider && parsed.provider_id) {
            setSelectedProvider(parsed.provider_id);
          }
          if (!selectedModel && parsed.model_name) {
            setSelectedModel(parsed.model_name);
          }
        }
      } catch (err) {
        // It's fine if .panther doesn't exist; treat as optional
      }
    };
    loadProjectConfig();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspacePath]);

  useEffect(() => {
    // Auto-save conversation when messages change
    if (chatMessages.length > 0) {
      saveConversation();
    }
  }, [chatMessages]);

  useEffect(() => {
    terminalEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [terminalLines]);

  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chatMessages]);

  useEffect(() => {
    if (selectedProvider) {
      loadModels();
    }
  }, [selectedProvider]);

  useEffect(() => {
    if (trainingProjectId) {
      loadTrainingLocalModels(trainingProjectId);
    } else {
      setTrainingLocalModels([]);
      setTrainingLocalModelId('');
    }
  }, [trainingProjectId]);

  useEffect(() => {
    if (currentPath) {
      loadDirectory(currentPath);
    }
  }, [currentPath]);

  const loadWorkflows = async () => {
    try {
      const list = await api.listCoderWorkflows();
      setWorkflows(list);
    } catch (error) {
      console.error('Failed to load workflows:', error);
    }
  };

  const loadTrainingProjects = async () => {
    try {
      const projects = await api.listProjects();
      setTrainingProjects(projects);
      if (!trainingProjectId && projects.length > 0) {
        setTrainingProjectId(projects[0].id);
      }
    } catch (error) {
      console.error('Failed to load training projects:', error);
    }
  };

  const loadTrainingLocalModels = async (projectId: string) => {
    try {
      const models = await api.listLocalModels(projectId);
      setTrainingLocalModels(models);
      if (models.length > 0) {
        setTrainingLocalModelId(models[0].id);
      } else {
        setTrainingLocalModelId('');
      }
    } catch (error) {
      console.error('Failed to load local models for training:', error);
    }
  };

  const loadWorkspace = async () => {
    try {
      const path = await api.getWorkspacePath();
      setWorkspacePath(path);
      setCurrentPath(path); // Start in the workspace root instead of empty string
      await loadDirectory(path);
    } catch (error) {
      console.error('Failed to load workspace:', error);
    }
  };

  const loadDirectory = async (path: string) => {
    try {
      const fileList = await api.listWorkspaceFiles(path || undefined);
      setFiles(fileList);
    } catch (error) {
      console.error('Failed to load directory:', error);
    }
  };

  const loadConversations = async () => {
    try {
      const convs = await api.loadCoderIDEConversations();
      setConversations(convs);
      // Auto-select most recent conversation on load if none active
      if (!currentConversationId && chatMessages.length === 0 && convs.length > 0) {
        setCurrentConversationId(convs[0].id);
        setChatMessages(convs[0].messages || []);
      }
    } catch (error) {
      console.error('Failed to load conversations:', error);
    }
  };

  const saveConversation = async () => {
    if (chatMessages.length === 0) return;
    
    const convId = currentConversationId || crypto.randomUUID();
    const title = chatMessages[0]?.content?.substring(0, 50) || 'New Conversation';
    
    const conversation: CoderIDEConversation = {
      id: convId,
      title,
      messages: chatMessages,
      created_at: chatMessages[0]?.timestamp || new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    
    try {
      await api.saveCoderIDEConversation(conversation);
      if (!currentConversationId) {
        setCurrentConversationId(convId);
      }
      // Update conversations state
      setConversations(prev => {
        const existing = prev.find(c => c.id === convId);
        if (existing) {
          return prev.map(c => c.id === convId ? conversation : c);
        }
        return [conversation, ...prev];
      });

      // Also write a markdown copy of the conversation into the workspace for external reference
      try {
        // Ensure history directory exists (ignore errors if it already exists)
        await api.createWorkspaceFile(HISTORY_DIR, true);
      } catch {
        // best-effort only
      }
      try {
        const mdPath = `${HISTORY_DIR}/${convId}.md`;
        const mdContentLines: string[] = [];
        mdContentLines.push(`# Simple Coder Conversation`);
        mdContentLines.push('');
        mdContentLines.push(`- ID: ${convId}`);
        mdContentLines.push(`- Title: ${title}`);
        mdContentLines.push(`- Created: ${conversation.created_at}`);
        mdContentLines.push(`- Updated: ${conversation.updated_at}`);
        mdContentLines.push('');
        mdContentLines.push('## Messages (JSON payload for reload)');
        mdContentLines.push('');
        mdContentLines.push('```json');
        mdContentLines.push(JSON.stringify(chatMessages, null, 2));
        mdContentLines.push('```');
        mdContentLines.push('');
        const mdContent = mdContentLines.join('\n');
        await api.writeWorkspaceFile(mdPath, mdContent);
      } catch (err) {
        console.error('Failed to write markdown history file:', err);
      }
    } catch (error) {
      console.error('Failed to save conversation:', error);
    }
  };

  const searchFiles = async (query: string) => {
    if (!query.trim()) {
      setFileSearchResults([]);
      return;
    }
    
    // Recursively search files
    const searchRecursive = async (path: string, results: FileEntry[]): Promise<FileEntry[]> => {
      try {
        const entries = await api.listWorkspaceFiles(path || undefined);
        for (const entry of entries) {
          if (entry.name.toLowerCase().includes(query.toLowerCase())) {
            results.push(entry);
          }
          if (entry.is_dir) {
            await searchRecursive(entry.path, results);
          }
        }
      } catch (error) {
        // Ignore permission errors for some directories
      }
      return results;
    };
    
    const results = await searchRecursive('', []);
    setFileSearchResults(results.slice(0, 50)); // Limit to 50 results
  };

  const scanAndSummarizeFolder = async (folderPath: string) => {
    if (!selectedProvider || !selectedModel) {
      alert('Please select a provider and model first');
      return;
    }

      addTerminalLine('output', `\n🔍 Scanning folder: ${folderPath}...`);
      addTerminalLine('output', 'Collecting files (this may take a moment)...');
    setChatLoading(true);

    try {
      // Recursively collect all files in the folder
      const collectFiles = async (path: string, files: Array<{ path: string; content: string; language: string }>): Promise<void> => {
        try {
          const entries = await api.listWorkspaceFiles(path || undefined);
          for (const entry of entries) {
            if (entry.is_dir) {
              // Skip common ignore directories
              const name = entry.name.toLowerCase();
              if (name === 'node_modules' || name === '.git' || name === '__pycache__' || name === '.venv' || name === 'target' || name === 'dist' || name === 'build') {
                continue;
              }
              await collectFiles(entry.path, files);
            } else {
              // Skip binary files and large files
              const ext = entry.name.split('.').pop()?.toLowerCase() || '';
              const binaryExts = ['exe', 'dll', 'so', 'dylib', 'bin', 'jpg', 'jpeg', 'png', 'gif', 'ico', 'pdf', 'zip', 'tar', 'gz'];
              if (binaryExts.includes(ext)) continue;

              try {
                const content = await api.readWorkspaceFile(entry.path);
                // Skip very large files (>100KB)
                if (content.length > 100000) {
                  addTerminalLine('output', `Skipping large file: ${entry.path} (>100KB)`);
                  continue;
                }
                const language = await api.getFileLanguage(entry.path);
                files.push({ path: entry.path, content, language });
                addTerminalLine('output', `✓ Read: ${entry.name}`);
              } catch (error: any) {
                // Skip files that can't be read (system files, permissions, etc.)
                // Only log if it's not a common system file
                const fileName = entry.name.toLowerCase();
                const systemFiles = ['desktop.ini', 'thumbs.db', '.ds_store', 'folder.jpg'];
                if (!systemFiles.some(sf => fileName.includes(sf))) {
                  addTerminalLine('output', `⚠ Skipping: ${entry.name} (${error.message || 'unreadable'})`);
                }
              }
            }
          }
        } catch (error) {
          // Ignore permission errors
        }
      };

      const files: Array<{ path: string; content: string; language: string }> = [];
      await collectFiles(folderPath, files);

      if (files.length === 0) {
        addTerminalLine('error', 'No readable files found in this folder. Make sure the folder contains code files.');
        setChatLoading(false);
        return;
      }

      addTerminalLine('output', `\n✓ Found ${files.length} files. Sending to AI for analysis...`);

      // Build context for AI - include full content for small files, preview for larger ones
      const fileContext = files.map(f => {
        // For files under 2000 chars, include full content. For larger, include first 1000 and last 500
        let contentPreview: string;
        if (f.content.length <= 2000) {
          contentPreview = f.content;
        } else {
          const start = f.content.substring(0, 1000);
          const end = f.content.substring(f.content.length - 500);
          contentPreview = `${start}\n\n... [${f.content.length - 1500} characters omitted] ...\n\n${end}`;
        }
        return `File: ${f.path}\nLanguage: ${f.language}\nSize: ${f.content.length} characters\nContent:\n\`\`\`${f.language}\n${contentPreview}\n\`\`\``;
      }).join('\n\n---\n\n');

      // Ask AI to summarize
      const summaryPrompt = `Analyze the following codebase in the folder "${folderPath}". 

Files found:
${files.map(f => `- ${f.path} (${f.language})`).join('\n')}

File contents:
${fileContext}

Please provide:
1. A comprehensive summary of what this project/program does
2. The main technologies, frameworks, and languages used
3. Key features and functionality
4. Architecture overview
5. Important files and their purposes
6. Dependencies and requirements
7. How to run/use the project

Format your response as a detailed markdown document that can be saved as a README or project summary.`;

      const userMsg: CoderMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content: summaryPrompt,
        timestamp: new Date().toISOString(),
        context_files: files.map(f => f.path),
      };

      setChatMessages(prev => [...prev, userMsg]);

      const conversationContext = chatMessages.map(msg => ({
        id: msg.id,
        run_id: 'coder-ide',
        author_type: msg.role === 'user' ? 'user' : 'assistant',
        profile_id: null,
        round_index: null,
        turn_index: null,
        text: msg.content,
        created_at: msg.timestamp,
        provider_metadata_json: null
      }));

      const response = await api.coderChat({
        provider_id: selectedProvider,
        model_name: selectedModel,
        user_message: summaryPrompt,
        conversation_context: conversationContext,
        system_prompt: `You are Panther Coder, an AI coding assistant. Analyze codebases and provide comprehensive summaries. Be thorough and detailed.`,
      });

      const assistantMsg: CoderMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: response,
        timestamp: new Date().toISOString(),
        model: selectedModel,
        provider: selectedProvider,
      };

      setChatMessages(prev => [...prev, assistantMsg]);

      // Generate a proper filename for the summary
      const folderName = folderPath.split('/').pop() || folderPath.split('\\').pop() || 'project';
      const sanitizedFolderName = folderName.replace(/[^a-zA-Z0-9-_]/g, '_');
      const summaryFileName = `${sanitizedFolderName}_summary.md`;
      const summaryPath = folderPath ? `${folderPath}/${summaryFileName}` : summaryFileName;

      // Create the summary file
      await api.writeWorkspaceFile(summaryPath, response);
      addTerminalLine('output', `✓ Created summary: ${summaryPath}`);

      // Open the summary file
      await openFile({ name: summaryFileName, path: summaryPath, is_dir: false });

      // Continue in agent mode - ask AI what to do next
      const continuePrompt = `I've created a summary of the codebase at ${summaryPath}. 

Now, please analyze the codebase and suggest:
1. What improvements or changes could be made
2. Any bugs or issues you've identified
3. Next steps for development
4. Or ask me what specific task you'd like to work on

You can now make changes to the codebase using [MODIFY FILE: path] or [CREATE FILE: path] markers.`;

      const continueUserMsg: CoderMessage = {
        id: crypto.randomUUID(),
        role: 'user',
        content: continuePrompt,
        timestamp: new Date().toISOString(),
      };

      setChatMessages(prev => [...prev, continueUserMsg]);

      const continueContext = [...chatMessages, assistantMsg, continueUserMsg].map(msg => ({
        id: msg.id,
        run_id: 'coder-ide',
        author_type: msg.role === 'user' ? 'user' : 'assistant',
        profile_id: null,
        round_index: null,
        turn_index: null,
        text: msg.content,
        created_at: msg.timestamp,
        provider_metadata_json: null
      }));

      const continueResponse = await api.coderChat({
        provider_id: selectedProvider,
        model_name: selectedModel,
        user_message: continuePrompt,
        conversation_context: continueContext,
        system_prompt: `You are Panther Coder. You've just analyzed a codebase. Now help the user improve it, fix issues, or add features. Use [MODIFY FILE: path] and [CREATE FILE: path] markers to make changes.`,
      });

      const continueAssistantMsg: CoderMessage = {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: continueResponse,
        timestamp: new Date().toISOString(),
        model: selectedModel,
        provider: selectedProvider,
      };

      setChatMessages(prev => [...prev, continueAssistantMsg]);

      // Parse and execute any actions from the response
      await parseAndExecuteActions(continueResponse);

      addTerminalLine('output', '✓ Folder scan complete. Agent mode active.');
    } catch (error: any) {
      addTerminalLine('error', `Scan failed: ${error.message || error}`);
      alert(`Failed to scan folder: ${error.message || error}`);
    } finally {
      setChatLoading(false);
    }
  };

  useEffect(() => {
    if (showFileSearch) {
      const timeout = setTimeout(() => {
        searchFiles(fileSearchQuery);
      }, 300);
      return () => clearTimeout(timeout);
    }
  }, [fileSearchQuery, showFileSearch]);

  const loadProviders = async () => {
    try {
      const data = await api.listProviders();
      useAppStore.getState().setProviders(data);
      if (data.length > 0 && !selectedProvider) {
        setSelectedProvider(data[0].id);
      }
      setBackendStatus('ok');
      setBackendError('');
    } catch (error) {
      console.error('Failed to load providers:', error);
      setBackendStatus('error');
      setBackendError(String((error as any)?.message || error));
    }
  };

  const loadModels = async () => {
    if (!selectedProvider) return;
    try {
      const models = await api.listProviderModels(selectedProvider);
      setAvailableModels(models);
      if (models.length > 0) {
        setSelectedModel(models[0]);
      }
      setBackendStatus('ok');
      setBackendError('');
    } catch (error) {
      console.error('Failed to load models:', error);
      setBackendStatus('error');
      setBackendError(String((error as any)?.message || error));
    }
  };

  const navigateToDirectory = async (path: string) => {
    if (path === '..') {
      // Go up one level
      const parts = currentPath.split('/').filter(p => p);
      if (parts.length > 0) {
        parts.pop();
        setCurrentPath(parts.join('/'));
      } else {
        setCurrentPath('');
      }
    } else {
      setCurrentPath(path);
    }
  };

  const openFile = async (entry: FileEntry) => {
    if (entry.is_dir) {
      navigateToDirectory(entry.path);
      return;
    }

    // Check if already open
    if (openFiles.some(f => f.path === entry.path)) {
      setActiveFile(entry.path);
      return;
    }

    try {
      // Ensure we have the correct path - if entry.path is relative, it should work
      // If it's just a filename, try to construct the full path
      let filePath = entry.path;
      if (!filePath.includes('/') && !filePath.includes('\\') && currentPath) {
        // If path is just a filename and we have a current path, join them
        filePath = currentPath ? `${currentPath}/${filePath}` : filePath;
      }
      
      console.log('📂 Opening file:', { entryPath: entry.path, resolvedPath: filePath, currentPath });
      
      const content = await api.readWorkspaceFile(filePath);
      const language = await api.getFileLanguage(filePath);
      const newFile: OpenFile = {
        path: filePath,
        name: entry.name,
        content,
        language,
        modified: false,
      };
      setOpenFiles([...openFiles, newFile]);
      setActiveFile(filePath);
      console.log('✅ File opened successfully:', filePath);
    } catch (error: any) {
      console.error('❌ Failed to open file:', error);
      const errorMsg = error?.message || String(error);
      addTerminalLine('error', `Failed to open file: ${entry.path} - ${errorMsg}`);
      alert(`Failed to open file: ${entry.path}\n\nError: ${errorMsg}\n\nTried path: ${entry.path}`);
    }
  };

  const closeFile = (path: string) => {
    const file = openFiles.find(f => f.path === path);
    if (file?.modified) {
      if (!confirm('File has unsaved changes. Close anyway?')) {
        return;
      }
    }
    setOpenFiles(openFiles.filter(f => f.path !== path));
    if (activeFile === path) {
      const remaining = openFiles.filter(f => f.path !== path);
      setActiveFile(remaining.length > 0 ? remaining[remaining.length - 1].path : null);
    }
  };

  const saveFile = async (path: string) => {
    const file = openFiles.find(f => f.path === path);
    if (!file) return;

    try {
      await api.writeWorkspaceFile(path, file.content);
      setOpenFiles(openFiles.map(f => 
        f.path === path ? { ...f, modified: false } : f
      ));
      addTerminalLine('output', `Saved: ${path}`);
    } catch (error) {
      console.error('Failed to save file:', error);
      addTerminalLine('error', `Failed to save: ${path}`);
    }
  };

  const updateFileContent = (path: string, content: string | undefined) => {
    if (content === undefined) return;
    setOpenFiles(openFiles.map(f => 
      f.path === path ? { ...f, content, modified: true } : f
    ));
  };

  const addTerminalLine = (type: 'input' | 'output' | 'error', content: string) => {
    setTerminalLines(prev => [...prev, {
      type,
      content,
      timestamp: new Date().toISOString(),
    }]);
  };

  const executeTerminalCommand = async () => {
    console.log('🖥️ executeTerminalCommand called with input:', terminalInput);
    if (!terminalInput.trim() || terminalRunning) {
      console.log('🖥️ Command execution blocked - empty input or already running');
      return;
    }

    const cmd = terminalInput.trim();
    console.log('🖥️ Executing terminal command:', cmd);
    console.log('🖥️ currentPath:', currentPath, 'workspacePath:', workspacePath);
    const workingDir = currentPath || undefined;
    console.log('🖥️ Working directory being passed:', workingDir);
    addTerminalLine('input', `PS ${currentPath || workspacePath}> ${cmd}`);
    setTerminalInput('');
    setTerminalRunning(true);

    try {
      console.log('🖥️ Calling api.executeCommand...');
      const result = await api.executeCommand(cmd, workingDir);
      console.log('🖥️ Command result:', result);

      if (result.stdout) {
        console.log('🖥️ Adding stdout line:', result.stdout);
        addTerminalLine('output', result.stdout);
      }
      if (result.stderr) {
        console.log('🖥️ Adding stderr line:', result.stderr);
        addTerminalLine('error', result.stderr);
      }
      if (!result.success) {
        console.log('🖥️ Adding exit code line:', result.exit_code);
        addTerminalLine('error', `Exit code: ${result.exit_code}`);
      }

      // Show result even if empty for debugging
      if (!result.stdout && !result.stderr) {
        console.log('🖥️ No output, adding empty result indicator');
        addTerminalLine('output', '(no output)');
      }

      // Debug: always show the command result details
      console.log('🖥️ Command completed with:', {
        exitCode: result.exit_code,
        success: result.success,
        stdoutLength: result.stdout.length,
        stderrLength: result.stderr.length
      });

      // Always refresh explorer after a command in case files/folders changed
      await loadDirectory(currentPath || '');
    } catch (error: any) {
      console.error('🖥️ Terminal command error:', error);
      addTerminalLine('error', `Error: ${error.message || error}`);
    } finally {
      setTerminalRunning(false);
    }
  };

  // Sync streamed text to assistant message when using hook
  useEffect(() => {
    if (!streamingAssistantId || streamedText === undefined) return;
    setChatMessages(prev =>
      prev.map((msg) =>
        msg.id === streamingAssistantId ? { ...msg, content: streamedText } : msg
      )
    );
  }, [streamedText, streamingAssistantId]);

  // Sync chatLoading with isStreaming when using streaming hook
  useEffect(() => {
    if (streamingAssistantId) setChatLoading(isStreaming);
  }, [isStreaming, streamingAssistantId]);

  // When streaming completes, run parseAndExecuteActions and verification
  useEffect(() => {
    if (prevStreamingRef.current && !isStreaming && streamingAssistantId) {
      prevStreamingRef.current = false;
      setStreamingAssistantId(null);
      if (error) {
        addTerminalLine('error', `Chat error: ${error}`);
        setChatMessages(prev =>
          prev.map((msg) =>
            msg.id === streamingAssistantId ? { ...msg, content: `Error: ${error}` } : msg
          )
        );
      } else if (streamedText) {
        console.log('🤖 AI Response:', streamedText);
        parseAndExecuteActions(streamedText);
        if (projectConfig?.verification?.test_command) {
          const lower = streamedText.toLowerCase();
          if (lower.includes('[modify file:') || lower.includes('[create file:')) {
            api.executeCommand(projectConfig.verification.test_command, workspacePath || undefined)
              .then((result) => {
                addTerminalLine('input', `Verification: ${projectConfig!.verification!.test_command}`);
                if (result.stdout) addTerminalLine('output', result.stdout);
                if (result.stderr) addTerminalLine('error', result.stderr);
                if (!result.success) addTerminalLine('error', `Verification failed (exit code ${result.exit_code}).`);
                else addTerminalLine('output', 'Verification command completed successfully.');
              })
              .catch((err: any) => addTerminalLine('error', `Verification command error: ${err?.message || err}`));
          }
        }
      }
    }
    if (isStreaming) prevStreamingRef.current = true;
  }, [isStreaming, streamingAssistantId, streamedText, error]);

  const handleChatSend = async () => {
    if (!chatInput.trim() || chatLoading) return;
    if (!selectedProvider || !selectedModel) {
      alert('Please select a provider and model');
      return;
    }

    // Capture context
    const contextFiles = activeFile ? [activeFile] : [];
    const recentTerminal = terminalLines.slice(-5).map(l => l.content).join('\n');
    
    const userMsg: CoderMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content: chatInput.trim(),
      timestamp: new Date().toISOString(),
      context_files: contextFiles.length > 0 ? contextFiles : undefined,
      terminal_output: recentTerminal || undefined,
    };

    setChatMessages(prev => [...prev, userMsg]);
    const input = chatInput.trim();
    setChatInput('');
    setChatLoading(true);

    try {
      // Build context with current file if open
      let context = '';
      if (activeFile) {
        const file = openFiles.find(f => f.path === activeFile);
        if (file) {
          context = `\n\nCurrent file (${file.name}):\n\`\`\`${file.language}\n${file.content}\n\`\`\``;
        }
      }

      // Get recent terminal output for context
      const recentTerminal = terminalLines.slice(-10).map(l => 
        `${l.type === 'input' ? '>' : l.type === 'error' ? 'ERROR' : 'OUT'}: ${l.content}`
      ).join('\n');

      const defaultSystemPrompt = `You are Panther Coder, an AI coding assistant integrated into an IDE.
Your primary job is to help the user build and evolve real projects (apps, libraries, scripts) in a way that is structured, platform-aware, and safe by default.

You must ALWAYS:
- Ask clarifying questions when the user's goal, target platform, or constraints are unclear
- Confirm the target platform and stack when the user is "building an app" (e.g. web, mobile, desktop, backend API, CLI; frameworks like React, Next.js, Flutter, .NET, etc.)
- Keep track of the current project context and refer back to previous decisions, files, and constraints
- Propose a short plan before large changes, and adjust the plan if the user pushes back
- Prefer minimal, incremental edits over huge rewrites unless the user explicitly asks for a full rewrite

You can help with:
- Writing, reviewing, and debugging code
- Explaining code and concepts
- Creating and modifying files and folders
- Running terminal commands
- Detecting and fixing errors iteratively

IMPORTANT WHEN RUNNING COMMANDS:
1. Analyze error messages carefully
2. Identify missing dependencies (Python, Node, npm, pip, etc.)
3. Fix the code or suggest installation commands
4. Test again with [RUN: command] until it works
5. If something is ambiguous or dangerous (like deleting many files), ASK THE USER FIRST.

FORMATTING RULES:
- When providing code, use markdown code blocks with the language specified.
- When suggesting file operations, clearly indicate:
  - [CREATE FILE: path] for new files
  - [MODIFY FILE: path] for changes
  - [DELETE FILE: path] for deletions
  - [RUN: command] for terminal commands
- When you need more information from the user, explicitly ask short, direct questions.

IMPORTANT: Always use the EXACT markers like [CREATE FILE: src/components/Button.tsx] followed immediately by a code block containing the file content. Do NOT use variations like "Create file:" or "Modify file:".

PROJECT CONTEXT:
- The user's workspace is at: ${workspacePath}
- Current directory: ${currentPath || '/'}

Recent terminal output:
${recentTerminal || '(No terminal output yet)'}

${context}`;

      const overridePrompt = projectConfig?.workflow_prompts?.['coder_ide_chat']?.system_prompt;
      const systemPrompt = overridePrompt || defaultSystemPrompt;

      const conversationContext = chatMessages.map(msg => ({
        id: msg.id,
        run_id: 'coder-ide',
        author_type: msg.role === 'user' ? 'user' : 'assistant',
        profile_id: null,
        round_index: null,
        turn_index: null,
        text: msg.content,
        created_at: msg.timestamp,
        provider_metadata_json: null
      }));

      let assistantText = '';

      if (chatMode === 'auto' && trainingProjectId) {
        const autoResponse = await api.coderAutoChat({
          project_id: trainingProjectId,
          local_model_id: trainingLocalModelId || undefined,
          provider_id: selectedProvider,
          model_name: selectedModel,
          user_message: input,
          conversation_context: conversationContext,
          system_prompt: systemPrompt,
        });
        assistantText = autoResponse.answer;
        setChatLoading(false); // Auto mode doesn't use streaming, so set loading false immediately

        // Best-effort: immediately ingest this turn as training data
        try {
          await api.ingestCoderTurn({
            project_id: trainingProjectId,
            local_model_id: trainingLocalModelId || undefined,
            user_text: input,
            assistant_text: assistantText,
            context_files: userMsg.context_files || [],
            terminal_output: userMsg.terminal_output,
          });
        } catch (err) {
          console.warn('Auto-training ingest failed (Coder IDE):', err);
        }
      } else if (useStreaming && coderStreamingConfig) {
        // Manual/IDE chat uses streaming via useStreamingLLM hook
        const assistantId = crypto.randomUUID();
        const assistantPlaceholder: CoderMessage = {
          id: assistantId,
          role: 'assistant',
          content: '',
          timestamp: new Date().toISOString(),
          model: selectedModel,
          provider: selectedProvider,
        };
        setChatMessages(prev => [...prev, assistantPlaceholder]);
        setStreamingAssistantId(assistantId);
        await startStream(input, {
          conversationContext,
          systemPrompt,
        });
        // parseAndExecuteActions and verification run in useEffect when streaming completes
      } else {
        // Non-streaming fallback
        console.log('📝 Using regular chat (streaming disabled)');
        const response = await api.coderChat({
          provider_id: selectedProvider,
          model_name: selectedModel,
          user_message: input,
          conversation_context: conversationContext,
          system_prompt: systemPrompt,
        });
        assistantText = response;
        setChatLoading(false);
      }

      // For streaming, assistant is already added as placeholder; parseAndExecuteActions runs in useEffect
      if (!(useStreaming && coderStreamingConfig)) {
        const assistantMsg: CoderMessage = {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: assistantText,
          timestamp: new Date().toISOString(),
          model: selectedModel,
          provider: selectedProvider,
        };
        setChatMessages(prev => [...prev, assistantMsg]);
        setChatLoading(false);
        console.log('🤖 AI Response:', assistantText);
        parseAndExecuteActions(assistantText);
        if (projectConfig?.verification?.test_command) {
          const lower = assistantText.toLowerCase();
          if (lower.includes('[modify file:') || lower.includes('[create file:')) {
            try {
              addTerminalLine('input', `Verification: ${projectConfig.verification.test_command}`);
              const result = await api.executeCommand(projectConfig.verification.test_command, workspacePath || undefined);
              if (result.stdout) addTerminalLine('output', result.stdout);
              if (result.stderr) addTerminalLine('error', result.stderr);
              if (!result.success) addTerminalLine('error', `Verification failed (exit code ${result.exit_code}).`);
              else addTerminalLine('output', 'Verification command completed successfully.');
            } catch (err: any) {
              addTerminalLine('error', `Verification command error: ${err?.message || err}`);
            }
          }
        }
      }
    } catch (error: any) {
      addTerminalLine('error', `Chat error: ${error.message || error}`);
    } finally {
      setChatLoading(false);
    }
  };

  const handleAgentRun = async () => {
    if (!agentTask.trim() || agentRunning) return;
    if (!selectedProvider || !selectedModel) {
      alert('Please select a provider and model');
      return;
    }

    const workingDir = currentPath || workspacePath || '';
    if (!workingDir) {
      alert('Please select a workspace directory in the file explorer');
      return;
    }

    setAgentRunning(true);
    setAgentSummary(null);
    setAgentError(null);
    setAgentChanges(null);
    setPendingTools([]);

    try {
      if (agentStyle === 'approve') {
        // Cline-style: tool approval flow
        const conversationContext = clineConversationMessages.length > 0
          ? clineConversationMessages.map(m => ({ role: m.role, content: m.content }))
          : undefined;

        const response = await api.clineAgentTask({
          provider_id: selectedProvider,
          model_name: selectedModel,
          task_description: agentTask.trim(),
          workspace_path: workingDir,
          target_paths: undefined,
          conversation_context: conversationContext,
        });

        setAgentSummary(response.summary);
        setClineRunId(response.run_id);
        setPendingTools(response.tool_executions || []);
        setClineConversationMessages(prev => [
          ...prev,
          { role: 'user', content: agentTask.trim() },
          { role: 'assistant', content: response.summary },
        ]);
      } else {
        // Panther-style: propose changes
        let targetPaths: string[] | undefined;
        if (agentScope === 'file' && activeFile) {
          targetPaths = [activeFile];
        } else if (agentScope === 'folder') {
          targetPaths = [workingDir];
        } else {
          targetPaths = undefined;
        }

        const response = await api.runCoderAgentTask({
          provider_id: selectedProvider,
          model_name: selectedModel,
          task_description: agentTask.trim(),
          target_paths: targetPaths,
          allow_file_writes: agentAllowWrites,
          allow_commands: agentAllowCommands,
          max_steps: 8,
        });

        setAgentSummary(response.summary);
        setAgentChanges({
          run_id: response.run_id,
          items: response.proposed_changes,
        });
      }
    } catch (error: any) {
      console.error('Agent task failed:', error);
      setAgentError(error?.message || String(error));
    } finally {
      setAgentRunning(false);
    }
  };

  const approveTool = async (toolId: string, approved: boolean) => {
    setToolBusyIds(prev => (prev.includes(toolId) ? prev : [...prev, toolId]));
    try {
      const result = await api.clineApproveTool({ tool_id: toolId, approved });
      setPendingTools(prev => prev.map(t =>
        t.id === toolId
          ? { ...t, approval_status: approved ? 'approved' : 'rejected', result: result.result }
          : t
      ));
      if (approved && result.executed && (currentPath || workspacePath)) {
        loadDirectory(currentPath || workspacePath || '');
      }
    } catch (error: any) {
      console.error('Failed to approve tool:', error);
      alert(`Failed: ${error?.message || error}`);
    } finally {
      setToolBusyIds(prev => prev.filter(id => id !== toolId));
    }
  };

  const handleClineContinue = async () => {
    if (!agentResponseInput.trim() || agentRunning || !clineRunId) return;
    const msg = agentResponseInput.trim();
    setAgentResponseInput('');
    setAgentRunning(true);
    try {
      const response = await api.clineAgentTask({
        provider_id: selectedProvider,
        model_name: selectedModel,
        task_description: msg,
        workspace_path: currentPath || workspacePath || '',
        conversation_context: clineConversationMessages.map(m => ({ role: m.role, content: m.content })),
      });
      setAgentSummary(response.summary);
      setPendingTools(response.tool_executions || []);
      setClineConversationMessages(prev => [
        ...prev,
        { role: 'user', content: msg },
        { role: 'assistant', content: response.summary },
      ]);
    } catch (error: any) {
      setAgentError(error?.message || String(error));
    } finally {
      setAgentRunning(false);
    }
  };

  const applyAgentChange = async (change: {
    file_path: string;
    description?: string;
    new_content: string;
  }) => {
    if (!agentChanges) return;
    try {
      // Mirror the logic from applyAllAgentChanges so single-apply
      // also understands folder vs file semantics.
      if (!change.new_content || change.new_content.trim() === '') {
        const pathParts = change.file_path.split(/[/\\]/);
        const lastPart = pathParts[pathParts.length - 1] || '';
        const hasExtension = lastPart.includes('.') && !lastPart.startsWith('.');
        const isDir =
          change.file_path.endsWith('/') ||
          change.file_path.endsWith('\\') ||
          !hasExtension;

        if (isDir) {
          // Create directory (and parents) instead of trying to write a file
          await api.createWorkspaceFile(change.file_path, true);
          addTerminalLine('output', `Created folder: ${change.file_path}`);
        } else {
          // Create empty file
          await api.writeWorkspaceFile(change.file_path, '');
          addTerminalLine('output', `Created empty file: ${change.file_path}`);
        }
      } else {
        // Write file with content (parent directories will be created automatically)
        await api.writeWorkspaceFile(change.file_path, change.new_content);
        addTerminalLine('output', `Applied agent change to ${change.file_path}`);
      }

      await api.recordAgentApplySteps({
        run_id: agentChanges.run_id,
        changes: [change],
      });
    } catch (error: any) {
      addTerminalLine('error', `Failed to apply agent change: ${error.message || error}`);
      alert(`Failed to apply change: ${error.message || error}`);
    }
  };

  const applyAllAgentChanges = async () => {
    if (!agentChanges || agentChanges.items.length === 0) return;
    if (!confirm(`Apply all ${agentChanges.items.length} proposed changes?`)) {
      return;
    }
    try {
      for (const change of agentChanges.items) {
        // If new_content is empty, create the folder/directory instead
        if (!change.new_content || change.new_content.trim() === '') {
          // Check if path ends with a separator or looks like a directory
          const pathParts = change.file_path.split(/[/\\]/);
          const lastPart = pathParts[pathParts.length - 1] || '';
          const hasExtension = lastPart.includes('.') && !lastPart.startsWith('.');
          const isDir = change.file_path.endsWith('/') || change.file_path.endsWith('\\') || !hasExtension;
          if (isDir) {
            // Create directory
            await api.createWorkspaceFile(change.file_path, true);
            addTerminalLine('output', `Created folder: ${change.file_path}`);
          } else {
            // Create empty file
            await api.writeWorkspaceFile(change.file_path, '');
            addTerminalLine('output', `Created empty file: ${change.file_path}`);
          }
        } else {
          // Write file with content (parent directories will be created automatically)
          await api.writeWorkspaceFile(change.file_path, change.new_content);
          addTerminalLine('output', `Applied change to: ${change.file_path}`);
        }
      }
      await api.recordAgentApplySteps({
        run_id: agentChanges.run_id,
        changes: agentChanges.items,
      });
      addTerminalLine('output', `✅ Applied ${agentChanges.items.length} agent changes successfully.`);
      // Refresh explorer to show new files/folders
      await loadDirectory(currentPath || '');
    } catch (error: any) {
      console.error('❌ Failed to apply agent changes:', error);
      addTerminalLine('error', `Failed to apply all agent changes: ${error.message || error}`);
      alert(`Failed to apply all changes: ${error.message || error}`);
    }
  };

  const openDiffForChange = async (change: {
    file_path: string;
    description?: string;
    new_content: string;
  }) => {
    // Instead of a popup diff, open the proposed file content
    // directly in the main editor area so it replaces the
    // "Select a file to edit, or start chatting..." placeholder.
    try {
      const path = change.file_path;

      // Try to detect language from the path; fall back to plaintext.
      let language = 'plaintext';
      try {
        language = await api.getFileLanguage(path);
      } catch {
        language = 'plaintext';
      }

      setOpenFiles(prev => {
        const existing = prev.find(f => f.path === path);
        const updated: OpenFile = existing
          ? {
              ...existing,
              content: change.new_content,
              language: existing.language || language,
              modified: true,
            }
          : {
              path,
              name: path.split('/').pop() || path,
              content: change.new_content,
              language,
              modified: true,
            };

        const others = prev.filter(f => f.path !== path);
        return [...others, updated];
      });

      setActiveFile(path);
    } catch (error: any) {
      console.error('Failed to open agent change in editor:', error);
      alert(`Failed to open file in editor: ${error.message || error}`);
    }
  };

  const checkAndInstallDependency = async (dependency: string, installCommand: string): Promise<boolean> => {
    const exists = await api.checkDependency(dependency);
    if (!exists) {
      const shouldInstall = confirm(`${dependency} is not installed. Would you like to install it?\n\nInstall command: ${installCommand}`);
      if (shouldInstall) {
        addTerminalLine('output', `Installing ${dependency}...`);
        try {
          const result = await api.installDependencyCommand(dependency, installCommand);
          if (result.stdout) addTerminalLine('output', result.stdout);
          if (result.stderr) addTerminalLine('error', result.stderr);
          if (result.success) {
            addTerminalLine('output', `✓ ${dependency} installed successfully`);
            return true;
          } else {
            addTerminalLine('error', `✗ Failed to install ${dependency}`);
            return false;
          }
        } catch (error: any) {
          addTerminalLine('error', `Failed to install ${dependency}: ${error.message || error}`);
          return false;
        }
      }
      return false;
    }
    return true;
  };

  const parseAndExecuteActions = async (response: string, iteration = 0, maxIterations = 5): Promise<void> => {
    if (iteration >= maxIterations) {
      addTerminalLine('error', 'Maximum iteration limit reached. Please review errors manually.');
      return;
    }

    console.log('🔍 parseAndExecuteActions called with response:', response.substring(0, 200) + '...');

    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    let hasErrors = false;
    let terminalOutput = '';

    // Look for file creation markers
    const createMatch = response.match(/\[CREATE FILE: ([^\]]+)\]/g);
    console.log('🔍 CREATE FILE matches:', createMatch);
    if (createMatch) {
      for (const match of createMatch) {
        const path = match.replace('[CREATE FILE: ', '').replace(']', '');
        // Find code block after this marker
        const codeMatch = response.match(new RegExp(`\\[CREATE FILE: ${path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\][\\s\\S]*?\`\`\`[a-z]*\\n([\\s\\S]*?)\`\`\``));
        if (codeMatch) {
          if (!isWorkspaceRelativePath(path)) {
            addTerminalLine('error', `Refusing to create file outside workspace: ${path}`);
            hasErrors = true;
            terminalOutput += `\nRefused to create outside workspace: ${path}`;
            continue;
          }
          try {
            await api.writeWorkspaceFile(path, codeMatch[1]);
            addTerminalLine('output', `Created: ${path}`);
            await loadDirectory(currentPath);
            // Open the file if it's not already open
            if (!openFiles.some(f => f.path === path)) {
              await openFile({ name: path.split('/').pop() || path, path, is_dir: false });
            }
          } catch (error: any) {
            addTerminalLine('error', `Failed to create: ${path} - ${error.message || error}`);
            hasErrors = true;
            terminalOutput += `\nError creating ${path}: ${error.message || error}`;
          }
        }
      }
    }

    // Look for file modification markers
    const modifyMatch = response.match(/\[MODIFY FILE: ([^\]]+)\]/g);
    console.log('🔍 MODIFY FILE matches:', modifyMatch);
    if (modifyMatch) {
      for (const match of modifyMatch) {
        const path = match.replace('[MODIFY FILE: ', '').replace(']', '');
        const codeMatch = response.match(new RegExp(`\\[MODIFY FILE: ${path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\][\\s\\S]*?\`\`\`[a-z]*\\n([\\s\\S]*?)\`\`\``));
        if (codeMatch) {
          if (!isWorkspaceRelativePath(path)) {
            addTerminalLine('error', `Refusing to modify file outside workspace: ${path}`);
            hasErrors = true;
            terminalOutput += `\nRefused to modify outside workspace: ${path}`;
            continue;
          }
          try {
            await api.writeWorkspaceFile(path, codeMatch[1]);
            addTerminalLine('output', `Modified: ${path}`);
            // Update open file if it's open
            if (openFiles.some(f => f.path === path)) {
              updateFileContent(path, codeMatch[1]);
            }
          } catch (error: any) {
            addTerminalLine('error', `Failed to modify: ${path} - ${error.message || error}`);
            hasErrors = true;
            terminalOutput += `\nError modifying ${path}: ${error.message || error}`;
          }
        }
      }
    }

    // Look for run commands
    const runMatch = response.match(/\[RUN: ([^\]]+)\]/g);
    console.log('🔍 RUN matches:', runMatch);
    if (runMatch) {
      for (const match of runMatch) {
        const cmd = match.replace('[RUN: ', '').replace(']', '');

        // Ask the user for permission before running any agent-suggested command.
        const cwdLabel = currentPath || workspacePath || '~';
        const shouldRun = confirm(
          `Panther agent wants to run the following command:\n\n` +
          `${cmd}\n\n` +
          `Working directory: ${cwdLabel}\n\n` +
          `Do you want to run this command?`
        );

        if (!shouldRun) {
          addTerminalLine('output', `Skipped command (user declined): ${cmd}`);
          terminalOutput += `\nSkipped command (user declined): ${cmd}`;
          continue;
        }

        addTerminalLine('input', `PS ${cwdLabel}> ${cmd}`);
        try {
          const result = await api.executeCommand(cmd, currentPath || undefined);
          if (result.stdout) {
            addTerminalLine('output', result.stdout);
            terminalOutput += `\n${result.stdout}`;
          }
          if (result.stderr) {
            addTerminalLine('error', result.stderr);
            terminalOutput += `\n${result.stderr}`;
            hasErrors = true;
          }
          if (!result.success) {
            hasErrors = true;
            terminalOutput += `\nExit code: ${result.exit_code}`;
          }

          // Check for common missing dependencies
          const stderrLower = result.stderr.toLowerCase();
          const stdoutLower = result.stdout.toLowerCase();
          const combined = stderrLower + stdoutLower;

          if (combined.includes("python") && (combined.includes("not found") || combined.includes("not recognized"))) {
            const installed = await checkAndInstallDependency('python', 'winget install Python.Python.3.12');
            if (installed) {
              // Retry the command
              addTerminalLine('output', 'Retrying command after Python installation...');
              const retryResult = await api.executeCommand(cmd, currentPath || undefined);
              if (retryResult.stdout) addTerminalLine('output', retryResult.stdout);
              if (retryResult.stderr) addTerminalLine('error', retryResult.stderr);
              if (retryResult.success) hasErrors = false;
            }
          } else if (combined.includes("node") && (combined.includes("not found") || combined.includes("not recognized"))) {
            await checkAndInstallDependency('node', 'winget install OpenJS.NodeJS');
          } else if (combined.includes("npm") && (combined.includes("not found") || combined.includes("not recognized"))) {
            await checkAndInstallDependency('npm', 'winget install OpenJS.NodeJS');
          } else if (combined.includes("pip") && (combined.includes("not found") || combined.includes("not recognized"))) {
            await checkAndInstallDependency('pip', 'python -m ensurepip --upgrade');
          }

          // Refresh explorer after each [RUN: ] command to reflect any file changes
          await loadDirectory(currentPath || '');
        } catch (error: any) {
          addTerminalLine('error', `Error: ${error.message || error}`);
          hasErrors = true;
          terminalOutput += `\nError: ${error.message || error}`;
        }
      }
    }

    // If no actions were found, log this for debugging
    if (!createMatch && !modifyMatch && !runMatch) {
      console.log('⚠️ No action markers found in response. AI might be using different formatting.');
      addTerminalLine('output', 'ℹ️ No executable actions found in response. AI may need to use specific markers like [CREATE FILE: path] or [RUN: command].');
    }
  };

  const handleContextMenu = (e: React.MouseEvent, entry: FileEntry) => {
    e.preventDefault();
    setContextMenu({
      x: e.clientX,
      y: e.clientY,
      path: entry.path,
      isDir: entry.is_dir,
    });
  };

  const handleCreateFile = async (isDir: boolean) => {
    if (!contextMenu) return;
    const parentPath = contextMenu.isDir ? contextMenu.path : currentPath;
    setCreatingFile({ parentPath, isDir });
    setNewFileName('');
    setContextMenu(null);
  };

  const submitCreateFile = async () => {
    if (!creatingFile || !newFileName.trim()) return;
    // Handle path construction properly
    let path: string;
    if (creatingFile.parentPath) {
      // If parent path is set, join it
      path = creatingFile.parentPath === '' ? newFileName : `${creatingFile.parentPath}/${newFileName}`;
    } else {
      // Use current path or workspace root
      path = currentPath ? `${currentPath}/${newFileName}` : newFileName;
    }
    
    try {
      await api.createWorkspaceFile(path, creatingFile.isDir);
      await loadDirectory(currentPath);
      if (!creatingFile.isDir) {
        // Wait a bit for file to be created, then open it
        setTimeout(async () => {
          await openFile({ name: newFileName, path, is_dir: false });
        }, 100);
      }
      addTerminalLine('output', `Created ${creatingFile.isDir ? 'folder' : 'file'}: ${path}`);
    } catch (error: any) {
      addTerminalLine('error', `Failed to create: ${error.message || error}`);
    }
    setCreatingFile(null);
    setNewFileName('');
  };

  const handleDelete = async () => {
    if (!contextMenu) return;
    if (!confirm(`Delete ${contextMenu.path}?`)) {
      setContextMenu(null);
      return;
    }
    try {
      await api.deleteWorkspaceFile(contextMenu.path);
      closeFile(contextMenu.path);
      await loadDirectory(currentPath);
      addTerminalLine('output', `Deleted: ${contextMenu.path}`);
    } catch (error: any) {
      addTerminalLine('error', `Failed to delete: ${error.message || error}`);
    }
    setContextMenu(null);
  };

  const handleRename = () => {
    if (!contextMenu) return;
    setRenamingFile(contextMenu.path);
    setRenameValue(contextMenu.path.split('/').pop() || '');
    setContextMenu(null);
  };

  const submitRename = async () => {
    if (!renamingFile || !renameValue.trim()) return;
    const oldPath = renamingFile;
    const pathParts = oldPath.split('/');
    pathParts[pathParts.length - 1] = renameValue.trim();
    const newPath = pathParts.join('/');
    
    try {
      await api.renameWorkspaceFile(oldPath, newPath);
      // Update open files if this file is open
      if (openFiles.some(f => f.path === oldPath)) {
        setOpenFiles(openFiles.map(f => 
          f.path === oldPath ? { ...f, path: newPath, name: renameValue.trim() } : f
        ));
        if (activeFile === oldPath) {
          setActiveFile(newPath);
        }
      }
      await loadDirectory(currentPath);
      addTerminalLine('output', `Renamed: ${oldPath} -> ${newPath}`);
    } catch (error: any) {
      addTerminalLine('error', `Failed to rename: ${error.message || error}`);
    }
    setRenamingFile(null);
    setRenameValue('');
  };

  const activeFileData = openFiles.find(f => f.path === activeFile);

  // Get display path
  const displayPath = currentPath || workspacePath;

  const openWorkflowEditor = (workflow?: {
    id: string;
    name: string;
    description?: string;
    workflow_json: any;
  }) => {
    if (workflow) {
      setEditingWorkflow(workflow.id);
      setWorkflowName(workflow.name);
      setWorkflowDescription(workflow.description || '');
      setWorkflowTemplate(
        workflow.workflow_json?.task_template ||
        workflow.workflow_json?.template ||
        ''
      );
    } else {
      setEditingWorkflow(null);
      setWorkflowName('');
      setWorkflowDescription('');
      setWorkflowTemplate('');
    }
    setShowWorkflowEditor(true);
  };

  const _saveWorkflow = async () => {
    if (!workflowName.trim() || !workflowTemplate.trim()) {
      alert('Please provide a name and template for the workflow.');
      return;
    }
    try {
      const workflowJson = {
        type: 'agent_task',
        task_template: workflowTemplate,
      };
      await api.saveCoderWorkflow({
        id: editingWorkflow || undefined,
        name: workflowName.trim(),
        description: workflowDescription.trim() || undefined,
        workflow_json: workflowJson,
      });
      await loadWorkflows();
      setShowWorkflowEditor(false);
      setEditingWorkflow(null);
      setWorkflowName('');
      setWorkflowDescription('');
      setWorkflowTemplate('');
    } catch (error: any) {
      alert(`Failed to save workflow: ${error.message || error}`);
    }
  };

  const deleteWorkflow = async (id: string) => {
    if (!confirm('Delete this workflow?')) return;
    try {
      await api.deleteCoderWorkflow(id);
      await loadWorkflows();
    } catch (error: any) {
      alert(`Failed to delete workflow: ${error.message || error}`);
    }
  };

  const runWorkflow = async (workflow: {
    id: string;
    name: string;
    description?: string;
    workflow_json: any;
  }) => {
    if (!selectedProvider || !selectedModel) {
      alert('Please select a provider and model first');
      return;
    }
    const template: string =
      workflow.workflow_json?.task_template ||
      workflow.workflow_json?.template ||
      '';
    if (!template.trim()) {
      alert('Workflow has no task_template defined.');
      return;
    }

    // Very simple templating: replace {{path}} with current file/folder
    const scopePath =
      activeFile || currentPath || workspacePath || '';
    const task = template.replace(/{{\s*path\s*}}/g, scopePath);

    setAgentTask(task);
    await handleAgentRun();
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: 'var(--bg-primary)' }}>
      {/* Toolbar */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '10px',
        padding: '8px 12px',
        borderBottom: '1px solid var(--border-color)',
        background: 'var(--bg-secondary)',
      }}>
        <button
          onClick={() => navigate('/home')}
          style={{
            padding: '4px 8px',
            fontSize: '12px',
            borderRadius: '4px',
            border: '1px solid var(--border-color)',
            background: 'var(--bg-primary)',
            color: 'var(--text-primary)',
            cursor: 'pointer',
          }}
        >
          ← Back to Home
        </button>
        <span style={{ fontWeight: '600', fontSize: '14px' }}>🐆 Panther Coder</span>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
            padding: '2px 8px',
            borderRadius: '999px',
            border: '1px solid var(--border-color)',
            background: backendStatus === 'error' ? 'rgba(220, 53, 69, 0.15)' : 'rgba(40, 167, 69, 0.15)',
            color: backendStatus === 'error' ? '#dc3545' : '#28a745',
            fontSize: '11px',
            fontWeight: 600,
          }}
          title={backendStatus === 'error' ? backendError : 'Backend reachable'}
        >
          {backendStatus === 'error' ? 'Backend error' : 'Backend OK'}
          {backendStatus === 'error' && (
            <button
              onClick={() => {
                loadProviders();
                if (selectedProvider) loadModels();
              }}
              style={{
                padding: '2px 6px',
                fontSize: '10px',
                borderRadius: '999px',
                border: '1px solid var(--border-color)',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                cursor: 'pointer',
              }}
            >
              Reload
            </button>
          )}
        </div>
        {/* Current folder display and selector */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginLeft: '12px' }}>
          <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Folder:</span>
          <input
            type="text"
            value={currentPath || workspacePath || ''}
            onChange={(e) => {
              const newPath = e.target.value.trim();
              setCurrentPath(newPath);
              if (newPath) {
                loadDirectory(newPath);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                const newPath = (e.target as HTMLInputElement).value.trim();
                setCurrentPath(newPath);
                if (newPath) {
                  loadDirectory(newPath);
                }
              }
            }}
            placeholder="Current folder path"
            style={{
              padding: '4px 8px',
              fontSize: '11px',
              borderRadius: '4px',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              minWidth: '200px',
              fontFamily: 'monospace',
            }}
            title="Current working folder for agent tasks. Press Enter to change."
          />
          <button
            onClick={() => {
              if (currentPath) {
                loadDirectory(currentPath);
              }
            }}
            style={{
              padding: '4px 8px',
              fontSize: '11px',
              borderRadius: '4px',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              cursor: 'pointer',
            }}
            title="Refresh folder"
          >
            ↻
          </button>
        </div>
        <div style={{ flex: 1 }} />
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <button
            onClick={startNewConversation}
            style={{
              padding: '4px 8px',
              fontSize: '12px',
              borderRadius: '4px',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              cursor: 'pointer',
            }}
            title="Start a brand new chat"
          >
            New Chat
          </button>
          {/* Conversation selector / history loader */}
          {conversations.length > 0 && (
            <select
              value={currentConversationId || ''}
              onChange={(e) => {
                const id = e.target.value;
                const conv = conversations.find(c => c.id === id);
                if (conv) {
                  setCurrentConversationId(conv.id);
                  setChatMessages(conv.messages || []);
                }
              }}
              style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
              title="Load previous conversation"
            >
              <option value="">Load History...</option>
              {conversations.map(conv => (
                <option key={conv.id} value={conv.id}>
                  {conv.title || conv.id}
                </option>
              ))}
            </select>
          )}
        </div>
        {/* Chat mode selector */}
        <select
          value={chatMode}
          onChange={(e) => setChatMode(e.target.value as 'manual' | 'auto' | 'agent')}
          style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
        >
          <option value="manual">Manual</option>
          <option value="auto">Auto</option>
          <option value="agent">Agent</option>
        </select>
        <select
          value={selectedProvider}
          onChange={(e) => setSelectedProvider(e.target.value)}
          style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
        >
          <option value="">Select Provider</option>
          {providers.map(p => (
            <option key={p.id} value={p.id}>{p.display_name}</option>
          ))}
        </select>
        <select
          value={selectedModel}
          onChange={(e) => setSelectedModel(e.target.value)}
          style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
        >
          <option value="">Select Model</option>
          {availableModels.map(m => (
            <option key={m} value={m}>{m}</option>
          ))}
        </select>
        <label style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '11px', color: 'var(--text-secondary)' }}>
          <input
            type="checkbox"
            checked={useStreaming}
            onChange={(e) => setUseStreaming(e.target.checked)}
          />
          Streaming
        </label>
        {chatMode === 'auto' && (
          <>
            <select
              value={trainingProjectId}
              onChange={(e) => setTrainingProjectId(e.target.value)}
              style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
            >
              <option value="">Training Project</option>
              {trainingProjects.map(p => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            <select
              value={trainingLocalModelId}
              onChange={(e) => setTrainingLocalModelId(e.target.value)}
              style={{ padding: '4px 8px', fontSize: '12px', background: 'var(--bg-primary)', border: '1px solid var(--border-color)', color: 'var(--text-primary)' }}
            >
              <option value="">Any Local Model</option>
              {trainingLocalModels.map(m => (
                <option key={m.id} value={m.id}>{m.name}</option>
              ))}
            </select>
          </>
        )}
      </div>

      {/* Main content */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Left panel - File Explorer */}
        <div style={{
          width: leftPanelWidth,
          borderRight: '1px solid var(--border-color)',
          display: 'flex',
          flexDirection: 'column',
          background: 'var(--card-bg)',
        }}>
          <div style={{
            padding: '8px 12px',
            borderBottom: '1px solid var(--border-color)',
            fontSize: '11px',
            fontWeight: '600',
            textTransform: 'uppercase',
            color: 'var(--text-secondary)',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}>
            <span>Explorer</span>
            <div style={{ display: 'flex', gap: '4px' }}>
              <button
                onClick={async () => {
                  if (!selectedProvider || !selectedModel) {
                    alert('Please select a provider and model first');
                    return;
                  }
                  // Only scan if user has navigated to a specific folder
                  if (!currentPath) {
                    alert('Please navigate to a folder first, or right-click on a folder and select "Scan & Summarize"');
                    return;
                  }
                  await scanAndSummarizeFolder(currentPath);
                }}
                disabled={!selectedProvider || !selectedModel}
                style={{
                  background: selectedProvider && selectedModel ? 'none' : 'transparent',
                  border: 'none',
                  cursor: selectedProvider && selectedModel ? 'pointer' : 'default',
                  fontSize: '14px',
                  padding: '2px 4px',
                  color: selectedProvider && selectedModel ? 'var(--text-primary)' : 'var(--text-secondary)',
                  opacity: selectedProvider && selectedModel ? 1 : 0.5,
                }}
                title="Scan & Summarize Current Folder"
              >
                🔍
              </button>
              <button
                onClick={() => {
                  setCreatingFile({ parentPath: currentPath, isDir: true });
                  setNewFileName('');
                }}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: '14px',
                  padding: '2px 4px',
                  color: 'var(--text-primary)',
                }}
                title="New Folder"
              >
                📁
              </button>
              <button
                onClick={() => {
                  setCreatingFile({ parentPath: currentPath, isDir: false });
                  setNewFileName('');
                }}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: '14px',
                  padding: '2px 4px',
                  color: 'var(--text-primary)',
                }}
                title="New File"
              >
                📄
              </button>
            </div>
          </div>
          
          {/* Path display */}
          <div style={{
            padding: '6px 12px',
            borderBottom: '1px solid var(--border-color)',
            fontSize: '11px',
            color: 'var(--text-secondary)',
            background: 'var(--bg-secondary)',
            display: 'flex',
            alignItems: 'center',
            gap: '6px',
          }}>
            <span>📂</span>
            <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {displayPath}
            </span>
            {currentPath && (
              <button
                onClick={() => navigateToDirectory('..')}
                style={{
                  background: 'none',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: '12px',
                  padding: '2px 4px',
                  color: 'var(--text-primary)',
                }}
                title="Go up"
              >
                ⬆
              </button>
            )}
            <button
              onClick={() => setCurrentPath('')}
              style={{
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                fontSize: '12px',
                padding: '2px 4px',
                color: 'var(--text-primary)',
              }}
              title="Go to root"
            >
              🏠
            </button>
          </div>

          <div style={{ flex: 1, overflow: 'auto' }}>
            {files.map(entry => (
              <div key={entry.path}>
                {renamingFile === entry.path ? (
                  <div style={{ padding: '4px 8px', display: 'flex', gap: '4px', alignItems: 'center' }}>
                    <input
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter') submitRename();
                        if (e.key === 'Escape') { setRenamingFile(null); setRenameValue(''); }
                      }}
                      autoFocus
                      style={{
                        flex: 1,
                        padding: '2px 4px',
                        fontSize: '12px',
                        border: '1px solid var(--border-color)',
                        background: 'var(--bg-primary)',
                        color: 'var(--text-primary)',
                      }}
                    />
                    <button
                      onClick={submitRename}
                      style={{ fontSize: '12px', padding: '2px 6px' }}
                    >
                      ✓
                    </button>
                    <button
                      onClick={() => { setRenamingFile(null); setRenameValue(''); }}
                      style={{ fontSize: '12px', padding: '2px 6px' }}
                    >
                      ×
                    </button>
                  </div>
                ) : (
                  <div
                    className="file-entry"
                    style={{
                      padding: '4px 8px',
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '6px',
                      fontSize: '13px',
                      color: 'var(--text-primary)',
                      background: activeFile === entry.path ? 'var(--bg-secondary)' : 'transparent',
                    }}
                    onClick={() => openFile(entry)}
                    onContextMenu={(e) => handleContextMenu(e, entry)}
                    onDoubleClick={() => entry.is_dir && navigateToDirectory(entry.path)}
                  >
                    <span style={{ fontSize: '12px', width: '16px', textAlign: 'center' }}>
                      {entry.is_dir ? '📁' : '📄'}
                    </span>
                    <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {entry.name}
                    </span>
                  </div>
                )}
              </div>
            ))}
            {files.length === 0 && (
              <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)', fontSize: '12px' }}>
                Directory is empty.<br />
                Right-click to create files.
              </div>
            )}
          </div>
        </div>

        {/* Center - Editor area */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {/* Tabs */}
          <div style={{
            display: 'flex',
            borderBottom: '1px solid var(--border-color)',
            background: 'var(--bg-secondary)',
            overflowX: 'auto',
          }}>
            {openFiles.map(file => (
              <div
                key={file.path}
                onClick={() => setActiveFile(file.path)}
                style={{
                  padding: '8px 12px',
                  cursor: 'pointer',
                  borderRight: '1px solid var(--border-color)',
                  background: activeFile === file.path ? 'var(--card-bg)' : 'transparent',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '8px',
                  fontSize: '13px',
                  whiteSpace: 'nowrap',
                }}
              >
                <span style={{ color: file.modified ? '#4a90e2' : 'var(--text-secondary)' }}>
                  {file.modified ? '●' : ''} {file.name}
                </span>
                <button
                  onClick={(e) => { e.stopPropagation(); closeFile(file.path); }}
                  style={{
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    fontSize: '14px',
                    color: 'var(--text-secondary)',
                    padding: '0',
                  }}
                >
                  ×
                </button>
              </div>
            ))}
          </div>

          {/* Editor content */}
          <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
            {activeFileData ? (
              <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
                {/* Editor Toolbar */}
                <div style={{
                  padding: '4px 8px',
                  background: 'var(--bg-secondary)',
                  borderBottom: '1px solid var(--border-color)',
                  fontSize: '11px',
                  color: 'var(--text-secondary)',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  gap: '8px',
                }}>
                  <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {activeFileData.path}
                  </span>
                  <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                    <button
                      onClick={() => setShowGoToLine(true)}
                      style={{
                        background: 'transparent',
                        border: '1px solid var(--border-color)',
                        padding: '2px 6px',
                        fontSize: '11px',
                        cursor: 'pointer',
                        borderRadius: '3px',
                        color: 'var(--text-primary)',
                      }}
                      title="Go to Line (Ctrl+G)"
                    >
                      Go to Line
                    </button>
                    <button
                      onClick={() => setShowFindReplace(true)}
                      style={{
                        background: 'transparent',
                        border: '1px solid var(--border-color)',
                        padding: '2px 6px',
                        fontSize: '11px',
                        cursor: 'pointer',
                        borderRadius: '3px',
                        color: 'var(--text-primary)',
                      }}
                      title="Find & Replace (Ctrl+F)"
                    >
                      Find
                    </button>
                    <button
                      onClick={() => {
                        if (editorInstance) {
                          editorInstance.getAction('editor.action.formatDocument')?.run();
                        }
                      }}
                      style={{
                        background: 'transparent',
                        border: '1px solid var(--border-color)',
                        padding: '2px 6px',
                        fontSize: '11px',
                        cursor: 'pointer',
                        borderRadius: '3px',
                        color: 'var(--text-primary)',
                      }}
                      title="Format Document (Shift+Alt+F)"
                    >
                      Format
                    </button>
                    <button
                      onClick={() => saveFile(activeFileData.path)}
                      disabled={!activeFileData.modified}
                      style={{
                        background: activeFileData.modified ? '#4a90e2' : 'transparent',
                        color: activeFileData.modified ? 'white' : 'var(--text-secondary)',
                        border: '1px solid var(--border-color)',
                        padding: '2px 8px',
                        fontSize: '11px',
                        cursor: activeFileData.modified ? 'pointer' : 'default',
                        borderRadius: '3px',
                      }}
                    >
                      {activeFileData.modified ? 'Save (Ctrl+S)' : 'Saved'}
                    </button>
                  </div>
                </div>
                <Editor
                  height="100%"
                  language={activeFileData.language}
                  value={activeFileData.content}
                  onChange={(value) => updateFileContent(activeFileData.path, value)}
                  theme={useAppStore.getState().theme === 'dark' ? 'vs-dark' : 'light'}
                  options={{
                    minimap: { enabled: true },
                    fontSize: 13,
                    wordWrap: 'on',
                    automaticLayout: true,
                    tabSize: 2,
                    insertSpaces: true,
                    formatOnPaste: true,
                    formatOnType: true,
                    quickSuggestions: true,
                    suggestOnTriggerCharacters: true,
                    acceptSuggestionOnEnter: 'on',
                    tabCompletion: 'on',
                    wordBasedSuggestions: 'allDocuments',
                    parameterHints: { enabled: true },
                    hover: { enabled: true },
                    links: true,
                    colorDecorators: true,
                    bracketPairColorization: { enabled: true },
                    guides: {
                      bracketPairs: true,
                      indentation: true,
                    },
                  }}
                  onMount={(editor, monaco) => {
                    setEditorInstance(editor);
                    
                    // Add keyboard shortcuts
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
                      saveFile(activeFileData.path);
                    });
                    
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyF, () => {
                      setShowFindReplace(true);
                      setTimeout(() => {
                        editor.getAction('actions.find')?.run();
                      }, 100);
                    });
                    
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyG, () => {
                      setShowGoToLine(true);
                    });
                    
                    editor.addCommand(monaco.KeyMod.Shift | monaco.KeyMod.Alt | monaco.KeyCode.KeyF, () => {
                      editor.getAction('editor.action.formatDocument')?.run();
                    });
                    
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyH, () => {
                      setShowFindReplace(true);
                      setReplaceText('');
                      setTimeout(() => {
                        editor.getAction('actions.find')?.run();
                      }, 100);
                    });
                    
                    // Additional useful shortcuts
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyK | monaco.KeyCode.KeyS, () => {
                      editor.getAction('workbench.action.files.save')?.run();
                    });
                    
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyP, () => {
                      setShowFileSearch(true);
                    });
                    
                    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyB, () => {
                      editor.getAction('editor.action.toggleMinimap')?.run();
                    });
                  }}
                />
              </div>
            ) : (
              <div style={{
                height: '100%',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                color: 'var(--text-secondary)',
                fontSize: '14px',
              }}>
                Select a file to edit, or start chatting with the AI assistant
              </div>
            )}
          </div>

          {/* Terminal at bottom */}
          <div style={{
            height: '200px',
            borderTop: '1px solid var(--border-color)',
            display: 'flex',
            flexDirection: 'column',
            background: '#1e1e1e',
          }}>
            <div style={{
              padding: '6px 12px',
              borderBottom: '1px solid #333',
              fontSize: '11px',
              fontWeight: '600',
              color: '#d4d4d4',
              background: '#252526',
            }}>
              ⌨️ PowerShell Terminal
            </div>
            <div style={{
              flex: 1,
              overflow: 'auto',
              padding: '8px 12px',
              fontFamily: 'Consolas, Monaco, "Courier New", monospace',
              fontSize: '12px',
              color: '#d4d4d4',
            }}>
              {terminalLines.map((line, i) => (
                <div key={i} style={{
                  color: line.type === 'error' ? '#f48771' : line.type === 'input' ? '#569cd6' : '#d4d4d4',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-all',
                }}>
                  {line.content}
                </div>
              ))}
              <div ref={terminalEndRef} />
            </div>
            <div style={{
              display: 'flex',
              borderTop: '1px solid #333',
              background: '#1e1e1e',
            }}>
              <span style={{ padding: '8px', color: '#569cd6', fontFamily: 'monospace', fontSize: '12px' }}>
                PS {currentPath || workspacePath}{'>'}
              </span>
              <input
                value={terminalInput}
                onChange={(e) => setTerminalInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && executeTerminalCommand()}
                disabled={terminalRunning}
                placeholder="Enter PowerShell command..."
                style={{
                  flex: 1,
                  border: 'none',
                  outline: 'none',
                  background: 'transparent',
                  color: '#d4d4d4',
                  fontFamily: 'Consolas, Monaco, "Courier New", monospace',
                  fontSize: '12px',
                  padding: '8px 0',
                }}
              />
              <button
                onClick={executeTerminalCommand}
                disabled={terminalRunning || !terminalInput.trim()}
                style={{
                  padding: '4px 8px',
                  background: terminalRunning ? '#666' : '#4a90e2',
                  color: 'white',
                  border: 'none',
                  borderRadius: '3px',
                  cursor: terminalRunning || !terminalInput.trim() ? 'default' : 'pointer',
                  fontSize: '11px',
                  marginRight: '8px',
                }}
              >
                {terminalRunning ? 'Running...' : 'Run'}
              </button>
            </div>
          </div>
        </div>

        {/* Right panel - AI Chat & Workflows */}
        <div style={{
          width: rightPanelWidth,
          borderLeft: '1px solid var(--border-color)',
          display: 'flex',
          flexDirection: 'column',
          background: 'var(--card-bg)',
        }}>
          <div style={{
            padding: '8px 12px',
            borderBottom: '1px solid var(--border-color)',
            fontSize: '11px',
            fontWeight: '600',
            textTransform: 'uppercase',
            color: 'var(--text-secondary)',
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
          }}>
            <span>AI Agent</span>
            <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
              <button
                onClick={() => setShowExportModal(true)}
                disabled={conversations.length === 0}
                style={{
                  padding: '4px 8px',
                  fontSize: '11px',
                  background: conversations.length > 0 ? '#4a90e2' : 'transparent',
                  color: conversations.length > 0 ? 'white' : 'var(--text-secondary)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '3px',
                  cursor: conversations.length > 0 ? 'pointer' : 'default',
                  opacity: conversations.length > 0 ? 1 : 0.5,
                }}
                title="Export to Training Data"
              >
                Export
              </button>
              <button
                onClick={() => openWorkflowEditor()}
                style={{
                  padding: '4px 8px',
                  fontSize: '11px',
                  borderRadius: '3px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-primary)',
                  cursor: 'pointer',
                }}
              >
                New Workflow
              </button>
            </div>
          </div>
          <div style={{
            flex: 1,
            overflow: 'auto',
            padding: '12px',
          }}>
            {/* Workflows list */}
            <div style={{
              marginBottom: '12px',
              paddingBottom: '8px',
              borderBottom: '1px solid var(--border-color)',
            }}>
              <div style={{ fontSize: '12px', fontWeight: 600, marginBottom: '4px' }}>
                Workflows
              </div>
              {workflows.length === 0 ? (
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                  No workflows yet. Click \"New Workflow\" to create one.
                </div>
              ) : (
                <div style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '4px',
                  maxHeight: '160px',
                  overflowY: 'auto',
                }}>
                  {workflows.map(wf => (
                    <div
                      key={wf.id}
                      style={{
                        padding: '6px 8px',
                        borderRadius: '4px',
                        border: '1px solid var(--border-color)',
                        background: 'var(--bg-primary)',
                      }}
                    >
                      <div style={{
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        gap: '6px',
                      }}>
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{
                            fontSize: '12px',
                            fontWeight: 500,
                            overflow: 'hidden',
                            textOverflow: 'ellipsis',
                            whiteSpace: 'nowrap',
                          }}>
                            {wf.name}
                          </div>
                          {wf.description && (
                            <div style={{
                              fontSize: '11px',
                              color: 'var(--text-secondary)',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}>
                              {wf.description}
                            </div>
                          )}
                        </div>
                        <div style={{ display: 'flex', gap: '4px' }}>
                          <button
                            onClick={() => runWorkflow(wf)}
                            style={{
                              padding: '2px 6px',
                              fontSize: '11px',
                              borderRadius: '3px',
                              border: '1px solid var(--border-color)',
                              background: '#4a90e2',
                              color: 'white',
                              cursor: 'pointer',
                            }}
                          >
                            Run
                          </button>
                          <button
                            onClick={() => openWorkflowEditor(wf)}
                            style={{
                              padding: '2px 4px',
                              fontSize: '11px',
                              borderRadius: '3px',
                              border: '1px solid var(--border-color)',
                              background: 'var(--bg-primary)',
                              cursor: 'pointer',
                            }}
                          >
                            Edit
                          </button>
                          <button
                            onClick={() => deleteWorkflow(wf.id)}
                            style={{
                              padding: '2px 4px',
                              fontSize: '11px',
                              borderRadius: '3px',
                              border: '1px solid var(--border-color)',
                              background: 'var(--bg-primary)',
                              cursor: 'pointer',
                            }}
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Chat messages */}
            {chatMessages.length === 0 && (
              <div style={{ textAlign: 'center', color: 'var(--text-secondary)', fontSize: '13px', padding: '20px' }}>
                Chat with the AI to get help with coding, create files, or run commands.
              </div>
            )}
            {chatMessages.map(msg => (
              <div key={msg.id} style={{
                marginBottom: '12px',
                display: 'flex',
                flexDirection: 'column',
                alignItems: msg.role === 'user' ? 'flex-end' : 'flex-start',
              }}>
                <div style={{
                  maxWidth: '90%',
                  padding: '8px 12px',
                  borderRadius: '8px',
                  background: msg.role === 'user' ? '#4a90e2' : 'var(--bg-secondary)',
                  color: msg.role === 'user' ? 'white' : 'var(--text-primary)',
                  fontSize: '13px',
                  whiteSpace: 'pre-wrap',
                  wordBreak: 'break-word',
                }}>
                  {msg.content}
                </div>
                {msg.role === 'assistant' && msg.content && (
                  <div style={{ marginTop: '4px', alignSelf: 'flex-start' }}>
                    <VoiceOutput text={msg.content} />
                  </div>
                )}
              </div>
            ))}

            {/* Agent Mode summary, proposed changes, or tool approvals */}
            {(agentSummary || agentError || agentChanges || pendingTools.length > 0) && (
              <div style={{
                marginTop: '16px',
                padding: '10px 12px',
                borderTop: '1px solid var(--border-color)',
                fontSize: '12px',
              }}>
                <div style={{ marginBottom: '6px', fontWeight: 600 }}>Agent Mode</div>
                {agentSummary && (
                  <div style={{
                    marginBottom: '8px',
                    color: 'var(--text-primary)',
                    whiteSpace: 'pre-wrap',
                  }}>
                    {agentSummary}
                  </div>
                )}
                {agentError && (
                  <div style={{
                    marginBottom: '8px',
                    color: '#c62828',
                    whiteSpace: 'pre-wrap',
                  }}>
                    {agentError}
                  </div>
                )}
                {agentChanges && agentChanges.items.length > 0 && (
                  <div>
                    <div style={{ marginBottom: '4px', fontWeight: 500 }}>
                      Proposed changes ({agentChanges.items.length}):
                    </div>
                    <ul style={{ margin: 0, paddingLeft: '16px' }}>
                      {agentChanges.items.map((c, idx) => (
                        <li key={`${c.file_path}-${idx}`} style={{ marginBottom: '4px' }}>
                          <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                            <span style={{ fontFamily: 'monospace' }}>{c.file_path}</span>
                            {c.description && (
                              <span style={{ color: 'var(--text-secondary)', fontSize: '11px' }}>
                                – {c.description}
                              </span>
                            )}
                            <div style={{ display: 'flex', gap: '4px' }}>
                              <button
                                onClick={() => openDiffForChange(c)}
                                style={{
                                  padding: '2px 6px',
                                  fontSize: '11px',
                                  borderRadius: '3px',
                                  border: '1px solid var(--border-color)',
                                  background: 'var(--bg-primary)',
                                  cursor: 'pointer',
                                }}
                              >
                                Diff
                              </button>
                              <button
                                onClick={() => applyAgentChange(c)}
                                style={{
                                  padding: '2px 6px',
                                  fontSize: '11px',
                                  borderRadius: '3px',
                                  border: '1px solid var(--border-color)',
                                  background: '#4a90e2',
                                  color: 'white',
                                  cursor: 'pointer',
                                }}
                              >
                                Apply
                              </button>
                            </div>
                          </div>
                        </li>
                      ))}
                    </ul>
                    <div style={{ marginTop: '6px', display: 'flex', gap: '8px', alignItems: 'center' }}>
                      <button
                        onClick={applyAllAgentChanges}
                        style={{
                          padding: '4px 10px',
                          fontSize: '11px',
                          borderRadius: '3px',
                          border: '1px solid var(--border-color)',
                          background: '#4a90e2',
                          color: 'white',
                          cursor: 'pointer',
                        }}
                      >
                        Apply all changes
                      </button>
                      <span style={{ color: 'var(--text-secondary)', fontSize: '11px' }}>
                        Applies new content directly to the workspace files.
                      </span>
                    </div>
                  </div>
                )}
                {/* Cline (Approve mode) pending tools */}
                {agentStyle === 'approve' && pendingTools.length > 0 && (
                  <div style={{ marginTop: '12px' }}>
                    <div style={{ marginBottom: '6px', fontWeight: 500 }}>
                      Tools pending approval ({pendingTools.filter(t => String(t.approval_status).toLowerCase() === 'pending').length}):
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                      {pendingTools.map(tool => {
                        const isPending = String(tool.approval_status).toLowerCase() === 'pending';
                        const desc = tool.tool_type === 'workspace_write' ? `Write: ${tool.tool_params?.path || '?'}` :
                          tool.tool_type === 'terminal' ? `Run: ${tool.tool_params?.command || '?'}` :
                          tool.tool_type === 'workspace_read' ? `Read: ${tool.tool_params?.path || '?'}` :
                          `${tool.tool_type}: ${JSON.stringify(tool.tool_params || {}).slice(0, 40)}...`;
                        return (
                          <div key={tool.id} style={{
                            padding: '6px 8px',
                            border: isPending ? '2px solid #4a90e2' : '1px solid var(--border-color)',
                            borderRadius: '4px',
                            fontSize: '11px',
                            background: 'var(--bg-primary)',
                          }}>
                            <div style={{ fontWeight: 500, marginBottom: '4px' }}>{desc}</div>
                            {isPending && (
                              <div style={{ display: 'flex', gap: '6px' }}>
                                <button onClick={() => approveTool(tool.id, true)} disabled={toolBusyIds.includes(tool.id)}
                                  style={{ padding: '4px 8px', fontSize: '10px', background: '#28a745', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer' }}>
                                  {toolBusyIds.includes(tool.id) ? '...' : 'Approve'}
                                </button>
                                <button onClick={() => approveTool(tool.id, false)} disabled={toolBusyIds.includes(tool.id)}
                                  style={{ padding: '4px 8px', fontSize: '10px', background: '#dc3545', color: 'white', border: 'none', borderRadius: '3px', cursor: 'pointer' }}>
                                  Reject
                                </button>
                              </div>
                            )}
                          </div>
                        );
                      })}
                    </div>
                    {clineRunId && (
                      <div style={{ display: 'flex', gap: '6px', marginTop: '8px', alignItems: 'center' }}>
                        <input type="text" value={agentResponseInput} onChange={(e) => setAgentResponseInput(e.target.value)}
                          placeholder="Respond to agent..." style={{ flex: 1, padding: '4px 8px', fontSize: '11px', border: '1px solid var(--border-color)', borderRadius: '4px', background: 'var(--bg-primary)', color: 'var(--text-primary)' }}
                          onKeyDown={e => e.key === 'Enter' && handleClineContinue()} />
                        <button onClick={handleClineContinue} disabled={agentRunning || !agentResponseInput.trim()}
                          style={{ padding: '4px 10px', fontSize: '11px', background: '#4a90e2', color: 'white', border: 'none', borderRadius: '4px', cursor: 'pointer' }}>
                          Send
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
            {chatLoading && (
              <div style={{ color: 'var(--text-secondary)', fontSize: '12px', padding: '8px' }}>
                Thinking...
              </div>
            )}
            <div ref={chatEndRef} />
          </div>
          <div style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '8px',
            padding: '8px 12px',
            borderTop: '1px solid var(--border-color)',
          }}>
            {/* Agent Task panel */}
            <div style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '4px',
              marginBottom: '6px',
            }}>
              <div style={{ fontSize: '12px', fontWeight: 600 }}>Agent Task</div>
              <textarea
                value={agentTask}
                onChange={(e) => setAgentTask(e.target.value)}
                placeholder="Describe a concrete coding task (e.g., 'Add unit tests for auth service')."
                rows={2}
                style={{
                  padding: '6px 10px',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  fontSize: '12px',
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                  resize: 'none',
                  fontFamily: 'inherit',
                }}
              />
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                <select
                  value={agentStyle}
                  onChange={(e) => setAgentStyle(e.target.value as 'propose' | 'approve')}
                  title={agentStyle === 'propose' ? 'Propose: AI suggests changes, you apply' : 'Approve: AI requests tools, you approve each'}
                  style={{
                    padding: '4px 8px',
                    fontSize: '11px',
                    borderRadius: '4px',
                    border: '1px solid var(--border-color)',
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                  }}
                >
                  <option value="propose">Propose</option>
                  <option value="approve">Approve</option>
                </select>
                {agentStyle === 'propose' && (
                  <select
                    value={agentScope}
                    onChange={(e) => setAgentScope(e.target.value as 'file' | 'folder' | 'workspace')}
                  style={{
                    padding: '4px 8px',
                    fontSize: '11px',
                    borderRadius: '4px',
                    border: '1px solid var(--border-color)',
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                  }}
                >
                  <option value="file">Current file</option>
                  <option value="folder">Current folder</option>
                  <option value="workspace">Whole workspace</option>
                </select>
                )}
                <button
                  onClick={handleAgentRun}
                  disabled={agentRunning || !agentTask.trim() || !selectedProvider || !selectedModel}
                  style={{
                    padding: '6px 12px',
                    background: agentRunning ? '#6c757d' : '#4a90e2',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: agentRunning ? 'default' : 'pointer',
                    fontSize: '12px',
                  }}
                >
                  {agentRunning ? 'Running agent...' : 'Run agent task'}
                </button>
              </div>
            </div>

            {/* Quick templates for common coding tasks */}
            <div style={{ display: 'flex', gap: '8px', marginBottom: '6px', flexWrap: 'wrap' }}>
              <button
                type="button"
                onClick={() => {
                  if (activeFile) {
                    setChatInput(`Explain the purpose and structure of the current file: ${activeFile}`);
                  } else {
                    setChatInput('Explain the current project structure and key files.');
                  }
                }}
                style={{
                  padding: '4px 8px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                  fontSize: '11px',
                  cursor: 'pointer',
                }}
              >
                Explain file
              </button>
              <button
                type="button"
                onClick={() => {
                  if (activeFile) {
                    setChatInput(`Write focused, realistic unit tests for the main functions in ${activeFile}.`);
                  } else {
                    setChatInput('Identify critical modules and propose a unit-test plan for this project.');
                  }
                }}
                style={{
                  padding: '4px 8px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                  fontSize: '11px',
                  cursor: 'pointer',
                }}
              >
                Write tests
              </button>
              <button
                type="button"
                onClick={() => {
                  setChatInput('Look at the latest terminal errors above and help me fix them step-by-step.');
                }}
                style={{
                  padding: '4px 8px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  background: 'var(--bg-secondary)',
                  color: 'var(--text-primary)',
                  fontSize: '11px',
                  cursor: 'pointer',
                }}
              >
                Fix errors
              </button>
            </div>

            {/* Regular chat input - hidden in agent mode */}
            {chatMode !== 'agent' && (
              <>
                <textarea
                  value={chatInput}
                  onChange={(e) => setChatInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleChatSend()}
                  placeholder="Ask the AI for help..."
                  disabled={chatLoading}
                  rows={3}
                  style={{
                    padding: '8px 12px',
                    border: '1px solid var(--border-color)',
                    borderRadius: '4px',
                    fontSize: '13px',
                    background: 'var(--bg-primary)',
                    color: 'var(--text-primary)',
                    resize: 'none',
                    fontFamily: 'inherit',
                  }}
                />
                <VoiceInput value={chatInput} onTranscript={setChatInput} disabled={chatLoading} />
                <button
                  onClick={handleChatSend}
                  disabled={chatLoading || !chatInput.trim()}
                  style={{
                    padding: '8px 16px',
                    background: '#4a90e2',
                    color: 'white',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '13px',
                  }}
                >
                  Send
                </button>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <>
          <div
            style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, zIndex: 999 }}
            onClick={() => setContextMenu(null)}
          />
          <div style={{
            position: 'fixed',
            top: contextMenu.y,
            left: contextMenu.x,
            background: 'var(--card-bg)',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            boxShadow: '0 2px 10px rgba(0,0,0,0.2)',
            zIndex: 1000,
            minWidth: '150px',
          }}>
            <div
              onClick={() => handleCreateFile(false)}
              style={{ padding: '8px 12px', cursor: 'pointer', fontSize: '13px' }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-secondary)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
            >
              📄 New File
            </div>
            <div
              onClick={() => handleCreateFile(true)}
              style={{ padding: '8px 12px', cursor: 'pointer', fontSize: '13px' }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-secondary)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
            >
              📁 New Folder
            </div>
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            {contextMenu.isDir && (
              <div
                onClick={async () => {
                  if (!selectedProvider || !selectedModel) {
                    alert('Please select a provider and model first');
                    setContextMenu(null);
                    return;
                  }
                  setContextMenu(null);
                  await scanAndSummarizeFolder(contextMenu.path);
                }}
                style={{ padding: '8px 12px', cursor: 'pointer', fontSize: '13px', color: '#4a90e2' }}
                onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-secondary)'}
                onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
              >
                🔍 Scan & Summarize
              </div>
            )}
            <div
              onClick={handleRename}
              style={{ padding: '8px 12px', cursor: 'pointer', fontSize: '13px' }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-secondary)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
            >
              ✏️ Rename
            </div>
            <div
              onClick={handleDelete}
              style={{ padding: '8px 12px', cursor: 'pointer', fontSize: '13px', color: '#dc3545' }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--bg-secondary)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}
            >
              🗑️ Delete
            </div>
          </div>
        </>
      )}

      {/* Find/Replace Dialog */}
      {showFindReplace && activeFileData && editorInstance && (
        <div style={{
          position: 'fixed',
          top: '100px',
          left: '50%',
          transform: 'translateX(-50%)',
          background: 'var(--card-bg)',
          border: '1px solid var(--border-color)',
          borderRadius: '4px',
          padding: '12px',
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
          zIndex: 2000,
          minWidth: '300px',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <strong style={{ fontSize: '13px' }}>Find & Replace</strong>
            <button
              onClick={() => {
                setShowFindReplace(false);
                setFindText('');
                setReplaceText('');
              }}
              style={{
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                fontSize: '18px',
                color: 'var(--text-secondary)',
              }}
            >
              ×
            </button>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <div>
              <label style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Find:</label>
              <input
                value={findText}
                onChange={(e) => {
                  setFindText(e.target.value);
                  if (editorInstance) {
                    // Trigger Monaco's built-in find
                    editorInstance.getAction('actions.find')?.run();
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    if (editorInstance) {
                      editorInstance.getAction('editor.action.nextMatchFindAction')?.run();
                    }
                  }
                  if (e.key === 'Escape') {
                    setShowFindReplace(false);
                    editorInstance?.focus();
                  }
                }}
                autoFocus
                style={{
                  width: '100%',
                  padding: '6px',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                }}
              />
            </div>
            <div>
              <label style={{ fontSize: '12px', display: 'block', marginBottom: '4px' }}>Replace:</label>
              <input
                value={replaceText}
                onChange={(e) => setReplaceText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && e.shiftKey) {
                    e.preventDefault();
                    if (editorInstance && findText) {
                      editorInstance.getAction('editor.action.replaceOne')?.run();
                    }
                  }
                  if (e.key === 'Escape') {
                    setShowFindReplace(false);
                  }
                }}
                style={{
                  width: '100%',
                  padding: '6px',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                }}
              />
            </div>
            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button
                onClick={() => {
                  if (editorInstance && findText && replaceText) {
                    // Set find and replace values in Monaco
                    const findController = editorInstance.getContribution('editor.contrib.findController');
                    if (findController) {
                      findController.start({
                        searchString: findText,
                        replaceString: replaceText,
                        isRegex: false,
                        matchCase: false,
                        wholeWord: false,
                      });
                      editorInstance.getAction('editor.action.replaceOne')?.run();
                    }
                  }
                }}
                disabled={!findText || !replaceText}
                style={{
                  padding: '4px 12px',
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: findText && replaceText ? 'pointer' : 'default',
                  fontSize: '12px',
                  opacity: findText && replaceText ? 1 : 0.5,
                }}
              >
                Replace
              </button>
              <button
                onClick={() => {
                  if (editorInstance && findText && replaceText) {
                    const findController = editorInstance.getContribution('editor.contrib.findController');
                    if (findController) {
                      findController.start({
                        searchString: findText,
                        replaceString: replaceText,
                        isRegex: false,
                        matchCase: false,
                        wholeWord: false,
                      });
                      editorInstance.getAction('editor.action.replaceAll')?.run();
                    }
                  }
                }}
                disabled={!findText || !replaceText}
                style={{
                  padding: '4px 12px',
                  background: '#4a90e2',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: findText && replaceText ? 'pointer' : 'default',
                  fontSize: '12px',
                  opacity: findText && replaceText ? 1 : 0.5,
                }}
              >
                Replace All
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Agent diff modal */}
      {diffModal && (
        <div
          style={{
            position: 'fixed',
            top: 0,
            left: 0,
            right: 0,
            bottom: 0,
            background: 'rgba(0,0,0,0.4)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 2500,
          }}
        >
          <div
            style={{
              background: 'var(--card-bg)',
              borderRadius: '4px',
              boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
              width: '80vw',
              height: '70vh',
              maxWidth: '1100px',
              display: 'flex',
              flexDirection: 'column',
            }}
          >
            <div
              style={{
                padding: '8px 12px',
                borderBottom: '1px solid var(--border-color)',
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                fontSize: '13px',
              }}
            >
              <span>Agent diff: {diffModal.filePath}</span>
              <button
                onClick={() => setDiffModal(null)}
                style={{
                  background: 'transparent',
                  border: 'none',
                  cursor: 'pointer',
                  fontSize: '16px',
                  color: 'var(--text-secondary)',
                }}
              >
                ×
              </button>
            </div>
            <div style={{ flex: 1 }}>
              <DiffEditor
                original={diffModal.original}
                modified={diffModal.modified}
                language={diffModal.language || undefined}
                options={{
                  readOnly: true,
                  renderSideBySide: true,
                  minimap: { enabled: false },
                }}
                theme="vs-dark"
              />
            </div>
          </div>
        </div>
      )}

      {/* File Search Dialog */}
      {showFileSearch && (
        <div style={{
          position: 'fixed',
          top: '100px',
          left: '50%',
          transform: 'translateX(-50%)',
          background: 'var(--card-bg)',
          border: '1px solid var(--border-color)',
          borderRadius: '4px',
          padding: '12px',
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
          zIndex: 2000,
          minWidth: '400px',
          maxWidth: '600px',
          maxHeight: '500px',
          display: 'flex',
          flexDirection: 'column',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <strong style={{ fontSize: '13px' }}>Quick File Search (Ctrl+P)</strong>
            <button
              onClick={() => {
                setShowFileSearch(false);
                setFileSearchQuery('');
                setFileSearchResults([]);
              }}
              style={{
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                fontSize: '18px',
                color: 'var(--text-secondary)',
              }}
            >
              ×
            </button>
          </div>
          <input
            value={fileSearchQuery}
            onChange={(e) => setFileSearchQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                setShowFileSearch(false);
                setFileSearchQuery('');
                setFileSearchResults([]);
              }
              if (e.key === 'Enter' && fileSearchResults.length > 0) {
                openFile(fileSearchResults[0]);
                setShowFileSearch(false);
                setFileSearchQuery('');
                setFileSearchResults([]);
              }
            }}
            placeholder="Type to search files..."
            autoFocus
            style={{
              width: '100%',
              padding: '8px',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              background: 'var(--bg-primary)',
              color: 'var(--text-primary)',
              marginBottom: '10px',
            }}
          />
          <div style={{
            maxHeight: '300px',
            overflow: 'auto',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            background: 'var(--bg-primary)',
          }}>
            {fileSearchResults.length > 0 ? (
              fileSearchResults.map((entry, idx) => (
                <div
                  key={entry.path}
                  onClick={() => {
                    openFile(entry);
                    setShowFileSearch(false);
                    setFileSearchQuery('');
                    setFileSearchResults([]);
                  }}
                  style={{
                    padding: '8px 12px',
                    cursor: 'pointer',
                    borderBottom: idx < fileSearchResults.length - 1 ? '1px solid var(--border-color)' : 'none',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '8px',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'var(--bg-secondary)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  <span>{entry.is_dir ? '📁' : '📄'}</span>
                  <span style={{ flex: 1 }}>{entry.name}</span>
                  <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{entry.path}</span>
                </div>
              ))
            ) : fileSearchQuery ? (
              <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)' }}>
                No files found
              </div>
            ) : (
              <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)' }}>
                Start typing to search files...
              </div>
            )}
          </div>
        </div>
      )}

      {/* Go to Line Dialog */}
      {showGoToLine && activeFileData && (
        <div style={{
          position: 'fixed',
          top: '100px',
          left: '50%',
          transform: 'translateX(-50%)',
          background: 'var(--card-bg)',
          border: '1px solid var(--border-color)',
          borderRadius: '4px',
          padding: '12px',
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
          zIndex: 2000,
          minWidth: '250px',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '10px' }}>
            <strong style={{ fontSize: '13px' }}>Go to Line</strong>
            <button
              onClick={() => {
                setShowGoToLine(false);
                setGoToLineNumber('');
              }}
              style={{
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                fontSize: '18px',
                color: 'var(--text-secondary)',
              }}
            >
              ×
            </button>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            <input
              type="number"
              value={goToLineNumber}
              onChange={(e) => setGoToLineNumber(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  if (editorInstance && goToLineNumber) {
                    const line = parseInt(goToLineNumber);
                    if (!isNaN(line) && line > 0) {
                      editorInstance.revealLineInCenter(line);
                      editorInstance.setPosition({ lineNumber: line, column: 1 });
                      editorInstance.focus();
                      setShowGoToLine(false);
                      setGoToLineNumber('');
                    }
                  }
                }
                if (e.key === 'Escape') {
                  setShowGoToLine(false);
                  setGoToLineNumber('');
                }
              }}
              placeholder="Line number"
              autoFocus
              style={{
                width: '100%',
                padding: '6px',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
              }}
            />
            <button
              onClick={() => {
                if (editorInstance && goToLineNumber) {
                  const line = parseInt(goToLineNumber);
                  if (!isNaN(line) && line > 0) {
                    editorInstance.revealLineInCenter(line);
                    editorInstance.setPosition({ lineNumber: line, column: 1 });
                    editorInstance.focus();
                    setShowGoToLine(false);
                    setGoToLineNumber('');
                  }
                }
              }}
              style={{
                padding: '6px 12px',
                background: '#4a90e2',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '12px',
              }}
            >
              Go
            </button>
          </div>
        </div>
      )}

      {/* Create File Dialog */}
      {creatingFile && (
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
          zIndex: 2000,
        }}>
          <div style={{
            background: 'var(--card-bg)',
            padding: '20px',
            borderRadius: '8px',
            minWidth: '300px',
          }}>
            <h3 style={{ marginTop: 0 }}>
              {creatingFile.isDir ? 'New Folder' : 'New File'}
            </h3>
            <input
              value={newFileName}
              onChange={(e) => setNewFileName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && submitCreateFile()}
              placeholder={creatingFile.isDir ? 'folder-name' : 'filename.ext'}
              autoFocus
              style={{
                width: '100%',
                padding: '8px',
                marginBottom: '15px',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
              }}
            />
            <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
              <button
                onClick={() => setCreatingFile(null)}
                style={{
                  padding: '6px 16px',
                  background: 'transparent',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: 'pointer',
                }}
              >
                Cancel
              </button>
              <button
                onClick={submitCreateFile}
                disabled={!newFileName.trim()}
                style={{
                  padding: '6px 16px',
                  background: '#4a90e2',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  cursor: 'pointer',
                }}
              >
                Create
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Export to Training Modal */}
      {showExportModal && (
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
          zIndex: 3000,
        }} onClick={() => setShowExportModal(false)}>
          <div style={{
            background: 'var(--card-bg)',
            border: '1px solid var(--border-color)',
            borderRadius: '8px',
            padding: '20px',
            maxWidth: '600px',
            width: '90%',
            maxHeight: '80vh',
            overflow: 'auto',
          }} onClick={(e) => e.stopPropagation()}>
            <h2 style={{ marginTop: 0, marginBottom: '16px' }}>Export to Training Data</h2>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '16px' }}>
              Export your Simple Coder conversations to train a local model. Select which conversations to include.
            </p>
            
            <div style={{ marginBottom: '16px' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
                <input
                  type="checkbox"
                  checked={includeContext}
                  onChange={(e) => setIncludeContext(e.target.checked)}
                />
                <span style={{ fontSize: '13px' }}>Include file context and terminal output</span>
              </label>
            </div>

            <div style={{ marginBottom: '16px' }}>
              <strong style={{ fontSize: '13px', display: 'block', marginBottom: '8px' }}>Select Conversations:</strong>
              <div style={{ maxHeight: '200px', overflow: 'auto', border: '1px solid var(--border-color)', borderRadius: '4px', padding: '8px' }}>
                {conversations.length === 0 ? (
                  <div style={{ padding: '20px', textAlign: 'center', color: 'var(--text-secondary)' }}>
                    No conversations found. Start chatting to create conversations.
                  </div>
                ) : (
                  conversations.map(conv => (
                    <label key={conv.id} style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '8px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={selectedConversations.has(conv.id)}
                        onChange={(e) => {
                          const newSet = new Set(selectedConversations);
                          if (e.target.checked) {
                            newSet.add(conv.id);
                          } else {
                            newSet.delete(conv.id);
                          }
                          setSelectedConversations(newSet);
                        }}
                      />
                      <div style={{ flex: 1 }}>
                        <div style={{ fontSize: '13px', fontWeight: '500' }}>{conv.title}</div>
                        <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
                          {conv.messages.length} messages • {new Date(conv.updated_at).toLocaleDateString()}
                        </div>
                      </div>
                    </label>
                  ))
                )}
              </div>
            </div>

            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button
                onClick={() => {
                  setShowExportModal(false);
                  setSelectedConversations(new Set());
                  setSelectedMessages(new Set());
                }}
                style={{
                  padding: '8px 16px',
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                Cancel
              </button>
              <button
                onClick={async () => {
                  if (selectedConversations.size === 0) {
                    alert('Please select at least one conversation');
                    return;
                  }

                  // Get project ID - for now, prompt user or use first project
                  const projectId = prompt('Enter Project ID for training data:');
                  if (!projectId) return;

                  const localModelId = prompt('Enter Local Model ID (optional, press Cancel to skip):') || undefined;

                  try {
                    const count = await api.exportCoderIDEConversationsToTraining({
                      conversation_ids: Array.from(selectedConversations),
                      message_ids: selectedMessages.size > 0 ? Array.from(selectedMessages) : undefined,
                      project_id: projectId,
                      local_model_id: localModelId,
                      include_context: includeContext,
                    });

                    alert(`Successfully exported ${count} training examples!`);
                    setShowExportModal(false);
                    setSelectedConversations(new Set());
                    setSelectedMessages(new Set());
                  } catch (error: any) {
                    alert(`Export failed: ${error.message || error}`);
                  }
                }}
                disabled={selectedConversations.size === 0}
                style={{
                  padding: '8px 16px',
                  background: selectedConversations.size > 0 ? '#4a90e2' : 'transparent',
                  color: selectedConversations.size > 0 ? 'white' : 'var(--text-secondary)',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: selectedConversations.size > 0 ? 'pointer' : 'default',
                  fontSize: '13px',
                  opacity: selectedConversations.size > 0 ? 1 : 0.5,
                }}
              >
                Export to Training
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
