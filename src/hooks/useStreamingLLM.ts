import { useState, useCallback, useRef, useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { api } from '../api';

export type StreamingLLMConfig =
  | {
      type: 'coder';
      providerId: string;
      modelName: string;
      systemPrompt?: string;
    }
  | {
      type: 'profile';
      profileId: string;
      conversationId?: string;
      conversationContext: any[];
      language?: string;
      webSearchResults?: any[];
      modelPreference?: 'default' | 'local' | 'cloud';
      attachedDocuments?: Array<{ name: string; content: string }>;
    };

export interface StreamOverrides {
  conversationContext?: any[];
  conversationId?: string;
  webSearchResults?: any[];
  attachedDocuments?: Array<{ name: string; content: string }>;
  /** For coder: override system prompt per call */
  systemPrompt?: string;
}

export interface UseStreamingLLMResult {
  streamedText: string;
  isStreaming: boolean;
  error: string | null;
  startStream: (userMessage: string, overrides?: StreamOverrides) => Promise<void>;
  stopStream: () => void;
}

/**
 * Shared hook for LLM chat. Supports coder-style streaming via Tauri events.
 * Profile chat uses non-streaming API (backend streaming can be added later).
 */
export function useStreamingLLM(config: StreamingLLMConfig | null): UseStreamingLLMResult {
  const [streamedText, setStreamedText] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const stopStream = useCallback(() => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }
    setIsStreaming(false);
  }, []);

  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, []);

  const startStream = useCallback(
    async (userMessage: string, overrides?: StreamOverrides) => {
      if (!config) return;

      setError(null);
      setStreamedText('');
      setIsStreaming(true);

      const convCtx = overrides?.conversationContext ?? (config.type === 'profile' ? config.conversationContext : []);
      const webSearch = overrides?.webSearchResults ?? (config.type === 'profile' ? config.webSearchResults : undefined);
      const attachedDocs = overrides?.attachedDocuments ?? (config.type === 'profile' ? config.attachedDocuments : undefined);
      const sysPrompt = config.type === 'coder' ? (overrides?.systemPrompt ?? config.systemPrompt) : undefined;

      try {
        if (config.type === 'coder') {
          const streamId = crypto.randomUUID();

          if (unlistenRef.current) {
            await unlistenRef.current();
            unlistenRef.current = null;
          }

          unlistenRef.current = await listen<{
            stream_id: string;
            chunk: string;
            done: boolean;
            error?: string | null;
          }>('panther://coder_stream', (event) => {
            const { stream_id, chunk, done, error: evtError } = event.payload;
            if (stream_id !== streamId) return;

            if (evtError) {
              setError(evtError);
              setIsStreaming(false);
              if (unlistenRef.current) {
                unlistenRef.current();
                unlistenRef.current = null;
              }
            } else if (chunk) {
              setStreamedText((prev) => prev + chunk);
            }

            if (done) {
              if (unlistenRef.current) {
                unlistenRef.current();
                unlistenRef.current = null;
              }
              setIsStreaming(false);
            }
          });

          const timeoutPromise = new Promise<string>((_, reject) => {
            setTimeout(() => reject(new Error('Streaming timeout')), 120000);
          });

          try {
            await Promise.race([
              api.coderChatStream(
                {
                  provider_id: config.providerId,
                  model_name: config.modelName,
                  user_message: userMessage,
                  conversation_context: convCtx ?? [],
                  system_prompt: sysPrompt,
                },
                streamId
              ),
              timeoutPromise,
            ]);
          } catch (timeoutErr) {
            // Fallback to non-streaming on timeout
            if (unlistenRef.current) {
              unlistenRef.current();
              unlistenRef.current = null;
            }
            try {
              const fallback = await api.coderChat({
                provider_id: config.providerId,
                model_name: config.modelName,
                user_message: userMessage,
                conversation_context: convCtx ?? [],
                system_prompt: sysPrompt,
              });
              setStreamedText(fallback);
            } catch (fallbackErr: any) {
              setError(fallbackErr?.message || String(fallbackErr));
            }
            setIsStreaming(false);
          }
        } else if (config.type === 'profile') {
          const convId = overrides?.conversationId ?? config.conversationId;
          const response = await Promise.race([
            api.chatWithProfile({
              profile_id: config.profileId,
              user_message: userMessage,
              conversation_context: convCtx,
              conversation_id: convId,
              language: config.language,
              web_search_results: webSearch,
              timeout_seconds: 90,
              model_preference: config.modelPreference,
              attached_documents: attachedDocs,
            }),
            new Promise<string>((_, reject) => {
              setTimeout(() => reject(new Error('Model timed out after 90 seconds')), 90_000);
            }),
          ]);
          setStreamedText(response);
          setIsStreaming(false);
        }
      } catch (err: any) {
        setError(err?.message || String(err));
        setIsStreaming(false);
        if (unlistenRef.current) {
          unlistenRef.current();
          unlistenRef.current = null;
        }
      }
    },
    [config]
  );

  return {
    streamedText,
    isStreaming,
    error,
    startStream,
    stopStream,
  };
}
