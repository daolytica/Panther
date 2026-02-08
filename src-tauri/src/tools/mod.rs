use crate::commands_workspace;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolRequest {
    Terminal {
        command: String,
        cwd: Option<String>,
    },
    WorkspaceRead {
        path: String,
    },
    WorkspaceWrite {
        path: String,
        content: String,
    },
    Http {
        method: String,
        url: String,
        headers: Option<HashMap<String, String>>,
        body: Option<String>,
    },
    GitStatus {
        cwd: String,
    },
    GitDiff {
        cwd: String,
        paths: Option<Vec<String>>,
    },
    TestCommand {
        cwd: String,
        command: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub extra_json: Option<Value>,
}

impl ToolResult {
    pub fn ok(output: String) -> Self {
        ToolResult {
            success: true,
            output,
            error: None,
            extra_json: None,
        }
    }

    pub fn err(msg: String) -> Self {
        ToolResult {
            success: false,
            output: String::new(),
            error: Some(msg),
            extra_json: None,
        }
    }
}

/// Execute a single tool request in a generic way.
///
/// This is intentionally conservative: filesystem operations are restricted to the
/// workspace APIs in `commands_workspace`, and git / test commands are executed
/// via the same shell abstraction used by the IDE.
pub async fn execute_tool(request: ToolRequest) -> ToolResult {
    match request {
        ToolRequest::Terminal { command, cwd } => {
            match commands_workspace::execute_command(command, cwd).await {
                Ok(result) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert("exit_code".to_string(), Value::from(result.exit_code));
                    ToolResult {
                        success: result.success,
                        output: result.stdout,
                        error: if result.stderr.is_empty() {
                            None
                        } else {
                            Some(result.stderr)
                        },
                        extra_json: Some(Value::Object(extra)),
                    }
                }
                Err(e) => ToolResult::err(e),
            }
        }
        ToolRequest::WorkspaceRead { path } => {
            match commands_workspace::read_workspace_file(path).await {
                Ok(content) => ToolResult::ok(content),
                Err(e) => ToolResult::err(e),
            }
        }
        ToolRequest::WorkspaceWrite { path, content } => {
            match commands_workspace::write_workspace_file(path, content).await {
                Ok(_) => ToolResult::ok("File written successfully".to_string()),
                Err(e) => ToolResult::err(e),
            }
        }
        ToolRequest::Http {
            method,
            url,
            headers,
            body,
        } => {
            let client = reqwest::Client::new();
            let mut req = match method.to_ascii_uppercase().as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                other => {
                    return ToolResult::err(format!("Unsupported HTTP method: {}", other));
                }
            };

            if let Some(hdrs) = headers {
                for (k, v) in hdrs {
                    req = req.header(k, v);
                }
            }

            if let Some(b) = body {
                req = req.body(b);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let text = resp.text().await.unwrap_or_default();
                    let mut extra = serde_json::Map::new();
                    extra.insert("status".to_string(), Value::from(status));
                    ToolResult {
                        success: status < 400,
                        output: text,
                        error: None,
                        extra_json: Some(Value::Object(extra)),
                    }
                }
                Err(e) => ToolResult::err(format!("HTTP error: {}", e)),
            }
        }
        ToolRequest::GitStatus { cwd } => {
            let cmd = "git status --short --branch".to_string();
            match commands_workspace::execute_command(cmd, Some(cwd)).await {
                Ok(result) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert("exit_code".to_string(), Value::from(result.exit_code));
                    ToolResult {
                        success: result.success,
                        output: result.stdout,
                        error: if result.stderr.is_empty() {
                            None
                        } else {
                            Some(result.stderr)
                        },
                        extra_json: Some(Value::Object(extra)),
                    }
                }
                Err(e) => ToolResult::err(e),
            }
        }
        ToolRequest::GitDiff { cwd, paths } => {
            let mut command = "git diff".to_string();
            if let Some(ps) = paths {
                for p in ps {
                    command.push(' ');
                    command.push_str(&p);
                }
            }
            match commands_workspace::execute_command(command, Some(cwd)).await {
                Ok(result) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert("exit_code".to_string(), Value::from(result.exit_code));
                    ToolResult {
                        success: result.success,
                        output: result.stdout,
                        error: if result.stderr.is_empty() {
                            None
                        } else {
                            Some(result.stderr)
                        },
                        extra_json: Some(Value::Object(extra)),
                    }
                }
                Err(e) => ToolResult::err(e),
            }
        }
        ToolRequest::TestCommand { cwd, command } => {
            match commands_workspace::execute_command(command, Some(cwd)).await {
                Ok(result) => {
                    let mut extra = serde_json::Map::new();
                    extra.insert("exit_code".to_string(), Value::from(result.exit_code));
                    ToolResult {
                        success: result.success,
                        output: result.stdout,
                        error: if result.stderr.is_empty() {
                            None
                        } else {
                            Some(result.stderr)
                        },
                        extra_json: Some(Value::Object(extra)),
                    }
                }
                Err(e) => ToolResult::err(e),
            }
        }
    }
}

