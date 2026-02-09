// Authentication commands

use crate::db::Database;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignupRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub remember_me: bool,
}

// Implementation function for signup (used by both Tauri command and HTTP handler)
pub async fn signup_impl(
    db: &Database,
    request: SignupRequest,
) -> Result<User, String> {
    // Validate input
    if request.username.is_empty() || request.username.len() < 3 {
        return Err("Username must be at least 3 characters".to_string());
    }
    if request.email.is_empty() || !request.email.contains('@') {
        return Err("Invalid email address".to_string());
    }
    if request.password.is_empty() || request.password.len() < 6 {
        return Err("Password must be at least 6 characters".to_string());
    }

    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    // Check if username or email already exists
    let existing: Result<String, _> = conn_guard.query_row(
        "SELECT id FROM users WHERE username = ?1 OR email = ?2",
        params![request.username, request.email],
        |row| row.get(0),
    );

    if existing.is_ok() {
        return Err("Username or email already exists".to_string());
    }

    // Hash password
    let password_hash = hash(&request.password, DEFAULT_COST)
        .map_err(|e| format!("Failed to hash password: {}", e))?;

    // Create user
    let user_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();

    conn_guard.execute(
        "INSERT INTO users (id, username, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, request.username, request.email, password_hash, created_at],
    )
    .map_err(|e| format!("Failed to create user: {}", e))?;

    Ok(User {
        id: user_id,
        username: request.username,
        email: request.email,
        created_at,
        last_login_at: None,
    })
}

#[tauri::command]
pub async fn signup(
    db: State<'_, Database>,
    request: SignupRequest,
) -> Result<User, String> {
    signup_impl(&*db, request).await
}

// Implementation function for login (used by both Tauri command and HTTP handler)
pub async fn login_impl(
    db: &Database,
    request: LoginRequest,
) -> Result<User, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    // Get user by username OR email (allow login with either)
    let (user_id, username, email, password_hash, created_at): (String, String, String, String, String) = 
        conn_guard.query_row(
            "SELECT id, username, email, password_hash, created_at FROM users WHERE username = ?1 OR email = ?1",
            params![request.username],
            |row| Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            )),
        )
        .map_err(|_| "Invalid username or password".to_string())?;

    // Verify password
    let password_valid = verify(&request.password, &password_hash)
        .map_err(|_| "Invalid username or password".to_string())?;
    
    if !password_valid {
        return Err("Invalid username or password".to_string());
    }

    // Update last login
    let last_login_at = Utc::now().to_rfc3339();
    conn_guard.execute(
        "UPDATE users SET last_login_at = ?1 WHERE id = ?2",
        params![last_login_at, user_id],
    )
    .map_err(|e| format!("Failed to update last login: {}", e))?;

    // If remember_me is true, we could store the password hash in keyring
    // For now, we'll just return the user
    if request.remember_me {
        // Store password in keyring for auto-login (optional)
        // This is a simplified approach - in production, use a session token
    }

    Ok(User {
        id: user_id,
        username,
        email,
        created_at,
        last_login_at: Some(last_login_at),
    })
}

#[tauri::command]
pub async fn login(
    db: State<'_, Database>,
    request: LoginRequest,
) -> Result<User, String> {
    login_impl(&*db, request).await
}

// Implementation function for get_current_user (used by both Tauri command and HTTP handler)
pub async fn get_current_user_impl(
    db: &Database,
    user_id: String,
) -> Result<User, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    let (username, email, created_at, last_login_at): (String, String, String, Option<String>) = 
        conn_guard.query_row(
            "SELECT username, email, created_at, last_login_at FROM users WHERE id = ?1",
            params![user_id],
            |row| Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
            )),
        )
        .map_err(|_| "User not found".to_string())?;

    Ok(User {
        id: user_id,
        username,
        email,
        created_at,
        last_login_at,
    })
}

#[tauri::command]
pub async fn get_current_user(
    db: State<'_, Database>,
    user_id: String,
) -> Result<User, String> {
    get_current_user_impl(&*db, user_id).await
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    // Clear any stored credentials
    // In a full implementation, you'd clear session tokens here
    Ok(())
}

/// Associate all existing anonymous data (user_id IS NULL) with a specific user.
/// This is useful when a user logs in for the first time and wants to claim their existing data.
pub async fn associate_anonymous_data_with_user_impl(
    db: &Database,
    user_id: &str,
) -> Result<usize, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let mut total_updated = 0usize;
    
    // Update provider_accounts
    let updated = conn_guard.execute(
        "UPDATE provider_accounts SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update provider_accounts: {}", e))?;
    total_updated += updated;
    
    // Update prompt_profiles
    let updated = conn_guard.execute(
        "UPDATE prompt_profiles SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update prompt_profiles: {}", e))?;
    total_updated += updated;
    
    // Update chat_messages
    let updated = conn_guard.execute(
        "UPDATE chat_messages SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update chat_messages: {}", e))?;
    total_updated += updated;
    
    // Update projects
    let updated = conn_guard.execute(
        "UPDATE projects SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update projects: {}", e))?;
    total_updated += updated;
    
    // Update coder_chats
    let updated = conn_guard.execute(
        "UPDATE coder_chats SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update coder_chats: {}", e))?;
    total_updated += updated;
    
    // Update coder_ide_conversations
    let updated = conn_guard.execute(
        "UPDATE coder_ide_conversations SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update coder_ide_conversations: {}", e))?;
    total_updated += updated;
    
    // Update coder_workflows
    let updated = conn_guard.execute(
        "UPDATE coder_workflows SET user_id = ?1 WHERE user_id IS NULL",
        params![user_id],
    ).map_err(|e| format!("Failed to update coder_workflows: {}", e))?;
    total_updated += updated;
    
    Ok(total_updated)
}

#[tauri::command]
pub async fn associate_anonymous_data_with_user(
    db: State<'_, Database>,
    user_id: String,
) -> Result<usize, String> {
    associate_anonymous_data_with_user_impl(&*db, &user_id).await
}

/// Create a user with specific credentials and associate all anonymous data with them.
/// This is a convenience function for initial setup.
pub async fn create_user_and_claim_data_impl(
    db: &Database,
    username: String,
    email: String,
    password: String,
) -> Result<User, String> {
    // First create the user
    let user = signup_impl(db, SignupRequest { username, email, password }).await?;
    
    // Then associate all anonymous data with this user
    let count = associate_anonymous_data_with_user_impl(db, &user.id).await?;
    eprintln!("[Auth] Created user {} and associated {} records with them", user.id, count);
    
    Ok(user)
}

#[tauri::command]
pub async fn create_user_and_claim_data(
    db: State<'_, Database>,
    username: String,
    email: String,
    password: String,
) -> Result<User, String> {
    create_user_and_claim_data_impl(&*db, username, email, password).await
}
