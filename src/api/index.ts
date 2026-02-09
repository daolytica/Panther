// API layer for Tauri commands

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '../utils/tauri';

// Check if running in Tauri
const TAURI_AVAILABLE = isTauri();

// HTTP API base URL for browser mode (when Tauri backend runs HTTP server)
// Try to detect which port the server is running on
const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:3001';

// For browser mode, we'll try to find an available server
let resolvedApiBase: string = API_BASE;
let apiBaseResolved = false;

/** Set when we couldn't reach any backend port (browser mode only) */
export let backendUnreachable = false;

async function getApiBase(): Promise<string> {
  if (apiBaseResolved) return resolvedApiBase;
  if (import.meta.env.VITE_API_URL) {
    resolvedApiBase = import.meta.env.VITE_API_URL;
    apiBaseResolved = true;
    return resolvedApiBase;
  }
  // When opened from mobile/remote (e.g. http://100.97.211.10:1420), use same hostname for API
  const hostname = typeof window !== 'undefined' ? window.location.hostname : 'localhost';
  const isRemote = hostname !== 'localhost' && hostname !== '127.0.0.1';
  const apiHost = isRemote ? hostname : 'localhost';
  const ports = [3001, 3002, 3003, 3004, 3005];
  for (const port of ports) {
    try {
      const url = `http://${apiHost}:${port}/api/health`;
      const res = await fetch(url, { method: 'GET' });
      if (res.ok) {
        resolvedApiBase = `http://${apiHost}:${port}`;
        apiBaseResolved = true;
        backendUnreachable = false;
        console.log(`[API] Connected to server at ${resolvedApiBase}`);
        return resolvedApiBase;
      }
    } catch {
      // Port not available, try next
    }
  }
  resolvedApiBase = `http://${apiHost}:3001`;
  backendUnreachable = true;
  apiBaseResolved = true;
  return resolvedApiBase;
}

/** Check if HTTP backend is reachable (for browser mode). Call once on load. */
export async function checkBackendReachable(): Promise<boolean> {
  await getApiBase();
  return !backendUnreachable;
}

// Helper to show browser mode warning for operations that have no HTTP fallback
function showBrowserModeWarning(operation: string) {
  if (!TAURI_AVAILABLE && typeof window !== 'undefined') {
    console.warn(`⚠️ Browser Mode: ${operation} requires Tauri backend.`);
    console.warn('💡 To use full functionality, run: npm run tauri dev');
    alert(`⚠️ Browser Mode\n\n${operation} requires the Tauri backend.\n\nTo use full functionality, please run:\n\nnpm run tauri dev\n\nThis will open the app in a native window with full backend support.`);
  }
}

// Get current user id for user-scoped data (from localStorage after login)
function getCurrentUserId(): string | null {
  if (typeof window === 'undefined') return null;
  return localStorage.getItem('userId');
}

// HTTP fetch helper for browser mode - calls REST API when Tauri unavailable
async function httpFetch<T>(
  method: string,
  path: string,
  body?: unknown,
  headers?: Record<string, string>
): Promise<T> {
  const base = await getApiBase();
  const h: Record<string, string> = { 'Content-Type': 'application/json', ...headers };
  const userId = getCurrentUserId();
  if (userId) h['X-User-Id'] = userId;
  const opts: RequestInit = { method, headers: h };
  if (body !== undefined && method !== 'GET') {
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(`${base}${path}`, opts);
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error((err as { error?: string }).error || `HTTP ${res.status}`);
  }
  if (res.status === 204 || res.headers.get('content-length') === '0') return undefined as T;
  const text = await res.text();
  return (text ? JSON.parse(text) : undefined) as T;
}

export interface CreateProviderRequest {
  provider_type: string;
  display_name: string;
  base_url?: string;
  region?: string;
  api_key?: string;
  provider_metadata_json?: Record<string, unknown>;
}

