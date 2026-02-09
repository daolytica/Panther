# Panther v2 – Codebase Reference

Quick reference for frontend, backend, and UI components.

---

## Frontend (src/)

### Entry & routing
| File | Purpose |
|------|---------|
| `main.tsx` | App entry point; mounts React and router |
| `App.tsx` | Root component; defines routes and layout |
| `App.css` | Global styles and CSS variables |
| `vite-env.d.ts` | Vite + TypeScript declarations |

### Pages (src/pages/)
| File | Purpose |
|------|---------|
| `Home.tsx` | Landing/dashboard |
| `Login.tsx` | Login |
| `Signup.tsx` | User registration |
| `Providers.tsx` | LLM provider management (OpenAI, Anthropic, Ollama, hybrid, unrestricted mode) |
| `Profiles.tsx` | List/manage chat profiles |
| `ProfileEditor.tsx` | Create/edit profiles |
| `ProfileChat.tsx` | Chat for a profile |
| `Projects.tsx` | List/manage projects |
| `ProjectTraining.tsx` | Training data & LoRA training |
| `Sessions.tsx` | List sessions (debates, brainstorming) |
| `SessionBuilder.tsx` | Create/configure sessions |
| `DebateRoom.tsx` | Multi-agent debate UI |
| `ParallelBrainstorm.tsx` | Parallel brainstorming UI |
| `AgentRuns.tsx` | Agent run history |
| `Compare.tsx` | Compare LLM outputs side-by-side |
| `Coder.tsx` | Code-focused agent |
| `SimpleCoder.tsx` | Simple code assistant |
| `Settings.tsx` | App settings |

### Components (src/components/)
| File | Purpose |
|------|---------|
| `MenuBar.tsx` | Top navigation |
| `Notification.tsx` | Toast notifications |
| `AuthModal.tsx` | Auth modals |
| `TestConnectionModal.tsx` | Test provider connection |
| `ExportChatModal.tsx` | Export chat |
| `ExportModelModal.tsx` | Export models |
| `ImportTrainingDataModal.tsx` | Import training data |
| `LoraTrainingModal.tsx` | LoRA training config |
| `TrainingDataChat.tsx` | Chat for training data |
| `ResponseCard.tsx` | Single LLM response card |
| `VoiceInput.tsx` | Voice input |
| `VoiceOutput.tsx` | Voice output |
| `VoiceSettings.tsx` | Voice settings |
| `WebSearchModal.tsx` | Web search modal |
| `ErrorBoundary.tsx` | React error boundary |

### Core utilities
| File | Purpose |
|------|---------|
| `api/index.ts` | API client for Tauri backend |
| `store/index.ts` | Zustand store (providers, profiles, etc.) |
| `types/index.ts` | Shared TypeScript types |
| `hooks/useStreamingLLM.ts` | Streaming LLM responses |
| `utils/tauri.ts` | Tauri invoke helpers |
| `utils/audioRecorder.ts` | Voice recording |
| `i18n/index.ts` | i18n setup |
| `voice/types.ts` | Voice types |

---

## Backend (src-tauri/)

| File | Purpose |
|------|---------|
| `main.rs` | Tauri app entry |
| `lib.rs` | Library wiring |
| `commands_provider.rs` | Provider CRUD commands |
| `commands_profile.rs` | Profile commands |
| `commands_session.rs` | Session commands |
| `commands_debate.rs` | Debate session commands |
| `commands_project.rs` | Project commands |
| `commands_training.rs` | Training commands |
| `debate_orchestrator.rs` | Debate orchestration |
| `training_ingest.rs` | Training data ingest |
| (other commands_*.rs) | Other feature commands |

---

## Project root

| File | Purpose |
|------|---------|
| `package.json` | Node dependencies |
| `vite.config.ts` | Vite config |
| `tsconfig.json` | TypeScript config |
| `index.html` | HTML entry |