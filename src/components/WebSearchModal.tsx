import { useState, useEffect } from 'react';
import { api } from '../api';

interface NewsResult {
  title: string;
  snippet: string;
  url: string;
}

interface WebSearchModalProps {
  isOpen: boolean;
  onClose: () => void;
  onConfirm: (results: NewsResult[]) => void;
  initialQuery: string;
  initialResults?: NewsResult[];
}

export function WebSearchModal({ isOpen, onClose, onConfirm, initialQuery, initialResults }: WebSearchModalProps) {
  const [query, setQuery] = useState(initialQuery);
  const [results, setResults] = useState<NewsResult[]>(initialResults || []);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset state when modal opens
  useEffect(() => {
    if (isOpen) {
      setQuery(initialQuery);
      setResults(initialResults || []);
      setError(null);
      setLoading(false);
    }
  }, [isOpen, initialQuery, initialResults]);

  const handleSearch = async () => {
    if (!query.trim()) return;
    
    setLoading(true);
    setError(null);
    
    try {
      const searchResults = await api.searchWeb(query.trim(), 5);
      setResults(searchResults || []);
    } catch (err: any) {
      setError(err instanceof Error ? err.message : (typeof err === 'string' ? err : 'Search failed'));
      setResults([]);
    } finally {
      setLoading(false);
    }
  };

  const handleRemove = (index: number) => {
    setResults(prev => prev.filter((_, i) => i !== index));
  };

  const handleEdit = (index: number, field: 'title' | 'snippet', value: string) => {
    setResults(prev => prev.map((r, i) => 
      i === index ? { ...r, [field]: value } : r
    ));
  };

  const handleConfirm = () => {
    onConfirm(results);
    onClose();
  };

  if (!isOpen) return null;

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0, 0, 0, 0.5)',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        zIndex: 2000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: 'var(--card-bg)',
          borderRadius: '8px',
          padding: '20px',
          maxWidth: '800px',
          width: '90%',
          maxHeight: '80vh',
          overflow: 'auto',
          border: '1px solid var(--border-color)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
          <h2 style={{ margin: 0, fontSize: '20px' }}>🌐 Web Search Results</h2>
          <button
            onClick={onClose}
            style={{
              background: 'transparent',
              border: 'none',
              fontSize: '24px',
              cursor: 'pointer',
              color: 'var(--text-secondary)',
            }}
          >
            ×
          </button>
        </div>

        <div style={{ marginBottom: '20px' }}>
          <div style={{ display: 'flex', gap: '10px' }}>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
              placeholder="Search for recent news and information..."
              style={{
                flex: 1,
                padding: '10px',
                borderRadius: '4px',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
              }}
            />
            <button
              onClick={handleSearch}
              disabled={loading || !query.trim()}
              className="btn btn-primary"
              style={{ padding: '10px 20px' }}
            >
              {loading ? 'Searching...' : '🔍 Search'}
            </button>
          </div>
        </div>

        {error && (
          <div style={{ 
            padding: '10px', 
            background: '#fee', 
            color: '#c00', 
            borderRadius: '4px', 
            marginBottom: '15px' 
          }}>
            {error}
          </div>
        )}

        <div style={{ marginBottom: '20px' }}>
          <h3 style={{ fontSize: '16px', marginBottom: '10px' }}>
            Results ({results.length})
          </h3>
          {results.length === 0 && !loading && (
            <p style={{ color: 'var(--text-secondary)', fontStyle: 'italic' }}>
              No results. Try searching for something.
            </p>
          )}
          {results.map((result, index) => (
            <div
              key={index}
              style={{
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                padding: '15px',
                marginBottom: '10px',
                background: 'var(--bg-secondary)',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start', marginBottom: '8px' }}>
                <div style={{ flex: 1 }}>
                  <input
                    type="text"
                    value={result.title}
                    onChange={(e) => handleEdit(index, 'title', e.target.value)}
                    style={{
                      width: '100%',
                      padding: '6px',
                      fontSize: '14px',
                      fontWeight: 'bold',
                      border: '1px solid var(--border-color)',
                      borderRadius: '4px',
                      background: 'var(--bg-primary)',
                      marginBottom: '6px',
                    }}
                    placeholder="Title"
                  />
                  <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '6px' }}>
                    <a href={result.url} target="_blank" rel="noopener noreferrer" style={{ color: '#667eea' }}>
                      {result.url}
                    </a>
                  </div>
                  <textarea
                    value={result.snippet}
                    onChange={(e) => handleEdit(index, 'snippet', e.target.value)}
                    rows={3}
                    style={{
                      width: '100%',
                      padding: '6px',
                      fontSize: '13px',
                      border: '1px solid var(--border-color)',
                      borderRadius: '4px',
                      background: 'var(--bg-primary)',
                      resize: 'vertical',
                    }}
                    placeholder="Summary/Description"
                  />
                </div>
                <button
                  onClick={() => handleRemove(index)}
                  style={{
                    marginLeft: '10px',
                    padding: '5px 10px',
                    background: '#fee',
                    color: '#c00',
                    border: '1px solid #fcc',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '12px',
                  }}
                >
                  Remove
                </button>
              </div>
            </div>
          ))}
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px' }}>
          <button
            onClick={onClose}
            className="btn"
            style={{ padding: '10px 20px' }}
          >
            Cancel
          </button>
          <button
            onClick={handleConfirm}
            className="btn btn-primary"
            style={{ padding: '10px 20px' }}
          >
            Use {results.length} Result{results.length !== 1 ? 's' : ''}
          </button>
        </div>
      </div>
    </div>
  );
}
