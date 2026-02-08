// Tauri detection utility

let isTauriAvailable: boolean | null = null;

export async function checkTauriAvailable(): Promise<boolean> {
  if (isTauriAvailable !== null) {
    return isTauriAvailable;
  }
  
  try {
    // Try to access Tauri API
    if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
      isTauriAvailable = true;
      return true;
    }
    
    // Try importing Tauri API
    const tauriCore = await import('@tauri-apps/api/core');
    // If we can import it and invoke exists, Tauri might be available
    if (tauriCore && typeof tauriCore.invoke === 'function') {
      isTauriAvailable = true;
      return true;
    }
    isTauriAvailable = false;
    return false;
  } catch (error) {
    isTauriAvailable = false;
    return false;
  }
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__;
}
