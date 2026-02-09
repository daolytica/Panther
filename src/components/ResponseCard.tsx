import { useState, useEffect } from 'react';
import { api } from '../api';

interface Citation {
  id: string;
  run_result_id: string;
  source_id: string;
  chunk_index: number;
  citation_text: string;
  created_at: string;
}

interface GroundednessScore {
  id: string;
  run_result_id: string;
  score: number;
  is_grounded: boolean;
  ungrounded_claims?: string;
  created_at: string;
}

interface ResponseCardProps {
  runResultId: string;
  text: string;
  showMetadata?: boolean;
  /** When true, skips citations/groundedness API calls (faster, e.g. for debate messages) */
  skipMetadataLoading?: boolean;
}

export function ResponseCard({ runResultId, text, showMetadata = true, skipMetadataLoading = false }: ResponseCardProps) {
  const [citations, setCitations] = useState<Citation[]>([]);
  const [groundedness, setGroundedness] = useState<GroundednessScore | null>(null);
  const [expandedCitations, setExpandedCitations] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (skipMetadataLoading) return;
    const loadMetadata = async () => {
      try {
        const [cits, ground] = await Promise.all([
          api.getCitationsForResult(runResultId),
          api.getGroundednessForResult(runResultId),
        ]);
        setCitations(cits || []);
        setGroundedness(ground || null);
      } catch (error) {
        console.error('Failed to load citations/groundedness:', error);
      } finally {
        // no-op
      }
    };

    if (showMetadata && runResultId) {
      loadMetadata();
    }
  }, [runResultId, showMetadata, skipMetadataLoading]);

  // Parse citations from text (format: [source:SOURCE_ID chunk:INDEX])
  const parseCitations = (text: string): Array<{ sourceId: string; chunkIndex: number; match: string }> => {
    const citationRegex = /\[source:([^\s]+)\s+chunk:(\d+)\]/g;
    const matches: Array<{ sourceId: string; chunkIndex: number; match: string }> = [];
    let match;
    
    while ((match = citationRegex.exec(text)) !== null) {
      matches.push({
        sourceId: match[1],
        chunkIndex: parseInt(match[2], 10),
        match: match[0],
      });
    }
    
    return matches;
  };

  const inlineCitations = parseCitations(text);
  const hasCitations = citations.length > 0 || inlineCitations.length > 0;

  // Render text with highlighted citations
  const renderTextWithCitations = (text: string) => {
    if (!hasCitations) {
      return <span style={{ whiteSpace: 'pre-wrap' }}>{text}</span>;
    }

    const parts: Array<{ text: string; isCitation: boolean; sourceId?: string; chunkIndex?: number }> = [];
    let lastIndex = 0;
    const citationRegex = /\[source:([^\s]+)\s+chunk:(\d+)\]/g;
    let match;

    while ((match = citationRegex.exec(text)) !== null) {
      // Add text before citation
      if (match.index > lastIndex) {
        parts.push({ text: text.substring(lastIndex, match.index), isCitation: false });
      }
      
      // Add citation
      parts.push({
        text: match[0],
        isCitation: true,
        sourceId: match[1],
        chunkIndex: parseInt(match[2], 10),
      });
      
      lastIndex = match.index + match[0].length;
    }

    // Add remaining text
    if (lastIndex < text.length) {
      parts.push({ text: text.substring(lastIndex), isCitation: false });
    }

    return (
      <span style={{ whiteSpace: 'pre-wrap' }}>
        {parts.map((part, idx) => {
          if (part.isCitation) {
            const citationId = `${part.sourceId}-${part.chunkIndex}`;
            const isExpanded = expandedCitations.has(citationId);
            return (
              <span key={idx}>
                <span
                  style={{
                    background: 'var(--highlight-bg)',
                    color: 'var(--text-primary)',
                    padding: '2px 6px',
                    borderRadius: '3px',
                    cursor: 'pointer',
                    fontWeight: '500',
                    fontSize: '0.9em',
                    border: '1px solid #90caf9',
                  }}
                  onClick={() => {
                    const newExpanded = new Set(expandedCitations);
                    if (isExpanded) {
                      newExpanded.delete(citationId);
                    } else {
                      newExpanded.add(citationId);
                    }
                    setExpandedCitations(newExpanded);
                  }}
                  title="Click to view source"
                >
                  {part.text}
                </span>
                {isExpanded && part.sourceId && part.chunkIndex !== undefined && (
                  <CitationTooltip
                    sourceId={part.sourceId}
                    chunkIndex={part.chunkIndex}
                    onClose={() => {
                      const newExpanded = new Set(expandedCitations);
                      newExpanded.delete(citationId);
                      setExpandedCitations(newExpanded);
                    }}
                  />
                )}
              </span>
            );
          }
          return <span key={idx}>{part.text}</span>;
        })}
      </span>
    );
  };

  return (
    <div>
      {showMetadata && (
        <div style={{ marginBottom: '10px', display: 'flex', gap: '10px', flexWrap: 'wrap', alignItems: 'center' }}>
          {groundedness && (
            <div
              style={{
                padding: '4px 10px',
                borderRadius: '12px',
                fontSize: '12px',
                fontWeight: '500',
                background: groundedness.is_grounded ? '#d4edda' : '#f8d7da',
                color: groundedness.is_grounded ? '#155724' : '#721c24',
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
              }}
              title={`Groundedness Score: ${(groundedness.score * 100).toFixed(1)}%`}
            >
              {groundedness.is_grounded ? '✅' : '⚠️'}
              <span>Grounded: {(groundedness.score * 100).toFixed(0)}%</span>
            </div>
          )}
          
          {hasCitations && (
            <div
              style={{
                padding: '4px 10px',
                borderRadius: '12px',
                fontSize: '12px',
                background: 'var(--highlight-bg)',
                color: 'var(--text-primary)',
                display: 'flex',
                alignItems: 'center',
                gap: '5px',
              }}
            >
              📚 {citations.length + inlineCitations.length} citation{citations.length + inlineCitations.length !== 1 ? 's' : ''}
            </div>
          )}
        </div>
      )}

      <div style={{ whiteSpace: 'pre-wrap', lineHeight: '1.6' }}>
        {renderTextWithCitations(text)}
      </div>

      {showMetadata && citations.length > 0 && (
        <div style={{ marginTop: '15px', paddingTop: '15px', borderTop: '1px solid var(--border-color)' }}>
          <div style={{ fontSize: '12px', fontWeight: '600', marginBottom: '8px', color: 'var(--text-secondary)' }}>
            Citations:
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {citations.map((citation) => (
              <div
                key={citation.id}
                style={{
                  padding: '8px',
                  background: '#f5f5f5',
                  borderRadius: '4px',
                  fontSize: '12px',
                }}
              >
                <div style={{ fontWeight: '500', marginBottom: '4px' }}>
                  Source: {citation.source_id} (Chunk {citation.chunk_index})
                </div>
                <div style={{ color: 'var(--text-secondary)', fontStyle: 'italic' }}>
                  "{citation.citation_text.substring(0, 100)}..."
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {showMetadata && groundedness && !groundedness.is_grounded && groundedness.ungrounded_claims && (
        <div
          style={{
            marginTop: '15px',
            padding: '10px',
            background: 'var(--warning-bg)',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            fontSize: '12px',
            color: 'var(--warning-text)',
          }}
        >
          <strong>⚠️ Ungrounded Claims Detected:</strong>
          <div style={{ marginTop: '5px' }}>{groundedness.ungrounded_claims}</div>
        </div>
      )}
    </div>
  );
}

function CitationTooltip({
  sourceId,
  chunkIndex,
  onClose,
}: {
  sourceId: string;
  chunkIndex: number;
  onClose: () => void;
}) {
  const [chunk, setChunk] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const loadChunk = async () => {
      try {
        const data = await api.getDocumentChunk(sourceId, chunkIndex);
        setChunk(data);
      } catch (error) {
        console.error('Failed to load chunk:', error);
      } finally {
        setLoading(false);
      }
    };
    loadChunk();
  }, [sourceId, chunkIndex]);

  return (
    <div
      style={{
        position: 'absolute',
        background: 'var(--card-bg)',
        border: '1px solid var(--border-color)',
        borderRadius: '4px',
        padding: '10px',
        marginTop: '5px',
        boxShadow: '0 2px 8px rgba(0,0,0,0.15)',
        zIndex: 1000,
        maxWidth: '400px',
        fontSize: '12px',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
        <strong>Source: {sourceId}</strong>
        <button
          onClick={onClose}
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
      {loading ? (
        <div>Loading...</div>
      ) : chunk ? (
        <div style={{ color: 'var(--text-secondary)', maxHeight: '200px', overflowY: 'auto' }}>
          {chunk.chunk_text}
        </div>
      ) : (
        <div style={{ color: 'var(--text-tertiary)' }}>Source not found</div>
      )}
    </div>
  );
}
