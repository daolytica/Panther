# Panther Architecture

This document contains Mermaid diagrams showing how the application is structured and how components connect.

## System Architecture

```mermaid
flowchart TB
    subgraph Frontend["Frontend (React + TypeScript)"]
        direction TB
        Home[Home Dashboard]
        Providers[Providers]
        Profiles[Profiles]
        ProfileChat[Profile Chat]
        Projects[Projects]
        ProjectTraining[Project Training]
        SessionBuilder[Session Builder]
        Sessions[Sessions]
        ParallelBrainstorm[Parallel Brainstorm]
        DebateRoom[Debate Room]
        SimpleCoder[Simple Coder]
        AgentRuns[Agent Runs]
        Settings[Settings]
    end

    subgraph State["State (Zustand)"]
        Store[(App Store)]
    end

    subgraph API["API Layer"]
        Invoke[Tauri Invoke]
        HTTP[HTTP REST]
    end

    subgraph Backend["Backend (Rust + Tauri)"]
        DB[(SQLite DB)]
        Keychain[Keychain]
        Orchestrator[Orchestrator]
        DebateOrchestrator[Debate Orchestrator]
        subgraph Commands["Tauri Commands"]
            Auth[Auth]
            Profile[Profile]
            Chat[Chat]
            Training[Training]
            Import[Import]
            Ollama[Ollama]
            RAG[RAG]
            Coder[Coder]
            Voice[Voice]
            Web[Web Search]
        end
        subgraph ProvidersBackend["Provider Adapters"]
            OpenAI[OpenAI]
            Anthropic[Anthropic]
            Google[Google]
            OllamaAdapter[Ollama]
            LocalHTTP[Local HTTP]
        end
    end

    Home --> Store
    Providers --> Store
    Profiles --> Store
    ProfileChat --> Store
    Projects --> Store
    ProjectTraining --> Store
    SessionBuilder --> Store
    Sessions --> Store
    ParallelBrainstorm --> Store
    DebateRoom --> Store
    SimpleCoder --> Store
    AgentRuns --> Store

    Home --> Invoke
    Providers --> Invoke
    Profiles --> Invoke
    ProfileChat --> Invoke
    ProfileChat --> HTTP
    Projects --> Invoke
    ProjectTraining --> Invoke
    SessionBuilder --> Invoke
    ParallelBrainstorm --> Invoke
    DebateRoom --> Invoke
    SimpleCoder --> Invoke
    AgentRuns --> Invoke

    Invoke --> Commands
    HTTP --> Commands

    Auth --> DB
    Profile --> DB
    Chat --> ProvidersBackend
    Training --> DB
    Training --> OllamaAdapter
    Import --> DB
    Ollama --> OllamaAdapter
    RAG --> DB
    Coder --> ProvidersBackend
    Voice --> OllamaAdapter

    Auth --> Keychain
    Profile --> Keychain
    Chat --> Keychain

    Orchestrator --> DebateOrchestrator
    Orchestrator --> ProvidersBackend
    DebateOrchestrator --> ProvidersBackend
```

## Application Flow

```mermaid
flowchart LR
    subgraph Entry["Entry Points"]
        A[Home]
        B[Session Builder]
        C[Profile Chat]
        D[Projects]
    end

    subgraph SessionFlow["Session Flow"]
        B --> E[Select Profiles]
        E --> F{Mode}
        F -->|Parallel| G[Parallel Brainstorm]
        F -->|Debate| H[Debate Room]
        G --> I[Compare Results]
        H --> I
    end

    subgraph TrainingFlow["Local Training Flow"]
        D --> J[Project Training]
        J --> K[Add Base Model]
        J --> L[Import Training Data]
        L --> M[File / Folder / URL]
        L --> N[Research Papers]
        L --> O[Coder History]
        L --> P[Profile Chat]
        K --> Q[Start LoRA Training]
        Q --> R[Export to Ollama / HF]
        R --> S[Use in Profile Chat]
    end

    subgraph ChatFlow["Chat Flow"]
        C --> T[Select Profile]
        T --> U[Load Training Data]
        U --> V[Talk to Trained Data]
        U --> W[Attach as Context]
        C --> X[Voice Input]
        C --> Y[Web Search]
    end

    A --> B
    A --> C
    A --> D
    A --> Z[Simple Coder]
```

## Data Model

```mermaid
erDiagram
    User ||--o{ Provider : has
    User ||--o{ Profile : has
    User ||--o{ Project : has
    User ||--o{ Session : has

    Provider ||--o{ Profile : used_by
    Profile ||--o{ Session : participates_in

    Project ||--o{ LocalModel : contains
    Project ||--o{ TrainingData : contains

    LocalModel ||--o{ TrainingData : trains_on
    LocalModel }o--|| Ollama : exports_to

    Session ||--o{ Run : produces
    Run ||--o{ Message : contains

    User {
        string id
        string username
        string email
    }

    Provider {
        string id
        string type
        string api_key
    }

    Profile {
        string id
        string name
        string provider_id
        string model
    }

    Project {
        string id
        string name
    }

    LocalModel {
        string id
        string base_model
        string training_status
    }

    TrainingData {
        string id
        string input_text
        string output_text
    }

    Session {
        string id
        string mode
    }

    Run {
        string id
        string session_id
    }
```

## Page Navigation

```mermaid
flowchart TD
    Home[Home]
    Home --> SessionBuilder[Session Builder]
    Home --> ProfileChat[Profile Chat]
    Home --> Projects[Projects]
    Home --> SimpleCoder[Simple Coder]
    Home --> Sessions[Sessions]

    SessionBuilder --> ParallelBrainstorm[Parallel Brainstorm]
    SessionBuilder --> DebateRoom[Debate Room]
    ParallelBrainstorm --> Compare[Compare]
    DebateRoom --> Compare

    Projects --> ProjectTraining[Project Training]
    ProjectTraining --> ProfileChat

    Profiles[Profiles] --> ProfileChat
    Providers[Providers]
    Settings[Settings]
    AgentRuns[Agent Runs]
```

## Training Data Import Sources

```mermaid
flowchart TB
    Import[Import Training Data]
    Import --> File[File]
    Import --> Folder[Folder]
    Import --> URL[URL]
    Import --> Text[Pasted Text]
    Import --> ResearchPaper[Research Paper]
    Import --> CoderHistory[Coder History]
    Import --> ProfileChat[Profile Chat]

    ResearchPaper --> PDF[Single PDF]
    ResearchPaper --> PDFFolder[Folder of PDFs]

    File --> JSON[JSON/JSONL]
    File --> CSV[CSV]
    File --> TXT[Text/Markdown]
    File --> PDF2[PDF]
```

## Component Dependencies

```mermaid
flowchart LR
    subgraph Pages
        ProfileChat
        ProjectTraining
        SessionBuilder
    end

    subgraph Shared
        useStreamingLLM[useStreamingLLM]
        api[api]
        store[store]
    end

    subgraph Modals
        ImportModal[ImportTrainingDataModal]
        LoraModal[LoraTrainingModal]
        ExportModal[ExportModelModal]
    end

    ProfileChat --> useStreamingLLM
    ProfileChat --> api
    ProfileChat --> store

    ProjectTraining --> ImportModal
    ProjectTraining --> LoraModal
    ProjectTraining --> ExportModal
    ProjectTraining --> api
    ProjectTraining --> store

    SessionBuilder --> api
    SessionBuilder --> store

    ImportModal --> api
    LoraModal --> api
    ExportModal --> api
```
