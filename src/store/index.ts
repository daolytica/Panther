// Zustand store for app state

import { create } from 'zustand';
import type { ProviderAccount, PromptProfile, Project, Session, Run } from '../types';
import { isRTL } from '../i18n';

interface User {
  id: string;
  username: string;
  email: string;
  created_at: string;
  last_login_at?: string;
}

type Theme = 'light' | 'dark';

interface AppState {
  // Authentication
  user: User | null;
  setUser: (user: User | null) => void;
  isAuthenticated: boolean;
  
  // Theme
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  
  // Language
  language: string;
  setLanguage: (lang: string) => void;

  // Providers
  providers: ProviderAccount[];
  setProviders: (providers: ProviderAccount[]) => void;
  addProvider: (provider: ProviderAccount) => void;
  updateProvider: (id: string, provider: Partial<ProviderAccount>) => void;
  removeProvider: (id: string) => void;

  // Profiles
  profiles: PromptProfile[];
  setProfiles: (profiles: PromptProfile[]) => void;
  addProfile: (profile: PromptProfile) => void;
  updateProfile: (id: string, profile: Partial<PromptProfile>) => void;
  removeProfile: (id: string) => void;

  // Projects
  projects: Project[];
  setProjects: (projects: Project[]) => void;
  addProject: (project: Project) => void;
  updateProject: (id: string, project: Partial<Project>) => void;
  removeProject: (id: string) => void;

  // Sessions
  sessions: Session[];
  setSessions: (sessions: Session[]) => void;
  addSession: (session: Session) => void;

  // Current run
  currentRun: Run | null;
  setCurrentRun: (run: Run | null) => void;

  // Voice (TTS/STT)
  voiceEnabled: boolean;
  setVoiceEnabled: (enabled: boolean) => void;
  useLocalVoice: boolean;
  setUseLocalVoice: (use: boolean) => void;
  /** In conversation mode: auto-send when silence detected (hands-free) */
  continuousAutoSend: boolean;
  setContinuousAutoSend: (enabled: boolean) => void;
  /** Auto-speak assistant responses */
  autoSpeakResponses: boolean;
  setAutoSpeakResponses: (enabled: boolean) => void;
  /** Default voice gender for TTS (any, male, female, neutral) */
  voiceGender: 'any' | 'male' | 'female' | 'neutral';
  setVoiceGender: (g: 'any' | 'male' | 'female' | 'neutral') => void;
  /** Default voice URI for TTS (browser speechSynthesis) */
  voiceUri: string;
  setVoiceUri: (uri: string) => void;
}

// Load voice settings from localStorage
const getInitialVoiceEnabled = (): boolean => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('voiceEnabled');
    if (saved === 'false') return false;
    if (saved === 'true') return true;
    return true; // default on
  }
  return true;
};

const getInitialUseLocalVoice = (): boolean => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('useLocalVoice');
    return saved === 'true';
  }
  return false;
};

const getInitialAutoSpeakResponses = (): boolean => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('autoSpeakResponses');
    return saved === 'true';
  }
  return false;
};

const getInitialContinuousAutoSend = (): boolean => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('continuousAutoSend');
    return saved !== 'false'; // default true for hands-free
  }
  return true;
};

const getInitialVoiceGender = (): 'any' | 'male' | 'female' | 'neutral' => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('voiceGender');
    if (saved === 'male' || saved === 'female' || saved === 'neutral') return saved;
  }
  return 'any';
};

const getInitialVoiceUri = (): string => {
  if (typeof window !== 'undefined') {
    return localStorage.getItem('voiceUri') || '';
  }
  return '';
};

// Load theme from localStorage on module load
const getInitialTheme = (): Theme => {
  if (typeof window !== 'undefined') {
    const saved = localStorage.getItem('theme') as Theme;
    if (saved === 'light' || saved === 'dark') {
      return saved;
    }
  }
  return 'light';
};

