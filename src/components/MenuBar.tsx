import { useState, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '../store';
import { api } from '../api';

/** Top navigation menu bar */
export function MenuBar() {
  const navigate = useNavigate();
  const { user, setUser, currentRun, theme, toggleTheme } = useAppStore();
  const [activeMenu, setActiveMenu] = useState<string | null>(null);
  const [findDialogOpen, setFindDialogOpen] = useState(false);
  const [findText, setFindText] = useState('');

  const handleLogout = async () => {
    try {
      await api.logout();
      setUser(null);
      localStorage.removeItem('userId');
      navigate('/login');
    } catch (error) {
      console.error('Logout failed:', error);
    }
  };

  const handleNewSession = () => {
    navigate('/session-builder');
    setActiveMenu(null);
  };

  const handleSave = async () => {
    // Save current session/run
    if (currentRun) {
      // Implementation depends on what needs to be saved
      alert('Save functionality - to be implemented');
    }
    setActiveMenu(null);
  };

  const handleSaveAs = () => {
    alert('Save As functionality - to be implemented');
    setActiveMenu(null);
  };

  const handleExport = async () => {
    if (currentRun) {
      try {
        const runStatus = await api.getRunStatus(currentRun.id);
        if (runStatus.session_id) {
          const markdown = await api.exportSessionMarkdown(runStatus.session_id);
          // Create download
          const blob = new Blob([markdown], { type: 'text/markdown' });
          const url = URL.createObjectURL(blob);
          const a = document.createElement('a');
          a.href = url;
          a.download = `session-${runStatus.session_id}.md`;
          a.click();
          URL.revokeObjectURL(url);
        }
      } catch (error) {
        console.error('Export failed:', error);
        alert('Export failed');
      }
    }
    setActiveMenu(null);
  };

  const handleExit = () => {
    if (window.confirm('Are you sure you want to exit?')) {
      window.close();
    }
    setActiveMenu(null);
  };

  const handleFind = () => {
    setFindDialogOpen(true);
    setActiveMenu(null);
  };

  const handleFindNext = () => {
    if (findText) {
      // Simple find implementation
      const selection = window.getSelection();
      if (selection) {
        const range = document.createRange();
        const walker = document.createTreeWalker(
          document.body,
          NodeFilter.SHOW_TEXT,
          null
        );
        
        let node;
        while (node = walker.nextNode()) {
          const text = node.textContent || '';
          const index = text.toLowerCase().indexOf(findText.toLowerCase());
          if (index !== -1) {
            range.setStart(node, index);
            range.setEnd(node, index + findText.length);
            selection.removeAllRanges();
            selection.addRange(range);
            node.parentElement?.scrollIntoView({ behavior: 'smooth', block: 'center' });
            break;
          }
        }
      }
    }
  };

  const MenuItem = ({ label, onClick, disabled }: { label: string; onClick: () => void; disabled?: boolean }) => (
    <div
      onClick={disabled ? undefined : onClick}
      style={{
        padding: '8px 20px',
        cursor: disabled ? 'not-allowed' : 'pointer',
        color: disabled ? 'var(--text-tertiary)' : 'var(--text-primary)',
        fontSize: '14px',
        whiteSpace: 'nowrap',
        backgroundColor: 'transparent',
        transition: 'background-color 0.2s',
      }}
      onMouseEnter={(e) => {
        if (!disabled) e.currentTarget.style.backgroundColor = 'var(--bg-secondary)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.backgroundColor = 'transparent';
      }}
    >
      {label}
    </div>
  );

  const hideTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const HIDE_DELAY_MS = 400;

  const Menu = ({ label, children }: { label: string; children: React.ReactNode }) => {
    const isActive = activeMenu === label;
    return (
      <div
        style={{ position: 'relative', display: 'inline-block' }}
        onMouseEnter={() => {
          if (hideTimeoutRef.current) {
            clearTimeout(hideTimeoutRef.current);
            hideTimeoutRef.current = null;
          }
          setActiveMenu(label);
        }}
        onMouseLeave={() => {
          hideTimeoutRef.current = setTimeout(() => setActiveMenu(null), HIDE_DELAY_MS);
        }}
      >
        <div
          style={{
            padding: '8px 16px',
            cursor: 'pointer',
            fontSize: '14px',
            fontWeight: isActive ? '600' : '400',
            color: isActive ? '#667eea' : 'var(--text-primary)',
            borderBottom: isActive ? '2px solid #667eea' : '2px solid transparent',
            transition: 'all 0.2s',
          }}
        >
          {label}
        </div>
        {isActive && (
          <div
            style={{
              position: 'absolute',
              top: '100%',
              left: 0,
              background: 'var(--card-bg)',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
              zIndex: 1000,
              minWidth: '200px',
              marginTop: '2px',
              transition: 'background-color 0.3s, border-color 0.3s',
            }}
          >
            {children}
          </div>
        )}
      </div>
    );
  };

  return (
    <>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          background: 'var(--card-bg)',
          borderBottom: '1px solid var(--border-color)',
          padding: '0 20px',
          height: '40px',
          boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
          zIndex: 100,
          transition: 'background-color 0.3s, border-color 0.3s',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', flex: 1 }}>
          <button
            onClick={() => { navigate('/home'); setActiveMenu(null); }}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '6px 12px',
              marginRight: '12px',
              fontSize: '13px',
              fontWeight: 500,
              background: 'transparent',
              border: '1px solid var(--border-color)',
              borderRadius: '4px',
              cursor: 'pointer',
              color: 'var(--text-primary)',
            }}
            title="Go to Home"
          >
            🏠 Home
          </button>
          <Menu label="File">
            <MenuItem label="New Session" onClick={handleNewSession} />
            <MenuItem label="Open Session" onClick={() => { navigate('/sessions'); setActiveMenu(null); }} />
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            <MenuItem label="Save" onClick={handleSave} disabled={!currentRun} />
            <MenuItem label="Save As..." onClick={handleSaveAs} disabled={!currentRun} />
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            <MenuItem label="Export Markdown" onClick={handleExport} disabled={!currentRun} />
            <MenuItem label="Export JSON" onClick={async () => {
              if (currentRun) {
                try {
                  const runStatus = await api.getRunStatus(currentRun.id);
                  if (runStatus.session_id) {
                    const json = await api.exportSessionJson(runStatus.session_id);
                    const blob = new Blob([json], { type: 'application/json' });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement('a');
                    a.href = url;
                    a.download = `session-${runStatus.session_id}.json`;
                    a.click();
                    URL.revokeObjectURL(url);
                  }
                } catch (error) {
                  alert('Export failed');
                }
              }
              setActiveMenu(null);
            }} disabled={!currentRun} />
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            <MenuItem label="Exit" onClick={handleExit} />
          </Menu>

          <Menu label="Edit">
            <MenuItem label="Find..." onClick={handleFind} />
            <MenuItem label="Find Next" onClick={handleFindNext} disabled={!findText} />
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            <MenuItem label="Cut" onClick={() => { document.execCommand('cut'); setActiveMenu(null); }} />
            <MenuItem label="Copy" onClick={() => { document.execCommand('copy'); setActiveMenu(null); }} />
            <MenuItem label="Paste" onClick={() => { document.execCommand('paste'); setActiveMenu(null); }} />
          </Menu>

          <Menu label="View">
            <MenuItem label={theme === 'light' ? 'Dark Theme' : 'Light Theme'} onClick={() => { toggleTheme(); setActiveMenu(null); }} />
            <MenuItem label="Agent Runs" onClick={() => { navigate('/agent-runs'); setActiveMenu(null); }} />
          </Menu>

          <Menu label="Settings">
            <MenuItem label="Voice" onClick={() => { 
              window.dispatchEvent(new CustomEvent('openSettingsModal', { detail: { tab: 'voice' } }));
              setActiveMenu(null); 
            }} />
            <MenuItem label="Providers" onClick={() => { navigate('/providers'); setActiveMenu(null); }} />
            <MenuItem label="Local Training Session" onClick={() => { navigate('/projects'); setActiveMenu(null); }} />
            <MenuItem label="Profiles" onClick={() => { navigate('/profiles'); setActiveMenu(null); }} />
            <div style={{ height: '1px', background: 'var(--border-color)', margin: '4px 0' }} />
            <MenuItem label="Privacy" onClick={() => { 
              window.dispatchEvent(new CustomEvent('openSettingsModal', { detail: { tab: 'privacy' } }));
              setActiveMenu(null); 
            }} />
            <MenuItem label="Dependencies" onClick={() => { 
              window.dispatchEvent(new CustomEvent('openSettingsModal', { detail: { tab: 'dependencies' } }));
              setActiveMenu(null); 
            }} />
            <MenuItem label="Ollama" onClick={() => { 
              window.dispatchEvent(new CustomEvent('openSettingsModal', { detail: { tab: 'ollama' } }));
              setActiveMenu(null); 
            }} />
            <MenuItem label="Training Cache" onClick={() => { 
              window.dispatchEvent(new CustomEvent('openSettingsModal', { detail: { tab: 'training-cache' } }));
              setActiveMenu(null); 
            }} />
          </Menu>

          <Menu label="Help">
            <MenuItem label="About" onClick={() => { 
              alert(`Panther v0.1.0\nAdvanced AI Agent Platform\n\nCreated by Reza Mirfayzi`); 
              setActiveMenu(null); 
            }} />
            <MenuItem label="Documentation" onClick={() => { window.open('https://github.com/your-repo/docs', '_blank'); setActiveMenu(null); }} />
          </Menu>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '15px', paddingLeft: '20px', borderLeft: '1px solid var(--border-color)' }}>
          {user ? (
            <>
              <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                {user.username}
              </span>
              <button
                onClick={handleLogout}
                style={{
                  padding: '4px 12px',
                  fontSize: '12px',
                  background: 'transparent',
                  border: '1px solid var(--border-color)',
                  borderRadius: '4px',
                  cursor: 'pointer',
                  color: 'var(--text-secondary)',
                }}
              >
                Logout
              </button>
            </>
          ) : (
              <>
                <button
                  onClick={() => {
                    window.dispatchEvent(new CustomEvent('openAuthModal', { detail: { mode: 'login' } }));
                  }}
                  style={{
                    padding: '4px 12px',
                    fontSize: '12px',
                    background: '#667eea',
                    border: 'none',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    color: 'white',
                  }}
                >
                  Sign In
                </button>
                <button
                  onClick={() => {
                    window.dispatchEvent(new CustomEvent('openAuthModal', { detail: { mode: 'signup' } }));
                  }}
                  style={{
                    padding: '4px 12px',
                    fontSize: '12px',
                    background: 'transparent',
                    border: '1px solid var(--border-color)',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    color: 'var(--text-secondary)',
                  }}
                >
                  Sign Up
                </button>
              </>
          )}
        </div>
      </div>

      {findDialogOpen && (
        <div
          style={{
            position: 'fixed',
            top: '50px',
            right: '20px',
            background: 'var(--card-bg)',
            border: '1px solid var(--border-color)',
            borderRadius: '4px',
            padding: '15px',
            boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
            zIndex: 2000,
            minWidth: '300px',
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <input
              type="text"
              placeholder="Find..."
              value={findText}
              onChange={(e) => setFindText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  handleFindNext();
                } else if (e.key === 'Escape') {
                  setFindDialogOpen(false);
                }
              }}
              autoFocus
              style={{
                flex: 1,
                padding: '6px 10px',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                fontSize: '14px',
              }}
            />
            <button
              onClick={() => setFindDialogOpen(false)}
              style={{
                padding: '6px 12px',
                background: 'var(--surface-hover)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '12px',
              }}
            >
              ✕
            </button>
          </div>
        </div>
      )}
    </>
  );
}
