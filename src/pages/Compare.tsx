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

export function Compare() {
  const { runId } = useParams<{ runId: string }>();
  const navigate = useNavigate();
  const { profiles, setProfiles } = useAppStore();
  const [results, setResults] = useState<RunResult[]>([]);
  const [sessionId, setSessionId] = useState<string>('');
  const [synthesis, setSynthesis] = useState<string>('');
  const [synthesizing, setSynthesizing] = useState(false);
  const [comparisonTable, setComparisonTable] = useState<string>('');
  const [generatingComparison, setGeneratingComparison] = useState(false);

  useEffect(() => {
    if (!runId) {
      navigate('/home');
      return;
    }

    const loadData = async () => {
      try {
        // Always load profiles to ensure they're available
        try {
          const profilesData = await api.listProfiles();
          setProfiles(profilesData);
        } catch (error) {
          console.error('Failed to load profiles:', error);
        }

        const status = await api.getRunStatus(runId);
        // Get session_id from run status
        if (status.session_id) {
          setSessionId(status.session_id);
        }
        
        const runResults = await api.getRunResults(runId);
        setResults(runResults);
      } catch (error) {
        console.error('Failed to load comparison data:', error);
      }
    };

    loadData();
  }, [runId, navigate]);

  const getProfileName = (profileId: string) => {
    const profile = profiles.find(p => p.id === profileId);
    return profile?.name || profileId;
  };

  const handleExportMarkdown = async () => {
    if (!sessionId) {
      alert('Session ID not available. Please try again.');
      return;
    }

    try {
      const markdown = await api.exportSessionMarkdown(sessionId);
      const blob = new Blob([markdown], { type: 'text/markdown' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `panther-session-${sessionId.substring(0, 8)}.md`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (error) {
      console.error('Failed to export markdown:', error);
      alert('Failed to export markdown. Please check the console for details.');
    }
  };

  const handleExportJson = async () => {
    if (!sessionId) {
      alert('Session ID not available. Please try again.');
      return;
    }

    try {
      const jsonData = await api.exportSessionJson(sessionId);
      const json = JSON.stringify(jsonData, null, 2);
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `panther-session-${sessionId.substring(0, 8)}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (error) {
      console.error('Failed to export JSON:', error);
      alert('Failed to export JSON. Please check the console for details.');
    }
  };

  const handleGenerateComparison = async () => {
    if (!runId || results.length === 0) {
      alert('No results to compare.');
      return;
    }

    setGeneratingComparison(true);
    try {
      const comparison = await api.generateComparisonTable(runId);
      setComparisonTable(comparison);
      // Also set synthesis to show it in the same view
      setSynthesis(comparison);
    } catch (error) {
      console.error('Failed to generate comparison:', error);
      alert(`Failed to generate comparison: ${error}. Please make sure you have an OpenAI-compatible provider configured.`);
    } finally {
      setGeneratingComparison(false);
    }
  };

  const handleSynthesize = async () => {
    if (results.length === 0) {
      alert('No results to synthesize.');
      return;
    }

    setSynthesizing(true);
    try {
      // Combine all successful outputs
      const outputs = results
        .filter(r => r.status === 'complete' && r.raw_output_text)
        .map(r => `**${getProfileName(r.profile_id)}:**\n${r.raw_output_text}`)
        .join('\n\n---\n\n');

      // For now, just show a placeholder
      // In a full implementation, this would call a synthesis profile
      setSynthesis(`# Synthesis\n\nThis is a placeholder for synthesis functionality.\n\nIn a full implementation, this would:\n1. Use a selected profile to synthesize the responses\n2. Generate a consolidated summary\n3. Highlight agreements, disagreements, and key insights\n\n## Agent Responses Summary\n\n${outputs}`);
    } catch (error) {
      console.error('Failed to synthesize:', error);
      alert('Failed to generate synthesis. Please check the console for details.');
    } finally {
      setSynthesizing(false);
    }
  };

  return (
    <div>
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: '15px', marginBottom: '10px' }}>
          <button
            onClick={() => navigate(-1)}
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
        <h1>Compare & Synthesis</h1>
        <p>Compare agent outputs and generate synthesis</p>
      </div>

      <div style={{ marginBottom: '20px', display: 'flex', gap: '10px' }}>
        <button
          className="btn btn-secondary"
          onClick={handleExportMarkdown}
          disabled={!sessionId}
        >
          Export Markdown
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleExportJson}
          disabled={!sessionId}
        >
          Export JSON
        </button>
        <button
          className="btn btn-primary"
          onClick={handleGenerateComparison}
          disabled={generatingComparison || results.length === 0}
        >
          {generatingComparison ? 'Generating Comparison...' : 'Generate Comparison Table'}
        </button>
        <button
          className="btn btn-secondary"
          onClick={handleSynthesize}
          disabled={synthesizing || results.length === 0}
        >
          {synthesizing ? 'Synthesizing...' : 'Generate Synthesis'}
        </button>
        <button
          className="btn btn-secondary"
          onClick={() => navigate('/home')}
        >
          Back to Home
        </button>
      </div>

      {results.length > 0 && (
        <div className="grid grid-2" style={{ marginBottom: '20px' }}>
          {results.map((result) => {
            const profileName = getProfileName(result.profile_id);
            return (
              <div key={result.id} className="card">
                <h3>{profileName}</h3>
                <div style={{ 
                  marginBottom: '10px',
                  padding: '4px 8px',
                  background: result.status === 'complete' ? '#d4edda' : '#f8d7da',
                  color: result.status === 'complete' ? '#155724' : '#721c24',
                  borderRadius: '4px',
                  fontSize: '12px',
                  display: 'inline-block',
                }}>
                  {result.status}
                </div>

                {result.status === 'complete' && result.raw_output_text && (
                  <div
                    style={{
                      marginTop: '15px',
                      padding: '15px',
                      background: '#f8f9fa',
                      borderRadius: '4px',
                      maxHeight: '400px',
                      overflowY: 'auto',
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
                      marginTop: '15px',
                      padding: '15px',
                      background: '#fff3cd',
                      border: '1px solid #ffc107',
                      borderRadius: '4px',
                      color: '#856404',
                    }}
                  >
                    <strong>Error:</strong> {result.error_message_safe}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {(comparisonTable || synthesis) && (
        <div className="card">
          <h2>{comparisonTable ? 'Comparison Table & Analysis' : 'Synthesis'}</h2>
          <div
            style={{
              padding: '20px',
              background: '#f8f9fa',
              borderRadius: '4px',
              maxHeight: '600px',
              overflowY: 'auto',
              whiteSpace: 'pre-wrap',
              fontFamily: 'monospace',
              fontSize: '14px',
              lineHeight: '1.6',
            }}
          >
            {comparisonTable || synthesis}
          </div>
        </div>
      )}

      {results.length === 0 && (
        <div className="card" style={{ textAlign: 'center', padding: '40px' }}>
          <p>No results available yet.</p>
        </div>
      )}
    </div>
  );
}
