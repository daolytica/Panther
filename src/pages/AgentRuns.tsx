import { useEffect, useState } from 'react';
import { api } from '../api';

interface AgentRun {
  id: string;
  task_description: string;
  status: string;
  provider_id?: string;
  model_name?: string;
  created_at: string;
  started_at?: string;
  finished_at?: string;
  error_text?: string;
}

interface AgentStep {
  id: string;
  run_id: string;
  step_index: number;
  step_type: string;
  description?: string;
  tool_name?: string;
  result_summary?: string;
  created_at: string;
}

export function AgentRuns() {
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [steps, setSteps] = useState<AgentStep[]>([]);
  const [loadingRuns, setLoadingRuns] = useState(false);
  const [loadingSteps, setLoadingSteps] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [workspacePath, setWorkspacePath] = useState<string | null>(null);
  const [gitStatus, setGitStatus] = useState<string | null>(null);
  const [gitDiff, setGitDiff] = useState<string | null>(null);

  useEffect(() => {
    const load = async () => {
      setLoadingRuns(true);
      setError(null);
      try {
        const ws = await api.getWorkspacePath();
        setWorkspacePath(ws);
        const list = await api.listAgentRuns();
        setRuns(list);
        if (list.length > 0 && !selectedRunId) {
          setSelectedRunId(list[0].id);
        }
      } catch (err: any) {
        setError(err?.message || String(err));
      } finally {
        setLoadingRuns(false);
      }
    };
    load();
  }, []);

  useEffect(() => {
    const loadSteps = async () => {
      if (!selectedRunId) {
        setSteps([]);
        return;
      }
      setLoadingSteps(true);
      setError(null);
      try {
        const list = await api.getAgentRunSteps(selectedRunId);
        setSteps(list);
      } catch (err: any) {
        setError(err?.message || String(err));
      } finally {
        setLoadingSteps(false);
      }
    };
    loadSteps();
  }, [selectedRunId]);

  const formatTime = (ts?: string) => {
    if (!ts) return '-';
    try {
      return new Date(ts).toLocaleString();
    } catch {
      return ts;
    }
  };

  const selectedRun = runs.find(r => r.id === selectedRunId) || null;

  const runGitStatus = async () => {
    if (!workspacePath) return;
    try {
      const result = await api.executeCommand('git status --short --branch', workspacePath);
      setGitStatus(result.stdout || result.stderr || '(no output)');
    } catch (err: any) {
      setGitStatus(`Error: ${err?.message || String(err)}`);
    }
  };

  const runGitDiff = async () => {
    if (!workspacePath) return;
    try {
      const result = await api.executeCommand('git diff', workspacePath);
      setGitDiff(result.stdout || result.stderr || '(no diff)');
    } catch (err: any) {
      setGitDiff(`Error: ${err?.message || String(err)}`);
    }
  };

  return (
    <div style={{ padding: '16px', height: '100%', display: 'flex', flexDirection: 'column', gap: '12px', color: 'var(--text-primary)' }}>
      <h1 style={{ margin: 0, fontSize: '20px', color: 'var(--text-primary)' }}>Agent Runs</h1>
      <p style={{ margin: 0, color: 'var(--text-secondary)', fontSize: '13px' }}>
        Inspect recent coding agent runs, their status, and the steps they performed.
      </p>

      {error && (
        <div
          style={{
            padding: '8px 12px',
            border: '1px solid var(--error-text)',
            borderRadius: '4px',
            background: 'var(--error-bg)',
            color: 'var(--error-text)',
            fontSize: '13px',
          }}
        >
          {error}
        </div>
      )}

      <div style={{ flex: 1, display: 'flex', gap: '12px', minHeight: 0 }}>
        {/* Runs list */}
        <div
          style={{
            width: 360,
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            background: 'var(--card-bg)',
            display: 'flex',
            flexDirection: 'column',
            minHeight: 0,
            color: 'var(--text-primary)',
          }}
        >
          <div
            style={{
              padding: '8px 10px',
              borderBottom: '1px solid var(--border-color)',
              fontSize: '12px',
              fontWeight: 600,
              textTransform: 'uppercase',
              color: 'var(--text-secondary)',
            }}
          >
            Runs
          </div>
          <div style={{ flex: 1, overflow: 'auto' }}>
            {loadingRuns && (
              <div style={{ padding: '10px', fontSize: '12px', color: 'var(--text-secondary)' }}>Loading runs...</div>
            )}
            {!loadingRuns && runs.length === 0 && (
              <div style={{ padding: '10px', fontSize: '12px', color: 'var(--text-secondary)' }}>
                No agent runs yet. Use Panther Coder Agent Mode to start one.
              </div>
            )}
            {runs.map(run => {
              const isSelected = run.id === selectedRunId;
              const statusColor =
                run.status === 'complete'
                  ? '#4caf50'
                  : run.status === 'running'
                  ? '#2196f3'
                  : run.status === 'failed'
                  ? '#f44336'
                  : '#ff9800';
              return (
                <button
                  key={run.id}
                  onClick={() => setSelectedRunId(run.id)}
                  style={{
                    display: 'block',
                    width: '100%',
                    textAlign: 'left',
                    padding: '8px 10px',
                    border: 'none',
                    borderBottom: '1px solid var(--border-color)',
                    background: isSelected ? 'var(--bg-secondary)' : 'transparent',
                    cursor: 'pointer',
                    color: 'var(--text-primary)',
                  }}
                >
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}>
                    <div
                      style={{
                        flex: 1,
                        minWidth: 0,
                        fontSize: '13px',
                        fontWeight: 500,
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {run.task_description}
                    </div>
                    <span
                      style={{
                        fontSize: '11px',
                        padding: '2px 6px',
                        borderRadius: '10px',
                        background: statusColor,
                        color: 'white',
                      }}
                    >
                      {run.status}
                    </span>
                  </div>
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '2px' }}>
                    {formatTime(run.created_at)}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Run details */}
        <div
          style={{
            flex: 1,
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            background: 'var(--card-bg)',
            display: 'flex',
            flexDirection: 'column',
            minHeight: 0,
            color: 'var(--text-primary)',
          }}
        >
          <div
            style={{
              padding: '8px 10px',
              borderBottom: '1px solid var(--border-color)',
              fontSize: '12px',
              fontWeight: 600,
              textTransform: 'uppercase',
              color: 'var(--text-secondary)',
            }}
          >
            Run details
          </div>

          {!selectedRun && (
            <div style={{ padding: '12px', fontSize: '12px', color: 'var(--text-secondary)' }}>
              Select a run on the left to see details.
            </div>
          )}

          {selectedRun && (
            <div style={{ padding: '10px 12px', borderBottom: '1px solid var(--border-color)', fontSize: '13px', color: 'var(--text-primary)' }}>
              <div style={{ marginBottom: '6px' }}>
                <strong>Task:</strong> {selectedRun.task_description}
              </div>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '12px', fontSize: '12px' }}>
                <div>
                  <strong>Status:</strong> {selectedRun.status}
                </div>
                {selectedRun.provider_id && (
                  <div>
                    <strong>Provider:</strong> {selectedRun.provider_id}
                  </div>
                )}
                {selectedRun.model_name && (
                  <div>
                    <strong>Model:</strong> {selectedRun.model_name}
                  </div>
                )}
                <div>
                  <strong>Created:</strong> {formatTime(selectedRun.created_at)}
                </div>
                <div>
                  <strong>Started:</strong> {formatTime(selectedRun.started_at)}
                </div>
                <div>
                  <strong>Finished:</strong> {formatTime(selectedRun.finished_at)}
                </div>
              </div>
              {selectedRun.error_text && (
                <div
                  style={{
                    marginTop: '6px',
                    padding: '6px 8px',
                    borderRadius: '4px',
                    border: '1px solid var(--error-text)',
                    background: 'var(--error-bg)',
                    color: 'var(--error-text)',
                    fontSize: '12px',
                    whiteSpace: 'pre-wrap',
                  }}
                >
                  {selectedRun.error_text}
                </div>
              )}
              {workspacePath && (
                <div style={{ marginTop: '8px', display: 'flex', flexWrap: 'wrap', gap: '8px', fontSize: '12px' }}>
                  <button
                    onClick={runGitStatus}
                    style={{
                      padding: '4px 8px',
                      borderRadius: '4px',
                      border: '1px solid var(--border-color)',
                      background: 'var(--bg-primary)',
                      cursor: 'pointer',
                    }}
                  >
                    Git status
                  </button>
                  <button
                    onClick={runGitDiff}
                    style={{
                      padding: '4px 8px',
                      borderRadius: '4px',
                      border: '1px solid var(--border-color)',
                      background: 'var(--bg-primary)',
                      cursor: 'pointer',
                    }}
                  >
                    Git diff
                  </button>
                  <span style={{ color: 'var(--text-secondary)' }}>
                    Uses local git in workspace: {workspacePath}
                  </span>
                </div>
              )}
            </div>
          )}

          {/* Steps */}
          <div style={{ flex: 1, overflow: 'auto', padding: '8px 12px', color: 'var(--text-primary)' }}>
            {selectedRun && loadingSteps && (
              <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Loading steps...</div>
            )}
            {selectedRun && !loadingSteps && steps.length === 0 && (
              <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
                No steps recorded for this run.
              </div>
            )}
            {steps.map(step => (
              <div
                key={step.id}
                style={{
                  padding: '6px 8px',
                  borderRadius: '4px',
                  border: '1px solid var(--border-color)',
                  marginBottom: '6px',
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: '12px' }}>
                  <div>
                    <strong>{step.step_type}</strong> #{step.step_index}
                    {step.tool_name && <> • {step.tool_name}</>}
                  </div>
                  <div style={{ color: 'var(--text-secondary)', fontSize: '11px' }}>
                    {formatTime(step.created_at)}
                  </div>
                </div>
                {step.description && (
                  <div style={{ fontSize: '12px', marginTop: '2px', color: 'var(--text-primary)' }}>{step.description}</div>
                )}
                {step.result_summary && (
                  <div
                    style={{
                      fontSize: '11px',
                      marginTop: '4px',
                      color: 'var(--text-secondary)',
                      whiteSpace: 'pre-wrap',
                    }}
                  >
                    {step.result_summary}
                  </div>
                )}
              </div>
            ))}

            {(gitStatus || gitDiff) && (
              <div style={{ marginTop: '8px' }}>
                {gitStatus && (
                  <div
                    style={{
                      marginBottom: '6px',
                      padding: '6px 8px',
                      borderRadius: '4px',
                      border: '1px solid var(--border-color)',
                      background: 'var(--bg-primary)',
                      fontSize: '11px',
                      whiteSpace: 'pre-wrap',
                      fontFamily: 'Consolas, Monaco, \"Courier New\", monospace',
                      color: 'var(--text-primary)',
                    }}
                  >
                    <strong>git status</strong>
                    <br />
                    {gitStatus}
                  </div>
                )}
                {gitDiff && (
                  <div
                    style={{
                      padding: '6px 8px',
                      borderRadius: '4px',
                      border: '1px solid var(--border-color)',
                      background: 'var(--bg-primary)',
                      fontSize: '11px',
                      whiteSpace: 'pre-wrap',
                      fontFamily: 'Consolas, Monaco, \"Courier New\", monospace',
                      color: 'var(--text-primary)',
                    }}
                  >
                    <strong>git diff</strong>
                    <br />
                    {gitDiff}
                  </div>
                )}
                <div style={{ marginTop: '4px', fontSize: '11px', color: 'var(--text-secondary)' }}>
                  To create a PR with GitHub CLI (if installed), you can run in this repo:
                  <br />
                  <code style={{ color: 'var(--text-primary)' }}>gh pr create --fill</code>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

