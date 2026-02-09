// Standalone HTTP server for browser mode - run without Tauri window.
// Use: cargo run --bin panther-http-server
// Or:  npm run dev:server

use brain_stormer_lib::http_server;
use brain_stormer_lib::Database;
use std::env;
use std::path::PathBuf;

fn resolve_db_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(p) = env::var("APPDATA") {
            let path = PathBuf::from(p).join("panther").join("panther.db");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return path;
        }
    }

    #[cfg(not(windows))]
    {
        if let Ok(h) = env::var("HOME") {
            let path = PathBuf::from(h)
                .join(".local")
                .join("share")
                .join("panther")
                .join("panther.db");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            return path;
        }
    }

    PathBuf::from("panther.db")
}

/// Try to bind to a port, returning the actual port used
async fn try_bind_port(start_port: u16) -> u16 {
    let mut port = start_port;
    for _ in 0..10 {
        match tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await {
            Ok(listener) => {
                // Successfully bound, drop the listener so the server can use it
                drop(listener);
                return port;
            }
            Err(_) => {
                eprintln!("Port {} is in use, trying {}...", port, port + 1);
                port += 1;
            }
        }
    }
    // Return the last tried port, let the server fail with a clear message
    port
}

#[tokio::main]
async fn main() {
    let db_path = resolve_db_path();
    eprintln!("Panther HTTP Server");
    eprintln!("Database: {}", db_path.display());

    let db = Database::new(db_path.clone()).expect("Failed to initialize database");

    let preferred_port: u16 = env::var("PANTHER_HTTP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3001);

    let port = try_bind_port(preferred_port).await;

    eprintln!();
    eprintln!("API: http://localhost:{}/api", port);
    eprintln!("Health: http://localhost:{}/api/health", port);
    eprintln!();
    eprintln!("Run the frontend with: VITE_API_URL=http://localhost:{} npm run dev", port);
    eprintln!("Then open: http://localhost:1420");
    eprintln!();

    http_server::run_http_server(db, port).await;
}
