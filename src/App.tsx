import { BrowserRouter, Routes, Route, Navigate, useNavigate } from 'react-router-dom';
import { useEffect, useState, lazy, Suspense } from 'react';
import { Home } from './pages/Home';
import { Providers } from './pages/Providers';
import { Profiles } from './pages/Profiles';
import { Sessions } from './pages/Sessions';
import { Projects } from './pages/Projects';
import { SessionBuilder } from './pages/SessionBuilder';
import { ParallelBrainstorm } from './pages/ParallelBrainstorm';
import { DebateRoom } from './pages/DebateRoom';
import { Compare } from './pages/Compare';
import { ProfileChat } from './pages/ProfileChat';
import { ProjectTraining } from './pages/ProjectTraining';
// Lazy-load Monaco-heavy pages so the main bundle stays smaller
const SimpleCoder = lazy(() => import('./pages/SimpleCoder').then((m) => ({ default: m.SimpleCoder })));
const AgentRuns = lazy(() => import('./pages/AgentRuns').then((m) => ({ default: m.AgentRuns })));
import { PrivacyModal, DependenciesModal, OllamaModal, TrainingCacheModal, TokenUsageModal, VoiceModal } from './pages/Settings';
import { MenuBar } from './components/MenuBar';
import { AuthModal } from './components/AuthModal';
import { ErrorBoundary } from './components/ErrorBoundary';
import { useAppStore } from './store';
import { api, checkBackendReachable } from './api';
import { isRTL } from './i18n';
import { isTauri } from './utils/tauri';
import './App.css';

// Authentication is now optional - no protected routes

