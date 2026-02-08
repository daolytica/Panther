// HTTP server for browser mode - exposes API over HTTP when app runs

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use crate::commands::{self, CreateProfileRequest, CreateProviderRequest, CreateSessionRequest};
use crate::commands_chat::{self, ChatRequest, ImproveWithCloudRequest};
use crate::commands_auth::{self, SignupRequest, LoginRequest};
use crate::db::Database;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
}

pub async fn run_http_server(db: Database, port: u16) {
    let state = AppState { db };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Root route - redirect to health or show API info
        .route("/", get(root))
        .route("/api/health", get(health))
        // Auth routes
        .route("/api/auth/signup", post(signup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/user/:id", get(get_current_user))
        .route("/api/auth/user/:id/claim-data", post(claim_user_data))
        .route("/api/auth/create-and-claim", post(create_and_claim))
        .route("/api/auth/logout", post(logout))
        // Providers
        .route("/api/providers", get(list_providers).post(create_provider))
        .route("/api/providers/:id", get(get_provider).put(update_provider).delete(delete_provider))
        .route("/api/providers/:id/test", post(test_provider))
        .route("/api/providers/:id/models", get(list_provider_models))
        // Profiles
        .route("/api/profiles", get(list_profiles).post(create_profile))
        .route("/api/profiles/:id", get(get_profile).put(update_profile))
        // Projects
        .route("/api/projects", get(list_projects))
        // Sessions
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/:id", delete(delete_session))
        .route("/api/sessions/:id/run", get(get_session_run))
        // Chat
        .route("/api/chat/:profile_id", post(chat_with_profile))
        .route("/api/chat/:profile_id/improve", post(improve_with_cloud))
        .route("/api/chat/:profile_id/messages", get(load_chat_messages).post(insert_chat_message).delete(clear_chat_messages))
        .route("/api/chat/:profile_id/conversations", get(list_profile_conversations).post(create_profile_conversation))
        .route("/api/chat/conversations/:conversation_id", delete(delete_profile_conversation))
        .route("/api/chat/conversations/:conversation_id/messages", delete(clear_conversation_messages))
        .route("/api/chat/messages/:id", put(update_chat_message))
        // Voice (local STT/TTS)
        .route("/api/voice/transcribe", post(voice_transcribe))
        .route("/api/voice/synthesize", post(voice_synthesize))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind HTTP server to port {}: {}", port, e);
            eprintln!("Try setting PANTHER_HTTP_PORT to a different port, e.g.:");
            eprintln!("  PANTHER_HTTP_PORT=3002 npm run dev:server");
            return;
        }
    };
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("HTTP server error: {}", e);
    }
}

