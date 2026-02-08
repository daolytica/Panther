// Smart context builder for workspace understanding

use std::path::Path;
use crate::commands_workspace;

pub struct ContextBuilder;

impl ContextBuilder {
    /// Build context for agent task
    pub async fn build_context(
        workspace_path: &Path,
        target_paths: Option<&[String]>,
    ) -> Result<String, String> {
        let mut context_parts = Vec::new();
        
        // 1. File structure
        context_parts.push("## Workspace Structure\n".to_string());
        let structure = Self::get_file_structure(workspace_path).await?;
        context_parts.push(structure);
        
        // 2. Recent changes (if git available)
        if let Ok(git_status) = Self::get_git_status(workspace_path).await {
            context_parts.push("\n## Recent Changes (Git)\n".to_string());
            context_parts.push(git_status);
        }
        
        // 3. Target files content (if specified)
        if let Some(paths) = target_paths {
            context_parts.push("\n## Target Files\n".to_string());
            for path in paths {
                match commands_workspace::read_workspace_file(path.clone()).await {
                    Ok(content) => {
                        context_parts.push(format!("### {}\n```\n{}\n```\n", path, content));
                    }
                    Err(_) => {
                        context_parts.push(format!("### {} (could not read)\n", path));
                    }
                }
            }
        }
        
        Ok(context_parts.join("\n"))
    }
    
    async fn get_file_structure(workspace_path: &Path) -> Result<String, String> {
        let entries = commands_workspace::list_workspace_files(
            workspace_path.to_str().map(|s| s.to_string())
        ).await
        .map_err(|e| format!("Failed to list files: {}", e))?;
        
        let mut structure = String::new();
        for entry in entries {
            let icon = if entry.is_dir { "📁" } else { "📄" };
            structure.push_str(&format!("{} {}\n", icon, entry.path));
        }
        
        Ok(structure)
    }
    
    async fn get_git_status(workspace_path: &Path) -> Result<String, String> {
        let cwd = workspace_path.to_str().map(|s| s.to_string());
        match commands_workspace::execute_command("git status --short".to_string(), cwd).await {
            Ok(result) => {
                if result.success && !result.stdout.is_empty() {
                    Ok(result.stdout)
                } else {
                    // Git command succeeded but no changes - this is fine
                    Err("No git changes or not a git repo".to_string())
                }
            }
            Err(e) => {
                // Git command failed - log but don't block
                eprintln!("⚠️ Git status check failed (non-fatal): {}", e);
                Err(format!("Git not available: {}", e))
            }
        }
    }
}