export const api = {
  // Providers (user-scoped when logged in)
  async createProvider(request: CreateProviderRequest): Promise<string> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('create_provider', { request, userId });
    const r = await httpFetch<{ id: string }>('POST', '/api/providers', { ...request, user_id: userId });
    return r.id;
  },

  async listProviders(): Promise<any[]> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('list_providers', { userId });
    const q = userId ? `?user_id=${encodeURIComponent(userId)}` : '';
    return httpFetch<any[]>('GET', `/api/providers${q}`);
  },

  async updateProvider(id: string, request: CreateProviderRequest): Promise<void> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('update_provider', { id, request, userId });
    await httpFetch<void>('PUT', `/api/providers/${id}`, { ...request, user_id: userId });
  },

  async deleteProvider(id: string): Promise<void> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('delete_provider', { id, userId });
    await httpFetch<void>('DELETE', `/api/providers/${id}`);
  },

  async testProviderConnection(providerId: string): Promise<boolean> {
    if (TAURI_AVAILABLE) return invoke('test_provider_connection', { providerId });
    const r = await httpFetch<{ success: boolean }>('POST', `/api/providers/${providerId}/test`);
    return r.success;
  },

  async listProviderModels(providerId: string): Promise<string[]> {
    if (TAURI_AVAILABLE) return invoke('list_provider_models', { providerId });
    return httpFetch<string[]>('GET', `/api/providers/${providerId}/models`);
  },

  // Keychain
  async storeApiKey(service: string, username: string, password: string): Promise<void> {
    return invoke('store_api_key', { service, username, password });
  },

  async retrieveApiKey(service: string, username: string): Promise<string> {
    return invoke('retrieve_api_key', { service, username });
  },

  async deleteApiKey(service: string, username: string): Promise<void> {
    return invoke('delete_api_key', { service, username });
  },

  // Profiles (user-scoped when logged in)
  async createProfile(request: {
    name: string;
    provider_account_id: string;
    model_name: string;
    persona_prompt: string;
    character_definition_json?: any;
    model_features_json?: any;
    params_json: any;
    photo_url?: string;
    voice_gender?: string;
    voice_uri?: string;
  }): Promise<string> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('create_profile', { request, userId });
    const r = await httpFetch<{ id: string }>('POST', '/api/profiles', { ...request, user_id: userId });
    return r.id;
  },

  async updateProfile(id: string, request: {
    name: string;
    provider_account_id: string;
    model_name: string;
    persona_prompt: string;
    character_definition_json?: any;
    model_features_json?: any;
    params_json: any;
    photo_url?: string;
    voice_gender?: string;
    voice_uri?: string;
  }): Promise<void> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('update_profile', { id, request, userId });
    await httpFetch<void>('PUT', `/api/profiles/${id}`, { ...request, user_id: userId });
  },

  async listProfiles(): Promise<any[]> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('list_profiles', { userId });
    const q = userId ? `?user_id=${encodeURIComponent(userId)}` : '';
    return httpFetch<any[]>('GET', `/api/profiles${q}`);
  },

  async generateCharacterFromUrl(urls: string[], personName: string | undefined, providerAccountId: string, modelName: string, signal?: AbortSignal, cancellationToken?: string): Promise<{ character: any; extracted_text: string }> {
    // Note: Tauri doesn't natively support AbortSignal, so we'll handle cancellation on the frontend
    // The backend will continue processing, but we can stop waiting for the response
    if (signal?.aborted) {
      throw new Error('Request cancelled');
    }
    return invoke('generate_character_from_url', { request: { urls, person_name: personName, provider_account_id: providerAccountId, model_name: modelName, cancellation_token: cancellationToken } });
  },

  async getLatestProfile(): Promise<any | null> {
    if (TAURI_AVAILABLE) return invoke('get_latest_profile');
    // In browser mode, get latest from the list
    const profiles = await this.listProfiles();
    if (profiles.length === 0) return null;
    // Sort by created_at descending and return the first
    profiles.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime());
    return profiles[0];
  },

  async cancelCharacterGeneration(token: string): Promise<void> {
    return invoke('cancel_character_generation', { token });
  },

  async loadChatMessages(profileId: string, conversationId?: string): Promise<any[]> {
    if (TAURI_AVAILABLE) return invoke('load_chat_messages', { profileId, conversationId: conversationId ?? null });
    const q = conversationId ? `?conversation_id=${encodeURIComponent(conversationId)}` : '';
    return httpFetch<any[]>('GET', `/api/chat/${profileId}/messages${q}`);
  },

  async listProfileConversations(profileId: string): Promise<any[]> {
    if (TAURI_AVAILABLE) return invoke('list_profile_conversations', { profileId });
    return httpFetch<any[]>('GET', `/api/chat/${profileId}/conversations`);
  },

  async createProfileConversation(profileId: string, title?: string): Promise<string> {
    if (TAURI_AVAILABLE) return invoke('create_profile_conversation', { profileId, title: title ?? null });
    const r = await httpFetch<{ id: string }>('POST', `/api/chat/${profileId}/conversations`, { title: title ?? null });
    return r.id;
  },

  async deleteProfileConversation(conversationId: string): Promise<void> {
    if (TAURI_AVAILABLE) return invoke('delete_profile_conversation', { conversationId });
    await httpFetch<void>('DELETE', `/api/chat/conversations/${conversationId}`);
  },

  async clearConversationMessages(conversationId: string): Promise<void> {
    if (TAURI_AVAILABLE) return invoke('clear_conversation_messages', { conversationId });
    await httpFetch<void>('DELETE', `/api/chat/conversations/${conversationId}/messages`);
  },

  async clearChatMessages(profileId: string): Promise<void> {
    if (TAURI_AVAILABLE) return invoke('clear_chat_messages', { profileId });
    await httpFetch<void>('DELETE', `/api/chat/${profileId}/messages`);
  },

  async chatWithProfile(request: {
    profile_id: string;
    user_message: string;
    conversation_context?: any[]; // Array of Message objects
    conversation_id?: string;
    language?: string;
    web_search_results?: any[];
    timeout_seconds?: number;
    apply_privacy?: boolean;
    /** For hybrid: 'default' | 'local' | 'cloud' */
    model_preference?: 'default' | 'local' | 'cloud';
    /** Documents to include as context (name + content) */
    attached_documents?: Array<{ name: string; content: string }>;
  }): Promise<string> {
    if (TAURI_AVAILABLE) return invoke('chat_with_profile', { request });
    const r = await httpFetch<{ text: string }>('POST', `/api/chat/${request.profile_id}`, request);
    return r.text;
  },

  async improveResponseWithCloud(request: {
    profile_id: string;
    assistant_message: string;
    user_improvement_prompt?: string;
  }): Promise<string> {
    if (TAURI_AVAILABLE) return invoke('improve_response_with_cloud', { request });
    const r = await httpFetch<{ text: string }>('POST', `/api/chat/${request.profile_id}/improve`, request);
    return r.text;
  },

  async insertChatMessage(profileId: string, role: string, content: string): Promise<string> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('insert_chat_message', { profile_id: profileId, role, content, userId });
    const r = await httpFetch<{ id: string }>('POST', `/api/chat/${profileId}/messages`, { role, content, user_id: userId });
    return r.id;
  },

  async updateChatMessageContent(messageId: string, content: string): Promise<void> {
    if (TAURI_AVAILABLE) return invoke('update_chat_message_content', { messageId, content });
    await httpFetch<void>('PUT', `/api/chat/messages/${messageId}`, { content });
  },

  async searchWeb(query: string, maxResults?: number): Promise<any[]> {
    return invoke('search_web', { request: { query, max_results: maxResults } });
  },

  // Training and Local Models
  async createLocalModel(request: { project_id: string; name?: string; base_model: string; training_config_json?: any }): Promise<string> {
    return invoke('create_local_model', { request });
  },

  async updateLocalModel(modelId: string, request: { name: string; base_model: string; training_config_json?: any }): Promise<void> {
    return invoke('update_local_model', { modelId, request });
  },

  async listLocalModels(projectId?: string): Promise<any[]> {
    return invoke('list_local_models', { projectId });
  },

  async createTrainingData(request: { project_id: string; local_model_id?: string; input_text: string; output_text: string; metadata_json?: any }): Promise<string> {
    return invoke('create_training_data', { request });
  },

  async listTrainingData(projectId?: string, localModelId?: string): Promise<any[]> {
    return invoke('list_training_data', { projectId, localModelId });
  },

  async updateLocalModelStatus(modelId: string, status: string, metricsJson?: any): Promise<void> {
    return invoke('update_local_model_status', { modelId, status, metricsJson });
  },

  async checkTrainingEnvironment(): Promise<any> {
    return invoke('check_training_environment');
  },

  async startTraining(modelId: string, projectId: string): Promise<string> {
    return invoke('start_training', { modelId, projectId });
  },

  // LoRA/QLoRA Training Configuration types
  async startLoraTraining(request: {
    model_id: string;
    project_id: string;
    base_model: string;
    config: {
      num_train_epochs: number;
      learning_rate: number;
      per_device_train_batch_size: number;
      gradient_accumulation_steps: number;
      warmup_ratio: number;
      weight_decay: number;
      max_seq_length: number;
      fp16: boolean;
      bf16: boolean;
      save_steps: number;
      logging_steps: number;
      save_total_limit: number;
      lora_config: {
        use_lora: boolean;
        use_qlora: boolean;
        lora_rank: number;
        lora_alpha: number;
        lora_dropout: number;
        target_modules: string[];
        bias: string;
        task_type: string;
      };
      max_train_samples?: number;
      max_tokens_per_sample?: number;
      use_8bit_adam: boolean;
      gradient_checkpointing: boolean;
      dry_run: boolean;
      dataset_format: string;
      prompt_template?: string;
      eval_split_ratio: number;
    };
  }): Promise<string> {
    return invoke('start_lora_training', { request });
  },

  async getTrainingProgress(modelId: string): Promise<any> {
    return invoke('get_training_progress', { modelId });
  },

  async stopTraining(modelId: string): Promise<string> {
    if (!TAURI_AVAILABLE) {
      showBrowserModeWarning('Stopping training');
      throw new Error('Tauri backend not available. Please run: npm run tauri dev');
    }
    return invoke('stop_training', { modelId });
  },

  async chatWithTrainingData(request: {
    project_id: string;
    local_model_id?: string;
    query: string;
    profile_id?: string;
    max_examples?: number;
    use_local?: boolean;
    local_model_name?: string;
  }): Promise<any> {
    return invoke('chat_with_training_data', { request });
  },

  // Dependencies
  async checkDependencies(): Promise<any> {
    return invoke('check_dependencies');
  },

  async installDependency(dependencyName: string): Promise<string> {
    return invoke('install_dependency', { dependencyName });
  },

  async installAllDependencies(): Promise<string> {
    return invoke('install_all_dependencies');
  },

  async saveHfToken(token: string): Promise<string> {
    if (!isTauri()) {
      console.warn('saveHfToken requires Tauri backend');
      return Promise.reject('Tauri backend required');
    }
    return invoke('save_hf_token', { token });
  },

  async getHfToken(): Promise<string | null> {
    if (!isTauri()) {
      console.warn('getHfToken requires Tauri backend');
      return Promise.reject('Tauri backend required');
    }
    return invoke('get_hf_token');
  },

  async deleteHfToken(): Promise<string> {
    if (!isTauri()) {
      console.warn('deleteHfToken requires Tauri backend');
      return Promise.reject('Tauri backend required');
    }
    return invoke('delete_hf_token');
  },

  async upgradeDependency(dependencyName: string): Promise<string> {
    if (!isTauri()) {
      console.warn('upgradeDependency requires Tauri backend');
      return Promise.reject('Tauri backend required');
    }
    return invoke('upgrade_dependency', { dependencyName });
  },

  async uninstallDependency(dependencyName: string): Promise<string> {
    if (!isTauri()) {
      console.warn('uninstallDependency requires Tauri backend');
      return Promise.reject('Tauri backend required');
    }
    return invoke('uninstall_dependency', { dependencyName });
  },

  async checkTrainingReadiness(): Promise<{
    ready: boolean;
    python_ok: boolean;
    python_version: string | null;
    python_path: string | null;
    core_packages_ok: boolean;
    lora_packages_ok: boolean;
    gpu_available: boolean;
    gpu_name: string | null;
    gpu_memory_gb: number | null;
    estimated_max_model_size: string | null;
    missing_packages: string[];
    warnings: string[];
    recommended_fixes: string[];
    install_all_command: string | null;
  }> {
    return invoke('check_training_readiness');
  },

  // Model Export
  async checkExportOptions(modelId: string): Promise<{
    model_id: string;
    model_name: string;
    training_status: string;
    model_path: string;
    model_exists: boolean;
    ready_to_export: boolean;
    export_options: {
      huggingface: {
        available: boolean;
        description: string;
      };
      gguf: {
        available: boolean;
        llama_cpp_found: boolean;
        gguf_exists: boolean;
        description: string;
      };
      ollama: {
        available: boolean;
        ollama_running: boolean;
        gguf_required: boolean;
        description: string;
      };
    };
  }> {
    return invoke('check_export_options', { modelId });
  },

  async exportModelHuggingface(request: {
    model_id: string;
    export_format: string;
    export_path?: string;
    merge_adapters: boolean;
    quantization?: string;
    ollama_model_name?: string;
    ollama_system_prompt?: string;
  }): Promise<{
    success: boolean;
    export_path: string;
    format: string;
    message: string;
  }> {
    return invoke('export_model_huggingface', { request });
  },

  async exportModelOllama(request: {
    model_id: string;
    export_format: string;
    export_path?: string;
    merge_adapters: boolean;
    quantization?: string;
    ollama_model_name?: string;
    ollama_system_prompt?: string;
  }): Promise<{
    success: boolean;
    ollama_model_name: string;
    modelfile_path: string;
    message: string;
  }> {
    return invoke('export_model_ollama', { request });
  },

  async listTrainedOllamaModels(): Promise<string[]> {
    return invoke('list_trained_ollama_models');
  },

  async convertModelToGguf(modelId: string, quantization?: string): Promise<{
    success: boolean;
    gguf_path: string;
    quantization: string;
  }> {
    return invoke('convert_model_to_gguf', { modelId, quantization });
  },

  async canStartTraining(): Promise<{
    can_start: boolean;
    current_count: number;
    max_allowed: number;
    message: string;
  }> {
    return invoke('can_start_training');
  },

  async getGgufQuantizationOptions(): Promise<{
    options: Array<{
      id: string;
      name: string;
      description: string;
      recommended: boolean;
    }>;
    default: string;
  }> {
    return invoke('get_gguf_quantization_options');
  },

  // Ollama
  async checkOllamaInstallation(baseUrl?: string): Promise<any> {
    return invoke('check_ollama_installation', { baseUrl });
  },

  async pullOllamaModel(baseUrl: string | undefined, modelName: string): Promise<string> {
    return invoke('pull_ollama_model', { baseUrl, modelName });
  },

  // Projects
  async createProject(request: { name: string; description?: string }): Promise<string> {
    return invoke('create_project', { request });
  },

  async listProjects(): Promise<any[]> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('list_projects', { userId });
    const q = userId ? `?user_id=${encodeURIComponent(userId)}` : '';
    return httpFetch<any[]>('GET', `/api/projects${q}`);
  },

  async updateProject(request: { id: string; name: string; description?: string }): Promise<void> {
    return invoke('update_project', { request });
  },

  async deleteProject(projectId: string): Promise<void> {
    return invoke('delete_project', { projectId });
  },

  async moveSessionToProject(sessionId: string, projectId: string): Promise<void> {
    return invoke('move_session_to_project', { sessionId, projectId });
  },

  // Sessions
  async createSession(request: {
    project_id: string;
    title: string;
    user_question: string;
    mode: string;
    selected_profile_ids: string[];
    run_settings?: any;
  }): Promise<string> {
    if (TAURI_AVAILABLE) return invoke('create_session', { request });
    const r = await httpFetch<{ id: string }>('POST', '/api/sessions', request);
    return r.id;
  },

  async listSessions(): Promise<any[]> {
    const userId = getCurrentUserId();
    if (TAURI_AVAILABLE) return invoke('list_sessions', { userId });
    const q = userId ? `?user_id=${encodeURIComponent(userId)}` : '';
    return httpFetch<any[]>('GET', `/api/sessions${q}`);
  },

  async getSession(sessionId: string): Promise<any | null> {
    return invoke('get_session', { sessionId });
  },

  async getSessionRun(sessionId: string): Promise<any | null> {
    if (TAURI_AVAILABLE) return invoke('get_session_run', { sessionId });
    return httpFetch<any | null>('GET', `/api/sessions/${sessionId}/run`);
  },

  async deleteSession(sessionId: string): Promise<void> {
    if (TAURI_AVAILABLE) return invoke('delete_session', { sessionId });
    await httpFetch<void>('DELETE', `/api/sessions/${sessionId}`);
  },

  // Run execution
  async startRun(runId: string): Promise<void> {
    return invoke('start_run', { runId });
  },

  async getRunStatus(runId: string): Promise<any> {
    return invoke('get_run_status', { runId });
  },

  async getRunResults(runId: string): Promise<any[]> {
    return invoke('get_run_results', { runId });
  },

  async cancelRun(runId: string): Promise<void> {
    return invoke('cancel_run', { runId });
  },

  async cancelRunResult(resultId: string): Promise<void> {
    return invoke('cancel_run_result', { resultId });
  },

  async deleteRunResult(resultId: string): Promise<void> {
    return invoke('delete_run_result', { resultId });
  },

  async rerunSingleAgent(runId: string, profileId: string): Promise<string> {
    return invoke('rerun_single_agent', { runId, profileId });
  },

  async continueAgent(resultId: string, followUpMessage: string): Promise<string> {
    return invoke('continue_agent', { resultId, followUpMessage });
  },

  // Debate
  async startDebate(runId: string, rounds: number, speakingOrder: string[], maxWords?: number, language?: string, tone?: string, webSearchResults?: any[]): Promise<void> {
    return invoke('start_debate', { runId, rounds, speakingOrder, maxWords, language, tone, webSearchResults });
  },

  async getDebateMessages(runId: string): Promise<any[]> {
    return invoke('get_debate_messages', { runId });
  },

  async pauseDebate(runId: string): Promise<void> {
    return invoke('pause_debate', { runId });
  },

  async resumeDebate(runId: string): Promise<void> {
    return invoke('resume_debate', { runId });
  },

  async cancelDebate(runId: string, opts?: { timeoutMs?: number }): Promise<void> {
    const timeoutMs = opts?.timeoutMs ?? 8000;
    return Promise.race([
      invoke('cancel_debate', { runId }) as Promise<void>,
      new Promise<void>((_, reject) =>
        setTimeout(() => reject(new Error('Stop timed out. The debate will stop in the background.')), timeoutMs)
      ),
    ]);
  },

  async deleteDebateMessage(messageId: string): Promise<void> {
    return invoke('delete_debate_message', { messageId });
  },

  async addUserMessage(runId: string, text: string, insertAfterMessageId?: string): Promise<string> {
    return invoke('add_user_message', { runId, text, insertAfterMessageId });
  },

  async continueDebate(runId: string, rounds: number): Promise<void> {
    return invoke('continue_debate', { runId, rounds });
  },

  // Export
  async exportSessionMarkdown(sessionId: string): Promise<string> {
    return invoke('export_session_markdown', { sessionId });
  },

  async exportSessionJson(sessionId: string): Promise<any> {
    return invoke('export_session_json', { sessionId });
  },

  async generateComparisonTable(runId: string): Promise<string> {
    return invoke('generate_comparison_table', { runId });
  },

  // Authentication
  async signup(username: string, email: string, password: string): Promise<any> {
    if (TAURI_AVAILABLE) {
      return invoke('signup', { request: { username, email, password } });
    }
    return httpFetch<any>('POST', '/api/auth/signup', { username, email, password });
  },

  async login(username: string, password: string, rememberMe: boolean = false): Promise<any> {
    if (TAURI_AVAILABLE) {
      return invoke('login', { request: { username, password, remember_me: rememberMe } });
    }
    return httpFetch<any>('POST', '/api/auth/login', { username, password, remember_me: rememberMe });
  },

  async logout(): Promise<void> {
    if (TAURI_AVAILABLE) {
      return invoke('logout');
    }
    await httpFetch<void>('POST', '/api/auth/logout');
  },

  async getCurrentUser(userId: string): Promise<any> {
    if (TAURI_AVAILABLE) {
      return invoke('get_current_user', { userId });
    }
    return httpFetch<any>('GET', `/api/auth/user/${userId}`);
  },

  // Associate anonymous data with logged-in user
  async associateAnonymousDataWithUser(userId: string): Promise<number> {
    if (TAURI_AVAILABLE) {
      return invoke('associate_anonymous_data_with_user', { userId });
    }
    const r = await httpFetch<{ count: number }>('POST', `/api/auth/user/${userId}/claim-data`);
    return r.count;
  },

  // Create user and claim all anonymous data
  async createUserAndClaimData(username: string, email: string, password: string): Promise<any> {
    if (TAURI_AVAILABLE) {
      return invoke('create_user_and_claim_data', { username, email, password });
    }
    return httpFetch<any>('POST', '/api/auth/create-and-claim', { username, email, password });
  },

  // Statistics & System
  async getAppStatistics(): Promise<any> {
    return invoke('get_app_statistics', {});
  },

  async getAppInfo(): Promise<any> {
    return invoke('get_app_info', {});
  },

  async getDatabasePath(): Promise<any> {
    return invoke('get_database_path');
  },

  // Token usage
  async getTokenUsageSummary(): Promise<any[]> {
    return invoke('get_token_usage_summary');
  },

  async resetTokenUsage(providerId?: string, modelName?: string): Promise<number> {
    return invoke('reset_token_usage', { providerId, modelName });
  },

  async clearCache(): Promise<string> {
    return invoke('clear_cache', {});
  },

  async exportDatabaseBackup(backupPath: string): Promise<string> {
    return invoke('export_database_backup', { backupPath });
  },

  // Training Data Import
  async importTrainingDataFromText(
    projectId: string,
    localModelId: string | null,
    content: string,
    format: string
  ): Promise<any> {
    return invoke('import_training_data_from_text', {
      project_id: projectId,
      local_model_id: localModelId,
      content,
      format,
    });
  },

  async importTrainingDataFromFile(request: {
    project_id: string;
    local_model_id?: string;
    source_type: string;
    source_path: string;
    format: string;
    mapping?: any;
  }): Promise<any> {
    return invoke('import_training_data_from_file', { request });
  },

  async importTrainingDataFromUrl(request: {
    project_id: string;
    local_model_id?: string;
    source_type: string;
    source_path: string;
    format: string;
    mapping?: any;
  }): Promise<any> {
    return invoke('import_training_data_from_url', { request });
  },

  async importTrainingDataFromFolder(request: {
    project_id: string;
    local_model_id?: string;
    folder_path: string;
    include_subfolders: boolean;
  }): Promise<any> {
    return invoke('import_training_data_from_folder', { request });
  },

  async importTrainingDataFromCoderHistory(request: {
    project_id: string;
    local_model_id?: string;
    workspace_path: string;
  }): Promise<any> {
    return invoke('import_training_data_from_coder_history', { request });
  },

  async importTrainingDataFromChatMessages(request: {
    project_id: string;
    local_model_id?: string;
    profile_id?: string;
  }): Promise<any> {
    return invoke('import_training_data_from_chat_messages', { request });
  },

  async getBuildDirectorySize(): Promise<any> {
    return invoke('get_build_directory_size', {});
  },

  async cleanBuildDirectory(): Promise<string> {
    return invoke('clean_build_directory', {});
  },

  // Local GPT
  async chatWithLocalGpt(request: {
    message: string;
    conversation_history?: any[];
    model_name?: string;
    temperature?: number;
    max_tokens?: number;
  }): Promise<any> {
    return invoke('chat_with_local_gpt', { request });
  },

  // Research Paper Parsing
  async listPdfFilesInFolder(folderPath: string, recursive = true): Promise<string[]> {
    return invoke('list_pdf_files_in_folder', { folderPath, recursive });
  },

  async parsePdfAsResearchPaper(filePath: string): Promise<{
    title: string | null;
    authors: string[];
    abstract_text: string | null;
    sections: Array<{
      id: string;
      heading: string;
      level: number;
      content: string;
      token_estimate: number;
    }>;
    unassigned_content: string;
    citations: Array<{
      marker: string;
      reference_text: string | null;
      citation_type: string;
    }>;
    tables: Array<{
      id: string;
      caption: string | null;
      content: string;
    }>;
    figures: Array<{
      id: string;
      caption: string;
    }>;
    metadata: {
      doi: string | null;
      journal: string | null;
      year: number | null;
      publisher: string | null;
      arxiv_id: string | null;
      keywords: string[];
    };
    parsing_warnings: string[];
  }> {
    return invoke('parse_pdf_as_research_paper', { filePath });
  },

  async importResearchPaper(request: {
    project_id: string;
    local_model_id?: string;
    file_path: string;
    include_sections: string[];
    include_abstract: boolean;
    include_unassigned: boolean;
    chunk_by_section: boolean;
  }): Promise<{
    success_count: number;
    error_count: number;
    errors: string[];
  }> {
    return invoke('import_research_paper', { request });
  },

  // System Stats
  async getSystemStats(): Promise<any> {
    if (TAURI_AVAILABLE) return invoke('get_system_stats');
    // Return mock stats in browser mode - system stats require native access
    return {
      cpu_usage: 0,
      memory_used: 0,
      memory_total: 16 * 1024 * 1024 * 1024,
      disk_used: 0,
      disk_total: 512 * 1024 * 1024 * 1024,
    };
  },

  // Coder Chat (unrestricted mode)
  async coderChat(request: {
    provider_id: string;
    model_name: string;
    user_message: string;
    conversation_context?: any[];
    system_prompt?: string;
  }): Promise<string> {
    return invoke('coder_chat', { request });
  },

  // Coder Chat - streaming mode
  async coderChatStream(request: {
    provider_id: string;
    model_name: string;
    user_message: string;
    conversation_context?: any[];
    system_prompt?: string;
  }, streamId: string): Promise<string> {
    return invoke('coder_chat_stream', { request, streamId });
  },

  // Coder Auto Mode chat
  async coderAutoChat(request: {
    project_id: string;
    local_model_id?: string;
    provider_id: string;
    model_name: string;
    user_message: string;
    conversation_context?: any[];
    system_prompt?: string;
  }): Promise<{
    answer: string;
    from_training: boolean;
    used_remote: boolean;
  }> {
    return invoke('coder_auto_chat', { request });
  },

  async loadCoderChats(): Promise<any[]> {
    return invoke('load_coder_chats');
  },

  async saveCoderChat(chat: any): Promise<void> {
    return invoke('save_coder_chat', { chat });
  },

  async deleteCoderChat(chatId: string): Promise<void> {
    return invoke('delete_coder_chat', { chatId });
  },

  async exportChatMessagesToTraining(request: {
    message_ids: string[];
    project_id: string;
    local_model_id?: string;
  }): Promise<number> {
    return invoke('export_chat_messages_to_training', { request });
  },

  async exportCoderChatsToTraining(request: {
    chat_ids: string[];
    message_ids?: string[];
    project_id: string;
    local_model_id?: string;
  }): Promise<number> {
    return invoke('export_coder_chats_to_training', { request });
  },

  // RAG & Citations
  async getCitationsForResult(runResultId: string): Promise<any[]> {
    return invoke('get_citations_for_result', { runResultId });
  },

  async getGroundednessForResult(runResultId: string): Promise<any | null> {
    return invoke('get_groundedness_for_result', { runResultId });
  },

  async getDocumentChunk(sourceId: string, chunkIndex: number): Promise<any | null> {
    return invoke('get_document_chunk', { sourceId, chunkIndex });
  },

  // Privacy Settings
  async getPrivacySettings(): Promise<any> {
    return invoke('get_privacy_settings');
  },

  async savePrivacySettings(settings: any): Promise<void> {
    return invoke('save_privacy_settings', { settings });
  },

  async addCustomIdentifier(identifier: string): Promise<any> {
    return invoke('add_custom_identifier', { identifier });
  },

  async removeCustomIdentifier(identifier: string): Promise<any> {
    return invoke('remove_custom_identifier', { identifier });
  },

  async previewRedaction(text: string): Promise<any> {
    return invoke('preview_redaction', { text });
  },

  async deleteAllConversations(): Promise<void> {
    return invoke('delete_all_conversations');
  },

  async deleteConversation(conversationId: string): Promise<void> {
    return invoke('delete_conversation', { conversationId });
  },

  async getPseudonymForConversation(conversationId: string): Promise<string> {
    return invoke('get_pseudonym_for_conversation', { conversationId });
  },

  // Workspace / IDE commands
  async getWorkspacePath(): Promise<string> {
    if (!TAURI_AVAILABLE) {
      showBrowserModeWarning('Loading workspace path');
      throw new Error('Tauri backend not available. Please run: npm run tauri dev');
    }
    return invoke('get_workspace_path');
  },

  async listWorkspaceFiles(path?: string): Promise<any[]> {
    if (!TAURI_AVAILABLE) {
      showBrowserModeWarning('Listing workspace files');
      throw new Error('Tauri backend not available. Please run: npm run tauri dev');
    }
    return invoke('list_workspace_files', { path });
  },

  async readWorkspaceFile(path: string): Promise<string> {
    if (!TAURI_AVAILABLE) {
      showBrowserModeWarning('Reading workspace file');
      throw new Error('Tauri backend not available. Please run: npm run tauri dev');
    }
    return invoke('read_workspace_file', { path });
  },

  async writeWorkspaceFile(path: string, content: string): Promise<void> {
    if (!TAURI_AVAILABLE) {
      showBrowserModeWarning('Writing workspace file');
      throw new Error('Tauri backend not available. Please run: npm run tauri dev');
    }
    return invoke('write_workspace_file', { path, content });
  },

  async createWorkspaceFile(path: string, isDir: boolean): Promise<void> {
    return invoke('create_workspace_file', { path, isDir });
  },

  async deleteWorkspaceFile(path: string): Promise<void> {
    return invoke('delete_workspace_file', { path });
  },

  async renameWorkspaceFile(oldPath: string, newPath: string): Promise<void> {
    return invoke('rename_workspace_file', { oldPath, newPath });
  },

  async executeCommand(command: string, workingDir?: string): Promise<{ stdout: string; stderr: string; exit_code: number; success: boolean }> {
    return invoke('execute_command', { command, workingDir });
  },

  async getFileLanguage(path: string): Promise<string> {
    return invoke('get_file_language', { path });
  },

  async getCurrentDirectory(): Promise<string> {
    return invoke('get_current_directory');
  },

  async changeDirectory(path: string): Promise<string> {
    return invoke('change_directory', { path });
  },

  async checkDependency(dependency: string): Promise<boolean> {
    return invoke('check_dependency', { dependency });
  },

  async installDependencyCommand(dependency: string, installCommand: string): Promise<{ stdout: string; stderr: string; exit_code: number; success: boolean }> {
    return invoke('install_dependency_command', { dependency, installCommand });
  },

  // Coder IDE conversation management
  async saveCoderIDEConversation(conversation: any): Promise<void> {
    return invoke('save_coder_ide_conversation', { conversation });
  },

  async loadCoderIDEConversations(): Promise<any[]> {
    return invoke('load_coder_ide_conversations');
  },

  async deleteCoderIDEConversation(conversationId: string): Promise<void> {
    return invoke('delete_coder_ide_conversation', { conversationId });
  },

  async exportCoderIDEConversationsToTraining(request: {
    conversation_ids: string[];
    message_ids?: string[];
    project_id: string;
    local_model_id?: string;
    include_context: boolean;
  }): Promise<number> {
    return invoke('export_coder_ide_conversations_to_training', { request });
  },

  async ingestCoderTurn(request: {
    project_id: string;
    local_model_id?: string;
    user_text: string;
    assistant_text: string;
    context_files: string[];
    terminal_output?: string;
  }): Promise<void> {
    return invoke('ingest_coder_turn_command', { request });
  },

  // Coder Agent Mode
  async runCoderAgentTask(request: {
    provider_id: string;
    model_name: string;
    task_description: string;
    target_paths?: string[];
    allow_file_writes: boolean;
    allow_commands: boolean;
    max_steps?: number;
  }): Promise<{
    run_id: string;
    status: string;
    summary: string;
    proposed_changes: Array<{
      file_path: string;
      description?: string;
      new_content: string;
    }>;
  }> {
    return invoke('coder_agent_task', { request });
  },

  // Coder Workflows
  async listCoderWorkflows(): Promise<Array<{
    id: string;
    name: string;
    description?: string;
    workflow_json: any;
    created_at: string;
    updated_at: string;
  }>> {
    return invoke('list_coder_workflows');
  },

  async saveCoderWorkflow(request: {
    id?: string;
    name: string;
    description?: string;
    workflow_json: any;
  }): Promise<string> {
    return invoke('save_coder_workflow', { request });
  },

  async deleteCoderWorkflow(workflowId: string): Promise<void> {
    return invoke('delete_coder_workflow', { workflowId });
  },

  async runCoderWorkflow(workflowId: string): Promise<Array<{
    success: boolean;
    output: string;
    error?: string | null;
    extra_json?: any;
  }>> {
    return invoke('run_coder_workflow', { workflowId });
  },

  async recordAgentApplySteps(request: {
    run_id: string;
    changes: Array<{
      file_path: string;
      description?: string;
      new_content: string;
    }>;
  }): Promise<void> {
    return invoke('coder_agent_record_apply_steps', { request });
  },

  async listAgentRuns(): Promise<Array<{
    id: string;
    task_description: string;
    status: string;
    provider_id?: string;
    model_name?: string;
    created_at: string;
    started_at?: string;
    finished_at?: string;
    error_text?: string;
  }>> {
    return invoke('list_agent_runs');
  },

  async getAgentRunSteps(runId: string): Promise<Array<{
    id: string;
    run_id: string;
    step_index: number;
    step_type: string;
    description?: string;
    tool_name?: string;
    result_summary?: string;
    created_at: string;
  }>> {
    return invoke('get_agent_run_steps', { runId });
  },

  // Training Cache Management
  async getTrainingCacheStats(): Promise<any> {
    return invoke('get_training_cache_stats');
  },

  async clearTrainingCache(projectId?: string, modelId?: string): Promise<any> {
    return invoke('clear_training_cache', { projectId, modelId });
  },

  // App Settings
  async getAppSettings(): Promise<any> {
    return invoke('get_app_settings');
  },

  async saveAppSettings(settings: any): Promise<void> {
    return invoke('save_app_settings', { settings });
  },

  async getDefaultGlobalPromptPath(): Promise<string> {
    return invoke('get_default_global_prompt_path');
  },

  async updateGlobalSystemPromptFile(filePath: string | null): Promise<any> {
    return invoke('update_global_system_prompt_file', { filePath });
  },

  async readGlobalPromptFile(): Promise<string | null> {
    return invoke('read_global_prompt_file');
  },

  async updateCacheSettings(cacheSettings: any): Promise<any> {
    return invoke('update_cache_settings', { cacheSettings });
  },

  async updateTrainingSettings(trainingSettings: any): Promise<any> {
    return invoke('update_training_settings', { trainingSettings });
  },

  // Cline IDE commands
  async clineAgentTask(request: {
    provider_id: string;
    model_name: string;
    task_description: string;
    workspace_path: string;
    target_paths?: string[];
    conversation_context?: Array<{ role: string; content: string }>;
  }): Promise<any> {
    return invoke('cline_agent_task', { request });
  },

  async clineApproveTool(request: {
    tool_id: string;
    approved: boolean;
  }): Promise<any> {
    return invoke('cline_approve_tool', { request });
  },

  async clineCreateCheckpoint(request: {
    run_id: string;
    step_index: number;
  }): Promise<string> {
    return invoke('cline_create_checkpoint', { request });
  },

  async clineRestoreCheckpoint(request: {
    checkpoint_id: string;
  }): Promise<void> {
    return invoke('cline_restore_checkpoint', { request });
  },

  async clineCompareCheckpoint(checkpoint_id: string): Promise<any> {
    return invoke('cline_compare_checkpoint', { checkpoint_id });
  },

  async clineGetErrors(workspace_path: string): Promise<any[]> {
    return invoke('cline_get_errors', { workspace_path });
  },

  async clineAnalyzeAST(path: string): Promise<any> {
    return invoke('cline_analyze_ast', { path });
  },

  async ingestClineTurn(request: {
    project_id: string;
    local_model_id?: string;
    user_text: string;
    assistant_text: string;
    tool_executions: any[];
    browser_steps: any[] | null;
    error_context: string | null;
  }): Promise<void> {
    return invoke('ingest_cline_turn', { request });
  },

  // Voice (local STT/TTS)
  async transcribeAudio(audioBase64: string, modelId?: string): Promise<{ text: string }> {
    if (TAURI_AVAILABLE) return invoke('transcribe_audio', { request: { audioBase64, modelId } });
    const base = await getApiBase();
    const r = await fetch(`${base}/api/voice/transcribe`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ audioBase64, modelId }),
    });
    if (!r.ok) throw new Error(await r.text());
    return r.json();
  },

  async synthesizeSpeech(text: string, voiceId?: string): Promise<string> {
    if (TAURI_AVAILABLE) return invoke('synthesize_speech', { request: { text, voiceId } });
    const base = await getApiBase();
    const r = await fetch(`${base}/api/voice/synthesize`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text, voiceId }),
    });
    if (!r.ok) throw new Error(await r.text());
    return r.text();
  },
};
