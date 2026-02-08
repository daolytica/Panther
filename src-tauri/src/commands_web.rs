// Web search commands

use crate::web_search::{WebSearch, NewsResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSearchRequest {
    pub query: String,
    pub max_results: Option<usize>,
}

#[tauri::command]
pub async fn search_web(request: WebSearchRequest) -> Result<Vec<NewsResult>, String> {
    let web_search = WebSearch::new();
    let max_results = request.max_results.unwrap_or(5);
    
    web_search.search_recent_news(&request.query, max_results)
        .await
        .map_err(|e| format!("Web search failed: {}", e))
}