function AppContent() {
  const _navigate = useNavigate(); // Keep for potential future use
  const { isAuthenticated, setUser, theme, language } = useAppStore();
  const userId = localStorage.getItem('userId');
  const [authModalOpen, setAuthModalOpen] = useState(false);
  const [authModalMode, setAuthModalMode] = useState<'login' | 'signup'>('login');
  const [showBrowserWarning, setShowBrowserWarning] = useState(false);
  const [backendDown, setBackendDown] = useState(false);
  const [privacyModalOpen, setPrivacyModalOpen] = useState(false);
  const [dependenciesModalOpen, setDependenciesModalOpen] = useState(false);
  const [ollamaModalOpen, setOllamaModalOpen] = useState(false);
  const [trainingCacheModalOpen, setTrainingCacheModalOpen] = useState(false);
  const [tokenUsageModalOpen, setTokenUsageModalOpen] = useState(false);
  const [voiceModalOpen, setVoiceModalOpen] = useState(false);

  // Check if running in browser mode - now a supported mode with HTTP backend
  useEffect(() => {
    if (!isTauri() && typeof window !== 'undefined') {
      const dismissed = localStorage.getItem('browserWarningDismissed');
      if (!dismissed) setShowBrowserWarning(true);
      // Check if HTTP backend is running
      checkBackendReachable().then(ok => setBackendDown(!ok));
    }
  }, []);

  useEffect(() => {
    // Apply theme on mount
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    // Apply language and RTL on mount
    document.documentElement.setAttribute('lang', language);
    document.documentElement.setAttribute('dir', isRTL(language as any) ? 'rtl' : 'ltr');
  }, [language]);

  useEffect(() => {
    // Check for stored user session on app load
    if (userId && !isAuthenticated) {
      api.getCurrentUser(userId)
        .then(user => {
          setUser(user);
        })
        .catch(() => {
          localStorage.removeItem('userId');
        });
    }
  }, [isAuthenticated, userId, setUser]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        switch (e.key) {
          case 'n':
            e.preventDefault();
            if (isAuthenticated) window.location.href = '/session-builder';
            break;
          case 'o':
            e.preventDefault();
            if (isAuthenticated) window.location.href = '/sessions';
            break;
          case 's':
            e.preventDefault();
            // Save functionality
            break;
          case 'f':
            e.preventDefault();
            // Find functionality
            break;
          case 'e':
            e.preventDefault();
            // Export functionality
            break;
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isAuthenticated]);

  // Listen for auth modal open events from menu bar
  useEffect(() => {
    const handleOpenAuth = (e: Event) => {
      const customEvent = e as CustomEvent;
      const mode = customEvent.detail?.mode || 'login';
      setAuthModalMode(mode);
      setAuthModalOpen(true);
    };
    
    window.addEventListener('openAuthModal', handleOpenAuth);
    return () => window.removeEventListener('openAuthModal', handleOpenAuth);
  }, []);

  // Listen for settings modal open events
  useEffect(() => {
    const handleOpenSettings = (e: any) => {
      const tab = e.detail?.tab;
      if (tab === 'privacy') {
        setPrivacyModalOpen(true);
      } else if (tab === 'dependencies') {
        setDependenciesModalOpen(true);
      } else if (tab === 'ollama') {
        setOllamaModalOpen(true);
      } else if (tab === 'training-cache') {
        setTrainingCacheModalOpen(true);
      } else if (tab === 'token-usage') {
        setTokenUsageModalOpen(true);
      } else if (tab === 'voice') {
        setVoiceModalOpen(true);
      }
    };
    
    window.addEventListener('openSettingsModal', handleOpenSettings);
    return () => window.removeEventListener('openSettingsModal', handleOpenSettings);
  }, []);

  return (
      <ErrorBoundary>
        <div className="app">
          {backendDown && !isTauri() && (
            <div style={{
              background: '#f8d7da',
              border: '1px solid #f5c6cb',
              padding: '12px 20px',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              zIndex: 10001,
              position: 'relative'
            }}>
              <div style={{ flex: 1 }}>
                <strong style={{ color: '#721c24' }}>Backend not running</strong>
                <span style={{ marginLeft: '10px', fontSize: '14px', color: '#721c24' }}>
                  Start the HTTP server to use the app in browser mode. Run in project root:
                </span>
                <code style={{
                  marginLeft: '10px',
                  background: '#fff',
                  padding: '4px 8px',
                  borderRadius: '4px',
                  fontSize: '13px',
                  color: '#721c24'
                }}>
                  npm run dev:browser
                </code>
              </div>
            </div>
          )}
          {showBrowserWarning && (
            <div style={{
              background: '#fff3cd',
              border: '1px solid #ffc107',
              padding: '12px 20px',
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              zIndex: 10000,
              position: 'relative'
            }}>
              <div style={{ flex: 1 }}>
                <strong>Browser Mode</strong>
                <span style={{ marginLeft: '10px', fontSize: '14px' }}>
                  Running with HTTP backend. Some features like system stats and file operations require the desktop app.
                </span>
              </div>
              <div style={{ display: 'flex', gap: '10px', alignItems: 'center' }}>
                <button
                  onClick={() => {
                    localStorage.setItem('browserWarningDismissed', 'true');
                    setShowBrowserWarning(false);
                  }}
                  style={{
                    background: 'transparent',
                    border: '1px solid #856404',
                    borderRadius: '4px',
                    padding: '4px 12px',
                    cursor: 'pointer',
                    fontSize: '12px',
                    color: '#856404'
                  }}
                >
                  Don't show again
                </button>
              </div>
            </div>
          )}
          <MenuBar />
          <div style={{
            height: `calc(100vh - 40px - ${backendDown ? 52 : 0}px - ${showBrowserWarning ? 52 : 0}px)`
          }}>
            <main className="main-content" style={{ width: '100%', height: '100%', overflow: 'auto' }}>
              <Suspense fallback={<div className="main-content" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: 200 }}>Loading…</div>}>
              <Routes>
                <Route path="/" element={<Navigate to="/home" replace />} />
                <Route path="/home" element={<Home />} />
                <Route path="/coder" element={<SimpleCoder />} />
                <Route path="/simple-coder" element={<SimpleCoder />} />
                <Route path="/providers" element={<Providers />} />
                <Route path="/profiles" element={<Profiles />} />
                <Route path="/sessions" element={<Sessions />} />
                <Route path="/projects" element={<Projects />} />
                <Route path="/session-builder" element={<SessionBuilder />} />
                <Route path="/parallel-brainstorm/:runId" element={<ParallelBrainstorm />} />
                <Route path="/debate-room/:runId" element={<DebateRoom />} />
                <Route path="/compare/:runId" element={<Compare />} />
                <Route path="/profile-chat/:profileId" element={<ProfileChat />} />
                <Route path="/project-training/:projectId" element={<ProjectTraining />} />
                <Route path="/agent-runs" element={<AgentRuns />} />
              </Routes>
              </Suspense>
            </main>
            
            <AuthModal
              isOpen={authModalOpen}
              onClose={() => setAuthModalOpen(false)}
              initialMode={authModalMode}
            />
            {privacyModalOpen && (
              <PrivacyModal onClose={() => setPrivacyModalOpen(false)} />
            )}
            {dependenciesModalOpen && (
              <DependenciesModal onClose={() => setDependenciesModalOpen(false)} />
            )}
            {ollamaModalOpen && (
              <OllamaModal onClose={() => setOllamaModalOpen(false)} />
            )}
            {trainingCacheModalOpen && (
              <TrainingCacheModal onClose={() => setTrainingCacheModalOpen(false)} />
            )}
            {tokenUsageModalOpen && (
              <TokenUsageModal onClose={() => setTokenUsageModalOpen(false)} />
            )}
            {voiceModalOpen && (
              <VoiceModal onClose={() => setVoiceModalOpen(false)} />
            )}
          </div>
        </div>
      </ErrorBoundary>
  );
}

function App() {
  return (
    <BrowserRouter
      future={{
        v7_startTransition: true,
        v7_relativeSplatPath: true,
      }}
    >
      <AppContent />
    </BrowserRouter>
  );
}

export default App;
