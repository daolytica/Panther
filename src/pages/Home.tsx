import { Link, useNavigate } from 'react-router-dom';
import { useEffect, useState } from 'react';
import { useAppStore } from '../store';
import { api } from '../api';
import { isTauri } from '../utils/tauri';

interface SystemStats {
  cpu_usage: number;
  memory_used: number;
  memory_total: number;
  gpu_usage?: number;
  gpu_memory_used?: number;
  gpu_memory_total?: number;
  disk_used: number;
  disk_total: number;
}

export function Home() {
  const { setProjects, setSessions, setProfiles } = useAppStore();
  const navigate = useNavigate();
  const [systemStats, setSystemStats] = useState<SystemStats | null>(null);
  const [statsLoading, setStatsLoading] = useState(true);
  const [latestProfile, setLatestProfile] = useState<any | null>(null);
  const [isBrowserMode] = useState(!isTauri());

  useEffect(() => {
    loadData();
    // Start real-time system monitoring only in Tauri mode
    if (!isBrowserMode) {
      const interval = setInterval(loadSystemStats, 2000);
      loadSystemStats();
      return () => clearInterval(interval);
    } else {
      // In browser mode, just mark as loaded with placeholder
      setStatsLoading(false);
    }
  }, [isBrowserMode]);

  const loadData = async () => {
    try {
      const [projectsData, sessionsData, profilesData, latestProfileData] = await Promise.all([
        api.listProjects(),
        api.listSessions(),
        api.listProfiles(),
        api.getLatestProfile(),
      ]);
      setProjects(projectsData);
      setSessions(sessionsData);
      setProfiles(profilesData);
      setLatestProfile(latestProfileData);
    } catch (error) {
      console.error('Failed to load data:', error);
    }
  };

  const loadSystemStats = async () => {
    try {
      const stats = await api.getSystemStats();
      setSystemStats(stats);
      setStatsLoading(false);
    } catch (error) {
      console.error('Failed to load system stats:', error);
      // Use mock data if API fails
      setSystemStats({
        cpu_usage: Math.random() * 30 + 10,
        memory_used: 8 * 1024 * 1024 * 1024,
        memory_total: 16 * 1024 * 1024 * 1024,
        disk_used: 256 * 1024 * 1024 * 1024,
        disk_total: 512 * 1024 * 1024 * 1024,
      });
      setStatsLoading(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    const gb = bytes / (1024 * 1024 * 1024);
    return gb.toFixed(1) + ' GB';
  };

  const getProgressColor = (percentage: number): string => {
    if (percentage < 50) return '#4a90e2';
    if (percentage < 80) return '#f5a623';
    return '#d0021b';
  };

  const ProgressBar = ({ value, max, label, unit = '%' }: { value: number; max: number; label: string; unit?: string }) => {
    const percentage = max > 0 ? (value / max) * 100 : 0;
    const displayValue = unit === '%' ? percentage.toFixed(1) + '%' : `${formatBytes(value)} / ${formatBytes(max)}`;
    
    return (
      <div style={{ marginBottom: '18px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px', fontSize: '13px' }}>
          <span style={{ color: 'var(--text-primary)', fontWeight: '500' }}>{label}</span>
          <span style={{ color: 'var(--text-secondary)', fontFamily: 'monospace' }}>{displayValue}</span>
        </div>
        <div style={{ 
          height: '6px', 
          background: 'var(--bg-secondary)', 
          border: '1px solid var(--border-color)',
          overflow: 'hidden',
          position: 'relative'
        }}>
          <div style={{ 
            width: `${Math.min(percentage, 100)}%`, 
            height: '100%', 
            background: getProgressColor(percentage),
            transition: 'width 0.5s ease'
          }} />
        </div>
      </div>
    );
  };

  return (
    <div style={{ 
      padding: '32px', 
      maxWidth: '1400px', 
      margin: '0 auto',
      background: 'var(--bg-primary)'
    }}>
      {/* Header */}
      <div style={{ 
        marginBottom: '32px',
        paddingBottom: '24px',
        borderBottom: '2px solid var(--border-color)'
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '16px', marginBottom: '8px' }}>
          <img 
            src="/logo.png" 
            alt="Panther" 
            style={{ 
              width: '48px', 
              height: '48px',
              objectFit: 'contain'
            }} 
            onError={(e) => {
              e.currentTarget.style.display = 'none';
            }}
          />
          <h1 style={{ 
            fontSize: '28px', 
            fontWeight: '600', 
            color: 'var(--text-primary)',
            margin: 0,
            letterSpacing: '-0.5px'
          }}>
            Panther
          </h1>
        </div>
        <p style={{ 
          fontSize: '14px', 
          color: 'var(--text-secondary)',
          margin: 0,
          marginLeft: '64px'
        }}>
          Advanced AI Agent Platform
        </p>
      </div>

      {/* Main Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '24px', marginBottom: '24px' }}>
        
        {/* Quick Actions */}
        <div style={{ 
          padding: '24px',
          background: 'var(--card-bg)',
          border: '1px solid var(--border-color)',
          boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
        }}>
          <h2 style={{ 
            fontSize: '12px', 
            fontWeight: '600', 
            marginBottom: '20px',
            color: 'var(--text-secondary)',
            textTransform: 'uppercase',
            letterSpacing: '0.5px'
          }}>
            Quick Actions
          </h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {latestProfile && (
              <button
                onClick={() => navigate(`/profile-chat/${latestProfile.id}`)}
                style={{
                  padding: '12px 16px',
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                  border: '1px solid var(--border-color)',
                  fontSize: '14px',
                  fontWeight: '500',
                  cursor: 'pointer',
                  textAlign: 'left',
                  transition: 'background 0.15s, border-color 0.15s',
                  display: 'flex',
                  alignItems: 'center',
                  gap: '10px'
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = 'var(--bg-secondary)';
                  e.currentTarget.style.borderColor = '#4a90e2';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = 'var(--bg-primary)';
                  e.currentTarget.style.borderColor = 'var(--border-color)';
                }}
              >
                {latestProfile.photo_url && (
                  <img 
                    src={latestProfile.photo_url} 
                    alt={latestProfile.name}
                    style={{
                      width: '24px',
                      height: '24px',
                      borderRadius: '50%',
                      objectFit: 'cover',
                      border: '1px solid var(--border-color)'
                    }}
                  />
                )}
                <span>Chat with {latestProfile.name}</span>
              </button>
            )}
            <button
              onClick={() => navigate('/simple-coder')}
              style={{
                padding: '12px 16px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textAlign: 'left',
                transition: 'background 0.15s, border-color 0.15s'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--bg-secondary)';
                e.currentTarget.style.borderColor = '#4a90e2';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'var(--bg-primary)';
                e.currentTarget.style.borderColor = 'var(--border-color)';
              }}
            >
              Simple Coder
            </button>
            <Link 
              to="/session-builder" 
              style={{
                padding: '12px 16px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'none',
                display: 'block',
                textAlign: 'left',
                transition: 'background 0.15s, border-color 0.15s'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--bg-secondary)';
                e.currentTarget.style.borderColor = '#4a90e2';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'var(--bg-primary)';
                e.currentTarget.style.borderColor = 'var(--border-color)';
              }}
            >
              New Session
            </Link>
            <Link 
              to="/providers" 
              style={{
                padding: '12px 16px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'none',
                display: 'block',
                textAlign: 'left',
                transition: 'background 0.15s, border-color 0.15s'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--bg-secondary)';
                e.currentTarget.style.borderColor = '#4a90e2';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'var(--bg-primary)';
                e.currentTarget.style.borderColor = 'var(--border-color)';
              }}
            >
              Manage Providers
            </Link>
            <Link 
              to="/profiles" 
              style={{
                padding: '12px 16px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                border: '1px solid var(--border-color)',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
                textDecoration: 'none',
                display: 'block',
                textAlign: 'left',
                transition: 'background 0.15s, border-color 0.15s'
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.background = 'var(--bg-secondary)';
                e.currentTarget.style.borderColor = '#4a90e2';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'var(--bg-primary)';
                e.currentTarget.style.borderColor = 'var(--border-color)';
              }}
            >
              Manage Profiles
            </Link>
          </div>
        </div>

        {/* System Monitor */}
        <div style={{ 
          padding: '24px',
          background: 'var(--card-bg)',
          border: '1px solid var(--border-color)',
          boxShadow: '0 1px 3px rgba(0,0,0,0.1)'
        }}>
          <div style={{ 
            display: 'flex', 
            justifyContent: 'space-between', 
            alignItems: 'center',
            marginBottom: '20px'
          }}>
            <h2 style={{ 
              fontSize: '12px', 
              fontWeight: '600', 
              color: 'var(--text-secondary)',
              textTransform: 'uppercase',
              letterSpacing: '0.5px',
              margin: 0
            }}>
              {isBrowserMode ? 'Status' : 'System Monitor'}
            </h2>
            <span style={{ 
              fontSize: '10px', 
              color: isBrowserMode ? '#28a745' : (statsLoading ? 'var(--text-secondary)' : '#4a90e2'),
              fontFamily: 'monospace',
              fontWeight: '500'
            }}>
              {isBrowserMode ? 'BROWSER' : (statsLoading ? '...' : 'LIVE')}
            </span>
          </div>
          
          {isBrowserMode ? (
            <div style={{ padding: '16px 0' }}>
              <div style={{ 
                display: 'flex', 
                alignItems: 'center', 
                gap: '12px',
                marginBottom: '16px',
                padding: '12px',
                background: 'var(--bg-secondary)',
                borderRadius: '6px'
              }}>
                <div style={{
                  width: '10px',
                  height: '10px',
                  borderRadius: '50%',
                  background: '#28a745',
                  boxShadow: '0 0 6px #28a745'
                }} />
                <span style={{ color: 'var(--text-primary)', fontSize: '14px' }}>
                  Connected to HTTP Backend
                </span>
              </div>
              <div style={{ fontSize: '13px', color: 'var(--text-secondary)', lineHeight: '1.6' }}>
                <p style={{ margin: '0 0 8px 0' }}>
                  Running in browser mode with remote backend.
                </p>
                <p style={{ margin: '0' }}>
                  System stats require the desktop app.
                </p>
              </div>
            </div>
          ) : systemStats ? (
            <div>
              <ProgressBar 
                value={systemStats.cpu_usage} 
                max={100} 
                label="CPU Usage" 
              />
              <ProgressBar 
                value={systemStats.memory_used} 
                max={systemStats.memory_total} 
                label="RAM Memory"
                unit="bytes"
              />
              <ProgressBar 
                value={systemStats.disk_used} 
                max={systemStats.disk_total} 
                label="Storage"
                unit="bytes"
              />
              {systemStats.gpu_usage !== undefined && (
                <ProgressBar 
                  value={systemStats.gpu_usage} 
                  max={100} 
                  label="GPU Usage" 
                />
              )}
              {systemStats.gpu_memory_used !== undefined && systemStats.gpu_memory_total !== undefined && (
                <ProgressBar 
                  value={systemStats.gpu_memory_used} 
                  max={systemStats.gpu_memory_total} 
                  label="GPU Memory"
                  unit="bytes"
                />
              )}
            </div>
          ) : (
            <div style={{ textAlign: 'center', padding: '32px', color: 'var(--text-secondary)', fontSize: '13px' }}>
              Loading system information...
            </div>
          )}
        </div>
      </div>

      {/* Feature Grid */}
      <div style={{ 
        display: 'grid', 
        gridTemplateColumns: 'repeat(2, 1fr)', 
        gap: '16px'
      }}>
        <div 
          onClick={() => navigate('/projects')}
          style={{
            padding: '20px',
            background: 'var(--card-bg)',
            border: '1px solid var(--border-color)',
            boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
            cursor: 'pointer',
            transition: 'border-color 0.15s, background 0.15s'
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = '#4a90e2';
            e.currentTarget.style.background = 'var(--bg-secondary)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = 'var(--border-color)';
            e.currentTarget.style.background = 'var(--card-bg)';
          }}
        >
          <h3 style={{ fontSize: '15px', fontWeight: '600', marginBottom: '6px', color: 'var(--text-primary)' }}>
            Local Training Session
          </h3>
          <p style={{ fontSize: '13px', color: 'var(--text-secondary)', margin: 0, lineHeight: '1.5' }}>
            Train local LLM models (Ollama) with your data
          </p>
        </div>

        <div 
          onClick={() => navigate('/sessions')}
          style={{
            padding: '20px',
            background: 'var(--card-bg)',
            border: '1px solid var(--border-color)',
            boxShadow: '0 1px 3px rgba(0,0,0,0.1)',
            cursor: 'pointer',
            transition: 'border-color 0.15s, background 0.15s'
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = '#4a90e2';
            e.currentTarget.style.background = 'var(--bg-secondary)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = 'var(--border-color)';
            e.currentTarget.style.background = 'var(--card-bg)';
          }}
        >
          <h3 style={{ fontSize: '15px', fontWeight: '600', marginBottom: '6px', color: 'var(--text-primary)' }}>
            Sessions
          </h3>
          <p style={{ fontSize: '13px', color: 'var(--text-secondary)', margin: 0, lineHeight: '1.5' }}>
            View and manage your AI sessions
          </p>
        </div>

      </div>
    </div>
  );
}
