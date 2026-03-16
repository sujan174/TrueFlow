# Model Context Protocol (MCP) Integration

TrueFlow includes a built-in MCP client that allows agents to discover and invoke tools from external MCP servers. This enables seamless tool integration without modifying agent code.

---

## Overview

The Model Context Protocol (MCP) is an open standard for connecting AI assistants to external tools and data sources. TrueFlow acts as an MCP client, bridging agents to MCP servers.

**Key Features:**

- Tool discovery and caching
- Automatic tool injection into LLM requests
- Transparent execution with response handling
- OAuth 2.0 authentication support
- Project-isolated tool access

---

## Architecture

```
Agent Request (with X-MCP-Servers header)
    |
    v
TrueFlow Gateway
    |
    +-- MCP Registry (in-memory tool cache)
    |
    +-- Model Router (injects tools into request body)
    |
    v
LLM Provider (returns tool_calls for MCP tools)
    |
    v
MCP Client (executes JSON-RPC calls)
    |
    v
MCP Server (external tool service)
    |
    v
Response returned to agent
```

---

## Quick Start

### 1. Register an MCP Server

```bash
curl -X POST http://localhost:8443/api/v1/mcp/servers \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "brave",
    "endpoint": "http://localhost:3001/mcp",
    "api_key": "optional-api-key"
  }'
```

The gateway performs the MCP `initialize` handshake and caches discovered tools.

### 2. List Available Tools

```bash
curl "http://localhost:8443/api/v1/mcp/servers/{id}/tools" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

Response:

```json
[
  {
    "name": "search",
    "description": "Search the web using Brave Search API",
    "input_schema": {
      "type": "object",
      "properties": {
        "query": { "type": "string", "description": "Search query" }
      },
      "required": ["query"]
    }
  }
]
```

### 3. Use Tools in Requests

Include the `X-MCP-Servers` header with your proxy request:

```bash
curl -X POST http://localhost:8443/v1/chat/completions \
  -H "Authorization: Bearer tf_v1_..." \
  -H "X-MCP-Servers: brave,slack" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {"role": "user", "content": "Search for the latest Rust releases"}
    ]
  }'
```

The gateway automatically:

1. Fetches cached tool schemas from registered servers
2. Injects tools into the request body as `mcp__brave__search`, `mcp__slack__send_message`, etc.
3. Sends the request to the LLM
4. If the LLM calls an MCP tool, executes via JSON-RPC
5. Appends the tool result to the conversation
6. Re-sends to the LLM (up to 10 iterations)

---

## API Reference

### List MCP Servers

`GET /mcp/servers`

Returns all registered MCP servers with their status and tool counts.

### Register MCP Server

`POST /mcp/servers`

```json
{
  "name": "my-server",
  "endpoint": "http://localhost:3001/mcp",
  "api_key": "optional-bearer-token"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Server identifier (alphanumeric, hyphens, underscores) |
| `endpoint` | string | Yes | MCP server HTTP endpoint |
| `api_key` | string | No | Bearer token for authentication |
| "auth" | object | No | OAuth 2.0 configuration |

### Delete MCP Server

`DELETE /mcp/servers/{id}`

Removes the server and clears its cached tools.

### Test Connection

`POST /mcp/servers/test`

```json
{
  "endpoint": "http://localhost:3001/mcp",
  "api_key": "optional"
}
```

Tests connectivity without registering. Returns discovered tools.

### Refresh Tool Cache

`POST /mcp/servers/{id}/refresh`

Re-runs the `initialize` handshake and refreshes cached tool schemas.

### List Cached Tools

`GET /mcp/servers/{id}/tools`

Returns all tools discovered from this server.

### Re-authenticate (OAuth)

`POST /mcp/servers/{id}/reauth`

Re-initiates OAuth 2.0 token exchange for servers with OAuth configuration.

---

## Tool Namespacing

MCP tools are automatically namespaced to prevent collisions:

| MCP Server | Tool Name | Namespaced Name |
|------------|-----------|-----------------|
| `brave` | `search` | `mcp__brave__search` |
| `slack` | `send_message` | `mcp__slack__send_message` |
| `github` | `create_issue` | `mcp__github__create_issue` |

This allows multiple servers to expose tools with the same name without conflict.

---

## Authentication

### Bearer Token

Simple token authentication:

```json
{
  "name": "my-server",
  "endpoint": "https://api.example.com/mcp",
  "api_key": "sk-xxx"
}
```

The gateway sends `Authorization: Bearer sk-xxx` with each request.

### OAuth 2.0

For servers requiring OAuth:

```json
{
  "name": "my-oauth-server",
  "endpoint": "https://api.example.com/mcp",
  "auth": {
    "type": "oauth",
    "client_id": "your-client-id",
    "client_secret": "your-client-secret",
    "token_url": "https://auth.example.com/oauth/token",
    "scopes": ["read", "write"]
  }
}
```

**OAuth Flow:**

1. Gateway performs client credentials grant
2. Stores access token in registry
3. Attaches token to each JSON-RPC request
4. Auto-refreshes when expired

**RFC 9728 Auto-Discovery:**

If the MCP server supports OAuth metadata discovery, the gateway can auto-discover endpoints:

```
GET /.well-known/oauth-authorization-server
```

---

## Execution Flow

### Tool Injection

When `X-MCP-Servers: brave,slack` is present:

1. Gateway looks up `brave` and `slack` in the registry
2. Fetches cached tool schemas
3. Transforms tool definitions to LLM provider format
4. Appends to request body `tools` array:

```json
{
  "model": "gpt-4o",
  "messages": [...],
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "mcp__brave__search",
        "description": "Search the web...",
        "parameters": { ... }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "mcp__slack__send_message",
        "description": "Send a Slack message...",
        "parameters": { ... }
      }
    }
  ]
}
```

### Tool Execution Loop

When the LLM returns `finish_reason: "tool_calls"` for an MCP tool:

1. Parse tool call from LLM response
2. Extract server name from namespaced tool (`mcp__brave__search` -> `brave`)
3. Call MCP server via JSON-RPC 2.0:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search",
    "arguments": { "query": "Rust releases" }
  }
}
```

4. Receive result:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      { "type": "text", "text": "Search results: ..." }
    ]
  }
}
```

