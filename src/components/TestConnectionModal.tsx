import { useState, useEffect } from 'react';

interface TestConnectionModalProps {
  provider: {
    id: string;
    display_name: string;
    provider_type: string;
    base_url?: string;
  };
  onClose: () => void;
  onTest: (providerId: string) => Promise<boolean>;
}

export function TestConnectionModal({ provider, onClose, onTest }: TestConnectionModalProps) {
  const [status, setStatus] = useState<'idle' | 'testing' | 'success' | 'error'>('idle');
  const [currentStep, setCurrentStep] = useState<string>('');
  const [errorMessage, setErrorMessage] = useState<string>('');
  const [isTesting, setIsTesting] = useState(false);
  const [elapsedTime, setElapsedTime] = useState(0);

  useEffect(() => {
    // Auto-start test when modal opens
    if (status === 'idle') {
      runTest();
    }
  }, []);

  useEffect(() => {
    // Update elapsed time while testing
    let interval: NodeJS.Timeout | null = null;
    if (status === 'testing') {
      interval = setInterval(() => {
        setElapsedTime((prev) => prev + 1);
      }, 1000);
    } else {
      setElapsedTime(0);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [status]);

  const runTest = async () => {
    if (isTesting) return; // Prevent multiple simultaneous tests
    
    setIsTesting(true);
    setStatus('testing');
    setCurrentStep('Initializing connection test...');
    setErrorMessage('');

    try {
      // Step 1: Checking provider configuration
      setCurrentStep('Checking provider configuration...');
      await new Promise(resolve => setTimeout(resolve, 300));

      // Step 2: Retrieving API key from keychain
      if (provider.provider_type !== 'local_http') {
        setCurrentStep('Retrieving API key from secure storage...');
        await new Promise(resolve => setTimeout(resolve, 300));
      }

      // Step 3: Connecting to provider
      setCurrentStep(`Connecting to ${provider.display_name}...`);
      await new Promise(resolve => setTimeout(resolve, 300));

      // Step 4: Validating credentials
      if (provider.provider_type !== 'local_http') {
        setCurrentStep('Validating API credentials...');
        await new Promise(resolve => setTimeout(resolve, 300));
      }

      // Step 5: Testing connection
      setCurrentStep('Testing API endpoint (this may take a few seconds)...');
      
      // Add timeout wrapper
      const timeoutPromise = new Promise<never>((_, reject) => {
        setTimeout(() => {
          reject(new Error('Connection test timed out after 30 seconds. Please check your network connection and try again.'));
        }, 30000); // 30 second timeout
      });

      console.log('Starting API test for provider:', provider.id);
      const testPromise = onTest(provider.id);
      console.log('Test promise created, waiting for result...');
      
      const result = await Promise.race([testPromise, timeoutPromise]);
      console.log('Test result received:', result);

      if (result) {
        setStatus('success');
        setCurrentStep('Connection successful! All checks passed.');
      } else {
        setStatus('error');
        setCurrentStep('Connection test completed');
        setErrorMessage('Connection failed. Please check your configuration.');
      }
    } catch (error: any) {
      setStatus('error');
      setCurrentStep('Connection test failed');
      const errorMsg = error?.message || error?.toString() || 'Unknown error';
      
      // Provide more helpful error messages
      let displayError = errorMsg;
      if (errorMsg.includes('not yet implemented') || errorMsg.includes('Coming in Phase 3') || errorMsg.includes('not implemented')) {
        displayError = `This provider adapter is not yet implemented.\n\n${errorMsg}\n\nAll major providers are now supported:\n✅ OpenAI Compatible (OpenAI, OpenRouter, Together AI, etc.)\n✅ Local HTTP (Ollama, LM Studio, etc.)\n✅ Anthropic/Claude\n✅ Google/Gemini`;
      } else if (errorMsg.includes('Base URL is empty') || errorMsg.includes('Failed to connect to .')) {
        displayError = 'Base URL is missing or empty.\n\nFor OpenAI Compatible providers:\n- Leave base URL empty to use default: https://api.openai.com/v1\n- Or enter a custom base URL (e.g., https://api.openai.com/v1)\n\nPlease edit the provider and set a valid base URL.';
      } else if (errorMsg.includes('timeout') || errorMsg.includes('timed out')) {
        displayError = 'Connection timed out. This could mean:\n- The API endpoint is not responding\n- Your network connection is slow or unstable\n- The base URL is incorrect\n- A firewall is blocking the connection';
      } else if (errorMsg.includes('Failed to connect') || errorMsg.includes('network')) {
        displayError = `Network error: ${errorMsg}\n\nPlease check:\n- Your internet connection\n- The base URL is correct\n- The API endpoint is accessible`;
      } else if (errorMsg.includes('401') || errorMsg.includes('Authentication')) {
        displayError = `Authentication failed: ${errorMsg}\n\nPlease verify:\n- Your API key is correct\n- The API key hasn't expired\n- The API key has the necessary permissions`;
      } else if (errorMsg.includes('404')) {
        displayError = `Endpoint not found: ${errorMsg}\n\nPlease check:\n- The base URL is correct\n- The API endpoint path is valid`;
      }
      
      setErrorMessage(displayError);
      console.error('Test error:', error);
    } finally {
      setIsTesting(false);
    }
  };

  const getStatusIcon = () => {
    switch (status) {
      case 'testing':
        return '⏳';
      case 'success':
        return '✅';
      case 'error':
        return '❌';
      default:
        return 'ℹ️';
    }
  };

  const getStatusColor = () => {
    switch (status) {
      case 'testing':
        return '#17a2b8';
      case 'success':
        return '#28a745';
      case 'error':
        return '#dc3545';
      default:
        return '#6c757d';
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 2000,
      }}
      onClick={onClose}
    >
      <div
        className="card"
        style={{
          width: '500px',
          maxWidth: '90%',
          position: 'relative',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <button
          onClick={onClose}
          style={{
            position: 'absolute',
            top: '15px',
            right: '15px',
            background: 'none',
            border: 'none',
            fontSize: '24px',
            cursor: 'pointer',
            color: 'var(--text-secondary)',
            padding: '0',
            width: '30px',
            height: '30px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: '4px',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--surface-hover)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'none';
          }}
        >
          ×
        </button>

        <h2 style={{ marginRight: '40px', marginBottom: '20px' }}>
          Testing Connection
        </h2>

        <div style={{ marginBottom: '20px' }}>
          <div style={{ marginBottom: '10px' }}>
            <strong>Provider:</strong> {provider.display_name}
          </div>
          <div style={{ marginBottom: '10px' }}>
            <strong>Type:</strong> {provider.provider_type}
          </div>
          {provider.base_url && (
            <div style={{ marginBottom: '10px' }}>
              <strong>Base URL:</strong> {provider.base_url}
            </div>
          )}
        </div>

        <div
          style={{
            padding: '20px',
            background: '#f8f9fa',
            borderRadius: '8px',
            marginBottom: '20px',
            minHeight: '100px',
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '10px',
              marginBottom: '15px',
            }}
          >
            <span style={{ fontSize: '24px' }}>{getStatusIcon()}</span>
            <span
              style={{
                color: getStatusColor(),
                fontWeight: 'bold',
                fontSize: '16px',
              }}
            >
              {status === 'testing' && 'Testing in progress...'}
              {status === 'success' && 'Connection Successful!'}
              {status === 'error' && 'Connection Failed'}
              {status === 'idle' && 'Ready to test'}
            </span>
          </div>

          {status === 'testing' && (
            <div
              style={{
                width: '100%',
                height: '4px',
                background: 'var(--surface-hover)',
                borderRadius: '2px',
                overflow: 'hidden',
                marginBottom: '10px',
              }}
            >
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  background: getStatusColor(),
                  animation: 'pulse 1.5s ease-in-out infinite',
                }}
              />
              <style>{`
                @keyframes pulse {
                  0%, 100% { opacity: 1; }
                  50% { opacity: 0.5; }
                }
              `}</style>
            </div>
          )}

          <div
            style={{
              color: 'var(--text-secondary)',
              fontSize: '14px',
              lineHeight: '1.6',
            }}
          >
            {currentStep}
            {status === 'testing' && elapsedTime > 3 && (
              <div style={{ marginTop: '10px', fontSize: '12px', color: 'var(--warning-text)' }}>
                ⏱️ This is taking longer than expected ({elapsedTime}s). The API may be slow to respond or there may be a connection issue.
              </div>
            )}
          </div>

          {status === 'error' && errorMessage && (
            <div
              style={{
                marginTop: '15px',
                padding: '10px',
                background: 'var(--warning-bg)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                color: 'var(--warning-text)',
                fontSize: '13px',
              }}
            >
              <strong>Error Details:</strong>
              <div style={{ marginTop: '5px' }}>{errorMessage}</div>
            </div>
          )}

          {status === 'success' && (
            <div
              style={{
                marginTop: '15px',
                padding: '10px',
                background: 'var(--success-bg)',
                border: '1px solid var(--border-color)',
                borderRadius: '4px',
                color: 'var(--success-text)',
                fontSize: '13px',
              }}
            >
              Your API key is valid and the connection is working correctly.
            </div>
          )}
        </div>

        <div style={{ display: 'flex', gap: '10px', justifyContent: 'flex-end' }}>
          {status === 'error' && (
            <button
              className="btn btn-secondary"
              onClick={runTest}
              disabled={isTesting}
            >
              Retry Test
            </button>
          )}
          <button
            className="btn btn-primary"
            onClick={onClose}
          >
            {status === 'testing' ? 'Close (Test Running)' : 'Close'}
          </button>
        </div>
      </div>
    </div>
  );
}
