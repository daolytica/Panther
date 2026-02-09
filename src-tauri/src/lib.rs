// Brain Stormer Tauri application

mod db;
mod keychain;
mod providers;
mod provider_resolver;
mod prompt_transform;
mod rag;
mod cache;
mod types;
mod commands;
mod commands_debate;
mod commands_auth;
mod commands_profile;
mod commands_chat;
mod commands_web;
mod commands_training;
mod commands_ollama;
mod commands_stats;
mod commands_local_gpt;
mod commands_import;
mod commands_rag;
mod commands_dependencies;
mod commands_coder;
mod commands_coder_ide;
mod commands_privacy;
mod commands_settings;
mod commands_workspace;
mod commands_voice;
mod token_usage;
mod voice;
mod training_ingest;
mod web_search;
mod orchestrator;
mod debate_orchestrator;
mod native_agent;
mod privacy;
mod tools;
mod cline;
mod commands_cline;
pub mod http_server;

// Re-export necessary items for external binary
pub use db::Database;
pub use providers::get_adapter;
pub use types::{PromptPacket, ProviderAccount, NormalizedResponse};

use tauri::Manager;
use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize database
            // Always use production database path so dev and production share the same database
            let db_path = if cfg!(windows) {
                std::env::var("APPDATA")
                    .map(|p| PathBuf::from(p).join("panther").join("panther.db"))
                    .unwrap_or_else(|_| {
                        // Fallback to dev path if APPDATA not available
                        let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
                        std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
                        app_data_dir.join("panther.db")
                    })
            } else {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("panther.db"))
                    .unwrap_or_else(|_| {
                        // Fallback to dev path if HOME not available
                        let app_data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
                        std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data dir");
                        app_data_dir.join("panther.db")
                    })
            };
            
            // Ensure parent directory exists
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create database directory");
            }
            
            let db = db::Database::new(db_path.clone())
                .expect("Failed to initialize database");
            
            let orchestrator = orchestrator::Orchestrator::new(db.clone());
            
            // Store for training processes (model_id -> Process ID as string for cancellation)
            let training_processes: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
            app.manage(training_processes);
            
            // Store for cancellation tokens (token -> bool)
            let cancellation_tokens: Arc<Mutex<HashMap<String, bool>>> = Arc::new(Mutex::new(HashMap::new()));
            app.manage(cancellation_tokens);
            
            app.manage(db.clone());
            app.manage(orchestrator);

            // Spawn HTTP server for browser mode (port 3001, configurable via env)
            let http_port: u16 = std::env::var("PANTHER_HTTP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3001);
            let db_for_http = db.clone();
            tauri::async_runtime::spawn(async move {
                http_server::run_http_server(db_for_http, http_port).await;
            });

            Ok(())
        })
            .invoke_handler(tauri::generate_handler![
            commands::create_provider,
            commands::list_providers,
            commands::update_provider,
            commands::delete_provider,
            commands::test_provider_connection,
            commands::list_provider_models,
            commands::create_profile,
            commands::update_profile,
            commands::list_profiles,
            commands::create_project,
            commands::list_projects,
            commands::update_project,
            commands::delete_project,
            commands::move_session_to_project,
            commands::create_session,
            commands::list_sessions,
            commands::get_session,
            commands::get_session_run,
            commands::delete_session,
            commands::start_run,
            commands::get_run_status,
            commands::get_run_results,
            commands::cancel_run,
            commands::cancel_run_result,
            commands::delete_run_result,
            commands::rerun_single_agent,
            commands::continue_agent,
            commands_debate::start_debate,
            commands_debate::get_debate_messages,
            commands_debate::pause_debate,
            commands_debate::resume_debate,
            commands_debate::cancel_debate,
            commands_debate::delete_debate_message,
            commands_debate::add_user_message,
            commands_debate::continue_debate,
            commands_debate::export_session_markdown,
            commands_debate::export_session_json,
            commands::generate_comparison_table,
            commands::store_api_key,
            commands::retrieve_api_key,
            commands::delete_api_key,
            commands_auth::signup,
            commands_auth::login,
            commands_auth::logout,
            commands_auth::get_current_user,
            commands_auth::associate_anonymous_data_with_user,
            commands_auth::create_user_and_claim_data,
            commands_profile::generate_character_from_url,
            commands_profile::get_latest_profile,
            commands_profile::cancel_character_generation,
            commands_chat::chat_with_profile,
            commands_chat::improve_response_with_cloud,
            commands_chat::load_chat_messages,
            commands_chat::insert_chat_message,
            commands_chat::update_chat_message_content,
            commands_chat::clear_chat_messages,
            commands_chat::list_profile_conversations,
            commands_chat::create_profile_conversation,
            commands_chat::delete_profile_conversation,
            commands_chat::clear_conversation_messages,
            commands_chat::export_chat_messages_to_training,
            commands_coder::export_coder_chats_to_training,
            commands_web::search_web,
            commands_training::create_local_model,
            commands_training::update_local_model,
            commands_training::list_local_models,
            commands_training::create_training_data,
            commands_training::list_training_data,
            commands_training::update_local_model_status,
            commands_training::check_training_environment,
            commands_training::start_training,
            commands_training::start_lora_training,
            commands_training::get_training_progress,
            commands_training::stop_training,
            commands_training::chat_with_training_data,
            commands_ollama::check_ollama_installation,
            commands_ollama::pull_ollama_model,
            commands_import::import_training_data_from_file,
            commands_import::import_training_data_from_url,
            commands_import::import_training_data_from_text,
            commands_import::import_training_data_from_folder,
            commands_import::import_training_data_from_coder_history,
            commands_import::import_training_data_from_chat_messages,
            commands_import::list_pdf_files_in_folder,
            commands_import::parse_pdf_as_research_paper,
            commands_import::import_research_paper,
            commands_stats::get_app_statistics,
            commands_stats::get_app_info,
            commands_stats::get_database_path,
            commands_stats::clear_cache,
            commands_stats::export_database_backup,
            commands_stats::get_build_directory_size,
            commands_stats::clean_build_directory,
            commands_local_gpt::chat_with_local_gpt,
            commands_rag::get_citations_for_result,
            commands_rag::get_groundedness_for_result,
            commands_rag::get_document_chunk,
            commands_dependencies::check_dependencies,
            commands_dependencies::install_dependency,
            commands_dependencies::check_system_cuda,
            commands_dependencies::install_all_dependencies,
            commands_dependencies::save_hf_token,
            commands_dependencies::get_hf_token,
            commands_dependencies::delete_hf_token,
            commands_dependencies::upgrade_dependency,
            commands_dependencies::uninstall_dependency,
            commands_dependencies::check_training_readiness,
            commands_coder::coder_chat,
            commands_coder::load_coder_chats,
            commands_coder::save_coder_chat,
            commands_coder::delete_coder_chat,
            commands_coder::get_system_stats,
            commands_coder::coder_agent_task,
            commands_coder::list_coder_workflows,
            commands_coder::save_coder_workflow,
            commands_coder::delete_coder_workflow,
            commands_coder::coder_agent_record_apply_steps,
            commands_coder::list_agent_runs,
            commands_coder::get_agent_run_steps,
            commands_coder::coder_chat_stream,
            commands_coder::coder_auto_chat,
            commands_coder::run_coder_workflow,
            // Coder IDE commands
            commands_coder_ide::save_coder_ide_conversation,
            commands_coder_ide::load_coder_ide_conversations,
            commands_coder_ide::delete_coder_ide_conversation,
            commands_coder_ide::export_coder_ide_conversations_to_training,
            commands_coder_ide::ingest_coder_turn_command,
            // Privacy commands
            commands_privacy::get_privacy_settings,
            commands_privacy::save_privacy_settings,
            commands_privacy::add_custom_identifier,
            commands_privacy::remove_custom_identifier,
            commands_privacy::preview_redaction,
            commands_privacy::delete_all_conversations,
            commands_privacy::delete_conversation,
            commands_privacy::get_pseudonym_for_conversation,
            // Workspace commands (IDE)
            commands_workspace::get_workspace_path,
            commands_workspace::list_workspace_files,
            commands_workspace::read_workspace_file,
            commands_workspace::write_workspace_file,
            commands_workspace::create_workspace_file,
            commands_workspace::delete_workspace_file,
            commands_workspace::rename_workspace_file,
            commands_workspace::execute_command,
            commands_workspace::get_file_language,
            commands_workspace::get_current_directory,
            commands_workspace::change_directory,
            commands_workspace::check_dependency,
            commands_workspace::install_dependency_command,
            // Training cache commands
            commands_training::get_training_cache_stats,
            commands_training::clear_training_cache,
            // Model export commands
            commands_training::export_model_huggingface,
            commands_training::export_model_ollama,
            commands_training::get_gguf_quantization_options,
            commands_training::check_export_options,
            commands_training::list_trained_ollama_models,
            commands_training::convert_model_to_gguf,
            commands_training::can_start_training,
            // App settings commands
            commands_settings::get_app_settings,
            commands_settings::save_app_settings,
            commands_settings::get_default_global_prompt_path,
            commands_settings::update_global_system_prompt_file,
            commands_settings::read_global_prompt_file,
            commands_settings::update_cache_settings,
            commands_settings::update_training_settings,
            // Voice commands (local STT/TTS)
            commands_voice::transcribe_audio,
            commands_voice::synthesize_speech,
            commands_voice::get_whisper_models_dir,
            commands_voice::get_piper_voices_dir,
            // Cline IDE commands
            commands_cline::cline_agent_task,
            commands_cline::cline_approve_tool,
            commands_cline::cline_create_checkpoint,
            commands_cline::cline_restore_checkpoint,
            commands_cline::cline_compare_checkpoint,
            commands_cline::cline_get_errors,
            commands_cline::cline_analyze_ast,
            commands_cline::ingest_cline_turn,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