5. Append tool result to conversation:

```json
{
  "role": "tool",
  "tool_call_id": "call_123",
  "content": "Search results: ..."
}
```

6. Re-send to LLM
7. Repeat up to 10 iterations

---

## Tool Access Control

### Token-Level Restrictions

Restrict which MCP tools a token can use via the `mcp_allowed_tools` and `mcp_blocked_tools` fields:

```bash
curl -X POST http://localhost:8443/api/v1/tokens \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "limited-agent",
    "credential_id": "...",
    "upstream_url": "...",
    "mcp_allowed_tools": ["mcp__brave__*"],
    "mcp_blocked_tools": ["mcp__brave__admin_*"]
  }'
```

**Matching Rules:**

1. If `mcp_allowed_tools` is non-empty, only listed tools are permitted
2. Tools in `mcp_blocked_tools` are always blocked
3. Glob patterns supported: `mcp__brave__*` matches all Brave tools

### Policy-Based Control

Use the `tool_scope` action for policy-level control:

```json
{
  "name": "restrict-mcp-tools",
  "rules": [
    {
      "when": { "always": true },
      "then": {
        "action": "tool_scope",
        "allowed_tools": ["mcp__brave__search", "mcp__slack__send_message"],
        "default": "deny"
      }
    }
  ]
}
```

---

## Error Handling

### Server Unavailable

If an MCP server is unreachable:

1. Tool call fails
2. Error message returned to LLM
3. LLM can retry or use alternative tools

### Rate Limiting

MCP servers may rate limit. The gateway:

1. Propagates rate limit errors to LLM
2. Does not automatically retry tool calls
3. Logs the failure for debugging

### Timeout

Default timeout: 30 seconds. Configurable per-server (roadmap).

---

## Monitoring

### Audit Logging

MCP tool calls are logged in the audit trail:

```json
{
  "tool_calls": [
    {
      "name": "mcp__brave__search",
      "arguments": { "query": "Rust releases" },
      "result_preview": "Search results: ...",
      "latency_ms": 450
    }
  ]
}
```

### Health Check

Check MCP server status:

```bash
curl "http://localhost:8443/api/v1/mcp/servers" \
  -H "Authorization: Bearer $ADMIN_KEY"
```

Response includes last connection status and tool count.

---

## Best Practices

### 1. Register Servers at Startup

Register MCP servers during deployment, not per-request:

```bash
# In your deployment script
curl -X POST http://localhost:8443/api/v1/mcp/servers -d '{"name": "brave", "endpoint": "..."}'
curl -X POST http://localhost:8443/api/v1/mcp/servers -d '{"name": "slack", "endpoint": "..."}'
```

### 2. Use Descriptive Names

```json
{ "name": "brave-search-prod", "endpoint": "..." }  // Good
{ "name": "mcp1", "endpoint": "..." }               // Bad
```

### 3. Restrict Tool Access

Use `mcp_allowed_tools` to limit exposure:

```json
{
  "mcp_allowed_tools": ["mcp__brave__search"],
  "mcp_blocked_tools": ["mcp__brave__delete_*"]
}
```

### 4. Monitor Tool Usage

Query audit logs for MCP tool patterns:

```bash
curl "http://localhost:8443/api/v1/audit?tool_name=mcp__brave__search"
```

### 5. Handle Failures Gracefully

Design agents to handle tool call failures:

- Provide fallback behavior
- Use multiple tools for redundancy
- Log failures for debugging