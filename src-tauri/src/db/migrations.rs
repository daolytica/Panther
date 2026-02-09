// Database migrations

use rusqlite::{Connection, Result};

pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create migrations table to track version
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    let current_version = get_current_version(conn)?;

    if current_version < 1 {
        migration_001_initial_schema(conn)?;
        set_version(conn, 1)?;
    }

    if current_version < 2 {
        migration_002_add_character_features(conn)?;
        set_version(conn, 2)?;
    }

    if current_version < 3 {
        migration_003_add_debate_max_words(conn)?;
        set_version(conn, 3)?;
    }

    if current_version < 4 {
        migration_004_add_usage_json_to_messages(conn)?;
        set_version(conn, 4)?;
    }

    if current_version < 5 {
        migration_005_add_users_table(conn)?;
        set_version(conn, 5)?;
    }

    if current_version < 6 {
        migration_006_add_profile_photo(conn)?;
        set_version(conn, 6)?;
    }

    if current_version < 7 {
        migration_007_add_debate_language_tone(conn)?;
        set_version(conn, 7)?;
    }

    if current_version < 8 {
        migration_008_add_local_models(conn)?;
        set_version(conn, 8)?;
    }

    if current_version < 9 {
        migration_010_add_rag_and_eval(conn)?;
        set_version(conn, 9)?;
    }

    if current_version < 10 {
        migration_011_add_chat_messages(conn)?;
        set_version(conn, 10)?;
    }

    if current_version < 11 {
        migration_012_add_privacy_tables(conn)?;
        set_version(conn, 11)?;
    }

    if current_version < 12 {
        migration_013_add_coder_ide_conversations(conn)?;
        set_version(conn, 12)?;
    }

    if current_version < 13 {
        migration_014_add_training_cache(conn)?;
        set_version(conn, 13)?;
    }

    if current_version < 14 {
        migration_015_add_app_settings(conn)?;
        set_version(conn, 14)?;
    }

    if current_version < 15 {
        migration_016_add_social_posting(conn)?;
        set_version(conn, 15)?;
    }

    if current_version < 16 {
        migration_017_add_agent_and_workflows(conn)?;
        set_version(conn, 16)?;
    }

    if current_version < 17 {
        migration_018_add_token_usage(conn)?;
        set_version(conn, 17)?;
    }

    if current_version < 18 {
        migration_019_add_cline_tables(conn)?;
        set_version(conn, 18)?;
    }

    if current_version < 19 {
        migration_020_add_user_id_to_tables(conn)?;
        set_version(conn, 19)?;
    }

    if current_version < 20 {
        migration_021_add_profile_conversations(conn)?;
        set_version(conn, 20)?;
    }

    if current_version < 21 {
        migration_022_add_voice_to_profiles(conn)?;
        set_version(conn, 21)?;
    }

    // Always run migration_013 to ensure table exists
    migration_013_add_coder_ide_conversations(conn).ok();

    // Always run these migrations to ensure columns exist (they use .ok() to ignore errors)
    // This handles cases where schema version was updated but column wasn't added
    migration_003_add_debate_max_words(conn)?;
    migration_004_add_usage_json_to_messages(conn)?;
    migration_005_add_users_table(conn)?;
    migration_006_add_profile_photo(conn)?;
    migration_007_add_debate_language_tone(conn)?;
    migration_009_add_session_local_model(conn)?;
    migration_023_add_runs_error_message(conn)?;

    Ok(())
}

fn migration_022_add_voice_to_profiles(conn: &Connection) -> Result<()> {
    conn.execute("ALTER TABLE prompt_profiles ADD COLUMN voice_gender TEXT", []).ok();
    conn.execute("ALTER TABLE prompt_profiles ADD COLUMN voice_uri TEXT", []).ok();
    Ok(())
}

fn migration_023_add_runs_error_message(conn: &Connection) -> Result<()> {
    conn.execute(
        "ALTER TABLE runs ADD COLUMN error_message_safe TEXT",
        [],
    ).ok();
    Ok(())
}

