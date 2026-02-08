use brain_stormer_lib::Database;
use brain_stormer_lib::get_adapter;
use brain_stormer_lib::{PromptPacket, ProviderAccount, NormalizedResponse};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use serde_json::json;
use std::env;
use std::path::PathBuf;

fn resolve_db_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        if let Ok(p) = env::var("APPDATA") {
            return Ok(PathBuf::from(p).join("panther").join("panther.db"));
        }
    }

    if let Ok(h) = env::var("HOME") {
        return Ok(
            PathBuf::from(h)
                .join(".local")
                .join("share")
                .join("panther")
                .join("panther.db"),
        );
    }

    // Fallback: current directory
    Ok(PathBuf::from("panther.db"))
}

fn load_provider(conn: &Connection, provider_id: &str) -> Result<ProviderAccount> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json
         FROM provider_accounts WHERE id = ?1",
    )?;

    let mut rows = stmt.query([provider_id])?;
    let row = rows
        .next()?
        .context("Provider not found; configure providers in the app first")?;

    let provider_metadata_json: Option<String> = row.get(8)?;
    let provider_metadata_json = provider_metadata_json
        .and_then(|s| serde_json::from_str(&s).ok());

    Ok(ProviderAccount {
        id: row.get(0)?,
        provider_type: row.get(1)?,
        display_name: row.get(2)?,
        base_url: row.get(3)?,
        region: row.get(4)?,
        auth_ref: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        provider_metadata_json,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        eprintln!("Panther Agent CLI");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  panther-agent run --task \"Refactor X\" --provider <id> --model <name> [--path <path>]");
        eprintln!();
        return Ok(());
    }

    let subcommand = args.remove(0);
    if subcommand != "run" {
        eprintln!("Unknown subcommand: {}", subcommand);
        eprintln!("Use `panther-agent run --help` for usage.");
        std::process::exit(1);
    }

    let mut task = String::new();
    let mut provider_id = String::new();
    let mut model_name = String::new();
    let mut target_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--task" => {
                if i + 1 >= args.len() {
                    eprintln!("--task requires a value");
                    std::process::exit(1);
                }
                task = args[i + 1].clone();
                i += 2;
            }
            "--provider" => {
                if i + 1 >= args.len() {
                    eprintln!("--provider requires a value");
                    std::process::exit(1);
                }
                provider_id = args[i + 1].clone();
                i += 2;
            }
            "--model" => {
                if i + 1 >= args.len() {
                    eprintln!("--model requires a value");
                    std::process::exit(1);
                }
                model_name = args[i + 1].clone();
                i += 2;
            }
            "--path" => {
                if i + 1 >= args.len() {
                    eprintln!("--path requires a value");
                    std::process::exit(1);
                }
                target_path = Some(args[i + 1].clone());
                i += 2;
            }
            "--help" | "-h" => {
                eprintln!("Usage:");
                eprintln!("  panther-agent run --task \"Refactor X\" --provider <id> --model <name> [--path <path>]");
                return Ok(());
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
    }

    if task.is_empty() || provider_id.is_empty() || model_name.is_empty() {
        eprintln!("Missing required arguments. Usage:");
        eprintln!("  panther-agent run --task \"Refactor X\" --provider <id> --model <name> [--path <path>]");
        std::process::exit(1);
    }

    let db_path = resolve_db_path()?;
    let db = Database::new(db_path.clone()).context("Failed to open Panther database")?;
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|_| anyhow::anyhow!("Failed to lock DB connection"))?;

    let provider = load_provider(&conn_guard, &provider_id)?;
    let adapter = get_adapter(&provider.provider_type).context("Failed to get provider adapter")?;

    let mut system_instructions = String::from(
        "You are the Panther CLI coding agent.\n\
        You see only a task description and an optional target path.\n\
        Provide a concise plan and proposed changes as plain text.\n\n",
    );

    if let Some(path) = &target_path {
        system_instructions.push_str(&format!("Target path: {}\n\n", path));
    }

    let packet = PromptPacket {
        global_instructions: Some(system_instructions),
        persona_instructions: String::from("You are a careful, senior-level coding agent."),
        user_message: task.clone(),
        conversation_context: None,
        params_json: json!({
            "temperature": 0.4,
            "max_tokens": 2048
        }),
        stream: false,
    };

    let started = Utc::now();
    let response: NormalizedResponse = adapter
        .complete(&packet, &provider, &model_name)
        .await
        .context("LLM error while executing task")?;
    let finished = Utc::now();

    println!("# Panther Agent Run");
    println!();
    println!("Task      : {}", task);
    if let Some(path) = target_path {
        println!("Target    : {}", path);
    }
    println!("Provider  : {} ({})", provider.display_name, provider.provider_type);
    println!("Model     : {}", model_name);
    println!("Started   : {}", started.to_rfc3339());
    println!("Finished  : {}", finished.to_rfc3339());
    println!();
    println!("--- Agent Output ---");
    println!("{}", response.text.trim());

    Ok(())
}
