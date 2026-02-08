import { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null,
    errorInfo: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, errorInfo: null };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    this.setState({
      error,
      errorInfo,
    });
  }

  public render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div style={{
          padding: '40px',
          maxWidth: '800px',
          margin: '50px auto',
          background: 'var(--card-bg)',
          border: '2px solid #f44336',
          borderRadius: '8px',
        }}>
          <h2 style={{ color: '#f44336', marginTop: 0 }}>⚠️ Something went wrong</h2>
          <p style={{ marginBottom: '20px' }}>
            An error occurred in the application. Details below:
          </p>
          
          {this.state.error && (
            <div style={{
              background: '#ffebee',
              padding: '15px',
              borderRadius: '4px',
              marginBottom: '15px',
              fontFamily: 'monospace',
              fontSize: '13px',
              whiteSpace: 'pre-wrap',
              overflow: 'auto',
              maxHeight: '300px',
            }}>
              <strong>Error:</strong> {this.state.error.toString()}
              {this.state.error.stack && (
                <div style={{ marginTop: '10px', fontSize: '11px', color: 'var(--text-secondary)' }}>
                  <strong>Stack:</strong>
                  <pre style={{ margin: '5px 0 0 0', whiteSpace: 'pre-wrap' }}>
                    {this.state.error.stack}
                  </pre>
                </div>
              )}
            </div>
          )}

          {this.state.errorInfo && (
            <details style={{ marginTop: '15px' }}>
              <summary style={{ cursor: 'pointer', color: 'var(--text-secondary)', marginBottom: '10px' }}>
                Component Stack
              </summary>
              <pre style={{
                background: '#f5f5f5',
                padding: '10px',
                borderRadius: '4px',
                fontSize: '11px',
                overflow: 'auto',
                maxHeight: '200px',
              }}>
                {this.state.errorInfo.componentStack}
              </pre>
            </details>
          )}

          <div style={{ marginTop: '20px', display: 'flex', gap: '10px' }}>
            <button
              onClick={() => {
                this.setState({ hasError: false, error: null, errorInfo: null });
                window.location.reload();
              }}
              style={{
                padding: '10px 20px',
                background: '#2196F3',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              🔄 Reload App
            </button>
            <button
              onClick={() => {
                this.setState({ hasError: false, error: null, errorInfo: null });
              }}
              style={{
                padding: '10px 20px',
                background: '#4CAF50',
                color: 'white',
                border: 'none',
                borderRadius: '4px',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              ✨ Try Again
            </button>
          </div>

          <div style={{ marginTop: '20px', padding: '15px', background: '#fff3cd', borderRadius: '4px', fontSize: '13px' }}>
            <strong>💡 Debugging Tips:</strong>
            <ul style={{ margin: '10px 0 0 20px' }}>
              <li>Check the terminal where you ran <code>npm run tauri dev</code> for detailed error logs</li>
              <li>Open browser DevTools (if available) or check the console output</li>
              <li>Try reloading the app or restarting the dev server</li>
            </ul>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