// Root route - shows API info and available endpoints
async fn root() -> impl IntoResponse {
    Json(serde_json::json!({
        "name": "Panther API",
        "version": "1.0.0",
        "status": "running",
        "endpoints": {
            "health": "/api/health",
            "auth": {
                "signup": "POST /api/auth/signup",
                "login": "POST /api/auth/login",
                "user": "GET /api/auth/user/:id",
                "logout": "POST /api/auth/logout"
            },
            "providers": "/api/providers",
            "profiles": "/api/profiles",
            "chat": "/api/chat/:profile_id"
        },
        "docs": "Use /api/health to check server status"
    }))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

// Auth handlers
async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> impl IntoResponse {
    match commands_auth::signup_impl(&state.db, req).await {
        Ok(user) => (StatusCode::CREATED, Json(serde_json::json!(user))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    match commands_auth::login_impl(&state.db, req).await {
        Ok(user) => (StatusCode::OK, Json(serde_json::json!(user))).into_response(),
        Err(e) => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn get_current_user(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match commands_auth::get_current_user_impl(&state.db, id).await {
        Ok(user) => (StatusCode::OK, Json(serde_json::json!(user))).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn logout() -> impl IntoResponse {
    // Logout is client-side (clear localStorage), server just acknowledges
    StatusCode::NO_CONTENT.into_response()
}

async fn claim_user_data(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match commands_auth::associate_anonymous_data_with_user_impl(&state.db, &id).await {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({ "count": count }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct CreateAndClaimRequest {
    username: String,
    email: String,
    password: String,
}

async fn create_and_claim(
    State(state): State<AppState>,
    Json(req): Json<CreateAndClaimRequest>,
) -> impl IntoResponse {
    match commands_auth::create_user_and_claim_data_impl(&state.db, req.username, req.email, req.password).await {
        Ok(user) => (StatusCode::CREATED, Json(serde_json::json!(user))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Providers
async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    match commands::list_providers_impl(&state.db, user_id).await {
        Ok(providers) => (StatusCode::OK, Json(providers)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn create_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| req.get("user_id").and_then(|v| v.as_str()).map(|s: &str| s.to_string()));
    let request: CreateProviderRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    match commands::create_provider_impl(&state.db, request, user_id).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn get_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    let providers = match commands::list_providers_impl(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    };
    match providers.into_iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&id)) {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Provider not found" }))).into_response(),
    }
}

async fn update_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| req.get("user_id").and_then(|v| v.as_str()).map(|s: &str| s.to_string()));
    let request: CreateProviderRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    match commands::update_provider_impl(&state.db, id, request, user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    match commands::delete_provider_impl(&state.db, id, user_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn test_provider(State(state): State<AppState>, axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands::test_provider_connection_impl(&state.db, id).await {
        Ok(ok) => (StatusCode::OK, Json(serde_json::json!({ "success": ok }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn list_provider_models(State(state): State<AppState>, axum::extract::Path(id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands::list_provider_models_impl(&state.db, id).await {
        Ok(models) => (StatusCode::OK, Json(models)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Profiles
async fn list_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    match commands::list_profiles_impl(&state.db, user_id).await {
        Ok(profiles) => (StatusCode::OK, Json(profiles)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn create_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| req.get("user_id").and_then(|v| v.as_str()).map(|s: &str| s.to_string()));
    let request: CreateProfileRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    match commands::create_profile_impl(&state.db, request, user_id).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    let profiles = match commands::list_profiles_impl(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    };
    match profiles.into_iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&id)) {
        Some(p) => (StatusCode::OK, Json(p)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "Profile not found" }))).into_response(),
    }
}

async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let _user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| req.get("user_id").and_then(|v| v.as_str()).map(|s: &str| s.to_string()));
    let request: CreateProfileRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    match commands::update_profile_impl(&state.db, id, request).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Projects
async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    match commands::list_projects_impl(&state.db, user_id).await {
        Ok(projects) => (StatusCode::OK, Json(projects)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Sessions
async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| params.get("user_id").cloned());
    match commands::list_sessions_impl(&state.db, user_id).await {
        Ok(sessions) => (StatusCode::OK, Json(sessions)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct CreateSessionBody {
    project_id: String,
    title: String,
    user_question: String,
    mode: String,
    selected_profile_ids: Vec<String>,
    run_settings: Option<serde_json::Value>,
    local_model_id: Option<String>,
}

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let request = CreateSessionRequest {
        project_id: req.project_id,
        title: req.title,
        user_question: req.user_question,
        mode: req.mode,
        selected_profile_ids: req.selected_profile_ids,
        run_settings: req.run_settings,
        local_model_id: req.local_model_id,
    };
    match commands::create_session_impl(&state.db, request).await {
        Ok(run_id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": run_id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn delete_session(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match commands::delete_session_impl(&state.db, &id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn get_session_run(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match commands::get_session_run_impl(&state.db, &id).await {
        Ok(Some(run)) => (StatusCode::OK, Json(run)).into_response(),
        Ok(None) => (StatusCode::OK, Json(serde_json::Value::Null)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Chat
async fn chat_with_profile(
    State(state): State<AppState>,
    axum::extract::Path(profile_id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut request: ChatRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    request.profile_id = profile_id;
    match commands_chat::chat_with_profile_impl(&state.db, request).await {
        Ok(text) => (StatusCode::OK, Json(serde_json::json!({ "text": text }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn improve_with_cloud(
    State(state): State<AppState>,
    axum::extract::Path(profile_id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut request: ImproveWithCloudRequest = match serde_json::from_value(req) {
        Ok(r) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e.to_string() }))).into_response(),
    };
    request.profile_id = profile_id;
    match commands_chat::improve_response_with_cloud_impl(&state.db, request).await {
        Ok(text) => (StatusCode::OK, Json(serde_json::json!({ "text": text }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn load_chat_messages(
    State(state): State<AppState>,
    axum::extract::Path(profile_id): axum::extract::Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let conversation_id = params.get("conversation_id").cloned();
    match commands_chat::load_chat_messages_impl(&state.db, profile_id, conversation_id).await {
        Ok(msgs) => (StatusCode::OK, Json(msgs)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn list_profile_conversations(State(state): State<AppState>, axum::extract::Path(profile_id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands_chat::list_profile_conversations_impl(&state.db, profile_id).await {
        Ok(list) => (StatusCode::OK, Json(list)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn create_profile_conversation(
    State(state): State<AppState>,
    axum::extract::Path(profile_id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let title = req.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
    match commands_chat::create_profile_conversation_impl(&state.db, profile_id, title).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn delete_profile_conversation(State(state): State<AppState>, axum::extract::Path(conversation_id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands_chat::delete_profile_conversation_impl(&state.db, conversation_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn clear_conversation_messages(State(state): State<AppState>, axum::extract::Path(conversation_id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands_chat::clear_conversation_messages_impl(&state.db, conversation_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn insert_chat_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(profile_id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let role = req.get("role").and_then(|v| v.as_str()).unwrap_or("assistant");
    let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let user_id: Option<String> = headers
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(|s: &str| s.to_string())
        .or_else(|| req.get("user_id").and_then(|v| v.as_str()).map(|s: &str| s.to_string()));
    match commands_chat::insert_chat_message_impl(&state.db, profile_id, role.to_string(), content.to_string(), user_id).await {
        Ok(id) => (StatusCode::CREATED, Json(serde_json::json!({ "id": id }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn clear_chat_messages(State(state): State<AppState>, axum::extract::Path(profile_id): axum::extract::Path<String>) -> impl IntoResponse {
    match commands_chat::clear_chat_messages_impl(&state.db, profile_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn update_chat_message(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    let content = req.get("content").and_then(|v| v.as_str()).unwrap_or("");
    match commands_chat::update_chat_message_content_impl(&state.db, id, content.to_string()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

// Voice handlers (local STT/TTS)
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceTranscribeRequest {
    audio_base64: String,
    model_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VoiceSynthesizeRequest {
    text: String,
    voice_id: Option<String>,
}

async fn voice_transcribe(Json(req): Json<VoiceTranscribeRequest>) -> impl IntoResponse {
    match crate::voice::stt::transcribe_audio(&req.audio_base64, req.model_id) {
        Ok(text) => (StatusCode::OK, Json(serde_json::json!({ "text": text }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

async fn voice_synthesize(Json(req): Json<VoiceSynthesizeRequest>) -> impl IntoResponse {
    match crate::voice::tts::synthesize_speech(&req.text, req.voice_id) {
        Ok(audio_base64) => (StatusCode::OK, axum::body::Body::from(audio_base64)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}
