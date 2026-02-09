// Core type definitions for the Panther app

export type ProviderType =
  | 'openai_compatible'
  | 'anthropic'
  | 'google'
  | 'local_http'
  | 'ollama'
  | 'grok'
  | 'hybrid';

export type RunStatus = 'queued' | 'running' | 'partial' | 'complete' | 'failed' | 'cancelled';

export type SessionMode = 'parallel' | 'debate';

export type DebateMode = 'sequential' | 'parallel' | 'fishbowl';

export type ContextPolicy = 'last_k_messages' | 'rolling_summary';

export type AuthorType = 'user' | 'agent' | 'moderator';

export type SynthesisMethod = 'model_based' | 'rule_based';

export type ErrorCode = 
  | 'auth' 
  | 'rate_limit' 
  | 'timeout' 
  | 'invalid_model' 
  | 'provider_error' 
  | 'network_error';

export interface ProviderAccount {
  id: string;
  provider_type: ProviderType;
  display_name: string;
  base_url?: string;
  region?: string;
  auth_ref?: string; // keychain reference
  created_at: string;
  updated_at: string;
  provider_metadata_json?: Record<string, unknown>;
}

export interface ModelDefinition {
  id: string;
  provider_account_id: string;
  model_name: string;
  capabilities_json?: Record<string, unknown>;
  context_limit?: number;
  is_discovered: boolean;
  created_at: string;
  updated_at: string;
}

export interface CharacterDefinition {
  name: string;
  role: string;
  personality: string[];
  expertise: string[];
  communication_style: string;
  background?: string;
  goals?: string[];
  constraints?: string[];
}

export interface ModelFeatures {
  supports_vision?: boolean;
  supports_function_calling?: boolean;
  supports_streaming?: boolean;
  max_context_length?: number;
  supports_tools?: boolean;
  custom_capabilities?: string[];
}

export interface PromptProfile {
  id: string;
  name: string;
  provider_account_id: string;
  model_name: string;
  persona_prompt: string;
  character_definition?: CharacterDefinition;
  model_features?: ModelFeatures;
  params_json: GenerationParams;
  output_preset_id?: string;
  photo_url?: string;
  /** Voice gender for TTS: 'male' | 'female' | 'neutral' | 'any' */
  voice_gender?: string;
  /** Specific voice URI from speechSynthesis.getVoices() */
  voice_uri?: string;
  created_at: string;
  updated_at: string;
}

export interface GenerationParams {
  temperature?: number;
  top_p?: number;
  max_tokens?: number;
  frequency_penalty?: number;
  presence_penalty?: number;
  [key: string]: unknown;
}

export interface Project {
  id: string;
  name: string;
  description?: string;
  created_at: string;
  updated_at: string;
}

export interface Session {
  id: string;
  project_id: string;
  title: string;
  user_question: string;
  mode: SessionMode;
  global_prompt_template_id?: string;
  created_at: string;
  updated_at: string;
}

export interface Run {
  id: string;
  session_id: string;
  selected_profile_ids_json: string[]; // JSON array of profile IDs
  status: RunStatus;
  run_settings_json: RunSettings;
  started_at?: string;
  finished_at?: string;
}

export interface RunSettings {
  concurrency?: number;
  streaming?: boolean;
  [key: string]: unknown;
}

export interface RunResult {
  id: string;
  run_id: string;
  profile_id: string;
  status: RunStatus;
  raw_output_text?: string;
  normalized_output_json?: Record<string, unknown>;
  usage_json?: UsageInfo;
  error_code?: ErrorCode;
  error_message_safe?: string;
  started_at?: string;
  finished_at?: string;
}

export interface UsageInfo {
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  [key: string]: unknown;
}

export interface DebateConfig {
  id: string;
  run_id: string;
  mode: DebateMode;
  rounds: number;
  speaking_order_json: string[]; // profile IDs in order
  context_policy: ContextPolicy;
  last_k?: number;
  per_turn_budget_json?: Record<string, unknown>;
  concurrency: number;
}

export interface DebateTurn {
  id: string;
  run_id: string;
  round_index: number;
  turn_index: number;
  speaker_profile_id: string;
  input_snapshot_json: Record<string, unknown>;
  status: RunStatus;
  started_at?: string;
  finished_at?: string;
  error_code?: ErrorCode;
  error_message?: string;
}

export interface Message {
  id: string;
  run_id: string;
  author_type: AuthorType;
  profile_id?: string;
  round_index?: number;
  turn_index?: number;
  text: string;
  created_at: string;
  provider_metadata_json?: Record<string, unknown>;
}

export interface Synthesis {
  id: string;
  run_id: string;
  method: SynthesisMethod;
  synthesizer_profile_id?: string;
  text: string;
  created_at: string;
}

// Provider adapter interfaces
export interface PromptPacket {
  global_instructions?: string;
  persona_instructions: string;
  user_message: string;
  conversation_context?: Message[];
  params_json: GenerationParams;
  stream: boolean;
}

export interface NormalizedResponse {
  text: string;
  finish_reason?: string;
  request_id?: string;
  usage_json?: UsageInfo;
  raw_provider_payload_json?: Record<string, unknown>;
}

export interface ProviderAdapter {
  validate(config: ProviderAccount): Promise<boolean>;
  listModels(config: ProviderAccount): Promise<string[]>;
  complete(packet: PromptPacket, config: ProviderAccount, model: string): Promise<NormalizedResponse>;
  stream(packet: PromptPacket, config: ProviderAccount, model: string, onChunk: (chunk: string) => void): Promise<NormalizedResponse>;
}

// Debate state machine
export type DebateState = 
  | 'IDLE' 
  | 'STARTING' 
  | 'ROUND_ACTIVE' 
  | 'TURN_ACTIVE' 
  | 'PAUSED' 
  | 'CANCELLED' 
  | 'COMPLETE';

// Event bus types
export interface DebateEvent {
  type: 
    | 'debate_started' 
    | 'round_started' 
    | 'turn_started' 
    | 'token_chunk' 
    | 'turn_completed' 
    | 'turn_failed' 
    | 'debate_paused' 
    | 'debate_resumed' 
    | 'debate_cancelled' 
    | 'debate_completed';
  run_id: string;
  profile_id?: string;
  round_index?: number;
  turn_index?: number;
  message_id?: string;
  chunk_text?: string;
  error_code?: ErrorCode;
  error_message?: string;
}
