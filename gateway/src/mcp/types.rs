//! MCP (Model Context Protocol) type definitions.
//!
//! Covers JSON-RPC 2.0 envelope, MCP-specific message types (initialize,
//! tools/list, tools/call), and conversion to OpenAI function-calling format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── JSON-RPC 2.0 ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[allow(dead_code)]
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

// ── MCP Initialize ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: Implementation,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientCapabilities {
    // We only need tool execution capability
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: Option<Implementation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolsCapability {
    #[allow(dead_code)]
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

// ── MCP Tool Definitions ───────────────────────────────────────

/// An MCP tool definition as returned by `tools/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(rename = "outputSchema", skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<McpToolDef>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

// ── MCP Tool Call / Result ─────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<McpContent>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        #[serde(rename = "resource")]
        resource: ResourceContent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
}

// ── OpenAI Conversion ──────────────────────────────────────────

/// Prefix used to namespace MCP tools when injected into OpenAI `tools[]`.
/// Format: `mcp__{server_name}__{tool_name}`
pub const MCP_TOOL_PREFIX: &str = "mcp__";

/// Convert an MCP tool definition to OpenAI function-calling format.
///
/// The tool name is namespaced as `mcp__{server_name}__{tool_name}` to:
/// 1. Avoid collisions with agent-supplied tools
/// 2. Enable routing tool_calls back to the correct MCP server
pub fn to_openai_function(server_name: &str, tool: &McpToolDef) -> Value {
    let namespaced = format!("{}{}__{}", MCP_TOOL_PREFIX, server_name, tool.name);

    // Build description — include server name for multi-server clarity
    let desc = tool
        .description
        .as_deref()
        .unwrap_or("No description provided");

    serde_json::json!({
        "type": "function",
        "function": {
            "name": namespaced,
            "description": desc,
            "parameters": tool.input_schema,
        }
    })
}

/// Parse a namespaced MCP tool call name back into (server_name, tool_name).
/// Returns None if the name doesn't match the `mcp__` prefix pattern.
pub fn parse_mcp_tool_name(name: &str) -> Option<(String, String)> {
    let stripped = name.strip_prefix(MCP_TOOL_PREFIX)?;
    let (server, tool) = stripped.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server.to_string(), tool.to_string()))
}

/// Convert an MCP tool result into a text string suitable for an OpenAI
/// tool response message `content` field.
pub fn mcp_result_to_text(result: &CallToolResult) -> String {
    let mut parts = Vec::new();
    for content in &result.content {
        match content {
            McpContent::Text { text } => parts.push(text.clone()),
            McpContent::Image { mime_type, .. } => {
                parts.push(format!("[Image: {}]", mime_type));
            }
            McpContent::Resource { resource } => {
                if let Some(text) = &resource.text {
                    parts.push(text.clone());
                } else {
                    parts.push(format!("[Resource: {}]", resource.uri));
                }
            }
        }
    }
    if result.is_error {
        format!("Error: {}", parts.join("\n"))
    } else {
        parts.join("\n")
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_openai_function() {
        let tool = McpToolDef {
            name: "search".to_string(),
            description: Some("Search the web".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
            output_schema: None,
        };

        let result = to_openai_function("brave", &tool);

        assert_eq!(result["type"], "function");
        assert_eq!(result["function"]["name"], "mcp__brave__search");
        assert_eq!(result["function"]["description"], "Search the web");
        assert_eq!(result["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn test_parse_mcp_tool_name() {
        assert_eq!(
            parse_mcp_tool_name("mcp__brave__search"),
            Some(("brave".to_string(), "search".to_string()))
        );
        assert_eq!(
            parse_mcp_tool_name("mcp__slack__send_message"),
            Some(("slack".to_string(), "send_message".to_string()))
        );
        // Not an MCP tool
        assert_eq!(parse_mcp_tool_name("get_weather"), None);
        // Malformed
        assert_eq!(parse_mcp_tool_name("mcp__"), None);
        assert_eq!(parse_mcp_tool_name("mcp____"), None);
    }

    #[test]
    fn test_mcp_result_to_text() {
        let result = CallToolResult {
            content: vec![
                McpContent::Text { text: "Hello world".into() },
                McpContent::Text { text: "Second line".into() },
            ],
            is_error: false,
        };
        assert_eq!(mcp_result_to_text(&result), "Hello world\nSecond line");

        let err_result = CallToolResult {
            content: vec![McpContent::Text { text: "not found".into() }],
            is_error: true,
        };
        assert_eq!(mcp_result_to_text(&err_result), "Error: not found");
    }

    #[test]
    fn test_mcp_result_image_and_resource() {
        let result = CallToolResult {
            content: vec![
                McpContent::Image {
                    data: "base64data".into(),
                    mime_type: "image/png".into(),
                },
                McpContent::Resource {
                    resource: ResourceContent {
                        uri: "file:///tmp/data.csv".into(),
                        text: Some("col1,col2\n1,2".into()),
                        mime_type: Some("text/csv".into()),
                    },
                },
            ],
            is_error: false,
        };
        let text = mcp_result_to_text(&result);
        assert!(text.contains("[Image: image/png]"));
        assert!(text.contains("col1,col2"));
    }

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest::new(1, "tools/list", None);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["method"], "tools/list");
        assert!(json.get("params").is_none());
    }

    #[test]
    fn test_jsonrpc_request_with_params() {
        let req = JsonRpcRequest::new(
            2,
            "tools/call",
            Some(serde_json::json!({
                "name": "search",
                "arguments": { "query": "rust" }
            })),
        );
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["params"]["name"], "search");
    }

    #[test]
    fn test_list_tools_result_deserialization() {
        let json = serde_json::json!({
            "tools": [
                {
                    "name": "fetch",
                    "description": "Fetch a URL",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string" }
                        },
                        "required": ["url"]
                    }
                },
                {
                    "name": "ping",
                    "inputSchema": { "type": "object" }
                }
            ]
        });

        let result: ListToolsResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.tools.len(), 2);
        assert_eq!(result.tools[0].name, "fetch");
        assert!(result.tools[0].description.is_some());
        assert_eq!(result.tools[1].name, "ping");
        assert!(result.tools[1].description.is_none());
    }

    #[test]
    fn test_call_tool_result_deserialization() {
        let json = serde_json::json!({
            "content": [
                { "type": "text", "text": "Result data here" }
            ],
            "isError": false
        });

        let result: CallToolResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.content.len(), 1);
        assert!(!result.is_error);
        match &result.content[0] {
            McpContent::Text { text } => assert_eq!(text, "Result data here"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_call_tool_error_result() {
        let json = serde_json::json!({
            "content": [
                { "type": "text", "text": "Something went wrong" }
            ],
            "isError": true
        });

        let result: CallToolResult = serde_json::from_value(json).unwrap();
        assert!(result.is_error);
    }
}
