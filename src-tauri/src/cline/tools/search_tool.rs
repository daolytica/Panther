// File and code search tools

use crate::commands_workspace;
use regex::Regex;

/// Search for files matching a pattern
pub async fn search_files(pattern: &str, regex: bool, workspace_path: &str) -> crate::tools::ToolResult {
    // TODO: Implement file search
    // For now, use basic workspace file listing
    match commands_workspace::list_workspace_files(Some(workspace_path.to_string())).await {
        Ok(files) => {
            let matches: Vec<String> = if regex {
                match Regex::new(pattern) {
                    Ok(re) => files.iter()
                        .filter(|f| re.is_match(&f.path))
                        .map(|f| f.path.clone())
                        .collect(),
                    Err(e) => {
                        return crate::tools::ToolResult::err(format!("Invalid regex pattern: {}", e));
                    }
                }
            } else {
                files.iter()
                    .filter(|f| f.path.contains(pattern))
                    .map(|f| f.path.clone())
                    .collect()
            };
            
            let result = serde_json::json!({
                "pattern": pattern,
                "matches": matches,
                "count": matches.len()
            });
            
            crate::tools::ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&result).unwrap_or_default(),
                error: None,
                extra_json: Some(result),
            }
        }
        Err(e) => crate::tools::ToolResult::err(format!("Failed to list files: {}", e)),
    }
}

/// Search for code patterns in files
pub async fn search_code(pattern: &str, language: Option<&str>, _workspace_path: &str) -> crate::tools::ToolResult {
    // TODO: Implement code search across files
    // For now, return placeholder
    let result = serde_json::json!({
        "pattern": pattern,
        "language": language,
        "matches": [],
        "note": "Code search not yet fully implemented"
    });
    
    crate::tools::ToolResult {
        success: true,
        output: serde_json::to_string_pretty(&result).unwrap_or_default(),
        error: None,
        extra_json: Some(result),
    }
}
