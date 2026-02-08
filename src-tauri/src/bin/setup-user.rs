// One-time setup script to create initial user and associate all anonymous data
// Run with: cargo run --bin setup-user

use std::path::PathBuf;

fn resolve_db_path() -> PathBuf {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("panther").join("panther.db"))
            .unwrap_or_else(|_| PathBuf::from("panther.db"))
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("panther.db"))
            .unwrap_or_else(|_| PathBuf::from("panther.db"))
    }
}

#[tokio::main]
async fn main() {
    let db_path = resolve_db_path();
    println!("Database path: {:?}", db_path);
    
    if !db_path.exists() {
        eprintln!("Error: Database not found at {:?}", db_path);
        eprintln!("Please run the main application first to initialize the database.");
        std::process::exit(1);
    }
    
    let db = brain_stormer_lib::Database::new(db_path)
        .expect("Failed to open database");
    
    // User details
    let username = "rezam";
    let email = "s.r.mirfayzi@gmail.com";
    let password = "password";
    
    println!("\nCreating user account:");
    println!("  Username: {}", username);
    println!("  Email: {}", email);
    println!("  Password: {}", password);
    println!();
    
    // Use the create_user_and_claim_data function from commands_auth
    // Since we can't access the internal function directly from the binary,
    // we'll do it manually with SQL
    
    let conn = db.get_connection();
    let conn_guard = conn.lock().expect("Failed to lock database");
    
    // Check if user already exists
    let existing: Result<String, _> = conn_guard.query_row(
        "SELECT id FROM users WHERE username = ?1 OR email = ?2",
        rusqlite::params![username, email],
        |row| row.get(0),
    );
    
    let user_id = if let Ok(id) = existing {
        println!("User already exists with ID: {}", id);
        id
    } else {
        // Create user
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .expect("Failed to hash password");
        let user_id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();
        
        conn_guard.execute(
            "INSERT INTO users (id, username, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![user_id, username, email, password_hash, created_at],
        ).expect("Failed to create user");
        
        println!("Created new user with ID: {}", user_id);
        user_id
    };
    
    // Associate all anonymous data with this user
    println!("\nAssociating anonymous data with user...");
    
    let tables = vec![
        "provider_accounts",
        "prompt_profiles", 
        "chat_messages",
        "projects",
        "coder_chats",
        "coder_ide_conversations",
        "coder_workflows",
    ];
    
    let mut total = 0usize;
    for table in tables {
        let query = format!("UPDATE {} SET user_id = ?1 WHERE user_id IS NULL", table);
        match conn_guard.execute(&query, rusqlite::params![user_id]) {
            Ok(count) => {
                if count > 0 {
                    println!("  {}: {} records updated", table, count);
                    total += count;
                }
            }
            Err(e) => {
                eprintln!("  Warning: Could not update {}: {}", table, e);
            }
        }
    }
    
    println!("\nTotal records associated: {}", total);
    println!("\n✅ Setup complete!");
    println!("\nYou can now log in with:");
    println!("  Username: {} (or email: {})", username, email);
    println!("  Password: {}", password);
}