fn migration_002_add_character_features(conn: &Connection) -> Result<()> {
    // Add character_definition_json and model_features_json columns if they don't exist
    // This migration is for existing databases that were created before these columns were added
    conn.execute(
        "ALTER TABLE prompt_profiles ADD COLUMN character_definition_json TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    conn.execute(
        "ALTER TABLE prompt_profiles ADD COLUMN model_features_json TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    Ok(())
}

fn migration_004_add_usage_json_to_messages(conn: &Connection) -> Result<()> {
    // Add usage_json column to messages table if it doesn't exist
    conn.execute(
        "ALTER TABLE messages ADD COLUMN usage_json TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    Ok(())
}

fn migration_005_add_users_table(conn: &Connection) -> Result<()> {
    // Create users table for authentication
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_login_at TEXT
        )",
        [],
    )?;
    
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)",
        [],
    )?;
    
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
        [],
    )?;
    
    Ok(())
}

fn migration_006_add_profile_photo(conn: &Connection) -> Result<()> {
    // Add photo_url column to prompt_profiles table if it doesn't exist
    conn.execute(
        "ALTER TABLE prompt_profiles ADD COLUMN photo_url TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    Ok(())
}

fn migration_007_add_debate_language_tone(conn: &Connection) -> Result<()> {
    // Add language and tone columns to debate_configs table if they don't exist
    conn.execute(
        "ALTER TABLE debate_configs ADD COLUMN language TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    conn.execute(
        "ALTER TABLE debate_configs ADD COLUMN tone TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    Ok(())
}

fn get_current_version(conn: &Connection) -> Result<i32> {
    let mut stmt = conn.prepare("SELECT MAX(version) FROM schema_migrations")?;
    let version: Option<i32> = stmt.query_row([], |row| Ok(row.get(0)?))?;
    Ok(version.unwrap_or(0))
}

fn set_version(conn: &Connection, version: i32) -> Result<()> {
    // Use INSERT OR REPLACE to handle cases where version might already exist
    // This can happen if a previous migration attempt partially completed
    conn.execute(
        "INSERT OR REPLACE INTO schema_migrations (version, applied_at) VALUES (?1, datetime('now'))",
        [version],
    )?;
    Ok(())
}

fn migration_001_initial_schema(conn: &Connection) -> Result<()> {
    // ProviderAccount
    conn.execute(
        "CREATE TABLE IF NOT EXISTS provider_accounts (
            id TEXT PRIMARY KEY,
            provider_type TEXT NOT NULL,
            display_name TEXT NOT NULL,
            base_url TEXT,
            region TEXT,
            auth_ref TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            provider_metadata_json TEXT
        )",
        [],
    )?;

    // ModelDefinition
    conn.execute(
        "CREATE TABLE IF NOT EXISTS model_definitions (
            id TEXT PRIMARY KEY,
            provider_account_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            capabilities_json TEXT,
            context_limit INTEGER,
            is_discovered INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // PromptProfile
    conn.execute(
        "CREATE TABLE IF NOT EXISTS prompt_profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            provider_account_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            persona_prompt TEXT NOT NULL,
            character_definition_json TEXT,
            model_features_json TEXT,
            params_json TEXT NOT NULL,
            output_preset_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (provider_account_id) REFERENCES provider_accounts(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Project
    conn.execute(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    // Session
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            user_question TEXT NOT NULL,
            mode TEXT NOT NULL,
            global_prompt_template_id TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Run
    conn.execute(
        "CREATE TABLE IF NOT EXISTS runs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            selected_profile_ids_json TEXT NOT NULL,
            status TEXT NOT NULL,
            run_settings_json TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // RunResult
    conn.execute(
        "CREATE TABLE IF NOT EXISTS run_results (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            profile_id TEXT NOT NULL,
            status TEXT NOT NULL,
            raw_output_text TEXT,
            normalized_output_json TEXT,
            usage_json TEXT,
            error_code TEXT,
            error_message_safe TEXT,
            started_at TEXT,
            finished_at TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (profile_id) REFERENCES prompt_profiles(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // DebateConfig
    conn.execute(
        "CREATE TABLE IF NOT EXISTS debate_configs (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL UNIQUE,
            mode TEXT NOT NULL,
            rounds INTEGER NOT NULL,
            speaking_order_json TEXT NOT NULL,
            context_policy TEXT NOT NULL,
            last_k INTEGER,
            per_turn_budget_json TEXT,
            concurrency INTEGER NOT NULL,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // DebateTurn
    conn.execute(
        "CREATE TABLE IF NOT EXISTS debate_turns (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            round_index INTEGER NOT NULL,
            turn_index INTEGER NOT NULL,
            speaker_profile_id TEXT NOT NULL,
            input_snapshot_json TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT,
            error_code TEXT,
            error_message TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (speaker_profile_id) REFERENCES prompt_profiles(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Message
    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            author_type TEXT NOT NULL,
            profile_id TEXT,
            round_index INTEGER,
            turn_index INTEGER,
            text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            provider_metadata_json TEXT,
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (profile_id) REFERENCES prompt_profiles(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // Synthesis
    conn.execute(
        "CREATE TABLE IF NOT EXISTS syntheses (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            method TEXT NOT NULL,
            synthesizer_profile_id TEXT,
            text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
            FOREIGN KEY (synthesizer_profile_id) REFERENCES prompt_profiles(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // Create indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_model_definitions_provider ON model_definitions(provider_account_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_prompt_profiles_provider ON prompt_profiles(provider_account_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_runs_session ON runs(session_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_run_results_run ON run_results(run_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_run ON messages(run_id)",
        [],
    )?;

    Ok(())
}

fn migration_003_add_debate_max_words(conn: &Connection) -> Result<()> {
    // Add max_words column to debate_configs if it doesn't exist
    conn.execute(
        "ALTER TABLE debate_configs ADD COLUMN max_words INTEGER",
        [],
    ).ok(); // Ignore error if column already exists
    
    Ok(())
}

fn migration_009_add_session_local_model(conn: &Connection) -> Result<()> {
    // Add local_model_id column to sessions table
    conn.execute(
        "ALTER TABLE sessions ADD COLUMN local_model_id TEXT",
        [],
    ).ok(); // Ignore error if column already exists
    
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_local_model ON sessions(local_model_id)",
        [],
    )?;
    
    Ok(())
}

fn migration_010_add_rag_and_eval(conn: &Connection) -> Result<()> {
    // Document chunks for RAG
    conn.execute(
        "CREATE TABLE IF NOT EXISTS document_chunks (
            id TEXT PRIMARY KEY,
            project_id TEXT,
            source_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            embedding_json TEXT,
            metadata_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_document_chunks_project ON document_chunks(project_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_document_chunks_source ON document_chunks(source_id)",
        [],
    )?;

    // SOPs
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sops (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            version INTEGER NOT NULL,
            project_id TEXT,
            sop_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // Evaluations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS evaluations (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            dimensions_json TEXT NOT NULL,
            overall_score REAL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Groundedness scores (per run_result)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS groundedness_scores (
            id TEXT PRIMARY KEY,
            run_result_id TEXT NOT NULL,
            score REAL NOT NULL,
            details_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_result_id) REFERENCES run_results(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Citations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS citations (
            id TEXT PRIMARY KEY,
            run_result_id TEXT NOT NULL,
            source_id TEXT NOT NULL,
            chunk_id TEXT,
            raw_citation_text TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_result_id) REFERENCES run_results(id) ON DELETE CASCADE
        )",
        [],
    )?;

    Ok(())
}

fn migration_008_add_local_models(conn: &Connection) -> Result<()> {
    // Create local_models table for project-specific trained models
    conn.execute(
        "CREATE TABLE IF NOT EXISTS local_models (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            name TEXT NOT NULL,
            base_model TEXT NOT NULL,
            model_path TEXT,
            training_status TEXT NOT NULL DEFAULT 'pending',
            training_config_json TEXT,
            training_metrics_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create training_data table for storing training examples
    conn.execute(
        "CREATE TABLE IF NOT EXISTS training_data (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            local_model_id TEXT,
            input_text TEXT NOT NULL,
            output_text TEXT NOT NULL,
            metadata_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (local_model_id) REFERENCES local_models(id) ON DELETE SET NULL
        )",
        [],
    )?;

    // Create indexes
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_local_models_project ON local_models(project_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_training_data_project ON training_data(project_id)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_training_data_model ON training_data(local_model_id)",
        [],
    )?;

    Ok(())
}

fn migration_011_add_chat_messages(conn: &Connection) -> Result<()> {
    // Create chat_messages table for persistent profile chats
    conn.execute(
        "CREATE TABLE IF NOT EXISTS chat_messages (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (profile_id) REFERENCES prompt_profiles(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chat_messages_profile ON chat_messages(profile_id, created_at)",
        [],
    )?;

    // Create coder_chats table for persistent coder conversations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS coder_chats (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            messages_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    Ok(())
}

fn migration_012_add_privacy_tables(conn: &Connection) -> Result<()> {
    // Privacy settings table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS privacy_settings (
            id TEXT PRIMARY KEY,
            settings_json TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    // Encrypted conversations table for envelope encryption
    conn.execute(
        "CREATE TABLE IF NOT EXISTS encrypted_conversations (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            encrypted_dek TEXT NOT NULL,
            privacy_mode INTEGER NOT NULL DEFAULT 0,
            retention_days INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_encrypted_conv_id ON encrypted_conversations(conversation_id)",
        [],
    )?;

    // Encrypted messages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS encrypted_messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            ciphertext_blob TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (conversation_id) REFERENCES encrypted_conversations(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_encrypted_messages_conv ON encrypted_messages(conversation_id, created_at)",
        [],
    )?;

    Ok(())
}

fn migration_013_add_coder_ide_conversations(conn: &Connection) -> Result<()> {
    // Create table for Coder IDE conversations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS coder_ide_conversations (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            messages_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_coder_ide_conv_updated ON coder_ide_conversations(updated_at DESC)",
        [],
    )?;

    Ok(())
}

fn migration_015_add_app_settings(conn: &Connection) -> Result<()> {
    // Create app_settings table for configurable application settings
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            id TEXT PRIMARY KEY,
            settings_json TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    // Insert default settings
    let default_settings = serde_json::json!({
        "cache": {
            "max_size_gb": 10,
            "eviction_threshold_percent": 80,
            "enable_compression": true,
            "enable_memory_mapped_files": true,
            "memory_mapped_threshold_mb": 100
        },
        "training": {
            "streaming_chunk_size": 1000,
            "enable_adaptive_memory": true,
            "min_chunk_size": 100,
            "max_chunk_size": 10000,
            "memory_pressure_threshold_mb": 2048,
            "enable_progress_tracking": true,
            "progress_update_interval": 1000,
            "enable_parallel_hashing": true,
            "parallel_hash_threshold": 10000
        },
        "auto_training": {
            "auto_training_enabled": true,
            "train_from_chat": true,
            "train_from_coder": true,
            "train_from_debate": true
        }
    });
    
    let default_settings_json = serde_json::to_string(&default_settings)
        .unwrap_or_else(|_| "{}".to_string());
    
    conn.execute(
        "INSERT OR IGNORE INTO app_settings (id, settings_json, updated_at) VALUES ('default', ?1, datetime('now'))",
        [default_settings_json],
    )?;

    Ok(())
}

fn migration_014_add_training_cache(conn: &Connection) -> Result<()> {
    // Create training_data_cache table for caching training data files
    conn.execute(
        "CREATE TABLE IF NOT EXISTS training_data_cache (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            model_id TEXT NOT NULL,
            data_hash TEXT NOT NULL,
            file_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_accessed_at TEXT NOT NULL DEFAULT (datetime('now')),
            access_count INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (model_id) REFERENCES local_models(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create composite index for faster cache lookups
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_training_data_cache_lookup ON training_data_cache(project_id, model_id, data_hash)",
        [],
    )?;

    // Create index for LRU eviction
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_training_data_cache_access ON training_data_cache(last_accessed_at)",
        [],
    )?;

    // Add composite index on training_data for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_training_data_project_model ON training_data(project_id, local_model_id)",
        [],
    )?;

    Ok(())
}

fn migration_018_add_token_usage(conn: &Connection) -> Result<()> {
    // Token usage accounting table for per‑model statistics.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS token_usage (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            provider_id TEXT,
            model_name TEXT NOT NULL,
            prompt_tokens INTEGER,
            completion_tokens INTEGER,
            total_tokens INTEGER,
            context_hash TEXT,
            source TEXT NOT NULL,
            metadata_json TEXT,
            FOREIGN KEY (provider_id) REFERENCES provider_accounts(id) ON DELETE SET NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_token_usage_provider_model_time ON token_usage(provider_id, model_name, timestamp)",
        [],
    )?;

    Ok(())
}

fn migration_016_add_social_posting(conn: &Connection) -> Result<()> {
    // Create social_connections table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS social_connections (
            id TEXT PRIMARY KEY,
            platform_type TEXT NOT NULL CHECK(platform_type IN ('linkedin', 'facebook')),
            display_name TEXT NOT NULL,
            external_account_id TEXT NOT NULL,
            access_token_key TEXT NOT NULL,
            refresh_token_key TEXT,
            token_expires_at TEXT,
            metadata_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(platform_type, external_account_id)
        )",
        [],
    )?;

    // Create social_posts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS social_posts (
            id TEXT PRIMARY KEY,
            platform_type TEXT NOT NULL CHECK(platform_type IN ('linkedin', 'facebook')),
            connection_id TEXT NOT NULL,
            post_text TEXT NOT NULL,
            media_url TEXT,
            scheduled_time TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN ('pending', 'publishing', 'published', 'failed')) DEFAULT 'pending',
            external_post_id TEXT,
            error_text TEXT,
            published_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (connection_id) REFERENCES social_connections(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for efficient queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_social_connections_platform ON social_connections(platform_type)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_social_posts_status_scheduled ON social_posts(status, scheduled_time)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_social_posts_connection ON social_posts(connection_id, created_at DESC)",
        [],
    )?;

    Ok(())
}

fn migration_017_add_agent_and_workflows(conn: &Connection) -> Result<()> {
    // Agent runs table - tracks high level agent executions
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_runs (
            id TEXT PRIMARY KEY,
            task_description TEXT NOT NULL,
            target_paths_json TEXT,
            allow_file_writes INTEGER NOT NULL DEFAULT 0,
            allow_commands INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            provider_id TEXT,
            model_name TEXT,
            error_text TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            started_at TEXT,
            finished_at TEXT,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_runs_status_created ON agent_runs(status, created_at DESC)",
        [],
    )?;

    // Agent steps table - individual tool calls / reasoning steps
    conn.execute(
        "CREATE TABLE IF NOT EXISTS agent_steps (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_index INTEGER NOT NULL,
            step_type TEXT NOT NULL,
            description TEXT,
            tool_name TEXT,
            params_json TEXT,
            result_summary TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES agent_runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_agent_steps_run ON agent_steps(run_id, step_index)",
        [],
    )?;

    // Coder workflows table - reusable agent recipes
    conn.execute(
        "CREATE TABLE IF NOT EXISTS coder_workflows (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            workflow_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_coder_workflows_updated ON coder_workflows(updated_at DESC)",
        [],
    )?;

    Ok(())
}

fn migration_019_add_cline_tables(conn: &Connection) -> Result<()> {
    // Cline task runs
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cline_runs (
            id TEXT PRIMARY KEY,
            task_description TEXT NOT NULL,
            provider_id TEXT NOT NULL,
            model_name TEXT NOT NULL,
            status TEXT NOT NULL,
            workspace_path TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            started_at TEXT,
            finished_at TEXT,
            error_text TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cline_runs_status ON cline_runs(status, created_at DESC)",
        [],
    )?;

    // Cline tool executions (with approvals)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cline_tool_executions (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_index INTEGER NOT NULL,
            tool_type TEXT NOT NULL,
            tool_params_json TEXT,
            approval_status TEXT NOT NULL DEFAULT 'pending',
            result_json TEXT,
            executed_at TEXT,
            FOREIGN KEY (run_id) REFERENCES cline_runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cline_tool_executions_run ON cline_tool_executions(run_id, step_index)",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cline_tool_executions_approval ON cline_tool_executions(approval_status)",
        [],
    )?;

    // Workspace checkpoints
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cline_checkpoints (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            step_index INTEGER NOT NULL,
            snapshot_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES cline_runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cline_checkpoints_run ON cline_checkpoints(run_id, step_index)",
        [],
    )?;

    // Browser automation sessions
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cline_browser_sessions (
            id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            url TEXT,
            screenshots_json TEXT,
            console_logs_json TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (run_id) REFERENCES cline_runs(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cline_browser_sessions_run ON cline_browser_sessions(run_id, created_at DESC)",
        [],
    )?;

    Ok(())
}

fn migration_020_add_user_id_to_tables(conn: &Connection) -> Result<()> {
    // Add user_id to key tables for per-account data isolation.
    // Existing rows get user_id=NULL (legacy/anonymous); new data from logged-in users gets their user_id.

    // app_settings and privacy_settings use id='default' for legacy; per-user uses id=user_id
    let tables_columns = [
        ("provider_accounts", "user_id"),
        ("prompt_profiles", "user_id"),
        ("projects", "user_id"),
        ("chat_messages", "user_id"),
        ("social_connections", "user_id"),
        ("coder_chats", "user_id"),
        ("coder_ide_conversations", "user_id"),
        ("coder_workflows", "user_id"),
    ];

    for (table, col) in tables_columns {
        let sql = format!("ALTER TABLE {} ADD COLUMN {} TEXT REFERENCES users(id) ON DELETE SET NULL", table, col);
        conn.execute(&sql, []).ok(); // Ignore if column exists
    }

    // Indexes for efficient user-scoped queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_provider_accounts_user ON provider_accounts(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_prompt_profiles_user ON prompt_profiles(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_projects_user ON projects(user_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chat_messages_user ON chat_messages(user_id)",
        [],
    )?;
    Ok(())
}

fn migration_021_add_profile_conversations(conn: &Connection) -> Result<()> {
    // Create profile_conversations table for multiple conversations per profile
    conn.execute(
        "CREATE TABLE IF NOT EXISTS profile_conversations (
            id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT 'New conversation',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (profile_id) REFERENCES prompt_profiles(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_profile_conversations_profile ON profile_conversations(profile_id, updated_at DESC)",
        [],
    )?;

    // Add conversation_id to chat_messages (nullable for backward compat)
    conn.execute("ALTER TABLE chat_messages ADD COLUMN conversation_id TEXT REFERENCES profile_conversations(id) ON DELETE CASCADE", []).ok();

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation ON chat_messages(conversation_id, created_at)",
        [],
    )?;

    // Migrate existing messages: create a default conversation per profile and assign messages to it
    let mut stmt = conn.prepare("SELECT DISTINCT profile_id FROM chat_messages WHERE conversation_id IS NULL")?;
    let profiles: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    for profile_id in profiles {
        let conv_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO profile_conversations (id, profile_id, title) VALUES (?1, ?2, ?3)",
            rusqlite::params![conv_id, profile_id, "Chat history"],
        )?;
        conn.execute(
            "UPDATE chat_messages SET conversation_id = ?1 WHERE profile_id = ?2 AND conversation_id IS NULL",
            rusqlite::params![conv_id, profile_id],
        )?;
    }

    Ok(())
}