export const useAppStore = create<AppState>((set, get) => {
  const initialTheme = getInitialTheme();
  const initialLanguage = (() => {
    if (typeof window !== 'undefined') {
      const stored = localStorage.getItem('language');
      return stored || 'en';
    }
    return 'en';
  })();
  
  // Set theme and language attributes on document root
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', initialTheme);
    document.documentElement.setAttribute('lang', initialLanguage);
    document.documentElement.setAttribute('dir', isRTL(initialLanguage as any) ? 'rtl' : 'ltr');
  }
  
  return {
    user: null,
    setUser: (user) => set({ user, isAuthenticated: user !== null }),
    isAuthenticated: false,
    
    theme: initialTheme,
    setTheme: (theme) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('theme', theme);
        document.documentElement.setAttribute('data-theme', theme);
      }
      set({ theme });
    },
    toggleTheme: () => {
      const currentTheme = get().theme;
      const newTheme = currentTheme === 'light' ? 'dark' : 'light';
      get().setTheme(newTheme);
    },
    
    language: initialLanguage,
    setLanguage: (lang: string) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('language', lang);
        document.documentElement.setAttribute('lang', lang);
        document.documentElement.setAttribute('dir', isRTL(lang as any) ? 'rtl' : 'ltr');
      }
      set({ language: lang });
    },

    providers: [],
    setProviders: (providers) => set({ providers }),
    addProvider: (provider) => set((state) => ({ providers: [...state.providers, provider] })),
    updateProvider: (id, updates) => set((state) => ({
      providers: state.providers.map((p) => (p.id === id ? { ...p, ...updates } : p))
    })),
    removeProvider: (id) => set((state) => ({
      providers: state.providers.filter((p) => p.id !== id)
    })),

    profiles: [],
    setProfiles: (profiles) => set({ profiles }),
    addProfile: (profile) => set((state) => ({ profiles: [...state.profiles, profile] })),
    updateProfile: (id, updates) => set((state) => ({
      profiles: state.profiles.map((p) => (p.id === id ? { ...p, ...updates } : p))
    })),
    removeProfile: (id) => set((state) => ({
      profiles: state.profiles.filter((p) => p.id !== id)
    })),

    projects: [],
    setProjects: (projects) => set({ projects }),
    addProject: (project) => set((state) => ({ projects: [...state.projects, project] })),
    updateProject: (id, updates) => set((state) => ({
      projects: state.projects.map((p) => (p.id === id ? { ...p, ...updates } : p))
    })),
    removeProject: (id) => set((state) => ({
      projects: state.projects.filter((p) => p.id !== id)
    })),

    sessions: [],
    setSessions: (sessions) => set({ sessions }),
    addSession: (session) => set((state) => ({ sessions: [...state.sessions, session] })),

    currentRun: null,
    setCurrentRun: (run) => set({ currentRun: run }),

    voiceEnabled: getInitialVoiceEnabled(),
    setVoiceEnabled: (enabled) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('voiceEnabled', String(enabled));
      }
      set({ voiceEnabled: enabled });
    },
    useLocalVoice: getInitialUseLocalVoice(),
    setUseLocalVoice: (use) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('useLocalVoice', String(use));
      }
      set({ useLocalVoice: use });
    },
    continuousAutoSend: getInitialContinuousAutoSend(),
    setContinuousAutoSend: (enabled) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('continuousAutoSend', String(enabled));
      }
      set({ continuousAutoSend: enabled });
    },
    autoSpeakResponses: getInitialAutoSpeakResponses(),
    setAutoSpeakResponses: (enabled) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('autoSpeakResponses', String(enabled));
      }
      set({ autoSpeakResponses: enabled });
    },
    voiceGender: getInitialVoiceGender(),
    setVoiceGender: (g) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('voiceGender', g);
      }
      set({ voiceGender: g });
    },
    voiceUri: getInitialVoiceUri(),
    setVoiceUri: (uri) => {
      if (typeof window !== 'undefined') {
        localStorage.setItem('voiceUri', uri);
      }
      set({ voiceUri: uri });
    },
  };
});
